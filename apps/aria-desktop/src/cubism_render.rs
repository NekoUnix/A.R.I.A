//! Cubism ArtMeshes rendered once to a transparent GPU texture shared by both windows.
use anyhow::{Context, Result, ensure};
use aria_core::asset_limits as limits;
use aria_live2d::{Blend, Canvas, Drawable};
use bytemuck::{Pod, Zeroable};
use eframe::{egui, egui_wgpu::RenderState};
use std::{fs::File, io::BufReader, num::NonZeroU64, ops::Range, path::PathBuf};
use wgpu::util::DeviceExt;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Grow the view to contain every visible ArtMesh, including revealed expressions.
/// Keep the texture's aspect ratio and never shrink on animation frames, avoiding
/// breathing/zoom jitter. Only projection changes; GPU allocations remain fixed.
fn fit_canvas(original: Canvas, drawables: &[Drawable], previous: Option<Canvas>) -> Canvas {
    let base = previous.unwrap_or(original);
    let mut min = if previous.is_some() {
        egui::vec2(-base.origin[0], -base.origin[1])
    } else {
        egui::Vec2::INFINITY
    };
    let mut max = if previous.is_some() {
        min + egui::vec2(base.size[0], base.size[1])
    } else {
        -egui::Vec2::INFINITY
    };
    let old_min = min;
    let old_max = max;
    for p in drawables
        .iter()
        .filter(|d| d.visible && d.opacity > 0.0)
        .flat_map(|d| &d.positions)
    {
        let p = egui::vec2(p[0], p[1]) * original.pixels_per_unit;
        if p.is_finite() {
            min = min.min(p);
            max = max.max(p);
        }
    }
    if !min.is_finite() || !max.is_finite() || (max - min).max_elem() < 1e-5 {
        return base;
    }
    if previous.is_some() && min == old_min && max == old_max {
        return base;
    }
    let center = (min + max) * 0.5;
    let aspect = original.size[0] / original.size[1];
    let height = (max.y - min.y).max((max.x - min.x) / aspect) * 1.04;
    let size = egui::vec2(height * aspect, height);
    let origin = size * 0.5 - center;
    Canvas {
        size: [size.x, size.y],
        origin: [origin.x, origin.y],
        pixels_per_unit: original.pixels_per_unit,
    }
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Style {
    multiply: [f32; 4],
    screen: [f32; 4],
    control: [f32; 4],
}
struct Mesh {
    indices: Range<u32>,
}

#[derive(Clone, Copy)]
pub struct ModelImage {
    pub id: egui::TextureId,
    pub size: egui::Vec2,
}
impl ModelImage {
    pub fn rect(self, rect: egui::Rect, zoom: f32) -> egui::Rect {
        let size = self.size * (rect.width() / self.size.x).min(rect.height() / self.size.y) * zoom;
        egui::Rect::from_center_size(rect.center(), size)
    }
    pub fn draw(self, painter: &egui::Painter, rect: egui::Rect, zoom: f32) {
        painter.image(
            self.id,
            self.rect(rect, zoom),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
}

pub struct ModelRenderer {
    state: RenderState,
    pub image: ModelImage,
    pub lease: std::sync::Arc<ModelTexture>,
    output: wgpu::TextureView,
    mask: wgpu::TextureView,
    // Bind groups own the atlas resources; output is also owned by egui's registered view.
    atlases: Vec<wgpu::BindGroup>,
    clipped: wgpu::BindGroup,
    unclipped: wgpu::BindGroup,
    pipelines: Vec<wgpu::RenderPipeline>,
    mask_pipelines: Vec<wgpu::RenderPipeline>,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    uniforms: wgpu::Buffer,
    uniform_stride: usize,
    meshes: Vec<Mesh>,
    vertex_count: usize,
    pub atlas_mib: f64,
    pub import_notes: Vec<String>,
    palette: crate::chroma::Palette,
    vertex_staging: Vec<Vertex>,
    style_staging: Vec<u8>,
    order: Vec<usize>,
    pub bounds: egui::Rect,
    pub view_canvas: Canvas,
}

impl ModelRenderer {
    #[cfg(all(test, windows))]
    pub fn read_rgba_for_test(&self) -> Result<(Vec<u8>, [u32; 2])> {
        self.read_rgba()
    }
    #[cfg(all(test, windows))]
    pub fn vertex_staging_capacity(&self) -> usize {
        self.vertex_staging.capacity()
    }
    pub fn save_png(&self, path: &std::path::Path) -> Result<()> {
        let (rgba, size) = self.read_rgba()?;
        image::save_buffer_with_format(
            path,
            &rgba,
            size[0],
            size[1],
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .context("Cannot save avatar PNG")
    }
    pub fn key_palette(&self) -> Result<crate::chroma::Palette> {
        let (rgba, _) = self.read_rgba()?;
        let mut palette = self.palette.clone();
        palette.add_rgba(&rgba);
        Ok(palette)
    }
    fn read_rgba(&self) -> Result<(Vec<u8>, [u32; 2])> {
        let texture = self.output.texture();
        let size = texture.size();
        let stride = (size.width * 4).div_ceil(256) * 256;
        let buffer = self.state.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA PNG readback"),
            size: u64::from(stride) * u64::from(size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );
        let submission = self.state.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.state.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(5)),
        })?;
        rx.recv_timeout(std::time::Duration::from_secs(1))??;
        let mapped = buffer.slice(..).get_mapped_range()?;
        let mut rgba = Vec::with_capacity(size.width as usize * size.height as usize * 4);
        for row in mapped.chunks_exact(stride as usize) {
            for pixel in row[..size.width as usize * 4].as_chunks::<4>().0 {
                rgba.extend_from_slice(&straight_alpha(*pixel));
            }
        }
        drop(mapped);
        buffer.unmap();
        Ok((rgba, [size.width, size.height]))
    }
    pub fn new(
        state: &RenderState,
        canvas: Canvas,
        drawables: &[Drawable],
        paths: &[PathBuf],
    ) -> Result<Self> {
        let device = &state.device;
        let queue = &state.queue;
        let mut palette = crate::chroma::Palette::default();
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Cubism atlas"),
            entries: &[
                texture_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let style_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Cubism mask and style"),
            entries: &[
                texture_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: NonZeroU64::new(48),
                    },
                    count: None,
                },
            ],
        });
        let mut atlases = Vec::new();
        let mut total_bytes = 0_u64;
        let mut import_notes = Vec::new();
        for path in paths {
            let file = File::open(path)
                .with_context(|| format!("Cannot open texture {}", path.display()))?;
            ensure!(
                file.metadata()?.len() <= limits::ATLAS_FILE,
                "Texture file exceeds 1280 MiB"
            );
            let mut reader = image::ImageReader::new(BufReader::new(file)).with_guessed_format()?;
            let mut limits = image::Limits::default();
            let maximum = device.limits().max_texture_dimension_2d;
            limits.max_image_width = Some(limits::ATLAS_SIDE);
            limits.max_image_height = Some(limits::ATLAS_SIDE);
            limits.max_alloc = Some(limits::ATLAS_DECODED);
            reader.limits(limits);
            let source = image::image_dimensions(path)?;
            ensure!(
                u64::from(source.0) * u64::from(source.1) * 4 <= limits::ATLAS_DECODED,
                "Texture exceeds 5120 MiB decoded"
            );
            let [width, height] = limits::texture_size(source.0, source.1, maximum);
            total_bytes += u64::from(width) * u64::from(height) * 4;
            ensure!(
                total_bytes <= limits::ATLAS_COLLECTION,
                "Texture atlases exceed 10 GiB after GPU fitting; export smaller atlases"
            );
            let rgba = reader
                .decode()
                .with_context(|| {
                    format!(
                        "Cannot decode texture {} (source maximum 81920px)",
                        path.display()
                    )
                })?
                .into_rgba8();
            ensure!(
                rgba.dimensions() == source,
                "Texture dimensions changed during import; retry"
            );
            if source != (width, height) {
                import_notes.push(format!("{}: {} × {} atlas fitted once to {width} × {height} for this GPU. The original file is unchanged.",
                    path.file_name().unwrap_or_default().to_string_lossy(), source.0, source.1));
            }
            let rgba = crate::media::fit_texture(rgba, maximum);
            palette.add_rgba(&rgba);
            let texture = target(
                device,
                width,
                height,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                "Cubism atlas",
            );
            queue.write_texture(
                texture.as_image_copy(),
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                texture.size(),
            );
            atlases.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Cubism atlas"),
                layout: &atlas_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &texture.create_view(&Default::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            }));
        }
        let view_canvas = fit_canvas(canvas, drawables, None);
        let canvas = canvas.size;
        let scale = 2048.0 / canvas[0].max(canvas[1]);
        let width = (canvas[0] * scale).round().max(16.0) as u32;
        let height = (canvas[1] * scale).round().max(16.0) as u32;
        let output = target(
            device,
            width,
            height,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            "Cubism output",
        )
        .create_view(&Default::default());
        let mask = target(
            device,
            width,
            height,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            "Cubism mask",
        )
        .create_view(&Default::default());
        let white = target(
            device,
            1,
            1,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            "Cubism no mask",
        );
        queue.write_texture(
            white.as_image_copy(),
            &[255; 4],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            white.size(),
        );
        let vertex_count = drawables.iter().map(|d| d.positions.len()).sum();
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cubism vertices"),
            size: (vertex_count as u64 * 16).max(16),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut index_data = Vec::new();
        let mut meshes = Vec::new();
        let mut base = 0_u32;
        for d in drawables {
            let start = index_data.len() as u32;
            index_data.extend(d.indices.iter().map(|&i| base + i as u32));
            meshes.push(Mesh {
                indices: start..index_data.len() as u32,
            });
            base += d.positions.len() as u32;
        }
        ensure!(!index_data.is_empty(), "Avatar has no drawable triangles");
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cubism indices"),
            contents: bytemuck::cast_slice(&index_data),
            usage: wgpu::BufferUsages::INDEX,
        });
        let uniform_stride = 48_usize
            .div_ceil(device.limits().min_uniform_buffer_offset_alignment as usize)
            * device.limits().min_uniform_buffer_offset_alignment as usize;
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cubism styles"),
            size: (drawables.len() * uniform_stride).max(uniform_stride) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let style_group = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Cubism style"),
                layout: &style_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &uniforms,
                            offset: 0,
                            size: NonZeroU64::new(48),
                        }),
                    },
                ],
            })
        };
        let clipped = style_group(&mask);
        let unclipped = style_group(&white.create_view(&Default::default()));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA Cubism"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cubism.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cubism pipeline"),
            bind_group_layouts: &[Some(&atlas_layout), Some(&style_layout)],
            immediate_size: 0,
        });
        let mut pipelines = Vec::new();
        for mode in [Blend::Normal, Blend::Add, Blend::Multiply] {
            for double_sided in [false, true] {
                pipelines.push(pipeline(
                    device,
                    &layout,
                    &shader,
                    blend_state(mode),
                    double_sided,
                    false,
                ));
            }
        }
        let mask_pipelines = [false, true]
            .into_iter()
            .map(|d| {
                pipeline(
                    device,
                    &layout,
                    &shader,
                    blend_state(Blend::Normal),
                    d,
                    true,
                )
            })
            .collect();
        let id = state.renderer.write().register_native_texture(
            device,
            &output,
            wgpu::FilterMode::Linear,
        );
        Ok(Self {
            lease: std::sync::Arc::new(ModelTexture {
                state: state.clone(),
                id,
            }),
            state: state.clone(),
            image: ModelImage {
                id,
                size: egui::vec2(width as f32, height as f32),
            },
            output,
            mask,
            atlases,
            clipped,
            unclipped,
            pipelines,
            mask_pipelines,
            vertices,
            indices,
            uniforms,
            uniform_stride,
            style_staging: vec![0; meshes.len() * uniform_stride],
            vertex_staging: Vec::with_capacity(vertex_count),
            order: Vec::with_capacity(meshes.len()),
            meshes,
            vertex_count,
            atlas_mib: total_bytes as f64 / 1048576.0,
            import_notes,
            palette,
            bounds: egui::Rect::NOTHING,
            view_canvas,
        })
    }

    pub fn render(&mut self, canvas: Canvas, drawables: &[Drawable]) -> Result<()> {
        self.render_layers(canvas, drawables, &Default::default())
    }
    pub fn render_layers(
        &mut self,
        canvas: Canvas,
        drawables: &[Drawable],
        layers: &aria_core::layers::Config,
    ) -> Result<()> {
        ensure!(
            self.meshes.len() == drawables.len()
                && self.vertex_count == drawables.iter().map(|d| d.positions.len()).sum::<usize>(),
            "Model topology changed unexpectedly"
        );
        self.view_canvas = fit_canvas(canvas, drawables, Some(self.view_canvas));
        let vertices = &mut self.vertex_staging;
        vertices.clear();
        let uniform_bytes = &mut self.style_staging;
        let c = self.view_canvas;
        let mut bounds = egui::Rect::NOTHING;
        for (i, d) in drawables.iter().enumerate() {
            vertices.extend(d.positions.iter().zip(&d.uvs).map(|(p, &uv)| Vertex {
                position: [
                    2.0 * (p[0] * c.pixels_per_unit + c.origin[0]) / c.size[0] - 1.0,
                    2.0 * (p[1] * c.pixels_per_unit + c.origin[1]) / c.size[1] - 1.0,
                ],
                uv,
            }));
            let opacity = d.opacity * layers.opacity(&d.id);
            if d.visible && opacity > 0.01 {
                for vertex in &vertices[vertices.len() - d.positions.len()..] {
                    bounds.extend_with(egui::pos2(
                        (vertex.position[0] + 1.0) * 0.5,
                        (1.0 - vertex.position[1]) * 0.5,
                    ));
                }
            }
            let style = Style {
                multiply: d.multiply,
                screen: d.screen,
                control: [
                    opacity,
                    if !d.masked {
                        0.0
                    } else if d.inverted {
                        2.0
                    } else {
                        1.0
                    },
                    self.image.size.x,
                    self.image.size.y,
                ],
            };
            uniform_bytes[i * self.uniform_stride..i * self.uniform_stride + 48]
                .copy_from_slice(bytemuck::bytes_of(&style));
        }
        self.bounds = bounds.intersect(egui::Rect::from_min_max(
            egui::Pos2::ZERO,
            egui::pos2(1., 1.),
        ));
        self.state
            .queue
            .write_buffer(&self.vertices, 0, bytemuck::cast_slice(vertices));
        self.state
            .queue
            .write_buffer(&self.uniforms, 0, uniform_bytes);
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&Default::default());
        drop(begin_pass(&mut encoder, &self.output, true));
        self.order.clear();
        self.order.extend((0..drawables.len()).filter(|&i| {
            drawables[i].visible && drawables[i].opacity * layers.opacity(&drawables[i].id) > 0.0
        }));
        self.order
            .sort_unstable_by_key(|&i| (drawables[i].order, i));
        let sorted = &self.order;
        let mut cursor = 0;
        while cursor < sorted.len() {
            let d = &drawables[sorted[cursor]];
            if d.masked {
                let mut pass = begin_pass(&mut encoder, &self.mask, true);
                for &index in &d.masks {
                    let mask = &drawables[index];
                    pass.set_pipeline(&self.mask_pipelines[usize::from(mask.double_sided)]);
                    self.draw_mesh(&mut pass, index, mask, false);
                }
            }
            let mut pass = begin_pass(&mut encoder, &self.output, false);
            loop {
                let index = sorted[cursor];
                let mesh = &drawables[index];
                let blend = match mesh.blend {
                    Blend::Normal => 0,
                    Blend::Add => 1,
                    Blend::Multiply => 2,
                };
                pass.set_pipeline(&self.pipelines[blend * 2 + usize::from(mesh.double_sided)]);
                self.draw_mesh(&mut pass, index, mesh, mesh.masked);
                cursor += 1;
                if cursor == sorted.len() {
                    break;
                }
                let next = &drawables[sorted[cursor]];
                if next.masked && (!d.masked || next.masks != d.masks) {
                    break;
                }
            }
        }
        self.state.queue.submit([encoder.finish()]);
        Ok(())
    }
    fn draw_mesh(&self, pass: &mut wgpu::RenderPass<'_>, index: usize, d: &Drawable, masked: bool) {
        pass.set_bind_group(0, &self.atlases[d.texture], &[]);
        pass.set_bind_group(
            1,
            if masked {
                &self.clipped
            } else {
                &self.unclipped
            },
            &[(index * self.uniform_stride) as u32],
        );
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(self.meshes[index].indices.clone(), 0, 0..1);
    }
}

pub(crate) fn straight_alpha(pixel: [u8; 4]) -> [u8; 4] {
    let alpha = u32::from(pixel[3]);
    if alpha == 0 {
        return [0; 4];
    }
    let channel = |v: u8| ((u32::from(v) * 255 + alpha / 2) / alpha).min(255) as u8;
    [
        channel(pixel[0]),
        channel(pixel[1]),
        channel(pixel[2]),
        pixel[3],
    ]
}
/// Deferred output windows can retain a model image after its Core instance is removed.
pub struct ModelTexture {
    state: RenderState,
    id: egui::TextureId,
}
impl ModelTexture {
    pub fn register(
        state: &RenderState,
        view: &wgpu::TextureView,
    ) -> (egui::TextureId, std::sync::Arc<Self>) {
        let id = state.renderer.write().register_native_texture(
            &state.device,
            view,
            wgpu::FilterMode::Linear,
        );
        (
            id,
            std::sync::Arc::new(Self {
                state: state.clone(),
                id,
            }),
        )
    }
}
impl Drop for ModelTexture {
    fn drop(&mut self) {
        self.state.renderer.write().free_texture(&self.id);
    }
}

fn target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    usage: wgpu::TextureUsages,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage,
        view_formats: &[],
    })
}
fn begin_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Cubism pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: if clear {
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                } else {
                    wgpu::LoadOp::Load
                },
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    })
}
fn blend_state(mode: Blend) -> wgpu::BlendState {
    use wgpu::{BlendComponent as C, BlendFactor as F, BlendOperation as O};
    match mode {
        Blend::Normal => wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
        Blend::Add => wgpu::BlendState {
            color: C {
                src_factor: F::One,
                dst_factor: F::One,
                operation: O::Add,
            },
            alpha: C {
                src_factor: F::Zero,
                dst_factor: F::One,
                operation: O::Add,
            },
        },
        Blend::Multiply => wgpu::BlendState {
            color: C {
                src_factor: F::Dst,
                dst_factor: F::OneMinusSrcAlpha,
                operation: O::Add,
            },
            alpha: C {
                src_factor: F::Zero,
                dst_factor: F::One,
                operation: O::Add,
            },
        },
    }
}
fn pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    blend: wgpu::BlendState,
    double_sided: bool,
    mask: bool,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Cubism mesh"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 16,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(if mask { "mask_fragment" } else { "fragment" }),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: if double_sided {
                None
            } else {
                Some(wgpu::Face::Back)
            },
            ..Default::default()
        },
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_export_converts_premultiplied_edges_to_straight_alpha() {
        assert_eq!(straight_alpha([64, 32, 0, 128]), [128, 64, 0, 128]);
        assert_eq!(straight_alpha([1, 2, 3, 0]), [0, 0, 0, 0]);
        assert_eq!(straight_alpha([255, 100, 40, 255]), [255, 100, 40, 255]);
    }
    fn quad(texture: usize, order: i32, right: f32) -> Drawable {
        Drawable {
            id: format!("Mesh{order}"),
            part: String::new(),
            positions: vec![[0.0, 0.0], [right, 0.0], [right, 4.0], [0.0, 4.0]],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture,
            masks: vec![],
            masked: false,
            inverted: false,
            double_sided: false,
            visible: true,
            order,
            opacity: 1.0,
            multiply: [1.0; 4],
            screen: [0.0, 0.0, 0.0, 1.0],
            blend: Blend::Normal,
        }
    }
    fn pixels(renderer: &ModelRenderer) -> Vec<u8> {
        let state = &renderer.state;
        let texture = renderer.output.texture();
        let size = texture.size();
        let stride = (size.width * 4).div_ceil(256) * 256;
        let buffer = state.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("test readback"),
            size: stride as u64 * size.height as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
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
                    rows_per_image: Some(size.height),
                },
            },
            size,
        );
        state.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            tx.send(r).unwrap();
        });
        state
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let data = buffer
            .slice(..)
            .get_mapped_range()
            .expect("mapped GPU readback")
            .to_vec();
        buffer.unmap();
        data
    }
    fn near(actual: &[u8], expected: [u8; 4]) {
        for (a, e) in actual.iter().zip(expected) {
            assert!(
                a.abs_diff(e) <= 2,
                "Actual {actual:?}, expected {expected:?}"
            );
        }
    }
    #[test]
    fn canvas_fits_off_canvas_meshes_and_grows_without_shrinking_or_reallocating() {
        let canvas = Canvas {
            size: [100., 200.],
            origin: [50., 100.],
            pixels_per_unit: 100.,
        };
        let mut d = quad(0, 0, 1.0);
        d.positions = vec![[-1.4, -2.2], [1.8, -2.2], [1.8, 2.3], [-1.4, 2.3]];
        d.visible = true;
        let view = fit_canvas(canvas, &[d.clone()], None);
        let check = |view: Canvas, d: &Drawable| {
            assert!((view.size[0] / view.size[1] - 0.5).abs() < 1e-6);
            for p in &d.positions {
                for (axis, coordinate) in p.iter().enumerate() {
                    let value =
                        (coordinate * view.pixels_per_unit + view.origin[axis]) / view.size[axis];
                    assert!((0.0..1.0).contains(&value), "Clipped axis {axis}: {value}");
                }
            }
        };
        check(view, &d);
        d.positions[0][0] = -3.0;
        let grown = fit_canvas(canvas, &[d.clone()], Some(view));
        check(grown, &d);
        assert!(grown.size[1] > view.size[1]);
        d.positions = vec![[0., 0.]; 4];
        let settled = fit_canvas(canvas, &[d], Some(grown));
        assert_eq!(settled.size, grown.size);
        assert_eq!(settled.origin, grown.origin);
    }
    #[test]
    #[ignore = "requires a graphics adapter; no Cubism SDK or model needed"]
    fn gpu_clipping_blending_colors_culling_and_draw_order() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let renderer = eframe::egui_wgpu::Renderer::new(&device, FORMAT, Default::default());
        let state = RenderState {
            instance: instance.clone(),
            surface_config: eframe::egui_wgpu::SurfaceConfig::LOW_LATENCY,
            adapter,
            available_adapters: vec![],
            device,
            queue,
            target_format: FORMAT,
            renderer: std::sync::Arc::new(egui::mutex::RwLock::new(renderer)),
        };
        let temp = tempfile::tempdir().unwrap();
        let paths: Vec<_> = [[255, 0, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]]
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let path = temp.path().join(format!("{i}.png"));
                image::save_buffer(&path, &c, 1, 1, image::ColorType::Rgba8).unwrap();
                path
            })
            .collect();
        let canvas = Canvas {
            size: [4.0, 4.0],
            origin: [0.0, 0.0],
            pixels_per_unit: 1.0,
        };
        // Exercise a source ten times the former atlas dimension ceiling on a real GPU.
        {
            let large = temp.path().join("wide-atlas.png");
            image::RgbaImage::from_pixel(81_920, 80, image::Rgba([120, 80, 220, 128]))
                .save(&large)
                .unwrap();
            let scene = [quad(0, 0, 4.0)];
            let mut fitted = ModelRenderer::new(&state, canvas, &scene, &[large]).unwrap();
            assert_eq!(fitted.import_notes.len(), 1);
            let [w, h] =
                limits::texture_size(81_920, 80, state.device.limits().max_texture_dimension_2d);
            assert_eq!(
                fitted.atlas_mib,
                f64::from(w) * f64::from(h) * 4.0 / 1048576.0
            );
            fitted.render(canvas, &scene).unwrap();
            let data = pixels(&fitted);
            let center = (1024 * 2048 + 1024) * 4;
            near(&data[center..center + 4], [60, 40, 110, 128]);
        }
        // Deliberately reverse storage order: blue background must render first.
        let mut scene = vec![
            quad(0, 1, 4.0),
            quad(1, 0, 4.0),
            quad(2, 2, 2.0),
            quad(2, 3, 2.0),
        ];
        scene[2].visible = false;
        scene[2].opacity = 0.0;
        scene[3].visible = false;
        for p in &mut scene[3].positions {
            p[0] += 2.0;
        }
        scene[0].masked = true;
        scene[0].masks = vec![2];
        let mut renderer = ModelRenderer::new(&state, canvas, &scene, &paths).unwrap();
        let mut check = |scene: &[Drawable], left, right| {
            renderer.render(canvas, scene).unwrap();
            let data = pixels(&renderer);
            let w = renderer.image.size.x as usize;
            let h = renderer.image.size.y as usize;
            let l = (h / 2 * w + w / 4) * 4;
            let r = (h / 2 * w + 3 * w / 4) * 4;
            near(&data[l..l + 4], left);
            near(&data[r..r + 4], right);
        };
        check(&scene, [255, 0, 0, 255], [0, 0, 255, 255]);
        scene[0].masks = vec![2, 3];
        check(&scene, [255, 0, 0, 255], [255, 0, 0, 255]);
        scene[0].masks = vec![2];
        scene[0].inverted = true;
        check(&scene, [0, 0, 255, 255], [255, 0, 0, 255]);
        scene[0].masked = false;
        scene[0].opacity = 0.5;
        check(&scene, [128, 0, 128, 255], [128, 0, 128, 255]);
        scene[1].visible = false;
        check(&scene, [128, 0, 0, 128], [128, 0, 0, 128]);
        scene[1].visible = true;
        scene[0].opacity = 1.0;
        scene[0].blend = Blend::Add;
        check(&scene, [255, 0, 255, 255], [255, 0, 255, 255]);
        scene[0].blend = Blend::Multiply;
        check(&scene, [0, 0, 0, 255], [0, 0, 0, 255]);
        scene[0].blend = Blend::Normal;
        scene[0].multiply = [0.5, 1.0, 1.0, 1.0];
        scene[0].screen = [0.0, 1.0, 0.0, 1.0];
        check(&scene, [128, 255, 0, 255], [128, 255, 0, 255]);
        // Reverse geometry winding via its positions while keeping UV/index topology.
        scene[0].positions.swap(0, 1);
        scene[0].positions.swap(2, 3);
        check(&scene, [0, 0, 255, 255], [0, 0, 255, 255]);
        scene[0].double_sided = true;
        check(&scene, [128, 255, 0, 255], [128, 255, 0, 255]);
        // User opacity applies to color only; hidden mask ArtMeshes still clip.
        scene[0].masked = true;
        scene[0].inverted = false;
        scene[0].multiply = [1.; 4];
        scene[0].screen = [0.; 4];
        let mut layers = aria_core::layers::Config::default();
        layers.opacity.insert(scene[0].id.clone(), 0.5);
        layers.opacity.insert(scene[2].id.clone(), 0.);
        renderer.render_layers(canvas, &scene, &layers).unwrap();
        let data = pixels(&renderer);
        let pixel = |x: usize| &data[(1024 * 2048 + x) * 4..(1024 * 2048 + x) * 4 + 4];
        near(pixel(512), [128, 0, 128, 255]);
        near(pixel(1536), [0, 0, 255, 255]);
    }
}
