use crate::cubism_render::{ModelImage, ModelRenderer};
use anyhow::{Context, Result};
use aria_core::{
    movement::{self, PoseMode, RigConfig},
    physics::Physics,
    rig::{self, Inputs},
};
use aria_live2d::CubismModel;
use aria_model::ModelFiles;
use eframe::egui_wgpu::RenderState;
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

pub struct Avatar {
    pub name: String,
    pub model_key: String,
    pub files: ModelFiles,
    pub model: CubismModel,
    pub initial_config: RigConfig,
    pub labels: BTreeMap<String, String>,
    pub physics: Option<Physics>,
    pub imported_count: usize,
    renderer: ModelRenderer,
    last_pose_mode: PoseMode,
}
impl Avatar {
    pub fn load(state: &RenderState, core: &Path, mut files: ModelFiles) -> Result<Self> {
        let bytes = aria_model::read_bounded(&files.moc, 128 * 1024 * 1024)?;
        let model_key = movement::model_key(&bytes);
        let model = CubismModel::load(core, &bytes, files.textures.len())
            .context("Cannot load Live2D avatar")?;
        let renderer = ModelRenderer::new(state, model.canvas, &model.drawables, &files.textures)?;
        renderer.render(model.canvas, &model.drawables)?;
        let mut initial_config = RigConfig::from_parameters(model.parameters());
        let mut imported_count = 0;
        let mut physics = files.physics.as_ref().and_then(|path| {
            match aria_model::read_bounded(path, 2 * 1024 * 1024)
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
            match aria_model::read_bounded(path, 2 * 1024 * 1024)
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
        let labels = files
            .display_info
            .as_ref()
            .and_then(|path| match read_labels(path) {
                Ok(labels) => Some(labels),
                Err(error) => {
                    files
                        .warnings
                        .push(format!("Parameter names could not load: {error:#}"));
                    None
                }
            })
            .unwrap_or_default();
        let name = files
            .moc
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Ok(Self {
            name,
            model_key,
            files,
            model,
            initial_config,
            labels,
            physics,
            imported_count,
            renderer,
            last_pose_mode: PoseMode::Live,
        })
    }
    pub fn image(&self) -> ModelImage {
        self.renderer.image
    }
    pub fn atlas_mib(&self) -> f64 {
        self.renderer.atlas_mib
    }
    pub fn key_palette(&self) -> Result<crate::chroma::Palette> {
        self.renderer.key_palette()
    }
    pub fn image_bounds(&self) -> eframe::egui::Rect {
        let c = self.model.canvas;
        let mut rect = eframe::egui::Rect::NOTHING;
        for d in self
            .model
            .drawables
            .iter()
            .filter(|d| d.visible && d.opacity > 0.01)
        {
            for p in &d.positions {
                rect.extend_with(eframe::egui::pos2(
                    (p[0] * c.pixels_per_unit + c.origin[0]) / c.size[0],
                    1.0 - (p[1] * c.pixels_per_unit + c.origin[1]) / c.size[1],
                ));
            }
        }
        rect.intersect(eframe::egui::Rect::from_min_max(
            eframe::egui::Pos2::ZERO,
            eframe::egui::pos2(1.0, 1.0),
        ))
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
    ) -> Result<()> {
        if config.pose.mode != self.last_pose_mode {
            self.reset_motion();
            self.last_pose_mode = config.pose.mode;
        }
        let mut values = self.model.parameters().to_vec();
        config.evaluate_with_expressions(
            inputs,
            &mut values,
            dt,
            self.physics.as_mut(),
            |p, active| expressions.update(p, active, dt),
        );
        for p in values {
            self.model.set_parameter(&p.id, p.value);
        }
        self.model.update()?;
        self.renderer
            .render(self.model.canvas, &self.model.drawables)
    }
    pub fn save_png(&self, path: &Path) -> Result<()> {
        self.renderer.save_png(path)
    }
}
fn read_labels(path: &Path) -> Result<BTreeMap<String, String>> {
    #[derive(Deserialize)]
    struct Info {
        #[serde(rename = "Parameters")]
        parameters: Vec<Label>,
    }
    #[derive(Deserialize)]
    struct Label {
        #[serde(rename = "Id")]
        id: String,
        #[serde(rename = "Name")]
        name: String,
    }
    let info: Info = serde_json::from_slice(&aria_model::read_bounded(path, 2 * 1024 * 1024)?)?;
    Ok(info
        .parameters
        .into_iter()
        .map(|v| (v.id, v.name))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
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
            &aria_model::read_bounded(&files.moc, 128 * 1024 * 1024).unwrap(),
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
            &aria_model::read_bounded(&files.moc, 128 * 1024 * 1024).unwrap(),
            files.textures.len(),
        )
        .unwrap();
        let mut profile = rig::import_profile(
            &aria_model::read_bounded(files.tracking_profile.as_ref().unwrap(), 2 * 1024 * 1024)
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
            &aria_model::read_bounded(files.physics.as_ref().unwrap(), 2 * 1024 * 1024).unwrap(),
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
