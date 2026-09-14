use aria_core::{
    MappingSettings,
    movement::{PoseMode, Preset, PresetFile, PresetKind, RigConfig, SavedRig, snap},
    rig::{self, Binding, Inputs, RigParameter},
};
use eframe::egui;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub enum Tab {
    #[default]
    Inputs,
    Pose,
    Physics,
    Expressions,
    Presets,
    Raw,
    Items,
    Effects,
    Images,
    Vrm,
    Layers,
    Customize,
    Microphone,
    Controller,
}
pub struct InputMonitor {
    pub vts: crate::vts_panel::Panel,
    pub vbridger: crate::vbridger_panel::Panel,
    pub vbridger_runtime: aria_core::vbridger::Runtime,
    pub layers_panel: crate::layers_panel::Panel,
    pub customization_panel: crate::customization_panel::Panel,
    pub setup_tracking_requested: bool,
    pub effect_requests: Vec<u64>,
    pub saved: SavedRig,
    pub model_key: String,
    pub tab: Tab,
    pub message: Option<String>,
    pub hotkey_status: Option<String>,
    pub save_requested: bool,
    pub reset_motion: bool,
    pub reset_item_rules: bool,
    pub export_png: Option<PathBuf>,
    search: String,
    mapped_only: bool,
    preset_name: String,
    draft_hotkey: Option<u8>,
    selected: Option<usize>,
    pub physics_groups: Vec<aria_core::physics::GroupInfo>,
    physics_defaults: aria_core::physics::PhysicsSettings,
    physics_panel: crate::physics_panel::PhysicsPanel,
    pub expressions: crate::expressions_panel::ExpressionsPanel,
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
        let physics_defaults = default.physics.clone();
        let mut saved = saved
            .filter(|s| match s.validate(parameters) {
                Ok(()) => true,
                Err(error) => {
                    message = Some(format!("Saved configuration was not applied: {error:#}"));
                    false
                }
            })
            .unwrap_or(SavedRig {
                config: default.clone(),
                input_compat_revision: 1,
                ..Default::default()
            });
        let repaired = saved.upgrade_inputs(&default);
        if repaired > 0 {
            message = Some(format!(
                "Restored {repaired} previously unsupported input assignments. Your existing mappings were preserved."
            ));
        }
        Self {
            vts: Default::default(),
            vbridger: Default::default(),
            vbridger_runtime: Default::default(),
            setup_tracking_requested: false,
            effect_requests: Vec::new(),
            saved,
            model_key,
            tab: Tab::Inputs,
            layers_panel: Default::default(),
            customization_panel: Default::default(),
            message,
            hotkey_status: None,
            save_requested: repaired > 0,
            reset_motion: true,
            reset_item_rules: true,
            export_png: None,
            search: String::new(),
            mapped_only: true,
            preset_name: String::new(),
            draft_hotkey: None,
            selected: None,
            physics_groups: Vec::new(),
            physics_defaults,
            physics_panel: Default::default(),
            expressions: Default::default(),
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
        let mut candidate = self.saved.clone();
        if preset.kind == PresetKind::Appearance {
            candidate
                .config
                .customization
                .values
                .clone_from(&preset.rig.customization.values);
            candidate.config.layers.clone_from(&preset.rig.layers);
        } else {
            candidate.config = preset.rig.clone();
        }
        if let Err(error) = candidate.validate_image_hotkeys() {
            self.message = Some(format!("Preset not applied: {error:#}"));
            return false;
        }
        if preset.kind != PresetKind::Appearance {
            self.vbridger_runtime.reset();
            self.reset_item_rules = true;
            candidate.config.reset_filters();
            *mapping = preset.mapping.clone();
            self.reset_motion = true;
        }
        self.saved.config = candidate.config;
        self.message = Some(format!("Applied {}", preset.name));
        self.selected = Some(index);
        self.save_requested = true;
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
    pub fn hotkey_action(
        &mut self,
        action: crate::hotkeys::Action,
        parameters: &[RigParameter],
        mapping: &mut MappingSettings,
    ) {
        match action {
            crate::hotkeys::Action::Profiles(_) => unreachable!("Workspace routes profile hotkeys"),
            crate::hotkeys::Action::Preset(key) => self.hotkey(key, parameters, mapping),
            crate::hotkeys::Action::TogglePose => self.toggle_pose(parameters),
            crate::hotkeys::Action::Expression(id) => {
                self.message = self.expressions.toggle(&id, &mut self.saved);
                self.save_requested = true;
            }
            crate::hotkeys::Action::Layers(id) => {
                self.saved.config.layers.toggle(id);
                self.save_requested = true;
            }
            crate::hotkeys::Action::ItemToggle(id) => {
                if let Some(item) = self.saved.config.items.iter_mut().find(|i| i.id == id) {
                    item.visible = !item.visible;
                    self.save_requested = true;
                }
            }
            crate::hotkeys::Action::Effect(id) => self.effect_requests.push(id),
            crate::hotkeys::Action::Image(id) => {
                self.saved.config.images.toggle(id);
                self.save_requested = true;
            }
        }
    }
    pub fn hotkey_keys(&self) -> Vec<crate::hotkeys::Registration> {
        use crate::hotkeys::{Action, Registration};
        use aria_core::shortcuts::Shortcut;
        if !self.saved.global_hotkeys {
            return Vec::new();
        }
        let mut keys: Vec<_> = self
            .saved
            .presets
            .iter()
            .filter_map(|p| p.hotkey)
            .map(|key| Registration {
                shortcut: Shortcut::preset(key),
                action: Action::Preset(key),
            })
            .collect();
        keys.push(Registration {
            shortcut: Shortcut::pose(),
            action: Action::TogglePose,
        });
        for entry in &self.expressions.entries {
            if let Some(&shortcut) = self.saved.expression_hotkeys.get(&entry.file.id) {
                keys.push(Registration {
                    shortcut,
                    action: Action::Expression(entry.file.id.clone()),
                });
            }
        }
        for group in &self.saved.config.layers.groups {
            if let Some(&shortcut) = self.saved.layer_hotkeys.get(&group.id) {
                keys.push(Registration {
                    shortcut,
                    action: Action::Layers(group.id),
                });
            }
        }
        for item in &self.saved.config.items {
            if let Some(&shortcut) = self.saved.item_hotkeys.get(&item.id) {
                keys.push(Registration {
                    shortcut,
                    action: Action::ItemToggle(item.id),
                });
            }
        }
        keys.extend(self.saved.effects.designs.iter().filter_map(|d| {
            d.hotkey.map(|shortcut| Registration {
                shortcut,
                action: Action::Effect(d.id),
            })
        }));
        keys.extend(self.saved.config.images.states.iter().filter_map(|s| {
            s.hotkey.map(|shortcut| Registration {
                shortcut,
                action: Action::Image(s.id),
            })
        }));
        keys
    }
    fn expression_uses_key(&self, key: Option<u8>) -> bool {
        key.is_some_and(|key| {
            self.saved
                .expression_hotkeys
                .values()
                .chain(self.saved.item_hotkeys.values())
                .chain(self.saved.layer_hotkeys.values())
                .chain(
                    self.saved
                        .effects
                        .designs
                        .iter()
                        .filter_map(|d| d.hotkey.as_ref()),
                )
                .chain(
                    self.saved
                        .config
                        .images
                        .states
                        .iter()
                        .filter_map(|s| s.hotkey.as_ref()),
                )
                .chain(
                    self.saved
                        .vts
                        .actions
                        .iter()
                        .filter_map(|h| h.shortcut.as_ref()),
                )
                .any(|shortcut| *shortcut == aria_core::shortcuts::Shortcut::preset(key))
        })
    }
    pub fn navigation(&mut self, ui: &mut egui::Ui, kind: Option<crate::avatar_import::Kind>) {
        if matches!(self.tab, Tab::Layers | Tab::Customize)
            && kind != Some(crate::avatar_import::Kind::Live2d)
        {
            self.tab = Tab::Inputs;
        }
        if kind == Some(crate::avatar_import::Kind::Live2d) && self.tab == Tab::Images {
            self.tab = Tab::Physics;
        }
        if kind == Some(crate::avatar_import::Kind::Images)
            && matches!(self.tab, Tab::Physics | Tab::Expressions)
        {
            self.tab = Tab::Images;
        }
        if kind == Some(crate::avatar_import::Kind::Vrm) && self.tab == Tab::Images {
            self.tab = Tab::Vrm;
        }
        if kind != Some(crate::avatar_import::Kind::Vrm) && self.tab == Tab::Vrm {
            self.tab = Tab::Inputs;
        }
        let mut group = match self.tab {
            Tab::Inputs | Tab::Microphone | Tab::Controller | Tab::Raw => 0,
            Tab::Physics
            | Tab::Expressions
            | Tab::Images
            | Tab::Vrm
            | Tab::Layers
            | Tab::Customize => 1,
            Tab::Items | Tab::Effects => 2,
            Tab::Pose | Tab::Presets => 3,
        };
        let previous = group;
        crate::theme::segments(
            ui,
            &mut group,
            &[(0, "Tracking"), (1, "Avatar"), (2, "Stage"), (3, "Poses")],
        );
        let pages: &[(Tab, &str)] = match group {
            0 => &[
                (Tab::Inputs, "Inputs"),
                (Tab::Microphone, "Microphone"),
                (Tab::Controller, "Controller"),
                (Tab::Raw, "Diagnostics"),
            ],
            1 => match kind {
                Some(crate::avatar_import::Kind::Images) => &[(Tab::Images, "Artwork & actions")],
                Some(crate::avatar_import::Kind::Live2d) => &[
                    (Tab::Customize, "Customize"),
                    (Tab::Physics, "Physics"),
                    (Tab::Expressions, "Expressions"),
                    (Tab::Layers, "Layers"),
                ],
                Some(crate::avatar_import::Kind::Vrm) => &[
                    (Tab::Vrm, "View"),
                    (Tab::Physics, "Springs"),
                    (Tab::Expressions, "Expressions"),
                ],
                None => &[(Tab::Physics, "Physics"), (Tab::Images, "PNG / GIF")],
            },
            2 => &[(Tab::Items, "Objects"), (Tab::Effects, "Throws & sprays")],
            _ => &[(Tab::Pose, "Pose controls"), (Tab::Presets, "Presets")],
        };
        if previous != group {
            self.tab = pages[0].0;
        }
        crate::theme::segments(ui, &mut self.tab, pages);
        ui.add_space(4.0);
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
        if self.tab == Tab::Inputs
            && crate::help::control(ui, "tracking-guide", |ui| {
                ui.button("Set up tracking for this avatar…")
            })
            .clicked()
        {
            self.setup_tracking_requested = true;
        }
        if self.tab == Tab::Inputs
            && crate::help::control(ui, "vbridger", |ui| {
                ui.button("VBridger config & equations…")
            })
            .clicked()
        {
            self.vbridger.open = true;
        }
        crate::help::label(
            ui,
            "About this tab",
            match self.tab {
                Tab::Inputs => "inputs",
                Tab::Pose => "pose",
                Tab::Physics => {
                    if self.model_key.starts_with("vrm:") {
                        "vrm-physics"
                    } else {
                        "physics"
                    }
                }
                Tab::Vrm => "vrm-view",
                Tab::Layers => "live2d-layers",
                Tab::Customize => "live2d-customization",
                Tab::Expressions => {
                    if self.model_key.starts_with("vrm:") {
                        "vrm-expressions"
                    } else {
                        "expressions"
                    }
                }
                Tab::Presets => "presets",
                Tab::Raw => "diagnostics",
                Tab::Items => "png-items",
                Tab::Effects => "effects",
                Tab::Images => "image-actions",
                Tab::Microphone => "microphone",
                Tab::Controller => "controller",
            },
        );
        if let Some(message) = &self.message {
            ui.label(
                egui::RichText::new(message)
                    .small()
                    .color(crate::theme::mint()),
            );
        }
        if self.saved.config.pose.mode != PoseMode::Live && self.tab != Tab::Pose {
            ui.horizontal_wrapped(|ui| {
                ui.strong(if self.saved.config.pose.mode == PoseMode::Frozen {
                    "POSE FROZEN"
                } else {
                    "MANUAL INPUTS ACTIVE"
                });
                if crate::help::control(ui, "pose", |ui| ui.button("Resume live")).clicked() {
                    self.saved.config.pose.mode = PoseMode::Live;
                    self.reset_motion = true;
                }
            });
        }
        match self.tab {
            Tab::Inputs => self.inputs_ui(ui, parameters, labels, inputs),
            Tab::Pose => self.pose_ui(ui, parameters, labels, can_export),
            Tab::Presets => self.presets_ui(ui, parameters, mapping),
            Tab::Physics if !self.model_key.starts_with("vrm:") => {
                let actions = self.physics_panel.show(
                    ui,
                    &mut self.saved.config.physics,
                    &self.physics_defaults,
                    &self.physics_groups,
                );
                if actions.save {
                    self.save_requested = true;
                    self.message = Some("Overall and group physics settings saved locally.".into());
                }
                self.reset_motion |= actions.settle;
                if actions.presets {
                    self.tab = Tab::Presets;
                }
            }
            Tab::Expressions => {
                let actions = self.expressions.show(ui, &mut self.saved, parameters);
                self.save_requested |= actions.save;
                if actions.message.is_some() {
                    self.message = actions.message;
                }
                if let Some(error) = &self.hotkey_status {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            }
            Tab::Raw
            | Tab::Items
            | Tab::Effects
            | Tab::Images
            | Tab::Microphone
            | Tab::Controller
            | Tab::Vrm
            | Tab::Layers
            | Tab::Customize
            | Tab::Physics => {}
        }
    }
    fn filter_ui(&mut self, ui: &mut egui::Ui) {
        crate::help::control(ui, "inputs", |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Find an input or parameter…")
                    .desired_width(f32::INFINITY),
            )
        });
        crate::help::control(ui, "inputs", |ui| {
            ui.checkbox(&mut self.mapped_only, "Mapped inputs only")
        });
    }
    fn inputs_ui(
        &mut self,
        ui: &mut egui::Ui,
        parameters: &[RigParameter],
        labels: &BTreeMap<String, String>,
        inputs: &Inputs,
    ) {
        if !self.saved.config.customization.values.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{} appearance overrides active",
                    self.saved.config.customization.values.len()
                ));
                if ui.small_button("Edit appearance").clicked() {
                    self.tab = Tab::Customize;
                }
            });
        }
        ui.label(
            egui::RichText::new(
                "Expand a control to edit its source, ranges and response. Saved with this model.",
            )
            .small(),
        );
        if crate::help::control(ui, "profiles", |ui| ui.button("Save input configuration"))
            .clicked()
        {
            self.save_requested = true;
            self.message = Some("Input configuration saved locally.".into());
        }
        self.filter_ui(ui);
        let mut names: Vec<_> = rig::INPUT_NAMES
            .iter()
            .map(|n| n.to_string())
            .chain(
                aria_core::controller::INPUT_NAMES
                    .iter()
                    .map(|n| n.to_string()),
            )
            .chain(rig::FACE_ALIASES.iter().map(|(n, _)| n.to_string()))
            .chain(aria_core::PARAMETER_SPECS.iter().map(|s| s.0.into()))
            .chain(inputs.keys().cloned())
            .chain(
                self.saved
                    .config
                    .vbridger
                    .outputs
                    .iter()
                    .map(|o| o.name.clone()),
            )
            .collect();
        names.sort();
        names.dedup();
        let mut shown = 0;
        for category in CATEGORIES {
            let visible: Vec<_> = parameters
                .iter()
                .filter(|p| self.matches(p, labels) && control_category(p, labels) == category)
                .collect();
            if visible.is_empty() {
                continue;
            }
            shown += visible.len();
            let title = format!("{category} · {}", visible.len());
            crate::theme::card(ui, |ui| {
                egui::CollapsingHeader::new(egui::RichText::new(title).strong())
                .id_salt(("input-category", category)).default_open(category == CATEGORIES[0])
                .open((!self.search.trim().is_empty()).then_some(true))
                .show(ui, |ui| {
            for p in visible {
            let label = labels.get(&p.id).map(String::as_str).unwrap_or(&p.id);
            ui.push_id(&p.id, |ui| {
                crate::help::parameter_header(ui, p, label,
                    crate::smoke_mode() && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("inputs"), |ui| {
                        ui.label(egui::RichText::new(&p.id).small());
                        crate::help::label(ui, "Source", "source");
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
                                let (min, max) = self.saved.config.vbridger.output(&source).map(|o|(o.min,o.max)).unwrap_or_else(||rig::input_range(&source));
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
                                crate::help::control(ui, "clamps", |ui| ui.checkbox(&mut b.clamp_input, "Clamp input"));
                                crate::help::control(ui, "clamps", |ui| ui.checkbox(&mut b.clamp_output, "Clamp output"));
                                if crate::help::control(ui, "ranges", |ui| ui.small_button("Invert output")).clicked() {
                                    std::mem::swap(&mut b.output_min, &mut b.output_max);
                                }
                            });
                            let responsive = self.saved.config.mouth_response.enabled && aria_core::speech::is_mouth_input(&b.input);
                            ui.add_enabled_ui(!responsive, |ui| {
                                crate::help::control(ui, "smoothing", |ui| ui.add(egui::Slider::new(&mut b.smoothing_ms, 0.0..=500.0)
                                    .text("Smoothing ms")));
                            });
                            if responsive {
                                crate::help::control(ui, "mouth-response", |ui| ui.small("Uses Tracking → Mouth response. This saved smoothing value is restored when responsive speech is off."));
                            }
                            crate::help::control(ui, "dead-zone", |ui| ui.add(egui::Slider::new(&mut b.dead_zone, 0.0..=0.45)
                                .text("Dead zone")))
                                .on_hover_text("Fraction of the input span ignored around its midpoint. 0 means no dead zone.");
                            crate::help::control(ui, "curve", |ui| ui.add(egui::Slider::new(&mut b.response_curve, 0.1..=4.0)
                                .text("Response curve")))
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
            });
            });
            ui.add_space(4.0);
        }
        if shown == 0 {
            crate::theme::caption(ui, "No controls match this filter.");
        }
    }
    fn matches(&self, p: &RigParameter, labels: &BTreeMap<String, String>) -> bool {
        let binding = self.saved.config.bindings.get(&p.id);
        (!self.mapped_only || binding.is_some())
            && format!(
                "{} {} {}",
                p.id,
                labels.get(&p.id).map_or("", String::as_str),
                binding.map_or("", |b| b.input.as_str())
            )
            .to_lowercase()
            .contains(&self.search.trim().to_lowercase())
    }
    fn pose_ui(
        &mut self,
        ui: &mut egui::Ui,
        parameters: &[RigParameter],
        labels: &BTreeMap<String, String>,
        can_export: bool,
    ) {
        crate::theme::category(ui, "pose-capture", "Pose & capture", true, |ui| {
            let mut active = self.saved.config.pose.mode != PoseMode::Live;
            if crate::help::control(ui, "pose", |ui| {
                ui.checkbox(&mut active, "Pose mode / manual inputs")
            })
            .changed()
            {
                self.toggle_pose(parameters);
            }
            if active {
                let mut frozen = self.saved.config.pose.mode == PoseMode::Frozen;
                if crate::help::control(ui, "pose", |ui| {
                    ui.checkbox(&mut frozen, "Freeze all animation for a picture")
                })
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
                    if crate::help::control(ui, "pose", |ui| ui.button("Capture current pose"))
                        .clicked()
                    {
                        self.saved.config.capture_pose(parameters);
                        self.reset_motion = true;
                    }
                    if crate::help::control(ui, "pose", |ui| ui.button("Resume live")).clicked() {
                        self.saved.config.pose.mode = PoseMode::Live;
                        self.reset_motion = true;
                    }
                });
            } else {
                ui.label("Enable pose mode to hold the current pose and adjust each control.");
            }
            if crate::help::control(ui, "png", |ui| {
                ui.add_enabled(can_export, egui::Button::new("Save transparent PNG…"))
            })
            .on_hover_text("Save the rendered avatar and visible stage objects without the studio UI or background.")
            .clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_file_name("aria-pose.png")
                    .add_filter("PNG", &["png"])
                    .save_file()
            {
                self.export_png = Some(path);
            }
            if crate::help::control(ui, "presets", |ui| ui.button("Save pose as preset…")).clicked()
            {
                self.tab = Tab::Presets;
            }
        });
        self.filter_ui(ui);
        for category in CATEGORIES {
            let visible: Vec<_> = parameters
                .iter()
                .filter(|p| self.matches(p, labels) && control_category(p, labels) == category)
                .collect();
            if visible.is_empty() {
                continue;
            }
            let title = format!("{category} · {}", visible.len());
            crate::theme::card(ui, |ui| {
                egui::CollapsingHeader::new(egui::RichText::new(title).strong())
                    .id_salt(("pose-category", category))
                    .default_open(category == CATEGORIES[0])
                    .open((!self.search.trim().is_empty()).then_some(true))
                    .show(ui, |ui| {
                        for p in visible {
                            let label = labels.get(&p.id).map(String::as_str).unwrap_or(&p.id);
                            ui.push_id(&p.id, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(label).on_hover_text(&p.id);
                                    crate::help::context_button(ui, "parameter", || {
                                        crate::help::parameter_context(p, label)
                                    });
                                    if self.saved.config.pose.mode == PoseMode::Override {
                                        let mut held =
                                            self.saved.config.pose.held.contains_key(&p.id);
                                        if crate::help::control(ui, "pose", |ui| {
                                            ui.checkbox(&mut held, "Hold")
                                        })
                                        .changed()
                                        {
                                            if held {
                                                self.saved
                                                    .config
                                                    .pose
                                                    .held
                                                    .insert(p.id.clone(), p.value);
                                            } else {
                                                self.saved.config.pose.held.remove(&p.id);
                                            }
                                        }
                                    }
                                });
                                let step =
                                    self.saved.config.steps.get(&p.id).copied().unwrap_or(0.0);
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
                                    PoseMode::Override
                                        if self.saved.config.pose.held.contains_key(&p.id) =>
                                    {
                                        let value =
                                            self.saved.config.pose.held.get_mut(&p.id).unwrap();
                                        value_slider(ui, value, p, step);
                                    }
                                    _ => {
                                        let mut value = p.value;
                                        ui.add_enabled_ui(false, |ui| {
                                            value_slider(ui, &mut value, p, step)
                                        });
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
                    });
            });
            ui.add_space(4.0);
        }
    }
    fn make_preset(
        &self,
        kind: PresetKind,
        parameters: &[RigParameter],
        mapping: &MappingSettings,
    ) -> Preset {
        let mut rig = if kind == PresetKind::Appearance {
            RigConfig {
                customization: aria_core::customization::Config {
                    values: self.saved.config.customization.values.clone(),
                    ..Default::default()
                },
                layers: self.saved.config.layers.clone(),
                ..Default::default()
            }
        } else {
            self.saved.config.clone()
        };
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
    pub fn save_appearance(&mut self, name: String, parameters: &[RigParameter]) {
        self.preset_name = name;
        self.draft_hotkey = None;
        self.save_new(
            PresetKind::Appearance,
            parameters,
            &MappingSettings::default(),
        );
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
                !self.expression_uses_key(preset.hotkey)
                    && (preset.hotkey.is_none()
                        || !self.saved.presets.iter().any(|p| p.hotkey == preset.hotkey)),
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
        crate::theme::category(ui, "preset-create", "Create a preset", true, |ui| {
            crate::theme::caption(
                ui,
                "Movement stores tuning and physics. Pose also freezes the model. Appearance stores customization values, layer visibility and colors while keeping tracking live.",
            );
            crate::help::control(ui, "presets", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.preset_name)
                        .hint_text("Preset name")
                        .char_limit(80)
                        .desired_width(f32::INFINITY),
                )
            });
            hotkey_combo(ui, "new-preset-key", &mut self.draft_hotkey);
            ui.horizontal_wrapped(|ui| {
                if crate::help::control(ui, "presets", |ui| ui.button("Save movement")).clicked() {
                    self.save_new(PresetKind::Movement, parameters, mapping);
                }
                if crate::help::control(ui, "presets", |ui| ui.button("Save pose")).clicked() {
                    self.save_new(PresetKind::Pose, parameters, mapping);
                }
                if self.model_key.starts_with("moc3:")
                    && crate::help::control(ui, "live2d-customization", |ui| {
                        ui.button("Save appearance")
                    })
                    .clicked()
                {
                    self.save_new(PresetKind::Appearance, parameters, mapping);
                }
            });
        });
        crate::theme::category(ui, "preset-shortcuts", "Keyboard shortcuts", false, |ui| {
            if crate::help::control(ui, "hotkeys", |ui| {
                ui.checkbox(
                    &mut self.saved.global_hotkeys,
                    "Enable global hotkeys (Windows)",
                )
            })
            .changed()
            {
                self.save_requested = true;
            }
            if let Some(error) = &self.hotkey_status {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            ui.label(egui::RichText::new("Ctrl+Alt+F1–F11 apply assigned presets. Ctrl+Alt+P freezes/resumes the pose. Keys are released when disabled or ARIA closes.").small());
        });
        crate::theme::category(ui, "preset-library", "Saved presets", true, |ui| {
            if self.saved.presets.is_empty() {
                crate::theme::caption(ui, "Your saved movement and poses will appear here.");
            }
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
                    if crate::help::control(ui, "presets", |ui| ui.button("Apply selected"))
                        .clicked()
                    {
                        self.apply_preset(index, mapping);
                    }
                    if crate::help::control(ui, "presets", |ui| ui.button("Replace selected"))
                        .clicked()
                    {
                        let preset =
                            self.make_preset(self.saved.presets[index].kind, parameters, mapping);
                        let conflict = self.saved.presets.iter().enumerate().any(|(i, p)| {
                            i != index
                                && (p.name.eq_ignore_ascii_case(&preset.name)
                                    || (preset.hotkey.is_some() && p.hotkey == preset.hotkey))
                        });
                        if conflict || self.expression_uses_key(preset.hotkey) {
                            self.message =
                                Some("Name or hotkey is already assigned to a preset, expression or PNG toggle.".into());
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
                    if crate::help::control(ui, "presets", |ui| ui.button("Rename / assign key"))
                        .clicked()
                    {
                        let mut preset = self.saved.presets[index].clone();
                        preset.name = self.preset_name.trim().into();
                        preset.hotkey = self.draft_hotkey;
                        let conflict = self.saved.presets.iter().enumerate().any(|(i, p)| {
                            i != index
                                && (p.name.eq_ignore_ascii_case(&preset.name)
                                    || (preset.hotkey.is_some() && p.hotkey == preset.hotkey))
                        });
                        if conflict || self.expression_uses_key(preset.hotkey) {
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
                    if crate::help::control(ui, "presets", |ui| ui.button("Delete selected"))
                        .clicked()
                    {
                        self.saved.presets.remove(index);
                        self.selected = None;
                        self.message = Some("Preset deleted.".into());
                        self.save_requested = true;
                    }
                });
                if self.selected.is_some()
                    && crate::help::control(ui, "presets", |ui| {
                        ui.button("Export selected preset…")
                    })
                    .clicked()
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
            if crate::help::control(ui, "presets", |ui| ui.button("Import preset…")).clicked()
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
        });
    }
}

const CATEGORIES: [&str; 7] = [
    "Head & body",
    "Eyes & brows",
    "Mouth & expression",
    "Hair, ears & tail",
    "Clothing & accessories",
    "Other controls",
    "Controller & hands",
];

fn control_category(p: &RigParameter, labels: &BTreeMap<String, String>) -> &'static str {
    let name = format!("{} {}", p.id, labels.get(&p.id).map_or("", String::as_str)).to_lowercase();
    // Presentation only: never change a binding based on a guessed category.
    if ["pad", "stick", "button", "lindex", "rindex"]
        .iter()
        .any(|s| name.contains(s))
        || ["ParamL1", "ParamL2", "ParamR1", "ParamR2"].contains(&p.id.as_str())
    {
        CATEGORIES[6]
    } else if ["hair", "ear", "tail"].iter().any(|s| name.contains(s)) {
        CATEGORIES[3]
    } else if ["eye", "brow", "pupil", "iris"]
        .iter()
        .any(|s| name.contains(s))
    {
        CATEGORIES[1]
    } else if ["mouth", "lip", "tongue", "cheek", "smile", "jaw"]
        .iter()
        .any(|s| name.contains(s))
    {
        CATEGORIES[2]
    } else if ["angle", "body", "head", "breath", "neck"]
        .iter()
        .any(|s| name.contains(s))
    {
        CATEGORIES[0]
    } else if ["cloth", "hood", "skirt", "ribbon", "sleeve", "accessor"]
        .iter()
        .any(|s| name.contains(s))
    {
        CATEGORIES[4]
    } else {
        CATEGORIES[5]
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
    ui.horizontal_wrapped(|ui| {
        ui.small(label);
        crate::help::context_button(ui, "ranges", || format!(
            "{label}\nStart: {start}\nEnd: {end}\nAllowed editor bounds: {min} to {max}\n{}\n\nValues captured when you opened this help window.",
            if nonzero { "Input endpoints must differ. Source units depend on the chosen signal." } else { "Output uses this avatar parameter's authored units. Reversed endpoints invert the binding." }
        ));
    });
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
        crate::help::button(ui, "step");
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
    crate::help::label(ui, "Preset shortcut", "hotkeys");
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
    #[test]
    fn layer_groups_persist_dispatch_and_reject_all_shortcut_conflicts() {
        use crate::hotkeys::Action;
        use aria_core::{layers::Group, shortcuts::Shortcut};
        let parameters = aria_core::movement::preview_parameters(aria_core::Parameters::default());
        let mut monitor = InputMonitor::new(
            "avatar-a".into(),
            RigConfig::from_parameters(&parameters),
            None,
            &parameters,
        );
        let key = Shortcut {
            ctrl: true,
            alt: true,
            key: b'L'.into(),
            ..Default::default()
        };
        monitor.saved.config.layers.groups.push(Group {
            id: 1,
            name: "Jacket".into(),
            protect_selection: false,
            layers: ["ArtMeshJacket".into()].into(),
            opacity: 0.,
            active: false,
        });
        monitor.saved.layer_hotkeys.insert(1, key);
        monitor.saved.global_hotkeys = true;
        monitor.saved.validate(&parameters).unwrap();
        assert!(
            monitor
                .hotkey_keys()
                .iter()
                .any(|r| r.shortcut == key && r.action == Action::Layers(1))
        );
        monitor.hotkey_action(Action::Layers(1), &parameters, &mut Default::default());
        assert_eq!(monitor.saved.config.layers.opacity("ArtMeshJacket"), 0.);
        let saved = serde_json::from_slice(&serde_json::to_vec(&monitor.saved).unwrap()).unwrap();
        let mut restored = InputMonitor::new(
            "avatar-a".into(),
            RigConfig::from_parameters(&parameters),
            Some(saved),
            &parameters,
        );
        restored.hotkey_action(Action::Layers(1), &parameters, &mut Default::default());
        assert_eq!(restored.saved.config.layers.opacity("ArtMeshJacket"), 1.);
        assert!(crate::items::Items::assign(&mut restored.saved, 1, key).is_err());
        assert!(
            crate::expressions_panel::ExpressionsPanel::assign(&mut restored.saved, "smile", key)
                .is_err()
        );
        restored.saved.effects.designs[0].hotkey = Some(key);
        assert!(restored.saved.validate(&parameters).is_err());
        restored.saved.effects.designs[0].hotkey = None;
        restored.saved.layer_hotkeys.insert(2, key);
        assert!(restored.saved.validate(&parameters).is_err());
        let other = InputMonitor::new(
            "avatar-b".into(),
            RigConfig::from_parameters(&parameters),
            None,
            &parameters,
        );
        assert!(
            other.saved.config.layers.groups.is_empty() && other.saved.layer_hotkeys.is_empty()
        );
    }
    use super::*;
    #[test]
    fn appearance_look_hotkey_changes_only_appearance_and_round_trips_for_this_avatar() {
        let parameters = aria_core::movement::preview_parameters(aria_core::Parameters::default());
        let mut monitor = InputMonitor::new(
            "moc3:appearance-test".into(),
            RigConfig::from_parameters(&parameters),
            None,
            &parameters,
        );
        monitor
            .saved
            .config
            .customization
            .values
            .insert("ParamAngleX".into(), 12.);
        monitor
            .saved
            .config
            .layers
            .opacity
            .insert("Jacket".into(), 0.);
        monitor.save_appearance("Jacket off".into(), &parameters);
        assert_eq!(monitor.saved.presets.len(), 1);
        monitor.saved.presets[0].hotkey = Some(3);
        monitor.saved.config.customization.values.clear();
        monitor.saved.config.customization.controls.insert(
            "ParamAngleX".into(),
            aria_core::customization::Control {
                label: "Keep my editor label".into(),
                ..Default::default()
            },
        );
        monitor.saved.config.layers.opacity.clear();
        monitor.saved.config.physics.strength = 0.37;
        monitor
            .saved
            .config
            .expressions
            .insert("keep-expression".into());
        monitor
            .saved
            .config
            .bindings
            .get_mut("ParamAngleX")
            .unwrap()
            .smoothing_ms = 123.;
        monitor.saved.config.capture_pose(&parameters);
        let mut expected = serde_json::to_value(&monitor.saved.config).unwrap();
        expected["customization"]["values"] = serde_json::json!({"ParamAngleX":12.});
        expected["layers"]["opacity"] = serde_json::json!({"Jacket":0.});
        let mut mapping = MappingSettings {
            head_gain: 2.3,
            ..Default::default()
        };
        monitor.hotkey_action(crate::hotkeys::Action::Preset(3), &parameters, &mut mapping);
        assert_eq!(
            serde_json::to_value(&monitor.saved.config).unwrap(),
            expected
        );
        assert_eq!(mapping.head_gain, 2.3);
        assert!(monitor.save_requested);
        let encoded = serde_json::to_vec(&PresetFile {
            version: 1,
            model_key: monitor.model_key.clone(),
            preset: monitor.saved.presets[0].clone(),
        })
        .unwrap();
        let decoded = PresetFile::decode(&encoded, &monitor.model_key, &parameters).unwrap();
        assert_eq!(decoded.preset.kind, PresetKind::Appearance);
        assert!(decoded.preset.hotkey.is_none());
        assert!(PresetFile::decode(&encoded, "moc3:other-avatar", &parameters).is_err());
        let saved = serde_json::from_slice(&serde_json::to_vec(&monitor.saved).unwrap()).unwrap();
        let restored = InputMonitor::new(
            monitor.model_key,
            RigConfig::from_parameters(&parameters),
            Some(saved),
            &parameters,
        );
        assert_eq!(
            restored.saved.config.customization.controls["ParamAngleX"].label,
            "Keep my editor label"
        );
    }
    #[test]
    fn imported_avatar_type_selects_only_relevant_controls() {
        use crate::avatar_import::Kind;
        let params = aria_core::movement::preview_parameters(aria_core::Parameters::default());
        let mut monitor = InputMonitor::new(
            "test".into(),
            RigConfig::from_parameters(&params),
            None,
            &params,
        );
        let ctx = egui::Context::default();
        for (kind, initial, expected) in [
            (Kind::Images, Tab::Physics, Tab::Images),
            (Kind::Images, Tab::Expressions, Tab::Images),
            (Kind::Live2d, Tab::Images, Tab::Physics),
            (Kind::Live2d, Tab::Items, Tab::Items),
            (Kind::Images, Tab::Customize, Tab::Inputs),
            (Kind::Vrm, Tab::Customize, Tab::Inputs),
            (Kind::Live2d, Tab::Customize, Tab::Customize),
        ] {
            monitor.tab = initial;
            let _ = crate::run_test_ui(&ctx, Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| monitor.navigation(ui, Some(kind)));
            });
            assert!(monitor.tab == expected);
        }
    }
    #[test]
    fn effect_shortcuts_dispatch_and_conflicts_are_rejected() {
        let parameters = aria_core::movement::preview_parameters(aria_core::Parameters::default());
        let mut monitor = InputMonitor::new(
            "a".into(),
            RigConfig::from_parameters(&parameters),
            None,
            &parameters,
        );
        let key = aria_core::shortcuts::Shortcut {
            ctrl: true,
            alt: true,
            key: 0x54,
            ..Default::default()
        };
        monitor.saved.effects.designs[0].hotkey = Some(key);
        monitor.saved.global_hotkeys = true;
        assert!(
            monitor
                .hotkey_keys()
                .iter()
                .any(|r| r.shortcut == key && r.action == crate::hotkeys::Action::Effect(1))
        );
        monitor.hotkey_action(
            crate::hotkeys::Action::Effect(1),
            &parameters,
            &mut MappingSettings::default(),
        );
        assert_eq!(monitor.effect_requests, vec![1]);
        monitor.saved.validate(&parameters).unwrap();
        monitor
            .saved
            .expression_hotkeys
            .insert("x.exp3.json".into(), key);
        assert!(monitor.saved.validate(&parameters).is_err());
    }
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
