//! A draft-only calibration tour: preview is an overlay; closing never saves it.
use crate::{help, theme};
use aria_core::{
    calibration::{Capture, Phase, Profile, Proposal},
    rig::{self, Inputs, RigParameter},
};
use eframe::egui;
use std::collections::BTreeMap;

pub enum Action {
    Connection,
    Resume,
    Save(Profile),
}

#[derive(Default)]
pub struct Guide {
    pub open: bool,
    context: String,
    pub preview: bool,
    stage: usize,
    capture: Capture,
    recording: bool,
    countdown: Option<f64>,
    original: Profile,
    origin: aria_core::Vec3,
    rows: Vec<Proposal>,
    bindings: BTreeMap<String, Vec<String>>,
    unmapped: Vec<String>,
    show_unused: bool,
    live: bool,
    now: f64,
    pub message: Option<String>,
}
const PHASES: [Phase; 4] = [Phase::Neutral, Phase::Head, Phase::Face, Phase::Gaze];
impl Guide {
    pub fn start(
        &mut self,
        context: String,
        config: &aria_core::movement::RigConfig,
        parameters: &[RigParameter],
        inputs: &Inputs,
        origin: aria_core::Vec3,
    ) {
        let mut names: Vec<String> = rig::INPUT_NAMES
            .iter()
            .map(|s| (*s).into())
            .chain(aria_core::PARAMETER_SPECS.iter().map(|s| s.0.into()))
            .chain(rig::FACE_ALIASES.iter().map(|s| s.0.into()))
            .collect();
        names.extend(inputs.keys().filter(|n| n.starts_with("ARKit:")).cloned());
        let mut bindings: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (id, binding) in &config.bindings {
            names.push(binding.input.clone());
            bindings
                .entry(binding.input.clone())
                .or_default()
                .push(id.clone());
        }
        *self = Self {
            open: true,
            context,
            capture: Capture::new(names),
            original: config.tracking.clone(),
            origin: if config.tracking.ranges.is_empty() {
                origin
            } else {
                config.tracking.origin
            },
            bindings,
            unmapped: parameters
                .iter()
                .filter(|p| !config.bindings.contains_key(&p.id))
                .map(|p| p.id.clone())
                .collect(),
            ..Self::default()
        };
    }
    pub fn check_context(&mut self, context: &str) {
        if self.open && self.context != context {
            self.open = false;
            self.preview = false;
            self.message = Some("Tracking setup closed because the avatar, connection or mapping changed. Your saved calibration was kept; reopen the guide to capture again.".into());
        }
    }
    pub fn profile(&self) -> Option<Profile> {
        if !self.open || !self.preview {
            return None;
        }
        Some(self.draft())
    }
    fn draft(&self) -> Profile {
        let mut profile = self.original.clone();
        profile.enabled = true;
        // Capture against the saved origin so retained and new ranges agree.
        profile.origin = self.origin;
        for row in &self.rows {
            if let Some(range) = row
                .range
                .as_ref()
                .filter(|r| r.enabled && r.valid(&row.input))
            {
                profile.ranges.insert(row.input.clone(), range.clone());
            }
        }
        profile
    }
    pub fn observe(&mut self, sequence: u64, now: f64, live: bool, inputs: &Inputs) {
        self.live = live;
        self.now = now;
        if !self.open || !(1..=4).contains(&self.stage) {
            return;
        }
        if let Some(start) = self.countdown {
            if !live {
                self.countdown = Some(now);
                return;
            }
            if now - start < 3.0 {
                return;
            }
            self.countdown = None;
            self.recording = true;
            self.capture.begin(PHASES[self.stage - 1]);
        }
        if !self.recording {
            return;
        }
        let phase = PHASES[self.stage - 1];
        self.capture.sample(phase, sequence, now, live, inputs);
        if self.capture.complete(phase) {
            self.recording = false;
            self.next();
        }
    }
    fn next(&mut self) {
        self.recording = false;
        self.countdown = None;
        self.preview = false;
        self.stage += 1;
        if self.stage == 5 {
            self.rows = self.capture.proposals();
        }
    }
    fn skip(&mut self) {
        // Discard even a partly recorded exercise; skip must preserve saved behavior.
        self.capture.begin(PHASES[self.stage - 1]);
        self.next();
    }
    pub fn measure(
        &self,
        frame: Option<&aria_core::TrackingFrame>,
        settings: &aria_core::MappingSettings,
        time: f32,
    ) -> Inputs {
        Profile {
            origin: self.origin,
            ..Default::default()
        }
        .measure(frame, settings, time)
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        status: &str,
        live_pose: bool,
        microphone: bool,
    ) -> Option<Action> {
        if !self.open {
            return None;
        }
        let mut action = None;
        let mut open = true;
        let mut cancel = false;
        egui::Window::new("Set up tracking for your avatar")
            .id(egui::Id::new("tracking-setup-guide"))
            .open(&mut open).default_width(570.0).default_pos(egui::pos2(690.0, 110.0))
            .resizable(true).collapsible(true).show(ctx, |ui| {
                help::label(ui, "Personal tracking setup", "tracking-guide");
                theme::caption(ui, "Connect → Neutral → Head → Expressions → Gaze → Try & save");
                ui.separator();
                if !live_pose {
                    ui.colored_label(egui::Color32::LIGHT_YELLOW, "A held pose can hide tracking changes.");
                    if ui.button("Resume live movement").clicked() { action = Some(Action::Resume); }
                }
                if microphone { theme::caption(ui, "Microphone control is active and may override phone mouth inputs. Disable it in Microphone to check facial lip sync."); }
                ui.label(format!("Tracker: {status}"));
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 330.0).clamp(120.0, 420.0)).show(ui, |ui| {
                    match self.stage {
                        0 => {
                            ui.heading("1. Connect and get comfortable");
                            ui.label("Put your camera or phone at eye level, use even lighting, and sit at your usual streaming distance. Keep the same position throughout setup.");
                            ui.label("For iPhone VTube Studio: enable 3rd Party PC Clients, use the phone's IPv4 address, and put both devices on the same network. Select iPhone tracking and connect in ARIA.");
                            if ui.button("Open connection settings").clicked() { action = Some(Action::Connection); }
                            ui.label(format!("Found {} model input assignments. Their output ranges, directions, physics and expressions will be preserved.", self.bindings.values().map(Vec::len).sum::<usize>()));
                            self.unmapped_ui(ui);
                            theme::caption(ui, "Demo movement and microphone-only input cannot calibrate your face. A live face signal is required. The guide measures comfortable motion; move naturally, without straining.");
                            if ui.add_enabled(self.live, egui::Button::new("Begin personal setup")).clicked() { self.stage = 1; }
                        }
                        1..=4 => {
                            let phase = PHASES[self.stage - 1];
                            let (title, instruction, diagram) = match phase {
                                Phase::Neutral => ("2. Your relaxed, neutral pose", "Look straight at the camera. Keep your eyes naturally open, lips relaxed and closed, and eyebrows at rest. Hold still for three seconds.", "Eyes open  ·  Face forward  ·  Mouth relaxed"),
                                Phase::Head => ("3. Your comfortable head movement", "Slowly turn left and right, nod up and down, then tilt toward each shoulder. Repeat and briefly hold each comfortable limit. Return to center between movements.", "Turn ← →     Nod ↑ ↓     Tilt ↶ ↷"),
                                Phase::Face => ("4. Your facial expressions", "Blink and hold your eyes closed briefly. Open your mouth, smile and frown, raise and lower your brows, pucker and puff your cheeks. Use tongue or extra mouth shapes if your tracker and model support them. Repeat comfortably.", "Relax → Express → Hold briefly → Relax"),
                                Phase::Gaze => ("5. Your eye movement", "Keep your head still. Look left, right, up and down with your eyes. Hold each direction briefly, then look back at the camera. Skip if your tracker does not provide eye gaze.", "        ↑\nLook  ←  •  →\n        ↓"),
                            };
                            ui.heading(title); ui.label(instruction);
                            egui::Frame::group(ui.style()).show(ui, |ui| { ui.monospace(diagram); });
                            if let Some(start) = self.countdown {
                                ui.heading(if self.live { format!("Get ready… {}", (3.0 - (self.now - start)).ceil().max(1.0) as u32) } else { "Waiting for a live face…".into() });
                            } else if self.recording {
                                ui.add(egui::ProgressBar::new((self.capture.elapsed / phase.seconds()).min(1.0) as f32).text(format!("{:.1} / {:.0} seconds · {} fresh samples", self.capture.elapsed, phase.seconds(), self.capture.samples)));
                                if !self.live { ui.colored_label(egui::Color32::LIGHT_YELLOW, "Paused — face or connection lost. Return to the camera to continue."); }
                            } else {
                                theme::caption(ui, format!("A 3-second countdown is followed by {:.0} seconds of capture. The next page opens automatically.", phase.seconds()));
                            }
                            ui.horizontal(|ui| {
                                if ui.add_enabled(self.live, egui::Button::new(if self.recording || self.countdown.is_some() { "Restart capture" } else { "Start capture" })).clicked() {
                                    self.recording = false; self.countdown = Some(self.now);
                                }
                                if self.stage > 1 && ui.button("Skip this movement").clicked() { self.skip(); }
                            });
                            theme::caption(ui, "Only fresh face packets count. Unsupported or still signals are left unchanged. Stay within a comfortable range; you can retry any movement in the review.");
                        }
                        _ => self.review_ui(ui),
                    }
                });
                ui.separator();
                if self.stage == 5 { self.review_actions(ui, &mut action, live_pose); }
                if ui.button("Cancel / keep saved settings").clicked() { cancel = true; }
            });
        if !open || cancel {
            self.open = false;
            self.preview = false;
        }
        action
    }
    fn unmapped_ui(&self, ui: &mut egui::Ui) {
        ui.collapsing(format!("{} parameters without tracking assignments", self.unmapped.len()), |ui| {
            ui.label("Many parameters belong to physics, clothing or expressions and should stay unassigned. For a custom face control, choose its input in Inspector → Tracking → Inputs, then rerun this guide. ARIA cannot infer an arbitrary rig's meaning from geometry alone.");
            for name in &self.unmapped { ui.monospace(name); }
        });
    }
    fn review_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("6. Try it on your avatar");
        let count = self
            .rows
            .iter()
            .filter(|r| {
                r.range
                    .as_ref()
                    .is_some_and(|v| v.enabled && v.valid(&r.input))
            })
            .count();
        ui.label(format!("{count} inputs ready. Low / Rest / High are your measured values. Rest stays centered even when your movement is asymmetric."));
        theme::caption(
            ui,
            "Unselected, skipped and unavailable signals keep their existing settings. Each calibrated signal is converted to its standard range before your model's existing assignments. Custom artistic ranges may still need adjustment in Inputs.",
        );
        ui.checkbox(
            &mut self.show_unused,
            "Show inputs without a model assignment (also usable for image actions)",
        );
        for row in &mut self.rows {
            let assigned = self.bindings.get(&row.input);
            if !self.show_unused && assigned.is_none() {
                continue;
            }
            ui.push_id(&row.input, |ui| {
                egui::CollapsingHeader::new(format!("{} · {}", row.input, if row.range.is_some() { "Captured" } else { "Needs review" }))
                    .default_open(matches!(row.input.as_str(), "FaceAngleX" | "ParamAngleX")).show(ui, |ui| {
                    if let Some(ids) = assigned { theme::caption(ui, format!("Drives: {}", ids.join(", "))); }
                    if let Some(r) = &mut row.range {
                        ui.checkbox(&mut r.enabled, "Apply this new range");
                        ui.horizontal_wrapped(|ui| {
                            for (label, value) in [("Low", &mut r.low), ("Rest", &mut r.neutral), ("High", &mut r.high)] {
                                ui.label(label);
                                help::control(ui, "tracking-guide-ranges", |ui| ui.add(egui::DragValue::new(value).speed(0.01).range(-10000.0..=10000.0).max_decimals(3)));
                            }
                        });
                        if !r.valid(&row.input) { ui.colored_label(egui::Color32::LIGHT_RED, "Invalid range: Low ≤ Rest ≤ High; each active direction needs a nonzero span."); }
                    } else { ui.label(row.message); }
                });
            });
        }
        ui.horizontal_wrapped(|ui| {
            for (index, label) in [
                (1, "Redo neutral (all captures)"),
                (2, "Redo head"),
                (3, "Redo expressions"),
                (4, "Redo gaze"),
            ] {
                if ui.button(label).clicked() {
                    self.stage = index;
                    self.preview = false;
                }
            }
        });
        self.unmapped_ui(ui);
        theme::caption(
            ui,
            "Saved calibration follows this avatar and new movement presets. Rerun after moving the camera, changing tracker, gains or axis settings. Older presets restore their previous calibration.",
        );
    }
    fn review_actions(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>, live_pose: bool) {
        let valid = self.rows.iter().any(|r| {
            r.range
                .as_ref()
                .is_some_and(|v| v.enabled && v.valid(&r.input))
        }) && self.rows.iter().all(|row| {
            row.range
                .as_ref()
                .is_none_or(|r| !r.enabled || r.valid(&row.input))
        }) && self.draft().validate().is_ok();
        ui.add_enabled(
            valid && live_pose,
            egui::Checkbox::new(&mut self.preview, "Preview new calibration on stage"),
        );
        if !valid || !live_pose {
            self.preview = false;
        }
        theme::caption(
            ui,
            "Move or collapse this window to see the stage. Turn preview off to compare with your saved settings. Check center, left/right, up/down, eyelids and lip sync. Change axis direction in Movement & calibration before recapturing if needed.",
        );
        if ui
            .add_enabled(valid, egui::Button::new("Save calibration for this avatar"))
            .clicked()
        {
            *action = Some(Action::Save(self.draft()));
            self.open = false;
            self.preview = false;
        }
    }
}

#[cfg(any(test, feature = "screenshots"))]
impl Guide {
    /// Deterministic test trace through the real capture state machine, never a
    /// synthetic stand-in for an end user's camera in production builds.
    pub fn rehearsal(&mut self) {
        self.stage = 1;
        let mut sequence = 1;
        let pipeline = aria_core::ParameterPipeline::default();
        let settings = aria_core::MappingSettings::default();
        for (index, phase) in PHASES.into_iter().enumerate() {
            assert_eq!(self.stage, index + 1);
            self.capture.begin(phase);
            self.recording = true;
            for n in 0..((phase.seconds() * 60.0) as usize + 5) {
                let mut frame = aria_core::TrackingFrame {
                    face_found: true,
                    ..Default::default()
                };
                let motion = if n % 120 < 40 {
                    -1.0
                } else if n % 120 < 80 {
                    1.0
                } else {
                    0.0
                };
                frame.rotation.y = 4.0;
                if phase == Phase::Head {
                    frame.rotation.y += if motion < 0.0 {
                        motion * 22.0
                    } else {
                        motion * 42.0
                    };
                    frame.rotation.x = motion * 18.0;
                    frame.rotation.z = motion * 16.0;
                }
                frame.blend_shapes.insert(
                    "jawopen".into(),
                    if phase == Phase::Face && motion > 0.0 {
                        0.75
                    } else {
                        0.1
                    },
                );
                for key in [
                    "eyeblinkleft",
                    "eyeblinkright",
                    "mouthsmileleft",
                    "mouthsmileright",
                    "browinnerup",
                    "tongueout",
                ] {
                    frame.blend_shapes.insert(
                        key.into(),
                        if phase == Phase::Face && motion > 0.0 {
                            0.8
                        } else {
                            0.0
                        },
                    );
                }
                for key in [
                    "browdownleft",
                    "browdownright",
                    "mouthfrownleft",
                    "mouthfrownright",
                ] {
                    frame.blend_shapes.insert(
                        key.into(),
                        if phase == Phase::Face && motion < 0.0 {
                            0.8
                        } else {
                            0.0
                        },
                    );
                }
                if phase == Phase::Gaze {
                    for key in if motion > 0.0 {
                        [
                            "eyelookinleft",
                            "eyelookoutright",
                            "eyelookupleft",
                            "eyelookupright",
                        ]
                    } else {
                        [
                            "eyelookoutleft",
                            "eyelookinright",
                            "eyelookdownleft",
                            "eyelookdownright",
                        ]
                    } {
                        frame.blend_shapes.insert(key.into(), motion.abs() * 0.8);
                    }
                }
                let inputs = rig::tracking_inputs(
                    Some(&frame),
                    pipeline.measure(Some(&frame), &settings),
                    false,
                    0.0,
                );
                self.observe(sequence, sequence as f64 / 60.0, true, &inputs);
                sequence += 1;
                if !self.recording {
                    break;
                }
            }
        }
        assert_eq!(self.stage, 5);
        assert!(self.draft().validate().is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires local ARIA_TEST_MODEL and ARIA_CUBISM_CORE; no phone required"]
    fn learned_tracking_drives_native_model_with_imported_assignments() {
        use std::path::Path;
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
        let imported = rig::import_profile(
            &aria_model::read_bounded(
                files.tracking_profile.as_ref().unwrap(),
                aria_core::asset_limits::MODEL_JSON,
            )
            .unwrap(),
            model.parameters(),
        )
        .unwrap();
        let mut config = aria_core::movement::RigConfig::from_parameters(model.parameters());
        config.bindings.extend(imported.bindings);
        let mut guide = Guide::default();
        guide.start(
            "native-fixture".into(),
            &config,
            model.parameters(),
            &Inputs::new(),
            Default::default(),
        );
        guide.rehearsal();
        config.tracking = guide.draft();
        config.validate(model.parameters()).unwrap();
        let mut frame = aria_core::TrackingFrame {
            face_found: true,
            rotation: aria_core::Vec3 {
                y: 46.0,
                ..Default::default()
            },
            ..Default::default()
        };
        frame.blend_shapes.insert("jawopen".into(), 0.75);
        let raw = config
            .tracking
            .measure(Some(&frame), &Default::default(), 0.0);
        let mut inputs = raw.clone();
        aria_core::calibration::Filter::default().apply(
            &config.tracking,
            &raw,
            &mut inputs,
            true,
            0.0,
            0.016,
        );
        assert_eq!(inputs["FaceAngleX"], 30.0);
        let mut parameters = model.parameters().to_vec();
        for binding in config.bindings.values_mut() {
            binding.smoothing_ms = 0.0;
        }
        config.evaluate(&inputs, &mut parameters, 0.016, None);
        for p in &parameters {
            model.set_parameter(&p.id, p.value);
        }
        model.update().unwrap();
        let mut checked = 0;
        for (id, binding) in &mut config.bindings {
            if config.tracking.ranges.contains_key(&binding.input) {
                let param = model.parameters().iter().find(|p| p.id == *id).unwrap();
                let expected = binding
                    .evaluate(inputs[&binding.input], 0.016)
                    .clamp(param.min, param.max);
                assert!(
                    (param.value - expected).abs() < 1e-4,
                    "{id}: {} != {expected}",
                    param.value
                );
                checked += 1;
            }
        }
        assert!(checked >= 6);
        eprintln!("Verified {checked} learned input assignments against native model parameters");
    }
    #[test]
    fn preview_is_an_overlay_and_context_change_discards_it() {
        let mut guide = Guide::default();
        let rig = aria_core::movement::RigConfig::default();
        guide.start(
            "model-a".into(),
            &rig,
            &[],
            &Inputs::new(),
            Default::default(),
        );
        guide.preview = true;
        assert!(guide.profile().unwrap().enabled);
        assert!(!rig.tracking.enabled);
        guide.check_context("model-b");
        assert!(guide.profile().is_none());
        assert!(!guide.open);
    }
    #[test]
    fn complete_tour_preserves_model_mapping_and_retries_only_selected_group() {
        let parameters = aria_core::movement::preview_parameters(Default::default());
        let mut config = aria_core::movement::RigConfig::from_parameters(&parameters);
        config.bindings.get_mut("ParamAngleX").unwrap().output_min = 22.0;
        config.bindings.get_mut("ParamAngleX").unwrap().output_max = -18.0;
        let before = serde_json::to_string(&config).unwrap();
        let mut guide = Guide::default();
        guide.start(
            "a".into(),
            &config,
            &parameters,
            &Inputs::new(),
            Default::default(),
        );
        guide.rehearsal();
        assert_eq!(serde_json::to_string(&config).unwrap(), before);
        let draft = guide.draft();
        let yaw = &draft.ranges["ParamAngleX"];
        assert_eq!(yaw.neutral, 4.0);
        assert_eq!(yaw.map("ParamAngleX", 46.0), 30.0);
        assert_eq!(
            config
                .bindings
                .get_mut("ParamAngleX")
                .unwrap()
                .evaluate(yaw.map("ParamAngleX", 46.0), 0.016),
            -18.0
        );
        assert!(draft.ranges.contains_key("JawOpen"));
        assert!(!draft.ranges.contains_key("CheekPuff"));
        guide.stage = 4;
        guide.skip();
        let retry = guide.capture.proposals();
        assert!(
            retry
                .iter()
                .find(|p| p.input == "ParamAngleX")
                .unwrap()
                .range
                .is_some()
        );
        assert!(
            retry
                .iter()
                .find(|p| p.input == "ParamEyeBallX")
                .unwrap()
                .range
                .is_none()
        );
        guide.preview = true;
        guide.open = false;
        assert!(guide.profile().is_none());
    }
}
