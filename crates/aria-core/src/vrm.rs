//! Per-avatar VRM view settings; included in movement and pose presets.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Last rendered secondary pose, carried into screenshot presets independently
/// of camera controls. Node identities are scoped by the avatar's content key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pose {
    pub rotations: std::collections::BTreeMap<usize, [f32; 4]>,
    /// Sampled additive gesture/idle angles in degrees, separate from editable tracking parameters.
    pub motion: std::collections::BTreeMap<String, [f32; 3]>,
    pub blink: f32,
}
impl Pose {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.motion.len() <= 11
                && self.motion.iter().all(|(id, angles)| !id.is_empty()
                    && id.len() <= 64
                    && angles.iter().all(|v| v.is_finite() && v.abs() <= 360.)),
            "Invalid frozen VRM motion"
        );
        ensure!(
            self.blink.is_finite()
                && (0.0..=1.0).contains(&self.blink)
                && self.rotations.len() <= 4096,
            "Invalid VRM frozen pose"
        );
        for (&node, q) in &self.rotations {
            let length = q.iter().map(|v| v * v).sum::<f32>();
            ensure!(
                node < 4096 && q.iter().all(|v| v.is_finite()) && (0.99..=1.01).contains(&length),
                "Invalid VRM frozen rotation"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub motion: Motion,
    pub yaw: f32,
    pub pitch: f32,
    /// Zero frames the whole body; one frames the head and shoulders.
    pub portrait: f32,
    pub resolution: u32,
    pub light: f32,
    pub outlines: bool,
    pub auto_blink: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            motion: Motion::default(),
            yaw: 0.0,
            pitch: 0.0,
            portrait: 0.0,
            resolution: 1536,
            light: 1.0,
            outlines: true,
            auto_blink: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        self.motion.validate()?;
        ensure!(
            self.yaw.is_finite()
                && self.yaw.abs() <= 180.0
                && self.pitch.is_finite()
                && self.pitch.abs() <= 75.0
                && self.portrait.is_finite()
                && (0.0..=1.0).contains(&self.portrait)
                && self.light.is_finite()
                && (0.25..=2.0).contains(&self.light)
                && [512, 1024, 1536, 2048, 3072, 4096].contains(&self.resolution),
            "Invalid VRM camera or quality settings"
        );
        Ok(())
    }
}

/// Model-owned procedural animation tuning; playing gestures are session state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Motion {
    pub enabled: bool,
    pub sway: f32,
    pub breathing: f32,
    pub arms: f32,
    pub speed: f32,
    pub gesture_strength: f32,
    pub gesture_speed: f32,
    pub gesture_loop: bool,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            enabled: true,
            sway: 0.65,
            breathing: 0.6,
            arms: 0.6,
            speed: 1.,
            gesture_strength: 1.,
            gesture_speed: 1.,
            gesture_loop: false,
        }
    }
}
impl Motion {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [self.sway, self.breathing, self.arms, self.gesture_strength]
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=2.0).contains(&v))
                && [self.speed, self.gesture_speed]
                    .into_iter()
                    .all(|v| v.is_finite() && (0.25..=2.0).contains(&v)),
            "Invalid VRM motion settings"
        );
        Ok(())
    }
    pub fn moving(&self) -> bool {
        self.enabled && (self.sway > 0. || self.breathing > 0. || self.arms > 0.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn motion_migrates_and_frozen_samples_roundtrip_with_validation() {
        let old: Settings = serde_json::from_str("{\"yaw\":25}").unwrap();
        assert!(old.motion.enabled);
        assert_eq!(old.yaw, 25.);
        let mut pose = Pose::default();
        pose.motion.insert("head".into(), [15., 5., 0.]);
        pose.validate().unwrap();
        assert_eq!(
            pose,
            serde_json::from_slice::<Pose>(&serde_json::to_vec(&pose).unwrap()).unwrap()
        );
        pose.motion.insert("head".into(), [f32::INFINITY; 3]);
        assert!(pose.validate().is_err());
        let mut invalid = old;
        invalid.motion.speed = 0.;
        assert!(invalid.validate().is_err());
        invalid.motion.speed = 1.;
        invalid.motion.arms = f32::NAN;
        assert!(invalid.validate().is_err());
    }
}
