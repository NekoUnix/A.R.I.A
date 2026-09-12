//! Offline contextual documentation. Only an open help window lays out articles;
//! expensive avatar-specific descriptions are built when their button is clicked.
use crate::theme;
use eframe::egui::{self, Align2, Color32, FontId, RichText, Stroke, Vec2};
use std::sync::{Arc, Mutex, OnceLock};

pub struct Article {
    pub id: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub body: &'static str,
    search: String,
}

pub fn articles() -> &'static [Article] {
    static ARTICLES: OnceLock<Vec<Article>> = OnceLock::new();
    ARTICLES.get_or_init(|| {
        include_str!("../../../docs/in-app-help.md")
            .split("\n## ")
            .skip(1)
            .map(|section| {
                let (header, body) = section.split_once('\n').expect("help article body");
                let mut fields = header.split(" | ");
                let id = fields.next().unwrap();
                let title = fields.next().expect("help title");
                let summary = fields.next().expect("help summary");
                Article {
                    id,
                    title,
                    summary,
                    body: body.trim(),
                    search: format!("{title} {summary} {body}").to_lowercase(),
                }
            })
            .collect()
    })
}

fn article(id: &str) -> &'static Article {
    articles()
        .iter()
        .find(|a| a.id == id)
        .expect("known help topic")
}

#[derive(Clone, Default)]
struct Selection {
    topic: &'static str,
    context: Arc<str>,
}
struct State {
    open: bool,
    selection: Selection,
    history: Vec<Selection>,
    search: String,
    revision: u64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            selection: Selection {
                topic: "welcome",
                ..Default::default()
            },
            history: Vec::new(),
            search: String::new(),
            revision: 0,
        }
    }
}
impl State {
    fn select(&mut self, topic: &'static str, context: Arc<str>) {
        if self.selection.topic != topic || self.selection.context != context {
            if self.history.len() >= 32 {
                self.history.remove(0);
            }
            self.history.push(self.selection.clone());
            self.selection = Selection { topic, context };
            self.revision = self.revision.wrapping_add(1);
        }
        self.open = true;
    }
}
fn state(ctx: &egui::Context) -> Arc<Mutex<State>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Arc<Mutex<State>>>(egui::Id::new("aria-help-state"))
            .clone()
    })
}
pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("aria-documentation")
}

pub fn open(ctx: &egui::Context, topic: &'static str, context: String) {
    article(topic);
    let shared = state(ctx);
    let mut state = shared.lock().unwrap();
    state.select(topic, context.into());
    state.search.clear();
    drop(state);
    ctx.send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::Focus);
    ctx.request_repaint_of(egui::ViewportId::ROOT);
    ctx.request_repaint_of(viewport_id());
}

pub fn button(ui: &mut egui::Ui, topic: &'static str) -> egui::Response {
    context_button(ui, topic, String::new)
}

pub fn context_button(
    ui: &mut egui::Ui,
    topic: &'static str,
    context: impl FnOnce() -> String,
) -> egui::Response {
    let a = article(topic);
    // A painted circle and ASCII question mark work even without symbol fonts.
    // Button sense includes keyboard activation and an accessible button role.
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(11.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("Help: {}", a.title),
        )
    });
    let color = if response.hovered() || response.has_focus() {
        theme::MINT
    } else {
        theme::MUTED
    };
    ui.painter().circle_filled(rect.center(), 4.25, theme::CARD);
    ui.painter()
        .circle_stroke(rect.center(), 4.25, Stroke::new(0.625_f32, color));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "?",
        FontId::proportional(6.5),
        color,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            3.0,
            Stroke::new(1.0_f32, theme::MINT),
            egui::StrokeKind::Inside,
        );
    }
    if response.clicked() {
        open(ui.ctx(), topic, context());
    }
    response.on_hover_ui(|ui| {
        ui.set_max_width(340.0);
        ui.strong(a.title);
        ui.label(a.summary);
        theme::caption(ui, "Click for the full guide, examples and diagrams. Keyboard: Tab, then Enter or Space.");
    }).on_hover_cursor(egui::CursorIcon::Help)
}

pub fn label(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>, topic: &'static str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        button(ui, topic);
    });
}

/// Preserve the control's return value, including changed/clicked/dragged flags.
pub fn control<R>(
    ui: &mut egui::Ui,
    topic: &'static str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        button(ui, topic);
        add(ui)
    })
    .inner
}

pub fn category_topic(title: &str) -> &'static str {
    match title {
        "Tracking & connection" => "tracking",
        "Movement & calibration" => "calibration",
        "Avatar & appearance" => "avatar",
        "Capture & performance" => "outputs",
        "Overall physics" => "physics",
        "Pose & capture" => "pose",
        "Create a preset" | "Saved presets" => "presets",
        "Keyboard shortcuts" | "Global shortcuts" => "hotkeys",
        "Expression library" | "Selected expression" => "expressions",
        "Files needing attention" => "expression-files",
        "Connection diagnostics & export" => "diagnostics",
        "Stage objects" => "png-items",
        "Live2D object" | "Object texture setup" | "Object parameters" => "model-items",
        "Placement & appearance" => "png-placement",
        "Pin to avatar" => "png-pins",
        "Input toggle" => "png-toggles",
        "Keyboard toggle" => "hotkeys",
        _ => "welcome",
    }
}

pub fn parameter_context(p: &aria_core::rig::RigParameter, label: &str) -> String {
    format!(
        "Avatar control: {label}\nParameter ID: {}\nAuthored minimum: {}\nAuthored maximum: {}\nAuthored default: {}\nValue when opened: {}\n\nThese limits come from the loaded avatar. They are parameter units, not necessarily degrees or pixels. The artwork determines what this control changes. This is a snapshot; reopen its question mark after changing avatars or values.",
        p.id, p.min, p.max, p.default, p.value
    )
}

pub fn parameter_header(
    ui: &mut egui::Ui,
    p: &aria_core::rig::RigParameter,
    label: &str,
    default_open: bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id("input-control"),
        default_open,
    );
    let header = ui.horizontal_wrapped(|ui| {
        let (rect, toggle) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::click());
        let clicked = toggle.clicked();
        egui::collapsing_header::paint_default_icon(
            ui,
            state.openness(ui.ctx()),
            &toggle.with_new_rect(rect.shrink(3.0)),
        );
        if clicked {
            state.toggle(ui);
        }
        if ui
            .add(egui::Label::new(format!("{label}   {:.3}", p.value)).sense(egui::Sense::click()))
            .clicked()
        {
            state.toggle(ui);
        }
        context_button(ui, "parameter", || parameter_context(p, label));
    });
    state.show_body_indented(&header.response, ui, body);
}

pub fn show(ctx: &egui::Context, _started: std::time::Instant) {
    let shared = state(ctx);
    if !shared.lock().unwrap().open {
        return;
    }
    ctx.show_viewport_deferred(
        viewport_id(),
        egui::ViewportBuilder::default()
            .with_title("A.R.I.A. — Help & documentation")
            .with_inner_size([880.0, 690.0])
            .with_min_inner_size([700.0, 430.0])
            .with_resizable(true),
        move |ctx, class| {
            let mut state = shared.lock().unwrap();
            if ctx.input(|i| i.viewport().close_requested()) {
                state.open = false;
                ctx.request_repaint_of(egui::ViewportId::ROOT);
                return;
            }
            if class == egui::ViewportClass::Embedded {
                let mut open = state.open;
                egui::Window::new("ARIA help & documentation")
                    .open(&mut open)
                    .resizable(true)
                    .default_size([850.0, 620.0])
                    .show(ctx, |ui| content(ui, &mut state));
                state.open = open;
            } else {
                egui::CentralPanel::default().show(ctx, |ui| content(ui, &mut state));
            }
            #[cfg(feature = "screenshots")]
            if crate::smoke_mode() {
                crate::screenshot::capture(ctx, _started, false);
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        },
    );
}

fn content(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal(|ui| {
        ui.heading("Help & documentation");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(!state.history.is_empty(), egui::Button::new("Back"))
                .clicked()
            {
                state.selection = state.history.pop().unwrap();
                state.revision = state.revision.wrapping_add(1);
            }
            if ui.button("Start here").clicked() {
                state.select("welcome", Arc::from(""));
            }
        });
    });
    theme::caption(
        ui,
        "Offline guide · Opening help leaves tracking running. Context values are snapshots from the control you clicked.",
    );
    ui.separator();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(210.0, ui.available_height()), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(210.0);
            ui.add(egui::TextEdit::singleline(&mut state.search).hint_text("Search all help…").desired_width(210.0));
            if !state.search.is_empty() && ui.small_button("Clear search").clicked() { state.search.clear(); }
            let query = state.search.trim().to_lowercase();
            egui::ScrollArea::vertical().id_salt("help-topics").show(ui, |ui| {
                let mut count = 0;
                for a in articles().iter().filter(|a| query.split_whitespace().all(|term| a.search.contains(term))) {
                    count += 1;
                    if ui.add(egui::Button::new(a.title).wrap().selected(state.selection.topic == a.id)).clicked() {
                        state.select(a.id, Arc::from(""));
                    }
                }
                if count == 0 { ui.label("No matching topics. Try a control name, such as smoothing, hold or VRAM."); }
            });
        });
        ui.separator();
        let a = article(state.selection.topic);
        ui.vertical(|ui| {
        egui::ScrollArea::vertical().id_salt(("help-article", state.revision)).auto_shrink([false, false]).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.heading(a.title);
            ui.add_space(6.0);
            ui.label(RichText::new(a.summary).color(theme::MINT));
            if !state.selection.context.is_empty() {
                ui.add_space(8.0);
                theme::card(ui, |ui| {
                    ui.strong("About the control you opened");
                    for paragraph in state.selection.context.split('\n') {
                        ui.add(egui::Label::new(paragraph).wrap());
                    }
                });
            }
            for block in a.body.split("\n\n") {
                ui.add_space(8.0);
                if let Some(title) = block.strip_prefix("### ") {
                    ui.label(RichText::new(title).strong().size(16.0));
                } else if let Some(kind) = block.strip_prefix("@diagram ") {
                    diagram(ui, kind.trim());
                } else {
                    ui.add(egui::Label::new(block).wrap());
                }
            }
            ui.add_space(20.0);
        });
        });
    });
}

fn diagram(ui: &mut egui::Ui, kind: &str) {
    if kind == "capture" {
        theme::card(ui, |ui| {
            ui.strong("One canvas, two destinations");
            let (r, _) = ui
                .allocate_exact_size(Vec2::new(ui.available_width(), 170.0), egui::Sense::hover());
            let center = r.center().x;
            let top = egui::Rect::from_center_size(
                egui::pos2(center, r.top() + 25.0),
                Vec2::new(r.width() - 16.0, 42.0),
            );
            let width = (r.width() - 24.0) * 0.5;
            let left = egui::Rect::from_min_size(
                egui::pos2(r.left(), r.top() + 104.0),
                Vec2::new(width, 56.0),
            );
            let right = egui::Rect::from_min_size(
                egui::pos2(r.right() - width, r.top() + 104.0),
                Vec2::new(width, 56.0),
            );
            for (rect, text) in [
                (top, "Avatar → full-resolution canvas"),
                (left, "Spout2 → OBS\nFull canvas pixels"),
                (right, "Desktop preview\nScaled to window"),
            ] {
                ui.painter()
                    .rect_filled(rect, 6.0, Color32::from_rgb(36, 58, 66));
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(12.0),
                    theme::TEXT,
                );
            }
            for destination in [left, right] {
                ui.painter().arrow(
                    top.center_bottom(),
                    destination.center_top() - top.center_bottom(),
                    Stroke::new(1.5_f32, theme::MINT),
                );
            }
            theme::caption(
                ui,
                "The preview and OBS read the same canvas. Changing preview size leaves the full-resolution OBS texture unchanged.",
            );
        });
        return;
    }
    let (labels, note): (&[&str], &str) = match kind {
        "pin" => (
            &[
                "Choose a point on the avatar",
                "Remember triangle + weights",
                "Tracking / physics move vertices",
                "Object follows anchor + your offset",
            ],
            "The accessory is a rigid image attached to the moving surface. Rotation, stretch and visibility following are optional.",
        ),
        "item-toggle" => (
            &[
                "Tracking input / final parameter",
                "Inclusive range + hysteresis",
                "Gate visibility / flip on entry",
                "Master toggle → object shown or hidden",
            ],
            "Frozen poses pause input rules. Manual checkboxes and keyboard toggles remain available for screenshots.",
        ),
        "pipeline" => (
            &[
                "Tracker / demo",
                "Calibrate + map inputs",
                "Expressions + holds",
                "Physics + final holds",
                "Avatar + OBS",
            ],
            "Frozen pose uses the captured final values and pauses this animation path.",
        ),
        "physics" => (
            &[
                "Authored input parameters",
                "Particle chain: inertia + forces",
                "Group output parameters",
                "Hair / ears / clothing movement",
            ],
            "Overall settings combine with each group. Only connections authored in the avatar can move its artwork.",
        ),
        "network" => (
            &[
                "PC → phone request port",
                "VTube Studio on iPhone",
                "Phone → PC receive port",
                "ARIA validates sender + packet",
                "Tracking inputs",
            ],
            "Both devices must reach each other on the local network. The request and receive ports have different jobs.",
        ),
        "mapping" => {
            mapping_diagram(ui);
            return;
        }
        _ => return,
    };
    theme::card(ui, |ui| {
        let width = ui.available_width();
        for (i, label) in labels.iter().enumerate() {
            let (r, _) = ui.allocate_exact_size(Vec2::new(width, 32.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(r, 6.0, Color32::from_rgb(36, 58, 66));
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                theme::TEXT,
            );
            if i + 1 < labels.len() {
                let (r, _) = ui.allocate_exact_size(Vec2::new(width, 15.0), egui::Sense::hover());
                ui.painter().arrow(
                    r.center_top(),
                    Vec2::new(0.0, 12.0),
                    Stroke::new(1.5_f32, theme::MINT),
                );
            }
        }
        theme::caption(ui, note);
    });
}

fn mapping_diagram(ui: &mut egui::Ui) {
    theme::card(ui, |ui| {
        ui.strong("Example: input −10…10 → output −30…30");
        let (r, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 170.0), egui::Sense::hover());
        let plot = r.shrink2(Vec2::new(34.0, 25.0));
        let pos = |x: f32, y: f32| {
            egui::pos2(
                egui::lerp(plot.x_range(), x),
                egui::lerp(plot.y_range(), 1.0 - y),
            )
        };
        let painter = ui.painter();
        painter.line_segment(
            [pos(0.0, 0.5), pos(1.0, 0.5)],
            Stroke::new(1.0_f32, theme::MUTED),
        );
        painter.line_segment(
            [pos(0.5, 0.0), pos(0.5, 1.0)],
            Stroke::new(1.0_f32, theme::MUTED),
        );
        painter.line_segment(
            [pos(0.0, 0.0), pos(1.0, 1.0)],
            Stroke::new(1.0_f32, theme::MUTED),
        );
        let points = (0..=100)
            .map(|n| {
                let x = n as f32 / 100.0;
                let c = x * 2.0 - 1.0;
                let y = (((c.abs() - 0.2).max(0.0) / 0.8).powi(2) * c.signum() + 1.0) * 0.5;
                pos(x, y)
            })
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(2.0_f32, theme::MINT)));
        for (p, align, text) in [
            (plot.left_bottom(), Align2::LEFT_TOP, "−10"),
            (plot.right_bottom(), Align2::RIGHT_TOP, "+10 input"),
            (plot.left_top(), Align2::RIGHT_TOP, "+30"),
            (plot.left_bottom(), Align2::RIGHT_BOTTOM, "−30"),
        ] {
            painter.text(p, align, text, FontId::proportional(11.0), theme::MUTED);
        }
        theme::caption(
            ui,
            "Gray: linear. Mint: dead zone 0.10 and curve 2.0. The center stays still; both endpoints remain reachable. Illustration only; this does not change your settings.",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
        add: impl FnMut(&mut egui::Ui),
    ) -> egui::FullOutput {
        let mut add = add;
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(900.0, 700.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| add(ui));
            },
        )
    }

    #[test]
    fn question_mark_hover_click_and_keyboard_open_context_without_changing_a_control() {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| {
            style.interaction.tooltip_delay = 0.0;
            style.interaction.tooltip_grace_time = 0.0;
        });
        let mut rect = egui::Rect::NOTHING;
        let mut id = egui::Id::NULL;
        let mut builds = 0;
        let mut draw = |ui: &mut egui::Ui| {
            let response = context_button(ui, "ranges", || {
                builds += 1;
                "Custom input: -4 to 8".into()
            });
            rect = response.rect;
            id = response.id;
        };
        let _ = frame(&ctx, 0.0, vec![], &mut draw);
        let position = rect.center();
        let _ = frame(&ctx, 0.1, vec![egui::Event::PointerMoved(position)], |ui| {
            button(ui, "ranges");
        });
        let hover = frame(&ctx, 1.0, vec![], |ui| {
            button(ui, "ranges");
        });
        fn text_contains(shape: &egui::Shape, needle: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.text().contains(needle),
                egui::Shape::Vec(shapes) => shapes.iter().any(|s| text_contains(s, needle)),
                _ => false,
            }
        }
        assert!(
            hover
                .shapes
                .iter()
                .any(|s| text_contains(&s.shape, "Input endpoints"))
        );
        assert!(!state(&ctx).lock().unwrap().open);
        assert_eq!(builds, 0, "context must not be rebuilt during idle frames");
        for (time, pressed) in [(1.1, true), (1.2, false)] {
            let _ = frame(
                &ctx,
                time,
                vec![egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
                |ui| {
                    context_button(ui, "ranges", || {
                        builds += 1;
                        "Custom input: -4 to 8".into()
                    });
                },
            );
        }
        assert_eq!(builds, 1);
        assert_eq!(
            &*state(&ctx).lock().unwrap().selection.context,
            "Custom input: -4 to 8"
        );
        state(&ctx).lock().unwrap().open = false;
        ctx.memory_mut(|memory| memory.request_focus(id));
        let _ = frame(
            &ctx,
            1.3,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            |ui| {
                button(ui, "ranges");
            },
        );
        assert!(state(&ctx).lock().unwrap().open);
    }
    #[test]
    fn catalog_is_complete_and_control_links_resolve() {
        assert!(articles().len() >= 35);
        let mut ids = std::collections::HashSet::new();
        for a in articles() {
            assert!(ids.insert(a.id), "duplicate {}", a.id);
            assert!(
                a.summary.len() > 30 && a.body.len() > 250,
                "thin article {}",
                a.id
            );
        }
        for source in [
            include_str!("app.rs"),
            include_str!("input_monitor.rs"),
            include_str!("physics_panel.rs"),
            include_str!("expressions_panel.rs"),
            include_str!("items_panel.rs"),
            include_str!("output.rs"),
            include_str!("metrics.rs"),
        ] {
            for line in source.lines().filter(|line| line.contains("help::")) {
                for (i, part) in line.split("|ui|").next().unwrap().split('"').enumerate() {
                    if i % 2 == 1
                        && part.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                        && !part.is_empty()
                    {
                        article(part);
                    }
                }
            }
        }
    }
    #[test]
    fn context_and_history_stay_independent_from_avatar_settings() {
        let ctx = egui::Context::default();
        open(&ctx, "ranges", "Avatar A: -4 to 8".into());
        open(&ctx, "physics", "Avatar B: custom tail".into());
        let shared = state(&ctx);
        let mut state = shared.lock().unwrap();
        assert_eq!(&*state.history.last().unwrap().context, "Avatar A: -4 to 8");
        state.open = false;
        assert_eq!(state.selection.topic, "physics");
        assert_ne!(viewport_id(), egui::ViewportId::ROOT);
        assert!((0..3).all(|i| viewport_id() != crate::output::viewport_id(i)));
    }

    #[test]
    fn native_help_close_leaves_root_and_outputs_running() {
        let ctx = egui::Context::default();
        ctx.set_embed_viewports(false);
        open(&ctx, "physics", String::new());
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            show(ctx, std::time::Instant::now())
        });
        let help = out
            .viewport_output
            .get(&viewport_id())
            .expect("separate help viewport");
        let callback = help.viewport_ui_cb.clone().unwrap();
        let mut raw = egui::RawInput {
            viewport_id: viewport_id(),
            ..Default::default()
        };
        raw.viewports.insert(
            viewport_id(),
            egui::ViewportInfo {
                parent: Some(egui::ViewportId::ROOT),
                events: vec![egui::ViewportEvent::Close],
                ..Default::default()
            },
        );
        let closed = ctx.run(raw, |ctx| callback(ctx));
        assert!(!state(&ctx).lock().unwrap().open);
        assert!(closed.viewport_output.values().all(|v| {
            !v.commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Close))
        }));
    }

    #[test]
    fn every_article_stacks_and_wraps_at_minimum_and_default_width() {
        for width in [700.0, 880.0] {
            for a in articles() {
                let ctx = egui::Context::default();
                theme::install(&ctx);
                let mut state = State::default();
                state.select(a.id, Arc::from(""));
                let out = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(width, 690.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| content(ui, &mut state));
                    },
                );
                let text_shapes: Vec<_> = out
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        egui::Shape::Text(t) => Some(t),
                        _ => None,
                    })
                    .collect();
                let title = text_shapes
                    .iter()
                    .find(|t| t.pos.x > 220.0 && t.galley.text() == a.title)
                    .expect("article title");
                let summary = text_shapes
                    .iter()
                    .find(|t| t.galley.text() == a.summary)
                    .expect("article summary");
                assert!(
                    summary.pos.y >= title.pos.y + title.galley.size().y,
                    "{} must stack vertically",
                    a.id
                );
                assert!(
                    summary.pos.x + summary.galley.size().x <= width,
                    "{} must wrap",
                    a.id
                );
                for block in a
                    .body
                    .split("\n\n")
                    .filter_map(|s| s.strip_prefix("@diagram "))
                {
                    assert!(
                        [
                            "mapping",
                            "pipeline",
                            "network",
                            "physics",
                            "capture",
                            "pin",
                            "item-toggle"
                        ]
                        .contains(&block)
                    );
                }
            }
        }
    }
}
