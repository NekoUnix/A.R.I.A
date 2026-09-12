//! Bounded PNG/JPEG/GIF decoding, shared immutable frame textures and playback.
use crate::avatar::Sprite;
use anyhow::{Context, Result, ensure};
use aria_core::asset_limits as limits;
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

/// Fit once during import. Never allocate a texture larger than the device supports.
pub fn fit_texture(mut rgba: image::RgbaImage, maximum: u32) -> image::RgbaImage {
    let [w, h] = limits::texture_size(rgba.width(), rgba.height(), maximum);
    if (w, h) == rgba.dimensions() {
        rgba
    } else {
        // Filter premultiplied colors so invisible RGB does not bleed into soft edges.
        // Work in place to avoid an additional source-sized float buffer.
        for pixel in rgba.pixels_mut() {
            let a = u32::from(pixel[3]);
            for c in &mut pixel.0[..3] {
                *c = ((u32::from(*c) * a + 127) / 255) as u8;
            }
        }
        let mut resized =
            image::imageops::resize(&rgba, w, h, image::imageops::FilterType::Triangle);
        for pixel in resized.pixels_mut() {
            let a = u32::from(pixel[3]);
            for c in &mut pixel.0[..3] {
                *c = (u32::from(*c) * 255 + a / 2)
                    .checked_div(a)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
        resized
    }
}
impl Sprite {
    pub fn bytes(&self) -> u64 {
        self.animation.as_ref().map_or_else(
            || {
                let [w, h] = self.texture.size();
                w as u64 * h as u64 * 4
            },
            |a| a.bytes,
        )
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
        .take(limits::IMAGE_FILE + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limits::IMAGE_FILE,
        "Image exceeds 320 MiB on disk"
    );
    let format = image::guess_format(&bytes)?;
    ensure!(
        matches!(format, image::ImageFormat::Png | image::ImageFormat::Gif)
            || (allow_jpeg && format == image::ImageFormat::Jpeg),
        "Choose a real PNG or GIF image"
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(limits::IMAGE_SIDE);
    limits.max_image_height = Some(limits::IMAGE_SIDE);
    limits.max_alloc = Some(limits::IMAGE_DECODED);
    let maximum = state.map_or_else(
        || ctx.input(|i| i.max_texture_side as u32),
        |s| s.device.limits().max_texture_dimension_2d,
    );
    // Imports also run before the first eframe input event updates this value.
    ctx.input_mut(|i| i.max_texture_side = maximum as usize);
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
                total <= limits::IMAGE_DECODED
                    && total.saturating_add(used) <= limits::IMAGE_COLLECTION,
                "GIF/image assets exceed the decoded image budget (1280 MiB per GIF, 2560 MiB per collection)"
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
            total <= limits::IMAGE_DECODED
                && total.saturating_add(used) <= limits::IMAGE_COLLECTION,
            "Image exceeds the decoded image budget (1280 MiB per image, 2560 MiB per collection)"
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
    let [texture_width, texture_height] = limits::texture_size(width, height, maximum);
    let mut uploaded_bytes = 0;
    for (i, (rgba, delay)) in decoded.into_iter().enumerate() {
        ensure!(
            rgba.width() == width && rgba.height() == height,
            "GIF frame canvas mismatch"
        );
        let rgba = fit_texture(rgba, maximum);
        uploaded_bytes += rgba.len() as u64;
        palette.add_rgba(&rgba);
        let pixels = egui::ColorImage::from_rgba_unmultiplied(
            [texture_width as usize, texture_height as usize],
            &rgba,
        );
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
        // Geometry keeps the exact source aspect; accounting uses uploaded dimensions.
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
                bytes: uploaded_bytes,
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
        assert!(load(&ctx, None, &file, limits::IMAGE_COLLECTION, false).is_err());
    }

    #[test]
    fn oversized_png_import_fits_gpu_and_keeps_model_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wide.png");
        let rgba = image::RgbaImage::from_pixel(40_960, 80, image::Rgba([120, 80, 220, 128]));
        let expected = format!("image:40960x80:{}", aria_core::movement::model_key(&rgba));
        rgba.save(&path).unwrap();
        // A valid PNG larger than the previous on-disk limit (PNG permits trailing data).
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(33 * 1024 * 1024)
            .unwrap();
        let ctx = egui::Context::default();
        ctx.input_mut(|i| i.max_texture_side = 8192);
        let sprite = load(&ctx, None, &path, 0, false).unwrap();
        assert_eq!(sprite.model_key, expected);
        assert_eq!(sprite.texture.size(), [8192, 16]);
        assert_eq!(sprite.size, egui::vec2(40960.0, 80.0));
        assert_eq!(sprite.bytes(), 8192 * 16 * 4);
        ctx.input_mut(|i| i.max_texture_side = 4096);
        let smaller = load(&ctx, None, &path, 0, false).unwrap();
        assert_eq!(smaller.model_key, sprite.model_key);
        assert_eq!(smaller.texture.size(), [4096, 8]);
        assert_eq!(smaller.size, sprite.size);
        let fitted = fit_texture(rgba, 8192);
        for (actual, expected) in fitted
            .get_pixel(200, 8)
            .0
            .into_iter()
            .zip([120, 80, 220, 128])
        {
            assert!(actual.abs_diff(expected) <= 1);
        }
        let mut edge = image::RgbaImage::from_pixel(4, 2, image::Rgba([255, 0, 0, 0]));
        for y in 0..2 {
            for x in 2..4 {
                edge.put_pixel(x, y, image::Rgba([0, 0, 255, 255]));
            }
        }
        let edge = fit_texture(edge, 2);
        assert!(
            edge.pixels()
                .all(|p| p[0] == 0 && p[1] == 0 && (p[3] == 0 || p[2] == 255))
        );
    }
}
