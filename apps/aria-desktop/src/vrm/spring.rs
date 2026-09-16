//! VRM/GLB spring joints with time-scaled inertia, bounded substeps and contact recovery.
use super::asset::{Node, array, index, number, vector};
use anyhow::{Context, Result, ensure};
use aria_core::physics::PhysicsSettings;
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;

// Approximate GLB envelopes must not launch a joint across its bend cone.
// These are per-second limits, independent of display/substep frequency.
const GENERATED_ANGULAR_SPEED: f32 = 4.;
const GENERATED_ANCHOR_LAG_SPEED: f32 = 6.;

/// Shortest rotation between unit directions without discarding sub-degree motion.
/// `Quat::from_rotation_arc` snaps nearly parallel vectors to identity. Applying
/// that shortcut repeatedly to tiny spring steps can pin one mirrored chain while
/// floating-point rounding lets its partner move. Keep the cross product near zero;
/// only the antiparallel case needs an arbitrary perpendicular rotation axis.
pub(super) fn direction_rotation(from: Vec3, to: Vec3) -> Quat {
    let dot = from.dot(to).clamp(-1., 1.);
    if dot < -0.999999 {
        Quat::from_rotation_arc(from, to)
    } else {
        let cross = from.cross(to);
        Quat::from_xyzw(cross.x, cross.y, cross.z, 1. + dot).normalize()
    }
}

#[derive(Clone)]
pub struct Collider {
    pub node: usize,
    pub offset: Vec3,
    pub tail: Vec3,
    pub radius: f32,
    pub generated: bool,
}
#[derive(Clone)]
pub struct Joint {
    pub node: usize,
    pub tip: Vec3,
    pub stiffness: f32,
    pub drag: f32,
    pub gravity: Vec3,
    pub power: f32,
    pub radius: f32,
}
#[derive(Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub center: Option<usize>,
    pub joints: Vec<Joint>,
    pub colliders: Vec<Collider>,
}
#[derive(Clone, Copy)]
struct Tail {
    velocity: Vec3,
    current: Vec3,
    rotation: Quat,
    rendered: Quat,
    origin: Vec3,
}
#[derive(Default)]
pub struct Simulation {
    tails: Vec<Vec<Tail>>,
    input_previous: Vec<Quat>,
    input_target: Vec<Quat>,
    baseline: Vec<Quat>,
    reference_world: Vec<Mat4>,
    collision_world: Vec<Mat4>,
    collision_rotations: Vec<Quat>,
    collision_scratch: Vec<super::collision::WorldCollider>,
    pub contacts: usize,
}

pub fn parse(json: &Value, vrm: &Value, old: bool, nodes: &[Node]) -> Result<Vec<Group>> {
    let mut groups = Vec::new();
    let node_index = |v: &Value| -> Result<usize> {
        let i = index(v).context("VRM spring node is missing")?;
        ensure!(i < nodes.len(), "Invalid spring bone or collider node");
        Ok(i)
    };
    let end_tip = |i: usize| {
        nodes[i].children.first().map_or_else(
            || nodes[i].translation.try_normalize().unwrap_or(Vec3::Y) * 0.07,
            |&c| nodes[c].translation,
        )
    };
    if old {
        let secondary = &vrm["secondaryAnimation"];
        let mut colliders = Vec::new();
        for c in array(&secondary["colliderGroups"]) {
            let node = node_index(&c["node"])?;
            let list = array(&c["colliders"])
                .iter()
                .map(|c| {
                    // VRM 0 collider offsets retain Unity's local Z direction.
                    // Convert the offset to glTF space before the avatar front rotation.
                    let offset = vector(&c["offset"], Vec3::ZERO) * Vec3::new(1., 1., -1.);
                    Collider {
                        node,
                        offset,
                        tail: offset,
                        radius: number(&c["radius"], 0.),
                        generated: false,
                    }
                })
                .collect::<Vec<_>>();
            colliders.push(list);
        }
        for (i, g) in array(&secondary["boneGroups"]).iter().enumerate() {
            let mut joints = Vec::new();
            let mut used = std::collections::BTreeSet::new();
            for root in array(&g["bones"]) {
                let mut stack = vec![node_index(root)?];
                while let Some(node) = stack.pop() {
                    ensure!(used.insert(node), "Repeated node inside a VRM spring group");
                    let tip = end_tip(node);
                    if tip.length_squared() > 1e-10 {
                        joints.push(Joint {
                            node,
                            tip,
                            stiffness: number(&g["stiffiness"], 1.),
                            drag: number(&g["dragForce"], 0.4),
                            gravity: vector(&g["gravityDir"], -Vec3::Y),
                            power: number(&g["gravityPower"], 0.),
                            radius: number(&g["hitRadius"], 0.),
                        });
                    }
                    stack.extend(nodes[node].children.iter().rev());
                }
            }
            let mut referenced = Vec::new();
            for c in array(&g["colliderGroups"]) {
                let idx = index(c).context("Invalid collider group")?;
                referenced.extend(
                    colliders
                        .get(idx)
                        .context("Unknown collider group")?
                        .iter()
                        .cloned(),
                );
            }
            let center = index(&g["center"]);
            ensure!(
                center.is_none_or(|n| n < nodes.len()),
                "Invalid spring center"
            );
            groups.push(Group {
                id: format!("vrm:spring:{i}"),
                name: g["comment"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Spring group")
                    .chars()
                    .take(128)
                    .collect(),
                center,
                joints,
                colliders: referenced,
            });
        }
    } else {
        let secondary = &json["extensions"]["VRMC_springBone"];
        let mut colliders = Vec::new();
        for c in array(&secondary["colliders"]) {
            let node = node_index(&c["node"])?;
            let sphere = &c["shape"]["sphere"];
            let capsule = &c["shape"]["capsule"];
            ensure!(
                sphere.is_object() || capsule.is_object(),
                "Unsupported VRM collider shape"
            );
            let shape = if sphere.is_object() { sphere } else { capsule };
            let offset = vector(&shape["offset"], Vec3::ZERO);
            colliders.push(Collider {
                node,
                offset,
                tail: if sphere.is_object() {
                    offset
                } else {
                    vector(&shape["tail"], offset)
                },
                radius: number(&shape["radius"], 0.),
                generated: false,
            });
        }
        for (i, g) in array(&secondary["springs"]).iter().enumerate() {
            let source = array(&g["joints"]);
            let mut joints = Vec::new();
            for (k, j) in source.iter().enumerate() {
                let node = node_index(&j["node"])?;
                let tip = if let Some(next) = source.get(k + 1) {
                    let next = node_index(&next["node"])?;
                    let mut parent = nodes[next].parent;
                    let mut found = false;
                    while let Some(p) = parent {
                        if p == node {
                            found = true;
                            break;
                        }
                        parent = nodes[p].parent;
                    }
                    ensure!(found, "VRM spring joints must follow their node hierarchy");
                    nodes[node]
                        .world
                        .inverse()
                        .transform_point3(nodes[next].world.transform_point3(Vec3::ZERO))
                } else {
                    continue;
                };
                if tip.length_squared() > 1e-10 {
                    joints.push(Joint {
                        node,
                        tip,
                        stiffness: number(&j["stiffness"], 1.),
                        drag: number(&j["dragForce"], 0.5),
                        gravity: vector(&j["gravityDir"], -Vec3::Y),
                        power: number(&j["gravityPower"], 0.),
                        radius: number(&j["hitRadius"], 0.),
                    });
                }
            }
            let mut referenced = Vec::new();
            for group in array(&g["colliderGroups"]) {
                let group = index(group).context("Invalid collider group")?;
                let group = array(&secondary["colliderGroups"])
                    .get(group)
                    .context("Unknown collider group")?;
                for c in array(&group["colliders"]) {
                    referenced.push(
                        colliders
                            .get(index(c).context("Invalid collider")?)
                            .context("Unknown collider")?
                            .clone(),
                    );
                }
            }
            let center = index(&g["center"]);
            ensure!(
                center.is_none_or(|n| n < nodes.len()),
                "Invalid spring center"
            );
            groups.push(Group {
                id: format!("vrm:spring:{i}"),
                name: g["name"]
                    .as_str()
                    .unwrap_or("Spring group")
                    .chars()
                    .take(128)
                    .collect(),
                center,
                joints,
                colliders: referenced,
            });
        }
    }
    ensure!(
        groups.len() <= 256 && groups.iter().map(|g| g.joints.len()).sum::<usize>() <= 4096,
        "VRM spring rig is too large"
    );
    for group in &groups {
        ensure!(group.colliders.len() <= 1024, "Too many VRM colliders");
        for j in &group.joints {
            ensure!(
                j.tip.is_finite()
                    && j.gravity.is_finite()
                    && (0.0..=100.).contains(&j.stiffness)
                    && (0.0..=1.).contains(&j.drag)
                    && (0.0..=10.).contains(&j.radius)
                    && (0.0..=100.).contains(&j.power),
                "Invalid VRM spring properties"
            );
        }
        for c in &group.colliders {
            ensure!(
                c.offset.is_finite() && c.tail.is_finite() && (0.0..=10.).contains(&c.radius),
                "Invalid VRM collider"
            );
        }
    }
    Ok(groups)
}

pub fn world_matrices(nodes: &[Node], order: &[usize], rotations: &[Quat], world: &mut [Mat4]) {
    for &i in order {
        let local = Mat4::from_scale_rotation_translation(
            nodes[i].scale,
            rotations[i],
            nodes[i].translation,
        );
        world[i] = nodes[i].parent.map_or(local, |p| world[p] * local);
    }
}
impl Simulation {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        nodes: &[Node],
        order: &[usize],
        groups: &[Group],
        rotations: &mut [Quat],
        world: &mut [Mat4],
        settings: &PhysicsSettings,
        secondary: &aria_core::vrm::Secondary,
        dt: f32,
        frozen: bool,
    ) {
        if !settings.enabled || settings.strength <= 0. {
            self.reset();
            return;
        }
        if self.tails.len() != groups.len()
            || self
                .tails
                .iter()
                .zip(groups)
                .any(|(t, g)| t.len() != g.joints.len())
            || dt > 0.25
        {
            self.tails = groups
                .iter()
                .map(|g| {
                    g.joints
                        .iter()
                        .map(|j| {
                            let p = world[j.node].transform_point3(j.tip);
                            Tail {
                                velocity: Vec3::ZERO,
                                current: p,
                                rotation: rotations[j.node],
                                rendered: rotations[j.node],
                                origin: world[j.node].transform_point3(Vec3::ZERO),
                            }
                        })
                        .collect()
                })
                .collect();
            self.input_previous.clear();
            self.input_previous.extend_from_slice(rotations);
        }
        if frozen {
            for (g, tails) in groups.iter().zip(&self.tails) {
                for (j, tail) in g.joints.iter().zip(tails) {
                    rotations[j.node] = tail.rendered;
                }
            }
            world_matrices(nodes, order, rotations, world);
            return;
        }
        let dt = if dt.is_finite() {
            dt.clamp(0., 0.1)
        } else {
            0.
        };
        self.input_target.clear();
        self.input_target.extend_from_slice(rotations);
        if self.input_previous.len() != rotations.len() {
            self.input_previous.clone_from(&self.input_target);
        }
        // Advance on every rendered frame. Smaller than 8.3 ms substeps and input
        // interpolation avoid the old 60 Hz held frames / lumped tracking impulses.
        let steps = (dt * 120.).ceil() as usize;
        let step = dt / steps.max(1) as f32;
        self.contacts = 0;
        for substep in 0..steps {
            let alpha = (substep + 1) as f32 / steps as f32;
            self.baseline.clear();
            self.baseline.extend(
                self.input_previous
                    .iter()
                    .zip(&self.input_target)
                    .map(|(a, b)| a.slerp(*b, alpha)),
            );
            let baseline = &self.baseline;
            self.reference_world.resize(nodes.len(), Mat4::IDENTITY);
            world_matrices(nodes, order, baseline, &mut self.reference_world);
            // Every group sees the same previous spring pose on the current tracked
            // skeleton. Taking shapes from a half-updated rig made neighboring groups
            // fight each other and made the result depend on group iteration order.
            self.collision_rotations.clear();
            self.collision_rotations.extend_from_slice(baseline);
            for (g, tails) in groups.iter().zip(&self.tails) {
                let tuning = settings.groups.get(&g.id).copied().unwrap_or_default();
                let advanced = secondary.groups.get(&g.id).unwrap_or(&secondary.tuning);
                if tuning.enabled && tuning.strength > 0. && advanced.swing > 0. {
                    for (j, tail) in g.joints.iter().zip(tails) {
                        self.collision_rotations[j.node] = tail.rotation;
                    }
                }
            }
            self.collision_world.resize(nodes.len(), Mat4::IDENTITY);
            world_matrices(
                nodes,
                order,
                &self.collision_rotations,
                &mut self.collision_world,
            );
            rotations.copy_from_slice(&self.collision_rotations);
            world.copy_from_slice(&self.collision_world);
            for (g, tails) in groups.iter().zip(&mut self.tails) {
                let tuning = settings.groups.get(&g.id).copied().unwrap_or_default();
                let advanced = secondary.groups.get(&g.id).unwrap_or(&secondary.tuning);
                self.collision_scratch.clear();
                if advanced.collisions {
                    self.collision_scratch.extend(g.colliders.iter().map(|c| {
                        super::collision::WorldCollider::new(
                            c,
                            &self.reference_world,
                            &self.collision_world,
                        )
                    }));
                }
                for (j, tail) in g.joints.iter().zip(tails) {
                    let node = &nodes[j.node];
                    let parent = node.parent.map_or(Mat4::IDENTITY, |p| world[p]);
                    let local = Mat4::from_scale_rotation_translation(
                        node.scale,
                        baseline[j.node],
                        node.translation,
                    );
                    let rest = parent * local;
                    let origin = rest.transform_point3(Vec3::ZERO);
                    let target = rest.transform_point3(j.tip);
                    let axis = target - origin;
                    let length = axis.length().max(1e-6);
                    if !tuning.enabled || tuning.strength <= 0. || advanced.swing <= 0. {
                        tail.current = target;
                        tail.velocity = Vec3::ZERO;
                        tail.rotation = baseline[j.node];
                        tail.origin = origin;
                        rotations[j.node] = baseline[j.node];
                        world[j.node] = rest;
                        refresh_children(j.node, nodes, rotations, world);
                        continue;
                    }
                    // Keep tips in world space: a head-centered VRM must still lag a head turn.
                    // A large discontinuity is a teleport, not an impulse to integrate.
                    if tail.current.distance(origin) > length * 8. {
                        tail.current = target;
                        tail.velocity = Vec3::ZERO;
                        tail.origin = origin;
                    }
                    // Tiny chain links cannot absorb a large anchor displacement in
                    // one tick. Carry the excess with their parent, preserving a
                    // bounded amount of world-space lag instead of hitting both limits.
                    let shift = origin - tail.origin;
                    let carry =
                        shift - shift.clamp_length_max(length * GENERATED_ANCHOR_LAG_SPEED * step);
                    tail.current += carry;
                    tail.origin = origin;
                    let current = tail.current;
                    let inertia = (1. - j.drag).clamp(0., 0.99)
                        * (1. - advanced.damping)
                        * settings.inertia
                        * tuning.inertia;
                    let force =
                        (j.stiffness * settings.response * tuning.response * 120.).min(1800.);
                    let wind = Vec3::X * (settings.wind + tuning.wind) * 0.5;
                    let damping = inertia.min(0.98).powf(step * 60.);
                    let velocity = (tail.velocity * damping
                        + (target - current) * force * step
                        + (j.gravity * j.power * settings.gravity * tuning.gravity + wind)
                            * length
                            * 60.
                            * step)
                        / (1. + force * step * step);
                    let mut next = current + velocity * step;
                    next =
                        origin + (next - origin).try_normalize().unwrap_or(axis / length) * length;
                    let swing = direction_rotation(axis / length, (next - origin).normalize());
                    let angle = swing.to_axis_angle().1.abs();
                    let max_angle = advanced.swing.to_radians();
                    let swing = if angle > max_angle {
                        Quat::IDENTITY.slerp(swing, max_angle / angle)
                    } else {
                        swing
                    };
                    // Simulate the full bend. Reusing a strength-blended pose as
                    // particle state applied the slider again on every step and
                    // could freeze weak settings or change them with refresh rate.
                    next = origin + swing * axis;
                    let predicted = next;
                    let (resolved, contacts) = super::collision::resolve(
                        origin,
                        target,
                        current,
                        next,
                        self.reference_world[j.node].transform_point3(Vec3::ZERO),
                        self.reference_world[j.node].transform_point3(j.tip),
                        j.radius * rest.to_scale_rotation_translation().0.abs().max_element(),
                        j.radius
                            * self.reference_world[j.node]
                                .to_scale_rotation_translation()
                                .0
                                .abs()
                                .max_element(),
                        max_angle,
                        &self.collision_scratch,
                    );
                    self.contacts += contacts;
                    if g.id.starts_with("aria:") {
                        // Generated envelopes approximate the mesh. Recover penetration
                        // with a bounded, compliant correction, never a one-frame kick.
                        let from = direction_rotation(
                            axis / length,
                            (current - origin).try_normalize().unwrap_or(axis / length),
                        );
                        let wanted =
                            direction_rotation(axis / length, (resolved - origin).normalize());
                        let angle = from.angle_between(wanted);
                        let fraction = (GENERATED_ANGULAR_SPEED * step / angle.max(1e-6)).min(1.);
                        next = origin + from.slerp(wanted, fraction) * axis;
                        let swing = direction_rotation(axis / length, (next - origin).normalize());
                        let angle = swing.angle_between(Quat::IDENTITY);
                        if angle > max_angle {
                            next = origin + Quat::IDENTITY.slerp(swing, max_angle / angle) * axis;
                        }
                    } else {
                        next = resolved;
                    }
                    let swing = direction_rotation(axis / length, (next - origin).normalize());
                    let parent_rotation = parent.to_scale_rotation_translation().1;
                    let mut wanted =
                        parent_rotation.inverse() * swing * parent_rotation * baseline[j.node];
                    if g.id.starts_with("aria:") {
                        let angle = tail.rotation.angle_between(wanted);
                        wanted = tail.rotation.slerp(
                            wanted,
                            (GENERATED_ANGULAR_SPEED * step / angle.max(1e-6)).min(1.),
                        );
                    }
                    rotations[j.node] = wanted.normalize();
                    world[j.node] = parent
                        * Mat4::from_scale_rotation_translation(
                            node.scale,
                            rotations[j.node],
                            node.translation,
                        );
                    next = world[j.node].transform_point3(j.tip);
                    let mut velocity = (next - current) / step;
                    if contacts > 0 {
                        // Remove only velocity into the correction. Tangential motion
                        // survives, so sliding hair does not stop and restart each tick.
                        velocity = (predicted - current) / step;
                    }
                    let radial = (next - origin).normalize();
                    velocity = super::collision::slide_velocity(
                        velocity,
                        if contacts > 0 {
                            resolved - predicted
                        } else {
                            Vec3::ZERO
                        },
                        radial,
                    );
                    tail.velocity = if g.id.starts_with("aria:") {
                        velocity.clamp_length_max(length * GENERATED_ANGULAR_SPEED)
                    } else {
                        velocity
                    };
                    tail.current = next;
                    tail.rotation = rotations[j.node];
                    refresh_children(j.node, nodes, rotations, world);
                }
            }
            world_matrices(nodes, order, rotations, world);
        }
        self.input_previous.clone_from(&self.input_target);
        // Preserve the latest pose at high refresh rates; disabling takes effect immediately.
        let baseline = &self.input_target;
        rotations.copy_from_slice(baseline);
        for (g, tails) in groups.iter().zip(&mut self.tails) {
            let tuning = settings.groups.get(&g.id).copied().unwrap_or_default();
            let advanced = secondary.groups.get(&g.id).unwrap_or(&secondary.tuning);
            for (j, tail) in g.joints.iter().zip(tails) {
                if !tuning.enabled || tuning.strength <= 0. || advanced.swing <= 0. {
                    tail.rotation = baseline[j.node];
                    tail.current = world[j.node].transform_point3(j.tip);
                    tail.velocity = Vec3::ZERO;
                    tail.origin = world[j.node].transform_point3(Vec3::ZERO);
                }
                let weight = (settings.strength * tuning.strength).clamp(0., 1.);
                tail.rendered = baseline[j.node].slerp(tail.rotation, weight);
                rotations[j.node] = tail.rendered;
            }
        }
        world_matrices(nodes, order, rotations, world);
    }
}
fn refresh_children(parent: usize, nodes: &[Node], rotations: &[Quat], world: &mut [Mat4]) {
    for &i in &nodes[parent].children {
        world[i] = world[parent]
            * Mat4::from_scale_rotation_translation(
                nodes[i].scale,
                rotations[i],
                nodes[i].translation,
            );
        refresh_children(i, nodes, rotations, world);
    }
}
