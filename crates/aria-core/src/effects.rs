//! Per-avatar throw/spray designs and a deterministic, bounded transient simulation.
use crate::shortcuts::Shortcut;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};

pub const MAX_ACTIVE: usize = 256;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Throw,
    Spray,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selection {
    #[default]
    Random,
    Cycle,
    All,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Design {
    pub id: u64,
    pub name: String,
    pub kind: Kind,
    /// Paths may use builtin:star, builtin:ball, builtin:cube, builtin:drop.
    pub assets: Vec<PathBuf>,
    pub selection: Selection,
    pub count: u32,
    pub interval: f32,
    pub flight: f32,
    pub size: f32,
    pub size_variance: f32,
    pub origin: [f32; 2],
    pub target: [f32; 2],
    pub spread: f32,
    pub arc: f32,
    pub spin: f32,
    pub bounce: f32,
    pub lifetime: f32,
    pub impact: f32,
    pub tint: [u8; 4],
    pub splash: f32,
    pub launch_sound: PathBuf,
    pub impact_sound: PathBuf,
    pub volume: f32,
    pub cooldown: f32,
    pub hotkey: Option<Shortcut>,
}
impl Default for Design {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Star toss".into(),
            kind: Kind::Throw,
            assets: vec!["builtin:star".into()],
            selection: Selection::Random,
            count: 5,
            interval: 0.12,
            flight: 0.65,
            size: 0.12,
            size_variance: 0.25,
            origin: [-0.8, -0.25],
            target: [0.0, -0.16],
            spread: 0.12,
            arc: 0.18,
            spin: 360.0,
            bounce: 0.6,
            lifetime: 1.4,
            impact: 0.18,
            tint: [255; 4],
            splash: 0.65,
            launch_sound: "builtin:whoosh".into(),
            impact_sound: "builtin:pop".into(),
            volume: 0.45,
            cooldown: 0.5,
            hotkey: None,
        }
    }
}
impl Design {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.id != 0 && !self.name.trim().is_empty() && self.name.chars().count() <= 80,
            "Give the effect a name of 1–80 characters"
        );
        ensure!(
            !self.assets.is_empty()
                && self.assets.len() <= 256
                && self
                    .assets
                    .iter()
                    .all(|p| !p.as_os_str().is_empty() && p.to_string_lossy().len() <= 4096),
            "Choose 1–256 visual assets for this design"
        );
        ensure!((1..=1000).contains(&self.count), "Count must be 1–1000");
        for (v, lo, hi) in [
            (self.interval, 0.0, 5.0),
            (self.flight, 0.1, 5.0),
            (self.size, 0.005, 1.5),
            (self.size_variance, 0.0, 0.9),
            (self.spread, 0.0, 1.0),
            (self.arc, -1.0, 1.0),
            (self.spin, -1440.0, 1440.0),
            (self.bounce, 0.0, 2.0),
            (self.lifetime, 0.1, 15.0),
            (self.impact, 0.0, 1.0),
            (self.splash, 0.0, 2.0),
            (self.volume, 0.0, 1.0),
            (self.cooldown, 0.0, 60.0),
        ] {
            ensure!(
                v.is_finite() && (lo..=hi).contains(&v),
                "Effect value outside its supported range"
            );
        }
        ensure!(
            self.origin
                .iter()
                .chain(&self.target)
                .all(|v| v.is_finite() && v.abs() <= 2.0),
            "Effect positions must be within -2 to 2 canvas heights"
        );
        ensure!(
            self.launch_sound.to_string_lossy().len() <= 4096
                && self.impact_sound.to_string_lossy().len() <= 4096,
            "Sound path too long"
        );
        if let Some(key) = self.hotkey {
            key.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub designs: Vec<Design>,
    pub muted: bool,
    pub volume: f32,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            volume: 0.7,
            muted: false,
            designs: vec![
                Design::default(),
                Design {
                    id: 2,
                    name: "Soft ball volley".into(),
                    assets: vec!["builtin:ball".into()],
                    count: 3,
                    bounce: 0.9,
                    ..Default::default()
                },
                Design {
                    id: 3,
                    name: "3D cube tumble".into(),
                    assets: vec!["builtin:cube".into()],
                    count: 3,
                    size: 0.18,
                    ..Default::default()
                },
                Design {
                    id: 4,
                    name: "Water spray".into(),
                    kind: Kind::Spray,
                    assets: vec!["builtin:drop".into()],
                    count: 80,
                    interval: 0.018,
                    size: 0.023,
                    flight: 0.4,
                    spin: 0.0,
                    origin: [-0.65, -0.12],
                    spread: 0.25,
                    impact: 0.04,
                    tint: [100, 195, 255, 205],
                    launch_sound: "builtin:spray".into(),
                    impact_sound: PathBuf::new(),
                    ..Default::default()
                },
                Design {
                    id: 5,
                    name: "Paint splash".into(),
                    kind: Kind::Spray,
                    assets: vec!["builtin:drop".into()],
                    count: 45,
                    interval: 0.025,
                    size: 0.04,
                    tint: [222, 105, 250, 220],
                    splash: 1.4,
                    lifetime: 4.0,
                    impact: 0.07,
                    launch_sound: "builtin:spray".into(),
                    impact_sound: "builtin:splat".into(),
                    ..Default::default()
                },
            ],
        }
    }
}
impl Library {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.volume.is_finite() && (0.0..=1.0).contains(&self.volume),
            "Invalid master effect volume"
        );
        let mut ids = BTreeSet::new();
        let mut keys = BTreeSet::new();
        for d in &self.designs {
            d.validate()?;
            ensure!(ids.insert(d.id), "Duplicate effect ID");
            if let Some(k) = d.hotkey {
                ensure!(keys.insert(k), "Duplicate effect shortcut");
            }
        }
        Ok(())
    }
    pub fn next_id(&self) -> u64 {
        self.designs
            .iter()
            .map(|d| d.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1)
    }
}
#[derive(Clone, Debug)]
pub struct Particle {
    pub pin: Option<crate::items::Pin>,
    pub pin_checked: bool,
    pub asset: PathBuf,
    pub kind: Kind,
    pub age: f32,
    pub flight: f32,
    pub lifetime: f32,
    pub origin: [f32; 2],
    pub target: [f32; 2],
    pub size: f32,
    pub arc: f32,
    pub spin: f32,
    pub bounce: f32,
    pub splash: f32,
    pub impact: f32,
    pub tint: [u8; 4],
    pub sound: PathBuf,
    pub volume: f32,
    pub hit: bool,
}
impl Particle {
    pub fn pose(&self) -> ([f32; 2], f32, f32, f32) {
        let t = (self.age / self.flight).clamp(0.0, 1.0);
        if !self.hit {
            return (
                [
                    self.origin[0] + (self.target[0] - self.origin[0]) * t,
                    self.origin[1] + (self.target[1] - self.origin[1]) * t
                        - self.arc * 4.0 * t * (1.0 - t),
                ],
                self.size,
                self.spin * self.age,
                1.0,
            );
        }
        let after = (self.age - self.flight).max(0.0);
        let fade = (1.0 - after / self.lifetime).clamp(0.0, 1.0);
        if self.kind == Kind::Spray {
            (
                [self.target[0], self.target[1] + after * 0.012],
                self.size * (1.0 + self.splash * (after * 12.0).min(1.0)),
                0.0,
                fade,
            )
        } else {
            (
                [
                    self.target[0]
                        + (self.target[0] - self.origin[0]).signum() * after * 0.15 * self.bounce,
                    self.target[1] - after * 0.28 * self.bounce + after * after * 0.65,
                ],
                self.size,
                self.spin * self.age,
                fade,
            )
        }
    }
}
struct Burst {
    design: Design,
    remaining: u32,
    emitted: usize,
    wait: f32,
}
#[derive(Default)]
pub struct Simulation {
    pub particles: Vec<Particle>,
    queue: std::collections::VecDeque<Burst>,
    seed: u64,
    pub impulse: [f32; 2],
    velocity: [f32; 2],
    accumulator: f32,
}
pub struct Sound {
    pub path: PathBuf,
    pub volume: f32,
}
impl Simulation {
    pub fn asset_paths(&self) -> BTreeSet<PathBuf> {
        self.particles
            .iter()
            .map(|p| p.asset.clone())
            .chain(
                self.queue
                    .iter()
                    .flat_map(|b| b.design.assets.iter().cloned()),
            )
            .collect()
    }
    pub fn active(&self) -> bool {
        !self.particles.is_empty()
            || !self.queue.is_empty()
            || self.impulse.iter().any(|v| v.abs() > 0.0001)
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn trigger(&mut self, d: &Design) -> Result<()> {
        d.validate()?;
        ensure!(
            self.queue.len() < 32,
            "Effect queue full; clear active effects or wait"
        );
        let count = if d.selection == Selection::All {
            d.count * d.assets.len() as u32
        } else {
            d.count
        };
        ensure!(count <= 1000, "All assets × count must be at most 1000");
        self.queue.push_back(Burst {
            design: d.clone(),
            remaining: count,
            emitted: 0,
            wait: 0.0,
        });
        Ok(())
    }
    pub fn tick(&mut self, dt: f32) -> Vec<Sound> {
        if !dt.is_finite() || dt <= 0.0 {
            return vec![];
        }
        // Fixed steps preserve emission timing and spring behavior at 15–120 FPS.
        // Discard long suspend gaps instead of spawning minutes of queued particles.
        self.accumulator += dt.min(0.25);
        let mut sounds = Vec::new();
        const STEP: f32 = 1.0 / 240.0;
        while self.accumulator + 1e-7 >= STEP {
            self.accumulator -= STEP;
            sounds.extend(self.step(STEP));
        }
        sounds
    }
    fn step(&mut self, dt: f32) -> Vec<Sound> {
        let mut sounds = Vec::new();
        let mut queue = std::mem::take(&mut self.queue);
        for burst in &mut queue {
            burst.wait -= dt;
            while burst.remaining > 0 && burst.wait <= 0.0 {
                if self.particles.len() >= MAX_ACTIVE {
                    break;
                }
                let d = &burst.design;
                let random = self.random();
                let i = match d.selection {
                    Selection::Random => (random * d.assets.len() as f32) as usize % d.assets.len(),
                    _ => burst.emitted % d.assets.len(),
                };
                let jitter = [
                    (self.random() - 0.5) * 2.0 * d.spread,
                    (self.random() - 0.5) * 2.0 * d.spread,
                ];
                let size = d.size * (1.0 + (self.random() - 0.5) * 2.0 * d.size_variance);
                self.particles.push(Particle {
                    pin: None,
                    pin_checked: false,
                    asset: d.assets[i].clone(),
                    kind: d.kind,
                    age: 0.0,
                    flight: d.flight,
                    lifetime: d.lifetime,
                    origin: d.origin,
                    target: [d.target[0] + jitter[0], d.target[1] + jitter[1]],
                    size,
                    arc: d.arc,
                    spin: d.spin,
                    bounce: d.bounce,
                    splash: d.splash,
                    impact: d.impact,
                    tint: d.tint,
                    sound: d.impact_sound.clone(),
                    volume: d.volume,
                    hit: false,
                });
                if burst.emitted == 0 && !d.launch_sound.as_os_str().is_empty() {
                    sounds.push(Sound {
                        path: d.launch_sound.clone(),
                        volume: d.volume,
                    });
                }
                burst.emitted += 1;
                burst.remaining -= 1;
                burst.wait += d.interval.max(0.001);
            }
        }
        queue.retain(|b| b.remaining > 0);
        self.queue = queue;
        for p in &mut self.particles {
            p.age += dt;
            if !p.hit && p.age >= p.flight {
                p.hit = true;
                self.velocity[0] += (p.target[0] - p.origin[0]).signum() * p.impact * 0.8;
                self.velocity[1] += p.impact * 0.35;
                if !p.sound.as_os_str().is_empty() {
                    sounds.push(Sound {
                        path: p.sound.clone(),
                        volume: p.volume,
                    });
                }
            }
        }
        self.particles.retain(|p| p.age < p.flight + p.lifetime);
        for axis in 0..2 {
            self.velocity[axis] =
                (self.velocity[axis] - self.impulse[axis] * 40.0 * dt) * (-10.0 * dt).exp();
            self.impulse[axis] = (self.impulse[axis] + self.velocity[axis] * dt).clamp(-0.08, 0.08);
            if self.impulse[axis].abs() < 0.00001 && self.velocity[axis].abs() < 0.00001 {
                self.impulse[axis] = 0.0;
                self.velocity[axis] = 0.0;
            }
        }
        sounds
    }
    fn random(&mut self) -> f32 {
        if self.seed == 0 {
            self.seed = 0xA913C581;
        }
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed as u32) as f32 / u32::MAX as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_selection_and_timing_are_independent_of_render_rate() {
        let d = Design {
            count: 3,
            selection: Selection::All,
            assets: vec!["a.png".into(), "b.obj".into()],
            interval: 0.08,
            lifetime: 4.0,
            ..Default::default()
        };
        let run = |fps: u32| {
            let mut s = Simulation::default();
            s.trigger(&d).unwrap();
            for _ in 0..fps {
                s.tick(1.0 / fps as f32);
            }
            s
        };
        let a = run(15);
        let b = run(120);
        assert_eq!(a.particles.len(), 6);
        for (p, q) in a.particles.iter().zip(&b.particles) {
            assert_eq!(p.asset, q.asset);
            assert!((p.age - q.age).abs() < 0.005);
        }
        assert!((a.impulse[0] - b.impulse[0]).abs() < 0.001);
    }
    #[test]
    fn burst_counts_cycle_assets_impact_once_and_settle() {
        let d = Design {
            count: 9,
            assets: vec!["a.png".into(), "b.moc3".into(), "c.glb".into()],
            selection: Selection::Cycle,
            interval: 0.0,
            ..Default::default()
        };
        let mut sim = Simulation::default();
        sim.trigger(&d).unwrap();
        sim.tick(0.016);
        assert_eq!(sim.particles.len(), 9);
        assert_eq!(sim.particles[1].asset, d.assets[1]);
        let mut hits = 0;
        for _ in 0..500 {
            hits += sim.tick(0.016).len();
        }
        assert_eq!(hits, 9);
        assert!(sim.particles.is_empty());
        assert!(sim.impulse[0].abs() < 0.0001);
    }
    #[test]
    fn large_bursts_are_bounded_and_freeze_does_not_advance() {
        let mut s = Simulation::default();
        s.trigger(&Design {
            count: 1000,
            interval: 0.0,
            ..Default::default()
        })
        .unwrap();
        for _ in 0..100 {
            s.tick(0.016);
            assert!(s.particles.len() <= MAX_ACTIVE);
        }
        let ages: Vec<_> = s.particles.iter().map(|p| p.age).collect();
        s.tick(0.0);
        assert_eq!(ages, s.particles.iter().map(|p| p.age).collect::<Vec<_>>());
    }
    #[test]
    fn designs_round_trip_and_reject_nonfinite_values() {
        let mut l = Library::default();
        l.validate().unwrap();
        assert_eq!(
            l,
            serde_json::from_slice::<Library>(&serde_json::to_vec(&l).unwrap()).unwrap()
        );
        l.designs[0].size = f32::NAN;
        assert!(l.validate().is_err());
    }
}
