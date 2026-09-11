use aria_core::{
    MappingSettings,
    movement::{PoseMode, Preset, PresetFile, PresetKind, RigConfig, SavedRig, snap},
    rig::{self, Binding, Inputs, RigParameter},
};
use eframe::egui;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Inputs,
    Pose,
    Presets,
    Raw,
}
pub struct InputMonitor {
    pub saved: SavedRig,
    pub model_key: String,
    pub tab: Tab,
    pub message: Option<String>,
    pub hotkey_status: Option<String>,
    pub save_requested: bool,
    pub reset_motion: bool,
    pub export_png: Option<PathBuf>,
    search: String,
    mapped_only: bool,
    preset_name: String,
    draft_hotkey: Option<u8>,
    selected: Option<usize>,
}
impl InputMonitor {
    #[cfg(feature = "screenshots")]
    pub fn prepare_smoke(&mut self, parameters: &[RigParameter], mapping: &MappingSettings) {
        self.preset_name = "Soft movement".into();
        self.saved.config.steps.insert("ParamAngleX".into(), 0.5);
        self.save_new(PresetKind::Movement, parameters, mapping);
        self.saved.config.capture_pose(parameters);
        for p in parameters {
            let target = match p.id.as_str() {
                "ParamAngleX" => Some(12.0_f32),
                "ParamAngleY" => Some(6.0),
                "ParamMouthOpenY" => Some(0.3),
                _ => None,
            };
            if let Some(value) = target {
                self.saved
                    .config
                    .pose
                    .frozen
                    .insert(p.id.clone(), value.clamp(p.min, p.max));
            }
        }
        let mut posed = parameters.to_vec();
        for p in &mut posed {
            p.value = self.saved.config.pose.frozen[&p.id];
        }
        self.preset_name = "Portrait pose".into();
        self.save_new(PresetKind::Pose, &posed, mapping);
        self.message = None;
        self.save_requested = false;
        if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("inputs") {
            self.search = "ParamAngleX".into();
        }
    }
    pub fn new(
        model_key: String,
        default: RigConfig,
        saved: Option<SavedRig>,
        parameters: &[RigParameter],
    ) -> Self {
        let mut message = None;
        let saved = saved
            .filter(|s| match s.validate(parameters) {
                Ok(()) => true,
                Err(error) => {
                    message = Some(format!("Saved configuration was not applied: {error:#}"));
                    false
                }
            })
            .unwrap_or(SavedRig {
                config: default,
                ..Default::default()
            });
        Self {
            saved,
            model_key,
            tab: Tab::Inputs,
            message,
            hotkey_status: None,
            save_requested: false,
            reset_motion: true,
            export_png: None,
            search: String::new(),
            mapped_only: true,
            preset_name: String::new(),
            draft_hotkey: None,
            selected: None,
        }
    }
    pub fn toggle_pose(&mut self, parameters: &[RigParameter]) {
        if self.saved.config.pose.mode == PoseMode::Live {
            self.saved.config.capture_pose(parameters);
            self.tab = Tab::Pose;
        } else {
            self.saved.config.pose.mode = PoseMode::Live;
        }
        self.reset_motion = true;
    }
    pub fn apply_preset(&mut self, index: usize, mapping: &mut MappingSettings) -> bool {
        let Some(preset) = self.saved.presets.get(index) else {
            return false;
        };
        self.saved.config = preset.rig.clone();
        self.saved.config.reset_filters();
        *mapping = preset.mapping.clone();
        self.reset_motion = true;
        self.message = Some(format!("Applied {}", preset.name));
        self.selected = Some(index);
        true
    }
    pub fn hotkey(&mut self, key: u8, parameters: &[RigParameter], mapping: &mut MappingSettings) {
        if key == crate::hotkeys::TOGGLE_POSE {
            self.toggle_pose(parameters);
        } else if let Some(index) = self
            .saved
            .presets
            .iter()
            .position(|p| p.hotkey == Some(key))
        {
            self.apply_preset(index, mapping);
        }
    }
    pub fn hotkey_keys(&self) -> Vec<u8> {
        if !self.saved.global_hotkeys {
            return Vec::new();
        }
        let mut keys: Vec<_> = self.saved.presets.iter().filter_map(|p| p.hotkey).collect();
        keys.push(crate::hotkeys::TOGGLE_POSE);
        keys
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        parameters: &[RigParameter],
        labels: &BTreeMap<String, String>,
        inputs: &Inputs,
        mapping: &mut MappingSettings,
        can_export: bool,
    ) {
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in [
                (Tab::Inputs, "Inputs"),
                (Tab::Pose, "Pose"),
                (Tab::Presets, "Presets"),
                (Tab::Raw, "Raw"),
            ] {
                ui.selectable_value(&mut self.tab, tab, label);
            }
        });
        if let Some(message) = &self.message {
            ui.label(
                egui::RichText::new(message)
                    .small()
                    .color(egui::Color32::from_rgb(114, 235, 209)),
            );
        }
        if self.saved.config.pose.mode != PoseMode::Live && self.tab != Tab::Pose {
            ui.horizontal_wrapped(|ui| {
                ui.strong(if self.saved.config.pose.mode == PoseMode::Frozen {
                    "POSE FROZEN"
                } else {
                    "MANUAL INPUTS ACTIVE"
                });
                if ui.button("Resume live").clicked() {
                    self.saved.config.pose.mode = PoseMode::Live;
                    self.reset_motion = true;
                }
            });
        }
        match self.tab {
            Tab::Inputs => self.inputs_ui(ui, parameters, labels, inputs),
            Tab::Pose => self.pose_ui(ui, parameters, labels, can_export),
            Tab::Presets => self.presets_ui(ui, parameters, mapping),
            Tab::Raw => {}
        }
    }
    fn filter_ui(&mut self, ui: &mut egui::Ui) {
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Find an input or parameter…")
                .desired_width(f32::INFINITY),
        );
        ui.checkbox(&mut self.mapped_only, "Mapped inputs only");
    }
    fn inputs_ui(
        &mut self,
        ui: &mut egui::Ui,
        parameters: &[RigParameter],
        labels: &BTreeMap<String, String>,
        inputs: &Inputs,
    ) {
        ui.label(
            egui::RichText::new(
                "Expand a control to edit its source, ranges and response. Saved with this model.",
            )
            .small(),
        );
        if ui.button("Save input configuration").clicked() {
            self.save_requested = true;
            self.message = Some("Input configuration saved locally.".into());
        }
        self.filter_ui(ui);
        let search = self.search.to_lowercase();
        let mut names: Vec<_> = rig::INPUT_NAMES
            .iter()
            .map(|n| n.to_string())
            .chain(aria_core::PARAMETER_SPECS.iter().map(|s| s.0.into()))
            .chain(inputs.keys().cloned())
            .collect();
        names.sort();
        names.dedup();
        for p in parameters {
            let binding = self.saved.config.bindings.get(&p.id);
            if self.mapped_only && binding.is_none() {
                continue;
            }
            let label = labels.get(&p.id).map(String::as_str).unwrap_or(&p.id);
            if !format!(
                "{label} {} {}",
                p.id,
                binding.map_or("", |b| b.input.as_str())
            )
            .to_lowercase()
            .contains(&search)
            {
                continue;
            }
            ui.push_id(&p.id, |ui| {
                egui::CollapsingHeader::new(format!("{label}   {:.3}", p.value))
                    .id_salt("input-control")
                    .default_open(crate::smoke_mode())
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&p.id).small());
                        let old = self.saved.config.bindings.get(&p.id).map(|b| b.input.clone());
                        let mut source = old.clone();
                        egui::ComboBox::from_id_salt("source")
                            .width(220.0)
                            .selected_text(source.as_deref().unwrap_or("Manual"))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut source, None, "Manual");
                                for name in &names {
                                    ui.selectable_value(&mut source, Some(name.clone()), name);
                                }
                            });
                        if source != old {
                            if let Some(source) = source {
                                let (min, max) = rig::input_range(&source);
                                let mut binding = Binding::direct(&source, min, max);
                                binding.output_min = p.min;
                                binding.output_max = p.max;
                                self.saved.config.bindings.insert(p.id.clone(), binding);
                            } else {
                                self.saved.config.bindings.remove(&p.id);
                                self.saved.config.manual.insert(p.id.clone(), p.value);
                            }
                        }
                        if let Some(b) = self.saved.config.bindings.get_mut(&p.id) {
                            ui.label(format!(
                                "Source value: {:.3}",
                                inputs.get(&b.input).copied().unwrap_or(0.0)
                            ));
                            range_row(ui, "Input start / end", &mut b.input_min,
                                &mut b.input_max, -1000.0, 1000.0, true);
                            range_row(ui, "Output start / end", &mut b.output_min,
                                &mut b.output_max, p.min, p.max, false);
                            ui.horizontal_wrapped(|ui| {
                                ui.checkbox(&mut b.clamp_input, "Clamp input");
                                ui.checkbox(&mut b.clamp_output, "Clamp output");
                                if ui.small_button("Invert output").clicked() {
                                    std::mem::swap(&mut b.output_min, &mut b.output_max);
                                }
                            });
                            ui.add(egui::Slider::new(&mut b.smoothing_ms, 0.0..=500.0)
                                .text("Smoothing ms"));
                            ui.add(egui::Slider::new(&mut b.dead_zone, 0.0..=0.45)
                                .text("Dead zone"))
                                .on_hover_text("Fraction of the input span ignored around its midpoint. 0 means no dead zone.");
                            ui.add(egui::Slider::new(&mut b.response_curve, 0.1..=4.0)
                                .text("Response curve"))
                                .on_hover_text("1 is linear. Higher values soften movement around the center; endpoints remain unchanged.");
                        } else {
                            let value = self.saved.config.manual.entry(p.id.clone()).or_insert(p.value);
                            value_slider(ui, value, p,
                                self.saved.config.steps.get(&p.id).copied().unwrap_or(0.0));
                        }
                        step_editor(ui, self.saved.config.steps.entry(p.id.clone()).or_insert(0.0), p);
                    });
                ui.add(egui::ProgressBar::new(if p.max > p.min {
                    (p.value - p.min) / (p.max - p.min)
                } else { 0.0 }).desired_height(4.0));
                ui.add_space(4.0);
            });
        }
    }
    fn pose_ui(
        &mut self,
        ui: &mut egui::Ui,
        parameters: &[RigParameter],
        labels: &BTreeMap<String, String>,
        can_export: bool,
    ) {
        let mut active = self.saved.config.pose.mode != PoseMode::Live;
        if ui
            .checkbox(&mut active, "Pose mode / manual inputs")
            .changed()
        {
            self.toggle_pose(parameters);
        }
        if active {
            let mut frozen = self.saved.config.pose.mode == PoseMode::Frozen;
            if ui
                .checkbox(&mut frozen, "Freeze all animation for a picture")
                .changed()
            {
                if frozen {
                    self.saved.config.capture_pose(parameters);
                } else {
                    self.saved.config.pose.mode = PoseMode::Override;
                }
                self.reset_motion = true;
            }
            ui.label(egui::RichText::new(if frozen {"Tracking, physics and breathing are frozen. Adjust any value; the pose stays fixed."} else {"Check Hold on individual controls. Other inputs and physics remain live."}).small());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Capture current pose").clicked() {
                    self.saved.config.capture_pose(parameters);
                    self.reset_motion = true;
                }
                if ui.button("Resume live").clicked() {
                    self.saved.config.pose.mode = PoseMode::Live;
                    self.reset_motion = true;
                }
            });
        } else {
            ui.label("Enable pose mode to hold the current pose and adjust each control.");
        }
        if ui
            .add_enabled(can_export, egui::Button::new("Save transparent PNG…"))
            .on_hover_text("Save the rendered Live2D avatar without the studio UI or background.")
            .clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_file_name("aria-pose.png")
                .add_filter("PNG", &["png"])
                .save_file()
        {
            self.export_png = Some(path);
        }
        if ui.button("Save pose as preset…").clicked() {
            self.tab = Tab::Presets;
        }
        self.filter_ui(ui);
        let search = self.search.to_lowercase();
        for p in parameters {
            if self.mapped_only && !self.saved.config.bindings.contains_key(&p.id) {
                continue;
            }
            let label = labels.get(&p.id).map(String::as_str).unwrap_or(&p.id);
            if !format!("{label} {}", p.id).to_lowercase().contains(&search) {
                continue;
            }
            ui.push_id(&p.id, |ui| {
                ui.horizontal(|ui| {
                    ui.label(label).on_hover_text(&p.id);
                    if self.saved.config.pose.mode == PoseMode::Override {
                        let mut held = self.saved.config.pose.held.contains_key(&p.id);
                        if ui.checkbox(&mut held, "Hold").changed() {
                            if held {
                                self.saved.config.pose.held.insert(p.id.clone(), p.value);
                            } else {
                                self.saved.config.pose.held.remove(&p.id);
                            }
                        }
                    }
                });
                let step = self.saved.config.steps.get(&p.id).copied().unwrap_or(0.0);
                match self.saved.config.pose.mode {
                    PoseMode::Frozen => {
                        let value = self
                            .saved
                            .config
                            .pose
                            .frozen
                            .entry(p.id.clone())
                            .or_insert(p.value);
                        value_slider(ui, value, p, step);
                    }
                    PoseMode::Override if self.saved.config.pose.held.contains_key(&p.id) => {
                        let value = self.saved.config.pose.held.get_mut(&p.id).unwrap();
                        value_slider(ui, value, p, step);
                    }
                    _ => {
                        let mut value = p.value;
                        ui.add_enabled_ui(false, |ui| value_slider(ui, &mut value, p, step));
                    }
                }
                step_editor(
                    ui,
                    self.saved.config.steps.entry(p.id.clone()).or_insert(0.0),
                    p,
                );
                ui.separator();
            });
        }
    }
    fn make_preset(
        &self,
        kind: PresetKind,
        parameters: &[RigParameter],
        mapping: &MappingSettings,
    ) -> Preset {
        let mut rig = self.saved.config.clone();
        if kind == PresetKind::Pose {
            rig.capture_pose(parameters);
        } else if rig.pose.mode == PoseMode::Frozen {
            rig.pose.mode = PoseMode::Live;
        }
        Preset {
            name: self.preset_name.trim().into(),
            kind,
            rig,
            mapping: mapping.clone(),
            hotkey: self.draft_hotkey,
        }
    }
    fn save_new(
        &mut self,
        kind: PresetKind,
        parameters: &[RigParameter],
        mapping: &MappingSettings,
    ) {
        let preset = self.make_preset(kind, parameters, mapping);
        let result = (|| -> anyhow::Result<()> {
            preset.validate(parameters)?;
            anyhow::ensure!(
                self.saved.presets.len() < 128,
                "Maximum 128 presets per model"
            );
            anyhow::ensure!(
                !self
                    .saved
                    .presets
                    .iter()
                    .any(|p| p.name.eq_ignore_ascii_case(&preset.name)),
                "Name already exists. Select it and use Replace selected."
            );
            anyhow::ensure!(
                preset.hotkey.is_none()
                    || !self.saved.presets.iter().any(|p| p.hotkey == preset.hotkey),
                "That hotkey is already assigned"
            );
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.saved.global_hotkeys |= preset.hotkey.is_some();
                self.saved.presets.push(preset);
                self.selected = Some(self.saved.presets.len() - 1);
                self.message = Some("Preset saved locally.".into());
                self.save_requested = true;
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }
    fn presets_ui(
        &mut self,
        ui: &mut egui::Ui,
        parameters: &[RigParameter],
        mapping: &mut MappingSettings,
    ) {
        ui.label("Movement presets store ranges, stepping, response, physics and manual holds. Pose presets also freeze the entire model.");
        ui.add(
            egui::TextEdit::singleline(&mut self.preset_name)
                .hint_text("Preset name")
                .char_limit(80)
                .desired_width(f32::INFINITY),
        );
        hotkey_combo(ui, "new-preset-key", &mut self.draft_hotkey);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Save movement").clicked() {
                self.save_new(PresetKind::Movement, parameters, mapping);
            }
            if ui.button("Save pose").clicked() {
                self.save_new(PresetKind::Pose, parameters, mapping);
            }
        });
        if ui
            .checkbox(
                &mut self.saved.global_hotkeys,
                "Enable global hotkeys (Windows)",
            )
            .changed()
        {
            self.save_requested = true;
        }
        if let Some(error) = &self.hotkey_status {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        ui.label(egui::RichText::new("Ctrl+Alt+F1–F11 apply assigned presets. Ctrl+Alt+P freezes/resumes the pose. Keys are released when disabled or ARIA closes.").small());
        ui.separator();
        for i in 0..self.saved.presets.len() {
            let p = &self.saved.presets[i];
            let title = format!(
                "{} · {:?}{}",
                p.name,
                p.kind,
                p.hotkey.map_or(String::new(), |key| format!(
                    " · {}",
                    crate::hotkeys::label(key)
                ))
            );
            if ui
                .selectable_label(self.selected == Some(i), title)
                .clicked()
            {
                self.selected = Some(i);
                self.preset_name = p.name.clone();
                self.draft_hotkey = p.hotkey;
            }
        }
        if let Some(index) = self.selected.filter(|&i| i < self.saved.presets.len()) {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Apply selected").clicked() {
                    self.apply_preset(index, mapping);
                }
                if ui.button("Replace selected").clicked() {
                    let preset =
                        self.make_preset(self.saved.presets[index].kind, parameters, mapping);
                    let conflict = self.saved.presets.iter().enumerate().any(|(i, p)| {
                        i != index
                            && (p.name.eq_ignore_ascii_case(&preset.name)
                                || (preset.hotkey.is_some() && p.hotkey == preset.hotkey))
                    });
                    if conflict {
                        self.message =
                            Some("Name or hotkey is already used by another preset.".into());
                    } else if let Err(error) = preset.validate(parameters) {
                        self.message = Some(error.to_string());
                    } else {
                        self.saved.global_hotkeys |= preset.hotkey.is_some();
                        self.saved.presets[index] = preset;
                        self.message = Some("Preset replaced with current settings.".into());
                        self.save_requested = true;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Rename / assign key").clicked() {
                    let mut preset = self.saved.presets[index].clone();
                    preset.name = self.preset_name.trim().into();
                    preset.hotkey = self.draft_hotkey;
                    let conflict = self.saved.presets.iter().enumerate().any(|(i, p)| {
                        i != index
                            && (p.name.eq_ignore_ascii_case(&preset.name)
                                || (preset.hotkey.is_some() && p.hotkey == preset.hotkey))
                    });
                    if conflict {
                        self.message = Some("Name or hotkey is already in use.".into());
                    } else if let Err(error) = preset.validate(parameters) {
                        self.message = Some(error.to_string());
                    } else {
                        self.saved.global_hotkeys |= preset.hotkey.is_some();
                        self.saved.presets[index] = preset;
                        self.message = Some("Name and hotkey saved.".into());
                        self.save_requested = true;
                    }
                }
                if ui.button("Delete selected").clicked() {
                    self.saved.presets.remove(index);
                    self.selected = None;
                    self.message = Some("Preset deleted.".into());
                    self.save_requested = true;
                }
            });
            if self.selected.is_some()
                && ui.button("Export selected preset…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_file_name("aria-preset.json")
                    .add_filter("ARIA preset", &["json"])
                    .save_file()
            {
                let file = PresetFile {
                    version: 1,
                    model_key: self.model_key.clone(),
                    preset: self.saved.presets[index].clone(),
                };
                self.message = Some(
                    match serde_json::to_vec_pretty(&file)
                        .map_err(anyhow::Error::from)
                        .and_then(|bytes| std::fs::write(path, bytes).map_err(Into::into))
                    {
                        Ok(()) => "Preset exported.".into(),
                        Err(error) => format!("Export failed: {error:#}"),
                    },
                );
            }
        }
        if ui.button("Import preset…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("ARIA preset", &["json"])
                .pick_file()
        {
            let result = aria_model::read_bounded(&path, 2 * 1024 * 1024)
                .and_then(|bytes| PresetFile::decode(&bytes, &self.model_key, parameters));
            match result {
                Ok(mut file) if self.saved.presets.len() < 128 => {
                    let name = file.preset.name.clone();
                    let mut n = 2;
                    while self
                        .saved
                        .presets
                        .iter()
                        .any(|p| p.name.eq_ignore_ascii_case(&file.preset.name))
                    {
                        file.preset.name =
                            format!("{} ({n})", name.chars().take(24).collect::<String>());
                        n += 1;
                    }
                    self.preset_name = file.preset.name.clone();
                    self.draft_hotkey = None;
                    self.saved.presets.push(file.preset);
                    self.selected = Some(self.saved.presets.len() - 1);
                    self.message =
                        Some("Imported. Apply to activate; assign a hotkey if wanted.".into());
                    self.save_requested = true;
                }
                Ok(_) => self.message = Some("Maximum 128 presets per model.".into()),
                Err(error) => self.message = Some(format!("Import failed: {error:#}")),
            }
        }
    }
}

fn range_row(
    ui: &mut egui::Ui,
    label: &str,
    start: &mut f32,
    end: &mut f32,
    min: f32,
    max: f32,
    nonzero: bool,
) {
    ui.label(egui::RichText::new(label).small());
    let before = (*start, *end);
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(start)
                .speed(0.01)
                .range(min..=max)
                .prefix("Start "),
        );
        ui.add(
            egui::DragValue::new(end)
                .speed(0.01)
                .range(min..=max)
                .prefix("End "),
        );
    });
    if nonzero && (*end - *start).abs() < 1e-6 {
        (*start, *end) = before;
        ui.colored_label(egui::Color32::LIGHT_RED, "Start and end must differ.");
    }
}
fn step_editor(ui: &mut egui::Ui, step: &mut f32, p: &RigParameter) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Step").small());
        ui.add(
            egui::DragValue::new(step)
                .speed(0.001)
                .range(0.0..=(p.max - p.min).max(0.001))
                .max_decimals(4),
        );
        ui.label(egui::RichText::new("0 = continuous").small());
    });
}
fn value_slider(ui: &mut egui::Ui, value: &mut f32, p: &RigParameter, step: f32) {
    let mut slider = egui::Slider::new(value, p.min..=p.max)
        .max_decimals(4)
        .clamping(egui::SliderClamping::Always);
    if step > 0.0 {
        slider = slider.step_by(f64::from(step));
    }
    if ui.add(slider).changed() {
        *value = snap(*value, p.min, p.max, step);
    }
}
fn hotkey_combo(ui: &mut egui::Ui, id: &str, key: &mut Option<u8>) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(key.map_or_else(|| "No hotkey".into(), crate::hotkeys::label))
        .show_ui(ui, |ui| {
            ui.selectable_value(key, None, "No hotkey");
            for n in 1..=11 {
                ui.selectable_value(key, Some(n), crate::hotkeys::label(n));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preset_hotkeys_restore_tuning_and_full_poses_after_reload() {
        let mut parameters =
            aria_core::movement::preview_parameters(aria_core::Parameters::default());
        let mut monitor = InputMonitor::new(
            "preview-v1".into(),
            RigConfig::from_parameters(&parameters),
            None,
            &parameters,
        );
        let mut mapping = MappingSettings {
            head_gain: 1.7,
            ..Default::default()
        };
        monitor.preset_name = "Motion".into();
        monitor.draft_hotkey = Some(2);
        monitor.saved.config.steps.insert("ParamAngleX".into(), 5.0);
        monitor.save_new(PresetKind::Movement, &parameters, &mapping);
        assert!(monitor.save_requested && monitor.saved.global_hotkeys);
        parameters[0].value = 13.0;
        monitor.preset_name = "Portrait".into();
        monitor.draft_hotkey = Some(3);
        monitor.save_new(PresetKind::Pose, &parameters, &mapping);
        let saved: SavedRig =
            serde_json::from_slice(&serde_json::to_vec(&monitor.saved).unwrap()).unwrap();
        let mut restored = InputMonitor::new(
            "preview-v1".into(),
            RigConfig::from_parameters(&parameters),
            Some(saved),
            &parameters,
        );
        restored.hotkey(3, &parameters, &mut mapping);
        assert_eq!(restored.saved.config.pose.mode, PoseMode::Frozen);
        assert_eq!(restored.saved.config.pose.frozen["ParamAngleX"], 13.0);
        restored.hotkey(2, &parameters, &mut mapping);
        assert_eq!(restored.saved.config.pose.mode, PoseMode::Live);
        assert_eq!(restored.saved.config.steps["ParamAngleX"], 5.0);
        assert_eq!(mapping.head_gain, 1.7);
        restored.hotkey(crate::hotkeys::TOGGLE_POSE, &parameters, &mut mapping);
        assert_eq!(restored.saved.config.pose.mode, PoseMode::Frozen);
        restored.hotkey(crate::hotkeys::TOGGLE_POSE, &parameters, &mut mapping);
        assert_eq!(restored.saved.config.pose.mode, PoseMode::Live);
        restored.preset_name = "Duplicate key".into();
        restored.draft_hotkey = Some(2);
        restored.save_new(PresetKind::Movement, &parameters, &mapping);
        assert_eq!(restored.saved.presets.len(), 2);
        assert!(restored.message.unwrap().contains("already assigned"));
    }
}
