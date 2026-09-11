//! Full-resolution offscreen canvases, independent of native preview windows.
use crate::output::{CanvasSettings, SENDERS, Scene};
use eframe::{
    egui,
    egui_wgpu::{RenderState, ScreenDescriptor},
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Broadcasts {
    #[cfg(windows)]
    bridge: Option<crate::spout::Bridge>,
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
    #[cfg(windows)]
    sender: crate::spout::Sender,
    last: Option<(CanvasSettings, u64)>,
}
impl Broadcasts {
    pub fn retry(&mut self) {
        // Drop senders before the bridge; old metadata is unregistered.
        self.targets = Default::default();
        self.attempted_size = Default::default();
        self.pending_resize = Default::default();
        self.status = Default::default();
        #[cfg(windows)]
        {
            self.bridge = None;
        }
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
            #[cfg(windows)]
            match crate::spout::Bridge::new(state) {
                Ok(bridge) => self.bridge = Some(bridge),
                Err(e) => self.bridge_error = Some(format!("Spout unavailable: {e:#}")),
            }
            #[cfg(not(windows))]
            {
                self.bridge_error =
                    Some("Spout output currently requires Windows and DirectX 12.".into());
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
                #[cfg(windows)]
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
            if target
                .last
                .as_ref()
                .is_some_and(|(c, r)| c == config && *r == revision)
            {
                continue;
            }
            render_canvas(ctx, &self.paint_context, state, &target.view, scene, config);
            #[cfg(windows)]
            match self.bridge.as_ref().unwrap().send(&target.sender) {
                Ok(true) => {
                    self.status[i] = Some(format!(
                        "{} · {} × {} · GPU sharing active",
                        target.sender.name(),
                        size[0],
                        size[1]
                    ));
                    target.last = Some((config.clone(), revision));
                }
                Ok(false) => {
                    self.status[i] = Some("Receiver busy · keeping previous frame".into());
                }
                Err(e) => {
                    self.status[i] = Some(format!("Spout send failed: {e:#}"));
                }
            }
        }
    }
    #[cfg(windows)]
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
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
    let output = paint_context.run(
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |ctx| {
            scene.paint(
                &ctx.layer_painter(egui::LayerId::background()),
                rect,
                config,
            );
        },
    );
    // These shapes reference the root renderer's existing model/sprite textures.
    // No second atlas upload, no scaled desktop screenshot, and no CPU pixels.
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
