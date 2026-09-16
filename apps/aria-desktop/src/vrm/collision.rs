//! Bounded bone collision envelopes and contact projection; no triangle-mesh cloth solver.
use super::{
    asset::Asset,
    secondary::Inventory,
    spring::{Collider, Group},
};
use aria_core::vrm::Secondary;
use glam::{Mat4, Quat, Vec3};
use std::collections::BTreeSet;

fn scale(matrix: Mat4) -> f32 {
    matrix
        .to_scale_rotation_translation()
        .0
        .abs()
        .max_element()
        .max(1e-6)
}

fn envelope(asset: &Asset, inventory: &Inventory, node: usize, size: f32) -> Option<Collider> {
    let [min, max] = *inventory.volumes.get(&node)?;
    let center = (min + max) * 0.5;
    let extent = (max - min) * 0.5;
    let mut axes = [0, 1, 2];
    axes.sort_by(|&a, &b| extent[a].total_cmp(&extent[b]));
    let height = (asset.bounds[1] - asset.bounds[0]).max_element().max(0.001);
    let units = height / scale(asset.nodes[node].world);
    let radius = extent[axes[0]].clamp(units * 0.003, units * 0.12) * size;
    let mut axis = Vec3::ZERO;
    axis[axes[2]] = (extent[axes[2]] - radius).max(0.);
    (radius.is_finite() && center.is_finite()).then_some(Collider {
        node,
        offset: center - axis,
        tail: center + axis,
        radius,
        generated: true,
    })
}

fn related(asset: &Asset, a: usize, b: usize) -> bool {
    let ancestor = |mut child: usize, root: usize| {
        loop {
            if child == root {
                return true;
            }
            let Some(p) = asset.nodes[child].parent else {
                return false;
            };
            child = p;
        }
    };
    ancestor(a, b) || ancestor(b, a)
}

pub fn attach(asset: &Asset, inventory: &Inventory, settings: &Secondary, groups: &mut [Group]) {
    let mut body = Vec::new();
    if settings.collision.body {
        let mut used = BTreeSet::new();
        for (role, &node) in &asset.bones {
            if role.contains("Eye") || role == "jaw" || !used.insert(node) {
                continue;
            }
            if let Some(shape) = envelope(asset, inventory, node, settings.collision.body_size) {
                // A row of capsules fits broad, shallow torsos better than one oversized sphere.
                let [min, max] = inventory.volumes[&node];
                let extent = (max - min) * 0.5;
                let mut axes = [0, 1, 2];
                axes.sort_by(|&a, &b| extent[a].total_cmp(&extent[b]));
                let span = (extent[axes[1]] - shape.radius).max(0.);
                let count = ((span * 2. / (shape.radius * 1.5)).ceil() as usize + 1).clamp(1, 7);
                for i in 0..count {
                    let mut shift = Vec3::ZERO;
                    if count > 1 {
                        shift[axes[1]] = -span + 2. * span * i as f32 / (count - 1) as f32;
                    }
                    body.push(Collider {
                        offset: shape.offset + shift,
                        tail: shape.tail + shift,
                        ..shape.clone()
                    });
                }
            }
        }
    }
    let mut dynamic = Vec::new();
    if settings.collision.other_groups {
        let mut used = BTreeSet::new();
        for group in groups.iter().filter(|g| g.id.starts_with("aria:")) {
            for joint in &group.joints {
                if used.insert(joint.node)
                    && let Some(shape) = envelope(asset, inventory, joint.node, 0.85)
                {
                    dynamic.push(shape);
                }
            }
        }
        // Keep the largest meaningful envelopes. Work and memory stay bounded on huge rigs.
        dynamic.sort_by(|a, b| {
            (b.radius * scale(asset.nodes[b.node].world))
                .total_cmp(&(a.radius * scale(asset.nodes[a.node].world)))
                .then(a.node.cmp(&b.node))
        });
        dynamic.truncate(256);
    }
    for group in groups.iter_mut().filter(|g| g.id.starts_with("aria:")) {
        for joint in &mut group.joints {
            joint.radius = joint.tip.length() * settings.collision.thickness;
        }
        group.colliders.extend(body.iter().cloned());
        group.colliders.extend(
            dynamic
                .iter()
                .filter(|c| !group.joints.iter().any(|j| related(asset, j.node, c.node)))
                .cloned(),
        );
    }
}

#[derive(Clone, Copy)]
pub struct WorldCollider {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
    rest_a: Vec3,
    rest_b: Vec3,
    rest_radius: f32,
    generated: bool,
}
impl WorldCollider {
    fn nearby(&self, origin: Vec3, length: f32, thickness: f32) -> bool {
        let reach = length + self.a.distance(self.b) * 0.5 + self.radius + thickness;
        origin.distance_squared((self.a + self.b) * 0.5) <= reach * reach
    }
    pub fn new(c: &Collider, reference: &[Mat4], world: &[Mat4]) -> Self {
        let pose = world[c.node];
        let rest = reference[c.node];
        Self {
            a: pose.transform_point3(c.offset),
            b: pose.transform_point3(c.tail),
            radius: c.radius * scale(pose),
            rest_a: rest.transform_point3(c.offset),
            rest_b: rest.transform_point3(c.tail),
            rest_radius: c.radius * scale(rest),
            generated: c.generated,
        }
    }
    fn radius_at(&self, rest_point: Vec3, thickness: f32, rest_thickness: f32) -> f32 {
        let radius = self.radius + thickness;
        if !self.generated {
            return radius;
        }
        // Generated shapes are approximate. Preserve clearance in the current kinematic
        // pose (including relaxed arms/gestures), not the export's T-pose. Otherwise an
        // arm lowered by the pose controls can force a skirt to alternate between limits.
        let rest_distance = rest_point.distance(closest(rest_point, self.rest_a, self.rest_b));
        radius.min(rest_distance * (radius / (self.rest_radius + rest_thickness).max(1e-6)))
    }
}

fn closest(point: Vec3, a: Vec3, b: Vec3) -> Vec3 {
    let ab = b - a;
    a + ab * ((point - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0., 1.)
}

/// Contact recovery is positional, not extra kinetic energy. Keep tangential
/// velocity on the bone's sphere and remove only motion into the contact surface.
pub fn slide_velocity(velocity: Vec3, correction: Vec3, radial: Vec3) -> Vec3 {
    let mut velocity = velocity - radial * velocity.dot(radial);
    let normal = correction - radial * correction.dot(radial);
    if let Some(normal) = normal.try_normalize() {
        velocity -= normal * velocity.dot(normal).min(0.);
    }
    velocity
}

/// Closest segment parameters, including degenerate sphere colliders.
fn segment_pair(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> (f32, Vec3, Vec3) {
    let u = b - a;
    let v = d - c;
    let w = a - c;
    let uu = u.length_squared();
    let vv = v.length_squared();
    let uv = u.dot(v);
    let uw = u.dot(w);
    let vw = v.dot(w);
    if uu < 1e-12 {
        return (0., a, closest(a, c, d));
    }
    if vv < 1e-12 {
        let s = (-uw / uu).clamp(0., 1.);
        return (s, a + u * s, c);
    }
    let den = uu * vv - uv * uv;
    let mut s = if den > 1e-12 {
        ((uv * vw - vv * uw) / den).clamp(0., 1.)
    } else {
        0.
    };
    let mut t = (uv * s + vw) / vv;
    if t < 0. {
        t = 0.;
        s = (-uw / uu).clamp(0., 1.);
    } else if t > 1. {
        t = 1.;
        s = ((uv - uw) / uu).clamp(0., 1.);
    }
    (s, a + u * s, c + v * t)
}

fn bend(origin: Vec3, rest: Vec3, point: Vec3, limit: f32) -> Vec3 {
    let axis = rest - origin;
    let length = axis.length().max(1e-6);
    let swing = super::spring::direction_rotation(
        axis / length,
        (point - origin).try_normalize().unwrap_or(axis / length),
    );
    let angle = swing.to_axis_angle().1.abs();
    let swing = if angle > limit {
        Quat::IDENTITY.slerp(swing, limit / angle)
    } else {
        swing
    };
    origin + swing * axis
}

#[allow(clippy::too_many_arguments)]
pub fn resolve(
    origin: Vec3,
    rest: Vec3,
    previous: Vec3,
    proposed: Vec3,
    bind_origin: Vec3,
    bind_tip: Vec3,
    thickness: f32,
    rest_thickness: f32,
    limit: f32,
    shapes: &[WorldCollider],
) -> (Vec3, usize) {
    let mut next = bend(origin, rest, proposed, limit);
    if shapes.is_empty() {
        return (next, 0);
    }
    let mut contacts = 0;
    let contact_radius = |shape: &WorldCollider, t: f32| {
        let radius = shape.radius_at(bind_origin.lerp(bind_tip, t), thickness, rest_thickness);
        if shape.generated {
            // A preceding spring can move this joint's anchor. A child cannot resolve
            // that overlap by rotating its own tip; preserve a feasible local rest
            // direction rather than launching the child to the opposite bend limit.
            let p = origin.lerp(rest, t);
            radius.min(p.distance(closest(p, shape.a, shape.b)))
        } else {
            radius
        }
    };
    // Swept tip test catches fast crossings even when both end positions are outside.
    for shape in shapes
        .iter()
        .filter(|c| c.nearby(origin, (rest - origin).length(), thickness))
    {
        let radius = contact_radius(shape, 1.);
        if radius <= 1e-6 || previous.distance(closest(previous, shape.a, shape.b)) < radius {
            continue;
        }
        let (along, point, center) = segment_pair(previous, next, shape.a, shape.b);
        if point.distance(center) < radius && along > 0. {
            let (mut low, mut high) = (0., along);
            for _ in 0..12 {
                let mid = (low + high) * 0.5;
                let p = previous.lerp(next, mid);
                if p.distance(closest(p, shape.a, shape.b)) < radius {
                    high = mid;
                } else {
                    low = mid;
                }
            }
            next = bend(origin, rest, previous.lerp(next, low), limit);
            contacts += 1;
        }
    }
    let penetration = |end: Vec3| -> f32 {
        shapes
            .iter()
            .filter(|c| c.nearby(origin, (rest - origin).length(), thickness))
            .map(|shape| {
                let start = if shape.generated { 0.15 } else { 1. };
                let (s, p, center) = segment_pair(origin.lerp(end, start), end, shape.a, shape.b);
                let t = start + s * (1. - start);
                (contact_radius(shape, t) - p.distance(center)).max(0.)
            })
            .fold(0., f32::max)
    };
    let mut best = next;
    let mut error = penetration(next);
    let generated = shapes.iter().all(|c| c.generated);
    // A loose feasibility threshold lets tightly packed generated chains alternate
    // between different contact boundaries. Solve more precisely, down to the
    // coordinate precision, without adding an outward bias on every correction.
    let epsilon = if generated {
        ((rest - origin).length().max(0.001) * 1e-5)
            .max(rest.abs().max(origin.abs()).max_element().max(1.) * f32::EPSILON)
    } else {
        (rest - origin).length().max(0.001) * 1e-4
    };
    for _ in 0..10 {
        if error <= epsilon {
            break;
        }
        for shape in shapes {
            // Sphere bound eliminates distant shapes before segment/capsule contact work.
            if origin.distance((shape.a + shape.b) * 0.5)
                > (rest - origin).length()
                    + shape.a.distance(shape.b) * 0.5
                    + shape.radius
                    + thickness
            {
                continue;
            }
            let start = if shape.generated { 0.15 } else { 1. };
            let (s, p, center) = segment_pair(origin.lerp(next, start), next, shape.a, shape.b);
            let t = start + s * (1. - start);
            let radius = contact_radius(shape, t);
            let delta = p - center;
            if delta.length() + epsilon < radius {
                let fallback =
                    origin.lerp(rest, t) - closest(origin.lerp(rest, t), shape.a, shape.b);
                let direction = delta
                    .try_normalize()
                    .or_else(|| fallback.try_normalize())
                    .unwrap_or(Vec3::X);
                let corrected = center + direction * radius;
                next = bend(
                    origin,
                    rest,
                    origin + (corrected - origin) / t.max(0.01),
                    limit,
                );
                contacts += 1;
            }
        }
        let current = penetration(next);
        if current < error {
            error = current;
            best = next;
        }
    }
    // Bend/length constraints can conflict with projection. A valid rest direction is safer
    // than publishing a newly penetrating pose. Authored conflicts keep the least error.
    if error > epsilon && penetration(rest) < error {
        if generated && penetration(rest) <= epsilon {
            // Recover toward the closest feasible point, rather than snapping the
            // entire joint to neutral when several approximate envelopes disagree.
            let (mut low, mut high) = (0., 1.);
            for _ in 0..12 {
                let mid = (low + high) * 0.5;
                let point = bend(origin, rest, rest.lerp(best, mid), limit);
                if penetration(point) <= epsilon {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            best = bend(origin, rest, rest.lerp(best, low), limit);
        } else {
            best = rest;
        }
    }
    (best, contacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_keeps_sliding_velocity_and_does_not_add_projection_energy() {
        let result = slide_velocity(Vec3::new(1., -2., 3.), Vec3::Y, Vec3::X);
        assert!(result.abs_diff_eq(Vec3::new(0., 0., 3.), 1e-6));
        assert_eq!(slide_velocity(Vec3::ZERO, Vec3::Y, Vec3::X), Vec3::ZERO);
        assert_eq!(slide_velocity(Vec3::Y, Vec3::Y, Vec3::X), Vec3::Y);
    }
    #[test]
    fn generated_clearance_follows_the_posed_skeleton_without_disabling_contact() {
        // An arm lowered beside a coat already overlaps the approximate capsule in
        // this pose. It must not kick a motionless coat out to the spring bend limit.
        let collider = Collider {
            node: 0,
            offset: Vec3::ZERO,
            tail: Vec3::ZERO,
            radius: 0.25,
            generated: true,
        };
        let posed = [Mat4::from_translation(Vec3::new(0.12, 0.6, 0.))];
        let shape = WorldCollider::new(&collider, &posed, &posed);
        let (end, contacts) = resolve(
            Vec3::ZERO,
            Vec3::Y,
            Vec3::Y,
            Vec3::Y,
            Vec3::ZERO,
            Vec3::Y,
            0.,
            0.,
            0.7,
            &[shape],
        );
        assert!(end.abs_diff_eq(Vec3::Y, 1e-5) && contacts == 0);
        let proposed = Vec3::new(0.1, 1., 0.).normalize();
        let (end, contacts) = resolve(
            Vec3::ZERO,
            Vec3::Y,
            Vec3::Y,
            proposed,
            Vec3::ZERO,
            Vec3::Y,
            0.,
            0.,
            0.7,
            &[shape],
        );
        assert!(
            contacts > 0 && end.x < proposed.x,
            "Still blocks deeper spring penetration"
        );
        let authored = WorldCollider::new(
            &Collider {
                generated: false,
                ..collider
            },
            &posed,
            &posed,
        );
        // Authored VRM shapes keep their full radius rather than adopting generated clearance.
        assert_eq!(authored.radius_at(Vec3::Y, 0., 0.), 0.25);
    }
    fn sphere(center: Vec3, radius: f32, generated: bool) -> WorldCollider {
        WorldCollider {
            a: center,
            b: center,
            radius,
            rest_a: center,
            rest_b: center,
            rest_radius: radius,
            generated,
        }
    }
    #[test]
    fn contact_stays_outside_after_length_and_bend_constraints() {
        let shape = sphere(Vec3::new(0.6, 0.6, 0.), 0.25, false);
        let proposed = Vec3::new(1., 1., 0.).normalize();
        let (end, contacts) = resolve(
            Vec3::ZERO,
            Vec3::Y,
            Vec3::Y,
            proposed,
            Vec3::ZERO,
            Vec3::Y,
            0.,
            0.,
            50f32.to_radians(),
            &[shape],
        );
        assert!(contacts > 0);
        assert!(end.distance(shape.a) >= shape.radius - 0.0002);
        assert!((end.length() - 1.).abs() < 1e-5);
        assert!(end.dot(Vec3::Y).clamp(-1., 1.).acos() <= 50f32.to_radians() + 0.001);
    }
    #[test]
    fn generated_collision_checks_the_bone_segment_not_only_its_tip() {
        let shape = sphere(Vec3::ZERO, 0.3, true);
        let origin = Vec3::NEG_X;
        let rest = origin + Vec3::Y * 2.;
        let (end, contacts) = resolve(
            origin,
            rest,
            rest,
            Vec3::X,
            origin,
            rest,
            0.,
            0.,
            std::f32::consts::FRAC_PI_2,
            &[shape],
        );
        let (_, point, center) = segment_pair(origin.lerp(end, 0.15), end, shape.a, shape.b);
        assert!(contacts > 0);
        assert!(
            point.distance(center) >= 0.3 - 0.0003,
            "A long spring must not cross the body: {end:?}"
        );
        assert!((end.distance(origin) - 2.).abs() < 1e-5);
    }
    #[test]
    fn fast_tip_cannot_jump_through_a_thin_obstacle() {
        let shape = sphere(Vec3::X, 0.08, false);
        let previous = Vec3::new(1., -0.15, 0.).normalize();
        let proposed = Vec3::new(1., 0.15, 0.).normalize();
        let (end, contacts) = resolve(
            Vec3::ZERO,
            previous,
            previous,
            proposed,
            Vec3::ZERO,
            previous,
            0.,
            0.,
            std::f32::consts::FRAC_PI_2,
            &[shape],
        );
        assert!(
            contacts > 0 && end.y < 0.,
            "Must stay on the original side: {end:?}"
        );
        assert!(end.distance(shape.a) >= shape.radius - 0.0002);
    }
    #[test]
    fn generated_rest_overlap_is_preserved_and_collision_disable_is_exact() {
        let shape = sphere(Vec3::ZERO, 1.2, true);
        let (end, contacts) = resolve(
            Vec3::ZERO,
            Vec3::Y,
            Vec3::Y,
            Vec3::Y,
            Vec3::ZERO,
            Vec3::Y,
            0.01,
            0.01,
            0.7,
            &[shape],
        );
        assert!(end.abs_diff_eq(Vec3::Y, 1e-5) && contacts == 0);
        let proposed = Vec3::new(0.4, 1., 0.).normalize();
        let (end, contacts) = resolve(
            Vec3::ZERO,
            Vec3::Y,
            Vec3::Y,
            proposed,
            Vec3::ZERO,
            Vec3::Y,
            0.01,
            0.01,
            0.7,
            &[],
        );
        assert!(end.abs_diff_eq(proposed, 1e-5) && contacts == 0);
    }
}
