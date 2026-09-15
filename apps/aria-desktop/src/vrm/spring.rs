//! Fixed-step VRM/GLB spring joints with world-space inertia and bounded swing.
use super::asset::{Node, array, index, number, vector};
use anyhow::{Context, Result, ensure};
use aria_core::physics::PhysicsSettings;
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;

#[derive(Clone)]
pub struct Collider {
    pub node: usize,
    pub offset: Vec3,
    pub tail: Vec3,
    pub radius: f32,
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
    previous: Vec3,
    current: Vec3,
    rotation: Quat,
}
#[derive(Default)]
pub struct Simulation {
    tails: Vec<Vec<Tail>>,
    accumulator: f32,
    baseline: Vec<Quat>,
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
                                previous: p,
                                current: p,
                                rotation: rotations[j.node],
                            }
                        })
                        .collect()
                })
                .collect();
        }
        if frozen {
            for (g, tails) in groups.iter().zip(&self.tails) {
                for (j, tail) in g.joints.iter().zip(tails) {
                    rotations[j.node] = tail.rotation;
                }
            }
            world_matrices(nodes, order, rotations, world);
            return;
        }
        self.accumulator = (self.accumulator + dt.clamp(0., 0.1)).min(0.1);
        self.baseline.clear();
        self.baseline.extend_from_slice(rotations);
        let baseline = &self.baseline;
        let step = 1. / 60.;
        while self.accumulator >= step {
            self.accumulator -= step;
            for (g, tails) in groups.iter().zip(&mut self.tails) {
                let tuning = settings.groups.get(&g.id).copied().unwrap_or_default();
                let advanced = secondary.groups.get(&g.id).unwrap_or(&secondary.tuning);
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
                        tail.previous = tail.current;
                        tail.rotation = baseline[j.node];
                        rotations[j.node] = baseline[j.node];
                        world[j.node] = rest;
                        refresh_children(j.node, nodes, rotations, world);
                        continue;
                    }
                    // Keep tips in world space: a head-centered VRM must still lag a head turn.
                    // A large discontinuity is a teleport, not an impulse to integrate.
                    if tail.current.distance(origin) > length * 8. {
                        tail.current = target;
                        tail.previous = target;
                    }
                    let current = tail.current;
                    let previous = tail.previous;
                    let inertia = (1. - j.drag).clamp(0., 0.99)
                        * (1. - advanced.damping)
                        * settings.inertia
                        * tuning.inertia;
                    let force =
                        (j.stiffness * settings.response * tuning.response * 120.).min(1800.);
                    let wind = Vec3::X * (settings.wind + tuning.wind) * 0.5;
                    let mut next = current
                        + (current - previous) * inertia.min(0.98)
                        + (target - current) * force * step * step
                        + (j.gravity * j.power * settings.gravity * tuning.gravity + wind)
                            * length
                            * 60.
                            * step
                            * step;
                    next =
                        origin + (next - origin).try_normalize().unwrap_or(axis / length) * length;
                    for collider in g.colliders.iter().filter(|_| advanced.collisions) {
                        let transform = world[collider.node];
                        let a = transform.transform_point3(collider.offset);
                        let b = transform.transform_point3(collider.tail);
                        let ab = b - a;
                        let along = (next - a).dot(ab) / ab.length_squared().max(1e-10);
                        let closest = a + ab * along.clamp(0., 1.);
                        let delta = next - closest;
                        let radius = collider.radius
                            * transform
                                .to_scale_rotation_translation()
                                .0
                                .abs()
                                .max_element()
                            + j.radius * rest.to_scale_rotation_translation().0.abs().max_element();
                        if delta.length_squared() < radius * radius {
                            next = closest + delta.try_normalize().unwrap_or(Vec3::Y) * radius;
                            next = origin
                                + (next - origin).try_normalize().unwrap_or(axis / length) * length;
                        }
                    }
                    let swing = Quat::from_rotation_arc(axis / length, (next - origin).normalize());
                    let angle = swing.to_axis_angle().1.abs();
                    let max_angle = advanced.swing.to_radians();
                    let swing = if angle > max_angle {
                        Quat::IDENTITY.slerp(swing, max_angle / angle)
                    } else {
                        swing
                    };
                    next = origin + swing * axis;
                    let parent_rotation = parent.to_scale_rotation_translation().1;
                    let wanted =
                        parent_rotation.inverse() * swing * parent_rotation * baseline[j.node];
                    let weight = (settings.strength * tuning.strength).clamp(0., 1.);
                    rotations[j.node] = baseline[j.node].slerp(wanted.normalize(), weight);
                    tail.previous = tail.current;
                    tail.current = next;
                    tail.rotation = rotations[j.node];
                    world[j.node] = parent
                        * Mat4::from_scale_rotation_translation(
                            node.scale,
                            rotations[j.node],
                            node.translation,
                        );
                    refresh_children(j.node, nodes, rotations, world);
                }
            }
            world_matrices(nodes, order, rotations, world);
        }
        // Preserve the latest pose at high refresh rates; disabling takes effect immediately.
        for (g, tails) in groups.iter().zip(&mut self.tails) {
            let tuning = settings.groups.get(&g.id).copied().unwrap_or_default();
            let advanced = secondary.groups.get(&g.id).unwrap_or(&secondary.tuning);
            for (j, tail) in g.joints.iter().zip(tails) {
                if !tuning.enabled || tuning.strength <= 0. || advanced.swing <= 0. {
                    tail.rotation = baseline[j.node];
                    tail.current = world[j.node].transform_point3(j.tip);
                    tail.previous = tail.current;
                }
                rotations[j.node] = tail.rotation;
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
