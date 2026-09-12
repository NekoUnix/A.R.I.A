//! Model-owned PNG attachment settings and input-trigger state, independent of a renderer.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MAX_ITEMS: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleMode {
    #[default]
    Manual,
    WhileInRange,
    ToggleOnEnter,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalKind {
    #[default]
    Tracking,
    Parameter,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputRule {
    pub mode: RuleMode,
    pub kind: SignalKind,
    pub source: String,
    pub start: f32,
    pub end: f32,
    pub hysteresis: f32,
}
impl Default for InputRule {
    fn default() -> Self {
        Self {
            mode: RuleMode::Manual,
            kind: SignalKind::Tracking,
            source: "MouthOpen".into(),
            start: 0.5,
            end: 1.0,
            hysteresis: 0.05,
        }
    }
}

/// Surface vertices are stable within a model's content-based profile identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pin {
    Surface {
        mesh: usize,
        vertices: [u16; 3],
        weights: [f32; 3],
        angle: f32,
        length: f32,
    },
    Puppet {
        point: [f32; 2],
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    /// Free position, or offset from a pin, in units of the model canvas height.
    pub position: [f32; 2],
    pub height: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub visible: bool,
    /// Last gate result, captured with frozen pose presets.
    pub condition: bool,
    pub behind: bool,
    pub locked: bool,
    pub pin: Option<Pin>,
    pub follow_rotation: bool,
    pub follow_scale: bool,
    pub follow_visibility: bool,
    pub rule: InputRule,
}
impl Default for Item {
    fn default() -> Self {
        Self {
            id: 1,
            name: "PNG item".into(),
            path: PathBuf::new(),
            position: [0.0; 2],
            height: 0.18,
            rotation: 0.0,
            opacity: 1.0,
            visible: true,
            condition: false,
            behind: false,
            locked: false,
            pin: None,
            follow_rotation: true,
            follow_scale: false,
            follow_visibility: true,
            rule: InputRule::default(),
        }
    }
}
pub fn validate(items: &[Item]) -> Result<()> {
    ensure!(
        items.len() <= MAX_ITEMS,
        "Maximum {MAX_ITEMS} PNG items per avatar"
    );
    let mut ids = std::collections::BTreeSet::new();
    for item in items {
        ensure!(
            item.id != 0 && ids.insert(item.id),
            "PNG item IDs must be unique"
        );
        ensure!(
            !item.name.trim().is_empty() && item.name.chars().count() <= 80,
            "PNG toggle needs a name of 1–80 characters"
        );
        ensure!(
            !item.path.as_os_str().is_empty() && item.path.to_string_lossy().len() <= 4096,
            "PNG item path is missing or too long"
        );
        ensure!(
            item.position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 4.0)
                && item.height.is_finite()
                && (0.005..=4.0).contains(&item.height)
                && item.rotation.is_finite()
                && (-180.0..=180.0).contains(&item.rotation)
                && item.opacity.is_finite()
                && (0.0..=1.0).contains(&item.opacity),
            "Invalid PNG item placement"
        );
        let rule = &item.rule;
        ensure!(
            rule.source.len() <= 256
                && !rule.source.trim().is_empty()
                && [rule.start, rule.end, rule.hysteresis]
                    .iter()
                    .all(|v| v.is_finite() && v.abs() <= 1e6)
                && rule.start <= rule.end
                && rule.hysteresis >= 0.0,
            "Invalid PNG input rule"
        );
        match &item.pin {
            Some(Pin::Surface {
                mesh,
                vertices,
                weights,
                angle,
                length,
            }) => {
                ensure!(
                    *mesh < 100_000
                        && vertices[0] != vertices[1]
                        && vertices[1] != vertices[2]
                        && vertices[0] != vertices[2]
                        && weights
                            .iter()
                            .all(|v| v.is_finite() && (-0.001..=1.001).contains(v))
                        && (weights.iter().sum::<f32>() - 1.0).abs() < 0.001
                        && angle.is_finite()
                        && length.is_finite()
                        && *length > 1e-7,
                    "Invalid surface pin"
                );
            }
            Some(Pin::Puppet { point }) => ensure!(
                point.iter().all(|v| v.is_finite() && v.abs() <= 4.0),
                "Invalid puppet pin"
            ),
            None => {}
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct RuleState {
    rule: Option<InputRule>,
    inside: Option<bool>,
}
impl RuleState {
    /// Missing signals clear edge history and hide gated items. A first sample
    /// establishes the baseline; loading a preset never fabricates a rising edge.
    pub fn update(&mut self, item: &mut Item, value: Option<f32>) -> bool {
        if self.rule.as_ref() != Some(&item.rule) {
            self.rule = Some(item.rule.clone());
            self.inside = None;
        }
        if item.rule.mode == RuleMode::Manual {
            return item.visible;
        }
        let Some(value) = value.filter(|v| v.is_finite()) else {
            self.inside = None;
            return item.visible && item.rule.mode == RuleMode::ToggleOnEnter;
        };
        let r = &item.rule;
        let margin = if self.inside == Some(true) {
            r.hysteresis
        } else {
            0.0
        };
        let inside = (r.start - margin..=r.end + margin).contains(&value);
        if r.mode == RuleMode::ToggleOnEnter && self.inside == Some(false) && inside {
            item.visible = !item.visible;
        }
        self.inside = Some(inside);
        item.visible && (r.mode == RuleMode::ToggleOnEnter || inside)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_toggle_uses_real_edges_hysteresis_and_missing_signal_recovery() {
        let mut item = Item {
            visible: false,
            rule: InputRule {
                mode: RuleMode::ToggleOnEnter,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut state = RuleState::default();
        assert!(
            !state.update(&mut item, Some(0.8)),
            "initial high is not a press"
        );
        assert!(!state.update(&mut item, Some(0.2)));
        assert!(state.update(&mut item, Some(0.6)));
        assert!(
            state.update(&mut item, Some(0.48)),
            "hysteresis prevents chatter"
        );
        assert!(state.update(&mut item, Some(0.6)));
        assert!(state.update(&mut item, None));
        assert!(
            state.update(&mut item, Some(0.7)),
            "reconnect must not toggle"
        );
        assert!(state.update(&mut item, Some(0.1)));
        assert!(!state.update(&mut item, Some(0.8)));
        item.rule.mode = RuleMode::WhileInRange;
        item.visible = true;
        assert!(!state.update(&mut item, None));
        assert!(state.update(&mut item, Some(0.7)));
        item.visible = false;
        assert!(
            !state.update(&mut item, Some(0.7)),
            "manual master still works"
        );
    }
    #[test]
    fn item_configs_reject_invalid_geometry_and_round_trip_custom_pins() {
        let mut item = Item {
            path: "custom.png".into(),
            pin: Some(Pin::Surface {
                mesh: 7,
                vertices: [2, 4, 8],
                weights: [0.2, 0.3, 0.5],
                angle: 0.4,
                length: 0.1,
            }),
            ..Default::default()
        };
        validate(&[item.clone()]).unwrap();
        let loaded: Item = serde_json::from_str(&serde_json::to_string(&item).unwrap()).unwrap();
        assert_eq!(item, loaded);
        assert!(validate(&[item.clone(), item.clone()]).is_err());
        item.height = f32::NAN;
        assert!(validate(&[item]).is_err());
    }
}
