//! Model-scoped normal-parameter geometry evaluation.
//!
//! Static binding and hierarchy metadata are decoded once. A frame computes
//! each deformer once before transforming its child ArtMeshes, then applies
//! glue constraints. Blend-shape targets are represented as `None` until their
//! geometry is supported.

use crate::moc::{
    BindingGraph, CompiledMesh, DeformerKind, DeformerLayout, GlueLayout, LocalDeformerFrame,
    LocalMeshFrame, Moc, ParameterSpec,
};
use crate::rig::{Point, RotationTransform, WarpGrid};
use anyhow::{Context, Result, ensure};

enum Transform {
    Warp(WarpGrid, bool),
    Rotation(RotationTransform),
}

impl Transform {
    fn apply(&self, point: Point) -> Result<Point> {
        match self {
            Self::Warp(grid, quad) => grid.sample_extended(point, *quad),
            Self::Rotation(rotation) => Ok(rotation.apply(point)),
        }
    }
}

struct DeformerState {
    transform: Transform,
    opacity: f32,
    scale: f32,
}

pub struct GeometryEvaluator<'model, 'bytes> {
    moc: &'model Moc<'bytes>,
    parameters: Vec<ParameterSpec>,
    graph: BindingGraph,
    deformers: Vec<DeformerLayout>,
    secondary: Vec<bool>,
    meshes: Vec<Option<CompiledMesh>>,
    glues: Vec<GlueLayout>,
}

impl<'model, 'bytes> GeometryEvaluator<'model, 'bytes> {
    pub fn new(moc: &'model Moc<'bytes>) -> Result<Self> {
        let glues = moc.glue_layouts()?;
        let mut secondary = moc.blend_shape_meshes()?;
        for _ in 0..glues.len() {
            let mut changed = false;
            for glue in &glues {
                if secondary[glue.left_mesh] || secondary[glue.right_mesh] {
                    changed |= !secondary[glue.left_mesh] || !secondary[glue.right_mesh];
                    secondary[glue.left_mesh] = true;
                    secondary[glue.right_mesh] = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut decoded_bytes = 0_usize;
        let mut meshes = Vec::with_capacity(secondary.len());
        for (index, &unsupported) in secondary.iter().enumerate() {
            if unsupported {
                meshes.push(None);
                continue;
            }
            decoded_bytes = decoded_bytes
                .checked_add(moc.mesh_keyform_position_bytes(index)?)
                .context("Decoded geometry size overflow")?;
            ensure!(
                decoded_bytes <= 512 * 1024 * 1024,
                "Decoded mesh keyforms exceed the 512 MiB RAM budget"
            );
            let mesh = moc.compile_mesh(index)?;
            meshes.push(Some(mesh));
        }
        Ok(Self {
            moc,
            parameters: moc.parameters()?,
            graph: moc.binding_graph()?,
            deformers: moc.deformer_layouts()?,
            secondary,
            meshes,
            glues,
        })
    }

    pub fn supported_mesh_count(&self) -> usize {
        self.secondary
            .iter()
            .filter(|&&secondary| !secondary)
            .count()
    }

    /// Return one slot per ArtMesh. `None` means secondary geometry must be
    /// evaluated first; callers must never render that incomplete mesh.
    pub fn frame(&self, values: &[f32]) -> Result<Vec<Option<LocalMeshFrame>>> {
        ensure!(
            values.len() == self.parameters.len(),
            "Parameter count differs from the model"
        );
        let values = self
            .parameters
            .iter()
            .zip(values)
            .map(|(spec, value)| spec.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        let weights = (0..self.graph.bindings.len())
            .map(|binding| self.graph.weights(binding, &values))
            .collect::<Result<Vec<_>>>()?;

        let mut states: Vec<Option<DeformerState>> = Vec::with_capacity(self.deformers.len());
        for node in &self.deformers {
            if !node.enabled {
                states.push(None);
                continue;
            }
            let parent = match node.parent_deformer {
                Some(index) => states
                    .get(index)
                    .context("Deformer parent is not topologically ordered")?
                    .as_ref(),
                None => None,
            };
            if node.parent_deformer.is_some() && parent.is_none() {
                states.push(None);
                continue;
            }
            let local = self
                .moc
                .local_deformer_frame_weighted(node, &weights[node.binding])?;
            let state = match (&node.kind, local) {
                (
                    DeformerKind::Warp {
                        columns,
                        rows,
                        quad,
                        ..
                    },
                    LocalDeformerFrame::Warp { points, opacity },
                ) => {
                    let mut points = points
                        .into_iter()
                        .map(|point| Point {
                            x: point[0],
                            y: point[1],
                        })
                        .collect::<Vec<_>>();
                    if let Some(parent) = parent {
                        for point in &mut points {
                            *point = parent.transform.apply(*point)?;
                        }
                    }
                    DeformerState {
                        transform: Transform::Warp(WarpGrid::new(*columns, *rows, points)?, *quad),
                        opacity: opacity * parent.map_or(1.0, |parent| parent.opacity),
                        scale: parent.map_or(1.0, |parent| parent.scale),
                    }
                }
                (
                    DeformerKind::Rotation { base_angle },
                    LocalDeformerFrame::Rotation {
                        origin,
                        angle,
                        scale,
                        opacity,
                        reflect,
                    },
                ) => {
                    let mut origin = Point {
                        x: origin[0],
                        y: origin[1],
                    };
                    let mut angle = angle;
                    if let Some(parent) = parent {
                        let transformed_origin = parent.transform.apply(origin)?;
                        let mut step = if matches!(parent.transform, Transform::Rotation(_)) {
                            -10.0
                        } else {
                            -0.1
                        };
                        let mut direction = Point::default();
                        for _ in 0..16 {
                            let probe = parent.transform.apply(Point {
                                x: origin.x,
                                y: origin.y + step,
                            })?;
                            direction = Point {
                                x: probe.x - transformed_origin.x,
                                y: probe.y - transformed_origin.y,
                            };
                            if direction.x != 0.0 || direction.y != 0.0 {
                                break;
                            }
                            step *= 0.1;
                        }
                        ensure!(
                            direction.x != 0.0 || direction.y != 0.0,
                            "Rotation direction collapsed under parent"
                        );
                        angle += direction.x.atan2(-direction.y).to_degrees();
                        origin = transformed_origin;
                    }
                    let inherited_scale = scale * parent.map_or(1.0, |parent| parent.scale);
                    DeformerState {
                        transform: Transform::Rotation(RotationTransform::new(
                            origin,
                            base_angle + angle,
                            inherited_scale,
                            reflect,
                        )?),
                        opacity: opacity * parent.map_or(1.0, |parent| parent.opacity),
                        scale: inherited_scale,
                    }
                }
                _ => anyhow::bail!("Deformer layout and frame types differ"),
            };
            states.push(Some(state));
        }

        let mut frames = Vec::with_capacity(self.secondary.len());
        for (index, &secondary) in self.secondary.iter().enumerate() {
            if secondary {
                frames.push(None);
                continue;
            }
            let mesh = self.meshes[index]
                .as_ref()
                .context("Supported mesh has no decoded keyforms")?;
            let mut frame = mesh.frame(&weights[mesh.binding])?;
            if let Some(parent) = frame.parent_deformer {
                let Some(state) = states
                    .get(parent)
                    .context("ArtMesh parent deformer does not exist")?
                else {
                    frames.push(None);
                    continue;
                };
                for position in &mut frame.positions {
                    let result = state.transform.apply(Point {
                        x: position[0],
                        y: position[1],
                    })?;
                    *position = [result.x, result.y];
                }
                frame.opacity *= state.opacity;
            }
            frames.push(Some(frame));
        }
        for glue in &self.glues {
            if self.secondary[glue.left_mesh] || self.secondary[glue.right_mesh] {
                continue;
            }
            let intensity = glue.intensity(&weights[glue.binding])?;
            let (low, high) = if glue.left_mesh < glue.right_mesh {
                (glue.left_mesh, glue.right_mesh)
            } else {
                (glue.right_mesh, glue.left_mesh)
            };
            let (first, rest) = frames.split_at_mut(high);
            let (Some(low_frame), Some(high_frame)) = (&mut first[low], &mut rest[0]) else {
                continue;
            };
            let (left_frame, right_frame) = if glue.left_mesh < glue.right_mesh {
                (low_frame, high_frame)
            } else {
                (high_frame, low_frame)
            };
            for pair in &glue.pairs {
                let left = &mut left_frame.positions[pair.left];
                let right = &mut right_frame.positions[pair.right];
                let delta = [right[0] - left[0], right[1] - left[1]];
                left[0] += delta[0] * intensity * pair.left_weight;
                left[1] += delta[1] * intensity * pair.left_weight;
                right[0] -= delta[0] * intensity * pair.right_weight;
                right[1] -= delta[1] * intensity * pair.right_weight;
            }
        }
        Ok(frames)
    }
}
