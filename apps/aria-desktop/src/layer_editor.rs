//! A mouse-first layer picker with its own frozen geometry and GPU target.
use crate::{
    cubism_render::ModelRenderer,
    layer_selection::{Operation, Projection, Selection},
    live2d::Avatar,
    theme,
};
use aria_core::layers::{Config, Group};
use eframe::egui::{self, Rect, Vec2};
use std::collections::BTreeSet;

pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("aria-frozen-layer-editor")
}

pub struct FrozenPreview {
    pub drawables: Vec<aria_live2d::Drawable>,
    pub renderer: ModelRenderer,
    canvas: aria_live2d::Canvas,
    rendered: Option<Config>,
}
impl FrozenPreview {
    pub fn capture(avatar: &Avatar, layers: &Config) -> anyhow::Result<Self> {
        let mut preview = Self {
            drawables: avatar.model.drawables.clone(),
            renderer: avatar.frozen_preview_renderer(),
            canvas: avatar.model.canvas,
            rendered: None,
        };
        preview.sync(layers)?;
        Ok(preview)
    }
    pub fn sync(&mut self, layers: &Config) -> anyhow::Result<()> {
        if self.rendered.as_ref() != Some(layers) {
            self.renderer
                .render_layers(self.canvas, &self.drawables, layers)?;
            self.rendered = Some(layers.clone());
        }
        Ok(())
    }
}

struct Edit {
    before: Config,
    after: Config,
}
#[derive(Default)]
struct History(Vec<Edit>);
impl History {
    fn remember(&mut self, before: Config, after: &Config) {
        if before == *after {
            return;
        }
        if self.0.len() == 24 {
            self.0.remove(0);
        }
        self.0.push(Edit {
            before,
            after: after.clone(),
        });
    }
    fn undo(&mut self, layers: &mut Config) -> Result<bool, &'static str> {
        let Some(edit) = self.0.last() else {
            return Ok(false);
        };
        if edit.after != *layers {
            return Err(
                "Layer settings changed elsewhere. Undo was skipped to preserve those changes.",
            );
        }
        *layers = self.0.pop().unwrap().before;
        Ok(true)
    }
}

pub struct Editor {
    pub requested: bool,
    open: bool,
    snapshot: Option<FrozenPreview>,
    model_key: String,
    selection: Selection,
    zoom: f32,
    pan: Vec2,
    reveal: bool,
    refresh: bool,
    history: History,
    group_name: String,
    pub message: Option<String>,
    pub manage_groups: bool,
}
impl Default for Editor {
    fn default() -> Self {
        let mut selection = Selection::default();
        selection.enabled = true;
        selection.operation = Operation::Add;
        Self {
            requested: false,
            open: false,
            snapshot: None,
            model_key: String::new(),
            selection,
            zoom: 1.,
            pan: Vec2::ZERO,
            reveal: false,
            refresh: false,
            history: History::default(),
            group_name: String::new(),
            message: None,
            manage_groups: false,
        }
    }
}
impl Editor {
    #[cfg(feature = "screenshots")]
    fn smoke(&self, ctx: &egui::Context, selected: &BTreeSet<String>, started: std::time::Instant) {
        if std::env::var("ARIA_SMOKE_SCENARIO").as_deref() == Ok("frozen-layer-editor") {
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("layer-smoke-selected"), selected.len()));
            let verified = egui::Id::new("frozen-layer-smoke-verified");
            if started.elapsed().as_secs() > 5
                && ctx.data(|d| {
                    d.get_temp::<u8>(egui::Id::new("frozen-layer-smoke-phase"))
                        .unwrap_or(0)
                }) > 10
                && !ctx.data(|d| d.get_temp::<bool>(verified).unwrap_or(false))
            {
                assert!(
                    selected.len() > 1,
                    "Native mouse boxes must select multiple layers"
                );
                eprintln!(
                    "Native frozen-window mouse clicks and boxes selected {} layers",
                    selected.len()
                );
                ctx.data_mut(|d| d.insert_temp(verified, true));
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        avatar: &Avatar,
        layers: &mut Config,
        selected: &mut BTreeSet<String>,
        next_group_id: u64,
        started: std::time::Instant,
    ) -> bool {
        // A model switch never keeps the previous avatar's geometry or edits.
        if self.model_key != avatar.model_key {
            let requested = self.requested;
            *self = Self::default();
            self.requested = requested;
            self.model_key.clone_from(&avatar.model_key);
        }
        // Keep the previous image lease alive through the frame that replaces it.
        let _previous_image = self.snapshot.as_ref().map(|s| s.renderer.lease.clone());
        if std::mem::take(&mut self.requested) {
            if !self.open {
                self.refresh = true;
                self.history = History::default();
            }
            self.open = true;
            ctx.send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::Focus);
        }
        if !self.open {
            self.selection.cancel(selected);
            self.snapshot = None;
            return false;
        }
        #[cfg(feature = "screenshots")]
        if crate::smoke_mode() {
            ctx.add_plugin(crate::screenshot::FrozenLayerInput);
        }
        if self.refresh || self.snapshot.is_none() {
            self.refresh = false;
            self.selection.cancel(selected);
            match FrozenPreview::capture(avatar, layers) {
                Ok(snapshot) => {
                    self.snapshot = Some(snapshot);
                    self.message = None;
                }
                Err(error) => {
                    let message = format!("Cannot capture the layer preview: {error:#}");
                    crate::diagnostics::record("error", "LAYER_EDITOR", &message);
                    self.message = Some(message);
                    self.open = false;
                    return false;
                }
            }
        }
        let mut changed = false;
        ctx.show_viewport_immediate(
            viewport_id(),
            egui::ViewportBuilder::default()
                .with_title("A.R.I.A. — Frozen Live2D layer selector")
                .with_inner_size([1000., 760.])
                .with_min_inner_size([720., 500.])
                .with_resizable(true),
            |root, _class| {
                if root.ctx().input(|i| i.viewport().close_requested()) {
                    self.open = false;
                    return;
                }
                changed |= self.content(root, avatar, layers, selected, next_group_id);
                #[cfg(feature = "screenshots")]
                if crate::smoke_mode() {
                    self.smoke(root.ctx(), selected, started);
                }
            },
        );
        let _ = started;
        if changed {
            ctx.request_repaint();
        }
        changed
    }

    fn content(
        &mut self,
        root: &mut egui::Ui,
        avatar: &Avatar,
        layers: &mut Config,
        selected: &mut BTreeSet<String>,
        next_group_id: u64,
    ) -> bool {
        let before = layers.clone();
        let mut undo = false;
        egui::Panel::top("frozen-layer-tools").show(root, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Select artwork on your model");
                crate::help::button(ui, "frozen-layer-editor");
                if ui.button("Done").clicked() { self.open = false; }
            });
            ui.label(format!("{} · Frozen preview · Your live stage keeps tracking", avatar.name));
            ui.horizontal_wrapped(|ui| {
                ui.label("Mouse selection:");
                for (mode, label) in [(Operation::Add,"Add"),(Operation::Remove,"Remove"),(Operation::Replace,"Replace"),(Operation::Toggle,"Toggle")] {
                    if ui.selectable_value(&mut self.selection.operation, mode, label).clicked() { self.selection.cancel(selected); }
                }
                if ui.button("Clear selection").clicked() { self.selection.cancel(selected); selected.clear(); }
            });
            ui.small("Click layers or drag boxes directly on the frozen model. Add keeps earlier selections. No keyboard keys needed.");
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!selected.is_empty(), egui::Button::new("Hide selected")).clicked() {
                    for id in selected.iter() { layers.opacity.insert(id.clone(), 0.); }
                }
                if ui.add_enabled(!selected.is_empty(), egui::Button::new("Restore selected")).clicked() {
                    for id in selected.iter() { layers.opacity.remove(id); }
                }
                undo = ui.add_enabled(!self.history.0.is_empty(), egui::Button::new("Undo")).clicked();
                if ui.button("Refresh frozen pose").clicked() { self.refresh = true; }
                if ui.button("Fit model").clicked() { self.zoom = 1.; self.pan = Vec2::ZERO; }
                ui.add(egui::Slider::new(&mut self.zoom, 0.25..=8.).logarithmic(true).text("Zoom"));
            });
            ui.checkbox(&mut self.reveal, "Reveal ARIA-hidden layers in this preview")
                .on_hover_text("Temporarily show layers hidden by ARIA opacity/groups, so you can select and restore them. This checkbox does not change the live model. Artwork hidden by the frozen pose's parameters stays hidden.");
        });
        egui::Panel::bottom("frozen-layer-status").show(root, |ui| {
            ui.small("Wheel: zoom · Right/middle drag: pan · Esc: cancel box · Visibility changes save to this avatar and apply to every output.");
            if let Some(message) = &self.message { ui.label(message); }
        });
        egui::Panel::left("frozen-layer-list").default_size(225.).size_range(190.0..=330.0).resizable(true).show(root, |ui| {
            ui.heading(format!("{} selected", selected.len()));
            ui.small("Uncheck a row to remove it from the selection.");
            let ids: Vec<_> = selected.iter().cloned().collect();
            egui::ScrollArea::vertical().id_salt("frozen-picked-list").max_height((ui.available_height() - 190.).max(100.))
                .show_rows(ui, ui.spacing().interact_size.y, ids.len(), |ui, rows| {
                    for id in &ids[rows] {
                        let mut keep = true;
                        let response = ui.checkbox(&mut keep, id);
                        if response.hovered() {
                            let part = self.snapshot.as_ref().and_then(|s| s.drawables.iter().find(|d| d.id == *id))
                                .and_then(|d| avatar.labels.get(&d.part));
                            response.clone().on_hover_text(part.map_or(id.as_str(), String::as_str));
                        }
                        if response.changed() && !keep {
                            selected.remove(id);
                        }
                    }
                });
            ui.separator();
            if layers.groups.iter().any(|g| g.active && !g.layers.is_disjoint(selected)) {
                ui.small("An active group can still hide restored layers.");
                if ui.button("Disable overlapping groups").on_hover_text("Disables entire active groups touching this selection, including their other members.").clicked() {
                    for group in &mut layers.groups { if !group.layers.is_disjoint(selected) { group.active = false; } }
                }
            }
            ui.add(egui::TextEdit::singleline(&mut self.group_name).hint_text("Group name").char_limit(60));
            if ui.add_enabled(!selected.is_empty() && !self.group_name.trim().is_empty() && self.group_name.len() <= 120 && layers.groups.len() < 128,
                egui::Button::new("Save selection as group")).clicked() {
                layers.groups.push(Group { id:next_group_id, name:self.group_name.trim().into(), layers:selected.clone(), opacity:0., active:false });
                self.message = Some("Group saved. Manage groups & hotkeys opens its shortcut settings.".into());
                self.group_name.clear();
            }
            if ui.button("Manage groups & hotkeys…").clicked() { self.manage_groups = true; }
            ui.small("Saved groups appear in this model's Layers inspector. Closing this window keeps visibility edits.");
        });
        if undo {
            match self.history.undo(layers) {
                Ok(_) => self.message = None,
                Err(message) => self.message = Some(message.into()),
            }
        } else {
            self.history.remember(before.clone(), layers);
        }
        let preview_layers = preview_config(layers, self.reveal);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::bg()).inner_margin(10.))
            .show(root, |ui| {
                let Some(snapshot) = &mut self.snapshot else {
                    return;
                };
                if let Err(error) = snapshot.sync(&preview_layers) {
                    self.message = Some(format!("Preview could not update: {error:#}"));
                    return;
                }
                let (area, _) = ui.allocate_exact_size(
                    ui.available_size().max(Vec2::splat(1.)),
                    egui::Sense::hover(),
                );
                let mut model_rect = snapshot
                    .renderer
                    .image
                    .rect(area.shrink(12.), self.zoom)
                    .translate(self.pan);
                let hovered = ui.rect_contains_pointer(area);
                if hovered {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                    if scroll != 0. {
                        let previous = self.zoom;
                        self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.25, 8.);
                        if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                            self.pan = pointer
                                - area.center()
                                - (pointer - model_rect.center()) * (self.zoom / previous);
                        }
                        ui.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
                        model_rect = snapshot
                            .renderer
                            .image
                            .rect(area.shrink(12.), self.zoom)
                            .translate(self.pan);
                    }
                }
                ui.painter_at(area).image(
                    snapshot.renderer.image.id,
                    model_rect,
                    Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                    egui::Color32::WHITE,
                );
                let projection = Projection {
                    canvas: snapshot.renderer.view_canvas,
                    rect: model_rect,
                    angle: 0.,
                    fields: vec![],
                };
                if let Some(response) = self.selection.stage(
                    ui,
                    area,
                    &snapshot.drawables,
                    &preview_layers,
                    &projection,
                    selected,
                ) && (response.dragged_by(egui::PointerButton::Secondary)
                    || response.dragged_by(egui::PointerButton::Middle))
                {
                    self.pan += ui.input(|i| i.pointer.delta());
                }
                #[cfg(feature = "screenshots")]
                if crate::smoke_mode() {
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(egui::Id::new("frozen-layer-smoke-area"), area)
                    });
                }
            });
        before != *layers
    }
}

fn preview_config(layers: &Config, reveal: bool) -> Config {
    let mut config = layers.clone();
    if reveal {
        config.model_opacity = 1.;
        config.opacity.clear();
        for group in &mut config.groups {
            group.active = false;
        }
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undo_restores_visibility_and_preserves_external_changes() {
        let mut layers = Config::default();
        let original = layers.clone();
        let mut history = History::default();
        layers.opacity.insert("Hair".into(), 0.);
        history.remember(original.clone(), &layers);
        assert!(history.undo(&mut layers).unwrap());
        assert_eq!(layers, original);
        layers.opacity.insert("Hair".into(), 0.);
        history.remember(original, &layers);
        layers.opacity.insert("Glasses".into(), 0.5);
        let external = layers.clone();
        assert!(history.undo(&mut layers).is_err());
        assert_eq!(layers, external);
    }
    #[test]
    fn revealing_hidden_artwork_changes_only_the_preview() {
        let mut layers = Config {
            model_opacity: 0.5,
            ..Default::default()
        };
        layers.opacity.insert("Hair".into(), 0.);
        layers.groups.push(Group {
            id: 1,
            name: "Outfit".into(),
            layers: BTreeSet::from(["Clothes".into()]),
            opacity: 0.,
            active: true,
        });
        layers.colors.insert(
            "Hair".into(),
            aria_core::layers::Colors {
                multiply: [0.5; 4],
                screen: [0.; 4],
            },
        );
        let preview = preview_config(&layers, true);
        assert_eq!(preview.opacity("Hair"), 1.);
        assert_eq!(preview.opacity("Clothes"), 1.);
        assert_eq!(preview.colors, layers.colors);
        assert_eq!(layers.opacity("Hair"), 0.);
        assert!(layers.groups[0].active);
        assert_eq!(preview_config(&layers, false), layers);
    }

    #[test]
    #[cfg(windows)]
    #[ignore = "requires ARIA_TEST_MODEL, ARIA_TEST_APPEARANCE_PARAMETER, Cubism Core and GPU"]
    fn native_frozen_preview_is_independent_of_live_tracking_and_visibility() {
        use aria_core::rig::Inputs;
        use std::path::Path;
        let state = crate::spout::tests::gpu_state();
        let path = std::env::var_os("ARIA_TEST_MODEL").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let mut avatar = Avatar::load(
            &state,
            Path::new(&core),
            aria_model::load_files(Path::new(&path)).unwrap(),
        )
        .unwrap();
        let mut config = avatar.initial_config.clone();
        config.physics.enabled = false;
        for binding in config.bindings.values_mut() {
            binding.smoothing_ms = 0.;
        }
        let mut expressions = crate::expressions_panel::ExpressionsPanel::default();
        let inputs =
            |value| Inputs::from([("ParamAngleX".into(), value), ("FaceAngleX".into(), value)]);
        avatar
            .update(&inputs(12.), &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        let mut frozen = FrozenPreview::capture(&avatar, &config.layers).unwrap();
        let pixels = frozen.renderer.read_rgba_for_test().unwrap();
        assert!(pixels.0.as_chunks::<4>().0.iter().any(|p| p[3] > 0));
        assert_eq!(
            frozen.renderer.atlas_mib, 0.,
            "Frozen previews share uploaded atlases"
        );
        let before_head = avatar
            .model
            .parameters()
            .iter()
            .find(|p| p.id == "ParamAngleX")
            .unwrap()
            .value;
        // Supplied rigs can use an angle parameter only as a physics input.
        // Drive a known authored artwork switch as well to guarantee changed pixels.
        let appearance_id = std::env::var("ARIA_TEST_APPEARANCE_PARAMETER").unwrap();
        let parameter = avatar
            .model
            .parameters()
            .iter()
            .find(|p| p.id == appearance_id)
            .unwrap();
        let target =
            if (parameter.value - parameter.min).abs() < (parameter.value - parameter.max).abs() {
                parameter.max
            } else {
                parameter.min
            };
        config.customization.values.insert(appearance_id, target);
        avatar
            .update(&inputs(-12.), &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        let after_head = avatar
            .model
            .parameters()
            .iter()
            .find(|p| p.id == "ParamAngleX")
            .unwrap()
            .value;
        assert!(
            (after_head - before_head).abs() > 1.,
            "Live head tracking keeps updating: {before_head} -> {after_head}"
        );
        assert!(
            pixels == frozen.renderer.read_rgba_for_test().unwrap(),
            "Live renders must not overwrite the frozen target"
        );
        let refreshed = FrozenPreview::capture(&avatar, &config.layers).unwrap();
        assert!(
            pixels != refreshed.renderer.read_rgba_for_test().unwrap(),
            "Refresh captures the new pose"
        );
        let folder = tempfile::tempdir().unwrap();
        let live = folder.path().join("live.png");
        avatar.save_png(&live).unwrap();
        let original_layers = config.layers.clone();
        for d in &frozen.drawables {
            config.layers.opacity.insert(d.id.clone(), 0.);
        }
        frozen.sync(&config.layers).unwrap();
        assert!(
            frozen
                .renderer
                .read_rgba_for_test()
                .unwrap()
                .0
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[3] == 0)
        );
        let unaffected = folder.path().join("unaffected.png");
        avatar.save_png(&unaffected).unwrap();
        assert!(
            image::open(&live).unwrap().into_rgba8()
                == image::open(&unaffected).unwrap().into_rgba8(),
            "Preview rendering never mutates the live render buffers"
        );
        avatar
            .update(&inputs(-12.), &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        let hidden = folder.path().join("hidden.png");
        avatar.save_png(&hidden).unwrap();
        assert!(
            image::open(&hidden)
                .unwrap()
                .into_rgba8()
                .pixels()
                .all(|p| p.0[3] == 0),
            "Visibility edits reach the live output on its next update"
        );
        config.layers = original_layers;
        frozen.sync(&config.layers).unwrap();
        assert!(
            pixels == frozen.renderer.read_rgba_for_test().unwrap(),
            "Restoring layers keeps the original frozen pose"
        );
        drop(refreshed);
        drop(frozen);
        avatar
            .update(&inputs(-12.), &mut config, &mut expressions, 1. / 60.)
            .unwrap();
        avatar.save_png(&unaffected).unwrap();
        assert!(
            image::open(&live).unwrap().into_rgba8()
                == image::open(&unaffected).unwrap().into_rgba8(),
            "Closing previews preserves the live texture and its atlases"
        );
        eprintln!(
            "Frozen preview passed: {} meshes; live tracking, refresh, hide, restore, close",
            avatar.model.drawables.len()
        );
    }
}
