//! A draft-only calibration tour: preview is an overlay; closing never saves it.
mod face;
use crate::{help, theme};
use aria_core::{
    calibration::{
        Profile, Proposal,
        guided::{MAX_TAKES, Session},
    },
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
    capture: Session,
    seconds: f64,
    prepare_seconds: f64,
    only_assigned: bool,
    return_to_review: bool,
    live_inputs: Inputs,
    notice: Option<String>,
    #[cfg(any(test, feature = "screenshots"))]
    rehearsal_face: Option<Inputs>,
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
        let used: Vec<_> = bindings.keys().cloned().collect();
        let only_assigned = !used.is_empty();
        *self = Self {
            open: true,
            context,
            capture: Session::new(names, only_assigned.then_some(used.as_slice())),
            seconds: 12.0,
            prepare_seconds: 5.0,
            only_assigned,
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
            self.close();
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
    fn close(&mut self) {
        self.open = false;
        self.preview = false;
        self.countdown = None;
        self.capture = Session::default();
        #[cfg(any(test, feature = "screenshots"))]
        {
            self.rehearsal_face = None;
        }
        self.live_inputs.clear();
        self.rows.clear();
    }
    fn reviewing(&self) -> bool {
        self.stage == self.capture.exercises.len() + 1
    }
    fn duration(&self) -> f64 {
        if self.stage == 1 { 8.0 } else { self.seconds }
    }
    pub fn observe(&mut self, sequence: u64, now: f64, live: bool, inputs: &Inputs) {
        if !self.open {
            return;
        }
        self.now = now;
        #[cfg(any(test, feature = "screenshots"))]
        if let Some(face) = &self.rehearsal_face {
            self.live = true;
            self.live_inputs.clone_from(face);
            return;
        }
        self.live = live;
        if live {
            self.live_inputs.clone_from(inputs);
        }
        if self.stage == 0 || self.reviewing() {
            return;
        }
        if let Some(start) = self.countdown {
            if !live {
                self.countdown = Some(now);
                return;
            }
            if now - start < self.prepare_seconds {
                return;
            }
            self.countdown = None;
            self.capture.begin(self.stage - 1);
        }
        if self
            .capture
            .sample(sequence, now, live, inputs, self.duration())
        {
            self.notice = Some("Take captured — compare it below or try again.".into());
        }
    }
    fn refresh_rows(&mut self, all: bool) {
        let changed = (!all).then(|| self.capture.exercises[self.stage - 1].inputs.clone());
        let mut proposals = self.capture.proposals();
        if let Some(changed) = changed {
            for old in std::mem::take(&mut self.rows) {
                if !changed.contains(&old.input)
                    && let Some(new) = proposals.iter_mut().find(|p| p.input == old.input)
                {
                    *new = old;
                }
            }
        }
        self.rows = proposals;
    }
    fn next(&mut self) {
        self.capture.cancel_take();
        self.countdown = None;
        self.preview = false;
        self.notice = None;
        if self.return_to_review && self.stage == 1 && self.capture.captured() <= 1 {
            self.rows.clear();
            self.return_to_review = false;
            self.stage = 2;
        } else if self.return_to_review {
            self.refresh_rows(self.stage == 1);
            self.stage = self.capture.exercises.len() + 1;
            self.return_to_review = false;
        } else {
            self.stage += 1;
            if self.reviewing() {
                self.rows = self.capture.proposals();
            }
        }
    }
    fn skip(&mut self) {
        if self.stage <= 1 {
            return;
        }
        self.capture.select(self.stage - 1, None);
        self.next();
    }
    fn prepare(&mut self) {
        self.capture.cancel_take();
        self.preview = false;
        self.notice = None;
        self.countdown = Some(self.now);
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
    fn is_rehearsal(&self) -> bool {
        #[cfg(any(test, feature = "screenshots"))]
        {
            self.rehearsal_face.is_some()
        }
        #[cfg(not(any(test, feature = "screenshots")))]
        {
            false
        }
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
            .open(&mut open)
            .default_width(760.0).min_width(600.0)
            .default_height((ctx.content_rect().height() - 160.0).clamp(360.0, 780.0))
            .default_pos(egui::pos2((ctx.content_rect().width() - 800.0).max(20.0), 80.0))
            .resizable(true).collapsible(true).show(ctx, |ui| {
                help::label(ui, "Personal tracking setup", "tracking-guide");
                theme::caption(ui, "One movement at a time · Repeat any take · Continue when you are ready");
                if !live_pose {
                    ui.colored_label(egui::Color32::LIGHT_YELLOW, "A held pose can hide tracking changes.");
                    if ui.button("Resume live movement").clicked() { action = Some(Action::Resume); }
                }
                if microphone { theme::caption(ui, "Microphone control may override facial mouth tracking. Turn it off in Microphone to check facial lip sync."); }
                ui.label(format!("Tracker: {status}"));
                ui.separator();
                let footer = if self.stage > 0 && !self.reviewing() { 86.0 } else { 42.0 };
                egui::ScrollArea::vertical().id_salt("personal-guide-body")
                    .max_height((ui.available_height() - footer).max(140.0))
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        let exercise = self.stage.checked_sub(1).and_then(|i| self.capture.exercises.get(i));
                        let cue = exercise.map_or("neutral", |e| e.id.as_str());
                        face::show(ui, &self.live_inputs, self.live, cue, self.is_rehearsal());
                        ui.add_space(8.0);
                        match self.stage {
                            0 => self.connection_ui(ui, &mut action),
                            _ if self.reviewing() => {
                                self.review_ui(ui);
                                if self.reviewing() { self.review_actions(ui, &mut action, live_pose); }
                            }
                            _ => self.exercise_ui(ui),
                        }
                    });
                ui.separator();
                if self.stage > 0 && !self.reviewing() { self.exercise_actions(ui); }
                if ui.button("Cancel / keep saved settings").clicked() { cancel = true; }
            });
        if !open || cancel || !self.open {
            self.close();
        }
        if self.countdown.is_some() || self.capture.active.is_some() || self.live {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        action
    }
    fn connection_ui(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.heading("Connect your face tracker");
        ui.label("Place your camera or phone at eye level, with even lighting and your usual streaming distance. The illustrated face above should follow you before you begin.");
        ui.collapsing("Connection instructions", |ui| {
            ui.label("Webcam: choose MediaPipe or NVIDIA RTX in Tracking, install its runtime, choose a camera and press Start camera.");
            ui.label("iPhone VTube Studio: enable 3rd Party PC Clients, enter the phone's IPv4 address in ARIA, and connect. Both devices need a reachable network.");
        });
        if ui.button("Open connection settings").clicked() {
            *action = Some(Action::Connection);
        }
        theme::caption(
            ui,
            format!(
                "{} model assignments found. Their output limits, directions, expressions and physics are preserved.",
                self.bindings.values().map(Vec::len).sum::<usize>()
            ),
        );
        if ui
            .checkbox(
                &mut self.only_assigned,
                "Only exercises used by this avatar's assignments",
            )
            .changed()
        {
            let used: Vec<_> = self.bindings.keys().cloned().collect();
            self.capture = Session::new(
                self.capture.known_names().to_vec(),
                self.only_assigned.then_some(used.as_slice()),
            );
        }
        self.timing_ui(ui);
        theme::caption(
            ui,
            format!(
                "{} individual exercises including neutral. Every take stops for review. You can skip unavailable movements and retain your existing settings.",
                self.capture.exercises.len()
            ),
        );
        if ui
            .add_enabled(
                self.live && self.capture.exercises.len() > 1,
                egui::Button::new("Begin with neutral pose"),
            )
            .clicked()
        {
            self.stage = 1;
        }
        theme::caption(
            ui,
            "Demo animation and microphone-only input cannot calibrate a face. No camera video or face images are recorded by this guide.",
        );
        self.unmapped_ui(ui);
    }
    fn timing_ui(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Capture timing", |ui| {
            ui.add(egui::Slider::new(&mut self.prepare_seconds, 3.0..=10.0).integer().text("Preparation seconds"));
            ui.add(egui::Slider::new(&mut self.seconds, 8.0..=30.0).integer().text("Movement seconds per take"));
            theme::caption(ui, "Default: 5 seconds to prepare, then 12 seconds for one movement. Neutral uses 8 seconds. Face loss pauses recording. Finishing never opens another exercise automatically.");
        });
    }
    fn exercise_ui(&mut self, ui: &mut egui::Ui) {
        let index = self.stage - 1;
        let exercise = &self.capture.exercises[index];
        theme::caption(
            ui,
            format!(
                "EXERCISE {} OF {} · {} exercises with a selected take",
                index + 1,
                self.capture.exercises.len(),
                self.capture.captured()
            ),
        );
        ui.heading(&exercise.title);
        ui.label(&exercise.instruction);
        if index == 0 && self.capture.captured() > 1 {
            ui.colored_label(egui::Color32::LIGHT_YELLOW, "Selecting a different neutral take resets the draft's movement takes. Saved settings remain intact.");
        }
        if let Some(start) = self.countdown {
            ui.heading(if self.live {
                format!(
                    "Get ready… {}",
                    (self.prepare_seconds - (self.now - start)).ceil().max(1.0) as u32
                )
            } else {
                "Waiting for a live face…".into()
            });
        } else if let Some((_, take)) = &self.capture.active {
            ui.add(
                egui::ProgressBar::new((take.elapsed / self.duration()).min(1.0) as f32).text(
                    format!(
                        "{:.1} / {:.0} seconds · {} fresh samples",
                        take.elapsed,
                        self.duration(),
                        take.samples
                    ),
                ),
            );
            if !self.live {
                ui.colored_label(
                    egui::Color32::LIGHT_YELLOW,
                    "Paused — face or connection lost. Return to the camera to continue.",
                );
            }
        } else {
            theme::caption(
                ui,
                format!(
                    "{:.0} seconds to prepare → {:.0} seconds to capture → review your take",
                    self.prepare_seconds,
                    self.duration()
                ),
            );
        }
        let busy = self.countdown.is_some() || self.capture.active.is_some();
        if busy {
            if ui.button("Stop this take").clicked() {
                self.capture.cancel_take();
                self.countdown = None;
            }
        } else {
            if let Some(message) = &self.notice {
                ui.colored_label(theme::accent("Tracking"), message);
            }
            let has_room = self.capture.takes(index).len() < MAX_TAKES;
            if ui
                .add_enabled(
                    self.live && has_room,
                    egui::Button::new(if self.capture.takes(index).is_empty() {
                        "Record a take"
                    } else {
                        "Record another take"
                    }),
                )
                .clicked()
            {
                self.prepare();
            }
            if !has_room {
                theme::caption(
                    ui,
                    "Five takes are kept for this exercise. Remove one below to record another.",
                );
            }
            self.takes_ui(ui, index);
            self.timing_ui(ui);
        }
        ui.collapsing("Live input values for this exercise", |ui| {
            for name in &self.capture.exercises[index].inputs {
                ui.horizontal(|ui| {
                    ui.monospace(name);
                    if self.live
                        && let Some(value) = self.live_inputs.get(name)
                    {
                        ui.label(format!("{value:.3}"));
                    } else {
                        ui.weak("No live value");
                    }
                });
            }
        });
        theme::caption(
            ui,
            "Move comfortably and briefly hold the requested pose. Other movements are measured on their own pages. Unsupported or weak signals remain unchanged.",
        );
    }
    fn exercise_actions(&mut self, ui: &mut egui::Ui) {
        let index = self.stage - 1;
        if self.countdown.is_some() || self.capture.active.is_some() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.capture.can_continue(index),
                    egui::Button::new(
                        if self.return_to_review && index == 0 && self.capture.captured() <= 1 {
                            "Use neutral & recapture movements"
                        } else if self.return_to_review {
                            "Use selected take / return to review"
                        } else {
                            "Use selected take & continue"
                        },
                    ),
                )
                .clicked()
            {
                self.next();
            }
            if index > 0 && ui.button("Skip / keep existing range").clicked() {
                self.skip();
            }
        });
    }
    fn takes_ui(&mut self, ui: &mut egui::Ui, index: usize) {
        if self.capture.takes(index).is_empty() {
            return;
        }
        help::label(ui, "Compare your takes", "tracking-guide-takes");
        let recommended = self.capture.recommended(index);
        let mut selected = self.capture.selected(index);
        let mut remove = None;
        for (ordinal, take) in self.capture.takes(index).iter().enumerate() {
            let quality = self.capture.quality(index, take);
            ui.horizontal_wrapped(|ui| {
                ui.radio_value(
                    &mut selected,
                    Some(take.id),
                    format!(
                        "Take {} · {}{}",
                        ordinal + 1,
                        quality.label(),
                        if recommended == Some(take.id) {
                            " · suggested"
                        } else {
                            ""
                        }
                    ),
                );
                ui.weak(format!(
                    "{} usable / {} measured inputs",
                    quality.usable, quality.measured
                ));
                if ui.small_button("Remove").clicked() {
                    remove = Some(take.id);
                }
            });
        }
        if let Some(id) = recommended
            && ui.button("Select suggested take").clicked()
        {
            selected = Some(id);
        }
        self.capture.select(index, selected);
        if let Some(id) = remove {
            self.capture.remove(index, id);
        }
        theme::caption(
            ui,
            "Only the selected take contributes to your range. The suggestion favors clear movement above resting noise and usable signals; check it on your avatar before saving.",
        );
    }
    fn unmapped_ui(&self, ui: &mut egui::Ui) {
        ui.collapsing(format!("{} parameters without tracking assignments", self.unmapped.len()), |ui| {
            ui.label("Many parameters belong to physics, clothing or expressions and should stay unassigned. For a custom face control, choose its input in Inspector → Tracking → Inputs, then rerun this guide. ARIA cannot infer an arbitrary rig's meaning from geometry alone.");
            for name in &self.unmapped { ui.monospace(name); }
        });
    }
    fn review_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Review and try it on your avatar");
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
        ui.collapsing("Retake an individual exercise", |ui| {
            let mut retry = None;
            for (index, exercise) in self.capture.exercises.iter().enumerate() {
                let label = if index == 0 {
                    "Neutral pose (changing its selected take resets movement takes)"
                } else {
                    &exercise.title
                };
                if ui.button(label).clicked() {
                    retry = Some(index);
                }
            }
            if let Some(index) = retry {
                self.stage = index + 1;
                self.return_to_review = true;
                self.preview = false;
                self.notice = None;
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
    /// Deterministic synthetic rehearsal through the real per-exercise take path.
    pub fn rehearsal(&mut self) {
        self.stage = 1;
        let mut sequence = 1;
        for index in 0..self.capture.exercises.len() {
            assert_eq!(self.stage, index + 1);
            let id = self.capture.exercises[index].id.clone();
            let count = if id == "yaw-left" { 2 } else { 1 };
            for _ in 0..count {
                self.capture.begin(index);
                let frames = (self.duration() * 60.0) as usize + 10;
                for n in 0..frames {
                    let inputs = self.trace_inputs(&id, n > 45 && n < frames - 45);
                    self.observe(sequence, sequence as f64 / 60.0, true, &inputs);
                    sequence += 1;
                    if self.capture.active.is_none() {
                        break;
                    }
                }
                assert_eq!(
                    self.stage,
                    index + 1,
                    "Finishing a take must wait for the user"
                );
            }
            if let Some(best) = self.capture.recommended(index) {
                self.capture.select(index, Some(best));
            }
            self.next();
        }
        assert!(self.reviewing());
        assert!(self.draft().validate().is_ok());
        self.rehearsal_face = Some(self.trace_inputs("yaw-left", true));
        self.live_inputs = self.rehearsal_face.clone().unwrap();
        self.live = true;
    }
    #[cfg(feature = "screenshots")]
    pub fn rehearsal_take(&mut self) {
        self.rehearsal();
        for index in 2..self.capture.exercises.len() {
            self.capture.select(index, None);
        }
        self.stage = 2;
        self.return_to_review = false;
        self.notice = None;
    }
    fn trace_inputs(&self, id: &str, active: bool) -> Inputs {
        let mut frame = aria_core::TrackingFrame {
            face_found: true,
            ..Default::default()
        };
        for name in self.capture.known_names() {
            if let Some(key) = name.strip_prefix("ARKit:") {
                frame.blend_shapes.insert(key.into(), 0.0);
            }
        }
        frame.rotation.y = 4.0;
        frame.blend_shapes.insert("jawopen".into(), 0.1);
        if active {
            let keys: &[&str] = match id {
                "yaw-left" => {
                    frame.rotation.y = -18.0;
                    &[]
                }
                "yaw-right" => {
                    frame.rotation.y = 46.0;
                    &[]
                }
                "pitch-up" => {
                    frame.rotation.x = 18.0;
                    &[]
                }
                "pitch-down" => {
                    frame.rotation.x = -18.0;
                    &[]
                }
                "roll-left" => {
                    frame.rotation.z = -16.0;
                    &[]
                }
                "roll-right" => {
                    frame.rotation.z = 16.0;
                    &[]
                }
                "eye-left" => &["eyeblinkleft"],
                "eye-right" => &["eyeblinkright"],
                "mouth-open" => &["jawopen"],
                "smile" => &["mouthsmileleft", "mouthsmileright"],
                "frown" => &["mouthfrownleft", "mouthfrownright"],
                "brows-up" => &["browinnerup", "browouterupleft", "browouterupright"],
                "brows-down" => &["browdownleft", "browdownright"],
                "mouth-left" => &["mouthleft"],
                "mouth-right" => &["mouthright"],
                "pucker" => &["mouthpucker"],
                "funnel" => &["mouthfunnel"],
                "shrug" => &["mouthshrugupper", "mouthshruglower"],
                "press" => &["mouthpressleft", "mouthpressright"],
                "cheek-puff" => &["cheekpuff"],
                "tongue" => &["tongueout"],
                "gaze-left" => &["eyelookoutleft", "eyelookinright"],
                "gaze-right" => &["eyelookinleft", "eyelookoutright"],
                "gaze-up" => &["eyelookupleft", "eyelookupright"],
                "gaze-down" => &["eyelookdownleft", "eyelookdownright"],
                _ => &[],
            };
            for key in keys {
                frame
                    .blend_shapes
                    .insert((*key).into(), if *key == "jawopen" { 0.75 } else { 0.8 });
            }
            if let Some(key) = id.strip_prefix("ARKit:") {
                frame.blend_shapes.insert(key.into(), 0.8);
            }
        }
        rig::tracking_inputs(
            Some(&frame),
            aria_core::ParameterPipeline::default().measure(Some(&frame), &Default::default()),
            false,
            0.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_countdown_waits_for_live_face_and_completion_waits_for_user() {
        let parameters = aria_core::movement::preview_parameters(Default::default());
        let config = aria_core::movement::RigConfig::from_parameters(&parameters);
        let mut guide = Guide::default();
        guide.start(
            "a".into(),
            &config,
            &parameters,
            &Inputs::new(),
            Default::default(),
        );
        guide.stage = 1;
        guide.prepare();
        let inputs = guide.trace_inputs("neutral", false);
        guide.observe(1, 10.0, false, &inputs);
        guide.observe(2, 14.0, true, &inputs);
        assert!(guide.capture.active.is_none());
        guide.observe(3, 15.0, true, &inputs);
        assert!(guide.capture.active.is_some());
        for n in 0..=500 {
            guide.observe(n + 4, 15.0 + n as f64 / 60.0, true, &inputs);
        }
        assert!(guide.capture.active.is_none());
        assert_eq!(guide.capture.takes(0).len(), 1);
        assert_eq!(guide.stage, 1);
        guide.next();
        assert_eq!(guide.stage, 2);
    }
    #[test]
    fn movement_retry_preserves_unrelated_manual_review_edits_and_close_frees_takes() {
        let parameters = aria_core::movement::preview_parameters(Default::default());
        let config = aria_core::movement::RigConfig::from_parameters(&parameters);
        let mut guide = Guide::default();
        guide.start(
            "a".into(),
            &config,
            &parameters,
            &Inputs::new(),
            Default::default(),
        );
        guide.rehearsal();
        let mouth = guide
            .rows
            .iter_mut()
            .find(|r| r.input == "JawOpen")
            .unwrap();
        mouth.range.as_mut().unwrap().high = 0.95;
        guide.stage = 2;
        guide.return_to_review = true;
        guide.next();
        assert!(guide.reviewing());
        assert_eq!(
            guide
                .rows
                .iter()
                .find(|r| r.input == "JawOpen")
                .unwrap()
                .range
                .as_ref()
                .unwrap()
                .high,
            0.95
        );
        guide.close();
        assert!(guide.capture.exercises.is_empty());
        assert!(guide.live_inputs.is_empty());
        assert!(guide.profile().is_none());
    }
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
    fn complete_tour_preserves_model_mapping_and_skips_only_selected_exercise() {
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
        let gaze = guide
            .capture
            .exercises
            .iter()
            .position(|e| e.id == "gaze-left")
            .unwrap();
        guide.stage = gaze + 1;
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
