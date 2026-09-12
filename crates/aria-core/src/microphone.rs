//! Audio amplitude envelope and talking gate; no recordings or speech recognition.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouthMode {
    Off,
    #[default]
    Replace,
    Combine,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub device: Option<String>,
    pub mouth: MouthMode,
    pub gain_db: f32,
    pub floor_db: f32,
    pub ceiling_db: f32,
    pub attack: f32,
    pub release: f32,
    pub open: f32,
    pub close: f32,
    pub hold: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            device: None,
            mouth: MouthMode::Replace,
            gain_db: 0.0,
            floor_db: -48.0,
            ceiling_db: -12.0,
            attack: 0.035,
            release: 0.16,
            open: 0.2,
            close: 0.1,
            hold: 0.12,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        for (v, min, max) in [
            (self.gain_db, -24.0, 36.0),
            (self.floor_db, -90.0, -1.0),
            (self.ceiling_db, self.floor_db + 1.0, 0.0),
            (self.attack, 0.001, 2.0),
            (self.release, 0.001, 3.0),
            (self.open, 0.01, 1.0),
            (self.close, 0.0, self.open),
            (self.hold, 0.0, 3.0),
        ] {
            ensure!(
                v.is_finite() && (min..=max).contains(&v),
                "Invalid microphone sensitivity or gate settings"
            );
        }
        ensure!(
            self.device.as_ref().is_none_or(|d| d.len() <= 4096),
            "Invalid microphone device ID"
        );
        Ok(())
    }
}
#[derive(Default)]
pub struct Envelope {
    pub level: f32,
    pub talking: bool,
    quiet: f32,
}
impl Envelope {
    pub fn update(&mut self, rms: f32, dt: f32, s: &Settings) -> f32 {
        let dt = dt.clamp(0.0, 0.25);
        let db = 20.0 * rms.clamp(1e-8, 1.0).log10() + s.gain_db;
        let target = if rms.is_finite() {
            ((db - s.floor_db) / (s.ceiling_db - s.floor_db)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let tau = if target > self.level {
            s.attack
        } else {
            s.release
        };
        self.level += (target - self.level) * (1.0 - (-dt / tau).exp());
        if self.level >= s.open {
            self.talking = true;
            self.quiet = 0.0;
        } else if self.level <= s.close {
            self.quiet += dt;
            if self.quiet >= s.hold {
                self.talking = false;
            }
        } else {
            self.quiet = 0.0;
        }
        self.level
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_hysteresis_hold_smoothing_and_invalid_samples() {
        let s = Settings::default();
        let mut e = Envelope::default();
        for _ in 0..20 {
            e.update(0.08, 0.016, &s);
        }
        assert!(e.talking && e.level > 0.5);
        e.update(0.0, 0.01, &s);
        assert!(e.talking);
        for _ in 0..200 {
            e.update(f32::NAN, 0.016, &s);
        }
        assert!(!e.talking && e.level < 0.001);
        let mut bad = s;
        bad.ceiling_db = bad.floor_db;
        assert!(bad.validate().is_err());
    }
}
