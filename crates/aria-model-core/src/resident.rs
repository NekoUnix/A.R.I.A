//! Immutable model sources retained in system RAM for the lifetime of an avatar.
//! Atlases stay encoded until the renderer needs to decode and upload them.

use crate::moc::Moc;
use anyhow::{Context, Result, ensure};
use std::{fs::File, io::Read, path::Path, sync::Arc};

const MOC_LIMIT: u64 = 1280 * 1024 * 1024;
const ATLAS_LIMIT: u64 = 1280 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 10 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ResidentModel {
    moc: Arc<[u8]>,
    atlases: Vec<Arc<[u8]>>,
    source_bytes: u64,
}

impl ResidentModel {
    pub fn load(moc_path: &Path, atlas_paths: &[impl AsRef<Path>]) -> Result<Self> {
        ensure!(atlas_paths.len() <= 32, "Too many model atlases");
        let moc = read_limited(moc_path, MOC_LIMIT)?;
        let parsed = Moc::parse(&moc)?;
        let meshes = parsed.mesh_layouts()?;
        for mesh in &meshes {
            ensure!(
                usize::from(mesh.texture) < atlas_paths.len(),
                "Model references an atlas outside its texture list"
            );
        }
        let mut source_bytes = moc.len() as u64;
        let mut atlases = Vec::with_capacity(atlas_paths.len());
        for path in atlas_paths {
            let size = std::fs::metadata(path.as_ref())?.len();
            ensure!(
                source_bytes
                    .checked_add(size)
                    .is_some_and(|total| total <= TOTAL_LIMIT),
                "Model sources exceed 10 GiB RAM limit"
            );
            let bytes = read_limited(path.as_ref(), ATLAS_LIMIT)?;
            source_bytes = source_bytes
                .checked_add(bytes.len() as u64)
                .context("Model source size overflow")?;
            ensure!(
                source_bytes <= TOTAL_LIMIT,
                "Model sources exceed 10 GiB RAM limit"
            );
            atlases.push(bytes);
        }
        Ok(Self {
            moc,
            atlases,
            source_bytes,
        })
    }

    pub fn moc(&self) -> &[u8] {
        &self.moc
    }

    pub fn atlas(&self, index: usize) -> Option<&[u8]> {
        self.atlases.get(index).map(AsRef::as_ref)
    }

    pub fn atlas_count(&self) -> usize {
        self.atlases.len()
    }

    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
}

fn read_limited(path: &Path, limit: u64) -> Result<Arc<[u8]>> {
    let mut file = File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    let size = file.metadata()?.len();
    ensure!(
        size > 0 && size <= limit,
        "Model source is empty or exceeds its size limit"
    );
    let mut bytes = Vec::with_capacity(size as usize);
    file.by_ref()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("Cannot read {}", path.display()))?;
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= limit,
        "Model source changed size while loading"
    );
    Ok(bytes.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_and_empty_sources() {
        let dir = std::env::temp_dir().join(format!("aria-resident-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.moc3");
        assert!(ResidentModel::load(&missing, &[] as &[&Path]).is_err());
        let empty = dir.join("empty.moc3");
        std::fs::write(&empty, []).unwrap();
        assert!(ResidentModel::load(&empty, &[] as &[&Path]).is_err());
        std::fs::remove_file(empty).unwrap();
    }

    #[test]
    fn loaded_sources_survive_file_changes() {
        let dir =
            std::env::temp_dir().join(format!("aria-resident-survive-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let moc_path = dir.join("avatar.moc3");
        let mut moc = vec![0_u8; 64 + 160 * 4 + 24];
        moc[..4].copy_from_slice(b"MOC3");
        moc[4] = 5;
        moc[64..68].copy_from_slice(&(64_u32 + 160 * 4).to_le_bytes());
        std::fs::write(&moc_path, &moc).unwrap();
        let atlas_path = dir.join("atlas.png");
        let atlas = b"encoded atlas source";
        std::fs::write(&atlas_path, atlas).unwrap();
        let resident = ResidentModel::load(&moc_path, &[&atlas_path]).unwrap();
        std::fs::write(&moc_path, b"changed").unwrap();
        std::fs::write(&atlas_path, b"changed").unwrap();
        assert_eq!(resident.moc(), moc);
        assert_eq!(resident.atlas(0), Some(atlas.as_slice()));
        assert_eq!(resident.source_bytes(), (moc.len() + atlas.len()) as u64);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_model_sources_are_resident_when_supplied() {
        let Ok(path) = std::env::var("ARIA_TEST_MOC") else {
            return;
        };
        let Ok(atlas_paths) = std::env::var("ARIA_TEST_ATLASES") else {
            return;
        };
        let Some(path) = std::env::split_paths(&path).next() else {
            return;
        };
        let moc = std::fs::read(&path).unwrap();
        let parsed = Moc::parse(&moc).unwrap();
        let required_atlases = parsed
            .mesh_layouts()
            .unwrap()
            .iter()
            .map(|m| usize::from(m.texture) + 1)
            .max()
            .unwrap_or(0);
        let paths = std::env::split_paths(&atlas_paths).collect::<Vec<_>>();
        assert!(paths.len() >= required_atlases);
        let resident = ResidentModel::load(&path, &paths).unwrap();
        assert_eq!(resident.moc(), moc);
        assert_eq!(resident.atlas_count(), paths.len());
        for (index, atlas_path) in paths.iter().enumerate() {
            assert_eq!(
                resident.atlas(index),
                Some(std::fs::read(atlas_path).unwrap().as_slice())
            );
        }
        assert!(resident.source_bytes() >= moc.len() as u64);
    }
}
