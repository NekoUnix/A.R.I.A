//! Independent live avatar sessions and the persistent workspace catalog.
//! The editor borrows one session at a time; captures and render resources stay owned.
use super::*;
use std::collections::VecDeque;
#[cfg(all(test, windows))]
mod native_tests;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Workspace {
    pub(super) entries: Vec<Entry>,
    active: Option<u64>,
    next_id: u64,
}
impl Workspace {
    fn source(&self, id: u64) -> Option<u64> {
        let mut current = id;
        let mut visited = std::collections::BTreeSet::new();
        while visited.insert(current) {
            let entry = self.entries.iter().find(|e| e.id == current)?;
            if !entry.enabled {
                return None;
            }
            match entry.follow {
                Some(next) => current = next,
                None => return Some(current),
            }
        }
        None
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Entry {
    id: u64,
    name: String,
    path: PathBuf,
    key: String,
    #[serde(default)]
    kind: Option<crate::avatar_import::Kind>,
    enabled: bool,
    #[serde(default)]
    follow: Option<u64>,
    rig: SavedRig,
    preferences: ModelPreferences,
}
#[derive(Default)]
pub(super) struct Sessions {
    pub(super) current: Option<u64>,
    pub(super) parked: BTreeMap<u64, AvatarState>,
    actions: VecDeque<Action>,
    loading: Option<u64>,
    previous: Option<u64>,
    restore: VecDeque<u64>,
    restore_active: Option<u64>,
    errors: BTreeMap<u64, String>,
}
enum Action {
    Focus(u64),
    Load(u64),
    Unload(u64),
    Add,
    Remove(u64),
    Order(u64, bool),
}
pub(super) struct AvatarState {
    preferences: ModelPreferences,
    image_avatar: Option<PathBuf>,
    vrm_avatar: Option<PathBuf>,
    camera: crate::webcam::Camera,
    tracking_guide: crate::tracking_guide::Guide,
    tracking_filter: aria_core::calibration::Filter,
    speech_filter: aria_core::speech::Filter,
    images: crate::image_actions::Images,
    microphone: crate::microphone::Microphone,
    controller: crate::controller::Controller,
    effects: crate::effects::Effects,
    receiver: Option<Receiver>,
    snapshot: Snapshot,
    pipeline: ParameterPipeline,
    raw: Option<TrackingFrame>,
    params: Parameters,
    status_message: Option<String>,
    input_monitor: InputMonitor,
    items: crate::items::Items,
    live_inputs: Inputs,
    animation_time: f32,
    idle: Option<Sprite>,
    talking: Option<Sprite>,
    model: Option<ModelReport>,
    model_open: bool,
    live2d: Option<crate::live2d::Avatar>,
    vrm: Option<crate::vrm::Avatar>,
}
impl Default for AvatarState {
    fn default() -> Self {
        let parameters = movement::preview_parameters(Parameters::default());
        Self {
            preferences: ModelPreferences::default(),
            image_avatar: None,
            vrm_avatar: None,
            camera: Default::default(),
            tracking_guide: Default::default(),
            tracking_filter: Default::default(),
            speech_filter: Default::default(),
            images: Default::default(),
            microphone: Default::default(),
            controller: Default::default(),
            effects: Default::default(),
            receiver: Default::default(),
            snapshot: Default::default(),
            pipeline: Default::default(),
            raw: Default::default(),
            params: Default::default(),
            status_message: Default::default(),
            input_monitor: InputMonitor::new(
                "preview-v1".into(),
                RigConfig::from_parameters(&parameters),
                None,
                &parameters,
            ),
            items: Default::default(),
            live_inputs: Default::default(),
            animation_time: Default::default(),
            idle: Default::default(),
            talking: Default::default(),
            model: Default::default(),
            model_open: Default::default(),
            live2d: Default::default(),
            vrm: Default::default(),
        }
    }
}
impl AvatarState {
    fn exchange(&mut self, app: &mut AriaApp) {
        let mut preferences = ModelPreferences::capture(&app.settings);
        preferences.calibration = app.pipeline.calibration();
        // Outputs, frame pacing and native window policy belong to the workspace.
        let outputs = app.settings.outputs.clone();
        let fps = app.settings.fps;
        let on_top = app.settings.always_on_top;
        self.preferences.restore(&mut app.settings);
        app.settings.outputs = outputs;
        app.settings.fps = fps;
        app.settings.always_on_top = on_top;
        self.preferences = preferences;
        std::mem::swap(&mut self.image_avatar, &mut app.settings.image_avatar);
        std::mem::swap(&mut self.vrm_avatar, &mut app.settings.vrm_avatar);
        std::mem::swap(&mut self.camera, &mut app.camera);
        std::mem::swap(&mut self.tracking_guide, &mut app.tracking_guide);
        std::mem::swap(&mut self.tracking_filter, &mut app.tracking_filter);
        std::mem::swap(&mut self.speech_filter, &mut app.speech_filter);
        std::mem::swap(&mut self.images, &mut app.images);
        std::mem::swap(&mut self.microphone, &mut app.microphone);
        std::mem::swap(&mut self.controller, &mut app.controller);
        std::mem::swap(&mut self.effects, &mut app.effects);
        std::mem::swap(&mut self.receiver, &mut app.receiver);
        std::mem::swap(&mut self.snapshot, &mut app.snapshot);
        std::mem::swap(&mut self.pipeline, &mut app.pipeline);
        std::mem::swap(&mut self.raw, &mut app.raw);
        std::mem::swap(&mut self.params, &mut app.params);
        std::mem::swap(&mut self.status_message, &mut app.status_message);
        std::mem::swap(&mut self.input_monitor, &mut app.input_monitor);
        std::mem::swap(&mut self.items, &mut app.items);
        std::mem::swap(&mut self.live_inputs, &mut app.live_inputs);
        std::mem::swap(&mut self.animation_time, &mut app.animation_time);
        std::mem::swap(&mut self.idle, &mut app.idle);
        std::mem::swap(&mut self.talking, &mut app.talking);
        std::mem::swap(&mut self.model, &mut app.model);
        std::mem::swap(&mut self.model_open, &mut app.model_open);
        std::mem::swap(&mut self.live2d, &mut app.live2d);
        std::mem::swap(&mut self.vrm, &mut app.vrm);
    }
    pub(super) fn process_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.live2d
            .iter()
            .map(|a| a.model.process_id())
            .chain(self.items.models.process_ids())
            .chain(self.effects.process_ids())
            .chain(self.camera.process_ids())
    }
}

fn short_name(name: &str) -> String {
    let mut text: String = name.chars().take(24).collect();
    if name.chars().count() > 24 {
        text.push('…');
    }
    text
}
impl AriaApp {
    pub(super) fn profile_name(&self) -> String {
        if self.profiles.current.is_none() && !self.settings.profiles.entries.is_empty() {
            return "No avatar selected".into();
        }
        self.settings
            .profiles
            .entries
            .iter()
            .find(|e| Some(e.id) == self.profiles.current)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| {
                self.live2d
                    .as_ref()
                    .map(|a| a.name.clone())
                    .or_else(|| self.vrm.as_ref().map(|a| a.asset.summary.name.clone()))
                    .or_else(|| self.idle.as_ref().map(|a| a.name.clone()))
                    .unwrap_or_else(|| "Mica · test stage".into())
            })
    }
    pub(super) fn following_profile(&self) -> Option<u64> {
        self.settings
            .profiles
            .entries
            .iter()
            .find(|e| Some(e.id) == self.profiles.current)
            .and_then(|e| e.follow)
    }
    pub(super) fn with_profile<R>(
        &mut self,
        id: u64,
        work: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        if self.profiles.current == Some(id) {
            return Some(work(self));
        }
        let mut state = self.profiles.parked.remove(&id)?;
        state.exchange(self);
        let current = self.profiles.current.replace(id);
        let result = work(self);
        self.profiles.current = current;
        state.exchange(self);
        self.profiles.parked.insert(id, state);
        Some(result)
    }
    pub(super) fn remember_profiles(&mut self) {
        let mut preferences = ModelPreferences::capture(&self.settings);
        preferences.calibration = self.pipeline.calibration();
        for entry in &mut self.settings.profiles.entries {
            if Some(entry.id) == self.profiles.current {
                entry.rig = self.input_monitor.saved.clone();
                entry.preferences = preferences.clone();
            } else if let Some(state) = self.profiles.parked.get(&entry.id) {
                entry.rig = state.input_monitor.saved.clone();
                entry.preferences = state.preferences.clone();
                entry.preferences.calibration = state.pipeline.calibration();
            }
        }
        self.settings.profiles.active = self.profiles.current;
    }
    pub(super) fn prepare_profile_import(&mut self) {
        self.remember_current_rig();
        self.profiles.previous = self.profiles.current.take();
        let mut old = AvatarState::default();
        old.exchange(self);
        if let Some(id) = self.profiles.previous {
            self.profiles.parked.insert(id, old);
        }
        // Identical avatar files can exist at different paths. Preserve the incoming
        // profile's saved settings after recording the outgoing content identity.
        if let Some(entry) = self
            .settings
            .profiles
            .entries
            .iter()
            .find(|e| Some(e.id) == self.profiles.loading)
        {
            self.settings
                .saved_rigs
                .insert(entry.key.clone(), entry.rig.clone());
            self.settings
                .model_preferences
                .insert(entry.key.clone(), entry.preferences.clone());
        }
        self.effect_api.invalidate_model();
    }
    pub(super) fn register_profile(&mut self, path: PathBuf) {
        let kind = self.avatar_kind();
        let existing = self.profiles.loading.take().or_else(|| {
            self.settings
                .profiles
                .entries
                .iter()
                .find(|e| e.path == path)
                .map(|e| e.id)
        });
        let id = existing.unwrap_or_else(|| {
            self.settings.profiles.next_id = self.settings.profiles.next_id.max(
                self.settings
                    .profiles
                    .entries
                    .iter()
                    .map(|e| e.id)
                    .max()
                    .unwrap_or(0),
            ) + 1;
            self.settings.profiles.next_id
        });
        if let Some(entry) = self
            .settings
            .profiles
            .entries
            .iter_mut()
            .find(|e| e.id == id)
        {
            entry.enabled = true;
            entry.path = path;
            entry.key = self.input_monitor.model_key.clone();
            entry.kind = kind;
        } else {
            let follow = self.profiles.previous.map(|previous| {
                self.settings
                    .profiles
                    .entries
                    .iter()
                    .find(|e| e.id == previous)
                    .and_then(|e| e.follow)
                    .unwrap_or(previous)
            });
            self.settings.profiles.entries.push(Entry {
                id,
                name: self
                    .live2d
                    .as_ref()
                    .map(|a| a.name.clone())
                    .or_else(|| self.vrm.as_ref().map(|a| a.asset.summary.name.clone()))
                    .or_else(|| self.idle.as_ref().map(|a| a.name.clone()))
                    .unwrap_or_else(|| "Mica".into()),
                path,
                key: self.input_monitor.model_key.clone(),
                kind,
                enabled: true,
                follow,
                rig: self.input_monitor.saved.clone(),
                preferences: ModelPreferences::capture(&self.settings),
            });
        }
        self.profiles.parked.remove(&id);
        self.profiles.current = Some(id);
        self.profiles.errors.remove(&id);
        self.settings.profiles.active = Some(id);
        self.outputs.ensure_avatar(id, self.profiles.parked.len());
        self.scene_revision = self.scene_revision.wrapping_add(1);
        self.input_monitor.save_requested = true;
    }
    pub(super) fn queue_profile_restore(&mut self) {
        if crate::smoke_mode() || std::env::args_os().nth(1).is_some() {
            return;
        }
        self.profiles.restore_active = self.settings.profiles.active;
        self.profiles.restore = self
            .settings
            .profiles
            .entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.id)
            .collect();
    }
    pub(super) fn load_profile(&mut self, ctx: &egui::Context, id: u64) {
        if self.profiles.current == Some(id) || self.profiles.parked.contains_key(&id) {
            self.focus_profile(id);
            return;
        }
        let Some(entry) = self
            .settings
            .profiles
            .entries
            .iter()
            .find(|e| e.id == id)
            .cloned()
        else {
            return;
        };
        self.settings
            .saved_rigs
            .insert(entry.key.clone(), entry.rig);
        self.settings
            .model_preferences
            .insert(entry.key, entry.preferences);
        self.profiles.loading = Some(id);
        self.status_message = None;
        if entry.path == Path::new("builtin:mica") {
            self.prepare_profile_import();
            self.use_preview_rig();
            self.register_profile(entry.path);
            return;
        }
        if entry.kind == Some(crate::avatar_import::Kind::Images)
            || entry.path.extension().is_some_and(|ext| {
                ["png", "gif", "jpg", "jpeg"]
                    .iter()
                    .any(|e| ext.eq_ignore_ascii_case(e))
            })
        {
            self.open_image(ctx, &entry.path, false);
        } else {
            self.open_model(&entry.path);
        }
        self.finish_profile_load();
    }
    pub(super) fn finish_profile_load(&mut self) {
        if self.pending_image.is_some() || self.pending_vrm.is_some() {
            return;
        }
        if let Some(id) = self.profiles.loading.take() {
            let error = self.status_message.clone().unwrap_or_else(|| {
                "Choose the missing asset or finish the guided import, then try again.".into()
            });
            self.profiles.errors.insert(id, error);
            if let Some(entry) = self
                .settings
                .profiles
                .entries
                .iter_mut()
                .find(|e| e.id == id)
            {
                entry.enabled = false;
            }
            self.input_monitor.save_requested = true;
        }
    }
    pub(super) fn focus_profile(&mut self, id: u64) {
        if self.profiles.current == Some(id) {
            return;
        }
        let Some(mut state) = self.profiles.parked.remove(&id) else {
            return;
        };
        self.remember_current_rig();
        state.exchange(self);
        if let Some(old) = self.profiles.current.replace(id) {
            self.profiles.parked.insert(old, state);
        }
        self.settings.profiles.active = Some(id);
        self.effect_api.invalidate_model();
        self.input_monitor.save_requested = true;
    }
    pub(super) fn unload_profile(&mut self, id: u64) {
        self.remember_current_rig();
        if self.profiles.current == Some(id) {
            if let Some(next) = self.profiles.parked.keys().next().copied() {
                self.focus_profile(next);
            } else {
                let mut old = AvatarState::default();
                old.exchange(self);
                self.profiles.current = None;
            }
        }
        self.profiles.parked.remove(&id);
        if let Some(entry) = self
            .settings
            .profiles
            .entries
            .iter_mut()
            .find(|e| e.id == id)
        {
            entry.enabled = false;
        }
        self.scene_revision = self.scene_revision.wrapping_add(1);
        self.input_monitor.save_requested = true;
    }
    pub(super) fn process_profile_actions(&mut self, ctx: &egui::Context) {
        if ctx.current_pass_index() != 0
            || self.pending_image.is_some()
            || self.pending_vrm.is_some()
        {
            return;
        }
        if let Some(action) = self.profiles.actions.pop_front() {
            match action {
                Action::Focus(id) => self.focus_profile(id),
                Action::Load(id) => self.load_profile(ctx, id),
                Action::Unload(id) => self.unload_profile(id),
                Action::Add => {
                    self.controls_page = ControlsPage::Avatar;
                    self.importer.start(None);
                }
                Action::Remove(id) => {
                    self.unload_profile(id);
                    self.settings.profiles.entries.retain(|e| e.id != id);
                    self.profiles.errors.remove(&id);
                    self.outputs.forget_avatar(id);
                }
                Action::Order(id, front) => {
                    let entries = &mut self.settings.profiles.entries;
                    if let Some(index) = entries.iter().position(|e| e.id == id) {
                        let next = if front {
                            (index + 1).min(entries.len() - 1)
                        } else {
                            index.saturating_sub(1)
                        };
                        entries.swap(index, next);
                        self.scene_revision = self.scene_revision.wrapping_add(1);
                        self.input_monitor.save_requested = true;
                    }
                }
            }
        } else if let Some(id) = self.profiles.restore.pop_front() {
            self.load_profile(ctx, id);
        } else if let Some(id) = self.profiles.restore_active.take() {
            self.focus_profile(id);
        }
    }
    pub(super) fn sample_own_tracking(&mut self) {
        if self.settings.source == Source::Demo {
            self.raw = Some(demo_frame(self.started.elapsed().as_secs_f32()));
        } else if matches!(self.settings.source, Source::Webcam | Source::Rtx) {
            self.snapshot = self.camera.snapshot();
            self.raw = self.snapshot.fresh_frame().cloned();
        } else if let Some(receiver) = &self.receiver {
            self.snapshot = receiver.snapshot();
            self.raw = self.snapshot.fresh_frame().cloned();
        } else {
            self.raw = None;
        }
    }
    pub(super) fn sample_profile_tracking(&mut self, dt: f32) {
        if dt <= 0. {
            return;
        }
        let ids: Vec<_> = self
            .profiles
            .parked
            .keys()
            .copied()
            .chain(self.profiles.current)
            .collect();
        let mut frames = BTreeMap::new();
        if self.profiles.current.is_none() {
            self.sample_own_tracking();
        }
        for id in &ids {
            if let Some(frame) = self
                .with_profile(*id, |app| {
                    if app.following_profile().is_none() {
                        app.sample_own_tracking();
                        Some((app.snapshot.clone(), app.raw.clone()))
                    } else {
                        None
                    }
                })
                .flatten()
            {
                frames.insert(*id, frame);
            }
        }
        for id in ids {
            self.with_profile(id, |app| {
                if app.following_profile().is_some() {
                    app.camera.stop();
                    app.receiver = None;
                    let frame = app
                        .settings
                        .profiles
                        .source(id)
                        .and_then(|source| frames.get(&source))
                        .cloned();
                    let (snapshot, raw) = frame.unwrap_or_default();
                    app.snapshot = snapshot;
                    app.raw = raw;
                }
            });
        }
    }
    pub(super) fn workspace_hotkeys(&self) -> Vec<crate::hotkeys::Registration> {
        let mut grouped: BTreeMap<
            aria_core::shortcuts::Shortcut,
            Vec<(u64, crate::hotkeys::Action)>,
        > = BTreeMap::new();
        for (id, monitor) in
            std::iter::once((self.profiles.current.unwrap_or(0), &self.input_monitor)).chain(
                self.profiles
                    .parked
                    .iter()
                    .map(|(id, a)| (*id, &a.input_monitor)),
            )
        {
            for key in monitor.hotkey_keys() {
                grouped
                    .entry(key.shortcut)
                    .or_default()
                    .push((id, key.action));
            }
        }
        grouped
            .into_iter()
            .map(|(shortcut, mut actions)| {
                actions.sort();
                actions.dedup();
                crate::hotkeys::Registration {
                    shortcut,
                    action: crate::hotkeys::Action::Profiles(actions),
                }
            })
            .collect()
    }
    pub(super) fn dispatch_hotkey(&mut self, action: crate::hotkeys::Action) {
        if let crate::hotkeys::Action::Profiles(actions) = action {
            for (id, action) in actions {
                if id == 0 && self.profiles.current.is_none() {
                    self.dispatch_hotkey(action);
                } else {
                    self.with_profile(id, |app| app.dispatch_hotkey(action));
                }
            }
        } else if self.input_monitor.saved.global_hotkeys {
            let parameters = self.current_parameters();
            self.input_monitor
                .hotkey_action(action, &parameters, &mut self.settings.mapping);
        }
    }
    pub(super) fn advance_profiles(&mut self, ctx: &egui::Context, dt: f32) {
        self.advance_avatar(ctx, dt);
        let ids: Vec<_> = self.profiles.parked.keys().copied().collect();
        let mut dirty = false;
        for id in ids {
            self.with_profile(id, |app| {
                if std::mem::take(&mut app.input_monitor.reset_item_rules) {
                    app.items.reset_rules();
                }
                app.effects
                    .pending
                    .extend(std::mem::take(&mut app.input_monitor.effect_requests));
                app.advance_avatar(ctx, dt);
                dirty |= std::mem::take(&mut app.input_monitor.save_requested)
                    | app.items.take_save()
                    | app.effects.take_save();
            });
        }
        self.input_monitor.save_requested |= dirty;
    }
    pub(super) fn composition(&mut self) -> crate::output::Composition {
        if self.settings.profiles.entries.is_empty() {
            return self.scene().into();
        }
        let ids: Vec<_> = self
            .settings
            .profiles
            .entries
            .iter()
            .filter(|e| e.enabled)
            .map(|e| (e.id, e.name.clone()))
            .collect();
        let mut avatars = Vec::new();
        for (id, name) in ids {
            if let Some(scene) = self.with_profile(id, |app| app.scene()) {
                avatars.push(crate::output::Actor { id, name, scene });
            }
        }
        crate::output::Composition { avatars }
    }
    pub(super) fn profile_tabs(&mut self, ui: &mut egui::Ui) {
        if self.settings.profiles.entries.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.small("EDIT STAGE");
            for entry in &self.settings.profiles.entries {
                if entry.enabled
                    && ui
                        .selectable_label(
                            self.profiles.current == Some(entry.id),
                            short_name(&entry.name),
                        )
                        .on_hover_text(&entry.name)
                        .clicked()
                {
                    self.profiles.actions.push_back(Action::Focus(entry.id));
                }
            }
            if ui.small_button("+ Add avatar").clicked() {
                self.profiles.actions.push_back(Action::Add);
            }
        });
        theme::caption(
            ui,
            "Each tab edits one avatar. OBS previews combine all checked profiles.",
        );
        ui.add_space(6.);
    }
    pub(super) fn shared_tracking_ui(&mut self, ui: &mut egui::Ui) {
        let name = self
            .settings
            .profiles
            .entries
            .iter()
            .find(|e| Some(e.id) == self.following_profile())
            .map(|e| e.name.as_str())
            .unwrap_or("an unloaded profile");
        ui.heading("Shared face tracking");
        ui.label(format!("Following {name}"));
        ui.label("This avatar uses the same face input, with its own ranges, smoothing, physics, expressions and microphone settings in the inspector.");
        if ui.button("Use this avatar's own tracker").clicked() {
            if let Some(e) = self
                .settings
                .profiles
                .entries
                .iter_mut()
                .find(|e| Some(e.id) == self.profiles.current)
            {
                e.follow = None;
            }
            self.input_monitor.save_requested = true;
        }
        if ui.button("Guided range calibration…").clicked() {
            self.start_tracking_guide();
        }
        crate::help::button(ui, "profiles");
    }
    pub(super) fn workspace_summary(&self) -> serde_json::Value {
        serde_json::json!({"editing_profile": self.profiles.current,
            "profiles":self.settings.profiles.entries.iter().map(|e|serde_json::json!({"id":e.id,"name":e.name,"loaded":Some(e.id)==self.profiles.current || self.profiles.parked.contains_key(&e.id),"follow":e.follow,"error":self.profiles.errors.get(&e.id)})).collect::<Vec<_>>()})
    }
    pub(super) fn profiles_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Profiles");
        crate::help::button(ui, "profiles");
        ui.label("Check avatars to load them together. Click Edit to work on one stage. Drag each avatar in an OBS preview to arrange your scene.");
        if ui.button("+ Add avatar…").clicked() {
            self.profiles.actions.push_back(Action::Add);
        }
        if self.pending_image.is_some()
            || self.pending_vrm.is_some()
            || !self.profiles.restore.is_empty()
        {
            ui.spinner();
            ui.small("Loading avatars…");
        }
        let owners: Vec<_> = self
            .settings
            .profiles
            .entries
            .iter()
            .filter(|e| e.follow.is_none())
            .map(|e| (e.id, e.name.clone()))
            .collect();
        for entry in &mut self.settings.profiles.entries {
            let loaded = Some(entry.id) == self.profiles.current
                || self.profiles.parked.contains_key(&entry.id);
            ui.push_id(entry.id, |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let mut enabled = entry.enabled;
                        if ui.checkbox(&mut enabled, "").on_hover_text(
                            "Load into the shared OBS scene. Uncheck to free resources; saved settings and layout remain."
                        ).changed() {
                            entry.enabled = enabled;
                            self.profiles.actions.push_back(if enabled { Action::Load(entry.id) } else { Action::Unload(entry.id) });
                            self.input_monitor.save_requested = true;
                        }
                        ui.add_sized([(ui.available_width()-55.).max(40.),24.],egui::Label::new(RichText::new(&entry.name).strong()).truncate()).on_hover_text(&entry.name);
                        if loaded && ui.small_button(if Some(entry.id)==self.profiles.current {"Editing"} else {"Edit"}).clicked() {
                            self.profiles.actions.push_back(Action::Focus(entry.id));
                        }
                    });
                    ui.small(match entry.kind {
                        Some(crate::avatar_import::Kind::Vrm) => "VRM · independent stage",
                        Some(crate::avatar_import::Kind::Images) => "PNG / GIF · independent stage",
                        Some(crate::avatar_import::Kind::Live2d) => "Live2D · independent stage",
                        None => "Mica · test stage",
                    });
                    if entry.enabled && !loaded { ui.small("Queued / loading…"); }
                    if let Some(error)=self.profiles.errors.get(&entry.id) {
                        ui.colored_label(theme::orange(),error);
                        if ui.button("Retry load").clicked() { self.profiles.actions.push_back(Action::Load(entry.id)); }
                    }
                    ui.collapsing("Name, tracking & order",|ui| {
                        if ui.add(egui::TextEdit::singleline(&mut entry.name).char_limit(80)).changed() {
                            self.input_monitor.save_requested=true;
                        }
                        ui.label("Face tracking");
                        let before=entry.follow;
                        let label=entry.follow.and_then(|id|owners.iter().find(|e|e.0==id).map(|e|format!("Follow {}",e.1)))
                            .unwrap_or_else(||if entry.follow.is_some(){"Source unloaded / missing".into()}else{"Own connection".into()});
                        egui::ComboBox::from_id_salt("follow").selected_text(label).show_ui(ui,|ui| {
                            ui.selectable_value(&mut entry.follow,None,"Own connection");
                            for (id,name) in &owners {
                                if *id!=entry.id {ui.selectable_value(&mut entry.follow,Some(*id),format!("Follow {name}"));}
                            }
                        });
                        if entry.follow!=before {self.input_monitor.save_requested=true;}
                        ui.horizontal(|ui| {
                            if ui.small_button("Send back").clicked() {self.profiles.actions.push_back(Action::Order(entry.id,false));}
                            if ui.small_button("Bring forward").clicked() {self.profiles.actions.push_back(Action::Order(entry.id,true));}
                        });
                        ui.small("Later profiles appear in front. OBS layout and scale are independent in each preview.");
                        ui.label(entry.path.display().to_string());
                        if ui.small_button("Remove from list").on_hover_text("Keeps avatar files and legacy saved model settings on disk.").clicked() {
                            self.profiles.actions.push_back(Action::Remove(entry.id));
                        }
                    });
                });
            });
        }
        ui.add_space(8.);
        theme::caption(
            ui,
            "Loaded avatars keep animating while you edit another stage. More avatars use more GPU memory and CPU. Uncheck avatars you are not using.",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Memory(BTreeMap<String, String>);
    impl eframe::Storage for Memory {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }
    fn app() -> (egui::Context, AriaApp) {
        let ctx = egui::Context::default();
        let app = AriaApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        (ctx, app)
    }
    fn sprite(ctx: &egui::Context, name: &str) -> Sprite {
        Sprite {
            animation: None,
            texture: ctx.load_texture(
                name,
                egui::ColorImage::filled([4, 4], Color32::WHITE),
                Default::default(),
            ),
            name: name.into(),
            size: egui::vec2(4., 4.),
            model_key: name.into(),
            palette: Default::default(),
        }
    }
    fn add(app: &mut AriaApp, ctx: &egui::Context, name: &str) -> u64 {
        app.apply_image(Path::new(name), false, sprite(ctx, name), 64);
        app.profiles.current.unwrap()
    }
    #[test]
    fn multi_avatar_lifecycle_keeps_settings_animation_and_outputs_independent() {
        let (ctx, mut app) = app();
        let first = add(&mut app, &ctx, "one.png");
        app.settings.mapping.smoothing_ms = 17.;
        app.input_monitor.saved.config.pose.mode = PoseMode::Frozen;
        app.animation_time = 7.;
        app.outputs.edit_canvas(0, |c| {
            c.avatars.get_mut(&first).unwrap().position = [-0.3, 0.1];
        });
        let second = add(&mut app, &ctx, "two.gif");
        assert_eq!(app.settings.profiles.entries[0].name, "one.png");
        assert_eq!(app.settings.profiles.entries[1].name, "two.gif");
        assert_ne!(first, second);
        assert_eq!(app.profiles.parked.len(), 1);
        assert_eq!(app.following_profile(), Some(first));
        app.settings.mapping.smoothing_ms = 91.;
        app.animation_time = 12.;
        let two_texture = app.idle.as_ref().unwrap().texture.id();
        app.focus_profile(first);
        assert_eq!(app.settings.mapping.smoothing_ms, 17.);
        assert_eq!(app.animation_time, 7.);
        assert_eq!(app.input_monitor.saved.config.pose.mode, PoseMode::Frozen);
        app.focus_profile(second);
        assert_eq!(app.settings.mapping.smoothing_ms, 91.);
        assert_eq!(app.animation_time, 12.);
        assert_eq!(app.idle.as_ref().unwrap().texture.id(), two_texture);
        assert_eq!(app.composition().avatars.len(), 2);
        assert_eq!(
            app.outputs.snapshot().canvas(0).avatars[&first].position,
            [-0.3, 0.1]
        );
        app.unload_profile(second);
        assert_eq!(app.profiles.current, Some(first));
        assert_eq!(app.composition().avatars.len(), 1);
        assert!(app.profiles.parked.is_empty());
        assert_eq!(
            app.settings.profiles.entries[1]
                .preferences
                .mapping
                .smoothing_ms,
            91.
        );
        app.unload_profile(first);
        assert!(app.composition().avatars.is_empty());
        assert_eq!(app.profile_name(), "No avatar selected");
        // Reload an image through the real import completion path: no data from the last editor leaks.
        app.profiles.loading = Some(second);
        let saved = app.settings.profiles.entries[1].clone();
        app.settings.saved_rigs.insert(saved.key.clone(), saved.rig);
        app.settings
            .model_preferences
            .insert(saved.key, saved.preferences);
        add(&mut app, &ctx, "two.gif");
        assert_eq!(app.settings.mapping.smoothing_ms, 91.);
        assert_eq!(app.settings.profiles.entries.len(), 2);
    }
    #[test]
    fn workspace_storage_migrates_and_round_trips_all_profiles() {
        let (ctx, mut app) = app();
        let first = add(&mut app, &ctx, "one.png");
        let second = add(&mut app, &ctx, "two.png");
        app.outputs
            .edit_canvas(1, |c| c.avatars.get_mut(&second).unwrap().zoom = 1.8);
        app.remember_current_rig();
        let mut storage = Memory::default();
        eframe::set_value(&mut storage, "test", &app.settings);
        let restored: Settings = eframe::get_value(&storage, "test").unwrap();
        assert_eq!(restored.profiles.entries.len(), 2);
        assert_eq!(restored.profiles.active, Some(second));
        assert_eq!(restored.profiles.source(second), Some(first));
        assert_eq!(
            restored.outputs.unwrap().canvas(1).avatars[&second].zoom,
            1.8
        );
        let old: Settings = serde_json::from_str(r#"{"zoom":1.2,"source":"Demo"}"#).unwrap();
        assert!(old.profiles.entries.is_empty());
    }
    #[test]
    fn tracking_follow_resolves_chains_and_rejects_cycles_or_unloaded_sources() {
        let (ctx, mut app) = app();
        let a = add(&mut app, &ctx, "a.png");
        let b = add(&mut app, &ctx, "b.png");
        let c = add(&mut app, &ctx, "c.png");
        app.settings.profiles.entries[2].follow = Some(b);
        assert_eq!(app.settings.profiles.source(c), Some(a));
        app.sample_profile_tracking(0.016);
        assert!(app.raw.is_some());
        assert!(app.profiles.parked[&a].raw.is_some());
        app.settings.profiles.entries[0].follow = Some(c);
        assert_eq!(app.settings.profiles.source(c), None);
        app.sample_profile_tracking(0.016);
        assert!(app.raw.is_none());
        app.settings.profiles.entries[0].follow = None;
        app.unload_profile(a);
        assert_eq!(app.settings.profiles.source(c), None);
    }
    #[test]
    fn shared_hotkey_dispatch_targets_all_loaded_avatars_and_preserves_editor() {
        let (ctx, mut app) = app();
        let a = add(&mut app, &ctx, "a.png");
        app.input_monitor.saved.global_hotkeys = true;
        let b = add(&mut app, &ctx, "b.png");
        app.input_monitor.saved.global_hotkeys = true;
        let keys = app.workspace_hotkeys();
        let key = keys
            .into_iter()
            .find(|k| k.shortcut == aria_core::shortcuts::Shortcut::pose())
            .unwrap();
        app.dispatch_hotkey(key.action);
        assert_eq!(app.profiles.current, Some(b));
        assert_eq!(app.input_monitor.saved.config.pose.mode, PoseMode::Frozen);
        assert_eq!(
            app.profiles.parked[&a].input_monitor.saved.config.pose.mode,
            PoseMode::Frozen
        );
    }
    #[test]
    fn missing_avatar_keeps_existing_stages_alive_and_retryable() {
        let (ctx, mut app) = app();
        let a = add(&mut app, &ctx, "a.png");
        let b = add(&mut app, &ctx, "missing-avatar.vrm");
        app.unload_profile(b);
        let saved = app.settings.profiles.entries[1].clone();
        app.profiles.loading = Some(b);
        app.status_message = Some("File missing".into());
        app.finish_profile_load();
        assert_eq!(app.profiles.current, Some(a));
        assert_eq!(app.composition().avatars.len(), 1);
        assert_eq!(app.settings.profiles.entries[1].path, saved.path);
        assert_eq!(app.profiles.errors[&b], "File missing");
    }
    #[test]
    fn queued_profile_choices_are_all_processed_and_api_framing_targets_editor() {
        let (ctx, mut app) = app();
        let a = add(&mut app, &ctx, "a.png");
        let b = add(&mut app, &ctx, "b.png");
        let c = add(&mut app, &ctx, "c.png");
        app.profiles.actions.push_back(Action::Unload(a));
        app.profiles.actions.push_back(Action::Unload(b));
        app.process_profile_actions(&ctx);
        app.process_profile_actions(&ctx);
        assert!(app.profiles.parked.is_empty());
        assert_eq!(app.composition().avatars.len(), 1);
        app.apply_api_action(crate::effect_api::Action::Output {
            index: 1,
            open: false,
            zoom: Some(1.4),
            position: Some([0.2, -0.3]),
        })
        .unwrap();
        assert_eq!(
            app.outputs.snapshot().canvas(1).avatars[&c].position,
            [0.2, -0.3]
        );
        assert_eq!(app.outputs.snapshot().canvas(0).avatars[&c].zoom, 0.7);
    }
    #[test]
    fn same_content_in_two_profiles_restores_the_incoming_config() {
        let (ctx, mut app) = app();
        let a = add(&mut app, &ctx, "a.png");
        app.settings.mapping.smoothing_ms = 13.;
        let mut same = sprite(&ctx, "a.png");
        same.name = "Copy".into();
        app.apply_image(Path::new("copy.png"), false, same.clone(), 64);
        let b = app.profiles.current.unwrap();
        app.settings.mapping.smoothing_ms = 83.;
        app.unload_profile(b);
        assert_eq!(app.profiles.current, Some(a));
        app.profiles.loading = Some(b);
        app.apply_image(Path::new("copy.png"), false, same, 64);
        assert_eq!(app.settings.mapping.smoothing_ms, 83.);
        assert_eq!(
            app.profiles.parked[&a].preferences.mapping.smoothing_ms,
            13.
        );
    }
}
