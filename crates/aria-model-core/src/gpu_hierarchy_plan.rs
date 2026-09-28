//! Load-time schedule for the complete warp/rotation deformer hierarchy.
//!
//! Dense mesh and warp-point transforms run on GPU. The small authored local
//! rotation frames are resolved in Rust and uploaded once per pose.

use crate::{
    gpu_key_plan::GpuPositionKeyPlan,
    moc::{
        BindingGraph, BlendGraph, CompiledBlendRotation, CompiledDeformer, DeformerKind,
        LocalDeformerFrame, Moc, ParameterSpec,
    },
};
use anyhow::{Context, Result, ensure};
use bytemuck::{Pod, Zeroable};

pub const ROOT_NODE: u32 = u32::MAX;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct HierarchyNode {
    pub kind: u32, // 0 warp, 1 rotation
    pub parent: u32,
    pub point_offset: u32,
    pub columns: u32,
    pub rows: u32,
    pub flags: u32, // bit 0: quad warp
    pub base_angle: f32,
    pub _padding: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct HierarchyTask {
    pub node_index: u32,
    pub output_index: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct RotationLocal {
    pub origin: [f32; 2],
    pub angle: f32,
    pub scale: f32,
    pub reflect: u32,
    pub _padding: u32,
}

struct RotationSource {
    binding: usize,
    compiled: CompiledDeformer,
    blends: Vec<CompiledBlendRotation>,
}

pub struct GpuHierarchyPlan {
    pub nodes: Vec<HierarchyNode>,
    pub levels: Vec<Vec<HierarchyTask>>,
    pub meshes: Vec<HierarchyTask>,
    pub rotation_count: usize,
    pub warp_count: usize,
    graph: BindingGraph,
    blend_graph: BlendGraph,
    parameters: Vec<ParameterSpec>,
    rotations: Vec<Option<RotationSource>>,
}

impl GpuHierarchyPlan {
    pub fn new(moc: &Moc<'_>, positions: &GpuPositionKeyPlan) -> Result<Self> {
        let layouts = moc.deformer_layouts()?;
        let mesh_layouts = moc.mesh_layouts()?;
        ensure!(
            positions.warp_ranges.len() == layouts.len()
                && positions.mesh_ranges.len() == mesh_layouts.len(),
            "GPU hierarchy and position topology differ"
        );
        let graph = moc.binding_graph()?;
        let blend_graph = moc.blend_graph()?;
        let mut nodes = Vec::with_capacity(layouts.len());
        let mut levels = Vec::<Vec<HierarchyTask>>::new();
        let mut depth = Vec::<usize>::with_capacity(layouts.len());
        let mut rotations = Vec::with_capacity(layouts.len());
        let (mut rotation_count, mut warp_count) = (0, 0);
        for (index, layout) in layouts.iter().enumerate() {
            let parent = layout
                .parent_deformer
                .map_or(Ok(ROOT_NODE), u32::try_from)?;
            let level = layout.parent_deformer.map_or(0, |parent| depth[parent] + 1);
            if levels.len() <= level {
                levels.resize_with(level + 1, Vec::new);
            }
            depth.push(level);
            let node_index = u32::try_from(index).context("Too many GPU hierarchy nodes")?;
            let (node, source) = match layout.kind {
                DeformerKind::Warp {
                    columns,
                    rows,
                    quad,
                    ..
                } => {
                    warp_count += 1;
                    let range = positions.warp_ranges[index]
                        .as_ref()
                        .context("Warp has no GPU point range")?;
                    ensure!(
                        range.len() == (columns + 1) * (rows + 1),
                        "Warp range and grid dimensions differ"
                    );
                    if parent == ROOT_NODE {
                        levels[level].push(HierarchyTask {
                            node_index,
                            output_index: ROOT_NODE,
                        });
                    } else {
                        levels[level].extend(range.clone().map(|output_index| HierarchyTask {
                            node_index,
                            output_index,
                        }));
                    }
                    (
                        HierarchyNode {
                            kind: 0,
                            parent,
                            point_offset: range.start,
                            columns: u32::try_from(columns)?,
                            rows: u32::try_from(rows)?,
                            flags: u32::from(quad),
                            ..Default::default()
                        },
                        None,
                    )
                }
                DeformerKind::Rotation { base_angle } => {
                    rotation_count += 1;
                    levels[level].push(HierarchyTask {
                        node_index,
                        output_index: ROOT_NODE,
                    });
                    let mut blends = Vec::new();
                    for target in blend_graph
                        .rotations
                        .iter()
                        .filter(|target| target.target == layout.local_index)
                    {
                        blends.push(moc.compile_blend_rotation(target, &blend_graph)?);
                    }
                    (
                        HierarchyNode {
                            kind: 1,
                            parent,
                            base_angle,
                            ..Default::default()
                        },
                        Some(RotationSource {
                            binding: layout.binding,
                            compiled: moc.compile_deformer(layout)?,
                            blends,
                        }),
                    )
                }
            };
            nodes.push(node);
            rotations.push(source);
        }
        let mut meshes = Vec::new();
        for (index, layout) in mesh_layouts.iter().enumerate() {
            if let Some(parent) = layout.parent_deformer {
                let node_index = u32::try_from(parent)?;
                meshes.extend(positions.mesh_ranges[index].clone().map(|output_index| {
                    HierarchyTask {
                        node_index,
                        output_index,
                    }
                }));
            }
        }
        Ok(Self {
            nodes,
            levels,
            meshes,
            rotation_count,
            warp_count,
            graph,
            blend_graph,
            parameters: moc.parameters()?,
            rotations,
        })
    }

    pub fn empty_rotation_locals(&self) -> Vec<RotationLocal> {
        vec![RotationLocal::default(); self.nodes.len()]
    }

    pub fn update_rotation_locals(
        &self,
        locals: &mut [RotationLocal],
        values: &[f32],
    ) -> Result<()> {
        ensure!(
            locals.len() == self.nodes.len(),
            "GPU rotation local count differs"
        );
        ensure!(
            values.len() == self.parameters.len(),
            "Parameter count differs"
        );
        let resolved = self
            .parameters
            .iter()
            .zip(values)
            .map(|(parameter, value)| parameter.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        for (index, source) in self.rotations.iter().enumerate() {
            let Some(source) = source else { continue };
            let mut frame = source
                .compiled
                .frame(&self.graph.weights(source.binding, &resolved)?)?;
            for blend in &source.blends {
                blend.apply(&mut frame, &self.blend_graph, &resolved)?;
            }
            let LocalDeformerFrame::Rotation {
                origin,
                angle,
                scale,
                reflect,
                ..
            } = frame
            else {
                anyhow::bail!("GPU rotation source is not a rotation")
            };
            locals[index] = RotationLocal {
                origin,
                angle,
                scale,
                reflect: u32::from(reflect[0]) | (u32::from(reflect[1]) << 1),
                _padding: 0,
            };
        }
        Ok(())
    }
}
