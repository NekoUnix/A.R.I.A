//! Experimental GPU warp primitive. This verifies the math needed by a future
//! whole-frame GPU evaluator; the current renderer still consumes CPU meshes.

use aria_model_core::rig::{Point, RotationTransform, WarpGrid};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GridDescriptor {
    point_offset: u32,
    columns: u32,
    rows: u32,
    flags: u32,
    center: [f32; 4],
    basis_u: [f32; 4],
    basis_v: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuSample {
    position: [f32; 2],
    grid: u32,
    output_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RotationDescriptor {
    origin: [f32; 2],
    x_axis: [f32; 2],
    y_axis: [f32; 2],
}

struct GridInput<'a> {
    columns: usize,
    rows: usize,
    points: &'a [[f32; 2]],
    quad: bool,
}

fn basis(columns: usize, rows: usize, points: &[[f32; 2]]) -> (bool, [[f32; 4]; 3]) {
    let p00 = points[0];
    let p10 = points[columns];
    let p01 = points[rows * (columns + 1)];
    let p11 = points[rows * (columns + 1) + columns];
    let diagonal = [p11[0] - p00[0], p11[1] - p00[1]];
    let opposite = [p10[0] - p01[0], p10[1] - p01[1]];
    let u = [
        (diagonal[0] + opposite[0]) * 0.5,
        (diagonal[1] + opposite[1]) * 0.5,
    ];
    let v = [
        (diagonal[0] - opposite[0]) * 0.5,
        (diagonal[1] - opposite[1]) * 0.5,
    ];
    let center = [
        (p00[0] + p10[0] + p01[0] + p11[0]) * 0.25 - diagonal[0] * 0.5,
        (p00[1] + p10[1] + p01[1] + p11[1]) * 0.25 - diagonal[1] * 0.5,
    ];
    let affine = points.iter().enumerate().all(|(index, point)| {
        let x = (index % (columns + 1)) as f32 / columns as f32;
        let y = (index / (columns + 1)) as f32 / rows as f32;
        let expected = [
            center[0] + u[0] * x + v[0] * y,
            center[1] + u[1] * x + v[1] * y,
        ];
        (point[0] - expected[0]).abs() <= 0.000001 && (point[1] - expected[1]).abs() <= 0.000001
    });
    (
        affine,
        [
            [center[0], center[1], 0.0, 0.0],
            [u[0], u[1], 0.0, 0.0],
            [v[0], v[1], 0.0, 0.0],
        ],
    )
}

fn sample_on_gpu(
    state: &eframe::egui_wgpu::RenderState,
    columns: usize,
    rows: usize,
    points: &[[f32; 2]],
    inputs: &[[f32; 2]],
    quad: bool,
) -> Vec<[f32; 2]> {
    let samples = inputs
        .iter()
        .map(|&position| GpuSample {
            position,
            grid: 0,
            output_index: 0,
        })
        .collect::<Vec<_>>();
    sample_batch_on_gpu(
        state,
        &[GridInput {
            columns,
            rows,
            points,
            quad,
        }],
        &samples,
    )
}

fn sample_batch_on_gpu(
    state: &eframe::egui_wgpu::RenderState,
    grids: &[GridInput<'_>],
    samples: &[GpuSample],
) -> Vec<[f32; 2]> {
    let device = &state.device;
    let mut all_points = Vec::new();
    let mut descriptors = Vec::new();
    for grid in grids {
        let (affine, [center, basis_u, basis_v]) = basis(grid.columns, grid.rows, grid.points);
        descriptors.push(GridDescriptor {
            point_offset: all_points.len() as u32,
            columns: grid.columns as u32,
            rows: grid.rows as u32,
            flags: u32::from(grid.quad) | (u32::from(affine) << 1),
            center,
            basis_u,
            basis_v,
        });
        all_points.extend_from_slice(grid.points);
    }
    let points_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA warp control points"),
        contents: bytemuck::cast_slice(&all_points),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA warp inputs"),
        contents: bytemuck::cast_slice(samples),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let size = (samples.len() * std::mem::size_of::<[f32; 2]>()) as u64;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA warp outputs"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA warp readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let grid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA warp grid descriptors"),
        contents: bytemuck::cast_slice(&descriptors),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ARIA warp layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ARIA warp inputs"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: points_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: input_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: output_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: grid_buffer.as_entire_binding(),
            },
        ],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA warp compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_warp.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ARIA warp pipeline layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA warp pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups((samples.len() as u32).div_ceil(64), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback, 0, size);
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
    bytemuck::cast_slice::<u8, [f32; 2]>(&bytes).to_vec()
}

#[test]
#[cfg(windows)]
#[ignore = "requires a DX12 GPU; compares experimental warp compute with Rust geometry"]
fn affine_and_bent_warp_sampling_matches_rust_on_gpu() {
    let state = crate::spout::tests::gpu_state();
    let mut inputs = Vec::new();
    for x in [
        -3.0, -2.0, -1.75, -0.7, 0.0, 0.1, 0.25, 0.5, 0.8, 1.0, 1.5, 2.8, 3.1,
    ] {
        for y in [-3.0, -1.3, -0.2, 0.0, 0.1, 0.35, 0.5, 0.9, 1.0, 1.8, 3.1] {
            inputs.push([x, y]);
        }
    }
    for bent in [false, true] {
        let mut points = (0..=2)
            .flat_map(|row| {
                (0..=3).map(move |column| {
                    [
                        1.25 + column as f32 * 0.7 + row as f32 * 0.1,
                        -0.4 + row as f32 * 0.6 - column as f32 * 0.05,
                    ]
                })
            })
            .collect::<Vec<_>>();
        if bent {
            points[5][0] += 0.3;
            points[6][1] -= 0.18;
        }
        let grid = WarpGrid::new(
            3,
            2,
            points.iter().map(|p| Point { x: p[0], y: p[1] }).collect(),
        )
        .unwrap();
        for quad in [false, true] {
            let gpu = sample_on_gpu(&state, 3, 2, &points, &inputs, quad);
            for (index, (input, output)) in inputs.iter().zip(gpu).enumerate() {
                let cpu = grid
                    .sample_extended(
                        Point {
                            x: input[0],
                            y: input[1],
                        },
                        quad,
                    )
                    .unwrap();
                assert!(
                    (cpu.x - output[0]).abs() < 0.00001 && (cpu.y - output[1]).abs() < 0.00001,
                    "warp mismatch bent={bent} quad={quad} index={index}: {cpu:?} vs {output:?}"
                );
            }
        }
    }
}

#[test]
#[cfg(windows)]
#[ignore = "requires a DX12 GPU; batches different warp grids in one compute dispatch"]
fn mixed_warp_grids_share_one_gpu_dispatch() {
    let state = crate::spout::tests::gpu_state();
    let mut point_sets = Vec::new();
    let mut grids = Vec::new();
    for bent in [false, true] {
        let mut points = (0..=2)
            .flat_map(|row| {
                (0..=3).map(move |column| {
                    [
                        0.7 + column as f32 * 0.4 + row as f32 * 0.08,
                        -0.2 + row as f32 * 0.5 - column as f32 * 0.03,
                    ]
                })
            })
            .collect::<Vec<_>>();
        if bent {
            points[5][0] += 0.19;
        }
        for _quad in [false, true] {
            let grid = WarpGrid::new(
                3,
                2,
                points.iter().map(|p| Point { x: p[0], y: p[1] }).collect(),
            )
            .unwrap();
            point_sets.push(points.clone());
            grids.push(grid);
        }
    }
    let specs = point_sets
        .iter()
        .enumerate()
        .map(|(index, points)| GridInput {
            columns: 3,
            rows: 2,
            points,
            quad: index % 2 != 0,
        })
        .collect::<Vec<_>>();
    let samples = (0..240)
        .map(|index| GpuSample {
            position: [
                ((index * 17) % 131) as f32 / 47.0 - 0.9,
                ((index * 43) % 127) as f32 / 43.0 - 0.8,
            ],
            grid: (index % 4) as u32,
            output_index: 0,
        })
        .collect::<Vec<_>>();
    let outputs = sample_batch_on_gpu(&state, &specs, &samples);
    for (index, (sample, output)) in samples.iter().zip(outputs).enumerate() {
        let cpu = grids[sample.grid as usize]
            .sample_extended(
                Point {
                    x: sample.position[0],
                    y: sample.position[1],
                },
                sample.grid % 2 != 0,
            )
            .unwrap();
        assert!(
            (cpu.x - output[0]).abs() < 0.00001 && (cpu.y - output[1]).abs() < 0.00001,
            "batched warp mismatch at sample {index}: {cpu:?} vs {output:?}"
        );
    }
}

#[test]
#[cfg(windows)]
#[ignore = "requires a DX12 GPU; two hierarchy levels stay in GPU storage until final readback"]
fn child_and_grandchild_warps_resolve_on_gpu_without_intermediate_readback() {
    let state = crate::spout::tests::gpu_state();
    let device = &state.device;
    let root = (0..=2)
        .flat_map(|row| {
            (0..=2).map(move |column| {
                [
                    column as f32 * 0.55 + row as f32 * 0.04,
                    row as f32 * 0.6 - column as f32 * 0.07,
                ]
            })
        })
        .collect::<Vec<_>>();
    let child_local = (0..=2)
        .flat_map(|row| {
            (0..=2).map(move |column| {
                [
                    -0.1 + column as f32 * 0.55 + row as f32 * 0.03,
                    0.05 + row as f32 * 0.45 - column as f32 * 0.02,
                ]
            })
        })
        .collect::<Vec<_>>();
    let grandchild_local = (0..=2)
        .flat_map(|row| {
            (0..=2).map(move |column| {
                [
                    -0.15 + column as f32 * 0.65 + row as f32 * 0.02,
                    -0.2 + row as f32 * 0.7 - column as f32 * 0.04,
                ]
            })
        })
        .collect::<Vec<_>>();
    let root_grid = WarpGrid::new(
        2,
        2,
        root.iter().map(|p| Point { x: p[0], y: p[1] }).collect(),
    )
    .unwrap();
    let expected_child = child_local
        .iter()
        .map(|p| {
            root_grid
                .sample_extended(Point { x: p[0], y: p[1] }, true)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let child_grid = WarpGrid::new(2, 2, expected_child.clone()).unwrap();
    let expected_grandchild = grandchild_local
        .iter()
        .map(|p| {
            child_grid
                .sample_extended(Point { x: p[0], y: p[1] }, true)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mesh_base = [[0.2_f32, 0.3], [0.8, 0.25], [1.2, 0.7], [-0.3, 1.1]];
    let mesh_delta = [0.05_f32, -0.02];
    let mesh_local = mesh_base.map(|point| [point[0] + mesh_delta[0], point[1] + mesh_delta[1]]);
    let grandchild_grid = WarpGrid::new(2, 2, expected_grandchild.clone()).unwrap();
    let warped_mesh = mesh_local
        .iter()
        .map(|p| {
            grandchild_grid
                .sample_extended(Point { x: p[0], y: p[1] }, true)
                .unwrap()
        })
        .collect::<Vec<_>>();
    let rotation =
        RotationTransform::new(Point { x: 0.25, y: -0.1 }, 35.0, 1.2, [true, false]).unwrap();
    let expected_mesh = warped_mesh
        .iter()
        .map(|point| rotation.apply(*point))
        .collect::<Vec<_>>();
    let (root_affine, [center, basis_u, basis_v]) = basis(2, 2, &root);
    let descriptors = [
        GridDescriptor {
            point_offset: 0,
            columns: 2,
            rows: 2,
            flags: 1 | (u32::from(root_affine) << 1),
            center,
            basis_u,
            basis_v,
        },
        GridDescriptor {
            point_offset: 9,
            columns: 2,
            rows: 2,
            flags: 1 | 4, // Resolve affine boundary basis from GPU-produced corners.
            center: [0.0; 4],
            basis_u: [0.0; 4],
            basis_v: [0.0; 4],
        },
        GridDescriptor {
            point_offset: 18,
            columns: 2,
            rows: 2,
            flags: 1 | 4,
            center: [0.0; 4],
            basis_u: [0.0; 4],
            basis_v: [0.0; 4],
        },
    ];
    let mut all_points = root;
    all_points.extend(child_local.iter().copied());
    all_points.extend(grandchild_local.iter().copied());
    all_points.extend(mesh_base);
    let points_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA in-place hierarchy points"),
        contents: bytemuck::cast_slice(&vec![[0.0_f32; 2]; all_points.len()]),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let grid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy grids"),
        contents: bytemuck::cast_slice(&descriptors),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let child_samples = child_local
        .iter()
        .enumerate()
        .map(|(index, &position)| GpuSample {
            position,
            grid: 0,
            output_index: (9 + index) as u32,
        })
        .collect::<Vec<_>>();
    let grandchild_samples = grandchild_local
        .iter()
        .enumerate()
        .map(|(index, &position)| GpuSample {
            position,
            grid: 1,
            output_index: (18 + index) as u32,
        })
        .collect::<Vec<_>>();
    let child_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA child warp samples"),
        contents: bytemuck::cast_slice(&child_samples),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let grandchild_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA grandchild warp samples"),
        contents: bytemuck::cast_slice(&grandchild_samples),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let mesh_samples = mesh_local
        .iter()
        .enumerate()
        .map(|(index, &position)| GpuSample {
            position,
            grid: 2,
            output_index: (27 + index) as u32,
        })
        .collect::<Vec<_>>();
    let mesh_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA mesh warp samples"),
        contents: bytemuck::cast_slice(&mesh_samples),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let unused_output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA unused hierarchy output"),
        size: 8,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ARIA hierarchy layout"),
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
    let bind = |samples: &wgpu::Buffer| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ARIA hierarchy level"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: points_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: samples.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: unused_output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: grid_buffer.as_entire_binding(),
                },
            ],
        })
    };
    let child_bind = bind(&child_buffer);
    let grandchild_bind = bind(&grandchild_buffer);
    let mesh_bind = bind(&mesh_buffer);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA hierarchy warp compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_warp.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ARIA hierarchy pipeline layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA hierarchy warp pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("resolve_points"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut source_points = Vec::with_capacity(all_points.len() * 2);
    for delta in [0.125_f32, -0.125] {
        source_points.extend(all_points.iter().map(|p| [p[0] + delta, p[1] + delta]));
    }
    let work = (0..all_points.len())
        .map(|index| WarpKeyWork {
            output_index: index as u32,
            local_index: index as u32,
            active_start: 0,
            active_count: 2,
        })
        .collect::<Vec<_>>();
    let active = [
        WarpActiveKey {
            source_offset: 0,
            weight: 0.5,
        },
        WarpActiveKey {
            source_offset: all_points.len() as u32,
            weight: 0.5,
        },
    ];
    let source_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy blended source keys"),
        contents: bytemuck::cast_slice(&source_points),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let work_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy blend work"),
        contents: bytemuck::cast_slice(&work),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let active_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy active keys"),
        contents: bytemuck::cast_slice(&active),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let blend_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ARIA hierarchy blend layout"),
        entries: &[(0, false), (1, false), (2, false), (3, true)].map(|(binding, writable)| {
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
    let blend_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ARIA hierarchy blend bind"),
        layout: &blend_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: source_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: work_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: active_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: points_buffer.as_entire_binding(),
            },
        ],
    });
    let blend_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA hierarchy key blend compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_warp_keys.wgsl").into()),
    });
    let blend_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ARIA hierarchy blend pipeline layout"),
        bind_group_layouts: &[Some(&blend_layout)],
        immediate_size: 0,
    });
    let blend_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA hierarchy blend pipeline"),
        layout: Some(&blend_pipeline_layout),
        module: &blend_shader,
        entry_point: Some("blend_warp_keys"),
        compilation_options: Default::default(),
        cache: None,
    });
    let delta_points = [mesh_delta; 4];
    let delta_work = (0..4)
        .map(|index| WarpKeyWork {
            output_index: 27 + index,
            local_index: index,
            active_start: 0,
            active_count: 1,
        })
        .collect::<Vec<_>>();
    let delta_active = [WarpActiveKey {
        source_offset: 0,
        weight: 1.0,
    }];
    let delta_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy mesh blend deltas"),
        contents: bytemuck::cast_slice(&delta_points),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let delta_work_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy mesh delta work"),
        contents: bytemuck::cast_slice(&delta_work),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let delta_active_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy active mesh delta"),
        contents: bytemuck::cast_slice(&delta_active),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let delta_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ARIA hierarchy mesh blend bind"),
        layout: &blend_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: delta_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: delta_work_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: delta_active_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: points_buffer.as_entire_binding(),
            },
        ],
    });
    let delta_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA hierarchy mesh blend compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_blend_deltas.wgsl").into()),
    });
    let delta_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA hierarchy mesh blend pipeline"),
        layout: Some(&blend_pipeline_layout),
        module: &delta_shader,
        entry_point: Some("apply_blend_deltas"),
        compilation_options: Default::default(),
        cache: None,
    });
    let (origin, x_axis, y_axis) = rotation.affine_columns();
    let rotation_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy rotation"),
        contents: bytemuck::bytes_of(&RotationDescriptor {
            origin,
            x_axis,
            y_axis,
        }),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let rotation_samples = mesh_local
        .iter()
        .enumerate()
        .map(|(index, &position)| GpuSample {
            position,
            grid: 0,
            output_index: (27 + index) as u32,
        })
        .collect::<Vec<_>>();
    let rotation_samples_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA hierarchy rotation samples"),
        contents: bytemuck::cast_slice(&rotation_samples),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let rotation_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ARIA hierarchy rotation layout"),
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
    let rotation_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ARIA hierarchy rotation bind"),
        layout: &rotation_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: points_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: rotation_samples_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: rotation_buffer.as_entire_binding(),
            },
        ],
    });
    let rotation_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA hierarchy rotation compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_rotation.wgsl").into()),
    });
    let rotation_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ARIA hierarchy rotation pipeline layout"),
        bind_group_layouts: &[Some(&rotation_layout)],
        immediate_size: 0,
    });
    let rotation_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA hierarchy rotation pipeline"),
        layout: Some(&rotation_pipeline_layout),
        module: &rotation_shader,
        entry_point: Some("resolve_rotation"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bytes = std::mem::size_of_val(all_points.as_slice()) as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA hierarchy final readback"),
        size: bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&blend_pipeline);
        pass.set_bind_group(0, &blend_bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&delta_pipeline);
        pass.set_bind_group(0, &delta_bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    for bind in [&child_bind, &grandchild_bind, &mesh_bind] {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&rotation_pipeline);
        pass.set_bind_group(0, &rotation_bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&points_buffer, 0, &readback, 0, bytes);
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
    let output = bytemuck::cast_slice::<u8, [f32; 2]>(&bytes);
    for (index, (expected, actual)) in expected_child.iter().zip(&output[9..18]).enumerate() {
        assert!(
            (expected.x - actual[0]).abs() < 0.00001 && (expected.y - actual[1]).abs() < 0.00001,
            "child {index}: {expected:?} vs {actual:?}"
        );
    }
    for (index, (expected, actual)) in expected_grandchild.iter().zip(&output[18..27]).enumerate() {
        assert!(
            (expected.x - actual[0]).abs() < 0.00001 && (expected.y - actual[1]).abs() < 0.00001,
            "grandchild {index}: {expected:?} vs {actual:?}"
        );
    }
    for (index, (expected, actual)) in expected_mesh.iter().zip(&output[27..]).enumerate() {
        assert!(
            (expected.x - actual[0]).abs() < 0.00001 && (expected.y - actual[1]).abs() < 0.00001,
            "mesh vertex {index}: {expected:?} vs {actual:?}"
        );
    }
}

#[test]
#[cfg(windows)]
#[ignore = "requires ARIA_TEST_MOC and DX12; checks authored local rotations on the GPU"]
fn local_moc3_rotation_frames_match_rust_on_gpu() {
    use aria_model_core::moc::{DeformerKind, LocalDeformerFrame, Moc};

    let bytes = std::fs::read(std::env::var_os("ARIA_TEST_MOC").unwrap()).unwrap();
    let moc = Moc::parse(&bytes).unwrap();
    let graph = moc.binding_graph().unwrap();
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
    let mut rotations = Vec::<RotationDescriptor>::new();
    let mut points = Vec::<[f32; 2]>::new();
    let mut samples = Vec::<GpuSample>::new();
    let mut expected = Vec::<[f32; 2]>::new();
    for node in moc.deformer_layouts().unwrap() {
        let DeformerKind::Rotation { base_angle } = node.kind else {
            continue;
        };
        let compiled = moc.compile_deformer(&node).unwrap();
        let LocalDeformerFrame::Rotation {
            origin,
            angle,
            scale,
            reflect,
            ..
        } = compiled
            .frame(&graph.weights(node.binding, &values).unwrap())
            .unwrap()
        else {
            unreachable!()
        };
        let transform = RotationTransform::new(
            Point {
                x: origin[0],
                y: origin[1],
            },
            base_angle + angle,
            scale,
            reflect,
        )
        .unwrap();
        let (origin, x_axis, y_axis) = transform.affine_columns();
        let rotation_index = rotations.len() as u32;
        rotations.push(RotationDescriptor {
            origin,
            x_axis,
            y_axis,
        });
        for point in [[0.0, 0.0], [0.17, -0.24], [0.6, 1.2], [-0.8, 0.4]] {
            let index = points.len() as u32;
            points.push(point);
            samples.push(GpuSample {
                position: point,
                grid: rotation_index,
                output_index: index,
            });
            let output = transform.apply(Point {
                x: point[0],
                y: point[1],
            });
            expected.push([output.x, output.y]);
        }
    }
    assert!(!rotations.is_empty(), "Model has no rotation deformers");
    let state = crate::spout::tests::gpu_state();
    let device = &state.device;
    let points_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA authored rotation points"),
        contents: bytemuck::cast_slice(&points),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let samples_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA authored rotation samples"),
        contents: bytemuck::cast_slice(&samples),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let rotation_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA authored rotations"),
        contents: bytemuck::cast_slice(&rotations),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ARIA authored rotation layout"),
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
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ARIA authored rotation bind"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: points_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: samples_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: rotation_buffer.as_entire_binding(),
            },
        ],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA authored rotation compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_rotation.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ARIA authored rotation pipeline layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA authored rotation pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("resolve_rotation"),
        compilation_options: Default::default(),
        cache: None,
    });
    let size = std::mem::size_of_val(points.as_slice()) as u64;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA authored rotation readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(samples.len().div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&points_buffer, 0, &readback, 0, size);
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
        assert!(error <= tolerance, "rotation {index}: {cpu:?} vs {gpu:?}");
    }
    eprintln!(
        "ARIA GPU rotation parity: {} authored frames, {} samples, max error {max_error}",
        rotations.len(),
        samples.len()
    );
}

#[test]
#[cfg(windows)]
#[ignore = "requires ARIA_TEST_MOC and DX12; samples private model grids on the GPU"]
fn local_moc3_warp_grids_match_rust_on_gpu() {
    use aria_model_core::moc::{DeformerKind, LocalDeformerFrame, Moc};

    let path = std::env::var_os("ARIA_TEST_MOC").unwrap();
    let bytes = std::fs::read(path).unwrap();
    let moc = Moc::parse(&bytes).unwrap();
    let graph = moc.binding_graph().unwrap();
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
    let state = crate::spout::tests::gpu_state();
    let inputs = (0..96)
        .map(|index| {
            [
                ((index * 17) % 101) as f32 / 50.0 - 0.5,
                ((index * 43) % 97) as f32 / 46.0 - 0.5,
            ]
        })
        .collect::<Vec<_>>();
    let mut checked = [0_usize; 2];
    for node in moc.deformer_layouts().unwrap() {
        let DeformerKind::Warp {
            columns,
            rows,
            quad,
            ..
        } = &node.kind
        else {
            continue;
        };
        let compiled = moc.compile_deformer(&node).unwrap();
        let LocalDeformerFrame::Warp { points, .. } = compiled
            .frame(&graph.weights(node.binding, &values).unwrap())
            .unwrap()
        else {
            unreachable!()
        };
        let (affine, _) = basis(*columns, *rows, &points);
        let kind = usize::from(affine);
        if checked[kind] >= 3 {
            continue;
        }
        let grid = WarpGrid::new(
            *columns,
            *rows,
            points.iter().map(|p| Point { x: p[0], y: p[1] }).collect(),
        )
        .unwrap();
        let gpu = sample_on_gpu(&state, *columns, *rows, &points, &inputs, *quad);
        for (index, (input, output)) in inputs.iter().zip(gpu).enumerate() {
            let cpu = grid
                .sample_extended(
                    Point {
                        x: input[0],
                        y: input[1],
                    },
                    *quad,
                )
                .unwrap();
            let tolerance = 0.0001_f32.max(cpu.x.abs().max(cpu.y.abs()) * 0.00001);
            assert!(
                (cpu.x - output[0]).abs() < tolerance && (cpu.y - output[1]).abs() < tolerance,
                "MOC3 warp {} mismatch at sample {index}: {cpu:?} vs {output:?}",
                node.id
            );
        }
        checked[kind] += 1;
        if checked.iter().all(|&count| count == 3) {
            break;
        }
    }
    assert!(
        checked.iter().sum::<usize>() >= 3,
        "No representative warp grids"
    );
    eprintln!(
        "MOC3 GPU warp parity: {} bent and {} affine grids × {} positions",
        checked[0],
        checked[1],
        inputs.len()
    );
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WarpKeyWork {
    output_index: u32,
    local_index: u32,
    active_start: u32,
    active_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WarpActiveKey {
    source_offset: u32,
    weight: f32,
}

fn append_active_deltas(
    delta_points: &mut Vec<[f32; 2]>,
    active_keys: &mut Vec<WarpActiveKey>,
    deltas: &[Vec<[f32; 2]>],
    source_start: usize,
    weights: &[aria_model_core::moc::KeyformWeight],
) {
    let point_count = deltas.first().map_or(0, Vec::len);
    assert!(deltas.iter().all(|delta| delta.len() == point_count));
    let base = delta_points.len();
    for delta in deltas {
        delta_points.extend_from_slice(delta);
    }
    for weight in weights {
        let index = weight.index - source_start;
        assert!(index < deltas.len());
        active_keys.push(WarpActiveKey {
            source_offset: (base + index * point_count) as u32,
            weight: weight.weight,
        });
    }
}

#[test]
#[cfg(windows)]
#[ignore = "requires ARIA_TEST_MOC and DX12; blends private model geometry keys on the GPU"]
fn local_moc3_geometry_keyforms_match_rust_on_gpu() {
    use aria_model_core::moc::{DeformerKind, LocalDeformerFrame, Moc};

    let bytes = std::fs::read(std::env::var_os("ARIA_TEST_MOC").unwrap()).unwrap();
    let moc = Moc::parse(&bytes).unwrap();
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
    let state = crate::spout::tests::gpu_state();
    let device = &state.device;
    let mut source_points = Vec::<[f32; 2]>::new();
    let mut work = Vec::<WarpKeyWork>::new();
    let mut active_keys = Vec::<WarpActiveKey>::new();
    let mut expected = Vec::<[f32; 2]>::new();
    let mut delta_points = Vec::<[f32; 2]>::new();
    let mut delta_work = Vec::<WarpKeyWork>::new();
    let mut delta_active = Vec::<WarpActiveKey>::new();
    let mut warp_count = 0;
    let mut blended_count = 0;
    for node in moc.deformer_layouts().unwrap() {
        if !matches!(node.kind, DeformerKind::Warp { .. }) {
            continue;
        }
        let compiled = moc.compile_deformer(&node).unwrap();
        let weights = graph.weights(node.binding, &values).unwrap();
        blended_count += usize::from(weights.len() > 1);
        let keyform_offsets = compiled
            .warp_keyform_points()
            .unwrap()
            .into_iter()
            .map(|points| {
                let offset = source_points.len() as u32;
                source_points.extend_from_slice(points);
                offset
            })
            .collect::<Vec<_>>();
        let mut frame = compiled.frame(&weights).unwrap();
        let delta_start = delta_active.len() as u32;
        for target in blend_graph
            .warps
            .iter()
            .filter(|target| target.target == node.local_index)
        {
            let blend = moc.compile_blend_warp(target, &blend_graph).unwrap();
            for source in &blend.bindings {
                append_active_deltas(
                    &mut delta_points,
                    &mut delta_active,
                    &source.position_deltas,
                    source.source_start,
                    &blend_graph.weights(source.binding, &values).unwrap(),
                );
            }
            blend.apply(&mut frame, &blend_graph, &values).unwrap();
        }
        let LocalDeformerFrame::Warp { points, .. } = frame else {
            unreachable!()
        };
        let active_start = active_keys.len() as u32;
        for key in &weights {
            active_keys.push(WarpActiveKey {
                source_offset: keyform_offsets[key.index],
                weight: key.weight,
            });
        }
        let output_start = expected.len() as u32;
        work.extend((0..points.len()).map(|local_index| WarpKeyWork {
            output_index: output_start + local_index as u32,
            local_index: local_index as u32,
            active_start,
            active_count: weights.len() as u32,
        }));
        if delta_active.len() > delta_start as usize {
            delta_work.extend((0..points.len()).map(|local_index| WarpKeyWork {
                output_index: output_start + local_index as u32,
                local_index: local_index as u32,
                active_start: delta_start,
                active_count: delta_active.len() as u32 - delta_start,
            }));
        }
        expected.extend(points);
        warp_count += 1;
    }
    assert!(warp_count > 0);
    let warp_point_count = expected.len();
    let mut mesh_count = 0;
    let mut blended_mesh_count = 0;
    for mesh_index in 0..moc.mesh_layouts().unwrap().len() {
        let compiled = moc.compile_mesh(mesh_index).unwrap();
        let weights = graph.weights(compiled.binding, &values).unwrap();
        blended_mesh_count += usize::from(weights.len() > 1);
        let keyform_offsets = compiled
            .keyform_points()
            .into_iter()
            .map(|points| {
                let offset = source_points.len() as u32;
                source_points.extend_from_slice(points);
                offset
            })
            .collect::<Vec<_>>();
        let mut frame = compiled.frame(&weights).unwrap();
        let delta_start = delta_active.len() as u32;
        for target in blend_graph
            .art_meshes
            .iter()
            .filter(|target| target.target == mesh_index)
        {
            let blend = moc.compile_blend_mesh(target, &blend_graph).unwrap();
            for source in &blend.bindings {
                append_active_deltas(
                    &mut delta_points,
                    &mut delta_active,
                    &source.deltas,
                    source.source_start,
                    &blend_graph.weights(source.binding, &values).unwrap(),
                );
            }
            blend.apply(&mut frame, &blend_graph, &values).unwrap();
        }
        let points = frame.positions;
        let active_start = active_keys.len() as u32;
        for key in &weights {
            active_keys.push(WarpActiveKey {
                source_offset: keyform_offsets[key.index],
                weight: key.weight,
            });
        }
        let output_start = expected.len() as u32;
        work.extend((0..points.len()).map(|local_index| WarpKeyWork {
            output_index: output_start + local_index as u32,
            local_index: local_index as u32,
            active_start,
            active_count: weights.len() as u32,
        }));
        if delta_active.len() > delta_start as usize {
            delta_work.extend((0..points.len()).map(|local_index| WarpKeyWork {
                output_index: output_start + local_index as u32,
                local_index: local_index as u32,
                active_start: delta_start,
                active_count: delta_active.len() as u32 - delta_start,
            }));
        }
        expected.extend(points);
        mesh_count += 1;
    }
    let limits = device.limits();
    let source_bytes = std::mem::size_of_val(source_points.as_slice()) as u64;
    let work_bytes = std::mem::size_of_val(work.as_slice()) as u64;
    let output_bytes = std::mem::size_of_val(expected.as_slice()) as u64;
    let delta_bytes = std::mem::size_of_val(delta_points.as_slice()) as u64;
    let delta_work_bytes = std::mem::size_of_val(delta_work.as_slice()) as u64;
    let delta_active_bytes = std::mem::size_of_val(delta_active.as_slice()) as u64;
    for size in [
        source_bytes,
        work_bytes,
        output_bytes,
        delta_bytes,
        delta_work_bytes,
        delta_active_bytes,
    ] {
        assert!(
            size <= limits.max_storage_buffer_binding_size,
            "geometry key blend exceeds GPU storage binding: {size} bytes"
        );
    }
    let source_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA resident warp keyforms"),
        contents: bytemuck::cast_slice(&source_points),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let work_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA warp key work"),
        contents: bytemuck::cast_slice(&work),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let active_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("ARIA active warp keys"),
        contents: bytemuck::cast_slice(&active_keys),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA GPU blended warp points"),
        size: output_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA warp blend diagnostic readback"),
        size: output_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("ARIA warp key blend layout"),
        entries: &[(0, false), (1, false), (2, false), (3, true)].map(|(binding, writable)| {
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
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("ARIA warp key blend bind"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: source_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: work_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: active_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("ARIA warp key blend compute"),
        source: wgpu::ShaderSource::Wgsl(include_str!("gpu_warp_keys.wgsl").into()),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ARIA warp key blend pipeline layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("ARIA warp key blend pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("blend_warp_keys"),
        compilation_options: Default::default(),
        cache: None,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(work.len().div_ceil(64) as u32, 1, 1);
    }
    if !delta_work.is_empty() {
        let delta_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA resident blend-shape deltas"),
            contents: bytemuck::cast_slice(&delta_points),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let delta_work_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA blend-shape point work"),
            contents: bytemuck::cast_slice(&delta_work),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let delta_active_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("ARIA active blend-shape keys"),
            contents: bytemuck::cast_slice(&delta_active),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let delta_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ARIA blend-shape deltas bind"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: delta_work_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: delta_active_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });
        let delta_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ARIA blend-shape delta compute"),
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu_blend_deltas.wgsl").into()),
        });
        let delta_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("ARIA blend-shape delta pipeline"),
            layout: Some(&pipeline_layout),
            module: &delta_shader,
            entry_point: Some("apply_blend_deltas"),
            compilation_options: Default::default(),
            cache: None,
        });
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&delta_pipeline);
        pass.set_bind_group(0, &delta_bind, &[]);
        pass.dispatch_workgroups(delta_work.len().div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output_buffer, 0, &readback, 0, output_bytes);
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
            "geometry point {index}: {cpu:?} vs {gpu:?}"
        );
    }
    eprintln!(
        "ARIA GPU key parity: {warp_count} warps ({blended_count} multi-key), {mesh_count} meshes ({blended_mesh_count} multi-key), {} warp points, {} mesh vertices, {:.2} MiB resident keys, {} delta points across {} affected vertices, max error {max_error}",
        warp_point_count,
        expected.len() - warp_point_count,
        source_bytes as f64 / 1048576.0,
        delta_points.len(),
        delta_work.len()
    );
}
