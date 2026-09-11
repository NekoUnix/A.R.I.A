//! Platform-independent tracking values and a small, deterministic parameter pipeline.
pub mod expressions;
pub mod movement;
pub mod physics;
pub mod rig;
pub mod shortcuts;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TrackingFrame {
    /// Sender's timestamp, in Unix milliseconds. Never used as local packet age.
    #[serde(default)]
    pub timestamp: u64,
    pub face_found: bool,
    /// Canonical degrees: X = pitch (up/down), Y = yaw (left/right), Z = roll.
    pub rotation: Vec3,
    #[serde(default)]
    pub position: Vec3,
    #[serde(default)]
    pub eye_left: Vec3,
    #[serde(default)]
    pub eye_right: Vec3,
    pub blend_shapes: BTreeMap<String, f32>,
    #[serde(default = "no_hotkey")]
    pub hotkey: i32,
}

fn no_hotkey() -> i32 {
    -1
}

impl TrackingFrame {
    /// Normalize case once at the input boundary. Keep unknown blendshapes for diagnostics.
    pub fn normalize(&mut self) {
        self.blend_shapes = std::mem::take(&mut self.blend_shapes)
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v.clamp(0.0, 1.0)))
            .collect();
    }

    pub fn is_finite(&self) -> bool {
        [self.rotation, self.position, self.eye_left, self.eye_right]
            .into_iter()
            .all(Vec3::is_finite)
            && self.blend_shapes.values().all(|v| v.is_finite())
    }

    pub fn blend(&self, name: &str) -> f32 {
        self.blend_shapes.get(name).copied().unwrap_or(0.0)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct MappingSettings {
    pub smoothing_ms: f32,
    pub head_gain: f32,
    pub mouth_gain: f32,
    pub mirror: bool,
    pub invert_pitch: bool,
    pub invert_yaw: bool,
    pub invert_roll: bool,
}

impl Default for MappingSettings {
    fn default() -> Self {
        Self {
            smoothing_ms: 75.0,
            head_gain: 1.0,
            mouth_gain: 1.3,
            mirror: false,
            invert_pitch: false,
            invert_yaw: false,
            invert_roll: false,
        }
    }
}

/// Order is stable for inspection/export and future model backends.
pub const PARAMETER_SPECS: [(&str, f32, f32); 12] = [
    ("ParamAngleX", -30.0, 30.0),
    ("ParamAngleY", -30.0, 30.0),
    ("ParamAngleZ", -30.0, 30.0),
    ("ParamEyeLOpen", 0.0, 1.0),
    ("ParamEyeROpen", 0.0, 1.0),
    ("ParamMouthOpenY", 0.0, 1.0),
    ("ParamMouthForm", -1.0, 1.0),
    ("ParamBrowLY", -1.0, 1.0),
    ("ParamBrowRY", -1.0, 1.0),
    ("ParamEyeBallX", -1.0, 1.0),
    ("ParamEyeBallY", -1.0, 1.0),
    ("ParamBodyAngleX", -10.0, 10.0),
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Parameters(pub [f32; 12]);

impl Default for Parameters {
    fn default() -> Self {
        Self([0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
    }
}

impl Parameters {
    pub fn named(&self) -> BTreeMap<&'static str, f32> {
        PARAMETER_SPECS
            .iter()
            .zip(self.0)
            .map(|((id, _, _), v)| (*id, v))
            .collect()
    }
}

#[derive(Default)]
pub struct ParameterPipeline {
    neutral: Vec3,
    pub current: Parameters,
}

/// Unity-style Euler values can cross 360/0; subtract on the circle, not the number line.
pub fn signed_angle(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

impl ParameterPipeline {
    pub fn calibration(&self) -> Vec3 {
        self.neutral
    }
    pub fn restore_calibration(&mut self, neutral: Vec3) {
        self.neutral = if neutral.is_finite() {
            neutral
        } else {
            Vec3::default()
        };
        self.current = Parameters::default();
    }
    pub fn calibrate(&mut self, frame: &TrackingFrame) -> bool {
        if !frame.face_found || !frame.is_finite() {
            return false;
        }
        self.neutral = frame.rotation;
        true
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Missing/stale/face-lost input returns gently to neutral instead of freezing a face.
    pub fn update(
        &mut self,
        frame: Option<&TrackingFrame>,
        settings: &MappingSettings,
        dt: f32,
    ) -> Parameters {
        let mut target = Parameters::default();
        if let Some(f) = frame.filter(|f| f.face_found && f.is_finite()) {
            let b = |name| f.blend(name);
            let sign = |inverted| if inverted { -1.0 } else { 1.0 };
            let mirror = sign(settings.mirror);
            target.0 = [
                signed_angle(f.rotation.y - self.neutral.y)
                    * settings.head_gain
                    * sign(settings.invert_yaw)
                    * mirror,
                signed_angle(f.rotation.x - self.neutral.x)
                    * settings.head_gain
                    * sign(settings.invert_pitch),
                signed_angle(f.rotation.z - self.neutral.z)
                    * settings.head_gain
                    * sign(settings.invert_roll)
                    * mirror,
                1.0 - b("eyeblinkleft"),
                1.0 - b("eyeblinkright"),
                (b("jawopen") - b("mouthclose") * 0.5).max(0.0) * settings.mouth_gain,
                (b("mouthsmileleft") + b("mouthsmileright")
                    - b("mouthfrownleft")
                    - b("mouthfrownright"))
                    * 0.5,
                b("browinnerup") + b("browouterupleft") - b("browdownleft"),
                b("browinnerup") + b("browouterupright") - b("browdownright"),
                (b("eyelookinleft") - b("eyelookoutleft") + b("eyelookoutright")
                    - b("eyelookinright"))
                    * 0.5
                    * mirror,
                (b("eyelookupleft") + b("eyelookupright")
                    - b("eyelookdownleft")
                    - b("eyelookdownright"))
                    * 0.5,
                signed_angle(f.rotation.y - self.neutral.y)
                    * settings.head_gain
                    * 0.2
                    * mirror
                    * sign(settings.invert_yaw),
            ];
            if settings.mirror {
                target.0.swap(3, 4);
                target.0.swap(7, 8);
            }
        }
        let tau = if settings.smoothing_ms.is_finite() {
            settings.smoothing_ms.clamp(0.0, 500.0) / 1000.0
        } else {
            0.075
        };
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        let alpha = if tau <= 0.0 {
            1.0
        } else {
            1.0 - (-dt / tau).exp()
        };
        for (i, (_, min, max)) in PARAMETER_SPECS.iter().enumerate() {
            let value = if target.0[i].is_finite() {
                target.0[i].clamp(*min, *max)
            } else {
                Parameters::default().0[i]
            };
            self.current.0[i] += (value - self.current.0[i]) * alpha;
        }
        self.current
    }
}

/// Procedural input is explicitly labeled Demo in the app; no socket is opened.
pub fn demo_frame(t: f32) -> TrackingFrame {
    let blink = if t.rem_euclid(4.2) < 0.16 { 0.95 } else { 0.0 };
    TrackingFrame {
        face_found: true,
        rotation: Vec3 {
            x: (t * 0.71).sin() * 9.0,
            y: (t * 0.53).sin() * 22.0,
            z: (t * 0.4).sin() * 7.0,
        },
        blend_shapes: BTreeMap::from([
            ("jawopen".into(), ((t * 3.0).sin() * 0.5 + 0.5) * 0.55),
            ("eyeblinkleft".into(), blink),
            ("eyeblinkright".into(), blink),
            ("mouthsmileleft".into(), 0.65),
            ("mouthsmileright".into(), 0.65),
            ("browinnerup".into(), (t * 0.8).sin() * 0.2 + 0.25),
        ]),
        hotkey: -1,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_calibration_restores_without_carrying_previous_filter_values() {
        let mut pipeline = ParameterPipeline::default();
        let mut frame = demo_frame(1.0);
        pipeline.calibrate(&frame);
        let saved = pipeline.calibration();
        pipeline.current.0[0] = 30.0;
        pipeline.restore_calibration(saved);
        let result = pipeline.update(Some(&frame), &MappingSettings::default(), 0.016);
        assert!(result.0[..3].iter().all(|v| v.abs() < 0.001));
        pipeline.restore_calibration(Vec3 {
            x: f32::NAN,
            y: 0.0,
            z: 0.0,
        });
        frame.rotation = Vec3::default();
        let result = pipeline.update(Some(&frame), &MappingSettings::default(), 0.016);
        assert!(result.0.iter().all(|v| v.is_finite()));
    }
    fn immediate() -> MappingSettings {
        MappingSettings {
            smoothing_ms: 0.0,
            ..Default::default()
        }
    }

    #[test]
    fn calibration_handles_euler_wrap() {
        let mut p = ParameterPipeline::default();
        let mut f = demo_frame(0.0);
        f.rotation.y = 359.0;
        assert!(p.calibrate(&f));
        f.rotation.y = 1.0;
        assert!((p.update(Some(&f), &immediate(), 0.016).0[0] - 2.0).abs() < 0.001);
    }

    #[test]
    fn lost_face_returns_to_neutral() {
        let mut p = ParameterPipeline::default();
        p.update(Some(&demo_frame(2.0)), &immediate(), 0.016);
        let f = TrackingFrame::default();
        assert_eq!(
            p.update(Some(&f), &immediate(), 0.016).0,
            Parameters::default().0
        );
        assert!(!p.calibrate(&f));
    }

    #[test]
    fn smoothing_is_independent_of_render_rate() {
        let f = demo_frame(2.0);
        let mut a = ParameterPipeline::default();
        let mut b = ParameterPipeline::default();
        for _ in 0..30 {
            a.update(Some(&f), &MappingSettings::default(), 1.0 / 60.0);
        }
        for _ in 0..60 {
            b.update(Some(&f), &MappingSettings::default(), 1.0 / 120.0);
        }
        for (a, b) in a.current.0.into_iter().zip(b.current.0) {
            assert!((a - b).abs() < 0.0001);
        }
    }

    #[test]
    fn clamps_and_mirrors_asymmetric_inputs() {
        let mut f = demo_frame(0.0);
        f.rotation.y = 100.0;
        f.blend_shapes.insert("eyeblinkleft".into(), 1.0);
        f.blend_shapes.insert("eyeblinkright".into(), 0.0);
        let s = MappingSettings {
            mirror: true,
            ..immediate()
        };
        let v = ParameterPipeline::default().update(Some(&f), &s, 0.016);
        assert_eq!(v.0[0], -30.0);
        assert_eq!(v.0[3], 1.0);
        assert_eq!(v.0[4], 0.0);
    }

    #[test]
    fn nonfinite_input_cannot_poison_pipeline() {
        let mut f = demo_frame(0.0);
        f.rotation.x = f32::NAN;
        assert_eq!(
            ParameterPipeline::default()
                .update(Some(&f), &immediate(), 0.016)
                .0,
            Parameters::default().0
        );
    }
}
