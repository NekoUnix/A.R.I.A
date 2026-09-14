//! Model-owned attachment settings and input-trigger state, independent of a renderer.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const MAX_ITEMS: usize = 32;
pub const MAX_MODEL_ITEMS: usize = 4;

pub fn is_model(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("moc3"))
        || path.file_name().is_some_and(|n| {
            n.to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".model3.json")
        })
}

/// An object's controls are separate from the main avatar's rig and profile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelSettings {
    pub textures: Vec<PathBuf>,
    pub animate: bool,
    pub physics: bool,
    pub parameters: BTreeMap<String, f32>,
    /// Latest final values, so a parent pose preset can freeze animated objects too.
    pub snapshot: BTreeMap<String, f32>,
}
impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            textures: Vec::new(),
            animate: true,
            physics: true,
            parameters: BTreeMap::new(),
            snapshot: BTreeMap::new(),
        }
    }
}

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
    VrmSurface {
        geometry: usize,
        vertices: [u32; 3],
        weights: [f32; 3],
        angle: f32,
        length: f32,
    },
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
    /// Import-owned scene membership prevents toggles from hiding unrelated objects.
    pub vts_scene: Option<String>,
    pub vts_slot: u16,
    pub animation_fps: Option<f32>,
    pub flip: bool,
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub model: ModelSettings,
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
            vts_scene: None,
            vts_slot: 0,
            animation_fps: None,
            flip: false,
            id: 1,
            name: "Stage object".into(),
            path: PathBuf::new(),
            model: ModelSettings::default(),
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
        "Maximum {MAX_ITEMS} stage items per avatar"
    );
    ensure!(
        items.iter().filter(|i| is_model(&i.path)).count() <= MAX_MODEL_ITEMS,
        "Maximum {MAX_MODEL_ITEMS} Live2D objects per avatar"
    );
    let mut ids = std::collections::BTreeSet::new();
    for item in items {
        ensure!(
            item.animation_fps
                .is_none_or(|fps| fps.is_finite() && (0.0..=240.).contains(&fps)),
            "Invalid item frame rate"
        );
        ensure!(
            item.vts_scene.as_ref().is_none_or(|s| s.len() <= 4096),
            "Invalid item scene identity"
        );
        ensure!(
            item.model.textures.len() <= 32
                && item
                    .model
                    .textures
                    .iter()
                    .all(|p| !p.as_os_str().is_empty() && p.to_string_lossy().len() <= 4096),
            "Invalid object texture list"
        );
        for values in [&item.model.parameters, &item.model.snapshot] {
            ensure!(
                values.len() <= 8192
                    && values.iter().all(|(id, v)| !id.is_empty()
                        && id.len() <= 256
                        && v.is_finite()
                        && v.abs() <= 1e6),
                "Invalid Live2D object parameter values"
            );
        }
        ensure!(
            item.id != 0 && ids.insert(item.id),
            "Stage object IDs must be unique"
        );
        ensure!(
            !item.name.trim().is_empty() && item.name.chars().count() <= 80,
            "Object toggle needs a name of 1–80 characters"
        );
        ensure!(
            !item.path.as_os_str().is_empty() && item.path.to_string_lossy().len() <= 4096,
            "Stage object path is missing or too long"
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
            "Invalid Stage object placement"
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
            "Invalid Object input rule"
        );
        match &item.pin {
            Some(Pin::VrmSurface {
                geometry,
                vertices,
                weights,
                angle,
                length,
            }) => {
                ensure!(
                    *geometry < 100_000
                        && vertices.iter().all(|v| *v < 2_000_000)
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
                    "Invalid VRM surface pin"
                );
            }
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
    fn live2d_items_migrate_and_validate_independent_settings() {
        let old: Item = serde_json::from_str(r#"{"path":"old.png"}"#).unwrap();
        assert_eq!(old.model, ModelSettings::default());
        let mut items: Vec<Item> = (1..=MAX_MODEL_ITEMS as u64)
            .map(|id| Item {
                id,
                path: "prop.moc3".into(),
                ..Default::default()
            })
            .collect();
        items[0]
            .model
            .parameters
            .insert("custom-parameter".into(), 0.6);
        items[0].model.textures.push("atlas.png".into());
        items[0]
            .model
            .snapshot
            .insert("other-parameter".into(), 0.4);
        validate(&items).unwrap();
        assert_eq!(
            items,
            serde_json::from_slice::<Vec<Item>>(&serde_json::to_vec(&items).unwrap()).unwrap()
        );
        let mut extra = items[0].clone();
        extra.id = 5;
        items.push(extra);
        assert!(validate(&items).is_err());
        items.pop();
        items[0]
            .model
            .parameters
            .insert("bad".into(), f32::INFINITY);
        assert!(validate(&items).is_err());
    }
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
