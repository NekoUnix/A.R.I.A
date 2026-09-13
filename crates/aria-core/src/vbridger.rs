//! EXPERIMENTAL VBridger compatibility: import legacy/V2 configuration data and evaluate
//! validated numeric expressions locally. Exact third-party timing/behavior is not guaranteed.
mod equation;
mod runtime;
use crate::{TrackingFrame, Vec3, rig::Inputs};
use anyhow::{Context, Result, ensure};
pub use equation::Equation;
pub use runtime::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_FILE: usize = 2 * 1024 * 1024;
const MASK: &[u8] = b"p3s6v9y$B&E)H@Mc";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub name: String,
    pub outputs: Vec<Output>,
    pub notes: Vec<String>,
    /// Additional corrections in imported coordinates; X=pitch, Y=yaw, Z=roll.
    pub invert_pitch: bool,
    pub invert_yaw: bool,
    pub invert_roll: bool,
    pub input_curves: BTreeMap<String, Curve>,
    pub offsets: BTreeMap<String, f32>,
    pub face_loss_ms: f32,
    pub external_inputs: BTreeMap<String, f32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    pub equation: Equation,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub curve: Curve,
    pub enabled: bool,
    pub face_send: bool,
    pub requires_face: bool,
    pub delay_ms: f32,
    pub delay_on: bool,
    pub smoothing: f32,
    pub smooth_on: bool,
    pub steps: Vec<Step>,
    pub step_on: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Curve {
    pub keys: Vec<Key>,
    pub pre_wrap: u8,
    pub post_wrap: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub trigger: f32,
    pub target: f32,
    pub threshold: f32,
    pub hold_ms: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key {
    pub time: f32,
    pub value: f32,
    #[serde(rename = "inTangent", alias = "inSlope")]
    pub incoming: f32,
    #[serde(rename = "outTangent", alias = "outSlope")]
    pub outgoing: f32,
    #[serde(default, rename = "weightedMode")]
    pub weighted: u8,
    #[serde(default = "third", rename = "inWeight")]
    pub in_weight: f32,
    #[serde(default = "third", rename = "outWeight")]
    pub out_weight: f32,
}
fn third() -> f32 {
    1.0 / 3.0
}

/// Names are exact for outputs, case-insensitive for known raw ARKit aliases.
fn blend_key(name: &str) -> Option<String> {
    let mut n = name.to_ascii_lowercase();
    if n.ends_with("_l") {
        n.truncate(n.len() - 2);
        n.push_str("left");
    } else if n.ends_with("_r") {
        n.truncate(n.len() - 2);
        n.push_str("right");
    }
    const SHAPES: &str = "browdownleft browdownright browinnerup browouterupleft browouterupright cheekpuff cheeksquintleft cheeksquintright eyeblinkleft eyeblinkright eyelookdownleft eyelookdownright eyelookinleft eyelookinright eyelookoutleft eyelookoutright eyelookupleft eyelookupright eyesquintleft eyesquintright eyewideleft eyewideright jawforward jawleft jawopen jawright mouthclose mouthdimpleleft mouthdimpleright mouthfrownleft mouthfrownright mouthfunnel mouthleft mouthlowerdownleft mouthlowerdownright mouthpressleft mouthpressright mouthpucker mouthright mouthrolllower mouthrollupper mouthshruglower mouthshrugupper mouthsmileleft mouthsmileright mouthstretchleft mouthstretchright mouthupperupleft mouthupperupright nosesneerleft nosesneerright tongueout";
    SHAPES.split(' ').any(|s| s == n).then_some(n)
}
fn raw_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "headrotx"
            | "headroty"
            | "headrotz"
            | "headposx"
            | "headposy"
            | "headposz"
            | "eyeleftx"
            | "eyelefty"
            | "eyeleftz"
            | "eyerightx"
            | "eyerighty"
            | "eyerightz"
            | "facefound"
            | "volume"
    ) || blend_key(name).is_some()
        || viseme(name)
        || body_axis(name)
}
fn body_axis(name: &str) -> bool {
    name.strip_suffix(['X', 'Y', 'Z']).is_some_and(|n| {
        [
            "Hips",
            "Spine",
            "Chest",
            "UpperChest",
            "LeftUpperArm",
            "RightUpperArm",
            "LeftLowerArm",
            "RightLowerArm",
            "LeftHand",
            "RightHand",
            "LeftUpperLeg",
            "RightUpperLeg",
            "LeftLowerLeg",
            "RightLowerLeg",
            "LeftFoot",
            "RightFoot",
        ]
        .contains(&n)
    })
}
fn viseme(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.strip_prefix("viseme_").is_some_and(|n| {
        [
            "sil", "pp", "ff", "th", "dd", "kk", "ch", "ss", "nn", "rr", "aa", "ee", "ih", "oh",
            "ou",
        ]
        .contains(&n.strip_suffix("_abs").unwrap_or(n))
    })
}
pub fn alias(input: &str) -> &str {
    match input {
        "ParamAngleX" => "FaceAngleX",
        "ParamAngleY" => "FaceAngleY",
        "ParamAngleZ" => "FaceAngleZ",
        "ParamEyeLOpen" => "EyeOpenLeft",
        "ParamEyeROpen" => "EyeOpenRight",
        "ParamMouthOpenY" => "MouthOpen",
        "ParamMouthForm" => "MouthSmile",
        "ParamBrowLY" => "BrowLeftY",
        "ParamBrowRY" => "BrowRightY",
        "ParamBodyAngleX" => "BodyAngleX",
        _ => input,
    }
}
impl Config {
    pub fn prepare(&mut self) -> Result<()> {
        let mut pending = self.outputs.clone();
        let mut sorted = Vec::new();
        let mut known = BTreeSet::new();
        loop {
            let before = pending.len();
            let mut next = Vec::new();
            for o in pending {
                if o.equation.variables.iter().all(|n| {
                    raw_name(n) || self.external_inputs.contains_key(n) || known.contains(n)
                }) {
                    known.insert(o.name.clone());
                    sorted.push(o);
                } else {
                    next.push(o);
                }
            }
            pending = next;
            if pending.is_empty() {
                break;
            }
            ensure!(
                pending.len() < before,
                "Equations contain unknown inputs or cyclic dependencies"
            );
        }
        let mut candidate = self.clone();
        candidate.outputs = sorted;
        candidate.validate()?;
        self.outputs = candidate.outputs;
        Ok(())
    }
    pub fn export(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let bytes =
            serde_json::to_vec_pretty(&serde_json::json!({"aria_vbridger":1,"config":self}))?;
        ensure!(bytes.len() <= MAX_FILE, "Export exceeds 2 MiB");
        Ok(bytes)
    }
    pub fn export_vbridger(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let rows:Vec<_>=self.outputs.iter().map(|o| {
            let keys=if o.curve.keys.is_empty(){[o.min,o.max].into_iter().map(|v|Key{time:v,value:v,incoming:1.0,outgoing:1.0,weighted:0,in_weight:third(),out_weight:third()}).collect()}else{o.curve.keys.clone()};
            serde_json::json!({
            "output":o.name,"equation":o.equation.source(),"equationY":"","equationZ":"","vectorMode":false,
            "min":o.min.to_string(),"max":o.max.to_string(),"defaultValue":o.default.to_string(),"on":o.enabled.to_string(),
            "faceSend":o.face_send,"delay":o.delay_ms.to_string(),"delayOn":o.delay_on,"smooth":o.smoothing,"smoothOn":o.smooth_on,
            "stepOn":o.step_on,"stepDetails":[],"stepDetails2":o.steps.iter().map(|s|[s.trigger,s.target,s.threshold,s.hold_ms]).collect::<Vec<_>>(),
            "curve":{"keys":keys,"length":keys.len(),"preWrapMode":o.curve.pre_wrap,"postWrapMode":o.curve.post_wrap}
        })}).collect();
        let mut bytes = b"vbridgerV2".to_vec();
        bytes.extend(serde_json::to_vec(&serde_json::json!({"store":rows}))?);
        ensure!(bytes.len() <= MAX_FILE, "Export exceeds 2 MiB");
        for (i, b) in bytes.iter_mut().enumerate() {
            *b ^= MASK[i % MASK.len()];
        }
        Ok(bytes)
    }
    pub fn raw_names(&self) -> BTreeSet<String> {
        self.outputs
            .iter()
            .flat_map(|o| {
                o.equation
                    .variables
                    .iter()
                    .filter(|n| raw_name(n) || self.external_inputs.contains_key(*n))
                    .cloned()
            })
            .collect()
    }
    pub fn calibrate_blends(&mut self, frame: &TrackingFrame) {
        for name in self.raw_names() {
            if let Some(key) = blend_key(&name) {
                self.offsets.insert(name, -frame.blend(&key));
            }
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.name.len() <= 256
                && self.outputs.len() <= 256
                && self.notes.len() <= 512
                && self.notes.iter().all(|n| n.len() <= 8192),
            "VBridger profile exceeds limits"
        );
        ensure!(
            self.face_loss_ms.is_finite()
                && (0.0..=5000.0).contains(&self.face_loss_ms)
                && self.input_curves.len() <= 128
                && self.offsets.len() <= 128,
            "Invalid input/face loss settings"
        );
        ensure!(
            self.external_inputs.len() <= 128
                && self.external_inputs.iter().all(|(n, v)| !n.is_empty()
                    && n.len() <= 128
                    && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                    && v.is_finite()
                    && v.abs() <= 1e6),
            "Invalid external inputs"
        );
        for (name, curve) in &self.input_curves {
            ensure!(
                raw_name(name) || self.external_inputs.contains_key(name),
                "Unknown input curve {name}"
            );
            curve.validate()?;
        }
        for (name, offset) in &self.offsets {
            ensure!(
                (raw_name(name) || self.external_inputs.contains_key(name))
                    && offset.is_finite()
                    && offset.abs() <= 1e6,
                "Invalid input offset"
            );
        }
        let mut seen = BTreeSet::new();
        for output in &self.outputs {
            output.validate()?;
            ensure!(
                !self.external_inputs.contains_key(&output.name),
                "External input and output cannot share name {}",
                output.name
            );
            ensure!(
                output.equation.variables.iter().all(|n| raw_name(n)
                    || self.external_inputs.contains_key(n)
                    || seen.contains(n)),
                "Unknown or forward dependency in {}",
                output.name
            );
            ensure!(
                seen.insert(output.name.clone()),
                "Duplicate output {}",
                output.name
            );
        }
        Ok(())
    }
    pub fn output(&self, name: &str) -> Option<&Output> {
        self.outputs.iter().find(|o| o.name == name)
    }

    /// Import channels intentionally replace built-in derived/calibrated channels.
    /// Raw input aliases are resolved before outputs, so `JawOpen = jawOpen` is not a cycle.
    pub fn apply(
        &self,
        frame: Option<&TrackingFrame>,
        origin: Vec3,
        inputs: &mut Inputs,
        state: &mut Runtime,
        dt: f32,
    ) {
        if !self.enabled {
            state.reset();
            return;
        }
        let external = frame.filter(|f| f.is_finite());
        state.advance(frame, dt, self.outputs.len());
        let held_frame = state.face(self.face_loss_ms).cloned();
        let frame = held_frame.as_ref();
        let sign = |flip| if flip { -1.0 } else { 1.0 };
        let rotation = frame
            .map(|f| Vec3 {
                x: -crate::signed_angle(f.rotation.x - origin.x) * sign(self.invert_pitch),
                y: -crate::signed_angle(f.rotation.y - origin.y) * sign(self.invert_yaw),
                z: crate::signed_angle(f.rotation.z - origin.z) * sign(self.invert_roll),
            })
            .unwrap_or_default();
        let volume = inputs.get("MicLevel").copied().unwrap_or(0.0);
        let mut computed = Inputs::new();
        for (index, o) in self.outputs.iter().enumerate() {
            if !o.enabled {
                computed.insert(o.name.clone(), o.default);
                continue;
            }
            let value = (!o.requires_face || frame.is_some())
                .then(|| {
                    o.equation.evaluate_at(
                        |n| {
                            let empty = TrackingFrame::default();
                            let f = frame.unwrap_or(&empty);
                            let raw = match n.to_ascii_lowercase().as_str() {
                                "headrotx" => Some(rotation.x),
                                "headroty" => Some(rotation.y),
                                "headrotz" => Some(rotation.z),
                                "headposx" => Some(f.position.x),
                                "headposy" => Some(f.position.y),
                                "headposz" => Some(f.position.z),
                                "eyeleftx" => Some(f.eye_left.x),
                                "eyelefty" => Some(f.eye_left.y),
                                "eyeleftz" => Some(f.eye_left.z),
                                "eyerightx" => Some(f.eye_right.x),
                                "eyerighty" => Some(f.eye_right.y),
                                "eyerightz" => Some(f.eye_right.z),
                                "facefound" => Some(if frame.is_some() { 1.0 } else { 0.0 }),
                                "volume" => Some(
                                    external
                                        .and_then(|f| f.parameters.get(n))
                                        .copied()
                                        .unwrap_or(volume),
                                ),
                                _ if viseme(n) => Some(
                                    external
                                        .and_then(|f| {
                                            f.parameters.get(n).or_else(|| {
                                                f.blend_shapes.get(&n.to_ascii_lowercase())
                                            })
                                        })
                                        .copied()
                                        .unwrap_or(if n.eq_ignore_ascii_case("viseme_SIL_abs") {
                                            1.0
                                        } else {
                                            0.0
                                        }),
                                ),
                                _ if body_axis(n) || self.external_inputs.contains_key(n) => Some(
                                    external
                                        .and_then(|f| f.parameters.get(n))
                                        .copied()
                                        .unwrap_or_else(|| {
                                            self.external_inputs.get(n).copied().unwrap_or(0.0)
                                        }),
                                ),
                                _ => blend_key(n).map(|key| f.blend(&key)),
                            };
                            raw.map(|v| {
                                let v = v + self.offsets.get(n).copied().unwrap_or(0.0);
                                self.input_curves
                                    .get(n)
                                    .map_or(v, |curve| curve.evaluate(v))
                            })
                            .or_else(|| computed.get(n).copied())
                            .map(f64::from)
                        },
                        state.seconds,
                    )
                })
                .flatten()
                .map(|v| o.curve.evaluate(v as f32))
                .filter(|v| v.is_finite());
            let value = state.modify(index, o, value, dt);
            computed.insert(o.name.clone(), value);
            inputs.insert(o.name.clone(), value);
            for (name, _, _) in crate::PARAMETER_SPECS {
                if alias(name) == o.name {
                    inputs.insert((*name).into(), value);
                }
            }
        }
        for (alias, channels) in [
            ("ParamEyeBallX", ["EyeLeftX", "EyeRightX"]),
            ("ParamEyeBallY", ["EyeLeftY", "EyeRightY"]),
        ] {
            let values: Vec<_> = channels
                .iter()
                .filter_map(|n| {
                    computed
                        .get(*n)
                        .filter(|_| self.output(n).is_some_and(|o| o.enabled))
                })
                .copied()
                .collect();
            if !values.is_empty() {
                inputs.insert(
                    alias.into(),
                    values.iter().sum::<f32>() / values.len() as f32,
                );
            }
        }
    }
    pub fn missing_inputs(&self, frame: &TrackingFrame) -> BTreeSet<String> {
        self.outputs
            .iter()
            .flat_map(|o| &o.equation.variables)
            .filter(|n| {
                if viseme(n) || body_axis(n) || self.external_inputs.contains_key(*n) {
                    !frame.parameters.contains_key(*n)
                        && !frame.blend_shapes.contains_key(&n.to_ascii_lowercase())
                } else {
                    blend_key(n).is_some_and(|k| !frame.blend_shapes.contains_key(&k))
                }
            })
            .cloned()
            .collect()
    }
}
impl Output {
    pub fn new(name: &str, equation: &str, min: f32, max: f32) -> Result<Self> {
        ensure!(
            min.is_finite() && max.is_finite() && max > min,
            "Invalid output range"
        );
        let value = Self {
            name: name.into(),
            equation: Equation::parse(equation)?,
            min,
            max,
            default: 0.0_f32.clamp(min, max),
            curve: Curve::default(),
            enabled: true,
            face_send: true,
            requires_face: true,
            delay_ms: 0.0,
            delay_on: false,
            smoothing: 0.0,
            smooth_on: false,
            steps: Vec::new(),
            step_on: false,
        };
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.is_empty()
                && self.name.len() <= 128
                && self
                    .name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_'),
            "Output name must use letters, digits or underscores"
        );
        ensure!(
            [self.min, self.max, self.default]
                .into_iter()
                .all(|v| v.is_finite() && v.abs() <= 1e6)
                && self.max - self.min > 1e-6
                && (self.min..=self.max).contains(&self.default),
            "Invalid range/default for {}",
            self.name
        );
        self.curve.validate()?;
        ensure!(
            self.delay_ms.is_finite()
                && (0.0..=5000.0).contains(&self.delay_ms)
                && self.smoothing.is_finite()
                && (0.0..=1.0).contains(&self.smoothing),
            "Invalid delay/smoothing"
        );
        ensure!(
            self.steps.len() <= 64
                && self
                    .steps
                    .iter()
                    .all(|s| [s.trigger, s.target, s.threshold, s.hold_ms]
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 1e6)
                        && (0.0..=5000.0).contains(&s.hold_ms)),
            "Invalid steps"
        );
        ensure!(
            self.steps.windows(2).all(|s| s[1].trigger > s[0].trigger),
            "Step triggers must increase"
        );
        Ok(())
    }
}
impl Curve {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.keys.len() <= 64
                && [self.pre_wrap, self.post_wrap]
                    .iter()
                    .all(|m| [0, 1, 2, 4, 8].contains(m)),
            "Invalid curve keys/wrap mode"
        );
        for k in &self.keys {
            ensure!(
                [k.time, k.value, k.incoming, k.outgoing]
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 1e6)
                    && k.weighted <= 3
                    && [k.in_weight, k.out_weight]
                        .iter()
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
                "Invalid curve key"
            );
        }
        ensure!(
            self.keys.windows(2).all(|p| p[1].time - p[0].time > 1e-6),
            "Curve times must increase"
        );
        Ok(())
    }
    pub fn evaluate(&self, mut value: f32) -> f32 {
        if self.keys.is_empty() {
            return value;
        }
        let first = &self.keys[0];
        let last = self.keys.last().unwrap();
        let mode = if value < first.time {
            self.pre_wrap
        } else if value > last.time {
            self.post_wrap
        } else {
            0
        };
        let span = last.time - first.time;
        if span > 0.0 && (mode == 2 || mode == 4) {
            let phase = (value - first.time).rem_euclid(span * if mode == 4 { 2.0 } else { 1.0 });
            value = first.time
                + if phase > span {
                    2.0 * span - phase
                } else {
                    phase
                };
        }
        if value <= first.time {
            return first.value;
        }
        for pair in self.keys.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if value <= b.time {
                let span = b.time - a.time;
                let t = (value - a.time) / span;
                if a.weighted & 2 != 0 || b.weighted & 1 != 0 {
                    let w1 = if a.weighted & 2 != 0 {
                        a.out_weight
                    } else {
                        third()
                    };
                    let w2 = if b.weighted & 1 != 0 {
                        b.in_weight
                    } else {
                        third()
                    };
                    let bez = |t: f32, p0: f32, p1: f32, p2: f32, p3: f32| {
                        let u = 1.0 - t;
                        u * u * u * p0
                            + 3.0 * u * u * t * p1
                            + 3.0 * u * t * t * p2
                            + t * t * t * p3
                    };
                    let (mut low, mut high) = (0.0, 1.0);
                    for _ in 0..24 {
                        let mid = (low + high) * 0.5;
                        if bez(mid, 0.0, w1, 1.0 - w2, 1.0) < t {
                            low = mid;
                        } else {
                            high = mid;
                        }
                    }
                    return bez(
                        (low + high) * 0.5,
                        a.value,
                        a.value + span * w1 * a.outgoing,
                        b.value - span * w2 * b.incoming,
                        b.value,
                    );
                }
                return (2.0 * t * t * t - 3.0 * t * t + 1.0) * a.value
                    + (t * t * t - 2.0 * t * t + t) * span * a.outgoing
                    + (-2.0 * t * t * t + 3.0 * t * t) * b.value
                    + (t * t * t - t * t) * span * b.incoming;
            }
        }
        last.value
    }
}

pub fn import(bytes: &[u8], name: &str) -> Result<Config> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_FILE,
        "VBridger import must be between 1 byte and 2 MiB"
    );
    let decoded;
    let looks_plain = std::str::from_utf8(bytes).is_ok_and(|s| {
        let s = s.trim_start_matches('\u{feff}').trim_start();
        s.starts_with('{') || s.starts_with("vbridgerV")
    });
    let plain = if looks_plain {
        bytes
    } else {
        decoded = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ MASK[i % MASK.len()])
            .collect::<Vec<_>>();
        &decoded
    };
    let text = std::str::from_utf8(plain)
        .context("Unrecognized VBridger encoding")?
        .trim_start_matches('\u{feff}')
        .trim();
    let text = if text.starts_with("vbridgerV") {
        text.strip_prefix("vbridgerV2")
            .context("Only VBridger V2 exports are supported")?
    } else {
        text
    };
    let root: Value = serde_json::from_str(text).context("Invalid VBridger V2/JSON export")?;
    if let Some(version) = root.get("aria_vbridger") {
        ensure!(
            version.as_u64() == Some(1),
            "Unsupported ARIA VBridger config version"
        );
        let config: Config =
            serde_json::from_value(root.get("config").context("Missing config")?.clone())?;
        config.validate()?;
        return Ok(config);
    }
    let rows = root
        .get("store")
        .and_then(Value::as_array)
        .context("VBridger export has no store array")?;
    ensure!(
        !rows.is_empty() && rows.len() <= 256,
        "Expected 1–256 VBridger outputs"
    );
    let mut result = Config {
        enabled: true,
        name: name.chars().take(200).collect(),
        ..Default::default()
    };
    for key in root
        .as_object()
        .context("Expected VBridger object")?
        .keys()
        .filter(|k| k.as_str() != "store")
    {
        result.notes.push(format!(
            "File setting {key} was not imported; only output settings are supported."
        ));
    }
    let mut names = BTreeMap::new();
    let mut pending = Vec::new();
    for row in rows {
        let label = row
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("Unnamed output");
        if let Some(previous) = names.insert(label.to_owned(), row) {
            ensure!(
                previous == row,
                "Conflicting duplicate output {label}; nothing imported"
            );
            result
                .notes
                .push(format!("{label}: merged an identical duplicate output."));
            continue;
        }
        let vector = boolean(row, "vectorMode", false)?;
        let fields: &[(&str, &str)] = if vector {
            &[("X", "equation"), ("Y", "equationY"), ("Z", "equationZ")]
        } else {
            &[("", "equation")]
        };
        let mut expanded = Vec::new();
        for (suffix, field) in fields {
            let mut row = row.clone();
            row["output"] = Value::String(format!("{label}{suffix}"));
            row["equation"] = row.get(field).cloned().unwrap_or(Value::String("0".into()));
            match parse_output(&row) {
                Ok(output) => expanded.push(output),
                Err(error) => result.notes.push(format!("{label}: skipped — {error:#}")),
            }
        }
        if expanded.len() == fields.len() {
            pending.extend(expanded);
        }
    }
    // Stable topological order, rather than silently reading last frame's values.
    let mut known = BTreeSet::new();
    loop {
        let before = pending.len();
        let mut next = Vec::new();
        for output in pending {
            if output
                .equation
                .variables
                .iter()
                .all(|n| raw_name(n) || known.contains(n))
            {
                known.insert(output.name.clone());
                result.outputs.push(output);
            } else {
                next.push(output);
            }
        }
        pending = next;
        if pending.len() == before {
            break;
        }
    }
    for output in pending {
        let unknown: Vec<_> = output
            .equation
            .variables
            .iter()
            .filter(|n| !raw_name(n) && !known.contains(*n))
            .cloned()
            .collect();
        result.notes.push(format!(
            "{}: skipped — unsupported, skipped or cyclic dependencies: {}",
            output.name,
            unknown.join(", ")
        ));
    }
    result.validate()?;
    Ok(result)
}
fn boolean(v: &Value, key: &str, default: bool) -> Result<bool> {
    match v.get(key) {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::String(s)) if s == "true" => Ok(true),
        Some(Value::String(s)) if s == "false" => Ok(false),
        _ => anyhow::bail!("Invalid {key} flag"),
    }
}
fn number(v: &Value, key: &str) -> Result<f32> {
    let x = v.get(key).with_context(|| format!("Missing {key}"))?;
    let value = if let Some(s) = x.as_str() {
        s.parse::<f32>()?
    } else {
        x.as_f64().with_context(|| format!("Invalid {key}"))? as f32
    };
    ensure!(value.is_finite() && value.abs() <= 1e6, "Invalid {key}");
    Ok(value)
}
fn parse_output(v: &Value) -> Result<Output> {
    // Reject unknown output behavior rather than dropping a modifier silently.
    for key in v.as_object().context("Expected output object")?.keys() {
        ensure!(
            [
                "output",
                "equation",
                "equationY",
                "equationZ",
                "min",
                "max",
                "defaultValue",
                "delay",
                "smooth",
                "on",
                "delayOn",
                "smoothOn",
                "stepOn",
                "vectorMode",
                "stepDetails",
                "stepDetails2",
                "curve",
                "faceSend"
            ]
            .contains(&key.as_str()),
            "Unknown output setting {key}"
        );
    }
    let mut curve = Curve::default();
    if let Some(c) = v.get("curve").filter(|v| !v.is_null()) {
        curve.pre_wrap = c
            .get("preWrapMode")
            .map(|m| m.as_u64().context("Invalid pre-wrap"))
            .transpose()?
            .unwrap_or(8)
            .try_into()?;
        curve.post_wrap = c
            .get("postWrapMode")
            .map(|m| m.as_u64().context("Invalid post-wrap"))
            .transpose()?
            .unwrap_or(8)
            .try_into()?;
        // Legacy Unity serialized m_* curves use infinity=2 for endpoint clamping.
        for wrap in ["m_PreInfinity", "m_PostInfinity"] {
            if let Some(mode) = c.get(wrap) {
                ensure!(
                    mode.as_u64() == Some(2),
                    "Unsupported legacy curve infinity mode"
                );
            }
        }
        let keys = c
            .get("keys")
            .or_else(|| c.get("m_Curve"))
            .and_then(Value::as_array)
            .context("Missing curve keys")?;
        ensure!(keys.len() <= 64, "Curve exceeds 64 keys");
        for key in keys {
            curve
                .keys
                .push(serde_json::from_value(key.clone()).context("Invalid curve key")?);
        }
    }
    let min = number(v, "min")?;
    let max = number(v, "max")?;
    let mut steps = Vec::new();
    if let Some(rows) = v.get("stepDetails2").and_then(Value::as_array) {
        ensure!(rows.len() <= 62, "Steps exceed 62 authored entries");
        for row in rows {
            let cells: [f32; 4] = serde_json::from_value(row.clone())
                .context("Step needs trigger, target, threshold and hold ms")?;
            steps.push(Step {
                trigger: cells[0],
                target: cells[1],
                threshold: cells[2],
                hold_ms: cells[3],
            });
        }
    }
    ensure!(
        v.get("stepDetails")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty),
        "Legacy stepDetails layout is unsupported; resave as V2"
    );
    if boolean(v, "stepOn", false)? {
        if !steps.iter().any(|s| s.trigger <= min) {
            steps.push(Step {
                trigger: min,
                target: min,
                threshold: min,
                hold_ms: 0.0,
            });
        }
        if !steps.iter().any(|s| s.target >= max || s.trigger >= max) {
            steps.push(Step {
                trigger: max,
                target: max,
                threshold: max,
                hold_ms: 0.0,
            });
        }
        steps.sort_by(|a, b| a.trigger.total_cmp(&b.trigger));
    }
    let output = Output {
        name: v
            .get("output")
            .and_then(Value::as_str)
            .context("Missing output name")?
            .into(),
        equation: Equation::parse(
            v.get("equation")
                .and_then(Value::as_str)
                .context("Missing equation")?,
        )?,
        min,
        max,
        default: number(v, "defaultValue")?,
        curve,
        enabled: boolean(v, "on", true)?,
        face_send: boolean(v, "faceSend", true)?,
        requires_face: Equation::parse(v["equation"].as_str().unwrap())?
            .variables
            .iter()
            .any(|n| blend_key(n).is_some() || n.starts_with("head") || n.starts_with("eye")),
        delay_ms: if v.get("delay").is_some() {
            number(v, "delay")?
        } else {
            0.0
        },
        delay_on: boolean(v, "delayOn", v.get("smoothOn").is_none())?,
        smoothing: if v.get("smooth").is_some() {
            number(v, "smooth")?
        } else {
            0.0
        },
        smooth_on: boolean(v, "smoothOn", false)?,
        steps,
        step_on: boolean(v, "stepOn", false)?,
    };
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests;
