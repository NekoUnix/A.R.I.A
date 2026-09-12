use crate::{effects::Effects, help, theme};
use aria_core::{
    effects::{Design, Kind, Selection},
    movement::SavedRig,
    shortcuts::Shortcut,
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
                saved.effects.designs.push(Design {
                    id,
                    name: format!("Throw {id}"),
                    ..Default::default()
                });
                self.selected = Some(id);
            }
            if help::control(ui, "sprays", |ui| ui.button("New spray")).clicked() {
                let id = saved.effects.next_id();
                let mut d = aria_core::effects::Library::default().designs[3].clone();
                d.id = id;
                d.name = format!("Spray {id}");
                saved.effects.designs.push(d);
                self.selected = Some(id);
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
                                    self.shortcut = d.hotkey.unwrap_or(Shortcut {
                                        ctrl: true,
                                        alt: true,
                                        ..Default::default()
                                    });
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
        let mut duplicate = None;
        let mut remove = None;
        let reserved: Vec<_> = saved
            .expression_hotkeys
            .values()
            .chain(saved.item_hotkeys.values())
            .copied()
            .chain(
                saved
                    .presets
                    .iter()
                    .filter_map(|p| p.hotkey)
                    .map(Shortcut::preset),
            )
            .chain(std::iter::once(Shortcut::pose()))
            .chain(
                saved
                    .effects
                    .designs
                    .iter()
                    .filter(|d| Some(d.id) != self.selected)
                    .filter_map(|d| d.hotkey),
            )
            .collect();
        if let Some(d) = saved
            .effects
            .designs
            .iter_mut()
            .find(|d| Some(d.id) == self.selected)
        {
            ui.push_id(d.id, |ui| {
                theme::category(ui, "effect-design", "Selected effect", true, |ui| {
                    help::control(ui, "effects", |ui| {
                        ui.add(egui::TextEdit::singleline(&mut d.name).char_limit(80).hint_text("Effect name"))
                    });
                    ui.horizontal_wrapped(|ui| {
                        help::label(ui, "Mode", "effects");
                        ui.selectable_value(&mut d.kind, Kind::Throw, "Throw");
                        ui.selectable_value(&mut d.kind, Kind::Spray, "Liquid spray");
                    });
                    ui.horizontal_wrapped(|ui| {
                        if help::control(ui, "effects", |ui| ui.button("Trigger now")).clicked() {
                            self.pending.push(d.id);
                        }
                        if help::control(ui, "effect-designs", |ui| ui.button("Duplicate")).clicked() {
                            duplicate = Some(d.clone());
                        }
                        if help::control(ui, "effect-designs", |ui| ui.button("Delete design")).clicked() {
                            remove = Some(d.id);
                        }
                    });
                });
                theme::category(ui, "effect-assets", "Visual assets", true, |ui| {
                    theme::caption(ui, "Pick PNG, moc3/model3.json, GLB/glTF, VRM, FBX or OBJ files. Live2D exports need their matching manifest and textures. 3D props use static geometry and base-color materials.");
                    if help::control(ui, "effect-assets", |ui| ui.button("Add asset files…")).clicked()
                        && let Some(paths) = rfd::FileDialog::new()
                            .add_filter("Throw assets", &["png", "moc3", "json", "glb", "gltf", "vrm", "fbx", "obj"])
                            .pick_files()
                    {
                        d.assets.extend(paths.into_iter().take(256_usize.saturating_sub(d.assets.len())));
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (label, name) in [("Star", "star"), ("Ball", "ball"), ("3D cube", "cube"), ("Droplet", "drop")] {
                            if help::control(ui, "effect-assets", |ui| ui.small_button(label)).clicked()
                                && d.assets.len() < 256
                            {
                                d.assets.push(format!("builtin:{name}").into());
                            }
                        }
                    });
                    let mut delete = None;
                    let mut swap = None;
                    for (i, p) in d.assets.iter().enumerate() {
                        ui.push_id(i, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.small(format!("{} · {}", i + 1, p.file_name().unwrap_or_default().to_string_lossy()))
                                    .on_hover_text(p.display().to_string());
                                if ui.add_enabled(i > 0, egui::Button::new("↑")).clicked() {
                                    swap = Some((i, i - 1));
                                }
                                if ui.add_enabled(i + 1 < d.assets.len(), egui::Button::new("↓")).clicked() {
                                    swap = Some((i, i + 1));
                                }
                                if ui.add_enabled(d.assets.len() > 1, egui::Button::new("Remove")).clicked() {
                                    delete = Some(i);
                                }
                                help::button(ui, "effect-assets");
                            });
                        });
                    }
                    if let Some((a, b)) = swap { d.assets.swap(a, b); }
                    if let Some(i) = delete { d.assets.remove(i); }
                    if help::control(ui, "effect-assets", |ui| ui.button("Reload cached assets / sounds")).clicked() {
                        self.reload();
                    }
                    help::label(ui, "Asset selection", "effect-assets");
                    egui::ComboBox::from_id_salt("effect-choice")
                        .selected_text(format!("{:?}", d.selection))
                        .show_ui(ui, |ui| {
                            for choice in [Selection::Random, Selection::Cycle, Selection::All] {
                                ui.selectable_value(&mut d.selection, choice, format!("{choice:?}"));
                            }
                        });
                    help::control(ui, "effect-motion", |ui| {
                        ui.label(if d.selection == Selection::All { "Copies of every asset" } else { "Number to emit" });
                        ui.add(egui::DragValue::new(&mut d.count).range(1..=1000));
                    });
                });
                theme::category(ui, "effect-motion", "Motion & impact", false, |ui| {
                    for (label, value, range) in [
                        ("Interval (s)", &mut d.interval, 0.0..=5.0),
                        ("Flight time (s)", &mut d.flight, 0.1..=5.0),
                        ("Item size", &mut d.size, 0.005..=1.5),
                        ("Size variation", &mut d.size_variance, 0.0..=0.9),
                        ("Target spread", &mut d.spread, 0.0..=1.0),
                        ("Flight arc", &mut d.arc, -1.0..=1.0),
                        ("Spin (°/s)", &mut d.spin, -1440.0..=1440.0),
                        ("Bounce", &mut d.bounce, 0.0..=2.0),
                        ("Time after impact (s)", &mut d.lifetime, 0.1..=15.0),
                        ("Avatar recoil", &mut d.impact, 0.0..=1.0),
                        ("Cooldown (s)", &mut d.cooldown, 0.0..=60.0),
                    ] {
                        help::control(ui, "effect-motion", |ui| ui.add(egui::Slider::new(value, range).text(label)));
                    }
                    for (label, point) in [("Launch position", &mut d.origin), ("Aim position", &mut d.target)] {
                        help::label(ui, label, "effect-motion");
                        ui.horizontal(|ui| {
                            for (n, value) in point.iter_mut().enumerate() {
                                ui.label(if n == 0 { "X →" } else { "Y ↓" });
                                ui.add(egui::DragValue::new(value).range(-2.0..=2.0).speed(0.005));
                            }
                        });
                    }
                    theme::caption(ui, "Positions and sizes use avatar canvas height. (0,0) is its center; negative Y is above. Spread randomizes impact positions. Freeze pose pauses effects for images.");
                });
                theme::category(ui, "effect-liquid", "Liquid & color", d.kind == Kind::Spray, |ui| {
                    help::control(ui, "sprays", |ui| ui.color_edit_button_srgba_unmultiplied(&mut d.tint));
                    help::control(ui, "sprays", |ui| {
                        ui.add(egui::Slider::new(&mut d.splash, 0.0..=2.0).text("Splat expansion"))
                    });
                    theme::caption(ui, "Tint multiplies the artwork; white preserves its colors. Spray particles spread, stick to an avatar surface, slowly drip, and fade. Missing a Live2D mesh lets droplets fall away.");
                });
                theme::category(ui, "effect-sounds", "Sound effects", false, |ui| {
                    for (label, path) in [("Launch sound", &mut d.launch_sound), ("Impact sound", &mut d.impact_sound)] {
                        help::label(ui, label, "effect-sounds");
                        ui.small(path.to_string_lossy());
                        ui.horizontal_wrapped(|ui| {
                            if help::control(ui, "effect-sounds", |ui| ui.button("Choose audio…")).clicked()
                                && let Some(p) = rfd::FileDialog::new()
                                    .add_filter("Sound effects", &["wav", "mp3", "ogg", "flac"])
                                    .pick_file()
                            {
                                *path = p;
                            }
                            if help::control(ui, "effect-sounds", |ui| ui.small_button("Preview")).clicked() {
                                self.audio.play(path, d.volume * saved.effects.volume);
                            }
                            if help::control(ui, "effect-sounds", |ui| ui.small_button("None")).clicked() {
                                path.clear();
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            for sound in ["whoosh", "pop", "splat", "spray"] {
                                if ui.small_button(sound).clicked() { *path = format!("builtin:{sound}").into(); }
                            }
                            help::button(ui, "effect-sounds");
                        });
                    }
                    help::control(ui, "effect-sounds", |ui| {
                        ui.add(egui::Slider::new(&mut d.volume, 0.0..=1.0).text("Design volume"))
                    });
                    theme::caption(ui, "WAV, MP3, Ogg Vorbis and FLAC; 10 seconds / 16 MiB maximum per clip. Uses the Windows default audio output. OBS must capture ARIA's application audio or desktop audio; Spout carries video only.");
                });
                theme::category(ui, "effect-key", "Effect hotkey", false, |ui| {
                    help::control(ui, "hotkeys", |ui| ui.checkbox(&mut saved.global_hotkeys, "Enable global hotkeys for this avatar"));
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut self.shortcut.ctrl, "Ctrl");
                        ui.checkbox(&mut self.shortcut.alt, "Alt");
                        ui.checkbox(&mut self.shortcut.shift, "Shift");
                        ui.checkbox(&mut self.shortcut.win, "Win");
                        help::button(ui, "hotkeys");
                    });
                    help::label(ui, "Key", "hotkeys");
                    egui::ComboBox::from_id_salt("effect-key-choice")
                        .selected_text(self.shortcut.label())
                        .show_ui(ui, |ui| {
                            for (key, name) in Shortcut::keys() {
                                ui.selectable_value(&mut self.shortcut.key, key, name);
                            }
                        });
                    if help::control(ui, "hotkeys", |ui| ui.button("Assign hotkey")).clicked() {
                        match self.shortcut.validate() {
                            Err(e) => self.message = Some(e.to_string()),
                            Ok(()) => {
                                if reserved.contains(&self.shortcut) {
                                    self.message = Some("Shortcut is already assigned to another action".into());
                                } else {
                                    d.hotkey = Some(self.shortcut);
                                    saved.global_hotkeys = true;
                                }
                            }
                        }
                    }
                    if help::control(ui, "hotkeys", |ui| ui.button("Clear hotkey")).clicked() { d.hotkey = None; }
                    if let Some(key) = d.hotkey { ui.small(format!("Assigned: {}", key.label())); }
                });
                theme::category(ui, "effect-export", "Design file & templates", false, |ui| {
                    if help::control(ui, "effect-designs", |ui| ui.button("Export this design…")).clicked()
                        && let Some(path) = rfd::FileDialog::new().set_file_name("custom-throw.aria-effect.json").save_file()
                    {
                        self.message = Some(match export(&path, d) {
                            Ok(()) => "Design exported. Keep referenced assets with the file or update their paths.".into(),
                            Err(e) => e.to_string(),
                        });
                    }
                    theme::caption(ui, "The templates folder beside ARIA includes PNG/SVG artwork, OBJ/GLB examples, a Live2D export layout, WAV audio, JSON designs and stream-tool integrations. Exported designs are editable JSON. Assets remain separate files.");
                });
                if d.name.trim().is_empty() { d.name = format!("Effect {}", d.id); }
            });
        }
        if let Some(mut d) = duplicate {
            d.id = saved.effects.next_id();
            d.name = format!("{} copy", d.name).chars().take(80).collect();
            d.hotkey = None;
            self.selected = Some(d.id);
            saved.effects.designs.push(d);
        }
        if let Some(id) = remove {
            saved.effects.designs.retain(|d| d.id != id);
            self.selected = None;
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
            assert!(d.assets.iter().all(|p| p.is_file()));
            let tmp = tempfile::tempdir().unwrap();
            let path = tmp.path().join("saved.json");
            export(&path, &d).unwrap();
            assert_eq!(import(&path).unwrap(), d);
        }
    }
}
