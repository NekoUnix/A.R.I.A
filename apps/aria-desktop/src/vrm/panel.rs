use super::{Avatar, spring::Group};
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
            "Subtle movement adds to face tracking and feeds the avatar's spring bones. Set a strength to zero to disable that motion. Tune Relax arms in Pose controls for the resting arm position.",
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
        help::button(ui, "vrm-import");
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
pub fn physics(ui: &mut egui::Ui, groups: &[Group], monitor: &mut InputMonitor) {
    let before = monitor.saved.config.physics.clone();
    let settings = &mut monitor.saved.config.physics;
    theme::caption(
        ui,
        "The groups below come from the current VRM. Global and group values multiply the avatar's authored settings. Save profile and movement presets include all spring settings.",
    );
    if groups.is_empty() {
        ui.label("This VRM does not export any spring-bone groups.");
        return;
    }
    theme::category(
        ui,
        "vrm-spring-overall",
        "Overall secondary motion",
        true,
        |ui| {
            help::control(ui, "vrm-physics", |ui| {
                ui.checkbox(&mut settings.enabled, "Enable spring bones")
            });
            sliders(
                ui,
                &mut settings.strength,
                &mut settings.inertia,
                &mut settings.response,
                &mut settings.gravity,
                &mut settings.wind,
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("Settle motion").clicked() {
                    monitor.reset_motion = true;
                }
                if ui.button("Restore authored physics").clicked() {
                    *settings = Default::default();
                    monitor.reset_motion = true;
                }
            });
        },
    );
    theme::category(
        ui,
        "vrm-spring-groups",
        &format!("Spring groups · {}", groups.len()),
        true,
        |ui| {
            for group in groups {
                ui.push_id(&group.id,|ui|ui.collapsing(&group.name,|ui|{
                let mut tuning=settings.groups.get(&group.id).copied().unwrap_or_default();let previous=tuning;
                help::context_button(ui,"vrm-physics",||format!("Group: {}\n{} simulated joints · {} sphere/capsule colliders\nStable profile ID: {}\nCenter node: {:?}\n\nAuthored per-joint values:\n{}",group.name,group.joints.len(),group.colliders.len(),group.id,group.center,group.joints.iter().map(|j|format!("Node {}: stiffness {}, drag {}, gravity {}, radius {} m",j.node,j.stiffness,j.drag,j.power,j.radius)).collect::<Vec<_>>().join("\n")));
                ui.small(format!("{} joints · {} colliders",group.joints.len(),group.colliders.len()));
                help::control(ui,"vrm-physics",|ui|ui.checkbox(&mut tuning.enabled,"Enable this group"));
                sliders(ui,&mut tuning.strength,&mut tuning.inertia,&mut tuning.response,&mut tuning.gravity,&mut tuning.wind);
                if ui.button("Reset this group").clicked(){tuning=Default::default();monitor.reset_motion=true;}
                if tuning!=previous {settings.groups.insert(group.id.clone(),tuning);}
            }));
            }
        },
    );
    monitor.save_requested |= before != *settings;
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
