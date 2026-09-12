//! Screen-picked surface attachments, following the same skin and morphs as rendering.
use super::Avatar;
use crate::items::{Anchor, barycentric};
use aria_core::items::Pin;
use eframe::egui::{Vec2, vec2};

impl Avatar {
    pub fn pick_pin(&self, point: Vec2) -> Option<Pin> {
        // One temporary projection per shared vertex, only on a user's click.
        let projected: Vec<Vec<_>> = self
            .asset
            .geometry
            .iter()
            .enumerate()
            .map(|(g, geometry)| {
                (0..geometry.vertices.len())
                    .map(|v| self.renderer.projected_vertex(g, v as u32))
                    .collect()
            })
            .collect();
        let mut best: Option<(f32, Pin)> = None;
        for part in &self.asset.parts {
            if self.asset.materials[part.material].uniform.color[3] <= 0.01 {
                continue;
            }
            for tri in part.indices.as_chunks::<3>().0 {
                let mut vertices = *tri;
                let [Some(a), Some(b), Some(c)] =
                    vertices.map(|v| projected[part.geometry][v as usize])
                else {
                    continue;
                };
                let mut p = [a, b, c];
                let longest = (0..3)
                    .max_by(|&a, &b| {
                        (p[(a + 1) % 3] - p[a])
                            .truncate()
                            .length_squared()
                            .total_cmp(&(p[(b + 1) % 3] - p[b]).truncate().length_squared())
                    })
                    .unwrap_or(0);
                p.rotate_left(longest);
                vertices.rotate_left(longest);
                let points = p.map(|v| vec2(v.x, v.y));
                let Some(weights) = barycentric(point, points) else {
                    continue;
                };
                let depth = p.iter().zip(weights).map(|(p, w)| p.z * w).sum::<f32>();
                if !(0.0..=1.0).contains(&depth) || best.as_ref().is_some_and(|(z, _)| depth >= *z)
                {
                    continue;
                }
                let edge = points[1] - points[0];
                if edge.length() < 1e-7 {
                    continue;
                }
                best = Some((
                    depth,
                    Pin::VrmSurface {
                        geometry: part.geometry,
                        vertices,
                        weights,
                        angle: edge.y.atan2(edge.x),
                        length: edge.length(),
                    },
                ));
            }
        }
        best.map(|(_, pin)| pin)
    }

    pub fn resolve_pin(&self, pin: &Pin) -> Anchor {
        let Pin::VrmSurface {
            geometry,
            vertices,
            weights,
            angle,
            length,
        } = pin
        else {
            return Anchor::Missing;
        };
        let [Some(a), Some(b), Some(c)] =
            vertices.map(|v| self.renderer.projected_vertex(*geometry, v))
        else {
            return Anchor::Missing;
        };
        let p = [a, b, c].map(|p| vec2(p.x, p.y));
        let edge = p[1] - p[0];
        if edge.length() < 1e-7 || *length < 1e-7 {
            return Anchor::Missing;
        }
        let depth = a.z * weights[0] + b.z * weights[1] + c.z * weights[2];
        Anchor::Surface {
            point: p[0] * weights[0] + p[1] * weights[1] + p[2] * weights[2],
            angle: edge.y.atan2(edge.x) - angle,
            scale: (edge.length() / length).clamp(0.25, 4.0),
            opacity: if (0.0..=1.0).contains(&depth) {
                1.0
            } else {
                0.0
            },
        }
    }
}
