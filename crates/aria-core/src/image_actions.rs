//! Per-avatar image states and deterministic transitions. No renderer or audio device required.
use crate::{
    items::{InputRule, SignalKind},
    rig::{Inputs, RigParameter},
    shortcuts::Shortcut,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    #[default]
    Idle,
    Talking,
    Quiet,
    Blink,
    Input,
    Manual,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transition {
    Cut,
    #[default]
    Crossfade,
    FadeThrough,
    Slide,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Animation {
    #[default]
    None,
    Shake,
    Jump,
    Blip,
    Pulse,
    Wobble,
    Bob,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Motion {
    pub kind: Animation,
    pub strength: f32,
    pub duration: f32,
    pub frequency: f32,
    pub repeat: bool,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            kind: Animation::None,
            strength: 0.08,
            duration: 0.4,
            frequency: 6.0,
            repeat: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub opacity: f32,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            offset: [0.0; 2],
            scale: [1.0; 2],
            rotation: 0.0,
            opacity: 1.0,
        }
    }
}
impl Motion {
    pub fn sample(&self, age: f32) -> Transform {
        let mut t = Transform::default();
        if self.kind == Animation::None
            || age < 0.0
            || !age.is_finite()
            || (!self.repeat && age >= self.duration)
        {
            return t;
        }
        let phase = if self.repeat {
            age.rem_euclid(self.duration)
        } else {
            age
        } / self.duration;
        let envelope = if self.repeat {
            1.0
        } else {
            (1.0 - phase).powi(2)
        };
        let wave = (age * self.frequency * std::f32::consts::TAU).sin();
        let s = self.strength;
        match self.kind {
            Animation::Shake => t.offset[0] = s * wave * envelope,
            Animation::Jump => t.offset[1] = -s * (phase * std::f32::consts::PI).sin(),
            Animation::Blip => {
                t.scale = [1.0 + s * envelope, 1.0 - s * envelope * 0.6];
            }
            Animation::Pulse => t.scale = [1.0 + s * wave * envelope; 2],
            Animation::Wobble => t.rotation = s * wave * envelope,
            Animation::Bob => t.offset[1] = -s * wave * envelope,
            Animation::None => {}
        }
        t
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            finite(self.strength, 0.0, 0.5)
                && finite(self.duration, 0.05, 10.0)
                && finite(self.frequency, 0.1, 30.0),
            "Invalid image animation settings"
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub id: u64,
    pub name: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub trigger: Trigger,
    pub rule: InputRule,
    pub priority: i32,
    pub hotkey: Option<Shortcut>,
    pub transition: Transition,
    pub fade_in: f32,
    pub fade_out: f32,
    pub minimum_hold: f32,
    pub motion: Motion,
    pub gif_speed: f32,
    pub gif_loop: bool,
    pub restart_gif: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Image action".into(),
            path: PathBuf::new(),
            enabled: true,
            trigger: Trigger::Idle,
            rule: InputRule::default(),
            priority: 0,
            hotkey: None,
            transition: Transition::Crossfade,
            fade_in: 0.15,
            fade_out: 0.15,
            minimum_hold: 0.08,
            motion: Motion::default(),
            gif_speed: 1.0,
            gif_loop: true,
            restart_gif: true,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub states: Vec<State>,
    pub manual: Option<u64>,
}
fn finite(v: f32, min: f32, max: f32) -> bool {
    v.is_finite() && (min..=max).contains(&v)
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.states.len() <= 128,
            "Maximum 128 image actions per avatar"
        );
        let mut ids = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for s in &self.states {
            ensure!(
                s.id != 0 && ids.insert(s.id),
                "Image action IDs must be unique"
            );
            ensure!(
                !s.name.trim().is_empty()
                    && s.name.chars().count() <= 80
                    && !s.path.as_os_str().is_empty()
                    && s.path.to_string_lossy().len() <= 4096,
                "Image action needs a name and file"
            );
            ensure!(
                finite(s.fade_in, 0.0, 10.0)
                    && finite(s.fade_out, 0.0, 10.0)
                    && finite(s.minimum_hold, 0.0, 10.0)
                    && finite(s.gif_speed, 0.05, 4.0),
                "Invalid image transition or playback timing"
            );
            ensure!(
                (-1000..=1000).contains(&s.priority)
                    && !s.rule.source.trim().is_empty()
                    && s.rule.source.len() <= 256
                    && finite(s.rule.start, -1e6, 1e6)
                    && finite(s.rule.end, s.rule.start, 1e6)
                    && finite(s.rule.hysteresis, 0.0, 1e6),
                "Invalid image trigger range"
            );
            s.motion.validate()?;
            if let Some(k) = s.hotkey {
                k.validate()?;
                ensure!(keys.insert(k), "Image actions cannot share a hotkey");
            }
        }
        ensure!(
            self.manual.is_none_or(|id| ids.contains(&id)),
            "Manual image action no longer exists"
        );
        Ok(())
    }
    pub fn toggle(&mut self, id: u64) {
        if self.states.iter().any(|s| s.id == id && s.enabled) {
            self.manual = if self.manual == Some(id) {
                None
            } else {
                Some(id)
            };
        }
    }
}
#[derive(Clone, Debug)]
pub struct Layer {
    pub id: u64,
    pub weight: f32,
    pub age: f32,
    pub transform: Transform,
}
#[derive(Default)]
pub struct Player {
    pub layers: Vec<Layer>,
    pub current: Option<u64>,
    time: f32,
    elapsed: f32,
    starts: BTreeMap<u64, f32>,
    inside: BTreeMap<u64, bool>,
    transition: Transition,
}
impl Player {
    pub fn update(
        &mut self,
        config: &Config,
        inputs: &Inputs,
        parameters: &[RigParameter],
        dt: f32,
        frozen: bool,
    ) {
        if frozen {
            return;
        }
        let dt = dt.clamp(0.0, 0.25);
        self.time += dt;
        self.inside
            .retain(|id, _| config.states.iter().any(|s| s.id == *id));
        let talking = inputs
            .get("Talking")
            .copied()
            .unwrap_or_else(|| inputs.get("MouthOpen").copied().unwrap_or(0.0))
            > 0.18;
        let blink = inputs
            .get("EyeOpenLeft")
            .copied()
            .unwrap_or(1.0)
            .min(inputs.get("EyeOpenRight").copied().unwrap_or(1.0))
            < 0.3
            || inputs.get("AutoBlink").copied().unwrap_or(1.0) < 0.3;
        let selected = if !config.enabled {
            None
        } else {
            config
                .manual
                .filter(|id| config.states.iter().any(|s| s.id == *id && s.enabled))
                .or_else(|| {
                    config
                        .states
                        .iter()
                        .filter(|s| s.enabled)
                        .filter(|s| match s.trigger {
                            Trigger::Idle => true,
                            Trigger::Talking => talking,
                            Trigger::Quiet => !talking,
                            Trigger::Blink => blink,
                            Trigger::Manual => false,
                            Trigger::Input => {
                                let v = match s.rule.kind {
                                    SignalKind::Tracking => inputs.get(&s.rule.source).copied(),
                                    SignalKind::Parameter => parameters
                                        .iter()
                                        .find(|p| p.id == s.rule.source)
                                        .map(|p| p.value),
                                };
                                let h = if self.inside.get(&s.id) == Some(&true) {
                                    s.rule.hysteresis
                                } else {
                                    0.0
                                };
                                let inside = v.is_some_and(|v| {
                                    v.is_finite()
                                        && (s.rule.start - h..=s.rule.end + h).contains(&v)
                                });
                                self.inside.insert(s.id, inside);
                                inside
                            }
                        })
                        .max_by_key(|s| (s.priority, s.id))
                        .map(|s| s.id)
                })
        };
        let may_switch = config.manual.is_some()
            || !config.enabled
            || self
                .current
                .and_then(|id| config.states.iter().find(|s| s.id == id))
                .is_none_or(|s| {
                    self.layers
                        .iter()
                        .find(|l| l.id == s.id)
                        .is_none_or(|l| l.age >= s.minimum_hold)
                });
        if selected != self.current && may_switch {
            self.starts = self.layers.iter().map(|l| (l.id, l.weight)).collect();
            self.current = selected;
            self.elapsed = 0.0;
            self.transition = selected
                .and_then(|id| config.states.iter().find(|s| s.id == id))
                .map_or(Transition::Crossfade, |s| s.transition);
            if let Some(id) = selected {
                if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
                    l.age = 0.0;
                } else {
                    self.layers.push(Layer {
                        id,
                        weight: 0.0,
                        age: 0.0,
                        transform: Transform::default(),
                    });
                }
            }
        }
        self.elapsed += dt;
        for layer in &mut self.layers {
            let Some(s) = config.states.iter().find(|s| s.id == layer.id) else {
                layer.weight = 0.0;
                continue;
            };
            layer.age += dt;
            let entering = Some(layer.id) == self.current;
            let duration = if self.transition == Transition::Cut {
                0.0
            } else if entering {
                s.fade_in
            } else {
                s.fade_out
            };
            let mut t = if duration <= 0.0 {
                1.0
            } else {
                (self.elapsed / duration).clamp(0.0, 1.0)
            };
            if self.transition == Transition::FadeThrough {
                t = if entering {
                    ((t - 0.5) * 2.0).max(0.0)
                } else {
                    (t * 2.0).min(1.0)
                };
            }
            t = t * t * (3.0 - 2.0 * t);
            let start = self.starts.get(&layer.id).copied().unwrap_or(0.0);
            layer.weight = start + ((if entering { 1.0 } else { 0.0 }) - start) * t;
            layer.transform = s.motion.sample(layer.age);
            if self.transition == Transition::Slide {
                layer.transform.offset[0] += if entering {
                    (1.0 - t) * 0.15
                } else {
                    -t * 0.15
                };
            }
        }
        self.layers
            .retain(|l| Some(l.id) == self.current || l.weight > 0.0001);
    }
    pub fn clock(&self) -> f32 {
        self.time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> Config {
        Config {
            enabled: true,
            states: vec![
                State {
                    path: "idle.png".into(),
                    ..Default::default()
                },
                State {
                    id: 2,
                    path: "talk.gif".into(),
                    trigger: Trigger::Talking,
                    priority: 10,
                    ..Default::default()
                },
            ],
            manual: None,
        }
    }
    #[test]
    fn actions_transition_interrupt_freeze_and_manual_return() {
        let mut c = config();
        let mut p = Player::default();
        let mut i = Inputs::new();
        p.update(&c, &i, &[], 0.2, false);
        assert_eq!(p.current, Some(1));
        assert_eq!(p.layers[0].weight, 1.0);
        i.insert("Talking".into(), 1.0);
        p.update(&c, &i, &[], 0.05, false);
        assert_eq!(p.current, Some(2));
        assert_eq!(p.layers.len(), 2);
        let weights: Vec<_> = p.layers.iter().map(|l| (l.weight, l.age)).collect();
        p.update(&c, &i, &[], 1.0, true);
        assert_eq!(
            weights,
            p.layers
                .iter()
                .map(|l| (l.weight, l.age))
                .collect::<Vec<_>>()
        );
        c.toggle(1);
        p.update(&c, &i, &[], 0.03, false);
        assert_eq!(p.current, Some(1));
        assert!(p.layers.iter().all(|l| (0.0..=1.0).contains(&l.weight)));
        c.toggle(1);
        p.update(&c, &i, &[], 0.2, false);
        p.update(&c, &i, &[], 0.2, false);
        assert_eq!(p.current, Some(2));
        assert_eq!(p.layers.len(), 1);
    }
    #[test]
    fn arbitrary_ranges_missing_inputs_and_animation_endpoints() {
        let mut c = config();
        c.states[1].trigger = Trigger::Input;
        c.states[1].rule.source = "MyInput".into();
        let mut p = Player::default();
        let mut i = Inputs::new();
        i.insert("MyInput".into(), 0.7);
        p.update(&c, &i, &[], 0.2, false);
        assert_eq!(p.current, Some(2));
        i.clear();
        p.update(&c, &i, &[], 0.2, false);
        assert_eq!(p.current, Some(1));
        for kind in [
            Animation::Shake,
            Animation::Jump,
            Animation::Blip,
            Animation::Pulse,
            Animation::Wobble,
            Animation::Bob,
        ] {
            let m = Motion {
                kind,
                ..Default::default()
            };
            assert_ne!(m.sample(0.1), Transform::default());
            assert_eq!(m.sample(m.duration), Transform::default());
        }
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(c, serde_json::from_str(&json).unwrap());
        c.states[0].gif_speed = f32::NAN;
        assert!(c.validate().is_err());
    }
}
