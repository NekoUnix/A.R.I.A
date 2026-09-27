//! Streaming local music, independent of avatar profiles and short effect clips.
use cpal::traits::{DeviceTrait, HostTrait};
use eframe::egui;
use rodio::Source;
use serde::{Deserialize, Serialize};
use std::{fs::File, path::PathBuf, sync::mpsc, time::Duration};

const MAX_TRACKS: usize = 256;
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Off,
    Playlist,
    Track,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub tracks: Vec<PathBuf>,
}
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub device: Option<String>,
    pub playlists: Vec<Playlist>,
    pub selected: usize,
    pub volume: f32,
    pub repeat: Repeat,
    pub shuffle: bool,
    pub duck: bool,
    pub duck_volume: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            device: None,
            playlists: vec![Playlist {
                name: "My music".into(),
                tracks: vec![],
            }],
            selected: 0,
            volume: 0.35,
            repeat: Repeat::Playlist,
            shuffle: false,
            duck: true,
            duck_volume: 0.25,
        }
    }
}
impl Settings {
    pub fn sanitize(&mut self) {
        self.playlists.truncate(32);
        if self.playlists.is_empty() {
            self.playlists = Self::default().playlists;
        }
        self.selected = self.selected.min(self.playlists.len() - 1);
        for list in &mut self.playlists {
            list.name = list.name.chars().take(80).collect();
            list.tracks.truncate(MAX_TRACKS);
        }
        self.volume = finite_gain(self.volume, 0.35);
        self.duck_volume = finite_gain(self.duck_volume, 0.25);
    }
}
fn finite_gain(value: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        default
    }
}
fn output_builder(id: Option<&str>) -> anyhow::Result<rodio::DeviceSinkBuilder> {
    let host = cpal::default_host();
    let device = match id {
        Some(id) => host.device_by_id(&id.parse()?),
        None => host.default_output_device(),
    }
    .ok_or_else(|| {
        anyhow::anyhow!(
            "Selected audio output is unavailable. Reconnect it or select another output."
        )
    })?;
    Ok(rodio::DeviceSinkBuilder::from_device(device)?)
}
type Decoder = rodio::Decoder<std::io::BufReader<File>>;
type Prepared = anyhow::Result<(Decoder, Option<Duration>)>;
struct Pending {
    generation: u64,
    result: mpsc::Receiver<Prepared>,
}
pub struct Music {
    devices: Vec<(String, String)>,
    scanned_devices: bool,
    stream_device: Option<String>,
    device_error: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    stream: Option<rodio::MixerDeviceSink>,
    voice: Option<rodio::Player>,
    pending: Option<Pending>,
    generation: u64,
    /// Playback owns a queue snapshot; editing a playlist never reindexes a playing track.
    queue: Vec<PathBuf>,
    played: std::collections::BTreeSet<usize>,
    current: usize,
    duration: Option<Duration>,
    seek_draft: Option<f32>,
    gain: f32,
    duck_hold: f32,
    random: u64,
    pub error: Option<String>,
}
impl Default for Music {
    fn default() -> Self {
        Self {
            devices: vec![],
            scanned_devices: false,
            stream_device: None,
            device_error: Default::default(),
            stream: None,
            voice: None,
            pending: None,
            generation: 0,
            queue: vec![],
            played: Default::default(),
            current: 0,
            duration: None,
            seek_draft: None,
            gain: 1.,
            duck_hold: 0.,
            random: 0x9267_acbf_1873,
            error: None,
        }
    }
}
fn prepare(path: PathBuf) -> Prepared {
    let file = File::open(path)?;
    anyhow::ensure!(file.metadata()?.is_file(), "Select a local audio file");
    let decoder = rodio::Decoder::try_from(file)?;
    anyhow::ensure!(
        decoder.channels().get() <= 2 && decoder.sample_rate().get() <= 192_000,
        "Use mono or stereo music at up to 192 kHz"
    );
    let duration = decoder.total_duration();
    Ok((decoder, duration))
}
impl Music {
    fn refresh_devices(&mut self) {
        self.scanned_devices = true;
        match cpal::default_host().output_devices() {
            Ok(devices) => {
                self.devices = devices
                    .filter_map(|device| {
                        Some((
                            device.id().ok()?.to_string(),
                            device.description().ok()?.name().to_owned(),
                        ))
                    })
                    .collect()
            }
            Err(error) => self.error = Some(format!("Cannot list audio outputs: {error}")),
        }
    }
    pub fn stop(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(voice) = self.voice.take() {
            voice.stop();
        }
        // Keep a pending worker until it finishes: repeated clicks cannot spawn unbounded workers.
        self.queue.clear();
        self.played.clear();
        self.duration = None;
        self.seek_draft = None;
    }
    fn load(&mut self, index: usize) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.pending.is_none(),
            "A music file is still opening; wait before trying another"
        );
        let path = self
            .queue
            .get(index)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Choose a music track"))?;
        if let Some(voice) = self.voice.take() {
            voice.stop();
        }
        self.current = index;
        self.seek_draft = None;
        self.played.insert(index);
        self.duration = None;
        self.error = None;
        self.generation = self.generation.wrapping_add(1);
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("aria-music-open".into())
            .spawn(move || {
                let _ = tx.send(prepare(path));
            })?;
        self.pending = Some(Pending {
            generation: self.generation,
            result: rx,
        });
        Ok(())
    }
    pub fn play(&mut self, settings: &Settings, index: usize) -> anyhow::Result<()> {
        anyhow::ensure!(self.pending.is_none(), "A music file is still opening");
        self.queue = settings
            .playlists
            .get(settings.selected)
            .map(|p| p.tracks.clone())
            .unwrap_or_default();
        self.played.clear();
        self.load(index)
    }
    pub fn toggle(&mut self, settings: &Settings) -> anyhow::Result<()> {
        if let Some(voice) = &self.voice {
            if voice.is_paused() {
                voice.play();
            } else {
                voice.pause();
            }
            Ok(())
        } else {
            self.play(settings, 0)
        }
    }
    pub fn next(&mut self, settings: &Settings) -> anyhow::Result<()> {
        if let Some(index) = next_index(
            self.current,
            self.queue.len(),
            settings.repeat,
            settings.shuffle,
            &mut self.random,
            true,
        ) {
            self.load(index)
        } else {
            self.stop();
            Ok(())
        }
    }
    fn next_automatic(&mut self, settings: &Settings) -> Option<usize> {
        if settings.shuffle && settings.repeat == Repeat::Off {
            let remaining: Vec<_> = (0..self.queue.len())
                .filter(|i| !self.played.contains(i))
                .collect();
            if remaining.is_empty() {
                return None;
            }
            self.random = self
                .random
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1);
            return Some(remaining[(self.random >> 32) as usize % remaining.len()]);
        }
        next_index(
            self.current,
            self.queue.len(),
            settings.repeat,
            settings.shuffle,
            &mut self.random,
            false,
        )
    }
    pub fn update(&mut self, settings: &Settings, speaking: bool, dt: f32) {
        if self.stream.is_some() && self.stream_device != settings.device {
            self.stop();
            self.stream = None;
        }
        let device_error = self
            .device_error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take();
        if let Some(error) = device_error {
            self.stop();
            self.stream = None;
            self.error = Some(format!(
                "Audio device stopped: {error}. Reconnect the device, then press Play."
            ));
        }
        if let Some(job) = &self.pending {
            let result = match job.result.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err(anyhow::anyhow!("Music decoder stopped unexpectedly")))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                let valid = job.generation == self.generation;
                self.pending = None;
                if valid {
                    let result = result.and_then(|(decoder, duration)| {
                        if self.stream.is_none() {
                            let errors = self.device_error.clone();
                            self.stream = Some(
                                output_builder(settings.device.as_deref())?
                                    .with_error_callback(move |error| {
                                        *errors.lock().unwrap_or_else(|p| p.into_inner()) =
                                            Some(error.to_string());
                                    })
                                    .open_stream()?,
                            );
                            self.stream_device = settings.device.clone();
                        }
                        let voice =
                            rodio::Player::connect_new(self.stream.as_ref().unwrap().mixer());
                        voice.set_volume(settings.volume * self.gain);
                        voice.append(decoder);
                        self.duration = duration;
                        self.voice = Some(voice);
                        Ok(())
                    });
                    if let Err(error) = result {
                        self.error = Some(format!(
                            "Music stopped: {error:#}. Repair the file or choose another track."
                        ));
                    }
                }
            }
        }
        let dt = if dt.is_finite() { dt.clamp(0., 1.) } else { 0. };
        if speaking {
            self.duck_hold = 0.35;
        } else {
            self.duck_hold = (self.duck_hold - dt).max(0.);
        }
        let target = if settings.duck && self.duck_hold > 0. {
            settings.duck_volume
        } else {
            1.
        };
        self.gain = duck_gain(self.gain, target, dt);
        if let Some(voice) = &self.voice {
            voice.set_volume(settings.volume * self.gain);
        }
        if self.voice.as_ref().is_some_and(|v| v.empty()) {
            self.voice = None;
            if let Some(index) = self.next_automatic(settings) {
                if let Err(error) = self.load(index) {
                    self.error = Some(error.to_string());
                }
            } else {
                self.stop();
            }
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &mut Settings) -> bool {
        settings.sanitize();
        let before = serde_json::to_string(settings).unwrap_or_default();
        ui.heading("Music & playlists");
        crate::theme::caption(
            ui,
            "Route local music to speakers, headphones or an installed virtual audio cable. Playback starts only when you press Play.",
        );
        crate::theme::card(ui, |ui| {
            if !self.scanned_devices {
                self.refresh_devices();
            }
            let old_device = settings.device.clone();
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("music-output-device")
                    .selected_text(settings.device.as_ref().map_or("System default", |id| {
                        self.devices
                            .iter()
                            .find(|(key, _)| key == id)
                            .map_or("Unavailable saved output", |(_, name)| name.as_str())
                    }))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.device, None, "System default");
                        for (id, name) in &self.devices {
                            ui.selectable_value(&mut settings.device, Some(id.clone()), name);
                        }
                    });
                if ui.button("Refresh outputs").clicked() {
                    self.refresh_devices();
                }
            });
            if old_device != settings.device {
                self.stop();
                self.stream = None;
                self.error = None;
            }
            crate::theme::caption(
                ui,
                "Changing output stops playback. A missing selected device never falls back to speakers. Capture the selected device separately in OBS; Spout carries no audio.",
            );
            if let Some(error) = &self.error {
                ui.colored_label(crate::theme::orange(), error);
            }
            if self.pending.is_some() {
                ui.spinner();
                ui.label("Opening music…");
            }
            if let Some(path) = self.queue.get(self.current) {
                ui.label(format!("Now playing: {}", file_label(path)));
            }
            ui.horizontal_wrapped(|ui| {
                let paused = self.voice.as_ref().is_none_or(|v| v.is_paused());
                if ui
                    .add_enabled(
                        self.pending.is_none()
                            && (self.voice.is_some()
                                || !settings.playlists[settings.selected].tracks.is_empty()),
                        egui::Button::new(if paused { "Play" } else { "Pause" }),
                    )
                    .clicked()
                    && let Err(e) = self.toggle(settings)
                {
                    self.error = Some(e.to_string());
                }
                if ui.button("Stop").clicked() {
                    self.stop();
                }
                if ui
                    .add_enabled(
                        !self.queue.is_empty() && self.pending.is_none(),
                        egui::Button::new("Next"),
                    )
                    .clicked()
                    && let Err(e) = self.next(settings)
                {
                    self.error = Some(e.to_string());
                }
            });
            if let Some(voice) = &self.voice {
                let mut position = self
                    .seek_draft
                    .unwrap_or_else(|| voice.get_pos().as_secs_f32());
                if let Some(duration) = self.duration {
                    let response = ui.add(
                        egui::Slider::new(&mut position, 0.0..=duration.as_secs_f32().max(0.01))
                            .text("Seconds"),
                    );
                    if response.dragged() {
                        self.seek_draft = Some(position);
                    }
                    if response.drag_stopped() || (response.changed() && !response.dragged()) {
                        let position = self.seek_draft.take().unwrap_or(position);
                        if let Err(e) = voice.try_seek(Duration::from_secs_f32(position)) {
                            self.error = Some(format!("This file cannot seek: {e}"));
                        }
                    }
                }
            }
            ui.add(egui::Slider::new(&mut settings.volume, 0.0..=1.0).text("Music volume"));
            ui.checkbox(&mut settings.duck, "Lower music while speaking");
            if settings.duck {
                ui.add(
                    egui::Slider::new(&mut settings.duck_volume, 0.0..=1.0).text("Speaking level"),
                );
                crate::theme::caption(
                    ui,
                    "Uses active microphone or face tracking; does not turn on a microphone.",
                );
            }
            ui.add(egui::ProgressBar::new(settings.volume * self.gain).text("Applied gain"));
            ui.checkbox(&mut settings.shuffle, "Shuffle without immediate repeats");
            ui.horizontal_wrapped(|ui| {
                ui.label("Repeat");
                for (value, label) in [
                    (Repeat::Off, "Off"),
                    (Repeat::Playlist, "Playlist"),
                    (Repeat::Track, "Track"),
                ] {
                    ui.selectable_value(&mut settings.repeat, value, label);
                }
            });
        });
        ui.add_space(8.);
        egui::ComboBox::from_id_salt("music-playlist")
            .selected_text(&settings.playlists[settings.selected].name)
            .show_ui(ui, |ui| {
                for (i, list) in settings.playlists.iter().enumerate() {
                    ui.selectable_value(&mut settings.selected, i, &list.name);
                }
            });
        if ui
            .add_enabled(
                settings.playlists.len() < 32,
                egui::Button::new("New playlist"),
            )
            .clicked()
        {
            settings.playlists.push(Playlist {
                name: format!("Playlist {}", settings.playlists.len() + 1),
                tracks: vec![],
            });
            settings.selected = settings.playlists.len() - 1;
        }
        let list = &mut settings.playlists[settings.selected];
        ui.add(egui::TextEdit::singleline(&mut list.name).char_limit(80));
        if ui.button("Add music files…").clicked()
            && let Some(paths) = rfd::FileDialog::new()
                .add_filter("Music", &["wav", "mp3", "ogg", "flac"])
                .pick_files()
        {
            for path in paths {
                if list.tracks.len() < MAX_TRACKS && !list.tracks.contains(&path) {
                    list.tracks.push(path);
                }
            }
        }
        let mut play = None;
        let mut remove = None;
        let mut swap = None;
        for (i, path) in list.tracks.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(self.pending.is_none(), egui::Button::new("▶"))
                        .clicked()
                    {
                        play = Some(i);
                    }
                    ui.label(file_label(path))
                        .on_hover_text(path.display().to_string());
                    if ui.small_button("Repair…").clicked()
                        && let Some(replacement) = rfd::FileDialog::new()
                            .add_filter("Music", &["wav", "mp3", "ogg", "flac"])
                            .pick_file()
                    {
                        *path = replacement;
                    }
                    if i > 0 && ui.small_button("↑").clicked() {
                        swap = Some((i, i - 1));
                    }
                    if ui.small_button("Remove").clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if let Some(i) = remove {
            list.tracks.remove(i);
        } else if let Some((a, b)) = swap {
            list.tracks.swap(a, b);
        } else if let Some(i) = play
            && let Err(e) = self.play(settings, i)
        {
            self.error = Some(e.to_string());
        }
        before != serde_json::to_string(settings).unwrap_or_default()
    }
}
fn file_label(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn duck_gain(current: f32, target: f32, dt: f32) -> f32 {
    let seconds = if target < current { 0.08 } else { 0.6 };
    target + (current - target) * (-dt / seconds).exp()
}
fn next_index(
    current: usize,
    len: usize,
    repeat: Repeat,
    shuffle: bool,
    random: &mut u64,
    manual: bool,
) -> Option<usize> {
    if len == 0 {
        return None;
    }
    if repeat == Repeat::Track && !manual {
        return Some(current.min(len - 1));
    }
    if shuffle && len > 1 {
        *random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let offset = ((*random >> 32) as usize % (len - 1)) + 1;
        return Some((current + offset) % len);
    }
    if current + 1 < len {
        Some(current + 1)
    } else if repeat != Repeat::Off || manual {
        Some(0)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shuffle_without_repeat_exhausts_queue_and_stop_invalidates_preparation() {
        let mut music = Music {
            queue: vec!["a".into(), "b".into(), "c".into()],
            ..Default::default()
        };
        let settings = Settings {
            shuffle: true,
            repeat: Repeat::Off,
            ..Default::default()
        };
        music.played.insert(0);
        let second = music.next_automatic(&settings).unwrap();
        assert_ne!(second, 0);
        music.played.insert(second);
        let third = music.next_automatic(&settings).unwrap();
        assert_ne!(third, second);
        music.played.insert(third);
        assert!(music.next_automatic(&settings).is_none());
        let (tx, rx) = mpsc::sync_channel(1);
        music.pending = Some(Pending {
            generation: music.generation,
            result: rx,
        });
        music.stop();
        tx.send(Err(anyhow::anyhow!("stale file error"))).unwrap();
        music.update(&settings, false, 0.1);
        assert!(music.pending.is_none() && music.voice.is_none() && music.error.is_none());
    }
    #[test]
    #[ignore = "requires a local audio output device; plays a silent generated WAV"]
    fn native_music_device_pause_seek_stop_and_cancel() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("silence.wav");
        let rate = 44100_u32;
        let bytes = rate * 2 * 4;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + bytes).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16_u32.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(1_u16.to_le_bytes());
        wav.extend(rate.to_le_bytes());
        wav.extend((rate * 2).to_le_bytes());
        wav.extend(2_u16.to_le_bytes());
        wav.extend(16_u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(bytes.to_le_bytes());
        wav.resize(wav.len() + bytes as usize, 0);
        std::fs::write(&path, wav).unwrap();
        // Exercise a persisted explicit output ID, not only the default-device path.
        let mut settings = Settings {
            device: Some(
                cpal::default_host()
                    .default_output_device()
                    .unwrap()
                    .id()
                    .unwrap()
                    .to_string(),
            ),
            ..Default::default()
        };
        settings.playlists[0].tracks.push(path);
        let mut music = Music::default();
        music.play(&settings, 0).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while music.pending.is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            music.update(&settings, false, 0.01);
        }
        assert!(music.error.is_none(), "{:?}", music.error);
        assert!(music.voice.is_some());
        music.toggle(&settings).unwrap();
        assert!(music.voice.as_ref().unwrap().is_paused());
        music
            .voice
            .as_ref()
            .unwrap()
            .try_seek(Duration::from_secs(1))
            .unwrap();
        music.toggle(&settings).unwrap();
        assert!(!music.voice.as_ref().unwrap().is_paused());
        settings.device = None;
        music.update(&settings, false, 0.01);
        assert!(
            music.voice.is_none() && music.stream.is_none(),
            "Changing output must release the old endpoint"
        );
        music.stop();
        assert!(music.voice.is_none() && music.queue.is_empty());
        music.play(&settings, 0).unwrap();
        music.stop();
        while music.pending.is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            music.update(&settings, false, 0.01);
        }
        assert!(music.voice.is_none());
    }
    #[test]
    fn repeat_and_shuffle_never_index_outside_queue() {
        let mut random = 1;
        assert_eq!(
            next_index(0, 0, Repeat::Playlist, true, &mut random, false),
            None
        );
        assert_eq!(
            next_index(2, 3, Repeat::Off, false, &mut random, false),
            None
        );
        assert_eq!(
            next_index(2, 3, Repeat::Playlist, false, &mut random, false),
            Some(0)
        );
        assert_eq!(
            next_index(1, 3, Repeat::Track, true, &mut random, false),
            Some(1)
        );
        for _ in 0..1000 {
            let next = next_index(1, 3, Repeat::Playlist, true, &mut random, false).unwrap();
            assert!(next < 3 && next != 1);
        }
    }
    #[test]
    fn duck_envelope_is_time_based_and_settings_are_bounded() {
        for fps in [15, 30, 120] {
            let mut gain = 1.;
            for _ in 0..fps {
                gain = duck_gain(gain, 0.2, 1. / fps as f32);
            }
            assert!((gain - duck_gain(1., 0.2, 1.)).abs() < 1e-5);
        }
        let mut settings = Settings {
            volume: f32::NAN,
            selected: usize::MAX,
            playlists: vec![],
            ..Default::default()
        };
        settings.sanitize();
        assert_eq!(settings.selected, 0);
        assert!(settings.volume.is_finite());
    }
    #[test]
    fn music_streams_a_real_clip_and_reports_missing_files() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../templates/effects/assets/pop.wav");
        let (decoder, duration) = prepare(path.clone()).unwrap();
        assert!(duration.is_some());
        assert!(decoder.take(100).all(f32::is_finite));
        assert!(prepare(path.with_extension("missing")).is_err());
    }
    #[test]
    fn old_music_settings_keep_default_output_and_invalid_ids_do_not_fallback() {
        let settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.device.is_none());
        assert!(output_builder(Some("not-a-device-id")).is_err());
    }
}
