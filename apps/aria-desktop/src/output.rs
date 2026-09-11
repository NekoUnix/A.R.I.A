//! Independent OBS canvases. Shared layout state lets either native viewport drag
//! without depending on the main window's repaint timing.
use crate::{
    avatar::{self, Sprite},
    chroma,
    cubism_render::ModelImage,
    theme,
};
use aria_core::Parameters;
use eframe::egui::{self, Color32, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const TITLES: [&str; 2] = [
    "A.R.I.A. Output — Landscape 16:9",
    "A.R.I.A. Output — Portrait 9:16",
];
pub const NAMES: [&str; 2] = ["Landscape · 16:9", "Portrait · 9:16"];
pub fn viewport_id(index: usize) -> egui::ViewportId {
    egui::ViewportId::from_hash_of(if index == 0 {
        "aria-output-landscape"
    } else {
        "aria-output-portrait"
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Background {
    #[default]
    Studio,
    Green,
    Transparent,
}
impl Background {
    pub fn color(self, key: [u8; 3]) -> Color32 {
        match self {
            Self::Studio => theme::BG,
            Self::Green => Color32::from_rgb(key[0], key[1], key[2]),
            Self::Transparent => Color32::TRANSPARENT,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasSettings {
    pub background: Background,
    pub key: [u8; 3],
    /// Relative to canvas center, in canvas widths/heights. Independent of DPI.
    pub position: [f32; 2],
    pub zoom: f32,
    pub long_edge: u32,
    pub always_on_top: bool,
    pub locked: bool,
}
impl Default for CanvasSettings {
    fn default() -> Self {
        Self {
            background: Background::Studio,
            key: [0, 255, 0],
            position: [0.0; 2],
            zoom: 1.0,
            long_edge: 960,
            always_on_top: false,
            locked: false,
        }
    }
}
impl CanvasSettings {
    pub fn pixels(&self, index: usize) -> [u32; 2] {
        let size = [self.long_edge, self.long_edge * 9 / 16];
        if index == 0 { size } else { [size[1], size[0]] }
    }
    fn sanitize(&mut self) {
        if ![640, 960, 1280, 1920].contains(&self.long_edge) {
            self.long_edge = 960;
        }
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.25, 3.0)
        } else {
            1.0
        };
        for v in &mut self.position {
            *v = if v.is_finite() {
                v.clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutputSettings {
    pub canvases: [CanvasSettings; 2],
    pub selected: usize,
}
impl OutputSettings {
    pub fn from_legacy(background: Background, zoom: f32, always_on_top: bool) -> Self {
        Self {
            canvases: std::array::from_fn(|_| CanvasSettings {
                background,
                zoom,
                always_on_top,
                ..Default::default()
            }),
            selected: 0,
        }
        .sanitized()
    }
    pub fn sanitized(mut self) -> Self {
        self.selected = self.selected.min(1);
        for c in &mut self.canvases {
            c.sanitize();
        }
        self
    }
}
struct State {
    config: OutputSettings,
    open: [bool; 2],
    generation: u64,
    dirty: bool,
}
pub struct OutputWindows {
    state: Arc<Mutex<State>>,
    hex_draft: String,
    pub message: Option<String>,
}
impl OutputWindows {
    pub fn new(config: OutputSettings) -> Self {
        let config = config.sanitized();
        Self {
            hex_draft: chroma::hex(config.canvases[config.selected].key),
            state: Arc::new(Mutex::new(State {
                config,
                open: [false; 2],
                generation: 0,
                dirty: false,
            })),
            message: None,
        }
    }
    pub fn snapshot(&self) -> OutputSettings {
        self.state.lock().unwrap().config.clone()
    }
    pub fn reset(&mut self, config: OutputSettings) {
        let mut state = self.state.lock().unwrap();
        state.config = config.sanitized();
        state.generation = state.generation.wrapping_add(1);
        state.dirty = false;
        self.hex_draft = chroma::hex(state.config.canvases[state.config.selected].key);
        self.message = None;
    }
    pub fn take_dirty(&self) -> bool {
        std::mem::take(&mut self.state.lock().unwrap().dirty)
    }
    #[cfg(test)]
    pub fn is_open(&self, index: usize) -> bool {
        self.state.lock().unwrap().open[index]
    }
    #[cfg(any(test, feature = "screenshots"))]
    pub fn set_open(&self, index: usize, open: bool) {
        self.state.lock().unwrap().open[index] = open;
    }
    pub fn edit_canvas(&self, index: usize, edit: impl FnOnce(&mut CanvasSettings)) {
        let mut state = self.state.lock().unwrap();
        edit(&mut state.config.canvases[index]);
        state.config.canvases[index].sanitize();
        state.dirty = true;
    }
    pub fn apply_suggestion(
        &mut self,
        index: usize,
        suggestion: anyhow::Result<chroma::Suggestion>,
    ) {
        match suggestion {
            Ok(result) => {
                self.edit_canvas(index, |c| c.key = result.rgb);
                self.hex_draft = chroma::hex(result.rgb);
                self.message = Some(format!(
                    "Set {} from {} avatar color groups. {}",
                    self.hex_draft,
                    result.colors,
                    if result.distance < 0.08 {
                        "Similar colors remain in this artwork. Use a low OBS similarity and inspect carefully; no clearly separated key was found."
                    } else {
                        "Copy this hex into OBS's custom Chroma Key color. Check hair, eyes and clothing after adjusting similarity."
                    }
                ));
            }
            Err(error) => self.message = Some(format!("Color detection failed: {error:#}")),
        }
    }
    /// Returns which canvas requested an expensive color analysis after releasing
    /// the layout lock. The renderer is never read back on ordinary repaint frames.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        let mut detect = None;
        let mut state = self.state.lock().unwrap();
        ui.label("OBS capture windows");
        for (i, name) in NAMES.iter().enumerate() {
            ui.checkbox(&mut state.open[i], format!("Open {name}"));
        }
        theme::caption(
            ui,
            "Both windows can stay open. Each has its own framing and background, saved with this avatar.",
        );
        let selected = state.config.selected;
        egui::ComboBox::from_id_salt("edit-output")
            .selected_text(format!("Edit {}", NAMES[selected]))
            .show_ui(ui, |ui| {
                for (i, name) in NAMES.iter().enumerate() {
                    ui.selectable_value(&mut state.config.selected, i, *name);
                }
            });
        if selected != state.config.selected {
            self.hex_draft = chroma::hex(state.config.canvases[state.config.selected].key);
            self.message = None;
        }
        let selected = state.config.selected;
        let mut changed = false;
        let config = &mut state.config.canvases[selected];
        egui::ComboBox::from_id_salt("output-background")
            .selected_text(match config.background {
                Background::Studio => "Studio background",
                Background::Green => "Green screen / color key",
                Background::Transparent => "Transparent (experimental)",
            })
            .show_ui(ui, |ui| {
                for (mode, label) in [
                    (Background::Studio, "Studio background"),
                    (Background::Green, "Green screen / color key"),
                    (Background::Transparent, "Transparent (experimental)"),
                ] {
                    changed |= ui
                        .selectable_value(&mut config.background, mode, label)
                        .changed();
                }
            });
        if config.background == Background::Green {
            ui.horizontal(|ui| {
                if ui.color_edit_button_srgb(&mut config.key).changed() {
                    self.hex_draft = chroma::hex(config.key);
                    changed = true;
                }
                ui.add(
                    egui::TextEdit::singleline(&mut self.hex_draft)
                        .hint_text("#00FF00")
                        .char_limit(7)
                        .desired_width(82.0),
                );
                if ui.button("Apply hex").clicked() {
                    match chroma::parse_hex(&self.hex_draft) {
                        Ok(rgb) => {
                            config.key = rgb;
                            self.hex_draft = chroma::hex(rgb);
                            self.message = None;
                            changed = true;
                        }
                        Err(error) => self.message = Some(error.to_string()),
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Detect safer color").clicked() {
                    detect = Some(selected);
                }
                if ui.button("Copy hex").clicked() {
                    ui.ctx().copy_text(chroma::hex(config.key));
                }
            });
            theme::caption(
                ui,
                "OBS removes pixels similar to the key color. Choosing a color away from your avatar's colors helps preserve its artwork.",
            );
            ui.collapsing("How automatic color detection works", |ui| {
                theme::caption(ui, "Scans non-transparent atlas colors (including hidden artwork) and the current rendered model, then chooses a saturated color with the largest chroma separation. PNG avatars include both idle and talking artwork.");
                theme::caption(ui, "This is a best-fit suggestion, not a guarantee: translucent edges, later color effects and high OBS similarity can still remove detail. Recheck after changing expressions or artwork. In OBS, add a Chroma Key filter, choose Custom, paste this hex, and start with low similarity. Disable Capture Cursor to keep your pointer out of the recording.");
                ui.hyperlink_to("OBS Chroma Key guide ↗", "https://obsproject.com/kb/chroma-key-filter");
            });
        } else if config.background == Background::Transparent {
            theme::caption(
                ui,
                "Desktop transparency depends on the OBS capture method. If OBS shows black, use a color key and a Chroma Key filter instead.",
            );
        }
        if let Some(message) = &self.message {
            ui.label(egui::RichText::new(message).small().color(theme::MINT));
        }
        ui.collapsing("Framing & window size", |ui| {
            theme::caption(ui, "Click and drag the model inside this output window. Double-click it, or press Center model, to recenter. No controls are drawn over the capture.");
            let zoom = ui.add(egui::Slider::new(&mut config.zoom,0.25..=3.0).text("Model scale"));
            changed |= zoom.drag_stopped() || (zoom.changed() && !zoom.dragged());
            ui.horizontal(|ui| {
                if ui.button("Center model").clicked() { config.position = [0.0;2]; changed = true; }
                if ui.button("Reset framing").clicked() { config.position = [0.0;2]; config.zoom = 1.0; changed = true; }
            });
            changed |= ui.checkbox(&mut config.locked,"Lock model position").changed();
            changed |= ui.checkbox(&mut config.always_on_top,"Keep this output on top").changed();
            let size = config.pixels(selected);
            egui::ComboBox::from_id_salt("output-size").selected_text(format!("{} × {} pixels",size[0],size[1])).show_ui(ui, |ui| {
                for edge in [640,960,1280,1920] {
                    let dims = CanvasSettings { long_edge: edge, ..Default::default() }.pixels(selected);
                    changed |= ui.selectable_value(&mut config.long_edge,edge,format!("{} × {}",dims[0],dims[1])).changed();
                }
            });
            theme::caption(ui, "The canvas keeps its aspect ratio. Choose a smaller size if a portrait window is taller than your screen.");
        });
        if ui.button("Save output layouts").clicked() {
            changed = true;
            self.message = Some("Both output layouts saved for this avatar.".into());
        }
        state.dirty |= changed;
        detect
    }

    pub fn show(&self, ctx: &egui::Context, scene: Scene, fps: u32, _started: Instant) {
        for (index, title) in TITLES.iter().enumerate() {
            let state = self.state.lock().unwrap();
            if !state.open[index] {
                continue;
            }
            let config = state.config.canvases[index].clone();
            let generation = state.generation;
            drop(state);
            let pixels = config.pixels(index);
            let native_scale = ctx.input(|i| {
                i.raw
                    .viewports
                    .get(&viewport_id(index))
                    .and_then(|v| v.native_pixels_per_point)
            });
            let pixels_per_point =
                native_scale.map_or(ctx.pixels_per_point(), |scale| scale * ctx.zoom_factor());
            let size = egui::vec2(pixels[0] as f32, pixels[1] as f32) / pixels_per_point;
            let shared = self.state.clone();
            let scene = scene.clone();
            let interval = Duration::from_secs_f64(1.0 / f64::from(fps));
            ctx.show_viewport_deferred(
                viewport_id(index),
                egui::ViewportBuilder::default()
                    .with_title(*title)
                    .with_inner_size(size)
                    .with_resizable(false)
                    .with_maximize_button(false)
                    .with_transparent(true)
                    .with_position(egui::pos2(30.0 + index as f32 * 580.0, 40.0))
                    .with_window_level(if config.always_on_top {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    }),
                move |ctx, _| {
                    let began = Instant::now();
                    let mut state = shared.lock().unwrap();
                    // Ignore callbacks/drag state retained from an outgoing model.
                    if state.generation != generation {
                        return;
                    }
                    if ctx.input(|i| i.viewport().close_requested()) {
                        state.open[index] = false;
                        ctx.request_repaint_of(egui::ViewportId::ROOT);
                        return;
                    }
                    let config = &mut state.config.canvases[index];
                    let mut dirty = false;
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(config.background.color(config.key)))
                        .show(ctx, |ui| {
                            dirty = scene.canvas(ui, config, generation);
                        });
                    state.dirty |= dirty;
                    drop(state);
                    if dirty {
                        ctx.request_repaint_of(egui::ViewportId::ROOT);
                    }
                    #[cfg(feature = "screenshots")]
                    crate::screenshot::capture(ctx, _started, true);
                    let prediction =
                        Duration::from_secs_f32(ctx.input(|i| i.predicted_dt).max(0.0));
                    ctx.request_repaint_after(
                        interval.saturating_sub(began.elapsed()) + prediction,
                    );
                },
            );
        }
    }
}

#[derive(Clone)]
pub struct Scene {
    pub model: Option<ModelImage>,
    pub model_bounds: Rect,
    pub sprite: Option<Sprite>,
    pub params: Parameters,
}
impl Scene {
    fn canvas(&self, ui: &mut egui::Ui, config: &mut CanvasSettings, generation: u64) -> bool {
        let canvas = ui.max_rect();
        let translated =
            canvas.translate(egui::vec2(config.position[0], config.position[1]) * canvas.size());
        let bounds = if let Some(model) = self.model {
            let rect = model.rect(translated, config.zoom);
            Rect::from_min_max(
                rect.min + self.model_bounds.min.to_vec2() * rect.size(),
                rect.min + self.model_bounds.max.to_vec2() * rect.size(),
            )
        } else {
            avatar::bounds(translated, self.params, self.sprite.as_ref(), config.zoom)
        };
        let dirty = if config.locked || !bounds.is_finite() || !bounds.intersects(canvas) {
            false
        } else {
            drag_model(ui, canvas, bounds, config, generation)
        };
        let translated =
            canvas.translate(egui::vec2(config.position[0], config.position[1]) * canvas.size());
        if let Some(model) = self.model {
            model.draw(ui.painter(), translated, config.zoom);
        } else {
            avatar::draw(
                ui.painter(),
                translated,
                self.params,
                self.sprite.as_ref(),
                config.zoom,
            );
        }
        dirty
    }
}
fn moved_position(start: [f32; 2], delta: Vec2, size: Vec2) -> [f32; 2] {
    [
        (start[0] + delta.x / size.x.max(1.0)).clamp(-1.0, 1.0),
        (start[1] + delta.y / size.y.max(1.0)).clamp(-1.0, 1.0),
    ]
}
fn drag_model(
    ui: &mut egui::Ui,
    canvas: Rect,
    bounds: Rect,
    config: &mut CanvasSettings,
    generation: u64,
) -> bool {
    let id = ui
        .id()
        .with(("output-model-drag", ui.ctx().viewport_id(), generation));
    let response = ui
        .interact(bounds.intersect(canvas), id, egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::Grab);
    if response.double_clicked() {
        config.position = [0.0; 2];
        return true;
    }
    if response.drag_started_by(egui::PointerButton::Primary) && ui.ctx().current_pass_index() == 0
    {
        ui.data_mut(|d| d.insert_temp(id, config.position));
    }
    if response.dragged_by(egui::PointerButton::Primary) && ui.ctx().current_pass_index() == 0 {
        let start = ui
            .data(|d| d.get_temp::<[f32; 2]>(id))
            .unwrap_or(config.position);
        config.position = moved_position(
            start,
            response.total_drag_delta().unwrap_or_default(),
            canvas.size(),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    response.drag_stopped_by(egui::PointerButton::Primary)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_drag_moves_model_once_across_layout_passes_and_lock_prevents_it() {
        for size in [egui::vec2(960.0, 540.0), egui::vec2(540.0, 960.0)] {
            let ctx = egui::Context::default();
            let scene = Scene {
                model: None,
                model_bounds: Rect::NOTHING,
                sprite: None,
                params: Parameters::default(),
            };
            let mut config = CanvasSettings::default();
            let center = egui::pos2(size.x / 2.0, size.y / 2.0);
            let mut time = 0.0;
            let mut run = |events: Vec<egui::Event>, config: &mut CanvasSettings| {
                time += 0.1;
                let mut dirty = false;
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                        events,
                        time: Some(time),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(ctx, |ui| {
                                dirty |= scene.canvas(ui, config, 0);
                            });
                        if ctx.current_pass_index() == 0 {
                            ctx.request_discard("test a second layout pass");
                        }
                    },
                );
                dirty
            };
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            };
            run(vec![egui::Event::PointerMoved(center)], &mut config);
            run(vec![button(center, true)], &mut config);
            let moved = center + egui::vec2(size.x * 0.2, -size.y * 0.1);
            run(vec![egui::Event::PointerMoved(moved)], &mut config);
            run(vec![egui::Event::PointerMoved(moved)], &mut config);
            assert!(
                (config.position[0] - 0.2).abs() < 0.001,
                "{:?}",
                config.position
            );
            assert!((config.position[1] + 0.1).abs() < 0.001);
            assert!(run(vec![button(moved, false)], &mut config));
            config.locked = true;
            run(vec![button(moved, true)], &mut config);
            run(vec![egui::Event::PointerMoved(center)], &mut config);
            run(vec![button(center, false)], &mut config);
            assert!((config.position[0] - 0.2).abs() < 0.001);
        }
    }
    #[test]
    fn independent_canvases_restore_and_keep_aspect_and_normalized_drag() {
        let runtime = OutputWindows::new(OutputSettings::default());
        runtime.set_open(0, true);
        runtime.set_open(1, true);
        runtime.edit_canvas(0, |c| {
            c.position = moved_position(
                [0.0; 2],
                egui::vec2(240.0, -135.0),
                egui::vec2(960.0, 540.0),
            );
            c.key = [255, 0, 255];
            c.background = Background::Green;
        });
        runtime.edit_canvas(1, |c| {
            c.position = [-0.25, 0.4];
            c.background = Background::Transparent;
            c.zoom = 0.7;
        });
        let saved = runtime.snapshot();
        assert_eq!(saved.canvases[0].position, [0.25, -0.25]);
        assert_eq!(
            moved_position(
                [0.0; 2],
                egui::vec2(480.0, -270.0),
                egui::vec2(1920.0, 1080.0)
            ),
            saved.canvases[0].position
        );
        let json = serde_json::to_string(&saved).unwrap();
        let mut restored = OutputWindows::new(serde_json::from_str(&json).unwrap());
        assert_eq!(restored.snapshot(), saved);
        for i in 0..2 {
            for edge in [640, 960, 1280, 1920] {
                let c = CanvasSettings {
                    long_edge: edge,
                    ..Default::default()
                };
                let [w, h] = c.pixels(i);
                assert_eq!(
                    w * if i == 0 { 9 } else { 16 },
                    h * if i == 0 { 16 } else { 9 }
                );
            }
        }
        restored.reset(OutputSettings::default());
        assert_ne!(restored.snapshot(), saved);
        assert!(runtime.is_open(0) && runtime.is_open(1));
        runtime.set_open(0, false);
        assert!(runtime.is_open(1));
    }
    #[test]
    fn legacy_and_invalid_layouts_get_safe_defaults() {
        let legacy = OutputSettings::from_legacy(Background::Green, 1.3, true);
        assert!(
            legacy
                .canvases
                .iter()
                .all(|c| c.background == Background::Green && c.zoom == 1.3 && c.always_on_top)
        );
        let mut bad = OutputSettings {
            selected: 999,
            ..Default::default()
        };
        bad.canvases[1].position = [f32::NAN, 500.0];
        bad.canvases[1].zoom = f32::INFINITY;
        bad.canvases[1].long_edge = u32::MAX;
        let clean = bad.sanitized();
        assert_eq!(clean.selected, 1);
        assert_eq!(clean.canvases[1].position, [0.0, 1.0]);
        assert_eq!(clean.canvases[1].pixels(1), [540, 960]);
    }
}
