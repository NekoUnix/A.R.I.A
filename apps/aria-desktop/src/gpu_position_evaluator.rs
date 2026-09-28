//! Resident GPU normal-key evaluation for MOC3 mesh and warp positions.
//!
//! Later hierarchy, blend-shape and glue stages can use the same output
//! buffer. Mesh vertices lead the buffer for direct binding by ModelRenderer.

use anyhow::{Result, ensure};
use aria_model_core::gpu_blend_plan::GpuBlendDeltaPlan;
use aria_model_core::gpu_key_plan::{ActiveFrame, GpuPositionKeyPlan};
use wgpu::util::DeviceExt;

pub struct GpuPositionEvaluator {
    plan: GpuPositionKeyPlan,
    active: ActiveFrame,
    _source: wgpu::Buffer,
    _work: wgpu::Buffer,
    active_buffer: wgpu::Buffer,
    counts_buffer: wgpu::Buffer,
    output: wgpu::Buffer,
    bind: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    blend: Option<ResidentBlendStage>,
}

struct ResidentBlendStage {
    plan: GpuBlendDeltaPlan,
    active: ActiveFrame,
    _source: wgpu::Buffer,
    _work: wgpu::Buffer,
    active_buffer: wgpu::Buffer,
    counts_buffer: wgpu::Buffer,
    bind: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
}

impl GpuPositionEvaluator {
    pub fn new(device: &wgpu::Device, plan: GpuPositionKeyPlan) -> Result<Self> {
        ensure!(!plan.work.is_empty(), "GPU position plan has no geometry");
        let active = plan.empty_active_frame();
        let limits = device.limits();
        let source_size = std::mem::size_of_val(plan.source_points.as_slice()) as u64;
        let work_size = std::mem::size_of_val(plan.work.as_slice()) as u64;
        let key_size = std::mem::size_of_val(active.keys.as_slice()) as u64;
        let count_size = std::mem::size_of_val(active.counts.as_slice()) as u64;
        let output_size = plan.work.len() as u64 * 8;
        for size in [source_size, work_size, key_size, count_size, output_size] {
            ensure!(
                size > 0 && size <= limits.max_storage_buffer_binding_size,
                "GPU position plan exceeds a storage binding"
            );
        }
        ensure!(
            plan.work.len().div_ceil(64) <= limits.max_compute_workgroups_per_dimension as usize,
            "GPU position plan exceeds a dispatch dimension"
        );
        let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA resident MOC3 key positions"),
            contents: bytemuck::cast_slice(&plan.source_points),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let work = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA resident MOC3 position work"),
            contents: bytemuck::cast_slice(&plan.work),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let active_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA dynamic MOC3 active keys"),
            size: key_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let counts_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA dynamic MOC3 active counts"),
            size: count_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA GPU MOC3 render positions"),
            size: output_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ARIA resident position key layout"),
            entries: &[(0, false), (1, false), (2, false), (3, false), (4, true)].map(
                |(binding, writable)| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage {
                            read_only: !writable,
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ),
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ARIA resident position key bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: work.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: active_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: counts_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA resident position key compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_position_keys.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ARIA resident position key pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ARIA resident position key pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("blend_positions"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Self {
            plan,
            active,
            _source: source,
            _work: work,
            active_buffer,
            counts_buffer,
            output,
            bind,
            pipeline,
            blend: None,
        })
    }

    /// Attach resident blend-shape deltas to the same output buffer. Call once
    /// after construction; empty models need no second dispatch.
    pub fn with_blends(mut self, device: &wgpu::Device, plan: GpuBlendDeltaPlan) -> Result<Self> {
        if plan.work.is_empty() {
            return Ok(self);
        }
        ensure!(
            plan.work
                .iter()
                .all(|item| (item.output_index as usize) < self.plan.work.len()),
            "GPU blend delta target exceeds the position buffer"
        );
        let active = plan.empty_active_frame();
        let limits = device.limits();
        for size in [
            std::mem::size_of_val(plan.source_points.as_slice()) as u64,
            std::mem::size_of_val(plan.work.as_slice()) as u64,
            std::mem::size_of_val(active.keys.as_slice()) as u64,
            std::mem::size_of_val(active.counts.as_slice()) as u64,
        ] {
            ensure!(
                size > 0 && size <= limits.max_storage_buffer_binding_size,
                "GPU blend plan exceeds a storage binding"
            );
        }
        ensure!(
            plan.work.len().div_ceil(64) <= limits.max_compute_workgroups_per_dimension as usize,
            "GPU blend plan exceeds a dispatch dimension"
        );
        let source = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA resident MOC3 blend deltas"),
            contents: bytemuck::cast_slice(&plan.source_points),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let work = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA MOC3 blend delta work"),
            contents: bytemuck::cast_slice(&plan.work),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let active_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA dynamic MOC3 blend keys"),
            size: std::mem::size_of_val(active.keys.as_slice()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let counts_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA dynamic MOC3 blend counts"),
            size: std::mem::size_of_val(active.counts.as_slice()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ARIA resident blend delta layout"),
            entries: &[(0, false), (1, false), (2, false), (3, false), (4, true)].map(
                |(binding, writable)| wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage {
                            read_only: !writable,
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ),
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ARIA resident blend delta bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: source.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: work.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: active_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: counts_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.output.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA resident blend delta compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_blend_deltas_resident.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ARIA resident blend delta pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ARIA resident blend delta pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("apply_blend_deltas"),
            compilation_options: Default::default(),
            cache: None,
        });
        self.blend = Some(ResidentBlendStage {
            plan,
            active,
            _source: source,
            _work: work,
            active_buffer,
            counts_buffer,
            bind,
            pipeline,
        });
        Ok(self)
    }

    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        values: &[f32],
    ) -> Result<()> {
        self.plan.update_active(&mut self.active, values)?;
        queue.write_buffer(
            &self.active_buffer,
            0,
            bytemuck::cast_slice(&self.active.keys),
        );
        queue.write_buffer(
            &self.counts_buffer,
            0,
            bytemuck::cast_slice(&self.active.counts),
        );
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(self.plan.work.len().div_ceil(64) as u32, 1, 1);
        drop(pass);
        if let Some(blend) = &mut self.blend {
            blend.plan.update_active(&mut blend.active, values)?;
            queue.write_buffer(
                &blend.active_buffer,
                0,
                bytemuck::cast_slice(&blend.active.keys),
            );
            queue.write_buffer(
                &blend.counts_buffer,
                0,
                bytemuck::cast_slice(&blend.active.counts),
            );
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&blend.pipeline);
            pass.set_bind_group(0, &blend.bind, &[]);
            pass.dispatch_workgroups(blend.plan.work.len().div_ceil(64) as u32, 1, 1);
        }
        Ok(())
    }

    pub fn positions(&self) -> &wgpu::Buffer {
        &self.output
    }

    pub fn plan(&self) -> &GpuPositionKeyPlan {
        &self.plan
    }
}
