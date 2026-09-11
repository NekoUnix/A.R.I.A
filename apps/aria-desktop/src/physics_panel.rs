use crate::theme;
use aria_core::physics::{GroupInfo, GroupSettings, PhysicsSettings};
use eframe::egui;

#[derive(Default)]
pub struct PhysicsPanel {
    search: String,
    modified_only: bool,
}
#[derive(Default)]
pub struct Actions {
    pub save: bool,
    pub settle: bool,
    pub presets: bool,
}

impl PhysicsPanel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut PhysicsSettings,
        defaults: &PhysicsSettings,
        groups: &[GroupInfo],
    ) -> Actions {
        let mut actions = Actions::default();
        if groups.is_empty() {
            theme::card(ui, |ui| {
                ui.strong("No physics groups loaded");
                theme::caption(
                    ui,
                    "Open an avatar whose model3 file references a physics3 export. Its groups will appear here automatically.",
                );
            });
            return actions;
        }
        ui.horizontal_wrapped(|ui| {
            actions.save = ui.button("Save physics settings").clicked();
            actions.presets = ui.button("Save as preset…").clicked();
        });
        theme::caption(
            ui,
            "Saved with this avatar and included in movement / pose presets.",
        );
        let show_overall = !(crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("physics-group"));
        theme::category(
            ui,
            "physics-overall",
            "Overall physics",
            show_overall,
            |ui| {
                ui.checkbox(&mut settings.enabled, "Enable avatar physics");
                ui.add_enabled_ui(settings.enabled, |ui| {
                    tuning(
                        ui,
                        &mut settings.strength,
                        &mut settings.inertia,
                        &mut settings.response,
                        &mut settings.gravity,
                        &mut settings.wind,
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    actions.settle = ui.button("Settle motion").clicked();
                    if ui.button("Reset overall").clicked() {
                        let groups = std::mem::take(&mut settings.groups);
                        *settings = defaults.clone();
                        settings.groups = groups;
                        actions.settle = true;
                    }
                });
            },
        );
        ui.horizontal(|ui| {
            ui.strong(format!("Avatar groups · {}", groups.len()));
            let enabled = groups
                .iter()
                .filter(|g| settings.groups.get(&g.id).is_none_or(|s| s.enabled))
                .count();
            theme::caption(ui, format!("{enabled} enabled"));
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Find hair, tail, ears or a parameter…")
                .desired_width(f32::INFINITY),
        );
        ui.checkbox(&mut self.modified_only, "Only modified groups");
        ui.collapsing("Group actions", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Enable all").clicked() {
                    for g in groups {
                        settings.groups.entry(g.id.clone()).or_default().enabled = true;
                    }
                }
                if ui.button("Disable all").clicked() {
                    for g in groups {
                        settings.groups.entry(g.id.clone()).or_default().enabled = false;
                    }
                }
                if ui.button("Reset all groups").clicked() {
                    settings.groups = defaults.groups.clone();
                    actions.settle = true;
                }
            });
        });
        let search = self.search.trim().to_lowercase();
        let mut shown = 0;
        for info in groups {
            let mut group = settings.groups.get(&info.id).copied().unwrap_or_default();
            let modified = group != GroupSettings::default();
            if self.modified_only && !modified {
                continue;
            }
            if !format!(
                "{} {} {} {}",
                info.name,
                info.id,
                info.inputs.join(" "),
                info.outputs.join(" ")
            )
            .to_lowercase()
            .contains(&search)
            {
                continue;
            }
            shown += 1;
            ui.push_id(&info.id, |ui| {
                theme::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut group.enabled, "")
                            .on_hover_text("Enable this physics group");
                        ui.strong(&info.name);
                        if modified {
                            theme::caption(ui, "modified");
                        }
                    });
                    theme::caption(
                        ui,
                        format!(
                            "{} inputs  /  {} outputs  /  {} particles",
                            info.inputs.len(),
                            info.outputs.len(),
                            info.particles
                        ),
                    );
                    egui::CollapsingHeader::new("Tune group")
                        .id_salt("group-tuning")
                        .default_open(
                            crate::smoke_mode()
                                && std::env::var("ARIA_SMOKE_SCENARIO").as_deref()
                                    == Ok("physics-group")
                                && info.id == groups[0].id,
                        )
                        .show(ui, |ui| {
                            ui.add_enabled_ui(group.enabled && settings.enabled, |ui| {
                                tuning(
                                    ui,
                                    &mut group.strength,
                                    &mut group.inertia,
                                    &mut group.response,
                                    &mut group.gravity,
                                    &mut group.wind,
                                );
                            });
                            if ui.small_button("Reset this group").clicked() {
                                group = defaults.groups.get(&info.id).copied().unwrap_or_default();
                            }
                            ui.collapsing("Authored inputs & outputs", |ui| {
                                theme::caption(ui, &info.id);
                                theme::caption(
                                    ui,
                                    format!(
                                        "Imported output multiplier: {:.2}×",
                                        info.imported_multiplier
                                    ),
                                );
                                ui.strong("Driven by");
                                for input in &info.inputs {
                                    theme::caption(ui, input);
                                }
                                ui.strong("Moves");
                                for output in &info.outputs {
                                    theme::caption(ui, output);
                                }
                            });
                        });
                });
                ui.add_space(4.0);
            });
            if group == GroupSettings::default() {
                settings.groups.remove(&info.id);
            } else {
                settings.groups.insert(info.id.clone(), group);
            }
        }
        if shown == 0 {
            theme::caption(ui, "No groups match this filter.");
        }
        let missing = settings
            .groups
            .keys()
            .filter(|id| !groups.iter().any(|g| g.id == **id))
            .count();
        if missing > 0 {
            theme::caption(
                ui,
                format!(
                    "{missing} saved group(s) are absent from this physics export. Their settings are retained and inactive."
                ),
            );
        }
        actions
    }
}

fn tuning(
    ui: &mut egui::Ui,
    strength: &mut f32,
    inertia: &mut f32,
    response: &mut f32,
    gravity: &mut f32,
    wind: &mut f32,
) {
    for (label, value, range, hint) in [
        (
            "Strength",
            strength,
            0.0..=2.0,
            "Output amplitude. Overall × group × the avatar's imported scale.",
        ),
        (
            "Inertia",
            inertia,
            0.0..=2.0,
            "Retained momentum. Lower settles faster; higher swings longer. Particle mobility remains capped at 1.",
        ),
        (
            "Response speed",
            response,
            0.25..=2.0,
            "Scales the authored particle response. Below 1 moves more slowly; above 1 responds faster.",
        ),
        (
            "Gravity",
            gravity,
            0.0..=2.0,
            "Scales the restoring force toward the rig's gravity direction.",
        ),
        (
            "Wind",
            wind,
            -1.0..=1.0,
            "Horizontal force added to authored wind. Overall and group wind add together.",
        ),
    ] {
        ui.horizontal(|ui| {
            ui.add_sized([98.0, 22.0], egui::Label::new(label))
                .on_hover_text(hint);
            ui.add(egui::Slider::new(value, range).fixed_decimals(2))
                .on_hover_text(hint);
        });
    }
}
