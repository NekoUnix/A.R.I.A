//! Experimental VBridger compatibility and model-local import review. Browsing/cancelling a file never changes the live rig.
use aria_core::{
    movement::RigConfig,
    rig::{self, Inputs, RigParameter},
    vbridger::{self, Config},
};
use eframe::egui;
use std::{collections::BTreeMap, path::Path};
mod editor;

#[derive(Default)]
pub struct Panel {
    pub open: bool,
    draft: Option<Config>,
    error: Option<String>,
    undo: Option<RigConfig>,
    pub dirty: bool,
    editor: editor::Editor,
    candidates: Option<Result<BTreeMap<String, rig::Binding>, String>>,
}
impl Panel {
    pub fn load(&mut self, path: &Path) {
        self.draft = None;
        self.error = None;
        self.candidates = None;
        self.editor = Default::default();
        match aria_model::read_bounded(path, vbridger::MAX_FILE).and_then(|b| {
            vbridger::import(&b, &path.file_name().unwrap_or_default().to_string_lossy())
        }) {
            Ok(config) => self.draft = Some(config),
            Err(e) => self.error = Some(format!("Import not applied: {e:#}")),
        }
        self.open = true;
    }
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        config: &mut RigConfig,
        parameters: &[RigParameter],
        inputs: &Inputs,
        raw: Option<&aria_core::TrackingFrame>,
        sidecar: Option<&Path>,
    ) {
        if !self.open {
            return;
        }
        let mut open = self.open;
        egui::Window::new("VBridger tracking · Experimental")
            .id(egui::Id::new("vbridger-import")).open(&mut open)
            .default_size(egui::vec2(690.0,620.0)).max_width(ctx.content_rect().width()-30.0)
            .max_height((ctx.content_rect().height()-80.0).max(180.0)).vscroll(true)
            .show(ctx, |ui| {
                crate::help::label(ui, "Import equations, curves and output ranges", "vbridger");
                ui.colored_label(crate::theme::orange(), "Experimental: compatibility and motion may differ from VBridger. Review this draft before applying.");
                ui.label("Choose a .vbridger file, review its outputs, then apply it to this avatar. Your phone or webcam still supplies live tracking.");
                if ui.button("Choose VBridger config…").clicked()
                    && let Some(path) = rfd::FileDialog::new().add_filter("VBridger config", &["vbridger", "json"]).pick_file() {
                    self.load(&path);
                }
                if let Some(error) = &self.error { ui.colored_label(crate::theme::orange(),error); }
                if ui.button("Create a new equation config").clicked() {
                    self.draft=Some(Config {enabled:true,name:"My tracking".into(),..Default::default()});
                    self.editor=Default::default(); self.candidates=None;
                }
                if let Some(mut draft) = self.draft.take() {
                    ui.separator();
                    ui.strong(format!("{} · {} supported outputs · {} notices", draft.name, draft.outputs.len(), draft.notes.len()));
                    egui::CollapsingHeader::new("How this connects to your model").show(ui,|ui| {
                        ui.small("Matching source names feed existing model assignments and preserve their output ranges. New names can be selected under Tracking → Inputs. Imported values replace ARIA's built-in gains, mirroring and personal calibration for those channels; model ranges and mouth response still apply.");
                        ui.small("Head axes stay separate: pitch = up/down, yaw = left/right. Test both after importing and use the correction switches below if your source needs them.");
                    });
                    let candidates = self.candidates.get_or_insert_with(||candidates(&draft,config,parameters,sidecar).map_err(|e|format!("{e:#}"))).clone();
                    match &candidates {
                        Err(e) => { ui.colored_label(crate::theme::orange(),format!("Could not read the model's VTS assignments: {e:#}. Fix the sidecar or remove it before applying.")); },
                        Ok(bindings) => { ui.small(format!("{} additional model assignments can be connected.", bindings.len())); },
                    }
                    egui::CollapsingHeader::new("Outputs and equations").default_open(true).show(ui,|ui| {
                        egui::ScrollArea::vertical().id_salt("vbridger-review-rows").max_height(270.0).show(ui,|ui| {
                            for o in &mut draft.outputs {
                                let targets: Vec<_> = config.bindings.iter().filter(|(_,b)| vbridger::alias(&b.input)==o.name).map(|(id,_)|id.as_str())
                                    .chain(candidates.as_ref().ok().into_iter().flat_map(|b| b.iter()).filter(|(_,b)|b.input==o.name).map(|(id,_)|id.as_str())).collect();
                                ui.push_id(o.name.clone(),|ui| {
                                    egui::CollapsingHeader::new(format!("{}   [{:.3} … {:.3}]",o.name,o.min,o.max)).show(ui,|ui| {
                                        ui.label(o.equation.source());
                                        ui.small(format!("Default {:.3} · {} curve keys",o.default,o.curve.keys.len()));
                                        ui.small(if targets.is_empty() { "Available as an input; no matching model assignment.".into() } else { format!("Drives: {}",targets.join(", ")) });
                                        self.editor.output(ui,o);
                                    });
                                });
                            }
                        });
                    });
                    self.editor.settings(ui,&mut draft,raw);
                    if !draft.notes.is_empty() {
                        egui::CollapsingHeader::new("Skipped settings / compatibility notices").default_open(true).show(ui,|ui| { for note in &draft.notes { ui.label(note); } });
                    }
                    let mut apply = false;
                    let mut cancel = false;
                    ui.horizontal(|ui| {
                        apply = ui.add_enabled(!draft.outputs.is_empty() && candidates.is_ok() && self.editor.valid(),egui::Button::new("Apply supported outputs to this avatar")).clicked();
                        cancel = ui.button("Cancel import").clicked();
                    });
                    if apply {
                        let mut next = config.clone();
                        let result=draft.prepare().and_then(|()| {
                            next.bindings.extend(self::candidates(&draft,config,parameters,sidecar)?);
                            next.vbridger=draft.clone(); next.validate(parameters)
                        });
                        match result {
                            Ok(()) => { self.undo = Some(config.clone()); *config = next; self.draft = None; self.dirty = true; self.error = None; },
                            Err(e) => {self.error = Some(format!("Nothing applied: {e:#}"));self.draft=Some(draft);},
                        }
                    } else if !cancel {self.draft=Some(draft);}
                }
                if !config.vbridger.outputs.is_empty() {
                    ui.separator();
                    ui.strong(format!("Saved config: {}",config.vbridger.name));
                    ui.horizontal(|ui| {
                        if ui.button("Edit saved settings…").clicked() {self.draft=Some(config.vbridger.clone());self.editor=Default::default();self.candidates=None;}
                        if ui.button("Export complete config…").clicked() && let Some(path)=rfd::FileDialog::new().add_filter("ARIA tracking config",&["json"]).set_file_name("my-tracking.json").save_file() {
                            self.error=config.vbridger.export().and_then(|b|std::fs::write(path,b).map_err(Into::into)).err().map(|e|format!("Export failed: {e:#}"));
                        }
                    });
                    if ui.button("Export .vbridger outputs only…").on_hover_text("Exports equations and output modifiers as encoded V2 data. Input calibration/curves, external input declarations and ARIA face-loss settings require the complete JSON export.").clicked()
                        && let Some(path)=rfd::FileDialog::new().add_filter("VBridger outputs",&["vbridger"]).set_file_name("my-outputs.vbridger").save_file() {
                        self.error=config.vbridger.export_vbridger().and_then(|b|std::fs::write(path,b).map_err(Into::into)).err().map(|e|format!("Export failed: {e:#}"));
                    }
                    self.dirty |= ui.checkbox(&mut config.vbridger.enabled,"Enable imported tracking for this avatar").changed();
                    ui.small("Saved automatically with this model and included in movement presets. Disabling restores ARIA's normal derived inputs. Outputs with no live source return to the file's defaults; missing blendshapes use zero.");
                    ui.horizontal_wrapped(|ui| {
                        self.dirty |= ui.checkbox(&mut config.vbridger.invert_pitch,"Invert imported pitch").changed();
                        self.dirty |= ui.checkbox(&mut config.vbridger.invert_yaw,"Invert imported yaw").changed();
                        self.dirty |= ui.checkbox(&mut config.vbridger.invert_roll,"Invert imported roll").changed();
                    });
                    if let Some(frame) = raw.filter(|f|f.face_found) {
                        let missing = config.vbridger.missing_inputs(frame);
                        if !missing.is_empty() {
                            egui::CollapsingHeader::new(format!("{} raw inputs absent from this source",missing.len())).show(ui,|ui|{ui.label(missing.into_iter().collect::<Vec<_>>().join(", "));ui.small("Use a tracking source that sends these blendshapes for the intended expression detail.");});
                        }
                    } else { ui.label("Waiting for a live face. Connect tracking to test the imported outputs."); }
                    egui::CollapsingHeader::new("Live imported values").show(ui,|ui| {
                        for o in &config.vbridger.outputs {
                            ui.label(format!("{}: {}",o.name, if config.vbridger.enabled && o.enabled { inputs.get(&o.name).map(|v|format!("{v:.3}")).unwrap_or_else(||"waiting".into()) } else { "disabled".into() }));
                        }
                    });
                    if self.undo.is_some() && ui.button("Undo last import (restore prior rig settings)").clicked() {
                        *config = self.undo.take().unwrap(); self.dirty = true;
                    }
                }
            });
        if !open {
            self.draft = None;
        }
        self.open = open;
    }
}

fn candidates(
    import: &Config,
    current: &RigConfig,
    parameters: &[RigParameter],
    sidecar: Option<&Path>,
) -> anyhow::Result<BTreeMap<String, rig::Binding>> {
    let names: Vec<_> = import.outputs.iter().map(|o| o.name.as_str()).collect();
    let defaults = rig::default_bindings(parameters);
    let mut bindings = if let Some(path) = sidecar {
        let bytes = aria_model::read_bounded(path, aria_core::asset_limits::MODEL_JSON)?;
        rig::import_profile_with_inputs(&bytes, parameters, &names)?.bindings
    } else {
        BTreeMap::new()
    };
    bindings.retain(|id, b| {
        let automatic_default = current
            .bindings
            .get(id)
            .zip(defaults.get(id))
            .is_some_and(|(a, b)| serde_json::to_value(a).ok() == serde_json::to_value(b).ok());
        (!current.bindings.contains_key(id) || automatic_default)
            && names.contains(&b.input.as_str())
    });
    for p in parameters {
        if current.bindings.contains_key(&p.id) || bindings.contains_key(&p.id) {
            continue;
        }
        if let Some(o) = import.output(vbridger::alias(&p.id)) {
            let mut b = rig::Binding::direct(&o.name, o.min, o.max);
            b.output_min = p.min;
            b.output_max = p.max;
            bindings.insert(p.id.clone(), b);
        }
    }
    Ok(bindings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires local ARIA_TEST_VBRIDGER, ARIA_TEST_MODEL and ARIA_CUBISM_CORE"]
    fn experimental_import_drives_native_model_and_preserves_frozen_pose() {
        let path = std::env::var_os("ARIA_TEST_MODEL").unwrap();
        let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
        let source = std::env::var_os("ARIA_TEST_VBRIDGER").unwrap();
        let files = aria_model::load_files(Path::new(&path)).unwrap();
        let mut model = aria_live2d::CubismModel::load(
            Path::new(&core),
            &aria_model::read_bounded(&files.moc, aria_core::asset_limits::MOC_FILE).unwrap(),
            files.textures.len(),
        )
        .unwrap();
        let import = vbridger::import(&std::fs::read(source).unwrap(), "local fixture").unwrap();
        let mut rig = RigConfig::from_parameters(model.parameters());
        rig.bindings.extend(
            candidates(
                &import,
                &rig,
                model.parameters(),
                files.tracking_profile.as_deref(),
            )
            .unwrap(),
        );
        rig.vbridger = import;
        rig.validate(model.parameters()).unwrap();
        let mut runtime = vbridger::Runtime::default();
        let mut values = model.parameters().to_vec();
        let mut snapshots = Vec::new();
        for mouth in [0.0, 1.0] {
            let packet = format!("jawOpen&{}|=head#0,0,0,0,0,0|", mouth * 100.0);
            let frame = aria_tracking::protocol::decode(
                packet.as_bytes(),
                aria_tracking::Protocol::IFacialMocap,
            )
            .unwrap();
            for _ in 0..60 {
                let mut input = Inputs::new();
                rig.vbridger.apply(
                    Some(&frame),
                    aria_core::Vec3::default(),
                    &mut input,
                    &mut runtime,
                    1.0 / 60.0,
                );
                rig.evaluate(&input, &mut values, 1.0 / 60.0, None);
            }
            for p in &values {
                model.set_parameter(&p.id, p.value);
            }
            model.update().unwrap();
            snapshots.push(
                model
                    .drawables
                    .iter()
                    .flat_map(|d| d.positions.clone())
                    .collect::<Vec<_>>(),
            );
        }
        assert_ne!(
            snapshots[0], snapshots[1],
            "Imported mouth equation must move the model's actual geometry"
        );
        rig.capture_pose(&values);
        let saved = values.iter().map(|p| p.value).collect::<Vec<_>>();
        rig.evaluate(&Inputs::new(), &mut values, 1.0 / 60.0, None);
        assert_eq!(values.iter().map(|p| p.value).collect::<Vec<_>>(), saved);
        eprintln!(
            "Experimental import: {} outputs drive {} model assignments; native geometry changes and freeze remains exact",
            rig.vbridger.outputs.len(),
            rig.bindings.len()
        );
    }
    fn config() -> Config {
        vbridger::import(br#"{"store":[{"output":"MouthOpen","equation":"jawOpen*.5","min":0,"max":1,"defaultValue":0}]}"#,"test").unwrap()
    }
    fn parameters() -> Vec<RigParameter> {
        vec![RigParameter {
            id: "ParamMouthOpenY".into(),
            min: 0.0,
            max: 1.0,
            default: 0.0,
            value: 0.0,
        }]
    }
    fn draw(
        ctx: &egui::Context,
        panel: &mut Panel,
        rig: &mut RigConfig,
        time: f64,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        crate::run_test_ui(
            ctx,
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 1000.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| panel.window(ui.ctx(), rig, &parameters(), &Inputs::new(), None, None),
        )
    }
    fn position(shape: &egui::Shape, name: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(t) if t.galley.text() == name => Some(t.pos + t.galley.size() * 0.5),
            egui::Shape::Vec(v) => v.iter().find_map(|s| position(s, name)),
            _ => None,
        }
    }
    fn click(ctx: &egui::Context, panel: &mut Panel, rig: &mut RigConfig, name: &str, time: f64) {
        draw(ctx, panel, rig, time, vec![]);
        let output = draw(ctx, panel, rig, time + 0.1, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|s| position(&s.shape, name))
            .unwrap_or_else(|| panic!("Missing {name}"));
        for (i, pressed) in [true, false].into_iter().enumerate() {
            draw(
                ctx,
                panel,
                rig,
                time + 0.2 + i as f64 * 0.1,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    #[test]
    fn review_cancel_apply_and_undo_use_actual_ui_without_changing_other_models() {
        let ctx = egui::Context::default();
        crate::theme::install(&ctx);
        let mut panel = Panel {
            open: true,
            draft: Some(config()),
            ..Default::default()
        };
        let mut rig = RigConfig::from_parameters(&parameters());
        let original = serde_json::to_value(&rig).unwrap();
        click(&ctx, &mut panel, &mut rig, "Cancel import", 0.0);
        assert!(panel.draft.is_none());
        assert!(!panel.dirty);
        assert_eq!(serde_json::to_value(&rig).unwrap(), original);
        panel.draft = Some(config());
        click(
            &ctx,
            &mut panel,
            &mut rig,
            "Apply supported outputs to this avatar",
            1.0,
        );
        assert!(rig.vbridger.enabled);
        assert!(panel.dirty);
        assert!(panel.draft.is_none());
        assert!(!RigConfig::from_parameters(&parameters()).vbridger.enabled);
        click(
            &ctx,
            &mut panel,
            &mut rig,
            "Undo last import (restore prior rig settings)",
            2.0,
        );
        assert_eq!(serde_json::to_value(&rig).unwrap(), original);
    }
    #[test]
    fn changing_a_draft_does_not_touch_saved_equations_and_failed_load_clears_draft() {
        let mut panel = Panel {
            open: true,
            draft: Some(config()),
            ..Default::default()
        };
        let rig = RigConfig {
            vbridger: config(),
            ..Default::default()
        };
        panel.draft.as_mut().unwrap().outputs[0].default = 0.2;
        assert_eq!(rig.vbridger.outputs[0].default, 0.0);
        panel.load(Path::new("definitely-absent-aria-test-config.vbridger"));
        assert!(panel.error.is_some());
        assert!(panel.draft.is_none());
        assert!(!panel.dirty);
    }
}
