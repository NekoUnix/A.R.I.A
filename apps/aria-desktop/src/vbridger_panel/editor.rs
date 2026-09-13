use aria_core::vbridger::{Config, Curve, Equation, Key, Output, Step};
use eframe::egui;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Editor {
    text: BTreeMap<String, String>,
    errors: BTreeMap<String, String>,
    new_name: String,
    external_name: String,
    remove: Option<String>,
    copied_curve: Option<Curve>,
}
impl Editor {
    pub fn valid(&self) -> bool {
        self.errors.is_empty()
    }
    pub fn output(&mut self, ui: &mut egui::Ui, o: &mut Output) {
        crate::help::label(ui, "Equation and modifiers", "vbridger-editor");
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut o.enabled, "Enabled");
            ui.checkbox(&mut o.requires_face, "Requires a tracked face");
            if ui.small_button("Remove output").clicked() {
                self.remove = Some(o.name.clone());
            }
        });
        let text = self
            .text
            .entry(o.name.clone())
            .or_insert_with(|| o.equation.source().into());
        if ui
            .add(
                egui::TextEdit::multiline(text)
                    .desired_width(f32::INFINITY)
                    .desired_rows(2),
            )
            .changed()
        {
            match Equation::parse(text) {
                Ok(e) => {
                    o.equation = e;
                    self.errors.remove(&o.name);
                }
                Err(e) => {
                    self.errors.insert(o.name.clone(), e.to_string());
                }
            }
        }
        if let Some(error) = self.errors.get(&o.name) {
            ui.colored_label(crate::theme::orange(), error);
        }
        ui.small(format!("Original VBridger faceSend flag: {} (retained for .vbridger export; network routing is not imported).",o.face_send));
        ui.horizontal_wrapped(|ui| {
            number(ui, "Min", &mut o.min, -1e6..=1e6);
            number(ui, "Default", &mut o.default, -1e6..=1e6);
            number(ui, "Max", &mut o.max, -1e6..=1e6);
        });
        ui.small("Order: input offset/curve → equation → output curve/range → delay → smoothing → steps → model mapping. All changes are a draft until Apply.");
        ui.checkbox(&mut o.delay_on, "Delay");
        if o.delay_on {
            ui.add(egui::Slider::new(&mut o.delay_ms, 0.0..=5000.0).text("Delay (ms)"));
        }
        ui.checkbox(&mut o.smooth_on, "Smoothing");
        if o.smooth_on {
            ui.add(egui::Slider::new(&mut o.smoothing, 0.0..=1.0).text("Amount"));
            ui.small("0 follows immediately; 1 holds the previous value. Uses time-adjusted lerp based on 60 Hz; proprietary filter details can differ.");
        }
        egui::CollapsingHeader::new("Output curve").show(ui, |ui| {
            self.curve(ui, &mut o.curve, (o.min, o.max));
        });
        ui.checkbox(&mut o.step_on, "Stepped animation");
        if o.step_on {
            ui.small("Trigger enters a step; Target is its value; fall below Threshold to leave it; Hold is the minimum time in milliseconds. Threshold is an absolute input value. Keep triggers increasing.");
            let mut remove = None;
            for (i, step) in o.steps.iter_mut().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        number(ui, "Trigger", &mut step.trigger, -1e6..=1e6);
                        number(ui, "Target", &mut step.target, -1e6..=1e6);
                        number(ui, "Threshold", &mut step.threshold, -1e6..=1e6);
                        number(ui, "Hold ms", &mut step.hold_ms, 0.0..=5000.0);
                        if ui.small_button("×").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                o.steps.remove(i);
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(o.steps.len() < 64, egui::Button::new("Add step"))
                    .clicked()
                {
                    o.steps.push(Step {
                        trigger: o.max,
                        target: o.max,
                        threshold: o.max,
                        hold_ms: 0.0,
                    });
                }
                if ui.button("Even 5-step preset").clicked() {
                    o.steps = (0..5)
                        .map(|i| {
                            let v = o.min + (o.max - o.min) * i as f32 / 4.0;
                            Step {
                                trigger: v,
                                target: v,
                                threshold: v,
                                hold_ms: 80.0,
                            }
                        })
                        .collect();
                }
                if ui.button("Sort triggers").clicked() {
                    o.steps.sort_by(|a, b| a.trigger.total_cmp(&b.trigger));
                }
            });
        }
    }
    pub fn settings(
        &mut self,
        ui: &mut egui::Ui,
        c: &mut Config,
        raw: Option<&aria_core::TrackingFrame>,
    ) {
        if let Some(name) = self.remove.take() {
            c.outputs.retain(|o| o.name != name);
            self.text.remove(&name);
            self.errors.remove(&name);
        }
        egui::CollapsingHeader::new("Add an output").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut self.new_name);
                if ui
                    .add_enabled(c.outputs.len() < 256, egui::Button::new("Add"))
                    .clicked()
                {
                    let result = Output::new(self.new_name.trim(), "jawOpen", 0.0, 1.0);
                    match result {
                        Ok(o) if !c.outputs.iter().any(|old| old.name == o.name) => {
                            c.outputs.push(o);
                            self.new_name.clear();
                            self.errors.remove("new-output");
                        }
                        _ => {
                            self.errors.insert(
                                "new-output".into(),
                                "Choose a unique output name using letters, digits or underscores."
                                    .into(),
                            );
                        }
                    }
                }
            });
            if let Some(e) = self.errors.get("new-output") {
                ui.colored_label(crate::theme::orange(), e);
            }
        });
        egui::CollapsingHeader::new("Input calibration and curves").show(ui,|ui| {
            crate::help::label(ui,"Tune your incoming face data","vbridger-inputs");
            ui.small("Offsets are applied before input curves. These settings are per avatar. VBridger stores input curves separately from its usual .vbridger output file; ARIA's complete JSON export includes both.");
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(raw.is_some_and(|f|f.face_found),egui::Button::new("Use current face as blendshape zero")).clicked(){c.calibrate_blends(raw.unwrap());}
                if ui.button("Reset offsets").clicked(){c.offsets.clear();}
                if ui.button("Reset input curves").clicked(){c.input_curves.clear();}
            });
            for name in c.raw_names() {
                egui::CollapsingHeader::new(&name).id_salt(("raw-curve",&name)).show(ui,|ui| {
                    let mut offset=c.offsets.get(&name).copied().unwrap_or(0.0);
                    if ui.add(egui::DragValue::new(&mut offset).speed(0.01).prefix("Offset ").range(-1e6..=1e6)).changed(){c.offsets.insert(name.clone(),offset);}
                    let curve=c.input_curves.entry(name.clone()).or_default();
                    self.curve(ui,curve,if name.to_ascii_lowercase().starts_with("headrot") {(-45.0,45.0)}else{(0.0,1.0)});
                });
            }
        });
        egui::CollapsingHeader::new("External / plugin inputs").show(ui,|ui| {
            ui.small("Declare extra numeric channels for equations. A tool can send these exact names in the ARIA JSON packet's parameters object. Values use the fallback until supplied. VMC body channels and standard viseme names are already recognized; there is no built-in VMC receiver or audio phoneme recognizer.");
            ui.horizontal(|ui|{ui.text_edit_singleline(&mut self.external_name);if ui.add_enabled(c.external_inputs.len()<128,egui::Button::new("Declare input")).clicked() && !self.external_name.is_empty(){c.external_inputs.insert(self.external_name.trim().into(),0.0);self.external_name.clear();}});
            let mut remove=None;
            for (name,value) in &mut c.external_inputs {
                ui.push_id(name,|ui|{ui.horizontal(|ui|{ui.label(name);number(ui,"Fallback",value,-1e6..=1e6);if ui.small_button("×").clicked(){remove=Some(name.clone());}});});
            }
            if let Some(name)=remove{c.external_inputs.remove(&name);c.offsets.remove(&name);c.input_curves.remove(&name);}
        });
        egui::CollapsingHeader::new("Profile settings").show(ui,|ui| {
            ui.label("Profile name");ui.text_edit_singleline(&mut c.name);
            ui.add(egui::Slider::new(&mut c.face_loss_ms,0.0..=5000.0).text("Hold last face after signal loss (ms)"));
            ui.small("After this extra delay, outputs requiring a face reset to their saved defaults and clear modifier history. Microphone-only and procedural outputs can continue when Requires a tracked face is off.");
            ui.checkbox(&mut c.invert_pitch,"Invert imported pitch (up/down)");
            ui.checkbox(&mut c.invert_yaw,"Invert imported yaw (left/right)");
            ui.checkbox(&mut c.invert_roll,"Invert imported roll (lean)");
        });
    }
    fn curve(&mut self, ui: &mut egui::Ui, c: &mut Curve, range: (f32, f32)) {
        ui.horizontal_wrapped(|ui| {
            for (label, tangent) in [("Linear", 1.0), ("Smooth S", 0.0)] {
                if ui.small_button(label).clicked() && range.1 > range.0 {
                    c.keys = vec![
                        key(range.0, range.0, tangent),
                        key(range.1, range.1, tangent),
                    ];
                }
            }
            if ui.small_button("Identity / clear").clicked() {
                *c = Curve::default();
            }
            if ui.small_button("Copy curve").clicked() {
                self.copied_curve = Some(c.clone());
            }
            if ui
                .add_enabled(
                    self.copied_curve.is_some(),
                    egui::Button::new("Paste curve"),
                )
                .clicked()
            {
                *c = self.copied_curve.clone().unwrap();
            }
        });
        plot(ui, c, range);
        ui.small("Drag curve points, or edit Time (input), Value (output), and tangents below. Weighted handles use fractions from 0 to 1. Curves are clamped outside the endpoints unless Loop or Ping-pong is chosen.");
        let mut remove = None;
        for (i, k) in c.keys.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                ui.horizontal_wrapped(|ui| {
                    number(ui, "Time", &mut k.time, -1e6..=1e6);
                    number(ui, "Value", &mut k.value, -1e6..=1e6);
                    number(ui, "In tangent", &mut k.incoming, -1e6..=1e6);
                    number(ui, "Out tangent", &mut k.outgoing, -1e6..=1e6);
                    if ui.small_button("×").clicked() {
                        remove = Some(i);
                    }
                });
                egui::CollapsingHeader::new("Weights").show(ui, |ui| {
                    egui::ComboBox::from_id_salt("weight-mode")
                        .selected_text(
                            ["None", "In", "Out", "Both"][usize::from(k.weighted.min(3))],
                        )
                        .show_ui(ui, |ui| {
                            for (i, n) in ["None", "In", "Out", "Both"].iter().enumerate() {
                                ui.selectable_value(&mut k.weighted, i as u8, *n);
                            }
                        });
                    number(ui, "In weight", &mut k.in_weight, 0.0..=1.0);
                    number(ui, "Out weight", &mut k.out_weight, 0.0..=1.0);
                });
            });
        }
        if let Some(i) = remove {
            c.keys.remove(i);
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(c.keys.len() < 64, egui::Button::new("Add point"))
                .clicked()
            {
                let x = c.keys.last().map_or(range.0, |k| k.time + 0.1);
                c.keys.push(key(x, x, 1.0));
            }
            if ui.button("Sort points").clicked() {
                c.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
            }
            for (id, value) in [("Before", &mut c.pre_wrap), ("After", &mut c.post_wrap)] {
                egui::ComboBox::from_id_salt(id)
                    .selected_text(format!("{id}: {}", wrap_name(*value)))
                    .show_ui(ui, |ui| {
                        for (v, n) in [(8, "Clamp"), (2, "Loop"), (4, "Ping-pong")] {
                            ui.selectable_value(value, v, n);
                        }
                    });
            }
        });
        if let Err(e) = c.validate() {
            ui.colored_label(crate::theme::orange(), format!("Fix before applying: {e}"));
        }
    }
}
fn wrap_name(v: u8) -> &'static str {
    match v {
        2 => "Loop",
        4 => "Ping-pong",
        _ => "Clamp",
    }
}
fn key(time: f32, value: f32, tangent: f32) -> Key {
    Key {
        time,
        value,
        incoming: tangent,
        outgoing: tangent,
        weighted: 0,
        in_weight: 1.0 / 3.0,
        out_weight: 1.0 / 3.0,
    }
}
fn number(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.add(
        egui::DragValue::new(value)
            .speed(0.01)
            .range(range)
            .prefix(format!("{label} ")),
    );
}
fn plot(ui: &mut egui::Ui, c: &mut Curve, range: (f32, f32)) {
    if c.validate().is_err() {
        return;
    }
    let lo = c.keys.first().map_or(range.0, |k| k.time);
    let hi = c.keys.last().map_or(range.1, |k| k.time);
    if hi - lo <= 1e-6 {
        return;
    }
    let ymin = c.keys.iter().map(|k| k.value).fold(range.0, f32::min);
    let ymax = c
        .keys
        .iter()
        .map(|k| k.value)
        .fold(range.1, f32::max)
        .max(ymin + 1e-3);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().min(530.0), 115.0),
        egui::Sense::drag(),
    );
    let plot = rect.shrink(9.0);
    let position = |x: f32, y: f32| {
        egui::pos2(
            plot.left() + (x - lo) / (hi - lo) * plot.width(),
            plot.bottom() - (y - ymin) / (ymax - ymin) * plot.height(),
        )
    };
    ui.painter().rect_filled(rect, 5.0, crate::theme::bg());
    let points: Vec<_> = (0..=80)
        .map(|i| {
            let x = lo + (hi - lo) * i as f32 / 80.0;
            position(x, c.evaluate(x).clamp(ymin, ymax))
        })
        .collect();
    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0, crate::theme::teal()),
    ));
    for k in &c.keys {
        ui.painter()
            .circle_filled(position(k.time, k.value), 4.0, crate::theme::pink());
    }
    if response.drag_started()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let nearest = c
            .keys
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                position(a.time, a.value)
                    .distance_sq(pointer)
                    .total_cmp(&position(b.time, b.value).distance_sq(pointer))
            })
            .map(|(i, _)| i);
        if let Some(i) = nearest {
            ui.ctx().data_mut(|d| d.insert_temp(response.id, i));
        }
    }
    if response.dragged()
        && let Some(i) = ui.ctx().data(|d| d.get_temp::<usize>(response.id))
        && i < c.keys.len()
        && let Some(p) = response.interact_pointer_pos()
    {
        let x = lo + (p.x - plot.left()) / plot.width() * (hi - lo);
        let y = ymin + (plot.bottom() - p.y) / plot.height() * (ymax - ymin);
        let low = if i > 0 { c.keys[i - 1].time + 1e-4 } else { lo };
        let high = c.keys.get(i + 1).map_or(hi, |k| k.time - 1e-4);
        if low <= high {
            c.keys[i].time = x.clamp(low, high);
            c.keys[i].value = y.clamp(ymin, ymax);
        }
    }
    response.on_hover_text(format!("Input {lo:.3} … {hi:.3} → output {ymin:.3} … {ymax:.3}. Drag a pink point to reshape the response."));
}
