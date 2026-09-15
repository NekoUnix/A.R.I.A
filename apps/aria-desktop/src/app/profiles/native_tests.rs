//! Opt-in mixed-asset validation with user-provided assets and the native GPU.
use super::*;
#[test]
#[cfg(windows)]
#[ignore = "requires ARIA_TEST_GLB, ARIA_TEST_RENDER_DIR and a DX12 GPU"]
fn native_glb_profile_uses_shared_tracking_and_renders_workspace() {
    let ctx = egui::Context::default();
    let state = crate::spout::tests::gpu_state();
    let mut cc = eframe::CreationContext::_new_kittest(ctx.clone());
    cc.wgpu_render_state = Some(state.clone());
    let mut app = AriaApp::new(&cc);
    let path = PathBuf::from(std::env::var_os("ARIA_TEST_GLB").unwrap());
    app.open_model(&path);
    let deadline = Instant::now() + Duration::from_secs(90);
    while app.pending_vrm.is_some() {
        assert!(Instant::now() < deadline, "GLB import timeout");
        app.poll_vrm_import();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        app.avatar_kind(),
        Some(crate::avatar_import::Kind::Glb),
        "{:?}",
        app.status_message
    );
    let id = app.profiles.current.unwrap();
    assert_eq!(app.settings.profiles.entries.len(), 1);
    app.rename_profile(id, "ICHIGO · GLB tracking");
    app.settings.source = Source::Demo;
    app.importer.open = false;
    app.controls_page = ControlsPage::Avatar;
    app.input_monitor.tab = Tab::Vrm;
    for _ in 0..3 {
        app.sample_profile_tracking(1. / 60.);
        app.advance_profiles(&ctx, 1. / 60.);
    }
    assert!(app.raw.is_some());
    assert!(
        app.current_parameters()
            .iter()
            .any(|p| p.id == "ParamAngleX" && p.value != 0.)
    );
    assert_eq!(app.current_support_snapshot()["avatar"]["kind"], "VRC/GLB");
    assert!(app.action_choices().iter().any(|choice| matches!(
        &choice.target,
        crate::actions::Target::Avatar {
            command: crate::actions::Command::Gesture(_),
            ..
        }
    )));
    let output_dir = PathBuf::from(std::env::var_os("ARIA_TEST_RENDER_DIR").unwrap());
    std::fs::create_dir_all(&output_dir).unwrap();
    struct Artwork(Vec<egui::epaint::ClippedShape>);
    impl crate::output::PaintScene for Artwork {
        fn paint(&self, painter: &egui::Painter, _: egui::Rect, _: &crate::output::CanvasSettings) {
            for shape in &self.0 {
                painter
                    .with_clip_rect(shape.clip_rect)
                    .add(shape.shape.clone());
            }
        }
    }
    for _ in 0..2 {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1500., 980.),
                )),
                ..Default::default()
            },
            |root| {
                eframe::App::ui(&mut app, root, &mut eframe::Frame::_new_kittest());
            },
        );
        {
            let mut renderer = state.renderer.write();
            for (id, deltas) in &output.textures_delta.set {
                for delta in deltas {
                    renderer.update_texture(&state.device, &state.queue, *id, delta);
                }
            }
            for id in &output.textures_delta.free {
                renderer.free_texture(id);
            }
        }
        output.textures_delta.clear();
        crate::broadcast::save_canvas_png(
            &ctx,
            &state,
            &Artwork(output.shapes),
            &Default::default(),
            [1500, 980],
            &output_dir.join("glb-workspace.png"),
        )
        .unwrap();
    }
    for (index, size) in [[1280, 720], [720, 1280], [1000, 800]]
        .into_iter()
        .enumerate()
    {
        let mut config = app.outputs.snapshot().canvas(index).clone();
        config.background = Background::Transparent;
        let path = output_dir.join(format!("glb-output-{index}.png"));
        crate::broadcast::save_canvas_png(&ctx, &state, &app.composition(), &config, size, &path)
            .unwrap();
        assert!(
            image::open(path)
                .unwrap()
                .into_rgba8()
                .pixels()
                .filter(|p| p.0[3] > 20)
                .count()
                > 1000
        );
    }
}

#[test]
#[cfg(windows)]
#[ignore = "requires Cubism Core, ARIA_TEST_MODEL, ARIA_TEST_SECOND_MODEL, ARIA_TEST_GIF and ARIA_TEST_VRM"]
fn native_mixed_workspace_keeps_four_avatars_live_and_composes_all_outputs() {
    let ctx = egui::Context::default();
    let state = crate::spout::tests::gpu_state();
    let mut cc = eframe::CreationContext::_new_kittest(ctx.clone());
    cc.wgpu_render_state = Some(state.clone());
    let mut app = AriaApp::new(&cc);
    let paths: Vec<_> = [
        "ARIA_TEST_MODEL",
        "ARIA_TEST_SECOND_MODEL",
        "ARIA_TEST_GIF",
        "ARIA_TEST_VRM",
    ]
    .into_iter()
    .map(|name| PathBuf::from(std::env::var_os(name).expect(name)))
    .collect();
    for path in &paths[..2] {
        app.import_model(aria_model::load_files(path).unwrap())
            .unwrap();
        eprintln!("Loaded Live2D {}", path.display());
    }
    let gif = crate::media::load(&ctx, Some(&state), &paths[2], 0, false).unwrap();
    eprintln!("Decoded supplied GIF");
    assert!(gif.animation.is_some());
    app.apply_image(&paths[2], false, gif, 256);
    app.begin_vrm(&paths[3]);
    let deadline = Instant::now() + Duration::from_secs(90);
    while app.pending_vrm.is_some() {
        assert!(Instant::now() < deadline, "VRM import timeout");
        app.poll_vrm_import();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(app.vrm.is_some(), "VRM import: {:?}", app.status_message);
    eprintln!("Loaded VRM; beginning mixed simulation");
    assert_eq!(app.settings.profiles.entries.len(), 4);
    assert_eq!(app.profiles.parked.len(), 3);
    let ids: Vec<_> = app.settings.profiles.entries.iter().map(|e| e.id).collect();
    let first_pid = app.profiles.parked[&ids[0]]
        .live2d
        .as_ref()
        .unwrap()
        .model
        .process_id();
    let second_pid = app.profiles.parked[&ids[1]]
        .live2d
        .as_ref()
        .unwrap()
        .model
        .process_id();
    assert_ne!(first_pid, second_pid);
    let output_dir = PathBuf::from(std::env::var_os("ARIA_TEST_RENDER_DIR").unwrap());
    std::fs::create_dir_all(&output_dir).unwrap();
    // Warm images, physics, live tracking, VRM pose and all independent render targets.
    for _ in 0..45 {
        app.sample_profile_tracking(1. / 60.);
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
            app.advance_profiles(&ctx, 1. / 60.)
        });
        let mut renderer = state.renderer.write();
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                renderer.update_texture(&state.device, &state.queue, *id, delta);
            }
        }
        for id in &output.textures_delta.free {
            renderer.free_texture(id);
        }
        output.textures_delta.clear();
    }
    for id in &ids {
        app.with_profile(*id, |app| {
            assert!(app.animation_time > 0.7);
            assert!(app.raw.is_some());
            assert!(app.current_parameters().iter().all(|p| p.value.is_finite()));
        });
    }
    eprintln!("Mixed simulation complete");
    for (index, size) in [[1280, 720], [720, 1280], [1000, 800]]
        .into_iter()
        .enumerate()
    {
        for (slot, id) in ids.iter().enumerate() {
            app.outputs.edit_canvas(index, |c| {
                c.background = Background::Transparent;
                c.avatars.insert(
                    *id,
                    crate::output::Transform {
                        position: [-0.375 + slot as f32 * 0.25, 0.],
                        zoom: if index == 0 {
                            [0.65, 0.62, 0.55, 0.65][slot]
                        } else {
                            0.32
                        },
                    },
                );
            });
        }
        let composition = app.composition();
        assert_eq!(composition.avatars.len(), 4);
        let config = app.outputs.snapshot().canvas(index).clone();
        let path = output_dir.join(format!("mixed-{index}.png"));
        crate::broadcast::save_canvas_png(&ctx, &state, &composition, &config, size, &path)
            .unwrap();
        let pixels = image::open(&path).unwrap().into_rgba8();
        for slot in 0..4 {
            let count = pixels
                .enumerate_pixels()
                .filter(|(x, _, p)| {
                    *x >= slot * size[0] / 4 && *x < (slot + 1) * size[0] / 4 && p.0[3] > 20
                })
                .count();
            assert!(
                count > 100,
                "Each avatar must reach the full resolution canvas: canvas {index}, slot {slot}, {count} pixels"
            );
        }
    }
    // Render the real workspace UI to a documentation image without native input automation.
    app.controls_page = ControlsPage::Profiles;
    app.importer.open = false;
    app.focus_profile(ids[3]);
    let ui_size = [1500, 980];
    struct UiArtwork(Vec<egui::epaint::ClippedShape>);
    impl crate::output::PaintScene for UiArtwork {
        fn paint(&self, painter: &egui::Painter, _: egui::Rect, _: &crate::output::CanvasSettings) {
            for shape in &self.0 {
                painter
                    .with_clip_rect(shape.clip_rect)
                    .add(shape.shape.clone());
            }
        }
    }
    for (theme_index, image_name) in [
        (6, "profiles-ui.png"),
        (7, "glass-light.png"),
        (5, "high-contrast.png"),
        (2, "sakura-solid.png"),
        (2, "action-nodes.png"),
        (2, "hotkey-window.png"),
    ] {
        app.settings.theme.active = crate::theme::presets().remove(theme_index);
        if theme_index == 2 {
            app.settings.theme.active.colors.glass = false;
            app.focus_profile(ids[0]);
            app.rename_profile(ids[0], "Odette · Live2D stage");
            app.controls_page = ControlsPage::Settings;
            if image_name == "action-nodes.png" {
                let mut graph = crate::actions::Graph::new(1);
                graph.name = "Screenshot pose".into();
                graph.nodes[1].step = crate::actions::Step::Action {
                    target: Some(crate::actions::Target::Avatar {
                        profile: ids[0],
                        command: crate::actions::Command::Pose,
                    }),
                    mode: crate::actions::Mode::On,
                };
                app.settings.actions.graphs.push(graph);
                app.action_editor.edit_graph(1);
            }
            if image_name == "hotkey-window.png" {
                app.action_editor.graph_page = false;
            }
        }
        crate::theme::apply(&ctx, app.settings.theme.active.colors);
        app.input_monitor.vts.open = false;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(ui_size[0] as f32, ui_size[1] as f32),
                    )),
                    ..Default::default()
                },
                |root| {
                    eframe::App::ui(&mut app, root, &mut eframe::Frame::_new_kittest());
                },
            );
            {
                let mut renderer = state.renderer.write();
                for (id, deltas) in &output.textures_delta.set {
                    for delta in deltas {
                        renderer.update_texture(&state.device, &state.queue, *id, delta);
                    }
                }
                for id in &output.textures_delta.free {
                    renderer.free_texture(id);
                }
            }
            output.textures_delta.clear();
            crate::broadcast::save_canvas_png(
                &ctx,
                &state,
                &UiArtwork(output.shapes),
                &Default::default(),
                ui_size,
                &output_dir.join(image_name),
            )
            .unwrap();
        }
    }
    // Editing a different stage reuses its runtime rather than reopening Cubism.
    app.focus_profile(ids[0]);
    assert_eq!(app.live2d.as_ref().unwrap().model.process_id(), first_pid);
    app.focus_profile(ids[1]);
    assert_eq!(app.live2d.as_ref().unwrap().model.process_id(), second_pid);
    app.unload_profile(ids[2]);
    assert_eq!(app.composition().avatars.len(), 3);
    assert!(app.profiles.parked[&ids[3]].vrm.is_some());
    eprintln!(
        "Mixed workspace: two independent Cubism workers, animated GIF, VRM; all three canvas formats rendered four live avatars; editor switching reused workers and unload preserved peers."
    );
}
