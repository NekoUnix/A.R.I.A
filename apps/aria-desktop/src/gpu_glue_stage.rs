//! Ordered GPU glue constraints over the resident final mesh positions.

use anyhow::{Result, ensure};
use aria_model_core::gpu_glue_plan::{GpuGluePair, GpuGluePlan};
use wgpu::util::DeviceExt;

pub struct GpuGlueStage {
    plan: GpuGluePlan,
    default_parts: Vec<f32>,
    intensities: Vec<f32>,
    _pairs: Vec<wgpu::Buffer>,
    intensity_buffer: wgpu::Buffer,
    binds: Vec<wgpu::BindGroup>,
    pipeline: wgpu::ComputePipeline,
}

impl GpuGlueStage {
    pub fn new(
        device: &wgpu::Device,
        positions: &wgpu::Buffer,
        plan: GpuGluePlan,
    ) -> Result<Option<Self>> {
        if plan.pair_count == 0 {
            return Ok(None);
        }
        let limits = device.limits();
        let intensity_size = plan.glue_count() as u64 * 4;
        ensure!(
            intensity_size > 0 && intensity_size <= limits.max_storage_buffer_binding_size,
            "GPU glue intensities exceed a storage binding"
        );
        for level in &plan.levels {
            let size = std::mem::size_of_val(level.as_slice()) as u64;
            ensure!(
                size > 0
                    && size <= limits.max_storage_buffer_binding_size
                    && level.len().div_ceil(64)
                        <= limits.max_compute_workgroups_per_dimension as usize,
                "GPU glue level exceeds a storage or dispatch limit"
            );
            ensure!(
                level.iter().all(|pair| {
                    (pair.left as u64 + 1) * 8 <= positions.size()
                        && (pair.right as u64 + 1) * 8 <= positions.size()
                        && (pair.glue_index as usize) < plan.glue_count()
                }),
                "GPU glue pair references missing geometry"
            );
        }
        let intensity_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA dynamic glue intensities"),
            size: intensity_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ARIA resident glue layout"),
            entries: &[(0, true), (1, false), (2, false)].map(|(binding, writable)| {
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
        let mut pair_buffers = Vec::new();
        let mut binds = Vec::new();
        for level in &plan.levels {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ARIA static glue pair level"),
                contents: bytemuck::cast_slice::<GpuGluePair, u8>(level),
                usage: wgpu::BufferUsages::STORAGE,
            });
            binds.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ARIA glue level bind"),
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
                        resource: intensity_buffer.as_entire_binding(),
                    },
                ],
            }));
            pair_buffers.push(buffer);
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA resident glue compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_glue.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ARIA resident glue pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ARIA resident glue pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("apply_glue"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Some(Self {
            default_parts: plan.default_part_values(),
            intensities: vec![0.0; plan.glue_count()],
            plan,
            _pairs: pair_buffers,
            intensity_buffer,
            binds,
            pipeline,
        }))
    }

    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        values: &[f32],
        part_values: Option<&[f32]>,
    ) -> Result<()> {
        self.plan.update_intensities(
            &mut self.intensities,
            values,
            part_values.unwrap_or(&self.default_parts),
        )?;
        queue.write_buffer(
            &self.intensity_buffer,
            0,
            bytemuck::cast_slice(&self.intensities),
        );
        for (level, bind) in self.plan.levels.iter().zip(&self.binds) {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.dispatch_workgroups(level.len().div_ceil(64) as u32, 1, 1);
        }
        Ok(())
    }
}
