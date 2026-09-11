use crate::cubism_render::{ModelImage, ModelRenderer};
use anyhow::{Context, Result};
use aria_core::{
    Parameters, TrackingFrame,
    physics::Physics,
    rig::{self, Binding, Inputs},
};
use aria_live2d::CubismModel;
use aria_model::ModelFiles;
use eframe::{egui, egui_wgpu::RenderState};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};

pub struct Avatar {
    pub name: String,
    pub files: ModelFiles,
    pub model: CubismModel,
    renderer: ModelRenderer,
    bindings: BTreeMap<String, Binding>,
    manual: BTreeMap<String, f32>,
    labels: BTreeMap<String, String>,
    inputs: Inputs,
    pub physics: Option<Physics>,
    pub imported_count: usize,
    search: String,
    elapsed: f32,
    pub controls_open: bool,
}
impl Avatar {
    pub fn load(state: &RenderState, core: &Path, mut files: ModelFiles) -> Result<Self> {
        let bytes = aria_model::read_bounded(&files.moc, 128 * 1024 * 1024)?;
        let model = CubismModel::load(core, &bytes, files.textures.len())
            .context("Cannot load Live2D avatar")?;
        let renderer = ModelRenderer::new(state, model.canvas, &model.drawables, &files.textures)?;
        renderer.render(model.canvas, &model.drawables)?;
        let mut bindings = rig::default_bindings(model.parameters());
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
                    bindings.extend(profile.bindings);
                    files.warnings.extend(profile.warnings);
                    if let Some(physics) = &mut physics {
                        physics.enabled = profile.physics_enabled;
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
            files,
            model,
            renderer,
            bindings,
            physics,
            imported_count,
            labels,
            inputs: Inputs::new(),
            manual: BTreeMap::new(),
            search: String::new(),
            elapsed: 0.0,
            controls_open: false,
        })
    }
    pub fn image(&self) -> ModelImage {
        self.renderer.image
    }
    pub fn mapped_count(&self) -> usize {
        self.bindings.len()
    }
    pub fn atlas_mib(&self) -> f64 {
        self.renderer.atlas_mib
    }
    pub fn update(
        &mut self,
        params: Parameters,
        frame: Option<&TrackingFrame>,
        mirror: bool,
        dt: f32,
    ) -> Result<()> {
        self.elapsed += dt.min(0.25);
        self.inputs = rig::tracking_inputs(frame, params, mirror, self.elapsed);
        self.model.reset_parameters();
        let mut values = self.model.parameters().to_vec();
        for p in &mut values {
            if let Some(&value) = self.manual.get(&p.id) {
                p.value = value;
            }
        }
        rig::apply_bindings(&mut self.bindings, &self.inputs, &mut values, dt);
        if let Some(physics) = &mut self.physics {
            physics.update(&mut values, dt);
        }
        for p in values {
            self.model.set_parameter(&p.id, p.value);
        }
        self.model.update()?;
        self.renderer
            .render(self.model.canvas, &self.model.drawables)
    }
    pub fn physics_controls(&mut self, ui: &mut egui::Ui) {
        if let Some(physics) = &mut self.physics {
            ui.checkbox(&mut physics.enabled, "Secondary motion / physics");
            ui.label(
                egui::RichText::new(format!(
                    "{} groups · {} driven outputs",
                    physics.group_count(),
                    physics.output_count()
                ))
                .small(),
            );
            ui.add_enabled(
                physics.enabled,
                egui::Slider::new(&mut physics.strength, 0.0..=2.0).text("Motion strength"),
            );
            ui.collapsing("Physics tuning", |ui| {
                ui.add(egui::Slider::new(&mut physics.wind_strength, -1.0..=1.0).text("Wind"));
                if ui.button("Settle motion").clicked() { physics.reset(); }
                ui.label("Breathing and the rig's physics run before each model update. Settings last until unload.");
            });
        } else {
            ui.label(egui::RichText::new("No physics rig loaded").small());
        }
    }
    pub fn controls(&mut self, ctx: &egui::Context) {
        egui::Window::new("Live2D model parameters").open(&mut self.controls_open).default_width(730.0).show(ctx, |ui| {
            ui.label(&self.name);
            ui.label(format!("{} assignments imported from the adjacent VTS profile. Edits last until unload.", self.imported_count));
            ui.label("Expand an input to adjust its ranges. Physics outputs are marked; disable physics to adjust those manually.");
            ui.horizontal(|ui| { ui.label("Filter name or ID"); ui.text_edit_singleline(&mut self.search); });
            let search = self.search.to_lowercase();
            egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                for (index,p) in self.model.parameters().iter().enumerate() {
                    let label = self.labels.get(&p.id).map(String::as_str).unwrap_or(&p.id);
                    if !format!("{label} {}",p.id).to_lowercase().contains(&search) { continue; }
                    let physics_driven = self.physics.as_ref().is_some_and(|v| v.enabled && v.controls_parameter(index));
                    ui.push_id(&p.id, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(label).on_hover_text(&p.id);
                            if physics_driven { ui.label(egui::RichText::new("PHYSICS").small()); }
                            let old = self.bindings.get(&p.id).map(|b| b.input.clone());
                            let mut source = old.clone();
                            egui::ComboBox::from_id_salt("source").selected_text(source.as_deref().unwrap_or("Manual")).show_ui(ui, |ui| {
                                ui.selectable_value(&mut source,None,"Manual");
                                for key in self.inputs.keys() { ui.selectable_value(&mut source,Some(key.clone()),key); }
                            });
                            if source != old {
                                if let Some(source) = source {
                                    let (min,max) = rig::input_range(&source);
                                    let mut b = Binding::direct(&source,min,max); b.output_min=p.min; b.output_max=p.max;
                                    self.bindings.insert(p.id.clone(),b);
                                } else { self.bindings.remove(&p.id); self.manual.insert(p.id.clone(),p.value); }
                            }
                        });
                        if let Some(b) = self.bindings.get_mut(&p.id) {
                            egui::CollapsingHeader::new(format!("{} · input {:.3} → {:.3}",b.name,self.inputs.get(&b.input).copied().unwrap_or(0.0),p.value)).id_salt("ranges").show(ui, |ui| {
                                ui.horizontal(|ui| { ui.label("Input range"); ui.add(egui::DragValue::new(&mut b.input_min).speed(0.01).range(-1000.0..=1000.0)); ui.add(egui::DragValue::new(&mut b.input_max).speed(0.01).range(-1000.0..=1000.0)); ui.checkbox(&mut b.clamp_input,"Clamp"); });
                                ui.horizontal(|ui| { ui.label("Output range"); ui.add(egui::DragValue::new(&mut b.output_min).speed(0.01).range(p.min..=p.max)); ui.add(egui::DragValue::new(&mut b.output_max).speed(0.01).range(p.min..=p.max)); ui.checkbox(&mut b.clamp_output,"Clamp"); });
                                ui.add(egui::Slider::new(&mut b.smoothing_ms,0.0..=500.0).text("Smoothing ms"));
                            });
                        }
                        let mut value=p.value;
                        if ui.add_enabled(!self.bindings.contains_key(&p.id) && !physics_driven,egui::Slider::new(&mut value,p.min..=p.max).text(&p.id)).changed() { self.manual.insert(p.id.clone(),value); }
                        ui.separator();
                    });
                }
            });
        });
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
    }
}
