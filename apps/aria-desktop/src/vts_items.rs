//! Experimental VTS item-scene adapter. Reads assets only beneath a reviewed root.
use anyhow::{Context, Result, ensure};
use aria_core::{
    items::{Item, Pin},
    vts::{Action, Config},
};
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};
fn n(v: &Value, key: &str, default: f32) -> f32 {
    v[key].as_f64().map_or(default, |v| v as f32)
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
pub fn resolve(
    config: &mut Config,
    base: &Path,
    chosen: Option<&Path>,
    avatar: &crate::live2d::Avatar,
) -> Result<()> {
    let root = chosen.map(Path::to_owned).or_else(|| {
        base.ancestors()
            .find(|p| p.file_name().is_some_and(|n| n == "Live2DModels"))
            .and_then(Path::parent)
            .map(Path::to_owned)
    });
    let Some(root) = root else {
        if config
            .actions
            .iter()
            .any(|h| matches!(h.action, Action::ItemScene { .. }))
        {
            config.notes.push("Choose the VTS StreamingAssets folder to locate ItemScenes and Items, then read this config again. Scene actions remain available for asset repair.".into());
        }
        return Ok(());
    };
    let root = root.canonicalize()?;
    config.assets_root = Some(root.clone());
    let names: BTreeSet<_> = config
        .actions
        .iter()
        .filter_map(|h| {
            if let Action::ItemScene { file, .. } = &h.action {
                Some(file.clone())
            } else {
                None
            }
        })
        .collect();
    for name in names {
        let result = (|| -> Result<Vec<Item>> {
            aria_core::vts::valid_reference(&name)?;
            let path = aria_model::resolve_asset(
                &root,
                &format!(
                    "ItemScenes/{}.itemscene.json",
                    name.trim_end_matches(".itemscene.json")
                ),
            )?;
            let json: Value = serde_json::from_slice(&aria_model::read_bounded(
                &path,
                aria_core::asset_limits::MODEL_JSON,
            )?)?;
            let rows = json["Items"]
                .as_array()
                .context("Scene has no Items array")?;
            ensure!(
                rows.len() <= aria_core::items::MAX_ITEMS,
                "Scene exceeds stage item limit"
            );
            let mut items = Vec::new();
            for (index, row) in rows.iter().enumerate() {
                let filename = s(row, "ItemFileName");
                aria_core::vts::valid_reference(filename)?;
                let mut matches = Vec::new();
                for folder in ["Items", "DragAndDropItems", "Live2DModels"] {
                    if !matches.is_empty() {
                        break;
                    } // VTS Items takes precedence over its staging/cache copies.
                    if !row["IsModel"].as_bool().unwrap_or(false)
                        && row["IsAnimated"].as_bool().unwrap_or(false)
                        && let Ok(p) = root.join(folder).join(filename).canonicalize()
                        && p.starts_with(&root)
                        && p.is_dir()
                    {
                        crate::media::sequence::inspect(&p)?;
                        matches.push(p);
                        continue;
                    }
                    if row["IsModel"].as_bool().unwrap_or(false) {
                        let directory = root.join(folder).join(filename).canonicalize();
                        if let Ok(directory) = directory
                            && directory.starts_with(&root)
                            && directory.is_dir()
                            && let Ok(files) = aria_model::load_files(&directory)
                        {
                            matches.push(files.source);
                            continue;
                        }
                    }
                    if let Ok(p) = crate::vts_panel::asset(&root.join(folder), filename, "") {
                        matches.push(p);
                    }
                }
                matches.sort();
                matches.dedup();
                ensure!(
                    matches.len() == 1,
                    "Item {filename}: {} asset matches; place one uniquely named asset in Items",
                    matches.len()
                );
                let path = matches.remove(0);
                ensure!(
                    crate::items::is_item(&path),
                    "Item {filename} needs the ARIA repair guide to choose local artwork"
                );
                if aria_core::items::is_model(&path) {
                    aria_model::load_files(&path)?;
                } else if !path.is_dir() {
                    let _ = image::ImageReader::open(&path)?
                        .with_guessed_format()?
                        .into_dimensions()?;
                }
                let canvas = avatar.view_canvas();
                let position = [n(&row["Position"], "x", 0.), n(&row["Position"], "y", 0.)];
                let point = crate::items::model_point(canvas, position);
                let mut item = Item {
                    vts_scene: Some(name.clone()),
                    vts_slot: index as u16,
                    animation_fps: row["IsAnimated"]
                        .as_bool()
                        .unwrap_or(false)
                        .then(|| n(row, "FPS", 30.).clamp(1., 240.)),
                    flip: row["IsFlipped"].as_bool().unwrap_or(false),
                    id: index as u64 + 1,
                    name: if s(row, "ItemName").is_empty() {
                        filename.chars().take(80).collect()
                    } else {
                        s(row, "ItemName").chars().take(80).collect()
                    },
                    path,
                    position: [point.x.clamp(-4., 4.), point.y.clamp(-4., 4.)],
                    height: (n(row, "Size", 0.1) * canvas.pixels_per_unit / canvas.size[1])
                        .clamp(0.005, 4.),
                    rotation: (-n(row, "Rotation", 0.)).clamp(-180., 180.),
                    locked: row["IsLocked"].as_bool().unwrap_or(false),
                    behind: n(row, "Order", 0.) < 0.,
                    ..Default::default()
                };
                item.model.animate = row["TrackingModel"].as_bool().unwrap_or(true);
                if row["IsPinned"].as_bool().unwrap_or(false) {
                    let mesh_id = s(row, "PinnedTo");
                    if let Some((mesh, d)) = avatar
                        .model
                        .drawables
                        .iter()
                        .enumerate()
                        .find(|(_, d)| d.id == mesh_id)
                    {
                        let indices: Vec<u16> = s(row, "PinnedVertexIDs")
                            .split_whitespace()
                            .filter_map(|v| v.parse().ok())
                            .collect();
                        if indices.len() >= 3
                            && indices[..3]
                                .iter()
                                .all(|i| usize::from(*i) < d.positions.len())
                        {
                            let vertices = [indices[0], indices[1], indices[2]];
                            let p = vertices.map(|v| {
                                crate::items::model_point(canvas, d.positions[usize::from(v)])
                            });
                            let center = (p[0] + p[1] + p[2]) / 3.;
                            let edge = p[1] - p[0];
                            if edge.length() > 1e-7 {
                                item.pin = Some(Pin::Surface {
                                    mesh,
                                    vertices,
                                    weights: [1. / 3.; 3],
                                    angle: edge.y.atan2(edge.x),
                                    length: edge.length(),
                                });
                                item.position = [
                                    (point.x - center.x).clamp(-4., 4.),
                                    (point.y - center.y).clamp(-4., 4.),
                                ];
                            }
                        }
                    }
                    if item.pin.is_none() {
                        config.notes.push(format!("{filename}: its original pin mesh/vertices are missing. Use Objects → Pick pin to reattach it."));
                    }
                }
                items.push(item);
            }
            aria_core::items::validate(&items)?;
            Ok(items)
        })();
        match result {Ok(items)=>{config.scenes.insert(name,items);},Err(e)=>config.notes.push(format!("Scene {name}: {e:#}. Choose/recover its original assets and reimport; use Repair to choose its files inside ARIA."))}
    }
    Ok(())
}
pub fn toggle(
    config: &mut aria_core::movement::RigConfig,
    scene: &[Item],
    random: bool,
) -> Result<Vec<u64>> {
    let names: BTreeSet<_> = scene
        .iter()
        .map(|i| (&i.path, &i.name, &i.vts_scene, i.vts_slot))
        .collect();
    let active = config
        .items
        .iter()
        .any(|i| i.visible && names.contains(&(&i.path, &i.name, &i.vts_scene, i.vts_slot)));
    if active {
        for i in &mut config.items {
            if names.contains(&(&i.path, &i.name, &i.vts_scene, i.vts_slot)) {
                i.visible = false;
            }
        }
        return Ok(Vec::new());
    }
    let mut next = config.items.clone();
    let mut ids = Vec::new();
    let pick = if random && !scene.is_empty() {
        let mut b = [0u8; 8];
        getrandom::fill(&mut b)
            .map_err(|e| anyhow::anyhow!("Random item selection failed: {e}"))?;
        Some(u64::from_le_bytes(b) as usize % scene.len())
    } else {
        None
    };
    for (index, template) in scene
        .iter()
        .enumerate()
        .filter(|(index, _)| pick.is_none_or(|n| n == *index))
    {
        let _ = index;
        if let Some(i) = next.iter_mut().find(|i| {
            i.path == template.path
                && i.name == template.name
                && i.vts_scene == template.vts_scene
                && i.vts_slot == template.vts_slot
        }) {
            i.visible = true;
            ids.push(i.id);
        } else {
            let mut i = template.clone();
            i.id = next
                .iter()
                .map(|i| i.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .context("Stage item IDs exhausted")?;
            i.visible = true;
            ids.push(i.id);
            next.push(i);
        }
    }
    aria_core::items::validate(&next)?;
    config.items = next;
    Ok(ids)
}
