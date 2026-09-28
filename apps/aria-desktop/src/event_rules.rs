//! Explicit event mappings and gesture conditions. Remote text never selects arbitrary targets.
use crate::actions::{Choice, Target};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Command,
    Reward,
    Follow,
    Subscription,
    Gift,
    Bits,
    Raid,
    Gesture,
    Tip,
    Like,
    Share,
    Custom,
}
const KINDS: [Kind; 12] = [
    Kind::Command,
    Kind::Reward,
    Kind::Follow,
    Kind::Subscription,
    Kind::Gift,
    Kind::Bits,
    Kind::Raid,
    Kind::Gesture,
    Kind::Tip,
    Kind::Like,
    Kind::Share,
    Kind::Custom,
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    #[default]
    Local,
    Twitch,
    YouTube,
    Kick,
    TikTok,
    X,
}
const PLATFORMS: [Platform; 6] = [
    Platform::Local,
    Platform::Twitch,
    Platform::YouTube,
    Platform::Kick,
    Platform::TikTok,
    Platform::X,
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    #[serde(default)]
    pub platform: Platform,
    #[serde(default)]
    pub test: bool,
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub amount: u32,
}
impl Event {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.id.is_empty()
                && self.id.len() <= 128
                && self.name.len() <= 128
                && self.amount <= 1_000_000,
            "Invalid event identity, name or amount"
        );
        Ok(())
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub input: String,
    pub threshold: f32,
    pub below: bool,
}
impl Condition {
    fn matches(&self, inputs: &aria_core::rig::Inputs, margin: f32) -> bool {
        inputs.get(&self.input).is_some_and(|value| {
            value.is_finite()
                && if self.below {
                    *value <= self.threshold + margin
                } else {
                    *value >= self.threshold - margin
                }
        })
    }
    fn valid(&self) -> bool {
        !self.input.is_empty()
            && self.input.len() <= 128
            && self.threshold.is_finite()
            && self.threshold.abs() <= 1e6
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub platform: Option<Platform>,
    #[serde(default)]
    pub accept_test: bool,
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub kind: Kind,
    pub match_name: String,
    pub minimum: u32,
    pub cooldown: f32,
    /// Maximum live dispatches per application session; zero means unlimited.
    #[serde(default)]
    pub session_limit: u32,
    pub target: Option<Target>,
    /// Gesture conditions are scoped to the currently edited avatar ID.
    pub profile: Option<u64>,
    pub threshold: f32,
    pub hold_seconds: f32,
    #[serde(default)]
    pub below: bool,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub any_condition: bool,
    #[serde(default)]
    pub release_margin: f32,
}
impl Rule {
    fn validate(&self) -> bool {
        self.name.len() <= 128
            && self.match_name.len() <= 128
            && self.cooldown.is_finite()
            && (0.1..=3600.).contains(&self.cooldown)
            && self.threshold.is_finite()
            && self.threshold.abs() <= 1e6
            && self.hold_seconds.is_finite()
            && (0.05..=10.).contains(&self.hold_seconds)
            && self.target.is_some()
            && self.conditions.len() <= 7
            && self.conditions.iter().all(Condition::valid)
            && self.release_margin.is_finite()
            && (0.0..=1e6).contains(&self.release_margin)
    }
    fn matches(&self, inputs: &aria_core::rig::Inputs, latched: bool) -> bool {
        let margin = if latched { self.release_margin } else { 0. };
        let primary = Condition {
            input: self.match_name.clone(),
            threshold: self.threshold,
            below: self.below,
        };
        let mut matches = std::iter::once(&primary)
            .chain(&self.conditions)
            .map(|c| c.matches(inputs, margin));
        if self.any_condition {
            matches.any(|v| v)
        } else {
            matches.all(|v| v)
        }
    }
}
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub chat_commands: bool,
    pub rules: Vec<Rule>,
    next_id: u64,
}
impl Settings {
    pub fn add_from_event(&mut self, event: &Event) -> bool {
        if self.rules.len() >= 128 || event.validate().is_err() {
            return false;
        }
        let Some(id) = self
            .next_id
            .max(self.rules.iter().map(|r| r.id).max().unwrap_or(0))
            .checked_add(1)
        else {
            return false;
        };
        self.next_id = id;
        self.rules.push(Rule {
            id,
            name: event.name.chars().take(32).collect(),
            enabled: false,
            kind: event.kind,
            platform: Some(event.platform),
            accept_test: false,
            match_name: event.name.clone(),
            minimum: event.amount.max(1),
            cooldown: 2.,
            session_limit: 0,
            target: None,
            profile: None,
            threshold: 0.5,
            hold_seconds: 0.3,
            below: false,
            conditions: vec![],
            any_condition: false,
            release_margin: 0.05,
        });
        true
    }
    fn duplicate(&mut self, id: u64) -> bool {
        let Some(mut rule) = self.rules.iter().find(|r| r.id == id).cloned() else {
            return false;
        };
        if self.rules.len() >= 128 {
            return false;
        }
        let Some(id) = self
            .next_id
            .max(self.rules.iter().map(|r| r.id).max().unwrap_or(0))
            .checked_add(1)
        else {
            return false;
        };
        self.next_id = id;
        rule.id = id;
        rule.enabled = false;
        rule.name = format!("{} copy", rule.name.chars().take(30).collect::<String>());
        self.rules.push(rule);
        true
    }
}
#[derive(Default)]
struct GestureState {
    duration: f32,
    latched: bool,
}
#[derive(Default)]
pub struct Events {
    pub page: usize,
    seen: VecDeque<String>,
    last: BTreeMap<u64, f64>,
    dispatched: BTreeMap<u64, u32>,
    gestures: BTreeMap<u64, GestureState>,
    pub log: VecDeque<String>,
    simulation: u64,
}
impl Events {
    fn preview(&mut self, rule: &Rule, now: f64) -> Vec<Target> {
        if rule.kind == Kind::Gesture || !rule.validate() {
            return vec![];
        }
        self.simulation = self.simulation.wrapping_add(1);
        let mut rule = rule.clone();
        rule.enabled = true;
        // Explicit previews keep cooldowns, but do not consume live session limits.
        rule.session_limit = 0;
        let rule_id = rule.id;
        let count = self.dispatched.get(&rule_id).copied();
        let event = Event {
            id: format!("preview-{}", self.simulation),
            platform: rule.platform.unwrap_or_default(),
            test: false,
            kind: rule.kind,
            name: rule.match_name.clone(),
            amount: rule.minimum,
        };
        let settings = Settings {
            enabled: true,
            rules: vec![rule],
            ..Default::default()
        };
        let targets = self.receive(&event, &settings, now).unwrap_or_default();
        if let Some(count) = count {
            self.dispatched.insert(rule_id, count);
        } else {
            self.dispatched.remove(&rule_id);
        }
        targets
    }
    fn log(&mut self, message: String) {
        self.log.push_front(message);
        self.log.truncate(30);
    }
    pub fn receive(
        &mut self,
        event: &Event,
        settings: &Settings,
        now: f64,
    ) -> anyhow::Result<Vec<Target>> {
        event.validate()?;
        anyhow::ensure!(now.is_finite(), "Invalid event clock");
        let identity = format!("{:?}:{}", event.platform, event.id);
        if !settings.enabled {
            self.log(format!("{} · reactions disabled", event.name));
            return Ok(vec![]);
        }
        if self.seen.contains(&identity) {
            self.log(format!("{} · duplicate ignored", event.name));
            return Ok(vec![]);
        }
        self.seen.push_back(identity);
        if self.seen.len() > 1024 {
            self.seen.pop_front();
        }
        let mut targets = Vec::new();
        let mut reasons = Vec::new();
        for rule in settings.rules.iter().take(128) {
            if rule.kind == event.kind
                && rule.platform.is_none_or(|p| p == event.platform)
                && rule.kind != Kind::Gesture
                && (rule.match_name.is_empty() || rule.match_name.eq_ignore_ascii_case(&event.name))
            {
                let reason = if !rule.enabled {
                    Some("rule disabled")
                } else if !rule.validate() {
                    Some("choose or repair the action/settings")
                } else if event.test && !rule.accept_test {
                    Some("provider test ignored")
                } else if event.amount < rule.minimum {
                    Some("below minimum amount")
                } else if rule.session_limit > 0
                    && self.dispatched.get(&rule.id).copied().unwrap_or(0) >= rule.session_limit
                {
                    Some("session limit reached")
                } else if !self
                    .last
                    .get(&rule.id)
                    .is_none_or(|last| now - *last >= f64::from(rule.cooldown))
                {
                    Some("cooldown active")
                } else if targets.len() == 16 {
                    Some("event action limit reached")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    reasons.push(format!("{}: {reason}", rule.name));
                    continue;
                }
                targets.push(rule.target.clone().unwrap());
                self.last.insert(rule.id, now);
                let count = self.dispatched.entry(rule.id).or_default();
                *count = count.saturating_add(1);
                reasons.push(format!("{}: dispatched", rule.name));
            }
        }
        if reasons.is_empty() {
            reasons.push("no rule matches this platform, kind and name".into());
        }
        self.log(format!(
            "{:?} · {:?} · {} · {} action(s){} — {}",
            event.platform,
            event.kind,
            event.name,
            targets.len(),
            if event.test { " · test" } else { "" },
            reasons.join("; ")
        ));
        Ok(targets)
    }
    pub fn gestures(
        &mut self,
        settings: &Settings,
        profile: Option<u64>,
        inputs: &aria_core::rig::Inputs,
        dt: f32,
        now: f64,
        live: bool,
    ) -> Vec<Target> {
        let mut targets = Vec::new();
        if !settings.enabled || !live {
            self.gestures.clear();
            return targets;
        }
        if !dt.is_finite() || dt <= 0. || !now.is_finite() {
            return targets;
        }
        self.gestures.retain(|id, _| {
            settings
                .rules
                .iter()
                .take(128)
                .any(|r| r.id == *id && r.enabled && r.kind == Kind::Gesture && r.validate())
        });
        for rule in settings
            .rules
            .iter()
            .take(128)
            .filter(|r| r.kind == Kind::Gesture && r.enabled && r.validate())
        {
            let state = self.gestures.entry(rule.id).or_default();
            let matched = rule.profile == profile && rule.matches(inputs, state.latched);
            if !matched {
                state.duration = 0.;
                state.latched = false;
                continue;
            }
            state.duration += dt.min(0.1);
            if !state.latched && state.duration >= rule.hold_seconds {
                state.latched = true;
                if self
                    .last
                    .get(&rule.id)
                    .is_none_or(|last| now - *last >= f64::from(rule.cooldown))
                    && (rule.session_limit == 0
                        || self.dispatched.get(&rule.id).copied().unwrap_or(0) < rule.session_limit)
                {
                    self.last.insert(rule.id, now);
                    let count = self.dispatched.entry(rule.id).or_default();
                    *count = count.saturating_add(1);
                    targets.push(rule.target.clone().unwrap());
                    if targets.len() == 16 {
                        break;
                    }
                }
            }
        }
        targets
    }
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        settings: &mut Settings,
        choices: &[Choice],
        profile: Option<u64>,
        inputs: &aria_core::rig::Inputs,
        now: f64,
    ) -> (bool, Vec<Target>) {
        let before = settings.clone();
        ui.heading("Your reactions");
        crate::theme::caption(
            ui,
            "Choose what your audience can trigger. Preview a reaction before enabling it.",
        );
        ui.checkbox(
            &mut settings.chat_commands,
            "Accept !commands from connected Twitch / YouTube chat",
        );
        if settings.chat_commands {
            crate::theme::caption(
                ui,
                "All viewers can trigger enabled Command rules. Match a name such as !bonk; arguments are ignored. Existing chat history is never replayed. Rule cooldowns apply.",
            );
        }
        ui.menu_button("Quick-start reaction", |ui| {
            for (label, kind, name) in [
                ("Chat command", Kind::Command, "!bonk"),
                ("Channel reward", Kind::Reward, "Your reward name"),
                ("New follower", Kind::Follow, ""),
                ("Subscription", Kind::Subscription, ""),
                ("Gift", Kind::Gift, ""),
            ] {
                if ui
                    .add_enabled(settings.rules.len() < 128, egui::Button::new(label))
                    .clicked()
                {
                    let event = Event {
                        platform: Platform::Local,
                        test: false,
                        id: "template".into(),
                        kind,
                        name: name.into(),
                        amount: 1,
                    };
                    if settings.add_from_event(&event) {
                        let rule = settings.rules.last_mut().unwrap();
                        rule.platform = None;
                        rule.name = label.into();
                    }
                    ui.close();
                }
            }
        });
        crate::theme::caption(
            ui,
            "Templates and copies start disabled. Choose an action, preview it, then enable the reaction.",
        );
        if ui
            .add_enabled(settings.rules.len() < 128, egui::Button::new("Add rule"))
            .clicked()
        {
            let max = settings
                .rules
                .iter()
                .map(|r| r.id)
                .max()
                .unwrap_or(0)
                .max(settings.next_id);
            if let Some(id) = max.checked_add(1) {
                settings.next_id = id;
                settings.rules.push(Rule {
                    platform: None,
                    accept_test: false,
                    id,
                    name: "New rule".into(),
                    enabled: false,
                    kind: Kind::Reward,
                    match_name: String::new(),
                    minimum: 1,
                    cooldown: 2.,
                    session_limit: 0,
                    target: None,
                    profile,
                    threshold: 0.5,
                    hold_seconds: 0.3,
                    below: false,
                    conditions: vec![],
                    any_condition: false,
                    release_margin: 0.05,
                });
            }
        }
        let mut remove = None;
        let mut duplicate = None;
        let can_duplicate = settings.rules.len() < 128;
        let single_rule = settings.rules.len() == 1;
        let mut simulate = None;
        for rule in &mut settings.rules {
            let rule_before = rule.clone();
            let heading = format!(
                "{} · {:?} · {}",
                rule.name,
                rule.kind,
                if rule.enabled { "Enabled" } else { "Off" }
            );
            crate::theme::category(
                ui,
                ("reaction-editor", rule.id),
                &heading,
                single_rule || rule.target.is_none(),
                |ui| {
                    ui.push_id(rule.id, |ui| {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut rule.enabled, "");
                        ui.add(egui::TextEdit::singleline(&mut rule.name).char_limit(80));
                    });
                    egui::ComboBox::from_id_salt("event-kind")
                        .selected_text(format!("{:?}", rule.kind))
                        .show_ui(ui, |ui| {
                            for kind in KINDS {
                                ui.selectable_value(&mut rule.kind, kind, format!("{kind:?}"));
                            }
                        });
                    if rule.kind == Kind::Gesture {
                        ui.label(format!("Avatar ID: {:?}", rule.profile));
                        if ui.button("Use current avatar").clicked() {
                            rule.profile = profile;
                        }
                        egui::ComboBox::from_id_salt("gesture-input")
                            .selected_text(if rule.match_name.is_empty() {
                                "Choose input"
                            } else {
                                &rule.match_name
                            })
                            .show_ui(ui, |ui| {
                                for name in inputs.keys() {
                                    ui.selectable_value(&mut rule.match_name, name.clone(), name);
                                }
                            });
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut rule.below, false, "At least");
                            ui.selectable_value(&mut rule.below, true, "At most");
                            ui.add(
                                egui::DragValue::new(&mut rule.threshold)
                                    .speed(0.01)
                                    .range(-1e6..=1e6),
                            );
                        });
                        ui.add(
                            egui::Slider::new(&mut rule.hold_seconds, 0.05..=10.)
                                .text("Hold seconds"),
                        );
                        if let Some(v) = inputs.get(&rule.match_name) {
                            ui.label(format!("Current input: {v:.3}"));
                        }
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut rule.any_condition, false, "All conditions");
                            ui.selectable_value(&mut rule.any_condition, true, "Any condition");
                        });
                        let mut remove_condition = None;
                        for (index, condition) in rule.conditions.iter_mut().enumerate() {
                            ui.push_id(index, |ui| {
                                egui::ComboBox::from_id_salt("extra-input").selected_text(if condition.input.is_empty() { "Choose input" } else { &condition.input }).show_ui(ui, |ui| {
                                    for name in inputs.keys() { ui.selectable_value(&mut condition.input, name.clone(), name); }
                                });
                                ui.horizontal_wrapped(|ui| {
                                    ui.selectable_value(&mut condition.below, false, "At least");
                                    ui.selectable_value(&mut condition.below, true, "At most");
                                    ui.add(egui::DragValue::new(&mut condition.threshold).speed(0.01).range(-1e6..=1e6));
                                    if ui.small_button("Remove condition").clicked() { remove_condition = Some(index); }
                                    if let Some(value) = inputs.get(&condition.input) { ui.label(format!("Now: {value:.3}")); }
                                });
                            });
                        }
                        if let Some(index) = remove_condition { rule.conditions.remove(index); }
                        if ui.add_enabled(rule.conditions.len() < 7, egui::Button::new("Add input condition")).clicked() {
                            rule.conditions.push(Condition { input: String::new(), threshold: 0.5, below: false });
                        }
                        ui.add(egui::DragValue::new(&mut rule.release_margin).speed(0.01).range(0.0..=1e6).prefix("Release margin: "));
                        crate::theme::caption(ui, "Hold the selected conditions together. The release margin prevents jitter from retriggering; it uses each input's units.");
                    } else {
                        egui::ComboBox::from_id_salt("event-platform").selected_text(rule.platform.map_or("Any platform".into(),|p|format!("{p:?}"))).show_ui(ui,|ui| {
                            ui.selectable_value(&mut rule.platform,None,"Any platform");
                            for platform in PLATFORMS { ui.selectable_value(&mut rule.platform,Some(platform),format!("{platform:?}")); }
                        });
                        ui.checkbox(&mut rule.accept_test,"Accept provider test events");
                        ui.add(
                            egui::TextEdit::singleline(&mut rule.match_name)
                                .hint_text("Exact event / reward / command name; blank = any")
                                .char_limit(128),
                        );
                        ui.add(
                            egui::DragValue::new(&mut rule.minimum)
                                .range(1..=1_000_000)
                                .prefix("Minimum amount: "),
                        );
                    }
                    egui::ComboBox::from_id_salt("event-target")
                        .width(400.)
                        .selected_text(
                            rule.target
                                .as_ref()
                                .and_then(|t| choices.iter().find(|c| &c.target == t))
                                .map_or("Choose / repair action", |c| &c.label),
                        )
                        .show_ui(ui, |ui| {
                            for choice in choices {
                                ui.selectable_value(
                                    &mut rule.target,
                                    Some(choice.target.clone()),
                                    &choice.label,
                                );
                            }
                        });
                    ui.add(
                        egui::Slider::new(&mut rule.cooldown, 0.1..=3600.)
                            .logarithmic(true)
                            .text("Cooldown seconds"),
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::DragValue::new(&mut rule.session_limit).range(0..=1_000_000).prefix("Session limit: "));
                        ui.label(format!("{} dispatched · 0 limit = unlimited", self.dispatched.get(&rule.id).copied().unwrap_or(0)));
                        if ui.small_button("Reset count").clicked() { self.dispatched.remove(&rule.id); }
                    });
                    crate::theme::caption(ui, "Counts dispatch attempts, not playback success. Resets when ARIA restarts. Preview does not consume the limit.");
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                rule.validate()
                                    && rule.kind != Kind::Gesture,
                                egui::Button::new("Preview reaction"),
                            )
                            .clicked()
                        {
                            simulate = Some(rule.clone());
                        }
                        if ui.small_button("Remove rule").clicked() {
                            remove = Some(rule.id);
                        }
                        if ui.add_enabled(can_duplicate, egui::Button::new("Duplicate")).clicked() { duplicate = Some(rule.id); }
                    });
                });
                },
            );
            if rule_before != *rule {
                self.gestures.remove(&rule.id);
            }
        }
        if let Some(id) = remove {
            settings.rules.retain(|r| r.id != id);
            self.gestures.remove(&id);
            self.last.remove(&id);
            self.dispatched.remove(&id);
        }
        if let Some(id) = duplicate {
            settings.duplicate(id);
        }
        let targets = if let Some(rule) = simulate {
            self.preview(&rule, now)
        } else {
            vec![]
        };
        ui.collapsing("Recent event results", |ui| {
            crate::theme::caption(ui, "Dispatch means the reaction was handed to the action system; it is not a completion or refund receipt.");
            if ui.small_button("Clear history").clicked() { self.log.clear(); }
            for line in &self.log {
                ui.label(line);
            }
        });
        (before != *settings, targets)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "manual UI dirty-check microbenchmark; reports timings without a machine-specific threshold"]
    fn benchmark_editor_change_detection() {
        use std::{hint::black_box, time::Instant};
        let mut settings = settings();
        for _ in 1..128 {
            assert!(settings.duplicate(1));
        }
        let started = Instant::now();
        for _ in 0..500 {
            let before = serde_json::to_string(black_box(&settings)).unwrap();
            for rule in &settings.rules {
                let snapshot = serde_json::to_string(black_box(rule)).unwrap();
                black_box(snapshot != serde_json::to_string(black_box(rule)).unwrap());
            }
            black_box(before != serde_json::to_string(black_box(&settings)).unwrap());
        }
        let json = started.elapsed();
        let started = Instant::now();
        for _ in 0..500 {
            let before = black_box(&settings).clone();
            for rule in &settings.rules {
                let snapshot = black_box(rule).clone();
                black_box(snapshot != *black_box(rule));
            }
            black_box(before != *black_box(&settings));
        }
        eprintln!(
            "128-rule dirty checks, 500 frames: JSON={json:?}, direct={:?}",
            started.elapsed()
        );
    }
    #[test]
    fn session_limits_preview_reset_and_skip_reasons() {
        let mut settings = settings();
        settings.rules[0].session_limit = 1;
        let mut runtime = Events::default();
        let mut event = Event {
            platform: Platform::Local,
            test: false,
            id: "first".into(),
            kind: Kind::Bits,
            name: String::new(),
            amount: 100,
        };
        assert_eq!(runtime.receive(&event, &settings, 0.).unwrap().len(), 1);
        event.id = "second".into();
        assert!(runtime.receive(&event, &settings, 3.).unwrap().is_empty());
        assert!(runtime.log[0].contains("session limit reached"));
        assert_eq!(runtime.preview(&settings.rules[0], 6.).len(), 1);
        assert_eq!(runtime.dispatched[&1], 1);
        runtime.dispatched.remove(&1);
        event.id = "third".into();
        assert_eq!(runtime.receive(&event, &settings, 9.).unwrap().len(), 1);
        assert!(runtime.receive(&event, &settings, 12.).unwrap().is_empty());
        assert!(runtime.log[0].contains("duplicate ignored"));
        settings.rules[0].session_limit = 0;
        event.id = "fourth".into();
        assert!(runtime.receive(&event, &settings, 9.1).unwrap().is_empty());
        assert!(runtime.log[0].contains("cooldown active"));
        event.id = "low".into();
        event.amount = 1;
        runtime.receive(&event, &settings, 20.).unwrap();
        assert!(runtime.log[0].contains("below minimum amount"));
        for n in 0..50 {
            event.id = format!("low-{n}");
            runtime.receive(&event, &settings, 30.).unwrap();
        }
        assert_eq!(runtime.log.len(), 30);
    }
    #[test]
    fn duplicates_are_disabled_keep_targets_and_never_reuse_ids() {
        let mut settings = settings();
        settings.rules[0].session_limit = 5;
        assert!(settings.duplicate(1));
        assert_eq!(settings.rules[1].id, 2);
        assert!(!settings.rules[1].enabled);
        assert_eq!(settings.rules[1].target, settings.rules[0].target);
        assert_eq!(settings.rules[1].session_limit, 5);
        settings.rules.pop();
        assert!(settings.duplicate(1));
        assert_eq!(settings.rules[1].id, 3);
        let mut legacy = serde_json::to_value(&settings.rules[0]).unwrap();
        legacy.as_object_mut().unwrap().remove("session_limit");
        assert_eq!(
            serde_json::from_value::<Rule>(legacy)
                .unwrap()
                .session_limit,
            0
        );
    }
    #[test]
    fn gesture_session_limit_requires_release_after_reset() {
        let mut settings = settings();
        let rule = &mut settings.rules[0];
        rule.kind = Kind::Gesture;
        rule.match_name = "Smile".into();
        rule.hold_seconds = 0.05;
        rule.session_limit = 1;
        let mut runtime = Events::default();
        let high = BTreeMap::from([("Smile".into(), 1.)]);
        let low = BTreeMap::from([("Smile".into(), 0.)]);
        assert_eq!(
            runtime
                .gestures(&settings, Some(1), &high, 0.1, 0., true)
                .len(),
            1
        );
        runtime.gestures(&settings, Some(1), &low, 0.1, 3., true);
        assert!(
            runtime
                .gestures(&settings, Some(1), &high, 0.1, 4., true)
                .is_empty()
        );
        runtime.dispatched.remove(&1);
        assert!(
            runtime
                .gestures(&settings, Some(1), &high, 0.1, 5., true)
                .is_empty()
        );
        runtime.gestures(&settings, Some(1), &low, 0.1, 6., true);
        assert_eq!(
            runtime
                .gestures(&settings, Some(1), &high, 0.1, 7., true)
                .len(),
            1
        );
    }
    #[test]
    fn explicit_preview_works_before_enabling_and_keeps_cooldowns() {
        let mut settings = settings();
        settings.enabled = false;
        settings.rules[0].enabled = false;
        let mut runtime = Events::default();
        assert_eq!(runtime.preview(&settings.rules[0], 0.).len(), 1);
        assert!(runtime.preview(&settings.rules[0], 0.01).is_empty());
        assert_eq!(runtime.preview(&settings.rules[0], 10.).len(), 1);
        assert!(!settings.enabled && !settings.rules[0].enabled);
    }
    #[test]
    fn platform_filters_test_events_and_old_profiles_are_compatible() {
        let mut settings = settings();
        settings.rules[0].platform = Some(Platform::Twitch);
        let mut event: Event = serde_json::from_value(
            serde_json::json!({"id":"same","kind":"Reward","name":"Bonk","amount":1}),
        )
        .unwrap();
        assert_eq!(event.platform, Platform::Local);
        settings.rules[0].match_name.clear();
        settings.rules[0].minimum = 1;
        settings.rules[0].kind = Kind::Reward;
        let mut runtime = Events::default();
        assert!(runtime.receive(&event, &settings, 0.).unwrap().is_empty());
        event.platform = Platform::Twitch;
        assert_eq!(runtime.receive(&event, &settings, 3.).unwrap().len(), 1);
        event.id = "provider-test".into();
        event.test = true;
        assert!(runtime.receive(&event, &settings, 6.).unwrap().is_empty());
        settings.rules[0].accept_test = true;
        event.id = "provider-test-2".into();
        assert_eq!(runtime.receive(&event, &settings, 9.).unwrap().len(), 1);
        assert!(runtime.receive(&event, &settings, 12.).unwrap().is_empty());
        let mut saved = serde_json::to_value(&settings.rules[0]).unwrap();
        saved.as_object_mut().unwrap().remove("platform");
        saved.as_object_mut().unwrap().remove("accept_test");
        let legacy: Rule = serde_json::from_value(saved).unwrap();
        assert!(legacy.platform.is_none());
        assert!(!legacy.accept_test);
    }
    #[test]
    fn composite_gestures_require_all_inputs_and_hysteresis_release() {
        let mut s = settings();
        let rule = &mut s.rules[0];
        rule.kind = Kind::Gesture;
        rule.match_name = "Smile".into();
        rule.hold_seconds = 0.1;
        rule.release_margin = 0.1;
        rule.conditions.push(Condition {
            input: "Blink".into(),
            threshold: 0.2,
            below: true,
        });
        let mut inputs = BTreeMap::from([("Smile".into(), 0.8)]);
        let mut runtime = Events::default();
        assert!(
            runtime
                .gestures(&s, Some(1), &inputs, 0.1, 0., true)
                .is_empty()
        );
        inputs.insert("Blink".into(), 0.1);
        assert_eq!(
            runtime.gestures(&s, Some(1), &inputs, 0.1, 1., true).len(),
            1
        );
        inputs.insert("Smile".into(), 0.45);
        assert!(
            runtime
                .gestures(&s, Some(1), &inputs, 0.1, 5., true)
                .is_empty()
        );
        inputs.insert("Smile".into(), 0.6);
        assert!(
            runtime
                .gestures(&s, Some(1), &inputs, 0.1, 6., true)
                .is_empty()
        );
        inputs.insert("Blink".into(), 0.4);
        runtime.gestures(&s, Some(1), &inputs, 0.1, 7., true);
        inputs.insert("Blink".into(), 0.1);
        assert_eq!(
            runtime.gestures(&s, Some(1), &inputs, 0.1, 8., true).len(),
            1
        );
        s.rules[0].any_condition = true;
        inputs.remove("Smile");
        assert!(s.rules[0].matches(&inputs, false));
        inputs.insert("Blink".into(), f32::NAN);
        assert!(!s.rules[0].matches(&inputs, false));
    }
    #[test]
    fn old_rules_migrate_and_disabled_gestures_discard_partial_holds() {
        let mut s = settings();
        s.rules[0].kind = Kind::Gesture;
        s.rules[0].match_name = "Smile".into();
        let mut json = serde_json::to_value(&s.rules[0]).unwrap();
        for key in ["below", "conditions", "any_condition", "release_margin"] {
            json.as_object_mut().unwrap().remove(key);
        }
        let old: Rule = serde_json::from_value(json).unwrap();
        assert!(
            old.conditions.is_empty()
                && !old.below
                && !old.any_condition
                && old.release_margin == 0.
        );
        let inputs = BTreeMap::from([("Smile".into(), 1.)]);
        let mut runtime = Events::default();
        runtime.gestures(&s, Some(1), &inputs, 0.1, 0., true);
        s.rules[0].enabled = false;
        runtime.gestures(&s, Some(1), &inputs, 0.1, 1., true);
        s.rules[0].enabled = true;
        assert!(
            runtime
                .gestures(&s, Some(1), &inputs, 0.1, 2., true)
                .is_empty()
        );
        assert_eq!(
            runtime.gestures(&s, Some(1), &inputs, 0.1, 2.1, true).len(),
            1
        );
    }
    fn settings() -> Settings {
        Settings {
            chat_commands: false,
            enabled: true,
            rules: vec![Rule {
                platform: None,
                accept_test: false,
                id: 1,
                name: "Bonk".into(),
                enabled: true,
                kind: Kind::Bits,
                match_name: String::new(),
                minimum: 100,
                cooldown: 2.,
                session_limit: 0,
                target: Some(Target::Scene(3)),
                profile: Some(1),
                threshold: 0.5,
                hold_seconds: 0.2,
                below: false,
                conditions: vec![],
                any_condition: false,
                release_margin: 0.,
            }],
            next_id: 1,
        }
    }
    #[test]
    fn events_deduplicate_and_respect_threshold_cooldown_and_disable() {
        let mut runtime = Events::default();
        let mut s = settings();
        let mut e = Event {
            platform: Platform::Local,
            test: false,
            id: "a".into(),
            kind: Kind::Bits,
            name: "".into(),
            amount: 99,
        };
        assert!(runtime.receive(&e, &s, 0.).unwrap().is_empty());
        e.amount = 100;
        e.id = "b".into();
        assert_eq!(runtime.receive(&e, &s, 0.).unwrap().len(), 1);
        assert!(runtime.receive(&e, &s, 5.).unwrap().is_empty());
        e.id = "c".into();
        assert!(runtime.receive(&e, &s, 1.).unwrap().is_empty());
        e.id = "d".into();
        assert_eq!(runtime.receive(&e, &s, 2.).unwrap().len(), 1);
        s.enabled = false;
        e.id = "e".into();
        assert!(runtime.receive(&e, &s, 10.).unwrap().is_empty());
    }
    #[test]
    fn gestures_hold_latch_and_require_release_and_correct_avatar() {
        let mut s = settings();
        s.rules[0].kind = Kind::Gesture;
        s.rules[0].match_name = "MouthOpen".into();
        let mut runtime = Events::default();
        let mut inputs = BTreeMap::from([("MouthOpen".into(), 0.8)]);
        assert!(
            runtime
                .gestures(&s, Some(2), &inputs, 0.1, 0., true)
                .is_empty()
        );
        assert!(
            runtime
                .gestures(&s, Some(1), &inputs, 0.1, 1., true)
                .is_empty()
        );
        assert_eq!(
            runtime.gestures(&s, Some(1), &inputs, 0.1, 1.1, true).len(),
            1
        );
        assert!(
            runtime
                .gestures(&s, Some(1), &inputs, 0.1, 10., true)
                .is_empty()
        );
        inputs.clear();
        runtime.gestures(&s, Some(1), &inputs, 0.1, 11., true);
        inputs.insert("MouthOpen".into(), 0.8);
        runtime.gestures(&s, Some(1), &inputs, 0.1, 12., true);
        assert_eq!(
            runtime
                .gestures(&s, Some(1), &inputs, 0.1, 12.1, true)
                .len(),
            1
        );
    }
}
