//! EXPERIMENTAL: Review/apply/undo for a model's VTS sidecar. Source exports are read-only.
use aria_core::{
    motion::Motion,
    movement::{PoseMode, SavedRig},
    shortcuts::Shortcut,
    vts::{self, Action},
};
use aria_model::ExpressionFile;
#[cfg(test)]
mod tests;
use eframe::egui;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

struct Preview {
    path: PathBuf,
    imported: vts::Imported,
    expressions: Vec<ExpressionFile>,
}
#[derive(Clone)]
struct Selection {
    tracking: bool,
    physics: bool,
    appearance: bool,
    expressions: bool,
    actions: bool,
    animations: bool,
    labels: bool,
    framing: bool,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            tracking: true,
            physics: true,
            appearance: true,
            expressions: true,
            actions: true,
            animations: true,
            labels: true,
            framing: false,
        }
    }
}
struct Undo {
    saved: SavedRig,
    multipliers: BTreeMap<String, f32>,
}
#[derive(Default)]
pub struct Panel {
    pub open: bool,
    preview: Option<Preview>,
    selection: Selection,
    undo: Option<Undo>,
    pub error: Option<String>,
    pub dirty: bool,
    pub state_dirty: bool,
    pub repair_tracking: bool,
    pub requests: Vec<String>,
    pub runtime: vts::Runtime,
    drafts: BTreeMap<String, Shortcut>,
    pub keys: crate::vts_keys::Keys,
    sway: [f32; 3],
    scene_deadlines: BTreeMap<String, f32>,
    held_scenes: BTreeMap<String, String>,
    repair: Option<crate::vts_repair::Guide>,
}

/// Resolve exact relative references first, then a unique basename in this model.
/// Never follow a link or parent reference outside the model directory.
pub(crate) fn asset(base: &Path, reference: &str, suffix: &str) -> anyhow::Result<PathBuf> {
    let owned_base = base.canonicalize()?;
    let base = owned_base.as_path();
    vts::valid_reference(reference)?;
    anyhow::ensure!(
        reference.to_ascii_lowercase().ends_with(suffix),
        "Expected {suffix}"
    );
    if let Ok(path) = aria_model::resolve_asset(base, reference) {
        return Ok(path);
    }
    let normalized = reference.replace('\\', "/");
    let wanted = normalized.rsplit('/').next().unwrap_or(reference);
    let mut folders = vec![(base.to_owned(), 0)];
    let mut visited = BTreeSet::new();
    let mut budget = 4096;
    let mut found = Vec::new();
    while let Some((folder, depth)) = folders.pop() {
        let folder = folder.canonicalize()?;
        if !folder.starts_with(base) || !visited.insert(folder.clone()) {
            continue;
        }
        for entry in std::fs::read_dir(folder)?.take(budget).flatten() {
            budget -= 1;
            let path = entry.path();
            if path.is_dir() && depth < 8 {
                folders.push((path, depth + 1));
            } else if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(wanted))
            {
                let path = path.canonicalize()?;
                if path.starts_with(base) && path.is_file() {
                    found.push(path);
                }
            }
            if budget == 0 {
                break;
            }
        }
        if budget == 0 {
            break;
        }
    }
    found.sort();
    found.dedup();
    anyhow::ensure!(
        found.len() == 1,
        "Asset {reference}: found {} matches; keep one uniquely named file inside this model folder",
        found.len()
    );
    Ok(found.remove(0))
}
fn identity(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn preview(
    path: &Path,
    avatar: &crate::live2d::Avatar,
    saved: &SavedRig,
) -> anyhow::Result<Preview> {
    let path = path.canonicalize()?;
    let files = aria_model::load_files(&path)?;
    anyhow::ensure!(
        files.moc == avatar.files.moc,
        "This config belongs to another avatar. Import its .vtube.json through Avatar → Import avatar first."
    );
    let bytes = aria_model::read_bounded(&path, aria_core::asset_limits::MODEL_JSON)?;
    let extra: Vec<_> = saved
        .config
        .vbridger
        .outputs
        .iter()
        .map(|o| o.name.as_str())
        .collect();
    let mut imported = vts::parse(&bytes, avatar.model.parameters(), &extra)?;
    let base = avatar
        .files
        .source
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Model has no folder"))?;
    let refs: BTreeSet<_> = imported
        .expressions
        .iter()
        .cloned()
        .chain(imported.config.actions.iter().filter_map(|h| {
            if let Action::Expression(s) = &h.action {
                Some(s.clone())
            } else {
                None
            }
        }))
        .collect();
    let mut expressions = Vec::new();
    let mut mapped = BTreeMap::new();
    for reference in refs {
        let result = asset(
            base,
            &reference,
            if reference.ends_with(".exp3") {
                ".exp3"
            } else {
                ".exp3.json"
            },
        )
        .and_then(|path| {
            let expression = aria_core::expressions::Expression::load(&aria_model::read_bounded(
                &path,
                aria_core::asset_limits::EXPRESSION_JSON,
            )?)?;
            let missing = expression.missing_parameters(avatar.model.parameters());
            if !missing.is_empty() {
                imported.config.notes.push(format!(
                    "{reference}: {} parameters are missing from this rig and will be ignored.",
                    missing.len()
                ));
            }
            Ok(ExpressionFile {
                id: identity(base, &path),
                name: aria_model::expression_name(&path),
                path,
            })
        });
        match result {
            Ok(file) => {
                mapped.insert(reference, file.id.clone());
                expressions.push(file);
            }
            Err(e) => imported.config.notes.push(format!(
                "Expression {reference}: {e:#}. Its actions need the ARIA repair guide."
            )),
        }
    }
    imported.expressions = imported
        .expressions
        .iter()
        .filter_map(|id| mapped.get(id).cloned())
        .collect();
    imported.config.actions.retain_mut(|h| {
        if let Action::Expression(id) = &mut h.action {
            if let Some(new) = mapped.get(id) {
                *id = new.clone();
                true
            } else {
                h.action = Action::NeedsRepair {
                    kind: format!("Missing expression {id}"),
                };
                true
            }
        } else {
            true
        }
    });
    let motions: BTreeSet<_> = imported
        .config
        .actions
        .iter()
        .filter_map(|h| {
            if let Action::Animation { file, .. } = &h.action {
                Some(file.clone())
            } else {
                None
            }
        })
        .chain(imported.config.idle.iter().cloned())
        .chain(imported.config.lost_idle.iter().cloned())
        .collect();
    let mut mapped = BTreeMap::new();
    for reference in motions {
        match asset(base, &reference, ".motion3.json").and_then(|path| {
            Motion::load(&aria_model::read_bounded(
                &path,
                aria_core::asset_limits::MODEL_JSON,
            )?)
            .map(|m| (path, m))
        }) {
            Ok((path, motion)) => {
                for n in motion.notes.iter().take(16) {
                    imported.config.notes.push(format!("{reference}: {n}"));
                }
                mapped.insert(reference, identity(base, &path));
            }
            Err(e) => imported.config.notes.push(format!(
                "Animation {reference}: {e:#}. Its actions need the ARIA repair guide."
            )),
        }
    }
    imported.config.actions.retain_mut(|h| {
        if let Action::Animation { file, .. } = &mut h.action {
            if let Some(new) = mapped.get(file) {
                *file = new.clone();
                true
            } else {
                h.action = Action::NeedsRepair {
                    kind: format!("Missing motion {file}"),
                };
                true
            }
        } else {
            true
        }
    });
    imported.config.idle = imported.config.idle.and_then(|id| mapped.get(&id).cloned());
    imported.config.lost_idle = imported
        .config
        .lost_idle
        .and_then(|id| mapped.get(&id).cloned());
    let meshes: BTreeSet<_> = avatar
        .model
        .drawables
        .iter()
        .map(|d| d.id.as_str())
        .collect();
    imported.layers.colors.retain(|id, _| {
        let present = meshes.contains(id.as_str());
        if !present {
            imported
                .config
                .notes
                .push(format!("Mesh {id} is missing; its color was skipped."));
        }
        present
    });
    let occupied = saved.occupied_shortcuts();
    for h in &mut imported.config.actions {
        if h.shortcut.is_some_and(|k| occupied.contains(&k)) {
            imported.config.notes.push(format!(
                "{}: shortcut is already used in ARIA; imported as a button for reassignment.",
                h.name
            ));
            h.shortcut = None;
            h.chord.clear();
        }
    }
    crate::vts_items::resolve(
        &mut imported.config,
        base,
        saved.vts.assets_root.as_deref(),
        avatar,
    )?;
    imported.config.notes.truncate(2048);
    imported.config.validate()?;
    Ok(Preview {
        path,
        imported,
        expressions,
    })
}
impl Panel {
    fn undo_import(&mut self, avatar: &mut crate::live2d::Avatar, saved: &mut SavedRig) {
        if let Some(undo) = self.undo.take() {
            *saved = undo.saved;
            if let Some(p) = &mut avatar.physics {
                p.set_multipliers(&undo.multipliers);
            }
        }
    }
    pub fn placement(&self, config: &vts::Config) -> vts::Placement {
        let mut p = config.placement.clone();
        p.position[0] += self.sway[0];
        p.position[1] += self.sway[1];
        p.rotation += self.sway[2];
        p
    }
    #[allow(clippy::too_many_arguments)]
    pub fn advance(
        &mut self,
        ctx: &egui::Context,
        _avatar: &mut crate::live2d::Avatar,
        saved: &mut SavedRig,
        expressions: &mut crate::expressions_panel::ExpressionsPanel,
        inputs: &aria_core::rig::Inputs,
        frame: Option<&aria_core::TrackingFrame>,
        dt: f32,
    ) {
        let frozen = saved.config.pose.mode == PoseMode::Frozen;
        expressions.motions.face_found = frame.is_some_and(|f| f.face_found);
        let (pressed, released) = self.keys.update(ctx, &saved.vts, saved.global_hotkeys);
        self.requests.extend(pressed);
        for id in released {
            self.release(&id, saved);
        }
        if !frozen {
            for (i, key) in ["FaceAngleX", "FaceAngleY", "FaceAngleZ"]
                .iter()
                .enumerate()
            {
                let target = if saved.vts.sway.enabled {
                    inputs.get(*key).copied().unwrap_or(0.).clamp(-30., 30.) / 30.
                        * saved.vts.sway.amount[i]
                        * if i == 2 {
                            1.
                        } else if i == 1 {
                            -0.01
                        } else {
                            0.01
                        }
                } else {
                    0.
                };
                let blend = if saved.vts.sway.smoothing[i] == 0. {
                    1.
                } else {
                    1. - (-dt.clamp(0., 0.25) / (saved.vts.sway.smoothing[i] * 0.01)).exp()
                };
                self.sway[i] += (target - self.sway[i]) * blend;
            }
        }
        if self.runtime.tick(dt, frozen, &mut saved.config.expressions) {
            self.state_dirty = true;
        }
        self.requests.extend(
            self.runtime
                .screen_actions(&saved.vts, frame.map_or(-1, |f| f.hotkey)),
        );
        if frozen {
            self.requests.clear();
            return;
        }
        let expired: Vec<_> = self
            .scene_deadlines
            .iter()
            .filter(|(_, at)| **at <= self.runtime.elapsed)
            .map(|(id, _)| id.clone())
            .collect();
        if !expired.is_empty() {
            self.state_dirty = true;
        }
        for id in expired {
            self.scene_deadlines.remove(&id);
            if let Some(scene) = saved.vts.scenes.get(&id) {
                for item in &mut saved.config.items {
                    if scene.iter().any(|s| {
                        s.path == item.path && s.name == item.name && s.vts_scene == item.vts_scene
                    }) {
                        item.visible = false;
                    }
                }
            }
        }
        let mut once = BTreeSet::new();
        for id in std::mem::take(&mut self.requests)
            .into_iter()
            .take(64)
            .filter(|id| once.insert(id.clone()))
        {
            if id == "__stop-motion" {
                expressions.motions.stop();
                continue;
            }
            let result = self.execute(&id, saved, expressions);
            if let Err(e) = result {
                self.error = Some(format!("Action {id}: {e:#}"));
                crate::diagnostics::record(
                    "error",
                    "VTS_ACTION",
                    self.error.as_deref().unwrap_or("Action failed"),
                );
            } else {
                crate::diagnostics::record("info", "VTS_ACTION", "Imported action applied");
            }
        }
        if expressions.motions.events.len() > 128 {
            expressions
                .motions
                .events
                .drain(..expressions.motions.events.len() - 128);
        }
    }
    fn release(&mut self, id: &str, saved: &mut SavedRig) {
        self.state_dirty |= self.runtime.release(id, &mut saved.config.expressions);
        if let Some(scene) = self.held_scenes.remove(id) {
            for item in &mut saved.config.items {
                if item.vts_scene.as_deref() == Some(&scene) {
                    item.visible = false;
                }
            }
            self.scene_deadlines.remove(&scene);
            self.state_dirty = true;
        }
    }
    #[cfg(feature = "screenshots")]
    pub fn prepare_smoke(
        &mut self,
        avatar: &mut crate::live2d::Avatar,
        saved: &mut SavedRig,
        repair: bool,
    ) {
        self.apply(avatar, saved)
            .expect("Reviewed local VTS import");
        if repair
            && let Some(h) = saved
                .vts
                .actions
                .iter()
                .find(|h| h.name.to_ascii_lowercase().contains("jacket"))
                .or_else(|| saved.vts.actions.first())
        {
            self.repair = Some(crate::vts_repair::Guide::new(h.id.clone()));
        }
    }
    /// Apply synchronously so API tickets reflect validation/execution failures.
    pub fn execute(
        &mut self,
        id: &str,
        saved: &mut SavedRig,
        expressions: &mut crate::expressions_panel::ExpressionsPanel,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            saved.config.pose.mode != PoseMode::Frozen,
            "Resume tracking before running an imported action"
        );
        let h = saved
            .vts
            .actions
            .iter()
            .find(|h| h.id == id && h.enabled)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Imported action is missing or disabled"))?;
        match &h.action {
            Action::Expression(id) => {
                anyhow::ensure!(
                    expressions.entries.iter().any(|e| e.file.id == *id),
                    "Expression is unavailable; open Repair to choose its file"
                );
                expressions.set_fade(id, h.fade);
                saved.vts.expression_fades.insert(id.clone(), h.fade);
                self.runtime.expression(&h, &mut saved.config.expressions);
            }
            Action::ClearExpressions => {
                saved.config.expressions.clear();
            }
            Action::Animation { file, hold_last } => expressions.motions.start(file, *hold_last)?,
            Action::MoveModel(p) => saved.vts.placement = p.clone(),
            Action::HideItems => {
                for i in &mut saved.config.items {
                    i.visible = false;
                }
                self.scene_deadlines.clear();
            }
            Action::TogglePhysics => saved.config.physics.enabled = !saved.config.physics.enabled,
            Action::ItemScene { file, random } => {
                if let Some(scene) = saved.vts.scenes.get(file) {
                    let ids = crate::vts_items::toggle(&mut saved.config, scene, *random)?;
                    self.scene_deadlines.remove(file);
                    if h.release && !ids.is_empty() {
                        self.held_scenes.insert(h.id.clone(), file.clone());
                    } else {
                        self.held_scenes.remove(&h.id);
                    }
                    if !ids.is_empty()
                        && let Some(seconds) = h.seconds
                    {
                        self.scene_deadlines
                            .insert(file.clone(), self.runtime.elapsed + seconds);
                    }
                } else {
                    anyhow::bail!(
                        "Scene assets need repair. Open Repair in the imported action list."
                    );
                }
            }
            Action::NeedsRepair { .. } => {
                anyhow::bail!("This action needs the ARIA repair guide before it can run")
            }
        }
        self.state_dirty = true;
        Ok(())
    }
    pub fn offer(&mut self, path: &Path, avatar: &crate::live2d::Avatar, saved: &SavedRig) {
        self.open = true;
        self.selection = Selection::default();
        match preview(path, avatar, saved) {
            Ok(p) => {
                self.preview = Some(p);
                self.error = None;
            }
            Err(e) => {
                self.preview = None;
                self.error = Some(format!("Nothing changed: {e:#}"));
            }
        }
    }
    fn apply(
        &mut self,
        avatar: &mut crate::live2d::Avatar,
        saved: &mut SavedRig,
    ) -> anyhow::Result<()> {
        let p = self
            .preview
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Choose a config first"))?;
        // Re-read on Apply so a changed/removed export cannot silently pass review.
        let checked = preview(&p.path, avatar, saved)?;
        let p = &checked;
        let mut candidate = saved.clone();
        let selection = &self.selection;
        if selection.tracking {
            candidate
                .config
                .bindings
                .extend(p.imported.bindings.clone());
        }
        if selection.physics {
            candidate.config.physics = p.imported.physics.clone();
            candidate.vts.physics_imported = true;
        }
        if selection.appearance {
            candidate
                .config
                .layers
                .colors
                .extend(p.imported.layers.colors.clone());
        }
        if selection.labels {
            candidate.vts.labels = p.imported.config.labels.clone();
        }
        if selection.expressions {
            candidate.config.expressions = p.imported.expressions.clone();
        }
        if selection.actions {
            candidate.vts.actions = p.imported.config.actions.clone();
            candidate.vts.scenes = p.imported.config.scenes.clone();
            candidate.vts.assets_root = p.imported.config.assets_root.clone();
            candidate.vts.keyboard_enabled = p.imported.config.keyboard_enabled;
            candidate.vts.screen_enabled = p.imported.config.screen_enabled;
        }
        if selection.animations {
            candidate.vts.idle = p.imported.config.idle.clone();
            candidate.vts.lost_idle = p.imported.config.lost_idle.clone();
            candidate.vts.lost_idle_delay = p.imported.config.lost_idle_delay;
            candidate.vts.idle_enabled =
                candidate.vts.idle.is_some() || candidate.vts.lost_idle.is_some();
        }
        if selection.framing {
            candidate.vts.sway = p.imported.config.sway.clone();
        }
        candidate.vts.source_id = p.imported.config.source_id.clone();
        if selection.framing
            && let Some(placement) = &p.imported.saved_placement
        {
            candidate.vts.placement = placement.clone();
        }
        if selection.actions || selection.expressions {
            for file in &p.expressions {
                if !candidate.expression_files.contains(&file.path) {
                    candidate.expression_files.push(file.path.clone());
                }
            }
        }
        candidate.vts.name = p.imported.config.name.clone();
        candidate.vts.source_model = p.imported.config.source_model.clone();
        candidate.vts.notes = p.imported.config.notes.clone();
        candidate.validate(avatar.model.parameters())?;
        self.undo = Some(Undo {
            saved: saved.clone(),
            multipliers: avatar
                .physics
                .as_ref()
                .map(|p| {
                    p.groups()
                        .into_iter()
                        .map(|g| (g.id, g.imported_multiplier))
                        .collect()
                })
                .unwrap_or_default(),
        });
        *saved = candidate;
        if selection.physics
            && let Some(physics) = &mut avatar.physics
        {
            physics.set_multipliers(&BTreeMap::new());
        }
        self.preview = None;
        self.dirty = true;
        self.runtime.reset();
        Ok(())
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        avatar: &mut crate::live2d::Avatar,
        saved: &mut SavedRig,
    ) {
        if !self.open {
            return;
        }
        let mut open = true;
        let mut released_buttons = Vec::new();
        egui::Window::new("VTube Studio import · EXPERIMENTAL").id(egui::Id::new("vts-import")).open(&mut open).default_width(620.).show(ctx,|ui|{
            if self.preview.is_none() && !saved.vts.source_model.is_empty() {ui.label("Experimental · Customizations run and can be repaired inside ARIA. Review them before streaming.");}else{ui.label("Experimental compatibility feature: review the result with your model before streaming. Bring the matching .vtube.json into this avatar. Preview first, choose what to transfer, then Apply. Your original VTube Studio files are never edited.");}
            crate::help::button(ui,"vts-import");
            if ui.button("Repair / calibrate tracking step by step…").clicked(){self.repair_tracking=true;}
            ui.horizontal_wrapped(|ui|{
                if ui.button("Choose .vtube.json…").clicked() && let Some(path)=rfd::FileDialog::new().add_filter("VTube Studio config",&["json"]).pick_file(){self.offer(&path,avatar,saved);}
                if ui.button("Choose VTS StreamingAssets folder…").clicked() && let Some(root)=rfd::FileDialog::new().pick_folder() {saved.vts.assets_root=Some(root);self.dirty=true;}
                if ui.button("Read model's VTS config").clicked() && let Some(path)=avatar.files.tracking_profile.clone(){self.offer(&path,avatar,saved);}
            });

            if let Some(error)=&self.error {ui.colored_label(crate::theme::orange(),error);}
            let mut apply=false;let mut cancel=false;
            if let Some(p)=&self.preview {
                ui.strong(format!("{} · {} assignments · {} actions · {} expressions",p.imported.config.name,p.imported.bindings.len(),p.imported.config.actions.len(),p.expressions.len()));
                ui.small(p.path.display().to_string());
                ui.horizontal_wrapped(|ui|{
                    for (enabled,label) in [(&mut self.selection.tracking,"Tracking ranges"),(&mut self.selection.physics,"Physics"),(&mut self.selection.appearance,"Mesh colors"),(&mut self.selection.expressions,"Active expressions"),(&mut self.selection.actions,"Hotkeys / toggles"),(&mut self.selection.animations,"Idle animations"),(&mut self.selection.labels,"Parameter names"),(&mut self.selection.framing,"Approximate framing")] {ui.checkbox(enabled,label);}
                });
                ui.small("Imported hotkeys replace this avatar's previous VTS action list. Existing ARIA shortcuts are preserved; conflicts become buttons. Global shortcuts use ARIA's existing master switch.");
                egui::ScrollArea::vertical().max_height(330.).id_salt("vts-preview").show(ui,|ui|{
                    egui::CollapsingHeader::new("Actions to transfer").default_open(true).show(ui,|ui|{for h in &p.imported.config.actions {ui.label(format!("{} · {:?} · {}{}",h.name,h.action,h.shortcut.map_or("button only".into(),|k|k.label()),if h.enabled{""}else{" · disabled in VTS"}));}});
                    egui::CollapsingHeader::new(format!("Compatibility / missing assets · {} notices",p.imported.config.notes.len())).default_open(true).show(ui,|ui|{for n in &p.imported.config.notes{ui.label(n);}});
                });
                ui.horizontal(|ui|{apply=ui.button("Apply selected customizations").clicked();cancel=ui.button("Cancel preview").clicked();});
            }
            if apply {self.error=self.apply(avatar,saved).err().map(|e|format!("Nothing applied: {e:#}"));}
            if cancel {self.preview=None;}
            if self.undo.is_some() && ui.button("Undo last import").clicked() {
                self.undo_import(avatar,saved);
                self.runtime.reset();self.dirty=true;self.error=None;
            }
            if !saved.vts.actions.is_empty() || saved.vts.idle.is_some() {
                ui.separator();ui.strong("This avatar's imported controls · Experimental");
                egui::CollapsingHeader::new("Playback, shortcut switches & framing").show(ui,|ui|{
                self.dirty|=ui.checkbox(&mut saved.vts.keyboard_enabled,"Enable imported keyboard hotkeys").changed();
                self.dirty|=ui.checkbox(&mut saved.vts.screen_enabled,"Enable VTS phone screen buttons").changed();
                self.dirty|=ui.checkbox(&mut saved.global_hotkeys,"Enable Windows global shortcuts").changed();
                self.dirty|=ui.checkbox(&mut saved.vts.idle_enabled,"Play imported idle animations").changed();
                if ui.button("Stop animation / return to idle").clicked(){self.requests.push("__stop-motion".into());}
                egui::CollapsingHeader::new("Stage framing (approximate)").show(ui,|ui|{
                    self.dirty|=ui.add(egui::Slider::new(&mut saved.vts.placement.position[0],-1.0..=1.0).text("Horizontal")).changed();
                    self.dirty|=ui.add(egui::Slider::new(&mut saved.vts.placement.position[1],-1.0..=1.0).text("Vertical")).changed();
                    self.dirty|=ui.add(egui::Slider::new(&mut saved.vts.placement.zoom,0.25..=3.0).text("Scale")).changed();
                    self.dirty|=ui.add(egui::Slider::new(&mut saved.vts.placement.rotation,-180.0..=180.0).text("Rotation")).changed();
                    self.dirty|=ui.checkbox(&mut saved.vts.sway.enabled,"Tracking sway").changed();
                    for (i,label) in ["Horizontal sway","Vertical sway","Roll sway"].iter().enumerate(){self.dirty|=ui.add(egui::Slider::new(&mut saved.vts.sway.amount[i],0.0..=100.0).text(*label)).changed();self.dirty|=ui.add(egui::Slider::new(&mut saved.vts.sway.smoothing[i],0.0..=100.0).text("Smoothing")).changed();}
                    if ui.button("Center / normal scale").clicked(){saved.vts.placement=Default::default();self.dirty=true;}
                });
                });
                let mut changed=None;let mut repair=None;
                egui::ScrollArea::vertical().max_height(320.).id_salt("vts-actions").show(ui,|ui|{
                    for h in &mut saved.vts.actions {
                        ui.push_id(&h.id,|ui|{
                            ui.horizontal(|ui|{
                                self.dirty|=ui.checkbox(&mut h.enabled,"").changed();
                                let response=ui.add_enabled(h.enabled && saved.config.pose.mode!=PoseMode::Frozen,egui::Button::new(&h.name));
                                if (!h.release && response.clicked()) || (h.release && response.is_pointer_button_down_on() && !self.keys.buttons.contains(&h.id)) {self.requests.push(h.id.clone());}
                                if h.release {if response.is_pointer_button_down_on(){self.keys.buttons.insert(h.id.clone());}else if self.keys.buttons.remove(&h.id){released_buttons.push(h.id.clone());}}

                                ui.small(h.shortcut.map_or("No key".into(),|k|k.label()));
                                if ui.button("Repair / guide…").clicked(){repair=Some(h.id.clone());}
                            });
                            match &h.action { Action::NeedsRepair{kind} => {ui.colored_label(crate::theme::orange(),format!("Repair needed: {kind}"));}, Action::ItemScene{file,..} if !saved.vts.scenes.contains_key(file) => {ui.colored_label(crate::theme::orange(),"Repair needed: locate the scene's artwork in ARIA");}, _=>{} }
                            egui::CollapsingHeader::new("Configure action").show(ui,|ui|{
                                self.dirty|=ui.add(egui::TextEdit::singleline(&mut h.name).char_limit(80)).changed();
                                if h.name.trim().is_empty(){h.name="Imported action".into();}
                                self.dirty|=ui.checkbox(&mut h.global,"Global (Windows)").changed();
                                let key=self.drafts.entry(h.id.clone()).or_insert(h.shortcut.unwrap_or(Shortcut{ctrl:true,alt:true,key:0x31,..Default::default()}));
                                ui.horizontal_wrapped(|ui|{
                                    ui.checkbox(&mut key.ctrl,"Ctrl");ui.checkbox(&mut key.alt,"Alt");ui.checkbox(&mut key.shift,"Shift");
                                    egui::ComboBox::from_id_salt("key").selected_text(key.label()).show_ui(ui,|ui|{for (k,label) in Shortcut::keys(){ui.selectable_value(&mut key.key,k,label);}});
                                    if ui.button("Assign").clicked(){changed=Some((h.id.clone(),Some(*key)));}
                                    if ui.button("Clear key").clicked(){changed=Some((h.id.clone(),None));}
                                });
                                if matches!(h.action,Action::Expression(_) | Action::ItemScene{..}) {
                                    self.dirty|=ui.checkbox(&mut h.release,"Hold until key/button released").changed();
                                    let mut timed=h.seconds.is_some();if ui.checkbox(&mut timed,"Turn off after a duration").changed(){h.seconds=timed.then_some(10.);self.dirty=true;}
                                    if let Some(seconds)=&mut h.seconds {self.dirty|=ui.add(egui::DragValue::new(seconds).range(0.01..=3600.).suffix(" s")).changed();}
                                    self.dirty|=ui.add(egui::Slider::new(&mut h.fade,0.0..=10.).text("Fade seconds")).changed();
                                }
                                if let Action::MoveModel(position)=&mut h.action {
                                    self.dirty|=ui.add(egui::Slider::new(&mut position.position[0],-1.0..=1.0).text("Horizontal")).changed();
                                    self.dirty|=ui.add(egui::Slider::new(&mut position.position[1],-1.0..=1.0).text("Vertical")).changed();
                                    self.dirty|=ui.add(egui::Slider::new(&mut position.zoom,0.25..=3.0).text("Scale")).changed();
                                }
                            });
                        });
                    }
                });
                if let Some(id)=repair{self.repair=Some(crate::vts_repair::Guide::new(id));}
                if let Some((id,key))=changed {let mut candidate=saved.clone();let h=candidate.vts.actions.iter_mut().find(|h|h.id==id).unwrap();h.shortcut=key;h.chord.clear();match candidate.validate_vts_hotkeys(){Ok(())=>{*saved=candidate;self.dirty=true;self.error=None;},Err(e)=>self.error=Some(format!("Shortcut not changed: {e:#}"))}}
                egui::CollapsingHeader::new(format!("Import notes / repairs · {}",saved.vts.notes.len())).show(ui,|ui|{ for note in &saved.vts.notes {ui.label(note);} });
                if ui.button("Remove imported action list").clicked(){saved.vts.actions.clear();self.runtime.reset();self.dirty=true;}
            }
        });
        self.open = open;
        for id in released_buttons {
            self.release(&id, saved);
        }
        if let Some(mut guide) = self.repair.take() {
            let done = guide.show(ctx, avatar, saved);
            if let Some(id) = guide.applied.take() {
                self.dirty = true;
                self.requests.push(id);
            }
            if !done {
                self.repair = Some(guide);
            }
        }
    }
}

#[derive(Default)]
pub struct Motions {
    pub parts: Vec<aria_core::rig::RigParameter>,
    pub model: BTreeMap<String, f32>,
    pub events: Vec<String>,
    groups: BTreeMap<String, Vec<String>>,
    clips: BTreeMap<String, Motion>,
    pub error: Option<String>,
    active: Option<(String, f32, bool)>,
    idle: Option<String>,
    lost_idle: Option<String>,
    idle_enabled: bool,
    idle_time: f32,
    lost_time: f32,
    lost_delay: f32,
    pub face_found: bool,
}
impl Motions {
    pub fn load(&mut self, base: &Path, config: &vts::Config) {
        *self = Self {
            idle: config.idle.clone(),
            lost_idle: config.lost_idle.clone(),
            idle_enabled: config.idle_enabled,
            lost_delay: config.lost_idle_delay,
            ..Default::default()
        };
        if let Ok(bytes) = aria_model::read_bounded(
            &base.join(&config.source_model),
            aria_core::asset_limits::MODEL_JSON,
        ) && let Ok(root) = serde_json::from_slice::<serde_json::Value>(&bytes)
            && let Some(groups) = root["Groups"].as_array()
        {
            for g in groups {
                if let (Some(name), Some(ids)) = (g["Name"].as_str(), g["Ids"].as_array()) {
                    self.groups.insert(
                        name.into(),
                        ids.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .take(8192)
                            .collect(),
                    );
                }
            }
        }
        let files: BTreeSet<_> = config
            .actions
            .iter()
            .filter_map(|h| {
                if let Action::Animation { file, .. } = &h.action {
                    Some(file.clone())
                } else {
                    None
                }
            })
            .chain(config.idle.iter().cloned())
            .chain(config.lost_idle.iter().cloned())
            .collect();
        let mut bytes_total = 0;
        for id in files {
            let loaded = asset(base, &id, ".motion3.json")
                .and_then(|p| aria_model::read_bounded(&p, aria_core::asset_limits::MODEL_JSON))
                .and_then(|bytes| {
                    bytes_total += bytes.len();
                    anyhow::ensure!(
                        bytes_total <= 64 * 1024 * 1024,
                        "Imported motion cache exceeds 64 MiB"
                    );
                    Motion::load(&bytes)
                });
            match loaded {
                Ok(motion) => {
                    self.clips.insert(id, motion);
                }
                Err(e) => self.error = Some(format!("{id}: {e:#}")),
            }
        }
    }
    pub fn stop(&mut self) {
        self.active = None;
        self.idle_time = 0.;
    }
    pub fn start(&mut self, file: &str, hold: bool) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.clips.contains_key(file),
            "Animation is unavailable; open Repair to locate its .motion3.json"
        );
        self.active = Some((file.into(), 0., hold));
        Ok(())
    }
    pub fn update(&mut self, parameters: &mut [aria_core::rig::RigParameter], dt: f32) {
        self.model.clear();
        for p in &mut self.parts {
            p.value = p.default;
        }
        let dt = if dt.is_finite() {
            dt.clamp(0., 0.25)
        } else {
            0.
        };
        if self.face_found {
            self.lost_time = 0.;
        } else {
            self.lost_time += dt;
        }
        self.idle_time += dt;
        if let Some((id, time, hold)) = &mut self.active {
            if let Some(motion) = self.clips.get(id) {
                *time += dt;
                for e in &motion.events {
                    if e.time > *time - dt && e.time <= *time {
                        self.events.push(e.value.clone());
                    }
                }
                motion.apply_all(
                    parameters,
                    &mut self.parts,
                    &mut self.model,
                    *time,
                    false,
                    *hold,
                );
                apply_model_tracks(parameters, &self.groups, &self.model);
                if *time >= motion.duration && !*hold {
                    self.active = None;
                }
                return;
            }
            self.active = None;
        }
        if self.idle_enabled {
            let id = if !self.face_found && self.lost_time >= self.lost_delay {
                self.lost_idle.as_ref().or(self.idle.as_ref())
            } else {
                self.idle.as_ref()
            };
            if let Some(motion) = id.and_then(|id| self.clips.get(id)) {
                motion.apply_all(
                    parameters,
                    &mut self.parts,
                    &mut self.model,
                    self.idle_time,
                    true,
                    false,
                );
                apply_model_tracks(parameters, &self.groups, &self.model);
            }
        }
    }
}

fn apply_model_tracks(
    parameters: &mut [aria_core::rig::RigParameter],
    groups: &BTreeMap<String, Vec<String>>,
    model: &BTreeMap<String, f32>,
) {
    for (name, value) in model {
        if let Some(ids) = groups.get(name) {
            for p in parameters.iter_mut().filter(|p| ids.contains(&p.id)) {
                p.value = if name == "EyeBlink" {
                    p.value * value
                } else if name == "LipSync" {
                    p.value + value
                } else {
                    p.value
                };
                p.value = p.value.clamp(p.min, p.max);
            }
        }
    }
}
