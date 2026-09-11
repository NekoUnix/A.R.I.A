//! Serializable Windows shortcut combinations shared by the UI and hotkey worker.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(default)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Windows virtual key, independent of the localized display name.
    pub key: u16,
}
impl Shortcut {
    pub fn preset(n: u8) -> Self {
        Self {
            ctrl: true,
            alt: true,
            key: 0x6f + u16::from(n),
            ..Default::default()
        }
    }
    pub fn pose() -> Self {
        Self {
            ctrl: true,
            alt: true,
            key: 0x50,
            ..Default::default()
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            Self::keys().iter().any(|(key, _)| *key == self.key),
            "Select a supported shortcut key (F12 is reserved by Windows)"
        );
        Ok(())
    }
    pub fn keys() -> Vec<(u16, String)> {
        let mut keys = Vec::new();
        for key in 0x41..=0x5a {
            keys.push((key, char::from_u32(u32::from(key)).unwrap().to_string()));
        }
        for key in 0x30..=0x39 {
            keys.push((key, char::from_u32(u32::from(key)).unwrap().to_string()));
        }
        for n in (1..=24).filter(|n| *n != 12) {
            keys.push((0x6f + n, format!("F{n}")));
        }
        for n in 0..=9 {
            keys.push((0x60 + n, format!("Numpad {n}")));
        }
        for (key, label) in [
            (0x20, "Space"),
            (0x0d, "Enter"),
            (0x09, "Tab"),
            (0x1b, "Escape"),
            (0x08, "Backspace"),
            (0x2d, "Insert"),
            (0x2e, "Delete"),
            (0x24, "Home"),
            (0x23, "End"),
            (0x21, "Page Up"),
            (0x22, "Page Down"),
            (0x25, "Left"),
            (0x26, "Up"),
            (0x27, "Right"),
            (0x28, "Down"),
            (0x6a, "Numpad *"),
            (0x6b, "Numpad +"),
            (0x6d, "Numpad -"),
            (0x6e, "Numpad ."),
            (0x6f, "Numpad /"),
        ] {
            keys.push((key, label.into()));
        }
        keys
    }
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        for (enabled, label) in [
            (self.ctrl, "Ctrl"),
            (self.alt, "Alt"),
            (self.shift, "Shift"),
            (self.win, "Win"),
        ] {
            if enabled {
                parts.push(label.to_string());
            }
        }
        parts.push(
            Self::keys()
                .into_iter()
                .find(|(key, _)| *key == self.key)
                .map_or("Choose key".into(), |(_, name)| name),
        );
        parts.join("+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcuts_roundtrip_and_reject_reserved_or_modifier_only_keys() {
        let key = Shortcut {
            ctrl: true,
            shift: true,
            key: 0x48,
            ..Default::default()
        };
        assert_eq!(key.label(), "Ctrl+Shift+H");
        assert_eq!(
            serde_json::from_str::<Shortcut>(&serde_json::to_string(&key).unwrap()).unwrap(),
            key
        );
        assert!(Shortcut::preset(12).validate().is_err());
        assert!(Shortcut { key: 0x11, ..key }.validate().is_err());
        assert!(
            Shortcut {
                key: 0x70,
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
}
