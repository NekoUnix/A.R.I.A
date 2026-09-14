//! Stage-only picking of exported ArtMeshes; never paints into avatar/OBS textures.
use aria_live2d::{Canvas, Drawable};
use eframe::egui::{self, Pos2, Rect, Stroke, Vec2};
use std::collections::BTreeSet;

pub struct Projection {
    pub canvas: Canvas,
    pub rect: Rect,
    pub angle: f32,
    pub fields: Vec<aria_core::deformation::Field>,
}
impl Projection {
    pub fn point(&self, point: [f32; 2]) -> Pos2 {
        let local = crate::items::model_point(self.canvas, point);
        crate::deformation::point(
            self.rect.center()
                + egui::emath::Rot2::from_angle(self.angle) * local * self.rect.height(),
            &self.fields,
        )
    }
    fn vertices(&self, drawable: &Drawable) -> Vec<Pos2> {
        drawable.positions.iter().map(|&p| self.point(p)).collect()
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Operation {
    #[default]
    Replace,
    Add,
    Remove,
    Toggle,
}
impl Operation {
    fn from(modifiers: egui::Modifiers, fallback: Self) -> Self {
        if modifiers.alt {
            Self::Remove
        } else if modifiers.shift {
            Self::Add
        } else if modifiers.command || modifiers.ctrl {
            Self::Toggle
        } else {
            fallback
        }
    }
    fn apply(self, base: &BTreeSet<String>, hits: &BTreeSet<String>) -> BTreeSet<String> {
        match self {
            Self::Replace => hits.clone(),
            Self::Add => base.union(hits).cloned().collect(),
            Self::Remove => base.difference(hits).cloned().collect(),
            Self::Toggle => base.symmetric_difference(hits).cloned().collect(),
        }
    }
}
struct Drag {
    start: Pos2,
    end: Pos2,
    original: BTreeSet<String>,
    operation: Operation,
}
#[derive(Default)]
pub struct Selection {
    pub enabled: bool,
    pub include_hidden: bool,
    pub operation: Operation,
    drag: Option<Drag>,
    press: Option<Drag>,
    processed_frame: Option<u64>,
}
impl Selection {
    pub fn cancel(&mut self, selected: &mut BTreeSet<String>) {
        self.press = None;
        if let Some(drag) = self.drag.take() {
            *selected = drag.original;
        }
    }
    pub fn stage(
        &mut self,
        ui: &mut egui::Ui,
        stage: Rect,
        drawables: &[Drawable],
        layers: &aria_core::layers::Config,
        projection: &Projection,
        selected: &mut BTreeSet<String>,
    ) -> Option<egui::Response> {
        if !self.enabled {
            self.cancel(selected);
            return None;
        }
        let response = ui.interact(
            stage,
            ui.id().with("live2d-layer-marquee"),
            egui::Sense::click_and_drag(),
        );
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        let frame = ui.ctx().cumulative_frame_nr();
        let process = self.processed_frame != Some(frame);
        self.processed_frame = Some(frame);
        if process
            && (self.drag.is_some() || self.press.is_some())
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.cancel(selected);
        } else if process {
            // Remember the press itself: native input can deliver the entire
            // gesture between frames, after PointerState has cleared its origin.
            let press = ui.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } if stage.intersect(ui.clip_rect()).contains(*pos) => Some((*pos, *modifiers)),
                    _ => None,
                })
            });
            if let Some((start, modifiers)) = press
                && ui.ctx().layer_id_at(start) == Some(ui.layer_id())
            {
                self.press = Some(Drag {
                    start,
                    end: start,
                    original: selected.clone(),
                    operation: Operation::from(modifiers, self.operation),
                });
            }
            if self.drag.is_none()
                && self.press.as_ref().is_some_and(|press| {
                    ui.input(|i| i.pointer.latest_pos())
                        .is_some_and(|point| point.distance(press.start) > 4.)
                })
            {
                self.drag = self.press.take();
            }
            if let Some(drag) = &mut self.drag {
                if let Some(point) = ui.input(|i| i.pointer.latest_pos()) {
                    drag.end = stage.clamp(point);
                }
                let hits = pick(
                    drawables,
                    layers,
                    projection,
                    Rect::from_two_pos(drag.start, drag.end),
                    self.include_hidden,
                    false,
                );
                *selected = drag.operation.apply(&drag.original, &hits);
                if response.drag_stopped_by(egui::PointerButton::Primary)
                    || !ui.input(|i| i.pointer.primary_down())
                {
                    self.drag = None;
                }
            } else if response.clicked_by(egui::PointerButton::Primary)
                && let Some(point) = response.interact_pointer_pos()
            {
                let hits = pick(
                    drawables,
                    layers,
                    projection,
                    Rect::from_center_size(point, Vec2::ZERO),
                    self.include_hidden,
                    true,
                );
                *selected = Operation::from(ui.input(|i| i.modifiers), self.operation)
                    .apply(selected, &hits);
            }
            if !ui.input(|i| i.pointer.primary_down()) {
                self.press = None;
            }
        }
        let painter = ui.painter_at(stage);
        let mut combined = Rect::NOTHING;
        for drawable in drawables.iter().filter(|d| selected.contains(&d.id)) {
            let bounds = Rect::from_points(&projection.vertices(drawable));
            if bounds.is_finite() && bounds.intersects(stage) {
                combined = combined.union(bounds);
                if selected.len() > 24 {
                    continue;
                }
                painter.rect_stroke(
                    bounds,
                    2.,
                    Stroke::new(1., crate::theme::mint().gamma_multiply(0.7)),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if selected.len() > 24 && combined.is_finite() {
            painter.rect_stroke(
                combined,
                2.,
                Stroke::new(1.5, crate::theme::mint()),
                egui::StrokeKind::Inside,
            );
        }
        if let Some(drag) = &self.drag {
            let rect = Rect::from_two_pos(drag.start, drag.end);
            painter.rect_filled(rect, 2., crate::theme::mint().gamma_multiply(0.12));
            painter.rect_stroke(
                rect,
                2.,
                Stroke::new(1.5, crate::theme::mint()),
                egui::StrokeKind::Inside,
            );
        }
        Some(response)
    }
}

fn pick(
    drawables: &[Drawable],
    layers: &aria_core::layers::Config,
    projection: &Projection,
    area: Rect,
    include_hidden: bool,
    topmost: bool,
) -> BTreeSet<String> {
    let mut hits = BTreeSet::new();
    let mut best = None;
    for (index, drawable) in drawables.iter().enumerate() {
        if !include_hidden
            && (!drawable.visible || drawable.opacity * layers.opacity(&drawable.id) <= 0.)
        {
            continue;
        }
        let points = projection.vertices(drawable);
        if !Rect::from_points(&points).intersects(area) {
            continue;
        }
        let touches = drawable.indices.as_chunks::<3>().0.iter().any(|indices| {
            let Some(a) = points.get(indices[0] as usize) else {
                return false;
            };
            let Some(b) = points.get(indices[1] as usize) else {
                return false;
            };
            let Some(c) = points.get(indices[2] as usize) else {
                return false;
            };
            intersects([*a, *b, *c], area)
        });
        if touches {
            if topmost {
                if best.is_none_or(|(order, previous): (i32, usize)| {
                    (drawable.order, index) >= (order, previous)
                }) {
                    best = Some((drawable.order, index));
                }
            } else {
                hits.insert(drawable.id.clone());
            }
        }
    }
    if let Some((_, index)) = best {
        hits.insert(drawables[index].id.clone());
    }
    hits
}

// Separating-axis test catches edge crossings and boxes entirely inside a mesh;
// testing only vertices or bounding boxes would select the wrong layer.
fn intersects(triangle: [Pos2; 3], rect: Rect) -> bool {
    if triangle.iter().any(|p| !p.is_finite()) {
        return false;
    }
    let edge = triangle[1] - triangle[0];
    let second = triangle[2] - triangle[0];
    if (edge.x * second.y - edge.y * second.x).abs() < 1e-5 {
        return false;
    }
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let axes = [
        Vec2::X,
        Vec2::Y,
        (triangle[1] - triangle[0]).rot90(),
        (triangle[2] - triangle[1]).rot90(),
        (triangle[0] - triangle[2]).rot90(),
    ];
    axes.into_iter().all(|axis| {
        let span = |points: &[Pos2]| {
            points
                .iter()
                .map(|p| p.to_vec2().dot(axis))
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(v), hi.max(v))
                })
        };
        let (a, b) = span(&triangle);
        let (c, d) = span(&corners);
        a <= d + 1e-4 && c <= b + 1e-4
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_mouse_clicks_and_repeated_boxes_accumulate_without_shift() {
        let ctx = egui::Context::default();
        let stage = projection().rect;
        let mut left = mesh("Left", 0);
        left.positions = vec![[-0.4, 0.4], [-0.1, 0.4], [-0.4, 0.1]];
        let mut right = mesh("Right", 1);
        right.positions = vec![[0.1, 0.4], [0.4, 0.4], [0.1, 0.1]];
        let meshes = [left, right];
        let mut selection = Selection {
            enabled: true,
            operation: Operation::Add,
            ..Default::default()
        };
        let mut selected = BTreeSet::new();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let frame = |events, selection: &mut Selection, selected: &mut BTreeSet<String>| {
            let _ = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(150.))),
                    events,
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default().show(root, |ui| {
                        selection.stage(
                            ui,
                            stage,
                            &meshes,
                            &Default::default(),
                            &projection(),
                            selected,
                        );
                    });
                },
            );
        };
        frame(vec![], &mut selection, &mut selected);
        // Two independent clicks with no modifiers preserve the first layer.
        for point in [egui::pos2(20., 20.), egui::pos2(70., 20.)] {
            frame(
                vec![egui::Event::PointerMoved(point), button(point, true)],
                &mut selection,
                &mut selected,
            );
            frame(vec![button(point, false)], &mut selection, &mut selected);
        }
        assert_eq!(selected, BTreeSet::from(["Left".into(), "Right".into()]));
        selected.clear();
        // Two separate mouse boxes have the same accumulation behavior.
        for x in [10., 60.] {
            let start = egui::pos2(x, 10.);
            let end = egui::pos2(x + 28., 38.);
            frame(
                vec![egui::Event::PointerMoved(start), button(start, true)],
                &mut selection,
                &mut selected,
            );
            frame(
                vec![egui::Event::PointerMoved(end)],
                &mut selection,
                &mut selected,
            );
            frame(vec![button(end, false)], &mut selection, &mut selected);
        }
        assert_eq!(selected.len(), 2);
        // The Remove button is a mouse-only alternative to holding Alt.
        selection.operation = Operation::Remove;
        let point = egui::pos2(20., 20.);
        frame(
            vec![egui::Event::PointerMoved(point), button(point, true)],
            &mut selection,
            &mut selected,
        );
        frame(vec![button(point, false)], &mut selection, &mut selected);
        assert_eq!(selected, BTreeSet::from(["Right".into()]));
        selected.clear();
        selection.operation = Operation::Add;
        let start = egui::pos2(10., 10.);
        let end = egui::pos2(38., 38.);
        frame(
            vec![
                egui::Event::PointerMoved(start),
                button(start, true),
                egui::Event::PointerMoved(end),
                button(end, false),
            ],
            &mut selection,
            &mut selected,
        );
        // Even a complete press/move/release delivered between render frames
        // must form a box instead of dropping the drag.
        assert!(
            !selected.is_empty(),
            "Fast native mouse gestures must not be lost"
        );
    }
    #[test]
    #[cfg(windows)]
    #[ignore = "requires ARIA_TEST_MODEL, Cubism Core and GPU"]
    fn native_marquee_selection_hides_and_restores_rendered_model_layers() {
        use std::path::Path;
        let state = crate::spout::tests::gpu_state();
        let path = std::env::var_os("ARIA_TEST_MODEL").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let mut avatar = crate::live2d::Avatar::load(
            &state,
            Path::new(&core),
            aria_model::load_files(Path::new(&path)).unwrap(),
        )
        .unwrap();
        let mut config = avatar.initial_config.clone();
        config.physics.enabled = false;
        let mut expressions = crate::expressions_panel::ExpressionsPanel::default();
        avatar
            .update(&Default::default(), &mut config, &mut expressions, 0.)
            .unwrap();
        let projection = Projection {
            canvas: avatar.view_canvas(),
            rect: Rect::from_min_size(Pos2::ZERO, egui::vec2(800., 800.)),
            angle: 0.,
            fields: vec![],
        };
        let area = Rect::from_min_max(egui::pos2(200., 120.), egui::pos2(600., 680.));
        let hits = pick(
            &avatar.model.drawables,
            &config.layers,
            &projection,
            area,
            false,
            false,
        );
        assert!(
            hits.len() > 1,
            "A real stage rectangle must select multiple meshes"
        );
        let folder = tempfile::tempdir().unwrap();
        let before = folder.path().join("before.png");
        avatar.save_png(&before).unwrap();
        for id in &hits {
            config.layers.opacity.insert(id.clone(), 0.);
        }
        avatar
            .update(&Default::default(), &mut config, &mut expressions, 0.)
            .unwrap();
        let hidden = folder.path().join("hidden.png");
        avatar.save_png(&hidden).unwrap();
        assert_ne!(
            image::open(&before).unwrap().into_rgba8(),
            image::open(&hidden).unwrap().into_rgba8()
        );
        config.layers.opacity.clear();
        avatar
            .update(&Default::default(), &mut config, &mut expressions, 0.)
            .unwrap();
        let restored = folder.path().join("restored.png");
        avatar.save_png(&restored).unwrap();
        assert_eq!(
            image::open(&before).unwrap().into_rgba8(),
            image::open(&restored).unwrap().into_rgba8()
        );
        eprintln!(
            "Native marquee: selected and restored {} / {} meshes",
            hits.len(),
            avatar.model.drawables.len()
        );
    }
    #[test]
    fn real_pointer_drag_selects_multiple_meshes_and_escape_restores_previous_selection() {
        let ctx = egui::Context::default();
        let stage = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.));
        let meshes = vec![mesh("Back", 0), mesh("Front", 3)];
        let mut selection = Selection {
            enabled: true,
            ..Default::default()
        };
        let mut selected = BTreeSet::new();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let frame = |events: Vec<egui::Event>,
                     selection: &mut Selection,
                     selected: &mut BTreeSet<String>| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.))),
                events,
                ..Default::default()
            };
            let _ = crate::run_test_ui(&ctx, input, |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    selection.stage(
                        ui,
                        stage,
                        &meshes,
                        &Default::default(),
                        &projection(),
                        selected,
                    )
                });
            });
        };
        frame(vec![], &mut selection, &mut selected);
        frame(
            vec![
                egui::Event::PointerMoved(egui::pos2(15., 15.)),
                button(egui::pos2(15., 15.), true),
            ],
            &mut selection,
            &mut selected,
        );
        frame(
            vec![egui::Event::PointerMoved(egui::pos2(55., 55.))],
            &mut selection,
            &mut selected,
        );
        frame(
            vec![button(egui::pos2(55., 55.), false)],
            &mut selection,
            &mut selected,
        );
        assert_eq!(selected, BTreeSet::from(["Back".into(), "Front".into()]));
        frame(
            vec![
                egui::Event::PointerMoved(egui::pos2(75., 75.)),
                button(egui::pos2(75., 75.), true),
            ],
            &mut selection,
            &mut selected,
        );
        frame(
            vec![egui::Event::PointerMoved(egui::pos2(85., 85.))],
            &mut selection,
            &mut selected,
        );
        assert!(selected.is_empty());
        frame(
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            &mut selection,
            &mut selected,
        );
        assert_eq!(selected.len(), 2);
    }
    fn projection() -> Projection {
        Projection {
            canvas: Canvas {
                size: [100., 100.],
                origin: [50., 50.],
                pixels_per_unit: 100.,
            },
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.)),
            angle: 0.,
            fields: vec![],
        }
    }
    fn mesh(id: &str, order: i32) -> Drawable {
        Drawable {
            id: id.into(),
            positions: vec![[-0.4, 0.4], [0.4, 0.4], [-0.4, -0.4]],
            indices: vec![0, 1, 2],
            visible: true,
            opacity: 1.,
            order,
            ..Default::default()
        }
    }
    #[test]
    fn rectangle_uses_triangles_and_preserves_hidden_and_frontmost_rules() {
        let meshes = vec![mesh("Back", 0), mesh("Front", 3)];
        let area = Rect::from_min_size(egui::pos2(20., 20.), Vec2::splat(2.));
        let mut layers = aria_core::layers::Config::default();
        assert_eq!(
            pick(&meshes, &layers, &projection(), area, false, false).len(),
            2
        );
        assert_eq!(
            pick(&meshes, &layers, &projection(), area, false, true),
            BTreeSet::from(["Front".into()])
        );
        let gap = Rect::from_min_size(egui::pos2(75., 75.), Vec2::splat(2.));
        assert!(pick(&meshes, &layers, &projection(), gap, false, false).is_empty());
        layers.opacity.insert("Front".into(), 0.);
        assert_eq!(
            pick(&meshes, &layers, &projection(), area, false, true),
            BTreeSet::from(["Back".into()])
        );
        assert_eq!(
            pick(&meshes, &layers, &projection(), area, true, false).len(),
            2
        );
    }
    #[test]
    fn drag_operations_use_original_selection_and_cancel_restores_it() {
        let base = BTreeSet::from(["A".into(), "B".into()]);
        let hits = BTreeSet::from(["B".into(), "C".into()]);
        assert_eq!(Operation::Add.apply(&base, &hits).len(), 3);
        assert_eq!(
            Operation::Remove.apply(&base, &hits),
            BTreeSet::from(["A".into()])
        );
        assert_eq!(
            Operation::Toggle.apply(&base, &hits),
            BTreeSet::from(["A".into(), "C".into()])
        );
        let mut selection = Selection {
            drag: Some(Drag {
                start: Pos2::ZERO,
                end: Pos2::ZERO,
                original: base.clone(),
                operation: Operation::Replace,
            }),
            ..Default::default()
        };
        let mut selected = hits;
        selection.cancel(&mut selected);
        assert_eq!(selected, base);
    }
    #[test]
    fn rotated_scaled_projection_and_edge_crossing_are_selectable() {
        let mut p = projection();
        p.rect = Rect::from_min_size(egui::pos2(200., 100.), Vec2::splat(200.));
        p.angle = std::f32::consts::FRAC_PI_2;
        let inside = p.point([-0.2, 0.2]);
        assert_eq!(
            pick(
                &[mesh("A", 0)],
                &Default::default(),
                &p,
                Rect::from_center_size(inside, Vec2::splat(2.)),
                false,
                false
            )
            .len(),
            1
        );
        assert!(intersects(
            [
                egui::pos2(0., 0.),
                egui::pos2(100., 0.),
                egui::pos2(50., 100.)
            ],
            Rect::from_min_max(egui::pos2(0., 40.), egui::pos2(100., 41.))
        ));
    }
}
