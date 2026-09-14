//! Slow, local-only repair for imported actions. No launch or network execution.
use aria_core::{
    items::Item,
    movement::SavedRig,
    vts::{Action, Placement},
};
use eframe::egui;
use std::path::PathBuf;
pub struct Guide {
    id: String,
    step: u8,
    kind: usize,
    paths: Vec<PathBuf>,
    expression: String,
    placement: Placement,
    error: Option<String>,
    pub applied: Option<String>,
}
impl Guide {
    pub fn new(id: String) -> Self {
        Self {
            id,
            step: 0,
            kind: 0,
            paths: Vec::new(),
            expression: String::new(),
            placement: Default::default(),
            error: None,
            applied: None,
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        avatar: &crate::live2d::Avatar,
        saved: &mut SavedRig,
    ) -> bool {
        let mut open = true;
        let mut done = false;
        egui::Window::new("Repair imported action · Experimental").id(egui::Id::new("vts-repair-guide")).open(&mut open).default_width(590.).show(ctx,|ui|{
            let name=saved.vts.actions.iter().find(|h|h.id==self.id).map(|h|h.name.as_str()).unwrap_or("Imported action");ui.heading(name);ui.label(format!("Step {} of 3",self.step+1));
            match self.step {
                0=>{ui.label("Choose what this action should do inside ARIA. Nothing changes until Apply. The original VTS files stay untouched.");egui::ComboBox::from_id_salt("repair-kind").selected_text(KINDS[self.kind]).show_ui(ui,|ui|{for (i,name) in KINDS.iter().enumerate(){ui.selectable_value(&mut self.kind,i,*name);}});ui.small("A service-specific action may need a new local behavior. Choose its visible result here, then use Configure action to assign a hotkey. This guide never sends the action to another application.");},
                1=>{
                    match self.kind {
                        0=>{ui.label("Select an expression already loaded with this model, or locate its .exp3.json. An expression changes model parameters such as clothes or eye shapes.");egui::ComboBox::from_id_salt("repair-expression").selected_text(if self.expression.is_empty(){"Choose expression"}else{&self.expression}).show_ui(ui,|ui|{for f in &avatar.files.expressions{ui.selectable_value(&mut self.expression,f.id.clone(),&f.name);}});if ui.button("Locate expression…").clicked() && let Some(p)=rfd::FileDialog::new().add_filter("Expression",&["json","exp3"]).pick_file(){self.paths=vec![p];self.expression.clear();}},
                        1=>{ui.label("Locate the original .motion3.json animation. ARIA will play its parameter and body-part opacity tracks on this avatar.");if ui.button("Locate animation…").clicked() && let Some(p)=rfd::FileDialog::new().add_filter("Motion",&["json"]).pick_file(){self.paths=vec![p];}},
                        2=>{ui.label("Choose the PNG, GIF or Live2D items for this scene. For a sequence of PNG/JPG animation frames, choose its folder. After applying, position the items under Objects, then use Pick pin to attach them.");if ui.button("Choose item files…").clicked() && let Some(paths)=rfd::FileDialog::new().add_filter("Local artwork",&["png","gif","jpg","jpeg","moc3","json"]).pick_files(){self.paths=paths;}
if ui.button("Choose frame / Live2D folder…").clicked() && let Some(path)=rfd::FileDialog::new().pick_folder(){self.paths.push(path);}},
                        3=>{ui.label("Set where this button should move the avatar. Changes affect the ARIA stage and its outputs.");for (i,label) in ["Horizontal","Vertical"].iter().enumerate(){ui.add(egui::Slider::new(&mut self.placement.position[i],-1.0..=1.0).text(*label));}ui.add(egui::Slider::new(&mut self.placement.zoom,0.25..=3.0).text("Scale"));ui.add(egui::Slider::new(&mut self.placement.rotation,-180.0..=180.0).text("Rotation"));},
                        _=>{ui.label("This action uses the currently loaded avatar. No asset files are needed.");}
                    }
                    for p in &self.paths{ui.small(p.display().to_string());}
                },
                _=>{ui.label(format!("Ready to replace this action with: {}",KINDS[self.kind]));ui.label("Apply and test runs the repaired action on the current stage. Check its result. Reopen Repair to change it again; edit item positions/pins under Objects and shortcut behavior under Configure action.");if ui.button("Apply and test in ARIA").clicked(){match self.apply(avatar,saved){Ok(())=>{self.applied=Some(self.id.clone());done=true;},Err(e)=>self.error=Some(format!("Nothing changed: {e:#}"))}}}
            }
            if let Some(e)=&self.error{ui.colored_label(crate::theme::orange(),e);}
            ui.separator();ui.horizontal(|ui|{if ui.add_enabled(self.step>0,egui::Button::new("Back")).clicked(){self.step-=1;}
if ui.add_enabled(self.step<2,egui::Button::new("Next")).clicked(){self.step+=1;self.error=None;}
if ui.button("Cancel").clicked(){done=true;}});
        });
        done || !open
    }
    fn apply(&self, avatar: &crate::live2d::Avatar, saved: &mut SavedRig) -> anyhow::Result<()> {
        self.apply_to(&avatar.files, avatar.model.parameters(), saved)
    }
    fn apply_to(
        &self,
        files: &aria_model::ModelFiles,
        parameters: &[aria_core::rig::RigParameter],
        saved: &mut SavedRig,
    ) -> anyhow::Result<()> {
        let mut next = saved.clone();
        let owned_base = files
            .source
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Model has no folder"))?
            .canonicalize()?;
        let base = owned_base.as_path();
        let file = || -> anyhow::Result<PathBuf> {
            self.paths
                .first()
                .ok_or_else(|| anyhow::anyhow!("Choose the missing file in step 2"))?
                .canonicalize()
                .map_err(Into::into)
        };
        let action = match self.kind {
            0 => {
                let id = if !self.expression.is_empty() {
                    anyhow::ensure!(
                        files.expressions.iter().any(|e| e.id == self.expression),
                        "Select an expression belonging to this avatar"
                    );
                    self.expression.clone()
                } else {
                    let path = file()?;
                    anyhow::ensure!(
                        path.starts_with(base),
                        "Keep the expression inside this model's folder, then choose it again"
                    );
                    aria_core::expressions::Expression::load(&aria_model::read_bounded(
                        &path,
                        aria_core::asset_limits::EXPRESSION_JSON,
                    )?)?;
                    let id = path
                        .strip_prefix(base)?
                        .to_string_lossy()
                        .replace('\\', "/");
                    if !next.expression_files.contains(&path) {
                        next.expression_files.push(path);
                    }
                    id
                };
                Action::Expression(id)
            }
            1 => {
                let path = file()?;
                anyhow::ensure!(
                    path.to_string_lossy()
                        .to_ascii_lowercase()
                        .ends_with(".motion3.json"),
                    "Choose a .motion3.json animation file"
                );
                anyhow::ensure!(
                    path.starts_with(base),
                    "Keep the motion inside this model's folder, then choose it again"
                );
                aria_core::motion::Motion::load(&aria_model::read_bounded(
                    &path,
                    aria_core::asset_limits::MODEL_JSON,
                )?)?;
                Action::Animation {
                    file: path
                        .strip_prefix(base)?
                        .to_string_lossy()
                        .replace('\\', "/"),
                    hold_last: false,
                }
            }
            2 => {
                anyhow::ensure!(!self.paths.is_empty(), "Choose at least one item");
                let id = format!("repaired-{}", self.id);
                aria_core::vts::valid_reference(&id)?;
                let mut items = Vec::new();
                for (index, path) in self.paths.iter().enumerate() {
                    let mut path = path.canonicalize()?;
                    if path.is_dir() {
                        if let Ok(files) = aria_model::load_files(&path) {
                            path = files.source;
                        } else {
                            crate::media::sequence::inspect(&path)?;
                        }
                    } else if aria_core::items::is_model(&path) {
                        aria_model::load_files(&path)?;
                    } else {
                        crate::media::inspect(&path)?;
                    }
                    anyhow::ensure!(
                        crate::items::is_item(&path),
                        "Choose supported local artwork"
                    );
                    items.push(Item {
                        id: index as u64 + 1,
                        vts_scene: Some(id.clone()),
                        vts_slot: index as u16,
                        name: format!("Repaired item {}", index + 1),
                        path,
                        ..Default::default()
                    });
                }
                aria_core::items::validate(&items)?;
                next.vts.scenes.insert(id.clone(), items);
                Action::ItemScene {
                    file: id,
                    random: false,
                }
            }
            3 => Action::MoveModel(self.placement.clone()),
            4 => Action::ClearExpressions,
            5 => Action::HideItems,
            _ => Action::TogglePhysics,
        };
        next.vts
            .actions
            .iter_mut()
            .find(|h| h.id == self.id)
            .ok_or_else(|| anyhow::anyhow!("Action was removed"))?
            .action = action;
        next.validate(parameters)?;
        *saved = next;
        crate::diagnostics::record("info", "VTS_REPAIR", "Imported action repaired inside ARIA");
        Ok(())
    }
}
const KINDS: [&str; 7] = [
    "Toggle expression",
    "Play animation",
    "Toggle item scene",
    "Move avatar",
    "Clear expressions",
    "Hide all items",
    "Toggle physics",
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repair_is_transactional_and_preserves_hotkey_identity() {
        let dir = tempfile::tempdir().unwrap();
        let files = aria_model::ModelFiles {
            source: dir.path().join("avatar.model3.json"),
            moc: dir.path().join("avatar.moc3"),
            textures: vec![],
            physics: None,
            tracking_profile: None,
            display_info: None,
            expressions: vec![],
            warnings: vec![],
        };
        let key = aria_core::shortcuts::Shortcut {
            ctrl: true,
            key: 0x41,
            ..Default::default()
        };
        let mut saved = SavedRig::default();
        saved.vts.actions.push(aria_core::vts::Hotkey {
            id: "repair-test".into(),
            name: "Missing scene".into(),
            action: Action::NeedsRepair {
                kind: "Unknown source action".into(),
            },
            shortcut: Some(key),
            screen_button: None,
            enabled: true,
            global: false,
            release: false,
            seconds: None,
            fade: 0.5,
            chord: vec![],
        });
        let before = serde_json::to_vec(&saved).unwrap();
        let mut guide = Guide::new("repair-test".into());
        guide.step = 2;
        assert!(guide.apply_to(&files, &[], &mut saved).is_err());
        assert_eq!(before, serde_json::to_vec(&saved).unwrap());
        guide.kind = 1;
        guide.paths = vec![dir.path().join("missing.motion3.json")];
        assert!(guide.apply_to(&files, &[], &mut saved).is_err());
        assert_eq!(before, serde_json::to_vec(&saved).unwrap());
        guide.kind = 2;
        let image = dir.path().join("item.png");
        image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
            .save(&image)
            .unwrap();
        guide.paths = vec![image];
        guide.apply_to(&files, &[], &mut saved).unwrap();
        assert_eq!(saved.vts.actions[0].shortcut, Some(key));
        assert_eq!(saved.vts.actions[0].id, "repair-test");
        let Action::ItemScene { file, .. } = &saved.vts.actions[0].action else {
            panic!("Repair did not create a local scene")
        };
        assert_eq!(saved.vts.scenes[file].len(), 1);
        saved.validate(&[]).unwrap();
        let mut panel = crate::vts_panel::Panel::default();
        panel
            .execute("repair-test", &mut saved, &mut Default::default())
            .unwrap();
        assert_eq!(saved.config.items.len(), 1);
        assert!(saved.config.items[0].visible);
        panel
            .execute("repair-test", &mut saved, &mut Default::default())
            .unwrap();
        assert!(!saved.config.items[0].visible);
        let old = serde_json::to_vec(&saved).unwrap();
        saved.vts.actions[0].action = Action::NeedsRepair {
            kind: "unavailable".into(),
        };
        assert!(
            panel
                .execute("repair-test", &mut saved, &mut Default::default())
                .is_err()
        );
        saved = serde_json::from_slice(&old).unwrap();
        assert!(!saved.config.items[0].visible);
    }
}
