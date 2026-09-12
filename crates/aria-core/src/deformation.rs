//! Reversible 2D impact response. Source meshes and tracking parameters stay untouched.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Response {
    pub enabled: bool,
    pub depth: f32,
    pub radius: f32,
    pub squash: f32,
    pub hold: f32,
    pub recovery: f32,
    pub elasticity: f32,
    pub shading: f32,
}
impl Default for Response {
    fn default() -> Self {
        Self {
            enabled: false,
            depth: 0.4,
            radius: 0.12,
            squash: 0.25,
            hold: 0.08,
            recovery: 0.8,
            elasticity: 0.35,
            shading: 0.25,
        }
    }
}
impl Response {
    pub fn validate(&self, object: bool) -> Result<()> {
        for (value, min, max) in [
            (self.depth, 0.0, 1.0),
            (
                self.radius,
                if object { 0.1 } else { 0.02 },
                if object { 1.5 } else { 0.6 },
            ),
            (self.squash, 0.0, 1.0),
            (self.hold, 0.0, 10.0),
            (self.recovery, 0.05, 10.0),
            (self.elasticity, 0.0, 1.0),
            (self.shading, 0.0, 1.0),
        ] {
            ensure!(
                value.is_finite() && (min..=max).contains(&value),
                "Deformation setting outside its supported range"
            );
        }
        Ok(())
    }
    pub fn duration(&self) -> f32 {
        self.hold + self.recovery
    }
    pub fn gain(&self, age: f32) -> f32 {
        if !self.enabled || !age.is_finite() || age < 0.0 || age >= self.duration() {
            return 0.0;
        }
        if age <= self.hold {
            return 1.0;
        }
        let t = (age - self.hold) / self.recovery;
        (1.0 - t).powi(2)
            * (1.0 - self.elasticity + self.elasticity * (t * std::f32::consts::TAU * 1.5).cos())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub avatar: Response,
    pub object: Response,
    pub speed_sensitive: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            avatar: Response::default(),
            object: Response {
                radius: 0.8,
                squash: 0.5,
                ..Default::default()
            },
            speed_sensitive: false,
        }
    }
}
impl Settings {
    pub fn gentle() -> Self {
        let mut settings = Self::default();
        settings.avatar.enabled = true;
        settings.object.enabled = true;
        settings
    }
    pub fn validate(&self) -> Result<()> {
        self.avatar.validate(false)?;
        self.object.validate(true)
    }
}

/// Coordinates can be pixels or normalized canvas units; radius uses the same units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Field {
    pub center: [f32; 2],
    pub direction: [f32; 2],
    pub radius: f32,
    pub depth: f32,
    pub squash: f32,
    pub shading: f32,
}
impl Field {
    pub fn new(center: [f32; 2], direction: [f32; 2], response: &Response, gain: f32) -> Self {
        Self {
            center,
            direction,
            radius: response.radius,
            depth: (response.depth * gain).clamp(-1.0, 1.0),
            squash: (response.squash * gain).clamp(-1.0, 1.0),
            shading: (response.shading * gain.abs()).clamp(0.0, 1.0),
        }
    }
    fn influence(&self, point: [f32; 2]) -> ([f32; 2], f32) {
        let x = (point[0] - self.center[0]) / self.radius;
        let y = (point[1] - self.center[1]) / self.radius;
        let r2 = x * x + y * y;
        if r2 >= 1.0 {
            return ([0.0; 2], 1.0);
        }
        let weight = (1.0 - r2).powi(3);
        let along = x * self.direction[0] + y * self.direction[1];
        let across = -x * self.direction[1] + y * self.direction[0];
        let push = self.depth * 0.32 - along * self.squash * 0.28;
        let bulge = across * self.squash * 0.12;
        (
            [
                (self.direction[0] * push - self.direction[1] * bulge) * weight * self.radius,
                (self.direction[1] * push + self.direction[0] * bulge) * weight * self.radius,
            ],
            1.0 - self.shading * 0.45 * weight,
        )
    }
}
pub fn direction(from: [f32; 2], to: [f32; 2]) -> [f32; 2] {
    let delta = [to[0] - from[0], to[1] - from[1]];
    let length = delta[0].hypot(delta[1]);
    if length < 0.00001 {
        [0.0, 1.0]
    } else {
        [delta[0] / length, delta[1] / length]
    }
}
pub fn apply(point: [f32; 2], fields: &[Field]) -> ([f32; 2], f32) {
    let mut delta = [0.0_f32; 2];
    let mut shade = 1.0_f32;
    let mut limit = 0.0_f32;
    for field in fields {
        // Compose local warps instead of summing their gradients. Repeated hits
        // then compress smoothly instead of folding a shared texture over itself.
        let (shift, light) = field.influence([point[0] + delta[0], point[1] + delta[1]]);
        delta[0] += shift[0];
        delta[1] += shift[1];
        shade *= light;
        if shift != [0.0; 2] {
            limit = limit.max(field.radius * 0.45);
        }
    }
    let length = delta[0].hypot(delta[1]);
    let factor = if length > limit { limit / length } else { 1.0 };
    (
        [point[0] + delta[0] * factor, point[1] + delta[1] * factor],
        shade.max(0.5),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn impacts_are_local_directional_bounded_and_recover_exactly() {
        let response = Response {
            enabled: true,
            hold: 0.2,
            recovery: 1.0,
            ..Default::default()
        };
        assert_eq!(response.gain(0.0), 1.0);
        assert_eq!(response.gain(0.2), 1.0);
        assert_eq!(response.gain(1.2), 0.0);
        assert_eq!(Response::default().gain(0.0), 0.0);
        let field = Field::new([0.0; 2], [1.0, 0.0], &response, 1.0);
        assert!(apply([0.0; 2], &[field]).0[0] > 0.0);
        assert_eq!(apply([0.0, 1.0], &[field]), ([0.0, 1.0], 1.0));
        let (p, shade) = apply([0.0; 2], &[field; 24]);
        assert!(p[0] <= response.radius * 0.45 + 1e-6 && shade >= 0.5);
        let recovered = Field::new([0.0; 2], [1.0, 0.0], &response, response.gain(1.2));
        assert_eq!(apply([0.03, 0.02], &[recovered]), ([0.03, 0.02], 1.0));
    }
    #[test]
    fn settings_migrate_and_reject_corrupt_values() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert!(!settings.avatar.enabled && !settings.object.enabled);
        settings.validate().unwrap();
        settings.avatar.radius = f32::NAN;
        assert!(settings.validate().is_err());
        let settings = Settings::gentle();
        assert_eq!(
            settings,
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap()
        );
    }
    #[test]
    fn maximum_overlapping_dents_preserve_local_mesh_winding() {
        let response = Response {
            enabled: true,
            radius: 0.2,
            depth: 1.0,
            squash: 1.0,
            ..Default::default()
        };
        let fields = [Field::new([0.0; 2], [1.0, 0.0], &response, 1.0); 24];
        for y in -30..30 {
            for x in -30..30 {
                let p = [x as f32 * 0.01, y as f32 * 0.01];
                let a = apply(p, &fields).0;
                let b = apply([p[0] + 0.001, p[1]], &fields).0;
                let c = apply([p[0], p[1] + 0.001], &fields).0;
                assert!(
                    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]) >= -1e-9,
                    "fold at {p:?}"
                );
            }
        }
    }
}
