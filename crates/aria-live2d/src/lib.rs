//! Renderer-facing data and ARIA's independent Rust MOC3 model evaluator.

pub mod host;
#[cfg(test)]
mod official_oracle;
pub mod rust_model;

use anyhow::Result;
use std::{
    ops::{Deref, DerefMut},
    path::Path,
};

pub use aria_core::rig::RigParameter as Parameter;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Drawable {
    pub id: String,
    pub part: String,
    pub positions: Vec<[f32; 2]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
    pub texture: usize,
    pub masks: Vec<usize>,
    pub masked: bool,
    pub inverted: bool,
    pub double_sided: bool,
    pub visible: bool,
    pub order: i32,
    pub opacity: f32,
    pub multiply: [f32; 4],
    pub screen: [f32; 4],
    pub blend: Blend,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Canvas {
    pub size: [f32; 2],
    pub origin: [f32; 2],
    pub pixels_per_unit: f32,
}

/// Compatibility adapter for older ARIA call sites. It contains only Rust code.
/// The path argument was used by the retired native runtime and is ignored.
pub struct CubismModel(rust_model::RustModel);

impl CubismModel {
    pub fn load(_legacy_core_path: &Path, bytes: &[u8], texture_count: usize) -> Result<Self> {
        Ok(Self(rust_model::RustModel::load(bytes, texture_count)?))
    }
}

impl Deref for CubismModel {
    type Target = rust_model::RustModel;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CubismModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_adapter_rejects_invalid_moc_without_a_native_runtime() {
        assert!(CubismModel::load(Path::new("unused"), &[0; 64], 1).is_err());
    }
}
