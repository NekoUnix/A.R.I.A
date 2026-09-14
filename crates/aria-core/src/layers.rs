//! Reversible ArtMesh visibility, scoped to an avatar's content identity.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub model_opacity: f32,
    pub colors: BTreeMap<String, Colors>,
    pub opacity: BTreeMap<String, f32>,
    pub groups: Vec<Group>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Colors {
    pub multiply: [f32; 4],
    pub screen: [f32; 4],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub name: String,
    pub layers: BTreeSet<String>,
    pub opacity: f32,
    pub active: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            model_opacity: 1.,
            colors: Default::default(),
            opacity: Default::default(),
            groups: Default::default(),
        }
    }
}
impl Config {
    /// Overlaps use the most transparent setting, never multiply unexpectedly.
    pub fn opacity(&self, id: &str) -> f32 {
        self.model_opacity
            * self
                .groups
                .iter()
                .filter(|g| g.active && g.layers.contains(id))
                .fold(self.opacity.get(id).copied().unwrap_or(1.), |v, g| {
                    v.min(g.opacity)
                })
    }
    pub fn toggle(&mut self, id: u64) {
        if let Some(group) = self.groups.iter_mut().find(|g| g.id == id) {
            group.active = !group.active;
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.model_opacity.is_finite() && (0.0..=1.0).contains(&self.model_opacity),
            "Invalid model opacity"
        );
        let valid_id = |id: &str| !id.is_empty() && id.len() <= 256;
        let valid_opacity = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        ensure!(
            self.colors.len() <= 8192
                && self.colors.iter().all(|(id, c)| valid_id(id)
                    && c.multiply
                        .iter()
                        .chain(c.screen.iter())
                        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))),
            "Invalid mesh colors"
        );
        ensure!(
            self.opacity.len() <= 8192 && self.groups.len() <= 128,
            "Too many Live2D layer settings"
        );
        ensure!(
            self.opacity
                .iter()
                .all(|(id, &v)| valid_id(id) && valid_opacity(v)),
            "Invalid layer opacity"
        );
        let mut ids = BTreeSet::new();
        for group in &self.groups {
            ensure!(
                group.id > 0
                    && ids.insert(group.id)
                    && !group.name.trim().is_empty()
                    && group.name.len() <= 120
                    && !group.layers.is_empty()
                    && group.layers.len() <= 8192
                    && group.layers.iter().all(|id| valid_id(id))
                    && valid_opacity(group.opacity),
                "Invalid Live2D layer group"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlapping_groups_toggle_restore_and_round_trip() {
        let mut config = Config::default();
        config.opacity.insert("Hair".into(), 0.8);
        for (id, opacity) in [(1, 0.5), (2, 0.0)] {
            config.groups.push(Group {
                id,
                name: format!("Look {id}"),
                layers: BTreeSet::from(["Hair".into()]),
                opacity,
                active: true,
            });
        }
        config.validate().unwrap();
        assert_eq!(config.opacity("Hair"), 0.);
        config.toggle(2);
        assert_eq!(config.opacity("Hair"), 0.5);
        config.toggle(1);
        assert_eq!(config.opacity("Hair"), 0.8);
        assert_eq!(config.opacity("Eyes"), 1.);
        let restored: Config =
            serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        assert_eq!(config, restored);
        config.groups[0].opacity = f32::NAN;
        assert!(config.validate().is_err());
    }
}
