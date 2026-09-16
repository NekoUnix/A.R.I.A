//! Two-surface mounting for independent, tracking-driven Live2D instances.
use crate::{
    items::{Anchor, Items},
    live2d::Avatar,
};
use aria_core::{
    items::{Item, Pin},
    movement::RigConfig,
};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use std::path::PathBuf;

pub struct Editor {
    id: u64,
    path: PathBuf,
    parent: Option<Pin>,
    child: Option<Pin>,
    tracking: bool,
}
impl Editor {
    pub fn new(item: &Item) -> Self {
        Self {
            id: item.id,
            path: item.path.clone(),
            parent: item.pin.clone(),
            child: item.model.mount.clone(),
            tracking: item.model.animate,
        }
    }
}

impl Items {
    pub fn mount_window(
        &mut self,
        ctx: &egui::Context,
        parent: Option<&Avatar>,
        config: &mut RigConfig,
    ) -> bool {
        let Some(mut editor) = self.mount_editor.take() else {
            return false;
        };
        let Some(item) = config
            .items
            .iter_mut()
            .find(|i| i.id == editor.id && i.path == editor.path)
        else {
            return false;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Mount two Live2D models")
            .id(egui::Id::new("live2d-mount-editor"))
            .open(&mut open).default_width(720.).resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                crate::help::button(ui, "model-mount");
                ui.label(format!("Attach: {}", item.name));
                ui.label("Click or drag a point on each model, then Mount together. Points follow the actual meshes as both models animate.");
                let Some(parent) = parent else { ui.label("Select a Live2D stage to mount another Live2D model."); return; };
                let Some(child) = self.models.avatar(item.id) else {
                    ui.label(self.models.error(item.id).unwrap_or("Loading the attached model… If textures are missing, finish Object texture setup first."));
                    return;
                };
                ui.columns(2, |columns| {
                    columns[0].strong("1 · Point on the current avatar");
                    preview(&mut columns[0], parent, &mut editor.parent);
                    columns[1].strong("2 · Point on the added model");
                    preview(&mut columns[1], child, &mut editor.child);
                });
                ui.checkbox(&mut editor.tracking, "Send current tracking to the added model");
                ui.small("The current avatar keeps its tracking. The added model uses its own mappings, physics and parameter overrides. Freeze pose freezes both. Size, rotation, draw order and fine offsets stay in the object controls.");
                let valid = surface(parent, editor.parent.as_ref()).is_some() && surface(child, editor.child.as_ref()).is_some();
                ui.horizontal(|ui| {
                    apply = ui.add_enabled(valid, egui::Button::new("Mount together")).clicked();
                    cancel = ui.button("Cancel").clicked();
                    if !valid { ui.small("Choose a visible mesh point in both previews."); }
                });
            });
        if apply {
            item.pin = editor.parent;
            item.model.mount = editor.child;
            item.model.animate = editor.tracking;
            item.position = [0.; 2];
            self.selected = Some(item.id);
            self.pick_pin = false;
            self.edit_pin = false;
            self.save_now = true;
            self.message = Some("Mounted. Both mesh points stay joined; drag the object to add an offset or reopen Mount two models to change the points.".into());
            true
        } else {
            if open && !cancel {
                self.mount_editor = Some(editor);
            }
            false
        }
    }
}

fn surface(avatar: &Avatar, pin: Option<&Pin>) -> Option<Vec2> {
    match crate::items::anchor(pin, Some(avatar)) {
        Anchor::Surface { point, .. } => Some(point),
        _ => None,
    }
}

fn preview(ui: &mut egui::Ui, avatar: &Avatar, pin: &mut Option<Pin>) {
    let image = avatar.image();
    let scale = (ui.available_width().max(1.) / image.size.x).min(320. / image.size.y);
    let (rect, response) = ui.allocate_exact_size(image.size * scale, Sense::click_and_drag());
    ui.painter().rect_filled(rect, 6., Color32::from_gray(40));
    ui.painter().image(
        image.id,
        rect,
        Rect::from_min_max(Pos2::ZERO, egui::pos2(1., 1.)),
        Color32::WHITE,
    );
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let point = (pos - rect.center()) / rect.height();
        if let Some(picked) =
            crate::items::pick_surface(avatar.view_canvas(), &avatar.model.drawables, point)
        {
            *pin = Some(picked);
        }
    }
    if let Some(point) = surface(avatar, pin.as_ref()) {
        let center = rect.center() + point * rect.height();
        ui.painter()
            .circle_stroke(center, 8., egui::Stroke::new(2., Color32::YELLOW));
        ui.painter().line_segment(
            [center - egui::vec2(12., 0.), center + egui::vec2(12., 0.)],
            egui::Stroke::new(1., Color32::YELLOW),
        );
        ui.painter().line_segment(
            [center - egui::vec2(0., 12.), center + egui::vec2(0., 12.)],
            egui::Stroke::new(1., Color32::YELLOW),
        );
        ui.small("Mount point selected · click again to change it");
    } else {
        ui.small("Click the model to choose its mount point");
    }
}
