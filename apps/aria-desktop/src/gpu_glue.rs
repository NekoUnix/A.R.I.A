//! Test-only GPU glue schedule. Production still evaluates glue on the CPU.

use aria_model_core::{glue_schedule::gpu_glue_levels, moc::Moc};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GluePair {
    left: u32,
    right: u32,
    left_weight: f32,
    right_weight: f32,
    glue_index: u32,
}

#[test]
#[cfg(windows)]
#[ignore = "requires ARIA_TEST_MOC and DX12; checks glue pairs from a private local model"]
fn local_moc3_glue_matches_sequential_rust_on_gpu() {
    let bytes = std::fs::read(std::env::var_os("ARIA_TEST_MOC").unwrap()).unwrap();
    let moc = Moc::parse(&bytes).unwrap();
    let glues = moc.glue_layouts().unwrap();
    if glues.is_empty() {
        eprintln!("Model has no glue pairs");
        return;
    }
    let graph = moc.binding_graph().unwrap();
    let blend_graph = moc.blend_graph().unwrap();
    let values = moc
        .parameters()
        .unwrap()
        .into_iter()
        .map(|parameter| {
            if parameter.maximum > parameter.minimum {
                parameter.minimum + (parameter.maximum - parameter.minimum) * 0.37
            } else {
                parameter.default
            }
        })
        .collect::<Vec<_>>();
    let mut offsets = Vec::new();
    let mut count = 0;
    for layout in moc.mesh_layouts().unwrap() {
        offsets.push(count);
        count += layout.uvs.len();
    }
    let positions = (0..count)
        .map(|index| {
            [
                ((index * 17) % 997) as f32 / 411.0 - 1.0,
                ((index * 43) % 991) as f32 / 347.0 - 1.2,
            ]
        })
        .collect::<Vec<_>>();
    let mut expected = positions.clone();
    let mut intensities = Vec::with_capacity(glues.len());
    let mut pairs_by_glue = Vec::<Vec<GluePair>>::new();
    for (glue_index, glue) in glues.iter().enumerate() {
        let mut intensity = glue
            .intensity(&graph.weights(glue.binding, &values).unwrap())
            .unwrap();
        for target in blend_graph
            .glues
            .iter()
            .filter(|target| target.target == glue_index)
        {
            intensity = moc
                .compile_blend_glue(target, &blend_graph)
                .unwrap()
                .apply(intensity, &blend_graph, &values)
                .unwrap();
        }
        intensities.push(intensity);
        let mut gpu_pairs = Vec::with_capacity(glue.pairs.len());
        for pair in &glue.pairs {
            let left = offsets[glue.left_mesh] + pair.left;
            let right = offsets[glue.right_mesh] + pair.right;
            assert!(left < count && right < count && left != right);
            let delta = [
                expected[right][0] - expected[left][0],
                expected[right][1] - expected[left][1],
            ];
            expected[left][0] += delta[0] * intensity * pair.left_weight;
            expected[left][1] += delta[1] * intensity * pair.left_weight;
            expected[right][0] -= delta[0] * intensity * pair.right_weight;
            expected[right][1] -= delta[1] * intensity * pair.right_weight;
            gpu_pairs.push(GluePair {
                left: left as u32,
                right: right as u32,
                left_weight: pair.left_weight,
                right_weight: pair.right_weight,
                glue_index: glue_index as u32,
            });
        }
        pairs_by_glue.push(gpu_pairs);
    }
    let levels = gpu_glue_levels(&glues)
        .expect("This rig needs the sequential glue fallback")
        .into_iter()
        .map(|indices| {
            indices
                .into_iter()
                .flat_map(|index| pairs_by_glue[index].iter().copied())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let state = crate::spout::tests::gpu_state();
    let device = &state.device;
    let size = std::mem::size_of_val(positions.as_slice()) as u64;
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA GPU glue synthetic positions"),
        contents: bytemuck::cast_slice(&positions),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let intensity_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA GPU glue intensities"),
        contents: bytemuck::cast_slice(&intensities),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ARIA GPU glue layout"),
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
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA GPU glue compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_glue.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ARIA GPU glue pipeline layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA GPU glue pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("apply_glue"),
        compilation_options: Default::default(),
        cache: None,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA GPU glue readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    for pairs in &levels {
        let pair_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA GPU ordered glue level"),
            contents: bytemuck::cast_slice(pairs),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ARIA GPU ordered glue bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: pair_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: intensity_buffer.as_entire_binding(),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(pairs.len().div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&buffer, 0, &readback, 0, size);
    state.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let bytes = readback.slice(..).get_mapped_range().unwrap().to_vec();
    readback.unmap();
    let gpu = bytemuck::cast_slice::<u8, [f32; 2]>(&bytes);
    let mut max_error = 0.0_f32;
    for (index, (cpu, gpu)) in expected.iter().zip(gpu).enumerate() {
        let error = (cpu[0] - gpu[0]).abs().max((cpu[1] - gpu[1]).abs());
        max_error = max_error.max(error);
        let tolerance = 0.00001_f32.max(cpu[0].abs().max(cpu[1].abs()) * 0.000001);
        assert!(
            error <= tolerance,
            "glue vertex {index}: {cpu:?} vs {gpu:?}"
        );
    }
    eprintln!(
        "ARIA GPU glue parity: {} glues, {} pairs, {} ordered levels, {count} vertices, max error {max_error}",
        glues.len(),
        glues.iter().map(|glue| glue.pairs.len()).sum::<usize>(),
        levels.len()
    );
}
