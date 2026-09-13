//! Transient effect assets and playback. Saved designs remain owned by the main avatar.
use crate::{
    avatar::Sprite,
    items::{Anchor, DrawItem, ItemImage},
};
use anyhow::{Context, Result, ensure};
use aria_core::{
    effects::{Design, Kind, Library, Simulation},
    items::{Item, Pin},
    movement::{PoseMode, RigConfig},
};
use eframe::{egui, egui_wgpu::RenderState};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
enum Asset {
    Image(ItemImage),
    Prop(Box<crate::prop_render::Prop>),
    Moc(u64),
}
struct ImpactDent {
    pin: Pin,
    response: aria_core::deformation::Response,
    direction: [f32; 2],
    age: f32,
    scale: f32,
}
#[derive(Default)]
pub struct Effects {
    impacts: Vec<ImpactDent>,
    pub dents: Arc<[crate::deformation::AvatarDent]>,
    pub editor: Option<Box<crate::effect_editor::Editor>>,
    liquid_art: crate::liquid_art::Cache,
    pub simulation: Simulation,
    pub draws: Arc<[DrawItem]>,
    pub pending: Vec<u64>,
    pub selected: Option<u64>,
    pub message: Option<String>,
    cache: BTreeMap<PathBuf, Asset>,
    image_job: Option<(PathBuf, crate::media::LoadJob)>,
    mocs: crate::object_models::ObjectModels,
    moc_rig: RigConfig,
    last: BTreeMap<u64, Instant>,
    pub audio: crate::effect_audio::Audio,
    time: f32,
    pub paused: bool,
    dirty: bool,
    pub save_after: Option<Instant>,
    pub search: String,
}
impl Effects {
    pub fn process_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.mocs.process_ids()
    }
    #[cfg(feature = "screenshots")]
    pub fn pause_smoke(&mut self, native_avatar: bool) -> bool {
        if self.paused || self.time < 1.3 {
            return false;
        }
        assert!(
            self.draws.len() > 50,
            "Effect particles must render: {:?}",
            self.message
        );
        assert!(
            self.cache
                .values()
                .filter(|a| matches!(a, Asset::Prop(_)))
                .count()
                >= 5,
            "Built-in cube and GLB/glTF/FBX/OBJ must all render: {:?}",
            self.message
        );
        let pins = self
            .simulation
            .particles
            .iter()
            .filter(|p| p.pin.is_some())
            .count();
        assert!(pins > 0, "Sprays must attach to the avatar");
        assert!(
            !self.dents.is_empty() && self.draws.iter().any(|d| d.deformation.is_some()),
            "Avatar and thrown objects must deform"
        );
        assert!(
            self.simulation
                .particles
                .iter()
                .any(|p| p.kind == Kind::Throw && p.stuck),
            "Thrown props must also attach to the avatar"
        );
        if native_avatar {
            assert!(
                self.simulation
                    .particles
                    .iter()
                    .any(|p| matches!(p.pin, Some(Pin::Surface { .. })))
            );
        }
        if std::env::var_os("ARIA_SMOKE_EFFECT_ASSET").is_some() {
            assert!(
                self.cache.values().any(|a| matches!(a, Asset::Moc(_))),
                "Independent moc3 throw must load"
            );
        }
        eprintln!(
            "Effects smoke verified: {} particles, {pins} surface/puppet pins, {} cached asset paths",
            self.draws.len(),
            self.cache.len()
        );
        self.paused = true;
        true
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn clear(&mut self) {
        self.impacts.clear();
        self.dents = Arc::from([]);
        self.liquid_art = Default::default();
        self.dirty = true;
        self.pending.clear();
        self.image_job = None;
        self.simulation.clear();
        self.draws = Arc::from([]);
        self.audio.stop();
    }
    pub fn reload(&mut self) {
        self.clear();
        self.cache.clear();
        self.mocs.clear();
        self.moc_rig.items.clear();
        self.audio.clear();
        self.liquid_art = Default::default();
    }
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        state: Option<&RenderState>,
        core: &Path,
        settings: (&Library, PoseMode),
        avatar: Option<&crate::live2d::Avatar>,
        dt: f32,
    ) -> bool {
        let (library, pose) = settings;
        let was =
            self.simulation.active() || !self.impacts.is_empty() || std::mem::take(&mut self.dirty);
        let mut requests = std::mem::take(&mut self.pending).into_iter();
        while let Some(id) = requests.next() {
            let Some(design) = library.designs.iter().find(|d| d.id == id) else {
                self.message = Some("Effect no longer exists in this avatar's library".into());
                continue;
            };
            if self
                .last
                .get(&id)
                .is_some_and(|t| t.elapsed().as_secs_f32() < design.cooldown)
            {
                self.message = Some("Effect is cooling down".into());
                continue;
            }
            self.message = None;
            let result = match self.prepare(ctx, state, core, design) {
                Ok(false) => {
                    // Finish this import before starting another burst. No particles,
                    // cooldown or sounds start until every requested asset is ready.
                    self.pending.push(id);
                    self.pending.extend(requests);
                    ctx.request_repaint_after(std::time::Duration::from_millis(30));
                    break;
                }
                Ok(true) => self.simulation.trigger(design),
                Err(error) => Err(error),
            };
            match result {
                Ok(()) => {
                    self.last.insert(id, Instant::now());
                    self.message = Some(format!(
                        "Triggered {}{}",
                        design.name,
                        self.message
                            .as_ref()
                            .map(|m| format!("\n{m}"))
                            .unwrap_or_default()
                    ));
                }
                Err(e) => self.message = Some(format!("Cannot trigger {}: {e:#}", design.name)),
            }
        }
        let active = self.simulation.active() || !self.impacts.is_empty();
        if dt > 0.0 && !self.paused && pose != PoseMode::Frozen {
            for impact in &mut self.impacts {
                impact.age += dt.min(0.25);
            }
            self.impacts
                .retain(|impact| impact.age < impact.response.duration());
            self.time += dt;
            for sound in self.simulation.tick(dt) {
                if !library.muted {
                    self.audio.play(&sound.path, sound.volume * library.volume);
                }
            }
            for p in &mut self.simulation.particles {
                if p.hit && !p.pin_checked {
                    let surface = if let Some(a) = avatar {
                        crate::items::pick_surface(
                            a.view_canvas(),
                            &a.model.drawables,
                            egui::vec2(p.target[0], p.target[1]),
                        )
                    } else {
                        Some(Pin::Puppet { point: p.target })
                    };
                    p.pin_checked = true;
                    p.contact = surface.is_some();
                    if let Some(pin) = &surface
                        && p.deformation.avatar.enabled
                    {
                        if self.impacts.len() >= 24 {
                            self.impacts.remove(0);
                        }
                        self.impacts.push(ImpactDent {
                            pin: pin.clone(),
                            response: p.deformation.avatar.clone(),
                            direction: aria_core::deformation::direction(p.origin, p.target),
                            age: (p.age - p.flight).max(0.0),
                            scale: p.impact_scale(),
                        });
                    }
                    if surface.is_none() {
                        p.bounce = 0.0;
                    }
                    p.pin = if p.wants_stick { surface } else { None };
                    p.stuck = p.pin.is_some();
                }
            }
            let used: BTreeSet<_> = self
                .simulation
                .particles
                .iter()
                .map(|p| p.asset.clone())
                .collect();
            for path in used {
                if let Some(Asset::Prop(p)) = self.cache.get_mut(&path) {
                    p.render(self.time);
                }
            }
        }
        if library.muted || self.paused || pose == PoseMode::Frozen {
            self.audio.stop();
        }
        self.dents = self
            .impacts
            .iter()
            .map(|impact| crate::deformation::AvatarDent {
                anchor: crate::items::anchor(Some(&impact.pin), avatar),
                field: aria_core::deformation::Field::new(
                    [0.0; 2],
                    impact.direction,
                    &impact.response,
                    impact.response.gain(impact.age) * impact.scale,
                ),
            })
            .collect::<Vec<_>>()
            .into();
        let mut draws = Vec::new();
        for p in &self.simulation.particles {
            if p.kind == Kind::Spray && p.asset.to_string_lossy() == "builtin:drop" {
                self.liquid_art.prepare(ctx, state, p);
            }
        }
        let base_draws = self
            .simulation
            .particles
            .iter()
            .filter_map(|p| {
                let mut image = match self.cache.get(&p.asset)? {
                    Asset::Image(ItemImage::Png(s)) => ItemImage::Png(s.at(p.age, 1.0, true)),
                    Asset::Image(i) => i.clone(),
                    Asset::Prop(p) => p.image(),
                    Asset::Moc(id) => self.mocs.image(*id)?,
                };
                let (position, mut size, mut rotation, opacity) = p.pose();
                let material = self.liquid_art.get(p);
                if let Some(art) = material {
                    let mut sprite = if p.stuck {
                        art.splat.clone()
                    } else {
                        art.drop.clone()
                    };
                    if !p.hit {
                        rotation = (p.target[1] - p.origin[1])
                            .atan2(p.target[0] - p.origin[0])
                            .to_degrees()
                            - 90.0;
                        let stretch = 1.0 + p.liquid.trail * 1.5;
                        size *= stretch;
                        sprite.size.x /= stretch;
                    }
                    image = ItemImage::Png(sprite);
                }
                let pinned = p.stuck && p.pin.is_some();
                let anchor = if pinned {
                    crate::items::anchor(p.pin.as_ref(), avatar)
                } else {
                    Anchor::Free
                };
                let item = Item {
                    position: if pinned {
                        [0.0, position[1] - p.target[1]]
                    } else {
                        position
                    },
                    height: size.clamp(0.005, 4.0),
                    rotation: rotation.rem_euclid(360.0),
                    opacity,
                    pin: p.pin.clone(),
                    follow_scale: pinned,
                    ..Default::default()
                };
                Some((
                    p,
                    DrawItem {
                        deformation: crate::deformation::ObjectWarp::from_particle(p),
                        tint: if material.is_some() {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::from_rgba_unmultiplied(
                                p.tint[0], p.tint[1], p.tint[2], p.tint[3],
                            )
                        },
                        item,
                        image,
                        anchor,
                        visible: true,
                    },
                ))
            })
            .collect::<Vec<_>>();
        for (p, draw) in &base_draws {
            if let Some(art) = self.liquid_art.get(p) {
                let after = p.age - p.flight;
                if p.stuck && (0.0..0.45).contains(&after) {
                    let mut ring = draw.clone();
                    ring.image = ItemImage::Png(art.crown.clone());
                    ring.deformation = None;
                    ring.item.height *= 1.2 + after * 4.0;
                    ring.item.opacity *= (1.0 - after / 0.45) * p.liquid.foam;
                    draws.push(ring);
                }
            }
        }
        draws.extend(base_draws.into_iter().map(|(_, draw)| draw));
        self.draws = draws.into();
        was || active || self.simulation.active() || !self.pending.is_empty()
    }
    pub fn take_save(&mut self) -> bool {
        if self
            .save_after
            .is_some_and(|t| t.elapsed().as_millis() >= 500)
        {
            self.save_after = None;
            true
        } else {
            false
        }
    }
    fn prepare(
        &mut self,
        ctx: &egui::Context,
        state: Option<&RenderState>,
        core: &Path,
        design: &Design,
    ) -> Result<bool> {
        design.validate()?;
        // Retain only cached assets still participating in playback or this next burst.
        let keep: BTreeSet<_> = self
            .simulation
            .asset_paths()
            .into_iter()
            .chain(design.assets.iter().cloned())
            .collect();
        self.cache.retain(|p, _| keep.contains(p));
        self.moc_rig.items.retain(|i| keep.contains(&i.path));
        self.mocs.sync(state, core, &self.moc_rig);
        for (i, path) in design.assets.iter().enumerate() {
            if design.asset_counts.get(i) == Some(&0) {
                continue;
            }
            if self.cache.contains_key(path) {
                continue;
            }
            validate_asset_path(path)?;
            if aria_core::items::is_model(path) {
                ensure!(
                    self.moc_rig.items.len() < 4,
                    "At most four different Live2D throw assets can be loaded at once; wait for other throws to finish"
                );
                let id = self.moc_rig.items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
                self.moc_rig.items.push(Item {
                    id,
                    path: path.clone(),
                    ..Default::default()
                });
                self.mocs.sync(state, core, &self.moc_rig);
                self.mocs
                    .update(&mut self.moc_rig, &Default::default(), 0.016);
                if self.mocs.image(id).is_none() {
                    let reason = self
                        .mocs
                        .error(id)
                        .unwrap_or("No rendered image")
                        .to_owned();
                    self.moc_rig.items.retain(|i| i.id != id);
                    anyhow::bail!(
                        "Cannot load Live2D throw asset {}: {reason}",
                        path.display()
                    );
                }
                self.cache.insert(path.clone(), Asset::Moc(id));
            } else if path.to_string_lossy() == "builtin:cube" || is_3d(path) {
                let state = state.context("GPU unavailable for 3D props")?;
                let mesh = if path.to_string_lossy() == "builtin:cube" {
                    crate::mesh_asset::cube()
                } else {
                    crate::mesh_asset::load(path)
                        .with_context(|| format!("Cannot load 3D throw asset {}", path.display()))?
                };
                let used: u64 = self
                    .cache
                    .values()
                    .map(|a| match a {
                        Asset::Prop(p) => p.bytes,
                        _ => 0,
                    })
                    .sum();
                let estimate = mesh
                    .parts
                    .iter()
                    .map(|p| {
                        p.vertices.len() as u64 * 48
                            + p.image.as_ref().map_or(0, |i| i.len() as u64)
                    })
                    .sum::<u64>()
                    + 512 * 512 * 8;
                ensure!(
                    used.saturating_add(estimate) <= 512 * 1024 * 1024,
                    "Active 3D prop memory exceeds 512 MiB; use smaller assets"
                );
                if !mesh.warnings.is_empty() {
                    self.message = Some(mesh.warnings.join("\n"));
                }
                let mut prop = crate::prop_render::Prop::new(state, mesh);
                prop.render(self.time);
                self.cache.insert(path.clone(), Asset::Prop(Box::new(prop)));
            } else {
                let used: u64 = self
                    .cache
                    .values()
                    .map(|a| match a {
                        Asset::Image(ItemImage::Png(s)) => s.bytes(),
                        _ => 0,
                    })
                    .sum();
                let sprite = if let Some(name) =
                    path.to_str().and_then(|s| s.strip_prefix("builtin:"))
                {
                    builtin(ctx, state, name)?
                } else {
                    if self
                        .image_job
                        .as_ref()
                        .is_none_or(|(source, _)| source != path)
                    {
                        self.image_job = Some((
                            path.clone(),
                            crate::media::LoadJob::start(
                                ctx,
                                state,
                                path,
                                used,
                                aria_core::asset_limits::GIF_PLAYBACK_MIB,
                            )?,
                        ));
                    }
                    let job = &self.image_job.as_ref().unwrap().1;
                    match job.poll() {
                        Some(result) => {
                            self.image_job = None;
                            result.map_err(anyhow::Error::msg).with_context(|| {
                                format!("Cannot load throw asset {}", path.display())
                            })?
                        }
                        None => {
                            let (done, total) = job.progress();
                            self.message = Some(format!(
                                "Loading {} · {done}/{total} frames. The burst starts when all assets are ready; Clear cancels.",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ));
                            return Ok(false);
                        }
                    }
                };
                ensure!(
                    used.saturating_add(sprite.bytes())
                        <= aria_core::asset_limits::IMAGE_COLLECTION,
                    "Active PNG/GIF throw assets exceed 2560 MiB"
                );
                self.cache
                    .insert(path.clone(), Asset::Image(ItemImage::Png(sprite)));
            }
        }
        Ok(true)
    }
}
pub fn validate_asset_path(path: &Path) -> Result<()> {
    if let Some(name) = path.to_str().and_then(|s| s.strip_prefix("builtin:")) {
        ensure!(
            ["star", "ball", "drop", "cube"].contains(&name),
            "Unknown built-in asset {name}"
        );
        return Ok(());
    }
    ensure!(
        crate::items::is_png(path) || aria_core::items::is_model(path) || is_3d(path),
        "{}: choose PNG/GIF, moc3/model3.json, GLB/glTF, VRM, FBX or OBJ. Other JSON files are settings, not visual assets.",
        path.display()
    );
    ensure!(
        path.is_file(),
        "{} is missing or not available locally. Keep downloaded/cloud artwork on this device, then choose it again.",
        path.display()
    );
    Ok(())
}
pub fn is_3d(path: &Path) -> bool {
    path.extension().is_some_and(|e| {
        ["glb", "gltf", "vrm", "fbx", "obj"]
            .iter()
            .any(|s| e.eq_ignore_ascii_case(s))
    })
}
fn builtin(ctx: &egui::Context, state: Option<&RenderState>, name: &str) -> Result<Sprite> {
    ensure!(
        ["star", "ball", "drop"].contains(&name),
        "Unknown built-in effect asset"
    );
    let size = 128;
    let mut rgba = vec![0_u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let angle = dy.atan2(dx);
            let radius = match name {
                "star" => 0.48 + 0.33 * (angle * 5.0 - std::f32::consts::FRAC_PI_2).cos(),
                "drop" => 0.68 * (1.0 + dy * 0.22),
                _ => 0.8,
            };
            let distance = (dx * dx + dy * dy).sqrt();
            let alpha = ((radius - distance) * 64.0).clamp(0.0, 1.0);
            let color = match name {
                "star" => [255, 212, 91],
                "ball" => {
                    if dx * dy > 0.0 {
                        [115, 231, 209]
                    } else {
                        [234, 249, 245]
                    }
                }
                _ => [255; 3],
            };
            let i = (y * size + x) * 4;
            rgba[i..i + 3].copy_from_slice(&color);
            rgba[i + 3] = (alpha * 255.0) as u8;
        }
    }
    let mut palette = crate::chroma::Palette::default();
    palette.add_rgba(&rgba);
    let pixels = egui::ColorImage::from_rgba_unmultiplied([size, size], &rgba);
    let texture = ctx.load_texture(
        format!("effect:{name}"),
        pixels.clone(),
        egui::TextureOptions::LINEAR,
    );
    if let Some(state) = state {
        state.renderer.write().update_texture(
            &state.device,
            &state.queue,
            texture.id(),
            &egui::epaint::ImageDelta::full(pixels, egui::TextureOptions::LINEAR),
        );
    }
    Ok(Sprite {
        animation: None,
        texture,
        name: name.into(),
        size: egui::vec2(size as f32, size as f32),
        model_key: format!("builtin:{name}"),
        palette: Arc::new(palette),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    #[ignore = "requires a DX12 GPU; optional ARIA_TEST_MODEL and ARIA_CUBISM_CORE exercise an independent Live2D throw"]
    fn custom_assets_render_on_demo_with_native_gpu_and_optional_cubism_host() {
        let state = crate::spout::tests::gpu_state();
        let ctx = egui::Context::default();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates");
        let mut assets: Vec<PathBuf> = [
            "effects/assets/star.png",
            "images/artwork/excited.gif",
            "effects/assets/cube.glb",
            "effects/assets/cube.gltf",
            "effects/assets/cube.obj",
            "effects/assets/cube.fbx",
        ]
        .iter()
        .map(|p| root.join(p))
        .collect();
        if let Some(model) = std::env::var_os("ARIA_TEST_MODEL") {
            assets.push(model.into());
        }
        let core = std::env::var_os("ARIA_CUBISM_CORE")
            .map(PathBuf::from)
            .unwrap_or_default();
        let library = Library {
            designs: vec![Design {
                asset_counts: vec![1; assets.len()],
                assets: assets.clone(),
                interval: 0.0,
                cooldown: 0.0,
                lifetime: 30.0,
                ..Default::default()
            }],
            muted: true,
            ..Default::default()
        };
        let mut effects = Effects::default();
        effects.pending.push(1);
        let start = Instant::now();
        while effects.draws.len() < assets.len() {
            effects.update(
                &ctx,
                Some(&state),
                &core,
                (&library, PoseMode::Live),
                None,
                0.001,
            );
            assert!(start.elapsed().as_secs() < 45, "{:?}", effects.message);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        for asset in &assets {
            assert_eq!(
                effects
                    .simulation
                    .particles
                    .iter()
                    .filter(|p| &p.asset == asset)
                    .count(),
                1
            );
            assert!(effects.cache.contains_key(asset));
        }
        assert!(effects.draws.iter().all(|d| match &d.image {
            ItemImage::Png(sprite) => sprite.size.x > 0.0,
            ItemImage::Model { image, .. } => image.size.x > 0.0,
        }));
        eprintln!(
            "Verified {} custom PNG/GIF/3D/Live2D throw assets on the demo avatar",
            assets.len()
        );
    }
    #[test]
    fn custom_png_gif_burst_waits_for_loading_renders_exact_counts_and_retries_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("Custom ' item.PNG");
        image::RgbaImage::from_pixel(16, 24, image::Rgba([255, 90, 30, 255]))
            .save_with_format(&png, image::ImageFormat::Png)
            .unwrap();
        let gif = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../templates/images/artwork/excited.gif");
        let library = Library {
            designs: vec![Design {
                assets: vec![png.clone(), gif.clone()],
                asset_counts: vec![2, 3],
                interval: 0.0,
                cooldown: 0.0,
                ..Default::default()
            }],
            muted: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut effects = Effects::default();
        effects.pending.push(1);
        let start = Instant::now();
        effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Live),
            None,
            0.0,
        );
        assert!(
            effects.simulation.particles.is_empty(),
            "Do not launch part of a burst during import"
        );
        while effects.draws.len() < 5 {
            effects.update(
                &ctx,
                None,
                Path::new(""),
                (&library, PoseMode::Live),
                None,
                0.001,
            );
            assert!(start.elapsed().as_secs() < 15, "{:?}", effects.message);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(
            effects
                .simulation
                .particles
                .iter()
                .filter(|p| p.asset == png)
                .count(),
            2
        );
        assert_eq!(
            effects
                .simulation
                .particles
                .iter()
                .filter(|p| p.asset == gif)
                .count(),
            3
        );
        assert!(
            matches!(effects.cache.get(&gif), Some(Asset::Image(ItemImage::Png(sprite))) if sprite.animation.is_some())
        );
        effects.reload();
        std::fs::write(&png, b"not a png").unwrap();
        effects.pending.push(1);
        let start = Instant::now();
        loop {
            effects.update(
                &ctx,
                None,
                Path::new(""),
                (&library, PoseMode::Live),
                None,
                0.0,
            );
            if effects.pending.is_empty() {
                break;
            }
            assert!(start.elapsed().as_secs() < 15);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(effects.draws.is_empty());
        assert!(
            effects
                .message
                .as_deref()
                .unwrap()
                .contains("Custom ' item.PNG")
        );
        image::RgbaImage::from_pixel(16, 24, image::Rgba([255, 90, 30, 255]))
            .save_with_format(&png, image::ImageFormat::Png)
            .unwrap();
        effects.pending.push(1);
        let start = Instant::now();
        while effects.draws.len() < 5 {
            effects.update(
                &ctx,
                None,
                Path::new(""),
                (&library, PoseMode::Live),
                None,
                0.001,
            );
            assert!(start.elapsed().as_secs() < 15, "{:?}", effects.message);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        effects.clear();
        assert!(
            effects.pending.is_empty() && effects.image_job.is_none() && effects.draws.is_empty()
        );
    }
    #[test]
    fn dents_outlive_particles_freeze_and_clear_without_touching_shared_art() {
        let ctx = egui::Context::default();
        let mut effects = Effects::default();
        let mut deformation = aria_core::deformation::Settings::gentle();
        deformation.avatar.hold = 0.4;
        deformation.avatar.recovery = 0.6;
        let library = Library {
            designs: vec![Design {
                count: 2,
                interval: 0.2,
                flight: 0.1,
                lifetime: 0.2,
                deformation,
                ..Default::default()
            }],
            muted: true,
            ..Default::default()
        };
        effects.pending.push(1);
        let update = |e: &mut Effects, dt, pose| {
            e.update(&ctx, None, Path::new(""), (&library, pose), None, dt)
        };
        update(&mut effects, 0.15, PoseMode::Live);
        assert_eq!(effects.dents.len(), 1);
        assert!(effects.draws[0].deformation.is_some());
        let texture = effects.draws[0].image.id();
        let held = effects.dents.clone();
        let age = effects.impacts[0].age;
        update(&mut effects, 1.0, PoseMode::Frozen);
        assert_eq!(effects.dents, held);
        assert_eq!(effects.impacts[0].age, age);
        update(&mut effects, 0.2, PoseMode::Live);
        assert_eq!(effects.draws[0].image.id(), texture);
        update(&mut effects, 0.25, PoseMode::Live);
        assert!(effects.simulation.particles.is_empty());
        assert!(!effects.dents.is_empty());
        for _ in 0..6 {
            update(&mut effects, 0.25, PoseMode::Live);
        }
        assert!(effects.dents.is_empty());
        effects.clear();
        assert!(effects.dents.is_empty() && effects.impacts.is_empty());
    }
    #[test]
    fn deformation_is_opt_in_for_legacy_designs_and_overlap_is_bounded() {
        let ctx = egui::Context::default();
        let mut effects = Effects::default();
        let mut library = Library {
            designs: vec![Design {
                count: 50,
                interval: 0.0,
                flight: 0.1,
                ..Default::default()
            }],
            muted: true,
            ..Default::default()
        };
        effects.pending.push(1);
        effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Live),
            None,
            0.2,
        );
        assert!(effects.dents.is_empty() && effects.draws.iter().all(|d| d.deformation.is_none()));
        effects.reset();
        library.designs[0].deformation = aria_core::deformation::Settings::gentle();
        effects.pending.push(1);
        effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Live),
            None,
            0.2,
        );
        assert_eq!(effects.impacts.len(), 24);
        assert!(effects.draws.iter().all(|d| d.deformation.is_some()));
    }
    #[test]
    fn zero_quantity_skips_missing_files_and_throw_assets_attach() {
        let ctx = egui::Context::default();
        let mut effects = Effects::default();
        let library = Library {
            designs: vec![Design {
                assets: vec!["builtin:star".into(), "missing.png".into()],
                asset_counts: vec![2, 0],
                stickiness: Some(1.0),
                interval: 0.0,
                flight: 0.1,
                ..Default::default()
            }],
            muted: true,
            ..Default::default()
        };
        effects.pending.push(1);
        effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Live),
            None,
            0.2,
        );
        assert_eq!(
            effects.simulation.particles.len(),
            2,
            "{:?}",
            effects.message
        );
        assert!(
            effects
                .simulation
                .particles
                .iter()
                .all(|p| p.stuck && p.pin.is_some())
        );
        assert_eq!(effects.draws.len(), 2);
    }
    #[test]
    fn freeze_holds_particles_and_clear_invalidates_a_static_output() {
        let ctx = egui::Context::default();
        let mut effects = Effects::default();
        let library = Library {
            muted: true,
            ..Default::default()
        };
        effects.pending.push(1);
        assert!(effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Live),
            None,
            0.1
        ));
        assert!(!effects.draws.is_empty());
        let ages: Vec<_> = effects.simulation.particles.iter().map(|p| p.age).collect();
        effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Frozen),
            None,
            1.0,
        );
        assert_eq!(
            ages,
            effects
                .simulation
                .particles
                .iter()
                .map(|p| p.age)
                .collect::<Vec<_>>()
        );
        effects.clear();
        assert!(effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Frozen),
            None,
            0.0
        ));
        assert!(effects.draws.is_empty());
        assert_eq!(effects.simulation.impulse, [0.0; 2]);
        assert!(!effects.update(
            &ctx,
            None,
            Path::new(""),
            (&library, PoseMode::Frozen),
            None,
            0.0
        ));
    }
}
