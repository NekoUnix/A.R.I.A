//! Saved, bounded action graphs. Execution plans are snapshots, never live editor data.
use anyhow::{Result, ensure};
use aria_core::shortcuts::Shortcut;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
mod editor;
pub use editor::Editor;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Command {
    Pose,
    Preset(String),
    Expression(String),
    Layers(u64),
    Item(u64),
    Image(u64),
    Effect(u64),
    Imported(String),
    Gesture(String),
    Load,
    Unload,
    Focus,
    Switch,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Target {
    Avatar { profile: u64, command: Command },
    Scene(u64),
    Music(MusicCommand),
    Sound(u64),
    StopSounds,
    Graph(u64),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MusicCommand {
    PlayPause,
    Stop,
    Next,
}
#[derive(Clone)]
pub struct Choice {
    pub target: Target,
    pub label: String,
    pub shortcut: Option<Shortcut>,
    pub legacy_label: Option<String>,
}

pub fn imported_shortcut(keys: &[u16]) -> Option<Shortcut> {
    let mut result = Shortcut::default();
    for &key in keys {
        match key {
            0x11 | 0xa2 | 0xa3 => result.ctrl = true,
            0x10 | 0xa0 | 0xa1 => result.shift = true,
            0x12 | 0xa4 | 0xa5 => result.alt = true,
            0x5b | 0x5c => result.win = true,
            _ if result.key == 0 => result.key = key,
            _ => return None,
        }
    }
    result.validate().ok().map(|()| result)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub graphs: Vec<Graph>,
    pub bindings: Vec<Binding>,
    pub shortcuts_enabled: bool,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            graphs: Vec::new(),
            bindings: Vec::new(),
            shortcuts_enabled: true,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Binding {
    pub target: Target,
    /// None explicitly clears an inherited shortcut.
    pub shortcut: Option<Shortcut>,
}
impl Library {
    pub fn shortcut(&self, choice: &Choice) -> Option<Shortcut> {
        self.bindings
            .iter()
            .find(|b| b.target == choice.target)
            .map_or(choice.shortcut, |b| b.shortcut)
    }
    pub fn bind(&mut self, target: Target, shortcut: Option<Shortcut>) {
        self.bindings.retain(|b| b.target != target);
        self.bindings.push(Binding { target, shortcut });
        if shortcut.is_some() {
            self.shortcuts_enabled = true;
        }
    }
    pub fn next_id(&self) -> u64 {
        self.graphs
            .iter()
            .map(|g| g.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }
}
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Toggle,
    On,
    Off,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Step {
    Start,
    Action {
        target: Option<Target>,
        mode: Mode,
    },
    RepeatAction {
        target: Option<Target>,
        mode: Mode,
        count: u16,
        interval: f32,
    },
    Branch {
        name: String,
        comparison: Compare,
        value: f64,
        true_next: Option<u64>,
    },
    Delay {
        seconds: f32,
    },
    Variable {
        name: String,
        operation: Math,
        value: f64,
    },
    Input {
        name: String,
        channel: String,
    },
    Require {
        name: String,
        comparison: Compare,
        value: f64,
    },
    End,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Math {
    Set,
    Add,
    Subtract,
    Multiply,
    Divide,
    Minimum,
    Maximum,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compare {
    AtLeast,
    AtMost,
    Equal,
}
fn variable_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    pub position: [f32; 2],
    pub step: Step,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Graph {
    pub id: u64,
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<[u64; 2]>,
}
impl Graph {
    pub fn branch_recipe(id: u64) -> Self {
        Self {
            id,
            name: "Choose and repeat".into(),
            nodes: vec![
                Node {
                    id: 1,
                    position: [30., 40.],
                    step: Step::Start,
                },
                Node {
                    id: 2,
                    position: [30., 210.],
                    step: Step::Input {
                        name: "mouth".into(),
                        channel: "MouthOpen".into(),
                    },
                },
                Node {
                    id: 3,
                    position: [280., 120.],
                    step: Step::Branch {
                        name: "mouth".into(),
                        comparison: Compare::AtLeast,
                        value: 0.5,
                        true_next: Some(4),
                    },
                },
                Node {
                    id: 4,
                    position: [530., 40.],
                    step: Step::RepeatAction {
                        target: None,
                        mode: Mode::Toggle,
                        count: 3,
                        interval: 1.,
                    },
                },
                Node {
                    id: 5,
                    position: [530., 210.],
                    step: Step::Action {
                        target: None,
                        mode: Mode::Toggle,
                    },
                },
                Node {
                    id: 6,
                    position: [780., 120.],
                    step: Step::End,
                },
            ],
            edges: vec![[1, 2], [2, 3], [3, 4], [3, 5], [4, 6], [5, 6]],
        }
    }
    /// Starter layouts contain no workspace IDs; users explicitly assign actions.
    pub fn recipe(id: u64, parallel: bool) -> Self {
        let action = || Step::Action {
            target: None,
            mode: Mode::Toggle,
        };
        let (name, steps, edges) = if parallel {
            (
                "Together",
                vec![Step::Start, action(), action(), Step::End],
                vec![[1, 2], [1, 3], [2, 4], [3, 4]],
            )
        } else {
            (
                "Timed sequence",
                vec![
                    Step::Start,
                    action(),
                    Step::Delay { seconds: 2.0 },
                    action(),
                    Step::End,
                ],
                vec![[1, 2], [2, 3], [3, 4], [4, 5]],
            )
        };
        Self {
            id,
            name: name.into(),
            nodes: steps
                .into_iter()
                .enumerate()
                .map(|(i, step)| Node {
                    id: i as u64 + 1,
                    position: if parallel {
                        match i {
                            0 => [30., 120.],
                            1 => [280., 40.],
                            2 => [280., 230.],
                            _ => [560., 120.],
                        }
                    } else {
                        [30. + (i % 3) as f32 * 250., 40. + (i / 3) as f32 * 210.]
                    },
                    step,
                })
                .collect(),
            edges,
        }
    }
    pub fn new(id: u64) -> Self {
        Self {
            id,
            name: "New action".into(),
            nodes: vec![
                Node {
                    id: 1,
                    position: [30., 40.],
                    step: Step::Start,
                },
                Node {
                    id: 2,
                    position: [300., 40.],
                    step: Step::Action {
                        target: None,
                        mode: Mode::Toggle,
                    },
                },
                Node {
                    id: 3,
                    position: [570., 40.],
                    step: Step::End,
                },
            ],
            edges: vec![[1, 2], [2, 3]],
        }
    }
    pub fn structure(&self) -> Result<Vec<u64>> {
        ensure!(
            !self.name.trim().is_empty() && self.name.chars().count() <= 80,
            "Name the action (1–80 characters)"
        );
        ensure!(
            !self.nodes.is_empty() && self.nodes.len() <= 256 && self.edges.len() <= 1024,
            "Use 1–256 nodes and at most 1,024 connections"
        );
        let ids: BTreeSet<_> = self.nodes.iter().map(|n| n.id).collect();
        ensure!(ids.len() == self.nodes.len(), "Node IDs must be unique");
        let starts: Vec<_> = self
            .nodes
            .iter()
            .filter(|n| matches!(n.step, Step::Start))
            .collect();
        ensure!(starts.len() == 1, "Use exactly one Start node");
        ensure!(
            self.nodes.iter().any(|n| matches!(n.step, Step::End)),
            "Add an End node"
        );
        for node in &self.nodes {
            match &node.step {
                Step::Variable { name, value, .. }
                | Step::Require { name, value, .. }
                | Step::Branch { name, value, .. } => {
                    ensure!(
                        variable_name(name) && value.is_finite() && value.abs() <= 1e12,
                        "Use a valid variable name and a finite value within ±1e12"
                    );
                }
                Step::Input { name, channel } => {
                    ensure!(
                        variable_name(name) && !channel.is_empty() && channel.len() <= 128,
                        "Name a variable and tracking input"
                    );
                }
                _ => {}
            }
            ensure!(
                node.position
                    .iter()
                    .all(|v| v.is_finite() && (0. ..=4000.).contains(v)),
                "Node {} is outside the canvas",
                node.id
            );
            if let Step::Delay { seconds } = node.step {
                ensure!(
                    seconds.is_finite() && (0. ..=300.).contains(&seconds),
                    "Delay must be 0–300 seconds"
                );
            }
            if let Step::RepeatAction {
                count, interval, ..
            } = node.step
            {
                ensure!(
                    (1..=100).contains(&count)
                        && interval.is_finite()
                        && (0.05..=300.).contains(&interval),
                    "Repeat an action 1–100 times with 0.05–300 seconds between successes"
                );
            }
            if let Step::Branch { true_next, .. } = node.step {
                let outputs: Vec<_> = self
                    .edges
                    .iter()
                    .filter(|e| e[0] == node.id)
                    .map(|e| e[1])
                    .collect();
                ensure!(
                    outputs.len() == 2 && true_next.is_some_and(|id| outputs.contains(&id)),
                    "Branch {} needs two outgoing connections and an assigned True path",
                    node.id
                );
            }
        }
        let mut unique = BTreeSet::new();
        for &[from, to] in &self.edges {
            ensure!(
                from != to && ids.contains(&from) && ids.contains(&to) && unique.insert([from, to]),
                "Invalid or duplicate connection {from} → {to}"
            );
            ensure!(to != starts[0].id, "Start cannot have incoming connections");
            ensure!(
                !self
                    .nodes
                    .iter()
                    .any(|n| n.id == from && matches!(n.step, Step::End)),
                "End cannot have outgoing connections"
            );
        }
        let mut incoming: BTreeMap<_, usize> = ids.iter().map(|&id| (id, 0)).collect();
        let mut children: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for &[from, to] in &self.edges {
            *incoming.get_mut(&to).unwrap() += 1;
            children.entry(from).or_default().push(to);
        }
        let mut ready: BTreeSet<_> = incoming
            .iter()
            .filter_map(|(&id, &count)| (count == 0).then_some(id))
            .collect();
        let mut order = Vec::new();
        while let Some(id) = ready.pop_first() {
            order.push(id);
            for &child in children.get(&id).into_iter().flatten() {
                let count = incoming.get_mut(&child).unwrap();
                *count -= 1;
                if *count == 0 {
                    ready.insert(child);
                }
            }
        }
        ensure!(
            order.len() == self.nodes.len(),
            "Connections contain a loop; remove it before running"
        );
        let mut reachable = BTreeSet::from([starts[0].id]);
        for &id in &order {
            if reachable.contains(&id) {
                reachable.extend(children.get(&id).into_iter().flatten().copied());
            }
        }
        ensure!(
            reachable == ids,
            "Connect every node to Start (or remove unused nodes)"
        );
        for node in &self.nodes {
            ensure!(
                matches!(node.step, Step::End) || self.edges.iter().any(|e| e[0] == node.id),
                "Connect node {} to a next node or End",
                node.id
            );
        }
        Ok(order)
    }
    pub fn validate(&self) -> Result<Vec<u64>> {
        let order = self.structure()?;
        for node in &self.nodes {
            if let Step::Action { target, .. } | Step::RepeatAction { target, .. } = &node.step {
                ensure!(
                    matches!(
                        target,
                        Some(
                            Target::Avatar { .. }
                                | Target::Scene(_)
                                | Target::Music(_)
                                | Target::Sound(_)
                                | Target::StopSounds
                        )
                    ),
                    "Choose an action for node {}. Graph nesting is not supported",
                    node.id
                );
            }
        }
        Ok(order)
    }
    pub fn connect(&mut self, from: u64, to: u64) -> Result<()> {
        ensure!(
            from != to
                && self
                    .nodes
                    .iter()
                    .any(|n| n.id == from && !matches!(n.step, Step::End))
                && self
                    .nodes
                    .iter()
                    .any(|n| n.id == to && !matches!(n.step, Step::Start)),
            "Connect an output to another node's input"
        );
        if self.edges.contains(&[from, to]) {
            return Ok(());
        }
        if self
            .nodes
            .iter()
            .any(|n| n.id == from && matches!(n.step, Step::Branch { .. }))
        {
            ensure!(
                self.edges.iter().filter(|e| e[0] == from).count() < 2,
                "A Branch has exactly two outputs; disconnect one before replacing it"
            );
        }
        // A new edge may not create a cycle, even while the draft is incomplete.
        let mut seen = BTreeSet::new();
        let mut queue = vec![to];
        while let Some(id) = queue.pop() {
            ensure!(id != from, "That connection would create a loop");
            if seen.insert(id) {
                queue.extend(self.edges.iter().filter(|e| e[0] == id).map(|e| e[1]));
            }
        }
        ensure!(self.edges.len() < 1024, "Connection limit reached");
        self.edges.push([from, to]);
        if let Some(Node {
            step: Step::Branch { true_next, .. },
            ..
        }) = self.nodes.iter_mut().find(|n| n.id == from)
            && true_next.is_none_or(|id| !self.edges.contains(&[from, id]))
        {
            *true_next = Some(to);
        }
        Ok(())
    }
}
pub struct Run {
    pub variables: BTreeMap<String, f64>,
    pub receipt: Option<crate::effect_api::Receipt>,
    pub graph: Graph,
    order: Vec<u64>,
    done: BTreeMap<u64, f64>,
    skipped: BTreeSet<u64>,
    branches: BTreeMap<u64, u64>,
    repeats: BTreeMap<u64, (u16, f64)>,
    pub started: f64,
    waiting: BTreeMap<u64, f64>,
    pub origins: BTreeMap<u64, Option<u64>>,
}
impl Run {
    pub fn new(graph: &Graph, now: f64) -> Result<Self> {
        Ok(Self {
            variables: BTreeMap::new(),
            receipt: None,
            graph: graph.clone(),
            order: graph.validate()?,
            done: BTreeMap::new(),
            skipped: BTreeSet::new(),
            branches: BTreeMap::new(),
            repeats: BTreeMap::new(),
            started: now,
            waiting: BTreeMap::new(),
            origins: BTreeMap::new(),
        })
    }
    pub fn ready(&self, now: f64) -> Vec<(u64, Step)> {
        self.order
            .iter()
            .filter_map(|&id| {
                if self.done.contains_key(&id) {
                    return None;
                }
                if self.repeats.get(&id).is_some_and(|(_, due)| now < *due) {
                    return None;
                }
                let incoming: Vec<_> = self.graph.edges.iter().filter(|e| e[1] == id).collect();
                if incoming.iter().any(|e| !self.done.contains_key(&e[0])) {
                    return None;
                }
                let ready_at = incoming
                    .iter()
                    .map(|e| self.done[&e[0]])
                    .fold(self.started, f64::max);
                let node = self.graph.nodes.iter().find(|n| n.id == id)?;
                let delay = if let Step::Delay { seconds } = node.step {
                    f64::from(seconds)
                } else {
                    0.
                };
                (now >= ready_at + delay).then(|| (id, node.step.clone()))
            })
            .collect()
    }
    pub fn complete(&mut self, id: u64, now: f64) {
        self.waiting.remove(&id);
        if let Some(Node {
            step: Step::RepeatAction {
                count, interval, ..
            },
            ..
        }) = self.graph.nodes.iter().find(|n| n.id == id)
        {
            let repeat = self.repeats.entry(id).or_insert((0, now));
            repeat.0 += 1;
            if repeat.0 < *count {
                repeat.1 = now + f64::from(*interval);
                return;
            }
        }
        self.done.insert(id, now);
        if self.branches.is_empty() {
            return;
        }
        // Resolve inactive paths topologically. A join waits for all predecessors
        // to resolve, then runs if any incoming path was selected.
        for &next in &self.order {
            if self.done.contains_key(&next) {
                continue;
            }
            let incoming: Vec<_> = self.graph.edges.iter().filter(|e| e[1] == next).collect();
            if !incoming.is_empty()
                && incoming.iter().all(|e| self.done.contains_key(&e[0]))
                && incoming.iter().all(|e| {
                    self.skipped.contains(&e[0])
                        || self
                            .branches
                            .get(&e[0])
                            .is_some_and(|chosen| *chosen != next)
                })
            {
                self.skipped.insert(next);
                self.done.insert(next, now);
            }
        }
    }
    pub fn choose_branch(&mut self, id: u64, inputs: &aria_core::rig::Inputs) -> Result<()> {
        let step = self
            .graph
            .nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| anyhow::anyhow!("Missing branch"))?
            .step
            .clone();
        let Step::Branch {
            true_next: Some(yes),
            ..
        } = &step
        else {
            anyhow::bail!("Unassigned branch");
        };
        let chosen = if self.compute(&step, inputs)? {
            *yes
        } else {
            self.graph
                .edges
                .iter()
                .find(|e| e[0] == id && e[1] != *yes)
                .ok_or_else(|| anyhow::anyhow!("Missing False path"))?[1]
        };
        self.branches.insert(id, chosen);
        Ok(())
    }
    pub fn compute(&mut self, step: &Step, inputs: &aria_core::rig::Inputs) -> Result<bool> {
        let (name, value) =
            match step {
                Step::Variable {
                    name,
                    operation,
                    value,
                } => {
                    let current = self.variables.get(name).copied().unwrap_or(0.);
                    let next = match operation {
                        Math::Set => *value,
                        Math::Add => current + value,
                        Math::Subtract => current - value,
                        Math::Multiply => current * value,
                        Math::Divide => {
                            ensure!(*value != 0., "Division by zero");
                            current / value
                        }
                        Math::Minimum => current.min(*value),
                        Math::Maximum => current.max(*value),
                    };
                    (name, next)
                }
                Step::Input { name, channel } => (
                    name,
                    f64::from(*inputs.get(channel).ok_or_else(|| {
                        anyhow::anyhow!("Tracking input {channel} is unavailable")
                    })?),
                ),
                Step::Require {
                    name,
                    comparison,
                    value,
                }
                | Step::Branch {
                    name,
                    comparison,
                    value,
                    ..
                } => {
                    let current = *self
                        .variables
                        .get(name)
                        .ok_or_else(|| anyhow::anyhow!("Variable {name} has not been set"))?;
                    return Ok(match comparison {
                        Compare::AtLeast => current >= *value,
                        Compare::AtMost => current <= *value,
                        Compare::Equal => (current - value).abs() <= 1e-9,
                    });
                }
                _ => return Ok(true),
            };
        ensure!(
            value.is_finite() && value.abs() <= 1e12,
            "Variable overflow"
        );
        ensure!(
            self.variables.len() < 128 || self.variables.contains_key(name),
            "At most 128 variables per action run"
        );
        self.variables.insert(name.clone(), value);
        Ok(true)
    }
    pub fn finish_without_actions(&mut self, now: f64) {
        for node in &self.graph.nodes {
            self.done.insert(node.id, now);
        }
    }
    pub fn finished(&self) -> bool {
        self.done.len() == self.graph.nodes.len()
    }
    pub fn wait_expired(&mut self, id: u64, now: f64) -> bool {
        now - *self.waiting.entry(id).or_insert(now) > 60.
    }
}

/// Shared entry point replaces the scattered modifier/key dropdowns.
pub fn hotkey_button(ui: &mut egui::Ui) {
    if ui
        .button("Hotkeys & actions…")
        .on_hover_text("Record your own global shortcut in one dedicated window.")
        .clicked()
    {
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("open-actions"), true));
    }
}

#[cfg(not(windows))]
pub fn key_pressed(ctx: &egui::Context, shortcut: Shortcut) -> bool {
    ctx.input(|i| {
        i.events.iter().any(|e| {
            if let egui::Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = e
            {
                editor::shortcut(*key, *modifiers).ok() == Some(shortcut)
            } else {
                false
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn branching() -> Graph {
        let mut graph = Graph::branch_recipe(9);
        for node in &mut graph.nodes {
            if let Step::Action { target, .. } | Step::RepeatAction { target, .. } = &mut node.step
            {
                *target = Some(Target::Music(MusicCommand::Stop));
            }
        }
        graph
    }
    #[test]
    fn conditional_paths_repeat_without_catchup_and_join_only_after_selected_work() {
        for mouth in [0.1, 0.9] {
            let graph = branching();
            let restored: Graph =
                serde_json::from_slice(&serde_json::to_vec(&graph).unwrap()).unwrap();
            let mut run = Run::new(&restored, 0.).unwrap();
            run.complete(1, 0.);
            run.compute(
                &graph.nodes[1].step,
                &BTreeMap::from([("MouthOpen".into(), mouth)]),
            )
            .unwrap();
            run.complete(2, 0.);
            run.choose_branch(3, &BTreeMap::new()).unwrap();
            run.complete(3, 0.);
            let selected = if mouth > 0.5 { 4 } else { 5 };
            assert_eq!(
                run.ready(0.).iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                [selected]
            );
            run.complete(selected, 0.);
            if selected == 4 {
                assert!(run.ready(0.9).is_empty());
                assert_eq!(run.ready(100.).len(), 1);
                run.complete(4, 100.);
                assert!(
                    run.ready(100.).is_empty(),
                    "A stalled frame must not catch up repeats"
                );
                assert_eq!(run.ready(101.)[0].0, 4);
                run.complete(4, 101.);
            }
            assert_eq!(
                run.ready(101.)
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>(),
                [6]
            );
            run.complete(6, 101.);
            assert!(run.finished());
        }
    }
    #[test]
    fn inactive_nested_branches_skip_delays_but_shared_live_paths_still_join() {
        let mut graph = branching();
        graph.nodes[3].step = Step::Branch {
            name: "unset".into(),
            comparison: Compare::Equal,
            value: 0.,
            true_next: Some(7),
        };
        graph.nodes.push(Node {
            id: 7,
            position: [0., 0.],
            step: Step::Delay { seconds: 300. },
        });
        graph.edges.retain(|e| *e != [4, 6]);
        graph.edges.extend([[4, 7], [4, 6], [7, 6]]);
        let mut run = Run::new(&graph, 0.).unwrap();
        run.complete(1, 0.);
        run.variables.insert("mouth".into(), 0.);
        run.complete(2, 0.);
        run.choose_branch(3, &BTreeMap::new()).unwrap();
        run.complete(3, 0.);
        assert!(run.skipped.contains(&4) && run.skipped.contains(&7));
        run.complete(5, 0.);
        assert_eq!(run.ready(0.)[0].0, 6);
        graph.edges.push([5, 7]); // The same node is also reachable through the live path.
        let mut run = Run::new(&graph, 0.).unwrap();
        run.complete(1, 0.);
        run.variables.insert("mouth".into(), 0.);
        run.complete(2, 0.);
        run.choose_branch(3, &BTreeMap::new()).unwrap();
        run.complete(3, 0.);
        run.complete(5, 0.);
        assert!(!run.skipped.contains(&7));
        assert!(run.ready(299.).is_empty());
        assert_eq!(run.ready(300.)[0].0, 7);
        run.complete(7, 300.);
        assert_eq!(run.ready(300.)[0].0, 6);
    }
    #[test]
    fn branches_and_repeats_reject_invalid_drafts_and_remain_run_local() {
        let graph = branching();
        let mut run = Run::new(&graph, 0.).unwrap();
        assert!(run.choose_branch(3, &BTreeMap::new()).is_err());
        for (count, interval) in [(0, 1.), (101, 1.), (3, 0.), (3, f32::NAN)] {
            let mut bad = graph.clone();
            bad.nodes[3].step = Step::RepeatAction {
                target: Some(Target::Music(MusicCommand::Stop)),
                mode: Mode::Toggle,
                count,
                interval,
            };
            assert!(bad.validate().is_err());
        }
        let mut bad = graph.clone();
        bad.edges.push([3, 6]);
        assert!(bad.validate().is_err());
        let mut bad = graph.clone();
        bad.edges.retain(|e| *e != [3, 4]);
        assert!(bad.validate().is_err());
        let mut bad = graph.clone();
        assert!(bad.connect(3, 6).is_err());
        assert!(bad.connect(5, 3).is_err());
        run.complete(4, 0.);
        assert_eq!(run.repeats[&4].0, 1);
        let other = Run::new(&graph, 0.).unwrap();
        assert!(other.repeats.is_empty() && other.branches.is_empty());
        run.finish_without_actions(0.);
        assert!(run.ready(1000.).is_empty());
    }
    #[test]
    fn variables_are_run_local_and_conditions_cancel_remaining_steps() {
        let mut graph = Graph::new(1);
        graph.nodes[1].step = Step::Variable {
            name: "score".into(),
            operation: Math::Set,
            value: 2.,
        };
        let mut run = Run::new(&graph, 0.).unwrap();
        let mut other = Run::new(&graph, 0.).unwrap();
        let inputs = BTreeMap::from([("MouthOpen".into(), 0.75)]);
        run.compute(&graph.nodes[1].step, &inputs).unwrap();
        run.compute(
            &Step::Variable {
                name: "score".into(),
                operation: Math::Multiply,
                value: 3.,
            },
            &inputs,
        )
        .unwrap();
        assert_eq!(run.variables["score"], 6.);
        assert!(other.variables.is_empty());
        run.compute(
            &Step::Input {
                name: "mouth".into(),
                channel: "MouthOpen".into(),
            },
            &inputs,
        )
        .unwrap();
        assert!(
            !run.compute(
                &Step::Require {
                    name: "mouth".into(),
                    comparison: Compare::AtLeast,
                    value: 1.
                },
                &inputs
            )
            .unwrap()
        );
        run.finish_without_actions(1.);
        assert!(run.finished());
        assert!(run.ready(10.).is_empty());
        assert!(
            other
                .compute(
                    &Step::Variable {
                        name: "score".into(),
                        operation: Math::Divide,
                        value: 0.
                    },
                    &inputs
                )
                .is_err()
        );
        graph.nodes[1].step = Step::Variable {
            name: "invalid name".into(),
            operation: Math::Set,
            value: 0.,
        };
        assert!(graph.validate().is_err());
    }
    #[test]
    fn recipes_require_binding_and_preserve_branch_timing() {
        for parallel in [false, true] {
            let mut graph = Graph::recipe(1, parallel);
            graph.structure().unwrap();
            assert!(graph.validate().is_err());
            for node in &mut graph.nodes {
                if let Step::Action { target, .. } = &mut node.step {
                    *target = Some(Target::Avatar {
                        profile: 1,
                        command: Command::Focus,
                    });
                }
            }
            graph.validate().unwrap();
            let mut run = Run::new(&graph, 0.).unwrap();
            run.complete(1, 0.);
            assert_eq!(run.ready(0.).len(), if parallel { 2 } else { 1 });
            run.complete(2, 0.);
            if !parallel {
                assert!(
                    !run.ready(1.)
                        .iter()
                        .any(|(_, s)| matches!(s, Step::Action { .. }))
                );
            }
        }
    }
    fn action() -> Graph {
        let mut g = Graph::new(7);
        g.nodes[1].step = Step::Action {
            target: Some(Target::Avatar {
                profile: 4,
                command: Command::Expression("smile".into()),
            }),
            mode: Mode::On,
        };
        g
    }
    #[test]
    fn graphs_validate_connections_and_roundtrip_without_losing_targets() {
        let g = action();
        assert!(g.validate().is_ok());
        let copy: Graph = serde_json::from_slice(&serde_json::to_vec(&g).unwrap()).unwrap();
        assert_eq!(copy.validate().unwrap(), vec![1, 2, 3]);
        let mut bad = g.clone();
        bad.edges.push([2, 1]);
        assert!(bad.validate().is_err());
        let mut bad = g.clone();
        bad.nodes.push(bad.nodes[1].clone());
        assert!(bad.validate().is_err());
        let mut bad = g.clone();
        bad.edges.remove(0);
        assert!(bad.validate().is_err());
        let mut bad = g.clone();
        bad.nodes[1].step = Step::Delay { seconds: f32::NAN };
        assert!(bad.validate().is_err());
        let mut bad = g.clone();
        bad.nodes[1].step = Step::Action {
            target: Some(Target::Graph(7)),
            mode: Mode::On,
        };
        assert!(bad.validate().is_err());
    }
    #[test]
    fn forks_run_once_and_join_waits_for_delayed_branch() {
        let mut g = action();
        g.nodes.push(Node {
            id: 4,
            position: [300., 200.],
            step: Step::Delay { seconds: 2. },
        });
        g.edges.extend([[1, 4], [4, 3]]);
        let mut run = Run::new(&g, 10.).unwrap();
        g.nodes.clear(); // Editing the draft cannot change an in-flight run.
        assert_eq!(run.ready(10.).len(), 1);
        run.complete(1, 10.);
        assert_eq!(
            run.ready(10.).iter().map(|n| n.0).collect::<Vec<_>>(),
            vec![2]
        );
        run.complete(2, 10.);
        assert!(run.ready(11.9).is_empty());
        assert_eq!(run.ready(12.)[0].0, 4);
        run.complete(4, 12.);
        assert_eq!(run.ready(12.)[0].0, 3);
        run.complete(3, 12.);
        assert!(run.finished());
        assert!(run.ready(50.).is_empty());
    }
}
