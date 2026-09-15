use super::*;
fn scene() -> Scene {
    Scene {
        lighting: Default::default(),
        placement: Default::default(),
        images: Default::default(),
        dents: Default::default(),
        effects: Default::default(),
        recoil: [0.; 2],
        _model_lease: None,
        items: Default::default(),
        model: None,
        model_bounds: Rect::NOTHING,
        sprite: None,
        params: Parameters::default(),
    }
}
fn composition() -> Composition {
    Composition {
        avatars: vec![
            Actor {
                id: 1,
                name: "First".into(),
                scene: scene(),
            },
            Actor {
                id: 2,
                name: "Second".into(),
                scene: scene(),
            },
        ],
    }
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
#[test]
fn mouse_drags_only_picked_avatar_in_each_aspect_and_wheel_respects_lock() {
    for size in [
        egui::vec2(960., 540.),
        egui::vec2(540., 960.),
        egui::vec2(777., 555.),
    ] {
        let ctx = egui::Context::default();
        let composition = composition();
        let mut config = CanvasSettings::default();
        config.avatars.insert(
            1,
            Transform {
                position: [-0.25, 0.],
                zoom: 0.6,
            },
        );
        config.avatars.insert(
            2,
            Transform {
                position: [0.25, 0.],
                zoom: 0.6,
            },
        );
        let original = config.avatars[&1];
        let mut time = 0.;
        let mut run = |events: Vec<egui::Event>, config: &mut CanvasSettings| {
            time += 0.1;
            let _ = crate::run_test_ui(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    time: Some(time),
                    ..Default::default()
                },
                |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| {
                            composition.canvas(ui, config, 0);
                        });
                    if root.current_pass_index() == 0 {
                        root.request_discard("verify drag across layout passes");
                    }
                },
            );
        };
        let start = egui::pos2(size.x * 0.75, size.y * 0.52);
        let end = start + egui::vec2(-size.x * 0.1, size.y * 0.1);
        run(vec![egui::Event::PointerMoved(start)], &mut config);
        run(vec![button(start, true)], &mut config);
        run(vec![egui::Event::PointerMoved(end)], &mut config);
        run(vec![button(end, false)], &mut config);
        assert_eq!(config.avatars[&1], original);
        assert!((config.avatars[&2].position[0] - 0.15).abs() < 0.001);
        assert!((config.avatars[&2].position[1] - 0.1).abs() < 0.001);
        let wheel = egui::Event::MouseWheel {
            phase: egui::TouchPhase::Move,
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0., 100.),
            modifiers: egui::Modifiers::NONE,
        };
        run(vec![wheel.clone()], &mut config);
        assert!(config.avatars[&2].zoom > 0.6);
        assert_eq!(config.avatars[&1], original);
        config.locked = true;
        let before = config.clone();
        run(
            vec![
                button(end, true),
                egui::Event::PointerMoved(start),
                button(start, false),
                wheel,
            ],
            &mut config,
        );
        assert_eq!(config.avatars, before.avatars);
    }
}
#[test]
fn overlapping_selection_targets_front_avatar_and_composite_paint_has_one_background() {
    let c = composition();
    let config = CanvasSettings::default();
    let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960., 540.));
    assert_eq!(c.pick(rect.center(), rect, &config), Some(2));
    let ctx = egui::Context::default();
    let output = crate::run_test_ui(
        &ctx,
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |root| {
            c.paint(root.painter(), rect, &config);
        },
    );
    let backgrounds=output.shapes.iter().filter(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect==rect && r.fill==config.background.color(config.key))).count();
    assert_eq!(
        backgrounds, 1,
        "Later avatars must not clear earlier avatars"
    );
}
#[test]
fn transforms_survive_unload_reload_and_remain_independent_between_canvases() {
    let output = OutputWindows::new(OutputSettings::default());
    output.ensure_avatar(10, 0);
    output.ensure_avatar(20, 1);
    output.edit_canvas(0, |c| {
        c.avatars.get_mut(&10).unwrap().position = [0.4, 0.2];
        c.locked_avatars.insert(10);
    });
    output.edit_canvas(1, |c| {
        c.avatars.get_mut(&20).unwrap().zoom = 1.8;
    });
    output.ensure_avatar(10, 1);
    let s = output.snapshot();
    assert_eq!(s.canvas(0).avatars[&10].position, [0.4, 0.2]);
    assert_eq!(s.canvas(1).avatars[&10].position, [0., 0.]);
    assert_eq!(s.canvas(1).avatars[&20].zoom, 1.8);
    assert_eq!(s.canvas(2).avatars[&20].zoom, 0.7);
    assert!(s.canvas(0).locked_avatars.contains(&10));
    assert!(!s.canvas(1).locked_avatars.contains(&10));
    let copy: OutputSettings = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
    assert_eq!(copy.canvas(0).locked_avatars, s.canvas(0).locked_avatars);
    output.forget_avatar(10);
    for index in 0..3 {
        assert!(!output.snapshot().canvas(index).avatars.contains_key(&10));
        assert!(!output.snapshot().canvas(index).locked_avatars.contains(&10));
    }
}

#[test]
fn modifier_click_locks_only_hit_avatar_and_unlock_does_not_drag_or_center() {
    let ctx = egui::Context::default();
    let c = composition();
    let size = egui::vec2(960., 540.);
    let mut config = CanvasSettings::default();
    config.avatars.insert(
        1,
        Transform {
            position: [-0.25, 0.],
            zoom: 0.6,
        },
    );
    config.avatars.insert(
        2,
        Transform {
            position: [0.25, 0.],
            zoom: 0.6,
        },
    );
    let original = config.avatars.clone();
    let mods = egui::Modifiers {
        ctrl: true,
        command: true,
        shift: true,
        ..Default::default()
    };
    let press = |p, down, m| egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed: down,
        modifiers: m,
    };
    let mut time = 0.;
    let mut run = |events, config: &mut CanvasSettings| {
        time += 0.05;
        crate::run_test_ui(
            &ctx,
            egui::RawInput {
                events,
                time: Some(time),
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| {
                        c.canvas(ui, config, 0);
                    });
                if root.current_pass_index() == 0 {
                    root.request_discard("Exercise repeated layout");
                }
            },
        );
    };
    let p = egui::pos2(720., 285.);
    run(vec![egui::Event::PointerMoved(p)], &mut config);
    run(vec![press(p, true, mods)], &mut config);
    run(vec![press(p, false, mods)], &mut config);
    assert_eq!(config.locked_avatars, std::collections::BTreeSet::from([2]));
    run(
        vec![
            press(p, true, egui::Modifiers::NONE),
            egui::Event::PointerMoved(p + egui::vec2(30., 20.)),
            press(p, false, egui::Modifiers::NONE),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0., 100.),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
            },
        ],
        &mut config,
    );
    assert_eq!(config.avatars, original);
    run(
        vec![egui::Event::PointerMoved(p), press(p, true, mods)],
        &mut config,
    );
    run(
        vec![
            egui::Event::PointerMoved(p + egui::vec2(50., 10.)),
            press(p, false, mods),
        ],
        &mut config,
    );
    assert!(config.locked_avatars.is_empty());
    assert_eq!(config.avatars, original);
    config.locked = true;
    run(vec![], &mut config);
    assert!(!config.locked);
    assert_eq!(
        config.locked_avatars,
        std::collections::BTreeSet::from([1, 2])
    );
}
