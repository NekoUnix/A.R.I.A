//! Poll only explicitly assigned VTS keys; no keyboard hooks or recorded typing.
use aria_core::{shortcuts::Shortcut, vts::Config};
use eframe::egui;
use std::collections::BTreeSet;
#[derive(Default)]
pub struct Keys {
    down: BTreeSet<String>,
    pub buttons: BTreeSet<String>,
}
impl Keys {
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        config: &Config,
        global: bool,
    ) -> (Vec<String>, Vec<String>) {
        let focused = ctx.input(|i| i.focused);
        let typing = ctx.egui_wants_keyboard_input();
        let mut next = BTreeSet::new();
        for h in &config.actions {
            if !h.enabled
                || !config.keyboard_enabled
                || (!focused && !(h.global && global))
                || (focused && typing)
            {
                continue;
            }
            let keys = h
                .shortcut
                .map(shortcut_keys)
                .unwrap_or_else(|| h.chord.clone());
            if let Some(s) = h.shortcut
                && [(s.ctrl, 0x11), (s.shift, 0x10), (s.alt, 0x12)]
                    .iter()
                    .any(|(required, key)| down(ctx, *key) != *required)
            {
                continue;
            }
            if !keys.is_empty() && keys.iter().all(|k| down(ctx, *k)) {
                next.insert(h.id.clone());
            }
        }
        let pressed = next.difference(&self.down).cloned().collect();
        let released = self.down.difference(&next).cloned().collect();
        self.down = next;
        (pressed, released)
    }
}
fn shortcut_keys(s: Shortcut) -> Vec<u16> {
    let mut v = vec![s.key];
    for (set, key) in [
        (s.ctrl, 0x11),
        (s.alt, 0x12),
        (s.shift, 0x10),
        (s.win, 0x5b),
    ] {
        if set {
            v.push(key);
        }
    }
    v
}
#[cfg(windows)]
fn down(_ctx: &egui::Context, key: u16) -> bool {
    // SAFETY: queries only the current state of an explicitly configured virtual key.
    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(key)) < 0 }
}
#[cfg(not(windows))]
fn down(ctx: &egui::Context, key: u16) -> bool {
    ctx.input(|i| match key {
        1 => i.pointer.primary_down(),
        2 => i.pointer.secondary_down(),
        4 => i.pointer.middle_down(),
        0x10 | 0xa0 | 0xa1 => i.modifiers.shift,
        0x11 | 0xa2 | 0xa3 => i.modifiers.ctrl,
        0x12 | 0xa4 | 0xa5 => i.modifiers.alt,
        0x5b | 0x5c => i.modifiers.command,
        _ => key_name(key)
            .and_then(egui::Key::from_name)
            .is_some_and(|k| i.key_down(k)),
    })
}
#[cfg(not(windows))]
fn key_name(key: u16) -> Option<&'static str> {
    match key {
        0x30..=0x39 => {
            Some(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"][(key - 0x30) as usize])
        }
        0x60..=0x69 => {
            Some(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"][(key - 0x60) as usize])
        }
        0x41..=0x5a => Some(
            [
                "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P",
                "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
            ][(key - 0x41) as usize],
        ),
        0x70..=0x87 => Some(
            [
                "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13",
                "F14", "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
            ][(key - 0x70) as usize],
        ),
        8 => Some("Backspace"),
        9 => Some("Tab"),
        13 => Some("Enter"),
        27 => Some("Escape"),
        32 => Some("Space"),
        0x25 => Some("ArrowLeft"),
        0x26 => Some("ArrowUp"),
        0x27 => Some("ArrowRight"),
        0x28 => Some("ArrowDown"),
        0x2d => Some("Insert"),
        0x2e => Some("Delete"),
        0x24 => Some("Home"),
        0x23 => Some("End"),
        0x21 => Some("PageUp"),
        0x22 => Some("PageDown"),
        _ => None,
    }
}
