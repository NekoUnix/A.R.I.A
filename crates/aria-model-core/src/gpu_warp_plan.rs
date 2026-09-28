//! Fixed GPU work schedule for warp-only MOC3 hierarchy branches.
//!
//! Rotation parents require a separate transform stage. This plan never
//! silently treats a mixed warp/rotation branch as a warp-only branch.

use crate::{
    gpu_key_plan::GpuPositionKeyPlan,
    moc::{DeformerKind, Moc},
};
use anyhow::{Context, Result, ensure};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WarpGridDescriptor {
    pub point_offset: u32,
    pub columns: u32,
    pub rows: u32,
    pub flags: u32,
    pub center: [f32; 4],
    pub basis_u: [f32; 4],
    pub basis_v: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct WarpSample {
    pub position: [f32; 2],
    pub grid: u32,
    pub output_index: u32,
}

#[derive(Debug, Clone)]
pub struct GpuWarpHierarchyPlan {
    pub grids: Vec<WarpGridDescriptor>,
    /// Each level reads only parent grids completed by earlier levels.
    pub warp_levels: Vec<Vec<WarpSample>>,
    pub mesh_samples: Vec<WarpSample>,
    pub supported_warps: usize,
    pub supported_meshes: usize,
}

impl GpuWarpHierarchyPlan {
    pub fn new(moc: &Moc<'_>, positions: &GpuPositionKeyPlan) -> Result<Self> {
        let deformers = moc.deformer_layouts()?;
        let meshes = moc.mesh_layouts()?;
        ensure!(
            positions.warp_ranges.len() == deformers.len()
                && positions.mesh_ranges.len() == meshes.len(),
            "GPU position plan and MOC3 topology differ"
        );
        let mut grids = Vec::new();
        let mut grid_index = vec![None; deformers.len()];
        let mut supported = vec![false; deformers.len()];
        let mut depth = vec![0_usize; deformers.len()];
        let mut warp_levels = Vec::<Vec<WarpSample>>::new();
        for (index, node) in deformers.iter().enumerate() {
            let DeformerKind::Warp {
                columns,
                rows,
                quad,
                ..
            } = node.kind
            else {
                continue;
            };
            let range = positions.warp_ranges[index]
                .as_ref()
                .context("Warp deformer has no GPU point range")?;
            ensure!(
                range.len() == (columns + 1) * (rows + 1),
                "Warp point range does not match grid dimensions"
            );
            let grid = u32::try_from(grids.len()).context("Too many GPU warp grids")?;
            grid_index[index] = Some(grid);
            grids.push(WarpGridDescriptor {
                point_offset: range.start,
                columns: u32::try_from(columns)?,
                rows: u32::try_from(rows)?,
                // Bit 2 resolves the exterior affine basis from resident GPU
                // control points after each parent level. Bit 0 is quad mode.
                flags: 4 | u32::from(quad),
                center: [0.0; 4],
                basis_u: [0.0; 4],
                basis_v: [0.0; 4],
            });
            let parent = node.parent_deformer;
            supported[index] = parent.is_none_or(|parent| supported[parent]);
            if !supported[index] {
                continue;
            }
            if let Some(parent) = parent {
                depth[index] = depth[parent] + 1;
                let parent_grid =
                    grid_index[parent].context("Supported warp parent has no grid")?;
                if warp_levels.len() <= depth[index] {
                    warp_levels.resize_with(depth[index] + 1, Vec::new);
                }
                warp_levels[depth[index]].extend(range.clone().map(|output_index| WarpSample {
                    position: [0.0; 2],
                    grid: parent_grid,
                    output_index,
                }));
            }
        }
        let mut mesh_samples = Vec::new();
        let mut supported_meshes = 0;
        for (index, mesh) in meshes.iter().enumerate() {
            let Some(parent) = mesh.parent_deformer else {
                continue;
            };
            if !supported[parent] {
                continue;
            }
            let grid = grid_index[parent].context("Supported mesh parent has no warp grid")?;
            supported_meshes += 1;
            mesh_samples.extend(positions.mesh_ranges[index].clone().map(|output_index| {
                WarpSample {
                    position: [0.0; 2],
                    grid,
                    output_index,
                }
            }));
        }
        Ok(Self {
            grids,
            warp_levels: warp_levels
                .into_iter()
                .filter(|level| !level.is_empty())
                .collect(),
            mesh_samples,
            supported_warps: supported.into_iter().filter(|value| *value).count(),
            supported_meshes,
        })
    }
}
