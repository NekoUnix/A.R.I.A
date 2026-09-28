//! Conflict-free pair schedule for GPU-resident MOC3 glue constraints.

use crate::{
    gpu_key_plan::GpuPositionKeyPlan,
    moc::{
        BindingGraph, BlendGraph, CompiledBlendScalar, DeformerLayout, GlueLayout, MeshLayout, Moc,
        ParameterSpec, PartLayout,
    },
};
use anyhow::{Context, Result, ensure};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuGluePair {
    pub left: u32,
    pub right: u32,
    pub left_weight: f32,
    pub right_weight: f32,
    pub glue_index: u32,
}

pub struct GpuGluePlan {
    pub levels: Vec<Vec<GpuGluePair>>,
    pub pair_count: usize,
    graph: BindingGraph,
    blend_graph: BlendGraph,
    parameters: Vec<ParameterSpec>,
    parts: Vec<PartLayout>,
    deformers: Vec<DeformerLayout>,
    meshes: Vec<MeshLayout>,
    mesh_bindings: Vec<usize>,
    glues: Vec<GlueLayout>,
    blends: Vec<Vec<CompiledBlendScalar>>,
}

fn schedule_pair(
    levels: &mut Vec<Vec<GpuGluePair>>,
    last_level: &mut HashMap<u32, usize>,
    pair: GpuGluePair,
) {
    let level = [pair.left, pair.right]
        .into_iter()
        .filter_map(|vertex| last_level.get(&vertex).map(|previous| previous + 1))
        .max()
        .unwrap_or(0);
    if levels.len() <= level {
        levels.resize_with(level + 1, Vec::new);
    }
    levels[level].push(pair);
    last_level.insert(pair.left, level);
    last_level.insert(pair.right, level);
}

impl GpuGluePlan {
    pub fn new(moc: &Moc<'_>, positions: &GpuPositionKeyPlan) -> Result<Self> {
        let glues = moc.glue_layouts()?;
        let meshes = moc.mesh_layouts()?;
        ensure!(
            positions.mesh_ranges.len() == meshes.len(),
            "GPU glue and position topology differ"
        );
        let blend_graph = moc.blend_graph()?;
        let mut blends = vec![Vec::new(); glues.len()];
        for target in &blend_graph.glues {
            blends
                .get_mut(target.target)
                .context("GPU glue blend target is missing")?
                .push(moc.compile_blend_glue(target, &blend_graph)?);
        }
        let mut levels = Vec::<Vec<GpuGluePair>>::new();
        let mut last_level = HashMap::<u32, usize>::new();
        let mut pair_count = 0;
        for (glue_index, glue) in glues.iter().enumerate() {
            let left_range = positions
                .mesh_ranges
                .get(glue.left_mesh)
                .context("GPU glue left mesh is missing")?;
            let right_range = positions
                .mesh_ranges
                .get(glue.right_mesh)
                .context("GPU glue right mesh is missing")?;
            for pair in &glue.pairs {
                ensure!(
                    pair.left < left_range.len() && pair.right < right_range.len(),
                    "GPU glue pair references a missing vertex"
                );
                let left = left_range.start + u32::try_from(pair.left)?;
                let right = right_range.start + u32::try_from(pair.right)?;
                ensure!(left != right, "GPU glue pair references one vertex twice");
                schedule_pair(
                    &mut levels,
                    &mut last_level,
                    GpuGluePair {
                        left,
                        right,
                        left_weight: pair.left_weight,
                        right_weight: pair.right_weight,
                        glue_index: u32::try_from(glue_index)?,
                    },
                );
                pair_count += 1;
            }
        }
        let mesh_bindings = (0..meshes.len())
            .map(|index| moc.mesh_binding(index))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            levels,
            pair_count,
            graph: moc.binding_graph()?,
            blend_graph,
            parameters: moc.parameters()?,
            parts: moc.part_layouts()?,
            deformers: moc.deformer_layouts()?,
            meshes,
            mesh_bindings,
            glues,
            blends,
        })
    }

    pub fn glue_count(&self) -> usize {
        self.glues.len()
    }

    pub fn default_part_values(&self) -> Vec<f32> {
        self.parts
            .iter()
            .map(|part| f32::from(part.visible))
            .collect()
    }

    pub fn update_intensities(
        &self,
        intensities: &mut [f32],
        values: &[f32],
        part_values: &[f32],
    ) -> Result<()> {
        ensure!(
            intensities.len() == self.glues.len()
                && values.len() == self.parameters.len()
                && part_values.len() == self.parts.len(),
            "GPU glue frame has the wrong shape"
        );
        let resolved = self
            .parameters
            .iter()
            .zip(values)
            .map(|(parameter, value)| parameter.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        let binding_active = (0..self.graph.bindings.len())
            .map(|binding| self.graph.in_range(binding, &resolved))
            .collect::<Result<Vec<_>>>()?;
        let mut part_active = Vec::with_capacity(self.parts.len());
        for (part, input) in self.parts.iter().zip(part_values) {
            ensure!(input.is_finite(), "Part opacity is not finite");
            part_active.push(
                part.enabled
                    && binding_active[part.binding]
                    && part.parent.is_none_or(|parent| part_active[parent]),
            );
        }
        let mut deformer_active = Vec::with_capacity(self.deformers.len());
        for node in &self.deformers {
            deformer_active.push(
                node.enabled
                    && binding_active[node.binding]
                    && node.parent_part.is_none_or(|part| part_active[part])
                    && node
                        .parent_deformer
                        .is_none_or(|parent| deformer_active[parent]),
            );
        }
        let mesh_active = self
            .meshes
            .iter()
            .zip(&self.mesh_bindings)
            .map(|(mesh, &binding)| {
                mesh.enabled
                    && binding_active[binding]
                    && mesh.parent_part.is_none_or(|part| part_active[part])
                    && mesh
                        .parent_deformer
                        .is_none_or(|parent| deformer_active[parent])
            })
            .collect::<Vec<_>>();
        for (index, glue) in self.glues.iter().enumerate() {
            if !mesh_active[glue.left_mesh] || !mesh_active[glue.right_mesh] {
                intensities[index] = 0.0;
                continue;
            }
            let mut intensity = glue.intensity(&self.graph.weights(glue.binding, &resolved)?)?;
            for blend in &self.blends[index] {
                intensity = blend.apply(intensity, &self.blend_graph, &resolved)?;
            }
            intensities[index] = intensity;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_pairs_keep_source_order_while_disjoint_pairs_share_a_pass() {
        let sources = [(0, 1), (2, 3), (0, 2), (1, 3), (3, 4)];
        let mut levels = Vec::new();
        let mut last = HashMap::new();
        for (left, right) in sources {
            schedule_pair(
                &mut levels,
                &mut last,
                GpuGluePair {
                    left,
                    right,
                    left_weight: 0.4,
                    right_weight: 0.6,
                    glue_index: 0,
                },
            );
        }
        assert_eq!(levels.iter().map(Vec::len).collect::<Vec<_>>(), [2, 2, 1]);
        for level in &levels {
            let mut touched = std::collections::HashSet::new();
            for pair in level {
                assert!(touched.insert(pair.left));
                assert!(touched.insert(pair.right));
            }
        }
        let apply = |positions: &mut [[f32; 2]], pair: GpuGluePair| {
            let (left, right) = (pair.left as usize, pair.right as usize);
            let delta = [
                positions[right][0] - positions[left][0],
                positions[right][1] - positions[left][1],
            ];
            for axis in 0..2 {
                positions[left][axis] += delta[axis] * pair.left_weight;
                positions[right][axis] -= delta[axis] * pair.right_weight;
            }
        };
        let seed = [[0.0, 1.0], [2.0, 3.0], [4.0, 5.0], [6.0, 7.0], [8.0, 9.0]];
        let mut serial = seed;
        for (left, right) in sources {
            apply(
                &mut serial,
                GpuGluePair {
                    left,
                    right,
                    left_weight: 0.4,
                    right_weight: 0.6,
                    glue_index: 0,
                },
            );
        }
        let mut grouped = seed;
        for level in levels {
            for pair in level {
                apply(&mut grouped, pair);
            }
        }
        assert_eq!(grouped, serial);
    }
}
