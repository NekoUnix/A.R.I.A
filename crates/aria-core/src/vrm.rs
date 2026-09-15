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
    /// GLB bone overrides: missing key = auto, None = disabled. Scoped by avatar key.
    pub bone_map: std::collections::BTreeMap<String, Option<usize>>,
    pub reverse_forward: bool,
    pub motion: Motion,
    pub secondary: Secondary,
    pub yaw: f32,
    pub pitch: f32,
    /// Zero frames the whole body; one frames the head and shoulders.
    pub portrait: f32,
    pub resolution: u32,
    pub light: f32,
    pub lighting: Lighting,
    pub outlines: bool,
    pub auto_blink: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            bone_map: Default::default(),
            reverse_forward: false,
            motion: Motion::default(),
            secondary: Secondary::default(),
            yaw: 0.0,
            pitch: 0.0,
            portrait: 0.0,
            resolution: 1536,
            light: 1.0,
            lighting: Lighting::default(),
            outlines: true,
            auto_blink: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        self.motion.validate()?;
        self.secondary.validate()?;
        self.lighting.validate()?;
        ensure!(
            self.bone_map.len() <= 64
                && self.bone_map.iter().all(|(name, node)| !name.is_empty()
                    && name.len() <= 64
                    && node.is_none_or(|n| n < 4096)),
            "Invalid GLB humanoid bone mapping"
        );
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

/// 3D-only secondary motion. Missing settings use Medium, including old profiles.
/// IDs and manual nodes are scoped by the avatar content key, never shared globally.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Secondary {
    pub auto_detect: bool,
    pub manual_roots: std::collections::BTreeSet<usize>,
    pub tuning: SpringTuning,
    pub groups: std::collections::BTreeMap<String, SpringTuning>,
}
impl Default for Secondary {
    fn default() -> Self {
        Self {
            auto_detect: true,
            manual_roots: Default::default(),
            tuning: Default::default(),
            groups: Default::default(),
        }
    }
}
impl Secondary {
    pub fn validate(&self) -> Result<()> {
        self.tuning.validate()?;
        ensure!(
            self.manual_roots.len() <= 128
                && self.manual_roots.iter().all(|&n| n < 4096)
                && self.groups.len() <= 256,
            "Too many secondary motion groups or invalid root"
        );
        for (id, tuning) in &self.groups {
            ensure!(
                !id.is_empty() && id.len() <= 256,
                "Invalid secondary motion group ID"
            );
            tuning.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SpringTuning {
    /// Additional drag on top of the exported per-joint drag.
    pub damping: f32,
    pub swing: f32,
    pub collisions: bool,
}
impl Default for SpringTuning {
    fn default() -> Self {
        Self {
            damping: 0.15,
            swing: 45.,
            collisions: true,
        }
    }
}
impl SpringTuning {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (0.0..=1.0).contains(&self.damping) && (0.0..=90.0).contains(&self.swing),
            "Invalid secondary motion damping or swing limit"
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
    fn medium_secondary_defaults_and_custom_profile_roundtrip() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(settings.secondary.auto_detect);
        assert_eq!(settings.secondary.tuning.swing, 45.);
        settings.secondary.tuning.damping = 0.6;
        settings.secondary.groups.insert(
            "aria:spring:5".into(),
            SpringTuning {
                swing: 12.,
                ..Default::default()
            },
        );
        settings.secondary.manual_roots.insert(5);
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(settings, restored);
        restored.validate().unwrap();
        settings.secondary.tuning.swing = f32::NAN;
        assert!(settings.validate().is_err());
    }
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

/// Shared per-avatar lighting for 2D artwork and 3D surfaces. Disabled preserves original rendering.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lighting {
    pub enabled: bool,
    pub directional: bool,
    pub color: [f32; 3],
    pub azimuth: f32,
    pub elevation: f32,
    pub ambient: f32,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            enabled: false,
            directional: true,
            color: [1.; 3],
            azimuth: -24.,
            elevation: 35.,
            ambient: 0.35,
        }
    }
}
impl Lighting {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.color.iter().all(|v| (0.0..=1.0).contains(v))
                && (-180.0..=180.0).contains(&self.azimuth)
                && (-90.0..=90.0).contains(&self.elevation)
                && (0.0..=1.0).contains(&self.ambient),
            "Invalid avatar lighting"
        );
        Ok(())
    }
    pub fn direction(&self) -> [f32; 3] {
        let (y, p) = (self.azimuth.to_radians(), self.elevation.to_radians());
        [y.sin() * p.cos(), p.sin(), y.cos() * p.cos()]
    }
    pub fn artwork_color(&self, uv: [f32; 2]) -> [f32; 3] {
        if !self.enabled {
            return [1.; 3];
        }
        let d = self.direction();
        let gradient = if self.directional {
            (0.75 + ((uv[0] - 0.5) * d[0] + (0.5 - uv[1]) * d[1]) * 0.5).clamp(0., 1.)
        } else {
            1.
        };
        let strength = self.ambient + (1. - self.ambient) * gradient;
        self.color.map(|c| c * strength)
    }
}
