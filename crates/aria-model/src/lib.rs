//! Bounded Cubism asset discovery. Native execution lives in aria-live2d.
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
    Ok(resolve_asset(base, reference)?.metadata()?.len())
}

fn resolve_asset(base: &Path, reference: &str) -> Result<PathBuf> {
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
    let meta = fs::metadata(&resolved)?;
    ensure!(meta.is_file(), "Reference is not a file");
    ensure!(meta.len() > 0, "Referenced file is empty");
    Ok(resolved)
}

/// A complete runtime input: a moc3 and its explicitly ordered texture atlas files.
#[derive(Debug)]
pub struct ModelFiles {
    pub source: PathBuf,
    pub moc: PathBuf,
    pub textures: Vec<PathBuf>,
    pub warnings: Vec<String>,
}

/// Accept a model3.json, or locate its manifest when the user selects a moc3.
/// Never guess texture ordering from filenames. Bare moc3 files need explicit textures.
pub fn load_files(path: &Path) -> Result<ModelFiles> {
    let source = path.canonicalize().context("Cannot open avatar")?;
    if source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("moc3"))
    {
        let base = source.parent().context("Avatar has no parent folder")?;
        let mut matches = Vec::new();
        for entry in fs::read_dir(base)?.take(1025) {
            let candidate = entry?.path();
            if candidate.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(".model3.json")
            }) && let Ok(files) = load_files(&candidate)
                && files.moc == source
            {
                matches.push(files);
            }
        }
        ensure!(
            matches.len() == 1,
            "Select the matching .model3.json (found {} matching manifests). A bare .moc3 also needs its texture images in atlas index order.",
            matches.len()
        );
        return Ok(matches.remove(0));
    }
    ensure!(
        source.file_name().is_some_and(|n| n
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".model3.json")),
        "Select a .model3.json or .moc3 export"
    );
    let bytes = read_bounded(&source, 2 * 1024 * 1024)?;
    let model: Manifest = serde_json::from_slice(&bytes).context("Invalid Cubism model3 JSON")?;
    ensure!(model.version == 3, "Expected model3 Version 3");
    let base = source.parent().context("Manifest has no parent folder")?;
    let f = model.file_references;
    ensure!(
        !f.textures.is_empty() && f.textures.len() <= 32,
        "Expected 1–32 texture atlases"
    );
    let moc = resolve_asset(base, &f.moc).context("Moc asset")?;
    let textures = f
        .textures
        .iter()
        .map(|t| resolve_asset(base, t).with_context(|| format!("Texture {t}")))
        .collect::<Result<Vec<_>>>()?;
    let mut warnings = Vec::new();
    if f.physics.is_some() {
        warnings.push("Physics is not simulated in this version.".into());
    }
    if f.pose.is_some() {
        warnings.push("Pose files are not applied in this version.".into());
    }
    if !f.expressions.is_empty() || !f.motions.is_empty() {
        warnings.push(
            "Motion and expression playback are not implemented; parameter controls are available."
                .into(),
        );
    }
    Ok(ModelFiles {
        source,
        moc,
        textures,
        warnings,
    })
}

/// Explicit texture selection for a moc3 without a manifest. Order is significant.
pub fn bare_moc(path: &Path, textures: &[PathBuf]) -> Result<ModelFiles> {
    ensure!(
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("moc3")),
        "Expected a .moc3 file"
    );
    ensure!(
        !textures.is_empty() && textures.len() <= 32,
        "Select 1–32 textures in atlas index order"
    );
    let moc = path.canonicalize()?;
    Ok(ModelFiles { source: moc.clone(), moc, textures: textures.iter().map(|p| p.canonicalize().map_err(Into::into)).collect::<Result<_>>()?, warnings: vec!["Bare moc3: textures use the order shown in the import dialog; no manifest metadata loaded.".into()] })
}

pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .with_context(|| format!("Cannot open {}", path.display()))?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= limit,
        "{} is empty or exceeds {} MiB",
        path.display(),
        limit / 1024 / 1024
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moc_selection_resolves_matching_manifest_and_preserves_texture_order() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["avatar.moc3", "z.png", "a.png"] {
            fs::write(dir.path().join(name), b"fixture").unwrap();
        }
        let manifest = dir.path().join("different name.model3.json");
        fs::write(
            &manifest,
            br#"{"Version":3,"FileReferences":{"Moc":"avatar.moc3","Textures":["z.png","a.png"]}}"#,
        )
        .unwrap();
        let files = load_files(&dir.path().join("avatar.moc3")).unwrap();
        assert_eq!(files.source, manifest.canonicalize().unwrap());
        assert_eq!(files.textures[0].file_name().unwrap(), "z.png");
        fs::copy(&manifest, dir.path().join("ambiguous.model3.json")).unwrap();
        assert!(
            load_files(&dir.path().join("avatar.moc3"))
                .unwrap_err()
                .to_string()
                .contains("2 matching")
        );
    }
    #[test]
    fn runtime_import_rejects_missing_textures_and_path_escape() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("avatar.moc3"), b"fixture").unwrap();
        let path = dir.path().join("avatar.model3.json");
        for texture in ["missing.png", "../outside.png", "C:/private.png"] {
            let input = serde_json::json!({"Version":3,"FileReferences":{"Moc":"avatar.moc3","Textures":[texture]}});
            fs::write(&path, input.to_string()).unwrap();
            assert!(load_files(&path).is_err());
        }
        assert!(read_bounded(&dir.path().join("avatar.moc3"), 3).is_err());
    }
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
