//! Resolve the official library for this process's OS/architecture. This is not emulation.
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub fn extensions() -> &'static [&'static str] {
    match std::env::consts::OS {
        "windows" => &["dll"],
        "macos" => &["dylib", "bundle"],
        _ => &["so"],
    }
}
pub fn guidance() -> &'static str {
    match std::env::consts::OS {
        "windows" => {
            "Choose the official Core/dll/windows/x86_64/Live2DCubismCore.dll or the extracted Native SDK folder."
        }
        "macos" => {
            "Choose the official macOS libLive2DCubismCore.dylib for your Mac, or the extracted Native SDK folder. A Windows DLL cannot run natively on macOS."
        }
        _ => {
            "Choose the official Core/dll/linux/x86_64/libLive2DCubismCore.so or the extracted Native SDK folder. A Windows DLL cannot run natively on Linux."
        }
    }
}
fn candidates(os: &str, arch: &str) -> Vec<String> {
    let arch = if arch == "aarch64" { "arm64" } else { arch };
    match os {
        "windows" => vec![format!("windows/{arch}/Live2DCubismCore.dll")],
        "macos" => vec![
            format!("macos/{arch}/libLive2DCubismCore.dylib"),
            "macos/libLive2DCubismCore.dylib".into(),
            "macos/Live2DCubismCore.bundle".into(),
        ],
        "linux" => vec![format!("linux/{arch}/libLive2DCubismCore.so")],
        _ => vec![],
    }
}
pub fn resolve(input: &Path) -> Result<PathBuf> {
    resolve_for(input, std::env::consts::OS, std::env::consts::ARCH)
}
fn resolve_for(input: &Path, os: &str, arch: &str) -> Result<PathBuf> {
    let allowed = match os {
        "windows" => &["dll"][..],
        "macos" => &["dylib", "bundle"][..],
        _ => &["so"][..],
    };
    if input.is_file()
        && input
            .extension()
            .is_some_and(|e| allowed.iter().any(|x| e.eq_ignore_ascii_case(x)))
    {
        return input
            .canonicalize()
            .context("Cannot resolve Cubism Core library");
    }
    // An extracted SDK folder or a library selected on a different OS can locate
    // its native sibling. Search only this SDK's bounded ancestor layout.
    let start = if input.is_file() {
        input.parent().unwrap_or(input)
    } else {
        input
    };
    for ancestor in start.ancestors().take(5) {
        for root in [
            ancestor.to_owned(),
            ancestor.join("Core/dll"),
            ancestor.join("dll"),
        ] {
            for relative in candidates(os, arch) {
                let candidate = root.join(relative);
                if candidate.is_file() {
                    return candidate
                        .canonicalize()
                        .context("Cannot resolve native Cubism library");
                }
            }
        }
    }
    bail!(
        "No Cubism Core library for {os}/{arch} at {}. {} The runtime host isolates native code; it does not translate Windows DLLs to another operating system.",
        input.display(),
        guidance()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sdk_selection_uses_native_sibling_and_rejects_windows_only_export() {
        let dir = tempfile::tempdir().unwrap();
        let win = dir
            .path()
            .join("Core/dll/windows/x86_64/Live2DCubismCore.dll");
        std::fs::create_dir_all(win.parent().unwrap()).unwrap();
        std::fs::write(&win, b"fixture").unwrap();
        assert!(resolve_for(&win, "linux", "x86_64").is_err());
        for (os, arch) in [
            ("linux", "x86_64"),
            ("macos", "aarch64"),
            ("windows", "x86_64"),
        ] {
            let native = dir.path().join("Core/dll").join(&candidates(os, arch)[0]);
            std::fs::create_dir_all(native.parent().unwrap()).unwrap();
            std::fs::write(&native, b"fixture").unwrap();
            assert_eq!(
                resolve_for(dir.path(), os, arch).unwrap(),
                native.canonicalize().unwrap()
            );
            if os != "windows" {
                assert_eq!(
                    resolve_for(&win, os, arch).unwrap(),
                    native.canonicalize().unwrap()
                );
            }
        }
    }
}
