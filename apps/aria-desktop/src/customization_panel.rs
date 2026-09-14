//! A curated appearance workspace built from each avatar's exported controls.
use crate::{help, live2d::Avatar, theme};
use aria_core::{
    customization::{Choice, Control, Style},
    movement::{PresetKind, SavedRig},
    rig::RigParameter,
};
use eframe::egui;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Panel {
    search: String,
    look_name: String,
}
#[derive(Default)]
pub struct Actions {
    pub changed: bool,
    pub save_look: Option<String>,
    pub apply_look: Option<usize>,
    pub manage_looks: bool,
    pub layers: bool,
    pub expressions: bool,
}

/// A suggestion is never an applied override. Unnamed controls remain available
/// through Browse all, and known physics outputs are excluded from suggestions.
fn suggested(id: &str, label: &str, group: &str, driven: bool) -> bool {
    let name = format!("{id} {label} {group}").to_lowercase();
    let explicit = [
        "toggle",
        "switch",
        "swap",
        "costume",
        "outfit",
        "customiz",
        "customis",
        "appearance",
        "wardrobe",
        "切替",
        "衣装",
    ]
    .iter()
    .any(|s| name.contains(s));
    explicit
        || (!driven
            && [
                "colour",
                "color",
                "accessor",
                "hairstyle",
                "hair style",
                "glasses",
                "jacket",
                "shirt",
                "hat",
                "ribbon",
                "衣服",
                "髪型",
            ]
            .iter()
            .any(|s| name.contains(s)))
}
fn defaults(p: &RigParameter, label: &str, group: &str) -> Control {
    let name = format!("{} {label}", p.id).to_lowercase();
    Control {
        category: group.chars().take(30).collect(),
        style: if (p.min.abs() < 1e-6 && (p.max - 1.).abs() < 1e-6)
            && ["toggle", "switch", "swap"]
                .iter()
                .any(|s| name.contains(s))
        {
            Style::Toggle
        } else {
            Style::Slider
        },
        ..Default::default()
    }
}
impl Panel {
    #[cfg(feature = "screenshots")]
    pub fn prepare_smoke(&mut self, avatar: &Avatar, saved: &mut SavedRig) {
        for p in avatar
            .model
            .parameters()
            .iter()
            .filter(|p| {
                suggested(
                    &p.id,
                    avatar.labels.get(&p.id).map_or("", String::as_str),
                    avatar
                        .parameter_groups
                        .get(&p.id)
                        .map_or("", String::as_str),
                    saved.config.bindings.contains_key(&p.id),
                )
            })
            .filter(|p| {
                avatar
                    .labels
                    .get(&p.id)
                    .is_some_and(|label| label.to_lowercase().contains("switch"))
            })
            .take(2)
        {
            saved.config.customization.controls.insert(
                p.id.clone(),
                defaults(
                    p,
                    avatar.labels.get(&p.id).map_or("", String::as_str),
                    avatar
                        .parameter_groups
                        .get(&p.id)
                        .map_or("", String::as_str),
                ),
            );
            saved
                .config
                .customization
                .values
                .insert(p.id.clone(), p.value);
        }
        self.look_name = "Streaming outfit".into();
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        avatar: &Avatar,
        labels: &BTreeMap<String, String>,
        saved: &mut SavedRig,
    ) -> Actions {
        let mut actions = Actions::default();
        ui.heading("Customize Live2D");
        help::button(ui, "live2d-customization");
        theme::caption(
            ui,
            "Build this avatar's appearance controls from its exported outfits, hair, accessories and colors. Lock selected values while the rest of the model keeps tracking.",
        );
        let physics: BTreeSet<_> = avatar
            .physics
            .as_ref()
            .map(|p| p.groups())
            .unwrap_or_default()
            .into_iter()
            .flat_map(|g| g.outputs)
            .collect();
        let parameters = avatar.model.parameters();
        let suggestions: Vec<_> = parameters
            .iter()
            .filter(|p| {
                !physics.contains(&p.id)
                    && suggested(
                        &p.id,
                        labels.get(&p.id).map_or("", String::as_str),
                        avatar
                            .parameter_groups
                            .get(&p.id)
                            .map_or("", String::as_str),
                        saved.config.bindings.contains_key(&p.id),
                    )
            })
            .collect();
        theme::category(
            ui,
            "custom-setup",
            "Set up appearance controls",
            saved.config.customization.controls.is_empty(),
            |ui| {
                ui.label(format!(
                    "{} exported parameters · {} suggested appearance controls",
                    parameters.len(),
                    suggestions.len()
                ));
                if ui
                    .add_enabled(
                        !suggestions.is_empty(),
                        egui::Button::new("Add this model's suggested controls"),
                    )
                    .clicked()
                {
                    for p in &suggestions {
                        saved
                            .config
                            .customization
                            .controls
                            .entry(p.id.clone())
                            .or_insert_with(|| {
                                defaults(
                                    p,
                                    labels.get(&p.id).map_or("", String::as_str),
                                    avatar
                                        .parameter_groups
                                        .get(&p.id)
                                        .map_or("", String::as_str),
                                )
                            });
                    }
                    actions.changed = true;
                }
                theme::caption(
                    ui,
                    "Adding a control does not change its value. Suggestions use the creator's names and folders. Use Browse all to add any missing or unnamed option, then rename and organize it.",
                );
                ui.collapsing("Browse all exported parameters", |ui| {
                    help::control(ui, "live2d-customization", |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("Search names, folders or parameter IDs")
                                .desired_width(f32::INFINITY),
                        )
                    });
                    let query = self.search.to_lowercase();
                    let matches: Vec<_> = parameters
                        .iter()
                        .filter(|p| {
                            format!(
                                "{} {} {}",
                                p.id,
                                labels.get(&p.id).map_or("", String::as_str),
                                avatar
                                    .parameter_groups
                                    .get(&p.id)
                                    .map_or("", String::as_str)
                            )
                            .to_lowercase()
                            .contains(&query)
                        })
                        .collect();
                    egui::ScrollArea::vertical()
                        .id_salt("custom-parameter-browser")
                        .max_height(220.)
                        .show_rows(ui, 26., matches.len(), |ui, range| {
                            for index in range {
                                let p = matches[index];
                                ui.push_id(&p.id, |ui| {
                                    let mut added =
                                        saved.config.customization.controls.contains_key(&p.id);
                                    let label =
                                        labels.get(&p.id).map_or(p.id.as_str(), String::as_str);
                                    if ui
                                        .checkbox(&mut added, label)
                                        .on_hover_text(format!(
                                            "{} · range {} to {} · default {}\n{}",
                                            p.id,
                                            p.min,
                                            p.max,
                                            p.default,
                                            avatar
                                                .parameter_groups
                                                .get(&p.id)
                                                .map_or("No exported folder", String::as_str)
                                        ))
                                        .changed()
                                    {
                                        if added {
                                            saved.config.customization.controls.insert(
                                                p.id.clone(),
                                                defaults(
                                                    p,
                                                    label,
                                                    avatar
                                                        .parameter_groups
                                                        .get(&p.id)
                                                        .map_or("", String::as_str),
                                                ),
                                            );
                                        } else {
                                            saved.config.customization.controls.remove(&p.id);
                                            saved.config.customization.values.remove(&p.id);
                                        }
                                        actions.changed = true;
                                    }
                                });
                            }
                        });
                });
            },
        );
        let mut groups: BTreeMap<String, Vec<&RigParameter>> = BTreeMap::new();
        for p in parameters
            .iter()
            .filter(|p| saved.config.customization.controls.contains_key(&p.id))
        {
            let control = &saved.config.customization.controls[&p.id];
            let group = if control.category.trim().is_empty() {
                avatar
                    .parameter_groups
                    .get(&p.id)
                    .map_or("My appearance controls", String::as_str)
            } else {
                control.category.trim()
            };
            groups.entry(group.into()).or_default().push(p);
        }
        if groups.is_empty() {
            theme::caption(
                ui,
                "Start with Add suggested controls, or add individual parameters in Browse all. A model can only offer artwork and shapes included by its creator.",
            );
        }
        for (group, entries) in groups {
            theme::category(ui, ("custom-group", &group), &group, true, |ui| {
                for p in entries {
                    ui.push_id(&p.id, |ui| {
                        actions.changed |= control_ui(ui, p, labels, saved);
                    });
                }
            });
        }
        if !saved.config.customization.values.is_empty() {
            ui.label(format!(
                "{} appearance values locked",
                saved.config.customization.values.len()
            ));
            if ui.button("Release all appearance values").on_hover_text("Remove these appearance overrides. Each parameter resumes its existing tracking, expression or authored default.").clicked() {
                saved.config.customization.values.clear(); actions.changed = true;
            }
        }
        theme::category(ui, "custom-looks", "Saved appearance looks", true, |ui| {
            help::button(ui, "live2d-customization");
            theme::caption(
                ui,
                "Looks store locked appearance values, layer visibility and layer colors. Applying a look keeps live tracking running. Use Presets to assign a shortcut or export a look for this same avatar.",
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.look_name)
                    .hint_text("Look name, e.g. Streaming outfit")
                    .char_limit(30)
                    .desired_width(f32::INFINITY),
            );
            if ui
                .add_enabled(
                    !self.look_name.trim().is_empty(),
                    egui::Button::new("Save appearance look"),
                )
                .clicked()
            {
                actions.save_look = Some(self.look_name.trim().into());
            }
            for (index, preset) in saved
                .presets
                .iter()
                .enumerate()
                .filter(|(_, p)| p.kind == PresetKind::Appearance)
            {
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        actions.apply_look = Some(index);
                    }
                    ui.label(&preset.name);
                    if let Some(key) = preset.hotkey {
                        ui.small(crate::hotkeys::label(key));
                    }
                });
            }
            actions.manage_looks = ui.button("Manage looks, shortcuts & exports…").clicked();
        });
        ui.horizontal_wrapped(|ui| {
            actions.layers = ui.button("Select layers & edit colors…").clicked();
            actions.expressions = ui.button("Expression-based outfits…").clicked();
        });
        theme::caption(
            ui,
            "Expressions remain available in their own library. A full Movement preset also remembers active expressions. Exported meshes and colors can be edited in Layers, even when the model has no named outfit controls.",
        );
        actions
    }
}

fn control_ui(
    ui: &mut egui::Ui,
    p: &RigParameter,
    labels: &BTreeMap<String, String>,
    saved: &mut SavedRig,
) -> bool {
    let mut changed = false;
    let mut control = saved.config.customization.controls[&p.id].clone();
    let label = if control.label.trim().is_empty() {
        labels.get(&p.id).map_or(p.id.as_str(), String::as_str)
    } else {
        control.label.trim()
    };
    let mut locked = saved.config.customization.values.contains_key(&p.id);
    let mut value = saved
        .config
        .customization
        .values
        .get(&p.id)
        .copied()
        .unwrap_or(p.value);
    ui.horizontal(|ui| {
        changed |= help::control(ui, "live2d-customization", |ui| {
            ui.checkbox(&mut locked, label)
        })
        .changed();
    });
    let constant = (p.max - p.min).abs() < 1e-6;
    ui.add_enabled_ui(locked && !constant, |ui| {
        let response_changed = match control.style {
            Style::Slider => {
                let mut slider = egui::Slider::new(&mut value, p.min..=p.max);
                if control.step > 0. {
                    slider = slider.step_by(control.step as f64);
                }
                ui.add(slider).changed()
            }
            Style::Toggle => {
                let mut on = value >= (p.min + p.max) * 0.5;
                let changed = ui
                    .checkbox(&mut on, format!("On = {} · Off = {}", p.max, p.min))
                    .changed();
                if changed {
                    value = if on { p.max } else { p.min };
                }
                changed
            }
            Style::Choices => {
                let mut changed = false;
                let name = control
                    .choices
                    .iter()
                    .find(|c| (c.value - value).abs() < 1e-5)
                    .map_or("Choose a value", |c| c.name.as_str());
                egui::ComboBox::from_id_salt("appearance-choice")
                    .selected_text(name)
                    .show_ui(ui, |ui| {
                        for choice in &control.choices {
                            changed |= ui
                                .selectable_value(&mut value, choice.value, &choice.name)
                                .changed();
                        }
                    });
                changed
            }
        };
        changed |= response_changed;
        ui.horizontal(|ui| {
            changed |= ui
                .add(egui::DragValue::new(&mut value).range(p.min..=p.max).speed(
                    if control.step > 0. {
                        control.step as f64
                    } else {
                        (p.max - p.min) as f64 / 100.
                    },
                ))
                .changed();
            if ui.small_button("Authored default").clicked() {
                value = p.default;
                changed = true;
            }
        });
    });
    ui.small(format!(
        "{} · {} … {} · live {:.3}",
        p.id, p.min, p.max, p.value
    ));
    if constant {
        ui.small("This exported parameter has a fixed value.");
    }
    if saved.config.bindings.contains_key(&p.id) || saved.config.pose.held.contains_key(&p.id) {
        ui.small("While checked, this appearance value takes priority over this parameter's tracking or pose hold. Uncheck it to resume those controls.");
    }
    if changed {
        if locked {
            saved.config.customization.values.insert(
                p.id.clone(),
                if control.style == Style::Slider {
                    aria_core::movement::snap(value, p.min, p.max, control.step)
                } else {
                    value.clamp(p.min, p.max)
                },
            );
        } else {
            saved.config.customization.values.remove(&p.id);
        }
    }
    let mut edited = false;
    ui.collapsing("Control name, category & options", |ui| {
        ui.label("Display name");
        edited |= ui
            .add(
                egui::TextEdit::singleline(&mut control.label)
                    .hint_text("Use exported name")
                    .char_limit(60)
                    .desired_width(f32::INFINITY),
            )
            .changed();
        ui.label("Category");
        edited |= ui
            .add(
                egui::TextEdit::singleline(&mut control.category)
                    .hint_text("Use exported folder")
                    .char_limit(30)
                    .desired_width(f32::INFINITY),
            )
            .changed();
        egui::ComboBox::from_id_salt("appearance-control-style")
            .selected_text(format!("{:?}", control.style))
            .show_ui(ui, |ui| {
                for (style, label) in [
                    (Style::Slider, "Continuous slider"),
                    (Style::Toggle, "On / off"),
                    (Style::Choices, "Named choices"),
                ] {
                    edited |= ui
                        .selectable_value(&mut control.style, style, label)
                        .changed();
                }
            });
        edited |= help::control(ui, "live2d-customization", |ui| {
            ui.add(
                egui::DragValue::new(&mut control.step)
                    .range(0.0..=(p.max - p.min).max(0.))
                    .speed(0.01)
                    .prefix("Step: "),
            )
        })
        .changed();
        ui.small("Step 0 allows continuous values; step 1 suits numbered outfit variants.");
        if control.style == Style::Choices {
            let mut remove = None;
            for (index, choice) in control.choices.iter_mut().enumerate() {
                ui.push_id(index, |ui| {
                    ui.horizontal(|ui| {
                        let mut name = choice.name.clone();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut name)
                                    .char_limit(30)
                                    .desired_width(120.),
                            )
                            .changed()
                            && !name.trim().is_empty()
                        {
                            choice.name = name;
                            edited = true;
                        }
                        edited |= ui
                            .add(
                                egui::DragValue::new(&mut choice.value)
                                    .range(p.min..=p.max)
                                    .speed(0.01),
                            )
                            .changed();
                        if ui.small_button("×").clicked() {
                            remove = Some(index);
                        }
                    })
                });
            }
            if let Some(index) = remove {
                control.choices.remove(index);
                edited = true;
            }
            if ui
                .add_enabled(
                    control.choices.len() < 64,
                    egui::Button::new("Add named choice"),
                )
                .clicked()
            {
                control.choices.push(Choice {
                    name: format!("Option {}", control.choices.len() + 1),
                    value: p.min,
                });
                edited = true;
            }
        }
    });
    if edited {
        saved
            .config
            .customization
            .controls
            .insert(p.id.clone(), control);
    }
    ui.separator();
    changed || edited
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    #[ignore = "requires ARIA_TEST_MODEL, ARIA_TEST_APPEARANCE_PARAMETER, Cubism Core and GPU"]
    fn native_appearance_override_renders_and_releases_without_stopping_face_tracking() {
        use aria_core::rig::Inputs;
        use std::path::Path;
        let state = crate::spout::tests::gpu_state();
        let path = std::env::var_os("ARIA_TEST_MODEL").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let id = std::env::var("ARIA_TEST_APPEARANCE_PARAMETER").unwrap();
        let mut avatar = Avatar::load(
            &state,
            Path::new(&core),
            aria_model::load_files(Path::new(&path)).unwrap(),
        )
        .unwrap();
        let parameter = avatar
            .model
            .parameters()
            .iter()
            .find(|p| p.id == id)
            .expect("Test control belongs to this avatar")
            .clone();
        assert!(avatar.parameter_groups.contains_key(&id));
        let mut config = avatar.initial_config.clone();
        config.physics.enabled = false;
        for binding in config.bindings.values_mut() {
            binding.smoothing_ms = 0.;
        }
        let mut expressions = crate::expressions_panel::ExpressionsPanel::default();
        let inputs = Inputs::from([("ParamAngleX".into(), 12.), ("FaceAngleX".into(), 12.)]);
        avatar
            .update(&inputs, &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        let before_head = avatar
            .model
            .parameters()
            .iter()
            .find(|p| p.id == "ParamAngleX")
            .unwrap()
            .value;
        let folder = tempfile::tempdir().unwrap();
        let before = folder.path().join("before.png");
        avatar.save_png(&before).unwrap();
        let target =
            if (parameter.value - parameter.min).abs() < (parameter.value - parameter.max).abs() {
                parameter.max
            } else {
                parameter.min
            };
        config.customization.values.insert(id.clone(), target);
        config.validate(avatar.model.parameters()).unwrap();
        avatar
            .update(&inputs, &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        assert!(
            (avatar
                .model
                .parameters()
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .value
                - target)
                .abs()
                < 1e-5
        );
        let changed = folder.path().join("appearance.png");
        avatar.save_png(&changed).unwrap();
        assert_ne!(
            image::open(&before).unwrap().into_rgba8(),
            image::open(&changed).unwrap().into_rgba8(),
            "Authored appearance control changes real rendered pixels"
        );
        let moved = Inputs::from([("ParamAngleX".into(), -12.), ("FaceAngleX".into(), -12.)]);
        avatar
            .update(&moved, &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        assert!(
            (avatar
                .model
                .parameters()
                .iter()
                .find(|p| p.id == "ParamAngleX")
                .unwrap()
                .value
                - before_head)
                .abs()
                > 1.
        );
        assert!(
            (avatar
                .model
                .parameters()
                .iter()
                .find(|p| p.id == id)
                .unwrap()
                .value
                - target)
                .abs()
                < 1e-5
        );
        config.customization.values.clear();
        avatar
            .update(&inputs, &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        let restored = folder.path().join("restored.png");
        avatar.save_png(&restored).unwrap();
        assert_eq!(
            image::open(&before).unwrap().into_rgba8(),
            image::open(&restored).unwrap().into_rgba8(),
            "Releasing appearance restores the original render"
        );
        eprintln!(
            "Native appearance passed: {} · {} parameters · {} authored folders",
            id,
            avatar.model.parameters().len(),
            avatar.parameter_groups.len()
        );
    }
    #[test]
    fn suggestions_use_exported_names_not_one_test_models_ids() {
        assert!(suggested("Param139", "90S OUTFIT SWITCH", "TOGGLE", false));
        assert!(suggested("Custom_Ears", "Ear variant", "Appearance", false));
        assert!(!suggested("Param9", "HAIR P X1", "HAIR P", true));
        assert!(!suggested("ParamAngleX", "Angle X", "Face", true));
    }
}
