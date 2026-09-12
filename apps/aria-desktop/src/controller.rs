//! SDL is used only for controller input. eframe owns windows, graphics and audio.
use aria_core::{
    controller::{Mapper, Sample, Settings, button},
    rig::Inputs,
};
use sdl2::{
    controller::{Axis, Button, GameController},
    event::Event,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

struct Pad {
    key: String,
    name: String,
    handle: GameController,
}
struct Backend {
    pads: Vec<Pad>,
    pump: sdl2::EventPump,
    controllers: sdl2::GameControllerSubsystem,
    joysticks: sdl2::JoystickSubsystem,
    _sdl: sdl2::Sdl,
    scanned: Option<Instant>,
    unmapped: Vec<String>,
}
impl Backend {
    fn new(mappings: &str) -> Result<Self, String> {
        // Non-exclusive observation must keep working while a game/OBS has focus.
        sdl2::hint::set_with_priority(
            "SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS",
            "1",
            &sdl2::hint::Hint::Override,
        );
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI", "1");
        sdl2::hint::set("SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "0");
        let sdl = sdl2::init()?;
        let controllers = sdl.game_controller()?;
        if !mappings.is_empty() {
            controllers
                .load_mappings_from_read(&mut mappings.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        Ok(Self {
            pads: Vec::new(),
            pump: sdl.event_pump()?,
            joysticks: sdl.joystick()?,
            controllers,
            _sdl: sdl,
            scanned: None,
            unmapped: Vec::new(),
        })
    }
    fn refresh(&mut self) -> Result<(), String> {
        self.pads.retain(|p| p.handle.attached());
        self.unmapped.clear();
        let mut ids: BTreeSet<_> = self.pads.iter().map(|p| p.handle.instance_id()).collect();
        for index in 0..self.controllers.num_joysticks()?.min(64) {
            if !self.controllers.is_game_controller(index) {
                let name = self
                    .joysticks
                    .name_for_index(index)
                    .unwrap_or_else(|_| "Unknown joystick".into());
                let guid = self
                    .joysticks
                    .device_guid(index)
                    .map(|g| g.to_string())
                    .unwrap_or_default();
                self.unmapped.push(format!("{name} · {guid}"));
                continue;
            }
            let handle = self.controllers.open(index).map_err(|e| e.to_string())?;
            let instance = handle.instance_id();
            if !ids.insert(instance) {
                continue;
            }
            let guid = self
                .joysticks
                .device_guid(index)
                .map_err(|e| e.to_string())?
                .to_string();
            // Serial is optional. Without one, keep stable ordinals for handles still connected.
            let serial = unsafe {
                let joystick = sdl2::sys::SDL_JoystickFromInstanceID(instance as i32);
                let serial = sdl2::sys::SDL_JoystickGetSerial(joystick);
                if serial.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(serial)
                        .to_string_lossy()
                        .chars()
                        .take(96)
                        .collect::<String>()
                }
            };
            let base = if serial.is_empty() {
                guid
            } else {
                format!("{guid}:{serial}")
            };
            let ordinal = (0..64)
                .find(|n| !self.pads.iter().any(|p| p.key == format!("{base}:{n}")))
                .unwrap_or(64);
            self.pads.push(Pad {
                key: format!("{base}:{ordinal}"),
                name: handle.name(),
                handle,
            });
        }
        self.scanned = Some(Instant::now());
        Ok(())
    }
    fn poll(&mut self) -> Result<BTreeMap<u32, u16>, String> {
        // Pump on the eframe thread. No SDL video or audio subsystem is created.
        let mut changed = false;
        let mut presses = BTreeMap::new();
        for event in self.pump.poll_iter().take(1024) {
            if let Event::ControllerButtonDown { which, button, .. } = event {
                *presses.entry(which).or_insert(0) |= button_mask(button);
            }
            changed |= matches!(
                event,
                Event::ControllerDeviceAdded { .. }
                    | Event::ControllerDeviceRemoved { .. }
                    | Event::ControllerDeviceRemapped { .. }
            );
        }
        self.controllers.update();
        if changed
            || self
                .scanned
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(1))
        {
            self.refresh()?;
        }
        Ok(presses)
    }
}
fn signed_axis(v: i16) -> f32 {
    f32::from(v) / if v < 0 { 32768.0 } else { 32767.0 }
}
fn read_sample(p: &GameController) -> Sample {
    let mut buttons = 0;
    for source in [
        Button::DPadUp,
        Button::DPadDown,
        Button::DPadLeft,
        Button::DPadRight,
        Button::Start,
        Button::Back,
        Button::LeftStick,
        Button::RightStick,
        Button::LeftShoulder,
        Button::RightShoulder,
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
    ] {
        if p.button(source) {
            buttons |= button_mask(source);
        }
    }
    Sample {
        left: [
            signed_axis(p.axis(Axis::LeftX)),
            -signed_axis(p.axis(Axis::LeftY)),
        ],
        right: [
            signed_axis(p.axis(Axis::RightX)),
            -signed_axis(p.axis(Axis::RightY)),
        ],
        triggers: [p.axis(Axis::TriggerLeft), p.axis(Axis::TriggerRight)]
            .map(|v| f32::from(v.max(0)) / 32767.0),
        buttons,
        pressed: 0,
    }
}
fn button_mask(button: Button) -> u16 {
    [
        (Button::DPadUp, button::UP),
        (Button::DPadDown, button::DOWN),
        (Button::DPadLeft, button::LEFT),
        (Button::DPadRight, button::RIGHT),
        (Button::Start, button::START),
        (Button::Back, button::SELECT),
        (Button::LeftStick, button::LS),
        (Button::RightStick, button::RS),
        (Button::LeftShoulder, button::LB),
        (Button::RightShoulder, button::RB),
        (Button::A, button::A),
        (Button::B, button::B),
        (Button::X, button::X),
        (Button::Y, button::Y),
    ]
    .into_iter()
    .find_map(|(b, mask)| (b == button).then_some(mask))
    .unwrap_or(0)
}

#[derive(Default)]
pub struct Controller {
    #[cfg(feature = "screenshots")]
    smoke_pad: Option<VirtualPad>,
    backend: Option<Backend>,
    attempted: bool,
    mappings: String,
    selected: Option<(String, u32)>,
    profile: String,
    mapper: Mapper,
    pub error: Option<String>,
    message: Option<String>,
}
impl Controller {
    #[cfg(feature = "screenshots")]
    pub fn prepare_smoke(&mut self, settings: &mut Settings) {
        let mut inputs = Inputs::new();
        self.update(settings, "controller-smoke", 0.016, &mut inputs);
        let pad = VirtualPad::new("ARIA controller test");
        pad.axis(1, -24576);
        pad.axis(2, 16384);
        pad.axis(4, 16000);
        pad.button(0, true);
        self.backend.as_mut().unwrap().refresh().unwrap();
        settings.device = self
            .backend
            .as_ref()
            .unwrap()
            .pads
            .iter()
            .find(|p| p.handle.instance_id() == pad.instance)
            .map(|p| p.key.clone());
        self.smoke_pad = Some(pad);
    }
    pub fn update(&mut self, s: &Settings, profile: &str, dt: f32, inputs: &mut Inputs) {
        if self.profile != profile {
            self.profile = profile.into();
            self.mapper = Mapper::default();
            self.selected = None;
        }
        if !s.enabled {
            self.backend = None;
            self.attempted = false;
            self.selected = None;
            self.mapper.inject(None, s, dt, inputs);
            return;
        }
        if !self.attempted || self.mappings != s.custom_mappings {
            self.backend = None;
            self.mappings.clone_from(&s.custom_mappings);
            self.attempted = true;
            self.selected = None;
            self.mapper = Mapper::default();
            match Backend::new(&self.mappings) {
                Ok(backend) => {
                    self.backend = Some(backend);
                    self.error = None;
                }
                Err(error) => {
                    self.error = Some(format!("Controller input could not start: {error}"))
                }
            }
        }
        let sample = self.backend.as_mut().and_then(|b| {
            let presses = match b.poll() {
                Ok(presses) => {
                    self.error = None;
                    presses
                }
                Err(error) => {
                    self.selected = None;
                    self.error = Some(format!("Controller input unavailable: {error}"));
                    return None;
                }
            };
            let selected = choose_pad(
                &b.pads,
                s.device.as_deref(),
                self.selected.as_ref().map(|p| p.0.as_str()),
            );
            let identity = selected.map(|p| (p.key.clone(), p.handle.instance_id()));
            if identity != self.selected {
                self.mapper = Mapper::default();
            }
            self.selected = identity;
            selected.map(|p| {
                let mut sample = read_sample(&p.handle);
                sample.pressed = presses.get(&p.handle.instance_id()).copied().unwrap_or(0);
                sample
            })
        });
        self.mapper.inject(sample, s, dt, inputs);
    }
    pub fn panel(&mut self, ui: &mut eframe::egui::Ui, s: &mut Settings, inputs: &Inputs) -> bool {
        use crate::{help, theme};
        use eframe::egui;
        let before = s.clone();
        theme::category(ui, "controller", "Gamepad input", true, |ui| {
            help::control(ui, "controller", |ui| {
                ui.checkbox(&mut s.enabled, "Enable controller input")
            });
            ui.label("Xbox, PlayStation, Switch and SDL-compatible gamepads. Works alongside phone tracking and while your game has focus.");
            let devices: Vec<_> = self
                .backend
                .as_ref()
                .map(|b| {
                    b.pads
                        .iter()
                        .map(|p| (p.key.clone(), p.name.clone()))
                        .collect()
                })
                .unwrap_or_default();
            help::control(ui, "controller", |ui| {
                egui::ComboBox::from_id_salt("controller-device")
                    .selected_text(s.device.as_ref().map_or("Auto · first available", |key| {
                        devices
                            .iter()
                            .find(|p| &p.0 == key)
                            .map_or("Saved controller · disconnected", |p| p.1.as_str())
                    }))
                    .width(ui.available_width().min(270.0))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut s.device, None, "Auto · first available");
                        for (i, (key, name)) in devices.iter().enumerate() {
                            ui.selectable_value(
                                &mut s.device,
                                Some(key.clone()),
                                format!("{} · {name}", i + 1),
                            );
                        }
                    })
            });
            let label = if !s.enabled {
                "Disabled".to_string()
            } else if let Some((key, _)) = &self.selected {
                format!(
                    "Connected · {}",
                    devices
                        .iter()
                        .find(|p| &p.0 == key)
                        .map_or("Controller", |p| p.1.as_str())
                )
            } else {
                "Waiting for a controller · connect USB or pair Bluetooth in Windows".into()
            };
            ui.colored_label(theme::MINT, label);
            if let Some(error) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
            if let Some(message) = &self.message {
                ui.label(message);
            }
            if help::control(ui, "controller", |ui| ui.button("Refresh / retry devices")).clicked()
            {
                self.backend = None;
                self.attempted = false;
            }
        });
        theme::category(
            ui,
            "controller-response",
            "Response & hand movement",
            true,
            |ui| {
                for (value, range, label) in [
                    (&mut s.stick_dead_zone, 0.0..=0.5, "Stick dead zone"),
                    (&mut s.trigger_dead_zone, 0.0..=0.5, "Trigger dead zone"),
                    (&mut s.press_release_ms, 0.0..=1000.0, "Press release · ms"),
                ] {
                    help::control(ui, "controller-response", |ui| {
                        ui.add(egui::Slider::new(value, range).text(label))
                    });
                }
                help::control(ui, "controller-response", |ui| {
                    ui.checkbox(&mut s.dpad_to_left_stick, "D-pad also moves the left stick")
                });
                help::control(ui, "controller-response", |ui| {
                    ui.checkbox(&mut s.invert_left_y, "Invert left stick up / down")
                });
                help::control(ui, "controller-response", |ui| {
                    ui.checkbox(&mut s.invert_right_y, "Invert right stick up / down")
                });
                ui.small("Saved separately for this avatar. Use Tracking → Inputs to change each NP_* assignment's range, smoothing and output.");
            },
        );
        theme::category(
            ui,
            "controller-values",
            "Live controller values",
            false,
            |ui| {
                egui::Grid::new("gamepad-values")
                    .striped(true)
                    .show(ui, |ui| {
                        for name in aria_core::controller::INPUT_NAMES {
                            ui.monospace(*name);
                            ui.monospace(format!(
                                "{:.3}",
                                inputs.get(*name).copied().unwrap_or(0.0)
                            ));
                            ui.end_row();
                        }
                    });
            },
        );
        theme::category(
            ui,
            "controller-compatibility",
            "Other controllers & custom layouts",
            false,
            |ui| {
                help::label(ui, "Controller compatibility", "controller-compatibility");
                ui.label("Most gamepads use built-in layouts. For an unrecognized device, load an SDL2 gamecontroller mapping file. Wheel / flight-stick layouts need a mapping to gamepad axes and buttons.");
                if let Some(b) = &self.backend {
                    for name in &b.unmapped {
                        ui.label(format!("Needs mapping: {name}"));
                    }
                }
                if help::control(ui, "controller-compatibility", |ui| {
                    ui.button("Import controller mappings…")
                })
                .clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .add_filter("SDL controller mappings", &["txt", "map"])
                        .pick_file()
                {
                    let result = aria_model::read_bounded(&path, 1024 * 1024).and_then(|b| {
                        let text = String::from_utf8(b)?;
                        anyhow::ensure!(
                            !text.contains('\0'),
                            "Mapping file contains a null character"
                        );
                        anyhow::ensure!(
                            text.lines()
                                .any(|l| !l.trim().is_empty() && !l.trim().starts_with('#')),
                            "Mapping file is empty"
                        );
                        // SDL performs format validation before committing the saved text.
                        let backend = self.backend.as_ref().ok_or_else(|| {
                            anyhow::anyhow!("Enable controller input and retry first")
                        })?;
                        let added = backend
                            .controllers
                            .load_mappings_from_read(&mut text.as_bytes())?;
                        anyhow::ensure!(
                            added > 0,
                            "No SDL2 controller mappings were found in this file"
                        );
                        Ok(text)
                    });
                    match result {
                        Ok(text) => {
                            s.custom_mappings = text;
                            self.message =
                                Some("Controller layouts imported for this avatar.".into());
                        }
                        Err(e) => {
                            self.message =
                                Some(format!("Could not import controller mappings: {e:#}"))
                        }
                    }
                }
                if !s.custom_mappings.is_empty()
                    && help::control(ui, "controller-compatibility", |ui| {
                        ui.button("Restore built-in controller layouts")
                    })
                    .clicked()
                {
                    s.custom_mappings.clear();
                    self.attempted = false;
                }
                ui.small("No gamepad controls affect your game settings. ARIA only reads their state; no rumble or button injection is sent.");
            },
        );
        *s != before
    }
}

// Process-local SDL virtual hardware for deterministic integration tests. Never
// installs a driver, creates a system controller or sends input to other apps.
#[cfg(any(test, feature = "screenshots"))]
struct VirtualPad {
    handle: *mut sdl2::sys::SDL_Joystick,
    instance: u32,
    _context: sdl2::Sdl,
    _joysticks: sdl2::JoystickSubsystem,
}
#[cfg(any(test, feature = "screenshots"))]
impl VirtualPad {
    fn new(name: &str) -> Self {
        let context = sdl2::init().unwrap();
        let joysticks = context.joystick().unwrap();
        let name = std::ffi::CString::new(name).unwrap();
        let desc = sdl2::sys::SDL_VirtualJoystickDesc {
            version: 1,
            type_: sdl2::sys::SDL_JoystickType::SDL_JOYSTICK_TYPE_GAMECONTROLLER as u16,
            naxes: 6,
            nbuttons: 15,
            name: name.as_ptr(),
            axis_mask: 0x3f,
            button_mask: 0x7fff,
            ..unsafe { std::mem::zeroed() }
        };
        let index = unsafe { sdl2::sys::SDL_JoystickAttachVirtualEx(&desc) };
        assert!(index >= 0, "{}", sdl2::get_error());
        let handle = unsafe { sdl2::sys::SDL_JoystickOpen(index) };
        assert!(!handle.is_null());
        let instance = unsafe { sdl2::sys::SDL_JoystickInstanceID(handle) } as u32;
        let pad = Self {
            handle,
            instance,
            _context: context,
            _joysticks: joysticks,
        };
        pad.axis(4, -32768);
        pad.axis(5, -32768);
        pad
    }
    fn axis(&self, axis: i32, value: i16) {
        assert_eq!(
            unsafe { sdl2::sys::SDL_JoystickSetVirtualAxis(self.handle, axis, value) },
            0
        );
    }
    fn button(&self, button: i32, value: bool) {
        assert_eq!(
            unsafe {
                sdl2::sys::SDL_JoystickSetVirtualButton(self.handle, button, u8::from(value))
            },
            0
        );
    }
}
#[cfg(any(test, feature = "screenshots"))]
impl Drop for VirtualPad {
    fn drop(&mut self) {
        unsafe {
            for index in 0..sdl2::sys::SDL_NumJoysticks() {
                if sdl2::sys::SDL_JoystickGetDeviceInstanceID(index) == self.instance as i32 {
                    sdl2::sys::SDL_JoystickDetachVirtual(index);
                    break;
                }
            }
            sdl2::sys::SDL_JoystickClose(self.handle);
        }
    }
}

fn choose_pad<'a>(
    pads: &'a [Pad],
    requested: Option<&str>,
    previous: Option<&str>,
) -> Option<&'a Pad> {
    if let Some(key) = requested {
        return pads.iter().find(|p| p.key == key && p.handle.attached());
    }
    previous
        .and_then(|key| pads.iter().find(|p| p.key == key && p.handle.attached()))
        .or_else(|| pads.iter().find(|p| p.handle.attached()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "native SDL devices and local ARIA_TEST_MODEL / ARIA_CUBISM_CORE; runs without physical hardware"]
    fn controller_events_drive_native_live2d_and_clear_on_disconnect() {
        use aria_core::{movement::RigConfig, rig};
        use std::path::Path;
        let mut controller = Controller::default();
        let mut settings = Settings::default();
        let mut inputs = Inputs::new();
        controller.update(&settings, "fixture", 0.016, &mut inputs);
        assert!(controller.error.is_none(), "{:?}", controller.error);
        assert_eq!(
            sdl2::hint::get("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS").as_deref(),
            Some("1")
        );
        eprintln!(
            "Physical controllers detected: {}",
            controller.backend.as_ref().unwrap().pads.len()
        );
        let pad = VirtualPad::new("ARIA virtual integration controller");
        controller.backend.as_mut().unwrap().refresh().unwrap();
        settings.device = controller
            .backend
            .as_ref()
            .unwrap()
            .pads
            .iter()
            .find(|p| p.handle.instance_id() == pad.instance)
            .map(|p| p.key.clone());
        assert!(settings.device.is_some());
        controller.update(&settings, "fixture", 0.016, &mut inputs);
        assert_eq!(inputs["NP_ON"], 1.0);
        assert_eq!(inputs["NP_L2"], 0.0);
        pad.axis(1, -32768); // SDL Y points down; rig Y must point up.
        pad.axis(2, -32768);
        pad.axis(4, 32767);
        pad.axis(5, 32767);
        for b in [0, 9, 10, 11] {
            pad.button(b, true);
        } // A, bumpers, D-pad up.
        controller.update(&settings, "fixture", 0.016, &mut inputs);
        assert_eq!(inputs["NP_LStickX"], 0.0);
        assert_eq!(inputs["NP_LStickY"], 1.0);
        assert_eq!(inputs["NP_RStickX"], -1.0);
        assert_eq!(inputs["NP_RStickY"], 0.0);
        for name in [
            "NP_RButtonDown",
            "NP_LButtonDown",
            "NP_RButtonPress",
            "NP_LButtonPress",
            "NP_L1",
            "NP_R1",
            "NP_L2",
            "NP_R2",
        ] {
            assert_eq!(inputs[name], 1.0, "{name}");
        }
        let files = aria_model::load_files(Path::new(
            &std::env::var_os("ARIA_TEST_MODEL").expect("test model"),
        ))
        .unwrap();
        let mut model = aria_live2d::CubismModel::load(
            Path::new(&std::env::var_os("ARIA_CUBISM_CORE").expect("Cubism Core")),
            &aria_model::read_bounded(&files.moc, aria_core::asset_limits::MOC_FILE).unwrap(),
            files.textures.len(),
        )
        .unwrap();
        let profile = rig::import_profile(
            &aria_model::read_bounded(
                files.tracking_profile.as_ref().unwrap(),
                aria_core::asset_limits::MODEL_JSON,
            )
            .unwrap(),
            model.parameters(),
        )
        .unwrap();
        assert!(profile.warnings.is_empty(), "{:?}", profile.warnings);
        let mut config = RigConfig::from_parameters(model.parameters());
        config.bindings.extend(profile.bindings);
        let controller_count = config
            .bindings
            .values()
            .filter(|b| b.input.starts_with("NP_"))
            .count();
        assert_eq!(controller_count, 15, "Vespera controller assignments");
        let mut parameters = model.parameters().to_vec();
        for b in config.bindings.values_mut() {
            b.smoothing_ms = 0.0;
        }
        rig::apply_bindings(&mut config.bindings, &inputs, &mut parameters, 0.016);
        for p in &parameters {
            model.set_parameter(&p.id, p.value);
        }
        model.update().unwrap();
        for (id, expected) in [
            ("ParamPadOn", 1.0),
            ("ParamLStickY", 1.0),
            ("ParamRStickX", -1.0),
            ("ParamRButtonPressed", 1.0),
        ] {
            assert_eq!(
                model
                    .parameters()
                    .iter()
                    .find(|p| p.id == id)
                    .unwrap()
                    .value,
                expected,
                "{id}"
            );
        }
        let before_pose: Vec<_> = model
            .drawables
            .iter()
            .map(|d| (d.opacity, d.positions.clone()))
            .collect();
        let mut saved = aria_core::movement::SavedRig {
            config: config.clone(),
            ..Default::default()
        };
        let mut expressions = crate::expressions_panel::ExpressionsPanel::default();
        expressions.load(
            &files.expressions,
            files.moc.parent().unwrap(),
            &saved,
            &parameters,
        );
        let pose = expressions
            .entries
            .iter()
            .find(|e| e.is_controller_pose())
            .expect("model controller-arm expression")
            .file
            .id
            .clone();
        expressions.toggle(&pose, &mut saved).unwrap();
        for _ in 0..90 {
            saved.config.evaluate_with_expressions(
                &inputs,
                &mut parameters,
                1.0 / 60.0,
                None,
                |p, active| expressions.update(p, active, 1.0 / 60.0),
            );
        }
        assert_eq!(
            parameters
                .iter()
                .find(|p| p.id == "ParamControllerArmSetONOFF")
                .unwrap()
                .value,
            1.0
        );
        for p in &parameters {
            model.set_parameter(&p.id, p.value);
        }
        model.update().unwrap();
        assert!(
            model
                .drawables
                .iter()
                .zip(before_pose)
                .any(|(d, (opacity, positions))| d.opacity != opacity || d.positions != positions),
            "Controller-arm expression must change actual rendered geometry/visibility"
        );
        // Switching to a second connected pad must not inherit pressed/finger state.
        let second = VirtualPad::new("ARIA second controller");
        controller.backend.as_mut().unwrap().refresh().unwrap();
        let first_key = settings.device.clone();
        settings.device = controller
            .backend
            .as_ref()
            .unwrap()
            .pads
            .iter()
            .find(|p| p.handle.instance_id() == second.instance)
            .map(|p| p.key.clone());
        controller.update(&settings, "fixture", 0.016, &mut inputs);
        assert_eq!(inputs["NP_RButtonDown"], 0.0);
        assert_eq!(inputs["NP_LIndexPos"], 0.0);
        settings.device = first_key;
        drop(pad);
        controller.update(&settings, "fixture", 0.016, &mut inputs);
        assert!(
            aria_core::controller::INPUT_NAMES
                .iter()
                .all(|n| inputs[*n] == 0.0)
        );
        // Explicit selection never silently picks another controller on disconnect.
        assert_eq!(inputs["NP_ON"], 0.0);
        settings.device = None;
        controller.update(&settings, "fixture", 0.016, &mut inputs);
        assert_eq!(inputs["NP_ON"], 1.0);
        settings.enabled = false;
        controller.update(&settings, "fixture", 0.016, &mut inputs);
        rig::apply_bindings(&mut config.bindings, &inputs, &mut parameters, 0.016);
        assert_eq!(
            parameters
                .iter()
                .find(|p| p.id == "ParamPadOn")
                .unwrap()
                .value,
            0.0
        );
        eprintln!(
            "Validated all {controller_count} Vespera controller bindings, native stick/button movement, controller-arm expression, selection, background hint and neutral disconnect."
        );
    }
}
