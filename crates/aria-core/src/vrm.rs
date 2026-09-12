//! Per-avatar VRM view settings; included in movement and pose presets.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Last rendered secondary pose, carried into screenshot presets independently
/// of camera controls. Node identities are scoped by the avatar's content key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pose {
    pub rotations: std::collections::BTreeMap<usize, [f32; 4]>,
    pub blink: f32,
}
impl Pose {
    pub fn validate(&self) -> Result<()> {
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
