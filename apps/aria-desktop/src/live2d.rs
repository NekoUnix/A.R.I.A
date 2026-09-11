use crate::cubism_render::{ModelImage, ModelRenderer};
use anyhow::{Context, Result};
use aria_core::{PARAMETER_SPECS, Parameters};
use aria_live2d::CubismModel;
use aria_model::ModelFiles;
use eframe::{egui, egui_wgpu::RenderState};
use std::{collections::BTreeMap, path::Path};

pub struct Avatar {
    pub name: String,
    pub files: ModelFiles,
    pub model: CubismModel,
    renderer: ModelRenderer,
    bindings: BTreeMap<String, usize>,
    manual: BTreeMap<String, f32>,
    search: String,
    pub controls_open: bool,
}
impl Avatar {
    pub fn load(state: &RenderState, core: &Path, files: ModelFiles) -> Result<Self> {
        let bytes = aria_model::read_bounded(&files.moc, 128 * 1024 * 1024)?;
        let model = CubismModel::load(core, &bytes, files.textures.len())
            .context("Cannot load Live2D avatar")?;
        let renderer = ModelRenderer::new(state, model.canvas, &model.drawables, &files.textures)?;
        renderer.render(model.canvas, &model.drawables)?;
        let bindings = model
            .parameters()
            .iter()
            .filter_map(|p| {
                PARAMETER_SPECS
                    .iter()
                    .position(|s| s.0 == p.id)
                    .map(|i| (p.id.clone(), i))
            })
            .collect();
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
            manual: BTreeMap::new(),
            search: String::new(),
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
    pub fn update(&mut self, params: Parameters) -> Result<()> {
        self.model.reset_parameters();
        for (id, &value) in &self.manual {
            self.model.set_parameter(id, value);
        }
        for (id, &index) in &self.bindings {
            self.model.set_parameter(id, params.0[index]);
        }
        self.model.update()?;
        self.renderer
            .render(self.model.canvas, &self.model.drawables)
    }
    pub fn controls(&mut self, ctx: &egui::Context) {
        egui::Window::new("Live2D model parameters").open(&mut self.controls_open).default_width(660.0).show(ctx,|ui| {
            ui.label(&self.name);
            ui.label("Choose a tracking input for each model parameter. Values clamp to the rig's range. Manual sliders work when input is Manual. Assignments last until the model is unloaded.");
            ui.horizontal(|ui| { ui.label("Filter"); ui.text_edit_singleline(&mut self.search); });
            egui::ScrollArea::vertical().max_height(500.0).show(ui, |ui| {
                for p in self.model.parameters() {
                    if !p.id.to_lowercase().contains(&self.search.to_lowercase()) { continue; }
                    ui.push_id(&p.id,|ui| {
                        ui.horizontal(|ui| {
                            ui.label(&p.id);
                            let mut binding = self.bindings.get(&p.id).copied();
                            egui::ComboBox::from_id_salt("binding").selected_text(binding.map(|i| PARAMETER_SPECS[i].0).unwrap_or("Manual")).show_ui(ui,|ui| {
                                ui.selectable_value(&mut binding,None,"Manual");
                                for (i,spec) in PARAMETER_SPECS.iter().enumerate() { ui.selectable_value(&mut binding,Some(i),spec.0); }
                            });
                            if let Some(index) = binding { self.bindings.insert(p.id.clone(),index); }
                            else { self.bindings.remove(&p.id); }
                        });
                        let mut value = p.value;
                        if ui.add_enabled(!self.bindings.contains_key(&p.id),egui::Slider::new(&mut value,p.min..=p.max)).changed() { self.manual.insert(p.id.clone(),value); }
                        ui.separator();
                    });
                }
            });
        });
    }
}
