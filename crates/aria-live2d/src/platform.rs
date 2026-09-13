//! Resolve the official library for this process's OS/architecture. This is not emulation.
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// The official Native SDK page supplies all desktop Core libraries behind its license form.
pub const DOWNLOAD_URL: &str = "https://www.live2d.com/en/sdk/download/native/";
pub const LIBRARY_LIST_URL: &str =
    "https://github.com/Live2D/CubismNativeSamples/blob/develop/Core/README.md#library-list";

pub fn download_label() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Get the Native SDK for Windows (.dll)",
        "macos" => "Get the Native SDK for macOS (.dylib)",
        _ => "Get the Native SDK for Linux (.so)",
    }
}

pub fn download_hint() -> &'static str {
    "The official download page supplies the Native SDK for all platforms. Complete its license form, extract the SDK, then choose its folder here; ARIA selects the library for this OS and architecture."
}

pub fn extensions() -> &'static [&'static str] {
    match std::env::consts::OS {
        "windows" => &["dll"],
        "macos" => &["dylib", "bundle"],
        _ => &["so"],
    }
}
pub fn guidance() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => {
            "Choose the official Core/dll/windows/x86_64/Live2DCubismCore.dll or the extracted Native SDK folder."
        }
        ("macos", "aarch64") => {
            "For Apple Silicon, choose Core/dll/macos/arm64/libLive2DCubismCore.dylib or the extracted SDK folder. Older SDKs may contain a universal macOS library."
        }
        ("macos", "x86_64") => {
            "For Intel Mac, choose Core/dll/macos/x86_64/libLive2DCubismCore.dylib or the extracted SDK folder. Older SDKs may contain a universal macOS library."
        }
        ("linux", "x86_64") => {
            "Choose the official Core/dll/linux/x86_64/libLive2DCubismCore.so or the extracted Native SDK folder. A Windows DLL cannot run natively on Linux."
        }
        ("linux", "aarch64") => {
            "For ARM64 Linux, the official SDK lists an experimental Core/dll/experimental/linux/ARM64/libLive2DCubismCore.so. Choose the extracted SDK folder; ARIA hardware validation for this target is pending."
        }
        _ => {
            "Choose the official Core dynamic library for this operating system and architecture, or an extracted Native SDK folder. See the official library list to confirm your target is available."
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
        "linux" if arch == "arm64" => vec![
            "experimental/linux/ARM64/libLive2DCubismCore.so".into(),
            "linux/arm64/libLive2DCubismCore.so".into(),
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
            ("linux", "aarch64"),
            ("macos", "aarch64"),
            ("macos", "x86_64"),
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
