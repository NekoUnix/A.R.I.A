use crate::cubism_render::{ModelImage, ModelRenderer};
use anyhow::{Context, Result};
use aria_core::{
    movement::{self, PoseMode, RigConfig},
    physics::Physics,
    rig::{self, Inputs},
};
#[cfg(test)]
use aria_live2d::CubismModel;
use aria_live2d::host::HostedModel;
use aria_model::ModelFiles;
use eframe::egui_wgpu::RenderState;
use std::{collections::BTreeMap, path::Path};

pub struct Avatar {
    pub name: String,
    pub model_key: String,
    pub files: ModelFiles,
    pub model: HostedModel,
    pub initial_config: RigConfig,
    pub labels: BTreeMap<String, String>,
    pub parameter_groups: BTreeMap<String, String>,
    pub physics: Option<Physics>,
    pub imported_count: usize,
    renderer: ModelRenderer,
    last_pose_mode: PoseMode,
    last_layers: aria_core::layers::Config,
    values: Vec<rig::RigParameter>,
}
impl Avatar {
    pub fn load(state: &RenderState, core: &Path, mut files: ModelFiles) -> Result<Self> {
        let bytes = aria_model::read_bounded(&files.moc, aria_core::asset_limits::MOC_FILE)?;
        let model_key = movement::model_key(&bytes);
        drop(bytes);
        let model = HostedModel::load(core, &files.moc, files.textures.len())
            .context("Cannot load Live2D avatar")?;
        let mut renderer =
            ModelRenderer::new(state, model.canvas, &model.drawables, &files.textures)?;
        files.warnings.extend(renderer.import_notes.clone());
        renderer.render(model.canvas, &model.drawables)?;
        let mut initial_config = RigConfig::from_parameters(model.parameters());
        let mut imported_count = 0;
        let mut physics = files.physics.as_ref().and_then(|path| {
            match aria_model::read_bounded(path, aria_core::asset_limits::MODEL_JSON)
                .and_then(|b| Physics::load(&b, model.parameters()))
            {
                Ok(physics) => {
                    files.warnings.extend(physics.warnings.clone());
                    Some(physics)
                }
                Err(error) => {
                    files
                        .warnings
                        .push(format!("Physics could not load: {error:#}"));
                    None
                }
            }
        });
        if let Some(path) = &files.tracking_profile {
            match aria_model::read_bounded(path, aria_core::asset_limits::MODEL_JSON)
                .and_then(|b| rig::import_profile(&b, model.parameters()))
            {
                Ok(profile) => {
                    imported_count = profile.bindings.len();
                    initial_config.bindings.extend(profile.bindings);
                    files.warnings.extend(profile.warnings);
                    initial_config.physics.enabled = profile.physics_enabled;
                    if let Some(physics) = &mut physics {
                        physics.set_multipliers(&profile.physics_multipliers);
                    }
                }
                Err(error) => files
                    .warnings
                    .push(format!("Tracking profile could not load: {error:#}")),
            }
        }
        let display = files
            .display_info
            .as_ref()
            .and_then(|path| {
                match aria_model::read_bounded(path, aria_core::asset_limits::MODEL_JSON)
                    .and_then(|bytes| aria_model::DisplayInfo::decode(&bytes))
                {
                    Ok(display) => Some(display),
                    Err(error) => {
                        files
                            .warnings
                            .push(format!("Parameter names could not load: {error:#}"));
                        None
                    }
                }
            })
            .unwrap_or_default();
        let name = files
            .moc
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let values = model.parameters().to_vec();
        Ok(Self {
            name,
            model_key,
            files,
            model,
            initial_config,
            labels: display.labels,
            parameter_groups: display.parameter_groups,
            physics,
            imported_count,
            renderer,
            last_pose_mode: PoseMode::Live,
            last_layers: Default::default(),
            values,
        })
    }
    pub fn image(&self) -> ModelImage {
        self.renderer.image
    }
    pub fn layer_opacity(&self, mesh: usize) -> f32 {
        self.model
            .drawables
            .get(mesh)
            .map_or(0., |d| self.last_layers.opacity(&d.id))
    }
    pub fn image_lease(&self) -> std::sync::Arc<crate::cubism_render::ModelTexture> {
        self.renderer.lease.clone()
    }
    pub fn atlas_mib(&self) -> f64 {
        self.renderer.atlas_mib
    }
    pub fn key_palette(&self) -> Result<crate::chroma::Palette> {
        self.renderer.key_palette()
    }
    pub fn image_bounds(&self) -> eframe::egui::Rect {
        self.renderer.bounds
    }
    pub fn view_canvas(&self) -> aria_live2d::Canvas {
        self.renderer.view_canvas
    }
    pub fn reset_motion(&mut self) {
        if let Some(physics) = &mut self.physics {
            physics.reset();
        }
    }
    pub fn update(
        &mut self,
        inputs: &Inputs,
        config: &mut RigConfig,
        expressions: &mut crate::expressions_panel::ExpressionsPanel,
        dt: f32,
    ) -> Result<bool> {
        if config.pose.mode != self.last_pose_mode {
            self.reset_motion();
            self.last_pose_mode = config.pose.mode;
        }
        // Parameter identities/ranges are immutable. Reuse their strings and
        // buffers; only values change between evaluations.
        for (p, native) in self.values.iter_mut().zip(self.model.parameters()) {
            p.value = native.value;
        }
        config.evaluate_with_expressions(
            inputs,
            &mut self.values,
            dt,
            self.physics.as_mut(),
            |p, active| expressions.update(p, active, dt),
        );
        let parts = &expressions.motions.parts;
        let parts_changed = parts.len() == self.model.parts.len()
            && parts
                .iter()
                .zip(&self.model.parts)
                .any(|(a, b)| a.value != b.value);
        let model_opacity = config.layers.model_opacity
            * expressions
                .motions
                .model
                .get("Opacity")
                .copied()
                .unwrap_or(1.)
                .clamp(0., 1.);
        // Most frames have no whole-model opacity motion. Borrow the layer maps
        // rather than cloning thousands of mesh entries on every tracking frame.
        let render_layers = if model_opacity == config.layers.model_opacity {
            std::borrow::Cow::Borrowed(&config.layers)
        } else {
            let mut layers = config.layers.clone();
            layers.model_opacity = model_opacity;
            std::borrow::Cow::Owned(layers)
        };
        let pose_changed = !self
            .values
            .iter()
            .zip(self.model.parameters())
            .all(|(a, b)| a.value == b.value);
        if !pose_changed && !parts_changed && self.last_layers == *render_layers {
            return Ok(false);
        }
        if pose_changed || parts_changed {
            if parts_changed {
                self.model.parts.clone_from(parts);
            }
            for p in &self.values {
                self.model.set_parameter(&p.id, p.value);
            }
            self.model.update()?;
        }
        self.renderer
            .render_layers(self.model.canvas, &self.model.drawables, &render_layers)?;
        if self.last_layers != *render_layers {
            self.last_layers.clone_from(&render_layers);
        }
        Ok(true)
    }
    pub fn save_png(&self, path: &Path) -> Result<()> {
        self.renderer.save_png(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    #[ignore = "requires ARIA_TEST_MODEL_FOLDER, Cubism Core and DX12"]
    fn nested_model_library_renders_full_geometry() {
        let state = crate::spout::tests::gpu_state();
        let root = std::env::var_os("ARIA_TEST_MODEL_FOLDER").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let models = aria_model::discover_models(Path::new(&root)).unwrap();
        assert!(!models.is_empty());
        for (index, path) in models.iter().enumerate() {
            let mut a = Avatar::load(
                &state,
                Path::new(&core),
                aria_model::load_files(path).unwrap(),
            )
            .unwrap();
            let original = a.model.canvas;
            let view = a.view_canvas();
            println!(
                "Export {index}: {} | {} atlases, {} meshes | declared {:?}, full view {:?}",
                a.name,
                a.files.textures.len(),
                a.model.drawables.len(),
                original.size,
                view.size
            );
            if let Some(folder) = std::env::var_os("ARIA_TEST_RENDER_DIR") {
                a.save_png(&Path::new(&folder).join(format!("friend-{index}.png")))
                    .unwrap();
            }
            for extreme in [false, true] {
                for p in a.model.parameters().to_vec().iter().filter(|p| {
                    p.id.starts_with("ParamAngle") || p.id.starts_with("ParamBodyAngle")
                }) {
                    a.model
                        .set_parameter(&p.id, if extreme { p.max } else { p.min });
                }
                a.model.update().unwrap();
                a.renderer.render(original, &a.model.drawables).unwrap();
                let c = a.view_canvas();
                for p in a
                    .model
                    .drawables
                    .iter()
                    .filter(|d| d.visible && d.opacity > 0.0)
                    .flat_map(|d| &d.positions)
                {
                    for (axis, coordinate) in p.iter().enumerate() {
                        let v = (coordinate * c.pixels_per_unit + c.origin[axis]) / c.size[axis];
                        assert!((0.0..=1.0).contains(&v), "Clipped mesh in {}", a.name);
                    }
                }
            }
            let (rgba, _) = a.renderer.read_rgba_for_test().unwrap();
            assert!(rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0));
        }
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "requires local Cubism Core, test model and DX12 GPU"]
    fn frozen_avatar_skips_core_and_gpu_work_but_edits_refresh_it() {
        let state = crate::spout::tests::gpu_state();
        let path = std::env::var_os("ARIA_TEST_MODEL").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let mut avatar = Avatar::load(
            &state,
            Path::new(&core),
            aria_model::load_files(Path::new(&path)).unwrap(),
        )
        .unwrap();
        let mut config = avatar.initial_config.clone();
        config.pose.mode = PoseMode::Frozen;
        config.pose.frozen = avatar
            .model
            .parameters()
            .iter()
            .map(|p| (p.id.clone(), p.value))
            .collect();
        let mut expressions = crate::expressions_panel::ExpressionsPanel::default();
        let before = avatar.renderer.vertex_staging_capacity();
        let original = avatar.renderer.read_rgba_for_test().unwrap().0;
        assert!(avatar.model.drawables.iter().all(|d| !d.id.is_empty()));
        assert_eq!(
            avatar
                .model
                .drawables
                .iter()
                .map(|d| &d.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            avatar.model.drawables.len()
        );
        config.layers.groups.push(aria_core::layers::Group {
            id: 1,
            name: "All hidden".into(),
            opacity: 0.,
            active: true,
            layers: avatar
                .model
                .drawables
                .iter()
                .map(|d| d.id.clone())
                .collect(),
        });
        assert!(
            avatar
                .update(&Inputs::new(), &mut config, &mut expressions, 0.)
                .unwrap()
        );
        let hidden = avatar.renderer.read_rgba_for_test().unwrap().0;
        assert!(
            hidden.as_chunks::<4>().0.iter().all(|p| p[3] == 0),
            "All selected ArtMeshes should be transparent"
        );
        config.layers.toggle(1);
        assert!(
            avatar
                .update(&Inputs::new(), &mut config, &mut expressions, 0.)
                .unwrap()
        );
        assert_eq!(
            original,
            avatar.renderer.read_rgba_for_test().unwrap().0,
            "Restoring layers restores the exact original image"
        );
        for _ in 0..60 {
            assert!(
                !avatar
                    .update(&Inputs::new(), &mut config, &mut expressions, 1.0 / 60.0)
                    .unwrap()
            );
        }
        let p = avatar
            .model
            .parameters()
            .iter()
            .find(|p| p.max > p.min)
            .unwrap();
        let id = p.id.clone();
        let value = if p.value == p.max { p.min } else { p.max };
        config.pose.frozen.insert(id.clone(), value);
        assert!(
            avatar
                .update(&Inputs::new(), &mut config, &mut expressions, 1.0 / 60.0)
                .unwrap()
        );
        assert_eq!(
            avatar
                .model
                .parameters()
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .value,
            value
        );
        assert!(
            !avatar
                .update(&Inputs::new(), &mut config, &mut expressions, 1.0 / 60.0)
                .unwrap()
        );
        assert_eq!(avatar.renderer.vertex_staging_capacity(), before);
    }
    #[test]
    #[ignore = "requires local ARIA_CUBISM_CORE and ARIA_TEST_MODEL with expression files"]
    fn local_expressions_toggle_native_avatar_and_restore_after_release() {
        use crate::{hotkeys::Action, input_monitor::InputMonitor};
        use aria_core::{MappingSettings, movement::SavedRig, shortcuts::Shortcut};
        let path = std::env::var_os("ARIA_TEST_MODEL").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let files = aria_model::load_files(Path::new(&path)).unwrap();
        assert!(!files.expressions.is_empty());
        let mut model = CubismModel::load(
            Path::new(&core),
            &aria_model::read_bounded(&files.moc, aria_core::asset_limits::MOC_FILE).unwrap(),
            files.textures.len(),
        )
        .unwrap();
        let mut baseline = model.parameters().to_vec();
        let mut config = RigConfig::from_parameters(&baseline);
        config.evaluate(&Inputs::new(), &mut baseline, 1.0 / 60.0, None);
        for p in &baseline {
            model.set_parameter(&p.id, p.value);
        }
        model.update().unwrap();
        let mut monitor = InputMonitor::new("fixture".into(), config, None, &baseline);
        monitor.expressions.load(
            &files.expressions,
            files.source.parent().unwrap(),
            &monitor.saved,
            &baseline,
        );
        assert_eq!(monitor.expressions.entries.len(), files.expressions.len());
        let mut mapping = MappingSettings::default();
        let mut values = baseline.clone();
        let snapshot = |model: &CubismModel| {
            model
                .drawables
                .iter()
                .map(|d| (d.positions.clone(), d.opacity, d.multiply, d.screen))
                .collect::<Vec<_>>()
        };
        let mut changed = 0;
        for file in &files.expressions {
            let expression = aria_core::expressions::Expression::load(
                &aria_model::read_bounded(&file.path, 1024 * 1024).unwrap(),
            )
            .unwrap();
            assert!(expression.missing_parameters(&baseline).is_empty());
            let key = Shortcut {
                ctrl: true,
                shift: true,
                key: 0x48,
                ..Default::default()
            };
            monitor.saved.expression_hotkeys.clear();
            crate::expressions_panel::ExpressionsPanel::assign(&mut monitor.saved, &file.id, key)
                .unwrap();
            assert!(
                monitor
                    .hotkey_keys()
                    .iter()
                    .any(|r| r.shortcut == key && r.action == Action::Expression(file.id.clone()))
            );
            let before = snapshot(&model);
            monitor.hotkey_action(Action::Expression(file.id.clone()), &values, &mut mapping);
            for _ in 0..120 {
                monitor.saved.config.evaluate_with_expressions(
                    &Inputs::new(),
                    &mut values,
                    1.0 / 60.0,
                    None,
                    |p, active| monitor.expressions.update(p, active, 1.0 / 60.0),
                );
            }
            for p in &values {
                model.set_parameter(&p.id, p.value);
            }
            model.update().unwrap();
            assert_ne!(
                before,
                snapshot(&model),
                "Expression {} should change this fixture's appearance",
                file.name
            );
            changed += values
                .iter()
                .zip(&baseline)
                .filter(|(a, b)| a.value != b.value)
                .count();
            let saved: SavedRig =
                serde_json::from_slice(&serde_json::to_vec(&monitor.saved).unwrap()).unwrap();
            assert!(saved.config.expressions.contains(&file.id));
            assert_eq!(saved.expression_hotkeys[&file.id], key);
            monitor.saved.config.capture_pose(&values);
            let frozen = snapshot(&model);
            monitor.hotkey_action(Action::Expression(file.id.clone()), &values, &mut mapping);
            assert!(monitor.saved.config.expressions.contains(&file.id));
            monitor.saved.config.evaluate_with_expressions(
                &Inputs::new(),
                &mut values,
                0.1,
                None,
                |_, _| panic!("Frozen pose evaluated expressions"),
            );
            for p in &values {
                model.set_parameter(&p.id, p.value);
            }
            model.update().unwrap();
            assert_eq!(frozen, snapshot(&model));
            monitor.saved.config.pose.mode = PoseMode::Live;
            monitor.hotkey_action(Action::Expression(file.id.clone()), &values, &mut mapping);
            for _ in 0..120 {
                monitor.saved.config.evaluate_with_expressions(
                    &Inputs::new(),
                    &mut values,
                    1.0 / 60.0,
                    None,
                    |p, active| monitor.expressions.update(p, active, 1.0 / 60.0),
                );
            }
            for (p, original) in values.iter().zip(&baseline) {
                assert_eq!(p.value, original.value);
                model.set_parameter(&p.id, p.value);
            }
            model.update().unwrap();
            assert_eq!(before, snapshot(&model));
        }
        eprintln!(
            "Verified {} expressions through hotkey actions: {changed} changed native parameters, appearance changes, exact frozen poses and restoration on release",
            files.expressions.len()
        );
    }
    #[test]
    #[ignore = "requires local ARIA_CUBISM_CORE and ARIA_TEST_MODEL with VTS and physics sidecars"]
    fn local_rig_assignments_and_physics_drive_native_parameters() {
        let path = std::env::var_os("ARIA_TEST_MODEL").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let files = aria_model::load_files(Path::new(&path)).unwrap();
        let mut model = CubismModel::load(
            Path::new(&core),
            &aria_model::read_bounded(&files.moc, aria_core::asset_limits::MOC_FILE).unwrap(),
            files.textures.len(),
        )
        .unwrap();
        let mut profile = rig::import_profile(
            &aria_model::read_bounded(
                files.tracking_profile.as_ref().unwrap(),
                aria_core::asset_limits::MODEL_JSON,
            )
            .unwrap(),
            model.parameters(),
        )
        .unwrap();
        assert!(profile.warnings.is_empty(), "{:?}", profile.warnings);
        assert!(!profile.bindings.is_empty());
        for (id, binding) in &profile.bindings {
            let mut binding = binding.clone();
            binding.smoothing_ms = 0.0;
            let parameter = model.parameters().iter().find(|p| p.id == *id).unwrap();
            let low = binding
                .evaluate(binding.input_min, 1.0 / 60.0)
                .clamp(parameter.min, parameter.max);
            let high = binding
                .evaluate(binding.input_max, 1.0 / 60.0)
                .clamp(parameter.min, parameter.max);
            assert!(
                (low - high).abs() > 0.001,
                "Binding for {id} must move its actual rig parameter"
            );
        }
        let mut physics = Physics::load(
            &aria_model::read_bounded(
                files.physics.as_ref().unwrap(),
                aria_core::asset_limits::MODEL_JSON,
            )
            .unwrap(),
            model.parameters(),
        )
        .unwrap();
        assert!(physics.warnings.is_empty(), "{:?}", physics.warnings);
        physics.set_multipliers(&profile.physics_multipliers);
        let mut pipeline = aria_core::ParameterPipeline::default();
        let mut ranges = vec![(f32::INFINITY, f32::NEG_INFINITY); model.parameters().len()];
        let initial_mesh = model
            .drawables
            .iter()
            .flat_map(|d| d.positions.clone())
            .collect::<Vec<_>>();
        let mut last = model.parameters().to_vec();
        for n in 0..600 {
            let t = n as f32 / 60.0;
            let frame = aria_core::demo_frame(t);
            let params = pipeline.update(
                Some(&frame),
                &aria_core::MappingSettings::default(),
                1.0 / 60.0,
            );
            let inputs = rig::tracking_inputs(Some(&frame), params, false, t);
            let mut values = model.parameters().to_vec();
            for p in &mut values {
                p.value = p.default;
            }
            rig::apply_bindings(&mut profile.bindings, &inputs, &mut values, 1.0 / 60.0);
            physics.update(&mut values, 1.0 / 60.0);
            for (p, range) in values.iter().zip(&mut ranges) {
                assert!(p.value.is_finite() && (p.min..=p.max).contains(&p.value));
                range.0 = range.0.min(p.value);
                range.1 = range.1.max(p.value);
            }
            last = values;
        }
        let moving = (0..last.len())
            .filter(|&i| physics.controls_parameter(i) && ranges[i].1 - ranges[i].0 > 0.001)
            .count();
        assert!(
            moving > physics.output_count() / 2,
            "Authored secondary motion must respond: {moving} moving outputs"
        );
        for p in &last {
            model.set_parameter(&p.id, p.value);
        }
        model.update().unwrap();
        assert_ne!(
            initial_mesh,
            model
                .drawables
                .iter()
                .flat_map(|d| d.positions.clone())
                .collect::<Vec<_>>()
        );
        eprintln!(
            "Verified {} imported assignments, {} physics groups, {} output assignments, {} moving physics parameters",
            profile.bindings.len(),
            physics.group_count(),
            physics.output_count(),
            moving
        );
        let mut config = RigConfig {
            bindings: profile.bindings.clone(),
            ..Default::default()
        };
        config.capture_pose(model.parameters());
        let serialized = serde_json::to_vec(&config).unwrap();
        let mut restored: RigConfig = serde_json::from_slice(&serialized).unwrap();
        restored.validate(model.parameters()).unwrap();
        let frozen_vertices = model
            .drawables
            .iter()
            .flat_map(|d| d.positions.clone())
            .collect::<Vec<_>>();
        for n in 0..120 {
            let inputs = rig::tracking_inputs(
                Some(&aria_core::demo_frame(n as f32)),
                aria_core::Parameters([1.0; 12]),
                true,
                n as f32,
            );
            let mut values = model.parameters().to_vec();
            restored.evaluate(&inputs, &mut values, 1.0 / 60.0, Some(&mut physics));
            for p in values {
                model.set_parameter(&p.id, p.value);
            }
            if n % 20 == 0 {
                model.update().unwrap();
                assert_eq!(
                    frozen_vertices,
                    model
                        .drawables
                        .iter()
                        .flat_map(|d| d.positions.clone())
                        .collect::<Vec<_>>()
                );
            }
        }
        let p = model
            .parameters()
            .iter()
            .find(|p| p.id == "ParamAngleX")
            .unwrap();
        restored.pose.frozen.insert(p.id.clone(), p.max);
        let mut values = model.parameters().to_vec();
        restored.evaluate(&Inputs::new(), &mut values, 0.016, Some(&mut physics));
        for p in values {
            model.set_parameter(&p.id, p.value);
        }
        model.update().unwrap();
        assert_ne!(
            frozen_vertices,
            model
                .drawables
                .iter()
                .flat_map(|d| d.positions.clone())
                .collect::<Vec<_>>()
        );
        eprintln!(
            "Serialized full pose stayed vertex-identical through changing tracking/physics inputs, and manual head edits deformed the rig"
        );
    }
}
