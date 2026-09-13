//! Personal tracking ranges. Calibration never changes a rig's authored outputs.
pub mod guided;
use crate::rig::{self, Inputs};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Head,
    Face,
    Gaze,
}

pub fn group(name: &str) -> Option<Group> {
    if name.starts_with("FaceAngle") || name.starts_with("ParamAngle") || name == "ParamBodyAngleX"
    {
        Some(Group::Head)
    } else if matches!(
        name,
        "EyeLeftX" | "EyeLeftY" | "EyeRightX" | "EyeRightY" | "ParamEyeBallX" | "ParamEyeBallY"
    ) || name.starts_with("ARKit:eyelook")
    {
        Some(Group::Gaze)
    } else if (rig::INPUT_NAMES.contains(&name) && !matches!(name, "Breath" | "AutoBlink"))
        || crate::PARAMETER_SPECS.iter().any(|s| s.0 == name)
        || rig::FACE_ALIASES.iter().any(|s| s.0 == name)
        || name
            .strip_prefix("ARKit:")
            .is_some_and(|s| !s.is_empty() && s.len() <= 100)
    {
        Some(Group::Face)
    } else {
        None
    }
}

pub fn rest(name: &str) -> f32 {
    if matches!(
        name,
        "EyeOpenLeft" | "EyeOpenRight" | "ParamEyeLOpen" | "ParamEyeROpen"
    ) {
        1.0
    } else {
        0.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub low: f32,
    pub neutral: f32,
    pub high: f32,
    pub enabled: bool,
}
impl Range {
    pub fn valid(&self, name: &str) -> bool {
        let (lo, hi) = rig::input_range(name);
        let center = rest(name);
        group(name).is_some()
            && [self.low, self.neutral, self.high]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 10000.0)
            && self.high - self.low > 1e-5
            && self.low <= self.neutral
            && self.neutral <= self.high
            && (center == lo || self.neutral - self.low > 1e-5)
            && (center == hi || self.high - self.neutral > 1e-5)
    }
    pub fn map(&self, name: &str, value: f32) -> f32 {
        let (lo, hi) = rig::input_range(name);
        let center = rest(name);
        if !value.is_finite() || !self.valid(name) {
            return center;
        }
        if value < self.neutral {
            center
                + (lo - center)
                    * ((self.neutral - value) / (self.neutral - self.low).max(1e-5)).clamp(0.0, 1.0)
        } else {
            center
                + (hi - center)
                    * ((value - self.neutral) / (self.high - self.neutral).max(1e-5))
                        .clamp(0.0, 1.0)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub enabled: bool,
    /// Retain the measurement origin when restoring an older movement preset.
    pub origin: crate::Vec3,
    pub ranges: BTreeMap<String, Range>,
}
impl Profile {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.origin.is_finite(),
            "Invalid tracking calibration origin"
        );
        ensure!(self.ranges.len() <= 128, "Too many calibrated inputs");
        for (name, range) in &self.ranges {
            ensure!(
                range.valid(name),
                "Invalid personal tracking range for {name}"
            );
        }
        Ok(())
    }
    pub fn measure(
        &self,
        frame: Option<&crate::TrackingFrame>,
        settings: &crate::MappingSettings,
        time: f32,
    ) -> Inputs {
        let mut pipeline = crate::ParameterPipeline::default();
        pipeline.restore_calibration(self.origin);
        rig::tracking_inputs(
            frame,
            pipeline.measure(frame, settings),
            settings.mirror,
            time,
        )
    }
}

/// Runtime-only smoothing. No frame history is serialized with a profile/preset.
#[derive(Default)]
pub struct Filter {
    values: Inputs,
}
impl Filter {
    pub fn reset(&mut self) {
        self.values.clear();
    }
    pub fn apply(
        &mut self,
        profile: &Profile,
        raw: &Inputs,
        output: &mut Inputs,
        face_found: bool,
        smoothing_ms: f32,
        dt: f32,
    ) {
        if !profile.enabled {
            self.reset();
            return;
        }
        let alpha = if smoothing_ms.is_finite() && smoothing_ms > 0.0 {
            1.0 - (-dt.clamp(0.0, 0.25) * 1000.0 / smoothing_ms.clamp(1.0, 500.0)).exp()
        } else {
            1.0
        };
        self.values
            .retain(|name, _| profile.ranges.get(name).is_some_and(|r| r.enabled));
        for (name, range) in profile.ranges.iter().filter(|(_, r)| r.enabled) {
            let target = if face_found {
                raw.get(name).map_or(rest(name), |v| range.map(name, *v))
            } else {
                rest(name)
            };
            let value = self.values.entry(name.clone()).or_insert(rest(name));
            *value += (target - *value) * alpha;
            output.insert(name.clone(), *value);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Neutral,
    Head,
    Face,
    Gaze,
}
impl Phase {
    pub fn accepts(self, name: &str) -> bool {
        group(name).is_some_and(|g| {
            self == Self::Neutral
                || matches!(
                    (self, g),
                    (Self::Head, Group::Head)
                        | (Self::Face, Group::Face)
                        | (Self::Gaze, Group::Gaze)
                )
        })
    }
    pub fn seconds(self) -> f64 {
        match self {
            Self::Neutral => 3.0,
            Self::Head => 8.0,
            Self::Face => 10.0,
            Self::Gaze => 6.0,
        }
    }
}

#[derive(Default)]
pub struct Capture {
    neutral: BTreeMap<String, Vec<f32>>,
    movement: BTreeMap<String, Vec<f32>>,
    names: Vec<String>,
    last_sequence: Option<u64>,
    last_time: Option<f64>,
    pub elapsed: f64,
    pub samples: usize,
}
impl Capture {
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        let mut names: Vec<_> = names.into_iter().filter(|n| group(n).is_some()).collect();
        names.sort();
        names.dedup();
        names.truncate(128);
        Self {
            names,
            ..Default::default()
        }
    }
    pub fn begin(&mut self, phase: Phase) {
        self.elapsed = 0.0;
        self.samples = 0;
        self.last_time = None;
        if phase == Phase::Neutral {
            self.neutral.clear();
            self.movement.clear();
        } else {
            self.movement.retain(|name, _| !phase.accepts(name));
        }
    }
    /// Count each accepted packet once. Lost faces and receive gaps never advance the timer.
    pub fn sample(&mut self, phase: Phase, sequence: u64, now: f64, valid: bool, inputs: &Inputs) {
        if !valid || !now.is_finite() {
            self.last_time = None;
            return;
        }
        if self.last_sequence == Some(sequence) {
            return;
        }
        self.last_sequence = Some(sequence);
        if let Some(last) = self.last_time {
            let delta = now - last;
            if (0.0..=0.25).contains(&delta) {
                self.elapsed += delta;
            }
        }
        self.last_time = Some(now);
        self.samples += 1;
        let target = if phase == Phase::Neutral {
            &mut self.neutral
        } else {
            &mut self.movement
        };
        for name in self.names.iter().filter(|n| phase.accepts(n)) {
            if let Some(&v) = inputs
                .get(name)
                .filter(|v| v.is_finite() && v.abs() <= 10000.0)
            {
                let values = target.entry(name.clone()).or_default();
                if values.len() < 1800 {
                    values.push(v);
                }
            }
        }
    }
    pub fn complete(&self, phase: Phase) -> bool {
        self.elapsed >= phase.seconds() && self.samples >= 30
    }
    pub fn proposals(&self) -> Vec<Proposal> {
        self.names
            .iter()
            .map(|name| {
                let failure = |message| Proposal {
                    input: name.clone(),
                    range: None,
                    message,
                };
                let Some(n) = self.neutral.get(name).filter(|v| v.len() >= 30) else {
                    return failure("No neutral sample from this tracker");
                };
                let Some(m) = self.movement.get(name).filter(|v| v.len() >= 30) else {
                    return failure("Not captured / skipped");
                };
                let center = percentile(n, 0.5);
                let noise = percentile(n, 0.95) - percentile(n, 0.05);
                let (lo, hi) = rig::input_range(name);
                let minimum = (hi - lo) * 0.04;
                let low = percentile(m, 0.02).min(center);
                let high = percentile(m, 0.98).max(center);
                if noise > (hi - lo) * 0.12 {
                    return failure("Neutral moved too much — retry neutral");
                }
                let threshold = minimum.max(noise * 3.0);
                if (rest(name) != lo && center - low < threshold)
                    || (rest(name) != hi && high - center < threshold)
                {
                    return failure(
                        "Too little movement / signal unavailable — retry or leave unchanged",
                    );
                }
                Proposal {
                    input: name.clone(),
                    range: Some(Range {
                        low,
                        neutral: center,
                        high,
                        enabled: true,
                    }),
                    message: "Ready — review on your avatar",
                }
            })
            .collect()
    }
}
pub struct Proposal {
    pub input: String,
    pub range: Option<Range>,
    pub message: &'static str,
}
fn percentile(values: &[f32], fraction: f32) -> f32 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f32::total_cmp);
    sorted[((sorted.len() - 1) as f32 * fraction).round() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asymmetric_motion_keeps_neutral_and_artist_inversion() {
        let range = Range {
            low: -18.0,
            neutral: 4.0,
            high: 46.0,
            enabled: true,
        };
        assert_eq!(range.map("FaceAngleX", 4.0), 0.0);
        assert_eq!(range.map("FaceAngleX", -18.0), -30.0);
        assert_eq!(range.map("FaceAngleX", 46.0), 30.0);
        let mut b = rig::Binding::direct("FaceAngleX", -30.0, 30.0);
        b.output_min = 12.0;
        b.output_max = -8.0;
        assert_eq!(b.evaluate(range.map("FaceAngleX", 46.0), 0.016), -8.0);
    }
    #[test]
    fn capture_sees_head_and_mouth_before_display_clipping() {
        let pipeline = crate::ParameterPipeline::default();
        let mut frame = crate::demo_frame(0.0);
        frame.rotation.y = 55.0;
        frame.blend_shapes.insert("jawopen".into(), 0.9);
        let settings = crate::MappingSettings {
            mouth_gain: 2.0,
            ..Default::default()
        };
        let measured = rig::tracking_inputs(
            Some(&frame),
            pipeline.measure(Some(&frame), &settings),
            false,
            0.0,
        );
        assert_eq!(measured["FaceAngleX"], 55.0);
        assert!((measured["MouthOpen"] - 1.8).abs() < 1e-6);
    }
    #[test]
    fn lost_face_returns_to_rest_instead_of_reapplying_learned_offset() {
        let p = Profile {
            enabled: true,
            origin: Default::default(),
            ranges: BTreeMap::from([(
                "MouthOpen".into(),
                Range {
                    low: 0.15,
                    neutral: 0.15,
                    high: 0.6,
                    enabled: true,
                },
            )]),
        };
        let mut f = Filter::default();
        let mut out = Inputs::new();
        f.apply(
            &p,
            &Inputs::from([("MouthOpen".into(), 0.6)]),
            &mut out,
            true,
            0.0,
            0.016,
        );
        assert_eq!(out["MouthOpen"], 1.0);
        f.apply(&p, &Inputs::new(), &mut out, false, 0.0, 0.016);
        assert_eq!(out["MouthOpen"], 0.0);
    }
    #[test]
    fn timer_ignores_duplicates_missing_faces_and_gaps() {
        let mut c = Capture::new(["FaceAngleX".into()]);
        c.begin(Phase::Neutral);
        let i = Inputs::from([("FaceAngleX".into(), 0.0)]);
        c.sample(Phase::Neutral, 1, 0.0, true, &i);
        c.sample(Phase::Neutral, 1, 5.0, true, &i);
        assert_eq!(c.samples, 1);
        assert_eq!(c.elapsed, 0.0);
        c.sample(Phase::Neutral, 2, 6.0, false, &i);
        c.sample(Phase::Neutral, 3, 7.0, true, &i);
        c.sample(Phase::Neutral, 4, 7.1, true, &i);
        assert!((c.elapsed - 0.1).abs() < 1e-6);
    }
    #[test]
    fn robust_capture_rejects_outliers_flat_signals_and_noisy_neutral() {
        let names = ["FaceAngleX", "FaceAngleY", "FaceAngleZ"];
        let mut c = Capture::new(names.map(String::from));
        c.begin(Phase::Neutral);
        for n in 0..200 {
            c.sample(
                Phase::Neutral,
                n,
                n as f64 / 60.0,
                true,
                &Inputs::from([
                    (names[0].into(), 4.0),
                    (names[1].into(), 0.0),
                    (names[2].into(), if n % 2 == 0 { -10.0 } else { 10.0 }),
                ]),
            );
        }
        assert!(c.complete(Phase::Neutral));
        c.begin(Phase::Head);
        for n in 200..800 {
            c.sample(
                Phase::Head,
                n,
                n as f64 / 60.0,
                true,
                &Inputs::from([
                    (
                        names[0].into(),
                        if n == 200 {
                            9000.0
                        } else if n % 2 == 0 {
                            -18.0
                        } else {
                            46.0
                        },
                    ),
                    (names[1].into(), 0.0),
                    (names[2].into(), if n % 2 == 0 { -30.0 } else { 30.0 }),
                ]),
            );
        }
        let p = c.proposals();
        assert_eq!(p[0].range.as_ref().unwrap().high, 46.0);
        assert!(p[1].range.is_none());
        assert!(p[2].range.is_none());
    }
    #[test]
    fn old_profiles_load_and_calibration_survives_presets() {
        let mut config: crate::movement::RigConfig = serde_json::from_str("{}").unwrap();
        assert!(!config.tracking.enabled);
        config.tracking = Profile {
            enabled: true,
            origin: crate::Vec3 {
                x: 2.0,
                y: 5.0,
                z: -3.0,
            },
            ranges: BTreeMap::from([(
                "EyeOpenLeft".into(),
                Range {
                    low: 0.15,
                    neutral: 0.8,
                    high: 0.8,
                    enabled: true,
                },
            )]),
        };
        let encoded = serde_json::to_string(&config).unwrap();
        let restored: crate::movement::RigConfig = serde_json::from_str(&encoded).unwrap();
        restored.validate(&[]).unwrap();
        assert_eq!(restored.tracking, config.tracking);
        let frame = crate::TrackingFrame {
            face_found: true,
            rotation: restored.tracking.origin,
            ..Default::default()
        };
        assert_eq!(
            restored
                .tracking
                .measure(Some(&frame), &Default::default(), 0.0)["FaceAngleX"],
            0.0
        );
        assert_eq!(
            restored.tracking.ranges["EyeOpenLeft"].map("EyeOpenLeft", 0.15),
            0.0
        );
        config
            .tracking
            .ranges
            .get_mut("EyeOpenLeft")
            .unwrap()
            .neutral = f32::NAN;
        assert!(config.validate(&[]).is_err());
    }
}
