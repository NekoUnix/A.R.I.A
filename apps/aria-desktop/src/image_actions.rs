//! PNG/GIF action UI, shared asset cache and stage/output drawing.
use crate::{avatar::Sprite, help, theme};
use aria_core::{
    image_actions::{Animation, Config, Motion, Player, State, Transform, Transition, Trigger},
    items::SignalKind,
    movement::{PoseMode, SavedRig},
    rig::{Inputs, RigParameter},
    shortcuts::Shortcut,
};
use eframe::{
    egui::{self, Color32, Rect, vec2},
    egui_wgpu::RenderState,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct Draw {
    pub sprite: Sprite,
    pub transform: Transform,
    pub opacity: f32,
}
#[derive(Default)]
pub struct Images {
    pub player: Player,
    pub draws: Arc<[Draw]>,
    cache: BTreeMap<PathBuf, Result<Sprite, String>>,
    pending: Option<(PathBuf, crate::media::LoadJob)>,
    playback_mib: u32,
    selected: Option<u64>,
    pub message: Option<String>,
    draft: Shortcut,
}
impl Images {
    #[cfg(feature = "screenshots")]
    pub fn all_loaded(&self, config: &Config) -> bool {
        !config.states.is_empty()
            && config
                .states
                .iter()
                .all(|s| self.cache.get(&s.path).is_some_and(|s| s.is_ok()))
    }
    #[cfg(feature = "screenshots")]
    pub fn select_smoke(&mut self, id: u64) {
        self.selected = Some(id);
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn seed(&mut self, path: PathBuf, sprite: Sprite, playback_mib: u32) {
        self.playback_mib = playback_mib;
        self.cache.insert(path, Ok(sprite));
    }
    pub fn artwork(&self, path: &std::path::Path) -> Option<&Sprite> {
        self.cache.get(path)?.as_ref().ok()
    }
    pub fn error(&self, path: &std::path::Path) -> Option<&str> {
        self.cache
            .get(path)
            .and_then(|r| r.as_ref().err())
            .map(String::as_str)
    }
    pub fn primary(&self) -> Option<&Sprite> {
        self.draws.last().map(|d| &d.sprite)
    }
    pub fn merge_palette(&self, palette: &mut crate::chroma::Palette) {
        for s in self.cache.values().filter_map(|s| s.as_ref().ok()) {
            palette.merge(&s.palette);
        }
    }
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        render: Option<&RenderState>,
        config: &Config,
        inputs: &Inputs,
        parameters: &[RigParameter],
        timing: (f32, PoseMode),
    ) {
        let (dt, pose) = timing;
        self.cache
            .retain(|p, _| config.states.iter().any(|s| &s.path == p));
        if self.playback_mib != 0 && self.playback_mib != config.playback_mib {
            self.pending = None;
            self.cache.clear();
        }
        self.playback_mib = config.playback_mib;
        if self
            .pending
            .as_ref()
            .is_some_and(|(p, _)| !config.states.iter().any(|s| s.path == *p))
        {
            self.pending = None;
        }
        if let Some((path, job)) = &self.pending
            && let Some(result) = job.poll()
        {
            self.cache.insert(path.clone(), result);
            self.pending = None;
        }
        for s in &config.states {
            if self.pending.is_none() && !self.cache.contains_key(&s.path) {
                let used = self
                    .cache
                    .values()
                    .filter_map(|s| s.as_ref().ok())
                    .map(Sprite::bytes)
                    .sum();
                // Existing tiny deterministic smoke fixtures can still load synchronously.
                if cfg!(test)
                    || (crate::smoke_mode()
                        && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() != Ok("odette-gifs"))
                {
                    self.cache.insert(
                        s.path.clone(),
                        crate::media::load(ctx, render, &s.path, used, true)
                            .map_err(|e| format!("{e:#}")),
                    );
                } else {
                    match crate::media::LoadJob::start(
                        ctx,
                        render,
                        &s.path,
                        used,
                        config.playback_mib,
                    ) {
                        Ok(job) => self.pending = Some((s.path.clone(), job)),
                        Err(e) => {
                            self.cache.insert(s.path.clone(), Err(e.to_string()));
                        }
                    }
                }
            }
        }
        let mut available = config.clone();
        for s in &mut available.states {
            if !self.cache.get(&s.path).is_some_and(|s| s.is_ok()) {
                s.enabled = false;
            }
        }
        self.player
            .update(&available, inputs, parameters, dt, pose == PoseMode::Frozen);
        self.draws = self
            .player
            .layers
            .iter()
            .filter_map(|layer| {
                let state = config.states.iter().find(|s| s.id == layer.id)?;
                let sprite = self.cache.get(&state.path)?.as_ref().ok()?;
                Some(Draw {
                    sprite: sprite.at(
                        if state.restart_gif {
                            layer.age
                        } else {
                            self.player.clock()
                        },
                        state.gif_speed,
                        state.gif_loop,
                    ),
                    transform: layer.transform,
                    opacity: layer.weight,
                })
            })
            .collect::<Vec<_>>()
            .into();
    }
    pub fn add(&mut self, config: &mut Config, path: PathBuf, trigger: Trigger) -> u64 {
        let mut id = config
            .states
            .iter()
            .map(|s| s.id)
            .max()
            .unwrap_or(0)
            .wrapping_add(1)
            .max(1);
        while config.states.iter().any(|s| s.id == id) {
            id = id.wrapping_add(1).max(1);
        }
        if config.states.len() >= 128 {
            self.message = Some("Maximum 128 image actions per avatar".into());
            return id;
        }
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(80)
            .collect::<String>();
        config.states.push(State {
            id,
            path,
            trigger,
            name: if name.is_empty() {
                "Image action".into()
            } else {
                name
            },
            priority: if trigger == Trigger::Idle { 0 } else { 10 },
            ..Default::default()
        });
        config.enabled = true;
        self.selected = Some(id);
        self.draft = Shortcut {
            ctrl: true,
            alt: true,
            ..Default::default()
        };
        id
    }
    pub fn panel(
        &mut self,
        ui: &mut egui::Ui,
        saved: &mut SavedRig,
        inputs: &Inputs,
        parameters: &[RigParameter],
        is_image: bool,
    ) -> bool {
        let before = saved.config.images.clone();
        let mut save = false;
        help::label(ui, "PNG & GIF image actions", "image-actions");
        if !is_image {
            ui.label("Open a PNG/GIF avatar in Avatar & appearance to use image actions. Microphone inputs also work with Live2D.");
        }
        help::control(ui, "image-actions", |ui| {
            ui.checkbox(&mut saved.config.images.enabled, "Enable image actions")
        });
        ui.horizontal_wrapped(|ui| {
            for (label, trigger) in [
                ("Add images…", Trigger::Input),
                ("Talking image…", Trigger::Talking),
                ("Blink image…", Trigger::Blink),
            ] {
                if help::control(ui, "image-actions", |ui| ui.button(label)).clicked()
                    && let Some(paths) = rfd::FileDialog::new()
                        .add_filter("PNG / GIF artwork", &["png", "gif"])
                        .pick_files()
                {
                    for path in paths {
                        self.add(&mut saved.config.images, path, trigger);
                    }
                }
            }
            if ui.button("Reload artwork").clicked() {
                self.pending = None;
                self.cache.clear();
            }
            if ui.button("Save actions").clicked() {
                self.message = Some("Image actions saved for this avatar.".into());
                save = true;
            }
            if help::control(ui,"image-actions",|ui|ui.button("Import actions…")).clicked()
                && let Some(path)=rfd::FileDialog::new().add_filter("Image action configuration", &["json"]).pick_file() {
                match import_config(&path) {
                    Ok(config)=>{saved.config.images=config;self.reset();save=true;self.message=Some("Imported image actions. Hotkeys were cleared; choose an action to configure it.".into());}
                    Err(e)=>self.message=Some(format!("Cannot import: {e:#}")),
                }
            }
            if help::control(ui,"image-actions",|ui|ui.button("Export actions…")).clicked()
                && let Some(path)=rfd::FileDialog::new().set_file_name("avatar.aria-images.json").save_file() {
                self.message=Some(match export_config(&saved.config.images,&path){Ok(())=>"Exported image actions; keep referenced artwork with the configuration.".into(),Err(e)=>format!("Cannot export: {e:#}")});
            }
        });
        help::control(ui, "gif-memory", |ui| {
            egui::ComboBox::from_id_salt("gif-memory")
                .selected_text(format!("{} MiB per GIF", saved.config.images.playback_mib))
                .show_ui(ui, |ui| {
                    for (mib, label) in [
                        (64, "Compact · 64 MiB"),
                        (128, "Balanced · 128 MiB"),
                        (256, "Detailed · 256 MiB"),
                        (512, "High detail · 512 MiB"),
                        (1024, "Maximum detail · 1024 MiB"),
                    ] {
                        ui.selectable_value(&mut saved.config.images.playback_mib, mib, label);
                    }
                })
        });
        if let Some((path, job)) = &self.pending {
            let (done, total) = job.progress();
            ui.label(format!(
                "Loading {} · {done}/{total} frames",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
            ui.add(egui::ProgressBar::new(if total > 0 {
                done as f32 / total as f32
            } else {
                0.0
            }));
        }
        if ui.button("Resume automatic actions").clicked() {
            saved.config.images.manual = None;
        }
        if let Some(message) = &self.message {
            ui.colored_label(theme::mint(), message);
        }
        theme::category(ui, "image-library", "Image action library", true, |ui| {
            for s in &mut saved.config.images.states {
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut s.enabled, "");
                    if ui
                        .selectable_label(self.selected == Some(s.id), &s.name)
                        .clicked()
                    {
                        self.selected = Some(s.id);
                        self.draft = s.hotkey.unwrap_or(Shortcut {
                            ctrl: true,
                            alt: true,
                            ..Default::default()
                        });
                    }
                    if self.player.current == Some(s.id) {
                        ui.colored_label(theme::mint(), "active");
                    }
                    if let Some(k) = s.hotkey {
                        ui.small(k.label());
                    }
                });
            }
        });
        let Some(index) = saved
            .config
            .images
            .states
            .iter()
            .position(|s| Some(s.id) == self.selected)
        else {
            return before != saved.config.images || save;
        };
        let mut remove = false;
        let mut assign = false;
        let mut clear = false;
        let mut preview = false;
        let mut duplicate = false;
        let s = &mut saved.config.images.states[index];
        let id = s.id;
        ui.push_id(id,|ui|{
            theme::category(ui,"image-art","Artwork & trigger",true,|ui|{
                help::control(ui,"image-actions",|ui|ui.add(egui::TextEdit::singleline(&mut s.name).char_limit(80).desired_width(f32::INFINITY)));
                if s.name.trim().is_empty(){s.name="Image action".into();}
                ui.small(s.path.display().to_string());
                if let Some(Err(error))=self.cache.get(&s.path){ui.colored_label(Color32::LIGHT_RED,error);}
                if help::control(ui,"image-actions",|ui|ui.button("Choose PNG / GIF…")).clicked()&&let Some(path)=rfd::FileDialog::new().add_filter("PNG / GIF", &["png","gif"]).pick_file(){s.path=path;}
                help::control(ui,"image-actions",|ui|egui::ComboBox::from_id_salt("image-trigger").selected_text(format!("{:?}",s.trigger)).show_ui(ui,|ui|{
                    for (v,label) in [(Trigger::Idle,"Idle / fallback"),(Trigger::Talking,"Talking"),(Trigger::Quiet,"Quiet"),(Trigger::Blink,"Blink"),(Trigger::Input,"Tracking or parameter range"),(Trigger::Manual,"Manual / hotkey only")]{ui.selectable_value(&mut s.trigger,v,label);}
                }));
                help::control(ui,"image-actions",|ui|ui.add(egui::Slider::new(&mut s.priority,-1000..=1000).text("Priority (higher wins)")));
                if s.trigger==Trigger::Input {
                    help::control(ui,"image-actions",|ui|egui::ComboBox::from_id_salt("image-signal-kind").selected_text(format!("{:?}",s.rule.kind)).show_ui(ui,|ui|{ui.selectable_value(&mut s.rule.kind,SignalKind::Tracking,"Tracking / microphone");ui.selectable_value(&mut s.rule.kind,SignalKind::Parameter,"Avatar parameter");}));
                    help::control(ui,"image-actions",|ui|egui::ComboBox::from_id_salt("image-source").selected_text(&s.rule.source).show_ui(ui,|ui|{
                        let names:Vec<_>=if s.rule.kind==SignalKind::Tracking{inputs.keys().cloned().collect()}else{parameters.iter().map(|p|p.id.clone()).collect()};
                        for name in names{ui.selectable_value(&mut s.rule.source,name.clone(),name);}
                    }));
                    let source_edit=help::control(ui,"image-actions",|ui|ui.add(egui::TextEdit::singleline(&mut s.rule.source).char_limit(256)));
                    if source_edit.lost_focus() && s.rule.source.trim().is_empty(){s.rule.source="MouthOpen".into();}
                    for (label,value) in [("Range starts",&mut s.rule.start),("Range ends",&mut s.rule.end),("Hysteresis",&mut s.rule.hysteresis)]{help::control(ui,"image-actions",|ui|{ui.label(label);ui.add(egui::DragValue::new(value).range(-1e6..=1e6).speed(0.01));});}
                    s.rule.end=s.rule.end.max(s.rule.start);s.rule.hysteresis=s.rule.hysteresis.max(0.0);
                    let value=if s.rule.kind==SignalKind::Tracking{inputs.get(&s.rule.source).copied()}else{parameters.iter().find(|p|p.id==s.rule.source).map(|p|p.value)};ui.small(format!("Current input: {}",value.map_or("unavailable".into(),|v|format!("{v:.3}"))));
                }
                ui.horizontal_wrapped(|ui|{preview=ui.button("Activate / hold image").clicked();duplicate=ui.button("Duplicate").clicked();remove=ui.button("Remove action").clicked();});
            });
            theme::category(ui,"image-transition","Fade & transition",true,|ui|{
                help::control(ui,"image-transitions",|ui|egui::ComboBox::from_id_salt("image-transition").selected_text(format!("{:?}",s.transition)).show_ui(ui,|ui|{for (v,label)in[(Transition::Cut,"Cut"),(Transition::Crossfade,"Crossfade"),(Transition::FadeThrough,"Fade through transparent"),(Transition::Slide,"Slide & fade")]{ui.selectable_value(&mut s.transition,v,label);}}));
                for (label,value)in[("Fade in (s)",&mut s.fade_in),("Fade out (s)",&mut s.fade_out),("Minimum state hold (s)",&mut s.minimum_hold)]{help::control(ui,"image-transitions",|ui|ui.add(egui::Slider::new(value,0.0..=10.0).text(label)));}
            });
            motion_ui(ui,&mut s.motion);
            theme::category(ui,"image-gif","GIF playback",true,|ui|{
                help::control(ui,"gif-playback",|ui|ui.add(egui::Slider::new(&mut s.gif_speed,0.05..=4.0).text("GIF speed")));
                help::control(ui,"gif-playback",|ui|ui.checkbox(&mut s.gif_loop,"Loop GIF"));
                help::control(ui,"gif-playback",|ui|ui.checkbox(&mut s.restart_gif,"Restart GIF when action activates"));
                if let Some(Ok(sprite))=self.cache.get(&s.path){ui.small(sprite.animation.as_ref().map_or("Static image".into(),|a|format!("{} frames · {:.2}s · {} × {} playback · {:.1} MiB",a.frames.len(),a.ends.last().unwrap(),sprite.texture.size()[0],sprite.texture.size()[1],sprite.bytes() as f64/1048576.0)));}
            });
            theme::category(ui,"image-hotkey","Image action hotkey",true,|ui|{
                shortcut_editor(ui,&mut self.draft);
                ui.horizontal_wrapped(|ui|{assign=ui.button("Assign hotkey").clicked();clear=ui.button("Clear hotkey").clicked();});
                ui.small("The hotkey holds this image; press it again to resume automatic actions. Enable global shortcuts in Presets to use it outside ARIA.");
            });
        });
        if preview {
            saved.config.images.manual = Some(id);
        }
        if clear {
            saved.config.images.states[index].hotkey = None;
        }
        if assign {
            let mut candidate = saved.clone();
            candidate.config.images.states[index].hotkey = Some(self.draft);
            match candidate.validate(parameters) {
                Ok(()) => {
                    saved.global_hotkeys = true;
                    saved.config.images.states[index].hotkey = Some(self.draft);
                    self.message = Some(format!("Assigned {}", self.draft.label()));
                }
                Err(e) => self.message = Some(format!("Cannot assign: {e:#}")),
            }
        }
        if duplicate {
            let mut copy = saved.config.images.states[index].clone();
            let new = self.add(&mut saved.config.images, copy.path.clone(), copy.trigger);
            copy.id = new;
            copy.name = format!("{} copy", copy.name.chars().take(75).collect::<String>());
            copy.hotkey = None;
            if let Some(last) = saved.config.images.states.last_mut()
                && last.id == new
            {
                *last = copy;
            }
        }
        if remove {
            saved.config.images.states.remove(index);
            if saved.config.images.manual == Some(id) {
                saved.config.images.manual = None;
            }
            self.selected = None;
        }
        before != saved.config.images || save
    }
}
pub fn import_config(path: &std::path::Path) -> anyhow::Result<Config> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 1024 * 1024,
        "Image configuration exceeds 1 MiB"
    );
    let mut config: Config = serde_json::from_slice(&bytes)?;
    config.manual = None;
    for s in &mut config.states {
        s.hotkey = None;
        if s.path.is_relative() {
            s.path = path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(&s.path);
        }
    }
    config.validate()?;
    Ok(config)
}
fn export_config(config: &Config, path: &std::path::Path) -> anyhow::Result<()> {
    config.validate()?;
    let mut config = config.clone();
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    for state in &mut config.states {
        if let Ok(relative) = state.path.strip_prefix(parent) {
            state.path = relative.to_owned();
        }
    }
    std::fs::write(path, serde_json::to_vec_pretty(&config)?)?;
    Ok(())
}
fn shortcut_editor(ui: &mut egui::Ui, draft: &mut Shortcut) {
    ui.horizontal_wrapped(|ui| {
        for (label, value) in [
            ("Ctrl", &mut draft.ctrl),
            ("Alt", &mut draft.alt),
            ("Shift", &mut draft.shift),
            ("Win", &mut draft.win),
        ] {
            help::control(ui, "hotkeys", |ui| ui.checkbox(value, label));
        }
    });
    egui::ComboBox::from_id_salt("image-shortcut-key")
        .selected_text(if draft.key == 0 {
            "Choose key".into()
        } else {
            draft.label()
        })
        .show_ui(ui, |ui| {
            for (key, label) in Shortcut::keys() {
                ui.selectable_value(&mut draft.key, key, label);
            }
        });
}

pub fn motion_ui(ui: &mut egui::Ui, m: &mut Motion) {
    theme::category(ui, "image-motion", "Animation on change", true, |ui| {
        help::control(ui, "image-motion", |ui| {
            egui::ComboBox::from_id_salt("image-motion-kind")
                .selected_text(format!("{:?}", m.kind))
                .show_ui(ui, |ui| {
                    for v in [
                        Animation::None,
                        Animation::Shake,
                        Animation::Jump,
                        Animation::Blip,
                        Animation::Pulse,
                        Animation::Wobble,
                        Animation::Bob,
                    ] {
                        ui.selectable_value(&mut m.kind, v, format!("{v:?}"));
                    }
                })
        });
        for (label, value, range) in [
            ("Strength", &mut m.strength, 0.0..=0.5),
            ("Duration (s)", &mut m.duration, 0.05..=10.0),
            ("Frequency (Hz)", &mut m.frequency, 0.1..=30.0),
        ] {
            help::control(ui, "image-motion", |ui| {
                ui.add(egui::Slider::new(value, range).text(label))
            });
        }
        help::control(ui, "image-motion", |ui| {
            ui.checkbox(&mut m.repeat, "Repeat while action is active")
        });
    });
}
pub fn paint(
    p: &egui::Painter,
    rect: Rect,
    params: aria_core::Parameters,
    zoom: f32,
    layers: &[Draw],
    fields: &[aria_core::deformation::Field],
) {
    for d in layers {
        let size = d.sprite.size
            * (rect.width() * 0.75 / d.sprite.size.x).min(rect.height() * 0.9 / d.sprite.size.y)
            * zoom;
        let center = rect.center()
            + vec2(
                params.0[0] * rect.width() * 0.002,
                -params.0[1] * rect.height() * 0.001,
            )
            + vec2(d.transform.offset[0], d.transform.offset[1]) * size.y;
        p.add(egui::Shape::mesh(crate::deformation::textured_mesh(
            d.sprite.texture.id(),
            center,
            size * vec2(d.transform.scale[0], d.transform.scale[1]),
            -params.0[2].to_radians() + d.transform.rotation,
            Color32::WHITE.gamma_multiply(d.opacity * d.transform.opacity),
            None,
            fields,
        )));
    }
}

#[cfg(feature = "screenshots")]
pub fn verify_smoke(
    ctx: &egui::Context,
    state: &RenderState,
    scene: &crate::output::Scene,
    rig: &mut aria_core::movement::RigConfig,
    parameters: &[RigParameter],
    time: f32,
) {
    let key = egui::Id::new("gif-native-proof");
    if ctx.data(|d| d.get_temp::<bool>(key.with("done")).unwrap_or(false)) {
        return;
    }
    let mut seen = ctx
        .data(|d| d.get_temp::<Vec<egui::TextureId>>(key))
        .unwrap_or_default();
    if let Some(draw) = scene.images.last()
        && !seen.contains(&draw.sprite.texture.id())
    {
        seen.push(draw.sprite.texture.id());
    }
    ctx.data_mut(|d| d.insert_temp(key, seen.clone()));
    if time < 2.0 {
        return;
    }
    assert!(seen.len() > 1, "Native avatar GIF must advance");
    assert!(
        scene
            .items
            .iter()
            .any(|d| matches!(&d.image,crate::items::ItemImage::Png(s) if s.animation.is_some())),
        "GIF stage object must load"
    );
    assert!(
        scene
            .effects
            .iter()
            .any(|d| matches!(&d.image,crate::items::ItemImage::Png(s) if s.animation.is_some())),
        "GIF throw must load"
    );
    let base = PathBuf::from(std::env::var_os("ARIA_SCREENSHOT_TO").unwrap()).with_extension("");
    let image_path = PathBuf::from(format!("{}-freeze.png", base.display()));
    if let Some((at, pixels)) = ctx.data(|d| d.get_temp::<(f32, Vec<u8>)>(key.with("frozen"))) {
        if time - at < 0.35 {
            return;
        }
        crate::broadcast::save_png(ctx, state, scene, &image_path).unwrap();
        let image = image::open(&image_path).unwrap().into_rgba8();
        assert!(
            pixels == image.as_raw().as_slice(),
            "Frozen GIF, throws and transitions must hold exactly"
        );
        assert_eq!(
            image.get_pixel(0, 0).0[3],
            0,
            "GIF output keeps transparent alpha"
        );
        ctx.data_mut(|d| d.insert_temp(key.with("done"), true));
        eprintln!(
            "GIF native verification: {} avatar frames observed, stage GIF and thrown GIFs rendered; frozen PNG is byte-identical and transparent",
            seen.len()
        );
    } else {
        rig.capture_pose(parameters);
        crate::broadcast::save_png(ctx, state, scene, &image_path).unwrap();
        let pixels = image::open(&image_path).unwrap().into_rgba8().into_raw();
        ctx.data_mut(|d| d.insert_temp(key.with("frozen"), (time, pixels)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_import_export_profile_storage_and_hotkey_conflicts() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../templates/images/starter.aria-images.json");
        let config = import_config(&path).unwrap();
        assert_eq!(config.states.len(), 4);
        assert!(config.states.iter().all(|s| s.path.is_file()));
        let mut saved = SavedRig::default();
        saved.config.images = config;
        let key = Shortcut {
            ctrl: true,
            alt: true,
            key: 0x41,
            ..Default::default()
        };
        saved.config.images.states[3].hotkey = Some(key);
        saved.microphone.gain_db = 6.0;
        let params = aria_core::movement::preview_parameters(Default::default());
        saved.validate(&params).unwrap();
        let encoded = serde_json::to_vec(&saved).unwrap();
        let decoded: SavedRig = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.config.images, saved.config.images);
        assert_eq!(decoded.microphone.gain_db, 6.0);
        assert!(crate::items::Items::assign(&mut saved, 1, key).is_err());
        assert!(
            crate::expressions_panel::ExpressionsPanel::assign(&mut saved, "test", key).is_err()
        );
        assert!(
            crate::effect_editor::validate_save(
                &aria_core::effects::Design {
                    hotkey: Some(key),
                    ..Default::default()
                },
                &saved
            )
            .is_err()
        );
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("actions.json");
        export_config(&saved.config.images, &file).unwrap();
        let imported = import_config(&file).unwrap();
        assert!(imported.states.iter().all(|s| s.hotkey.is_none()));
        assert!(imported.states[0].path.is_file());
    }
}
