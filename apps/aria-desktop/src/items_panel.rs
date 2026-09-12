use crate::{help, items::Items, theme};
use aria_core::{
    items::{Pin, RuleMode, SignalKind},
    movement::SavedRig,
    rig::{Inputs, RigParameter},
    shortcuts::Shortcut,
};
use eframe::egui;

impl Items {
    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        saved: &mut SavedRig,
        inputs: &Inputs,
        parameters: &[RigParameter],
    ) -> bool {
        let before = saved.config.items.clone();
        let keys = saved.item_hotkeys.clone();
        let global = saved.global_hotkeys;
        theme::caption(
            ui,
            "Drop PNGs or Live2D exports onto Your stage. Select and drag an object, then pin it to your avatar. Each object keeps its own settings.",
        );
        ui.horizontal_wrapped(|ui| {
            if help::control(ui, "model-items", |ui| ui.button("Add objects…")).clicked()
                && let Some(paths) = rfd::FileDialog::new()
                    .add_filter("PNG / Live2D objects", &["png", "moc3", "json"])
                    .pick_files()
            {
                self.add(paths, &mut saved.config, [0.0; 2]);
            }
            if help::control(ui, "model-items", |ui| ui.button("Reload assets")).clicked() {
                self.reload();
            }
        });
        let mut save =
            help::control(ui, "png-items", |ui| ui.button("Save item settings")).clicked();
        if save {
            self.save_now = true;
            self.message = Some(
                "Stage objects and toggles saved for this avatar. Presets can also store this layout."
                    .into(),
            );
        }
        if let Some(message) = &self.message {
            ui.label(egui::RichText::new(message).small().color(theme::MINT));
        }
        theme::category(ui, "png-library", "Stage objects", true, |ui| {
            if saved.config.items.is_empty() {
                ui.label("No objects yet. Drop a PNG or moc3 onto the stage.");
            }
            for item in &mut saved.config.items {
                ui.push_id(item.id, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        help::control(ui, "png-toggles", |ui| ui.checkbox(&mut item.visible, ""));
                        if ui
                            .selectable_label(self.selected == Some(item.id), &item.name)
                            .clicked()
                        {
                            self.selected = Some(item.id);
                            self.pick_pin = false;
                        }
                        ui.small(if item.pin.is_some() { "Pinned" } else { "Free" });
                        if aria_core::items::is_model(&item.path) {
                            ui.small("Live2D");
                        }
                        if let Some(key) = saved.item_hotkeys.get(&item.id) {
                            ui.small(key.label());
                        }
                    });
                });
            }
        });
        let Some(index) = saved
            .config
            .items
            .iter()
            .position(|i| Some(i.id) == self.selected)
        else {
            return save || before != saved.config.items;
        };
        let id = saved.config.items[index].id;
        let draft_id = ui.id().with("png-shortcut-draft");
        if ui.data(|d| d.get_temp::<u64>(draft_id)) != Some(id) {
            self.draft = saved.item_hotkeys.get(&id).copied().unwrap_or(Shortcut {
                ctrl: true,
                alt: true,
                ..Default::default()
            });
            ui.data_mut(|d| d.insert_temp(draft_id, id));
        }
        let mut remove = false;
        let mut reorder = 0_i32;
        let model_count = saved
            .config
            .items
            .iter()
            .filter(|i| aria_core::items::is_model(&i.path))
            .count();
        let item = &mut saved.config.items[index];
        ui.push_id(id,|ui| {
            theme::category(ui,"png-placement","Placement & appearance",true,|ui| {
                help::label(ui,"Toggle name","png-toggles");
                let response = help::control(ui,"png-toggles",|ui| ui.add(egui::TextEdit::singleline(&mut item.name).char_limit(80).desired_width(f32::INFINITY)));
                if response.lost_focus() && item.name.trim().is_empty() { item.name = "Stage object".into(); }
                help::context_button(ui,"png-items",|| format!("Object file: {}\nItem ID: {}\nAssets are loaded locally. The file is referenced by this profile and its presets; keep it in a stable folder.",item.path.display(),item.id));
                ui.small(item.path.file_name().unwrap_or_default().to_string_lossy());
                if let Some(error) = self.error(&item.path) { ui.colored_label(egui::Color32::LIGHT_RED,error); }
                if help::control(ui,"model-items",|ui| ui.button("Replace object / locate file…")).clicked()
                    && let Some(path) = rfd::FileDialog::new().add_filter("PNG / Live2D objects", &["png","moc3","json"]).pick_file()
                    && crate::items::is_item(&path) {
                        if aria_core::items::is_model(&path) && !aria_core::items::is_model(&item.path) && model_count >= aria_core::items::MAX_MODEL_ITEMS {
                            self.message = Some("Maximum four Live2D objects per avatar. Remove one before adding another.".into());
                        } else { item.path = path.canonicalize().unwrap_or(path); item.model = Default::default(); self.models.reset_draft(); }
                    }
                help::control(ui,"png-placement",|ui| ui.add(egui::Slider::new(&mut item.height,0.005..=4.0).logarithmic(true).text("Size")));
                help::control(ui,"png-placement",|ui| ui.add(egui::Slider::new(&mut item.rotation,-180.0..=180.0).suffix("°").text("Rotation")));
                help::control(ui,"png-placement",|ui| ui.add(egui::Slider::new(&mut item.opacity,0.0..=1.0).text("Opacity")));
                for (axis,label) in ["X offset →","Y offset ↓"].into_iter().enumerate() {
                    help::control(ui,"png-placement",|ui| { ui.label(label); ui.add(egui::DragValue::new(&mut item.position[axis]).range(-4.0..=4.0).speed(0.001)); });
                }
                help::control(ui,"png-placement",|ui| ui.checkbox(&mut item.locked,"Lock stage dragging"));
                help::control(ui,"png-placement",|ui| ui.checkbox(&mut item.behind,"Behind the avatar"));
                ui.horizontal_wrapped(|ui| {
                    if help::control(ui,"png-placement",|ui| ui.button("Backward")).clicked() { reorder = -1; }
                    if help::control(ui,"png-placement",|ui| ui.button("Forward")).clicked() { reorder = 1; }
                    if help::control(ui,"png-placement",|ui| ui.button("Reset offset")).clicked() { item.position = [0.0;2]; }
                });
                if help::control(ui,"png-items",|ui| ui.button("Remove item")).clicked() { remove = true; }
            });
            self.models.panel(ui,item);
            theme::category(ui,"png-pin","Pin to avatar",true,|ui| {
                help::label(ui,match &item.pin { None => "Free on canvas".into(), Some(Pin::Puppet {..}) => "Pinned to puppet movement".into(), Some(Pin::Surface { mesh,..}) => format!("Pinned to ArtMesh #{}",mesh+1) },"png-pins");
                if self.draws.iter().any(|d| d.item.id == id && d.anchor == crate::items::Anchor::Missing) {
                    ui.colored_label(egui::Color32::LIGHT_RED,"Pin surface unavailable. Choose a new pin point.");
                }
                ui.horizontal_wrapped(|ui| {
                    if help::control(ui,"png-pins",|ui| ui.button(if self.pick_pin { "Cancel pin" } else { "Choose pin point" })).clicked() { self.pick_pin = !self.pick_pin; }
                    if help::control(ui,"png-pins",|ui| ui.button("Pin here")).clicked() { self.pin_here = true; }
                    if help::control(ui,"png-pins",|ui| ui.add_enabled(item.pin.is_some(),egui::Button::new("Unpin"))).clicked() { self.unpin = true; }
                });
                theme::caption(ui,"Choose pin point, then click the model where this object should attach. Pin here uses the object's center. Drag a pinned object to fine-tune its offset.");
                help::control(ui,"png-pins",|ui| ui.checkbox(&mut item.follow_rotation,"Follow pin rotation"));
                help::control(ui,"png-pins",|ui| ui.checkbox(&mut item.follow_scale,"Follow surface stretch"));
                help::control(ui,"png-pins",|ui| ui.checkbox(&mut item.follow_visibility,"Follow surface visibility"));
            });
            theme::category(ui,"png-rule","Input toggle",true,|ui| {
                help::control(ui,"png-toggles",|ui| ui.checkbox(&mut item.visible,"Master visibility"));
                help::label(ui,"Behavior","png-toggles");
                egui::ComboBox::from_id_salt("png-rule-mode").selected_text(rule_name(item.rule.mode)).show_ui(ui,|ui| {
                    for mode in [RuleMode::Manual,RuleMode::WhileInRange,RuleMode::ToggleOnEnter] { ui.selectable_value(&mut item.rule.mode,mode,rule_name(mode)); }
                });
                if item.rule.mode != RuleMode::Manual {
                    help::label(ui,"Read a signal from","png-toggles");
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut item.rule.kind,SignalKind::Tracking,"Tracking input");
                        ui.selectable_value(&mut item.rule.kind,SignalKind::Parameter,"Model parameter");
                    });
                    egui::ComboBox::from_id_salt("png-rule-source").selected_text(&item.rule.source).width(ui.available_width().min(250.0)).show_ui(ui,|ui| {
                        match item.rule.kind {
                            SignalKind::Tracking => for name in inputs.keys() { ui.selectable_value(&mut item.rule.source,name.clone(),name); },
                            SignalKind::Parameter => for p in parameters { ui.selectable_value(&mut item.rule.source,p.id.clone(),&p.id); },
                        }
                    });
                    let edit = help::control(ui,"png-toggles",|ui| ui.add(egui::TextEdit::singleline(&mut item.rule.source).char_limit(256).hint_text("Exact input / parameter ID").desired_width(f32::INFINITY)));
                    if edit.lost_focus() && item.rule.source.trim().is_empty() { item.rule.source = "MouthOpen".into(); }
                    ui.small(match crate::items::signal(item,inputs,parameters) { Some(v) => format!("Current signal: {v:.4}"), None => "Signal unavailable — check the name and tracking connection.".into() });
                    help::control(ui,"png-toggles",|ui| { ui.label("Start (inclusive)"); ui.add(egui::DragValue::new(&mut item.rule.start).range(-1e6..=item.rule.end).speed(0.01)); });
                    help::control(ui,"png-toggles",|ui| { ui.label("End (inclusive)"); ui.add(egui::DragValue::new(&mut item.rule.end).range(item.rule.start..=1e6).speed(0.01)); });
                    help::control(ui,"png-toggles",|ui| { ui.label("Hysteresis"); ui.add(egui::DragValue::new(&mut item.rule.hysteresis).range(0.0..=1e6).speed(0.01)); });
                    theme::caption(ui,"Visible in range follows the condition. Toggle on entering flips once, then waits for the signal to leave. Frozen poses pause these rules.");
                }
            });
        });
        theme::category(ui, "png-hotkey", "Keyboard toggle", false, |ui| {
            help::control(ui, "hotkeys", |ui| {
                ui.checkbox(
                    &mut saved.global_hotkeys,
                    "Enable global hotkeys for this avatar",
                )
            });
            ui.horizontal_wrapped(|ui| {
                help::control(ui, "hotkeys", |ui| {
                    ui.checkbox(&mut self.draft.ctrl, "Ctrl")
                });
                help::control(ui, "hotkeys", |ui| ui.checkbox(&mut self.draft.alt, "Alt"));
                help::control(ui, "hotkeys", |ui| {
                    ui.checkbox(&mut self.draft.shift, "Shift")
                });
                help::control(ui, "hotkeys", |ui| ui.checkbox(&mut self.draft.win, "Win"));
            });
            help::label(ui, "Main key", "hotkeys");
            egui::ComboBox::from_id_salt("png-shortcut-key")
                .selected_text(if self.draft.key == 0 {
                    "Choose key".into()
                } else {
                    self.draft.label()
                })
                .show_ui(ui, |ui| {
                    for (key, name) in Shortcut::keys() {
                        ui.selectable_value(&mut self.draft.key, key, name);
                    }
                });
            ui.horizontal_wrapped(|ui| {
                if help::control(ui, "png-toggles", |ui| ui.button("Assign shortcut")).clicked() {
                    self.message = Some(match Self::assign(saved, id, self.draft) {
                        Ok(()) => format!(
                            "{} toggles this object's master visibility.",
                            self.draft.label()
                        ),
                        Err(e) => e.to_string(),
                    });
                }
                if help::control(ui, "hotkeys", |ui| ui.button("Clear shortcut")).clicked() {
                    saved.item_hotkeys.remove(&id);
                }
            });
            if let Some(key) = saved.item_hotkeys.get(&id) {
                ui.small(format!("Assigned: {}", key.label()));
            }
            theme::caption(
                ui,
                "Shortcuts work while ARIA is unfocused. A key without modifiers also intercepts typing. In range mode, a shortcut controls master visibility; the range must still match.",
            );
        });
        if remove {
            saved.config.items.remove(index);
            saved.item_hotkeys.remove(&id);
            self.selected = None;
            self.pick_pin = false;
        } else if reorder != 0 {
            let destination =
                (index as i32 + reorder).clamp(0, saved.config.items.len() as i32 - 1) as usize;
            saved.config.items.swap(index, destination);
        }
        for item in &mut saved.config.items {
            if item.name.trim().is_empty() {
                item.name = "Stage object".into();
            }
            if item.rule.source.trim().is_empty() {
                item.rule.source = "MouthOpen".into();
            }
        }
        save |= before != saved.config.items
            || keys != saved.item_hotkeys
            || global != saved.global_hotkeys;
        save
    }
    pub fn assign(saved: &mut SavedRig, id: u64, shortcut: Shortcut) -> anyhow::Result<()> {
        shortcut.validate()?;
        anyhow::ensure!(
            saved.config.items.iter().any(|i| i.id == id),
            "Stage object no longer exists"
        );
        anyhow::ensure!(
            shortcut != Shortcut::pose(),
            "That shortcut is reserved for Freeze pose"
        );
        anyhow::ensure!(
            !saved
                .presets
                .iter()
                .filter_map(|p| p.hotkey)
                .any(|n| Shortcut::preset(n) == shortcut),
            "That shortcut belongs to a preset"
        );
        anyhow::ensure!(
            !saved.expression_hotkeys.values().any(|k| *k == shortcut),
            "That shortcut belongs to an expression"
        );
        anyhow::ensure!(
            !saved
                .item_hotkeys
                .iter()
                .any(|(other, k)| *other != id && *k == shortcut),
            "That shortcut belongs to another object toggle"
        );
        saved.item_hotkeys.insert(id, shortcut);
        saved.global_hotkeys = true;
        Ok(())
    }
}
fn rule_name(mode: RuleMode) -> &'static str {
    match mode {
        RuleMode::Manual => "Manual / hotkey",
        RuleMode::WhileInRange => "Visible while in range",
        RuleMode::ToggleOnEnter => "Toggle on entering range",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn item_shortcuts_persist_per_avatar_dispatch_and_reject_expression_conflicts() {
        let parameters = aria_core::movement::preview_parameters(aria_core::Parameters::default());
        let mut monitor = crate::input_monitor::InputMonitor::new(
            "avatar-a".into(),
            aria_core::movement::RigConfig::from_parameters(&parameters),
            None,
            &parameters,
        );
        monitor.saved.config.items.push(aria_core::items::Item {
            id: 9,
            path: "badge.png".into(),
            ..Default::default()
        });
        let key = Shortcut {
            ctrl: true,
            alt: true,
            key: 0x4a,
            ..Default::default()
        };
        Items::assign(&mut monitor.saved, 9, key).unwrap();
        assert!(Items::assign(&mut monitor.saved, 9, Shortcut::pose()).is_err());
        assert!(
            crate::expressions_panel::ExpressionsPanel::assign(
                &mut monitor.saved,
                "x.exp3.json",
                key
            )
            .is_err()
        );
        assert!(
            monitor
                .hotkey_keys()
                .iter()
                .any(|k| k.shortcut == key && k.action == crate::hotkeys::Action::ItemToggle(9))
        );
        monitor.hotkey_action(
            crate::hotkeys::Action::ItemToggle(9),
            &parameters,
            &mut Default::default(),
        );
        assert!(!monitor.saved.config.items[0].visible);
        let profiles = std::collections::BTreeMap::from([
            ("avatar-a", monitor.saved),
            ("avatar-b", SavedRig::default()),
        ]);
        let loaded: std::collections::BTreeMap<String, SavedRig> =
            serde_json::from_str(&serde_json::to_string(&profiles).unwrap()).unwrap();
        loaded["avatar-a"].validate(&parameters).unwrap();
        assert_eq!(loaded["avatar-a"].item_hotkeys[&9], key);
        assert!(loaded["avatar-b"].item_hotkeys.is_empty());
        assert!(loaded["avatar-b"].config.items.is_empty());
    }
}
