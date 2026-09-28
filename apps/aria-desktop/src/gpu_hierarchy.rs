//! Depth-ordered GPU warp and rotation hierarchy over resident positions.

use anyhow::{Result, ensure};
use aria_model_core::gpu_hierarchy_plan::{
    GpuHierarchyPlan, HierarchyTask, ROOT_NODE, RotationLocal,
};
use wgpu::util::DeviceExt;

pub struct GpuHierarchyStage {
    plan: GpuHierarchyPlan,
    locals: Vec<RotationLocal>,
    _nodes: wgpu::Buffer,
    _states: wgpu::Buffer,
    _tasks: Vec<wgpu::Buffer>,
    locals_buffer: wgpu::Buffer,
    binds: Vec<wgpu::BindGroup>,
    deformer_pipeline: wgpu::ComputePipeline,
    mesh_pipeline: wgpu::ComputePipeline,
}

impl GpuHierarchyStage {
    pub fn new(
        device: &wgpu::Device,
        positions: &wgpu::Buffer,
        plan: GpuHierarchyPlan,
    ) -> Result<Option<Self>> {
        if plan.nodes.is_empty() {
            return Ok(None);
        }
        let limits = device.limits();
        let locals = plan.empty_rotation_locals();
        let nodes_size = std::mem::size_of_val(plan.nodes.as_slice()) as u64;
        let locals_size = std::mem::size_of_val(locals.as_slice()) as u64;
        let states_size = plan.nodes.len() as u64 * 32;
        for size in [nodes_size, locals_size, states_size] {
            ensure!(
                size > 0 && size <= limits.max_storage_buffer_binding_size,
                "GPU hierarchy state exceeds a storage binding"
            );
        }
        for node in &plan.nodes {
            ensure!(
                node.parent == ROOT_NODE || (node.parent as usize) < plan.nodes.len(),
                "GPU hierarchy parent is missing"
            );
            if node.kind == 0 {
                let count = (node.columns as u64 + 1) * (node.rows as u64 + 1);
                ensure!(
                    (node.point_offset as u64 + count) * 8 <= positions.size(),
                    "GPU hierarchy grid references missing control points"
                );
            }
        }
        for tasks in plan.levels.iter().chain(std::iter::once(&plan.meshes)) {
            let size = std::mem::size_of_val(tasks.as_slice()) as u64;
            if tasks.is_empty() {
                continue;
            }
            ensure!(
                size <= limits.max_storage_buffer_binding_size
                    && tasks.len().div_ceil(64)
                        <= limits.max_compute_workgroups_per_dimension as usize,
                "GPU hierarchy work exceeds a storage or dispatch limit"
            );
            ensure!(
                tasks.iter().all(|task| {
                    (task.node_index as usize) < plan.nodes.len()
                        && (task.output_index == ROOT_NODE
                            || (task.output_index as u64 + 1) * 8 <= positions.size())
                }),
                "GPU hierarchy work references a missing node or point"
            );
        }
        let nodes = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA resident hierarchy nodes"),
            contents: bytemuck::cast_slice(&plan.nodes),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let states = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA resolved GPU deformer states"),
            size: states_size,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let locals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ARIA dynamic local rotation frames"),
            size: locals_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ARIA GPU hierarchy layout"),
            entries: &[(0, true), (1, false), (2, false), (3, false), (4, true)].map(
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
        let mut task_buffers = Vec::new();
        let mut binds = Vec::new();
        for tasks in plan.levels.iter().chain(std::iter::once(&plan.meshes)) {
            if tasks.is_empty() {
                continue;
            }
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ARIA hierarchy depth work"),
                contents: bytemuck::cast_slice::<HierarchyTask, u8>(tasks),
                usage: wgpu::BufferUsages::STORAGE,
            });
            binds.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ARIA hierarchy depth bind"),
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
                        resource: nodes.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: locals_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: states.as_entire_binding(),
                    },
                ],
            }));
            task_buffers.push(buffer);
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA GPU warp/rotation hierarchy compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_hierarchy.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ARIA GPU hierarchy pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |label, entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Ok(Some(Self {
            plan,
            locals,
            _nodes: nodes,
            _states: states,
            _tasks: task_buffers,
            locals_buffer,
            binds,
            deformer_pipeline: pipeline("ARIA resolve GPU deformers", "resolve_deformer"),
            mesh_pipeline: pipeline("ARIA resolve GPU meshes", "resolve_mesh"),
        }))
    }

    pub fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        values: &[f32],
    ) -> Result<()> {
        self.plan.update_rotation_locals(&mut self.locals, values)?;
        queue.write_buffer(&self.locals_buffer, 0, bytemuck::cast_slice(&self.locals));
        let mut bind_index = 0;
        for level in &self.plan.levels {
            if level.is_empty() {
                continue;
            }
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.deformer_pipeline);
            pass.set_bind_group(0, &self.binds[bind_index], &[]);
            pass.dispatch_workgroups(level.len().div_ceil(64) as u32, 1, 1);
            bind_index += 1;
        }
        if !self.plan.meshes.is_empty() {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&self.mesh_pipeline);
            pass.set_bind_group(0, &self.binds[bind_index], &[]);
            pass.dispatch_workgroups(self.plan.meshes.len().div_ceil(64) as u32, 1, 1);
        }
        Ok(())
    }
}
