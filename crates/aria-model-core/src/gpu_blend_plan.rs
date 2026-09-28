//! Load-time GPU schedule for MOC3 blend-shape position deltas.

use crate::{
    gpu_key_plan::{ActiveFrame, ActiveKey, GpuPositionKeyPlan, PositionWork},
    moc::{BlendGraph, DeformerKind, Moc, ParameterSpec},
};
use anyhow::{Context, Result, ensure};
use std::ops::Range;

#[derive(Debug, Clone)]
struct DeltaSource {
    binding: usize,
    source_start: usize,
    offsets: Vec<u32>,
}

#[derive(Debug, Clone)]
struct DeltaNode {
    sources: Vec<DeltaSource>,
    active_start: usize,
    active_capacity: usize,
}

#[derive(Debug, Clone)]
pub struct GpuBlendDeltaPlan {
    pub source_points: Vec<[f32; 2]>,
    pub work: Vec<PositionWork>,
    graph: BlendGraph,
    parameters: Vec<ParameterSpec>,
    nodes: Vec<DeltaNode>,
    active_key_capacity: usize,
}

impl GpuBlendDeltaPlan {
    pub fn new(moc: &Moc<'_>, positions: &GpuPositionKeyPlan) -> Result<Self> {
        let graph = moc.blend_graph()?;
        let mut plan = Self {
            source_points: Vec::new(),
            work: Vec::new(),
            graph,
            parameters: moc.parameters()?,
            nodes: Vec::new(),
            active_key_capacity: 0,
        };
        let mut mesh_sources = vec![Vec::new(); positions.mesh_ranges.len()];
        for target in plan.graph.art_meshes.clone() {
            let range = positions
                .mesh_ranges
                .get(target.target)
                .context("Blend ArtMesh target is missing")?;
            let compiled = moc.compile_blend_mesh(&target, &plan.graph)?;
            for source in compiled.bindings {
                mesh_sources[target.target].push(plan.push_source(
                    source.binding,
                    source.source_start,
                    &source.deltas,
                    range.len(),
                )?);
            }
        }
        let deformers = moc.deformer_layouts()?;
        let mut warp_sources = vec![Vec::new(); deformers.len()];
        for target in plan.graph.warps.clone() {
            let index = deformers
                .iter()
                .position(|node| {
                    matches!(node.kind, DeformerKind::Warp { .. })
                        && node.local_index == target.target
                })
                .context("Blend warp target is missing")?;
            let range = positions.warp_ranges[index]
                .as_ref()
                .context("Blend warp has no GPU point range")?;
            let compiled = moc.compile_blend_warp(&target, &plan.graph)?;
            for source in compiled.bindings {
                warp_sources[index].push(plan.push_source(
                    source.binding,
                    source.source_start,
                    &source.position_deltas,
                    range.len(),
                )?);
            }
        }
        for (range, sources) in positions.mesh_ranges.iter().zip(mesh_sources) {
            plan.push_node(range.clone(), sources)?;
        }
        for (range, sources) in positions.warp_ranges.iter().zip(warp_sources) {
            if let Some(range) = range {
                plan.push_node(range.clone(), sources)?;
            } else {
                ensure!(sources.is_empty(), "Rotation deformer has warp deltas");
            }
        }
        Ok(plan)
    }

    fn push_source(
        &mut self,
        binding: usize,
        source_start: usize,
        deltas: &[Vec<[f32; 2]>],
        point_count: usize,
    ) -> Result<DeltaSource> {
        ensure!(
            deltas.iter().all(|delta| delta.len() == point_count),
            "Blend-shape point count differs from its target"
        );
        let source_end = deltas
            .iter()
            .try_fold(self.source_points.len(), |count, delta| {
                count
                    .checked_add(delta.len())
                    .context("GPU blend delta count overflow")
            })?;
        ensure!(
            source_end <= 512 * 1024 * 1024 / std::mem::size_of::<[f32; 2]>(),
            "GPU blend deltas exceed the 512 MiB RAM budget"
        );
        u32::try_from(source_end).context("GPU blend deltas exceed index space")?;
        let mut offsets = Vec::with_capacity(deltas.len());
        for delta in deltas {
            offsets.push(u32::try_from(self.source_points.len())?);
            self.source_points.extend_from_slice(delta);
        }
        Ok(DeltaSource {
            binding,
            source_start,
            offsets,
        })
    }

    fn push_node(&mut self, range: Range<u32>, sources: Vec<DeltaSource>) -> Result<()> {
        if sources.is_empty() {
            return Ok(());
        }
        let active_start = self.active_key_capacity;
        let capacity = sources
            .len()
            .checked_mul(2)
            .context("GPU blend active slot count overflow")?;
        self.active_key_capacity = active_start
            .checked_add(capacity)
            .context("GPU blend active slot count overflow")?;
        let active_start_u32 = u32::try_from(active_start)?;
        let node_index = u32::try_from(self.nodes.len())?;
        for (local_index, output_index) in range.enumerate() {
            self.work.push(PositionWork {
                output_index,
                local_index: u32::try_from(local_index)?,
                active_start: active_start_u32,
                node_index,
            });
        }
        self.nodes.push(DeltaNode {
            sources,
            active_start,
            active_capacity: capacity,
        });
        Ok(())
    }

    pub fn empty_active_frame(&self) -> ActiveFrame {
        ActiveFrame {
            keys: vec![ActiveKey::default(); self.active_key_capacity],
            counts: vec![0; self.nodes.len()],
        }
    }

    pub fn update_active(&self, frame: &mut ActiveFrame, values: &[f32]) -> Result<()> {
        ensure!(
            values.len() == self.parameters.len(),
            "Parameter count differs from model"
        );
        ensure!(
            frame.keys.len() == self.active_key_capacity && frame.counts.len() == self.nodes.len(),
            "GPU blend frame has the wrong shape"
        );
        let resolved = self
            .parameters
            .iter()
            .zip(values)
            .map(|(parameter, value)| parameter.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        for (node_index, node) in self.nodes.iter().enumerate() {
            let mut count = 0;
            for source in &node.sources {
                for key in self.graph.weights(source.binding, &resolved)? {
                    ensure!(
                        count < node.active_capacity,
                        "GPU blend active slots exceeded"
                    );
                    let local_key = key
                        .index
                        .checked_sub(source.source_start)
                        .context("Blend key precedes its source range")?;
                    frame.keys[node.active_start + count] = ActiveKey {
                        source_offset: *source
                            .offsets
                            .get(local_key)
                            .context("Blend key is missing")?,
                        weight: key.weight,
                    };
                    count += 1;
                }
            }
            frame.counts[node_index] = u32::try_from(count)?;
        }
        Ok(())
    }
}
