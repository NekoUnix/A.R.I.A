//! Native acceptance for the production resident-position GPU evaluator.

use aria_model_core::{
    geometry::GeometryEvaluator,
    gpu_blend_plan::GpuBlendDeltaPlan,
    gpu_key_plan::GpuPositionKeyPlan,
    gpu_warp_plan::{GpuWarpHierarchyPlan, WarpSample},
    moc::{DeformerKind, LocalDeformerFrame, Moc},
    rig::{Point, WarpGrid},
};
use std::collections::{HashMap, HashSet};

fn apply_warp_work(
    plan: &GpuWarpHierarchyPlan,
    samples: &[WarpSample],
    positions: &mut [[f32; 2]],
) {
    let mut grids = HashMap::<u32, WarpGrid>::new();
    for item in samples {
        let desc = plan.grids[item.grid as usize];
        let grid = grids.entry(item.grid).or_insert_with(|| {
            let count = (desc.columns as usize + 1) * (desc.rows as usize + 1);
            let start = desc.point_offset as usize;
            WarpGrid::new(
                desc.columns as usize,
                desc.rows as usize,
                positions[start..start + count]
                    .iter()
                    .map(|point| Point {
                        x: point[0],
                        y: point[1],
                    })
                    .collect(),
            )
            .unwrap()
        });
        let index = item.output_index as usize;
        let local = Point {
            x: positions[index][0],
            y: positions[index][1],
        };
        let output = grid.sample_extended(local, desc.flags & 1 != 0).unwrap();
        positions[index] = [output.x, output.y];
    }
}

fn apply_warp_hierarchy(plan: &GpuWarpHierarchyPlan, positions: &mut [[f32; 2]]) {
    for level in &plan.warp_levels {
        apply_warp_work(plan, level, positions);
    }
    apply_warp_work(plan, &plan.mesh_samples, positions);
}

fn expected_positions(moc: &Moc<'_>, values: &[f32], blended: bool) -> Vec<[f32; 2]> {
    let parameters = moc.parameters().unwrap();
    let resolved = parameters
        .iter()
        .zip(values)
        .map(|(parameter, value)| parameter.resolve(*value).unwrap())
        .collect::<Vec<_>>();
    let graph = moc.binding_graph().unwrap();
    let blend_graph = moc.blend_graph().unwrap();
    let mut positions = Vec::new();
    for index in 0..moc.mesh_layouts().unwrap().len() {
        let mesh = moc.compile_mesh(index).unwrap();
        let mut frame = mesh
            .frame(&graph.weights(mesh.binding, &resolved).unwrap())
            .unwrap();
        if blended {
            for target in blend_graph
                .art_meshes
                .iter()
                .filter(|target| target.target == index)
            {
                moc.compile_blend_mesh(target, &blend_graph)
                    .unwrap()
                    .apply(&mut frame, &blend_graph, &resolved)
                    .unwrap();
            }
        }
        positions.extend(frame.positions);
    }
    for node in moc.deformer_layouts().unwrap() {
        if !matches!(&node.kind, DeformerKind::Warp { .. }) {
            continue;
        }
        let compiled = moc.compile_deformer(&node).unwrap();
        let mut frame = compiled
            .frame(&graph.weights(node.binding, &resolved).unwrap())
            .unwrap();
        if blended {
            for target in blend_graph
                .warps
                .iter()
                .filter(|target| target.target == node.local_index)
            {
                moc.compile_blend_warp(target, &blend_graph)
                    .unwrap()
                    .apply(&mut frame, &blend_graph, &resolved)
                    .unwrap();
            }
        }
        let LocalDeformerFrame::Warp { points, .. } = frame else {
            unreachable!()
        };
        positions.extend(points);
    }
    positions
}

#[test]
#[cfg(windows)]
#[ignore = "requires ARIA_TEST_MOC and DX12; compares resident GPU keys with a private model"]
fn local_moc3_static_gpu_plan_matches_rust_across_pose_changes() {
    let bytes = std::fs::read(std::env::var_os("ARIA_TEST_MOC").unwrap()).unwrap();
    let moc = Moc::parse(&bytes).unwrap();
    let plan = GpuPositionKeyPlan::new(&moc).unwrap();
    let blend_plan = GpuBlendDeltaPlan::new(&moc, &plan).unwrap();
    let warp_plan = GpuWarpHierarchyPlan::new(&moc, &plan).unwrap();
    let warped_points =
        warp_plan.warp_levels.iter().map(Vec::len).sum::<usize>() + warp_plan.mesh_samples.len();
    let blend_work = blend_plan.work.len();
    let blend_bytes = blend_plan.source_points.len() * 8;
    let blend_active = blend_plan.empty_active_frame();
    let blend_dynamic_bytes = blend_active.keys.len() * 8 + blend_active.counts.len() * 4;
    let state = crate::spout::tests::gpu_state();
    let cpu = GeometryEvaluator::new(&moc).unwrap();
    let glued_meshes = moc
        .glue_layouts()
        .unwrap()
        .into_iter()
        .flat_map(|glue| [glue.left_mesh, glue.right_mesh])
        .collect::<HashSet<_>>();
    let warp_mesh_vertices = warp_plan
        .mesh_samples
        .iter()
        .map(|item| item.output_index)
        .collect::<HashSet<_>>();
    let device = &state.device;
    let mut evaluator = crate::gpu_position_evaluator::GpuPositionEvaluator::new(device, plan)
        .unwrap()
        .with_blends(device, blend_plan)
        .unwrap()
        .with_warp_hierarchy(device, warp_plan.clone())
        .unwrap();
    let output_bytes = evaluator.positions().size();
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA MOC3 position parity readback"),
        size: output_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let parameters = moc.parameters().unwrap();
    let mut max_error = 0.0_f32;
    let mut full_geometry_checked = 0_usize;
    for pose in 0..3 {
        let values = parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| match pose {
                0 => parameter.default,
                1 => parameter.minimum + (parameter.maximum - parameter.minimum) * 0.37,
                _ => {
                    let phase = ((index * 17 + 3) % 23) as f32 / 22.0;
                    parameter.minimum + (parameter.maximum - parameter.minimum) * phase
                }
            })
            .collect::<Vec<_>>();
        let mut encoder = device.create_command_encoder(&Default::default());
        evaluator
            .encode(&state.queue, &mut encoder, &values)
            .unwrap();
        encoder.copy_buffer_to_buffer(evaluator.positions(), 0, &readback, 0, output_bytes);
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
        let mut expected = expected_positions(&moc, &values, true);
        apply_warp_hierarchy(&warp_plan, &mut expected);
        assert_eq!(gpu.len(), expected.len());
        for (index, (cpu, gpu)) in expected.iter().zip(gpu).enumerate() {
            let error = (cpu[0] - gpu[0]).abs().max((cpu[1] - gpu[1]).abs());
            max_error = max_error.max(error);
            let tolerance = 0.00001_f32.max(cpu[0].abs().max(cpu[1].abs()) * 0.000001);
            assert!(
                error <= tolerance,
                "pose {pose}, vertex {index}: {cpu:?} vs {gpu:?}"
            );
        }
        let frames = cpu.frame(&values).unwrap();
        for (mesh, (range, frame)) in evaluator.plan().mesh_ranges.iter().zip(frames).enumerate() {
            if glued_meshes.contains(&mesh)
                || !warp_mesh_vertices.contains(&range.start)
                || frame.is_none()
            {
                continue;
            }
            let frame = frame.unwrap();
            for (cpu, gpu) in frame
                .positions
                .iter()
                .zip(&gpu[range.start as usize..range.end as usize])
            {
                let error = (cpu[0] - gpu[0]).abs().max((cpu[1] - gpu[1]).abs());
                let tolerance = 0.00001_f32.max(cpu[0].abs().max(cpu[1].abs()) * 0.000001);
                assert!(
                    error <= tolerance,
                    "full Rust geometry mesh {mesh}: {cpu:?} vs {gpu:?}"
                );
                full_geometry_checked += 1;
            }
        }
    }
    let plan = evaluator.plan();
    let active = plan.empty_active_frame();
    eprintln!(
        "ARIA resident GPU key/blend/warp parity: {} mesh vertices, {} warp points, {blend_work} blend points, {warped_points} hierarchy points over {} supported warps and {} meshes, {full_geometry_checked} full-Rust vertices, {:.2} MiB static key positions, {:.2} MiB static work, {:.2} MiB static deltas, {:.2} KiB per-frame normal keys/counts, {:.2} KiB per-frame blend keys/counts, max error {max_error}",
        plan.mesh_vertex_count,
        plan.work.len() - plan.mesh_vertex_count as usize,
        warp_plan.supported_warps,
        warp_plan.supported_meshes,
        plan.source_points.len() as f64 * 8.0 / 1048576.0,
        std::mem::size_of_val(plan.work.as_slice()) as f64 / 1048576.0,
        blend_bytes as f64 / 1048576.0,
        (active.keys.len() * 8 + active.counts.len() * 4) as f64 / 1024.0,
        blend_dynamic_bytes as f64 / 1024.0,
    );
}
