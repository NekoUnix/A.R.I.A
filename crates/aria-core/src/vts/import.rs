use super::*;
use crate::{
    layers, physics,
    rig::{self, RigParameter},
};
use serde_json::Value;

pub struct Imported {
    pub config: Config,
    pub bindings: BTreeMap<String, rig::Binding>,
    pub physics: physics::PhysicsSettings,
    pub layers: layers::Config,
    pub expressions: BTreeSet<String>,
    pub saved_placement: Option<Placement>,
}
fn string(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().trim().to_owned()
}
fn boolean(v: &Value, key: &str, default: bool) -> bool {
    v[key].as_bool().unwrap_or(default)
}
fn number(v: &Value, key: &str, default: f32) -> f32 {
    v[key].as_f64().map_or(default, |n| n as f32)
}
fn array<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v[key].as_array().map_or(&[], Vec::as_slice)
}
fn note(config: &mut Config, text: impl Into<String>) {
    if config.notes.len() < 2048 {
        config.notes.push(text.into().chars().take(4096).collect());
    }
}
pub fn chord(triggers: &[&str]) -> Result<Vec<u16>> {
    let mut keys = Vec::new();
    for raw in triggers.iter().filter(|s| !s.is_empty()) {
        let key = match raw.to_ascii_lowercase().as_str() {
            "leftmousebutton" => 1,
            "rightmousebutton" => 2,
            "middlemousebutton" => 4,
            "mousebutton4" => 5,
            "mousebutton5" => 6,
            "capslock" => 0x14,
            "numlock" => 0x90,
            "scrolllock" => 0x91,
            "leftshift" => 0xa0,
            "rightshift" => 0xa1,
            "leftcontrol" | "leftctrl" => 0xa2,
            "rightcontrol" | "rightctrl" => 0xa3,
            "leftalt" => 0xa4,
            "rightalt" => 0xa5,
            "shift" => 0x10,
            "ctrl" => 0x11,
            "alt" => 0x12,
            "leftwindows" | "leftcommand" | "win" => 0x5b,
            "rightwindows" | "rightcommand" => 0x5c,
            "f12" => 0x7b,
            _ => shortcut(&[raw])?
                .map(|s| s.key)
                .ok_or_else(|| anyhow::anyhow!("Unknown key {raw}"))?,
        };
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    ensure!(keys.len() <= 3, "At most three keys per VTS chord");
    Ok(keys)
}
pub fn shortcut(triggers: &[&str]) -> Result<Option<Shortcut>> {
    let mut result = Shortcut::default();
    for raw in triggers.iter().filter(|s| !s.is_empty()) {
        let k = raw.to_ascii_lowercase();
        match k.as_str() {
            "leftcontrol" | "rightcontrol" | "leftctrl" | "rightctrl" | "ctrl" => {
                result.ctrl = true
            }
            "leftalt" | "rightalt" | "alt" => result.alt = true,
            "leftshift" | "rightshift" | "shift" => result.shift = true,
            "leftwindows" | "rightwindows" | "leftcommand" | "rightcommand" | "win" => {
                result.win = true
            }
            _ => {
                ensure!(
                    result.key == 0,
                    "Multiple non-modifier keys need a replacement shortcut"
                );
                result.key = if k.len() == 1 && k.as_bytes()[0].is_ascii_alphanumeric() {
                    u16::from(k.as_bytes()[0].to_ascii_uppercase())
                } else if k.len() == 2 && k.starts_with('n') && k.as_bytes()[1].is_ascii_digit() {
                    u16::from(k.as_bytes()[1])
                } else if let Some(n) = k
                    .strip_prefix("numpad")
                    .and_then(|n| n.parse::<u16>().ok())
                    .filter(|n| *n <= 9)
                {
                    0x60 + n
                } else if let Some(n) = k
                    .strip_prefix('f')
                    .and_then(|n| n.parse::<u16>().ok())
                    .filter(|n| (1..=24).contains(n))
                {
                    0x6f + n
                } else {
                    match k.as_str() {
                        "space" => 0x20,
                        "enter" | "return" => 0x0d,
                        "tab" => 0x09,
                        "escape" => 0x1b,
                        "backspace" => 0x08,
                        "insert" => 0x2d,
                        "delete" => 0x2e,
                        "home" => 0x24,
                        "end" => 0x23,
                        "pageup" => 0x21,
                        "pagedown" => 0x22,
                        "leftarrow" => 0x25,
                        "uparrow" => 0x26,
                        "rightarrow" => 0x27,
                        "downarrow" => 0x28,
                        _ => anyhow::bail!("Unsupported key {raw}; choose a replacement"),
                    }
                };
            }
        }
    }
    if result == Shortcut::default() {
        return Ok(None);
    }
    result.validate()?;
    Ok(Some(result))
}
fn placement(v: &Value, saved: bool) -> Placement {
    let (x, y, z) = if saved {
        (
            number(&v["Position"], "x", 0.),
            number(&v["Position"], "y", 0.),
            number(&v["Scale"], "x", 1.),
        )
    } else {
        (number(v, "X", 0.), number(v, "Y", 0.), number(v, "Z", 1.))
    };
    Placement {
        position: [(x / 1000.).clamp(-1., 1.), (-y / 1000.).clamp(-1., 1.)],
        zoom: z.clamp(0.25, 3.),
        rotation: if saved {
            let q = &v["Rotation"];
            let z = number(q, "z", 0.);
            let w = number(q, "w", 1.);
            (-2. * z.atan2(w).to_degrees()).clamp(-180., 180.)
        } else {
            -number(v, "Rotation", 0.).clamp(-180., 180.)
        },
    }
}
pub fn parse(bytes: &[u8], parameters: &[RigParameter], extra: &[&str]) -> Result<Imported> {
    ensure!(
        bytes.len() <= crate::asset_limits::MODEL_JSON,
        "VTube Studio config exceeds 20 MiB"
    );
    let root: Value = serde_json::from_slice(bytes)?;
    ensure!(
        root.is_object()
            && root["ParameterSettings"].is_array()
            && root["FileReferences"]["Model"].is_string(),
        "Select a VTube Studio .vtube.json model config"
    );
    ensure!(
        array(&root, "Hotkeys").len() <= 512,
        "Maximum 512 imported actions"
    );
    let profile = rig::import_profile_with_inputs(bytes, parameters, extra)?;
    let mut config = Config {
        name: string(&root, "Name"),
        source_model: string(&root["FileReferences"], "Model"),
        keyboard_enabled: boolean(&root["HotkeySettings"], "UseKeyboardHotkeys", true),
        screen_enabled: boolean(&root["HotkeySettings"], "UseOnScreenHotkeys", true),
        notes: profile.warnings,
        ..Default::default()
    };
    valid_reference(&config.source_model)?;
    for (key, lost) in [
        ("IdleAnimation", false),
        ("IdleAnimationWhenTrackingLost", true),
    ] {
        let name = string(&root["FileReferences"], key);
        if !name.is_empty() {
            valid_reference(&name)?;
            if lost {
                config.lost_idle = Some(name)
            } else {
                config.idle = Some(name)
            }
        }
    }
    config.lost_idle_delay = number(
        &root["GeneralSettings"],
        "TimeUntilTrackingLostIdleAnimation",
        0.,
    )
    .clamp(0., 3600.);
    let mut keys = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for (index, h) in array(&root, "Hotkeys").iter().enumerate() {
        let mut id = string(h, "HotkeyID");
        if id.is_empty() {
            id = format!("action-{index}");
        }
        ensure!(ids.insert(id.clone()), "Duplicate HotkeyID in config");
        let name = string(h, "Name");
        let name = if name.is_empty() {
            format!("Action {}", index + 1)
        } else {
            name
        };
        let file = string(h, "File");
        let action = match string(h, "Action").as_str() {
            "ToggleExpression" => {
                if let Err(e) = valid_reference(&file) {
                    note(&mut config, format!("{name}: {e}"));
                    continue;
                }
                Action::Expression(file.replace('\\', "/"))
            }
            "RemoveAllExpressions" => Action::ClearExpressions,
            "TriggerAnimation" => {
                if let Err(e) = valid_reference(&file) {
                    note(&mut config, format!("{name}: {e}"));
                    continue;
                }
                Action::Animation {
                    file: file.replace('\\', "/"),
                    hold_last: boolean(h, "StopsOnLastFrame", false),
                }
            }
            "MoveModel" => Action::MoveModel(placement(&h["Position"], false)),
            "ToggleItemScene" => {
                valid_reference(&file)?;
                Action::ItemScene {
                    file,
                    random: boolean(h, "OnlyLoadOneRandomItem", false),
                }
            }
            "RemoveAllItems" => Action::HideItems,
            "TogglePhysics" => Action::TogglePhysics,
            other => {
                note(
                    &mut config,
                    format!(
                        "{name}: {other} needs repair. Use the ARIA repair guide to configure its local replacement."
                    ),
                );
                Action::NeedsRepair { kind: other.into() }
            }
        };
        let t = &h["Triggers"];
        let chord = chord(&[
            t["Trigger1"].as_str().unwrap_or(""),
            t["Trigger2"].as_str().unwrap_or(""),
            t["Trigger3"].as_str().unwrap_or(""),
        ]);
        let mut key = match shortcut(&[
            t["Trigger1"].as_str().unwrap_or(""),
            t["Trigger2"].as_str().unwrap_or(""),
            t["Trigger3"].as_str().unwrap_or(""),
        ]) {
            Ok(key) => key,
            Err(e) => {
                if chord.is_err() {
                    note(
                        &mut config,
                        format!("{name}: {e}. Assign a new key in the action editor."),
                    );
                }
                None
            }
        };
        let simple_key = key.is_some();
        if key.is_some_and(|k| k == Shortcut::pose() || !keys.insert(k)) {
            note(
                &mut config,
                format!("{name}: duplicate/reserved shortcut removed; assign another key."),
            );
            key = None;
        }
        if boolean(&h["TwitchTriggers"], "Active", false) {
            note(
                &mut config,
                format!(
                    "{name}: use the ARIA repair guide to assign a local trigger. Service authorization is not transferred."
                ),
            );
        }
        if boolean(&h["SoundConfig"], "SoundActive", false) {
            note(
                &mut config,
                format!("{name}: sound slots need a local sound assignment in ARIA."),
            );
        }
        if !string(&h["HandGestureSettings"], "GestureLeft").is_empty()
            || !string(&h["HandGestureSettings"], "GestureRight").is_empty()
        {
            note(
                &mut config,
                format!(
                    "{name}: use the repair guide to choose an available ARIA input or hotkey for this gesture trigger."
                ),
            );
        }
        let release =
            boolean(h, "DeactivateAfterKeyUp", false) && matches!(&action, Action::Expression(_));
        let seconds = boolean(h, "DeactivateAfterSeconds", false)
            .then(|| number(h, "DeactivateAfterSecondsAmount", 10.).clamp(0.01, 3600.));
        let fade = number(h, "FadeSecondsAmount", 0.5).clamp(0., 60.);
        config.actions.push(Hotkey {
            id,
            name,
            action,
            shortcut: key,
            chord: if !simple_key {
                chord.unwrap_or_default()
            } else {
                Vec::new()
            },
            screen_button: t["ScreenButton"]
                .as_i64()
                .filter(|v| (0..=128).contains(v))
                .map(|v| v as i32),
            enabled: boolean(h, "IsActive", true),
            global: boolean(h, "IsGlobal", true),
            release,
            seconds,
            fade,
        });
    }
    let mut expressions = BTreeSet::new();
    ensure!(
        array(&root, "SavedActiveExpressions").len() <= 256,
        "Too many active expressions"
    );
    for e in array(&root, "SavedActiveExpressions") {
        if let Some(name) = e.as_str() {
            valid_reference(name)?;
            expressions.insert(name.replace('\\', "/"));
        }
    }
    let p = &root["PhysicsSettings"];
    let mut physics = physics::PhysicsSettings {
        enabled: profile.physics_enabled,
        strength: (number(p, "PhysicsStrength", 50.) / 50.).clamp(0., 2.),
        wind: (number(p, "WindStrength", 0.) / 100.).clamp(-1., 1.),
        ..Default::default()
    };
    for (id, multiplier) in profile.physics_multipliers {
        physics.groups.entry(id).or_default().strength = multiplier.clamp(0., 2.);
    }
    for row in array(
        &root["PhysicsCustomizationSettings"],
        "WindMultipliersPerPhysicsGroup",
    ) {
        let id = string(row, "ID");
        if !id.is_empty() {
            physics.groups.entry(id).or_default().wind =
                (physics.wind * (number(row, "Value", 1.) - 1.)).clamp(-1., 1.);
        }
    }
    note(
        &mut config,
        "Physics strength/wind use ARIA's solver and supported ranges. VTS legacy physics, physics FPS and dragging-force algorithms are not reproduced.",
    );
    let sway = &root["ModelPositionMovement"];
    config.sway = Sway {
        enabled: boolean(sway, "Use", false),
        amount: [
            number(sway, "X", 6.),
            number(sway, "Y", 8.),
            number(sway, "Z", 11.),
        ]
        .map(|v| v.clamp(0., 100.)),
        smoothing: [
            number(sway, "SmoothingX", 10.),
            number(sway, "SmoothingY", 10.),
            number(sway, "SmoothingZ", 10.),
        ]
        .map(|v| v.clamp(0., 100.)),
    };
    config.source_id = string(&root, "ModelID");
    let mut layers = layers::Config::default();
    ensure!(
        array(&root["ArtMeshDetails"], "ArtMeshMultiplyAndScreenColors").len() <= 8192,
        "Too many mesh colors"
    );
    for row in array(&root["ArtMeshDetails"], "ArtMeshMultiplyAndScreenColors") {
        let id = string(row, "ID");
        let text = string(row, "Value");
        if let Some((multiply, screen)) = text.split_once('|') {
            let color = |s: &str| -> Option<[f32; 4]> {
                if s.len() != 8 || !s.is_ascii() {
                    return None;
                }
                let mut c = [0.; 4];
                for i in 0..4 {
                    c[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()? as f32 / 255.;
                }
                Some(c)
            };
            if let (Some(multiply), Some(screen)) = (color(multiply), color(screen)) {
                layers
                    .colors
                    .insert(id, layers::Colors { multiply, screen });
                continue;
            }
        }
        note(
            &mut config,
            format!("Invalid color for mesh {id}; kept the model's original color."),
        );
    }
    for row in array(&root["UserCustomization"], "UserParamInfo") {
        let id = string(row, "ID");
        let name = string(row, "Name");
        if !id.is_empty() && !name.is_empty() && parameters.iter().any(|p| p.id == id) {
            config.labels.insert(id, name);
        }
    }
    for row in array(&root, "ParameterSettings") {
        let id = string(row, "OutputLive2D");
        let name = string(row, "Name");
        if !id.is_empty() && !name.is_empty() && parameters.iter().any(|p| p.id == id) {
            config.labels.entry(id).or_insert(name);
        }
    }
    for key in [
        "ArtMeshesExcludedFromPinning",
        "ArtMeshesThatDeleteItemsOnDrop",
        "ArtMeshSceneLightingMultipliers",
        "ModelArtMeshGroups",
    ] {
        if !array(&root["ArtMeshDetails"], key).is_empty() {
            note(
                &mut config,
                format!("{key}: not transferred; use ARIA's layer and pin controls."),
            );
        }
    }
    if !array(&root["ParameterCustomization"], "SoundTriggers").is_empty() {
        note(
            &mut config,
            "Parameter sound triggers require manual recreation.",
        );
    }
    let saved_placement = root.get("SavedModelPosition").map(|v| placement(v, true));
    if saved_placement.is_some() {
        note(
            &mut config,
            "Saved model framing is available as an optional approximation. Preview uses ARIA's full-model bounds; VTS camera framing can differ; use the framing controls to fine tune.",
        );
    }
    if config.name.is_empty() {
        config.name = "VTube Studio model".into();
    }
    config.validate()?;
    physics.validate()?;
    layers.validate()?;
    Ok(Imported {
        config,
        bindings: profile.bindings,
        physics,
        layers,
        expressions,
        saved_placement,
    })
}
