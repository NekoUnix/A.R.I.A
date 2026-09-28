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
            .context("Truncated MOC3 field")?;
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
            parameters.push(ParameterSpec {
                id: self.id(50, i)?,
                minimum,
                maximum,
                default: default.clamp(minimum, maximum),
                repeat: self.word(54, i)? != 0,
            });
        }
        Ok(parameters)
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
        }
    }
}
