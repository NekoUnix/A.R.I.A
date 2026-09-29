//! Optional local CPU-preparation timing for a user-supplied MOC3 file.

use aria_model_core::{
    gpu_blend_plan::GpuBlendDeltaPlan, gpu_glue_plan::GpuGluePlan,
    gpu_hierarchy_plan::GpuHierarchyPlan, gpu_key_plan::GpuPositionKeyPlan, moc::Moc,
};
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "requires ARIA_TEST_MOC; reports CPU preparation only"]
fn supplied_model_gpu_plan_preparation_times() {
    let path = std::env::var_os("ARIA_TEST_MOC").expect("ARIA_TEST_MOC");
    let bytes = std::fs::read(path).unwrap();
    let moc = Moc::parse(&bytes).unwrap();
    let positions = GpuPositionKeyPlan::new(&moc).unwrap();
    let blend = GpuBlendDeltaPlan::new(&moc, &positions).unwrap();
    let hierarchy = GpuHierarchyPlan::new(&moc, &positions).unwrap();
    let glue = GpuGluePlan::new(&moc, &positions).unwrap();
    let mut position_frame = positions.empty_active_frame();
    let mut blend_frame = blend.empty_active_frame();
    let mut locals = hierarchy.empty_rotation_locals();
    let mut intensities = vec![0.0; glue.glue_count()];
    let parts = glue.default_part_values();
    let parameters = moc.parameters().unwrap();
    let mut values = parameters.iter().map(|p| p.default).collect::<Vec<_>>();
    let mut sums = [0.0_f64; 4];
    for frame in 0..330 {
        for (index, (value, parameter)) in values.iter_mut().zip(&parameters).enumerate() {
            let phase = frame as f32 * 0.017 + index as f32 * 0.13;
            *value = parameter.minimum
                + (parameter.maximum - parameter.minimum) * (0.5 + 0.4 * phase.sin());
        }
        let start = Instant::now();
        positions
            .update_active(&mut position_frame, black_box(&values))
            .unwrap();
        let position_done = Instant::now();
        blend
            .update_active(&mut blend_frame, black_box(&values))
            .unwrap();
        let blend_done = Instant::now();
        hierarchy
            .update_rotation_locals(&mut locals, black_box(&values))
            .unwrap();
        let hierarchy_done = Instant::now();
        glue.update_intensities(&mut intensities, black_box(&values), &parts)
            .unwrap();
        let glue_done = Instant::now();
        if frame >= 30 {
            for (sum, duration) in sums.iter_mut().zip([
                position_done - start,
                blend_done - position_done,
                hierarchy_done - blend_done,
                glue_done - hierarchy_done,
            ]) {
                *sum += duration.as_secs_f64() * 1000.0;
            }
        }
    }
    eprintln!(
        "GPU plan CPU prep: key={:.3} ms blend={:.3} ms rotation={:.3} ms glue={:.3} ms; key nodes={} blend nodes={} hierarchy levels={} glue levels={}",
        sums[0] / 300.0,
        sums[1] / 300.0,
        sums[2] / 300.0,
        sums[3] / 300.0,
        position_frame.counts.len(),
        blend_frame.counts.len(),
        hierarchy.levels.len(),
        glue.levels.len(),
    );
}
