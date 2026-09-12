//! Bounded PNG/JPEG/GIF decoding, shared immutable frame textures and playback.
use crate::avatar::Sprite;
use anyhow::{Context, Result, ensure};
use eframe::{egui, egui_wgpu::RenderState};
use image::{AnimationDecoder, ImageDecoder};
use std::{
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
};

pub struct Animation {
    pub frames: Vec<egui::TextureHandle>,
    pub ends: Vec<f32>,
    pub bytes: u64,
}
impl Sprite {
    pub fn bytes(&self) -> u64 {
        self.animation
            .as_ref()
            .map_or((self.size.x * self.size.y * 4.0) as u64, |a| a.bytes)
    }
    pub fn at(&self, time: f32, speed: f32, looping: bool) -> Self {
        let mut sprite = self.clone();
        if let Some(a) = &self.animation {
            let duration = *a.ends.last().unwrap_or(&1.0);
            let t = if time.is_finite() {
                (time.max(0.0) * speed).max(0.0)
            } else {
                0.0
            };
            let t = if looping {
                t.rem_euclid(duration)
            } else {
                t.min(duration)
            };
            let index = a
                .ends
                .partition_point(|end| *end <= t)
                .min(a.frames.len() - 1);
            sprite.texture = a.frames[index].clone();
        }
        sprite
    }
}
pub fn load(
    ctx: &egui::Context,
    state: Option<&RenderState>,
    path: &Path,
    used: u64,
    allow_jpeg: bool,
) -> Result<Sprite> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "Image exceeds 32 MiB on disk"
    );
    let format = image::guess_format(&bytes)?;
    ensure!(
        matches!(format, image::ImageFormat::Png | image::ImageFormat::Gif)
            || (allow_jpeg && format == image::ImageFormat::Jpeg),
        "Choose a real PNG or GIF image"
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let mut decoded = Vec::new();
    let mut total = 0_u64;
    if format == image::ImageFormat::Gif {
        let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(&bytes))?;
        decoder.set_limits(limits)?;
        for frame in decoder.into_frames() {
            ensure!(
                decoded.len() < 256,
                "GIF exceeds 256 frames; shorten or resize the animation"
            );
            let frame = frame.context("Cannot decode GIF frame")?;
            total += frame.buffer().len() as u64;
            ensure!(
                total <= 128 * 1024 * 1024 && total.saturating_add(used) <= 256 * 1024 * 1024,
                "GIF/image assets exceed the decoded image budget (128 MiB per GIF, 256 MiB per collection)"
            );
            let (n, d) = frame.delay().numer_denom_ms();
            let delay = (n as f32 / d as f32 / 1000.0).clamp(0.02, 60.0);
            decoded.push((frame.into_buffer(), delay));
        }
    } else {
        let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
        reader.limits(limits);
        let (w, h) =
            image::ImageReader::with_format(Cursor::new(&bytes), format).into_dimensions()?;
        total = u64::from(w).saturating_mul(u64::from(h)).saturating_mul(4);
        ensure!(
            total.saturating_add(used) <= 256 * 1024 * 1024,
            "Image assets exceed the 256 MiB decoded image budget"
        );
        decoded.push((reader.decode()?.into_rgba8(), 1.0));
    }
    ensure!(!decoded.is_empty(), "Image contains no frames");
    let width = decoded[0].0.width();
    let height = decoded[0].0.height();
    let model_key = if format == image::ImageFormat::Gif {
        format!("gif:{}", aria_core::movement::model_key(&bytes))
    } else {
        format!(
            "image:{width}x{height}:{}",
            aria_core::movement::model_key(&decoded[0].0)
        )
    };
    let mut palette = crate::chroma::Palette::default();
    let mut frames = Vec::new();
    let mut ends = Vec::new();
    let mut elapsed = 0.0;
    for (i, (rgba, delay)) in decoded.into_iter().enumerate() {
        ensure!(
            rgba.width() == width && rgba.height() == height,
            "GIF frame canvas mismatch"
        );
        palette.add_rgba(&rgba);
        let pixels =
            egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &rgba);
        let texture = ctx.load_texture(
            format!("{}:{i}", path.display()),
            pixels.clone(),
            egui::TextureOptions::LINEAR,
        );
        // Offscreen PNG/Spout render before eframe's end-of-frame texture upload.
        if let Some(state) = state {
            state.renderer.write().update_texture(
                &state.device,
                &state.queue,
                texture.id(),
                &egui::epaint::ImageDelta::full(pixels, egui::TextureOptions::LINEAR),
            );
        }
        frames.push(texture);
        elapsed += delay;
        ends.push(elapsed);
    }
    Ok(Sprite {
        texture: frames[0].clone(),
        size: egui::vec2(width as f32, height as f32),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        model_key,
        palette: Arc::new(palette),
        animation: if frames.len() > 1 {
            Some(Arc::new(Animation {
                frames,
                ends,
                bytes: total,
            }))
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gif_frames_timing_alpha_budget_and_independent_clocks() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("motion.gif");
        {
            let mut encoder =
                image::codecs::gif::GifEncoder::new(std::fs::File::create(&file).unwrap());
            for (color, delay) in [([255, 0, 0, 255], 100), ([0, 255, 0, 255], 200)] {
                let mut rgba = image::RgbaImage::from_pixel(16, 16, image::Rgba(color));
                rgba.put_pixel(0, 0, image::Rgba([0; 4]));
                encoder
                    .encode_frame(image::Frame::from_parts(
                        rgba,
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(delay, 1),
                    ))
                    .unwrap();
            }
        }
        let ctx = egui::Context::default();
        let a = load(&ctx, None, &file, 0, false).unwrap();
        assert_eq!(a.animation.as_ref().unwrap().frames.len(), 2);
        assert_eq!(a.bytes(), 2048);
        assert_ne!(
            a.at(0.02, 1.0, true).texture.id(),
            a.at(0.15, 1.0, true).texture.id()
        );
        assert_eq!(a.at(0.32, 1.0, true).texture.id(), a.texture.id());
        assert_ne!(a.at(2.0, 1.0, false).texture.id(), a.texture.id());
        assert!(load(&ctx, None, &file, 256 * 1024 * 1024, false).is_err());
    }
}
