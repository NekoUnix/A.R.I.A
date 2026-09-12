//! Local replaceable sound clips. Missing audio never blocks visual effects.
use anyhow::{Result, ensure};
use rodio::{Source, buffer::SamplesBuffer};
use std::{
    collections::BTreeMap,
    num::NonZeroU32,
    path::{Path, PathBuf},
};
#[derive(Default)]
pub struct Audio {
    stream: Option<rodio::MixerDeviceSink>,
    voices: Vec<rodio::Player>,
    cache: BTreeMap<PathBuf, Result<SamplesBuffer, String>>,
    pub error: Option<String>,
    device_failed: bool,
    cached_bytes: usize,
}
impl Audio {
    pub fn clear(&mut self) {
        self.voices.clear();
        self.cache.clear();
        self.error = None;
        self.device_failed = false;
        self.cached_bytes = 0;
    }
    pub fn stop(&mut self) {
        self.voices.clear();
    }
    pub fn play(&mut self, path: &Path, gain: f32) {
        self.voices.retain(|v| !v.empty());
        if gain <= 0.0 || path.as_os_str().is_empty() || self.device_failed {
            return;
        }
        if self.stream.is_none() {
            match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(s) => self.stream = Some(s),
                Err(e) => {
                    self.device_failed = true;
                    self.error = Some(format!("Audio device unavailable: {e}"));
                    return;
                }
            }
        }
        if !self.cache.contains_key(path) {
            let loaded = load(path).map_err(|e| format!("{e:#}"));
            let bytes = loaded.as_ref().map_or(0, |s| s.clone().count() * 4);
            if self.cache.len() >= 64 || self.cached_bytes + bytes > 64 * 1024 * 1024 {
                self.cache.clear();
                self.cached_bytes = 0;
            }
            self.cached_bytes += bytes;
            self.cache.insert(path.into(), loaded);
        }
        match &self.cache[path] {
            Err(e) => self.error = Some(e.clone()),
            Ok(sound) => {
                if self.voices.len() >= 16 {
                    self.voices.remove(0).stop();
                }
                let voice = rodio::Player::connect_new(self.stream.as_ref().unwrap().mixer());
                voice.set_volume(gain.clamp(0.0, 1.0) / (self.voices.len() as f32 + 1.0).sqrt());
                voice.append(sound.clone());
                self.voices.push(voice);
            }
        }
    }
}
fn load(path: &Path) -> Result<SamplesBuffer> {
    let name = path.to_string_lossy();
    if let Some(kind) = name.strip_prefix("builtin:") {
        let duration = if kind == "spray" { 0.8 } else { 0.18 };
        let rate = 44100;
        let samples = (0..(duration * rate as f32) as usize)
            .map(|n| {
                let t = n as f32 / rate as f32;
                let fade = (1.0 - t / duration).max(0.0);
                let noise = ((n as u32)
                    .wrapping_mul(1664525)
                    .wrapping_add(1013904223)
                    .rotate_left(13) as f32
                    / u32::MAX as f32)
                    * 2.0
                    - 1.0;
                let wave = match kind {
                    "whoosh" | "spray" => noise * fade,
                    "splat" => noise * fade * fade,
                    _ => (std::f32::consts::TAU * (620.0 * t - 900.0 * t * t)).sin() * fade * fade,
                };
                wave * 0.2
            })
            .collect::<Vec<_>>();
        return Ok(SamplesBuffer::new(
            1.try_into().unwrap(),
            NonZeroU32::new(rate).unwrap(),
            samples,
        ));
    }
    let bytes = aria_model::read_bounded(path, 16 * 1024 * 1024)?;
    let decoder = rodio::Decoder::try_from(std::io::Cursor::new(bytes))?;
    let channels = decoder.channels();
    let rate = decoder.sample_rate();
    ensure!(
        channels.get() <= 2 && rate.get() <= 192000,
        "Use mono or stereo audio at up to 192 kHz"
    );
    let limit = rate.get() as usize * channels.get() as usize * 10;
    let mut samples: Vec<f32> = decoder.take(limit + 1).collect();
    ensure!(
        samples.len() <= limit && !samples.is_empty(),
        "Sound effects must be at most 10 seconds long"
    );
    ensure!(
        samples.iter().all(|v| v.is_finite()),
        "Invalid sound samples"
    );
    let peak = samples.iter().fold(0.001_f32, |a, b| a.max(b.abs()));
    for s in &mut samples {
        *s = *s / peak * 0.5;
    }
    Ok(SamplesBuffer::new(channels, rate, samples))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wav_template_decodes_and_missing_clips_fail_without_panicking() {
        let file =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/effects/assets/pop.wav");
        let clip = load(&file).unwrap();
        assert_eq!(clip.channels().get(), 1);
        assert_eq!(clip.sample_rate().get(), 44100);
        assert!(clip.total_duration().unwrap().as_secs_f32() < 0.2);
        assert!(load(&file.with_extension("missing")).is_err());
    }
    #[test]
    fn built_in_clips_have_finite_bounded_audio() {
        for s in ["whoosh", "pop", "spray", "splat"] {
            let b = load(Path::new(&format!("builtin:{s}"))).unwrap();
            let v: Vec<_> = b.collect();
            assert!(v.len() > 1000);
            assert!(v.iter().all(|v| v.is_finite() && v.abs() <= 0.21));
        }
    }
}
