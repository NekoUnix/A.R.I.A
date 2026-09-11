//! Cubism expression values layered over fresh tracking values each frame.
use crate::rig::RigParameter;
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Blend {
    #[default]
    Add,
    Multiply,
    Overwrite,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExpressionParameter {
    pub id: String,
    pub value: f32,
    #[serde(default)]
    pub blend: Blend,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Expression {
    #[serde(rename = "Type")]
    kind: String,
    #[serde(default = "default_fade")]
    pub fade_in_time: f32,
    #[serde(default = "default_fade")]
    pub fade_out_time: f32,
    pub parameters: Vec<ExpressionParameter>,
}
fn default_fade() -> f32 {
    1.0
}
impl Expression {
    pub fn load(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= 1024 * 1024, "Expression exceeds 1 MiB");
        let value: Self = serde_json::from_slice(bytes)?;
        ensure!(
            value.kind == "Live2D Expression",
            "Expected a Live2D Expression file"
        );
        ensure!(
            value.parameters.len() <= 4096,
            "Too many expression parameters"
        );
        ensure!(
            [value.fade_in_time, value.fade_out_time]
                .iter()
                .all(|v| v.is_finite() && (0.0..=3600.0).contains(v)),
            "Expression fade times must be between 0 and 3600 seconds"
        );
        let mut ids = BTreeSet::new();
        for p in &value.parameters {
            ensure!(
                !p.id.is_empty() && p.id.len() <= 256 && ids.insert(&p.id),
                "Invalid or duplicate expression parameter ID"
            );
            ensure!(
                p.value.is_finite() && p.value.abs() <= 1e6,
                "Invalid expression value for {}",
                p.id
            );
        }
        Ok(value)
    }
    pub fn missing_parameters(&self, parameters: &[RigParameter]) -> Vec<String> {
        self.parameters
            .iter()
            .filter(|p| !parameters.iter().any(|v| v.id == p.id))
            .map(|p| p.id.clone())
            .collect()
    }
    pub fn apply(&self, parameters: &mut [RigParameter], weight: f32) {
        for input in &self.parameters {
            if let Some(p) = parameters.iter_mut().find(|p| p.id == input.id) {
                p.value = match input.blend {
                    Blend::Add => p.value + input.value * weight,
                    Blend::Multiply => p.value * (1.0 + (input.value - 1.0) * weight),
                    Blend::Overwrite => p.value * (1.0 - weight) + input.value * weight,
                }
                .clamp(p.min, p.max);
            }
        }
    }
}

#[derive(Default)]
struct Fade {
    enabled: bool,
    from: f32,
    weight: f32,
    elapsed: f32,
}
#[derive(Default)]
pub struct ExpressionPlayer {
    fades: BTreeMap<String, Fade>,
}
impl ExpressionPlayer {
    /// Caller supplies a stable layer order. Fade state is transient and per avatar.
    /// Interrupted fades start from their current weight, so rapid toggles do not pop.
    pub fn update<'a>(
        &mut self,
        expressions: impl IntoIterator<Item = (&'a str, &'a Expression)>,
        enabled: &BTreeSet<String>,
        parameters: &mut [RigParameter],
        dt: f32,
    ) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        for (id, expression) in expressions {
            let fade = self.fades.entry(id.into()).or_default();
            let target = enabled.contains(id);
            if target != fade.enabled {
                fade.enabled = target;
                fade.from = fade.weight;
                fade.elapsed = 0.0;
            }
            let seconds = if target {
                expression.fade_in_time
            } else {
                expression.fade_out_time
            };
            fade.elapsed = (fade.elapsed + dt).min(seconds);
            let t = if seconds == 0.0 {
                1.0
            } else {
                fade.elapsed / seconds
            };
            let eased = 0.5 - 0.5 * (t * std::f32::consts::PI).cos();
            fade.weight = fade.from + (f32::from(target) - fade.from) * eased;
            if fade.weight > 0.0 {
                expression.apply(parameters, fade.weight);
            }
        }
    }
    pub fn weight(&self, id: &str) -> f32 {
        self.fades.get(id).map_or(0.0, |f| f.weight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Parameters,
        movement::{PoseMode, RigConfig, preview_parameters},
        rig::Inputs,
    };
    fn expression(blend: &str, value: f32) -> Expression {
        Expression::load(
            serde_json::json!({"Type":"Live2D Expression", "Parameters":[
                {"Id":"ParamAngleX", "Value":value, "Blend":blend}
            ]})
            .to_string()
            .as_bytes(),
        )
        .unwrap()
    }
    #[test]
    fn cubism_blend_math_and_bounds() {
        let mut p = preview_parameters(Parameters::default());
        for (blend, value, expected) in [
            ("Add", 8.0, 14.0),
            ("Multiply", 2.0, 15.0),
            ("Overwrite", 20.0, 15.0),
        ] {
            p[0].value = 10.0;
            expression(blend, value).apply(&mut p, 0.5);
            assert_eq!(p[0].value, expected);
        }
        expression("Add", 100.0).apply(&mut p, 1.0);
        assert_eq!(p[0].value, p[0].max);
    }
    #[test]
    fn fades_reverse_without_popping_and_never_accumulate_or_disturb_frozen_poses() {
        let mut p = preview_parameters(Parameters::default());
        let mut rig = RigConfig::from_parameters(&p);
        let expr = expression("Add", 10.0);
        let mut player = ExpressionPlayer::default();
        rig.expressions.insert("smile".into());
        for _ in 0..120 {
            rig.evaluate_with_expressions(&Inputs::new(), &mut p, 1.0 / 60.0, None, |p, active| {
                player.update([("smile", &expr)], active, p, 1.0 / 60.0);
            });
        }
        assert_eq!(p[0].value, 10.0);
        rig.capture_pose(&p);
        rig.expressions.clear();
        rig.evaluate_with_expressions(&Inputs::new(), &mut p, 0.1, None, |_, _| {
            panic!("Frozen expression advanced")
        });
        assert_eq!(p[0].value, 10.0);
        rig.pose.mode = PoseMode::Live;
        player.update([("smile", &expr)], &rig.expressions, &mut p, 0.1);
        let falling = player.weight("smile");
        assert!(falling < 1.0 && falling > 0.9);
        rig.expressions.insert("smile".into());
        player.update([("smile", &expr)], &rig.expressions, &mut p, 0.0);
        assert_eq!(falling, player.weight("smile"));
        rig.expressions.clear();
        for _ in 0..120 {
            rig.evaluate_with_expressions(&Inputs::new(), &mut p, 1.0 / 60.0, None, |p, active| {
                player.update([("smile", &expr)], active, p, 1.0 / 60.0);
            });
        }
        assert_eq!(p[0].value, 0.0);
        rig.pose.mode = PoseMode::Override;
        rig.pose.held.insert(p[0].id.clone(), -5.0);
        rig.expressions.insert("smile".into());
        rig.evaluate_with_expressions(&Inputs::new(), &mut p, 0.1, None, |p, _| expr.apply(p, 1.0));
        assert_eq!(p[0].value, -5.0);
    }
    #[test]
    fn defaults_missing_targets_and_invalid_files() {
        let bytes = br#"{"Type":"Live2D Expression","Parameters":[{"Id":"custom","Value":1}]}"#;
        let expr = Expression::load(bytes).unwrap();
        assert_eq!(expr.fade_in_time, 1.0);
        assert_eq!(expr.parameters[0].blend, Blend::Add);
        assert_eq!(expr.missing_parameters(&[]), ["custom"]);
        for bad in [
            "{}",
            r#"{"Type":"Other","Parameters":[]}"#,
            r#"{"Type":"Live2D Expression","FadeInTime":-1,"Parameters":[]}"#,
            r#"{"Type":"Live2D Expression","Parameters":[{"Id":"a","Value":1,"Blend":"Unknown"}]}"#,
        ] {
            assert!(Expression::load(bad.as_bytes()).is_err());
        }
    }
}
