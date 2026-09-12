//! Tessellated display deformation shared by stage, editor, PNG and OBS rendering.
use crate::{items::Anchor, output::Scene};
use aria_core::{
    deformation::{self, Field},
    effects::Particle,
};
use eframe::egui::{self, Color32, Pos2, Vec2, vec2};

#[derive(Clone, Debug, PartialEq)]
pub struct AvatarDent {
    pub anchor: Anchor,
    pub field: Field,
}
pub fn avatar_fields(scene: &Scene, canvas: egui::Rect, zoom: f32) -> Vec<Field> {
    scene
        .dents
        .iter()
        .filter_map(|dent| crate::items::dent_field(scene, canvas, zoom, dent))
        .collect()
}
pub fn point(point: Pos2, fields: &[Field]) -> Pos2 {
    let (p, _) = deformation::apply([point.x, point.y], fields);
    Pos2::new(p[0], p[1])
}
pub fn shade(color: Color32, factor: f32) -> Color32 {
    Color32::from_rgba_premultiplied(
        (color.r() as f32 * factor) as u8,
        (color.g() as f32 * factor) as u8,
        (color.b() as f32 * factor) as u8,
        color.a(),
    )
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectWarp {
    pub field: Field,
    pub squash: f32,
}
impl ObjectWarp {
    pub fn from_particle(p: &Particle) -> Option<Self> {
        if !p.contact {
            return None;
        }
        let response = &p.deformation.object;
        let gain = response.gain(p.age - p.flight) * p.impact_scale();
        if gain.abs() < 0.0001 {
            return None;
        }
        // The contact side stays attached to this copy as it continues spinning.
        let axis = deformation::direction(p.origin, p.target);
        let axis = egui::emath::Rot2::from_angle(-(p.spin * p.flight).to_radians())
            * vec2(axis[0], axis[1]);
        Some(Self {
            field: Field::new(
                [axis.x * 0.28, axis.y * 0.28],
                [-axis.x, -axis.y],
                response,
                gain,
            ),
            squash: (response.squash * gain * 0.55).clamp(-0.4, 0.65),
        })
    }
    fn apply(&self, local: Vec2) -> (Vec2, f32) {
        let (p, shade) = deformation::apply([local.x, local.y], &[self.field]);
        let p = vec2(p[0], p[1]);
        let axis = vec2(self.field.direction[0], self.field.direction[1]);
        let along = axis * p.dot(axis);
        (
            along * (1.0 - self.squash) + (p - along) * (1.0 + self.squash * 0.4),
            shade,
        )
    }
}
pub fn textured_mesh(
    texture: egui::TextureId,
    center: Pos2,
    size: Vec2,
    angle: f32,
    color: Color32,
    object: Option<&ObjectWarp>,
    fields: &[Field],
) -> egui::Mesh {
    let steps = if object.is_some() {
        24
    } else if !fields.is_empty() {
        64
    } else {
        1
    };
    let mut mesh = egui::Mesh::with_texture(texture);
    mesh.vertices.reserve((steps + 1) * (steps + 1));
    mesh.indices.reserve(steps * steps * 6);
    let rotation = egui::emath::Rot2::from_angle(angle);
    for y in 0..=steps {
        for x in 0..=steps {
            let uv = vec2(x as f32 / steps as f32, y as f32 / steps as f32);
            let local = (uv - Vec2::splat(0.5)) * size / size.y;
            let (local, light) = object.map_or((local, 1.0), |o| o.apply(local));
            let pos = center + rotation * (local * size.y);
            let (pos, light2) = deformation::apply([pos.x, pos.y], fields);
            mesh.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(pos[0], pos[1]),
                uv: uv.to_pos2(),
                color: shade(color, light * light2),
            });
        }
    }
    for y in 0..steps {
        for x in 0..steps {
            let a = (y * (steps + 1) + x) as u32;
            let b = a + 1;
            let c = a + (steps + 1) as u32;
            let d = c + 1;
            mesh.indices.extend([a, b, d, a, d, c]);
        }
    }
    mesh
}

#[cfg(feature = "screenshots")]
pub fn verify_smoke(
    ctx: &egui::Context,
    state: &eframe::egui_wgpu::RenderState,
    scene: &Scene,
    path: &std::path::Path,
) {
    let base = path.with_extension("");
    let save = |name: &str, scene: &Scene| {
        let file = std::path::PathBuf::from(format!("{}-{name}.png", base.display()));
        crate::broadcast::save_png(ctx, state, scene, &file).expect("Deformation PNG export");
        image::open(file).unwrap().into_rgba8()
    };
    let mut avatar = scene.clone();
    avatar.effects = Default::default();
    let dented = save("avatar-dented", &avatar);
    avatar.dents = Default::default();
    let original = save("avatar-original", &avatar);
    let changed = dented
        .pixels()
        .zip(original.pixels())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 500,
        "Avatar dent must change visible pixels: {changed}"
    );
    assert_eq!(
        dented.get_pixel(0, 0).0[3],
        0,
        "Deformation must preserve transparent output"
    );
    let restored = save("avatar-restored", &avatar);
    assert_eq!(
        original.as_raw(),
        restored.as_raw(),
        "Clearing deformation must restore original render exactly"
    );
    let mut objects = scene.clone();
    objects.dents = Default::default();
    let deformed = save("objects-dented", &objects);
    objects.effects = objects
        .effects
        .iter()
        .cloned()
        .map(|mut d| {
            d.deformation = None;
            d
        })
        .collect::<Vec<_>>()
        .into();
    let normal = save("objects-original", &objects);
    let object_pixels = deformed
        .pixels()
        .zip(normal.pixels())
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        object_pixels > 100,
        "Object dents must change visible pixels"
    );
    eprintln!(
        "Deformation PNGs verified: {changed} avatar pixels, {object_pixels} object pixels changed; exact restoration and alpha preserved"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_deform_independently_only_after_contact_then_recover() {
        let mut sim = aria_core::effects::Simulation::default();
        sim.trigger(&aria_core::effects::Design {
            count: 2,
            interval: 0.0,
            deformation: aria_core::deformation::Settings::gentle(),
            ..Default::default()
        })
        .unwrap();
        sim.tick(0.1);
        assert!(
            sim.particles
                .iter()
                .all(|p| ObjectWarp::from_particle(p).is_none())
        );
        let a = &mut sim.particles[0];
        a.contact = true;
        a.age = a.flight;
        let warp = ObjectWarp::from_particle(a).unwrap();
        assert_ne!(warp.apply(vec2(0.2, 0.1)).0, vec2(0.2, 0.1));
        assert!(ObjectWarp::from_particle(&sim.particles[1]).is_none());
        let a = &mut sim.particles[0];
        a.age = a.flight + a.deformation.object.duration() + 0.01;
        assert!(ObjectWarp::from_particle(a).is_none());
    }
    #[test]
    fn dents_move_texture_geometry_keep_uv_and_alpha_and_scale_with_output() {
        let response = aria_core::deformation::Response {
            enabled: true,
            radius: 50.0,
            ..Default::default()
        };
        let field = Field::new([100.0, 100.0], [1.0, 0.0], &response, 1.0);
        let color = Color32::from_rgba_unmultiplied(220, 170, 110, 180);
        let a = textured_mesh(
            Default::default(),
            Pos2::new(100.0, 100.0),
            Vec2::splat(200.0),
            0.0,
            color,
            None,
            &[field],
        );
        let b = textured_mesh(
            Default::default(),
            Pos2::new(200.0, 200.0),
            Vec2::splat(400.0),
            0.0,
            color,
            None,
            &[Field {
                center: [200.0, 200.0],
                radius: 100.0,
                ..field
            }],
        );
        let center = &a.vertices[32 * 65 + 32];
        assert!(center.pos.x > 100.0);
        assert_eq!(center.uv, Pos2::new(0.5, 0.5));
        assert_eq!(center.color.a(), color.a());
        assert!(center.color.r() < color.r());
        for (a, b) in a.vertices.iter().zip(&b.vertices) {
            assert!((a.pos.to_vec2() * 2.0 - b.pos.to_vec2()).length() < 0.001);
            assert_eq!(a.uv, b.uv);
        }
    }
}
