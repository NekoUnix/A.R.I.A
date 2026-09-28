//! Workspace sound actions. Decoding and device opening never run on the UI thread.
use crate::actions::{Choice, Target};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Instant,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub volume: f32,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub clips: Vec<Clip>,
    next_id: u64,
}
impl Library {
    fn add(&mut self, path: PathBuf) -> anyhow::Result<()> {
        anyhow::ensure!(self.clips.len() < 128, "Sound library is full (128 clips)");
        let id = self
            .next_id
            .max(self.clips.iter().map(|c| c.id).max().unwrap_or(0))
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Sound IDs exhausted"))?;
        self.next_id = id;
        self.clips.push(Clip {
            id,
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .take(80)
                .collect(),
            path,
            volume: 0.5,
        });
        Ok(())
    }
    pub fn choices(&self) -> impl Iterator<Item = Choice> + '_ {
        self.clips
            .iter()
            .take(128)
            .map(|c| Choice {
                target: Target::Sound(c.id),
                label: format!("Sound · {}", c.name),
                shortcut: None,
                legacy_label: None,
            })
            .chain(std::iter::once(Choice {
                target: Target::StopSounds,
                label: "Sound · Stop all clips".into(),
                shortcut: None,
                legacy_label: None,
            }))
    }
}
struct Request {
    clip: Clip,
    generation: u64,
    created: Instant,
}
struct Worker {
    tx: mpsc::SyncSender<Request>,
    generation: Arc<AtomicU64>,
    error: Arc<Mutex<Option<String>>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}
#[derive(Default)]
pub struct Sounds {
    worker: Option<Worker>,
    message: Option<String>,
}
impl Sounds {
    pub fn stop(&mut self) {
        if let Some(w) = &self.worker {
            w.generation.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn play(&mut self, library: &Library, id: u64) -> anyhow::Result<()> {
        let clip = library
            .clips
            .iter()
            .take(128)
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow::anyhow!("Sound was removed; repair the action target"))?
            .clone();
        anyhow::ensure!(
            clip.volume.is_finite() && (0.0..=1.0).contains(&clip.volume),
            "Invalid sound volume"
        );
        anyhow::ensure!(
            !clip.path.as_os_str().is_empty(),
            "Choose a sound file first"
        );
        if self.worker.is_none() {
            let (tx, rx) = mpsc::sync_channel::<Request>(16);
            let generation = Arc::new(AtomicU64::new(0));
            let error = Arc::new(Mutex::new(None));
            let g = generation.clone();
            let e = error.clone();
            std::thread::Builder::new()
                .name("aria-sounds".into())
                .spawn(move || {
                    let mut audio = crate::effect_audio::Audio::default();
                    let mut current = 0;
                    loop {
                        let next = g.load(Ordering::Relaxed);
                        if next != current {
                            audio.stop();
                            current = next;
                        }
                        match rx.recv_timeout(std::time::Duration::from_millis(25)) {
                            Ok(request)
                                if request.generation == current
                                    && request.created.elapsed().as_secs_f32() <= 5. =>
                            {
                                audio.error = None;
                                audio.play(&request.clip.path, request.clip.volume);
                                if g.load(Ordering::Relaxed) != current {
                                    audio.stop();
                                }
                                if let Some(err) = audio.error.take() {
                                    *e.lock().unwrap() =
                                        Some(format!("{}: {err}", request.clip.name));
                                }
                            }
                            Ok(_) => {}
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                })?;
            self.worker = Some(Worker {
                tx,
                generation,
                error,
            });
        }
        let w = self.worker.as_ref().unwrap();
        w.tx.try_send(Request {
            clip,
            generation: w.generation.load(Ordering::Relaxed),
            created: Instant::now(),
        })
        .map_err(|_| anyhow::anyhow!("Sound queue is busy; trigger was not queued"))?;
        Ok(())
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, library: &mut Library) -> bool {
        let before = serde_json::to_string(library).unwrap_or_default();
        crate::theme::card(ui, |ui| {
            ui.heading("Sound reactions");
            ui.label("Add a local clip, then choose Sound in an event rule or action graph. WAV, MP3, OGG and FLAC; up to 10 seconds / 16 MiB. Uses the default playback device.");
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(library.clips.len() < 128, egui::Button::new("Add sound"))
                    .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("Audio", &["wav", "mp3", "ogg", "flac"])
                        .pick_file()
                    && let Err(e) = library.add(path)
                {
                    self.message = Some(e.to_string());
                }
                if ui.button("Stop sounds").clicked() {
                    self.stop();
                }
                if ui.button("Retry audio / reload clips").clicked() {
                    self.worker = None;
                    self.message = None;
                }
            });
            let mut play = None;
            let mut remove = None;
            for clip in library.clips.iter_mut().take(128) {
                ui.push_id(clip.id, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut clip.name)
                                .char_limit(80)
                                .desired_width(160.),
                        );
                        ui.add(egui::Slider::new(&mut clip.volume, 0.0..=1.).text("Volume"));
                        if ui.button("Preview").clicked() {
                            play = Some(clip.id);
                        }
                        if ui.small_button("Remove").clicked() {
                            remove = Some(clip.id);
                        }
                    });
                    ui.small(clip.path.to_string_lossy());
                    if !clip.path.is_file() {
                        ui.colored_label(
                            egui::Color32::YELLOW,
                            "File missing; choose a replacement.",
                        );
                    }
                    if ui.small_button("Replace file").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Audio", &["wav", "mp3", "ogg", "flac"])
                            .pick_file()
                    {
                        clip.path = path;
                        self.worker = None;
                    }
                });
            }
            if let Some(id) = remove {
                library.clips.retain(|c| c.id != id);
            }
            if let Some(id) = play
                && let Err(e) = self.play(library, id)
            {
                self.message = Some(e.to_string());
            }
            if let Some(w) = &self.worker
                && let Some(e) = w.error.lock().unwrap().take()
            {
                self.message = Some(e);
            }
            if let Some(e) = &self.message {
                ui.colored_label(egui::Color32::YELLOW, e);
            }
        });
        before != serde_json::to_string(library).unwrap_or_default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sound_ids_survive_removal_and_roundtrip_and_missing_targets_fail() {
        let mut library = Library::default();
        library.add("one.wav".into()).unwrap();
        library.clips.clear();
        let mut library: Library =
            serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        library.add("two.wav".into()).unwrap();
        assert_eq!(library.clips[0].id, 2);
        assert!(Sounds::default().play(&library, 1).is_err());
        library.clips[0].volume = f32::NAN;
        assert!(Sounds::default().play(&library, 2).is_err());
    }
}
