use crate::{effects::Effects, help, theme};
use aria_core::{
    effects::{Design, Kind},
    movement::SavedRig,
};
use eframe::egui;
use std::path::Path;
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DesignFile {
    pub aria_effect: u32,
    pub design: Design,
}
impl Effects {
    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        saved: &mut SavedRig,
        api: &mut crate::effect_api::Settings,
        api_error: Option<&str>,
    ) -> bool {
        let before = saved.effects.clone();
        let old_api = api.clone();
        let old_global = saved.global_hotkeys;
        let mut save = false;
        theme::caption(
            ui,
            "Build reusable throws and liquid sprays for this avatar. Buttons, hotkeys and stream-tool plugins trigger the same saved designs. Add as many designs as you need.",
        );
        ui.horizontal_wrapped(|ui| {
            if help::control(ui, "effects", |ui| ui.button("New throw")).clicked() {
                let id = saved.effects.next_id();
                self.editor = Some(Box::new(crate::effect_editor::Editor::new(Design {
                    id,
                    name: format!("Throw {id}"),
                    fade_out: Some(0.7),
                    ..Default::default()
                })));
            }
            if help::control(ui, "sprays", |ui| ui.button("New spray")).clicked() {
                let id = saved.effects.next_id();
                let mut d = aria_core::effects::Library::default().designs[3].clone();
                d.id = id;
                d.name = format!("Spray {id}");
                d.fade_out = Some(0.7);
                self.editor = Some(Box::new(crate::effect_editor::Editor::new(d)));
            }
            if help::control(ui, "effects", |ui| ui.button("Clear active effects")).clicked() {
                self.clear();
            }
        });
        ui.horizontal_wrapped(|ui| {
            help::control(ui, "effects", |ui| {
                ui.checkbox(&mut self.paused, "Pause effects")
            });
            help::control(ui, "effect-sounds", |ui| {
                ui.checkbox(&mut saved.effects.muted, "Mute effects")
            });
        });
        help::control(ui, "effect-sounds", |ui| {
            ui.add(egui::Slider::new(&mut saved.effects.volume, 0.0..=1.0).text("Master volume"))
        });
        ui.small(format!(
            "{} particles active / {} maximum",
            self.simulation.particles.len(),
            aria_core::effects::MAX_ACTIVE
        ));
        if let Some(message) = &self.message {
            ui.colored_label(theme::MINT, message);
        }
        if let Some(error) = &self.audio.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        theme::category(ui, "effect-library", "Throws & sprays", true, |ui| {
            help::control(ui, "effects", |ui| {
                ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search designs"))
            });
            egui::ScrollArea::vertical()
                .id_salt("effect-library-scroll")
                .max_height(240.0)
                .show(ui, |ui| {
                    for d in saved
                        .effects
                        .designs
                        .iter()
                        .filter(|d| d.name.to_lowercase().contains(&self.search.to_lowercase()))
                    {
                        ui.push_id(d.id, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .selectable_label(self.selected == Some(d.id), &d.name)
                                    .clicked()
                                {
                                    self.selected = Some(d.id);
                                }
                                if help::control(ui, "effects", |ui| {
                                    ui.small_button(if d.kind == Kind::Spray {
                                        "Spray"
                                    } else {
                                        "Throw"
                                    })
                                })
                                .clicked()
                                {
                                    self.pending.push(d.id);
                                }
                                ui.small(format!("#{} · {} assets", d.id, d.assets.len()));
                                if let Some(key) = d.hotkey {
                                    ui.small(key.label());
                                }
                            });
                        });
                    }
                });
        });
        if let Some(d) = saved
            .effects
            .designs
            .iter()
            .find(|d| Some(d.id) == self.selected)
            .cloned()
        {
            ui.horizontal_wrapped(|ui| {
                if help::control(ui, "effect-editor", |ui| {
                    ui.button("Edit toggle / directions…")
                })
                .clicked()
                {
                    self.editor = Some(Box::new(crate::effect_editor::Editor::new(d.clone())));
                }
                if ui.button("Duplicate").clicked() {
                    let mut copy = d.clone();
                    copy.id = saved.effects.next_id();
                    copy.hotkey = None;
                    copy.name = format!("{} copy", copy.name).chars().take(80).collect();
                    self.editor = Some(Box::new(crate::effect_editor::Editor::new(copy)));
                }
                if ui.button("Delete").clicked() {
                    saved.effects.designs.retain(|other| other.id != d.id);
                    self.selected = None;
                }
            });
            ui.small(format!(
                "{} objects · {} paths · {:.1}s after impact",
                d.total_count(),
                d.routes.len().max(1),
                d.lifetime
            ));
        }
        ui.horizontal_wrapped(|ui| {
            if help::control(ui, "effect-designs", |ui| ui.button("Import design…")).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("ARIA effect design", &["json"])
                    .pick_file()
            {
                match import(&path) {
                    Ok(mut d) => {
                        d.id = saved.effects.next_id();
                        d.hotkey = None;
                        self.selected = Some(d.id);
                        saved.effects.designs.push(d);
                    }
                    Err(e) => self.message = Some(format!("Import failed: {e:#}")),
                }
            }
            if help::control(ui, "effect-designs", |ui| ui.button("Save effects")).clicked() {
                save = true;
                self.message = Some("Effect designs saved for this avatar".into());
            }
        });
        theme::category(
            ui,
            "effect-plugin",
            "Stream events & plugins",
            false,
            |ui| {
                if help::control(ui, "effect-api", |ui| {
                    ui.checkbox(&mut api.enabled, "Enable local plugin API")
                })
                .changed()
                    && api.enabled
                    && api.token.is_empty()
                    && let Err(e) = api.regenerate()
                {
                    api.enabled = false;
                    self.message = Some(e.to_string());
                }
                help::control(ui, "effect-api", |ui| {
                    ui.label("Loopback TCP port");
                    ui.add(egui::DragValue::new(&mut api.port).range(1024..=65535));
                });
                ui.small(format!("http://127.0.0.1:{}/v1/effects", api.port));
                ui.horizontal_wrapped(|ui| {
                    if help::control(ui, "effect-api", |ui| ui.button("Copy API key")).clicked() {
                        ui.ctx().copy_text(api.token.clone());
                    }
                    if help::control(ui, "effect-api", |ui| ui.button("Generate new key")).clicked()
                        && let Err(e) = api.regenerate()
                    {
                        self.message = Some(e.to_string());
                    }
                });
                if let Some(e) = api_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
                theme::caption(
                    ui,
                    "Bind Twitch redeems, cheers, subs, chat commands or other stream events in Streamer.bot to the supplied HTTP/C# adapter. Touch Portal and custom plugins can use the same API. Only saved effect IDs can run; the API cannot open asset paths or execute programs.",
                );
            },
        );
        if before != saved.effects || old_api != *api || old_global != saved.global_hotkeys {
            self.save_after = Some(std::time::Instant::now());
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(550));
        }
        if save {
            self.save_after = None;
        }
        save
    }
}
pub fn export(path: &Path, d: &Design) -> anyhow::Result<()> {
    d.validate()?;
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&DesignFile {
            aria_effect: 1,
            design: d.clone(),
        })?,
    )?;
    Ok(())
}
pub fn import(path: &Path) -> anyhow::Result<Design> {
    let mut file: DesignFile =
        serde_json::from_slice(&aria_model::read_bounded(path, 2 * 1024 * 1024)?)?;
    anyhow::ensure!(file.aria_effect == 1, "Unsupported effect design version");
    file.design.validate()?;
    for p in file
        .design
        .assets
        .iter_mut()
        .chain([&mut file.design.launch_sound, &mut file.design.impact_sound])
    {
        if !p.as_os_str().is_empty()
            && !p.to_string_lossy().starts_with("builtin:")
            && !p.is_absolute()
        {
            *p = path.parent().unwrap_or(Path::new(".")).join(&*p);
        }
    }
    Ok(file.design)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_templates_import_relative_assets_and_round_trip() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/effects");
        for name in ["custom-throw", "custom-spray", "custom-3d"] {
            let d = import(&root.join(format!("{name}.aria-effect.json"))).unwrap();
            assert!(
                d.assets
                    .iter()
                    .all(|p| p.is_file() || p.to_string_lossy().starts_with("builtin:"))
            );
            assert_eq!(d.assets.len(), d.asset_counts.len());
            assert_eq!(d.routes.len(), 2);
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path().join("saved.json");
            export(&path, &d).unwrap();
            assert_eq!(import(&path).unwrap(), d);
        }
    }
}
