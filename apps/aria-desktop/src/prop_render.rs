//! One cached GPU prop image per active 3D asset; projectiles share it across outputs.
use crate::{
    cubism_render::{ModelImage, ModelTexture},
    items::ItemImage,
    mesh_asset::{Mesh, Vertex},
};
use eframe::{egui, egui_wgpu::RenderState};
use std::sync::Arc;
use wgpu::util::DeviceExt;
pub struct Prop {
    state: RenderState,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    frame: wgpu::BindGroup,
    parts: Vec<(wgpu::Buffer, u32, wgpu::BindGroup)>,
    output: wgpu::TextureView,
    depth: wgpu::TextureView,
    image: ModelImage,
    lease: Arc<ModelTexture>,
    pub bytes: u64,
    revision: u64,
}
const SHADER: &str = r#"
struct Frame {mvp:mat4x4<f32>,rotation:mat4x4<f32>};
@group(0) @binding(0) var<uniform> frame:Frame;
@group(1) @binding(0) var art:texture_2d<f32>;
@group(1) @binding(1) var sam:sampler;
struct Out {@builtin(position) pos:vec4<f32>,@location(0) normal:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) color:vec4<f32>};
@vertex fn vs(@location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) color:vec4<f32>)->Out {
 var o:Out;o.pos=frame.mvp*vec4(pos,1);o.normal=(frame.rotation*vec4(normal,0)).xyz;o.uv=uv;o.color=color;return o;
}
@fragment fn fs(i:Out)->@location(0) vec4<f32> {
 let c=textureSample(art,sam,i.uv)*i.color;if(c.a<0.01){discard;}
 let light=0.55+0.45*abs(dot(normalize(i.normal),normalize(vec3(-0.3,0.7,1))));
 return vec4(c.rgb*light*c.a,c.a);
}
"#;
impl Prop {
    pub fn new(state: &RenderState, mesh: Mesh) -> Self {
        let device = &state.device;
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Prop transform"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let frame = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA static 3D props"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline=device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {label:Some("ARIA 3D prop pipeline"),layout:Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor{label:None,bind_group_layouts:&[Some(&layout), Some(&texture_layout)],immediate_size:0})),vertex:wgpu::VertexState{module:&shader,entry_point:Some("vs"),compilation_options:Default::default(),buffers:&[Some(wgpu::VertexBufferLayout{array_stride:std::mem::size_of::<Vertex>() as u64,step_mode:wgpu::VertexStepMode::Vertex,attributes:&wgpu::vertex_attr_array![0=>Float32x3,1=>Float32x3,2=>Float32x2,3=>Float32x4]})]},fragment:Some(wgpu::FragmentState{module:&shader,entry_point:Some("fs"),compilation_options:Default::default(),targets:&[Some(wgpu::ColorTargetState{format:wgpu::TextureFormat::Rgba8Unorm,blend:Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),write_mask:wgpu::ColorWrites::ALL})]}),primitive:wgpu::PrimitiveState{cull_mode:None,..Default::default()},depth_stencil:Some(wgpu::DepthStencilState{format:wgpu::TextureFormat::Depth32Float,depth_write_enabled:Some(true),depth_compare:Some(wgpu::CompareFunction::LessEqual),stencil:Default::default(),bias:Default::default()}),multisample:Default::default(),multiview_mask:None,cache:None});
        let target = |format, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("ARIA 3D prop target"),
                    size: wgpu::Extent3d {
                        width: 512,
                        height: 512,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let output = target(
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = target(
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let (id, lease) = ModelTexture::register(state, &output);
        let mut bytes = 512 * 512 * 8;
        let mut parts = Vec::new();
        for part in mesh.parts {
            let count = part.vertices.len() as u32;
            let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Prop mesh"),
                contents: bytemuck::cast_slice(&part.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            bytes += vertex.size();
            let image = part
                .image
                .unwrap_or_else(|| image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4])));
            bytes += image.len() as u64;
            let texture = device.create_texture_with_data(
                &state.queue,
                &wgpu::TextureDescriptor {
                    label: Some("Prop material"),
                    size: wgpu::Extent3d {
                        width: image.width(),
                        height: image.height(),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &image,
            );
            let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &texture_layout,
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
            });
            parts.push((vertex, count, binding));
        }
        Self {
            state: state.clone(),
            pipeline,
            uniform,
            frame,
            parts,
            output,
            depth,
            image: ModelImage {
                id,
                size: egui::vec2(512.0, 512.0),
            },
            lease,
            bytes,
            revision: 0,
        }
    }
    pub fn render(&mut self, time: f32) {
        let rotation = glam::Mat4::from_rotation_y(time * 1.8)
            * glam::Mat4::from_rotation_x(0.25 + (time * 0.8).sin() * 0.3);
        let view = glam::Mat4::look_at_rh(
            glam::Vec3::new(0.0, 0.0, 3.0),
            glam::Vec3::ZERO,
            glam::Vec3::Y,
        );
        let projection = glam::Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, 0.1, 10.0);
        self.state.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&[
                (projection * view * rotation).to_cols_array(),
                rotation.to_cols_array(),
            ]),
        );
        let mut encoder = self
            .state
            .device
            .create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Prop render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.output,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.frame, &[]);
            for (vertex, count, texture) in &self.parts {
                pass.set_vertex_buffer(0, vertex.slice(..));
                pass.set_bind_group(1, texture, &[]);
                pass.draw(0..*count, 0..1);
            }
        }
        self.state.queue.submit([encoder.finish()]);
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn image(&self) -> ItemImage {
        ItemImage::Model {
            mount: None,
            image: self.image,
            _lease: self.lease.clone(),
            revision: self.revision,
        }
    }
}
