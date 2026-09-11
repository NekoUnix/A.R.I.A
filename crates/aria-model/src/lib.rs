//! Cubism manifest inspection only. No Cubism Core binary or model renderer is bundled.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    version: u32,
    file_references: FileReferences,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FileReferences {
    moc: String,
    textures: Vec<String>,
    #[serde(default)]
    physics: Option<String>,
    #[serde(default)]
    pose: Option<String>,
    #[serde(default)]
    display_info: Option<String>,
    #[serde(default)]
    user_data: Option<String>,
    #[serde(default)]
    expressions: Vec<NamedFile>,
    #[serde(default)]
    motions: BTreeMap<String, Vec<MotionFile>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NamedFile {
    file: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MotionFile {
    file: String,
    #[serde(default)]
    sound: Option<String>,
}

#[derive(Debug)]
pub struct AssetReport {
    pub kind: &'static str,
    pub relative_path: String,
    pub bytes: Option<u64>,
    pub problem: Option<String>,
}

#[derive(Debug)]
pub struct ModelReport {
    pub manifest: PathBuf,
    pub assets: Vec<AssetReport>,
    pub texture_count: usize,
    pub expression_count: usize,
    pub motion_count: usize,
}

impl ModelReport {
    pub fn total_bytes(&self) -> u64 {
        self.assets.iter().filter_map(|a| a.bytes).sum()
    }
    pub fn problem_count(&self) -> usize {
        self.assets.iter().filter(|a| a.problem.is_some()).count()
    }
}

pub fn inspect(path: &Path) -> Result<ModelReport> {
    let manifest_path = path.canonicalize().context("Cannot open model manifest")?;
    let mut bytes = Vec::new();
    fs::File::open(&manifest_path)?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 2 * 1024 * 1024,
        "Model manifest exceeds 2 MiB"
    );
    let model: Manifest = serde_json::from_slice(&bytes).context("Invalid Cubism model3 JSON")?;
    ensure!(
        model.version == 3,
        "Expected Cubism model3 Version 3, got {}",
        model.version
    );
    ensure!(
        !model.file_references.moc.is_empty(),
        "Missing Moc file reference"
    );
    ensure!(
        !model.file_references.textures.is_empty(),
        "No textures referenced by model"
    );
    let base = manifest_path
        .parent()
        .context("Manifest has no parent directory")?;
    let files = model.file_references;
    let mut report = ModelReport {
        manifest: manifest_path.clone(),
        assets: Vec::new(),
        texture_count: files.textures.len(),
        expression_count: files.expressions.len(),
        motion_count: files.motions.values().map(Vec::len).sum(),
    };
    let mut add = |kind, relative: String| {
        let result = asset_size(base, &relative);
        report.assets.push(AssetReport {
            kind,
            relative_path: relative,
            bytes: result.as_ref().ok().copied(),
            problem: result.err().map(|e| e.to_string()),
        });
    };
    add("Moc", files.moc);
    for file in files.textures {
        add("Texture", file);
    }
    for (kind, file) in [
        ("Physics", files.physics),
        ("Pose", files.pose),
        ("Display info", files.display_info),
        ("User data", files.user_data),
    ] {
        if let Some(file) = file {
            add(kind, file);
        }
    }
    for file in files.expressions {
        add("Expression", file.file);
    }
    for motion in files.motions.into_values().flatten() {
        add("Motion", motion.file);
        if let Some(sound) = motion.sound {
            add("Sound", sound);
        }
    }
    Ok(report)
}

fn asset_size(base: &Path, reference: &str) -> Result<u64> {
    let normalized = reference.replace('\\', "/");
    let relative = Path::new(&normalized);
    ensure!(
        !reference.is_empty()
            && !reference.contains(':')
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
        "Reference must stay inside the model directory"
    );
    let resolved = base
        .join(relative)
        .canonicalize()
        .context("Referenced file is missing or inaccessible")?;
    ensure!(
        resolved.starts_with(base),
        "Reference resolves outside the model directory"
    );
    let meta = fs::metadata(resolved)?;
    ensure!(meta.is_file(), "Reference is not a file");
    ensure!(meta.len() > 0, "Referenced file is empty");
    Ok(meta.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_missing_assets_and_forbids_external_references() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("avatar.moc3"),
            b"fixture only, not a Cubism model",
        )
        .unwrap();
        fs::write(dir.path().join("test.model3.json"), br#"{
            "Version":3,"FileReferences":{"Moc":"avatar.moc3","Textures":["missing.png","../outside.png","C:/private.png"],
            "Motions":{"Idle":[{"File":"idle.motion3.json","Sound":"idle.wav"}]}}
        }"#).unwrap();
        let r = inspect(&dir.path().join("test.model3.json")).unwrap();
        assert_eq!(r.texture_count, 3);
        assert_eq!(r.motion_count, 1);
        assert_eq!(r.assets.len(), 6);
        assert_eq!(r.problem_count(), 5);
        assert!(r.assets[0].bytes.unwrap() > 0);
        assert!(r.assets[2].problem.as_ref().unwrap().contains("inside"));
    }
    #[test]
    fn rejects_wrong_version_and_empty_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.model3.json");
        for input in [
            "{}",
            r#"{"Version":2,"FileReferences":{"Moc":"x","Textures":["x"]}}"#,
        ] {
            fs::write(&path, input).unwrap();
            assert!(inspect(&path).is_err());
        }
    }
}
