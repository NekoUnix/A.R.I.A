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
}
const KINDS: [Kind; 8] = [
    Kind::Command,
    Kind::Reward,
    Kind::Follow,
    Kind::Subscription,
    Kind::Gift,
    Kind::Bits,
    Kind::Raid,
    Kind::Gesture,
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
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
#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub kind: Kind,
    pub match_name: String,
    pub minimum: u32,
    pub cooldown: f32,
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
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub chat_commands: bool,
    pub rules: Vec<Rule>,
    next_id: u64,
}
#[derive(Default)]
struct GestureState {
    duration: f32,
    latched: bool,
}
#[derive(Default)]
pub struct Events {
    seen: VecDeque<String>,
    last: BTreeMap<u64, f64>,
    gestures: BTreeMap<u64, GestureState>,
    pub log: VecDeque<String>,
    simulation: u64,
}
impl Events {
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
        if !settings.enabled || self.seen.contains(&event.id) {
            return Ok(vec![]);
        }
        self.seen.push_back(event.id.clone());
        if self.seen.len() > 1024 {
            self.seen.pop_front();
        }
        let mut targets = Vec::new();
        for rule in settings.rules.iter().take(128) {
            if rule.enabled
                && rule.validate()
                && rule.kind == event.kind
                && rule.kind != Kind::Gesture
                && event.amount >= rule.minimum
                && (rule.match_name.is_empty() || rule.match_name.eq_ignore_ascii_case(&event.name))
                && self
                    .last
                    .get(&rule.id)
                    .is_none_or(|last| now - *last >= f64::from(rule.cooldown))
            {
                targets.push(rule.target.clone().unwrap());
                self.last.insert(rule.id, now);
                if targets.len() == 16 {
                    break;
                }
            }
        }
        self.log(format!(
            "{:?} · {} · {} action(s)",
            event.kind,
            event.name,
            targets.len()
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
                {
                    self.last.insert(rule.id, now);
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
        let before = serde_json::to_string(settings).unwrap_or_default();
        ui.heading("Events & gestures");
        crate::theme::caption(
            ui,
            "Map a stream event or a held tracking condition to a saved action. Preview uses the same cooldown and matching rules as incoming events.",
        );
        crate::theme::caption(
            ui,
            "Connected chat can supply opt-in !commands. Other stream events use the authenticated integration. Direct rewards/subscriptions require additional provider authorization.",
        );
        ui.checkbox(&mut settings.enabled, "Enable event and gesture actions");
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
                    id,
                    name: "New rule".into(),
                    enabled: false,
                    kind: Kind::Reward,
                    match_name: String::new(),
                    minimum: 1,
                    cooldown: 2.,
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
        let mut simulate = None;
        for rule in &mut settings.rules {
            let rule_before = serde_json::to_string(rule).unwrap_or_default();
            crate::theme::card(ui, |ui| {
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
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                settings.enabled
                                    && rule.enabled
                                    && rule.validate()
                                    && rule.kind != Kind::Gesture,
                                egui::Button::new("Simulate event"),
                            )
                            .clicked()
                        {
                            simulate = Some((rule.kind, rule.match_name.clone(), rule.minimum));
                        }
                        if ui.small_button("Remove rule").clicked() {
                            remove = Some(rule.id);
                        }
                    });
                });
            });
            if rule_before != serde_json::to_string(rule).unwrap_or_default() {
                self.gestures.remove(&rule.id);
            }
        }
        if let Some(id) = remove {
            settings.rules.retain(|r| r.id != id);
            self.gestures.remove(&id);
            self.last.remove(&id);
        }
        let targets = if let Some((kind, name, amount)) = simulate {
            self.simulation = self.simulation.wrapping_add(1);
            self.receive(
                &Event {
                    id: format!("preview-{}", self.simulation),
                    kind,
                    name,
                    amount,
                },
                settings,
                now,
            )
            .unwrap_or_default()
        } else {
            vec![]
        };
        ui.collapsing("Recent event results", |ui| {
            for line in &self.log {
                ui.label(line);
            }
        });
        (
            before != serde_json::to_string(settings).unwrap_or_default(),
            targets,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
                id: 1,
                name: "Bonk".into(),
                enabled: true,
                kind: Kind::Bits,
                match_name: String::new(),
                minimum: 100,
                cooldown: 2.,
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
