//! EXPERIMENTAL: Bounded, model-owned VTube Studio customization import. This is a compatibility
//! adapter, not the VTS runtime or its plugin/account configuration.
mod import;
#[cfg(test)]
mod tests;
use crate::shortcuts::Shortcut;
use anyhow::{Result, ensure};
pub use import::{Imported, parse, shortcut};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Placement {
    pub position: [f32; 2],
    pub zoom: f32,
    pub rotation: f32,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            position: [0.; 2],
            zoom: 1.,
            rotation: 0.,
        }
    }
}
impl Placement {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.position
                .iter()
                .all(|v| v.is_finite() && (-1.0..=1.0).contains(v))
                && self.zoom.is_finite()
                && (0.25..=3.0).contains(&self.zoom),
            "Invalid imported model framing"
        );
        ensure!(
            self.rotation.is_finite() && self.rotation.abs() <= 180.,
            "Invalid model rotation"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Expression(String),
    ClearExpressions,
    Animation {
        file: String,
        hold_last: bool,
    },
    MoveModel(Placement),
    HideItems,
    TogglePhysics,
    ItemScene {
        file: String,
        random: bool,
    },
    /// Retained as an in-ARIA repair task; never executed by another application.
    NeedsRepair {
        kind: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hotkey {
    pub id: String,
    pub name: String,
    pub action: Action,
    pub shortcut: Option<Shortcut>,
    pub screen_button: Option<i32>,
    pub enabled: bool,
    pub global: bool,
    pub release: bool,
    pub seconds: Option<f32>,
    pub fade: f32,
    #[serde(default)]
    pub chord: Vec<u16>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Sway {
    pub enabled: bool,
    pub amount: [f32; 3],
    pub smoothing: [f32; 3],
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub name: String,
    pub source_model: String,
    pub keyboard_enabled: bool,
    pub screen_enabled: bool,
    pub actions: Vec<Hotkey>,
    pub idle: Option<String>,
    pub lost_idle: Option<String>,
    pub idle_enabled: bool,
    pub lost_idle_delay: f32,
    pub placement: Placement,
    pub labels: BTreeMap<String, String>,
    pub expression_fades: BTreeMap<String, f32>,
    pub notes: Vec<String>,
    pub sway: Sway,
    pub physics_imported: bool,
    pub source_id: String,
    /// User-approved asset root, never taken from absolute paths in a config.
    pub assets_root: Option<std::path::PathBuf>,
    pub scenes: BTreeMap<String, Vec<crate::items::Item>>,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.name.len() <= 256
                && self.source_model.len() <= 4096
                && self.actions.len() <= 512
                && self.labels.len() <= 4096
                && self.notes.len() <= 2048
                && self.expression_fades.len() <= 256,
            "Too many VTube Studio customizations"
        );
        self.placement.validate()?;
        ensure!(
            self.placement.rotation.is_finite()
                && self.placement.rotation.abs() <= 180.
                && self
                    .sway
                    .amount
                    .iter()
                    .chain(&self.sway.smoothing)
                    .all(|v| v.is_finite() && (0.0..=100.).contains(v)),
            "Invalid sway or rotation"
        );
        ensure!(
            self.scenes.len() <= 128 && self.source_id.len() <= 256,
            "Too many item scenes"
        );
        for (name, items) in &self.scenes {
            valid_reference(name)?;
            crate::items::validate(items)?;
        }
        ensure!(
            self.lost_idle_delay.is_finite() && (0.0..=3600.).contains(&self.lost_idle_delay),
            "Invalid idle delay"
        );
        for path in self.idle.iter().chain(self.lost_idle.iter()) {
            valid_reference(path)?;
        }
        let mut ids = BTreeSet::new();
        for h in &self.actions {
            ensure!(
                !h.id.is_empty()
                    && h.id.len() <= 256
                    && ids.insert(&h.id)
                    && !h.name.trim().is_empty()
                    && h.name.len() <= 256,
                "Invalid or duplicate imported action"
            );
            if let Some(key) = h.shortcut {
                key.validate()?;
            }
            ensure!(
                h.chord.len() <= 3 && h.chord.iter().all(|k| (1..=254).contains(k)),
                "Invalid imported key chord"
            );
            ensure!(
                h.fade.is_finite()
                    && (0.0..=60.).contains(&h.fade)
                    && h.seconds
                        .is_none_or(|v| v.is_finite() && (0.01..=3600.).contains(&v))
                    && h.screen_button.is_none_or(|v| (0..=128).contains(&v)),
                "Invalid action duration/button"
            );
            match &h.action {
                Action::Expression(id)
                | Action::Animation { file: id, .. }
                | Action::ItemScene { file: id, .. } => valid_reference(id)?,
                Action::NeedsRepair { kind } => {
                    ensure!(
                        !kind.is_empty() && kind.len() <= 256,
                        "Invalid repair action"
                    )
                }
                Action::MoveModel(p) => p.validate()?,
                _ => {}
            }
        }
        for (id, name) in &self.labels {
            ensure!(
                !id.is_empty() && id.len() <= 256 && name.len() <= 256,
                "Invalid custom parameter label"
            );
        }
        for (id, fade) in &self.expression_fades {
            valid_reference(id)?;
            ensure!(
                fade.is_finite() && (0.0..=60.).contains(fade),
                "Invalid expression fade"
            );
        }
        ensure!(
            self.notes.iter().all(|n| n.len() <= 4096),
            "Import notice is too long"
        );
        Ok(())
    }
}
pub fn valid_reference(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 4096
            && !value.starts_with(['/', '\\'])
            && !value.contains([':', '\0'])
            && value
                .replace('\\', "/")
                .split('/')
                .all(|p| p != ".." && !p.is_empty()),
        "Asset reference must stay inside this model's folder"
    );
    Ok(())
}

/// Transient action state. Model changes drop it; frozen poses pause its clock.
pub struct Runtime {
    pub elapsed: f32,
    deadlines: BTreeMap<String, (String, f32)>,
    held: BTreeMap<String, String>,
    last_screen: i32,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            elapsed: 0.,
            deadlines: Default::default(),
            held: Default::default(),
            last_screen: -1,
        }
    }
}
impl Runtime {
    pub fn reset(&mut self) {
        *self = Self {
            last_screen: -1,
            ..Default::default()
        };
    }
    pub fn screen_actions(&mut self, config: &Config, button: i32) -> Vec<String> {
        let changed = button != self.last_screen;
        self.last_screen = button;
        if !config.screen_enabled || !changed || button < 0 {
            return Vec::new();
        }
        config
            .actions
            .iter()
            .filter(|h| h.enabled && h.screen_button == Some(button))
            .map(|h| h.id.clone())
            .collect()
    }
    pub fn expression(&mut self, h: &Hotkey, active: &mut BTreeSet<String>) -> bool {
        let Action::Expression(id) = &h.action else {
            return false;
        };
        let enabled = if h.release {
            active.insert(id.clone());
            true
        } else if active.remove(id) {
            false
        } else {
            active.insert(id.clone());
            true
        };
        self.deadlines.retain(|_, (expression, _)| expression != id);
        self.held.retain(|_, expression| expression != id);
        if enabled {
            if let Some(seconds) = h.seconds {
                self.deadlines
                    .insert(h.id.clone(), (id.clone(), self.elapsed + seconds));
            }
            if h.release {
                self.held.insert(h.id.clone(), id.clone());
            }
        }
        enabled
    }
    pub fn release(&mut self, id: &str, active: &mut BTreeSet<String>) -> bool {
        self.deadlines.remove(id);
        self.held
            .remove(id)
            .is_some_and(|expression| active.remove(&expression))
    }
    pub fn held_ids(&self) -> impl Iterator<Item = &str> {
        self.held.keys().map(String::as_str)
    }
    pub fn tick(&mut self, dt: f32, frozen: bool, active: &mut BTreeSet<String>) -> bool {
        if frozen {
            return false;
        }
        if dt.is_finite() {
            self.elapsed += dt.clamp(0., 0.25);
        }
        let mut changed = false;
        self.deadlines.retain(|_, (id, at)| {
            if *at <= self.elapsed {
                changed |= active.remove(id);
                false
            } else {
                true
            }
        });
        changed
    }
}
