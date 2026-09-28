//! Native acceptance for the production resident-position GPU evaluator.

use aria_model_core::{
    gpu_key_plan::GpuPositionKeyPlan,
    moc::{DeformerKind, LocalDeformerFrame, Moc},
};

fn expected_positions(moc: &Moc<'_>, values: &[f32]) -> Vec<[f32; 2]> {
    let parameters = moc.parameters().unwrap();
    let resolved = parameters
        .iter()
        .zip(values)
        .map(|(parameter, value)| parameter.resolve(*value).unwrap())
        .collect::<Vec<_>>();
    let graph = moc.binding_graph().unwrap();
    let mut positions = Vec::new();
    for index in 0..moc.mesh_layouts().unwrap().len() {
        let mesh = moc.compile_mesh(index).unwrap();
        positions.extend(
            mesh.frame(&graph.weights(mesh.binding, &resolved).unwrap())
                .unwrap()
                .positions,
        );
    }
    for node in moc.deformer_layouts().unwrap() {
        if !matches!(&node.kind, DeformerKind::Warp { .. }) {
            continue;
        }
        let compiled = moc.compile_deformer(&node).unwrap();
        let LocalDeformerFrame::Warp { points, .. } = compiled
            .frame(&graph.weights(node.binding, &resolved).unwrap())
            .unwrap()
        else {
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
    let state = crate::spout::tests::gpu_state();
    let device = &state.device;
    let mut evaluator =
        crate::gpu_position_evaluator::GpuPositionEvaluator::new(device, plan).unwrap();
    let output_bytes = evaluator.positions().size();
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("ARIA MOC3 position parity readback"),
        size: output_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let parameters = moc.parameters().unwrap();
    let mut max_error = 0.0_f32;
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
        let expected = expected_positions(&moc, &values);
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
    }
    let plan = evaluator.plan();
    let active = plan.empty_active_frame();
    eprintln!(
        "ARIA resident GPU key parity: {} mesh vertices, {} warp points, {:.2} MiB static key positions, {:.2} MiB static work, {:.2} KiB per-frame keys/counts, max error {max_error}",
        plan.mesh_vertex_count,
        plan.work.len() - plan.mesh_vertex_count as usize,
        plan.source_points.len() as f64 * 8.0 / 1048576.0,
        std::mem::size_of_val(plan.work.as_slice()) as f64 / 1048576.0,
        (active.keys.len() * 8 + active.counts.len() * 4) as f64 / 1024.0,
    );
}
