//! Resident warp-chain evaluation over the GPU key/blend position buffer.

use anyhow::{Result, ensure};
use aria_model_core::gpu_warp_plan::{GpuWarpHierarchyPlan, WarpSample};
use wgpu::util::DeviceExt;

pub struct GpuWarpHierarchyStage {
    plan: GpuWarpHierarchyPlan,
    _grids: wgpu::Buffer,
    _samples: Vec<wgpu::Buffer>,
    _unused: wgpu::Buffer,
    binds: Vec<wgpu::BindGroup>,
    pipeline: wgpu::ComputePipeline,
}

impl GpuWarpHierarchyStage {
    pub fn new(
        device: &wgpu::Device,
        positions: &wgpu::Buffer,
        plan: GpuWarpHierarchyPlan,
    ) -> Result<Option<Self>> {
        if plan.warp_levels.is_empty() && plan.mesh_samples.is_empty() {
            return Ok(None);
        }
        let limits = device.limits();
        let grids_size = std::mem::size_of_val(plan.grids.as_slice()) as u64;
        ensure!(
            grids_size > 0 && grids_size <= limits.max_storage_buffer_binding_size,
            "GPU warp grids exceed a storage binding"
        );
        ensure!(
            plan.grids.iter().all(|grid| {
                let points = (grid.columns as u64 + 1) * (grid.rows as u64 + 1);
                (grid.point_offset as u64 + points) * 8 <= positions.size()
            }),
            "GPU warp grid references missing control points"
        );
        for samples in plan
            .warp_levels
            .iter()
            .chain(std::iter::once(&plan.mesh_samples))
            .filter(|samples| !samples.is_empty())
        {
            let size = std::mem::size_of_val(samples.as_slice()) as u64;
            ensure!(
                size <= limits.max_storage_buffer_binding_size
                    && samples.len().div_ceil(64)
                        <= limits.max_compute_workgroups_per_dimension as usize,
                "GPU warp work exceeds a storage or dispatch limit"
            );
            ensure!(
                samples.iter().all(|item| {
                    (item.output_index as u64) * 8 + 8 <= positions.size()
                        && (item.grid as usize) < plan.grids.len()
                }),
                "GPU warp work references a missing point or grid"
            );
        }
        let grids = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA resident warp grid metadata"),
            contents: bytemuck::cast_slice(&plan.grids),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let unused = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA warp unused output binding"),
            size: 8,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ARIA resident warp hierarchy layout"),
            entries: &[(0, true), (1, false), (2, true), (3, false)].map(|(binding, writable)| {
                wgpu::BindGroupLayoutEntry {
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
                }
            }),
        });
        let mut sample_buffers = Vec::new();
        let mut binds = Vec::new();
        for samples in plan
            .warp_levels
            .iter()
            .chain(std::iter::once(&plan.mesh_samples))
            .filter(|samples| !samples.is_empty())
        {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ARIA resident warp work level"),
                contents: bytemuck::cast_slice::<WarpSample, u8>(samples),
                usage: wgpu::BufferUsages::STORAGE,
            });
            binds.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ARIA resident warp hierarchy level"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: positions.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: unused.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: grids.as_entire_binding(),
                    },
                ],
            }));
            sample_buffers.push(buffer);
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA resident warp hierarchy compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_warp.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ARIA resident warp hierarchy pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ARIA resident warp hierarchy pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("resolve_points"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Some(Self {
            plan,
            _grids: grids,
            _samples: sample_buffers,
            _unused: unused,
            binds,
            pipeline,
        }))
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        for (index, bind) in self.binds.iter().enumerate() {
            let count = self
                .plan
                .warp_levels
                .get(index)
                .map_or(self.plan.mesh_samples.len(), Vec::len);
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.dispatch_workgroups(count.div_ceil(64) as u32, 1, 1);
        }
    }
}
