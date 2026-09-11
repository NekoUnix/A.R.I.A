use anyhow::{Context, Result, ensure};
use aria_core::Parameters;
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2, vec2};
use std::{fs::File, io::BufReader, path::Path};

#[derive(Clone)]
pub struct Sprite {
    pub texture: egui::TextureHandle,
    pub name: String,
    pub size: Vec2,
}

pub fn load_sprite(ctx: &egui::Context, path: &Path) -> Result<Sprite> {
    ensure!(
        path.metadata()?.len() <= 32 * 1024 * 1024,
        "Image file exceeds 32 MiB"
    );
    let mut reader =
        image::ImageReader::new(BufReader::new(File::open(path)?)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let rgba = reader
        .decode()
        .context("Cannot decode PNG/JPEG image (maximum 4096 × 4096)")?
        .into_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let texture = ctx.load_texture(
        path.display().to_string(),
        egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
        egui::TextureOptions::LINEAR,
    );
    Ok(Sprite {
        texture,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        size: vec2(size[0] as f32, size[1] as f32),
    })
}

pub fn draw(p: &Painter, rect: Rect, params: Parameters, sprite: Option<&Sprite>, zoom: f32) {
    if let Some(sprite) = sprite {
        draw_sprite(p, rect, params, sprite, zoom);
    } else {
        draw_mica(p, rect, params, zoom);
    }
}

fn draw_sprite(p: &Painter, rect: Rect, params: Parameters, sprite: &Sprite, zoom: f32) {
    let size = sprite.size
        * (rect.width() * 0.75 / sprite.size.x).min(rect.height() * 0.9 / sprite.size.y)
        * zoom;
    let center = rect.center()
        + vec2(
            params.0[0] * rect.width() * 0.0015,
            -params.0[1] * rect.height() * 0.001,
        );
    let rotation = egui::emath::Rot2::from_angle(-params.0[2].to_radians());
    let mut mesh = egui::Mesh::with_texture(sprite.texture.id());
    for (corner, uv) in [
        (vec2(-0.5, -0.5), pos2(0.0, 0.0)),
        (vec2(0.5, -0.5), pos2(1.0, 0.0)),
        (vec2(0.5, 0.5), pos2(1.0, 1.0)),
        (vec2(-0.5, 0.5), pos2(0.0, 1.0)),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: center + rotation * (corner * size),
            uv,
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    p.add(Shape::mesh(mesh));
}

/// Original vector test puppet. Its mouth, eyelids, pupils, brows and head all use mapped inputs.
fn draw_mica(p: &Painter, rect: Rect, Parameters(v): Parameters, zoom: f32) {
    let scale = (rect.width() / 480.0).min(rect.height() / 560.0) * zoom;
    let origin = rect.center() + vec2(v[0] * 0.8 * scale, 22.0 * scale - v[1] * 0.35 * scale);
    let rot = egui::emath::Rot2::from_angle(-v[2].to_radians());
    let point = |x: f32, y: f32| origin + rot * vec2(x * scale, y * scale);
    let outline = Color32::from_rgb(25, 28, 49);
    let fur = Color32::from_rgb(217, 226, 241);
    let dark = Color32::from_rgb(43, 49, 73);
    let cyan = Color32::from_rgb(114, 235, 209);
    let lavender = Color32::from_rgb(157, 145, 241);
    let line = Stroke::new(3.0 * scale, outline);
    let poly = |coords: &[(f32, f32)], fill: Color32| {
        p.add(Shape::convex_polygon(
            coords.iter().map(|&(x, y)| point(x, y)).collect(),
            fill,
            line,
        ));
    };
    // Hoodie/body remains separate from the head's facial geometry.
    poly(
        &[
            (-126.0, 150.0),
            (-91.0, 94.0),
            (91.0, 94.0),
            (126.0, 150.0),
            (137.0, 233.0),
            (-137.0, 233.0),
        ],
        dark,
    );
    poly(
        &[(-50.0, 100.0), (0.0, 152.0), (50.0, 100.0), (0.0, 119.0)],
        cyan,
    );
    for x in [-38.0, 38.0] {
        p.line_segment(
            [point(x, 146.0), point(x * 1.1, 184.0)],
            Stroke::new(3.0 * scale, lavender),
        );
    }
    p.circle_filled(point(0.0, 197.0), 10.0 * scale, cyan);
    // Ears, inner ear, and round face.
    poly(&[(-110.0, -64.0), (-130.0, -193.0), (-36.0, -123.0)], fur);
    poly(&[(110.0, -64.0), (36.0, -123.0), (130.0, -193.0)], fur);
    poly(
        &[(-101.0, -96.0), (-116.0, -165.0), (-66.0, -125.0)],
        lavender,
    );
    poly(&[(101.0, -96.0), (66.0, -125.0), (116.0, -165.0)], lavender);
    let face: Vec<Pos2> = (0..64)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 64.0;
            point(a.cos() * 127.0, -13.0 + a.sin() * 132.0)
        })
        .collect();
    p.add(Shape::convex_polygon(face, fur, line));
    // Forehead markings.
    poly(
        &[
            (-44.0, -139.0),
            (-17.0, -145.0),
            (-8.0, -96.0),
            (-30.0, -109.0),
        ],
        lavender,
    );
    poly(
        &[(0.0, -146.0), (28.0, -143.0), (34.0, -103.0), (11.0, -93.0)],
        cyan,
    );
    let look = v[0] * 0.35;
    for (index, x) in [-49.0, 49.0].into_iter().enumerate() {
        let open = v[3 + index].clamp(0.03, 1.0);
        let cx = x + look;
        let ellipse = |x: f32, y: f32, rx: f32, ry: f32, fill: Color32| {
            let points = (0..32)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 32.0;
                    point(x + a.cos() * rx, y + a.sin() * ry)
                })
                .collect();
            p.add(Shape::convex_polygon(points, fill, Stroke::NONE));
        };
        ellipse(cx, -20.0, 28.0, 31.0 * open, outline);
        if open > 0.15 {
            ellipse(
                cx + v[9] * 8.0,
                -18.0 - v[10] * 8.0 * open,
                15.0,
                23.0 * open,
                cyan,
            );
            ellipse(
                cx + v[9] * 8.0,
                -17.0 - v[10] * 8.0 * open,
                6.0,
                20.0 * open,
                dark,
            );
            ellipse(
                cx - 6.0 + v[9] * 8.0,
                -30.0 * open,
                5.0,
                6.0 * open,
                Color32::WHITE,
            );
        }
        let brow_y = -67.0 - v[7 + index] * 15.0;
        p.line_segment(
            [point(cx - 22.0, brow_y + 4.0), point(cx + 18.0, brow_y)],
            Stroke::new(5.0 * scale, dark),
        );
        ellipse(cx * 1.46, 23.0, 19.0, 8.0, Color32::from_rgb(223, 167, 191));
    }
    poly(
        &[(-8.0 + look, 20.0), (8.0 + look, 20.0), (look, 27.0)],
        dark,
    );
    let mouth_y = 52.0;
    let mouth = v[5];
    if mouth > 0.08 {
        let points = (0..32)
            .map(|i| {
                let a = i as f32 * std::f32::consts::TAU / 32.0;
                point(
                    look + a.cos() * (15.0 + v[6] * 10.0),
                    mouth_y + a.sin() * (4.0 + mouth * 25.0),
                )
            })
            .collect();
        p.add(Shape::convex_polygon(points, outline, Stroke::NONE));
        p.line_segment(
            [
                point(look - 7.0, mouth_y + mouth * 15.0),
                point(look + 7.0, mouth_y + mouth * 15.0),
            ],
            Stroke::new(6.0 * scale, lavender),
        );
    } else {
        p.add(Shape::line(
            vec![
                point(look - 17.0, mouth_y - v[6] * 6.0),
                point(look, mouth_y + 5.0),
                point(look + 17.0, mouth_y - v[6] * 6.0),
            ],
            Stroke::new(3.0 * scale, outline),
        ));
    }
    for side in [-1.0, 1.0] {
        for y in [42.0, 55.0] {
            p.line_segment(
                [point(side * 90.0, y), point(side * 120.0, y - 6.0)],
                Stroke::new(2.0 * scale, dark),
            );
        }
    }
}
