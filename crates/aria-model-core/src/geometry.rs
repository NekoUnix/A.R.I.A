//! Model-scoped geometry evaluation with normal and blend-shape parameters.
//!
//! Static binding and hierarchy metadata are decoded once. A frame computes
//! each deformer once before transforming its child ArtMeshes, then applies
//! glue constraints, including their blend-shape intensities.

use crate::moc::{
    BindingGraph, BlendGraph, ColorKeyforms, ColorPair, CompiledBlendMesh, CompiledBlendRotation,
    CompiledBlendScalar, CompiledBlendWarp, CompiledDeformer, CompiledMesh, DeformerKind,
    DeformerLayout, GlueLayout, LocalDeformerFrame, LocalMeshFrame, Moc, ParameterSpec, PartLayout,
};
use crate::rig::{Point, RotationTransform, WarpGrid};
use anyhow::{Context, Result, ensure};
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Default)]
struct StageTiming {
    frames: usize,
    axes: Duration,
    deformers: Duration,
    deformer_keyforms: Duration,
    deformer_resolve: Duration,
    meshes: Duration,
    glue: Duration,
}

fn record_stage_timing(
    axes: Duration,
    deformers: Duration,
    deformer_keyforms: Duration,
    deformer_resolve: Duration,
    meshes: Duration,
    glue: Duration,
) {
    static STAGES: OnceLock<Mutex<StageTiming>> = OnceLock::new();
    let mut timings = STAGES
        .get_or_init(|| Mutex::new(StageTiming::default()))
        .lock()
        .unwrap();
    timings.frames += 1;
    timings.axes += axes;
    timings.deformers += deformers;
    timings.deformer_keyforms += deformer_keyforms;
    timings.deformer_resolve += deformer_resolve;
    timings.meshes += meshes;
    timings.glue += glue;
    if timings.frames >= 120 {
        let ms = |duration: Duration| duration.as_secs_f64() * 1000.0 / timings.frames as f64;
        eprintln!(
            "ARIA_RUST_CORE_STAGES axes_ms={:.3} deformers_ms={:.3} keyforms_ms={:.3} resolve_ms={:.3} meshes_ms={:.3} glue_ms={:.3}",
            ms(timings.axes),
            ms(timings.deformers),
            ms(timings.deformer_keyforms),
            ms(timings.deformer_resolve),
            ms(timings.meshes),
            ms(timings.glue)
        );
        *timings = StageTiming::default();
    }
}

enum Transform {
    Warp(WarpGrid, bool),
    Rotation(RotationTransform),
}

impl Transform {
    fn apply(&self, point: Point) -> Result<Point> {
        match self {
            Self::Warp(grid, quad) => Ok(grid.sample_extended_finite(point, *quad)),
            Self::Rotation(rotation) => Ok(rotation.apply(point)),
        }
    }
}

struct DeformerState {
    transform: Transform,
    opacity: f32,
    scale: f32,
    color: ColorPair,
}

#[derive(Default)]
struct DeformerCache {
    values: Vec<f32>,
    states: Vec<Option<DeformerState>>,
}

pub struct GeometryEvaluator {
    parameters: Vec<ParameterSpec>,
    graph: BindingGraph,
    blend_graph: BlendGraph,
    deformers: Vec<DeformerLayout>,
    compiled_deformers: Vec<CompiledDeformer>,
    deformer_colors: Vec<ColorKeyforms>,
    parts: Vec<PartLayout>,
    mesh_parts: Vec<Option<usize>>,
    mesh_enabled: Vec<bool>,
    secondary: Vec<bool>,
    meshes: Vec<Option<CompiledMesh>>,
    mesh_colors: Vec<ColorKeyforms>,
    blend_meshes: Vec<Vec<CompiledBlendMesh>>,
    blend_warps: Vec<Vec<CompiledBlendWarp>>,
    blend_rotations: Vec<Vec<CompiledBlendRotation>>,
    blend_glues: Vec<Vec<CompiledBlendScalar>>,
    glues: Vec<GlueLayout>,
    deformer_dependencies: Vec<Vec<usize>>,
    deformer_cache: Mutex<DeformerCache>,
}

impl GeometryEvaluator {
    pub fn new(moc: &Moc<'_>) -> Result<Self> {
        let glues = moc.glue_layouts()?;
        let deformers = moc.deformer_layouts()?;
        let parts = moc.part_layouts()?;
        let graph = moc.binding_graph()?;
        let blend_graph = moc.blend_graph()?;
        let secondary = vec![false; moc.counts()?.art_meshes as usize];
        let mesh_parts = (0..secondary.len())
            .map(|index| moc.mesh_parent_part(index))
            .collect::<Result<Vec<_>>>()?;
        let mesh_enabled = (0..secondary.len())
            .map(|index| moc.mesh_enabled(index))
            .collect::<Result<Vec<_>>>()?;
        let mut decoded_bytes = 0_usize;
        let mut meshes = Vec::with_capacity(secondary.len());
        let mut mesh_colors = Vec::with_capacity(secondary.len());
        for (index, &unsupported) in secondary.iter().enumerate() {
            if unsupported {
                meshes.push(None);
                mesh_colors.push(moc.compile_mesh_colors(index)?);
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
            let colors = moc.compile_mesh_colors(index)?;
            decoded_bytes = decoded_bytes
                .checked_add(colors.decoded_bytes())
                .context("Decoded mesh color size overflow")?;
            ensure!(
                decoded_bytes <= 512 * 1024 * 1024,
                "Decoded model exceeds RAM budget"
            );
            mesh_colors.push(colors);
        }
        let mut blend_meshes = vec![Vec::new(); secondary.len()];
        for target in &blend_graph.art_meshes {
            if secondary[target.target] {
                continue;
            }
            decoded_bytes = decoded_bytes
                .checked_add(moc.blend_mesh_position_bytes(target, &blend_graph)?)
                .context("Decoded blend geometry size overflow")?;
            ensure!(
                decoded_bytes <= 512 * 1024 * 1024,
                "Decoded model geometry exceeds the 512 MiB RAM budget"
            );
            blend_meshes[target.target].push(moc.compile_blend_mesh(target, &blend_graph)?);
        }
        let mut blend_warps = vec![Vec::new(); moc.counts()?.warp_deformers as usize];
        for target in &blend_graph.warps {
            decoded_bytes = decoded_bytes
                .checked_add(moc.blend_warp_position_bytes(target, &blend_graph)?)
                .context("Decoded blend geometry size overflow")?;
            ensure!(
                decoded_bytes <= 512 * 1024 * 1024,
                "Decoded model geometry exceeds the 512 MiB RAM budget"
            );
            blend_warps[target.target].push(moc.compile_blend_warp(target, &blend_graph)?);
        }
        let mut blend_rotations = vec![Vec::new(); moc.counts()?.rotation_deformers as usize];
        for target in &blend_graph.rotations {
            blend_rotations[target.target].push(moc.compile_blend_rotation(target, &blend_graph)?);
        }
        let mut blend_glues = vec![Vec::new(); glues.len()];
        for target in &blend_graph.glues {
            blend_glues[target.target].push(moc.compile_blend_glue(target, &blend_graph)?);
        }
        let mut compiled_deformers = Vec::with_capacity(deformers.len());
        let mut deformer_colors = Vec::with_capacity(deformers.len());
        for layout in &deformers {
            decoded_bytes = decoded_bytes
                .checked_add(moc.deformer_keyform_position_bytes(layout)?)
                .context("Decoded warp geometry size overflow")?;
            ensure!(
                decoded_bytes <= 512 * 1024 * 1024,
                "Decoded model geometry exceeds the 512 MiB RAM budget"
            );
            compiled_deformers.push(moc.compile_deformer(layout)?);
            let colors = moc.compile_deformer_colors(layout)?;
            decoded_bytes = decoded_bytes
                .checked_add(colors.decoded_bytes())
                .context("Decoded deformer color size overflow")?;
            ensure!(
                decoded_bytes <= 512 * 1024 * 1024,
                "Decoded model exceeds RAM budget"
            );
            deformer_colors.push(colors);
        }
        let binding_count = graph.bindings.len();
        ensure!(
            parts.iter().all(|part| part.binding < binding_count)
                && deformers.iter().all(|node| node.binding < binding_count)
                && meshes
                    .iter()
                    .flatten()
                    .all(|mesh| mesh.binding < binding_count)
                && glues.iter().all(|glue| glue.binding < binding_count),
            "Model object references an invalid normal-parameter binding"
        );
        let parameters = moc.parameters()?;
        let mut deformer_dependencies = Vec::with_capacity(deformers.len());
        for node in &deformers {
            let mut dependencies = Vec::new();
            for &table in &graph.bindings[node.binding] {
                dependencies.push(
                    graph
                        .tables
                        .get(table)
                        .context("Deformer binding references a missing key table")?
                        .parameter,
                );
            }
            let blends = if matches!(node.kind, DeformerKind::Warp { .. }) {
                blend_warps
                    .get(node.local_index)
                    .context("Deformer references a missing warp")?
                    .iter()
                    .flat_map(|blend| blend.bindings.iter().map(|binding| binding.binding))
                    .collect::<Vec<_>>()
            } else {
                blend_rotations
                    .get(node.local_index)
                    .context("Deformer references a missing rotation")?
                    .iter()
                    .flat_map(|blend| blend.bindings.iter().map(|binding| binding.binding))
                    .collect::<Vec<_>>()
            };
            for binding in blends {
                let source = blend_graph
                    .bindings
                    .get(binding)
                    .context("Deformer blend references a missing binding")?;
                dependencies.push(
                    blend_graph
                        .tables
                        .get(source.table)
                        .context("Deformer blend references a missing key table")?
                        .parameter,
                );
                dependencies.extend(
                    source
                        .constraints
                        .iter()
                        .map(|constraint| constraint.parameter),
                );
            }
            dependencies.sort_unstable();
            dependencies.dedup();
            ensure!(
                dependencies
                    .iter()
                    .all(|&parameter| parameter < parameters.len()),
                "Deformer dependency references a missing parameter"
            );
            deformer_dependencies.push(dependencies);
        }
        Ok(Self {
            parameters,
            graph,
            blend_graph,
            deformers,
            compiled_deformers,
            deformer_colors,
            parts,
            mesh_parts,
            mesh_enabled,
            secondary,
            meshes,
            mesh_colors,
            blend_meshes,
            blend_warps,
            blend_rotations,
            blend_glues,
            glues,
            deformer_dependencies,
            deformer_cache: Mutex::new(DeformerCache::default()),
        })
    }

    pub fn supported_mesh_count(&self) -> usize {
        self.secondary
            .iter()
            .filter(|&&secondary| !secondary)
            .count()
    }

    /// Evaluate the renderer-facing scalar state without touching vertex or
    /// warp-point positions. GPU geometry can be paired with these frames and
    /// the regular render-order evaluator.
    pub fn metadata_frame_with_parts(
        &self,
        values: &[f32],
        part_values: &[f32],
    ) -> Result<Vec<Option<LocalMeshFrame>>> {
        ensure!(
            values.len() == self.parameters.len() && part_values.len() == self.parts.len(),
            "GPU metadata pose has the wrong shape"
        );
        let values = self
            .parameters
            .iter()
            .zip(values)
            .map(|(spec, value)| spec.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        let binding_active = (0..self.graph.bindings.len())
            .map(|binding| self.graph.in_range(binding, &values))
            .collect::<Result<Vec<_>>>()?;
        let weights = (0..self.graph.bindings.len())
            .map(|binding| self.graph.weights(binding, &values))
            .collect::<Result<Vec<_>>>()?;
        let mut part_active = Vec::with_capacity(self.parts.len());
        let mut part_opacity = Vec::with_capacity(self.parts.len());
        for (part, input) in self.parts.iter().zip(part_values) {
            ensure!(input.is_finite(), "Part opacity is not finite");
            let active = part.enabled
                && binding_active[part.binding]
                && part.parent.is_none_or(|parent| part_active[parent]);
            let inherited = part.parent.map_or(1.0, |parent| part_opacity[parent]);
            part_active.push(active);
            part_opacity.push(if active {
                input.clamp(0.0, 1.0) * inherited
            } else {
                0.0
            });
        }
        let mut states = Vec::<Option<(f32, ColorPair)>>::with_capacity(self.deformers.len());
        for (index, node) in self.deformers.iter().enumerate() {
            if !node.enabled
                || !binding_active[node.binding]
                || node.parent_part.is_some_and(|part| !part_active[part])
                || node
                    .parent_deformer
                    .is_some_and(|parent| states[parent].is_none())
            {
                states.push(None);
                continue;
            }
            let parent = node.parent_deformer.and_then(|parent| states[parent]);
            let mut opacity =
                self.compiled_deformers[index].metadata_opacity(&weights[node.binding])?;
            if matches!(node.kind, DeformerKind::Warp { .. }) {
                for blend in &self.blend_warps[node.local_index] {
                    for source in &blend.bindings {
                        for key in self.blend_graph.weights(source.binding, &values)? {
                            let key_index = key
                                .index
                                .checked_sub(source.source_start)
                                .context("Warp blend key precedes its source range")?;
                            opacity += source
                                .opacity_deltas
                                .get(key_index)
                                .context("Warp opacity blend key is missing")?
                                * key.weight;
                        }
                    }
                    opacity = opacity.clamp(0.0, 1.0);
                }
            } else {
                for blend in &self.blend_rotations[node.local_index] {
                    for source in &blend.bindings {
                        for key in self.blend_graph.weights(source.binding, &values)? {
                            let key_index = key
                                .index
                                .checked_sub(source.source_start)
                                .context("Rotation blend key precedes its source range")?;
                            opacity += source
                                .deltas
                                .get(key_index)
                                .context("Rotation opacity blend key is missing")?[4]
                                * key.weight;
                        }
                    }
                    opacity = opacity.clamp(0.0, 1.0);
                }
            }
            let color = self.deformer_colors[index]
                .frame(&weights[node.binding])?
                .under(parent.map_or(ColorPair::default(), |(_, color)| color));
            states.push(Some((
                opacity * parent.map_or(1.0, |(opacity, _)| opacity),
                color,
            )));
        }
        let mut frames = Vec::with_capacity(self.meshes.len());
        for index in 0..self.meshes.len() {
            if self.secondary[index]
                || !self.mesh_enabled[index]
                || !binding_active[self.meshes[index]
                    .as_ref()
                    .context("Supported mesh has no decoded keyforms")?
                    .binding]
                || self.mesh_parts[index].is_some_and(|part| !part_active[part])
            {
                frames.push(None);
                continue;
            }
            let mesh = self.meshes[index]
                .as_ref()
                .context("Supported mesh has no decoded keyforms")?;
            let mut frame = mesh.metadata_frame(&weights[mesh.binding])?;
            let mut color = self.mesh_colors[index].frame(&weights[mesh.binding])?;
            if let Some(parent) = frame.parent_deformer {
                let Some((opacity, parent_color)) = states[parent] else {
                    frames.push(None);
                    continue;
                };
                frame.opacity *= opacity;
                color = color.under(parent_color);
            }
            color = color.under(ColorPair::default());
            frame.multiply = [color.multiply[0], color.multiply[1], color.multiply[2], 1.0];
            frame.screen = [color.screen[0], color.screen[1], color.screen[2], 1.0];
            if let Some(part) = self.mesh_parts[index] {
                frame.opacity *= part_opacity[part];
            }
            frames.push(Some(frame));
        }
        Ok(frames)
    }

    /// Return one slot per ArtMesh. `None` means its deformer chain is disabled;
    /// callers must never render that inactive mesh.
    pub fn frame(&self, values: &[f32]) -> Result<Vec<Option<LocalMeshFrame>>> {
        let part_values = self
            .parts
            .iter()
            .map(|part| if part.visible { 1.0 } else { 0.0 })
            .collect::<Vec<_>>();
        self.frame_with_parts(values, &part_values)
    }

    /// Evaluate with caller-controlled part opacities in MOC3 part order.
    pub fn frame_with_parts(
        &self,
        values: &[f32],
        part_values: &[f32],
    ) -> Result<Vec<Option<LocalMeshFrame>>> {
        static PROFILE: OnceLock<bool> = OnceLock::new();
        let timing_start = PROFILE
            .get_or_init(|| std::env::var("ARIA_PERF_RUST_CORE").as_deref() == Ok("1"))
            .then(Instant::now);
        ensure!(
            values.len() == self.parameters.len(),
            "Parameter count differs from the model"
        );
        ensure!(
            part_values.len() == self.parts.len(),
            "Part count differs from the model"
        );
        let values = self
            .parameters
            .iter()
            .zip(values)
            .map(|(spec, value)| spec.resolve(*value))
            .collect::<Result<Vec<_>>>()?;
        let binding_active = (0..self.graph.bindings.len())
            .map(|binding| self.graph.in_range(binding, &values))
            .collect::<Result<Vec<_>>>()?;
        let mut part_active = Vec::with_capacity(self.parts.len());
        let mut part_opacity = Vec::with_capacity(self.parts.len());
        for (part, input) in self.parts.iter().zip(part_values) {
            ensure!(input.is_finite(), "Part opacity is not finite");
            let parent_active = part.parent.is_none_or(|parent| part_active[parent]);
            let parent_opacity = part.parent.map_or(1.0, |parent| part_opacity[parent]);
            let active = part.enabled && parent_active && binding_active[part.binding];
            part_active.push(active);
            part_opacity.push(if active {
                input.clamp(0.0, 1.0) * parent_opacity
            } else {
                0.0
            });
        }
        let weights = (0..self.graph.bindings.len())
            .map(|binding| self.graph.weights(binding, &values))
            .collect::<Result<Vec<_>>>()?;
        let axes_done = timing_start.map(|_| Instant::now());
        let mut keyform_time = Duration::ZERO;
        let mut resolve_time = Duration::ZERO;
        let mut cache = self.deformer_cache.lock().unwrap();
        let cache_ready =
            cache.values.len() == values.len() && cache.states.len() == self.deformers.len();
        let mut previous_states = std::mem::take(&mut cache.states);
        let mut states: Vec<Option<DeformerState>> = Vec::with_capacity(self.deformers.len());
        let mut changed = Vec::with_capacity(self.deformers.len());
        for (index, node) in self.deformers.iter().enumerate() {
            if !node.enabled
                || !binding_active[node.binding]
                || node.parent_part.is_some_and(|part| !part_active[part])
            {
                changed.push(previous_states.get(index).is_some_and(Option::is_some));
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
                changed.push(previous_states.get(index).is_some_and(Option::is_some));
                states.push(None);
                continue;
            }
            let parent_changed = node.parent_deformer.is_some_and(|parent| changed[parent]);
            let input_changed = !cache_ready
                || self.deformer_dependencies[index]
                    .iter()
                    .any(|&parameter| cache.values[parameter] != values[parameter]);
            if !parent_changed
                && !input_changed
                && let Some(previous) = previous_states.get_mut(index).and_then(Option::take)
            {
                changed.push(false);
                states.push(Some(previous));
                continue;
            }
            changed.push(true);
            let keyform_start = timing_start.map(|_| Instant::now());
            let mut local = self.compiled_deformers[index].frame(&weights[node.binding])?;
            let color = self.deformer_colors[index]
                .frame(&weights[node.binding])?
                .under(parent.map_or(ColorPair::default(), |parent| parent.color));
            if matches!(node.kind, DeformerKind::Warp { .. }) {
                for blend in &self.blend_warps[node.local_index] {
                    blend.apply(&mut local, &self.blend_graph, &values)?;
                }
            } else {
                for blend in &self.blend_rotations[node.local_index] {
                    blend.apply(&mut local, &self.blend_graph, &values)?;
                }
            }
            if let Some(start) = keyform_start {
                keyform_time += start.elapsed();
            }
            let resolve_start = timing_start.map(|_| Instant::now());
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
                    let grid = WarpGrid::new(*columns, *rows, points)?;
                    DeformerState {
                        transform: Transform::Warp(grid, *quad),
                        opacity: opacity * parent.map_or(1.0, |parent| parent.opacity),
                        scale: parent.map_or(1.0, |parent| parent.scale),
                        color,
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
                        color,
                    }
                }
                _ => anyhow::bail!("Deformer layout and frame types differ"),
            };
            states.push(Some(state));
            if let Some(start) = resolve_start {
                resolve_time += start.elapsed();
            }
        }

        let deformers_done = timing_start.map(|_| Instant::now());

        let mut frames = Vec::with_capacity(self.secondary.len());
        for (index, &secondary) in self.secondary.iter().enumerate() {
            if secondary
                || !self.mesh_enabled[index]
                || !binding_active[self.meshes[index]
                    .as_ref()
                    .context("Supported mesh has no decoded keyforms")?
                    .binding]
                || self.mesh_parts[index].is_some_and(|part| !part_active[part])
            {
                frames.push(None);
                continue;
            }
            let mesh = self.meshes[index]
                .as_ref()
                .context("Supported mesh has no decoded keyforms")?;
            let mut frame = mesh.frame(&weights[mesh.binding])?;
            let mut color = self.mesh_colors[index].frame(&weights[mesh.binding])?;
            for blend in &self.blend_meshes[index] {
                blend.apply(&mut frame, &self.blend_graph, &values)?;
            }
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
                color = color.under(state.color);
            }
            color = color.under(ColorPair::default());
            frame.multiply = [color.multiply[0], color.multiply[1], color.multiply[2], 1.0];
            frame.screen = [color.screen[0], color.screen[1], color.screen[2], 1.0];
            if let Some(part) = self.mesh_parts[index] {
                frame.opacity *= part_opacity[part];
            }
            frames.push(Some(frame));
        }
        let meshes_done = timing_start.map(|_| Instant::now());
        for (glue_index, glue) in self.glues.iter().enumerate() {
            if self.secondary[glue.left_mesh] || self.secondary[glue.right_mesh] {
                continue;
            }
            let mut intensity = glue.intensity(&weights[glue.binding])?;
            for blend in &self.blend_glues[glue_index] {
                intensity = blend.apply(intensity, &self.blend_graph, &values)?;
            }
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
        ensure!(
            frames.iter().flatten().all(|frame| frame
                .positions
                .iter()
                .flatten()
                .all(|value| value.is_finite())),
            "Non-finite ArtMesh vertex"
        );
        cache.values = values;
        cache.states = states;
        if let (Some(start), Some(axes), Some(deformers), Some(meshes)) =
            (timing_start, axes_done, deformers_done, meshes_done)
        {
            record_stage_timing(
                axes - start,
                deformers - axes,
                keyform_time,
                resolve_time,
                meshes - deformers,
                meshes.elapsed(),
            );
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use crate::draw_order::RenderOrderEvaluator;
    use crate::moc::ParameterKind;

    #[test]
    #[ignore = "requires ARIA_TEST_MOC; compares metadata-only output with full geometry"]
    fn local_metadata_only_frames_match_full_geometry_across_poses() {
        let bytes = std::fs::read(std::env::var_os("ARIA_TEST_MOC").unwrap()).unwrap();
        let moc = Moc::parse(&bytes).unwrap();
        let evaluator = GeometryEvaluator::new(&moc).unwrap();
        let orderer = RenderOrderEvaluator::new(&moc).unwrap();
        let parameters = moc.parameters().unwrap();
        let parts = moc.part_layouts().unwrap();
        let mut compared = 0_usize;
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
            let part_values = parts
                .iter()
                .enumerate()
                .map(|(index, part)| {
                    if part.visible {
                        ((index * 7 + pose * 3) % 13) as f32 / 12.0
                    } else {
                        0.0
                    }
                })
                .collect::<Vec<_>>();
            let full = evaluator.frame_with_parts(&values, &part_values).unwrap();
            let metadata = evaluator
                .metadata_frame_with_parts(&values, &part_values)
                .unwrap();
            assert_eq!(
                orderer.frame(&values, &full).unwrap(),
                orderer.frame(&values, &metadata).unwrap(),
                "Render order changed on pose {pose}"
            );
            for (index, (full, metadata)) in full.iter().zip(&metadata).enumerate() {
                match (full, metadata) {
                    (None, None) => {}
                    (Some(full), Some(metadata)) => {
                        assert!(metadata.positions.is_empty());
                        assert_eq!(full.opacity, metadata.opacity, "mesh {index} opacity");
                        assert_eq!(full.draw_order, metadata.draw_order, "mesh {index} order");
                        assert_eq!(full.multiply, metadata.multiply, "mesh {index} multiply");
                        assert_eq!(full.screen, metadata.screen, "mesh {index} screen");
                        assert_eq!(full.parent_deformer, metadata.parent_deformer);
                        compared += 1;
                    }
                    _ => panic!("mesh {index} activation changed on pose {pose}"),
                }
            }
        }
        assert!(compared > 0);
        eprintln!("ARIA metadata-only parity: {compared} active mesh-poses");
    }

    #[test]
    #[ignore = "requires ARIA_TEST_MOC; reports local GPU eligibility without redistributing art"]
    fn local_unparented_mesh_gpu_eligibility() {
        let path = std::env::var_os("ARIA_TEST_MOC").unwrap();
        let bytes = std::fs::read(path).unwrap();
        let moc = Moc::parse(&bytes).unwrap();
        let layouts = moc.mesh_layouts().unwrap();
        let glued = moc
            .glue_layouts()
            .unwrap()
            .into_iter()
            .flat_map(|glue| [glue.left_mesh, glue.right_mesh])
            .collect::<std::collections::BTreeSet<_>>();
        let total_vertices: usize = layouts.iter().map(|mesh| mesh.uvs.len()).sum();
        let eligible = layouts
            .iter()
            .enumerate()
            .filter(|(index, mesh)| mesh.parent_deformer.is_none() && !glued.contains(index))
            .collect::<Vec<_>>();
        let eligible_vertices: usize = eligible.iter().map(|(_, mesh)| mesh.uvs.len()).sum();
        eprintln!(
            "GPU direct-mesh eligibility: {}/{} meshes, {}/{} vertices",
            eligible.len(),
            layouts.len(),
            eligible_vertices,
            total_vertices
        );
        let deformers = moc.deformer_layouts().unwrap();
        let mut depth = vec![0_usize; deformers.len()];
        let mut warp_points = 0_usize;
        let mut warp_keyform_bytes = 0_usize;
        let mut max_warp_keyform_bytes = 0_usize;
        for (index, node) in deformers.iter().enumerate() {
            depth[index] = node.parent_deformer.map_or(1, |parent| depth[parent] + 1);
            if let DeformerKind::Warp { points, .. } = node.kind {
                warp_points += points;
                let bytes = moc.deformer_keyform_position_bytes(node).unwrap();
                warp_keyform_bytes += bytes;
                max_warp_keyform_bytes = max_warp_keyform_bytes.max(bytes);
            }
        }
        eprintln!(
            "GPU hierarchy: {} deformers, max depth {}, {} warp control points, {:.2} MiB warp keyforms, {:.2} MiB largest warp",
            deformers.len(),
            depth.iter().copied().max().unwrap_or(0),
            warp_points,
            warp_keyform_bytes as f64 / 1048576.0,
            max_warp_keyform_bytes as f64 / 1048576.0
        );
    }

    #[test]
    #[ignore = "requires ARIA_TEST_MOC; surveys local glue dependencies for GPU scheduling"]
    fn local_glue_gpu_schedule_survey() {
        let bytes = std::fs::read(std::env::var_os("ARIA_TEST_MOC").unwrap()).unwrap();
        let moc = Moc::parse(&bytes).unwrap();
        let glues = moc.glue_layouts().unwrap();
        let mut all_vertices = std::collections::BTreeSet::new();
        let mut shared_across_glues = std::collections::BTreeSet::new();
        let mut internal_collisions = 0_usize;
        let mut total_pairs = 0_usize;
        let mut max_pairs = 0_usize;
        let mut last_level = std::collections::BTreeMap::new();
        let mut levels = Vec::<usize>::new();
        for glue in &glues {
            let mut local = std::collections::BTreeSet::new();
            total_pairs += glue.pairs.len();
            max_pairs = max_pairs.max(glue.pairs.len());
            for pair in &glue.pairs {
                for vertex in [(glue.left_mesh, pair.left), (glue.right_mesh, pair.right)] {
                    if !local.insert(vertex) {
                        internal_collisions += 1;
                    }
                    if !all_vertices.insert(vertex) {
                        shared_across_glues.insert(vertex);
                    }
                }
            }
            let level = local
                .iter()
                .filter_map(|vertex| last_level.get(vertex).map(|last: &usize| last + 1))
                .max()
                .unwrap_or(0);
            levels.push(level);
            for vertex in local {
                last_level.insert(vertex, level);
            }
        }
        eprintln!(
            "GPU glue schedule: {} glues, {total_pairs} pairs, max {max_pairs} pairs/glue, {internal_collisions} within-glue vertex overlaps, {} vertices reused across glues, {} ordered levels",
            glues.len(),
            shared_across_glues.len(),
            levels.iter().copied().max().map_or(0, |max| max + 1)
        );
    }

    #[test]
    #[ignore = "requires ARIA_TEST_MOC; private model remains on the local machine"]
    fn cached_deformers_match_a_fresh_evaluation_across_parameter_and_part_changes() {
        let path = std::env::var_os("ARIA_TEST_MOC").unwrap();
        let bytes = std::fs::read(path).unwrap();
        let moc = Moc::parse(&bytes).unwrap();
        let cached = GeometryEvaluator::new(&moc).unwrap();
        let reference = GeometryEvaluator::new(&moc).unwrap();
        let parameters = moc.parameters().unwrap();
        let parts = moc.part_layouts().unwrap();
        for frame in 0..20 {
            let values = parameters
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    let phase = ((frame * 17 + index * 13) % 23) as f32 / 22.0;
                    if index < 32 || parameter.kind == ParameterKind::BlendShape {
                        parameter.minimum + (parameter.maximum - parameter.minimum) * phase
                    } else {
                        parameter.default
                    }
                })
                .collect::<Vec<_>>();
            let part_values = parts
                .iter()
                .enumerate()
                .map(|(index, part)| {
                    if part.visible {
                        ((frame * 7 + index * 5) % 13) as f32 / 12.0
                    } else {
                        0.0
                    }
                })
                .collect::<Vec<_>>();
            let output = cached.frame_with_parts(&values, &part_values).unwrap();
            *reference.deformer_cache.lock().unwrap() = DeformerCache::default();
            let expected = reference.frame_with_parts(&values, &part_values).unwrap();
            assert_eq!(output, expected, "Cached geometry changed on frame {frame}");
        }
    }
}
