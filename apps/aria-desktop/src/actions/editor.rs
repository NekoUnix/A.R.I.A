use super::*;
use egui::{Color32, Rect, Sense, Vec2};

#[derive(Default)]
pub struct Editor {
    pub open: bool,
    pub graph_page: bool,
    pub message: Option<String>,
    pub registration_errors: Option<String>,
    pub run: Option<u64>,
    pub stop: bool,
    selected: Option<u64>,
    node: Option<u64>,
    linking: Option<u64>,
    search: String,
    recording: Option<Target>,
    captured: Option<Shortcut>,
}
impl Editor {
    #[cfg(all(test, windows))]
    pub fn edit_graph(&mut self, id: u64) {
        self.open = true;
        self.graph_page = true;
        self.selected = Some(id);
        self.node = Some(2);
    }
    pub fn recording(&self) -> bool {
        self.open && self.recording.is_some()
    }
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        library: &mut Library,
        choices: &[Choice],
        running: &[String],
    ) -> bool {
        if ctx
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new("open-actions")))
            .unwrap_or(false)
        {
            self.open = true;
        }
        if !self.open {
            self.recording = None;
            return false;
        }
        let mut changed = false;
        let mut open = self.open;
        egui::Window::new("Hotkeys & actions")
            .id(egui::Id::new("action-studio"))
            .open(&mut open)
            .default_size([1040., 650.])
            .min_size([620., 400.])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.graph_page, false, "Keyboard shortcuts");
                    ui.selectable_value(&mut self.graph_page, true, "Action nodes");
                    crate::help::button(
                        ui,
                        if self.graph_page {
                            "action-nodes"
                        } else {
                            "hotkeys"
                        },
                    );
                });
                ui.separator();
                if let Some(message) = &self.message {
                    ui.label(message);
                }
                if let Some(error) = &self.registration_errors {
                    ui.colored_label(crate::theme::orange(), error);
                }
                if !running.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("Running: {}", running.join(", ")));
                        if ui.button("Stop all actions").clicked() {
                            self.stop = true;
                        }
                    });
                }
                if self.graph_page {
                    changed |= self.graphs(ui, library, choices);
                } else {
                    changed |= self.shortcuts(ui, library, choices);
                }
            });
        self.open = open;
        if !open {
            self.recording = None;
            self.captured = None;
        }
        changed
    }
    fn shortcuts(&mut self, ui: &mut egui::Ui, library: &mut Library, choices: &[Choice]) -> bool {
        let mut changed = ui
            .checkbox(&mut library.shortcuts_enabled, "Enable keyboard shortcuts")
            .changed();
        ui.small(if cfg!(windows) {
            "Global on Windows: shortcuts work while another app has focus. Record listens only while this window is open. Other ARIA shortcuts pause during recording."
        } else { "Shortcuts currently work while ARIA is focused on this platform. System-wide registration is available on Windows." });
        if let Some(target) = self.recording.clone() {
            ui.separator();
            ui.strong(
                choices
                    .iter()
                    .find(|c| c.target == target)
                    .map_or("Action no longer available", |c| c.label.as_str()),
            );
            if self.captured.is_none() {
                ui.label("Press your key combination now. Escape cancels. Use modifiers to avoid triggering actions while typing.");
                let events = ui.input(|i| i.events.clone());
                for event in events {
                    if let egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } = event
                    {
                        if key == egui::Key::Escape {
                            self.recording = None;
                            break;
                        }
                        match recorded_shortcut(key, modifiers) {
                            Ok(key) => {
                                self.captured = Some(key);
                                self.message = None;
                                break;
                            }
                            Err(error) => self.message = Some(error.to_string()),
                        }
                    }
                }
                // The focused recorder owns these key presses, including Tab/Enter.
                ui.input_mut(|i| {
                    i.events
                        .retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_)))
                });
            }
            if let Some(key) = self.captured {
                ui.heading(key.label());
                let conflicts: Vec<_> = choices
                    .iter()
                    .filter(|c| c.target != target && library.shortcut(c) == Some(key))
                    .map(|c| c.label.as_str())
                    .collect();
                let unloaded_conflicts = library
                    .bindings
                    .iter()
                    .filter(|b| {
                        b.target != target
                            && b.shortcut == Some(key)
                            && !choices.iter().any(|c| c.target == b.target)
                    })
                    .count();
                if unloaded_conflicts > 0 {
                    ui.colored_label(crate::theme::orange(), "This combination belongs to an unloaded avatar. Load that profile and clear its shortcut first.");
                }
                if !conflicts.is_empty() {
                    ui.colored_label(crate::theme::orange(), format!("Already assigned to {}. Clear that binding or record another combination.", conflicts.join(", ")));
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            conflicts.is_empty() && unloaded_conflicts == 0,
                            egui::Button::new("Save shortcut"),
                        )
                        .clicked()
                    {
                        library.bind(target, Some(key));
                        changed = true;
                        self.recording = None;
                        self.captured = None;
                    }
                    if ui.button("Record again").clicked() {
                        self.captured = None;
                    }
                });
            }
            if ui.button("Cancel recording").clicked() {
                self.recording = None;
                self.captured = None;
            }
            ui.separator();
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Find an avatar, expression, toggle or action…"),
        );
        let search = self.search.to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt("hotkey-list")
            .max_height(ui.available_height())
            .show(ui, |ui| {
                for choice in choices
                    .iter()
                    .filter(|c| c.label.to_lowercase().contains(&search))
                {
                    ui.push_id(&choice.target, |ui| {
                        crate::theme::glass_card().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(&choice.label);
                            ui.horizontal_wrapped(|ui| {
                                ui.monospace(library.shortcut(choice).map_or_else(
                                    || choice.legacy_label.clone().unwrap_or("Unassigned".into()),
                                    |k| k.label(),
                                ));
                                if ui.button("Record…").clicked() {
                                    self.recording = Some(choice.target.clone());
                                    self.captured = None;
                                    self.message = None;
                                }
                                if ui
                                    .add_enabled(
                                        library.shortcut(choice).is_some()
                                            || choice.legacy_label.is_some(),
                                        egui::Button::new("Clear"),
                                    )
                                    .clicked()
                                {
                                    library.bind(choice.target.clone(), None);
                                    changed = true;
                                }
                            });
                        });
                    });
                }
            });
        changed
    }
    fn graphs(&mut self, ui: &mut egui::Ui, library: &mut Library, choices: &[Choice]) -> bool {
        let mut changed = false;
        ui.small("Connect output dots to input dots. Drag cards to arrange them. Branches run together; a joined node waits for every incoming branch. Saved actions can be run with a button or recorded hotkey.");
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("action-graph").selected_text(library.graphs.iter().find(|g| Some(g.id) == self.selected).map_or("Choose an action", |g| &g.name)).show_ui(ui, |ui| {
                for graph in &library.graphs {
                    if ui.selectable_value(&mut self.selected, Some(graph.id), &graph.name).changed() { self.node = None; self.linking = None; }
                }
            });
            if ui.add_enabled(library.graphs.len() < 128, egui::Button::new("New action")).clicked() {
                let id = library.next_id(); library.graphs.push(Graph::new(id)); self.selected = Some(id); self.node = Some(2); self.linking = None; changed = true;
            }
            if ui.button("Import…").clicked() && let Some(path) = rfd::FileDialog::new().add_filter("ARIA action graph", &["json"]).pick_file() {
                let result = (|| -> Result<Graph> {
                    ensure!(std::fs::metadata(&path)?.len() <= 1_048_576, "Action files must be at most 1 MiB");
                    ensure!(library.graphs.len() < 128, "Action library is full");
                    let mut graph: Graph = serde_json::from_slice(&aria_model::read_bounded(&path, 1_048_576)?)?;
                    graph.structure()?;
                    for node in &mut graph.nodes { if let Step::Action { target, .. } = &mut node.step { *target = None; } }
                    graph.id = library.next_id(); Ok(graph)
                })();
                match result {
                    Ok(graph) => { self.selected = Some(graph.id); self.node = None; self.linking = None; library.graphs.push(graph); changed = true; self.message = Some("Imported layout and steps. Choose this workspace's avatar action for each Action node before running.".into()); }
                    Err(e) => self.message = Some(format!("Import failed: {e}")),
                }
            }
        });
        let Some(index) = library
            .graphs
            .iter()
            .position(|g| Some(g.id) == self.selected)
        else {
            ui.label("Create an action to connect expressions, layers, props, images, effects, movement presets and avatar changes.");
            return changed;
        };
        let graph = &mut library.graphs[index];
        let mut delete = false;
        ui.horizontal_wrapped(|ui| {
            changed |= ui.add(egui::TextEdit::singleline(&mut graph.name).char_limit(80).desired_width(220.)).changed();
            if ui.button("Run action").clicked() { self.run = Some(graph.id); }
            if ui.button("Export…").clicked() && let Some(path) = rfd::FileDialog::new().add_filter("ARIA action", &["json"]).set_file_name("my-action.aria-action.json").save_file() {
                self.message = Some(match serde_json::to_vec_pretty(graph).map_err(anyhow::Error::from).and_then(|b| std::fs::write(path,b).map_err(Into::into)) {
                    Ok(()) => "Action exported. This template contains actions and profile IDs, never avatar artwork.".into(), Err(e) => format!("Export failed: {e}"),
                });
            }
            delete = ui.button("Delete action").clicked();
        });
        ui.horizontal_wrapped(|ui| {
            for (name, step) in [
                (
                    "+ Action",
                    Step::Action {
                        target: None,
                        mode: Mode::Toggle,
                    },
                ),
                ("+ Delay", Step::Delay { seconds: 1. }),
                ("+ End", Step::End),
            ] {
                if ui
                    .add_enabled(graph.nodes.len() < 256, egui::Button::new(name))
                    .clicked()
                {
                    let id = graph
                        .nodes
                        .iter()
                        .map(|n| n.id)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1);
                    graph.nodes.push(Node {
                        id,
                        position: [
                            30. + (id % 5) as f32 * 240.,
                            170. + ((id / 5) % 10) as f32 * 130.,
                        ],
                        step,
                    });
                    self.node = Some(id);
                    changed = true;
                }
            }
            if let Some(from) = self.linking {
                ui.label(format!("Connecting node {from}: click an input dot"));
                if ui.small_button("Cancel").clicked() {
                    self.linking = None;
                }
            }
        });
        if let Err(error) = graph.validate() {
            ui.colored_label(crate::theme::orange(), error.to_string());
        }
        if let Some(node) = graph.nodes.iter_mut().find(|n| Some(n.id) == self.node) {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("Node {}", node.id));
                match &mut node.step {
                    Step::Action { target, mode } => {
                        let before = target.clone();
                        egui::ComboBox::from_id_salt("node-target")
                            .width(350.)
                            .selected_text(
                                target
                                    .as_ref()
                                    .and_then(|t| choices.iter().find(|c| &c.target == t))
                                    .map_or("Choose / repair avatar action…", |c| {
                                        c.label.as_str()
                                    }),
                            )
                            .show_ui(ui, |ui| {
                                for choice in choices
                                    .iter()
                                    .filter(|c| matches!(c.target, Target::Avatar { .. }))
                                {
                                    ui.selectable_value(
                                        target,
                                        Some(choice.target.clone()),
                                        &choice.label,
                                    );
                                }
                            });
                        changed |= *target != before;
                        if matches!(
                            target,
                            Some(Target::Avatar {
                                command: Command::Pose
                                    | Command::Expression(_)
                                    | Command::Layers(_)
                                    | Command::Item(_)
                                    | Command::Image(_),
                                ..
                            })
                        ) {
                            egui::ComboBox::from_id_salt("node-mode")
                                .selected_text(format!("{mode:?}"))
                                .show_ui(ui, |ui| {
                                    for value in [Mode::Toggle, Mode::On, Mode::Off] {
                                        changed |= ui
                                            .selectable_value(mode, value, format!("{value:?}"))
                                            .changed();
                                    }
                                });
                        }
                    }
                    Step::Delay { seconds } => {
                        changed |= ui
                            .add(
                                egui::DragValue::new(seconds)
                                    .speed(0.1)
                                    .range(0. ..=300.)
                                    .suffix(" seconds"),
                            )
                            .changed();
                    }
                    Step::Start => {
                        ui.label("Run button or assigned hotkey starts here.");
                    }
                    Step::End => {
                        ui.label("This branch finishes here.");
                    }
                }
            });
        }
        if let Some(id) = self.node {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        graph
                            .nodes
                            .iter()
                            .any(|n| n.id == id && !matches!(n.step, Step::Start)),
                        egui::Button::new("Delete node"),
                    )
                    .clicked()
                {
                    graph.nodes.retain(|n| n.id != id);
                    graph.edges.retain(|e| !e.contains(&id));
                    self.node = None;
                    self.linking = None;
                    changed = true;
                }
                let edges = graph.edges.clone();
                for edge in edges.iter().filter(|e| e.contains(&id)) {
                    if ui
                        .small_button(format!("Disconnect {} → {}", edge[0], edge[1]))
                        .clicked()
                    {
                        graph.edges.retain(|e| e != edge);
                        changed = true;
                    }
                }
            });
        }
        egui::ScrollArea::both()
            .id_salt(("node-canvas", graph.id))
            .auto_shrink([false, false])
            .max_height(ui.available_height().max(180.))
            .show(ui, |ui| {
                let (canvas, _) = ui.allocate_exact_size(Vec2::new(4300., 4300.), Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(canvas, 8., crate::theme::bg());
                let rects: BTreeMap<_, _> = graph
                    .nodes
                    .iter()
                    .map(|n| {
                        (
                            n.id,
                            Rect::from_min_size(
                                canvas.min + Vec2::from(n.position),
                                Vec2::new(210., 92.),
                            ),
                        )
                    })
                    .collect();
                for &[from, to] in &graph.edges {
                    if let (Some(a), Some(b)) = (rects.get(&from), rects.get(&to)) {
                        let start = a.right_center();
                        let end = b.left_center();
                        painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                            [
                                start,
                                start + Vec2::new(75., 0.),
                                end - Vec2::new(75., 0.),
                                end,
                            ],
                            false,
                            Color32::TRANSPARENT,
                            egui::Stroke::new(2., crate::theme::mint()),
                        ));
                    }
                }
                let mut connect = None;
                for node in &mut graph.nodes {
                    let rect = rects[&node.id];
                    let color = match node.step {
                        Step::Start | Step::End => crate::theme::mint(),
                        Step::Delay { .. } => crate::theme::orange(),
                        _ => Color32::from_rgb(145, 120, 240),
                    };
                    painter.rect_filled(rect, 10., crate::theme::card_color());
                    painter.rect_stroke(
                        rect,
                        10.,
                        egui::Stroke::new(if self.node == Some(node.id) { 2.5 } else { 1. }, color),
                        egui::StrokeKind::Inside,
                    );
                    let title = match &node.step {
                        Step::Start => "Start".into(),
                        Step::End => "End".into(),
                        Step::Delay { seconds } => format!("Wait {seconds:.1}s"),
                        Step::Action { .. } => format!("Action {}", node.id),
                    };
                    painter.text(
                        rect.min + Vec2::new(14., 16.),
                        egui::Align2::LEFT_TOP,
                        title,
                        egui::FontId::proportional(14.),
                        crate::theme::text_color(),
                    );
                    if let Step::Action { target, .. } = &node.step {
                        let label = target
                            .as_ref()
                            .and_then(|t| choices.iter().find(|c| &c.target == t))
                            .map_or("Choose an avatar action…", |c| c.label.as_str());
                        let text = painter.layout(
                            label.into(),
                            egui::FontId::proportional(11.),
                            crate::theme::muted(),
                            182.,
                        );
                        painter.with_clip_rect(rect.shrink(10.)).galley(
                            rect.min + Vec2::new(14., 40.),
                            text,
                            crate::theme::muted(),
                        );
                    }
                    let response = ui.interact(
                        rect.shrink2(Vec2::new(12., 0.)),
                        ui.id().with(("node", node.id)),
                        Sense::click_and_drag(),
                    );
                    if response.clicked() {
                        self.node = Some(node.id);
                    }
                    if response.dragged() && ui.ctx().current_pass_index() == 0 {
                        let delta = ui.input(|i| i.pointer.delta());
                        node.position[0] = (node.position[0] + delta.x).clamp(0., 4000.);
                        node.position[1] = (node.position[1] + delta.y).clamp(0., 4000.);
                        changed |= delta != Vec2::ZERO;
                    }
                    for (input, pos) in [(true, rect.left_center()), (false, rect.right_center())] {
                        if (input && matches!(node.step, Step::Start))
                            || (!input && matches!(node.step, Step::End))
                        {
                            continue;
                        }
                        painter.circle_filled(pos, 7., color);
                        if ui
                            .interact(
                                Rect::from_center_size(pos, Vec2::splat(22.)),
                                ui.id().with((node.id, input)),
                                Sense::click(),
                            )
                            .on_hover_text(if input {
                                "Input · click to finish connection"
                            } else {
                                "Output · click to begin connection"
                            })
                            .clicked()
                        {
                            if input {
                                if let Some(from) = self.linking.take() {
                                    connect = Some((from, node.id));
                                }
                            } else {
                                self.linking = Some(node.id);
                            }
                        }
                    }
                }
                if let Some((from, to)) = connect {
                    match graph.connect(from, to) {
                        Ok(()) => {
                            changed = true;
                            self.message = None;
                        }
                        Err(e) => self.message = Some(e.to_string()),
                    }
                }
            });
        if delete {
            let id = graph.id;
            library.graphs.remove(index);
            library.bindings.retain(|b| b.target != Target::Graph(id));
            self.selected = None;
            self.node = None;
            changed = true;
        }
        changed
    }
}
pub(super) fn shortcut(key: egui::Key, m: egui::Modifiers) -> Result<Shortcut> {
    let name = key.name();
    let vk = match key {
        egui::Key::ArrowLeft => 0x25, egui::Key::ArrowUp => 0x26, egui::Key::ArrowRight => 0x27, egui::Key::ArrowDown => 0x28,
        egui::Key::PageUp => 0x21, egui::Key::PageDown => 0x22,
        _ => Shortcut::keys().iter().find(|(_,n)| n == name).map(|(k,_)| *k).ok_or_else(|| anyhow::anyhow!("{name} is not supported. Choose a letter, number, navigation key or F1–F11/F13–F24."))?,
    };
    let shortcut = Shortcut {
        ctrl: m.ctrl,
        alt: m.alt,
        shift: m.shift,
        win: m.mac_cmd,
        key: vk,
    };
    shortcut.validate()?;
    Ok(shortcut)
}
fn recorded_shortcut(key: egui::Key, modifiers: egui::Modifiers) -> Result<Shortcut> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        // egui omits the Windows modifier and collapses numpad keys. Query only
        // while the user explicitly records in the focused shortcut window.
        let down = |vk| unsafe { GetAsyncKeyState(vk) < 0 };
        let numpad = (0x60..=0x6f).find(|&vk| down(vk));
        let mut result = if let Some(vk) = numpad {
            Shortcut {
                ctrl: modifiers.ctrl,
                alt: modifiers.alt,
                shift: modifiers.shift,
                win: false,
                key: vk as u16,
            }
        } else {
            shortcut(key, modifiers)?
        };
        result.win = down(0x5b) || down(0x5c);
        result.validate()?;
        Ok(result)
    }
    #[cfg(not(windows))]
    {
        shortcut(key, modifiers)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorder_ui_captures_saves_and_clears_a_shortcut() {
        let ctx = egui::Context::default();
        let target = Target::Graph(42);
        let choices = vec![Choice {
            target: target.clone(),
            label: "My screenshot action".into(),
            shortcut: None,
            legacy_label: None,
        }];
        let mut editor = Editor {
            open: true,
            ..Default::default()
        };
        let mut library = Library::default();
        let mut time = 0.;
        let mut draw = |events, editor: &mut Editor, library: &mut Library| {
            time += 0.1;
            crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    time: Some(time),
                    events,
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(1200., 850.),
                    )),
                    ..Default::default()
                },
                |_| {
                    editor.ui(&ctx, library, &choices, &[]);
                },
            )
        };
        fn text_pos(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
            fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
                match shape {
                    egui::Shape::Text(t) if t.galley.text() == label => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    egui::Shape::Vec(v) => v.iter().find_map(|s| find(s, label)),
                    _ => None,
                }
            }
            output
                .shapes
                .iter()
                .find_map(|s| find(&s.shape, label))
                .unwrap_or_else(|| panic!("Missing button {label}"))
        }
        let click = |pos| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        draw(vec![], &mut editor, &mut library);
        let output = draw(vec![], &mut editor, &mut library);
        draw(
            click(text_pos(&output, "Record…")),
            &mut editor,
            &mut library,
        );
        assert!(editor.recording());
        let output = draw(
            vec![egui::Event::Key {
                key: egui::Key::H,
                physical_key: Some(egui::Key::H),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Default::default()
                },
            }],
            &mut editor,
            &mut library,
        );
        assert_eq!(editor.captured.unwrap().label(), "Ctrl+Shift+H");
        let _ = output;
        let output = draw(vec![], &mut editor, &mut library);
        draw(
            click(text_pos(&output, "Save shortcut")),
            &mut editor,
            &mut library,
        );
        assert!(!editor.recording());
        assert_eq!(
            library.shortcut(&choices[0]).unwrap().label(),
            "Ctrl+Shift+H"
        );
        let output = draw(vec![], &mut editor, &mut library);
        draw(click(text_pos(&output, "Clear")), &mut editor, &mut library);
        assert!(library.shortcut(&choices[0]).is_none());
    }
    #[test]
    fn recording_preserves_modifiers_and_rejects_reserved_keys() {
        let m = egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        assert_eq!(shortcut(egui::Key::H, m).unwrap().label(), "Ctrl+Shift+H");
        assert_eq!(shortcut(egui::Key::ArrowDown, m).unwrap().key, 0x28);
        assert!(shortcut(egui::Key::F12, m).is_err());
    }
}
