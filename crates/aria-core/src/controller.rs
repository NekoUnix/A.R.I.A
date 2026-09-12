//! Normalized controller input and Nyarupad-compatible rig signals.
//! No platform API or device handle is stored in an avatar's configuration.
use crate::rig::Inputs;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const INPUT_NAMES: &[&str] = &[
    "NP_ON",
    "NP_LButtonDown",
    "NP_RButtonDown",
    "NP_LButtonPress",
    "NP_RButtonPress",
    "NP_LThumbX",
    "NP_LThumbY",
    "NP_RThumbX",
    "NP_RThumbY",
    "NP_LStickX",
    "NP_LStickY",
    "NP_RStickX",
    "NP_RStickY",
    "NP_LOnStick",
    "NP_ROnStick",
    "NP_L1",
    "NP_R1",
    "NP_L2",
    "NP_R2",
    "NP_ButtonA",
    "NP_ButtonB",
    "NP_ButtonX",
    "NP_ButtonY",
    "NP_DPadUp",
    "NP_DPadDown",
    "NP_DPadLeft",
    "NP_DPadRight",
    "NP_SelectDown",
    "NP_StartDown",
    "NP_ButtonLS",
    "NP_ButtonRS",
    "NP_LIndexPos",
    "NP_RIndexPos",
];

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    /// SDL device GUID and serial/ordinal, or first connected controller in Auto.
    pub device: Option<String>,
    pub custom_mappings: String,
    pub stick_dead_zone: f32,
    pub trigger_dead_zone: f32,
    pub press_release_ms: f32,
    pub dpad_to_left_stick: bool,
    pub invert_left_y: bool,
    pub invert_right_y: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            device: None,
            custom_mappings: String::new(),
            stick_dead_zone: 0.12,
            trigger_dead_zone: 0.03,
            press_release_ms: 100.0,
            dpad_to_left_stick: false,
            invert_left_y: false,
            invert_right_y: false,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.device
                .as_ref()
                .is_none_or(|n| !n.is_empty() && n.len() <= 512),
            "Invalid controller identity"
        );
        ensure!(
            self.custom_mappings.len() <= 1024 * 1024 && !self.custom_mappings.contains('\0'),
            "Controller mappings must be text up to 1 MiB"
        );
        ensure!(
            self.stick_dead_zone.is_finite() && (0.0..=0.5).contains(&self.stick_dead_zone),
            "Invalid stick dead zone"
        );
        ensure!(
            self.trigger_dead_zone.is_finite() && (0.0..=0.5).contains(&self.trigger_dead_zone),
            "Invalid trigger dead zone"
        );
        ensure!(
            self.press_release_ms.is_finite() && (0.0..=1000.0).contains(&self.press_release_ms),
            "Invalid controller press release time"
        );
        Ok(())
    }
}

/// Button positions use the public XInput bit layout, independent of lettering.
pub mod button {
    pub const UP: u16 = 0x0001;
    pub const DOWN: u16 = 0x0002;
    pub const LEFT: u16 = 0x0004;
    pub const RIGHT: u16 = 0x0008;
    pub const START: u16 = 0x0010;
    pub const SELECT: u16 = 0x0020;
    pub const LS: u16 = 0x0040;
    pub const RS: u16 = 0x0080;
    pub const LB: u16 = 0x0100;
    pub const RB: u16 = 0x0200;
    pub const A: u16 = 0x1000;
    pub const B: u16 = 0x2000;
    pub const X: u16 = 0x4000;
    pub const Y: u16 = 0x8000;
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    /// Positive X is right; positive Y is up. Sticks range from -1 to +1.
    pub left: [f32; 2],
    pub right: [f32; 2],
    /// Released = 0, fully squeezed = 1.
    pub triggers: [f32; 2],
    pub buttons: u16,
    /// Press edges received since the last frame, including taps already released.
    pub pressed: u16,
}

#[derive(Default)]
pub struct Mapper {
    previous: u16,
    pulses: [f32; 2],
    thumbs: [[f32; 2]; 2],
    on_stick: [bool; 2],
    index: [bool; 2],
}
fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}
fn flag(v: bool) -> f32 {
    if v { 1.0 } else { 0.0 }
}
fn stick(v: [f32; 2], dead: f32) -> [f32; 2] {
    let [x, y] = v.map(|n| finite(n).clamp(-1.0, 1.0));
    let length = x.hypot(y);
    let dead = finite(dead).clamp(0.0, 0.5);
    if length <= dead {
        return [0.0; 2];
    }
    let gain = ((length.min(1.0) - dead) / (1.0 - dead)) / length;
    [x * gain, y * gain]
}
impl Mapper {
    pub fn inject(&mut self, sample: Option<Sample>, s: &Settings, dt: f32, inputs: &mut Inputs) {
        // Always publish neutral values after disconnect/disable, including the visibility switch.
        for name in INPUT_NAMES {
            inputs.insert((*name).into(), 0.0);
        }
        let Some(pad) = sample.filter(|_| s.enabled) else {
            *self = Self::default();
            return;
        };
        use button::*;
        let down = |mask| flag(pad.buttons & mask != 0);
        let pressed = (pad.buttons & !self.previous) | pad.pressed;
        self.previous = pad.buttons;
        let mut sticks = [
            stick(pad.left, s.stick_dead_zone),
            stick(pad.right, s.stick_dead_zone),
        ];
        let triggers = pad.triggers.map(|v| {
            let dead = finite(s.trigger_dead_zone).clamp(0.0, 0.5);
            ((finite(v).clamp(0.0, 1.0) - dead) / (1.0 - dead)).max(0.0)
        });
        for (side, (faces, click, menu, bumper, directions)) in [
            (
                UP | DOWN | LEFT | RIGHT,
                LS,
                SELECT,
                LB,
                [RIGHT, LEFT, UP, DOWN],
            ),
            (A | B | X | Y, RS, START, RB, [B, X, Y, A]),
        ]
        .into_iter()
        .enumerate()
        {
            let face_down = pad.buttons & faces != 0;
            if sticks[side] != [0.0; 2] {
                self.on_stick[side] = true;
            }
            if face_down || pressed & faces != 0 {
                let buttons = if face_down { pad.buttons } else { pressed };
                let direction = |mask| flag(buttons & mask != 0);
                self.thumbs[side] = [
                    direction(directions[0]) - direction(directions[1]),
                    direction(directions[2]) - direction(directions[3]),
                ];
            }
            if pressed & faces != 0 {
                self.on_stick[side] = false;
            }
            if pressed & click != 0 {
                self.on_stick[side] = true;
            }
            if pad.buttons & bumper != 0 {
                self.index[side] = false;
            } else if triggers[side] > 0.0 {
                self.index[side] = true;
            }
            let press_mask = click
                | menu
                | if side == 0 && s.dpad_to_left_stick {
                    0
                } else {
                    faces
                };
            self.pulses[side] = if pressed & press_mask != 0 {
                1.0
            } else if s.press_release_ms > 0.0 {
                self.pulses[side]
                    * (-finite(dt).clamp(0.0, 1.0) * 1000.0 / s.press_release_ms).exp()
            } else {
                0.0
            };
        }
        if s.dpad_to_left_stick && pad.buttons & (UP | DOWN | LEFT | RIGHT) != 0 {
            sticks[0] = stick(self.thumbs[0], 0.0);
            self.on_stick[0] = true;
        }
        if s.invert_left_y {
            sticks[0][1] *= -1.0;
        }
        if s.invert_right_y {
            sticks[1][1] *= -1.0;
        }
        let values = [
            1.0,
            down(UP | DOWN | LEFT | RIGHT),
            down(A | B | X | Y),
            self.pulses[0],
            self.pulses[1],
            self.thumbs[0][0],
            self.thumbs[0][1],
            self.thumbs[1][0],
            self.thumbs[1][1],
            sticks[0][0],
            sticks[0][1],
            sticks[1][0],
            sticks[1][1],
            flag(self.on_stick[0]),
            flag(self.on_stick[1]),
            down(LB),
            down(RB),
            triggers[0],
            triggers[1],
            down(A),
            down(B),
            down(X),
            down(Y),
            down(UP),
            down(DOWN),
            down(LEFT),
            down(RIGHT),
            down(SELECT),
            down(START),
            down(LS),
            down(RS),
            flag(self.index[0]),
            flag(self.index[1]),
        ];
        debug_assert_eq!(values.len(), INPUT_NAMES.len());
        inputs.extend(
            INPUT_NAMES
                .iter()
                .zip(values)
                .map(|(k, v)| ((*k).into(), v)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_tap_between_frames_keeps_its_press_and_thumb_direction() {
        let mut mapper = Mapper::default();
        let mut inputs = Inputs::new();
        mapper.inject(
            Some(Sample {
                pressed: button::B,
                ..Default::default()
            }),
            &Settings::default(),
            0.016,
            &mut inputs,
        );
        assert_eq!(inputs["NP_RButtonDown"], 0.0);
        assert_eq!(inputs["NP_RButtonPress"], 1.0);
        assert_eq!(inputs["NP_RThumbX"], 1.0);
        assert_eq!(inputs["NP_ROnStick"], 0.0);
    }
    #[test]
    fn nyarupad_buttons_pulse_latch_and_clear() {
        let mut mapper = Mapper::default();
        let mut inputs = Inputs::new();
        let s = Settings::default();
        let mut sample = Sample {
            buttons: button::UP | button::A,
            triggers: [0.8, 0.0],
            ..Default::default()
        };
        mapper.inject(Some(sample), &s, 0.016, &mut inputs);
        assert_eq!(inputs["NP_ON"], 1.0);
        assert_eq!(inputs["NP_LButtonDown"], 1.0);
        assert_eq!(inputs["NP_RButtonPress"], 1.0);
        assert_eq!(inputs["NP_LThumbY"], 1.0);
        assert_eq!(inputs["NP_RThumbY"], -1.0);
        assert_eq!(inputs["NP_LIndexPos"], 1.0);
        mapper.inject(Some(sample), &s, 0.1, &mut inputs);
        assert!((inputs["NP_RButtonPress"] - (-1.0_f32).exp()).abs() < 1e-6);
        sample.buttons |= button::B; // A second button creates a new pulse even with A held.
        mapper.inject(Some(sample), &s, 0.016, &mut inputs);
        assert_eq!(inputs["NP_RButtonPress"], 1.0);
        sample.buttons = button::LB;
        mapper.inject(Some(sample), &s, 0.016, &mut inputs);
        assert_eq!(inputs["NP_LIndexPos"], 0.0); // Bumper wins over trigger.
        assert_eq!(inputs["NP_RButtonDown"], 0.0);
        mapper.inject(None, &s, 0.016, &mut inputs);
        assert!(inputs.values().all(|n| *n == 0.0));
        mapper.inject(
            Some(sample),
            &Settings {
                enabled: false,
                ..s
            },
            0.016,
            &mut inputs,
        );
        assert!(inputs.values().all(|n| *n == 0.0));
    }
    #[test]
    fn axes_stay_independent_and_drift_is_removed() {
        let mut mapper = Mapper::default();
        let mut inputs = Inputs::new();
        let s = Settings::default();
        mapper.inject(
            Some(Sample {
                left: [0.0, 1.0],
                right: [-1.0, 0.0],
                triggers: [0.01, 1.0],
                ..Default::default()
            }),
            &s,
            0.016,
            &mut inputs,
        );
        assert_eq!(inputs["NP_LStickX"], 0.0);
        assert_eq!(inputs["NP_LStickY"], 1.0);
        assert_eq!(inputs["NP_RStickX"], -1.0);
        assert_eq!(inputs["NP_RStickY"], 0.0);
        assert_eq!(inputs["NP_L2"], 0.0);
        assert_eq!(inputs["NP_R2"], 1.0);
        mapper.inject(
            Some(Sample {
                left: [0.03, 0.04],
                right: [f32::NAN, f32::INFINITY],
                ..Default::default()
            }),
            &s,
            0.016,
            &mut inputs,
        );
        assert_eq!(inputs["NP_LStickX"], 0.0);
        assert_eq!(inputs["NP_RStickY"], 0.0);
        let s = Settings {
            dpad_to_left_stick: true,
            ..s
        };
        mapper.inject(
            Some(Sample {
                buttons: button::UP | button::RIGHT,
                ..Default::default()
            }),
            &s,
            0.016,
            &mut inputs,
        );
        assert!((inputs["NP_LStickX"] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        assert_eq!(inputs["NP_LStickX"], inputs["NP_LStickY"]);
        assert_eq!(inputs["NP_LButtonPress"], 0.0);
        assert_eq!(inputs["NP_LOnStick"], 1.0);
    }
}
