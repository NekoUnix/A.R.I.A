//! Rust particle-chain solver for authored physics3 rigs. Evaluates fixed steps
//! before Cubism Core; retains momentum and interpolates outputs between steps.
//! This implements the data format, not VTube Studio's proprietary physics modes.
use crate::rig::RigParameter;
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::BTreeMap;

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
    drivers: Vec<Driver>,
    outputs: Vec<Destination>,
    particles: Vec<Particle>,
    normalization: Normalization,
    multiplier: f32,
}

pub struct Physics {
    chains: Vec<Chain>,
    step: f64,
    accumulator: f64,
    previous_inputs: Vec<f32>,
    rest_gravity: V2,
    wind: V2,
    initialized: bool,
    pub enabled: bool,
    pub strength: f32,
    pub wind_strength: f32,
    pub warnings: Vec<String>,
}

impl Physics {
    pub fn load(bytes: &[u8], parameters: &[RigParameter]) -> Result<Self> {
        ensure!(bytes.len() <= 2 * 1024 * 1024, "Physics file exceeds 2 MiB");
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
        let mut runtime = Self {
            chains: Vec::new(),
            step: 1.0
                / if doc.meta.fps >= 1.0 {
                    doc.meta.fps as f64
                } else {
                    60.0
                },
            accumulator: 0.0,
            previous_inputs: Vec::new(),
            rest_gravity: doc.meta.effective_forces.gravity.mul(-1.0).unit(),
            wind: doc.meta.effective_forces.wind,
            initialized: false,
            enabled: true,
            strength: 1.0,
            wind_strength: 0.0,
            warnings: Vec::new(),
        };
        for s in doc.physics_settings {
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
                id: s.id,
                drivers: Vec::new(),
                outputs: Vec::new(),
                normalization: s.normalization,
                multiplier: 1.0,
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
        if !self.enabled {
            self.reset();
            return;
        }
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if dt > 0.5 {
            self.reset();
        }
        let dt = dt.min(0.25) as f64;
        let values: Vec<_> = parameters.iter().map(|p| p.value).collect();
        if !self.initialized || self.previous_inputs.len() != parameters.len() {
            let mut initial = parameters.to_vec();
            for c in &mut self.chains {
                let (translation, gravity) = drivers(c, &initial, self.rest_gravity);
                c.particles[0].pos = translation;
                for i in 1..c.particles.len() {
                    c.particles[i].pos = c.particles[i - 1]
                        .pos
                        .add(gravity.mul(c.particles[i].spec.radius));
                    c.particles[i].velocity = V2::default();
                    c.particles[i].gravity = gravity;
                }
                outputs(c, self.rest_gravity);
                for o in &mut c.outputs {
                    o.previous = o.current;
                }
                apply(c, &mut initial, 1.0, self.strength);
            }
            self.previous_inputs = values.clone();
            self.initialized = true;
        }
        let carried = self.accumulator;
        self.accumulator += dt;
        let mut elapsed = self.step - carried;
        // At most 60 steps after a bounded frame gap, independent of UI FPS.
        while self.accumulator + 1e-9 >= self.step {
            let t = (elapsed / dt).clamp(0.0, 1.0) as f32;
            let mut working = parameters.to_vec();
            for (i, p) in working.iter_mut().enumerate() {
                p.value = self.previous_inputs[i] + (values[i] - self.previous_inputs[i]) * t;
            }
            let wind = self.wind.add(V2 {
                x: self.wind_strength.clamp(-1.0, 1.0) * 0.1,
                y: 0.0,
            });
            for c in &mut self.chains {
                let (translation, gravity) = drivers(c, &working, self.rest_gravity);
                c.particles[0].pos = translation;
                for i in 1..c.particles.len() {
                    let parent = c.particles[i - 1].pos;
                    let p = &mut c.particles[i];
                    let delay = p.spec.delay * self.step as f32 * 30.0;
                    let before = p.pos;
                    let direction = p.pos.sub(parent).rotate(p.gravity.angle(gravity) / 5.0);
                    let force = gravity.mul(p.spec.acceleration).add(wind);
                    let predicted = direction
                        .add(p.velocity.mul(delay))
                        .add(force.mul(delay * delay));
                    p.pos = parent.add(predicted.unit().mul(p.spec.radius));
                    p.velocity = if delay > 1e-6 {
                        p.pos.sub(before).mul(p.spec.mobility / delay)
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
                apply(c, &mut working, 1.0, self.strength);
            }
            self.accumulator = (self.accumulator - self.step).max(0.0);
            elapsed += self.step;
        }
        self.previous_inputs = values;
        let alpha = (self.accumulator / self.step).clamp(0.0, 1.0) as f32;
        for c in &self.chains {
            apply(c, parameters, alpha, self.strength);
        }
    }
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
    for o in &c.outputs {
        let p = &mut parameters[o.index];
        let value = (o.previous + (o.current - o.previous) * alpha)
            * c.multiplier
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
}
