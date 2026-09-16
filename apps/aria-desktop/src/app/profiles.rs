//! Independent live avatar sessions and the persistent workspace catalog.
//! The editor borrows one session at a time; captures and render resources stay owned.
use super::*;
mod actions;
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
        self.tracking_owner(id, true)
    }
    fn tracking_owner(&self, id: u64, require_enabled: bool) -> Option<u64> {
        let mut current = id;
        let mut visited = std::collections::BTreeSet::new();
        while visited.insert(current) {
            let entry = self.entries.iter().find(|e| e.id == current)?;
            if require_enabled && !entry.enabled {
                return None;
            }
            match entry.follow {
                Some(next) => current = next,
                None => return Some(current),
            }
        }
        None
    }
    pub(super) fn migrate_vts_roll(&mut self) {
        // Disabled avatars and followers also need the new input coordinates
        // when loaded later. Keep independent entries independent of model keys.
        let affected: Vec<_> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let owner = self.tracking_owner(entry.id, false)?;
                self.entries
                    .iter()
                    .find(|e| e.id == owner)
                    .filter(|e| e.preferences.source == Source::Vts)
                    .map(|_| entry.id)
            })
            .collect();
        for entry in &mut self.entries {
            if affected.contains(&entry.id) {
                entry.preferences.calibration.z = -entry.preferences.calibration.z;
                migrate_roll_rig(&mut entry.rig);
            }
        }
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
    rename: Option<(u64, String)>,
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
        self.preferences.exchange_live(&mut app.settings);
        self.preferences.calibration = app.pipeline.calibration();
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
    pub(super) fn refresh_avatar_label(&mut self) {
        if let Some(avatar) = &mut self.live2d
            && let Some(entry) = self
                .settings
                .profiles
                .entries
                .iter()
                .find(|e| Some(e.id) == self.profiles.current)
            && avatar.name != entry.name
        {
            avatar.name.clone_from(&entry.name);
        }
    }
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
        // Single avatars and independent trackers do not need a second owned
        // snapshot/raw-frame copy (including every blendshape string/map).
        let shared_sources: std::collections::BTreeSet<_> = self
            .settings
            .profiles
            .entries
            .iter()
            .filter(|entry| entry.enabled && entry.follow.is_some() && ids.contains(&entry.id))
            .filter_map(|entry| self.settings.profiles.source(entry.id))
            .collect();
        if self.profiles.current.is_none() {
            self.sample_own_tracking();
        }
        for id in &ids {
            if let Some(frame) = self
                .with_profile(*id, |app| {
                    if app.following_profile().is_none() {
                        app.sample_own_tracking();
                        shared_sources
                            .contains(id)
                            .then(|| (app.snapshot.clone(), app.raw.clone()))
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
        self.custom_hotkeys()
    }
    #[cfg(test)]
    fn legacy_workspace_hotkeys(&self) -> Vec<crate::hotkeys::Registration> {
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
        if let crate::hotkeys::Action::Custom(target) = action {
            self.trigger_target(target);
        } else if let crate::hotkeys::Action::Profiles(actions) = action {
            for (id, action) in actions {
                if matches!(action, crate::hotkeys::Action::Custom(_))
                    || (id == 0 && self.profiles.current.is_none())
                {
                    self.dispatch_hotkey(action);
                } else {
                    self.with_profile(id, |app| app.dispatch_hotkey(action));
                }
            }
        } else {
            #[cfg(test)]
            if self.input_monitor.saved.global_hotkeys {
                let parameters = self.current_parameters();
                self.input_monitor
                    .hotkey_action(action, &parameters, &mut self.settings.mapping);
            }
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
                        .add(
                            egui::Button::new(
                                egui::RichText::new(short_name(&entry.name)).strong(),
                            )
                            .selected(self.profiles.current == Some(entry.id))
                            .fill(if self.profiles.current == Some(entry.id) {
                                theme::wash(theme::mint())
                            } else {
                                theme::card_color()
                            })
                            .stroke(if self.profiles.current == Some(entry.id) {
                                egui::Stroke::new(1.5, theme::mint())
                            } else {
                                theme::surface_edge()
                            })
                            .corner_radius(egui::CornerRadius {
                                nw: 10,
                                ne: 10,
                                sw: 2,
                                se: 2,
                            })
                            .min_size(egui::vec2(100., 34.)),
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
        ui.separator();
        theme::caption(
            ui,
            "Each tab edits one avatar. OBS previews combine all checked profiles.",
        );
        ui.add_space(6.);
    }
    pub(super) fn rename_stage_button(&mut self, ui: &mut egui::Ui) {
        if let Some(id) = self.profiles.current
            && ui
                .small_button("Rename…")
                .on_hover_text("One name for this stage and profile, everywhere in ARIA.")
                .clicked()
        {
            self.profiles.rename = Some((id, self.profile_name()));
        }
    }
    pub(super) fn show_profile_rename(&mut self, ctx: &egui::Context) {
        let Some((id, mut name)) = self.profiles.rename.clone() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        egui::Window::new("Rename stage & profile").id(egui::Id::new("rename-stage"))
            .open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label("This name appears in tabs, the inspector, tracking sources and OBS preview selectors. Your avatar files keep their original names.");
                ui.add(egui::TextEdit::singleline(&mut name).char_limit(80));
                ui.horizontal(|ui| {
                    save = ui.add_enabled(!name.trim().is_empty(), egui::Button::new("Save name")).clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if save {
            self.rename_profile(id, &name);
        }
        self.profiles.rename = if open && !save && !cancel {
            Some((id, name))
        } else {
            None
        };
    }
    fn rename_profile(&mut self, id: u64, name: &str) {
        let name: String = name.trim().chars().take(80).collect();
        if !name.is_empty()
            && let Some(entry) = self
                .settings
                .profiles
                .entries
                .iter_mut()
                .find(|e| e.id == id)
        {
            entry.name = name;
            self.input_monitor.save_requested = true;
        }
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
                let editing = Some(entry.id) == self.profiles.current;
                theme::glass_card().stroke(if editing {
                    egui::Stroke::new(1.0, theme::mint())
                } else {
                    theme::surface_edge()
                }).show(ui, |ui| {
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
                        Some(crate::avatar_import::Kind::Glb) => "VRC / GLB · independent stage",
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
                        if ui.button("Rename stage & profile…").clicked() {
                            self.profiles.rename = Some((entry.id, entry.name.clone()));
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
    fn vts_roll_upgrade_preserves_calibrated_presets_and_disabled_shared_profiles() {
        let (ctx, mut app) = app();
        let source = add(&mut app, &ctx, "phone.png");
        let follower = add(&mut app, &ctx, "follower.png");
        let other = add(&mut app, &ctx, "json.png");
        app.settings.vts_roll_revision = 0;
        for entry in &mut app.settings.profiles.entries {
            entry.enabled = false;
            entry.preferences.source = if entry.id == source {
                Source::Vts
            } else {
                Source::Json
            };
            entry.follow = (entry.id == follower).then_some(source);
            entry.preferences.calibration = aria_core::Vec3 {
                x: 3.,
                y: 5.,
                z: 7.,
            };
            entry.preferences.mapping.invert_roll = true;
            let config = &mut entry.rig.config;
            config.tracking.origin = entry.preferences.calibration;
            config.tracking.enabled = true;
            for name in ["FaceAngleZ", "ParamAngleZ", "FaceAngleX"] {
                config.tracking.ranges.insert(
                    name.into(),
                    aria_core::calibration::Range {
                        low: -20.,
                        neutral: 2.,
                        high: 40.,
                        enabled: true,
                    },
                );
            }
            config.pose.held.insert("HeldAccessory".into(), 0.75);
            entry.rig.presets = vec![movement::Preset {
                name: "Personal movement".into(),
                kind: movement::PresetKind::Movement,
                rig: config.clone(),
                mapping: entry.preferences.mapping.clone(),
                hotkey: Some(3),
            }];
        }
        let legacy = app.settings.profiles.entries[0].clone();
        app.settings
            .model_preferences
            .insert("legacy-phone".into(), legacy.preferences);
        app.settings
            .saved_rigs
            .insert("legacy-phone".into(), legacy.rig);
        // Load an actual pre-fix serialization with the new marker absent.
        let mut encoded = serde_json::to_value(&app.settings).unwrap();
        encoded.as_object_mut().unwrap().remove("vts_roll_revision");
        let mut settings: Settings = serde_json::from_value(encoded).unwrap();
        assert_eq!(settings.vts_roll_revision, 0);
        settings.migrate_vts_roll();
        let once = serde_json::to_value(&settings).unwrap();
        settings.migrate_vts_roll();
        assert_eq!(serde_json::to_value(&settings).unwrap(), once);
        assert_eq!(
            settings.model_preferences["legacy-phone"].calibration.z,
            -7.
        );
        assert_eq!(
            settings.saved_rigs["legacy-phone"].config.tracking.origin.z,
            -7.
        );
        for entry in &settings.profiles.entries {
            let corrected = entry.id != other;
            assert_eq!(
                entry.preferences.calibration.z,
                if corrected { -7. } else { 7. }
            );
            assert_eq!(entry.preferences.calibration.x, 3.);
            assert!(entry.preferences.mapping.invert_roll);
            for config in
                std::iter::once(&entry.rig.config).chain(entry.rig.presets.iter().map(|p| &p.rig))
            {
                assert_eq!(config.tracking.origin.z, if corrected { -7. } else { 7. });
                let range = &config.tracking.ranges["FaceAngleZ"];
                assert_eq!(
                    (range.low, range.neutral, range.high),
                    if corrected {
                        (-40., -2., 20.)
                    } else {
                        (-20., 2., 40.)
                    }
                );
                assert!(range.valid("FaceAngleZ"));
                assert_eq!(config.tracking.ranges["FaceAngleX"].neutral, 2.);
                assert_eq!(config.pose.held["HeldAccessory"], 0.75);
            }
            assert_eq!(entry.rig.presets[0].hotkey, Some(3));
        }
        assert_eq!(settings.vts_roll_revision, 1);
        assert_eq!(Settings::default().vts_roll_revision, 1);
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
    fn renamed_profiles_keep_action_targets_and_names_in_composition_and_storage() {
        use crate::actions::{Command, Mode, Step, Target};
        let (ctx, mut app) = app();
        let first = add(&mut app, &ctx, "first.png");
        let second = add(&mut app, &ctx, "second.png");
        let target = Target::Avatar {
            profile: first,
            command: Command::Pose,
        };
        app.settings.actions.bind(
            target.clone(),
            Some(aria_core::shortcuts::Shortcut::preset(4)),
        );
        let mut graph = crate::actions::Graph::new(1);
        graph.name = "Freeze first".into();
        graph.nodes[1].step = Step::Action {
            target: Some(target.clone()),
            mode: Mode::On,
        };
        app.settings.actions.graphs.push(graph);
        app.rename_profile(first, "  New stage name  ");
        assert_eq!(app.composition().avatars[0].name, "New stage name");
        assert!(
            app.action_choices()
                .iter()
                .any(|c| c.target == target && c.label.starts_with("New stage name"))
        );
        assert_eq!(app.profiles.current, Some(second));
        app.trigger_target(Target::Graph(1));
        let _ = crate::run_test_ui(&ctx, egui::RawInput::default(), |_| {
            app.update_actions(&ctx)
        });
        assert!(app.action_runs.is_empty());
        assert_eq!(
            app.profiles.parked[&first]
                .input_monitor
                .saved
                .config
                .pose
                .mode,
            PoseMode::Frozen
        );
        assert_eq!(app.input_monitor.saved.config.pose.mode, PoseMode::Live);
        assert_eq!(app.profiles.current, Some(second));
        app.remember_current_rig();
        let restored: Settings =
            serde_json::from_slice(&serde_json::to_vec(&app.settings).unwrap()).unwrap();
        assert_eq!(restored.profiles.entries[0].name, "New stage name");
        assert_eq!(
            restored.actions.graphs[0].validate().unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(restored.actions.bindings[0].target, target);
    }
    #[test]
    fn streamerbot_targets_profiles_not_focus_and_rejects_missing_targets() {
        use crate::actions::{Command, Mode, Target};
        let (ctx, mut app) = app();
        let api_settings = crate::effect_api::Settings {
            enabled: true,
            port: 0,
            token: "a".repeat(48),
        };
        app.effect_api
            .update(&api_settings, "test", &Default::default(), &ctx);
        let first = add(&mut app, &ctx, "bot-first.png");
        let second = add(&mut app, &ctx, "bot-second.png");
        let target = Target::Avatar {
            profile: first,
            command: Command::Pose,
        };
        app.start_api_target(target.clone(), Mode::On, 1).unwrap();
        assert!(app.effect_api.test_result(1).is_none());
        let _ = crate::run_test_ui(&ctx, egui::RawInput::default(), |_| {
            app.update_actions(&ctx)
        });
        assert_eq!(app.profiles.current, Some(second));
        assert_eq!(app.input_monitor.saved.config.pose.mode, PoseMode::Live);
        assert_eq!(
            app.profiles.parked[&first]
                .input_monitor
                .saved
                .config
                .pose
                .mode,
            PoseMode::Frozen
        );
        assert_eq!(app.effect_api.test_result(1).unwrap()["status"], "applied");
        app.start_api_target(target, Mode::Off, 2).unwrap();
        let _ = crate::run_test_ui(&ctx, egui::RawInput::default(), |_| {
            app.update_actions(&ctx)
        });
        assert_eq!(
            app.profiles.parked[&first]
                .input_monitor
                .saved
                .config
                .pose
                .mode,
            PoseMode::Live
        );
        assert!(
            app.start_api_target(
                Target::Avatar {
                    profile: first,
                    command: Command::Item(999)
                },
                Mode::On,
                3
            )
            .is_err()
        );
        assert!(
            app.start_api_target(
                Target::Avatar {
                    profile: first,
                    command: Command::Load
                },
                Mode::Off,
                3
            )
            .is_err()
        );
        assert!(
            app.start_api_target(Target::Graph(999), Mode::Toggle, 3)
                .is_err()
        );
        app.start_api_target(
            Target::Avatar {
                profile: first,
                command: Command::Pose,
            },
            Mode::On,
            4,
        )
        .unwrap();
        app.stop_actions();
        assert!(app.action_runs.is_empty());
        assert_eq!(app.effect_api.test_result(4).unwrap()["status"], "rejected");
        app.start_api_target(
            Target::Avatar {
                profile: first,
                command: Command::Pose,
            },
            Mode::On,
            5,
        )
        .unwrap();
        app.unload_profile(first);
        let _ = crate::run_test_ui(&ctx, egui::RawInput::default(), |_| {
            app.update_actions(&ctx)
        });
        assert_eq!(app.effect_api.test_result(5).unwrap()["status"], "rejected");
    }
    #[test]
    fn custom_shortcuts_replace_inherited_keys_and_disabled_keys_do_not_register() {
        let (ctx, mut app) = app();
        let id = add(&mut app, &ctx, "one.png");
        app.input_monitor.saved.global_hotkeys = true;
        let target = crate::actions::Target::Avatar {
            profile: id,
            command: crate::actions::Command::Pose,
        };
        let key = aria_core::shortcuts::Shortcut::preset(5);
        app.settings.actions.bind(target.clone(), Some(key));
        assert!(
            !app.workspace_hotkeys()
                .iter()
                .any(|k| k.shortcut == aria_core::shortcuts::Shortcut::pose())
        );
        let binding = app
            .workspace_hotkeys()
            .into_iter()
            .find(|k| k.shortcut == key)
            .unwrap();
        app.dispatch_hotkey(binding.action);
        let _ = crate::run_test_ui(&ctx, egui::RawInput::default(), |_| {
            app.update_actions(&ctx)
        });
        assert_eq!(app.input_monitor.saved.config.pose.mode, PoseMode::Frozen);
        app.settings.actions.bind(target, None);
        assert!(app.workspace_hotkeys().is_empty());
    }
    #[test]
    fn missing_action_targets_stop_with_repair_message_without_touching_other_avatars() {
        let (ctx, mut app) = app();
        add(&mut app, &ctx, "one.png");
        app.trigger_target(crate::actions::Target::Avatar {
            profile: 999,
            command: crate::actions::Command::Pose,
        });
        let _ = crate::run_test_ui(&ctx, egui::RawInput::default(), |_| {
            app.update_actions(&ctx)
        });
        assert!(app.action_runs.is_empty());
        assert!(
            app.action_editor
                .message
                .as_ref()
                .unwrap()
                .contains("not loaded")
        );
        assert_eq!(app.input_monitor.saved.config.pose.mode, PoseMode::Live);
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
        let keys = app.legacy_workspace_hotkeys();
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
