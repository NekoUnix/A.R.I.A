//! Exported ArtMesh selection and model-owned visibility looks.
use crate::{help, live2d::Avatar, theme};
use aria_core::{layers::Group, movement::SavedRig, shortcuts::Shortcut};
use eframe::egui;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct Panel {
    pub stage_selection: crate::layer_selection::Selection,
    pub editor: crate::layer_editor::Editor,
    search: String,
    selected: BTreeSet<String>,
    name: String,
    editing: Option<u64>,
    draft: Shortcut,
    message: Option<String>,
    range_anchor: Option<String>,
    row_drag: Option<(String, BTreeSet<String>, bool)>,
    tint: Option<[f32; 3]>,
    include_protected: bool,
}
impl Panel {
    pub fn stage_tools(&mut self, ui: &mut egui::Ui, saved: &mut SavedRig) -> bool {
        saved
            .config
            .layers
            .retain_selectable(&mut self.selected, self.include_protected);
        self.stage_selection.include_protected = self.include_protected;
        if ui.button("Select layers…").on_hover_text("Open a separate frozen model window. Click multiple layers or draw boxes with your mouse, then hide or restore the selection. No modifier keys needed.").clicked() {
            self.stage_selection.cancel(&mut self.selected);
            self.stage_selection.enabled = false;
            self.editor.requested = true;
        }
        ui.toggle_value(&mut self.stage_selection.enabled, "On stage")
            .on_hover_text(
                "Optional: select on the moving main stage instead of opening the frozen preview.",
            );
        if !self.stage_selection.enabled {
            self.stage_selection.cancel(&mut self.selected);
            return false;
        }
        let mut changed = false;
        ui.small(format!("{} selected", self.selected.len()));
        for (label, opacity) in [("Hide", 0.), ("Restore", 1.)] {
            if ui
                .add_enabled(!self.selected.is_empty(), egui::Button::new(label))
                .clicked()
            {
                self.set_opacity(saved, opacity);
                changed = true;
            }
        }
        if ui.small_button("Clear").clicked() {
            self.selected.clear();
            self.editing = None;
        }
        changed
    }
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        avatar: &Avatar,
        saved: &mut SavedRig,
        started: std::time::Instant,
    ) -> bool {
        let next_id = saved
            .config
            .layers
            .groups
            .iter()
            .map(|g| g.id)
            .chain(saved.layer_hotkeys.keys().copied())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.editor.include_protected = self.include_protected;
        let changed = self.editor.show(
            ctx,
            avatar,
            &mut saved.config.layers,
            &mut self.selected,
            next_id,
            started,
        );
        self.include_protected = self.editor.include_protected;
        changed
    }
    pub fn stage(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        scene: &crate::output::Scene,
        zoom: f32,
        avatar: &Avatar,
        saved: &SavedRig,
    ) {
        let translated =
            rect.translate(egui::vec2(scene.recoil[0], scene.recoil[1]) * rect.height() * zoom);
        let projection = crate::layer_selection::Projection {
            canvas: avatar.view_canvas(),
            rect: scene.model_rect(translated, zoom),
            angle: scene.placement.rotation.to_radians(),
            fields: crate::deformation::avatar_fields(scene, translated, zoom),
        };
        self.stage_selection.stage(
            ui,
            rect,
            &avatar.model.drawables,
            &saved.config.layers,
            &projection,
            &mut self.selected,
        );
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode() {
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new("layer-smoke-selected"), self.selected.len())
            });
        }
    }
    fn set_opacity(&self, saved: &mut SavedRig, opacity: f32) {
        for id in &self.selected {
            if opacity == 1. {
                saved.config.layers.opacity.remove(id);
            } else {
                saved.config.layers.opacity.insert(id.clone(), opacity);
            }
        }
    }
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
                protect_selection: false,
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
        ui.horizontal_wrapped(|ui| {
            changed |= self.stage_tools(ui, saved);
        });
        help::control(ui, "live2d-layers", |ui| {
            ui.checkbox(
                &mut self.stage_selection.include_hidden,
                "Include hidden layers in stage selection",
            )
        });
        if help::control(ui, "live2d-layers", |ui| ui.checkbox(&mut self.include_protected, "Include protected layers"))
            .on_hover_text("Selection tools skip protected groups by default. Enable this to select and edit their members deliberately. Direct opacity controls and saved visibility hotkeys still work.").changed() {
            self.row_drag = None;
            self.stage_selection.cancel(&mut self.selected);
            saved.config.layers.retain_selectable(&mut self.selected, self.include_protected);
        }
        theme::caption(
            ui,
            "Select layers… opens a frozen model window: click layers or draw boxes with your mouse, without holding keys. On stage enables the optional moving-stage picker. List rows also support range selection.",
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
                    self.selected.extend(
                        filtered
                            .iter()
                            .filter(|d| {
                                saved
                                    .config
                                    .layers
                                    .selectable(&d.id, self.include_protected)
                            })
                            .map(|d| d.id.clone()),
                    );
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
            let row_height = 44.;
            let mut hovered_row = None;
            egui::ScrollArea::vertical()
                .id_salt("layer-list")
                .max_height(290.)
                .show_rows(ui, row_height, filtered.len(), |ui, range| {
                    for index in range {
                        let d = filtered[index];
                        ui.push_id(&d.id, |ui| {
                            ui.horizontal(|ui| {
                                let selected = self.selected.contains(&d.id);
                                let width = (ui.available_width() * 0.42).clamp(72., 170.);
                                let response = ui
                                    .add_sized(
                                        [width, ui.spacing().interact_size.y],
                                        egui::Button::selectable(selected, &d.id)
                                            .truncate()
                                            .sense(egui::Sense::click_and_drag()),
                                    )
                                    .on_hover_text(if saved.config.layers.selectable(&d.id, self.include_protected) { d.id.clone() } else { format!("{} · Protected from selection. Enable Include protected layers to select it.", d.id) });
                                if response.clicked() && saved.config.layers.selectable(&d.id, self.include_protected) {
                                    if ui.input(|i| i.modifiers.shift) {
                                        let anchor = filtered
                                            .iter()
                                            .position(|d| Some(&d.id) == self.range_anchor.as_ref())
                                            .unwrap_or(index);
                                        self.selected.extend(
                                            filtered[anchor.min(index)..=anchor.max(index)]
                                                .iter()
                                                .map(|d| d.id.clone()),
                                        );
                                    } else if selected {
                                        self.selected.remove(&d.id);
                                    } else {
                                        self.selected.insert(d.id.clone());
                                    }
                                    self.range_anchor = Some(d.id.clone());
                                }
                                if response.drag_started_by(egui::PointerButton::Primary) && saved.config.layers.selectable(&d.id, self.include_protected) {
                                    self.row_drag =
                                        Some((d.id.clone(), self.selected.clone(), !selected));
                                }
                                if let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
                                    && ui.clip_rect().contains(pointer)
                                    && pointer.y >= response.rect.top()
                                    && pointer.y < response.rect.top() + row_height
                                {
                                    hovered_row = Some(index);
                                }
                                ui.spacing_mut().slider_width =
                                    (ui.available_width() - 45.).clamp(40., 100.);
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
                            let part = avatar.labels.get(&d.part).unwrap_or(&d.part);
                            ui.add(egui::Label::new(egui::RichText::new(part).small()).truncate())
                                .on_hover_text(part);
                        });
                    }
                    // The anchor can scroll outside the virtualized rows while
                    // held; keep scrolling from gesture state, not its response.
                    if self.row_drag.is_some()
                        && ui.input(|i| i.pointer.primary_down())
                        && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
                        && pointer.x >= ui.clip_rect().left()
                        && pointer.x <= ui.clip_rect().right()
                    {
                        if pointer.y < ui.clip_rect().top() + 14. {
                            ui.scroll_with_delta(egui::vec2(0., 8.));
                        }
                        if pointer.y > ui.clip_rect().bottom() - 14. {
                            ui.scroll_with_delta(egui::vec2(0., -8.));
                        }
                    }
                });
            if let Some((anchor, original, add)) = &self.row_drag {
                if let Some(index) = hovered_row {
                    let anchor = filtered
                        .iter()
                        .position(|d| &d.id == anchor)
                        .unwrap_or(index);
                    self.selected.clone_from(original);
                    for d in &filtered[anchor.min(index)..=anchor.max(index)] {
                        if *add {
                            self.selected.insert(d.id.clone());
                        } else {
                            self.selected.remove(&d.id);
                        }
                    }
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.selected.clone_from(original);
                    self.row_drag = None;
                } else if !ui.input(|i| i.pointer.primary_down()) {
                    self.row_drag = None;
                }
            }
            saved
                .config
                .layers
                .retain_selectable(&mut self.selected, self.include_protected);
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
                        self.set_opacity(saved, value);
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
            if saved.config.layers.groups.iter().any(|g| g.active && !g.layers.is_disjoint(&self.selected))
                && ui.button("Disable groups affecting selected layers").on_hover_text("Turns off each active group touching the selection. Other layers in those groups also return to their underlying opacity.").clicked()
            {
                for group in &mut saved.config.layers.groups { if !group.layers.is_disjoint(&self.selected) { group.active = false; } }
                changed = true;
            }
        });
        theme::category(ui, "layer-colors", "Selected layer colors", false, |ui| {
            help::button(ui, "live2d-customization");
            let tint = self.tint.get_or_insert([1.; 3]);
            ui.horizontal(|ui| {
                ui.color_edit_button_rgb(tint);
                ui.label("Multiply tint");
            });
            ui.add_enabled_ui(!self.selected.is_empty(), |ui| {
                if ui.button("Apply tint to selected layers").clicked() {
                    for d in avatar
                        .model
                        .drawables
                        .iter()
                        .filter(|d| self.selected.contains(&d.id))
                    {
                        let colors = saved.config.layers.colors.entry(d.id.clone()).or_insert(
                            aria_core::layers::Colors {
                                multiply: d.multiply,
                                screen: d.screen,
                            },
                        );
                        colors.multiply[..3].copy_from_slice(tint);
                    }
                    changed = true;
                }
                if ui.button("Restore selected colors").clicked() {
                    for id in &self.selected {
                        saved.config.layers.colors.remove(id);
                    }
                    changed = true;
                }
            });
            theme::caption(
                ui,
                "White keeps the original texture colors. Other tints multiply the artwork's colors; they cannot recover detail or recolor black pixels. Save an appearance look to recall colors with layer visibility.",
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
                        protect_selection: false,
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
                        changed |= ui.checkbox(&mut group.protect_selection, "Protect from selection")
                            .on_hover_text("Clicks, boxes and list selection skip these layers unless Include protected layers is on. This is independent of the group's visibility/hotkey state and saves with this avatar.").changed();
                        changed |= help::control(ui, "live2d-layers", |ui| ui.add(egui::Slider::new(&mut group.opacity, 0.0..=1.0).text("Group opacity"))).changed();
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Edit selected layers").on_hover_text("Selects the group's full membership for editing, including protected layers. This enables Include protected layers.").clicked() { self.include_protected = true; self.selected.clone_from(&group.layers); self.name.clone_from(&group.name); self.editing = Some(group.id); }
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
        saved
            .config
            .layers
            .retain_selectable(&mut self.selected, self.include_protected);
        if let Some(message) = &self.editor.message {
            ui.label(message);
        }
        changed
    }
}
