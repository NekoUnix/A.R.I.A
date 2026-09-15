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
pub enum Target {
    Avatar { profile: u64, command: Command },
    Graph(u64),
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
    Action { target: Option<Target>, mode: Mode },
    Delay { seconds: f32 },
    End,
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
            if let Step::Action { target, .. } = &node.step {
                ensure!(
                    matches!(target, Some(Target::Avatar { .. })),
                    "Choose an avatar action for node {}. Graph nesting is not supported",
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
        Ok(())
    }
}
pub struct Run {
    pub graph: Graph,
    order: Vec<u64>,
    done: BTreeMap<u64, f64>,
    pub started: f64,
    waiting: BTreeMap<u64, f64>,
    pub origins: BTreeMap<u64, Option<u64>>,
}
impl Run {
    pub fn new(graph: &Graph, now: f64) -> Result<Self> {
        Ok(Self {
            graph: graph.clone(),
            order: graph.validate()?,
            done: BTreeMap::new(),
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
        self.done.insert(id, now);
        self.waiting.remove(&id);
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
