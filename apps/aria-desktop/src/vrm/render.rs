//! GPU skinning and bounded active morphs on one transparent canvas.
use super::asset::{Asset, Geometry as AssetGeometry, Vertex};
use crate::cubism_render::{ModelImage, ModelTexture};
use anyhow::{Result, ensure};
use aria_core::vrm::Settings;
use eframe::{egui, egui_wgpu::RenderState};
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Joint {
    position: [[f32; 4]; 4],
    normal: [[f32; 4]; 4],
}
fn joint_transform(matrix: Mat4) -> Joint {
    Joint {
        position: matrix.to_cols_array_2d(),
        normal: matrix.inverse().transpose().to_cols_array_2d(),
    }
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MorphDelta {
    position: [f32; 4],
    normal: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MorphInfo {
    vertex_count: u32,
    morph_count: u32,
    padding: [u32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Frame {
    vp: [[f32; 4]; 4],
    front: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    settings: [f32; 4],
    light_direction: [f32; 4],
    light_color: [f32; 4],
}
struct Geometry {
    vertices: wgpu::Buffer,
    joints: wgpu::Buffer,
    morph_deltas: wgpu::Buffer,
    morph_weights: wgpu::Buffer,
    morph_info: wgpu::Buffer,
    gpu_morph: bool,
    active_morphs: Vec<usize>,
    using_cpu_morphs: bool,
    bind: wgpu::BindGroup,
    staging: Vec<Vertex>,
    weights: Vec<f32>,
    palette: Vec<Joint>,
}
impl Geometry {
    fn update_morphs(
        &mut self,
        source: &AssetGeometry,
        weights: &[f32],
        queue: &wgpu::Queue,
    ) -> Result<()> {
        ensure!(
            weights.len() == source.morphs.len(),
            "VRM morph weight count changed"
        );
        ensure!(
            weights.iter().all(|weight| weight.is_finite()),
            "Non-finite VRM morph weight"
        );
        if self.weights == weights {
            return Ok(());
        }
        let active = weights
            .iter()
            .enumerate()
            .filter_map(|(index, weight)| (weight.abs() >= 0.00001).then_some(index))
            .collect::<Vec<_>>();
        let capacity = (self.morph_weights.size() as usize) / std::mem::size_of::<f32>();
        if self.gpu_morph && active.len() <= capacity {
            if self.using_cpu_morphs {
                queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&source.vertices));
                self.using_cpu_morphs = false;
            }
            if active != self.active_morphs {
                let mut packed = Vec::with_capacity(active.len() * source.vertices.len());
                for &index in &active {
                    let morph = &source.morphs[index];
                    ensure!(
                        morph.position.len() == source.vertices.len()
                            && morph.normal.len() == source.vertices.len(),
                        "VRM morph vertex count differs from geometry"
                    );
                    for (position, normal) in morph.position.iter().zip(&morph.normal) {
                        packed.push(MorphDelta {
                            position: [position.x, position.y, position.z, 0.0],
                            normal: [normal.x, normal.y, normal.z, 0.0],
                        });
                    }
                }
                if !packed.is_empty() {
                    queue.write_buffer(&self.morph_deltas, 0, bytemuck::cast_slice(&packed));
                }
                queue.write_buffer(
                    &self.morph_info,
                    0,
                    bytemuck::bytes_of(&MorphInfo {
                        vertex_count: source.vertices.len() as u32,
                        morph_count: active.len() as u32,
                        padding: [0; 2],
                    }),
                );
                self.active_morphs = active.clone();
            }
            if !active.is_empty() {
                let active_weights = active
                    .iter()
                    .map(|&index| weights[index])
                    .collect::<Vec<_>>();
                queue.write_buffer(
                    &self.morph_weights,
                    0,
                    bytemuck::cast_slice(&active_weights),
                );
            }
        } else {
            if !self.active_morphs.is_empty() {
                queue.write_buffer(
                    &self.morph_info,
                    0,
                    bytemuck::bytes_of(&MorphInfo {
                        vertex_count: source.vertices.len() as u32,
                        morph_count: 0,
                        padding: [0; 2],
                    }),
                );
                self.active_morphs.clear();
            }
            if self.staging.len() == source.vertices.len() {
                self.staging.copy_from_slice(&source.vertices);
            } else {
                self.staging = source.vertices.clone();
            }
            for (morph, &weight) in source.morphs.iter().zip(weights) {
                if weight.abs() < 0.00001 {
                    continue;
                }
                for (vertex, delta) in self.staging.iter_mut().zip(&morph.position) {
                    vertex.position = (Vec3::from(vertex.position) + *delta * weight).to_array();
                }
                for (vertex, delta) in self.staging.iter_mut().zip(&morph.normal) {
                    vertex.normal = (Vec3::from(vertex.normal) + *delta * weight).to_array();
                }
            }
            queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&self.staging));
            self.using_cpu_morphs = true;
        }
        self.weights.clone_from_slice(weights);
        Ok(())
    }
}
struct Part {
    indices: wgpu::Buffer,
    count: u32,
    geometry: usize,
    material: usize,
    pipeline: usize,
}
pub struct Renderer {
    state: RenderState,
    geometry: Vec<Geometry>,
    skin_palettes: Vec<Vec<Joint>>,
    parts: Vec<Part>,
    materials: Vec<wgpu::BindGroup>,
    pipelines: Vec<wgpu::RenderPipeline>,
    outline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    frame: wgpu::BindGroup,
    output: wgpu::TextureView,
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    pub image: ModelImage,
    pub lease: Arc<ModelTexture>,
    pub bytes: u64,
    palette: crate::chroma::Palette,
    projection: Mat4,
}
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
impl Renderer {
    pub fn new(state: &RenderState, asset: &mut Asset, settings: &Settings) -> Result<Self> {
        let device = &state.device;
        let buffer_layout = |binding, ty, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VRM frame"),
            entries: &[buffer_layout(
                0,
                wgpu::BufferBindingType::Uniform,
                wgpu::ShaderStages::VERTEX_FRAGMENT,
            )],
        });
        let skin_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VRM skin"),
            entries: &[
                buffer_layout(
                    0,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                buffer_layout(
                    1,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                buffer_layout(
                    2,
                    wgpu::BufferBindingType::Storage { read_only: true },
                    wgpu::ShaderStages::VERTEX,
                ),
                buffer_layout(
                    3,
                    wgpu::BufferBindingType::Uniform,
                    wgpu::ShaderStages::VERTEX,
                ),
            ],
        });
        let mut entries: Vec<_> = (0..5)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        entries.push(buffer_layout(
            6,
            wgpu::BufferBindingType::Uniform,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
        ));
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("VRM material"),
            entries: &entries,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VRM camera"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VRM pipeline"),
            bind_group_layouts: &[
                Some(&frame_layout),
                Some(&material_layout),
                Some(&skin_layout),
            ],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA VRM toon and skinning"),
            source: wgpu::ShaderSource::Wgsl(include_str!("render.wgsl").into()),
        });
        let pipeline = |cull, transparent: bool, outline: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor{
            label:Some("VRM material pipeline"),layout:Some(&layout),
            vertex:wgpu::VertexState{module:&shader,entry_point:Some(if outline{"vs_outline"}else{"vs"}),compilation_options:Default::default(),buffers:&[Some(wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Vertex>()as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Float32x4,4=>Uint32x4,5=>Float32x4]})]},
            fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some(if outline{"fs_outline"}else{"fs"}),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format:FORMAT,blend:Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),
            primitive:wgpu::PrimitiveState{cull_mode:cull,..Default::default()},
            depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:Some(!transparent),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:Default::default()}),
            multisample:wgpu::MultisampleState{count:4,..Default::default()},multiview_mask:None,cache:None})
        };
        let pipelines = (0..6)
            .map(|i| {
                pipeline(
                    match i % 3 {
                        1 => Some(wgpu::Face::Front),
                        2 => Some(wgpu::Face::Back),
                        _ => None,
                    },
                    i >= 3,
                    false,
                )
            })
            .collect();
        let outline = pipeline(Some(wgpu::Face::Front), false, true);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("VRM texture sampling"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let mut bytes = 0;
        let mut palette = crate::chroma::Palette::default();
        let mut views = Vec::new();
        for slot in &mut asset.images {
            if let Some(mut image) = slot.take() {
                let max = device.limits().max_texture_dimension_2d.min(8192);
                let size =
                    aria_core::asset_limits::texture_size(image.width(), image.height(), max);
                if [image.width(), image.height()] != size {
                    image = image::imageops::resize(
                        &image,
                        size[0],
                        size[1],
                        image::imageops::FilterType::Triangle,
                    );
                    asset.warnings.push(format!(
                        "A VRM texture was fitted to {} × {} for this GPU.",
                        size[0], size[1]
                    ));
                }
                palette.add_rgba(&image);
                bytes += image.len() as u64;
                views.push(Some(texture(state, &image)));
            } else {
                views.push(None);
            }
        }
        let white = texture(
            state,
            &image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4])),
        );
        let black = texture(
            state,
            &image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255])),
        );
        let normal = texture(
            state,
            &image::RgbaImage::from_pixel(1, 1, image::Rgba([128, 128, 255, 255])),
        );
        let mut materials = Vec::new();
        for m in &asset.materials {
            let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM material values"),
                contents: bytemuck::bytes_of(&m.uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let base = m.textures[0]
                .and_then(|i| views[i].as_ref())
                .unwrap_or(&white);
            let fallback = [base, base, &white, &normal, &black];
            let mut entries: Vec<_> = (0..5)
                .map(|i| wgpu::BindGroupEntry {
                    binding: i as u32,
                    resource: wgpu::BindingResource::TextureView(
                        m.textures[i]
                            .and_then(|j| views[j].as_ref())
                            .unwrap_or(fallback[i]),
                    ),
                })
                .collect();
            entries.push(wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::Sampler(&sampler),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 6,
                resource: uniform.as_entire_binding(),
            });
            materials.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("VRM textures"),
                layout: &material_layout,
                entries: &entries,
            }));
        }
        let mut geometry = Vec::new();
        let skin_palettes = asset
            .skins
            .iter()
            .map(|skin| vec![joint_transform(Mat4::IDENTITY); skin.joints.len()])
            .collect();
        let mut gpu_morph_budget = 64 * 1024 * 1024_usize;
        for g in &asset.geometry {
            let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM shared morph mesh"),
                contents: bytemuck::cast_slice(&g.vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
            let count = g.skin.map_or(1, |s| asset.skins[s].joints.len());
            let palette = vec![
                Joint {
                    position: Mat4::IDENTITY.to_cols_array_2d(),
                    normal: Mat4::IDENTITY.to_cols_array_2d()
                };
                count
            ];
            let joints = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM skin palette"),
                contents: bytemuck::cast_slice(&palette),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
            let vertex_count = g.vertices.len();
            let slot_count = g.morphs.len().min(8);
            let morph_bytes = slot_count
                .checked_mul(vertex_count)
                .and_then(|count| count.checked_mul(std::mem::size_of::<MorphDelta>()))
                .ok_or_else(|| anyhow::anyhow!("VRM morph storage size overflow"))?;
            let max_binding = device.limits().max_storage_buffer_binding_size as usize;
            let gpu_morph = !g.morphs.is_empty()
                && morph_bytes <= max_binding
                && morph_bytes <= gpu_morph_budget
                && (morph_bytes as u64) <= device.limits().max_buffer_size;
            if gpu_morph {
                gpu_morph_budget -= morph_bytes;
            }
            let morph_deltas = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("VRM active morph deltas"),
                size: if gpu_morph {
                    morph_bytes
                } else {
                    std::mem::size_of::<MorphDelta>()
                } as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let initial_weights = vec![0.0_f32; slot_count.max(1)];
            let morph_weights = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM morph weights"),
                contents: bytemuck::cast_slice(&initial_weights),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
            let morph_info = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM morph layout"),
                contents: bytemuck::bytes_of(&MorphInfo {
                    vertex_count: vertex_count as u32,
                    morph_count: 0,
                    padding: [0; 2],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &skin_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: joints.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: morph_deltas.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: morph_weights.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: morph_info.as_entire_binding(),
                    },
                ],
            });
            bytes += vertices.size()
                + joints.size()
                + morph_deltas.size()
                + morph_weights.size()
                + morph_info.size();
            geometry.push(Geometry {
                vertices,
                joints,
                morph_deltas,
                morph_weights,
                morph_info,
                gpu_morph,
                active_morphs: Vec::new(),
                using_cpu_morphs: false,
                bind,
                staging: if gpu_morph {
                    Vec::new()
                } else {
                    g.vertices.clone()
                },
                weights: vec![f32::NAN; g.weights.len()],
                palette,
            });
        }
        let mut parts = Vec::new();
        for p in &asset.parts {
            let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM triangle indices"),
                contents: bytemuck::cast_slice(&p.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            bytes += indices.size();
            let m = &asset.materials[p.material];
            parts.push(Part {
                indices,
                count: p.indices.len() as u32,
                geometry: p.geometry,
                material: p.material,
                pipeline: m.cull.min(2) as usize + if m.uniform.mode[1] == 2. { 3 } else { 0 },
            });
        }
        parts.sort_by_key(|p| {
            (
                asset.materials[p.material].uniform.mode[1] >= 2.,
                asset.materials[p.material].queue,
            )
        });
        let (output, msaa, depth, image, lease) = targets(state, settings.resolution)?;
        Ok(Self {
            state: state.clone(),
            geometry,
            skin_palettes,
            parts,
            materials,
            pipelines,
            outline,
            uniform,
            frame,
            output,
            msaa,
            depth,
            image,
            lease,
            bytes,
            palette,
            projection: Mat4::IDENTITY,
        })
    }
    pub fn palette(&self) -> crate::chroma::Palette {
        self.palette.clone()
    }
    #[cfg(test)]
    pub(super) fn gpu_morph_usage(&self) -> (usize, usize) {
        (
            self.geometry.iter().filter(|g| g.gpu_morph).count(),
            self.geometry
                .iter()
                .filter(|g| g.gpu_morph && !g.active_morphs.is_empty() && !g.using_cpu_morphs)
                .count(),
        )
    }
    #[cfg(test)]
    pub(super) fn force_cpu_morphs_for_test(&mut self) -> Vec<bool> {
        self.geometry
            .iter_mut()
            .map(|geometry| {
                let enabled = geometry.gpu_morph;
                geometry.gpu_morph = false;
                geometry.weights.fill(f32::NAN);
                enabled
            })
            .collect()
    }
    #[cfg(test)]
    pub(super) fn restore_gpu_morphs_for_test(&mut self, enabled: &[bool]) {
        for (geometry, &enabled) in self.geometry.iter_mut().zip(enabled) {
            geometry.gpu_morph = enabled;
            geometry.weights.fill(f32::NAN);
        }
    }
    /// Project one vertex for picking without morphing every vertex on the CPU.
    pub fn projected_vertex(&self, asset: &Asset, geometry: usize, vertex: u32) -> Option<Vec3> {
        let g = self.geometry.get(geometry)?;
        let source = asset.geometry.get(geometry)?;
        let v = source.vertices.get(vertex as usize)?;
        let mut position = Vec3::from(v.position);
        for (morph, &weight) in source.morphs.iter().zip(&g.weights) {
            if weight.abs() >= 0.00001 {
                position += *morph.position.get(vertex as usize)? * weight;
            }
        }
        let mut world = glam::Vec4::ZERO;
        for (&joint, &weight) in v.joints.iter().zip(&v.weights) {
            if weight == 0.0 {
                continue;
            }
            let m = Mat4::from_cols_array_2d(&g.palette.get(joint as usize)?.position);
            world += (m * position.extend(1.0)) * weight;
        }
        let clip = self.projection * world;
        if !clip.is_finite() || clip.w.abs() < 1e-6 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(Vec3::new(ndc.x * 0.375, -ndc.y * 0.5, ndc.z))
    }
    pub fn render(
        &mut self,
        asset: &Asset,
        world: &[Mat4],
        weights: &[Vec<f32>],
        settings: &Settings,
    ) -> Result<()> {
        if self.image.size.y as u32 != settings.resolution {
            let (output, msaa, depth, image, lease) = targets(&self.state, settings.resolution)?;
            self.output = output;
            self.msaa = msaa;
            self.depth = depth;
            self.image = image;
            self.lease = lease;
        }
        let queue = &self.state.queue;
        for (skin, palette) in asset.skins.iter().zip(&mut self.skin_palettes) {
            for ((joint, &node), &inverse) in
                palette.iter_mut().zip(&skin.joints).zip(&skin.inverse)
            {
                *joint = joint_transform(world[node] * inverse);
            }
        }
        for (i, (source, gpu)) in asset.geometry.iter().zip(&mut self.geometry).enumerate() {
            gpu.update_morphs(source, &weights[i], queue)?;
            if let Some(skin) = source.skin {
                gpu.palette.copy_from_slice(&self.skin_palettes[skin]);
            } else {
                gpu.palette[0] = joint_transform(world[source.node]);
            }
            queue.write_buffer(&gpu.joints, 0, bytemuck::cast_slice(&gpu.palette));
        }
        let extent = asset.bounds[1] - asset.bounds[0];
        let center = (asset.bounds[1] + asset.bounds[0]) * 0.5;
        let portrait = asset
            .bones
            .get("head")
            .map_or(center + Vec3::Y * extent.y * 0.25, |&h| {
                asset
                    .front
                    .transform_point3(asset.nodes[h].world.transform_point3(Vec3::ZERO))
                    + Vec3::Y * extent.y * 0.02
            });
        let target = center.lerp(portrait, settings.portrait);
        let full_height = extent.y.max(extent.x / 0.75) * 1.1;
        let height = full_height + (extent.y * 0.38 - full_height) * settings.portrait;
        let orbit = glam::Quat::from_rotation_y(settings.yaw.to_radians())
            * glam::Quat::from_rotation_x(-settings.pitch.to_radians());
        let distance = extent.length().max(1.) * 2.;
        let view = Mat4::look_at_rh(target + orbit * Vec3::Z * distance, target, orbit * Vec3::Y);
        let projection = Mat4::orthographic_rh(
            -height * 0.375,
            height * 0.375,
            -height * 0.5,
            height * 0.5,
            0.01,
            distance * 3.,
        );
        self.projection = projection * view * asset.front;
        let frame = Frame {
            vp: (projection * view).to_cols_array_2d(),
            front: asset.front.to_cols_array_2d(),
            view: view.to_cols_array_2d(),
            settings: [
                settings.light,
                f32::from(settings.lighting.enabled),
                f32::from(settings.lighting.directional),
                settings.lighting.ambient,
            ],
            light_direction: {
                let d = settings.lighting.direction();
                [d[0], d[1], d[2], 0.]
            },
            light_color: [
                settings.lighting.color[0],
                settings.lighting.color[1],
                settings.lighting.color[2],
                1.,
            ],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&frame));
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("VRM avatar canvas"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa,
                    resolve_target: Some(&self.output),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Discard,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.frame, &[]);
            for p in &self.parts {
                let g = &self.geometry[p.geometry];
                pass.set_vertex_buffer(0, g.vertices.slice(..));
                pass.set_index_buffer(p.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_bind_group(1, &self.materials[p.material], &[]);
                pass.set_bind_group(2, &g.bind, &[]);
                if settings.outlines && asset.materials[p.material].uniform.mode[0] > 0. {
                    pass.set_pipeline(&self.outline);
                    pass.draw_indexed(0..p.count, 0, 0..1);
                }
                pass.set_pipeline(&self.pipelines[p.pipeline]);
                pass.draw_indexed(0..p.count, 0, 0..1);
            }
        }
        queue.submit([encoder.finish()]);
        Ok(())
    }
    #[cfg(all(test, windows))]
    pub fn read_rgba(&self) -> Result<Vec<u8>> {
        let size = self.output.texture().size();
        let stride = (size.width * 4).div_ceil(256) * 256;
        let buffer = self.state.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("VRM test readback"),
            size: u64::from(stride) * u64::from(size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.output.texture().as_image_copy(),
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
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.state.device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(15)),
        })?;
        rx.recv_timeout(std::time::Duration::from_secs(1))??;
        let data = buffer.slice(..).get_mapped_range()?;
        Ok(data
            .chunks_exact(stride as usize)
            .flat_map(|row| row[..size.width as usize * 4].iter().copied())
            .collect())
    }
}
fn texture(state: &RenderState, image: &image::RgbaImage) -> wgpu::TextureView {
    state
        .device
        .create_texture_with_data(
            &state.queue,
            &wgpu::TextureDescriptor {
                label: Some("VRM material image"),
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            image,
        )
        .create_view(&Default::default())
}
type Targets = (
    wgpu::TextureView,
    wgpu::TextureView,
    wgpu::TextureView,
    ModelImage,
    Arc<ModelTexture>,
);
fn targets(state: &RenderState, resolution: u32) -> Result<Targets> {
    ensure!(
        (512..=4096).contains(&resolution)
            && resolution <= state.device.limits().max_texture_dimension_2d,
        "VRM canvas resolution is unsupported by this GPU"
    );
    let size = wgpu::Extent3d {
        width: resolution * 3 / 4,
        height: resolution,
        depth_or_array_layers: 1,
    };
    let target = |format, sample_count, usage| {
        state
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("VRM avatar canvas"),
                size,
                mip_level_count: 1,
                sample_count,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&Default::default())
    };
    let output = target(
        FORMAT,
        1,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
    );
    let msaa = target(FORMAT, 4, wgpu::TextureUsages::RENDER_ATTACHMENT);
    let depth = target(
        wgpu::TextureFormat::Depth32Float,
        4,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let (id, lease) = ModelTexture::register(state, &output);
    Ok((
        output,
        msaa,
        depth,
        ModelImage {
            id,
            size: egui::vec2(size.width as f32, size.height as f32),
        },
        lease,
    ))
}
