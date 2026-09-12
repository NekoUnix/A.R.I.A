//! Procedural anime-water materials: colored depth, clear centers, bright glints and foam crowns.
//! Three 192px images per material, generated once and shared by every droplet/output.
use crate::avatar::Sprite;
use aria_core::effects::{Kind, Particle};
use eframe::{egui, egui_wgpu::RenderState};
use std::{collections::BTreeMap, sync::Arc};
pub struct Art {
    pub drop: Sprite,
    pub splat: Sprite,
    pub crown: Sprite,
}
#[derive(Default)]
pub struct Cache(BTreeMap<[u8; 7], Art>);
fn key(p: &Particle) -> [u8; 7] {
    [
        p.tint[0],
        p.tint[1],
        p.tint[2],
        p.tint[3],
        (p.liquid.gloss * 255.0) as u8,
        (p.liquid.clarity * 255.0) as u8,
        (p.liquid.foam * 255.0) as u8,
    ]
}
impl Cache {
    pub fn get(&self, p: &Particle) -> Option<&Art> {
        if p.kind != Kind::Spray || p.asset.to_string_lossy() != "builtin:drop" {
            return None;
        }
        self.0.get(&key(p))
    }
    pub fn prepare(&mut self, ctx: &egui::Context, state: Option<&RenderState>, p: &Particle) {
        let key = key(p);
        if self.0.contains_key(&key) {
            return;
        }
        // Bound cached material memory; clearing playback makes room for new styles.
        if self.0.len() >= 32 {
            return;
        }
        let sprites: Vec<_> = (0..3)
            .map(|shape| {
                let pixels = pixels(key, shape, 192);
                let mut palette = crate::chroma::Palette::default();
                palette.add_rgba(&pixels);
                let data = egui::ColorImage::from_rgba_unmultiplied([192, 192], &pixels);
                let texture = ctx.load_texture(
                    format!("water:{key:?}:{shape}"),
                    data.clone(),
                    egui::TextureOptions::LINEAR,
                );
                if let Some(state) = state {
                    state.renderer.write().update_texture(
                        &state.device,
                        &state.queue,
                        texture.id(),
                        &egui::epaint::ImageDelta::full(data, egui::TextureOptions::LINEAR),
                    );
                }
                Sprite {
                    animation: None,
                    texture,
                    name: "Anime water".into(),
                    size: egui::vec2(192.0, 192.0),
                    model_key: String::new(),
                    palette: Arc::new(palette),
                }
            })
            .collect();
        let mut iter = sprites.into_iter();
        self.0.insert(
            key,
            Art {
                drop: iter.next().unwrap(),
                splat: iter.next().unwrap(),
                crown: iter.next().unwrap(),
            },
        );
    }
}
pub fn pixels(key: [u8; 7], shape: u8, size: usize) -> Vec<u8> {
    let mut result = vec![0; size * size * 4];
    let gloss = key[4] as f32 / 255.0;
    let clarity = key[5] as f32 / 255.0;
    let foam = key[6] as f32 / 255.0;
    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
            let theta = v.atan2(u);
            let radius = if shape == 0 {
                0.65 * (1.0 + v * 0.38)
            } else {
                // Uneven lobes avoid a repeated flower-shaped stamp at each impact.
                0.66 + 0.10 * (theta * 3.0 + 0.5).sin()
                    + 0.045 * (theta * 7.0 + 0.8).cos()
                    + 0.025 * (theta * 11.0).sin()
            };
            let r = (u * u + v * v).sqrt() / radius;
            if r >= 1.0 {
                continue;
            }
            let edge = (r.powi(8) * 0.8).min(1.0);
            let glint = (-((u + 0.23).powi(2) / 0.012 + (v + 0.27).powi(2) / 0.045)).exp();
            let small = (-((u - 0.19).powi(2) / 0.003 + (v - 0.34).powi(2) / 0.009)).exp();
            let rim = (-(r - 0.84).powi(2) / 0.003).exp() * ((theta + 0.7).sin() * 0.5 + 0.5);
            let caustic =
                (-(v - (0.36 + 0.06 * (u * 12.0).sin())).powi(2) / 0.001).exp() * (1.0 - r);
            let spec = ((glint * 1.4 + small + rim * 0.6 + caustic * 0.7) * gloss).clamp(0.0, 1.0);
            let coverage = ((1.0 - r) * size as f32 / 2.0).clamp(0.0, 1.0);
            let a = if shape == 2 {
                (-(r - 0.83).powi(2) / 0.004).exp() * (0.3 + foam * 0.6) + glint * 0.3
            } else {
                0.22 + (1.0 - clarity) * 0.48 + edge * 0.35 + spec * 0.65
            };
            let offset = (y * size + x) * 4;
            for c in 0..3 {
                let base = key[c] as f32 / 255.0 * (0.78 + v * 0.12 - edge * 0.22);
                let light = if shape == 2 {
                    0.7 + base * 0.3
                } else {
                    base * (1.0 - spec) + spec
                };
                result[offset + c] = (light.clamp(0.0, 1.0) * 255.0) as u8;
            }
            result[offset + 3] = (a.clamp(0.0, 1.0) * coverage * key[3] as f32) as u8;
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn materials_have_transparency_white_highlights_and_color_depth() {
        let p = pixels([60, 170, 240, 230, 255, 220, 150], 0, 192);
        assert_eq!(p[3], 0);
        assert!(
            p.as_chunks::<4>()
                .0
                .iter()
                .any(|c| c[0] > 220 && c[1] > 220 && c[2] > 220)
        );
        assert!(p.as_chunks::<4>().0.iter().any(|c| c[3] > 40 && c[3] < 160));
        assert_ne!(p, pixels([240, 80, 140, 230, 255, 220, 150], 0, 192));
    }
}
