//! Shared OBS composition. Per-avatar transforms never change the editing stages.
use super::*;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Transform {
    pub position: [f32; 2],
    pub zoom: f32,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.; 2],
            zoom: 1.,
        }
    }
}
impl Transform {
    pub fn sanitize(&mut self) {
        for value in &mut self.position {
            *value = if value.is_finite() {
                value.clamp(-1., 1.)
            } else {
                0.
            };
        }
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.25, 3.)
        } else {
            1.
        };
    }
}
#[derive(Clone)]
pub struct Actor {
    pub id: u64,
    pub name: String,
    pub scene: Scene,
}
#[derive(Clone, Default)]
pub struct Composition {
    pub avatars: Vec<Actor>,
}
impl From<Scene> for Composition {
    fn from(scene: Scene) -> Self {
        Self {
            avatars: vec![Actor {
                id: 0,
                name: "Avatar".into(),
                scene,
            }],
        }
    }
}
pub trait PaintScene {
    fn paint(&self, painter: &egui::Painter, canvas: Rect, config: &CanvasSettings);
}
impl PaintScene for Scene {
    fn paint(&self, painter: &egui::Painter, canvas: Rect, config: &CanvasSettings) {
        self.paint(painter, canvas, config);
    }
}
impl PaintScene for Composition {
    fn paint(&self, painter: &egui::Painter, canvas: Rect, config: &CanvasSettings) {
        painter.rect_filled(canvas, 0., config.background.color(config.key));
        for avatar in &self.avatars {
            let transform = self.transform(avatar.id, config);
            avatar.scene.paint_subject(
                painter,
                canvas.translate(
                    egui::vec2(transform.position[0], transform.position[1]) * canvas.size(),
                ),
                transform.zoom,
            );
        }
    }
}
#[derive(Clone, Copy)]
struct Drag {
    id: u64,
    origin: egui::Pos2,
    start: [f32; 2],
}
impl Composition {
    fn transform(&self, id: u64, config: &CanvasSettings) -> Transform {
        config.avatars.get(&id).copied().unwrap_or(Transform {
            position: config.position,
            zoom: config.zoom,
        })
    }
    fn bounds(&self, actor: &Actor, canvas: Rect, config: &CanvasSettings) -> Rect {
        let transform = self.transform(actor.id, config);
        // Bounds uses only framing and model placement, so avoid copying the transform map.
        actor.scene.bounds(
            canvas,
            &CanvasSettings {
                position: transform.position,
                zoom: transform.zoom,
                ..Default::default()
            },
        )
    }
    fn pick(&self, pos: egui::Pos2, canvas: Rect, config: &CanvasSettings) -> Option<u64> {
        self.avatars
            .iter()
            .rev()
            .find(|a| self.bounds(a, canvas, config).contains(pos))
            .map(|a| a.id)
    }
    fn target(
        &self,
        pos: egui::Pos2,
        manual: Option<u64>,
        canvas: Rect,
        config: &CanvasSettings,
    ) -> Option<u64> {
        manual
            .filter(|id| {
                self.avatars
                    .iter()
                    .any(|a| a.id == *id && self.bounds(a, canvas, config).contains(pos))
            })
            .or_else(|| self.pick(pos, canvas, config))
    }
    pub(super) fn canvas(
        &self,
        ui: &mut egui::Ui,
        config: &mut CanvasSettings,
        generation: u64,
    ) -> bool {
        let canvas = ui.max_rect();
        let id = ui
            .id()
            .with(("composition-drag", ui.ctx().viewport_id(), generation));
        let manual_id = id.with("menu-target");
        let menu_rect = Rect::from_min_size(canvas.min + egui::vec2(8., 8.), egui::vec2(190., 28.));
        let manual = ui.data(|d| d.get_temp::<u64>(manual_id));
        let response = ui.interact(canvas, id, egui::Sense::click_and_drag());
        let mut dirty = false;
        if config.locked && !self.avatars.is_empty() {
            config.locked_avatars.extend(config.avatars.keys().copied());
            config
                .locked_avatars
                .extend(self.avatars.iter().map(|a| a.id));
            config.locked = false;
            dirty = true;
            ui.data_mut(|d| d.remove::<Drag>(id));
        }
        if ui.ctx().current_pass_index() == 0 {
            let mut released_actor = None;
            let events = ui.input(|i| i.raw.events.clone());
            for event in events {
                match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers,
                    } if canvas.contains(pos)
                        && ui.rect_contains_pointer(canvas)
                        && !(self.avatars.len() > 1 && menu_rect.contains(pos)) =>
                    {
                        ui.data_mut(|d| d.remove::<Drag>(id));
                        let target = self.target(pos, manual, canvas, config);
                        if let Some(actor) = target {
                            let start = self.transform(actor, config).position;
                            dirty |= config.selected_avatar != Some(actor);
                            config.selected_avatar = Some(actor);
                            if modifiers.command && modifiers.shift && !modifiers.alt {
                                if !config.locked_avatars.remove(&actor) {
                                    config.locked_avatars.insert(actor);
                                }
                                dirty = true;
                                continue;
                            }
                            if config.locked_avatars.contains(&actor) {
                                continue;
                            }
                            ui.data_mut(|d| {
                                d.insert_temp(
                                    id,
                                    Drag {
                                        id: actor,
                                        origin: pos,
                                        start,
                                    },
                                )
                            });
                        }
                    }
                    egui::Event::PointerMoved(pos) => {
                        if let Some(drag) = ui.data(|d| d.get_temp::<Drag>(id))
                            && self.avatars.iter().any(|a| a.id == drag.id)
                            && !config.locked_avatars.contains(&drag.id)
                        {
                            let mut transform = self.transform(drag.id, config);
                            transform.position =
                                moved_position(drag.start, pos - drag.origin, canvas.size());
                            config.avatars.insert(drag.id, transform);
                            dirty = true;
                        }
                    }
                    egui::Event::PointerButton {
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => {
                        released_actor = ui.data(|d| d.get_temp::<Drag>(id)).map(|d| d.id);
                        ui.data_mut(|d| d.remove::<Drag>(id));
                    }
                    egui::Event::WindowFocused(false) => {
                        ui.data_mut(|d| d.remove::<Drag>(id));
                    }
                    egui::Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        ..
                    } if ui.rect_contains_pointer(canvas) => {
                        let pointer = ui.input(|i| i.pointer.hover_pos());
                        let target = pointer
                            .filter(|p| !(self.avatars.len() > 1 && menu_rect.contains(*p)))
                            .and_then(|p| self.target(p, manual, canvas, config));
                        if let Some(target) = target
                            && !config.locked_avatars.contains(&target)
                        {
                            let options = ui.ctx().options(|o| o.input_options);
                            let horizontal =
                                modifiers.matches_any(options.horizontal_scroll_modifier);
                            let vertical = modifiers.matches_any(options.vertical_scroll_modifier);
                            let delta = if horizontal && !vertical {
                                0.
                            } else if vertical && !horizontal {
                                delta.x + delta.y
                            } else {
                                delta.y
                            };
                            let scale = match unit {
                                egui::MouseWheelUnit::Point => 1.,
                                egui::MouseWheelUnit::Line => options.line_scroll_speed,
                                egui::MouseWheelUnit::Page => canvas.height(),
                            };
                            let mut transform = self.transform(target, config);
                            transform.zoom =
                                (transform.zoom * (delta * scale * 0.002).exp()).clamp(0.25, 3.);
                            config.avatars.insert(target, transform);
                            config.selected_avatar = Some(target);
                            dirty |= delta != 0.;
                        }
                    }
                    _ => {}
                }
            }
            if response.double_clicked()
                && let Some(target) = released_actor
                && !config.locked_avatars.contains(&target)
            {
                let mut transform = self.transform(target, config);
                transform.position = [0.; 2];
                config.avatars.insert(target, transform);
                dirty = true;
            }
        }
        self.paint(ui.painter(), canvas, config);
        response.context_menu(|ui| {
            if let Some(actor) = config
                .selected_avatar
                .and_then(|id| self.avatars.iter().find(|a| a.id == id))
                .or_else(|| self.avatars.first())
            {
                ui.label(&actor.name);
                let mut locked = config.locked_avatars.contains(&actor.id);
                if ui.checkbox(&mut locked, "Lock position & scale").changed() {
                    if locked {
                        config.locked_avatars.insert(actor.id);
                    } else {
                        config.locked_avatars.remove(&actor.id);
                    }
                    dirty = true;
                }
                ui.add_enabled_ui(!locked, |ui| {
                    if ui.button("Center avatar").clicked() {
                        let mut transform = self.transform(actor.id, config);
                        transform.position = [0.; 2];
                        config.avatars.insert(actor.id, transform);
                        dirty = true;
                        ui.close();
                    }
                    if ui.button("Reset position & scale").clicked() {
                        config.avatars.insert(actor.id, Transform::default());
                        dirty = true;
                        ui.close();
                    }
                });
            }
            ui.separator();
            if ui
                .checkbox(&mut config.always_on_top, "Keep preview on top")
                .changed()
            {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                        if config.always_on_top {
                            egui::WindowLevel::AlwaysOnTop
                        } else {
                            egui::WindowLevel::Normal
                        },
                    ));
                dirty = true;
            }
        });
        if self.avatars.len() > 1 {
            // Preview-only targeting menu; full-resolution native output receives artwork only.
            let menu = Rect::from_min_size(canvas.min + egui::vec2(8., 8.), egui::vec2(190., 28.));
            ui.scope_builder(egui::UiBuilder::new().max_rect(menu), |ui| {
                egui::ComboBox::from_id_salt("preview-avatar")
                    .selected_text(
                        config
                            .selected_avatar
                            .and_then(|id| self.avatars.iter().find(|a| a.id == id))
                            .map_or("Select avatar", |a| a.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for actor in &self.avatars {
                            if ui
                                .selectable_value(
                                    &mut config.selected_avatar,
                                    Some(actor.id),
                                    &actor.name,
                                )
                                .clicked()
                            {
                                ui.data_mut(|d| d.insert_temp(manual_id, actor.id));
                                dirty = true;
                            }
                        }
                    });
            });
        }
        if let Some(actor) = config
            .selected_avatar
            .and_then(|id| self.avatars.iter().find(|a| a.id == id))
        {
            if response.hovered() || response.dragged() {
                let bounds = self.bounds(actor, canvas, config).intersect(canvas);
                ui.painter().rect_stroke(
                    bounds,
                    2.,
                    egui::Stroke::new(
                        1.,
                        if config.locked_avatars.contains(&actor.id) {
                            theme::orange()
                        } else {
                            theme::mint()
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
            }
            // Legacy controls/API fields retain the one-avatar behavior.
            if actor.id == 0
                && let Some(transform) = config.avatars.remove(&0)
            {
                config.position = transform.position;
                config.zoom = transform.zoom;
            }
        }
        let locked = config
            .selected_avatar
            .is_some_and(|id| config.locked_avatars.contains(&id));
        let text = format!(
            "{} · {LOCK_GESTURE} to {}",
            if locked {
                "Framing locked"
            } else {
                "Drag / scroll to arrange"
            },
            if locked { "unlock" } else { "lock" }
        );
        let galley =
            ui.painter()
                .layout_no_wrap(text, egui::FontId::proportional(11.), theme::text_color());
        let pos = canvas.left_bottom() + egui::vec2(8., -galley.size().y - 10.);
        ui.painter().rect_filled(
            Rect::from_min_size(pos, galley.size()).expand(4.),
            4.,
            theme::card_color(),
        );
        ui.painter().galley(pos, galley, theme::text_color());
        dirty
    }
}
impl OutputWindows {
    pub fn ensure_avatar(&self, id: u64, already_loaded: usize) {
        let mut state = self.state.lock().unwrap();
        for index in 0..3 {
            let config = state.config.canvas_mut(index);
            config.avatars.entry(id).or_insert_with(|| Transform {
                position: if already_loaded == 0 {
                    config.position
                } else {
                    [
                        (if already_loaded.is_multiple_of(2) {
                            -1.
                        } else {
                            1.
                        }) * 0.22,
                        0.,
                    ]
                },
                zoom: if already_loaded == 0 {
                    config.zoom
                } else {
                    0.7
                },
            });
            config.selected_avatar = Some(id);
        }
        state.generation = state.generation.wrapping_add(1);
        state.dirty = true;
    }
    pub fn forget_avatar(&self, id: u64) {
        let mut state = self.state.lock().unwrap();
        for index in 0..3 {
            let config = state.config.canvas_mut(index);
            config.avatars.remove(&id);
            config.locked_avatars.remove(&id);
            if config.selected_avatar == Some(id) {
                config.selected_avatar = None;
            }
        }
        state.generation = state.generation.wrapping_add(1);
        state.dirty = true;
    }
}
