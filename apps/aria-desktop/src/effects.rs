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
#[derive(Default)]
pub struct Effects {
    pub simulation: Simulation,
    pub draws: Arc<[DrawItem]>,
    pub pending: Vec<u64>,
    pub selected: Option<u64>,
    pub message: Option<String>,
    cache: BTreeMap<PathBuf, Asset>,
    mocs: crate::object_models::ObjectModels,
    moc_rig: RigConfig,
    last: BTreeMap<u64, Instant>,
    pub audio: crate::effect_audio::Audio,
    time: f32,
    pub paused: bool,
    pub shortcut: aria_core::shortcuts::Shortcut,
    dirty: bool,
    pub save_after: Option<Instant>,
    pub search: String,
}
impl Effects {
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
        self.dirty = true;
        self.pending.clear();
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
        let was = self.simulation.active() || std::mem::take(&mut self.dirty);
        for id in std::mem::take(&mut self.pending) {
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
            let result = self
                .prepare(ctx, state, core, design)
                .and_then(|_| self.simulation.trigger(design));
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
        let active = self.simulation.active();
        if dt > 0.0 && !self.paused && pose != PoseMode::Frozen {
            self.time += dt;
            for sound in self.simulation.tick(dt) {
                if !library.muted {
                    self.audio.play(&sound.path, sound.volume * library.volume);
                }
            }
            for p in &mut self.simulation.particles {
                if p.hit && p.kind == Kind::Spray && !p.pin_checked {
                    p.pin = if let Some(a) = avatar {
                        crate::items::pick_surface(
                            a.model.canvas,
                            &a.model.drawables,
                            egui::vec2(p.target[0], p.target[1]),
                        )
                    } else {
                        Some(Pin::Puppet { point: p.target })
                    };
                    p.pin_checked = true;
                    if p.pin.is_none() {
                        p.kind = Kind::Throw;
                        p.bounce = 0.0;
                    }
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
        let draws = self
            .simulation
            .particles
            .iter()
            .filter_map(|p| {
                let image = match self.cache.get(&p.asset)? {
                    Asset::Image(i) => i.clone(),
                    Asset::Prop(p) => p.image(),
                    Asset::Moc(id) => self.mocs.image(*id)?,
                };
                let (position, size, rotation, opacity) = p.pose();
                let pinned = p.kind == Kind::Spray && p.hit && p.pin.is_some();
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
                Some(DrawItem {
                    tint: egui::Color32::from_rgba_unmultiplied(
                        p.tint[0], p.tint[1], p.tint[2], p.tint[3],
                    ),
                    item,
                    image,
                    anchor,
                    visible: true,
                })
            })
            .collect::<Vec<_>>();
        self.draws = draws.into();
        was || active || self.simulation.active()
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
    ) -> Result<()> {
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
        for path in &design.assets {
            if self.cache.contains_key(path) {
                continue;
            }
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
                    self.moc_rig.items.retain(|i| i.id != id);
                    anyhow::bail!(
                        "Live2D prop could not load. Keep its matching model3.json and atlases together and select Cubism Core in Avatar setup."
                    );
                }
                self.cache.insert(path.clone(), Asset::Moc(id));
            } else if path.to_string_lossy() == "builtin:cube" || is_3d(path) {
                let state = state.context("GPU unavailable for 3D props")?;
                let mesh = if path.to_string_lossy() == "builtin:cube" {
                    crate::mesh_asset::cube()
                } else {
                    crate::mesh_asset::load(path)?
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
                let used: f32 = self
                    .cache
                    .values()
                    .map(|a| match a {
                        Asset::Image(ItemImage::Png(s)) => s.size.x * s.size.y * 4.0,
                        _ => 0.0,
                    })
                    .sum();
                let sprite =
                    if let Some(name) = path.to_str().and_then(|s| s.strip_prefix("builtin:")) {
                        builtin(ctx, state, name)?
                    } else {
                        ensure!(
                            crate::items::is_png(path),
                            "Use PNG, moc3/model3.json, GLB/glTF, VRM, FBX or OBJ assets"
                        );
                        crate::items::load_png(ctx, state, path, used as u64)?
                    };
                ensure!(
                    used + sprite.size.x * sprite.size.y * 4.0 <= 256.0 * 1024.0 * 1024.0,
                    "Active PNG throw assets exceed 256 MiB"
                );
                self.cache
                    .insert(path.clone(), Asset::Image(ItemImage::Png(sprite)));
            }
        }
        Ok(())
    }
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
