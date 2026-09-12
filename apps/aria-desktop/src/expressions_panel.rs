use aria_core::{
    expressions::{Expression, ExpressionPlayer},
    movement::{PoseMode, SavedRig},
    rig::RigParameter,
    shortcuts::Shortcut,
};
use aria_model::ExpressionFile;
use eframe::egui;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub struct Entry {
    pub file: ExpressionFile,
    expression: Expression,
    missing: Vec<String>,
}
#[derive(Default)]
pub struct ExpressionsPanel {
    pub entries: Vec<Entry>,
    player: ExpressionPlayer,
    files: Vec<ExpressionFile>,
    base: PathBuf,
    errors: Vec<String>,
    selected: Option<String>,
    draft: Shortcut,
    search: String,
}
#[derive(Default)]
pub struct Actions {
    pub save: bool,
    pub message: Option<String>,
}
impl ExpressionsPanel {
    pub fn load(
        &mut self,
        files: &[ExpressionFile],
        base: &Path,
        saved: &SavedRig,
        parameters: &[RigParameter],
    ) {
        self.files = files.to_vec();
        self.base = base.to_path_buf();
        self.reload(saved, parameters);
    }
    fn reload(&mut self, saved: &SavedRig, parameters: &[RigParameter]) {
        self.entries.clear();
        self.errors.clear();
        self.player = ExpressionPlayer::default();
        let mut files = self.files.clone();
        for path in &saved.expression_files {
            match self.file_for_path(path) {
                Ok(file) if !files.iter().any(|f| f.path == file.path) => files.push(file),
                Ok(_) => {}
                Err(error) => self.errors.push(format!("{}: {error:#}", path.display())),
            }
        }
        files.sort_by(|a, b| a.id.cmp(&b.id));
        files.dedup_by(|a, b| a.id == b.id);
        if files.len() > 256 {
            self.errors
                .push("Maximum 256 expressions per avatar; additional files were skipped.".into());
        }
        for file in files.into_iter().take(256) {
            match Self::read(&file.path) {
                Ok(expression) => {
                    let missing = expression.missing_parameters(parameters);
                    self.entries.push(Entry {
                        file,
                        expression,
                        missing,
                    });
                }
                Err(error) => self.errors.push(format!("{}: {error:#}", file.name)),
            }
        }
        let selected = self
            .selected
            .clone()
            .filter(|id| self.entries.iter().any(|e| e.file.id == *id))
            .or_else(|| self.entries.first().map(|e| e.file.id.clone()));
        self.select(selected, saved);
    }
    fn file_for_path(&self, path: &Path) -> anyhow::Result<ExpressionFile> {
        anyhow::ensure!(
            aria_model::is_expression(path),
            "Select .exp3.json or .exp3 files"
        );
        let path = path.canonicalize()?;
        let id = match path.strip_prefix(&self.base) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(_) => format!("external:{}", path.to_string_lossy().replace('\\', "/")),
        };
        Ok(ExpressionFile {
            id,
            name: aria_model::expression_name(&path),
            path,
        })
    }
    fn read(path: &Path) -> anyhow::Result<Expression> {
        Expression::load(&aria_model::read_bounded(path, 1024 * 1024)?)
    }
    fn select(&mut self, id: Option<String>, saved: &SavedRig) {
        self.draft = id
            .as_ref()
            .and_then(|id| saved.expression_hotkeys.get(id))
            .copied()
            .unwrap_or(Shortcut {
                ctrl: true,
                alt: true,
                ..Default::default()
            });
        self.selected = id;
    }
    pub fn update(&mut self, parameters: &mut [RigParameter], active: &BTreeSet<String>, dt: f32) {
        self.player.update(
            self.entries
                .iter()
                .map(|e| (e.file.id.as_str(), &e.expression)),
            active,
            parameters,
            dt,
        );
    }
    pub fn toggle(&self, id: &str, saved: &mut SavedRig) -> Option<String> {
        let entry = self.entries.iter().find(|e| e.file.id == id)?;
        if saved.config.pose.mode == PoseMode::Frozen {
            return Some("Pose is frozen. Resume live before changing expressions.".into());
        }
        let enabled = if saved.config.expressions.remove(id) {
            false
        } else {
            saved.config.expressions.insert(id.into());
            true
        };
        Some(format!(
            "{} {}",
            entry.file.name,
            if enabled { "on" } else { "off" }
        ))
    }
    pub fn assign(saved: &mut SavedRig, id: &str, shortcut: Shortcut) -> anyhow::Result<()> {
        shortcut.validate()?;
        anyhow::ensure!(
            shortcut != Shortcut::pose(),
            "That shortcut is reserved for Freeze / resume pose"
        );
        anyhow::ensure!(
            !saved
                .presets
                .iter()
                .filter_map(|p| p.hotkey)
                .any(|n| Shortcut::preset(n) == shortcut),
            "That shortcut is assigned to a preset"
        );
        anyhow::ensure!(
            !saved
                .expression_hotkeys
                .iter()
                .any(|(other, key)| other != id && *key == shortcut),
            "That shortcut is assigned to another expression"
        );
        anyhow::ensure!(
            !saved.item_hotkeys.values().any(|key| *key == shortcut),
            "That shortcut is assigned to a PNG toggle"
        );
        saved.expression_hotkeys.insert(id.into(), shortcut);
        saved.global_hotkeys = true;
        Ok(())
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        saved: &mut SavedRig,
        parameters: &[RigParameter],
    ) -> Actions {
        let mut actions = Actions::default();
        if self.base.as_os_str().is_empty() {
            crate::help::button(ui, "expressions");
            crate::theme::caption(
                ui,
                "Load a Live2D avatar to use its expression files and assign shortcuts.",
            );
            return actions;
        }
        crate::theme::category(ui, "expression-library", "Expression library", true, |ui| {
            crate::theme::caption(
                ui,
                "Toggle expressions independently. Settings and shortcuts are saved for this avatar.",
            );
            ui.horizontal_wrapped(|ui| {
                if crate::help::control(ui, "expression-files", |ui| {
                    ui.button("Import expression files…")
                })
                .clicked()
                    && let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Live2D expression", &["json", "exp3"])
                        .pick_files()
                {
                    let mut imported = 0;
                    let mut errors = Vec::new();
                    for path in paths.into_iter().take(256) {
                        let result = self.file_for_path(&path).and_then(|file| {
                            Self::read(&file.path)?;
                            Ok(file)
                        });
                        match result {
                            Ok(file) if self.entries.iter().any(|e| e.file.id == file.id) => {}
                            Ok(file)
                                if self.entries.len() + imported < 256
                                    && saved.expression_files.len() < 256 =>
                            {
                                if !saved.expression_files.contains(&file.path) {
                                    saved.expression_files.push(file.path);
                                    imported += 1;
                                }
                            }
                            Ok(_) => errors.push("Maximum 256 expression files per avatar.".into()),
                            Err(error) => errors.push(format!("{}: {error:#}", path.display())),
                        }
                    }
                    self.reload(saved, parameters);
                    self.errors.extend(errors);
                    actions.save = true;
                    actions.message = Some(format!("Imported {imported} expression files."));
                }
                if crate::help::control(ui, "expression-files", |ui| ui.button("Reload files"))
                    .on_hover_text("Reread the listed expression files after editing them on disk.")
                    .clicked()
                {
                    self.reload(saved, parameters);
                }
                if crate::help::control(ui, "expressions", |ui| {
                    ui.add_enabled(
                        saved.config.pose.mode != PoseMode::Frozen,
                        egui::Button::new("All off"),
                    )
                })
                .clicked()
                {
                    saved.config.expressions.clear();
                    actions.save = true;
                }
            });
            crate::help::control(ui, "expressions", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Find an expression…")
                        .desired_width(f32::INFINITY),
                )
            });
            if self.entries.is_empty() {
                crate::theme::caption(
                    ui,
                    "No valid expressions found. Import an expression exported for this avatar.",
                );
            }
            let search = self.search.to_lowercase();
            let mut selection = None;
            for entry in &self.entries {
                if !format!("{} {}", entry.file.name, entry.file.id)
                    .to_lowercase()
                    .contains(&search)
                {
                    continue;
                }
                ui.push_id(&entry.file.id, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let mut active = saved.config.expressions.contains(&entry.file.id);
                        crate::help::context_button(ui, "expressions", || format!(
                            "Expression: {}\nFile ID: {}\nFade in: {} seconds\nFade out: {} seconds\n\nExported values:\n{}\n\nMissing targets (skipped): {}",
                            entry.file.name, entry.file.id, entry.expression.fade_in_time, entry.expression.fade_out_time,
                            entry.expression.parameters.iter().map(|p| format!("{}: {:?} {}", p.id, p.blend, p.value)).collect::<Vec<_>>().join("\n"),
                            if entry.missing.is_empty() { "None".into() } else { entry.missing.join(", ") }
                        ));
                        if ui
                            .add_enabled(
                                saved.config.pose.mode != PoseMode::Frozen,
                                egui::Checkbox::without_text(&mut active),
                            )
                            .changed()
                        {
                            actions.message = self.toggle(&entry.file.id, saved);
                            actions.save = true;
                        }
                        if ui
                            .selectable_label(
                                self.selected.as_ref() == Some(&entry.file.id),
                                &entry.file.name,
                            )
                            .clicked()
                        {
                            selection = Some(entry.file.id.clone());
                        }
                        if let Some(key) = saved.expression_hotkeys.get(&entry.file.id) {
                            ui.small(key.label());
                        }
                    });
                });
            }
            if let Some(id) = selection {
                self.select(Some(id), saved);
            }
        });
        if let Some(entry) = self
            .entries
            .iter()
            .find(|e| Some(&e.file.id) == self.selected.as_ref())
        {
            crate::theme::category(
                ui,
                "expression-shortcut",
                "Selected expression",
                true,
                |ui| {
                    ui.strong(&entry.file.name);
                    ui.small(&entry.file.id)
                        .on_hover_text(entry.file.path.display().to_string());
                    let active = saved.config.expressions.contains(&entry.file.id);
                    ui.small(format!(
                        "{} · blend {:.0}% · fade in {:.2}s / out {:.2}s",
                        if active { "On" } else { "Off" },
                        self.player.weight(&entry.file.id) * 100.0,
                        entry.expression.fade_in_time,
                        entry.expression.fade_out_time
                    ));
                    ui.separator();
                    crate::help::label(ui, "Keyboard shortcut", "hotkeys");
                    ui.horizontal_wrapped(|ui| {
                        crate::help::control(ui, "hotkeys", |ui| {
                            ui.checkbox(&mut self.draft.ctrl, "Ctrl")
                        });
                        crate::help::control(ui, "hotkeys", |ui| {
                            ui.checkbox(&mut self.draft.alt, "Alt")
                        });
                        crate::help::control(ui, "hotkeys", |ui| {
                            ui.checkbox(&mut self.draft.shift, "Shift")
                        });
                        crate::help::control(ui, "hotkeys", |ui| {
                            ui.checkbox(&mut self.draft.win, "Win")
                        });
                    });
                    egui::ComboBox::from_id_salt("expression-key")
                        .selected_text(if self.draft.key == 0 {
                            "Choose main key".into()
                        } else {
                            self.draft.label()
                        })
                        .show_ui(ui, |ui| {
                            for (key, name) in Shortcut::keys() {
                                ui.selectable_value(&mut self.draft.key, key, name);
                            }
                        });
                    ui.horizontal_wrapped(|ui| {
                        if crate::help::control(ui, "hotkeys", |ui| ui.button("Assign shortcut"))
                            .clicked()
                        {
                            match Self::assign(saved, &entry.file.id, self.draft) {
                                Ok(()) => {
                                    actions.save = true;
                                    actions.message = Some(format!(
                                        "{} saved. Press once to turn on, again to turn off.",
                                        self.draft.label()
                                    ));
                                }
                                Err(error) => actions.message = Some(error.to_string()),
                            }
                        }
                        if crate::help::control(ui, "hotkeys", |ui| ui.button("Clear shortcut"))
                            .clicked()
                        {
                            saved.expression_hotkeys.remove(&entry.file.id);
                            actions.save = true;
                            actions.message = Some("Expression shortcut cleared.".into());
                        }
                    });
                    crate::theme::caption(
                        ui,
                        "Global shortcuts work while ARIA is unfocused. Unmodified keys also intercept normal typing. F12 is reserved by Windows.",
                    );
                    egui::CollapsingHeader::new(format!(
                        "{} parameter values",
                        entry.expression.parameters.len()
                    ))
                    .id_salt("expression-values")
                    .show(ui, |ui| {
                        for p in &entry.expression.parameters {
                            ui.small(format!("{} · {:?} {}", p.id, p.blend, p.value));
                        }
                    });
                    if !entry.missing.is_empty() {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            format!("Not on this avatar (skipped): {}", entry.missing.join(", ")),
                        );
                    }
                },
            );
        }
        crate::theme::category(
            ui,
            "expression-global-keys",
            "Global shortcuts",
            true,
            |ui| {
                actions.save |= crate::help::control(ui, "hotkeys", |ui| {
                    ui.checkbox(&mut saved.global_hotkeys, "Enable global hotkeys (Windows)")
                })
                .changed();
                crate::theme::caption(
                    ui,
                    "This switch also controls preset and pose shortcuts. Multiple expressions blend in the displayed file order. Frozen poses pause expressions.",
                );
            },
        );
        if !self.errors.is_empty() {
            crate::theme::category(
                ui,
                "expression-errors",
                "Files needing attention",
                true,
                |ui| {
                    for error in &self.errors {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                },
            );
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aria_core::{
        MappingSettings, Parameters,
        movement::{Preset, PresetKind, RigConfig, preview_parameters},
    };
    #[test]
    fn assignments_reject_conflicts_and_survive_per_model_storage() {
        let parameters = preview_parameters(Parameters::default());
        let mut saved = SavedRig {
            config: RigConfig::from_parameters(&parameters),
            ..Default::default()
        };
        let key = Shortcut {
            ctrl: true,
            shift: true,
            key: 0x48,
            ..Default::default()
        };
        ExpressionsPanel::assign(&mut saved, "hood.exp3.json", key).unwrap();
        assert!(saved.global_hotkeys);
        assert!(ExpressionsPanel::assign(&mut saved, "smile.exp3.json", key).is_err());
        assert!(ExpressionsPanel::assign(&mut saved, "hood.exp3.json", Shortcut::pose()).is_err());
        saved.presets.push(Preset {
            name: "Pose".into(),
            kind: PresetKind::Movement,
            rig: saved.config.clone(),
            mapping: MappingSettings::default(),
            hotkey: Some(1),
        });
        assert!(
            ExpressionsPanel::assign(&mut saved, "hood.exp3.json", Shortcut::preset(1)).is_err()
        );
        saved.config.expressions.insert("hood.exp3.json".into());
        saved
            .expression_files
            .push(PathBuf::from("C:/avatar/hood.exp3.json"));
        let profiles = std::collections::BTreeMap::from([
            ("model-a", saved),
            ("model-b", SavedRig::default()),
        ]);
        let bytes = serde_json::to_vec(&profiles).unwrap();
        let loaded: std::collections::BTreeMap<String, SavedRig> =
            serde_json::from_slice(&bytes).unwrap();
        loaded["model-a"].validate(&parameters).unwrap();
        assert_eq!(loaded["model-a"].expression_hotkeys["hood.exp3.json"], key);
        assert!(
            loaded["model-a"]
                .config
                .expressions
                .contains("hood.exp3.json")
        );
        assert_eq!(loaded["model-a"].expression_files.len(), 1);
        assert!(loaded["model-b"].expression_hotkeys.is_empty());
        assert!(loaded["model-b"].config.expressions.is_empty());
    }
}
