//! One mouth filter before rig mapping, shared by standard and raw VTS inputs.
use crate::rig::Inputs;
use serde::{Deserialize, Serialize};

pub const INPUTS: &[&str] = &[
    "MouthOpen",
    "ParamMouthOpenY",
    "JawOpen",
    "MouthPressLipOpen",
    "ARKit:jawopen",
    "ARKit:mouthclose",
];

pub fn is_mouth_input(name: &str) -> bool {
    INPUTS.contains(&name)
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Response {
    pub enabled: bool,
    pub smoothing_ms: f32,
}
impl Default for Response {
    fn default() -> Self {
        Self {
            enabled: true,
            smoothing_ms: 12.0,
        }
    }
}
impl Response {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.smoothing_ms.is_finite() && (0.0..=200.0).contains(&self.smoothing_ms),
            "Invalid mouth response smoothing"
        );
        Ok(())
    }
}

#[derive(Default)]
pub struct Filter {
    values: Inputs,
}
impl Filter {
    pub fn reset(&mut self) {
        self.values.clear();
    }
    pub fn apply(&mut self, response: Response, inputs: &mut Inputs, dt: f32) {
        if !response.enabled {
            self.reset();
            return;
        }
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        let ms = if response.smoothing_ms.is_finite() {
            response.smoothing_ms.clamp(0.0, 200.0)
        } else {
            12.0
        };
        let alpha = if ms == 0.0 {
            1.0
        } else {
            1.0 - (-dt * 1000.0 / ms).exp()
        };
        for &name in INPUTS {
            let target = inputs
                .get(name)
                .copied()
                .filter(|v| v.is_finite())
                .unwrap_or(0.0);
            let value = self.values.entry(name.into()).or_insert(0.0);
            *value += (target - *value) * alpha;
            inputs.insert(name.into(), *value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MappingSettings, ParameterPipeline, TrackingFrame, calibration,
        movement::{PoseMode, RigConfig},
        rig,
    };

    // Exercise the complete desktop path, including personally calibrated input
    // and an imported VTS binding with 400 ms of authored smoothing.
    fn step(
        open: f32,
        source: &str,
        calibrated: bool,
        rig: &mut RigConfig,
        pipeline: &mut ParameterPipeline,
        filter: &mut Filter,
    ) -> (f32, f32) {
        let settings = MappingSettings {
            mouth_gain: 1.0,
            ..Default::default()
        };
        let frame = TrackingFrame {
            face_found: true,
            rotation: crate::Vec3 {
                y: 30.0,
                ..Default::default()
            },
            blend_shapes: [("jawopen".into(), open)].into(),
            ..Default::default()
        };
        let p = pipeline.update_with_response(
            Some(&frame),
            &settings,
            1.0 / 60.0,
            rig.mouth_response.enabled,
        );
        let mut inputs = rig::tracking_inputs(Some(&frame), p, false, 0.0);
        if calibrated {
            let profile = calibration::Profile {
                enabled: true,
                ranges: [(
                    source.into(),
                    calibration::Range {
                        low: 0.0,
                        neutral: 0.0,
                        high: 0.5,
                        enabled: true,
                    },
                )]
                .into(),
                ..Default::default()
            };
            let measured = profile.measure(Some(&frame), &settings, 0.0);
            calibration::Filter::default().apply(
                &profile,
                &measured,
                &mut inputs,
                true,
                calibration::Smoothing {
                    milliseconds: 75.0,
                    responsive_mouth: rig.mouth_response.enabled,
                },
                1.0 / 60.0,
            );
        }
        filter.apply(rig.mouth_response, &mut inputs, 1.0 / 60.0);
        let mut parameters = [rig::RigParameter {
            id: "Mouth".into(),
            min: 0.0,
            max: 1.0,
            default: 0.0,
            value: 0.0,
        }];
        rig.evaluate(&inputs, &mut parameters, 1.0 / 60.0, None);
        (parameters[0].value, p.0[0])
    }
    #[test]
    fn speech_opens_and_closes_in_two_frames_without_speeding_up_head() {
        for source in ["MouthOpen", "ParamMouthOpenY", "JawOpen", "ARKit:jawopen"] {
            for calibrated in [false, true] {
                let mut rig = RigConfig::default();
                let mut binding = rig::Binding::direct(source, 0.0, 1.0);
                binding.smoothing_ms = 400.0;
                rig.bindings.insert("Mouth".into(), binding);
                let mut pipeline = ParameterPipeline::default();
                let mut filter = Filter::default();
                step(
                    0.0,
                    source,
                    calibrated,
                    &mut rig,
                    &mut pipeline,
                    &mut filter,
                );
                let amount = if calibrated { 0.5 } else { 1.0 };
                step(
                    amount,
                    source,
                    calibrated,
                    &mut rig,
                    &mut pipeline,
                    &mut filter,
                );
                let (open, head) = step(
                    amount,
                    source,
                    calibrated,
                    &mut rig,
                    &mut pipeline,
                    &mut filter,
                );
                assert!(open > 0.93, "{source}: {open}");
                assert!(head < 20.0, "head smoothing must remain intact");
                step(
                    0.0,
                    source,
                    calibrated,
                    &mut rig,
                    &mut pipeline,
                    &mut filter,
                );
                let (closed, _) = step(
                    0.0,
                    source,
                    calibrated,
                    &mut rig,
                    &mut pipeline,
                    &mut filter,
                );
                assert!(closed < 0.07, "{source}: {closed}");
                assert_eq!(rig.bindings["Mouth"].smoothing_ms, 400.0);
            }
        }
    }
    #[test]
    fn presets_migrate_and_legacy_response_inversion_and_frozen_pose_survive() {
        let mut config: RigConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.mouth_response, Response::default());
        config.mouth_response = Response {
            enabled: false,
            smoothing_ms: 35.0,
        };
        let mut b = rig::Binding::direct("JawOpen", 0.0, 1.0);
        b.output_min = 1.0;
        b.output_max = 0.0;
        b.smoothing_ms = 400.0;
        config.bindings.insert("Mouth".into(), b);
        let mut config: RigConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert!(!config.mouth_response.enabled);
        let mut pipeline = ParameterPipeline::default();
        let mut filter = Filter::default();
        assert_eq!(
            step(
                0.0,
                "JawOpen",
                false,
                &mut config,
                &mut pipeline,
                &mut filter
            )
            .0,
            1.0
        );
        assert!(
            step(
                1.0,
                "JawOpen",
                false,
                &mut config,
                &mut pipeline,
                &mut filter
            )
            .0 > 0.9
        );
        config.pose.mode = PoseMode::Frozen;
        config.pose.frozen.insert("Mouth".into(), 0.4);
        config.mouth_response.enabled = true;
        assert_eq!(
            step(
                1.0,
                "JawOpen",
                false,
                &mut config,
                &mut pipeline,
                &mut filter
            )
            .0,
            0.4
        );
        config.pose.mode = PoseMode::Live;
        config.mouth_response.smoothing_ms = 0.0;
        assert_eq!(
            step(
                1.0,
                "JawOpen",
                false,
                &mut config,
                &mut pipeline,
                &mut filter
            )
            .0,
            0.0
        );
    }
    #[test]
    fn lost_raw_inputs_relax_and_filter_is_rate_independent() {
        let mut endings = Vec::new();
        for fps in [30, 60, 120] {
            let mut filter = Filter::default();
            let mut out = Inputs::new();
            for _ in 0..fps / 10 {
                out.insert("ARKit:jawopen".into(), 1.0);
                filter.apply(Response::default(), &mut out, 1.0 / fps as f32);
            }
            endings.push(out["ARKit:jawopen"]);
            out.clear();
            filter.apply(Response::default(), &mut out, 0.1);
            assert!(out["ARKit:jawopen"] < 0.001);
        }
        assert!((endings[0] - endings[2]).abs() < 1e-5);
        assert!(
            Response {
                enabled: true,
                smoothing_ms: f32::NAN
            }
            .validate()
            .is_err()
        );
    }
}
