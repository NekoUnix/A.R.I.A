//! Bounded reader for the binary container of an exported MOC3 model.
//! No mutation of the input or pointer relocation is needed.

use anyhow::{Context, Result, ensure};
use std::ops::Range;

const HEADER_BYTES: usize = 64;
const LEGACY_SECTIONS: usize = 160;
const EXTENDED_SECTIONS: usize = 480;
const MAX_MOC_BYTES: usize = 1280 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Little,
    Big,
}

#[derive(Debug, Clone)]
pub struct Moc<'a> {
    bytes: &'a [u8],
    version: u8,
    byte_order: ByteOrder,
    sections: Vec<Option<Range<usize>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelCounts {
    pub parts: u32,
    pub deformers: u32,
    pub warp_deformers: u32,
    pub rotation_deformers: u32,
    pub art_meshes: u32,
    pub parameters: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Canvas {
    pub pixels_per_unit: f32,
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub reverse_y: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshLayout {
    pub id: String,
    pub texture: u8,
    pub constant_flags: u8,
    pub parent_part: Option<usize>,
    pub parent_deformer: Option<usize>,
    pub enabled: bool,
    pub default_visible: bool,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<u16>,
    pub masks: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpec {
    pub id: String,
    pub minimum: f32,
    pub maximum: f32,
    pub default: f32,
    pub repeat: bool,
    pub kind: ParameterKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterKind {
    Normal,
    BlendShape,
}

impl ParameterSpec {
    /// Convert an external value to the value used by keyform evaluation.
    pub fn resolve(&self, value: f32) -> Result<f32> {
        ensure!(value.is_finite(), "Non-finite parameter value");
        if self.repeat {
            let span = self.maximum - self.minimum;
            ensure!(
                span > 0.0 && span.is_finite(),
                "Invalid repeating parameter range"
            );
            Ok((value - self.minimum).rem_euclid(span) + self.minimum)
        } else {
            Ok(value.clamp(self.minimum, self.maximum))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyTable {
    pub parameter: usize,
    pub keys: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindingGraph {
    pub tables: Vec<KeyTable>,
    pub bindings: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlendKeyTable {
    pub parameter: usize,
    pub keys: Vec<f32>,
    pub base_key: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlendConstraint {
    pub parameter: usize,
    pub keys: Vec<f32>,
    pub weights: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlendBinding {
    pub table: usize,
    pub source_start: usize,
    pub source_len: usize,
    pub constraints: Vec<BlendConstraint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlendTarget {
    pub target: usize,
    pub bindings: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlendGraph {
    pub tables: Vec<BlendKeyTable>,
    pub bindings: Vec<BlendBinding>,
    pub art_meshes: Vec<BlendTarget>,
    pub warps: Vec<BlendTarget>,
    pub rotations: Vec<BlendTarget>,
    pub glues: Vec<BlendTarget>,
}

#[derive(Debug, Clone)]
pub struct CompiledBlendRotationBinding {
    pub binding: usize,
    pub source_start: usize,
    /// Origin X/Y, angle, scale and opacity deltas in local deformer space.
    pub deltas: Vec<[f32; 5]>,
}

#[derive(Debug, Clone)]
pub struct CompiledBlendRotation {
    pub target: usize,
    pub bindings: Vec<CompiledBlendRotationBinding>,
}

#[derive(Debug, Clone)]
pub struct CompiledBlendScalarBinding {
    pub binding: usize,
    pub source_start: usize,
    pub deltas: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct CompiledBlendScalar {
    pub target: usize,
    pub bindings: Vec<CompiledBlendScalarBinding>,
}

impl CompiledBlendScalar {
    pub fn apply(&self, mut value: f32, graph: &BlendGraph, values: &[f32]) -> Result<f32> {
        for source in &self.bindings {
            for key in graph.weights(source.binding, values)? {
                let index = key
                    .index
                    .checked_sub(source.source_start)
                    .context("Scalar blend key precedes its source range")?;
                value += source
                    .deltas
                    .get(index)
                    .context("Scalar blend key is outside its source range")?
                    * key.weight;
            }
        }
        ensure!(value.is_finite(), "Non-finite scalar blend result");
        Ok(value.clamp(0.0, 1.0))
    }
}

impl CompiledBlendRotation {
    pub fn apply(
        &self,
        frame: &mut LocalDeformerFrame,
        graph: &BlendGraph,
        values: &[f32],
    ) -> Result<()> {
        let LocalDeformerFrame::Rotation {
            origin,
            angle,
            scale,
            opacity,
            ..
        } = frame
        else {
            anyhow::bail!("Blend rotation target is not a rotation deformer");
        };
        for source in &self.bindings {
            for key in graph.weights(source.binding, values)? {
                let index = key
                    .index
                    .checked_sub(source.source_start)
                    .context("Rotation blend key precedes its source range")?;
                let delta = source
                    .deltas
                    .get(index)
                    .context("Rotation blend key is outside its source range")?;
                origin[0] += delta[0] * key.weight;
                origin[1] += delta[1] * key.weight;
                *angle += delta[2] * key.weight;
                *scale += delta[3] * key.weight;
                *opacity += delta[4] * key.weight;
            }
        }
        *angle = angle.clamp(-3600.0, 3600.0);
        *scale = scale.clamp(0.0001, 100.0);
        *opacity = opacity.clamp(0.0, 1.0);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CompiledBlendBinding {
    pub binding: usize,
    pub source_start: usize,
    pub deltas: Vec<Vec<[f32; 2]>>,
}

#[derive(Debug, Clone)]
pub struct CompiledBlendMesh {
    pub target: usize,
    pub bindings: Vec<CompiledBlendBinding>,
}

#[derive(Debug, Clone)]
pub struct CompiledBlendWarpBinding {
    pub binding: usize,
    pub source_start: usize,
    pub position_deltas: Vec<Vec<[f32; 2]>>,
    pub opacity_deltas: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct CompiledBlendWarp {
    pub target: usize,
    pub bindings: Vec<CompiledBlendWarpBinding>,
}

impl CompiledBlendWarp {
    pub fn apply(
        &self,
        frame: &mut LocalDeformerFrame,
        graph: &BlendGraph,
        values: &[f32],
    ) -> Result<()> {
        let LocalDeformerFrame::Warp { points, opacity } = frame else {
            anyhow::bail!("Blend warp target is not a warp deformer");
        };
        for source in &self.bindings {
            for key in graph.weights(source.binding, values)? {
                let index = key
                    .index
                    .checked_sub(source.source_start)
                    .context("Warp blend key precedes its source range")?;
                let delta = source
                    .position_deltas
                    .get(index)
                    .context("Warp blend key is outside its source range")?;
                ensure!(
                    delta.len() == points.len(),
                    "Warp blend key has the wrong point count"
                );
                for (point, delta) in points.iter_mut().zip(delta) {
                    point[0] += delta[0] * key.weight;
                    point[1] += delta[1] * key.weight;
                }
                *opacity += source.opacity_deltas[index] * key.weight;
            }
        }
        *opacity = opacity.clamp(0.0, 1.0);
        Ok(())
    }
}

impl CompiledBlendMesh {
    pub fn apply(
        &self,
        frame: &mut LocalMeshFrame,
        graph: &BlendGraph,
        values: &[f32],
    ) -> Result<()> {
        for source in &self.bindings {
            for key in graph.weights(source.binding, values)? {
                let index = key
                    .index
                    .checked_sub(source.source_start)
                    .context("Blend key precedes its source range")?;
                let delta = source
                    .deltas
                    .get(index)
                    .context("Blend key is outside its source range")?;
                ensure!(
                    delta.len() == frame.positions.len(),
                    "Blend shape and ArtMesh vertex counts differ"
                );
                for (position, delta) in frame.positions.iter_mut().zip(delta) {
                    position[0] += delta[0] * key.weight;
                    position[1] += delta[1] * key.weight;
                }
            }
        }
        Ok(())
    }
}

impl BlendGraph {
    /// Resolve the non-base delta keyforms for one blend-shape binding.
    /// Constraints attenuate the result by their smallest authored response.
    pub fn weights(&self, binding: usize, values: &[f32]) -> Result<Vec<KeyformWeight>> {
        let binding = self
            .bindings
            .get(binding)
            .context("Unknown blend binding")?;
        let table = self
            .tables
            .get(binding.table)
            .context("Unknown blend table")?;
        let value = *values
            .get(table.parameter)
            .context("Missing blend parameter")?;
        let mut constraint_factor = 1.0_f32;
        for constraint in &binding.constraints {
            let value = *values
                .get(constraint.parameter)
                .context("Missing blend constraint parameter")?;
            let factors = crate::rig::linear_keys(&constraint.keys, value)?;
            let factor = factors
                .into_iter()
                .map(|key| constraint.weights[key.index] * key.weight)
                .sum::<f32>();
            constraint_factor = constraint_factor.min(factor);
        }
        ensure!(constraint_factor.is_finite(), "Invalid blend constraint");
        let mut weights = Vec::new();
        for key in crate::rig::linear_keys(&table.keys, value)? {
            if key.index == table.base_key || key.weight == 0.0 {
                continue;
            }
            ensure!(
                key.index < binding.source_len,
                "Blend source key is missing"
            );
            weights.push(KeyformWeight {
                index: binding.source_start + key.index,
                weight: key.weight * constraint_factor,
            });
        }
        Ok(weights)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyformWeight {
    pub index: usize,
    pub weight: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalMeshFrame {
    pub positions: Vec<[f32; 2]>,
    pub opacity: f32,
    pub draw_order: f32,
    pub multiply: [f32; 4],
    pub screen: [f32; 4],
    pub parent_deformer: Option<usize>,
}

impl LocalMeshFrame {
    /// The exported render order is integral. Interpolation may land one ULP
    /// below an integer, so snap only numerical noise before truncating.
    pub fn integer_draw_order(&self) -> i32 {
        let rounded = self.draw_order.round();
        let tolerance = 2.0 * f32::EPSILON * self.draw_order.abs().max(1.0);
        let value = if (self.draw_order - rounded).abs() <= tolerance {
            rounded
        } else {
            self.draw_order
        };
        value as i32
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorPair {
    pub multiply: [f32; 3],
    pub screen: [f32; 3],
}

impl Default for ColorPair {
    fn default() -> Self {
        Self {
            multiply: [1.0; 3],
            screen: [0.0; 3],
        }
    }
}

impl ColorPair {
    pub fn under(self, parent: Self) -> Self {
        let mut output = self;
        for channel in 0..3 {
            output.multiply[channel] =
                (self.multiply[channel] * parent.multiply[channel]).clamp(0.0, 1.0);
            output.screen[channel] = (self.screen[channel] + parent.screen[channel]
                - self.screen[channel] * parent.screen[channel])
                .clamp(0.0, 1.0);
        }
        output
    }
}

#[derive(Debug, Clone)]
pub struct ColorKeyforms {
    colors: Vec<ColorPair>,
}

impl ColorKeyforms {
    pub fn decoded_bytes(&self) -> usize {
        self.colors.len() * std::mem::size_of::<ColorPair>()
    }

    pub fn frame(&self, weights: &[KeyformWeight]) -> Result<ColorPair> {
        ensure!(!weights.is_empty(), "Color binding has no active keyforms");
        let mut color = ColorPair {
            multiply: [0.0; 3],
            screen: [0.0; 3],
        };
        for key in weights {
            let source = self
                .colors
                .get(key.index)
                .context("Unknown color keyform")?;
            ensure!(key.weight.is_finite(), "Non-finite color weight");
            for channel in 0..3 {
                color.multiply[channel] += source.multiply[channel] * key.weight;
                color.screen[channel] += source.screen[channel] * key.weight;
            }
        }
        Ok(color)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartLayout {
    pub id: String,
    pub binding: usize,
    pub parent: Option<usize>,
    pub visible: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrawItem {
    Mesh(usize),
    Part { index: usize, child_group: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawGroupLayout {
    pub min_order: i32,
    pub max_order: i32,
    pub total_count: usize,
    pub items: Vec<DrawItem>,
}

#[derive(Debug, Clone)]
struct MeshKeyform {
    positions: Vec<[f32; 2]>,
    opacity: f32,
    draw_order: f32,
}

/// Immutable, load-time-decoded mesh keyforms. The runtime blends these RAM
/// arrays without re-reading or revalidating the MOC3 container each frame.
#[derive(Debug, Clone)]
pub struct CompiledMesh {
    pub binding: usize,
    parent_deformer: Option<usize>,
    keyforms: Vec<MeshKeyform>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlueVertexPair {
    pub left: usize,
    pub right: usize,
    pub left_weight: f32,
    pub right_weight: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlueLayout {
    pub left_mesh: usize,
    pub right_mesh: usize,
    pub binding: usize,
    pub intensities: Vec<f32>,
    pub pairs: Vec<GlueVertexPair>,
}

impl GlueLayout {
    pub fn intensity(&self, weights: &[KeyformWeight]) -> Result<f32> {
        ensure!(!weights.is_empty(), "Glue binding has no active keyforms");
        let mut intensity = 0.0_f32;
        for key in weights {
            let source = *self
                .intensities
                .get(key.index)
                .context("Glue binding exceeds its keyforms")?;
            intensity += source * key.weight;
        }
        ensure!(intensity.is_finite(), "Glue intensity is not finite");
        Ok(intensity)
    }
}

impl CompiledMesh {
    /// Borrow load-time ArtMesh keys for GPU upload without cloning positions.
    pub fn keyform_points(&self) -> Vec<&[[f32; 2]]> {
        self.keyforms
            .iter()
            .map(|keyform| keyform.positions.as_slice())
            .collect()
    }

    pub fn decoded_position_bytes(&self) -> usize {
        self.keyforms
            .iter()
            .map(|keyform| keyform.positions.len() * std::mem::size_of::<[f32; 2]>())
            .sum()
    }

    pub fn frame(&self, weights: &[KeyformWeight]) -> Result<LocalMeshFrame> {
        ensure!(!weights.is_empty(), "Mesh binding has no active keyforms");
        ensure!(
            weights.iter().all(|weight| weight.weight.is_finite()),
            "Mesh binding contains non-finite weights"
        );
        if let [key] = weights
            && key.weight == 1.0
        {
            let source = self
                .keyforms
                .get(key.index)
                .context("Binding exceeds mesh keyforms")?;
            return Ok(LocalMeshFrame {
                positions: source.positions.clone(),
                opacity: source.opacity,
                draw_order: source.draw_order,
                multiply: [1.0; 4],
                screen: [0.0, 0.0, 0.0, 1.0],
                parent_deformer: self.parent_deformer,
            });
        }
        let first = self.keyforms.first().context("Mesh has no keyforms")?;
        let mut positions = vec![[0.0_f32; 2]; first.positions.len()];
        let mut opacity = 0.0_f32;
        let mut draw_order = 0.0_f32;
        for key in weights {
            let source = self
                .keyforms
                .get(key.index)
                .context("Binding exceeds mesh keyforms")?;
            opacity += source.opacity * key.weight;
            draw_order += source.draw_order * key.weight;
            for (output, source) in positions.iter_mut().zip(&source.positions) {
                output[0] += source[0] * key.weight;
                output[1] += source[1] * key.weight;
            }
        }
        Ok(LocalMeshFrame {
            positions,
            opacity,
            draw_order,
            multiply: [1.0; 4],
            screen: [0.0, 0.0, 0.0, 1.0],
            parent_deformer: self.parent_deformer,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeformerKind {
    Warp {
        columns: usize,
        rows: usize,
        points: usize,
        quad: bool,
    },
    Rotation {
        base_angle: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeformerLayout {
    pub id: String,
    pub parent_part: Option<usize>,
    pub parent_deformer: Option<usize>,
    pub binding: usize,
    pub local_index: usize,
    pub keyform_start: usize,
    pub keyform_count: usize,
    pub enabled: bool,
    pub visible: bool,
    pub kind: DeformerKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalDeformerFrame {
    Warp {
        points: Vec<[f32; 2]>,
        opacity: f32,
    },
    Rotation {
        origin: [f32; 2],
        angle: f32,
        scale: f32,
        opacity: f32,
        reflect: [bool; 2],
    },
}

/// Immutable load-time-decoded deformer keyforms. Frame evaluation uses these
/// resident arrays and does not inspect the encoded MOC3 buffer again.
#[derive(Debug, Clone)]
pub struct CompiledDeformer {
    keyforms: Vec<LocalDeformerFrame>,
}

impl CompiledDeformer {
    /// Borrow load-time warp keys for GPU upload without cloning their points.
    pub fn warp_keyform_points(&self) -> Result<Vec<&[[f32; 2]]>> {
        self.keyforms
            .iter()
            .map(|frame| match frame {
                LocalDeformerFrame::Warp { points, .. } => Ok(points.as_slice()),
                LocalDeformerFrame::Rotation { .. } => anyhow::bail!("Expected warp keyform"),
            })
            .collect()
    }

    pub fn frame(&self, weights: &[KeyformWeight]) -> Result<LocalDeformerFrame> {
        ensure!(
            !weights.is_empty(),
            "Deformer binding has no active keyforms"
        );
        ensure!(
            weights.iter().all(|key| key.weight.is_finite()),
            "Deformer binding contains non-finite weights"
        );
        if let [key] = weights
            && key.weight == 1.0
        {
            return self
                .keyforms
                .get(key.index)
                .cloned()
                .context("Unknown deformer keyform");
        }
        match self.keyforms.first().context("Deformer has no keyforms")? {
            LocalDeformerFrame::Warp { points, .. } => {
                let mut output = vec![[0.0_f32; 2]; points.len()];
                let mut opacity = 0.0_f32;
                for key in weights {
                    let LocalDeformerFrame::Warp {
                        points: source,
                        opacity: source_opacity,
                    } = self
                        .keyforms
                        .get(key.index)
                        .context("Unknown warp keyform")?
                    else {
                        anyhow::bail!("Mixed deformer keyform types");
                    };
                    ensure!(source.len() == output.len(), "Warp keyform size changed");
                    opacity += source_opacity * key.weight;
                    for (out, position) in output.iter_mut().zip(source) {
                        out[0] += position[0] * key.weight;
                        out[1] += position[1] * key.weight;
                    }
                }
                Ok(LocalDeformerFrame::Warp {
                    points: output,
                    opacity,
                })
            }
            LocalDeformerFrame::Rotation { .. } => {
                let mut origin = [0.0_f32; 2];
                let (mut angle, mut scale, mut opacity) = (0.0_f32, 0.0_f32, 0.0_f32);
                let mut dominant = (f32::NEG_INFINITY, [false; 2]);
                for key in weights {
                    let LocalDeformerFrame::Rotation {
                        origin: source_origin,
                        angle: source_angle,
                        scale: source_scale,
                        opacity: source_opacity,
                        reflect,
                    } = self
                        .keyforms
                        .get(key.index)
                        .context("Unknown rotation keyform")?
                    else {
                        anyhow::bail!("Mixed deformer keyform types");
                    };
                    origin[0] += source_origin[0] * key.weight;
                    origin[1] += source_origin[1] * key.weight;
                    angle += source_angle * key.weight;
                    scale += source_scale * key.weight;
                    opacity += source_opacity * key.weight;
                    if key.weight > dominant.0 {
                        dominant = (key.weight, *reflect);
                    }
                }
                Ok(LocalDeformerFrame::Rotation {
                    origin,
                    angle,
                    scale,
                    opacity,
                    reflect: dominant.1,
                })
            }
        }
    }
}

impl BindingGraph {
    /// Whether every parameter axis is within this binding's authored keys.
    /// Objects with an out-of-range normal binding are disabled, rather than
    /// rendered at a clamped endpoint.
    pub fn in_range(&self, binding: usize, values: &[f32]) -> Result<bool> {
        let axes = self
            .bindings
            .get(binding)
            .context("Unknown keyform binding")?;
        for &table_index in axes {
            let table = self.tables.get(table_index).context("Invalid key table")?;
            let value = *values
                .get(table.parameter)
                .context("Missing parameter value")?;
            let first = *table.keys.first().context("Empty key table")?;
            let last = *table.keys.last().context("Empty key table")?;
            if value < first || value > last {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Resolve a binding in mixed-radix keyform order. The first parameter
    /// axis changes fastest, matching the exported keyform layout.
    pub fn weights(&self, binding: usize, values: &[f32]) -> Result<Vec<KeyformWeight>> {
        let axes = self
            .bindings
            .get(binding)
            .context("Unknown keyform binding")?;
        let mut combinations = vec![KeyformWeight {
            index: 0,
            weight: 1.0,
        }];
        let mut stride = 1_usize;
        for &table_index in axes {
            let table = self.tables.get(table_index).context("Invalid key table")?;
            let value = *values
                .get(table.parameter)
                .context("Missing parameter value")?;
            let selected = crate::rig::linear_keys(&table.keys, value)?;
            let capacity = combinations
                .len()
                .checked_mul(selected.len())
                .context("Keyform combination overflow")?;
            ensure!(capacity <= 65_536, "Too many active keyform combinations");
            let mut next = Vec::with_capacity(capacity);
            for key in selected {
                for current in &combinations {
                    next.push(KeyformWeight {
                        index: current.index + key.index * stride,
                        weight: current.weight * key.weight,
                    });
                }
            }
            combinations = next;
            stride = stride
                .checked_mul(table.keys.len())
                .context("Keyform index overflow")?;
            ensure!(stride <= 16_777_216, "Keyform grid exceeds ARIA limits");
        }
        Ok(combinations)
    }
}

impl<'a> Moc<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        ensure!(
            (HEADER_BYTES..=MAX_MOC_BYTES).contains(&bytes.len()),
            "MOC3 size is invalid"
        );
        ensure!(&bytes[..4] == b"MOC3", "MOC3 signature is missing");
        let version = bytes[4];
        ensure!(
            (1..=6).contains(&version),
            "Unsupported MOC3 version {version}"
        );
        let byte_order = match bytes[5] {
            0 => ByteOrder::Little,
            1 => ByteOrder::Big,
            _ => anyhow::bail!("Invalid MOC3 byte-order flag"),
        };
        let count = if version == 6 {
            EXTENDED_SECTIONS
        } else {
            LEGACY_SECTIONS
        };
        let table_end = HEADER_BYTES + count * 4;
        ensure!(bytes.len() >= table_end, "MOC3 section table is truncated");

        let mut offsets = Vec::with_capacity(count);
        for &raw in bytes[HEADER_BYTES..table_end].as_chunks::<4>().0 {
            let offset = match byte_order {
                ByteOrder::Little => u32::from_le_bytes(raw),
                ByteOrder::Big => u32::from_be_bytes(raw),
            } as usize;
            ensure!(
                offset == 0 || (offset >= table_end && offset <= bytes.len()),
                "MOC3 section offset is outside the file"
            );
            offsets.push(offset);
        }

        let mut sections = Vec::with_capacity(count);
        let mut last = table_end;
        for (i, &start) in offsets.iter().enumerate() {
            if start == 0 {
                sections.push(None);
                continue;
            }
            ensure!(start >= last, "MOC3 section offsets are out of order");
            let end = offsets[i + 1..]
                .iter()
                .copied()
                .find(|&next| next != 0)
                .unwrap_or(bytes.len());
            ensure!(end >= start, "MOC3 section has a negative length");
            sections.push(Some(start..end));
            last = start;
        }
        Ok(Self {
            bytes,
            version,
            byte_order,
            sections,
        })
    }

    pub fn version(&self) -> u8 {
        self.version
    }
    pub fn offscreen_count(&self) -> Result<usize> {
        if self.version >= 6 {
            self.nonnegative(0, 35)
        } else {
            Ok(0)
        }
    }
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }
    pub fn section(&self, index: usize) -> Option<&'a [u8]> {
        self.sections
            .get(index)?
            .as_ref()
            .map(|r| &self.bytes[r.clone()])
    }

    fn word(&self, section: usize, index: usize) -> Result<u32> {
        let start = index.checked_mul(4).context("MOC3 field index overflow")?;
        let end = start.checked_add(4).context("MOC3 field index overflow")?;
        let bytes = self
            .section(section)
            .context("Missing MOC3 section")?
            .get(start..end)
            .with_context(|| format!("Truncated MOC3 field in section {section} at {index}"))?;
        let raw: [u8; 4] = bytes.try_into().context("Invalid MOC3 field")?;
        Ok(match self.byte_order {
            ByteOrder::Little => u32::from_le_bytes(raw),
            ByteOrder::Big => u32::from_be_bytes(raw),
        })
    }

    fn short(&self, section: usize, index: usize) -> Result<u16> {
        let start = index.checked_mul(2).context("MOC3 index overflow")?;
        let end = start.checked_add(2).context("MOC3 index overflow")?;
        let bytes = self
            .section(section)
            .context("Missing MOC3 section")?
            .get(start..end)
            .context("Truncated MOC3 index")?;
        let raw: [u8; 2] = bytes.try_into().context("Invalid MOC3 index")?;
        Ok(match self.byte_order {
            ByteOrder::Little => u16::from_le_bytes(raw),
            ByteOrder::Big => u16::from_be_bytes(raw),
        })
    }

    fn nonnegative(&self, section: usize, index: usize) -> Result<usize> {
        usize::try_from(self.word(section, index)? as i32).context("Negative MOC3 count or offset")
    }

    fn id(&self, section: usize, index: usize) -> Result<String> {
        let start = index.checked_mul(64).context("MOC3 ID offset overflow")?;
        let end = start.checked_add(64).context("MOC3 ID offset overflow")?;
        let record = self
            .section(section)
            .context("Missing MOC3 IDs")?
            .get(start..end)
            .context("Truncated MOC3 ID")?;
        let terminator = record.iter().position(|&b| b == 0).unwrap_or(64);
        let id = std::str::from_utf8(&record[..terminator])
            .context("Invalid MOC3 ID")?
            .to_owned();
        ensure!(!id.is_empty(), "Empty MOC3 ID");
        Ok(id)
    }

    pub fn parameters(&self) -> Result<Vec<ParameterSpec>> {
        let count = self.counts()?.parameters as usize;
        let mut parameters = Vec::with_capacity(count);
        for i in 0..count {
            let minimum = f32::from_bits(self.word(52, i)?);
            let maximum = f32::from_bits(self.word(51, i)?);
            let default = f32::from_bits(self.word(53, i)?);
            ensure!(
                minimum.is_finite()
                    && maximum.is_finite()
                    && default.is_finite()
                    && minimum <= maximum,
                "Invalid MOC3 parameter range"
            );
            let kind = if self.version < 4 {
                ParameterKind::Normal
            } else {
                match self.word(114, i)? {
                    0 => ParameterKind::Normal,
                    1 => ParameterKind::BlendShape,
                    _ => anyhow::bail!("Unknown MOC3 parameter type"),
                }
            };
            parameters.push(ParameterSpec {
                id: self.id(50, i)?,
                minimum,
                maximum,
                default: default.clamp(minimum, maximum),
                repeat: self.word(54, i)? != 0,
                kind,
            });
        }
        Ok(parameters)
    }

    /// Decode the normal-parameter key table graph without native pointers.
    /// Blend-shape bindings are a separate format and are decoded later.
    pub fn binding_graph(&self) -> Result<BindingGraph> {
        let counts = self.counts()?;
        let parameter_count = counts.parameters as usize;
        let table_count = self.nonnegative(0, 13)?;
        let binding_count = self.nonnegative(0, 12)?;
        let index_count = self.nonnegative(0, 11)?;
        let key_count = self.nonnegative(0, 14)?;
        ensure!(
            table_count <= 1_000_000
                && binding_count <= 1_000_000
                && index_count <= 4_000_000
                && key_count <= 4_000_000,
            "MOC3 binding graph exceeds ARIA limits"
        );
        let mut owner = vec![None; table_count];
        for parameter in 0..parameter_count {
            let raw_start = self.word(56, parameter)? as i32;
            let count = self.nonnegative(57, parameter)?;
            if raw_start == -1 && count == 0 {
                continue;
            }
            let start = usize::try_from(raw_start).context("Invalid parameter key table offset")?;
            let end = start
                .checked_add(count)
                .context("Key table range overflow")?;
            ensure!(end <= table_count, "Parameter key table range is invalid");
            for slot in &mut owner[start..end] {
                ensure!(slot.is_none(), "Key table has multiple parameter owners");
                *slot = Some(parameter);
            }
        }
        let mut tables = Vec::with_capacity(table_count);
        for (table, parameter) in owner.into_iter().enumerate() {
            let start = self.nonnegative(75, table)?;
            let count = self.nonnegative(76, table)?;
            ensure!(count > 0 && count <= 256, "Invalid parameter key count");
            let end = start
                .checked_add(count)
                .context("Parameter key range overflow")?;
            ensure!(end <= key_count, "Parameter keys exceed the key pool");
            let mut keys = Vec::with_capacity(count);
            for index in start..end {
                let key = f32::from_bits(self.word(77, index)?);
                ensure!(key.is_finite(), "Non-finite parameter key");
                keys.push(key);
            }
            ensure!(
                keys.windows(2).all(|pair| pair[0] < pair[1]),
                "Parameter keys are not strictly increasing"
            );
            tables.push(KeyTable {
                parameter: parameter.context("Key table has no parameter owner")?,
                keys,
            });
        }
        let mut bindings = Vec::with_capacity(binding_count);
        for binding in 0..binding_count {
            let start = self.nonnegative(73, binding)?;
            let count = self.nonnegative(74, binding)?;
            ensure!(count <= 16, "Binding has too many parameter axes");
            let end = start
                .checked_add(count)
                .context("Binding index range overflow")?;
            ensure!(end <= index_count, "Binding indexes exceed the index pool");
            let mut axes = Vec::with_capacity(count);
            for index in start..end {
                let table = self.nonnegative(72, index)?;
                ensure!(
                    table < table_count,
                    "Binding references an invalid key table"
                );
                axes.push(table);
            }
            bindings.push(axes);
        }
        Ok(BindingGraph { tables, bindings })
    }

    pub fn blend_graph(&self) -> Result<BlendGraph> {
        if self.version < 4 {
            return Ok(BlendGraph {
                tables: Vec::new(),
                bindings: Vec::new(),
                art_meshes: Vec::new(),
                warps: Vec::new(),
                rotations: Vec::new(),
                glues: Vec::new(),
            });
        }
        let counts = self.counts()?;
        let table_count = self.nonnegative(0, 25)?;
        let binding_count = self.nonnegative(0, 26)?;
        let constraint_index_count = self.nonnegative(0, 29)?;
        let constraint_count = self.nonnegative(0, 30)?;
        let constraint_value_count = self.nonnegative(0, 31)?;
        let key_pool_count = self.nonnegative(0, 14)?;
        ensure!(
            table_count <= 1_000_000
                && binding_count <= 1_000_000
                && constraint_index_count <= 4_000_000
                && constraint_count <= 1_000_000
                && constraint_value_count <= 4_000_000,
            "Blend-shape graph exceeds ARIA limits"
        );
        let mut owner = vec![None; table_count];
        for parameter in 0..counts.parameters as usize {
            let raw_start = self.word(115, parameter)? as i32;
            let count = self.nonnegative(116, parameter)?;
            if raw_start == -1 && count == 0 {
                continue;
            }
            let start = usize::try_from(raw_start).context("Invalid blend table offset")?;
            ensure!(
                start
                    .checked_add(count)
                    .is_some_and(|end| end <= table_count),
                "Blend table range is invalid"
            );
            for slot in &mut owner[start..start + count] {
                ensure!(slot.is_none(), "Blend table has multiple owners");
                *slot = Some(parameter);
            }
        }
        let mut tables = Vec::with_capacity(table_count);
        for (index, parameter) in owner.into_iter().enumerate() {
            let start = self.nonnegative(117, index)?;
            let count = self.nonnegative(118, index)?;
            let base_key = self.nonnegative(119, index)?;
            ensure!(
                count > 0
                    && count <= 256
                    && base_key < count
                    && start
                        .checked_add(count)
                        .is_some_and(|end| end <= key_pool_count),
                "Blend key table is invalid"
            );
            let mut keys = Vec::with_capacity(count);
            for key in start..start + count {
                let value = f32::from_bits(self.word(77, key)?);
                ensure!(value.is_finite(), "Blend key is non-finite");
                keys.push(value);
            }
            ensure!(
                keys.windows(2).all(|pair| pair[0] < pair[1]),
                "Blend keys are not strictly increasing"
            );
            tables.push(BlendKeyTable {
                parameter: parameter.context("Blend table has no parameter owner")?,
                keys,
                base_key,
            });
        }
        let mut bindings = Vec::with_capacity(binding_count);
        for index in 0..binding_count {
            let table = self.nonnegative(120, index)?;
            ensure!(table < table_count, "Blend binding has an invalid table");
            let source_start = self.nonnegative(121, index)?;
            let source_len = self.nonnegative(122, index)?;
            ensure!(
                source_len >= tables[table].keys.len(),
                "Blend binding is missing source keyforms"
            );
            let raw_constraint_start = self.word(123, index)? as i32;
            let constraint_len = self.nonnegative(124, index)?;
            let mut constraints = Vec::with_capacity(constraint_len);
            if constraint_len > 0 {
                let start = usize::try_from(raw_constraint_start)
                    .context("Invalid blend constraint index offset")?;
                ensure!(
                    start
                        .checked_add(constraint_len)
                        .is_some_and(|end| end <= constraint_index_count),
                    "Blend constraint index range is invalid"
                );
                for item in start..start + constraint_len {
                    let constraint = self.nonnegative(131, item)?;
                    ensure!(constraint < constraint_count, "Unknown blend constraint");
                    let parameter = self.nonnegative(132, constraint)?;
                    ensure!(
                        parameter < counts.parameters as usize,
                        "Blend constraint has an invalid parameter"
                    );
                    let key_start = self.nonnegative(133, constraint)?;
                    let key_len = self.nonnegative(134, constraint)?;
                    ensure!(
                        key_len > 0
                            && key_len <= 256
                            && key_start
                                .checked_add(key_len)
                                .is_some_and(|end| end <= constraint_value_count),
                        "Blend constraint key range is invalid"
                    );
                    let mut keys = Vec::with_capacity(key_len);
                    let mut weights = Vec::with_capacity(key_len);
                    for key in key_start..key_start + key_len {
                        let value = f32::from_bits(self.word(135, key)?);
                        let weight = f32::from_bits(self.word(136, key)?);
                        ensure!(
                            value.is_finite() && weight.is_finite(),
                            "Non-finite blend constraint"
                        );
                        keys.push(value);
                        weights.push(weight);
                    }
                    ensure!(
                        keys.windows(2).all(|pair| pair[0] < pair[1]),
                        "Blend constraint keys are not increasing"
                    );
                    constraints.push(BlendConstraint {
                        parameter,
                        keys,
                        weights,
                    });
                }
            }
            bindings.push(BlendBinding {
                table,
                source_start,
                source_len,
                constraints,
            });
        }
        let targets = |target_section: usize,
                       binding_section: usize,
                       length_section: usize,
                       count: usize,
                       target_limit: usize|
         -> Result<Vec<BlendTarget>> {
            let mut result = Vec::with_capacity(count);
            for index in 0..count {
                let target = self.nonnegative(target_section, index)?;
                let start = self.nonnegative(binding_section, index)?;
                let len = self.nonnegative(length_section, index)?;
                ensure!(
                    target < target_limit
                        && start
                            .checked_add(len)
                            .is_some_and(|end| end <= binding_count),
                    "Blend target range is invalid"
                );
                result.push(BlendTarget {
                    target,
                    bindings: (start..start + len).collect(),
                });
            }
            Ok(result)
        };
        Ok(BlendGraph {
            tables,
            bindings,
            art_meshes: targets(
                128,
                129,
                130,
                self.nonnegative(0, 28)?,
                counts.art_meshes as usize,
            )?,
            warps: targets(
                125,
                126,
                127,
                self.nonnegative(0, 27)?,
                counts.warp_deformers as usize,
            )?,
            rotations: if self.version >= 5 {
                targets(
                    146,
                    147,
                    148,
                    self.nonnegative(0, 33)?,
                    counts.rotation_deformers as usize,
                )?
            } else {
                Vec::new()
            },
            glues: if self.version >= 5 {
                targets(
                    149,
                    150,
                    151,
                    self.nonnegative(0, 34)?,
                    self.nonnegative(0, 20)?,
                )?
            } else {
                Vec::new()
            },
        })
    }

    pub fn blend_mesh_position_bytes(
        &self,
        target: &BlendTarget,
        graph: &BlendGraph,
    ) -> Result<usize> {
        let vertices = self.nonnegative(43, target.target)?;
        let mut total = 0_usize;
        for &binding in &target.bindings {
            let table = graph
                .bindings
                .get(binding)
                .context("Unknown ArtMesh blend binding")?;
            total = total
                .checked_add(
                    table
                        .source_len
                        .checked_mul(vertices)
                        .and_then(|points| points.checked_mul(std::mem::size_of::<[f32; 2]>()))
                        .context("Blend ArtMesh RAM size overflow")?,
                )
                .context("Blend ArtMesh RAM size overflow")?;
        }
        Ok(total)
    }

    pub fn compile_blend_mesh(
        &self,
        target: &BlendTarget,
        graph: &BlendGraph,
    ) -> Result<CompiledBlendMesh> {
        let counts = self.counts()?;
        ensure!(
            target.target < counts.art_meshes as usize,
            "Unknown blend ArtMesh"
        );
        ensure!(
            self.blend_mesh_position_bytes(target, graph)? <= 512 * 1024 * 1024,
            "Blend ArtMesh keyforms exceed the 512 MiB RAM budget"
        );
        let vertices = self.nonnegative(43, target.target)?;
        let keyform_total = self.nonnegative(0, 9)?;
        let coordinate_total = self.nonnegative(0, 10)?;
        let mut bindings = Vec::with_capacity(target.bindings.len());
        for &binding_index in &target.bindings {
            let binding = graph
                .bindings
                .get(binding_index)
                .context("Unknown ArtMesh blend binding")?;
            ensure!(
                binding
                    .source_start
                    .checked_add(binding.source_len)
                    .is_some_and(|end| end <= keyform_total),
                "Blend ArtMesh keyforms exceed the keyform pool"
            );
            let mut deltas = Vec::with_capacity(binding.source_len);
            for frame in binding.source_start..binding.source_start + binding.source_len {
                let start = self.nonnegative(70, frame)?;
                ensure!(
                    vertices
                        .checked_mul(2)
                        .and_then(|coordinates| start.checked_add(coordinates))
                        .is_some_and(|end| end <= coordinate_total),
                    "Blend ArtMesh positions exceed the coordinate pool"
                );
                let mut positions = Vec::with_capacity(vertices);
                for vertex in 0..vertices {
                    let offset = start + vertex * 2;
                    let x = f32::from_bits(self.word(71, offset)?);
                    let y = f32::from_bits(self.word(71, offset + 1)?);
                    ensure!(
                        x.is_finite() && y.is_finite(),
                        "Non-finite blend ArtMesh delta"
                    );
                    positions.push([x, y]);
                }
                deltas.push(positions);
            }
            bindings.push(CompiledBlendBinding {
                binding: binding_index,
                source_start: binding.source_start,
                deltas,
            });
        }
        Ok(CompiledBlendMesh {
            target: target.target,
            bindings,
        })
    }

    pub fn blend_warp_position_bytes(
        &self,
        target: &BlendTarget,
        graph: &BlendGraph,
    ) -> Result<usize> {
        ensure!(
            target.target < self.counts()?.warp_deformers as usize,
            "Unknown blend warp"
        );
        let points = self.nonnegative(22, target.target)?;
        let mut total = 0_usize;
        for &binding in &target.bindings {
            let source = graph
                .bindings
                .get(binding)
                .context("Unknown warp blend binding")?;
            total = total
                .checked_add(
                    source
                        .source_len
                        .checked_mul(points)
                        .and_then(|count| count.checked_mul(std::mem::size_of::<[f32; 2]>()))
                        .context("Blend warp RAM size overflow")?,
                )
                .context("Blend warp RAM size overflow")?;
        }
        Ok(total)
    }

    pub fn compile_blend_warp(
        &self,
        target: &BlendTarget,
        graph: &BlendGraph,
    ) -> Result<CompiledBlendWarp> {
        ensure!(
            self.blend_warp_position_bytes(target, graph)? <= 512 * 1024 * 1024,
            "Blend warp keyforms exceed the 512 MiB RAM budget"
        );
        let points = self.nonnegative(22, target.target)?;
        let keyform_total = self.nonnegative(0, 7)?;
        let coordinate_total = self.nonnegative(0, 10)?;
        let mut bindings = Vec::with_capacity(target.bindings.len());
        for &binding_index in &target.bindings {
            let binding = graph
                .bindings
                .get(binding_index)
                .context("Unknown warp blend binding")?;
            ensure!(
                binding
                    .source_start
                    .checked_add(binding.source_len)
                    .is_some_and(|end| end <= keyform_total),
                "Blend warp keyforms exceed the keyform pool"
            );
            let mut position_deltas = Vec::with_capacity(binding.source_len);
            let mut opacity_deltas = Vec::with_capacity(binding.source_len);
            for frame in binding.source_start..binding.source_start + binding.source_len {
                let opacity = f32::from_bits(self.word(59, frame)?);
                ensure!(opacity.is_finite(), "Non-finite blend warp opacity");
                opacity_deltas.push(opacity);
                let start = self.nonnegative(60, frame)?;
                ensure!(
                    points
                        .checked_mul(2)
                        .and_then(|coordinates| start.checked_add(coordinates))
                        .is_some_and(|end| end <= coordinate_total),
                    "Blend warp positions exceed the coordinate pool"
                );
                let mut positions = Vec::with_capacity(points);
                for point in 0..points {
                    let offset = start + point * 2;
                    let x = f32::from_bits(self.word(71, offset)?);
                    let y = f32::from_bits(self.word(71, offset + 1)?);
                    ensure!(
                        x.is_finite() && y.is_finite(),
                        "Non-finite blend warp delta"
                    );
                    positions.push([x, y]);
                }
                position_deltas.push(positions);
            }
            bindings.push(CompiledBlendWarpBinding {
                binding: binding_index,
                source_start: binding.source_start,
                position_deltas,
                opacity_deltas,
            });
        }
        Ok(CompiledBlendWarp {
            target: target.target,
            bindings,
        })
    }

    pub fn compile_blend_rotation(
        &self,
        target: &BlendTarget,
        graph: &BlendGraph,
    ) -> Result<CompiledBlendRotation> {
        ensure!(
            target.target < self.counts()?.rotation_deformers as usize,
            "Unknown blend rotation"
        );
        let keyform_total = self.nonnegative(0, 8)?;
        let mut bindings = Vec::with_capacity(target.bindings.len());
        for &binding_index in &target.bindings {
            let binding = graph
                .bindings
                .get(binding_index)
                .context("Unknown rotation blend binding")?;
            ensure!(
                binding
                    .source_start
                    .checked_add(binding.source_len)
                    .is_some_and(|end| end <= keyform_total),
                "Blend rotation keyforms exceed the keyform pool"
            );
            let mut deltas = Vec::with_capacity(binding.source_len);
            for frame in binding.source_start..binding.source_start + binding.source_len {
                let source = [
                    f32::from_bits(self.word(63, frame)?),
                    f32::from_bits(self.word(64, frame)?),
                    f32::from_bits(self.word(62, frame)?),
                    f32::from_bits(self.word(65, frame)?),
                    f32::from_bits(self.word(61, frame)?),
                ];
                ensure!(
                    source.iter().all(|value| value.is_finite()),
                    "Non-finite blend rotation delta"
                );
                deltas.push(source);
            }
            bindings.push(CompiledBlendRotationBinding {
                binding: binding_index,
                source_start: binding.source_start,
                deltas,
            });
        }
        Ok(CompiledBlendRotation {
            target: target.target,
            bindings,
        })
    }

    pub fn compile_blend_glue(
        &self,
        target: &BlendTarget,
        graph: &BlendGraph,
    ) -> Result<CompiledBlendScalar> {
        ensure!(
            target.target < self.nonnegative(0, 20)?,
            "Unknown blend glue"
        );
        let keyform_total = self.nonnegative(0, 22)?;
        let mut bindings = Vec::with_capacity(target.bindings.len());
        for &binding_index in &target.bindings {
            let binding = graph
                .bindings
                .get(binding_index)
                .context("Unknown glue blend binding")?;
            ensure!(
                binding
                    .source_start
                    .checked_add(binding.source_len)
                    .is_some_and(|end| end <= keyform_total),
                "Blend glue keyforms exceed the keyform pool"
            );
            let mut deltas = Vec::with_capacity(binding.source_len);
            for frame in binding.source_start..binding.source_start + binding.source_len {
                let delta = f32::from_bits(self.word(100, frame)?);
                ensure!(delta.is_finite(), "Non-finite blend glue intensity");
                deltas.push(delta);
            }
            bindings.push(CompiledBlendScalarBinding {
                binding: binding_index,
                source_start: binding.source_start,
                deltas,
            });
        }
        Ok(CompiledBlendScalar {
            target: target.target,
            bindings,
        })
    }

    /// Evaluate authored ArtMesh keyforms before parent deformers, glues and
    /// blend shapes. This is useful for testing the independent interpolation
    /// path, but is not a complete drawable evaluation.
    pub fn local_mesh_frame(
        &self,
        mesh: usize,
        graph: &BindingGraph,
        values: &[f32],
    ) -> Result<LocalMeshFrame> {
        let binding = self.mesh_binding(mesh)?;
        let weights = graph.weights(binding, values)?;
        self.local_mesh_frame_weighted(mesh, &weights)
    }

    pub fn mesh_binding(&self, mesh: usize) -> Result<usize> {
        ensure!(mesh < self.counts()?.art_meshes as usize, "Unknown ArtMesh");
        self.nonnegative(34, mesh)
    }

    pub fn mesh_parent_deformer(&self, mesh: usize) -> Result<Option<usize>> {
        let counts = self.counts()?;
        ensure!(mesh < counts.art_meshes as usize, "Unknown ArtMesh");
        let parent = self.word(40, mesh)? as i32;
        ensure!(
            parent == -1 || (parent >= 0 && parent < counts.deformers as i32),
            "Invalid ArtMesh parent deformer"
        );
        Ok((parent >= 0).then_some(parent as usize))
    }

    pub fn mesh_parent_part(&self, mesh: usize) -> Result<Option<usize>> {
        let counts = self.counts()?;
        ensure!(mesh < counts.art_meshes as usize, "Unknown ArtMesh");
        let parent = self.word(39, mesh)? as i32;
        ensure!(
            parent == -1 || (parent >= 0 && parent < counts.parts as i32),
            "Invalid ArtMesh parent part"
        );
        Ok((parent >= 0).then_some(parent as usize))
    }

    pub fn mesh_enabled(&self, mesh: usize) -> Result<bool> {
        ensure!(mesh < self.counts()?.art_meshes as usize, "Unknown ArtMesh");
        Ok(self.word(38, mesh)? != 0)
    }

    pub fn part_layouts(&self) -> Result<Vec<PartLayout>> {
        let count = self.counts()?.parts as usize;
        let mut parts = Vec::with_capacity(count);
        for index in 0..count {
            let parent = self.word(9, index)? as i32;
            ensure!(
                parent == -1 || (parent >= 0 && (parent as usize) < index),
                "Parts are not topologically ordered"
            );
            parts.push(PartLayout {
                id: self.id(3, index)?,
                binding: self.nonnegative(4, index)?,
                parent: (parent >= 0).then_some(parent as usize),
                visible: self.word(7, index)? != 0,
                enabled: self.word(8, index)? != 0,
            });
        }
        Ok(parts)
    }

    pub fn part_draw_order_keyforms(&self, part: usize) -> Result<Vec<f32>> {
        ensure!(part < self.counts()?.parts as usize, "Unknown part");
        let start = self.nonnegative(5, part)?;
        let count = self.nonnegative(6, part)?;
        let total = self.nonnegative(0, 6)?;
        ensure!(
            count > 0 && start.checked_add(count).is_some_and(|end| end <= total),
            "Part keyforms exceed the keyform pool"
        );
        (start..start + count)
            .map(|index| {
                let value = f32::from_bits(self.word(58, index)?);
                ensure!(value.is_finite(), "Non-finite part draw order");
                Ok(value)
            })
            .collect()
    }

    pub fn draw_group_layouts(&self) -> Result<Vec<DrawGroupLayout>> {
        let groups = self.nonnegative(0, 18)?;
        let items = self.nonnegative(0, 19)?;
        let counts = self.counts()?;
        ensure!(
            groups <= 16_384 && items <= 1_000_000,
            "Draw groups exceed ARIA limits"
        );
        let mut layouts = Vec::with_capacity(groups);
        for group in 0..groups {
            let start = self.nonnegative(81, group)?;
            let count = self.nonnegative(82, group)?;
            let total_count = self.nonnegative(83, group)?;
            let max_order = self.word(84, group)? as i32;
            let min_order = self.word(85, group)? as i32;
            ensure!(
                start.checked_add(count).is_some_and(|end| end <= items)
                    && total_count <= counts.art_meshes as usize + counts.parts as usize
                    && min_order <= max_order,
                "Invalid draw-group range"
            );
            let mut children = Vec::with_capacity(count);
            for item in start..start + count {
                let object = self.nonnegative(87, item)?;
                let entry = match self.word(86, item)? as i32 {
                    0 => {
                        ensure!(
                            object < counts.art_meshes as usize,
                            "Invalid draw-group mesh"
                        );
                        DrawItem::Mesh(object)
                    }
                    1 => {
                        ensure!(object < counts.parts as usize, "Invalid draw-group part");
                        let child_group = self.nonnegative(88, item)?;
                        ensure!(
                            child_group < groups && child_group != group,
                            "Invalid child draw group"
                        );
                        DrawItem::Part {
                            index: object,
                            child_group,
                        }
                    }
                    _ => anyhow::bail!("Unknown draw-group item type"),
                };
                children.push(entry);
            }
            layouts.push(DrawGroupLayout {
                min_order,
                max_order,
                total_count,
                items: children,
            });
        }
        Ok(layouts)
    }

    /// Decode normal-parameter color keyforms for an ArtMesh or deformer.
    /// MOC3 v5 stores separate pool indices per keyform; v4 uses one
    /// contiguous range per object. Missing authored colors use identity.
    pub fn compile_colors(
        &self,
        base_section: usize,
        multiply_key_section: usize,
        screen_key_section: usize,
        target: usize,
        keyform_start: usize,
        keyform_count: usize,
    ) -> Result<ColorKeyforms> {
        let mut colors = Vec::with_capacity(keyform_count);
        if self.version < 4 {
            colors.resize(keyform_count, ColorPair::default());
            return Ok(ColorKeyforms { colors });
        }
        let multiply_count = self.nonnegative(0, 23)?;
        let screen_count = self.nonnegative(0, 24)?;
        let base = self.word(base_section, target)? as i32;
        let read_rgb = |index: i32, sections: [usize; 3], count: usize, fallback: f32| {
            if index < 0 {
                return Ok([fallback; 3]);
            }
            let index = index as usize;
            ensure!(index < count, "Color keyform exceeds its pool");
            let mut rgb = [0.0_f32; 3];
            for (channel, section) in sections.into_iter().enumerate() {
                rgb[channel] = f32::from_bits(self.word(section, index)?);
                ensure!(rgb[channel].is_finite(), "Non-finite color keyform");
            }
            Ok(rgb)
        };
        for local in 0..keyform_count {
            let frame = keyform_start
                .checked_add(local)
                .context("Color keyform index overflow")?;
            let legacy_index = if base < 0 {
                -1
            } else {
                base.checked_add(i32::try_from(local).context("Too many color keys")?)
                    .context("Color keyform index overflow")?
            };
            let (multiply_index, screen_index) = if self.version >= 5
                && self.section(multiply_key_section).is_some()
                && self.section(screen_key_section).is_some()
            {
                (
                    self.word(multiply_key_section, frame)? as i32,
                    self.word(screen_key_section, frame)? as i32,
                )
            } else {
                (legacy_index, legacy_index)
            };
            colors.push(ColorPair {
                multiply: read_rgb(multiply_index, [108, 109, 110], multiply_count, 1.0)?,
                screen: read_rgb(screen_index, [111, 112, 113], screen_count, 0.0)?,
            });
        }
        Ok(ColorKeyforms { colors })
    }

    pub fn compile_mesh_colors(&self, mesh: usize) -> Result<ColorKeyforms> {
        ensure!(mesh < self.counts()?.art_meshes as usize, "Unknown ArtMesh");
        self.compile_colors(
            107,
            141,
            142,
            mesh,
            self.nonnegative(35, mesh)?,
            self.nonnegative(36, mesh)?,
        )
    }

    pub fn compile_deformer_colors(&self, layout: &DeformerLayout) -> Result<ColorKeyforms> {
        let (base, multiply, screen) = match layout.kind {
            DeformerKind::Warp { .. } => (105, 137, 138),
            DeformerKind::Rotation { .. } => (106, 139, 140),
        };
        self.compile_colors(
            base,
            multiply,
            screen,
            layout.local_index,
            layout.keyform_start,
            layout.keyform_count,
        )
    }

    pub fn mesh_keyform_position_bytes(&self, mesh: usize) -> Result<usize> {
        ensure!(mesh < self.counts()?.art_meshes as usize, "Unknown ArtMesh");
        self.nonnegative(36, mesh)?
            .checked_mul(self.nonnegative(43, mesh)?)
            .and_then(|points| points.checked_mul(std::mem::size_of::<[f32; 2]>()))
            .context("Mesh keyform RAM size overflow")
    }

    pub fn compile_mesh(&self, mesh: usize) -> Result<CompiledMesh> {
        let counts = self.counts()?;
        ensure!(mesh < counts.art_meshes as usize, "Unknown ArtMesh");
        ensure!(
            self.mesh_keyform_position_bytes(mesh)? <= 512 * 1024 * 1024,
            "Mesh keyforms exceed the 512 MiB RAM budget"
        );
        let keyform_start = self.nonnegative(35, mesh)?;
        let keyform_count = self.nonnegative(36, mesh)?;
        let vertex_count = self.nonnegative(43, mesh)?;
        let keyform_total = self.nonnegative(0, 9)?;
        ensure!(
            keyform_count > 0
                && keyform_start
                    .checked_add(keyform_count)
                    .is_some_and(|end| end <= keyform_total)
                && vertex_count <= 65_536,
            "ArtMesh keyform range is invalid"
        );
        let coordinate_count = self.nonnegative(0, 10)?;
        let parent = self.word(40, mesh)? as i32;
        ensure!(
            parent == -1 || (parent >= 0 && parent < counts.deformers as i32),
            "Invalid ArtMesh parent deformer"
        );
        let mut keyforms = Vec::with_capacity(keyform_count);
        for frame in keyform_start..keyform_start + keyform_count {
            let start = self.nonnegative(70, frame)?;
            ensure!(
                vertex_count
                    .checked_mul(2)
                    .and_then(|coordinates| start.checked_add(coordinates))
                    .is_some_and(|end| end <= coordinate_count),
                "ArtMesh key positions exceed the position pool"
            );
            let opacity = f32::from_bits(self.word(68, frame)?);
            let draw_order = f32::from_bits(self.word(69, frame)?);
            ensure!(
                opacity.is_finite() && draw_order.is_finite(),
                "Invalid ArtMesh keyform scalar"
            );
            let mut positions = Vec::with_capacity(vertex_count);
            for vertex in 0..vertex_count {
                let offset = start + vertex * 2;
                let x = f32::from_bits(self.word(71, offset)?);
                let y = f32::from_bits(self.word(71, offset + 1)?);
                ensure!(
                    x.is_finite() && y.is_finite(),
                    "Invalid ArtMesh key position"
                );
                positions.push([x, y]);
            }
            keyforms.push(MeshKeyform {
                positions,
                opacity,
                draw_order,
            });
        }
        Ok(CompiledMesh {
            binding: self.mesh_binding(mesh)?,
            parent_deformer: (parent >= 0).then_some(parent as usize),
            keyforms,
        })
    }

    /// Evaluate a mesh with weights already computed for its binding. The
    /// model-scoped evaluator shares those weights across all bound objects.
    pub fn local_mesh_frame_weighted(
        &self,
        mesh: usize,
        weights: &[KeyformWeight],
    ) -> Result<LocalMeshFrame> {
        let counts = self.counts()?;
        ensure!(mesh < counts.art_meshes as usize, "Unknown ArtMesh");
        let keyform_start = self.nonnegative(35, mesh)?;
        let keyform_count = self.nonnegative(36, mesh)?;
        let vertex_count = self.nonnegative(43, mesh)?;
        let keyform_total = self.nonnegative(0, 9)?;
        ensure!(
            keyform_count > 0
                && keyform_start
                    .checked_add(keyform_count)
                    .is_some_and(|end| end <= keyform_total),
            "ArtMesh keyforms exceed the keyform pool"
        );
        ensure!(vertex_count <= 65_536, "ArtMesh has too many vertices");
        let mut positions = vec![[0.0_f32; 2]; vertex_count];
        let mut opacity = 0.0_f32;
        let mut draw_order = 0.0_f32;
        let coordinate_count = self.nonnegative(0, 10)?;
        for key in weights {
            ensure!(
                key.index < keyform_count,
                "Binding exceeds ArtMesh keyforms"
            );
            let frame = keyform_start + key.index;
            let start = self.nonnegative(70, frame)?;
            ensure!(
                vertex_count
                    .checked_mul(2)
                    .and_then(|coordinates| start.checked_add(coordinates))
                    .is_some_and(|end| end <= coordinate_count),
                "ArtMesh key positions exceed the position pool"
            );
            let frame_opacity = f32::from_bits(self.word(68, frame)?);
            let frame_order = f32::from_bits(self.word(69, frame)?);
            ensure!(
                frame_opacity.is_finite() && frame_order.is_finite(),
                "Invalid ArtMesh key opacity or order"
            );
            opacity += frame_opacity * key.weight;
            draw_order += frame_order * key.weight;
            for (vertex, out) in positions.iter_mut().enumerate() {
                let point = start + vertex * 2;
                let x = f32::from_bits(self.word(71, point)?);
                let y = f32::from_bits(self.word(71, point + 1)?);
                ensure!(
                    x.is_finite() && y.is_finite(),
                    "Invalid ArtMesh key position"
                );
                out[0] += x * key.weight;
                out[1] += y * key.weight;
            }
        }
        let parent = self.word(40, mesh)? as i32;
        ensure!(
            parent == -1 || (parent >= 0 && parent < counts.deformers as i32),
            "Invalid ArtMesh parent deformer"
        );
        Ok(LocalMeshFrame {
            positions,
            opacity,
            draw_order,
            multiply: [1.0; 4],
            screen: [0.0, 0.0, 0.0, 1.0],
            parent_deformer: (parent >= 0).then_some(parent as usize),
        })
    }

    pub fn deformer_layouts(&self) -> Result<Vec<DeformerLayout>> {
        let counts = self.counts()?;
        let mut layouts = Vec::with_capacity(counts.deformers as usize);
        let binding_total = self.nonnegative(0, 12)?;
        for index in 0..counts.deformers as usize {
            let parent_part = self.word(15, index)? as i32;
            let parent_deformer = self.word(16, index)? as i32;
            ensure!(
                parent_part == -1 || (parent_part >= 0 && parent_part < counts.parts as i32),
                "Deformer has an invalid parent part"
            );
            ensure!(
                parent_deformer == -1 || (parent_deformer >= 0 && parent_deformer < index as i32),
                "Deformer hierarchy is not topologically ordered"
            );
            let local = self.nonnegative(18, index)?;
            let (binding, keyform_start, keyform_count, kind, total) = match self.word(17, index)? {
                0 => {
                    ensure!(local < counts.warp_deformers as usize, "Invalid warp index");
                    let columns = self.nonnegative(24, local)?;
                    let rows = self.nonnegative(23, local)?;
                    let points = self.nonnegative(22, local)?;
                    ensure!(
                        (1..=256).contains(&columns)
                            && (1..=256).contains(&rows)
                            && (columns + 1) * (rows + 1) == points,
                        "Invalid warp control grid"
                    );
                    let quad = self.version >= 2 && self.word(101, local)? != 0;
                    (
                        self.nonnegative(19, local)?,
                        self.nonnegative(20, local)?,
                        self.nonnegative(21, local)?,
                        DeformerKind::Warp {
                            columns,
                            rows,
                            points,
                            quad,
                        },
                        self.nonnegative(0, 7)?,
                    )
                }
                1 => {
                    ensure!(
                        local < counts.rotation_deformers as usize,
                        "Invalid rotation index"
                    );
                    let base_angle = f32::from_bits(self.word(28, local)?);
                    ensure!(base_angle.is_finite(), "Invalid rotation base angle");
                    (
                        self.nonnegative(25, local)?,
                        self.nonnegative(26, local)?,
                        self.nonnegative(27, local)?,
                        DeformerKind::Rotation { base_angle },
                        self.nonnegative(0, 8)?,
                    )
                }
                _ => anyhow::bail!("Unknown MOC3 deformer type"),
            };
            ensure!(binding < binding_total, "Deformer binding is invalid");
            ensure!(
                keyform_count > 0
                    && keyform_start
                        .checked_add(keyform_count)
                        .is_some_and(|end| end <= total),
                "Deformer keyform range is invalid"
            );
            layouts.push(DeformerLayout {
                id: self.id(11, index)?,
                parent_part: (parent_part >= 0).then_some(parent_part as usize),
                parent_deformer: (parent_deformer >= 0).then_some(parent_deformer as usize),
                binding,
                local_index: local,
                keyform_start,
                keyform_count,
                enabled: self.word(14, index)? != 0,
                visible: self.word(13, index)? != 0,
                kind,
            });
        }
        Ok(layouts)
    }

    /// Mark meshes whose final vertices also depend on glue or blend-shape
    /// geometry. These require additional evaluation after warp deformation.
    pub fn secondary_meshes(&self) -> Result<Vec<bool>> {
        let mesh_count = self.counts()?.art_meshes as usize;
        let mut secondary = self.blend_shape_meshes()?;
        let glues = self.nonnegative(0, 20)?;
        ensure!(glues <= 1_000_000, "Too many glue relationships");
        for glue in 0..glues {
            for section in [94, 95] {
                let mesh = self.nonnegative(section, glue)?;
                ensure!(mesh < mesh_count, "Glue references an invalid ArtMesh");
                secondary[mesh] = true;
            }
        }
        Ok(secondary)
    }

    pub fn blend_shape_meshes(&self) -> Result<Vec<bool>> {
        let mesh_count = self.counts()?.art_meshes as usize;
        let mut affected = vec![false; mesh_count];
        if self.version >= 4 {
            let blend_meshes = self.nonnegative(0, 28)?;
            ensure!(blend_meshes <= 1_000_000, "Too many ArtMesh blend shapes");
            for shape in 0..blend_meshes {
                let mesh = self.nonnegative(128, shape)?;
                ensure!(mesh < mesh_count, "Blend shape has an invalid ArtMesh");
                affected[mesh] = true;
            }
        }
        Ok(affected)
    }

    pub fn blend_rotation_targets(&self) -> Result<Vec<usize>> {
        if self.version < 5 {
            return Ok(Vec::new());
        }
        let count = self.nonnegative(0, 33)?;
        ensure!(count <= 1_000_000, "Too many rotation blend shapes");
        let limit = self.counts()?.rotation_deformers as usize;
        (0..count)
            .map(|index| {
                let target = self.nonnegative(146, index)?;
                ensure!(target < limit, "Blend shape has an invalid rotation");
                Ok(target)
            })
            .collect()
    }

    pub fn blend_glue_targets(&self) -> Result<Vec<usize>> {
        if self.version < 5 {
            return Ok(Vec::new());
        }
        let count = self.nonnegative(0, 34)?;
        ensure!(count <= 1_000_000, "Too many glue blend shapes");
        let limit = self.nonnegative(0, 20)?;
        (0..count)
            .map(|index| {
                let target = self.nonnegative(149, index)?;
                ensure!(target < limit, "Blend shape has an invalid glue");
                Ok(target)
            })
            .collect()
    }

    pub fn glue_layouts(&self) -> Result<Vec<GlueLayout>> {
        let counts = self.counts()?;
        let glue_count = self.nonnegative(0, 20)?;
        let info_count = self.nonnegative(0, 21)?;
        let keyform_count = self.nonnegative(0, 22)?;
        let binding_count = self.nonnegative(0, 12)?;
        ensure!(
            glue_count <= 1_000_000 && info_count <= 4_000_000,
            "Glue data exceeds ARIA limits"
        );
        let mut layouts = Vec::with_capacity(glue_count);
        for glue in 0..glue_count {
            let left_mesh = self.nonnegative(94, glue)?;
            let right_mesh = self.nonnegative(95, glue)?;
            ensure!(
                left_mesh < counts.art_meshes as usize
                    && right_mesh < counts.art_meshes as usize
                    && left_mesh != right_mesh,
                "Glue references invalid or identical ArtMeshes"
            );
            let binding = self.nonnegative(91, glue)?;
            ensure!(binding < binding_count, "Glue binding is invalid");
            let key_start = self.nonnegative(92, glue)?;
            let key_len = self.nonnegative(93, glue)?;
            ensure!(
                key_len > 0
                    && key_start
                        .checked_add(key_len)
                        .is_some_and(|end| end <= keyform_count),
                "Glue keyform range is invalid"
            );
            let mut intensities = Vec::with_capacity(key_len);
            for index in key_start..key_start + key_len {
                let value = f32::from_bits(self.word(100, index)?);
                ensure!(value.is_finite(), "Glue intensity is non-finite");
                intensities.push(value);
            }
            let info_start = self.nonnegative(96, glue)?;
            let info_len = self.nonnegative(97, glue)?;
            ensure!(
                info_len % 2 == 0
                    && info_start
                        .checked_add(info_len)
                        .is_some_and(|end| end <= info_count),
                "Glue vertex-pair range is invalid"
            );
            let left_vertices = self.nonnegative(43, left_mesh)?;
            let right_vertices = self.nonnegative(43, right_mesh)?;
            let mut pairs = Vec::with_capacity(info_len / 2);
            for info in (info_start..info_start + info_len).step_by(2) {
                let left = self.short(99, info)? as usize;
                let right = self.short(99, info + 1)? as usize;
                let left_weight = f32::from_bits(self.word(98, info)?);
                let right_weight = f32::from_bits(self.word(98, info + 1)?);
                ensure!(
                    left < left_vertices
                        && right < right_vertices
                        && left_weight.is_finite()
                        && right_weight.is_finite(),
                    "Glue vertex pair is invalid"
                );
                pairs.push(GlueVertexPair {
                    left,
                    right,
                    left_weight,
                    right_weight,
                });
            }
            layouts.push(GlueLayout {
                left_mesh,
                right_mesh,
                binding,
                intensities,
                pairs,
            });
        }
        Ok(layouts)
    }

    /// Blend local deformer keyforms before hierarchy transforms and blend
    /// shapes. All position and scalar sources remain in model coordinates.
    pub fn local_deformer_frame(
        &self,
        layout: &DeformerLayout,
        graph: &BindingGraph,
        values: &[f32],
    ) -> Result<LocalDeformerFrame> {
        let weights = graph.weights(layout.binding, values)?;
        self.local_deformer_frame_weighted(layout, &weights)
    }

    pub fn deformer_keyform_position_bytes(&self, layout: &DeformerLayout) -> Result<usize> {
        match layout.kind {
            DeformerKind::Warp { points, .. } => layout
                .keyform_count
                .checked_mul(points)
                .and_then(|count| count.checked_mul(std::mem::size_of::<[f32; 2]>()))
                .context("Warp keyform RAM size overflow"),
            DeformerKind::Rotation { .. } => Ok(0),
        }
    }

    pub fn compile_deformer(&self, layout: &DeformerLayout) -> Result<CompiledDeformer> {
        ensure!(
            self.deformer_keyform_position_bytes(layout)? <= 512 * 1024 * 1024,
            "Warp keyforms exceed the 512 MiB RAM budget"
        );
        let mut keyforms = Vec::with_capacity(layout.keyform_count);
        for index in 0..layout.keyform_count {
            keyforms.push(
                self.local_deformer_frame_weighted(
                    layout,
                    &[KeyformWeight { index, weight: 1.0 }],
                )?,
            );
        }
        Ok(CompiledDeformer { keyforms })
    }

    pub fn local_deformer_frame_weighted(
        &self,
        layout: &DeformerLayout,
        weights: &[KeyformWeight],
    ) -> Result<LocalDeformerFrame> {
        match layout.kind {
            DeformerKind::Warp { points, .. } => {
                let coordinate_total = self.nonnegative(0, 10)?;
                let mut output = vec![[0.0_f32; 2]; points];
                let mut opacity = 0.0_f32;
                for key in weights {
                    ensure!(
                        key.index < layout.keyform_count,
                        "Warp binding exceeds keyforms"
                    );
                    let frame = layout.keyform_start + key.index;
                    let start = self.nonnegative(60, frame)?;
                    ensure!(
                        points
                            .checked_mul(2)
                            .and_then(|coordinates| start.checked_add(coordinates))
                            .is_some_and(|end| end <= coordinate_total),
                        "Warp key positions exceed the position pool"
                    );
                    let source_opacity = f32::from_bits(self.word(59, frame)?);
                    ensure!(source_opacity.is_finite(), "Invalid warp opacity");
                    opacity += source_opacity * key.weight;
                    for (vertex, out) in output.iter_mut().enumerate() {
                        let offset = start + vertex * 2;
                        let x = f32::from_bits(self.word(71, offset)?);
                        let y = f32::from_bits(self.word(71, offset + 1)?);
                        ensure!(x.is_finite() && y.is_finite(), "Invalid warp position");
                        out[0] += x * key.weight;
                        out[1] += y * key.weight;
                    }
                }
                Ok(LocalDeformerFrame::Warp {
                    points: output,
                    opacity,
                })
            }
            DeformerKind::Rotation { .. } => {
                let mut origin = [0.0_f32; 2];
                let mut angle = 0.0_f32;
                let mut scale = 0.0_f32;
                let mut opacity = 0.0_f32;
                let mut dominant = (f32::NEG_INFINITY, [false; 2]);
                for key in weights {
                    ensure!(
                        key.index < layout.keyform_count,
                        "Rotation binding exceeds keyforms"
                    );
                    let frame = layout.keyform_start + key.index;
                    let values = [
                        f32::from_bits(self.word(63, frame)?),
                        f32::from_bits(self.word(64, frame)?),
                        f32::from_bits(self.word(62, frame)?),
                        f32::from_bits(self.word(65, frame)?),
                        f32::from_bits(self.word(61, frame)?),
                    ];
                    ensure!(
                        values.iter().all(|v| v.is_finite()),
                        "Invalid rotation keyform"
                    );
                    origin[0] += values[0] * key.weight;
                    origin[1] += values[1] * key.weight;
                    angle += values[2] * key.weight;
                    scale += values[3] * key.weight;
                    opacity += values[4] * key.weight;
                    if key.weight > dominant.0 {
                        dominant = (
                            key.weight,
                            [self.word(66, frame)? != 0, self.word(67, frame)? != 0],
                        );
                    }
                }
                Ok(LocalDeformerFrame::Rotation {
                    origin,
                    angle,
                    scale,
                    opacity,
                    reflect: dominant.1,
                })
            }
        }
    }

    /// Evaluate an ArtMesh with one root warp parent. Returns `None` for
    /// hierarchy cases that are not supported by this narrow evaluator yet.
    /// Blend shapes and glues are not applied.
    pub fn single_warp_mesh_frame(
        &self,
        mesh: usize,
        graph: &BindingGraph,
        values: &[f32],
        deformers: &[DeformerLayout],
    ) -> Result<Option<LocalMeshFrame>> {
        let mut frame = self.local_mesh_frame(mesh, graph, values)?;
        let Some(parent_index) = frame.parent_deformer else {
            return Ok(None);
        };
        let parent = deformers
            .get(parent_index)
            .context("Missing mesh parent deformer")?;
        let DeformerKind::Warp {
            columns,
            rows,
            quad,
            ..
        } = parent.kind
        else {
            return Ok(None);
        };
        if parent.parent_deformer.is_some() || !parent.enabled {
            return Ok(None);
        }
        let LocalDeformerFrame::Warp { points, opacity } =
            self.local_deformer_frame(parent, graph, values)?
        else {
            unreachable!()
        };
        let grid = crate::rig::WarpGrid::new(
            columns,
            rows,
            points
                .into_iter()
                .map(|p| crate::rig::Point { x: p[0], y: p[1] })
                .collect(),
        )?;
        for position in &mut frame.positions {
            let point = grid.sample_extended(
                crate::rig::Point {
                    x: position[0],
                    y: position[1],
                },
                quad,
            )?;
            *position = [point.x, point.y];
        }
        frame.opacity *= opacity;
        Ok(Some(frame))
    }

    /// Evaluate a mesh under one root rotation. This deliberately excludes
    /// secondary geometry and inherited transforms until those paths have
    /// independent parity coverage.
    pub fn root_rotation_mesh_frame(
        &self,
        mesh: usize,
        graph: &BindingGraph,
        values: &[f32],
        deformers: &[DeformerLayout],
    ) -> Result<Option<LocalMeshFrame>> {
        let mut frame = self.local_mesh_frame(mesh, graph, values)?;
        let Some(parent_index) = frame.parent_deformer else {
            return Ok(None);
        };
        let parent = deformers
            .get(parent_index)
            .context("Missing mesh parent deformer")?;
        let DeformerKind::Rotation { base_angle } = parent.kind else {
            return Ok(None);
        };
        if parent.parent_deformer.is_some() || !parent.enabled {
            return Ok(None);
        }
        let LocalDeformerFrame::Rotation {
            origin,
            angle,
            scale,
            opacity,
            reflect,
        } = self.local_deformer_frame(parent, graph, values)?
        else {
            unreachable!()
        };
        let transform = crate::rig::RotationTransform::new(
            crate::rig::Point {
                x: origin[0],
                y: origin[1],
            },
            base_angle + angle,
            scale,
            reflect,
        )?;
        for position in &mut frame.positions {
            let result = transform.apply(crate::rig::Point {
                x: position[0],
                y: position[1],
            });
            *position = [result.x, result.y];
        }
        frame.opacity *= opacity;
        Ok(Some(frame))
    }

    /// Evaluate a hierarchy made entirely of warp deformers. Rotation nodes,
    /// glues, blend shapes, part opacity, and exterior fidelity require their
    /// own parity gates before this can be used as a complete model frame.
    pub fn warp_chain_mesh_frame(
        &self,
        mesh: usize,
        graph: &BindingGraph,
        values: &[f32],
        deformers: &[DeformerLayout],
    ) -> Result<Option<LocalMeshFrame>> {
        let mut frame = self.local_mesh_frame(mesh, graph, values)?;
        let Some(mut parent_index) = frame.parent_deformer else {
            return Ok(None);
        };
        let mut chain = Vec::new();
        loop {
            ensure!(chain.len() < deformers.len(), "Cyclic deformer hierarchy");
            let node = deformers
                .get(parent_index)
                .context("Missing mesh parent deformer")?;
            if !node.enabled || !matches!(node.kind, DeformerKind::Warp { .. }) {
                return Ok(None);
            }
            chain.push(parent_index);
            let Some(next) = node.parent_deformer else {
                break;
            };
            parent_index = next;
        }
        chain.reverse();
        let mut parent_grid: Option<(crate::rig::WarpGrid, bool)> = None;
        for index in chain {
            let node = &deformers[index];
            let DeformerKind::Warp {
                columns,
                rows,
                quad,
                ..
            } = node.kind
            else {
                unreachable!()
            };
            let LocalDeformerFrame::Warp { points, opacity } =
                self.local_deformer_frame(node, graph, values)?
            else {
                unreachable!()
            };
            let mut points = points
                .into_iter()
                .map(|p| crate::rig::Point { x: p[0], y: p[1] })
                .collect::<Vec<_>>();
            if let Some((grid, parent_quad)) = &parent_grid {
                for point in &mut points {
                    *point = grid.sample_extended(*point, *parent_quad)?;
                }
            }
            frame.opacity *= opacity;
            parent_grid = Some((crate::rig::WarpGrid::new(columns, rows, points)?, quad));
        }
        let Some((grid, quad)) = parent_grid else {
            return Ok(None);
        };
        for position in &mut frame.positions {
            let p = grid.sample_extended(
                crate::rig::Point {
                    x: position[0],
                    y: position[1],
                },
                quad,
            )?;
            *position = [p.x, p.y];
        }
        Ok(Some(frame))
    }

    /// Evaluate a mesh through any authored warp/rotation hierarchy. The
    /// caller must exclude meshes with glue or blend-shape geometry; this
    /// method currently evaluates the normal-parameter geometry only.
    pub fn deformer_chain_mesh_frame(
        &self,
        mesh: usize,
        graph: &BindingGraph,
        values: &[f32],
        deformers: &[DeformerLayout],
    ) -> Result<LocalMeshFrame> {
        enum Transform {
            Warp(crate::rig::WarpGrid, bool),
            Rotation(crate::rig::RotationTransform),
        }
        impl Transform {
            fn apply(&self, point: crate::rig::Point) -> Result<crate::rig::Point> {
                match self {
                    Self::Warp(grid, quad) => grid.sample_extended(point, *quad),
                    Self::Rotation(transform) => Ok(transform.apply(point)),
                }
            }
        }

        let mut frame = self.local_mesh_frame(mesh, graph, values)?;
        let Some(mut parent_index) = frame.parent_deformer else {
            return Ok(frame);
        };
        let mut chain = Vec::new();
        loop {
            ensure!(chain.len() < deformers.len(), "Cyclic deformer hierarchy");
            let node = deformers
                .get(parent_index)
                .context("Missing mesh parent deformer")?;
            ensure!(node.enabled, "Disabled deformer in active mesh hierarchy");
            chain.push(parent_index);
            let Some(next) = node.parent_deformer else {
                break;
            };
            parent_index = next;
        }
        chain.reverse();
        let mut parent: Option<Transform> = None;
        let mut inherited_scale = 1.0_f32;
        for index in chain {
            let node = &deformers[index];
            let local = self.local_deformer_frame(node, graph, values)?;
            let next = match (node.kind.clone(), local) {
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
                        .map(|p| crate::rig::Point { x: p[0], y: p[1] })
                        .collect::<Vec<_>>();
                    if let Some(transform) = &parent {
                        for point in &mut points {
                            *point = transform.apply(*point)?;
                        }
                    }
                    frame.opacity *= opacity;
                    Transform::Warp(crate::rig::WarpGrid::new(columns, rows, points)?, quad)
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
                    let mut origin = crate::rig::Point {
                        x: origin[0],
                        y: origin[1],
                    };
                    let mut angle = angle;
                    if let Some(transform) = &parent {
                        let delta = if matches!(transform, Transform::Rotation(_)) {
                            -10.0
                        } else {
                            -0.1
                        };
                        let transformed_origin = transform.apply(origin)?;
                        // Transport the local negative-Y direction through
                        // the parent. The direction gives an orientation even
                        // when the parent warp is not affine.
                        let mut direction = crate::rig::Point::default();
                        let mut step = delta;
                        for _ in 0..16 {
                            let probe = transform.apply(crate::rig::Point {
                                x: origin.x,
                                y: origin.y + step,
                            })?;
                            direction = crate::rig::Point {
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
                    inherited_scale *= scale;
                    frame.opacity *= opacity;
                    Transform::Rotation(crate::rig::RotationTransform::new(
                        origin,
                        base_angle + angle,
                        inherited_scale,
                        reflect,
                    )?)
                }
                _ => anyhow::bail!("Deformer layout and frame types differ"),
            };
            parent = Some(next);
        }
        if let Some(transform) = parent {
            for position in &mut frame.positions {
                let result = transform.apply(crate::rig::Point {
                    x: position[0],
                    y: position[1],
                })?;
                *position = [result.x, result.y];
            }
        }
        Ok(frame)
    }

    /// Read immutable mesh topology and UVs, independently of the evaluator.
    /// All offsets and triangle indices are checked before returning data.
    pub fn mesh_layouts(&self) -> Result<Vec<MeshLayout>> {
        let count = self.counts()?.art_meshes as usize;
        let mut meshes = Vec::with_capacity(count);
        let mut total_vertices = 0_usize;
        let mut total_indices = 0_usize;
        for i in 0..count {
            let id = self.id(33, i)?;

            let texture = self.nonnegative(41, i)?;
            ensure!(texture < 32, "MOC3 texture index exceeds ARIA limits");
            let vertices = self.nonnegative(43, i)?;
            let uv_start = self.nonnegative(44, i)?;
            let index_start = self.nonnegative(45, i)?;
            let index_count = self.nonnegative(46, i)?;
            let mask_start = self.nonnegative(47, i)?;
            let mask_count = self.nonnegative(48, i)?;
            ensure!(
                vertices <= 65536
                    && index_count <= 1_000_000
                    && index_count % 3 == 0
                    && mask_count <= count,
                "MOC3 mesh exceeds ARIA geometry limits"
            );
            total_vertices = total_vertices
                .checked_add(vertices)
                .context("MOC3 vertex count overflow")?;
            total_indices = total_indices
                .checked_add(index_count)
                .context("MOC3 index count overflow")?;
            ensure!(
                total_vertices <= 2_000_000 && total_indices <= 12_000_000,
                "MOC3 model exceeds ARIA geometry limits"
            );

            let mut uvs = Vec::with_capacity(vertices);
            for vertex in 0..vertices {
                let offset = uv_start
                    .checked_add(vertex.checked_mul(2).context("MOC3 UV offset overflow")?)
                    .context("MOC3 UV offset overflow")?;
                let x = f32::from_bits(self.word(78, offset)?);
                let y = f32::from_bits(self.word(78, offset + 1)?);
                ensure!(x.is_finite() && y.is_finite(), "Invalid MOC3 UV");
                // MOC3 stores atlas V from the opposite edge of ARIA's
                // renderer-facing coordinates.
                uvs.push([x, 1.0 - y]);
            }
            let mut triangles = Vec::with_capacity(index_count);
            for index in index_start..index_start + index_count {
                let vertex = self.short(79, index)?;
                ensure!(
                    usize::from(vertex) < vertices,
                    "MOC3 triangle index is invalid"
                );
                triangles.push(vertex);
            }
            // Reversing the V axis also reverses triangle winding in the
            // renderer-facing coordinate system.
            for triangle in triangles.as_chunks_mut::<3>().0 {
                triangle.swap(0, 2);
            }
            let mut masks = Vec::with_capacity(mask_count);
            for index in mask_start..mask_start + mask_count {
                let mesh = self.word(80, index)? as i32;
                if mesh >= 0 {
                    ensure!((mesh as usize) < count, "MOC3 mask index is invalid");
                    masks.push(mesh as u32);
                }
            }
            meshes.push(MeshLayout {
                id,
                texture: texture as u8,
                constant_flags: *self
                    .section(42)
                    .context("Missing ArtMesh flags")?
                    .get(i)
                    .context("Truncated ArtMesh flags")?,
                parent_part: self.mesh_parent_part(i)?,
                parent_deformer: self.mesh_parent_deformer(i)?,
                enabled: self.mesh_enabled(i)?,
                default_visible: self.word(37, i)? != 0,
                uvs,
                triangles,
                masks,
            });
        }
        Ok(meshes)
    }

    pub fn counts(&self) -> Result<ModelCounts> {
        let counts = ModelCounts {
            parts: self.word(0, 0)?,
            deformers: self.word(0, 1)?,
            warp_deformers: self.word(0, 2)?,
            rotation_deformers: self.word(0, 3)?,
            art_meshes: self.word(0, 4)?,
            parameters: self.word(0, 5)?,
        };
        ensure!(
            counts.parts <= 8192
                && counts.deformers <= 8192
                && counts.art_meshes <= 8192
                && counts.parameters <= 8192,
            "MOC3 model count exceeds ARIA limits"
        );
        ensure!(
            counts.warp_deformers.checked_add(counts.rotation_deformers) == Some(counts.deformers),
            "MOC3 deformer counts disagree"
        );
        Ok(counts)
    }

    pub fn canvas(&self) -> Result<Canvas> {
        let ppu = f32::from_bits(self.word(1, 0)?);
        let x = f32::from_bits(self.word(1, 1)?);
        let y = f32::from_bits(self.word(1, 2)?);
        let width = f32::from_bits(self.word(1, 3)?);
        let height = f32::from_bits(self.word(1, 4)?);
        ensure!(
            [ppu, x, y, width, height].iter().all(|v| v.is_finite())
                && ppu > 0.0
                && width > 0.0
                && height > 0.0,
            "Invalid MOC3 canvas"
        );
        let flags = self
            .section(1)
            .context("Missing MOC3 canvas")?
            .get(20)
            .copied()
            .context("Truncated MOC3 canvas")?;
        Ok(Canvas {
            pixels_per_unit: ppu,
            origin: [x, y],
            size: [width, height],
            reverse_y: flags & 1 != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal(version: u8) -> Vec<u8> {
        let count = if version == 6 {
            EXTENDED_SECTIONS
        } else {
            LEGACY_SECTIONS
        };
        let mut bytes = vec![0; HEADER_BYTES + count * 4 + 8];
        bytes[..4].copy_from_slice(b"MOC3");
        bytes[4] = version;
        bytes[HEADER_BYTES..HEADER_BYTES + 4]
            .copy_from_slice(&((HEADER_BYTES + count * 4) as u32).to_le_bytes());
        bytes
    }

    #[test]
    fn parses_both_table_sizes_without_mutating_input() {
        for (version, count) in [(5, LEGACY_SECTIONS), (6, EXTENDED_SECTIONS)] {
            let bytes = minimal(version);
            let original = bytes.clone();
            let moc = Moc::parse(&bytes).unwrap();
            assert_eq!(moc.section_count(), count);
            assert_eq!(moc.section(0).unwrap().len(), 8);
            assert!(moc.section(1).is_none());
            assert_eq!(bytes, original);
        }
    }

    #[test]
    fn rejects_truncated_and_out_of_bounds_offsets() {
        assert!(Moc::parse(b"MOC3").is_err());
        let mut bytes = minimal(5);
        bytes[HEADER_BYTES..HEADER_BYTES + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Moc::parse(&bytes).is_err());
    }

    #[test]
    fn mixed_radix_binding_weights_cover_interpolated_keyforms() {
        let graph = BindingGraph {
            tables: vec![
                KeyTable {
                    parameter: 0,
                    keys: vec![-1.0, 0.0, 1.0],
                },
                KeyTable {
                    parameter: 1,
                    keys: vec![0.0, 10.0],
                },
            ],
            bindings: vec![vec![0, 1]],
        };
        assert_eq!(
            graph.weights(0, &[0.5, 5.0]).unwrap(),
            vec![
                KeyformWeight {
                    index: 1,
                    weight: 0.25
                },
                KeyformWeight {
                    index: 2,
                    weight: 0.25
                },
                KeyformWeight {
                    index: 4,
                    weight: 0.25
                },
                KeyformWeight {
                    index: 5,
                    weight: 0.25
                },
            ]
        );
        assert_eq!(
            graph.weights(0, &[1.0, 10.0]).unwrap(),
            vec![KeyformWeight {
                index: 5,
                weight: 1.0
            }]
        );
        assert!(graph.weights(0, &[0.0]).is_err());
        assert!(graph.in_range(0, &[-1.0, 10.0]).unwrap());
        assert!(graph.in_range(0, &[1.0, 0.0]).unwrap());
        assert!(!graph.in_range(0, &[1.1, 0.0]).unwrap());
        assert!(!graph.in_range(0, &[0.0, -0.1]).unwrap());
    }

    #[test]
    fn integral_draw_order_snaps_only_float_roundoff() {
        let mut frame = LocalMeshFrame {
            positions: Vec::new(),
            opacity: 1.0,
            draw_order: 499.99997,
            multiply: [1.0; 4],
            screen: [0.0; 4],
            parent_deformer: None,
        };
        assert_eq!(frame.integer_draw_order(), 500);
        frame.draw_order = 508.75;
        assert_eq!(frame.integer_draw_order(), 508);
    }

    #[test]
    fn blend_weights_skip_base_key_and_honor_constraints() {
        let graph = BlendGraph {
            tables: vec![BlendKeyTable {
                parameter: 0,
                keys: vec![0.0, 0.5, 1.0],
                base_key: 0,
            }],
            bindings: vec![BlendBinding {
                table: 0,
                source_start: 4,
                source_len: 3,
                constraints: vec![BlendConstraint {
                    parameter: 1,
                    keys: vec![0.0, 1.0],
                    weights: vec![0.0, 1.0],
                }],
            }],
            art_meshes: Vec::new(),
            warps: Vec::new(),
            rotations: Vec::new(),
            glues: Vec::new(),
        };
        assert!(graph.weights(0, &[0.0, 1.0]).unwrap().is_empty());
        assert_eq!(
            graph.weights(0, &[0.75, 0.5]).unwrap(),
            vec![
                KeyformWeight {
                    index: 5,
                    weight: 0.25
                },
                KeyformWeight {
                    index: 6,
                    weight: 0.25
                },
            ]
        );
        assert!(graph.weights(0, &[1.0, 0.0]).unwrap()[0].weight == 0.0);
        assert!(graph.weights(0, &[f32::NAN, 0.5]).is_err());
    }

    #[test]
    fn parameter_resolution_clamps_and_wraps_finite_values() {
        let mut parameter = ParameterSpec {
            id: "ParamTest".into(),
            minimum: -1.0,
            maximum: 1.0,
            default: 0.0,
            repeat: false,
            kind: ParameterKind::Normal,
        };
        assert_eq!(parameter.resolve(-2.0).unwrap(), -1.0);
        assert_eq!(parameter.resolve(2.0).unwrap(), 1.0);
        assert!(parameter.resolve(f32::NAN).is_err());
        parameter.repeat = true;
        assert_eq!(parameter.resolve(1.0).unwrap(), -1.0);
        assert_eq!(parameter.resolve(2.5).unwrap(), 0.5);
        assert_eq!(parameter.resolve(-2.5).unwrap(), -0.5);
    }

    #[test]
    fn accepts_local_model_headers_when_explicitly_supplied() {
        let Ok(paths) = std::env::var("ARIA_TEST_MOC") else {
            return;
        };
        for path in std::env::split_paths(&paths) {
            let bytes = std::fs::read(&path).unwrap();
            let moc = Moc::parse(&bytes).unwrap();
            assert!((1..=6).contains(&moc.version()));
            assert!(moc.section(0).is_some());
            let counts = moc.counts().unwrap();
            assert!(counts.art_meshes > 0);
            assert!(moc.canvas().is_ok());
            let meshes = moc.mesh_layouts().unwrap();
            assert_eq!(meshes.len(), counts.art_meshes as usize);
            assert_eq!(moc.parameters().unwrap().len(), counts.parameters as usize);
            let graph = moc.binding_graph().unwrap();
            let blend_graph = moc.blend_graph().unwrap();
            moc.blend_rotation_targets().unwrap();
            moc.blend_glue_targets().unwrap();
            let deformers = moc.deformer_layouts().unwrap();
            assert_eq!(deformers.len(), counts.deformers as usize);
            assert_eq!(moc.secondary_meshes().unwrap().len(), meshes.len());
            assert_eq!(
                moc.glue_layouts().unwrap().len(),
                moc.nonnegative(0, 20).unwrap()
            );
            let values = moc
                .parameters()
                .unwrap()
                .iter()
                .map(|parameter| parameter.default)
                .collect::<Vec<_>>();
            for binding in 0..graph.bindings.len() {
                let weights = graph.weights(binding, &values).unwrap();
                assert!(weights.iter().all(|item| item.weight.is_finite()));
                assert!((weights.iter().map(|item| item.weight).sum::<f32>() - 1.0).abs() < 1e-4);
            }
            for binding in 0..blend_graph.bindings.len() {
                let weights = blend_graph.weights(binding, &values).unwrap();
                assert!(weights.iter().all(|item| item.weight.is_finite()));
            }
            for target in &blend_graph.art_meshes {
                let compiled = moc.compile_blend_mesh(target, &blend_graph).unwrap();
                let mut frame = moc
                    .local_mesh_frame(target.target, &graph, &values)
                    .unwrap();
                compiled.apply(&mut frame, &blend_graph, &values).unwrap();
                assert!(frame.positions.iter().flatten().all(|v| v.is_finite()));
            }
            for target in &blend_graph.warps {
                let compiled = moc.compile_blend_warp(target, &blend_graph).unwrap();
                assert_eq!(compiled.target, target.target);
            }
            for (mesh, layout) in meshes.iter().enumerate().take(8) {
                let frame = moc.local_mesh_frame(mesh, &graph, &values).unwrap();
                assert_eq!(frame.positions.len(), layout.uvs.len());
                assert!(frame.positions.iter().flatten().all(|v| v.is_finite()));
            }
            for layout in deformers.iter().take(8) {
                match moc.local_deformer_frame(layout, &graph, &values).unwrap() {
                    LocalDeformerFrame::Warp { points, opacity } => {
                        assert!(points.iter().flatten().all(|v| v.is_finite()));
                        assert!(opacity.is_finite());
                    }
                    LocalDeformerFrame::Rotation {
                        origin,
                        angle,
                        scale,
                        opacity,
                        ..
                    } => {
                        assert!(
                            origin
                                .into_iter()
                                .chain([angle, scale, opacity])
                                .all(f32::is_finite)
                        );
                    }
                }
            }
            let evaluator = crate::geometry::GeometryEvaluator::new(&moc).unwrap();
            drop(moc);
            drop(bytes);
            let frames = evaluator.frame(&values).unwrap();
            assert_eq!(frames.len(), meshes.len());
            for (frame, layout) in frames.iter().zip(&meshes) {
                if let Some(frame) = frame {
                    assert_eq!(frame.positions.len(), layout.uvs.len());
                    assert!(frame.positions.iter().flatten().all(|v| v.is_finite()));
                    assert!(frame.opacity.is_finite());
                }
            }
        }
    }
}
