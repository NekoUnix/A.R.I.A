//! Workspace action routing uses profile identity, independent of which stage is edited.
use super::*;
use crate::actions::{Choice, Command, Mode, Step, Target};
use anyhow::{Result, ensure};
use aria_core::shortcuts::Shortcut;

impl AriaApp {
    pub(in crate::app) fn action_choices(&self) -> Vec<Choice> {
        let mut choices = Vec::new();
        for entry in &self.settings.profiles.entries {
            for (command, label) in [
                (Command::Load, "Load avatar"),
                (Command::Unload, "Unload avatar"),
                (Command::Focus, "Edit stage"),
                (Command::Switch, "Switch current avatar to this profile"),
            ] {
                choices.push(Choice {
                    target: Target::Avatar {
                        profile: entry.id,
                        command,
                    },
                    label: format!("{} · {label}", entry.name),
                    shortcut: None,
                    legacy_label: None,
                });
            }
        }
        for (id, monitor, has_vrm) in std::iter::once((
            self.profiles.current.unwrap_or(0),
            &self.input_monitor,
            self.vrm.is_some(),
        ))
        .chain(
            self.profiles
                .parked
                .iter()
                .map(|(&id, s)| (id, &s.input_monitor, s.vrm.is_some())),
        ) {
            if id == 0 && !self.settings.profiles.entries.is_empty() {
                continue;
            }
            let name = self
                .settings
                .profiles
                .entries
                .iter()
                .find(|e| e.id == id)
                .map_or("Mica", |e| e.name.as_str());
            let saved = &monitor.saved;
            let mut add = |command, label: String, key| {
                choices.push(Choice {
                    target: Target::Avatar {
                        profile: id,
                        command,
                    },
                    label: format!("{name} · {label}"),
                    shortcut: if saved.global_hotkeys { key } else { None },
                    legacy_label: None,
                })
            };
            add(
                Command::Pose,
                "Freeze / resume pose".into(),
                Some(Shortcut::pose()),
            );
            for p in &saved.presets {
                add(
                    Command::Preset(p.name.clone()),
                    format!("Preset · {}", p.name),
                    p.hotkey.map(Shortcut::preset),
                );
            }
            for e in &monitor.expressions.entries {
                add(
                    Command::Expression(e.file.id.clone()),
                    format!("Expression · {}", e.file.name),
                    saved.expression_hotkeys.get(&e.file.id).copied(),
                );
            }
            for g in &saved.config.layers.groups {
                add(
                    Command::Layers(g.id),
                    format!("Layer group · {}", g.name),
                    saved.layer_hotkeys.get(&g.id).copied(),
                );
            }
            for i in &saved.config.items {
                add(
                    Command::Item(i.id),
                    format!("Object · {}", i.name),
                    saved.item_hotkeys.get(&i.id).copied(),
                );
            }
            for i in &saved.config.images.states {
                add(
                    Command::Image(i.id),
                    format!("Image · {}", i.name),
                    i.hotkey,
                );
            }
            for d in &saved.effects.designs {
                add(
                    Command::Effect(d.id),
                    format!("Effect · {}", d.name),
                    d.hotkey,
                );
            }
            for h in &saved.vts.actions {
                add(
                    Command::Imported(h.id.clone()),
                    format!("Imported action · {}", h.name),
                    if h.enabled && saved.vts.keyboard_enabled {
                        h.shortcut
                    } else {
                        None
                    },
                );
            }
            if has_vrm {
                for gesture in crate::vrm::motion::Gesture::ALL {
                    add(
                        Command::Gesture(gesture.name().into()),
                        format!("3D animation · {}", gesture.name()),
                        None,
                    );
                }
            }
        }
        choices.extend(self.settings.actions.graphs.iter().map(|g| Choice {
            target: Target::Graph(g.id),
            label: format!("Action graph · {}", g.name),
            shortcut: None,
            legacy_label: None,
        }));
        for choice in &mut choices {
            if let Target::Avatar {
                profile,
                command: Command::Imported(id),
            } = &choice.target
            {
                let monitor = if self.profiles.current == Some(*profile) {
                    Some(&self.input_monitor)
                } else {
                    self.profiles.parked.get(profile).map(|s| &s.input_monitor)
                };
                if let Some(monitor) = monitor
                    && let Some(h) = monitor.saved.vts.actions.iter().find(|h| &h.id == id)
                {
                    choice.shortcut = if h.enabled && monitor.saved.vts.keyboard_enabled {
                        h.shortcut
                            .or_else(|| crate::actions::imported_shortcut(&h.chord))
                    } else {
                        None
                    };
                    if !h.chord.is_empty() && choice.shortcut.is_none() {
                        choice.legacy_label = Some("Imported key chord (retained)".into());
                    }
                }
            }
        }
        choices
    }
    pub(in crate::app) fn custom_hotkeys(&self) -> Vec<crate::hotkeys::Registration> {
        if !self.settings.actions.shortcuts_enabled {
            return Vec::new();
        }
        // Imported VTS actions keep their press/release lifecycle in the VTS key worker.
        let mut grouped: BTreeMap<Shortcut, Vec<(u64, crate::hotkeys::Action)>> = BTreeMap::new();
        for choice in self.action_choices() {
            if matches!(
                choice.target,
                Target::Avatar {
                    command: Command::Imported(_),
                    ..
                }
            ) {
                continue;
            }
            if let Some(shortcut) = self.settings.actions.shortcut(&choice) {
                grouped
                    .entry(shortcut)
                    .or_default()
                    .push((0, crate::hotkeys::Action::Custom(choice.target)));
            }
        }
        grouped
            .into_iter()
            .map(|(shortcut, actions)| crate::hotkeys::Registration {
                shortcut,
                action: crate::hotkeys::Action::Profiles(actions),
            })
            .collect()
    }
    pub(in crate::app) fn trigger_target(&mut self, target: Target) {
        if let Target::Graph(id) = target {
            self.start_action(id);
        } else {
            // Single hotkey actions use the same scheduler as compound actions,
            // including asynchronous avatar loading and error reporting.
            let mut graph = crate::actions::Graph::new(0);
            graph.name = self
                .action_choices()
                .iter()
                .find(|c| c.target == target)
                .map_or("Shortcut".into(), |c| c.label.chars().take(80).collect());
            graph.nodes[1].step = Step::Action {
                target: Some(target),
                mode: Mode::Toggle,
            };
            match crate::actions::Run::new(&graph, self.started.elapsed().as_secs_f64()) {
                Ok(run) if self.action_runs.len() < 16 => self.action_runs.push(run),
                Ok(_) => {
                    self.action_editor.message = Some(
                        "Too many actions are running. Stop or wait for an action to finish."
                            .into(),
                    )
                }
                Err(e) => self.action_editor.message = Some(e.to_string()),
            }
        }
    }
    pub(in crate::app) fn start_api_target(
        &mut self,
        target: Target,
        mode: Mode,
        ticket: u64,
    ) -> Result<()> {
        let choices = self.action_choices();
        let choice = choices.iter().find(|c| c.target == target)
            .ok_or_else(|| anyhow::anyhow!("Action is unavailable. Load its avatar or select a current action in Streamer.bot setup."))?;
        ensure!(
            self.action_runs.len() < 16,
            "At most 16 actions can run together"
        );
        let single = !matches!(target, Target::Graph(_));
        let graph = if let Target::Graph(id) = target {
            ensure!(
                mode == Mode::Toggle,
                "Graphs are one-shot actions; use Toggle mode"
            );
            ensure!(
                !self.action_runs.iter().any(|r| r.graph.id == id),
                "Action graph is already running"
            );
            self.settings
                .actions
                .graphs
                .iter()
                .find(|g| g.id == id)
                .unwrap()
                .clone()
        } else {
            ensure!(
                mode == Mode::Toggle || crate::streamerbot::supports_mode(&target),
                "This is a one-shot action; use Toggle mode"
            );
            let mut graph = crate::actions::Graph::new(0);
            graph.name = choice.label.chars().take(80).collect();
            graph.nodes[1].step = Step::Action {
                target: Some(target),
                mode,
            };
            graph
        };
        let mut run = crate::actions::Run::new(&graph, self.started.elapsed().as_secs_f64())?;
        run.receipt = Some(self.effect_api.receipt(ticket));
        // Bind Switch's replacement origin before another queued action can change focus.
        if single {
            run.origins.insert(2, self.profiles.current);
        }
        self.action_runs.push(run);
        Ok(())
    }
    pub(in crate::app) fn stop_actions(&mut self) {
        for run in self.action_runs.drain(..) {
            if let Some(receipt) = run.receipt {
                self.effect_api
                    .finish(receipt, Err("Action cancelled before completion".into()));
            }
        }
    }
    fn start_action(&mut self, id: u64) {
        let Some(graph) = self.settings.actions.graphs.iter().find(|g| g.id == id) else {
            self.action_editor.message = Some("Action was removed. Choose another action.".into());
            return;
        };
        if self.action_runs.iter().any(|r| r.graph.id == id) {
            self.action_editor.message = Some(format!(
                "{} is already running. Stop it or wait before replaying.",
                graph.name
            ));
            return;
        }
        if self.action_runs.len() >= 16 {
            self.action_editor.message = Some("At most 16 actions can run together.".into());
            return;
        }
        match crate::actions::Run::new(graph, self.started.elapsed().as_secs_f64()) {
            Ok(run) => {
                self.action_editor.message = Some(format!("Running {}", graph.name));
                self.action_runs.push(run);
            }
            Err(e) => {
                self.action_editor.message = Some(format!("Cannot run {}: {e}", graph.name));
                self.action_editor.open = true;
            }
        }
    }
    pub(in crate::app) fn update_actions(&mut self, ctx: &egui::Context) {
        self.show_profile_rename(ctx);
        let choices = self.action_choices();
        let running = self
            .action_runs
            .iter()
            .map(|r| r.graph.name.clone())
            .collect::<Vec<_>>();
        let changed = self
            .action_editor
            .ui(ctx, &mut self.settings.actions, &choices, &running);
        if changed {
            self.input_monitor.save_requested = true;
            // Preserve VTS hold/release behavior while replacing its old shortcut editor.
            let overrides = self.settings.actions.bindings.clone();
            for binding in overrides {
                if let Target::Avatar {
                    profile,
                    command: Command::Imported(id),
                } = binding.target
                {
                    self.with_profile(profile, |app| {
                        if let Some(h) = app
                            .input_monitor
                            .saved
                            .vts
                            .actions
                            .iter_mut()
                            .find(|h| h.id == id)
                        {
                            h.shortcut = binding.shortcut;
                            h.chord.clear();
                            h.global = true;
                            if binding.shortcut.is_some() {
                                h.enabled = true;
                                app.input_monitor.saved.vts.keyboard_enabled = true;
                                app.input_monitor.saved.global_hotkeys = true;
                            }
                        }
                    });
                }
            }
        }
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("aria-shortcuts-paused"),
                self.action_editor.recording() || !self.settings.actions.shortcuts_enabled,
            )
        });
        if ctx.current_pass_index() != 0 {
            return;
        }
        if let Some(id) = self.action_editor.run.take() {
            self.start_action(id);
        }
        if std::mem::take(&mut self.action_editor.stop) {
            self.stop_actions();
            self.action_editor.message =
                Some("Stopped pending steps. Changes already applied remain in place.".into());
        }
        #[cfg(not(windows))]
        if !self.action_editor.recording()
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|i| i.focused)
        {
            for binding in self.custom_hotkeys() {
                if crate::actions::key_pressed(ctx, binding.shortcut) {
                    self.dispatch_hotkey(binding.action);
                }
            }
        }
        let now = self.started.elapsed().as_secs_f64();
        let mut runs = std::mem::take(&mut self.action_runs);
        let mut remaining = Vec::new();
        let mut budget = 32;
        for mut run in runs.drain(..) {
            if run
                .receipt
                .is_some_and(|r| !self.effect_api.receipt_is_current(r))
            {
                continue;
            }
            let mut failed = false;
            loop {
                let ready = run.ready(now);
                if ready.is_empty() || budget == 0 {
                    break;
                }
                let mut progressed = false;
                for (id, step) in ready.into_iter().take(budget) {
                    budget -= 1;
                    let result = match step {
                        Step::Action {
                            target: Some(target),
                            mode,
                        } => {
                            let origin = *run.origins.entry(id).or_insert(self.profiles.current);
                            self.execute_action(ctx, &target, mode, origin)
                        }
                        Step::Action { target: None, .. } => {
                            Err(anyhow::anyhow!("No target selected"))
                        }
                        _ => Ok(true),
                    };
                    match result {
                        Ok(true) => {
                            run.complete(id, now);
                            progressed = true;
                        }
                        Ok(false) if !run.wait_expired(id, now) => {}
                        result => {
                            let error = result
                                .err()
                                .map_or("Avatar loading timed out after 60 seconds".into(), |e| {
                                    e.to_string()
                                });
                            if let Some(receipt) = run.receipt {
                                self.effect_api.finish(receipt, Err(error.clone()));
                            }
                            crate::diagnostics::record(
                                "error",
                                "action-node",
                                &format!("Action {} node {id}: {error}", run.graph.id),
                            );
                            self.action_editor.message = Some(format!(
                                "{} stopped at node {id}: {error}. Repair the node's target, then run again.",
                                run.graph.name
                            ));
                            self.action_editor.open = true;
                            failed = true;
                            break;
                        }
                    }
                }
                if failed || !progressed {
                    break;
                }
            }
            if !failed && !run.finished() {
                remaining.push(run);
            } else if !failed {
                if let Some(receipt) = run.receipt {
                    self.effect_api.finish(receipt, Ok(()));
                }
                self.action_editor.message = Some(format!("Finished {}", run.graph.name));
            }
        }
        self.action_runs.extend(remaining);
        if !self.action_runs.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
    fn execute_action(
        &mut self,
        ctx: &egui::Context,
        target: &Target,
        mode: Mode,
        origin: Option<u64>,
    ) -> Result<bool> {
        let Target::Avatar { profile, command } = target else {
            anyhow::bail!("Graphs cannot call other graphs");
        };
        let profile = *profile;
        let loaded = self.profiles.current == Some(profile)
            || self.profiles.parked.contains_key(&profile)
            || (profile == 0 && self.settings.profiles.entries.is_empty());
        if matches!(
            command,
            Command::Load | Command::Switch | Command::Focus | Command::Unload
        ) {
            ensure!(
                self.settings
                    .profiles
                    .entries
                    .iter()
                    .any(|e| e.id == profile),
                "This profile was removed"
            );
            if matches!(command, Command::Unload) {
                self.unload_profile(profile);
                return Ok(true);
            }
            if !loaded {
                if let Some(error) = self.profiles.errors.get(&profile) {
                    anyhow::bail!("Avatar cannot load: {error}. Retry it in Profiles first");
                }
                if self.pending_image.is_some()
                    || self.pending_vrm.is_some()
                    || self.profiles.loading.is_some()
                {
                    return Ok(false);
                }
                self.load_profile(ctx, profile);
                return Ok(false);
            }
            if matches!(command, Command::Switch) {
                // Replacement is explicit; other already loaded avatars stay on stage.
                if let Some(old) = origin
                    && old != profile
                {
                    self.unload_profile(old);
                }
            }
            if !matches!(command, Command::Load) {
                self.focus_profile(profile);
            } else if let Some(origin) = origin {
                self.focus_profile(origin);
            }
            return Ok(true);
        }
        ensure!(
            loaded,
            "Avatar is not loaded. Connect a Load avatar node before this action, or check its profile"
        );
        if profile == 0 && self.profiles.current.is_none() {
            self.execute_avatar_command(command, mode)?;
        } else {
            self.with_profile(profile, |app| app.execute_avatar_command(command, mode))
                .ok_or_else(|| anyhow::anyhow!("Avatar was unloaded"))??;
        }
        self.input_monitor.save_requested = true;
        Ok(true)
    }
    fn execute_avatar_command(&mut self, command: &Command, mode: Mode) -> Result<()> {
        let toggle = |old: bool| match mode {
            Mode::Toggle => !old,
            Mode::On => true,
            Mode::Off => false,
        };
        match command {
            Command::Pose => {
                let frozen = toggle(self.input_monitor.saved.config.pose.mode == PoseMode::Frozen);
                self.apply_api_action(crate::effect_api::Action::Pose { frozen })?;
            }
            Command::Preset(name) => {
                let index = self
                    .input_monitor
                    .saved
                    .presets
                    .iter()
                    .position(|p| &p.name == name)
                    .ok_or_else(|| anyhow::anyhow!("Preset was removed or renamed"))?;
                ensure!(
                    self.input_monitor
                        .apply_preset(index, &mut self.settings.mapping),
                    "Preset is not valid for this avatar"
                );
            }
            Command::Expression(id) => {
                let enabled = toggle(self.input_monitor.saved.config.expressions.contains(id));
                self.apply_api_action(crate::effect_api::Action::Expression {
                    id: id.clone(),
                    enabled,
                })?;
            }
            Command::Layers(id) => {
                let g = self
                    .input_monitor
                    .saved
                    .config
                    .layers
                    .groups
                    .iter_mut()
                    .find(|g| g.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("Layer group was removed"))?;
                g.active = toggle(g.active);
            }
            Command::Item(id) => {
                let item = self
                    .input_monitor
                    .saved
                    .config
                    .items
                    .iter_mut()
                    .find(|i| i.id == *id)
                    .ok_or_else(|| anyhow::anyhow!("Object was removed"))?;
                item.visible = toggle(item.visible);
            }
            Command::Image(id) => {
                let images = &mut self.input_monitor.saved.config.images;
                ensure!(
                    images.states.iter().any(|s| s.id == *id),
                    "Image action was removed"
                );
                if toggle(images.manual == Some(*id)) {
                    images.manual = Some(*id);
                } else if images.manual == Some(*id) {
                    images.manual = None;
                }
            }
            Command::Effect(id) => {
                ensure!(
                    self.input_monitor
                        .saved
                        .effects
                        .designs
                        .iter()
                        .any(|d| d.id == *id),
                    "Effect was removed"
                );
                self.input_monitor.effect_requests.push(*id);
            }
            Command::Imported(id) => {
                self.apply_api_action(crate::effect_api::Action::Imported { id: id.clone() })?
            }
            Command::Gesture(name) => {
                let gesture = crate::vrm::motion::Gesture::ALL
                    .into_iter()
                    .find(|g| g.name() == name)
                    .ok_or_else(|| anyhow::anyhow!("Unknown 3D animation"))?;
                self.vrm
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("This avatar is not a VRM or GLB humanoid"))?
                    .motion
                    .play(gesture);
            }
            _ => anyhow::bail!("This action belongs to the workspace"),
        }
        self.input_monitor.save_requested = true;
        Ok(())
    }
}
