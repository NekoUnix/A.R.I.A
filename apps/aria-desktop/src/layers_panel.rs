//! Exported ArtMesh selection and model-owned visibility looks.
use crate::{help, live2d::Avatar, theme};
use aria_core::{layers::Group, movement::SavedRig, shortcuts::Shortcut};
use eframe::egui;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct Panel {
    search: String,
    selected: BTreeSet<String>,
    name: String,
    editing: Option<u64>,
    draft: Shortcut,
    message: Option<String>,
}
impl Panel {
    #[cfg(feature = "screenshots")]
    pub fn prepare_smoke(&mut self, avatar: &Avatar, saved: &mut SavedRig) {
        if let Some(part) = avatar
            .model
            .drawables
            .iter()
            .find(|d| !d.part.is_empty())
            .map(|d| d.part.clone())
        {
            self.selected = avatar
                .model
                .drawables
                .iter()
                .filter(|d| d.part == part)
                .map(|d| d.id.clone())
                .collect();
            self.search = part;
            saved.config.layers.groups.push(Group {
                id: 1,
                name: "Example layer selection".into(),
                layers: self.selected.clone(),
                opacity: 0.,
                active: false,
            });
        }
    }
    pub fn show(&mut self, ui: &mut egui::Ui, avatar: &Avatar, saved: &mut SavedRig) -> bool {
        let mut changed = false;
        help::button(ui, "live2d-layers");
        theme::caption(
            ui,
            "Select exported layers to hide or fade. Changes, named groups and shortcuts belong to this avatar. Your source files stay intact.",
        );
        theme::category(ui, "layer-selection", "Layers & transparency", true, |ui| {
            help::control(ui, "live2d-layers", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Find a layer or part…")
                        .desired_width(f32::INFINITY),
                )
            });
            let query = self.search.to_lowercase();
            let filtered: Vec<_> = avatar
                .model
                .drawables
                .iter()
                .filter(|d| {
                    format!(
                        "{} {} {}",
                        d.id,
                        d.part,
                        avatar.labels.get(&d.part).map_or("", String::as_str)
                    )
                    .to_lowercase()
                    .contains(&query)
                })
                .collect();
            ui.horizontal_wrapped(|ui| {
                if ui.button("Select results").clicked() {
                    self.selected.extend(filtered.iter().map(|d| d.id.clone()));
                }
                if ui.button("Clear selection").clicked() {
                    self.selected.clear();
                    self.editing = None;
                }
                ui.small(format!(
                    "{} selected · {} layers",
                    self.selected.len(),
                    avatar.model.drawables.len()
                ));
            });
            egui::ScrollArea::vertical()
                .id_salt("layer-list")
                .max_height(290.)
                .show(ui, |ui| {
                    for d in filtered {
                        ui.push_id(&d.id, |ui| {
                            ui.horizontal(|ui| {
                                let mut selected = self.selected.contains(&d.id);
                                if ui.checkbox(&mut selected, &d.id).changed() {
                                    if selected {
                                        self.selected.insert(d.id.clone());
                                    } else {
                                        self.selected.remove(&d.id);
                                    }
                                }
                                let mut opacity = saved
                                    .config
                                    .layers
                                    .opacity
                                    .get(&d.id)
                                    .copied()
                                    .unwrap_or(1.);
                                if ui
                                    .add(
                                        egui::Slider::new(&mut opacity, 0.0..=1.0)
                                            .show_value(false),
                                    )
                                    .on_hover_text(
                                        "Layer opacity: left = hidden, right = authored opacity",
                                    )
                                    .changed()
                                {
                                    if opacity == 1. {
                                        saved.config.layers.opacity.remove(&d.id);
                                    } else {
                                        saved.config.layers.opacity.insert(d.id.clone(), opacity);
                                    }
                                    changed = true;
                                }
                                ui.small(format!(
                                    "{:.0}%",
                                    100. * saved.config.layers.opacity(&d.id)
                                ));
                            });
                            if !d.part.is_empty() {
                                ui.small(avatar.labels.get(&d.part).unwrap_or(&d.part));
                            }
                        });
                    }
                });
            ui.horizontal_wrapped(|ui| {
                for (label, value) in [
                    ("Hide selected", 0.),
                    ("Half opacity", 0.5),
                    ("Restore selected", 1.),
                ] {
                    if ui
                        .add_enabled(!self.selected.is_empty(), egui::Button::new(label))
                        .clicked()
                    {
                        for id in &self.selected {
                            if value == 1. {
                                saved.config.layers.opacity.remove(id);
                            } else {
                                saved.config.layers.opacity.insert(id.clone(), value);
                            }
                        }
                        changed = true;
                    }
                }
                if ui.button("Show all layers").clicked() {
                    saved.config.layers.opacity.clear();
                    for group in &mut saved.config.layers.groups {
                        group.active = false;
                    }
                    changed = true;
                }
            });
            theme::caption(
                ui,
                "Percentages multiply the model's authored opacity. An active group can still hide a restored layer. Show all disables groups too; layers hidden by the model's own parameters remain hidden.",
            );
        });
        theme::category(ui, "layer-groups", "Saved layer groups", true, |ui| {
            help::control(ui, "live2d-layers", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.name)
                        .hint_text("Group name, e.g. Hide jacket")
                        .char_limit(60)
                        .desired_width(f32::INFINITY),
                )
            });
            if ui
                .add_enabled(
                    !self.selected.is_empty()
                        && !self.name.trim().is_empty()
                        && self.name.len() <= 120
                        && (self.editing.is_some() || saved.config.layers.groups.len() < 128),
                    egui::Button::new(if self.editing.is_some() {
                        "Save group selection"
                    } else {
                        "Create group from selection"
                    }),
                )
                .clicked()
            {
                if let Some(group) = saved
                    .config
                    .layers
                    .groups
                    .iter_mut()
                    .find(|g| Some(g.id) == self.editing)
                {
                    group.name = self.name.trim().into();
                    group.layers.clone_from(&self.selected);
                } else {
                    let id = saved
                        .config
                        .layers
                        .groups
                        .iter()
                        .map(|g| g.id)
                        .chain(saved.layer_hotkeys.keys().copied())
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1);
                    saved.config.layers.groups.push(Group {
                        id,
                        name: self.name.trim().into(),
                        layers: self.selected.clone(),
                        opacity: 0.,
                        active: false,
                    });
                }
                self.editing = None;
                self.name.clear();
                changed = true;
            }
            let mut remove = None;
            let mut assign = None;
            for group in &mut saved.config.layers.groups {
                ui.push_id(group.id, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        changed |= ui.checkbox(&mut group.active, &group.name).on_hover_text("Apply this group's transparency; toggle off to restore the underlying layer settings").changed();
                        ui.small(format!("{} layers", group.layers.len()));
                    });
                    ui.collapsing("Group settings & shortcut", |ui| {
                        changed |= help::control(ui, "live2d-layers", |ui| ui.add(egui::Slider::new(&mut group.opacity, 0.0..=1.0).text("Group opacity"))).changed();
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Edit selected layers").clicked() { self.selected.clone_from(&group.layers); self.name.clone_from(&group.name); self.editing = Some(group.id); }
                            if ui.button("Delete group").clicked() { remove = Some(group.id); }
                        });
                        if let Some(key) = saved.layer_hotkeys.get(&group.id) { ui.label(format!("Shortcut: {}", key.label())); }
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Assign chosen shortcut").clicked() { assign = Some(group.id); }
                            if ui.button("Clear shortcut").clicked() { saved.layer_hotkeys.remove(&group.id); changed = true; }
                        });
                        let missing = group.layers.iter().filter(|id| !avatar.model.drawables.iter().any(|d| &d.id == *id)).count();
                        if missing > 0 { ui.label(format!("{missing} saved layers are absent from this export and are skipped.")); }
                    });
                });
            }
            if let Some(id) = remove {
                saved.config.layers.groups.retain(|g| g.id != id);
                saved.layer_hotkeys.remove(&id);
                if self.editing == Some(id) {
                    self.editing = None;
                }
                changed = true;
            }
            help::button(ui, "hotkeys");
            changed |= ui
                .checkbox(
                    &mut saved.global_hotkeys,
                    "Enable global hotkeys for this avatar",
                )
                .changed();
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.draft.ctrl, "Ctrl");
                ui.checkbox(&mut self.draft.alt, "Alt");
                ui.checkbox(&mut self.draft.shift, "Shift");
                ui.checkbox(&mut self.draft.win, "Win");
                egui::ComboBox::from_id_salt("layer-key")
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
            });
            if let Some(id) = assign {
                let old = saved.layer_hotkeys.insert(id, self.draft);
                match saved
                    .validate_layer_hotkeys()
                    .and_then(|()| saved.validate_vts_hotkeys())
                {
                    Ok(()) => {
                        saved.global_hotkeys = true;
                        changed = true;
                        self.message =
                            Some("Shortcut assigned. It toggles this group's transparency.".into());
                    }
                    Err(e) => {
                        if let Some(old) = old {
                            saved.layer_hotkeys.insert(id, old);
                        } else {
                            saved.layer_hotkeys.remove(&id);
                        }
                        self.message = Some(e.to_string());
                    }
                }
            }
            theme::caption(
                ui,
                "Choose a key, then use Assign chosen shortcut in a group's settings. Global shortcuts currently require Windows; group buttons work on every platform. Movement and pose presets also save layer visibility and groups.",
            );
        });
        if let Some(message) = &self.message {
            ui.label(message);
        }
        changed
    }
}
