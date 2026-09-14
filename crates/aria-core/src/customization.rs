//! Per-avatar appearance overrides. Tracking bindings remain available on release.
use crate::rig::RigParameter;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Style {
    #[default]
    Slider,
    Toggle,
    Choices,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Choice {
    pub name: String,
    pub value: f32,
}
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Control {
    pub label: String,
    pub category: String,
    pub style: Style,
    pub step: f32,
    pub choices: Vec<Choice>,
}
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    /// Only explicitly locked values override tracking, expressions and physics.
    pub values: BTreeMap<String, f32>,
    /// User-curated controls; merely adding a control never changes the model.
    pub controls: BTreeMap<String, Control>,
}
impl Config {
    pub fn apply(&self, parameters: &mut [RigParameter]) {
        for p in parameters {
            if let Some(&value) = self.values.get(&p.id) {
                p.value = value.clamp(p.min, p.max);
            }
        }
    }
    pub fn validate(&self, parameters: &[RigParameter]) -> Result<()> {
        ensure!(
            self.values.len() <= 4096 && self.controls.len() <= 4096,
            "Too many appearance controls"
        );
        let by_id: BTreeMap<_, _> = parameters.iter().map(|p| (p.id.as_str(), p)).collect();
        for (id, value) in &self.values {
            let p = by_id.get(id.as_str()).ok_or_else(|| {
                anyhow::anyhow!("Appearance parameter {id} is absent from this avatar")
            })?;
            ensure!(
                value.is_finite() && (p.min..=p.max).contains(value),
                "Appearance value for {id} exceeds the avatar's limits"
            );
        }
        for (id, control) in &self.controls {
            let p = by_id.get(id.as_str()).ok_or_else(|| {
                anyhow::anyhow!("Appearance control {id} is absent from this avatar")
            })?;
            ensure!(
                control.label.len() <= 256 && control.category.len() <= 120,
                "Appearance labels are too long"
            );
            ensure!(
                control.step.is_finite()
                    && control.step >= 0.
                    && control.step <= (p.max - p.min).max(0.),
                "Invalid appearance step for {id}"
            );
            ensure!(
                control.choices.len() <= 64,
                "Maximum 64 choices per control"
            );
            for choice in &control.choices {
                ensure!(
                    !choice.name.trim().is_empty()
                        && choice.name.len() <= 120
                        && choice.value.is_finite()
                        && (p.min..=p.max).contains(&choice.value),
                    "Invalid appearance choice for {id}"
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_locks_leave_face_tracking_live_and_release_without_losing_bindings() {
        use crate::{
            movement::RigConfig,
            rig::{Binding, Inputs},
        };
        let mut parameters = vec![
            RigParameter {
                id: "Outfit".into(),
                min: 0.,
                max: 1.,
                default: 0.,
                value: 0.,
            },
            RigParameter {
                id: "ParamAngleX".into(),
                min: -30.,
                max: 30.,
                default: 0.,
                value: 0.,
            },
        ];
        let mut rig = RigConfig::from_parameters(&parameters);
        rig.bindings
            .insert("Outfit".into(), Binding::direct("OutfitInput", 0., 1.));
        rig.customization.values.insert("Outfit".into(), 1.);
        for angle in [12., -18.] {
            rig.evaluate_with_expressions(
                &Inputs::from([("ParamAngleX".into(), angle), ("OutfitInput".into(), 0.)]),
                &mut parameters,
                0.016,
                None,
                |p, _| p[0].value = 0.2,
            );
            assert_eq!(
                parameters[0].value, 1.,
                "Appearance beats expressions and its own tracking binding"
            );
            assert!(
                (parameters[1].value - angle).abs() < 1e-5,
                "Face tracking remains live"
            );
        }
        rig.customization.values.clear();
        rig.evaluate(
            &Inputs::from([("OutfitInput".into(), 0.4)]),
            &mut parameters,
            0.016,
            None,
        );
        assert!((parameters[0].value - 0.4).abs() < 1e-5);
        assert_eq!(rig.bindings["Outfit"].input, "OutfitInput");
        rig.capture_pose(&parameters);
        let frozen_head = parameters[1].value;
        rig.customization.values.insert("Outfit".into(), 1.);
        rig.evaluate(
            &Inputs::from([("ParamAngleX".into(), 30.)]),
            &mut parameters,
            0.016,
            None,
        );
        assert_eq!(parameters[0].value, 1.);
        assert_eq!(parameters[1].value, frozen_head);
        let restored: RigConfig =
            serde_json::from_slice(&serde_json::to_vec(&rig).unwrap()).unwrap();
        restored.validate(&parameters).unwrap();
        assert_eq!(restored.customization, rig.customization);
        let old: RigConfig = serde_json::from_str("{}").unwrap();
        assert!(old.customization.values.is_empty());
    }
    #[test]
    fn bounds_and_foreign_model_values_are_rejected() {
        let p = RigParameter {
            id: "Outfit".into(),
            min: -1.,
            max: 3.,
            default: 0.,
            value: 0.,
        };
        let mut config = Config::default();
        config.values.insert(p.id.clone(), 3.);
        config.validate(std::slice::from_ref(&p)).unwrap();
        assert!(config.validate(&[]).is_err());
        config.values.insert(p.id.clone(), 3.1);
        assert!(config.validate(std::slice::from_ref(&p)).is_err());
        config.values.clear();
        config.controls.insert(
            p.id.clone(),
            Control {
                step: f32::NAN,
                ..Default::default()
            },
        );
        assert!(config.validate(&[p]).is_err());
    }
}
