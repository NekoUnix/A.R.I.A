use super::Avatar;
use crate::{
    help,
    input_monitor::{InputMonitor, Tab},
    theme,
};
use eframe::egui;

fn motion(ui: &mut egui::Ui, avatar: &mut Avatar, monitor: &mut InputMonitor) {
    let before = monitor.saved.config.vrm.motion.clone();
    let frozen = monitor.saved.config.pose.mode == aria_core::movement::PoseMode::Frozen;
    let settings = &mut monitor.saved.config.vrm.motion;
    theme::category(ui, "vrm-life", "Life & idle movement", true, |ui| {
        help::button(ui, "vrm-motion");
        help::control(ui, "vrm-motion", |ui| {
            ui.checkbox(&mut settings.enabled, "Natural idle movement")
        });
        for (value, label) in [
            (&mut settings.sway, "Body sway"),
            (&mut settings.breathing, "Breathing"),
            (&mut settings.arms, "Arms & elbows"),
        ] {
            help::control(ui, "vrm-motion", |ui| {
                ui.add(egui::Slider::new(value, 0.0..=2.0).text(label))
            });
        }
        help::control(ui, "vrm-motion", |ui| {
            ui.add(egui::Slider::new(&mut settings.speed, 0.25..=2.0).text("Idle speed"))
        });
        theme::caption(
            ui,
            "Subtle movement adds to face tracking and feeds the avatar's spring bones. Changes blend smoothly; zero gently stops that motion. Tune Relax arms in Pose controls for the resting arm position.",
        );
    });
    theme::category(ui, "vrm-gestures", "Gesture animations", true, |ui| {
        help::button(ui, "vrm-motion");
        if frozen {
            ui.label("Resume the pose to play animations. Freeze captures the current gesture and secondary motion.");
        }
        ui.add_enabled_ui(!frozen, |ui| {
            ui.horizontal_wrapped(|ui| {
                for gesture in super::motion::Gesture::ALL {
                    if ui
                        .selectable_label(avatar.motion.current == Some(gesture), gesture.name())
                        .clicked()
                    {
                        avatar.motion.play(gesture);
                    }
                }
                if ui.button("Stop / return to tracking").clicked() {
                    avatar.motion.stop();
                }
            });
        });
        help::control(ui, "vrm-motion", |ui| {
            ui.add(
                egui::Slider::new(&mut settings.gesture_strength, 0.0..=2.0)
                    .text("Gesture strength"),
            )
        });
        help::control(ui, "vrm-motion", |ui| {
            ui.add(egui::Slider::new(&mut settings.gesture_speed, 0.25..=2.0).text("Gesture speed"))
        });
        help::control(ui, "vrm-motion", |ui| {
            ui.checkbox(&mut settings.gesture_loop, "Repeat gestures")
        });
        theme::caption(
            ui,
            "Click a gesture to play or restart it. Switching and stopping blend over 0.3 seconds. Settings save with this model and its presets; gestures do not start automatically when importing a model. Missing optional bones are skipped. These are procedural gestures, without hand/body capture or collision avoidance.",
        );
    });
    monitor.save_requested |= before != *settings;
}

pub fn details(ui: &mut egui::Ui, avatar: &Avatar) {
    ui.collapsing("Model details", |ui| {
        help::button(
            ui,
            if avatar.asset.summary.is_glb() {
                "glb-import"
            } else {
                "vrm-import"
            },
        );
        let s = &avatar.asset.summary;
        ui.label(format!("Author: {}", s.author));
        ui.label(format!("License declared in file: {}", s.license));
        ui.small(format!(
            "{} unique vertices · {} draw sections · {:.0} MiB GPU textures, meshes and canvas",
            avatar
                .asset
                .geometry
                .iter()
                .map(|g| g.vertices.len())
                .sum::<usize>(),
            avatar.asset.parts.len(),
            avatar.atlas_mib()
        ));
        ui.small(avatar.asset.path.display().to_string());
        for note in &avatar.asset.warnings {
            ui.label(note);
        }
    });
}
pub fn view(ui: &mut egui::Ui, avatar: &mut Avatar, monitor: &mut InputMonitor) {
    if avatar.asset.summary.is_glb() {
        glb_rig(ui, avatar, monitor);
    }
    motion(ui, avatar, monitor);
    let before = monitor.saved.config.vrm.clone();
    let settings = &mut monitor.saved.config.vrm;
    theme::category(ui, "vrm-camera", "View & framing", true, |ui| {
        help::control(ui, "vrm-view", |ui| {
            ui.add(egui::Slider::new(&mut settings.portrait, 0.0..=1.0).text("Portrait crop"))
        });
        theme::caption(
            ui,
            "0 = full body · 1 = head and shoulders. Drag the avatar and scroll to resize it in each output.",
        );
        help::control(ui, "vrm-view", |ui| {
            ui.add(egui::Slider::new(&mut settings.yaw, -180.0..=180.0).text("Camera orbit"))
        });
        help::control(ui, "vrm-view", |ui| {
            ui.add(egui::Slider::new(&mut settings.pitch, -75.0..=75.0).text("Camera elevation"))
        });
        if ui.button("Reset framing").clicked() {
            settings.yaw = 0.;
            settings.pitch = 0.;
            settings.portrait = 0.;
        }
    });
    theme::category(ui, "vrm-render", "Appearance & performance", true, |ui| {
        help::control(ui, "vrm-view", |ui| {
            egui::ComboBox::from_id_salt("vrm-quality")
                .selected_text(format!("{} px avatar canvas", settings.resolution))
                .show_ui(ui, |ui| {
                    for n in [512, 1024, 1536, 2048, 3072, 4096] {
                        ui.selectable_value(
                            &mut settings.resolution,
                            n,
                            format!("{} × {n}", n * 3 / 4),
                        );
                    }
                })
        });
        theme::caption(
            ui,
            "All outputs share this transparent 3D canvas. Higher resolution improves detail when enlarged and uses more GPU memory; OBS output resolution is configured separately.",
        );
        help::control(ui, "vrm-view", |ui| {
            ui.add(egui::Slider::new(&mut settings.light, 0.25..=2.).text("Toon lighting"))
        });
        help::control(ui, "vrm-view", |ui| {
            ui.checkbox(&mut settings.outlines, "Authored outlines")
        });
        help::control(ui, "vrm-expressions", |ui| {
            ui.checkbox(&mut settings.auto_blink, "Automatic blinking")
        });
    });
    monitor.save_requested |= before != *settings;
    theme::category(ui, "vrm-posing", "Tracking & posing", false, |ui| {
        ui.label("Phone face angles drive head/neck rotation. Eye opening drives blink expressions, mouth opening drives A / aa, and gaze rotates the eyes or uses look expressions. Matching ARKit expressions get individual input assignments.");
        ui.label("Relax arms is a parameter in Inputs and Pose controls. Lower it toward 0° for a T-pose; raise it to lower the arms. Freeze a pose before exporting a screenshot.");
        if ui.button("Open pose controls").clicked() {
            monitor.tab = Tab::Pose;
        }
        if ui.button("Tracking inputs").clicked() {
            monitor.tab = Tab::Inputs;
        }
    });
    details(ui, avatar);
}

fn glb_rig(ui: &mut egui::Ui, avatar: &Avatar, monitor: &mut InputMonitor) {
    theme::category(
        ui,
        "glb-rig",
        "GLB rig & tracking · experimental",
        true,
        |ui| {
            help::button(ui, "glb-import");
            monitor.save_requested |= help::control(ui, "glb-import", |ui| {
                ui.checkbox(
                    &mut monitor.saved.config.vrm.reverse_forward,
                    "Reverse avatar forward direction",
                )
            })
            .changed();
            ui.label("Uses your current tracking source and calibration. Review head turns, nods, blinks and speech, then save the profile. No second tracker is needed.");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Set up personal tracking…").clicked() {
                    monitor.setup_tracking_requested = true;
                }
                if ui.button("Edit all shape inputs…").clicked() {
                    monitor.tab = Tab::Inputs;
                }
            });
            ui.collapsing("Humanoid bone assignments", |ui| {
            ui.small("Auto matches common Unity, Blender and Mixamo bone names. Choose another node or disable a motion here. These choices belong only to this avatar.");
            let before = monitor.saved.config.vrm.bone_map.clone();
            for &(bone, _) in super::glb::BONES {
                let auto = avatar.detected_bones.get(bone).copied();
                let selected = monitor.saved.config.vrm.bone_map.get(bone).copied();
                let mut value = selected;
                let label = |node: usize| avatar.asset.nodes.get(node).map_or_else(|| format!("Missing node {node}"), |n| format!("{} · #{node}", n.name));
                ui.horizontal(|ui| {
                    ui.label(bone);
                    egui::ComboBox::from_id_salt(("glb-bone", bone)).width(210.)
                        .selected_text(match selected { None => format!("Auto: {}", auto.map_or_else(|| "not found".into(), label)), Some(None) => "Disabled".into(), Some(Some(n)) => label(n) })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut value, None, "Auto detect");
                            ui.selectable_value(&mut value, Some(None), "Disabled");
                            for (n, _) in avatar.asset.nodes.iter().enumerate() { ui.selectable_value(&mut value, Some(Some(n)), label(n)); }
                        });
                });
                if value != selected { match value { None => { monitor.saved.config.vrm.bone_map.remove(bone); }, Some(n) => { monitor.saved.config.vrm.bone_map.insert(bone.into(), n); } } }
            }
            if ui.button("Restore detected bones").clicked() { monitor.saved.config.vrm.bone_map.clear(); }
            monitor.save_requested |= before != monitor.saved.config.vrm.bone_map;
        });
            ui.collapsing("Face input assignments", |ui| {
            ui.small("Choose a shape for each common input. Eyes use 1 − eye opening to close the eyelid. Inputs offers additional ARKit/controller channels, ranges, smoothing and multiple shapes per input.");
            for (title, input, inverted) in [("Mouth open", "ParamMouthOpenY", false), ("Left eyelid", "ParamEyeLOpen", true), ("Right eyelid", "ParamEyeROpen", true), ("Smile", "MouthSmile", false), ("Brows", "Brows", false)] {
                let selected = avatar.asset.expressions.iter().enumerate().find(|(i, _)| monitor.saved.config.bindings.get(&super::expression_id(*i)).is_some_and(|b| b.input == input)).map(|(i, _)| i);
                let mut value = selected;
                ui.horizontal(|ui| {
                    ui.label(title);
                    egui::ComboBox::from_id_salt(("glb-face", input)).width(210.)
                        .selected_text(selected.map_or("Unassigned", |i| avatar.asset.expressions[i].name.as_str()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut value, None, "Unassigned");
                            for (i, shape) in avatar.asset.expressions.iter().enumerate() { ui.selectable_value(&mut value, Some(i), &shape.name); }
                        });
                });
                if value != selected {
                    monitor.saved.config.bindings.retain(|id, b| !(id.starts_with("VRMExpression:") && b.input == input));
                    if let Some(i) = value {
                        let mut b = aria_core::rig::Binding::direct(input, 0., 1.);
                        if inverted { b.output_min = 1.; b.output_max = 0.; }
                        monitor.saved.config.bindings.insert(super::expression_id(i), b);
                    }
                    monitor.save_requested = true;
                }
            }
            if ui.button("Restore detected face inputs").clicked() {
                monitor.saved.config.bindings.retain(|id, _| !id.starts_with("VRMExpression:"));
                monitor.saved.config.bindings.extend(super::glb::bindings(&avatar.asset.expressions));
                monitor.save_requested = true;
            }
            if avatar.asset.expressions.is_empty() { ui.label("No exported morph targets. Head and body tracking still work; export facial shape keys to enable mouth and eyelid control."); }
        });
        },
    );
}
pub fn physics(ui: &mut egui::Ui, avatar: &mut Avatar, monitor: &mut InputMonitor) {
    let before = monitor.saved.config.physics.clone();
    let before_secondary = monitor.saved.config.vrm.secondary.clone();
    let settings = &mut monitor.saved.config.physics;
    let secondary = &mut monitor.saved.config.vrm.secondary;
    theme::caption(
        ui,
        "Medium is the starting setting for every VRM / GLB avatar. Hair, tails, ears and clothing follow tracking and idle motion. Changes save automatically with this avatar; Save profile and movement presets keep them too.",
    );
    theme::category(
        ui,
        "vrm-spring-overall",
        "Overall secondary motion",
        true,
        |ui| {
            help::button(ui, "vrm-physics");
            ui.checkbox(&mut settings.enabled, "Enable spring bones");
            ui.horizontal_wrapped(|ui| {
            for (name, damping, swing, response) in [("Soft", 0.08, 65., 0.65), ("Medium", 0.15, 45., 1.), ("Firm", 0.35, 25., 1.5)] {
                if ui.button(name).on_hover_text("Apply this preset to overall physics and all groups. Detection and manual roots are kept.").clicked() {
                    *settings = Default::default(); settings.response = response;
                    secondary.tuning = aria_core::vrm::SpringTuning { damping, swing, collisions: true };
                    secondary.groups.clear(); monitor.reset_motion = true;
                }
            }
        });
            sliders(
                ui,
                &mut settings.strength,
                &mut settings.inertia,
                &mut settings.response,
                &mut settings.gravity,
                &mut settings.wind,
            );
            spring_tuning(ui, &mut secondary.tuning);
            ui.collapsing("Body & self collision", |ui| {
                help::button(ui, "vrm-physics");
                ui.checkbox(&mut secondary.collision.body, "Generate body collision shapes");
                ui.checkbox(&mut secondary.collision.other_groups, "Collide with other flexible groups");
                ui.add(egui::Slider::new(&mut secondary.collision.body_size, 0.5..=1.5).text("Body collision size"));
                ui.add(egui::Slider::new(&mut secondary.collision.thickness, 0.0..=0.5).text("Spring thickness"));
                let body_shapes = avatar.spring_groups.iter().find(|g| g.id.starts_with("aria:")).map_or(0, |g|
                    g.colliders.iter().filter(|c| avatar.asset.bones.values().any(|&n| n == c.node)).count());
                ui.small(format!("{body_shapes} fitted body shapes available to generated groups"));
                if body_shapes == 0 && secondary.collision.body {
                    ui.small("No body shapes found yet. For GLB, review the humanoid bone assignments; body bones need weighted mesh vertices. Authored VRM groups use their exported shapes.");
                }
                ui.small("On by default for generated physics. Fits head, torso and limb shapes from the weighted rig; flexible parts use up to 256 moving envelopes. Existing overlaps in the posed chain are retained. Contact recovery is gradual to avoid kicks, and sliding motion is preserved. Increase size/thickness gently; authored VRM shapes stay unchanged.");
                ui.small(format!("{} contact corrections in the latest physics update", avatar.springs.contacts));
                ui.small("These are bone collision envelopes, not full cloth or triangle-mesh self collision. Tracking and gestures still pose the rigid skeleton; hands and arms are not repositioned by spring physics.");
            });
            if ui
                .button("Settle motion")
                .on_hover_text("Return springs to rest without changing your saved settings.")
                .clicked()
            {
                monitor.reset_motion = true;
            }
            ui.collapsing("Find secondary bones", |ui| {
                super::secondary_panel::show(ui, avatar, secondary);
            });
        },
    );
    let groups = &avatar.spring_groups;
    let generated = groups.iter().filter(|g| g.id.starts_with("aria:")).count();
    ui.label(format!(
        "{} exported · {} automatically / manually added groups",
        groups.len() - generated,
        generated
    ));
    if groups.is_empty() {
        ui.label("No flexible bones found. Review the bone inspector above, or import a skinned rig with flexible bones.");
    }
    theme::category(
        ui,
        "vrm-spring-groups",
        &format!("Spring groups · {}", groups.len()),
        false,
        |ui| {
            for group in groups {
                ui.push_id(&group.id, |ui| ui.collapsing(&group.name, |ui| {
                help::context_button(ui, "vrm-physics", || format!("{}\n{} joints · {} collision shapes\nProfile ID: {}\nExported center: {:?}", group.name, group.joints.len(), group.colliders.len(), group.id, group.center));
                let mut tuning = settings.groups.get(&group.id).copied().unwrap_or_default();
                let previous = tuning;
                ui.small(if group.id.starts_with("aria:") { "ARIA generated chain · Medium defaults" } else { "Exported VRM chain · retains authored stiffness and colliders" });
                ui.checkbox(&mut tuning.enabled, "Enable this group");
                sliders(ui, &mut tuning.strength, &mut tuning.inertia, &mut tuning.response, &mut tuning.gravity, &mut tuning.wind);
                let mut override_tuning = secondary.groups.contains_key(&group.id);
                if ui.checkbox(&mut override_tuning, "Custom damping, swing and collisions").changed() {
                    if override_tuning { secondary.groups.insert(group.id.clone(), secondary.tuning); }
                    else { secondary.groups.remove(&group.id); }
                }
                if let Some(advanced) = secondary.groups.get_mut(&group.id) { spring_tuning(ui, advanced); }
                if ui.button("Reset group to overall settings").clicked() {
                    tuning = Default::default(); secondary.groups.remove(&group.id); monitor.reset_motion = true;
                }
                if tuning != previous { settings.groups.insert(group.id.clone(), tuning); }
            }));
            }
        },
    );
    if ui.button("Save physics for this avatar").clicked() {
        monitor.save_requested = true;
    }
    monitor.save_requested |= before != *settings || before_secondary != *secondary;
}
fn spring_tuning(ui: &mut egui::Ui, settings: &mut aria_core::vrm::SpringTuning) {
    help::control(ui, "vrm-physics", |ui| {
        ui.add(egui::Slider::new(&mut settings.damping, 0.0..=1.0).text("Extra damping"))
    });
    help::control(ui, "vrm-physics", |ui| {
        ui.add(egui::Slider::new(&mut settings.swing, 0.0..=90.0).text("Maximum bend (degrees)"))
    });
    help::control(ui, "vrm-physics", |ui| {
        ui.checkbox(&mut settings.collisions, "Use collision shapes")
    });
}
fn sliders(
    ui: &mut egui::Ui,
    strength: &mut f32,
    inertia: &mut f32,
    response: &mut f32,
    gravity: &mut f32,
    wind: &mut f32,
) {
    for (value, label, min, max) in [
        (strength, "Motion strength", 0., 2.),
        (inertia, "Inertia", 0., 2.),
        (response, "Stiffness / response", 0.25, 2.),
        (gravity, "Gravity multiplier", 0., 2.),
        (wind, "Side wind", -1., 1.),
    ] {
        help::control(ui, "vrm-physics", |ui| {
            ui.add(egui::Slider::new(value, min..=max).text(label))
        });
    }
}
