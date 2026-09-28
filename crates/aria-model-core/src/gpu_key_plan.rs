//! Resident normal-key positions and a fixed GPU work schedule.
//!
//! Mesh positions occupy the front of the output buffer so the renderer can
//! bind that buffer directly as its vertex source. Warp control points follow.

use crate::moc::{BindingGraph, DeformerKind, Moc, ParameterSpec};
use anyhow::{Context, Result, ensure};
use bytemuck::{Pod, Zeroable};
use std::ops::Range;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable, PartialEq)]
pub struct PositionWork {
    pub output_index: u32,
    pub local_index: u32,
    pub active_start: u32,
    pub node_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable, PartialEq)]
pub struct ActiveKey {
    pub source_offset: u32,
    pub weight: f32,
}

#[derive(Debug, Clone)]
struct KeyNode {
    binding: usize,
    key_offsets: Vec<u32>,
    active_start: usize,
    active_capacity: usize,
}

#[derive(Debug, Clone)]
pub struct ActiveFrame {
    pub keys: Vec<ActiveKey>,
    pub counts: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct GpuPositionKeyPlan {
    pub source_points: Vec<[f32; 2]>,
    pub work: Vec<PositionWork>,
    pub mesh_ranges: Vec<Range<u32>>,
    pub warp_ranges: Vec<Option<Range<u32>>>,
    pub mesh_vertex_count: u32,
    graph: BindingGraph,
    parameters: Vec<ParameterSpec>,
    nodes: Vec<KeyNode>,
    active_key_capacity: usize,
}

impl GpuPositionKeyPlan {
    pub fn new(moc: &Moc<'_>) -> Result<Self> {
        let graph = moc.binding_graph()?;
        let parameters = moc.parameters()?;
        let mut plan = Self {
            source_points: Vec::new(),
            work: Vec::new(),
            mesh_ranges: Vec::new(),
            warp_ranges: Vec::new(),
            mesh_vertex_count: 0,
            graph,
            parameters,
            nodes: Vec::new(),
            active_key_capacity: 0,
        };
        for index in 0..moc.mesh_layouts()?.len() {
            let mesh = moc.compile_mesh(index)?;
            let keys = mesh.keyform_points();
            let range = plan.push_node(mesh.binding, &keys)?;
            plan.mesh_ranges.push(range);
        }
        plan.mesh_vertex_count =
            u32::try_from(plan.work.len()).context("Mesh vertex count exceeds GPU index space")?;
        for deformer in moc.deformer_layouts()? {
            if matches!(&deformer.kind, DeformerKind::Warp { .. }) {
                let compiled = moc.compile_deformer(&deformer)?;
                let keys = compiled.warp_keyform_points()?;
                let range = plan.push_node(deformer.binding, &keys)?;
                plan.warp_ranges.push(Some(range));
            } else {
                plan.warp_ranges.push(None);
            }
        }
        Ok(plan)
    }

    fn push_node(&mut self, binding: usize, keys: &[&[[f32; 2]]]) -> Result<Range<u32>> {
        let point_count = keys.first().context("Geometry node has no keyforms")?.len();
        ensure!(
            keys.iter().all(|key| key.len() == point_count),
            "Geometry keyform size changed"
        );
        let axes = self
            .graph
            .bindings
            .get(binding)
            .context("Unknown geometry key binding")?;
        let mut capacity = 1_usize;
        for &table in axes {
            let key_count = self
                .graph
                .tables
                .get(table)
                .context("Unknown key table")?
                .keys
                .len();
            capacity = capacity
                .checked_mul(if key_count > 1 { 2 } else { 1 })
                .context("Active key capacity overflow")?;
            ensure!(
                capacity <= 65_536,
                "GPU active key capacity exceeds model limit"
            );
        }
        let active_start = self.active_key_capacity;
        self.active_key_capacity = self
            .active_key_capacity
            .checked_add(capacity)
            .context("Active key slot count overflow")?;
        let output_start =
            u32::try_from(self.work.len()).context("GPU geometry output exceeds index space")?;
        let output_end = self
            .work
            .len()
            .checked_add(point_count)
            .context("GPU geometry output count overflow")?;
        u32::try_from(output_end).context("GPU geometry output exceeds index space")?;
        let source_end = keys
            .iter()
            .try_fold(self.source_points.len(), |count, key| {
                count
                    .checked_add(key.len())
                    .context("GPU key data count overflow")
            })?;
        ensure!(
            source_end <= 512 * 1024 * 1024 / std::mem::size_of::<[f32; 2]>(),
            "GPU position keys exceed the 512 MiB RAM budget"
        );
        u32::try_from(source_end).context("GPU key data exceeds index space")?;
        let node_index =
            u32::try_from(self.nodes.len()).context("GPU node count exceeds index space")?;
        for local_index in 0..point_count {
            self.work.push(PositionWork {
                output_index: output_start + u32::try_from(local_index)?,
                local_index: u32::try_from(local_index)?,
                active_start: u32::try_from(active_start)?,
                node_index,
            });
        }
        let mut key_offsets = Vec::with_capacity(keys.len());
        for key in keys {
            key_offsets.push(
                u32::try_from(self.source_points.len())
                    .context("GPU key data exceeds index space")?,
            );
            self.source_points.extend_from_slice(key);
        }
        self.nodes.push(KeyNode {
            binding,
            key_offsets,
            active_start,
            active_capacity: capacity,
        });
        Ok(output_start..u32::try_from(self.work.len())?)
    }

    pub fn empty_active_frame(&self) -> ActiveFrame {
        ActiveFrame {
            keys: vec![ActiveKey::default(); self.active_key_capacity],
            counts: vec![0; self.nodes.len()],
        }
    }

    /// Reuse the same small frame buffers; the per-vertex work and key points
    /// are immutable and need no per-frame upload.
    pub fn update_active(&self, frame: &mut ActiveFrame, values: &[f32]) -> Result<()> {
        ensure!(
            values.len() == self.parameters.len(),
            "Parameter count differs from the model"
        );
        ensure!(
            frame.keys.len() == self.active_key_capacity && frame.counts.len() == self.nodes.len(),
            "GPU active frame has the wrong shape"
        );
        let resolved = self
            .parameters
            .iter()
            .zip(values)
            .map(|(parameter, value)| parameter.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        for (index, node) in self.nodes.iter().enumerate() {
            let weights = self.graph.weights(node.binding, &resolved)?;
            ensure!(
                weights.len() <= node.active_capacity,
                "GPU active key capacity was exceeded"
            );
            frame.counts[index] = u32::try_from(weights.len())?;
            for (slot, key) in weights.iter().enumerate() {
                frame.keys[node.active_start + slot] = ActiveKey {
                    source_offset: *node
                        .key_offsets
                        .get(key.index)
                        .context("GPU keyform index is missing")?,
                    weight: key.weight,
                };
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moc::LocalDeformerFrame;

    #[test]
    #[ignore = "requires ARIA_TEST_MOC; tests resident GPU packing on a private local model"]
    fn local_static_gpu_key_schedule_matches_decoded_frames() {
        let bytes = std::fs::read(std::env::var_os("ARIA_TEST_MOC").unwrap()).unwrap();
        let moc = Moc::parse(&bytes).unwrap();
        let plan = GpuPositionKeyPlan::new(&moc).unwrap();
        let work = plan.work.clone();
        let parameters = moc.parameters().unwrap();
        let graph = moc.binding_graph().unwrap();
        let mut active = plan.empty_active_frame();
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
            let resolved = parameters
                .iter()
                .zip(&values)
                .map(|(parameter, value)| parameter.resolve(*value).unwrap())
                .collect::<Vec<_>>();
            plan.update_active(&mut active, &values).unwrap();
            let mut expected = Vec::new();
            for index in 0..plan.mesh_ranges.len() {
                let mesh = moc.compile_mesh(index).unwrap();
                expected.extend(
                    mesh.frame(&graph.weights(mesh.binding, &resolved).unwrap())
                        .unwrap()
                        .positions,
                );
            }
            assert_eq!(expected.len(), plan.mesh_vertex_count as usize);
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
                expected.extend(points);
            }
            assert_eq!(expected.len(), plan.work.len());
            for (item, cpu) in plan.work.iter().zip(&expected) {
                let mut gpu_input = [0.0_f32; 2];
                let count = active.counts[item.node_index as usize] as usize;
                for key in
                    &active.keys[item.active_start as usize..item.active_start as usize + count]
                {
                    let point = plan.source_points[(key.source_offset + item.local_index) as usize];
                    gpu_input[0] += point[0] * key.weight;
                    gpu_input[1] += point[1] * key.weight;
                }
                let error = (gpu_input[0] - cpu[0])
                    .abs()
                    .max((gpu_input[1] - cpu[1]).abs());
                max_error = max_error.max(error);
                let tolerance = 0.00001_f32.max(cpu[0].abs().max(cpu[1].abs()) * 0.000001);
                assert!(
                    error <= tolerance,
                    "pose {pose}, output {}: {gpu_input:?} vs {cpu:?}",
                    item.output_index
                );
            }
            assert_eq!(work, plan.work, "Per-vertex GPU work changed with pose");
        }
        eprintln!(
            "GPU resident key plan: {} mesh vertices, {} warp points, {:.2} MiB source keys, {:.2} KiB dynamic keys, {:.2} KiB dynamic counts, max error {max_error}",
            plan.mesh_vertex_count,
            plan.work.len() - plan.mesh_vertex_count as usize,
            plan.source_points.len() as f64 * 8.0 / 1048576.0,
            active.keys.len() as f64 * 8.0 / 1024.0,
            active.counts.len() as f64 * 4.0 / 1024.0,
        );
    }
}
