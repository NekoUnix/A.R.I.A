//! Platform-specific full-resolution output behind a common render-loop contract.
#[cfg(target_os = "linux")]
pub use crate::linux_output::{Bridge, Sender};
#[cfg(windows)]
pub use crate::spout::{Bridge, Sender};
#[cfg(target_os = "macos")]
pub use crate::syphon::{Bridge, Sender};

#[cfg(windows)]
pub const LABEL: &str = "Spout2";
#[cfg(target_os = "macos")]
pub const LABEL: &str = "Syphon";
#[cfg(target_os = "linux")]
pub const LABEL: &str = "ARIA Canvas";

#[cfg(windows)]
pub const GUIDE: &str = "OBS → Spout2 Capture → matching ARIA sender. Use the same GPU in both apps and Premultiplied Alpha for transparency.";
#[cfg(target_os = "macos")]
pub const GUIDE: &str = "OBS → Syphon Client → ARIA and the matching canvas. Syphon is included in the macOS app. Allow transparency; turn off alpha correction for premultiplied output.";
#[cfg(target_os = "linux")]
pub const GUIDE: &str = "Install the included ARIA Canvas OBS plugin, restart native OBS, then add ARIA Canvas (Alpha) and select this output. Local shared memory preserves alpha and full resolution. This alpha backend uses asynchronous GPU readback (up to 60 FPS), so it costs more bandwidth than Spout/Syphon. Flatpak OBS requires a separately built compatible plugin.";

pub fn send(bridge: &Bridge, sender: &mut Sender, changed: bool) -> anyhow::Result<bool> {
    #[cfg(windows)]
    {
        if changed {
            bridge.send(sender)
        } else {
            Ok(true)
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = changed;
        bridge.send(sender)
    }
    #[cfg(target_os = "linux")]
    {
        bridge.send(sender, changed)
    }
}

pub fn sender_hint(base: &str) -> String {
    if cfg!(windows) {
        base.into()
    } else {
        format!("{base} ({})", std::process::id())
    }
}

pub fn description() -> &'static str {
    if cfg!(target_os = "linux") {
        "local shared-memory output"
    } else {
        "GPU texture sharing"
    }
}
