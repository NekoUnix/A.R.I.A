//! Stage assets, mesh anchors, and interaction. All persisted data lives in RigConfig.
use crate::{avatar::Sprite, live2d::Avatar, output::Scene};
use anyhow::Result;
use aria_core::{
    items::{Item, MAX_ITEMS, Pin, RuleMode, RuleState, SignalKind},
    movement::RigConfig,
    rig::{Inputs, RigParameter},
};
use eframe::egui::{self, Color32, Pos2, Rect, Vec2, vec2};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Free,
    Surface {
        point: Vec2,
        angle: f32,
        scale: f32,
        opacity: f32,
    },
    Puppet(Vec2),
    Missing,
}

#[derive(Clone, Copy)]
pub enum Target<'a> {
    Live2d(&'a Avatar),
    Vrm(&'a crate::vrm::Avatar),
    Puppet,
}
impl<'a> From<Option<&'a Avatar>> for Target<'a> {
    fn from(value: Option<&'a Avatar>) -> Self {
        value.map_or(Self::Puppet, Self::Live2d)
    }
}
impl<'a> From<(Option<&'a Avatar>, Option<&'a crate::vrm::Avatar>)> for Target<'a> {
    fn from((live2d, vrm): (Option<&'a Avatar>, Option<&'a crate::vrm::Avatar>)) -> Self {
        live2d
            .map(Self::Live2d)
            .or_else(|| vrm.map(Self::Vrm))
            .unwrap_or(Self::Puppet)
    }
}
impl Target<'_> {
    fn is_puppet(self) -> bool {
        matches!(self, Self::Puppet)
    }
    fn pick(self, point: Vec2) -> Option<Pin> {
        match self {
            Self::Live2d(a) => pick_surface(a.view_canvas(), &a.model.drawables, point),
            Self::Vrm(a) => a.pick_pin(point),
            Self::Puppet => Some(Pin::Puppet {
                point: bounded(point),
            }),
        }
    }
    fn anchor(self, pin: Option<&Pin>) -> Anchor {
        match (self, pin) {
            (_, None) => Anchor::Free,
            (Self::Vrm(a), Some(pin @ Pin::VrmSurface { .. })) => a.resolve_pin(pin),
            (Self::Live2d(a), pin) => anchor(pin, Some(a)),
            (_, pin) => anchor(pin, None),
        }
    }
}
#[derive(Clone)]
pub enum ItemImage {
    Png(Sprite),
    Model {
        image: crate::cubism_render::ModelImage,
        _lease: Arc<crate::cubism_render::ModelTexture>,
        revision: u64,
    },
}
impl ItemImage {
    pub fn id(&self) -> egui::TextureId {
        match self {
            Self::Png(s) => s.texture.id(),
            Self::Model { image, .. } => image.id,
        }
    }
    fn size(&self) -> Vec2 {
        match self {
            Self::Png(s) => s.size,
            Self::Model { image, .. } => image.size,
        }
    }
    fn revision(&self) -> u64 {
        match self {
            Self::Png(_) => 0,
            Self::Model { revision, .. } => *revision,
        }
    }
}
#[derive(Clone)]
pub struct DrawItem {
    pub deformation: Option<crate::deformation::ObjectWarp>,
    pub tint: Color32,
    pub item: Item,
    pub image: ItemImage,
    pub anchor: Anchor,
    pub visible: bool,
}
impl DrawItem {
    fn same(&self, other: &Self) -> bool {
        self.item == other.item
            && self.deformation == other.deformation
            && self.tint == other.tint
            && self.image.id() == other.image.id()
            && self.image.revision() == other.image.revision()
            && self.anchor == other.anchor
            && self.visible == other.visible
    }
    fn pose(&self, scene: &Scene, canvas: Rect, zoom: f32) -> Option<Placement> {
        self.pose_with_fields(
            scene,
            canvas,
            zoom,
            &crate::deformation::avatar_fields(scene, canvas, zoom),
        )
    }
    fn pose_with_fields(
        &self,
        scene: &Scene,
        canvas: Rect,
        zoom: f32,
        fields: &[aria_core::deformation::Field],
    ) -> Option<Placement> {
        let base = frame(scene, canvas, zoom, false);
        let (origin, angle, scale, opacity) = match self.anchor {
            Anchor::Missing => return None,
            Anchor::Free => (base.origin, 0.0, 1.0, 1.0),
            Anchor::Surface {
                point,
                angle,
                scale,
                opacity,
            } => (base.to_screen(point), angle, scale, opacity),
            Anchor::Puppet(point) => {
                let moving = frame(scene, canvas, zoom, true);
                (
                    moving.to_screen(point),
                    moving.angle,
                    (moving.stretch.x * moving.stretch.y).abs().sqrt(),
                    scene
                        .images
                        .last()
                        .map_or(1.0, |d| d.opacity * d.transform.opacity),
                )
            }
        };
        let angle = if self.item.follow_rotation {
            angle
        } else {
            0.0
        };
        let scale = if self.item.follow_scale { scale } else { 1.0 };
        let offset =
            rotate(vec2(self.item.position[0], self.item.position[1]), angle) * base.scale * scale;
        let origin = if self.anchor != Anchor::Free && !fields.is_empty() {
            crate::deformation::point(origin, fields)
        } else {
            origin
        };
        Some(Placement {
            center: origin + offset,
            size: self.image.size() / self.image.size().y * self.item.height * base.scale * scale,
            angle: angle + self.item.rotation.to_radians(),
            opacity: self.item.opacity
                * if self.item.follow_visibility {
                    opacity
                } else {
                    1.0
                },
            offset_angle: angle,
            offset_scale: base.scale * scale,
        })
    }
}
#[derive(Clone, Copy)]
struct Placement {
    center: Pos2,
    size: Vec2,
    angle: f32,
    opacity: f32,
    offset_angle: f32,
    offset_scale: f32,
}
impl Placement {
    fn corners(self) -> [Pos2; 4] {
        [
            vec2(-0.5, -0.5),
            vec2(0.5, -0.5),
            vec2(0.5, 0.5),
            vec2(-0.5, 0.5),
        ]
        .map(|p| self.center + rotate(p * self.size, self.angle))
    }
    fn contains(self, point: Pos2) -> bool {
        let local = rotate(point - self.center, -self.angle).abs();
        local.x <= self.size.x * 0.5 && local.y <= self.size.y * 0.5
    }
}
#[derive(Clone, Copy)]
struct Frame {
    origin: Pos2,
    scale: f32,
    angle: f32,
    stretch: Vec2,
}
impl Frame {
    fn to_screen(self, point: Vec2) -> Pos2 {
        self.origin + rotate(point * self.stretch, self.angle) * self.scale
    }
    fn local(self, point: Pos2) -> Vec2 {
        rotate(point - self.origin, -self.angle) / self.scale / self.stretch
    }
}
fn rotate(v: Vec2, angle: f32) -> Vec2 {
    egui::emath::Rot2::from_angle(angle) * v
}
fn frame(scene: &Scene, canvas: Rect, zoom: f32, moving: bool) -> Frame {
    if let Some(model) = scene.model {
        let rect = model.rect(canvas, zoom);
        return Frame {
            stretch: Vec2::splat(1.0),
            origin: rect.center(),
            scale: rect.height(),
            angle: 0.0,
        };
    }
    let p = scene.params.0;
    let angle = if moving { -p[2].to_radians() } else { 0.0 };
    if let Some(sprite) = &scene.sprite {
        let scale = (canvas.width() * 0.75 / sprite.size.x)
            .min(canvas.height() * 0.9 / sprite.size.y)
            * zoom;
        let offset = if moving {
            vec2(
                p[0] * canvas.width()
                    * if scene.images.is_empty() {
                        0.0015
                    } else {
                        0.002
                    },
                -p[1] * canvas.height() * 0.001,
            )
        } else {
            Vec2::ZERO
        };
        let motion = if moving {
            scene.images.last().map(|d| d.transform).unwrap_or_default()
        } else {
            Default::default()
        };
        Frame {
            stretch: vec2(motion.scale[0], motion.scale[1]),
            origin: canvas.center()
                + offset
                + vec2(motion.offset[0], motion.offset[1]) * sprite.size.y * scale,
            scale: sprite.size.y * scale,
            angle: angle + motion.rotation,
        }
    } else {
        let scale = (canvas.width() / 480.0).min(canvas.height() / 560.0) * zoom;
        let offset = if moving {
            vec2(p[0] * 0.8, 22.0 - p[1] * 0.35)
        } else {
            vec2(0.0, 22.0)
        };
        Frame {
            stretch: Vec2::splat(1.0),
            origin: canvas.center() + offset * scale,
            scale: 560.0 * scale,
            angle,
        }
    }
}
pub fn effect_to_screen(scene: &Scene, canvas: Rect, zoom: f32, point: [f32; 2]) -> Pos2 {
    frame(scene, canvas, zoom, false).to_screen(vec2(point[0], point[1]))
}
pub fn effect_from_screen(scene: &Scene, canvas: Rect, zoom: f32, point: Pos2) -> [f32; 2] {
    let v = frame(scene, canvas, zoom, false).local(point);
    [v.x.clamp(-2.0, 2.0), v.y.clamp(-2.0, 2.0)]
}
pub fn dent_field(
    scene: &Scene,
    canvas: Rect,
    zoom: f32,
    dent: &crate::deformation::AvatarDent,
) -> Option<aria_core::deformation::Field> {
    let base = frame(scene, canvas, zoom, false);
    let (center, angle, scale, opacity) = match dent.anchor {
        Anchor::Surface {
            point,
            angle,
            scale,
            opacity,
        } => (base.to_screen(point), angle, scale, opacity),
        Anchor::Puppet(point) => {
            let moving = frame(scene, canvas, zoom, true);
            (
                moving.to_screen(point),
                moving.angle,
                (moving.stretch.x * moving.stretch.y).abs().sqrt(),
                scene
                    .images
                    .last()
                    .map_or(1.0, |d| d.opacity * d.transform.opacity),
            )
        }
        _ => return None,
    };
    if opacity <= 0.001 {
        return None;
    }
    let mut field = dent.field;
    field.center = [center.x, center.y];
    field.radius *= base.scale * scale;
    let direction = rotate(vec2(field.direction[0], field.direction[1]), angle);
    field.direction = [direction.x, direction.y];
    field.depth *= opacity;
    field.squash *= opacity;
    field.shading *= opacity;
    Some(field)
}
pub fn paint(painter: &egui::Painter, scene: &Scene, canvas: Rect, zoom: f32, behind: bool) {
    paint_list(painter, scene, canvas, zoom, behind, &scene.items);
}
pub fn paint_list(
    painter: &egui::Painter,
    scene: &Scene,
    canvas: Rect,
    zoom: f32,
    behind: bool,
    draws: &[DrawItem],
) {
    let fields = crate::deformation::avatar_fields(scene, canvas, zoom);
    for draw in draws
        .iter()
        .filter(|d| d.item.behind == behind && d.visible)
    {
        let Some(pose) = draw.pose_with_fields(scene, canvas, zoom, &fields) else {
            continue;
        };
        if pose.opacity <= 0.001 {
            continue;
        }
        let mesh = crate::deformation::textured_mesh(
            draw.image.id(),
            pose.center,
            pose.size,
            pose.angle,
            draw.tint.gamma_multiply(pose.opacity),
            draw.deformation.as_ref(),
            &[],
        );
        painter.add(egui::Shape::mesh(mesh));
    }
}

#[derive(Default)]
pub struct Items {
    pub clock: f32,
    pub models: crate::object_models::ObjectModels,
    assets: BTreeMap<PathBuf, Result<Sprite, String>>,
    rules: BTreeMap<u64, RuleState>,
    pending_save: Option<std::time::Instant>,
    pub save_now: bool,
    was_frozen: bool,
    pub selected: Option<u64>,
    pub pick_pin: bool,
    pub edit_pin: bool,
    anchor_drag: Option<Placement>,
    pub message: Option<String>,
    pub draft: aria_core::shortcuts::Shortcut,
    pub draws: Arc<[DrawItem]>,
    pub revision: u64,
    pub pin_here: bool,
    pub unpin: bool,
    drag: Option<(u64, Pos2, [f32; 2])>,
    pub last_hover: Option<Pos2>,
}
impl Items {
    #[cfg(feature = "screenshots")]
    pub fn prepare_smoke<'a>(&mut self, config: &mut RigConfig, avatar: impl Into<Target<'a>>) {
        let avatar = avatar.into();
        assert_eq!(
            config.items.len(),
            1,
            "The application must receive the dropped object exactly once"
        );
        let item = &mut config.items[0];
        item.name = "Head sparkle".into();
        item.height = 0.16;
        item.position = [0.0; 2];
        item.pin = Some(
            avatar
                .pick(vec2(0.06, -0.16))
                .or_else(|| avatar.pick(Vec2::ZERO))
                .expect("Smoke surface must hit the avatar"),
        );
        self.selected = Some(item.id);
        if aria_core::items::is_model(&item.path) {
            item.name = "Pinned Live2D object".into();
            item.height = 0.42;
            item.position = [0.22, -0.12];
            item.model.parameters.insert("ParamAngleX".into(), 30.0);
            self.message =
                Some("Independent Live2D object · mesh pin · separate parameter pose".into());
            return;
        }
        let mut background = item.clone();
        background.id += 1;
        background.name = "Backdrop glow".into();
        background.pin = None;
        background.position = [-0.1, 0.12];
        background.height = 0.65;
        background.opacity = 0.3;
        background.behind = true;
        background.rule.mode = RuleMode::WhileInRange;
        background.rule.kind = SignalKind::Parameter;
        background.rule.source = "ParamAngleX".into();
        background.rule.start = -30.0;
        background.rule.end = 30.0;
        background.condition = true;
        config.items.push(background);
        self.message =
            Some("Pinned accessories · per-avatar layouts · input-driven toggles".into());
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn reset_rules(&mut self) {
        self.rules.clear();
    }
    pub fn reload(&mut self) {
        self.assets.clear();
        self.models.clear();
    }
    pub fn edited(&mut self) {
        self.pending_save = Some(std::time::Instant::now());
    }
    pub fn take_save(&mut self) -> bool {
        if std::mem::take(&mut self.save_now)
            || self
                .pending_save
                .is_some_and(|t| t.elapsed() > std::time::Duration::from_millis(500))
                && self.drag.is_none()
        {
            self.pending_save = None;
            true
        } else {
            false
        }
    }
    pub fn error(&self, path: &Path) -> Option<&str> {
        self.assets
            .get(path)
            .and_then(|r| r.as_ref().err())
            .map(String::as_str)
    }
    pub fn merge_palette(&self, palette: &mut crate::chroma::Palette) {
        self.models.merge_palette(palette);
        for sprite in self.assets.values().filter_map(|r| r.as_ref().ok()) {
            palette.merge(&sprite.palette);
        }
    }
    pub fn add(&mut self, paths: Vec<PathBuf>, config: &mut RigConfig, position: [f32; 2]) -> bool {
        let mut added = 0;
        let mut errors = Vec::new();
        for path in paths {
            if config.items.len() == MAX_ITEMS {
                errors.push(format!("Limit: {MAX_ITEMS} stage objects per avatar."));
                break;
            }
            if !is_item(&path) {
                errors.push(format!(
                    "{} is not a PNG, GIF, moc3 or model3 export.",
                    path.display()
                ));
                continue;
            }
            if aria_core::items::is_model(&path)
                && config
                    .items
                    .iter()
                    .filter(|i| aria_core::items::is_model(&i.path))
                    .count()
                    >= aria_core::items::MAX_MODEL_ITEMS
            {
                errors.push("Maximum four Live2D objects per avatar.".into());
                continue;
            }
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            let mut id = id.max(1);
            while config.items.iter().any(|i| i.id == id) {
                id = id.wrapping_add(1).max(1);
            }
            let name: String = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .take(80)
                .collect();
            config.items.push(Item {
                id,
                name: if name.trim().is_empty() {
                    "Stage object".into()
                } else {
                    name
                },
                path: path.canonicalize().unwrap_or(path),
                position,
                ..Default::default()
            });
            self.selected = Some(id);
            added += 1;
        }
        self.pick_pin = false;
        self.message = Some(format!(
            "Added {added} object(s). Drag to position, then choose a pin point. {}",
            errors.join(" ")
        ));
        added > 0
    }
    pub fn sync_assets(
        &mut self,
        ctx: &egui::Context,
        state: Option<&eframe::egui_wgpu::RenderState>,
        config: &RigConfig,
    ) {
        self.assets
            .retain(|path, _| config.items.iter().any(|i| &i.path == path));
        self.rules
            .retain(|id, _| config.items.iter().any(|i| &i.id == id));
        if self
            .selected
            .is_some_and(|id| !config.items.iter().any(|i| i.id == id))
        {
            self.selected = None;
            self.pick_pin = false;
        }
        for item in config.items.iter().filter(|i| is_png(&i.path)) {
            if !self.assets.contains_key(&item.path) {
                let used: u64 = self
                    .assets
                    .values()
                    .filter_map(|r| r.as_ref().ok())
                    .map(Sprite::bytes)
                    .sum();
                let result = load_png(ctx, state, &item.path, used).map_err(|e| format!("{e:#}"));
                self.assets.insert(item.path.clone(), result);
            }
        }
    }
    pub fn evaluate(
        &mut self,
        config: &mut RigConfig,
        inputs: &Inputs,
        parameters: &[RigParameter],
    ) -> bool {
        if config.pose.mode == aria_core::movement::PoseMode::Frozen {
            self.was_frozen = true;
            return false;
        }
        if std::mem::take(&mut self.was_frozen) {
            self.reset_rules();
        }
        let mut changed = false;
        for item in &mut config.items {
            let was = item.visible;
            // Evaluate the gate with a true master so manual toggles also work while frozen.
            let gate = if item.rule.mode == RuleMode::WhileInRange {
                item.visible = true;
                let gate = self
                    .rules
                    .entry(item.id)
                    .or_default()
                    .update(item, signal(item, inputs, parameters));
                item.visible = was;
                gate
            } else {
                self.rules
                    .entry(item.id)
                    .or_default()
                    .update(item, signal(item, inputs, parameters));
                true
            };
            item.condition = gate;
            changed |= was != item.visible;
        }
        changed
    }
    pub fn refresh<'a>(&mut self, config: &RigConfig, avatar: impl Into<Target<'a>>) {
        let avatar = avatar.into();
        let draws: Vec<_> = config
            .items
            .iter()
            .filter_map(|item| {
                let image = if aria_core::items::is_model(&item.path) {
                    self.models.image(item.id)?
                } else {
                    ItemImage::Png(
                        self.assets
                            .get(&item.path)?
                            .as_ref()
                            .ok()?
                            .at(self.clock, 1.0, true),
                    )
                };
                Some(DrawItem {
                    deformation: None,
                    tint: Color32::WHITE,
                    item: item.clone(),
                    image,
                    anchor: avatar.anchor(item.pin.as_ref()),
                    visible: item.visible
                        && (item.rule.mode != RuleMode::WhileInRange || item.condition),
                })
            })
            .collect();
        if draws.len() != self.draws.len()
            || draws.iter().zip(self.draws.iter()).any(|(a, b)| !a.same(b))
        {
            self.draws = draws.into();
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn stage<'a>(
        &mut self,
        ui: &mut egui::Ui,
        canvas: Rect,
        scene: &Scene,
        config: &mut RigConfig,
        avatar: impl Into<Target<'a>>,
        zoom: f32,
    ) -> bool {
        let avatar = avatar.into();
        if ui.ctx().current_pass_index() != 0 {
            return false;
        }
        if let Some(pos) = ui
            .input(|i| i.pointer.hover_pos())
            .filter(|p| canvas.contains(*p))
        {
            self.last_hover = Some(pos);
        }
        let response = ui.interact(
            canvas,
            ui.id().with("png-stage"),
            egui::Sense::click_and_drag(),
        );
        let mut changed = false;
        let selected = self
            .selected
            .and_then(|id| scene.items.iter().find(|d| d.item.id == id));
        if selected.is_none_or(|d| d.item.pin.is_none()) {
            self.edit_pin = false;
            self.anchor_drag = None;
        }
        if self.edit_pin {
            self.pick_pin = false;
            self.drag = None;
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.edit_pin = false;
                self.anchor_drag = None;
                return false;
            }
            if let Some(draw) = selected
                && !draw.item.locked
            {
                if response.drag_started()
                    && let Some(start) = ui.input(|i| i.pointer.press_origin())
                    && let Some(marker) = anchor_marker(draw, scene, canvas, zoom)
                    && marker.distance(start) < 18.
                {
                    self.anchor_drag = draw.pose(scene, canvas, zoom);
                }
                let pose = self.anchor_drag.or_else(|| draw.pose(scene, canvas, zoom));
                if (response.clicked() || response.dragged() && self.anchor_drag.is_some())
                    && let Some(point) = response.interact_pointer_pos()
                    && let Some(pose) = pose
                    && let Some(pin) =
                        avatar.pick(frame(scene, canvas, zoom, avatar.is_puppet()).local(point))
                    && let Some(item) = config.items.iter_mut().find(|i| i.id == draw.item.id)
                {
                    if let Some(next) = reanchor(
                        draw,
                        pose,
                        avatar.anchor(Some(&pin)),
                        pin,
                        scene,
                        canvas,
                        zoom,
                    ) {
                        *item = next;
                        changed = true;
                    } else {
                        self.message = Some("That anchor needs an offset outside the supported range. Choose a closer point.".into());
                    }
                }
            }
            if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
                self.anchor_drag = None;
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            return changed;
        }
        if self.unpin {
            self.unpin = false;
            if let Some(draw) = selected
                && let Some(pose) = draw.pose(scene, canvas, zoom)
                && let Some(item) = config.items.iter_mut().find(|i| i.id == draw.item.id)
            {
                let base = frame(scene, canvas, zoom, false);
                item.position = bounded(base.local(pose.center));
                item.rotation = degrees(pose.angle);
                item.height = (pose.size.y / base.scale).clamp(0.005, 4.0);
                item.pin = None;
                changed = true;
            } else if let Some(item) = config
                .items
                .iter_mut()
                .find(|i| Some(i.id) == self.selected)
            {
                item.pin = None;
                item.position = [0.0; 2];
                changed = true;
            }
        }
        let point = if self.pin_here {
            selected
                .and_then(|d| d.pose(scene, canvas, zoom))
                .map(|p| p.center)
        } else if self.pick_pin && response.clicked() {
            response.interact_pointer_pos()
        } else {
            None
        };
        self.pin_here = false;
        if let Some(point) = point {
            let base = frame(scene, canvas, zoom, avatar.is_puppet());
            let pin = avatar.pick(base.local(point));
            if let Some(pin) = pin
                && let Some(item) = config
                    .items
                    .iter_mut()
                    .find(|i| Some(i.id) == self.selected)
            {
                if let Some(pose) = selected.and_then(|d| d.pose(scene, canvas, zoom)) {
                    let angle = if avatar.is_puppet() && item.follow_rotation {
                        frame(scene, canvas, zoom, true).angle
                    } else {
                        0.0
                    };
                    item.rotation = degrees(pose.angle - angle);
                    let moving = frame(scene, canvas, zoom, true);
                    let stretch = if avatar.is_puppet() && item.follow_scale {
                        (moving.stretch.x * moving.stretch.y)
                            .abs()
                            .sqrt()
                            .max(0.001)
                    } else {
                        1.0
                    };
                    item.height = (pose.size.y / frame(scene, canvas, zoom, false).scale / stretch)
                        .clamp(0.005, 4.0);
                }
                item.pin = Some(pin);
                item.position = [0.0; 2];
                self.pick_pin = false;
                changed = true;
                self.message = Some("Pinned. Movement follows the selected surface. Drag the object to adjust its offset.".into());
            } else {
                self.message = Some(
                    "No visible model triangle at that point. Click a solid part of the avatar."
                        .into(),
                );
            }
        } else if !self.pick_pin {
            if (response.clicked() || response.drag_started())
                && let Some(pos) = if response.drag_started() {
                    ui.input(|i| i.pointer.press_origin())
                } else {
                    response.interact_pointer_pos()
                }
            {
                let hit = [false, true].into_iter().find_map(|behind| {
                    scene.items.iter().rev().find(|d| {
                        d.item.behind == behind
                            && d.visible
                            && d.pose(scene, canvas, zoom)
                                .is_some_and(|p| p.opacity > 0.01 && p.contains(pos))
                    })
                });
                if let Some(draw) = hit {
                    self.selected = Some(draw.item.id);
                    if response.drag_started() && !draw.item.locked {
                        self.drag = Some((draw.item.id, pos, draw.item.position));
                    }
                } else if response.clicked() {
                    self.selected = None;
                }
            }
            if response.dragged()
                && let Some((id, start, position)) = self.drag
                && let Some(pos) = response.interact_pointer_pos()
                && let Some(draw) = scene.items.iter().find(|d| d.item.id == id)
                && let Some(pose) = draw.pose(scene, canvas, zoom)
                && let Some(item) = config.items.iter_mut().find(|i| i.id == id)
                && !item.locked
            {
                item.position = bounded(
                    vec2(position[0], position[1])
                        + rotate(pos - start, -pose.offset_angle) / pose.offset_scale,
                );
                changed = true;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pick_pin = false;
            self.drag = None;
        }
        if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
            self.drag = None;
        }
        if self.pick_pin {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        changed
    }
    pub fn selection(&self, painter: &egui::Painter, scene: &Scene, canvas: Rect, zoom: f32) {
        if self.edit_pin {
            painter.text(
                canvas.left_top() + vec2(12., 12.),
                egui::Align2::LEFT_TOP,
                "Move anchor: drag the circle or click a new surface • Esc finishes",
                egui::FontId::proportional(13.),
                crate::theme::mint(),
            );
            if let Some(draw) = scene
                .items
                .iter()
                .find(|d| Some(d.item.id) == self.selected)
                && let Some(marker) = anchor_marker(draw, scene, canvas, zoom)
            {
                painter.circle_filled(marker, 7., Color32::from_black_alpha(190));
                painter.circle_stroke(marker, 9., egui::Stroke::new(2., crate::theme::mint()));
                if let Some(pose) = draw.pose(scene, canvas, zoom) {
                    painter.line_segment(
                        [marker, pose.center],
                        egui::Stroke::new(1., crate::theme::mint()),
                    );
                }
            }
        }
        if self.pick_pin {
            painter.text(
                canvas.left_top() + vec2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                "Click the avatar to pin • Esc cancels",
                egui::FontId::proportional(13.0),
                crate::theme::mint(),
            );
        }
        if let Some(draw) = scene
            .items
            .iter()
            .find(|d| Some(d.item.id) == self.selected && d.visible)
            && let Some(pose) = draw.pose(scene, canvas, zoom)
        {
            let points = pose.corners();
            for n in 0..4 {
                painter.line_segment(
                    [points[n], points[(n + 1) % 4]],
                    egui::Stroke::new(1.0_f32, crate::theme::mint()),
                );
            }
            painter.text(
                points[0] - vec2(0.0, 4.0),
                egui::Align2::LEFT_BOTTOM,
                &draw.item.name,
                egui::FontId::proportional(11.0),
                crate::theme::mint(),
            );
        }
    }
    pub fn drop_position(scene: &Scene, canvas: Rect, zoom: f32, point: Option<Pos2>) -> [f32; 2] {
        bounded(
            frame(scene, canvas, zoom, false).local(
                point
                    .filter(|p| canvas.contains(*p))
                    .unwrap_or(canvas.center()),
            ),
        )
    }
}
fn anchor_marker(draw: &DrawItem, scene: &Scene, canvas: Rect, zoom: f32) -> Option<Pos2> {
    let mut marker = draw.clone();
    marker.item.position = [0.; 2];
    marker.pose(scene, canvas, zoom).map(|p| p.center)
}
/// Change the attachment surface while preserving the rendered center, angle and size.
fn reanchor(
    draw: &DrawItem,
    old: Placement,
    anchor: Anchor,
    pin: Pin,
    scene: &Scene,
    canvas: Rect,
    zoom: f32,
) -> Option<Item> {
    let mut next = draw.clone();
    next.anchor = anchor;
    next.item.pin = Some(pin);
    next.item.position = [0.; 2];
    let at = next.pose(scene, canvas, zoom)?;
    let offset = rotate(old.center - at.center, -at.offset_angle) / at.offset_scale;
    let height = old.size.y / at.offset_scale;
    if !offset.is_finite() || offset.abs().max_elem() > 4. || !(0.005..=4.).contains(&height) {
        return None;
    }
    next.item.position = [offset.x, offset.y];
    next.item.rotation = degrees(old.angle - at.offset_angle);
    next.item.height = height;
    Some(next.item)
}
fn bounded(v: Vec2) -> [f32; 2] {
    [v.x.clamp(-4.0, 4.0), v.y.clamp(-4.0, 4.0)]
}
fn degrees(angle: f32) -> f32 {
    (angle.to_degrees() + 180.0).rem_euclid(360.0) - 180.0
}
pub fn is_png(path: &Path) -> bool {
    path.extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png") || s.eq_ignore_ascii_case("gif"))
}
pub fn is_item(path: &Path) -> bool {
    is_png(path) || aria_core::items::is_model(path)
}
pub fn signal(item: &Item, inputs: &Inputs, parameters: &[RigParameter]) -> Option<f32> {
    match item.rule.kind {
        SignalKind::Tracking => inputs.get(&item.rule.source).copied(),
        SignalKind::Parameter => parameters
            .iter()
            .find(|p| p.id == item.rule.source)
            .map(|p| p.value),
    }
}
pub(crate) fn load_png(
    ctx: &egui::Context,
    state: Option<&eframe::egui_wgpu::RenderState>,
    path: &Path,
    used: u64,
) -> Result<Sprite> {
    crate::media::load(ctx, state, path, used, false)
}

fn model_point(canvas: aria_live2d::Canvas, point: [f32; 2]) -> Vec2 {
    vec2(
        (point[0] * canvas.pixels_per_unit + canvas.origin[0] - canvas.size[0] * 0.5)
            / canvas.size[1],
        0.5 - (point[1] * canvas.pixels_per_unit + canvas.origin[1]) / canvas.size[1],
    )
}
pub fn anchor(pin: Option<&Pin>, avatar: Option<&Avatar>) -> Anchor {
    match pin {
        None => Anchor::Free,
        Some(Pin::Puppet { point }) if avatar.is_none() => Anchor::Puppet(vec2(point[0], point[1])),
        Some(Pin::Surface {
            mesh,
            vertices,
            weights,
            angle,
            length,
        }) => {
            let Some(avatar) = avatar else {
                return Anchor::Missing;
            };
            let mut resolved = resolve_surface(
                avatar.view_canvas(),
                &avatar.model.drawables,
                *mesh,
                *vertices,
                *weights,
                *angle,
                *length,
            );
            if let Anchor::Surface { opacity, .. } = &mut resolved {
                *opacity *= avatar.layer_opacity(*mesh);
            }
            resolved
        }
        _ => Anchor::Missing,
    }
}
fn resolve_surface(
    canvas: aria_live2d::Canvas,
    drawables: &[aria_live2d::Drawable],
    mesh: usize,
    vertices: [u16; 3],
    weights: [f32; 3],
    angle: f32,
    length: f32,
) -> Anchor {
    let Some(d) = drawables.get(mesh) else {
        return Anchor::Missing;
    };
    let mut p = [Vec2::ZERO; 3];
    for n in 0..3 {
        let Some(&v) = d.positions.get(usize::from(vertices[n])) else {
            return Anchor::Missing;
        };
        p[n] = model_point(canvas, v);
    }
    let edge = p[1] - p[0];
    if !edge.is_finite() || edge.length() < 1e-7 || length <= 1e-7 {
        return Anchor::Missing;
    }
    Anchor::Surface {
        point: p[0] * weights[0] + p[1] * weights[1] + p[2] * weights[2],
        angle: edge.y.atan2(edge.x) - angle,
        scale: (edge.length() / length).clamp(0.25, 4.0),
        opacity: if d.visible {
            d.opacity.clamp(0.0, 1.0)
        } else {
            0.0
        },
    }
}
pub(crate) fn barycentric(point: Vec2, p: [Vec2; 3]) -> Option<[f32; 3]> {
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    let a = p[1] - p[0];
    let b = p[2] - p[0];
    let q = point - p[0];
    let det = cross(a, b);
    if det.abs() < 1e-10 {
        return None;
    }
    let v = cross(q, b) / det;
    let w = cross(a, q) / det;
    let weights = [1.0 - v - w, v, w];
    weights
        .iter()
        .all(|v| v.is_finite() && (-1e-6..=1.000001).contains(v))
        .then_some(weights)
}
pub fn pick_surface(
    canvas: aria_live2d::Canvas,
    drawables: &[aria_live2d::Drawable],
    point: Vec2,
) -> Option<Pin> {
    let mut best: Option<((i32, usize), Pin)> = None;
    for (mesh, d) in drawables
        .iter()
        .enumerate()
        .filter(|(_, d)| d.visible && d.opacity > 0.01)
    {
        if best
            .as_ref()
            .is_some_and(|(order, _)| *order > (d.order, mesh))
        {
            continue;
        }
        for tri in d.indices.as_chunks::<3>().0 {
            let mut vertices = *tri;
            let [Some(a), Some(b), Some(c)] = vertices.map(|v| {
                d.positions
                    .get(usize::from(v))
                    .copied()
                    .map(|p| model_point(canvas, p))
            }) else {
                continue;
            };
            let mut p = [a, b, c];
            // Use the longest triangle edge as the orientation reference.
            let longest = (0..3)
                .max_by(|&a, &b| {
                    (p[(a + 1) % 3] - p[a])
                        .length_sq()
                        .total_cmp(&(p[(b + 1) % 3] - p[b]).length_sq())
                })
                .unwrap_or(0);
            vertices.rotate_left(longest);
            p.rotate_left(longest);
            if let Some(weights) = barycentric(point, [p[0], p[1], p[2]]) {
                let edge = p[1] - p[0];
                best = Some((
                    (d.order, mesh),
                    Pin::Surface {
                        mesh,
                        vertices,
                        weights,
                        angle: edge.y.atan2(edge.x),
                        length: edge.length(),
                    },
                ));
                break;
            }
        }
    }
    best.map(|(_, pin)| pin)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reanchoring_preserves_world_placement_across_transforms_and_canvases() {
        let ctx = egui::Context::default();
        let draw = DrawItem {
            deformation: None,
            tint: Color32::WHITE,
            item: Item {
                position: [0.13, -0.08],
                rotation: 37.,
                height: 0.23,
                follow_scale: true,
                ..Default::default()
            },
            image: ItemImage::Png(sprite(&ctx)),
            anchor: Anchor::Surface {
                point: vec2(-0.2, 0.1),
                angle: 0.5,
                scale: 1.3,
                opacity: 1.,
            },
            visible: true,
        };
        let scene = Scene {
            images: Default::default(),
            dents: Default::default(),
            effects: Default::default(),
            recoil: [0.; 2],
            _model_lease: None,
            model: None,
            model_bounds: Rect::NOTHING,
            sprite: None,
            params: aria_core::Parameters::default(),
            items: Default::default(),
        };
        for size in [vec2(1920., 1080.), vec2(1080., 1920.), vec2(512., 384.)] {
            for zoom in [0.6, 1.5] {
                let canvas = Rect::from_min_size(egui::pos2(27., 40.), size);
                let old = draw.pose(&scene, canvas, zoom).unwrap();
                for anchor in [
                    Anchor::Surface {
                        point: vec2(0.15, -0.22),
                        angle: -0.8,
                        scale: 0.7,
                        opacity: 1.,
                    },
                    Anchor::Puppet(vec2(-0.1, 0.2)),
                ] {
                    let pin = Pin::Puppet { point: [0.; 2] };
                    let item = reanchor(&draw, old, anchor, pin, &scene, canvas, zoom).unwrap();
                    let next = DrawItem {
                        item,
                        anchor,
                        ..draw.clone()
                    }
                    .pose(&scene, canvas, zoom)
                    .unwrap();
                    assert!((next.center - old.center).length() < 0.001);
                    assert!((next.size - old.size).length() < 0.001);
                    assert!((next.angle - old.angle).abs() < 0.00001);
                }
                assert!(
                    reanchor(
                        &draw,
                        old,
                        Anchor::Missing,
                        Pin::Puppet { point: [0.; 2] },
                        &scene,
                        canvas,
                        zoom
                    )
                    .is_none()
                );
            }
        }
    }
    use super::*;
    #[test]
    fn gif_pins_match_rendered_artwork_during_motion_and_drag_in_all_canvases() {
        let ctx = egui::Context::default();
        let mut artwork = sprite(&ctx);
        artwork.size = vec2(800., 1200.);
        let point = vec2(0.2 * 800. / 1200., -0.2);
        let item = Item {
            path: "badge.png".into(),
            pin: Some(Pin::Puppet {
                point: [point.x, point.y],
            }),
            height: 0.1,
            follow_scale: true,
            ..Default::default()
        };
        let draw = DrawItem {
            deformation: None,
            tint: Color32::WHITE,
            item: item.clone(),
            image: ItemImage::Png(sprite(&ctx)),
            anchor: Anchor::Puppet(point),
            visible: true,
        };
        let mut params = aria_core::Parameters::default();
        params.0[0] = -20.;
        params.0[1] = 17.;
        params.0[2] = -14.;
        let motion = aria_core::image_actions::Transform {
            offset: [0.1, -0.1],
            scale: [0.8, 1.2],
            rotation: 0.3,
            opacity: 0.8,
        };
        let scene = Scene {
            images: vec![crate::image_actions::Draw {
                sprite: artwork.clone(),
                transform: motion,
                opacity: 0.7,
            }]
            .into(),
            dents: Default::default(),
            effects: Default::default(),
            recoil: [0.; 2],
            _model_lease: None,
            model: None,
            model_bounds: Rect::NOTHING,
            sprite: Some(artwork),
            params,
            items: vec![draw].into(),
        };
        for size in [vec2(1920., 1080.), vec2(1080., 1920.), vec2(750., 610.)] {
            let canvas = Rect::from_min_size(Pos2::ZERO, size);
            let pose = scene.items[0].pose(&scene, canvas, 0.9).unwrap();
            let out = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(canvas),
                    ..Default::default()
                },
                |ctx| {
                    crate::image_actions::paint(
                        &ctx.layer_painter(egui::LayerId::background()),
                        canvas,
                        params,
                        0.9,
                        &scene.images,
                        &[],
                    );
                },
            );
            let mesh = out
                .shapes
                .iter()
                .find_map(|s| {
                    if let egui::Shape::Mesh(m) = &s.shape {
                        Some(m)
                    } else {
                        None
                    }
                })
                .unwrap();
            let origin = mesh.vertices[0].pos;
            let expected = origin
                + (mesh.vertices[1].pos - origin) * 0.7
                + (mesh.vertices[2].pos - origin) * 0.3;
            assert!(
                (pose.center - expected).length() < 0.001,
                "GIF pin slipped away from rendered artwork"
            );
            assert!((pose.opacity - 0.56).abs() < 1e-6);
        }
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(800., 800.));
        let mut config = RigConfig {
            items: vec![item],
            ..Default::default()
        };
        let mut manager = Items::default();
        manager.assets.insert("badge.png".into(), Ok(sprite(&ctx)));
        let start = scene.items[0].pose(&scene, canvas, 1.).unwrap().center;
        let end = start + vec2(80., 31.);
        let mut time = 1.0;
        let mut run = |events: Vec<egui::Event>| {
            time += 0.1;
            let _ = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(canvas),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        manager.refresh(&config, None);
                        let scene = Scene {
                            items: manager.draws.clone(),
                            ..scene.clone()
                        };
                        manager.stage(ui, canvas, &scene, &mut config, None, 1.);
                    });
                },
            );
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        run(vec![egui::Event::PointerMoved(start)]);
        run(vec![button(start, true)]);
        run(vec![egui::Event::PointerMoved(end)]);
        run(vec![button(end, false)]);
        manager.refresh(&config, None);
        let after = manager.draws[0].pose(&scene, canvas, 1.).unwrap();
        assert!(
            (after.center - end).length() < 0.001,
            "Pinned GIF accessory must remain draggable"
        );
        assert!(config.items[0].pin.is_some());
    }
    #[test]
    fn png_assets_validate_format_and_budget_cache_failures_and_reload_without_losing_other_items()
    {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.png");
        let missing = dir.path().join("missing.png");
        image::save_buffer(&good, &[255; 4 * 4 * 4], 4, 4, image::ColorType::Rgba8).unwrap();
        let ctx = egui::Context::default();
        assert!(load_png(&ctx, None, &good, aria_core::asset_limits::IMAGE_COLLECTION).is_err());
        let invalid = dir.path().join("invalid.png");
        std::fs::write(&invalid, "not an image").unwrap();
        assert!(load_png(&ctx, None, &invalid, 0).is_err());
        let mut manager = Items::default();
        let mut config = RigConfig::default();
        manager.add(vec![good.clone(), missing.clone()], &mut config, [0.0; 2]);
        manager.sync_assets(&ctx, None, &config);
        manager.refresh(&config, None);
        assert_eq!(manager.draws.len(), 1);
        assert!(manager.error(&missing).is_some());
        image::save_buffer(&missing, &[255; 4 * 4 * 4], 4, 4, image::ColorType::Rgba8).unwrap();
        manager.sync_assets(&ctx, None, &config);
        assert!(
            manager.error(&missing).is_some(),
            "No per-frame file polling"
        );
        manager.reload();
        manager.sync_assets(&ctx, None, &config);
        manager.refresh(&config, None);
        assert_eq!(manager.draws.len(), 2);
        let revision = manager.revision;
        manager.refresh(&config, None);
        assert_eq!(
            manager.revision, revision,
            "Static attachments reuse their output frame"
        );
    }
    fn sprite(ctx: &egui::Context) -> Sprite {
        Sprite {
            animation: None,
            texture: ctx.load_texture(
                "test item",
                egui::ColorImage::filled([8, 8], Color32::WHITE),
                Default::default(),
            ),
            name: "test".into(),
            size: vec2(8.0, 8.0),
            model_key: String::new(),
            palette: Default::default(),
        }
    }
    #[test]
    fn stage_drag_handles_fast_pointer_movement_lock_and_preserves_pin_when_unpinning() {
        let ctx = egui::Context::default();
        let canvas = Rect::from_min_size(Pos2::ZERO, vec2(480.0, 560.0));
        let sprite = sprite(&ctx);
        let item = Item {
            path: "test.png".into(),
            pin: Some(Pin::Puppet { point: [0.0; 2] }),
            height: 0.1,
            ..Default::default()
        };
        let mut config = RigConfig {
            items: vec![item],
            ..Default::default()
        };
        let mut manager = Items::default();
        manager.assets.insert("test.png".into(), Ok(sprite));
        let scene_base = Scene {
            images: Default::default(),
            dents: Default::default(),
            effects: Arc::from([]),
            recoil: [0.0; 2],
            _model_lease: None,
            model: None,
            model_bounds: Rect::NOTHING,
            sprite: None,
            params: aria_core::Parameters::default(),
            items: Default::default(),
        };
        let center = frame(&scene_base, canvas, 1.0, true).origin;
        let mut time = 0.0;
        let mut run = |events, manager: &mut Items, config: &mut RigConfig| {
            time += 0.1;
            let _ = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(canvas),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        manager.refresh(config, None);
                        let scene = Scene {
                            items: manager.draws.clone(),
                            ..scene_base.clone()
                        };
                        manager.stage(ui, canvas, &scene, config, None, 1.0);
                    });
                },
            );
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        run(
            vec![egui::Event::PointerMoved(center)],
            &mut manager,
            &mut config,
        );
        run(vec![button(center, true)], &mut manager, &mut config);
        let target = center + vec2(120.0, -56.0);
        run(
            vec![egui::Event::PointerMoved(target)],
            &mut manager,
            &mut config,
        );
        run(vec![button(target, false)], &mut manager, &mut config);
        assert!((config.items[0].position[0] - 120.0 / 560.0).abs() < 1e-5);
        assert!((config.items[0].position[1] + 0.1).abs() < 1e-5);
        assert!(config.items[0].pin.is_some());
        // Dragging the anchor moves the surface reference, not the accessory.
        manager.edit_pin = true;
        let before_pin = config.items[0].pin.clone();
        let before_draw = manager.draws[0].clone();
        let before_pose = before_draw.pose(&scene_base, canvas, 1.).unwrap();
        run(
            vec![egui::Event::PointerMoved(center)],
            &mut manager,
            &mut config,
        );
        run(vec![button(center, true)], &mut manager, &mut config);
        let next_anchor = center + vec2(-110., 90.);
        run(
            vec![egui::Event::PointerMoved(next_anchor)],
            &mut manager,
            &mut config,
        );
        run(vec![button(next_anchor, false)], &mut manager, &mut config);
        assert_ne!(config.items[0].pin, before_pin);
        let after_pose = manager.draws[0].pose(&scene_base, canvas, 1.).unwrap();
        assert!((before_pose.center - after_pose.center).length() < 0.001);
        assert!((before_pose.size - after_pose.size).length() < 0.001);
        assert!((before_pose.angle - after_pose.angle).abs() < 0.00001);
        manager.edit_pin = false;
        config.items[0].locked = true;
        let position = config.items[0].position;
        run(vec![button(target, true)], &mut manager, &mut config);
        run(
            vec![egui::Event::PointerMoved(center)],
            &mut manager,
            &mut config,
        );
        run(vec![button(center, false)], &mut manager, &mut config);
        assert_eq!(config.items[0].position, position);
        manager.unpin = true;
        run(vec![], &mut manager, &mut config);
        assert!(config.items[0].pin.is_none());
        let unpinned = manager.draws[0].clone();
        let unpinned = DrawItem {
            item: config.items[0].clone(),
            anchor: Anchor::Free,
            ..unpinned
        };
        assert!(
            (unpinned.pose(&scene_base, canvas, 1.).unwrap().center - before_pose.center).length()
                < 0.001
        );
    }
    #[test]
    fn frozen_gate_survives_pose_preset_storage_and_manual_toggles_still_work() {
        let parameters = aria_core::movement::preview_parameters(aria_core::Parameters::default());
        let mut config = RigConfig::from_parameters(&parameters);
        config.items.push(Item {
            path: "badge.png".into(),
            rule: aria_core::items::InputRule {
                mode: RuleMode::WhileInRange,
                ..Default::default()
            },
            ..Default::default()
        });
        let mut manager = Items::default();
        manager.evaluate(
            &mut config,
            &Inputs::from([("MouthOpen".into(), 0.7)]),
            &parameters,
        );
        assert!(config.items[0].condition);
        config.capture_pose(&parameters);
        let mut restored: RigConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        restored.validate(&parameters).unwrap();
        manager.reset();
        manager.evaluate(&mut restored, &Inputs::new(), &parameters);
        assert!(restored.items[0].condition);
        restored.items[0].visible = false;
        manager.evaluate(&mut restored, &Inputs::new(), &parameters);
        assert!(!restored.items[0].visible);
        restored.pose.mode = aria_core::movement::PoseMode::Live;
        manager.evaluate(&mut restored, &Inputs::new(), &parameters);
        assert!(!restored.items[0].condition);
    }
    #[test]
    fn output_item_positions_and_sizes_scale_with_each_canvas_without_ui_shapes() {
        let ctx = egui::Context::default();
        let draw = DrawItem {
            deformation: None,
            tint: Color32::WHITE,
            item: Item {
                height: 0.2,
                position: [0.1, -0.2],
                ..Default::default()
            },
            image: ItemImage::Png(sprite(&ctx)),
            anchor: Anchor::Free,
            visible: true,
        };
        let scene = Scene {
            images: Default::default(),
            dents: Default::default(),
            effects: Arc::from([]),
            recoil: [0.0; 2],
            _model_lease: None,
            model: None,
            model_bounds: Rect::NOTHING,
            sprite: None,
            params: aria_core::Parameters::default(),
            items: vec![draw].into(),
        };
        for size in [
            vec2(1920.0, 1080.0),
            vec2(1080.0, 1920.0),
            vec2(1536.0, 1024.0),
        ] {
            let large = Rect::from_min_size(Pos2::ZERO, size);
            let small = Rect::from_min_size(Pos2::ZERO, size / 4.0);
            let a = scene.items[0].pose(&scene, large, 1.0).unwrap();
            let b = scene.items[0].pose(&scene, small, 1.0).unwrap();
            assert!((a.center.to_vec2() - b.center.to_vec2() * 4.0).length() < 1e-3);
            assert!((a.size - b.size * 4.0).length() < 1e-3);
            let output = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(large),
                    ..Default::default()
                },
                |ctx| {
                    scene.paint(
                        &ctx.layer_painter(egui::LayerId::background()),
                        large,
                        &crate::output::CanvasSettings::default(),
                    );
                },
            );
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|s| matches!(s.shape, egui::Shape::Text(_)))
            );
            assert!(output.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Mesh(m) if m.texture_id == scene.items[0].image.id())));
        }
    }
    #[test]
    fn surface_pin_tracks_translation_rotation_and_stretch_and_rejects_missing_mesh() {
        let canvas = aria_live2d::Canvas {
            size: [100.0, 100.0],
            origin: [50.0, 50.0],
            pixels_per_unit: 100.0,
        };
        let mut d = aria_live2d::Drawable {
            positions: vec![[0.0, 0.0], [0.4, 0.0], [0.0, 0.4]],
            indices: vec![0, 1, 2],
            visible: true,
            opacity: 1.0,
            ..Default::default()
        };
        let Pin::Surface {
            mesh,
            vertices,
            weights,
            angle,
            length,
        } = pick_surface(canvas, &[d.clone()], vec2(0.1, -0.1)).unwrap()
        else {
            panic!()
        };
        for p in &mut d.positions {
            let v = rotate(vec2(p[0], p[1]), 0.4) * 1.5 + vec2(0.2, -0.1);
            *p = [v.x, v.y];
        }
        let Anchor::Surface {
            point,
            angle: a,
            scale,
            ..
        } = resolve_surface(canvas, &[d], mesh, vertices, weights, angle, length)
        else {
            panic!()
        };
        let expected = rotate(vec2(0.1, 0.1), 0.4) * 1.5 + vec2(0.2, -0.1);
        assert!((point - vec2(expected.x, -expected.y)).length() < 1e-5);
        assert!((a + 0.4).abs() < 1e-5);
        assert!((scale - 1.5).abs() < 1e-5);
        assert_eq!(
            resolve_surface(canvas, &[], mesh, vertices, weights, angle, length),
            Anchor::Missing
        );
    }
    #[test]
    fn picking_uses_frontmost_mesh_and_handles_degenerate_geometry() {
        let canvas = aria_live2d::Canvas {
            size: [1.0, 1.0],
            origin: [0.5, 0.5],
            pixels_per_unit: 1.0,
        };
        let d = aria_live2d::Drawable {
            positions: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            indices: vec![0, 1, 2],
            visible: true,
            opacity: 1.0,
            ..Default::default()
        };
        let front = aria_live2d::Drawable {
            order: 10,
            ..d.clone()
        };
        assert!(matches!(
            pick_surface(canvas, &[d, front], vec2(0.2, -0.2)),
            Some(Pin::Surface { mesh: 1, .. })
        ));
        assert!(barycentric(Vec2::ZERO, [Vec2::ZERO; 3]).is_none());
    }
}
