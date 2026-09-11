//! Serializable movement tuning and screenshot poses, independent of the UI/Core ABI.
pub use crate::physics::PhysicsSettings;
use crate::{
    MappingSettings, PARAMETER_SPECS, Parameters,
    physics::Physics,
    rig::{self, Binding, Inputs, RigParameter},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum PoseMode {
    #[default]
    Live,
    Override,
    Frozen,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Pose {
    pub mode: PoseMode,
    pub held: BTreeMap<String, f32>,
    pub frozen: BTreeMap<String, f32>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RigConfig {
    pub bindings: BTreeMap<String, Binding>,
    pub manual: BTreeMap<String, f32>,
    /// Quantization in native parameter units. Zero means continuous.
    pub steps: BTreeMap<String, f32>,
    pub pose: Pose,
    pub physics: PhysicsSettings,
    /// Active expression file identities for this avatar, also captured in presets.
    pub expressions: BTreeSet<String>,
}
impl RigConfig {
    pub fn from_parameters(parameters: &[RigParameter]) -> Self {
        Self {
            bindings: rig::default_bindings(parameters),
            ..Default::default()
        }
    }
    pub fn capture_pose(&mut self, parameters: &[RigParameter]) {
        self.pose.frozen = parameters.iter().map(|p| (p.id.clone(), p.value)).collect();
        self.pose.mode = PoseMode::Frozen;
    }
    pub fn reset_filters(&mut self) {
        for b in self.bindings.values_mut() {
            b.reset_filter();
        }
    }
    pub fn evaluate(
        &mut self,
        inputs: &Inputs,
        parameters: &mut [RigParameter],
        dt: f32,
        physics: Option<&mut Physics>,
    ) {
        self.evaluate_with_expressions(inputs, parameters, dt, physics, |_, _| {});
    }
    pub fn evaluate_with_expressions(
        &mut self,
        inputs: &Inputs,
        parameters: &mut [RigParameter],
        dt: f32,
        physics: Option<&mut Physics>,
        expressions: impl FnOnce(&mut [RigParameter], &BTreeSet<String>),
    ) {
        if self.pose.mode == PoseMode::Frozen {
            // Freeze FINAL values, not tracking inputs. No breathing, physics or
            // smoothing is evaluated while taking a picture, even if packets change.
            for p in parameters {
                p.value = self
                    .pose
                    .frozen
                    .get(&p.id)
                    .copied()
                    .unwrap_or(p.default)
                    .clamp(p.min, p.max);
            }
            return;
        }
        for p in parameters.iter_mut() {
            p.value = self
                .manual
                .get(&p.id)
                .copied()
                .unwrap_or(p.default)
                .clamp(p.min, p.max);
        }
        rig::apply_bindings(&mut self.bindings, inputs, parameters, dt);
        expressions(parameters, &self.expressions);
        self.apply_holds_and_steps(parameters);
        if let Some(physics) = physics {
            physics.configure(&self.physics);
            physics.update(parameters, dt);
        }
        // A held physics output must stay held, while a held driver still
        // pushes the unheld secondary-motion chains in partial override mode.
        self.apply_holds_and_steps(parameters);
    }
    fn apply_holds_and_steps(&self, parameters: &mut [RigParameter]) {
        for p in parameters {
            if self.pose.mode == PoseMode::Override
                && let Some(&value) = self.pose.held.get(&p.id)
            {
                p.value = value;
            }
            p.value = snap(
                p.value,
                p.min,
                p.max,
                self.steps.get(&p.id).copied().unwrap_or(0.0),
            );
        }
    }
    pub fn validate(&self, parameters: &[RigParameter]) -> Result<()> {
        ensure!(
            parameters.len() <= 4096 && self.bindings.len() <= parameters.len(),
            "Too many rig controls"
        );
        let ids: BTreeSet<_> = parameters.iter().map(|p| p.id.as_str()).collect();
        for (id, b) in &self.bindings {
            ensure!(
                ids.contains(id.as_str()),
                "Preset parameter {id} is absent from this rig"
            );
            ensure!(
                !b.input.is_empty() && b.input.len() <= 128 && b.name.len() <= 256,
                "Invalid input name"
            );
            ensure!(
                [b.input_min, b.input_max, b.output_min, b.output_max]
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 1e6)
                    && (b.input_max - b.input_min).abs() > 1e-6,
                "Invalid input/output range for {id}"
            );
            ensure!(
                b.smoothing_ms.is_finite()
                    && (0.0..=500.0).contains(&b.smoothing_ms)
                    && b.dead_zone.is_finite()
                    && (0.0..=0.49).contains(&b.dead_zone)
                    && b.response_curve.is_finite()
                    && (0.1..=4.0).contains(&b.response_curve),
                "Invalid response settings for {id}"
            );
        }
        for values in [&self.manual, &self.pose.held, &self.pose.frozen] {
            ensure!(values.len() <= parameters.len(), "Too many pose values");
            for (id, v) in values {
                let p = parameters
                    .iter()
                    .find(|p| p.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("Preset parameter {id} is absent"))?;
                ensure!(
                    v.is_finite() && (p.min..=p.max).contains(v),
                    "Pose value for {id} exceeds rig limits"
                );
            }
        }
        if self.pose.mode == PoseMode::Frozen {
            ensure!(
                self.pose.frozen.len() == parameters.len(),
                "A frozen pose must contain every rig parameter"
            );
        }
        ensure!(
            self.steps.len() <= parameters.len(),
            "Too many step settings"
        );
        for (id, v) in &self.steps {
            ensure!(
                ids.contains(id.as_str()) && v.is_finite() && (0.0..=1e6).contains(v),
                "Invalid stepping for {id}"
            );
        }
        self.physics.validate()?;
        ensure!(
            self.expressions.len() <= 256
                && self
                    .expressions
                    .iter()
                    .all(|id| !id.is_empty() && id.len() <= 4096),
            "Invalid active expression list"
        );
        Ok(())
    }
}

pub fn snap(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let value = if value.is_finite() {
        value.clamp(min, max)
    } else {
        min
    };
    if step.is_finite() && step > 0.0 && value > min && value < max {
        (min + ((value - min) / step).round() * step).clamp(min, max)
    } else {
        value
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum PresetKind {
    Movement,
    Pose,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Preset {
    pub name: String,
    pub kind: PresetKind,
    pub rig: RigConfig,
    pub mapping: MappingSettings,
    /// Ctrl+Alt+F1..F11 on Windows. F12 is reserved by Windows debuggers.
    pub hotkey: Option<u8>,
}
impl Preset {
    pub fn validate(&self, parameters: &[RigParameter]) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 120,
            "Preset name must be 1–120 bytes"
        );
        ensure!(
            self.hotkey.is_none_or(|n| (1..=11).contains(&n)),
            "Hotkey must be F1–F11"
        );
        ensure!(
            self.mapping.smoothing_ms.is_finite()
                && (0.0..=500.0).contains(&self.mapping.smoothing_ms)
                && self.mapping.head_gain.is_finite()
                && (0.0..=5.0).contains(&self.mapping.head_gain)
                && self.mapping.mouth_gain.is_finite()
                && (0.0..=5.0).contains(&self.mapping.mouth_gain),
            "Invalid global mapping settings"
        );
        ensure!(
            self.kind != PresetKind::Pose || self.rig.pose.mode == PoseMode::Frozen,
            "Pose presets must freeze the full model"
        );
        self.rig.validate(parameters)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SavedRig {
    pub config: RigConfig,
    pub presets: Vec<Preset>,
    pub global_hotkeys: bool,
    pub expression_hotkeys: BTreeMap<String, crate::shortcuts::Shortcut>,
    pub expression_files: Vec<std::path::PathBuf>,
}
impl SavedRig {
    pub fn validate(&self, parameters: &[RigParameter]) -> Result<()> {
        self.config.validate(parameters)?;
        ensure!(self.presets.len() <= 128, "Maximum 128 presets per model");
        let mut keys = BTreeSet::new();
        for preset in &self.presets {
            preset.validate(parameters)?;
            if let Some(key) = preset.hotkey {
                ensure!(keys.insert(key), "Duplicate preset hotkey F{key}");
            }
        }
        ensure!(
            self.expression_hotkeys.len() <= 256 && self.expression_files.len() <= 256,
            "Too many expression settings"
        );
        for (id, key) in &self.expression_hotkeys {
            ensure!(
                !id.is_empty() && id.len() <= 4096,
                "Invalid expression identity"
            );
            key.validate()?;
            ensure!(
                *key != crate::shortcuts::Shortcut::pose()
                    && !keys
                        .iter()
                        .any(|n| crate::shortcuts::Shortcut::preset(*n) == *key),
                "Expression hotkey conflicts with a preset or the pose shortcut"
            );
        }
        ensure!(
            self.expression_hotkeys
                .values()
                .collect::<BTreeSet<_>>()
                .len()
                == self.expression_hotkeys.len(),
            "Duplicate expression hotkey"
        );
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
pub struct PresetFile {
    pub version: u32,
    pub model_key: String,
    pub preset: Preset,
}
impl PresetFile {
    pub fn decode(bytes: &[u8], model_key: &str, parameters: &[RigParameter]) -> Result<Self> {
        ensure!(bytes.len() <= 2 * 1024 * 1024, "Preset file exceeds 2 MiB");
        let mut file: Self = serde_json::from_slice(bytes)?;
        ensure!(file.version == 1, "Unsupported preset format version");
        ensure!(
            file.model_key == model_key,
            "This preset belongs to a different model"
        );
        file.preset.validate(parameters)?;
        // Importing a file must not unexpectedly register somebody else's keys.
        file.preset.hotkey = None;
        Ok(file)
    }
}

/// A stable model-content identity, not a path or a security/integrity signature.
pub fn model_key(moc: &[u8]) -> String {
    let hash = moc.iter().fold(0xcbf29ce484222325u64, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    format!("moc3:{hash:016x}:{}", moc.len())
}
pub fn preview_parameters(values: Parameters) -> Vec<RigParameter> {
    PARAMETER_SPECS
        .iter()
        .enumerate()
        .map(|(i, (id, min, max))| RigParameter {
            id: (*id).into(),
            min: *min,
            max: *max,
            default: Parameters::default().0[i],
            value: values.0[i],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freeze_is_exact_and_partial_overrides_release() {
        let mut p = preview_parameters(Parameters::default());
        p[0].value = 12.345;
        p[5].value = 0.72;
        let mut config = RigConfig::from_parameters(&p);
        config.capture_pose(&p);
        let held: Vec<_> = p.iter().map(|p| p.value).collect();
        config.steps.insert("ParamAngleX".into(), 5.0);
        for _ in 0..300 {
            config.evaluate(
                &Inputs::from([("ParamAngleX".into(), -30.0)]),
                &mut p,
                1.0 / 60.0,
                None,
            );
            assert_eq!(held, p.iter().map(|p| p.value).collect::<Vec<_>>());
        }
        config.pose.mode = PoseMode::Override;
        config.pose.held.insert("ParamMouthOpenY".into(), 0.4);
        config.evaluate(
            &Inputs::from([("ParamAngleX".into(), -18.0)]),
            &mut p,
            0.016,
            None,
        );
        assert_eq!(p[0].value, -20.0);
        assert_eq!(p[5].value, 0.4);
        config.pose.mode = PoseMode::Live;
        config.evaluate(&Inputs::new(), &mut p, 0.016, None);
        assert_eq!(p[5].value, 0.0);
    }
    #[test]
    fn presets_round_trip_ranges_pose_and_keys_but_not_filter_state() {
        let p = preview_parameters(Parameters::default());
        let mut config = RigConfig::from_parameters(&p);
        let b = config.bindings.get_mut("ParamAngleX").unwrap();
        b.input_min = -12.0;
        b.output_max = 20.0;
        b.response_curve = 2.0;
        b.dead_zone = 0.1;
        b.evaluate(9.0, 0.016);
        config.steps.insert("ParamAngleX".into(), 0.25);
        config.physics.groups.insert(
            "Custom spring".into(),
            crate::physics::GroupSettings {
                response: 0.6,
                strength: 0.8,
                ..Default::default()
            },
        );
        config.capture_pose(&p);
        let saved = SavedRig {
            config: config.clone(),
            presets: vec![Preset {
                name: "Photo".into(),
                kind: PresetKind::Pose,
                rig: config,
                mapping: MappingSettings::default(),
                hotkey: Some(3),
            }],
            global_hotkeys: true,
            ..Default::default()
        };
        let json = serde_json::to_vec(&saved).unwrap();
        let loaded: SavedRig = serde_json::from_slice(&json).unwrap();
        loaded.validate(&p).unwrap();
        assert_eq!(loaded.presets[0].hotkey, Some(3));
        assert_eq!(loaded.config.bindings["ParamAngleX"].input_min, -12.0);
        assert_eq!(loaded.config.steps["ParamAngleX"], 0.25);
        assert_eq!(
            loaded.presets[0].rig.physics.groups["Custom spring"].response,
            0.6
        );
        let file = PresetFile {
            version: 1,
            model_key: "model-a".into(),
            preset: loaded.presets[0].clone(),
        };
        let bytes = serde_json::to_vec(&file).unwrap();
        assert!(PresetFile::decode(&bytes, "model-b", &p).is_err());
        assert_eq!(
            PresetFile::decode(&bytes, "model-a", &p)
                .unwrap()
                .preset
                .hotkey,
            None
        );
        let mut bad = loaded;
        bad.config
            .bindings
            .get_mut("ParamAngleX")
            .unwrap()
            .input_max = -12.0;
        assert!(bad.validate(&p).is_err());
    }
    #[test]
    fn response_curve_dead_zone_and_stepping_keep_endpoints() {
        let mut b = Binding::direct("FaceAngleX", -30.0, 30.0);
        b.dead_zone = 0.1;
        b.response_curve = 2.0;
        assert_eq!(b.evaluate(-30.0, 0.016), -30.0);
        assert_eq!(b.evaluate(30.0, 0.016), 30.0);
        assert!(b.evaluate(2.0, 0.016).abs() < 0.0001);
        assert!(b.evaluate(15.0, 0.016) < 15.0);
        assert_eq!(snap(0.36, 0.0, 1.0, 0.1), 0.4);
        assert_eq!(snap(1.0, 0.0, 1.0, 0.3), 1.0);
        assert_eq!(snap(f32::NAN, -1.0, 1.0, 0.1), -1.0);
    }
}
