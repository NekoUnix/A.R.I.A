//! Rust particle-chain solver for authored physics3 rigs. Evaluates fixed steps
//! before Purism Core; retains momentum and interpolates outputs between steps.
//! This implements the data format, not VTube Studio's proprietary physics modes.
use crate::rig::RigParameter;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Independent motion styles, not implementations of any third-party solver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum MotionStyle {
    /// Preserve ARIA's previous authored-rate solver for saved profiles.
    #[default]
    Authored,
    Natural,
    Bouncy,
}

/// Multipliers on the avatar's authored particle properties, never file edits.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct GroupSettings {
    pub enabled: bool,
    pub strength: f32,
    pub inertia: f32,
    pub response: f32,
    pub gravity: f32,
    pub wind: f32,
}
impl Default for GroupSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            strength: 1.0,
            inertia: 1.0,
            response: 1.0,
            gravity: 1.0,
            wind: 0.0,
        }
    }
}
impl GroupSettings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [self.strength, self.inertia, self.gravity]
                .iter()
                .all(|v| v.is_finite() && (0.0..=2.0).contains(v))
                && self.response.is_finite()
                && (0.25..=2.0).contains(&self.response)
                && self.wind.is_finite()
                && (-1.0..=1.0).contains(&self.wind),
            "Invalid physics tuning; multipliers or wind exceed supported limits"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct PhysicsSettings {
    // Missing fields in existing profiles retain the old solver. New avatars
    // use PhysicsSettings::default(), which selects expressive motion.
    #[serde(default)]
    pub motion_style: MotionStyle,
    // Keep the v0.4 fields at their original JSON/RON locations.
    pub enabled: bool,
    pub strength: f32,
    pub wind: f32,
    pub inertia: f32,
    pub response: f32,
    pub gravity: f32,
    pub groups: BTreeMap<String, GroupSettings>,
}
impl Default for PhysicsSettings {
    fn default() -> Self {
        Self {
            motion_style: MotionStyle::Bouncy,
            enabled: true,
            strength: 1.0,
            wind: 0.0,
            inertia: 1.0,
            response: 1.0,
            gravity: 1.0,
            groups: BTreeMap::new(),
        }
    }
}
impl PhysicsSettings {
    pub fn validate(&self) -> Result<()> {
        GroupSettings {
            enabled: self.enabled,
            strength: self.strength,
            wind: self.wind,
            inertia: self.inertia,
            response: self.response,
            gravity: self.gravity,
        }
        .validate()?;
        ensure!(self.groups.len() <= 256, "Too many saved physics groups");
        for (id, group) in &self.groups {
            ensure!(
                !id.is_empty() && id.len() <= 256,
                "Invalid physics group ID"
            );
            group.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub particles: usize,
    pub imported_multiplier: f32,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct V2 {
    x: f32,
    y: f32,
}
impl V2 {
    const UP: Self = Self { x: 0.0, y: 1.0 };
    fn add(self, b: Self) -> Self {
        Self {
            x: self.x + b.x,
            y: self.y + b.y,
        }
    }
    fn sub(self, b: Self) -> Self {
        Self {
            x: self.x - b.x,
            y: self.y - b.y,
        }
    }
    fn mul(self, n: f32) -> Self {
        Self {
            x: self.x * n,
            y: self.y * n,
        }
    }
    fn unit(self) -> Self {
        let n = self.x.hypot(self.y);
        if n > 1e-6 {
            self.mul(1.0 / n)
        } else {
            Self::UP
        }
    }
    fn rotate(self, a: f32) -> Self {
        let (s, c) = a.sin_cos();
        Self {
            x: self.x * c - self.y * s,
            y: self.x * s + self.y * c,
        }
    }
    fn angle(self, b: Self) -> f32 {
        (self.x * b.y - self.y * b.x).atan2(self.x * b.x + self.y * b.y)
    }
    fn valid(self) -> bool {
        [self.x, self.y]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e6)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Document {
    version: u32,
    meta: Meta,
    physics_settings: Vec<Setting>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Meta {
    #[serde(default)]
    fps: f32,
    effective_forces: Forces,
    #[serde(default)]
    physics_dictionary: Vec<DictionaryEntry>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DictionaryEntry {
    id: String,
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Forces {
    gravity: V2,
    wind: V2,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Setting {
    id: String,
    input: Vec<Input>,
    output: Vec<Output>,
    vertices: Vec<Vertex>,
    normalization: Normalization,
}
#[derive(Clone, Copy, Deserialize)]
enum Kind {
    X,
    Y,
    Angle,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Target {
    target: String,
    id: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Input {
    source: Target,
    weight: f32,
    #[serde(rename = "Type")]
    kind: Kind,
    reflect: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Output {
    destination: Target,
    vertex_index: usize,
    scale: f32,
    weight: f32,
    #[serde(rename = "Type")]
    kind: Kind,
    reflect: bool,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Vertex {
    position: V2,
    mobility: f32,
    delay: f32,
    acceleration: f32,
    radius: f32,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Normalization {
    position: Range,
    angle: Range,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Range {
    minimum: f32,
    maximum: f32,
    default: f32,
}
impl Range {
    fn validate(&self) -> Result<()> {
        ensure!(
            [self.minimum, self.maximum, self.default]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1e4)
                && self.minimum <= self.default
                && self.default <= self.maximum,
            "Invalid physics normalization range"
        );
        Ok(())
    }
    fn normalize(&self, p: &RigParameter, reflect: bool) -> f32 {
        let middle = (p.min + p.max) * 0.5;
        let value = p.value.clamp(p.min, p.max);
        let value = if value > middle && p.max > middle {
            self.default + (value - middle) / (p.max - middle) * (self.maximum - self.default)
        } else if value < middle && p.min < middle {
            self.default + (value - middle) / (middle - p.min) * (self.default - self.minimum)
        } else {
            self.default
        };
        value * if reflect { 1.0 } else { -1.0 }
    }
}

struct Particle {
    spec: Vertex,
    pos: V2,
    velocity: V2,
    gravity: V2,
}
struct Driver {
    index: usize,
    spec: Input,
}
struct Destination {
    index: usize,
    spec: Output,
    previous: f32,
    current: f32,
}
struct Chain {
    id: String,
    name: String,
    drivers: Vec<Driver>,
    outputs: Vec<Destination>,
    particles: Vec<Particle>,
    normalization: Normalization,
    multiplier: f32,
    tuning: GroupSettings,
    reset_pending: bool,
}

pub struct Physics {
    chains: Vec<Chain>,
    authored_step: f64,
    step: f64,
    accumulator: f64,
    previous_inputs: Vec<f32>,
    input_values: Vec<f32>,
    working: Vec<RigParameter>,
    motion_style: MotionStyle,
    rest_gravity: V2,
    wind: V2,
    initialized: bool,
    pub enabled: bool,
    pub strength: f32,
    pub wind_strength: f32,
    inertia: f32,
    response: f32,
    gravity: f32,
    pub warnings: Vec<String>,
}

impl Physics {
    pub fn load(bytes: &[u8], parameters: &[RigParameter]) -> Result<Self> {
        ensure!(
            bytes.len() <= crate::asset_limits::MODEL_JSON,
            "Physics file exceeds 20 MiB"
        );
        let doc: Document = serde_json::from_slice(bytes)?;
        ensure!(doc.version == 3, "Expected physics3 Version 3");
        ensure!(doc.physics_settings.len() <= 256, "Too many physics groups");
        ensure!(
            doc.meta.fps.is_finite() && (0.0..=240.0).contains(&doc.meta.fps),
            "Invalid physics FPS"
        );
        ensure!(
            doc.meta.effective_forces.gravity.valid() && doc.meta.effective_forces.wind.valid(),
            "Invalid physics forces"
        );
        let authored_step = 1.0
            / if doc.meta.fps >= 1.0 {
                doc.meta.fps as f64
            } else {
                60.0
            };
        let mut runtime = Self {
            chains: Vec::new(),
            authored_step,
            step: authored_step,
            accumulator: 0.0,
            previous_inputs: Vec::new(),
            input_values: Vec::with_capacity(parameters.len()),
            working: parameters.to_vec(),
            motion_style: MotionStyle::Authored,
            rest_gravity: doc.meta.effective_forces.gravity.mul(-1.0).unit(),
            wind: doc.meta.effective_forces.wind,
            initialized: false,
            enabled: true,
            strength: 1.0,
            wind_strength: 0.0,
            inertia: 1.0,
            response: 1.0,
            gravity: 1.0,
            warnings: Vec::new(),
        };
        let names: BTreeMap<_, _> = doc
            .meta
            .physics_dictionary
            .into_iter()
            .filter(|e| e.id.len() <= 256 && !e.name.trim().is_empty() && e.name.len() <= 512)
            .map(|e| (e.id, e.name))
            .collect();
        let mut ids = BTreeSet::new();
        for s in doc.physics_settings {
            ensure!(
                !s.id.is_empty() && s.id.len() <= 256 && ids.insert(s.id.clone()),
                "Physics group IDs must be unique and nonempty"
            );
            ensure!(
                (2..=64).contains(&s.vertices.len())
                    && s.input.len() <= 128
                    && s.output.len() <= 256,
                "Invalid physics group size"
            );
            s.normalization.position.validate()?;
            s.normalization.angle.validate()?;
            for (i, v) in s.vertices.iter().enumerate() {
                ensure!(
                    v.position.valid()
                        && v.mobility.is_finite()
                        && (0.0..=1.0).contains(&v.mobility)
                        && v.delay.is_finite()
                        && (0.0..=10.0).contains(&v.delay)
                        && v.acceleration.is_finite()
                        && (0.0..=100.0).contains(&v.acceleration)
                        && v.radius.is_finite()
                        && (0.0..=1e4).contains(&v.radius)
                        && (i == 0 || v.radius > 0.0),
                    "Invalid physics particle"
                );
            }
            let mut chain = Chain {
                name: names.get(&s.id).cloned().unwrap_or_else(|| s.id.clone()),
                id: s.id,
                drivers: Vec::new(),
                outputs: Vec::new(),
                normalization: s.normalization,
                multiplier: 1.0,
                tuning: GroupSettings::default(),
                reset_pending: false,
                particles: s
                    .vertices
                    .into_iter()
                    .map(|spec| Particle {
                        pos: spec.position,
                        spec,
                        velocity: V2::default(),
                        gravity: V2::UP,
                    })
                    .collect(),
            };
            for input in s.input {
                ensure!(
                    input.source.target == "Parameter"
                        && input.weight.is_finite()
                        && (0.0..=100.0).contains(&input.weight),
                    "Invalid physics input"
                );
                if let Some(index) = parameters.iter().position(|p| p.id == input.source.id) {
                    chain.drivers.push(Driver { index, spec: input });
                } else {
                    runtime
                        .warnings
                        .push(format!("Physics input {} is absent", input.source.id));
                }
            }
            for output in s.output {
                ensure!(
                    output.destination.target == "Parameter"
                        && output.weight.is_finite()
                        && (0.0..=100.0).contains(&output.weight)
                        && output.scale.is_finite()
                        && output.scale.abs() <= 1e6
                        && (1..chain.particles.len()).contains(&output.vertex_index),
                    "Invalid physics output or vertex index"
                );
                if let Some(index) = parameters
                    .iter()
                    .position(|p| p.id == output.destination.id)
                {
                    chain.outputs.push(Destination {
                        index,
                        spec: output,
                        previous: 0.0,
                        current: 0.0,
                    });
                } else {
                    runtime.warnings.push(format!(
                        "Physics output {} is absent",
                        output.destination.id
                    ));
                }
            }
            runtime.chains.push(chain);
        }
        Ok(runtime)
    }
    pub fn group_count(&self) -> usize {
        self.chains.len()
    }
    pub fn groups(&self) -> Vec<GroupInfo> {
        self.chains
            .iter()
            .map(|c| GroupInfo {
                id: c.id.clone(),
                name: c.name.clone(),
                inputs: c.drivers.iter().map(|d| d.spec.source.id.clone()).collect(),
                outputs: c
                    .outputs
                    .iter()
                    .map(|o| o.spec.destination.id.clone())
                    .collect(),
                particles: c.particles.len(),
                imported_multiplier: c.multiplier,
            })
            .collect()
    }
    pub fn configure(&mut self, settings: &PhysicsSettings) {
        if self.motion_style != settings.motion_style {
            self.motion_style = settings.motion_style;
            self.step = if self.motion_style == MotionStyle::Authored {
                self.authored_step
            } else {
                1.0 / 120.0
            };
            // Authored velocity uses response-scaled units; enhanced velocity
            // uses units/second. Never carry one into the other on a mode change.
            self.reset();
        }
        self.enabled = settings.enabled;
        self.strength = settings.strength;
        self.wind_strength = settings.wind;
        self.inertia = settings.inertia;
        self.response = settings.response;
        self.gravity = settings.gravity;
        for chain in &mut self.chains {
            let tuning = settings.groups.get(&chain.id).copied().unwrap_or_default();
            if chain.tuning.enabled != tuning.enabled {
                chain.reset_pending = true;
            }
            chain.tuning = tuning;
        }
    }
    pub fn output_count(&self) -> usize {
        self.chains.iter().map(|c| c.outputs.len()).sum()
    }
    pub fn controls_parameter(&self, index: usize) -> bool {
        self.chains
            .iter()
            .any(|c| c.outputs.iter().any(|o| o.index == index))
    }
    pub fn set_multipliers(&mut self, values: &BTreeMap<String, f32>) {
        for c in &mut self.chains {
            c.multiplier = values
                .get(&c.id)
                .copied()
                .filter(|v| v.is_finite())
                .unwrap_or(1.0)
                .clamp(0.0, 5.0);
        }
    }
    pub fn reset(&mut self) {
        self.initialized = false;
        self.accumulator = 0.0;
    }

    pub fn update(&mut self, parameters: &mut [RigParameter], dt: f32) {
        if parameters.len() != self.working.len() {
            self.reset();
            return;
        }
        if !self.enabled {
            self.reset();
            return;
        }
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let enhanced = self.motion_style != MotionStyle::Authored;
        let resume = enhanced && dt > 0.25;
        if dt > 0.5 || resume {
            self.reset();
        }
        // A suspended window must not replay a quarter-second of imaginary motion.
        let dt = if resume { 0.0 } else { dt.min(0.25) as f64 };
        self.input_values.clear();
        self.input_values.extend(parameters.iter().map(|p| {
            if p.value.is_finite() {
                p.value.clamp(p.min, p.max)
            } else {
                p.default.clamp(p.min, p.max)
            }
        }));
        for (p, value) in parameters.iter_mut().zip(&self.input_values) {
            p.value = *value;
        }
        if !self.initialized || self.previous_inputs.len() != parameters.len() {
            self.working.clone_from_slice(parameters);
            for (p, value) in self.working.iter_mut().zip(&self.input_values) {
                p.value = *value;
            }
            for c in &mut self.chains {
                if !c.tuning.enabled {
                    continue;
                }
                initialize_chain(c, &self.working, self.rest_gravity);
                apply(c, &mut self.working, 1.0, self.strength);
            }
            self.previous_inputs.clone_from(&self.input_values);
            self.initialized = true;
        }
        let carried = self.accumulator;
        self.accumulator += dt;
        let mut elapsed = self.step - carried;
        // At most 30 enhanced steps (60 authored), independent of render FPS.
        while self.accumulator + 1e-9 >= self.step {
            let t = (elapsed / dt).clamp(0.0, 1.0) as f32;
            for (i, p) in self.working.iter_mut().enumerate() {
                p.value =
                    self.previous_inputs[i] + (self.input_values[i] - self.previous_inputs[i]) * t;
            }
            for c in &mut self.chains {
                if !c.tuning.enabled {
                    continue;
                }
                if c.reset_pending {
                    initialize_chain(c, &self.working, self.rest_gravity);
                }
                let wind = self.wind.add(V2 {
                    x: (self.wind_strength.clamp(-1.0, 1.0) + c.tuning.wind.clamp(-1.0, 1.0)) * 0.1,
                    y: 0.0,
                });
                let (translation, gravity) = drivers(c, &self.working, self.rest_gravity);
                c.particles[0].pos = translation;
                let dynamics = enhanced.then(|| {
                    EnhancedStep::new(
                        self.step as f32,
                        &c.tuning,
                        self.inertia,
                        self.response,
                        self.gravity,
                        self.motion_style,
                    )
                });
                for i in 1..c.particles.len() {
                    let parent = c.particles[i - 1].pos;
                    let p = &mut c.particles[i];
                    if let Some(dynamics) = &dynamics {
                        step_enhanced(p, parent, gravity, wind, dynamics);
                        continue;
                    }
                    let delay = p.spec.delay
                        * self.step as f32
                        * 30.0
                        * self.response.clamp(0.25, 2.0)
                        * c.tuning.response.clamp(0.25, 2.0);
                    let before = p.pos;
                    let direction = p.pos.sub(parent).rotate(p.gravity.angle(gravity) / 5.0);
                    let force = gravity
                        .mul(
                            p.spec.acceleration
                                * self.gravity.clamp(0.0, 2.0)
                                * c.tuning.gravity.clamp(0.0, 2.0),
                        )
                        .add(wind);
                    let predicted = direction
                        .add(p.velocity.mul(delay))
                        .add(force.mul(delay * delay));
                    p.pos = parent.add(predicted.unit().mul(p.spec.radius));
                    p.velocity = if delay > 1e-6 {
                        let mobility = (p.spec.mobility
                            * self.inertia.clamp(0.0, 2.0)
                            * c.tuning.inertia.clamp(0.0, 2.0))
                        .clamp(0.0, 1.0);
                        p.pos.sub(before).mul(mobility / delay)
                    } else {
                        V2::default()
                    };
                    p.gravity = gravity;
                }
                for o in &mut c.outputs {
                    o.previous = o.current;
                }
                outputs(c, self.rest_gravity);
                // Preserve authored group order: later chains can use earlier outputs.
                apply(c, &mut self.working, 1.0, self.strength);
            }
            self.accumulator = (self.accumulator - self.step).max(0.0);
            elapsed += self.step;
        }
        self.previous_inputs.clone_from(&self.input_values);
        let alpha = (self.accumulator / self.step).clamp(0.0, 1.0) as f32;
        for c in &self.chains {
            apply(c, parameters, alpha, self.strength);
        }
    }
}

struct EnhancedStep {
    dt: f32,
    response: f32,
    inertia: f32,
    gravity: f32,
    decay_exponent: f32,
    drag: f32,
    follow: f32,
}
impl EnhancedStep {
    fn new(
        dt: f32,
        tuning: &GroupSettings,
        inertia: f32,
        response: f32,
        gravity: f32,
        style: MotionStyle,
    ) -> Self {
        Self {
            dt,
            response: 30.0 * response.clamp(0.25, 2.0) * tuning.response.clamp(0.25, 2.0),
            inertia: inertia.clamp(0.0, 2.0) * tuning.inertia.clamp(0.0, 2.0),
            gravity: gravity.clamp(0.0, 2.0) * tuning.gravity.clamp(0.0, 2.0),
            decay_exponent: dt
                * 60.0
                * if style == MotionStyle::Bouncy {
                    0.35
                } else {
                    1.0
                },
            drag: (-0.6 * dt).exp(),
            follow: 1.0 - 0.8_f32.powf(dt * 60.0),
        }
    }
}

/// Projected particle integration in world units/second. Authored mobility is
/// interpreted as retention per 1/60 second, so more substeps do not add damping.
/// Bouncy reduces damping, not the spring's force or the exported chain lengths.
fn step_enhanced(p: &mut Particle, parent: V2, gravity: V2, wind: V2, step: &EnhancedStep) {
    let dt = step.dt;
    let speed = p.spec.delay * step.response;
    let mobility = (p.spec.mobility * step.inertia).clamp(0.0, 1.0);
    // Even maximum inertia dissipates energy rather than ringing indefinitely.
    let retention = mobility.powf(step.decay_exponent) * step.drag;
    let before = p.pos;
    let direction = before
        .sub(parent)
        .rotate(p.gravity.angle(gravity) * step.follow);
    let force = gravity.mul(p.spec.acceleration * step.gravity).add(wind);
    let predicted = direction
        .add(p.velocity.mul(dt))
        .add(force.mul((dt * speed).powi(2)));
    p.pos = parent.add(predicted.unit().mul(p.spec.radius));
    p.velocity = if speed > 1e-6 {
        p.pos.sub(before).mul(retention / dt)
    } else {
        V2::default()
    };
    // Bound kinetic energy after abrupt tracking jumps without stretching the rig.
    let velocity = p.velocity.x.hypot(p.velocity.y);
    let limit = p.spec.radius * 20.0;
    if velocity > limit {
        p.velocity = p.velocity.mul(limit / velocity);
    }
    p.gravity = gravity;
}

fn initialize_chain(c: &mut Chain, parameters: &[RigParameter], rest: V2) {
    let (translation, gravity) = drivers(c, parameters, rest);
    c.particles[0].pos = translation;
    for i in 1..c.particles.len() {
        c.particles[i].pos = c.particles[i - 1]
            .pos
            .add(gravity.mul(c.particles[i].spec.radius));
        c.particles[i].velocity = V2::default();
        c.particles[i].gravity = gravity;
    }
    outputs(c, rest);
    for o in &mut c.outputs {
        o.previous = o.current;
    }
    c.reset_pending = false;
}

fn drivers(c: &Chain, parameters: &[RigParameter], rest: V2) -> (V2, V2) {
    let mut translation = V2::default();
    let mut angle = 0.0;
    for d in &c.drivers {
        let range = if matches!(d.spec.kind, Kind::Angle) {
            &c.normalization.angle
        } else {
            &c.normalization.position
        };
        let value = range.normalize(&parameters[d.index], d.spec.reflect) * d.spec.weight / 100.0;
        match d.spec.kind {
            Kind::X => translation.x += value,
            Kind::Y => translation.y += value,
            Kind::Angle => angle += value,
        }
    }
    (
        translation.rotate(-angle.to_radians()),
        rest.rotate(-angle.to_radians()),
    )
}
fn outputs(c: &mut Chain, rest: V2) {
    for o in &mut c.outputs {
        let i = o.spec.vertex_index;
        let direction = c.particles[i].pos.sub(c.particles[i - 1].pos);
        let value = match o.spec.kind {
            Kind::X => direction.x,
            Kind::Y => direction.y,
            Kind::Angle => {
                let base = if i > 1 {
                    c.particles[i - 1].pos.sub(c.particles[i - 2].pos)
                } else {
                    rest
                };
                base.angle(direction)
            }
        } * if o.spec.reflect { -1.0 } else { 1.0 };
        o.current = if value.is_finite() {
            value * o.spec.scale
        } else {
            0.0
        };
    }
}
fn apply(c: &Chain, parameters: &mut [RigParameter], alpha: f32, strength: f32) {
    if !c.tuning.enabled || c.reset_pending {
        return;
    }
    for o in &c.outputs {
        let p = &mut parameters[o.index];
        let value = (o.previous + (o.current - o.previous) * alpha)
            * c.multiplier
            * c.tuning.strength.clamp(0.0, 2.0)
            * strength.clamp(0.0, 3.0);
        let target = value.clamp(p.min, p.max);
        p.value += (target - p.value) * o.spec.weight / 100.0;
        p.value = p.value.clamp(p.min, p.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = br#"{"Version":3,"Meta":{"Fps":60,"EffectiveForces":{"Gravity":{"X":0,"Y":-1},"Wind":{"X":0,"Y":0}}},"PhysicsSettings":[{"Id":"Hair","Input":[{"Source":{"Target":"Parameter","Id":"Head"},"Weight":100,"Type":"X","Reflect":true}],"Output":[{"Destination":{"Target":"Parameter","Id":"Hair"},"VertexIndex":1,"Scale":3,"Weight":100,"Type":"Angle","Reflect":false}],"Vertices":[{"Position":{"X":0,"Y":0},"Mobility":0.8,"Delay":0.8,"Acceleration":0.8,"Radius":0},{"Position":{"X":0,"Y":10},"Mobility":0.8,"Delay":0.8,"Acceleration":0.8,"Radius":10}],"Normalization":{"Position":{"Minimum":-10,"Default":0,"Maximum":10},"Angle":{"Minimum":-30,"Default":0,"Maximum":30}}}]}"#;
    fn parameters() -> Vec<RigParameter> {
        ["Head", "Hair"]
            .map(|id| RigParameter {
                id: id.into(),
                min: -1.0,
                max: 1.0,
                default: 0.0,
                value: 0.0,
            })
            .to_vec()
    }

    fn impulse_trace(style: MotionStyle) -> Vec<f32> {
        let mut p = parameters();
        let mut physics = Physics::load(FIXTURE, &p).unwrap();
        physics.configure(&PhysicsSettings {
            motion_style: style,
            ..Default::default()
        });
        (0..1200)
            .map(|n| {
                p[0].value = if n < 30 { 0.0 } else { 0.03 };
                p[1].value = 0.0;
                physics.update(&mut p, 1.0 / 120.0);
                p[1].value
            })
            .collect()
    }

    #[test]
    fn expressive_motion_rebounds_more_and_still_settles() {
        let authored = impulse_trace(MotionStyle::Authored);
        let natural = impulse_trace(MotionStyle::Natural);
        let bouncy = impulse_trace(MotionStyle::Bouncy);
        let rebound = |trace: &[f32]| -trace[60..360].iter().copied().fold(0.0_f32, f32::min);
        eprintln!(
            "Rebound peaks: authored={}, natural={}, bouncy={}",
            rebound(&authored),
            rebound(&natural),
            rebound(&bouncy)
        );
        assert!(bouncy[30..60].iter().any(|v| *v > 0.03), "initial hair lag");
        assert!(rebound(&bouncy) > rebound(&natural) * 1.5 + 0.005);
        assert!(rebound(&bouncy) > rebound(&authored) * 1.5 + 0.005);
        assert!(
            bouncy[1000..].iter().all(|v| v.abs() < 0.002),
            "rebound must decay"
        );
        assert!(
            bouncy.iter().all(|v| v.is_finite() && v.abs() < 0.5),
            "no clipping in this fixture"
        );
    }

    #[test]
    fn expressive_trajectory_is_consistent_at_30_60_120_and_jittery_fps() {
        let run = |schedule: &[f32]| {
            let mut p = parameters();
            let mut physics = Physics::load(FIXTURE, &p).unwrap();
            physics.configure(&PhysicsSettings::default());
            physics.update(&mut p, 1.0 / 120.0);
            let mut time = 0.0;
            let mut trace = Vec::new();
            for _ in 0..120 {
                for dt in schedule {
                    time += dt;
                    p[0].value = (time * 3.0_f32).sin() * 0.03;
                    p[1].value = 0.0;
                    physics.update(&mut p, *dt);
                }
                trace.push(p[1].value);
            }
            trace
        };
        let reference = run(&[1.0 / 120.0; 4]);
        for schedule in [
            &[1.0 / 30.0][..],
            &[1.0 / 60.0; 2],
            &[1.0 / 240.0, 1.0 / 80.0, 1.0 / 120.0, 1.0 / 120.0],
        ] {
            let other = run(schedule);
            let rms = (reference
                .iter()
                .zip(&other)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>()
                / reference.len() as f32)
                .sqrt();
            eprintln!("Expressive frame-rate RMS error: {rms}");
            assert!(rms < 0.003, "frame cadence changed motion: {rms}");
        }
    }

    #[test]
    fn enhanced_chain_preserves_lengths_and_bounds_energy_under_extreme_inputs() {
        let mut doc: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        let vertex = doc["PhysicsSettings"][0]["Vertices"][1].clone();
        doc["PhysicsSettings"][0]["Vertices"]
            .as_array_mut()
            .unwrap()
            .extend([vertex.clone(), vertex]);
        let mut p = parameters();
        let mut physics = Physics::load(&serde_json::to_vec(&doc).unwrap(), &p).unwrap();
        let settings = PhysicsSettings {
            inertia: 2.0,
            response: 2.0,
            gravity: 2.0,
            wind: 1.0,
            ..Default::default()
        };
        physics.configure(&settings);
        for frame in 0..3600 {
            p[0].value = if frame % 19 == 0 {
                f32::NAN
            } else {
                (frame as f32 * 0.7).sin() * 5.0
            };
            p[1].value = 0.0;
            physics.update(
                &mut p,
                if frame % 3 == 0 {
                    1.0 / 15.0
                } else {
                    1.0 / 144.0
                },
            );
            assert!(p[1].value.is_finite() && p[1].value.abs() <= 1.0);
            for chain in &physics.chains {
                for pair in chain.particles.windows(2) {
                    let d = pair[1].pos.sub(pair[0].pos);
                    assert!((d.x.hypot(d.y) - pair[1].spec.radius).abs() < 0.0001);
                    let v = pair[1].velocity;
                    assert!(v.x.is_finite() && v.y.is_finite());
                    assert!(v.x.hypot(v.y) <= pair[1].spec.radius * 20.001);
                }
            }
        }
    }

    #[test]
    fn pause_and_style_changes_discard_old_momentum_without_replaying_time() {
        let mut p = parameters();
        let mut physics = Physics::load(FIXTURE, &p).unwrap();
        let mut settings = PhysicsSettings::default();
        physics.configure(&settings);
        physics.update(&mut p, 1.0 / 120.0);
        p[0].value = 0.3;
        physics.update(&mut p, 1.0 / 60.0);
        assert!(p[1].value.abs() > 0.01);
        physics.update(&mut p, 5.0);
        assert!(p[1].value.abs() < 1e-6);
        assert_eq!(physics.accumulator, 0.0);
        for style in [
            MotionStyle::Authored,
            MotionStyle::Bouncy,
            MotionStyle::Natural,
        ] {
            settings.motion_style = style;
            physics.configure(&settings);
            physics.update(&mut p, 1.0 / 120.0);
            assert!(p[1].value.abs() < 1e-6);
        }
        physics.update(&mut [], 1.0 / 120.0); // Stale parameter layout cannot index invalid memory.
    }
    #[test]
    fn movement_produces_inertia_then_settles_and_disable_removes_it() {
        let mut p = parameters();
        let mut physics = Physics::load(FIXTURE, &p).unwrap();
        physics.update(&mut p, 1.0 / 60.0);
        p[0].value = 0.2;
        for _ in 0..4 {
            p[1].value = 0.0;
            physics.update(&mut p, 1.0 / 60.0);
        }
        let pushed = p[1].value;
        assert!(pushed.abs() > 0.1);
        p[1].value = 0.0;
        physics.update(&mut p, 1.0 / 60.0);
        assert!(
            (p[1].value - pushed).abs() > 0.001,
            "Hair keeps moving after the head stops"
        );
        for _ in 0..900 {
            p[1].value = 0.0;
            physics.update(&mut p, 1.0 / 60.0);
            assert!(p[1].value.is_finite() && p[1].value.abs() <= 1.0);
        }
        assert!(p[1].value.abs() < 0.01);
        physics.enabled = false;
        p[1].value = 0.3;
        physics.update(&mut p, 1.0 / 60.0);
        assert_eq!(p[1].value, 0.3);
    }
    #[test]
    fn fixed_step_stays_close_at_different_render_rates() {
        let run = |fps: u32| {
            let mut p = parameters();
            let mut physics = Physics::load(FIXTURE, &p).unwrap();
            for n in 0..fps * 3 {
                p[0].value = (n as f32 / fps as f32 * 2.0).sin() * 0.7;
                p[1].value = 0.0;
                physics.update(&mut p, 1.0 / fps as f32);
            }
            p[1].value
        };
        let reference = run(60);
        assert!((run(30) - reference).abs() < 0.06);
        assert!((run(120) - reference).abs() < 0.06);
    }
    #[test]
    fn rejects_invalid_particle_indices_and_ranges() {
        let text = String::from_utf8(FIXTURE.to_vec()).unwrap();
        assert!(
            Physics::load(
                text.replace("\"VertexIndex\":1", "\"VertexIndex\":9")
                    .as_bytes(),
                &parameters()
            )
            .is_err()
        );
        assert!(
            Physics::load(
                text.replace("\"Delay\":0.8", "\"Delay\":-1").as_bytes(),
                &parameters()
            )
            .is_err()
        );
    }

    fn two_group_rig() -> (Vec<u8>, Vec<RigParameter>) {
        let mut doc: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        doc["Meta"]["PhysicsDictionary"] = serde_json::json!([
            {"Id":"Hair", "Name":"Custom ribbon"}, {"Id":"Tail", "Name":"Custom antenna"}]);
        let mut second = doc["PhysicsSettings"][0].clone();
        second["Id"] = "Tail".into();
        second["Output"][0]["Destination"]["Id"] = "Tail".into();
        doc["PhysicsSettings"].as_array_mut().unwrap().push(second);
        let mut p = parameters();
        let mut tail = p[1].clone();
        tail.id = "Tail".into();
        p.push(tail);
        (serde_json::to_vec(&doc).unwrap(), p)
    }
    fn trajectory(settings: &PhysicsSettings) -> Vec<[f32; 2]> {
        let (bytes, mut p) = two_group_rig();
        let mut physics = Physics::load(&bytes, &p).unwrap();
        physics.configure(settings);
        let mut values = Vec::new();
        for n in 0..240 {
            p[0].value = if n < 20 { 0.0 } else { 0.03 };
            p[1].value = 0.0;
            p[2].value = 0.0;
            physics.update(&mut p, 1.0 / 60.0);
            values.push([p[1].value, p[2].value]);
        }
        values
    }
    #[test]
    fn arbitrary_group_names_are_discovered_and_group_changes_are_isolated() {
        let (bytes, p) = two_group_rig();
        let physics = Physics::load(&bytes, &p).unwrap();
        let groups = physics.groups();
        assert_eq!(groups[0].name, "Custom ribbon");
        assert_eq!(groups[1].outputs, ["Tail"]);
        let base = trajectory(&PhysicsSettings::default());
        let mut settings = PhysicsSettings::default();
        settings.groups.insert(
            "Hair".into(),
            GroupSettings {
                strength: 0.5,
                ..Default::default()
            },
        );
        let tuned = trajectory(&settings);
        assert!(base.iter().any(|v| v[0].abs() > 0.01));
        for (a, b) in base.iter().zip(tuned) {
            assert!((a[0] * 0.5 - b[0]).abs() < 1e-6);
            assert_eq!(a[1], b[1], "An unrelated group must remain unchanged");
        }
        settings.groups.get_mut("Hair").unwrap().enabled = false;
        let muted = trajectory(&settings);
        assert!(muted.iter().all(|v| v[0] == 0.0));
        for (a, b) in base.iter().zip(muted) {
            assert_eq!(a[1], b[1]);
        }
        settings.groups.clear();
        settings.strength = 0.5;
        for (a, b) in base.iter().zip(trajectory(&settings)) {
            assert!((a[0] * 0.5 - b[0]).abs() < 1e-6 && (a[1] * 0.5 - b[1]).abs() < 1e-6);
        }
    }
    #[test]
    fn particle_tuning_changes_motion_and_extremes_stay_finite() {
        let base = trajectory(&PhysicsSettings::default());
        for group in [
            GroupSettings {
                inertia: 0.2,
                ..Default::default()
            },
            GroupSettings {
                response: 0.4,
                ..Default::default()
            },
            GroupSettings {
                gravity: 0.2,
                ..Default::default()
            },
            GroupSettings {
                wind: 1.0,
                ..Default::default()
            },
        ] {
            let settings = PhysicsSettings {
                groups: BTreeMap::from([("Hair".into(), group)]),
                ..Default::default()
            };
            let tuned = trajectory(&settings);
            assert!(
                base.iter()
                    .zip(&tuned)
                    .any(|(a, b)| (a[0] - b[0]).abs() > 0.001)
            );
            assert!(base.iter().zip(&tuned).all(|(a, b)| a[1] == b[1]));
        }
        let settings = PhysicsSettings {
            motion_style: MotionStyle::Bouncy,
            strength: 2.0,
            inertia: 2.0,
            gravity: 2.0,
            response: 2.0,
            wind: -1.0,
            groups: BTreeMap::from([(
                "Hair".into(),
                GroupSettings {
                    strength: 2.0,
                    inertia: 2.0,
                    gravity: 2.0,
                    response: 2.0,
                    wind: -1.0,
                    enabled: true,
                },
            )]),
            enabled: true,
        };
        settings.validate().unwrap();
        assert!(
            trajectory(&settings)
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (-1.0..=1.0).contains(v))
        );
        let (bytes, mut p) = two_group_rig();
        let mut physics = Physics::load(&bytes, &p).unwrap();
        let mut settings = settings;
        for n in 0..180 {
            settings.groups.get_mut("Hair").unwrap().enabled = n % 4 == 0;
            settings.enabled = n % 7 != 0;
            physics.configure(&settings);
            p[0].value = (n as f32).sin();
            p[1].value = 0.3;
            p[2].value = 0.2;
            physics.update(&mut p, 1.0 / 120.0);
            if !settings.enabled || !settings.groups["Hair"].enabled {
                assert_eq!(p[1].value, 0.3);
            }
            assert!(
                p.iter()
                    .all(|p| p.value.is_finite() && (p.min..=p.max).contains(&p.value))
            );
        }
    }
    #[test]
    fn saved_physics_migrates_and_rejects_invalid_groups() {
        let legacy: PhysicsSettings =
            serde_json::from_str(r#"{"enabled":false,"strength":0.6,"wind":-0.3}"#).unwrap();
        assert_eq!(legacy.motion_style, MotionStyle::Authored);
        assert_eq!(PhysicsSettings::default().motion_style, MotionStyle::Bouncy);
        assert!(!legacy.enabled && legacy.inertia == 1.0 && legacy.groups.is_empty());
        legacy.validate().unwrap();
        let mut settings = legacy;
        settings.groups.insert(
            "AnyCustomGroup42".into(),
            GroupSettings {
                response: 0.5,
                wind: 0.8,
                ..Default::default()
            },
        );
        let restored: PhysicsSettings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(restored, settings);
        settings.groups.get_mut("AnyCustomGroup42").unwrap().inertia = f32::NAN;
        assert!(settings.validate().is_err());
        let (bytes, p) = two_group_rig();
        let mut doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        doc["PhysicsSettings"][1]["Id"] = "Hair".into();
        assert!(Physics::load(&serde_json::to_vec(&doc).unwrap(), &p).is_err());
    }

    #[test]
    fn enhanced_settings_corner_matrix_keeps_long_chains_bounded() {
        let mut doc: serde_json::Value = serde_json::from_slice(FIXTURE).unwrap();
        let vertex = doc["PhysicsSettings"][0]["Vertices"][1].clone();
        doc["PhysicsSettings"][0]["Vertices"]
            .as_array_mut()
            .unwrap()
            .extend(std::iter::repeat_n(vertex, 62));
        let bytes = serde_json::to_vec(&doc).unwrap();
        for style in [MotionStyle::Natural, MotionStyle::Bouncy] {
            for bits in 0..64 {
                let high = |bit| bits & (1 << bit) != 0;
                let tuning = GroupSettings {
                    strength: if high(0) { 2.0 } else { 0.0 },
                    inertia: if high(1) { 2.0 } else { 0.0 },
                    response: if high(2) { 2.0 } else { 0.25 },
                    gravity: if high(3) { 2.0 } else { 0.0 },
                    wind: if high(4) { 1.0 } else { -1.0 },
                    enabled: true,
                };
                let mut settings = PhysicsSettings {
                    motion_style: style,
                    strength: tuning.strength,
                    inertia: tuning.inertia,
                    response: tuning.response,
                    gravity: tuning.gravity,
                    wind: tuning.wind,
                    groups: BTreeMap::from([("Hair".into(), tuning)]),
                    ..Default::default()
                };
                settings.validate().unwrap();
                let mut p = parameters();
                let mut physics = Physics::load(&bytes, &p).unwrap();
                for frame in 0..600 {
                    settings.enabled = !high(5) || frame % 11 != 0;
                    settings.groups.get_mut("Hair").unwrap().enabled = !high(5) || frame % 7 != 0;
                    physics.configure(&settings);
                    p[0].value = if frame % 2 == 0 { -1.0 } else { 1.0 };
                    p[1].value = 0.0;
                    let dt = [1.0 / 240.0, 1.0 / 30.0, 0.25, 3.0][frame % 4];
                    physics.update(&mut p, dt);
                    assert!(
                        p.iter()
                            .all(|p| p.value.is_finite() && (p.min..=p.max).contains(&p.value))
                    );
                    if !physics.initialized {
                        continue;
                    }
                    for pair in physics.chains[0].particles.windows(2) {
                        let d = pair[1].pos.sub(pair[0].pos);
                        assert!(
                            (d.x.hypot(d.y) - pair[1].spec.radius).abs() < 0.001,
                            "stretched chain: {style:?}, corner {bits}, frame {frame}"
                        );
                        let v = pair[1].velocity;
                        assert!(v.x.is_finite() && v.y.is_finite());
                        assert!(v.x.hypot(v.y) <= pair[1].spec.radius * 20.001);
                    }
                }
            }
        }
    }

    #[test]
    fn invalid_time_is_noop_and_nonfinite_values_recover_deterministically() {
        for style in [
            MotionStyle::Authored,
            MotionStyle::Natural,
            MotionStyle::Bouncy,
        ] {
            let run = || {
                let mut p = parameters();
                let mut physics = Physics::load(FIXTURE, &p).unwrap();
                physics.configure(&PhysicsSettings {
                    motion_style: style,
                    ..Default::default()
                });
                let mut trace = Vec::new();
                for frame in 0..1200 {
                    p[0].value = (frame as f32 * 0.13).sin();
                    p[1].value = 0.0;
                    for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
                        let before = p.iter().map(|p| p.value).collect::<Vec<_>>();
                        physics.update(&mut p, dt);
                        assert_eq!(before, p.iter().map(|p| p.value).collect::<Vec<_>>());
                    }
                    if frame % 17 == 0 {
                        p[0].value = f32::INFINITY;
                        p[1].value = f32::NAN;
                    }
                    if frame % 43 == 0 {
                        physics.reset();
                    }
                    physics.update(&mut p, 1.0 / 144.0);
                    assert!(
                        p.iter()
                            .all(|p| p.value.is_finite() && (p.min..=p.max).contains(&p.value))
                    );
                    trace.push(p[1].value);
                }
                trace
            };
            assert_eq!(run(), run(), "replay must be deterministic: {style:?}");
        }
    }
}
