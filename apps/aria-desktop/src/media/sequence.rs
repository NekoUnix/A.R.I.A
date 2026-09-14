//! Bounded VTS image-frame folders. Natural filename order; one source frame decoded at a time.
use super::*;
pub fn files(path: &Path) -> Result<Vec<std::path::PathBuf>> {
    let root = path.canonicalize()?;
    let mut paths = Vec::new();
    for e in std::fs::read_dir(&root)?.take(8193) {
        let e = e?;
        if e.path().extension().is_some_and(|x| {
            x.eq_ignore_ascii_case("png")
                || x.eq_ignore_ascii_case("jpg")
                || x.eq_ignore_ascii_case("jpeg")
        }) {
            let p = e.path().canonicalize()?;
            ensure!(
                p.starts_with(&root) && p.is_file(),
                "Animation frame escapes its folder"
            );
            paths.push(p);
        }
        ensure!(
            paths.len() <= limits::GIF_FRAMES,
            "Frame folder exceeds 4096 images"
        );
    }
    paths.sort_by_key(|p| order(&p.file_name().unwrap_or_default().to_string_lossy()));
    ensure!(
        !paths.is_empty(),
        "Animation folder contains no PNG/JPG frames"
    );
    Ok(paths)
}
fn order(s: &str) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    for c in s.to_lowercase().chars().chain(['\0']) {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            if !digits.is_empty() {
                out.push_str(&format!("{:0>20}", digits));
                digits.clear();
            }
            out.push(c);
        }
    }
    out
}
pub fn inspect(path: &Path) -> Result<(Vec<std::path::PathBuf>, Info)> {
    let paths = files(path)?;
    let mut info = Info {
        size: [0; 2],
        frames: paths.len(),
        duration: paths.len() as f32 / 30.,
        file_bytes: 0,
    };
    for p in &paths {
        let bytes = p.metadata()?.len();
        ensure!(
            bytes <= limits::IMAGE_FILE,
            "Animation frame exceeds file limit"
        );
        let (w, h) = image::ImageReader::open(p)?
            .with_guessed_format()?
            .into_dimensions()?;
        ensure!(
            w > 0
                && h > 0
                && w <= limits::IMAGE_SIDE
                && h <= limits::IMAGE_SIDE
                && u64::from(w) * u64::from(h) * 4 <= limits::IMAGE_DECODED,
            "Animation frame dimensions exceed limits"
        );
        if info.size == [0; 2] {
            info.size = [w, h];
        } else {
            ensure!(
                info.size == [w, h],
                "All animation frames must use the same canvas size"
            );
        }
        info.file_bytes = info.file_bytes.saturating_add(bytes);
    }
    ensure!(
        info.source_bytes() <= limits::GIF_SOURCE_TOTAL,
        "Frame sequence exceeds source animation limit"
    );
    Ok((paths, info))
}
pub fn load(
    ctx: &egui::Context,
    state: Option<&RenderState>,
    path: &Path,
    used: u64,
    playback_mib: u32,
    progress: &dyn Fn(usize, usize) -> Result<()>,
) -> Result<Sprite> {
    let (paths, info) = inspect(path)?;
    let maximum = state.map_or_else(
        || ctx.input(|i| i.max_texture_side as u32),
        |s| s.device.limits().max_texture_dimension_2d,
    );
    let [w, h] = info.playback_size(
        maximum,
        u64::from(playback_mib.clamp(32, 1024)) * limits::MIB,
    );
    let retained = u64::from(w) * u64::from(h) * 4 * paths.len() as u64;
    ensure!(
        used.saturating_add(retained) <= limits::IMAGE_COLLECTION,
        "Frame folder exceeds playback texture budget"
    );
    let mut frames = Vec::new();
    let mut palette = crate::chroma::Palette::default();
    let mut hash = sha2::Sha256::new();
    for p in &paths {
        progress(frames.len(), paths.len())?;
        let (bytes, format) = read_image(p, true)?;
        hash.update(&bytes);
        let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
        let mut decode = image::Limits::default();
        decode.max_image_width = Some(limits::IMAGE_SIDE);
        decode.max_image_height = Some(limits::IMAGE_SIDE);
        decode.max_alloc = Some(limits::IMAGE_DECODED);
        reader.limits(decode);
        let rgba = reader.decode()?.into_rgba8();
        ensure!(
            [rgba.width(), rgba.height()] == info.size,
            "Frame changed during import"
        );
        let rgba = fit_texture(rgba, w.max(h));
        palette.add_rgba(&rgba);
        let pixels = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
        let texture = ctx.load_texture(
            format!("aria-frame-{}", frames.len()),
            pixels.clone(),
            egui::TextureOptions::LINEAR,
        );
        if let Some(state) = state {
            state.renderer.write().update_texture(
                &state.device,
                &state.queue,
                texture.id(),
                &egui::epaint::ImageDelta::full(pixels, egui::TextureOptions::LINEAR),
            );
        }
        frames.push(texture);
    }
    progress(frames.len(), paths.len())?;
    Ok(Sprite {
        texture: frames[0].clone(),
        size: egui::vec2(info.size[0] as f32, info.size[1] as f32),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        model_key: format!("sequence:{:x}", hash.finalize()),
        palette: Arc::new(palette),
        animation: Some(Arc::new(Animation {
            ends: (1..=frames.len()).map(|n| n as f32 / 30.).collect(),
            frames,
            bytes: retained,
        })),
    })
}
use sha2::Digest;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_order_and_dimension_checks() {
        let d = tempfile::tempdir().unwrap();
        for n in [10, 2, 1] {
            image::RgbaImage::new(2, 2)
                .save(d.path().join(format!("{n}.png")))
                .unwrap();
        }
        let (files, info) = inspect(d.path()).unwrap();
        assert_eq!(files[1].file_name().unwrap(), "2.png");
        assert_eq!(info.frames, 3);
        image::RgbaImage::new(3, 2)
            .save(d.path().join("4.png"))
            .unwrap();
        assert!(inspect(d.path()).is_err());
    }
}
