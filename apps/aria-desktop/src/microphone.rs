//! Local input amplitude only. Samples are discarded in the audio callback.
use anyhow::{Context, Result, bail};
use aria_core::{
    microphone::{Envelope, MouthMode, Settings},
    rig::Inputs,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

#[derive(Clone)]
pub struct Device {
    pub id: String,
    pub name: String,
}
#[derive(Default)]
struct Meter {
    rms: AtomicU32,
    frames: AtomicU64,
    error: Mutex<Option<String>>,
}
#[derive(Default)]
pub struct Microphone {
    stream: Option<cpal::Stream>,
    meter: Arc<Meter>,
    attempted: Option<Option<String>>,
    pub devices: Vec<Device>,
    scanned: bool,
    pub error: Option<String>,
    pub label: String,
    envelope: Envelope,
    pub raw_db: f32,
    last_frames: u64,
    stale: f32,
}
impl Microphone {
    pub fn refresh(&mut self) {
        self.scanned = true;
        match cpal::default_host().input_devices() {
            Ok(devices) => {
                self.devices = devices
                    .filter_map(|d| {
                        Some(Device {
                            id: d.id().ok()?.to_string(),
                            name: d.description().ok()?.name().into(),
                        })
                    })
                    .collect()
            }
            Err(e) => self.error = Some(format!("Cannot list microphones: {e}")),
        }
    }
    pub fn stop(&mut self) {
        self.stream = None;
        self.attempted = None;
        self.envelope = Envelope::default();
        self.meter = Arc::default();
        self.stale = 0.0;
        self.last_frames = 0;
    }
    pub fn retry(&mut self) {
        self.stop();
        self.error = None;
    }
    fn start(&mut self, settings: &Settings) -> Result<()> {
        let host = cpal::default_host();
        let device = if let Some(id) = &settings.device {
            host.device_by_id(&id.parse().context("Saved microphone ID is invalid")?)
        } else {
            host.default_input_device()
        }
        .context(
            "Microphone unavailable. Choose a device, check Windows microphone access, then Retry.",
        )?;
        self.label = device.description()?.name().into();
        let supported = device.default_input_config()?;
        let format = supported.sample_format();
        let config = supported.config();
        let stream = match format {
            cpal::SampleFormat::F32 => capture::<f32>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::F64 => capture::<f64>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::I8 => capture::<i8>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::I16 => capture::<i16>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::I32 => capture::<i32>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::I64 => capture::<i64>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::U8 => capture::<u8>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::U16 => capture::<u16>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::U32 => capture::<u32>(&device, &config, self.meter.clone())?,
            cpal::SampleFormat::U64 => capture::<u64>(&device, &config, self.meter.clone())?,
            other => bail!("Unsupported microphone sample format: {other}"),
        };
        stream.play()?;
        self.stream = Some(stream);
        Ok(())
    }
    pub fn update(&mut self, s: &Settings, dt: f32) {
        if !s.enabled {
            if self.attempted.is_some() {
                self.stop();
            }
            return;
        }
        if self.attempted.as_ref() != Some(&s.device) {
            self.stop();
            self.attempted = Some(s.device.clone());
            self.error = self.start(s).err().map(|e| format!("{e:#}"));
        }
        if let Some(error) = self.meter.error.lock().unwrap().take() {
            self.error = Some(error);
            self.stream = None;
        }
        let frames = self.meter.frames.load(Ordering::Relaxed);
        if frames != self.last_frames {
            self.stale = 0.0;
            self.last_frames = frames;
        } else {
            self.stale += dt;
        }
        let rms = if self.stream.is_some() && self.stale < 0.3 {
            f32::from_bits(self.meter.rms.load(Ordering::Relaxed))
        } else {
            0.0
        };
        self.raw_db = 20.0 * rms.max(1e-8).log10();
        self.envelope.update(rms, dt, s);
    }
    pub fn inject(&self, s: &Settings, inputs: &mut Inputs) {
        let level = if s.enabled { self.envelope.level } else { 0.0 };
        let talking = s.enabled && self.envelope.talking;
        inputs.insert("MicLevel".into(), level);
        inputs.insert("MicTalking".into(), if talking { 1.0 } else { 0.0 });
        inputs.insert(
            "MicEnabled".into(),
            if s.enabled && self.stream.is_some() {
                1.0
            } else {
                0.0
            },
        );
        let original = inputs.get("MouthOpen").copied().unwrap_or(0.0);
        let mouth = if s.enabled {
            match s.mouth {
                MouthMode::Off => original,
                MouthMode::Replace => level,
                MouthMode::Combine => original.max(level),
            }
        } else {
            original
        };
        inputs.insert(
            "Talking".into(),
            if s.enabled {
                if talking { 1.0 } else { 0.0 }
            } else {
                mouth
            },
        );
        inputs.insert("MouthOpen".into(), mouth);
        inputs.insert("ParamMouthOpenY".into(), mouth);
    }
    pub fn panel(&mut self, ui: &mut eframe::egui::Ui, s: &mut Settings) -> bool {
        use crate::{help, theme};
        use eframe::egui;
        let before = s.clone();
        theme::category(ui, "microphone", "Microphone & talking", true, |ui| {
            help::control(ui, "microphone", |ui| {
                ui.checkbox(&mut s.enabled, "Enable microphone")
            });
            if !self.scanned {
                self.refresh();
            }
            help::control(ui, "microphone", |ui| {
                egui::ComboBox::from_id_salt("mic-device")
                    .selected_text(
                        s.device
                            .as_ref()
                            .and_then(|id| self.devices.iter().find(|d| &d.id == id))
                            .map_or("Windows default input", |d| d.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.device, None, "Windows default input");
                        for d in &self.devices {
                            ui.selectable_value(&mut s.device, Some(d.id.clone()), &d.name);
                        }
                    })
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Refresh devices").clicked() {
                    self.refresh();
                }
                if ui.button("Retry input").clicked() {
                    self.retry();
                }
            });
            ui.add(egui::ProgressBar::new(self.envelope.level).text(format!(
                "{:.0}% · {}",
                self.envelope.level * 100.0,
                if self.envelope.talking {
                    "TALKING"
                } else {
                    "QUIET"
                }
            )));
            ui.small(format!(
                "Input: {} · {:.1} dBFS",
                if self.stream.is_some() {
                    self.label.as_str()
                } else {
                    "stopped"
                },
                if self.stream.is_some() {
                    self.raw_db.max(-96.0)
                } else {
                    -96.0
                }
            ));
            if let Some(e) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            help::control(ui, "microphone", |ui| {
                egui::ComboBox::from_id_salt("mic-mouth")
                    .selected_text(format!("Mouth control: {:?}", s.mouth))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut s.mouth,
                            MouthMode::Replace,
                            "Replace tracking mouth",
                        );
                        ui.selectable_value(
                            &mut s.mouth,
                            MouthMode::Combine,
                            "Combine with tracking (maximum)",
                        );
                        ui.selectable_value(
                            &mut s.mouth,
                            MouthMode::Off,
                            "Image actions / inputs only",
                        );
                    })
            });
            for (label, value, range) in [
                ("Gain (dB)", &mut s.gain_db, -24.0..=36.0),
                ("Noise floor (dBFS)", &mut s.floor_db, -90.0..=-1.0),
                ("Full mouth (dBFS)", &mut s.ceiling_db, -89.0..=0.0),
                ("Attack (s)", &mut s.attack, 0.001..=2.0),
                ("Release (s)", &mut s.release, 0.001..=3.0),
                ("Talk starts at", &mut s.open, 0.01..=1.0),
                ("Talk ends below", &mut s.close, 0.0..=1.0),
                ("Quiet hold (s)", &mut s.hold, 0.0..=3.0),
            ] {
                help::control(ui, "microphone", |ui| {
                    ui.add(egui::Slider::new(value, range).text(label))
                });
            }
            s.ceiling_db = s.ceiling_db.max(s.floor_db + 1.0);
            s.close = s.close.min(s.open);
            if help::control(ui, "microphone", |ui| {
                ui.button("Set noise floor from current level")
            })
            .clicked()
            {
                s.floor_db = (self.raw_db + s.gain_db + 6.0).clamp(-90.0, s.ceiling_db - 1.0);
            }
            ui.small("Audio is measured locally and discarded. No recording or speech recognition. MicLevel and MicTalking can drive any input rule. Save profile stores these settings for this avatar.");
        });
        before != *s
    }
}
fn capture<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    meter: Arc<Meter>,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let errors = meter.clone();
    Ok(device.build_input_stream(
        config,
        move |samples: &[T], _: &cpal::InputCallbackInfo| {
            let energy = samples
                .iter()
                .map(|s| {
                    let v: f32 = cpal::Sample::to_sample(*s);
                    if v.is_finite() {
                        f64::from(v) * f64::from(v)
                    } else {
                        0.0
                    }
                })
                .sum::<f64>();
            let rms = (energy / samples.len().max(1) as f64).sqrt().min(1.0) as f32;
            meter.rms.store(rms.to_bits(), Ordering::Relaxed);
            meter.frames.fetch_add(1, Ordering::Relaxed);
        },
        move |e| {
            if let Ok(mut message) = errors.error.lock() {
                *message = Some(format!(
                    "Microphone stopped: {e}. Reconnect the device, then Retry input."
                ));
            }
        },
        None,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn microphone_routes_to_mouth_or_custom_inputs_without_overwriting_tracking() {
        let mut mic = Microphone::default();
        mic.envelope.level = 0.8;
        mic.envelope.talking = true;
        let mut settings = Settings {
            enabled: true,
            ..Default::default()
        };
        let mut inputs = Inputs::from([("MouthOpen".into(), 0.3)]);
        mic.inject(&settings, &mut inputs);
        assert_eq!(inputs["MouthOpen"], 0.8);
        assert_eq!(inputs["MicTalking"], 1.0);
        settings.mouth = MouthMode::Off;
        inputs.insert("MouthOpen".into(), 0.2);
        mic.inject(&settings, &mut inputs);
        assert_eq!(inputs["MouthOpen"], 0.2);
        settings.mouth = MouthMode::Combine;
        inputs.insert("MouthOpen".into(), 0.95);
        mic.inject(&settings, &mut inputs);
        assert_eq!(inputs["MouthOpen"], 0.95);
        settings.enabled = false;
        mic.inject(&settings, &mut inputs);
        assert_eq!(inputs["MicTalking"], 0.0);
        assert_eq!(inputs["Talking"], 0.95);
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "opens the default Windows microphone briefly; measures and discards samples"]
    fn native_microphone_callbacks_and_stop() {
        let mut mic = Microphone::default();
        mic.refresh();
        assert!(!mic.devices.is_empty(), "No Windows microphones found");
        let settings = Settings {
            enabled: true,
            ..Default::default()
        };
        mic.update(&settings, 0.016);
        assert!(mic.stream.is_some(), "Microphone failed: {:?}", mic.error);
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            mic.update(&settings, 0.05);
        }
        let callbacks = mic.meter.frames.load(Ordering::Relaxed);
        assert!(callbacks > 5, "No microphone callbacks");
        assert!(mic.envelope.level.is_finite());
        eprintln!(
            "Windows microphone: {} · {callbacks} callbacks · local amplitude only",
            mic.label
        );
        mic.update(&Settings::default(), 0.016);
        assert!(mic.stream.is_none());
        assert_eq!(mic.envelope.level, 0.0);
    }
}
