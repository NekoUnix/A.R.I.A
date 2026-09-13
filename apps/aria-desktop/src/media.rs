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
#[derive(Clone, Debug)]
pub struct Info {
    pub size: [u32; 2],
    pub frames: usize,
    pub duration: f32,
    pub file_bytes: u64,
}
impl Info {
    pub fn source_bytes(&self) -> u64 {
        u64::from(self.size[0]) * u64::from(self.size[1]) * 4 * self.frames as u64
    }
    pub fn playback_size(&self, maximum: u32, budget: u64) -> [u32; 2] {
        let per_frame = budget / self.frames.max(1) as u64;
        let pixels = u64::from(self.size[0]) * u64::from(self.size[1]) * 4;
        let ratio = (per_frame as f64 / pixels.max(1) as f64).sqrt().min(1.0);
        let edge = ((self.size[0].max(self.size[1]) as f64 * ratio).floor() as u32).max(1);
        limits::texture_size(self.size[0], self.size[1], maximum.min(edge))
    }
}
fn inspect_bytes(bytes: &[u8], format: image::ImageFormat) -> Result<Info> {
    let (size, frames, duration) = if format == image::ImageFormat::Gif {
        let mut options = gif::DecodeOptions::new();
        options.skip_frame_decoding(true);
        options.set_memory_limit(gif::MemoryLimit::Bytes(
            limits::IMAGE_FILE.try_into().unwrap(),
        ));
        let mut reader = options.read_info(Cursor::new(bytes))?;
        let size = [u32::from(reader.width()), u32::from(reader.height())];
        let mut frames = 0;
        let mut duration = 0.0;
        while let Some(frame) = reader.read_next_frame()? {
            frames += 1;
            ensure!(frames <= limits::GIF_FRAMES, "GIF exceeds 4096 frames");
            duration += (f32::from(frame.delay) / 100.0).clamp(0.02, 60.0);
        }
        (size, frames, duration)
    } else {
        let (w, h) =
            image::ImageReader::with_format(Cursor::new(bytes), format).into_dimensions()?;
        ([w, h], 1, 1.0)
    };
    ensure!(
        frames > 0 && size.iter().all(|&v| v > 0 && v <= limits::IMAGE_SIDE),
        "Image must have frames and source edges of at most 40960 pixels"
    );
    ensure!(
        u64::from(size[0]) * u64::from(size[1]) * 4 <= limits::IMAGE_DECODED,
        "Source image canvas exceeds 4096 MiB decoded"
    );
    let info = Info {
        size,
        frames,
        duration,
        file_bytes: bytes.len() as u64,
    };
    ensure!(
        info.source_bytes() <= limits::GIF_SOURCE_TOTAL,
        "GIF exceeds 256 GiB of source frame data"
    );
    Ok(info)
}
fn read_image(path: &Path, allow_jpeg: bool) -> Result<(Vec<u8>, image::ImageFormat)> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limits::IMAGE_FILE + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limits::IMAGE_FILE,
        "Image exceeds 512 MiB on disk"
    );
    let format = image::guess_format(&bytes)?;
    ensure!(
        matches!(format, image::ImageFormat::Png | image::ImageFormat::Gif)
            || (allow_jpeg && format == image::ImageFormat::Jpeg),
        "Choose a real PNG or GIF image"
    );
    Ok((bytes, format))
}
pub fn inspect(path: &Path) -> Result<Info> {
    let (bytes, format) = read_image(path, true)?;
    inspect_bytes(&bytes, format)
}
pub fn load(
    ctx: &egui::Context,
    state: Option<&RenderState>,
    path: &Path,
    used: u64,
    allow_jpeg: bool,
) -> Result<Sprite> {
    load_with_progress(
        ctx,
        state,
        path,
        used,
        allow_jpeg,
        limits::GIF_PLAYBACK_MIB,
        &|_, _| Ok(()),
    )
}
fn load_with_progress(
    ctx: &egui::Context,
    state: Option<&RenderState>,
    path: &Path,
    used: u64,
    allow_jpeg: bool,
    playback_mib: u32,
    progress: &dyn Fn(usize, usize) -> Result<()>,
) -> Result<Sprite> {
    progress(0, 0)?;
    let (bytes, format) = read_image(path, allow_jpeg)?;
    let info = inspect_bytes(&bytes, format)?;
    let maximum = state.map_or_else(
        || ctx.input(|i| i.max_texture_side as u32),
        |s| s.device.limits().max_texture_dimension_2d,
    );
    ctx.input_mut(|i| i.max_texture_side = maximum as usize);
    let [w, h] = if format == image::ImageFormat::Gif {
        info.playback_size(
            maximum,
            u64::from(playback_mib.clamp(32, 1024)) * limits::MIB,
        )
    } else {
        limits::texture_size(info.size[0], info.size[1], maximum)
    };
    let retained = u64::from(w) * u64::from(h) * 4 * info.frames as u64;
    ensure!(
        retained.saturating_add(used) <= limits::IMAGE_COLLECTION,
        "Image collection exceeds 2560 MiB of playback textures; lower the GIF memory setting or remove artwork"
    );
    let mut decode_limits = image::Limits::default();
    decode_limits.max_image_width = Some(limits::IMAGE_SIDE);
    decode_limits.max_image_height = Some(limits::IMAGE_SIDE);
    decode_limits.max_alloc = Some(limits::IMAGE_DECODED);
    let mut palette = crate::chroma::Palette::default();
    let mut frames = Vec::new();
    let mut ends = Vec::new();
    let mut elapsed = 0.0;
    let model_key;
    let mut upload = |rgba: image::RgbaImage, delay: f32| -> Result<()> {
        progress(frames.len(), info.frames)?;
        ensure!(
            [rgba.width(), rgba.height()] == info.size,
            "GIF canvas changed during decode"
        );
        let rgba = fit_texture(rgba, w.max(h));
        ensure!(
            [rgba.width(), rgba.height()] == [w, h],
            "Unexpected fitted texture dimensions"
        );
        palette.add_rgba(&rgba);
        let pixels = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let texture = ctx.load_texture(
            format!("{}:{}", path.display(), frames.len()),
            pixels.clone(),
            egui::TextureOptions::LINEAR,
        );
        // Required for the offscreen/Spout pass, which precedes eframe's texture upload.
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
        progress(frames.len(), info.frames)?;
        ctx.request_repaint();
        Ok(())
    };
    if format == image::ImageFormat::Gif {
        model_key = format!("gif:{}", aria_core::movement::model_key(&bytes));
        // The source canvas was checked independently above. The compositor may
        // hold its previous canvas, the current frame and its output together.
        // This is a ceiling, not an allocation or a retained playback budget.
        decode_limits.max_alloc = Some(limits::IMAGE_DECODED * 3);
        let mut decoder = image::codecs::gif::GifDecoder::new(Cursor::new(&bytes))?;
        decoder.set_limits(decode_limits)?;
        // Composite and fit one source frame at a time; never retain the full source animation.
        for frame in decoder.into_frames() {
            let frame = frame.context("Cannot decode GIF frame")?;
            let (n, d) = frame.delay().numer_denom_ms();
            upload(
                frame.into_buffer(),
                (n as f32 / d as f32 / 1000.0).clamp(0.02, 60.0),
            )?;
        }
    } else {
        let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
        reader.limits(decode_limits);
        let rgba = reader.decode()?.into_rgba8();
        model_key = format!(
            "image:{}x{}:{}",
            info.size[0],
            info.size[1],
            aria_core::movement::model_key(&rgba)
        );
        upload(rgba, 1.0)?;
    }
    ensure!(
        frames.len() == info.frames,
        "Decoded frame count does not match GIF metadata"
    );
    Ok(Sprite {
        texture: frames[0].clone(),
        size: egui::vec2(info.size[0] as f32, info.size[1] as f32),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        model_key,
        palette: Arc::new(palette),
        animation: (frames.len() > 1).then(|| {
            Arc::new(Animation {
                frames,
                ends,
                bytes: retained,
            })
        }),
    })
}

/// One bounded import worker at a time across avatar, image-library and accessory jobs.
pub struct LoadJob {
    result: std::sync::mpsc::Receiver<Result<Sprite, String>>,
    progress: Arc<std::sync::Mutex<(usize, usize)>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}
impl LoadJob {
    pub fn start(
        ctx: &egui::Context,
        state: Option<&RenderState>,
        path: &Path,
        used: u64,
        playback_mib: u32,
    ) -> Result<Self> {
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        static GATE: Mutex<()> = Mutex::new(());
        let (tx, result) = mpsc::channel();
        let progress = Arc::new(Mutex::new((0, 0)));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (report, stop) = (progress.clone(), cancelled.clone());
        let (ctx, state, path) = (ctx.clone(), state.cloned(), path.to_owned());
        std::thread::Builder::new()
            .name("aria-image-import".into())
            .spawn(move || {
                let _permit = GATE.lock().unwrap_or_else(|e| e.into_inner());
                let outcome = load_with_progress(
                    &ctx,
                    state.as_ref(),
                    &path,
                    used,
                    true,
                    playback_mib,
                    &|done, total| {
                        ensure!(!stop.load(Ordering::Relaxed), "Image import cancelled");
                        *report.lock().unwrap() = (done, total);
                        Ok(())
                    },
                )
                .map_err(|e| format!("{e:#}"));
                let _ = tx.send(outcome);
                ctx.request_repaint();
            })?;
        Ok(Self {
            result,
            progress,
            cancelled,
        })
    }
    pub fn progress(&self) -> (usize, usize) {
        *self.progress.lock().unwrap()
    }
    pub fn poll(&self) -> Option<Result<Sprite, String>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(_) => Some(Err("Image import worker stopped unexpectedly".into())),
        }
    }
}
impl Drop for LoadJob {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tenfold_sources_and_long_gifs_fit_a_bounded_playback_budget() {
        const {
            assert!(limits::IMAGE_FILE >= 10 * 44_179_519);
        }
        for (size, frames) in [
            ([3500, 2500], 63),
            ([35000, 25000], 63),
            ([3500, 2500], 630),
        ] {
            let info = Info {
                size,
                frames,
                duration: 4.2,
                file_bytes: 44_179_519,
            };
            assert!(u64::from(size[0]) * u64::from(size[1]) * 4 <= limits::IMAGE_DECODED);
            assert!(info.source_bytes() <= limits::GIF_SOURCE_TOTAL);
            let [w, h] = info.playback_size(8192, 256 * limits::MIB);
            assert!(u64::from(w) * u64::from(h) * 4 * frames as u64 <= 256 * limits::MIB);
            assert!(w <= 8192 && h <= 8192);
        }
    }
    #[test]
    fn background_import_returns_image_and_reports_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("idle.png");
        image::RgbaImage::from_pixel(32, 24, image::Rgba([120, 80, 60, 128]))
            .save(&file)
            .unwrap();
        let ctx = egui::Context::default();
        for path in [&file, &dir.path().join("missing.gif")] {
            let job = LoadJob::start(&ctx, None, path, 0, 256).unwrap();
            let start = std::time::Instant::now();
            let result = loop {
                if let Some(result) = job.poll() {
                    break result;
                }
                assert!(start.elapsed().as_secs() < 10);
                std::thread::sleep(std::time::Duration::from_millis(5));
            };
            assert_eq!(result.is_ok(), path == &file);
            if let Ok(sprite) = result {
                assert_eq!(sprite.size, egui::vec2(32.0, 24.0));
                assert_eq!(job.progress(), (1, 1));
            }
        }
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "requires ARIA_TEST_GIF_DIR and a Windows GPU; retains the complete local GIF set"]
    fn local_gif_set_imports_with_bounded_textures_and_original_timing() {
        let root = std::path::PathBuf::from(
            std::env::var_os("ARIA_TEST_GIF_DIR").expect("ARIA_TEST_GIF_DIR"),
        );
        let state = crate::spout::tests::gpu_state();
        let ctx = egui::Context::default();
        let mut paths: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gif")))
            .collect();
        paths.sort();
        assert!(!paths.is_empty());
        // Exercise a real GIF container at 10x the largest supplied file's byte
        // size. Padding after its trailer changes disk size, not image detail.
        let dir = tempfile::tempdir().unwrap();
        let largest = paths
            .iter()
            .max_by_key(|p| p.metadata().unwrap().len())
            .unwrap();
        let padded = dir.path().join("tenfold-file-bytes.gif");
        std::fs::copy(largest, &padded).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .open(&padded)
            .unwrap()
            .set_len(largest.metadata().unwrap().len() * 10)
            .unwrap();
        paths.push(padded);
        let mut loaded = Vec::<Sprite>::new();
        for path in paths {
            let info = inspect(&path).unwrap();
            let used = loaded.iter().map(Sprite::bytes).sum();
            let job = LoadJob::start(&ctx, Some(&state), &path, used, 256).unwrap();
            let start = std::time::Instant::now();
            let sprite = loop {
                // Drain eframe texture deltas just as the application does each frame;
                // the worker has already uploaded these to the offscreen renderer.
                let _ = crate::run_test_ui(&ctx, Default::default(), |_| {});
                if let Some(result) = job.poll() {
                    break result.unwrap();
                }
                assert!(start.elapsed().as_secs() < 300, "GIF import timed out");
                std::thread::sleep(std::time::Duration::from_millis(16));
            };
            let animation = sprite.animation.as_ref().unwrap();
            assert_eq!(animation.frames.len(), info.frames);
            assert!((animation.ends.last().unwrap() - info.duration).abs() < 0.01);
            assert!(sprite.bytes() <= 256 * limits::MIB);
            assert_eq!(
                sprite.size,
                egui::vec2(info.size[0] as f32, info.size[1] as f32)
            );
            assert_ne!(
                sprite.at(0.0, 1.0, true).texture.id(),
                sprite.at(info.duration * 0.5, 1.0, true).texture.id()
            );
            eprintln!(
                "{}: {} source frames, {:?} playback, {:.1} MiB in {:.1}s",
                path.file_name().unwrap().to_string_lossy(),
                info.frames,
                sprite.texture.size(),
                sprite.bytes() as f64 / 1048576.0,
                start.elapsed().as_secs_f64()
            );
            loaded.push(sprite);
        }
        assert!(loaded.iter().map(Sprite::bytes).sum::<u64>() <= limits::IMAGE_COLLECTION);
    }
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
