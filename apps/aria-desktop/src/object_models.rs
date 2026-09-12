//! Independent Cubism instances for stage objects. Never reads or replaces the main rig.
use crate::{items::ItemImage, live2d::Avatar};
use anyhow::{Result, ensure};
use aria_core::{
    items::{Item, is_model},
    movement::{PoseMode, RigConfig},
    rig::Inputs,
};
use eframe::{egui, egui_wgpu::RenderState};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const ATLAS_BUDGET: u64 = 1024 * 1024 * 1024;
#[derive(PartialEq, Eq)]
struct Source {
    path: PathBuf,
    textures: Vec<PathBuf>,
    core: PathBuf,
}
struct Entry {
    source: Source,
    loaded: Result<Loaded, String>,
}
struct Loaded {
    avatar: Avatar,
    rig: RigConfig,
    expressions: crate::expressions_panel::ExpressionsPanel,
    revision: u64,
    error: Option<String>,
}
#[derive(Default)]
pub struct ObjectModels {
    entries: BTreeMap<u64, Entry>,
    draft_source: Option<(u64, PathBuf)>,
    textures: Vec<PathBuf>,
    search: String,
}
impl ObjectModels {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.draft_source = None;
    }
    pub fn reset_draft(&mut self) {
        self.draft_source = None;
    }
    pub fn sync(&mut self, state: Option<&RenderState>, core: &Path, config: &RigConfig) {
        self.entries.retain(|id, _| {
            config
                .items
                .iter()
                .any(|i| i.id == *id && is_model(&i.path))
        });
        for item in config.items.iter().filter(|i| is_model(&i.path)) {
            let source = Source {
                path: item.path.clone(),
                textures: item.model.textures.clone(),
                core: core.to_owned(),
            };
            if self
                .entries
                .get(&item.id)
                .is_some_and(|e| e.source == source)
            {
                continue;
            }
            self.entries.remove(&item.id);
            let used: u64 = self
                .entries
                .values()
                .filter_map(|e| e.loaded.as_ref().ok())
                .map(|l| (l.avatar.atlas_mib() * 1048576.0) as u64)
                .sum();
            let loaded = (|| -> Result<Loaded> {
                ensure!(self.entries.len() < aria_core::items::MAX_MODEL_ITEMS, "Maximum four Live2D objects");
                let state = state.ok_or_else(|| anyhow::anyhow!("GPU renderer unavailable"))?;
                ensure!(core.is_file(), "Choose the Cubism Core x64 DLL under Avatar & appearance → Cubism runtime setup, then retry");
                let files = object_files(item)?;
                let mut estimate = 0_u64;
                for path in &files.textures {
                    let (w,h) = image::image_dimensions(path)?;
                    estimate += u64::from(w)*u64::from(h)*4;
                    ensure!(estimate.saturating_add(used) <= ATLAS_BUDGET, "Live2D object atlases exceed the combined 1 GiB budget; use smaller atlases or remove another object");
                }
                let avatar = Avatar::load(state, core, files)?;
                ensure!((avatar.atlas_mib()*1048576.0) as u64 + used <= ATLAS_BUDGET, "Live2D object atlas budget exceeded");
                let rig = avatar.initial_config.clone();
                Ok(Loaded { avatar,rig,expressions:Default::default(),revision:0,error:None })
            })().map_err(|e| format!("{e:#}"));
            self.entries.insert(item.id, Entry { source, loaded });
        }
    }
    pub fn update(&mut self, config: &mut RigConfig, inputs: &Inputs, dt: f32) {
        let frozen = config.pose.mode == PoseMode::Frozen;
        for item in &mut config.items {
            let Some(loaded) = self
                .entries
                .get_mut(&item.id)
                .and_then(|e| e.loaded.as_mut().ok())
            else {
                continue;
            };
            if loaded.error.is_some() {
                continue;
            }
            configure_rig(&mut loaded.rig, item, frozen);
            match loaded
                .avatar
                .update(inputs, &mut loaded.rig, &mut loaded.expressions, dt)
            {
                Ok(changed) => {
                    if changed {
                        loaded.revision = loaded.revision.wrapping_add(1);
                    }
                    if changed || item.model.snapshot.is_empty() {
                        for p in loaded.avatar.model.parameters() {
                            item.model
                                .snapshot
                                .entry(p.id.clone())
                                .and_modify(|v| *v = p.value)
                                .or_insert(p.value);
                        }
                    }
                }
                Err(e) => {
                    loaded.error = Some(format!(
                        "Object update stopped: {e:#}. Reload this object to retry."
                    ))
                }
            }
        }
    }
    pub fn image(&self, id: u64) -> Option<ItemImage> {
        let loaded = self.entries.get(&id)?.loaded.as_ref().ok()?;
        if loaded.error.is_some() {
            return None;
        }
        Some(ItemImage::Model {
            image: loaded.avatar.image(),
            _lease: loaded.avatar.image_lease(),
            revision: loaded.revision,
        })
    }
    pub fn merge_palette(&self, palette: &mut crate::chroma::Palette) {
        for loaded in self.entries.values().filter_map(|e| e.loaded.as_ref().ok()) {
            if let Ok(colors) = loaded.avatar.key_palette() {
                palette.merge(&colors);
            }
        }
    }
    pub fn panel(&mut self, ui: &mut egui::Ui, item: &mut Item) {
        if !is_model(&item.path) {
            return;
        }
        if self
            .draft_source
            .as_ref()
            .is_none_or(|(id, path)| *id != item.id || *path != item.path)
        {
            self.draft_source = Some((item.id, item.path.clone()));
            self.textures = item.model.textures.clone();
            self.search.clear();
            #[cfg(feature = "screenshots")]
            if crate::smoke_mode()
                && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("object-controls")
            {
                self.search = "ParamAngleX".into();
            }
        }
        crate::theme::category(ui, "object-runtime", "Live2D object", true, |ui| {
            crate::help::label(ui, "Independent model instance", "model-items");
            crate::theme::caption(
                ui,
                "This object's parameters and movement are separate from your main avatar. Pinning moves its whole canvas with the selected surface.",
            );
            if let Some(entry) = self.entries.get(&item.id) {
                match &entry.loaded {
                    Err(e) => {
                        ui.colored_label(egui::Color32::LIGHT_RED, e);
                    }
                    Ok(loaded) => {
                        ui.small(format!(
                            "{} parameters · {:.1} MiB atlases",
                            loaded.avatar.model.parameters().len(),
                            loaded.avatar.atlas_mib()
                        ));
                        if let Some(e) = &loaded.error {
                            ui.colored_label(egui::Color32::LIGHT_RED, e);
                        }
                        for warning in &loaded.avatar.files.warnings {
                            ui.small(warning);
                        }
                    }
                }
            }
            if crate::help::control(ui, "model-items", |ui| ui.button("Reload this object"))
                .clicked()
            {
                self.entries.remove(&item.id);
            }
            crate::help::control(ui, "model-items", |ui| {
                ui.checkbox(&mut item.model.animate, "Animate object from tracking")
            });
            ui.add_enabled_ui(item.model.animate, |ui| {
                crate::help::control(ui, "model-items", |ui| {
                    ui.checkbox(&mut item.model.physics, "Use object's physics")
                });
            });
            crate::theme::caption(
                ui,
                "Off keeps a static pose that can still follow its pin. On uses this object's own tracking mappings and optional physics. Freeze pose pauses both the main avatar and its objects.",
            );
            if item
                .path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("moc3"))
            {
                crate::theme::category(ui, "object-atlases", "Object texture setup", false, |ui| {
                    crate::help::label(ui, "Bare moc3: supply atlas index order", "model-items");
                    crate::theme::caption(
                        ui,
                        "Keep the matching model3.json beside the moc3 for automatic loading. Otherwise add the PNG atlases here in index order. These are this object's textures, not new PNG accessories.",
                    );
                    if crate::help::control(ui, "model-items", |ui| ui.button("Add atlas PNGs…"))
                        .clicked()
                        && let Some(paths) = rfd::FileDialog::new()
                            .add_filter("Texture atlases", &["png"])
                            .pick_files()
                    {
                        self.textures.extend(
                            paths
                                .into_iter()
                                .take(32_usize.saturating_sub(self.textures.len())),
                        );
                    }
                    let mut swap = None;
                    let mut remove = None;
                    for (n, path) in self.textures.iter().enumerate() {
                        ui.push_id(n, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!(
                                    "{n}: {}",
                                    path.file_name().unwrap_or_default().to_string_lossy()
                                ))
                                .on_hover_text(path.display().to_string());
                                if ui.add_enabled(n > 0, egui::Button::new("Up")).clicked() {
                                    swap = Some((n, n - 1));
                                }
                                if ui
                                    .add_enabled(
                                        n + 1 < self.textures.len(),
                                        egui::Button::new("Down"),
                                    )
                                    .clicked()
                                {
                                    swap = Some((n, n + 1));
                                }
                                if ui.button("Remove").clicked() {
                                    remove = Some(n);
                                }
                                crate::help::button(ui, "model-items");
                            });
                        });
                    }
                    if let Some((a, b)) = swap {
                        self.textures.swap(a, b);
                    }
                    if let Some(n) = remove {
                        self.textures.remove(n);
                    }
                    if crate::help::control(ui, "model-items", |ui| {
                        ui.add_enabled(
                            !self.textures.is_empty(),
                            egui::Button::new("Apply texture order"),
                        )
                    })
                    .clicked()
                    {
                        item.model.textures = self.textures.clone();
                        self.entries.remove(&item.id);
                    }
                    if crate::help::control(ui, "model-items", |ui| {
                        ui.button("Use matching manifest instead")
                    })
                    .clicked()
                    {
                        self.textures.clear();
                        item.model.textures.clear();
                        self.entries.remove(&item.id);
                    }
                });
            }
            if let Some(loaded) = self
                .entries
                .get(&item.id)
                .and_then(|e| e.loaded.as_ref().ok())
            {
                crate::theme::category(ui, "object-parameters", "Object parameters", false, |ui| {
                    crate::help::control(ui, "model-items", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Find this object's parameter…")
                                .desired_width(f32::INFINITY),
                        )
                    });
                    if crate::help::control(ui, "model-items", |ui| {
                        ui.button("Reset object parameters")
                    })
                    .clicked()
                    {
                        item.model.parameters.clear();
                        item.model.snapshot.clear();
                    }
                    crate::theme::caption(
                        ui,
                        "Override holds only this object's parameter. Values use its authored limits. Uncheck to use its default or tracking mapping.",
                    );
                    let query = self.search.to_lowercase();
                    egui::ScrollArea::vertical().id_salt("object-values").max_height(280.0).show(ui,|ui| {
                        for p in loaded.avatar.model.parameters().iter().filter(|p| p.id.to_lowercase().contains(&query) || loaded.avatar.labels.get(&p.id).is_some_and(|s| s.to_lowercase().contains(&query))) {
                            ui.push_id(&p.id,|ui| {
                                crate::help::context_button(ui,"model-items",|| format!("Object: {}\nParameter: {}\nMinimum: {}\nMaximum: {}\nDefault: {}\nCurrent: {}",item.name,p.id,p.min,p.max,p.default,p.value));
                                ui.label(loaded.avatar.labels.get(&p.id).unwrap_or(&p.id));
                                ui.small(&p.id);
                                let mut hold = item.model.parameters.contains_key(&p.id);
                                let mut value = item.model.parameters.get(&p.id).copied().unwrap_or(p.value);
                                ui.horizontal(|ui| {
                                    if ui.checkbox(&mut hold,"Override").changed() {
                                        if hold { item.model.parameters.insert(p.id.clone(),value); } else { item.model.parameters.remove(&p.id); }
                                    }
                                    if ui.add(egui::Slider::new(&mut value,p.min..=p.max)).changed() { item.model.parameters.insert(p.id.clone(),value); }
                                });
                            });
                        }
                    });
                });
            }
        });
    }
}
fn configure_rig(rig: &mut RigConfig, item: &Item, frozen: bool) {
    rig.physics.enabled = item.model.physics;
    if frozen || !item.model.animate {
        rig.pose.mode = PoseMode::Frozen;
        if frozen {
            rig.pose.frozen.clone_from(&item.model.snapshot);
        } else {
            rig.pose.frozen.clear();
        }
        rig.pose
            .frozen
            .extend(item.model.parameters.iter().map(|(id, v)| (id.clone(), *v)));
    } else {
        rig.pose.mode = PoseMode::Override;
        rig.pose.held.clone_from(&item.model.parameters);
    }
}
fn object_files(item: &Item) -> Result<aria_model::ModelFiles> {
    if item.model.textures.is_empty() {
        aria_model::load_files(&item.path)
    } else {
        aria_model::bare_moc(&item.path, &item.model.textures)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_objects_and_parent_freeze_hold_their_own_values() {
        let mut item = Item {
            path: "prop.moc3".into(),
            ..Default::default()
        };
        item.model.parameters.insert("angle".into(), 12.0);
        item.model.snapshot.insert("secondary".into(), 0.4);
        let mut object = RigConfig::default();
        configure_rig(&mut object, &item, false);
        assert_eq!(object.pose.mode, PoseMode::Frozen);
        assert_eq!(object.pose.frozen["angle"], 12.0);
        assert!(!object.pose.frozen.contains_key("secondary"));
        item.model.animate = true;
        configure_rig(&mut object, &item, false);
        assert_eq!(object.pose.mode, PoseMode::Override);
        assert_eq!(object.pose.held["angle"], 12.0);
        configure_rig(&mut object, &item, true);
        assert_eq!(object.pose.frozen["secondary"], 0.4);
    }

    #[test]
    #[ignore = "requires a GPU, ARIA_CUBISM_CORE and ARIA_TEST_MODEL"]
    fn local_objects_preserve_main_avatar_and_freeze_independently() {
        use aria_live2d::CubismModel;
        let path = PathBuf::from(std::env::var_os("ARIA_TEST_MODEL").unwrap());
        let core = PathBuf::from(std::env::var_os("ARIA_CUBISM_CORE").unwrap());
        let files = aria_model::load_files(&path).unwrap();
        let main = CubismModel::load(
            &core,
            &aria_model::read_bounded(&files.moc, 128 * 1024 * 1024).unwrap(),
            files.textures.len(),
        )
        .unwrap();
        let before: Vec<_> = main
            .parameters()
            .iter()
            .map(|p| (p.id.clone(), p.value))
            .collect();
        let before_mesh: Vec<_> = main.drawables.iter().map(|d| d.positions.clone()).collect();
        let p = main
            .parameters()
            .iter()
            .find(|p| p.id == "ParamAngleX")
            .unwrap();
        let mut item = Item {
            path: files.moc.clone(),
            ..Default::default()
        };
        // Both loading paths must resolve the original export's atlas order.
        assert_eq!(object_files(&item).unwrap().textures, files.textures);
        item.model.textures = files.textures.clone();
        assert_eq!(object_files(&item).unwrap().textures, files.textures);
        item.model.textures.clear();
        let instance = eframe::wgpu::Instance::new(&eframe::wgpu::InstanceDescriptor {
            backends: eframe::wgpu::Backends::DX12,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let format = eframe::wgpu::TextureFormat::Rgba8Unorm;
        let renderer = eframe::egui_wgpu::Renderer::new(&device, format, Default::default());
        let state = RenderState {
            adapter,
            available_adapters: vec![],
            device,
            queue,
            target_format: format,
            renderer: std::sync::Arc::new(egui::mutex::RwLock::new(renderer)),
        };
        let mut parent = RigConfig::default();
        parent.manual.insert("main-only".into(), 0.7);
        parent.items.push(item);
        let parent_controls = parent.manual.clone();
        let mut objects = ObjectModels::default();
        objects.sync(Some(&state), &core, &parent);
        objects.update(&mut parent, &Inputs::new(), 0.016);
        let baseline_mesh: Vec<_> = objects.entries[&1]
            .loaded
            .as_ref()
            .unwrap()
            .avatar
            .model
            .drawables
            .iter()
            .map(|d| d.positions.clone())
            .collect();
        parent.items[0].model.parameters.insert(p.id.clone(), p.max);
        objects.update(&mut parent, &Inputs::new(), 0.016);
        let loaded = objects.entries[&1].loaded.as_ref().unwrap();
        assert_eq!(
            loaded
                .avatar
                .model
                .parameters()
                .iter()
                .find(|v| v.id == p.id)
                .unwrap()
                .value,
            p.max
        );
        assert_ne!(
            baseline_mesh,
            loaded
                .avatar
                .model
                .drawables
                .iter()
                .map(|d| d.positions.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            before,
            main.parameters()
                .iter()
                .map(|p| (p.id.clone(), p.value))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            before_mesh,
            main.drawables
                .iter()
                .map(|d| d.positions.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(parent.manual, parent_controls);
        parent.items[0].model.animate = true;
        parent.items[0].model.parameters.clear();
        let varying = Inputs::from([("FaceAngleX".into(), -30.0)]);
        let rig = &objects.entries[&1].loaded.as_ref().unwrap().rig;
        if files.physics.is_some() {
            assert!(
                objects.entries[&1]
                    .loaded
                    .as_ref()
                    .unwrap()
                    .avatar
                    .physics
                    .is_some()
            );
        }
        let binding = rig.bindings[&p.id].clone();
        let previous = parent.items[0].model.snapshot[&p.id];
        for _ in 0..30 {
            objects.update(
                &mut parent,
                &Inputs::from([(binding.input.clone(), binding.input_min)]),
                0.016,
            );
        }
        assert_ne!(
            parent.items[0].model.snapshot[&p.id], previous,
            "Object's own tracking mapping should move it"
        );
        assert_eq!(
            before,
            main.parameters()
                .iter()
                .map(|p| (p.id.clone(), p.value))
                .collect::<Vec<_>>()
        );
        parent.pose.mode = PoseMode::Frozen;
        let frozen = parent.items[0].model.snapshot.clone();
        let mut restored: RigConfig =
            serde_json::from_slice(&serde_json::to_vec(&parent).unwrap()).unwrap();
        objects.update(&mut restored, &varying, 0.5);
        assert_eq!(restored.items[0].model.snapshot, frozen);
        // An output's old frame owns the GPU registration until it finishes painting.
        let frame = objects.image(1).unwrap();
        let ItemImage::Model { _lease, .. } = &frame else {
            panic!("Expected model image")
        };
        let lease = std::sync::Arc::downgrade(_lease);
        restored.items.clear();
        objects.sync(Some(&state), &core, &restored);
        assert!(objects.entries.is_empty());
        assert!(lease.upgrade().is_some());
        drop(frame);
        assert!(lease.upgrade().is_none());
        eprintln!(
            "Verified independent native object pose, unchanged main parameters and mesh, bare atlas setup, frozen preset restoration, and output texture lifetime"
        );
    }
}
