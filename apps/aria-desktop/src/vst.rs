//! Experimental music effects. Third-party plugin code runs only in a child process.
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
};
#[cfg(windows)]
mod worker;

pub type AudioSource = Box<dyn rodio::Source<Item = f32> + Send>;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub path: Option<PathBuf>,
    pub class_id: String,
    pub enabled: bool,
    pub parameters: BTreeMap<u32, f64>,
}
impl Settings {
    pub fn sanitize(&mut self) {
        self.parameters
            .retain(|_, v| v.is_finite() && (0.0..=1.0).contains(v));
        while self.parameters.len() > 256 {
            self.parameters.pop_last();
        }
        if self.class_id.len() > 64 {
            self.class_id.clear();
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Parameter {
    id: u32,
    name: String,
    value: f64,
    read_only: bool,
    steps: i32,
}
#[derive(Clone, Serialize, Deserialize)]
struct Info {
    name: String,
    class_id: String,
    parameters: Vec<Parameter>,
    latency: u32,
    tail: u32,
}
type Probe = mpsc::Receiver<anyhow::Result<(PathBuf, Info)>>;
#[derive(Clone, Default)]
pub struct Control {
    settings: Arc<Mutex<Settings>>,
    error: Arc<Mutex<Option<String>>>,
    cancel: Arc<AtomicBool>,
    underruns: Arc<AtomicU64>,
}
impl Control {
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn update(&self, settings: &Settings) {
        *self.settings.lock().unwrap_or_else(|e| e.into_inner()) = settings.clone();
    }
    pub fn take_error(&self) -> Option<String> {
        self.error.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
    pub fn wrap(&self, source: AudioSource) -> anyhow::Result<AudioSource> {
        let settings = self
            .settings
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if !settings.enabled {
            return Ok(source);
        }
        #[cfg(windows)]
        {
            worker::stream(source, self.clone())
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!(
                "VST3 music effects currently require Windows. Disable the effect to play this track."
            )
        }
    }
}
#[derive(Default)]
pub struct Editor {
    probe: Option<Probe>,
    info: Option<(PathBuf, Info)>,
    error: Option<String>,
}
impl Editor {
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &mut Settings, control: &Control) {
        settings.sanitize();
        if let Some(probe) = &self.probe {
            match probe.try_recv() {
                Ok(result) => {
                    self.probe = None;
                    match result {
                        Ok((path, info)) if settings.path.as_ref() == Some(&path) => {
                            settings.class_id = info.class_id.clone();
                            settings.parameters.retain(|id, _| {
                                info.parameters.iter().any(|p| p.id == *id && !p.read_only)
                            });
                            for p in info.parameters.iter().filter(|p| !p.read_only) {
                                settings.parameters.entry(p.id).or_insert(p.value);
                            }
                            self.info = Some((path, info));
                            self.error = None;
                        }
                        Ok(_) => {}
                        Err(error) => {
                            self.error = Some(format!("Plugin inspection failed: {error:#}"))
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.probe = None;
                    self.error = Some("Plugin inspection stopped unexpectedly".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let smoke = crate::smoke_mode()
            && std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("studio-vst");
        let response = egui::CollapsingHeader::new("VST3 music effect · experimental").default_open(settings.path.is_some()).show(ui, |ui| {
            crate::theme::caption(ui, "Add one effect to your music. Choose an installed plugin, enable it, then press Play. Changing plugins stops playback; parameter adjustments apply live.");
            if !cfg!(windows) { ui.label("VST3 processing currently requires Windows."); }
            ui.add_enabled(settings.enabled || (cfg!(windows) && settings.path.is_some()), egui::Checkbox::new(&mut settings.enabled, "Process music through VST3"));
            let mut inspect = smoke && self.info.is_none() && self.probe.is_none() && self.error.is_none();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(cfg!(windows) && self.probe.is_none(), egui::Button::new("Choose .vst3 file…")).clicked()
                    && let Some(path) = rfd::FileDialog::new().add_filter("VST3 plugin", &["vst3"]).pick_file() {
                    settings.path = Some(path); settings.class_id.clear(); settings.parameters.clear(); self.info = None; inspect = true;
                }
                if ui.add_enabled(cfg!(windows) && self.probe.is_none(), egui::Button::new("Choose .vst3 bundle…")).clicked()
                    && let Some(path) = rfd::FileDialog::new().set_title("Choose a .vst3 bundle folder").pick_folder() {
                    settings.path = Some(path); settings.class_id.clear(); settings.parameters.clear(); self.info = None; inspect = true;
                }
                if ui.add_enabled(cfg!(windows) && settings.path.is_some() && self.probe.is_none(), egui::Button::new("Inspect / retry")).clicked() { inspect = true; }
            });
            if let Some(path) = &settings.path { ui.label(path.display().to_string()); }
            #[cfg(windows)]
            if inspect && let Some(path) = settings.path.clone() {
                let (tx, rx) = mpsc::sync_channel(1);
                self.probe = Some(rx);
                match std::thread::Builder::new().name("aria-vst-probe".into()).spawn(move || {
                    let result = worker::probe(&path).map(|info| (path, info)); let _ = tx.send(result);
                }) {
                    Ok(_) => {}, Err(error) => { self.probe = None; self.error = Some(error.to_string()); }
                }
            }
            #[cfg(not(windows))]
            let _ = inspect;
            if self.probe.is_some() { ui.spinner(); ui.label("Inspecting plugin…"); }
            if let Some(error) = &self.error { ui.colored_label(crate::theme::orange(), error); }
            if let Some((path, info)) = &self.info && settings.path.as_ref() == Some(path) {
                ui.strong(&info.name);
                ui.label(format!("Plugin latency: {} samples at 48 kHz during inspection", info.latency));
                egui::ScrollArea::vertical().id_salt("vst-parameters").max_height(260.).show(ui, |ui| {
                    for parameter in &info.parameters {
                        let mut read_only = parameter.value;
                        let value = if parameter.read_only { &mut read_only } else { settings.parameters.entry(parameter.id).or_insert(parameter.value) };
                        let slider = egui::Slider::new(value, 0.0..=1.0).text(&parameter.name);
                        let slider = if parameter.steps > 0 { slider.step_by(1. / f64::from(parameter.steps)) } else { slider };
                        ui.add_enabled(!parameter.read_only, slider);
                    }
                });
            }
            let underruns = control.underruns.load(Ordering::Relaxed);
            if underruns > 0 { ui.colored_label(crate::theme::orange(), format!("Audio buffering underruns: {underruns} frames. Stop playback if you hear gaps.")); }
            crate::theme::caption(ui, "Controls use a normalized 0–1 scale. Seeking, plugin editor windows, instruments, sidechains and microphone effects are not available yet. Effect errors stop music; disable the effect and press Play to play without it.");
        });
        if smoke {
            response
                .header_response
                .scroll_to_me(Some(egui::Align::Min));
        }
    }
}

#[cfg(windows)]
pub fn serve() -> anyhow::Result<()> {
    worker::serve()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_reject_nonfinite_and_out_of_range_parameters() {
        let mut settings = Settings {
            parameters: BTreeMap::from([(0, f64::NAN), (1, -1.), (2, 2.), (3, 0.4)]),
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(settings.parameters, BTreeMap::from([(3, 0.4)]));
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(!old.enabled && old.path.is_none());
    }
}
