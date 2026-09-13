//! Model-specific tracking assignments. VTS's proprietary tracking filters are
//! approximated from the public raw blendshapes; authored ranges are preserved.
use crate::{PARAMETER_SPECS, Parameters, TrackingFrame};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RigParameter {
    pub id: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub value: f32,
}

pub const INPUT_NAMES: &[&str] = &[
    "FaceAngleX",
    "FaceAngleY",
    "FaceAngleZ",
    "EyeOpenLeft",
    "EyeOpenRight",
    "EyeLeftX",
    "EyeLeftY",
    "EyeRightX",
    "EyeRightY",
    "MouthOpen",
    "MouthSmile",
    "Brows",
    "BrowLeftY",
    "BrowRightY",
    "MouthX",
    "MouthFunnel",
    "MouthShrug",
    "MouthPressLipOpen",
    "MouthPucker",
    "CheekPuff",
    "Breath",
    "AutoBlink",
];

pub type Inputs = BTreeMap<String, f32>;

pub const FACE_ALIASES: &[(&str, &str)] = &[
    ("JawOpen", "jawopen"),
    ("TongueOut", "tongueout"),
    ("BrowInnerUp", "browinnerup"),
];
/// Newly supported imports that need an additive migration in pre-0.21 profiles.
pub fn upgraded_input(name: &str) -> bool {
    crate::controller::INPUT_NAMES.contains(&name)
        || FACE_ALIASES.iter().any(|(alias, _)| *alias == name)
}

pub fn input_range(name: &str) -> (f32, f32) {
    if let Some(spec) = PARAMETER_SPECS.iter().find(|s| s.0 == name) {
        return (spec.1, spec.2);
    }
    match name {
        "NP_LStickX" | "NP_LStickY" | "NP_RStickX" | "NP_RStickY" | "NP_LThumbX" | "NP_LThumbY"
        | "NP_RThumbX" | "NP_RThumbY" => (-1.0, 1.0),
        "FaceAngleX" | "FaceAngleY" | "FaceAngleZ" => (-30.0, 30.0),
        "EyeLeftX" | "EyeLeftY" | "EyeRightX" | "EyeRightY" => (-0.5, 0.5),
        "MouthX" | "MouthPressLipOpen" | "BrowLeftY" | "BrowRightY" => (-1.0, 1.0),
        _ => (0.0, 1.0),
    }
}

/// Head values have already been calibrated, mirrored and smoothed by the
/// common pipeline. Keep each eye's gaze and nonstandard mouth inputs available.
pub fn tracking_inputs(
    frame: Option<&TrackingFrame>,
    p: Parameters,
    mirror: bool,
    time: f32,
) -> Inputs {
    let frame = frame.filter(|f| f.face_found && f.is_finite());
    let b = |key| frame.map_or(0.0, |f| f.blend(key));
    let left_x = b("eyelookoutleft") - b("eyelookinleft");
    let right_x = b("eyelookinright") - b("eyelookoutright");
    let left_y = b("eyelookupleft") - b("eyelookdownleft");
    let right_y = b("eyelookupright") - b("eyelookdownright");
    let phase = if time.is_finite() { time } else { 0.0 };
    let blink_phase = phase.rem_euclid(4.2);
    let blink = if blink_phase < 0.18 {
        (blink_phase / 0.18 * std::f32::consts::PI).sin()
    } else {
        0.0
    };
    let values = [
        p.0[0],
        p.0[1],
        p.0[2],
        p.0[3],
        p.0[4],
        if mirror { -right_x } else { left_x } * 0.5,
        if mirror { right_y } else { left_y } * 0.5,
        if mirror { -left_x } else { right_x } * 0.5,
        if mirror { left_y } else { right_y } * 0.5,
        p.0[5],
        p.0[6].max(0.0),
        ((p.0[7] + p.0[8]) * 0.5).clamp(0.0, 1.0),
        p.0[7],
        p.0[8],
        (b("mouthright") - b("mouthleft")) * if mirror { -1.0 } else { 1.0 },
        b("mouthfunnel"),
        (b("mouthshrugupper") + b("mouthshruglower")) * 0.5,
        b("jawopen") - (b("mouthpressleft") + b("mouthpressright")) * 0.5,
        b("mouthpucker"),
        b("cheekpuff"),
        0.5 - 0.5 * (phase * std::f32::consts::TAU / 4.0).cos(),
        1.0 - blink,
    ];
    let mut inputs: Inputs = INPUT_NAMES
        .iter()
        .zip(values)
        .map(|(k, v)| ((*k).into(), v))
        .collect();
    inputs.extend(
        PARAMETER_SPECS
            .iter()
            .zip(p.0)
            .map(|(s, v)| (s.0.into(), v)),
    );
    inputs.extend(
        FACE_ALIASES
            .iter()
            .map(|(alias, key)| ((*alias).into(), b(key))),
    );
    if let Some(frame) = frame {
        inputs.extend(
            frame
                .blend_shapes
                .iter()
                .map(|(k, v)| (format!("ARKit:{k}"), *v)),
        );
    }
    inputs
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Binding {
    pub name: String,
    pub input: String,
    pub input_min: f32,
    pub input_max: f32,
    pub output_min: f32,
    pub output_max: f32,
    pub clamp_input: bool,
    pub clamp_output: bool,
    pub smoothing_ms: f32,
    #[serde(default)]
    pub dead_zone: f32,
    #[serde(default = "unit_curve")]
    pub response_curve: f32,
    #[serde(skip)]
    current: Option<f32>,
}

fn unit_curve() -> f32 {
    1.0
}

impl Binding {
    pub fn direct(input: &str, min: f32, max: f32) -> Self {
        Self {
            name: input.into(),
            input: input.into(),
            input_min: min,
            input_max: max,
            output_min: min,
            output_max: max,
            clamp_input: true,
            clamp_output: true,
            smoothing_ms: 0.0,
            dead_zone: 0.0,
            response_curve: 1.0,
            current: None,
        }
    }
    pub fn evaluate(&mut self, input: f32, dt: f32) -> f32 {
        let span = self.input_max - self.input_min;
        let mut t = if span.abs() < 1e-6 {
            0.0
        } else {
            (input - self.input_min) / span
        };
        if self.clamp_input {
            t = t.clamp(0.0, 1.0);
        }
        let centered = t * 2.0 - 1.0;
        let dead = self.dead_zone.clamp(0.0, 0.49) * 2.0;
        let response = ((centered.abs() - dead).max(0.0) / (1.0 - dead))
            .powf(self.response_curve.clamp(0.1, 4.0))
            * centered.signum();
        t = (response + 1.0) * 0.5;
        let mut target = self.output_min + t * (self.output_max - self.output_min);
        if self.clamp_output {
            target = target.clamp(
                self.output_min.min(self.output_max),
                self.output_min.max(self.output_max),
            );
        }
        if !target.is_finite() {
            target = self.output_min;
        }
        let current = self.current.get_or_insert(target);
        let alpha = if self.smoothing_ms <= 0.0 {
            1.0
        } else {
            1.0 - (-dt.clamp(0.0, 0.25) * 1000.0 / self.smoothing_ms).exp()
        };
        *current += (target - *current) * alpha;
        *current
    }
    pub fn reset_filter(&mut self) {
        self.current = None;
    }
}

pub fn default_bindings(parameters: &[RigParameter]) -> BTreeMap<String, Binding> {
    let mut bindings = BTreeMap::new();
    for p in parameters {
        if let Some(s) = PARAMETER_SPECS.iter().find(|s| s.0 == p.id) {
            bindings.insert(p.id.clone(), Binding::direct(s.0, s.1, s.2));
        } else if p.id == "ParamBreath" {
            bindings.insert(p.id.clone(), Binding::direct("Breath", 0.0, 1.0));
        } else if p.id == "ParamBodyAngleY" || p.id == "ParamBodyAngleZ" {
            let input = if p.id.ends_with('Y') {
                "FaceAngleY"
            } else {
                "FaceAngleZ"
            };
            let mut binding = Binding::direct(input, -30.0, 30.0);
            binding.output_min = -10.0;
            binding.output_max = 10.0;
            binding.smoothing_ms = 180.0;
            bindings.insert(p.id.clone(), binding);
        }
    }
    bindings
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Profile {
    parameter_settings: Vec<Assignment>,
    #[serde(default)]
    physics_settings: ProfilePhysics,
    #[serde(default)]
    physics_customization_settings: CustomPhysics,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Assignment {
    #[serde(default)]
    name: String,
    input: String,
    #[serde(rename = "OutputLive2D")]
    output: String,
    input_range_lower: f32,
    input_range_upper: f32,
    output_range_lower: f32,
    output_range_upper: f32,
    #[serde(default)]
    clamp_input: bool,
    #[serde(default)]
    clamp_output: bool,
    #[serde(default)]
    smoothing: f32,
    #[serde(default)]
    use_breathing: bool,
    #[serde(default)]
    use_blinking: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ProfilePhysics {
    #[serde(rename = "Use")]
    enabled: bool,
}
impl Default for ProfilePhysics {
    fn default() -> Self {
        Self { enabled: true }
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CustomPhysics {
    #[serde(default)]
    physics_multipliers_per_physics_group: Vec<GroupMultiplier>,
}
#[derive(Deserialize)]
struct GroupMultiplier {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "Value")]
    value: f32,
}

pub struct ImportedProfile {
    pub bindings: BTreeMap<String, Binding>,
    pub warnings: Vec<String>,
    pub physics_enabled: bool,
    pub physics_multipliers: BTreeMap<String, f32>,
}

pub fn import_profile(bytes: &[u8], parameters: &[RigParameter]) -> Result<ImportedProfile> {
    ensure!(
        bytes.len() <= crate::asset_limits::MODEL_JSON,
        "VTS profile exceeds 20 MiB"
    );
    let profile: Profile = serde_json::from_slice(bytes)?;
    ensure!(
        profile.parameter_settings.len() <= 4096,
        "Too many tracking assignments"
    );
    let mut imported = ImportedProfile {
        bindings: BTreeMap::new(),
        warnings: Vec::new(),
        physics_enabled: profile.physics_settings.enabled,
        physics_multipliers: BTreeMap::new(),
    };
    for a in profile.parameter_settings {
        let input = if a.use_breathing {
            "Breath"
        } else if a.use_blinking {
            "AutoBlink"
        } else {
            &a.input
        };
        if !parameters.iter().any(|p| p.id == a.output) {
            imported.warnings.push(format!(
                "Profile output {} is absent from this rig",
                a.output
            ));
            continue;
        }
        if !INPUT_NAMES.contains(&input)
            && !upgraded_input(input)
            && !PARAMETER_SPECS.iter().any(|s| s.0 == input)
        {
            imported.warnings.push(format!(
                "Unsupported profile input {input} for {}",
                a.output
            ));
            continue;
        }
        ensure!(
            [
                a.input_range_lower,
                a.input_range_upper,
                a.output_range_lower,
                a.output_range_upper,
                a.smoothing
            ]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e6),
            "Invalid profile range"
        );
        ensure!(
            (a.input_range_upper - a.input_range_lower).abs() > 1e-6,
            "Zero-width profile input range for {}",
            a.output
        );
        imported.bindings.insert(
            a.output,
            Binding {
                name: a.name,
                input: input.into(),
                input_min: a.input_range_lower,
                input_max: a.input_range_upper,
                output_min: a.output_range_lower,
                output_max: a.output_range_upper,
                clamp_input: a.clamp_input,
                clamp_output: a.clamp_output,
                // VTS does not publish its filter algorithm. Interpret its 0..100
                // amount as a time constant, keeping body movement softer than eyes.
                smoothing_ms: a.smoothing.clamp(0.0, 100.0) * 4.0,
                dead_zone: 0.0,
                response_curve: 1.0,
                current: None,
            },
        );
    }
    for g in profile
        .physics_customization_settings
        .physics_multipliers_per_physics_group
    {
        ensure!(
            g.value.is_finite() && (0.0..=5.0).contains(&g.value),
            "Invalid physics group multiplier"
        );
        imported.physics_multipliers.insert(g.id, g.value);
    }
    Ok(imported)
}

pub fn apply_bindings(
    bindings: &mut BTreeMap<String, Binding>,
    inputs: &Inputs,
    parameters: &mut [RigParameter],
    dt: f32,
) {
    for p in parameters {
        if let Some(binding) = bindings.get_mut(&p.id) {
            // Missing custom sources relax to zero; stale frames do not freeze a mouth.
            let value = binding.evaluate(inputs.get(&binding.input).copied().unwrap_or(0.0), dt);
            p.value = value.clamp(p.min, p.max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_controller_and_raw_face_inputs_with_authored_ranges() {
        let names: Vec<_> = crate::controller::INPUT_NAMES
            .iter()
            .copied()
            .chain(FACE_ALIASES.iter().map(|p| p.0))
            .collect();
        let parameters: Vec<_> = names
            .iter()
            .map(|n| RigParameter {
                id: format!("Output{n}"),
                min: -10.0,
                max: 10.0,
                default: 0.0,
                value: 0.0,
            })
            .collect();
        let settings: Vec<_> = names
            .iter()
            .map(|n| {
                let (lo, hi) = input_range(n);
                serde_json::json!({"Input":n,"OutputLive2D":format!("Output{n}"),
                "InputRangeLower":lo,"InputRangeUpper":hi,
                "OutputRangeLower":2.0,"OutputRangeUpper":-3.0,"ClampInput":true})
            })
            .collect();
        let profile = import_profile(
            &serde_json::to_vec(&serde_json::json!({"ParameterSettings":settings})).unwrap(),
            &parameters,
        )
        .unwrap();
        assert!(profile.warnings.is_empty(), "{:?}", profile.warnings);
        assert_eq!(profile.bindings.len(), 36);
        for mut b in profile.bindings.into_values() {
            assert_eq!(b.evaluate(b.input_min, 0.016), 2.0);
            assert_eq!(b.evaluate(b.input_max, 0.016), -3.0);
        }
        assert_eq!(input_range("NP_LStickY"), (-1.0, 1.0));
        assert_eq!(input_range("NP_L2"), (0.0, 1.0));
    }
    #[test]
    fn raw_face_aliases_follow_phone_and_clear_on_tracking_loss() {
        let mut frame = TrackingFrame {
            face_found: true,
            ..Default::default()
        };
        for (_, key) in FACE_ALIASES {
            frame.blend_shapes.insert((*key).into(), 0.75);
        }
        let live = tracking_inputs(Some(&frame), Parameters::default(), false, 0.0);
        for (alias, _) in FACE_ALIASES {
            assert_eq!(live[*alias], 0.75);
        }
        frame.face_found = false;
        let lost = tracking_inputs(Some(&frame), Parameters::default(), false, 0.0);
        for (alias, _) in FACE_ALIASES {
            assert_eq!(lost[*alias], 0.0);
        }
    }
    #[test]
    fn imports_custom_ranges_and_inverted_gaze() {
        let p = RigParameter {
            id: "CustomMouth".into(),
            min: -1.0,
            max: 1.0,
            default: 0.0,
            value: 0.0,
        };
        let json = br#"{"ParameterSettings":[{"Input":"MouthX","OutputLive2D":"CustomMouth","InputRangeLower":-0.2,"InputRangeUpper":0.2,"OutputRangeLower":1,"OutputRangeUpper":-1,"ClampInput":true}]}"#;
        let mut profile = import_profile(json, std::slice::from_ref(&p)).unwrap();
        let b = profile.bindings.get_mut("CustomMouth").unwrap();
        assert_eq!(b.evaluate(-0.2, 0.016), 1.0);
        assert_eq!(b.evaluate(0.0, 0.016), 0.0);
        assert_eq!(b.evaluate(0.5, 0.016), -1.0);
        assert!(profile.warnings.is_empty());
    }
    #[test]
    fn custom_mouth_and_each_eye_react_and_clear_on_face_loss() {
        let mut f = TrackingFrame {
            face_found: true,
            ..Default::default()
        };
        for (k, v) in [
            ("mouthright", 0.2),
            ("mouthfunnel", 0.8),
            ("cheekpuff", 0.6),
            ("eyelookinright", 0.7),
            ("mouthpressleft", 0.6),
        ] {
            f.blend_shapes.insert(k.into(), v);
        }
        let inputs = tracking_inputs(Some(&f), Parameters::default(), false, 1.0);
        assert_eq!(inputs["MouthX"], 0.2);
        assert_eq!(inputs["EyeRightX"], 0.35);
        assert_eq!(inputs["EyeLeftX"], 0.0);
        assert_eq!(inputs["MouthPressLipOpen"], -0.3);
        assert_eq!(inputs["MouthFunnel"], 0.8);
        assert_eq!(inputs["CheekPuff"], 0.6);
        f.face_found = false;
        let lost = tracking_inputs(Some(&f), Parameters::default(), false, 1.0);
        assert_eq!(lost["MouthFunnel"], 0.0);
        assert_eq!(lost["EyeRightX"], 0.0);
        assert!(lost["Breath"] > 0.0);
    }
}
