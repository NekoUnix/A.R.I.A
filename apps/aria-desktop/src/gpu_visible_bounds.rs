//! Two-pass GPU reduction of visible mesh bounds without reading vertices back.

use anyhow::{Result, ensure};
use aria_model_core::gpu_key_plan::GpuPositionKeyPlan;
use wgpu::util::DeviceExt;

pub struct GpuVisibleBounds {
    mesh_count: usize,
    vertex_count: usize,
    group_count: u32,
    visible_scratch: Vec<u32>,
    _mesh_ids: wgpu::Buffer,
    visible: wgpu::Buffer,
    _groups: wgpu::Buffer,
    bounds: wgpu::Buffer,
    mesh_bind: wgpu::BindGroup,
    reduce_bind: wgpu::BindGroup,
    mesh_pipeline: wgpu::ComputePipeline,
    reduce_pipeline: wgpu::ComputePipeline,
}

impl GpuVisibleBounds {
    pub fn new(
        device: &wgpu::Device,
        positions: &wgpu::Buffer,
        plan: &GpuPositionKeyPlan,
    ) -> Result<Self> {
        let vertex_count = plan.mesh_vertex_count as usize;
        let mesh_count = plan.mesh_ranges.len();
        ensure!(
            vertex_count > 0 && mesh_count > 0,
            "GPU bounds plan has no meshes"
        );
        ensure!(
            positions.size() >= vertex_count as u64 * 8,
            "GPU bounds positions are short"
        );
        let mut mesh_ids = vec![u32::MAX; vertex_count];
        for (mesh, range) in plan.mesh_ranges.iter().enumerate() {
            ensure!(
                range.start <= range.end && range.end as usize <= vertex_count,
                "GPU bounds mesh range is invalid"
            );
            for id in &mut mesh_ids[range.start as usize..range.end as usize] {
                ensure!(*id == u32::MAX, "GPU bounds mesh ranges overlap");
                *id = u32::try_from(mesh)?;
            }
        }
        ensure!(
            mesh_ids.iter().all(|id| *id != u32::MAX),
            "GPU bounds mesh ranges leave gaps"
        );
        let group_count = u32::try_from(vertex_count.div_ceil(64))?;
        let limits = device.limits();
        ensure!(
            group_count <= limits.max_compute_workgroups_per_dimension,
            "GPU bounds dispatch exceeds device limit"
        );
        ensure!(
            vertex_count as u64 * 8 <= limits.max_storage_buffer_binding_size
                && vertex_count as u64 * 4 <= limits.max_storage_buffer_binding_size
                && mesh_count as u64 * 4 <= limits.max_storage_buffer_binding_size
                && u64::from(group_count) * 16 <= limits.max_storage_buffer_binding_size,
            "GPU bounds buffer exceeds device binding limit"
        );
        let ids = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA resident mesh vertex owners"),
            contents: bytemuck::cast_slice(&mesh_ids),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let visible = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA dynamic visible meshes"),
            size: mesh_count as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let groups = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA GPU partial visible bounds"),
            size: u64::from(group_count) * 16,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let bounds = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA GPU visible bounds"),
            size: 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let storage_entry = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let mesh_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ARIA GPU mesh bounds layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, false),
            ],
        });
        let reduce_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ARIA GPU bounds reduction layout"),
            entries: &[storage_entry(0, true), storage_entry(1, false)],
        });
        let mesh_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ARIA GPU mesh bounds bind"),
            layout: &mesh_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: positions,
                        offset: 0,
                        size: std::num::NonZeroU64::new(vertex_count as u64 * 8),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ids.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: visible.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: groups.as_entire_binding(),
                },
            ],
        });
        let reduce_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ARIA GPU bounds reduction bind"),
            layout: &reduce_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: groups.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bounds.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA GPU visible bounds shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_visible_bounds.wgsl").into()),
        });
        let reduce_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA GPU visible bounds reduction shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_visible_bounds_reduce.wgsl").into()),
        });
        let pipeline =
            |label, layout: &wgpu::BindGroupLayout, shader: &wgpu::ShaderModule, entry| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(label),
                        bind_group_layouts: &[Some(layout)],
                        immediate_size: 0,
                    });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(label),
                    layout: Some(&pipeline_layout),
                    module: shader,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            };
        let mesh_pipeline = pipeline(
            "ARIA GPU mesh bounds pipeline",
            &mesh_layout,
            &shader,
            "mesh_bounds",
        );
        let reduce_pipeline = pipeline(
            "ARIA GPU bounds reduction pipeline",
            &reduce_layout,
            &reduce_shader,
            "reduce_bounds",
        );
        Ok(Self {
            mesh_count,
            vertex_count,
            group_count,
            visible_scratch: vec![0; mesh_count],
            _mesh_ids: ids,
            visible,
            _groups: groups,
            bounds,
            mesh_bind,
            reduce_bind,
            mesh_pipeline,
            reduce_pipeline,
        })
    }

    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        visible: &[bool],
    ) -> Result<()> {
        ensure!(
            visible.len() == self.mesh_count,
            "GPU bounds visibility count changed"
        );
        for (flag, source) in self.visible_scratch.iter_mut().zip(visible) {
            *flag = u32::from(*source);
        }
        queue.write_buffer(
            &self.visible,
            0,
            bytemuck::cast_slice(&self.visible_scratch),
        );
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.mesh_pipeline);
        pass.set_bind_group(0, &self.mesh_bind, &[]);
        pass.dispatch_workgroups(self.group_count, 1, 1);
        drop(pass);
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&self.reduce_pipeline);
        pass.set_bind_group(0, &self.reduce_bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
        Ok(())
    }

    pub fn output(&self) -> &wgpu::Buffer {
        &self.bounds
    }
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }
}
