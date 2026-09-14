//! Bounded Cubism asset discovery. Native execution lives in aria-live2d.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
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
    #[serde(default)]
    name: String,
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
    if path.file_name().is_some_and(|n| {
        n.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".vtube.json")
    }) {
        return inspect(&load_files(path)?.source);
    }
    let manifest_path = path.canonicalize().context("Cannot open model manifest")?;
    let mut bytes = Vec::new();
    fs::File::open(&manifest_path)?
        .take(aria_core::asset_limits::MODEL_JSON as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= aria_core::asset_limits::MODEL_JSON,
        "Model manifest exceeds 20 MiB"
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

pub fn resolve_asset(base: &Path, reference: &str) -> Result<PathBuf> {
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
    pub physics: Option<PathBuf>,
    pub tracking_profile: Option<PathBuf>,
    pub display_info: Option<PathBuf>,
    pub expressions: Vec<ExpressionFile>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ExpressionFile {
    /// Relative to the avatar folder so relocating that folder preserves settings.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}
pub fn expression_name(path: &Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let lower = name.to_ascii_lowercase();
    for suffix in [".exp3.json", ".exp3"] {
        if lower.ends_with(suffix) {
            return name[..name.len() - suffix.len()].into();
        }
    }
    name.into_owned()
}
pub fn is_expression(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    name.ends_with(".exp3.json") || name.ends_with(".exp3")
}

fn discover_expressions(
    base: &Path,
    references: Vec<NamedFile>,
    warnings: &mut Vec<String>,
) -> Vec<ExpressionFile> {
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |path: PathBuf, name: String| {
        if seen.insert(path.clone()) {
            files.push(ExpressionFile {
                id: path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/"),
                name: if name.trim().is_empty() {
                    expression_name(&path)
                } else {
                    name.chars().take(256).collect()
                },
                path,
            });
        }
    };
    if references.len() > 256 {
        warnings.push("Only the first 256 manifest expressions were inspected.".into());
    }
    for reference in references.into_iter().take(256) {
        match resolve_asset(base, &reference.file) {
            Ok(path) => add(path, reference.name),
            Err(error) => warnings.push(format!("Expression {}: {error:#}", reference.file)),
        }
    }
    // VTube Studio exports often omit Expressions from model3.json. Inspect a
    // bounded directory tree, without following links out of the avatar folder.
    let mut folders = vec![(base.to_path_buf(), 0)];
    let mut visited = BTreeSet::new();
    let mut remaining = 4096usize;
    while let Some((folder, depth)) = folders.pop() {
        let Ok(folder) = folder.canonicalize() else {
            continue;
        };
        if !folder.starts_with(base) || !visited.insert(folder.clone()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        let mut entries: Vec<_> = entries
            .take(remaining)
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        remaining -= entries.len();
        entries.sort();
        for path in entries {
            if path.is_dir() && depth < 8 {
                folders.push((path, depth + 1));
            } else if is_expression(&path) {
                let Ok(relative) = path.strip_prefix(base) else {
                    continue;
                };
                match resolve_asset(base, &relative.to_string_lossy()) {
                    Ok(path) => add(path, String::new()),
                    Err(error) => {
                        warnings.push(format!("Expression {}: {error:#}", relative.display()))
                    }
                }
            }
        }
        if remaining == 0 {
            warnings.push("Expression discovery reached 4096 directory entries. Use Import expression files for additional files.".into());
            break;
        }
    }
    files.sort_by(|a, b| a.id.cmp(&b.id));
    if files.len() > 256 {
        files.truncate(256);
        warnings.push("Maximum 256 expressions per avatar. Additional files were skipped.".into());
    }
    files
}

/// Find exports inside nested download/extraction folders, preserving their layout.
pub fn discover_models(folder: &Path) -> Result<Vec<PathBuf>> {
    let root = folder.canonicalize().context("Cannot open avatar folder")?;
    ensure!(root.is_dir(), "Choose an extracted model folder");
    let mut pending = vec![(root.clone(), 0)];
    let mut seen = BTreeSet::new();
    let mut models = BTreeSet::new();
    let mut count = 0;
    while let Some((dir, depth)) = pending.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        for entry in fs::read_dir(&dir).with_context(|| format!("Cannot read {}", dir.display()))? {
            count += 1;
            ensure!(
                count <= 20000,
                "Folder scan exceeds 20,000 entries; choose a smaller model folder"
            );
            let path = entry?.path();
            // Windows junctions and OneDrive entries are resolved before traversal.
            let resolved = path.canonicalize().with_context(|| {
                format!(
                    "Cannot access {}; make this OneDrive folder available offline",
                    path.display()
                )
            })?;
            if !resolved.starts_with(&root) {
                continue;
            }
            if resolved.is_dir() {
                ensure!(
                    depth < 16,
                    "Folder nesting exceeds 16 levels; choose a folder closer to the export"
                );
                pending.push((resolved, depth + 1));
            } else if resolved.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(".model3.json")
            }) {
                models.insert(resolved);
                ensure!(
                    models.len() <= 256,
                    "More than 256 exports found; choose a smaller folder"
                );
            }
        }
    }
    ensure!(
        !models.is_empty(),
        "No .model3.json exports found. Extract ZIP/RAR downloads first, then select the extracted folder. Keep each model's textures and sidecar files together."
    );
    Ok(models.into_iter().collect())
}

/// Accept a model3.json, a folder with one export, or locate a moc3's manifest.
/// Never guess texture ordering from filenames. Bare moc3 files need explicit textures.
pub fn load_files(path: &Path) -> Result<ModelFiles> {
    let source = path.canonicalize().context("Cannot open avatar")?;
    if source.file_name().is_some_and(|n| {
        n.to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(".vtube.json")
    }) {
        let bytes = read_bounded(&source, aria_core::asset_limits::MODEL_JSON)?;
        let document: serde_json::Value = serde_json::from_slice(&bytes)?;
        let reference = document["FileReferences"]["Model"]
            .as_str()
            .context("VTube Studio config has no model reference")?;
        ensure!(
            reference.to_ascii_lowercase().ends_with(".model3.json"),
            "VTube Studio config must reference a model3.json export"
        );
        let mut files = load_files(&resolve_asset(
            source.parent().context("Config has no folder")?,
            reference,
        )?)?;
        files.tracking_profile = Some(source);
        return Ok(files);
    }
    if source.is_dir() {
        let models = discover_models(&source)?;
        ensure!(
            models.len() == 1,
            "Found {} exports. Use the guided folder importer to choose one model.",
            models.len()
        );
        return load_files(&models[0]);
    }
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
    let bytes = read_bounded(&source, aria_core::asset_limits::MODEL_JSON)?;
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
    let mut optional = |reference: Option<&str>, kind: &str| {
        reference.and_then(|r| match resolve_asset(base, r) {
            Ok(path) => Some(path),
            Err(error) => {
                warnings.push(format!("{kind}: {error:#}"));
                None
            }
        })
    };
    let physics = optional(f.physics.as_deref(), "Physics file");
    let display_info = optional(f.display_info.as_deref(), "Parameter names");
    let sidecar = format!(
        "{}.vtube.json",
        moc.file_stem().unwrap_or_default().to_string_lossy()
    );
    let tracking_profile = if base.join(&sidecar).exists() {
        optional(Some(&sidecar), "VTS profile")
    } else {
        None
    };
    if f.pose.is_some() {
        warnings.push("Pose files are not applied in this version.".into());
    }
    let expressions = discover_expressions(base, f.expressions, &mut warnings);
    if !f.motions.is_empty() {
        warnings
            .push("Import the VTube Studio config to use its motion hotkeys and idle animations. ARIA evaluates parameter, part-opacity and standard model tracks; missing assets can be repaired in ARIA.".into());
    }
    Ok(ModelFiles {
        source,
        moc,
        textures,
        physics,
        tracking_profile,
        display_info,
        expressions,
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
    let mut warnings = vec!["Bare moc3: textures use the order shown in the import dialog; no manifest metadata loaded.".into()];
    let expressions = discover_expressions(
        moc.parent().context("Avatar has no parent folder")?,
        Vec::new(),
        &mut warnings,
    );
    Ok(ModelFiles {
        source: moc.clone(),
        moc,
        textures: textures
            .iter()
            .map(|p| p.canonicalize().map_err(Into::into))
            .collect::<Result<_>>()?,
        physics: None,
        tracking_profile: None,
        display_info: None,
        expressions,
        warnings,
    })
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
    fn nested_folders_unicode_spaces_and_ambiguous_libraries_preserve_references() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("Friend's Models/下載/yumii-20260506/yumii");
        fs::create_dir_all(nested.join("Yumii .2048")).unwrap();
        fs::write(nested.join("Yumii .moc3"), b"fixture").unwrap();
        fs::write(nested.join("Yumii .2048/texture_00.png"), b"fixture").unwrap();
        let manifest = nested.join("Yumii .model3.json");
        fs::write(&manifest, br#"{"Version":3,"FileReferences":{"Moc":"Yumii .moc3","Textures":["Yumii .2048/texture_00.png"]}}"#).unwrap();
        fs::write(dir.path().join("archive.zip"), b"not an extracted export").unwrap();
        let found = discover_models(dir.path()).unwrap();
        assert_eq!(found, vec![manifest.canonicalize().unwrap()]);
        let files = load_files(dir.path()).unwrap();
        assert_eq!(
            files.textures[0],
            nested
                .join("Yumii .2048/texture_00.png")
                .canonicalize()
                .unwrap()
        );
        fs::copy(&manifest, nested.join("alternate.model3.json")).unwrap();
        assert_eq!(discover_models(dir.path()).unwrap().len(), 2);
        assert!(
            load_files(dir.path())
                .unwrap_err()
                .to_string()
                .contains("guided")
        );
        let empty = tempfile::tempdir().unwrap();
        assert!(
            discover_models(empty.path())
                .unwrap_err()
                .to_string()
                .contains("Extract")
        );
    }
    #[test]
    fn large_manifest_loads_and_still_enforces_new_ceiling() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        for name in ["avatar.moc3", "atlas.png"] {
            fs::write(dir.path().join(name), b"fixture").unwrap();
        }
        let path = dir.path().join("avatar.model3.json");
        let json = serde_json::json!({"Version":3,"FileReferences":{"Moc":"avatar.moc3","Textures":["atlas.png"]},"Metadata":"x".repeat(3 * 1024 * 1024)});
        fs::write(&path, json.to_string()).unwrap();
        assert_eq!(load_files(&path).unwrap().textures.len(), 1);
        assert_eq!(inspect(&path).unwrap().texture_count, 1);
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&vec![b' '; aria_core::asset_limits::MODEL_JSON])
            .unwrap();
        assert!(load_files(&path).is_err());
        assert!(inspect(&path).is_err());
    }
    #[test]
    fn expressions_combine_manifest_and_unlisted_files_without_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("expressions")).unwrap();
        for name in [
            "avatar.moc3",
            "atlas.png",
            "expressions/smile.exp3.json",
            "toggle.exp3",
            "ignore.json",
        ] {
            fs::write(dir.path().join(name), b"fixture").unwrap();
        }
        let path = dir.path().join("avatar.model3.json");
        fs::write(&path, br#"{"Version":3,"FileReferences":{"Moc":"avatar.moc3","Textures":["atlas.png"],"Expressions":[{"Name":"Smile!","File":"expressions/smile.exp3.json"},{"Name":"bad","File":"../escape.exp3.json"}]}}"#).unwrap();
        let files = load_files(&path).unwrap();
        assert_eq!(files.expressions.len(), 2);
        assert_eq!(files.expressions[0].name, "Smile!");
        assert_eq!(files.expressions[0].id, "expressions/smile.exp3.json");
        assert_eq!(files.expressions[1].name, "toggle");
        assert!(files.warnings.iter().any(|w| w.contains("inside")));
    }
    #[test]
    fn discovers_optional_rig_files_and_reports_broken_physics() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "avatar.moc3",
            "atlas.png",
            "avatar.vtube.json",
            "avatar.cdi3.json",
        ] {
            fs::write(dir.path().join(name), b"fixture").unwrap();
        }
        let path = dir.path().join("avatar.model3.json");
        fs::write(&path,br#"{"Version":3,"FileReferences":{"Moc":"avatar.moc3","Textures":["atlas.png"],"Physics":"missing.physics3.json","DisplayInfo":"avatar.cdi3.json"}}"#).unwrap();
        let files = load_files(&path).unwrap();
        assert!(files.tracking_profile.is_some());
        assert!(files.display_info.is_some());
        assert!(files.physics.is_none());
        assert!(
            files
                .warnings
                .iter()
                .any(|w| w.starts_with("Physics file:"))
        );
    }
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
