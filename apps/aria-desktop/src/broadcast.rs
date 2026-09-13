//! Full-resolution offscreen canvases, independent of native preview windows.
use crate::output::SENDERS;
use crate::output::{CanvasSettings, Scene};
use eframe::{
    egui,
    egui_wgpu::{RenderState, ScreenDescriptor},
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Broadcasts {
    bridge: Option<crate::native_output::Bridge>,
    attempted_bridge: bool,
    bridge_error: Option<String>,
    targets: [Option<Target>; 3],
    attempted_size: [Option<[u32; 2]>; 3],
    pending_resize: [Option<([u32; 2], Instant)>; 3],
    pub status: [Option<String>; 3],
    paint_context: egui::Context,
}
struct Target {
    view: wgpu::TextureView,
    sender: crate::native_output::Sender,
    last: Option<(CanvasSettings, u64)>,
}
impl Broadcasts {
    pub fn retry(&mut self) {
        // Drop senders before the bridge; old metadata is unregistered.
        self.targets = Default::default();
        self.attempted_size = Default::default();
        self.pending_resize = Default::default();
        self.status = Default::default();
        self.bridge = None;
        self.attempted_bridge = false;
        self.bridge_error = None;
    }
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        state: &RenderState,
        configs: [Option<CanvasSettings>; 3],
        scene: &Scene,
        revision: u64,
    ) {
        if ctx.current_pass_index() != 0 {
            return;
        }
        for (index, config) in configs.iter().enumerate() {
            if config.is_none() {
                self.targets[index] = None;
                self.attempted_size[index] = None;
                self.pending_resize[index] = None;
                self.status[index] = None;
            }
        }
        if configs.iter().all(Option::is_none) {
            return;
        }
        if !self.attempted_bridge {
            self.attempted_bridge = true;
            match crate::native_output::Bridge::new(state) {
                Ok(bridge) => self.bridge = Some(bridge),
                Err(e) => self.bridge_error = Some(format!("Native OBS output unavailable: {e:#}")),
            }
        }
        for (i, config) in configs.iter().enumerate() {
            let Some(config) = config else {
                continue;
            };
            if let Some(error) = &self.bridge_error {
                self.status[i] = Some(error.clone());
                continue;
            }
            let size = config.pixels(i);
            if self.attempted_size[i] != Some(size) {
                if self.attempted_size[i].is_some() {
                    if self.pending_resize[i].is_none_or(|(s, _)| s != size) {
                        self.pending_resize[i] = Some((size, Instant::now()));
                    }
                    if self.pending_resize[i].unwrap().1.elapsed() < Duration::from_millis(250) {
                        self.status[i] = Some("Applying resolution when editing settles…".into());
                        continue;
                    }
                }
                self.pending_resize[i] = None;
                self.targets[i] = None;
                self.attempted_size[i] = Some(size);
                match self.create_target(state, i, size) {
                    Ok(target) => self.targets[i] = Some(target),
                    Err(e) => {
                        self.status[i] =
                            Some(format!("Output failed: {e:#}. Use Retry OBS output."))
                    }
                }
            }
            let Some(target) = &mut self.targets[i] else {
                continue;
            };
            let changed = !target
                .last
                .as_ref()
                .is_some_and(|(c, r)| c == config && *r == revision);
            if changed {
                render_canvas(ctx, &self.paint_context, state, &target.view, scene, config);
            }
            match crate::native_output::send(
                self.bridge.as_ref().unwrap(),
                &mut target.sender,
                changed,
            ) {
                Ok(sent) => {
                    self.status[i] = Some(format!(
                        "{} · {} × {} · {}{}",
                        target.sender.name(),
                        size[0],
                        size[1],
                        crate::native_output::description(),
                        if sent { "" } else { " · frame pending" }
                    ));
                    if sent || cfg!(target_os = "linux") {
                        target.last = Some((config.clone(), revision));
                    }
                }
                Err(e) => {
                    self.status[i] = Some(format!("OBS send failed: {e:#}. Use Retry OBS output."));
                }
            }
        }
    }
    fn create_target(
        &self,
        state: &RenderState,
        index: usize,
        size: [u32; 2],
    ) -> anyhow::Result<Target> {
        anyhow::ensure!(
            size.iter()
                .all(|&v| v > 0 && v <= state.device.limits().max_texture_dimension_2d),
            "Canvas exceeds GPU texture limits"
        );
        let texture = state.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(SENDERS[index]),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: state.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let sender = self
            .bridge
            .as_ref()
            .unwrap()
            .sender(SENDERS[index], &texture)?;
        Ok(Target {
            view: texture.create_view(&Default::default()),
            sender,
            last: None,
        })
    }
}
fn render_canvas(
    root: &egui::Context,
    paint_context: &egui::Context,
    state: &RenderState,
    view: &wgpu::TextureView,
    scene: &Scene,
    config: &CanvasSettings,
) {
    let size = [view.texture().width(), view.texture().height()];
    let rect =
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size[0] as f32, size[1] as f32));
    let mut output = paint_context.run_ui(
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |ctx| {
            scene.paint(
                &ctx.ctx().layer_painter(egui::LayerId::background()),
                rect,
                config,
            );
        },
    );
    // These shapes reference the root renderer's existing model/sprite textures.
    // No second atlas upload, no scaled desktop screenshot, and no CPU pixels.
    // This geometry-only context has no text; discard its unused default font atlas.
    output.textures_delta.clear();
    let jobs = root.tessellate(output.shapes, 1.0);
    let screen = ScreenDescriptor {
        size_in_pixels: size,
        pixels_per_point: 1.0,
    };
    let mut encoder = state.device.create_command_encoder(&Default::default());
    let mut renderer = state.renderer.write();
    let commands =
        renderer.update_buffers(&state.device, &state.queue, &mut encoder, &jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ARIA full resolution canvas"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            })
            .forget_lifetime();
        renderer.render(&mut pass, &jobs, &screen);
    }
    encoder.transition_resources(
        std::iter::empty(),
        std::iter::once(wgpu::TextureTransition {
            texture: view.texture(),
            selector: None,
            state: wgpu::TextureUses::COPY_SRC,
        }),
    );
    // Submit before egui reuses its streaming buffers for any other viewport.
    state
        .queue
        .submit(commands.into_iter().chain([encoder.finish()]));
}

/// One-shot readback for image export; independent of native live outputs.
pub fn save_png(
    ctx: &egui::Context,
    state: &RenderState,
    scene: &Scene,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    use anyhow::Context;
    let size = scene
        .model
        .map_or([1200, 1400], |m| [m.size.x as u32, m.size.y as u32]);
    let bgra = matches!(
        state.target_format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );
    anyhow::ensure!(
        bgra || matches!(
            state.target_format,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb
        ),
        "PNG export requires an 8-bit renderer"
    );
    let texture = state.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("ARIA avatar and stage objects export"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: state.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let config = CanvasSettings {
        background: crate::output::Background::Transparent,
        zoom: 1.0,
        position: [0.0; 2],
        ..Default::default()
    };
    render_canvas(
        ctx,
        &egui::Context::default(),
        state,
        &texture.create_view(&Default::default()),
        scene,
        &config,
    );
    let stride = (size[0] * 4).div_ceil(256) * 256;
    let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA composition PNG readback"),
        size: u64::from(stride) * u64::from(size[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = state.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(size[1]),
            },
        },
        texture.size(),
    );
    let submission = state.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    state.device.poll(wgpu::PollType::Wait {
        submission_index: Some(submission),
        timeout: Some(Duration::from_secs(5)),
    })?;
    rx.recv_timeout(Duration::from_secs(1))??;
    let mapped = buffer.slice(..).get_mapped_range()?;
    let mut rgba = Vec::with_capacity(size[0] as usize * size[1] as usize * 4);
    for row in mapped.chunks_exact(stride as usize) {
        for pixel in row[..size[0] as usize * 4].as_chunks::<4>().0 {
            let mut pixel = *pixel;
            if bgra {
                pixel.swap(0, 2);
            }
            rgba.extend_from_slice(&crate::cubism_render::straight_alpha(pixel));
        }
    }
    drop(mapped);
    buffer.unmap();
    image::save_buffer_with_format(
        path,
        &rgba,
        size[0],
        size[1],
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .context("Cannot save avatar and items PNG")
}
