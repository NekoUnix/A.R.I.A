use super::*;
#[test]
fn asset_resolution_rejects_escape_and_ambiguity() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("a")).unwrap();
    std::fs::create_dir(d.path().join("b")).unwrap();
    std::fs::write(d.path().join("a/x.exp3.json"), "{}").unwrap();
    assert!(asset(d.path(), "../secret.exp3.json", ".exp3.json").is_err());
    assert!(asset(d.path(), "x.exp3.json", ".exp3.json").is_ok());
    std::fs::write(d.path().join("b/x.exp3.json"), "{}").unwrap();
    assert!(asset(d.path(), "x.exp3.json", ".exp3.json").is_err());
    assert!(asset(d.path(), "a/x.exp3.json", ".exp3.json").is_ok());
}
#[test]
#[cfg(windows)]
#[ignore = "local model collection, Core and GPU; no artwork redistributed"]
fn local_collection_import_apply_undo_and_render() {
    let state = crate::spout::tests::gpu_state();
    let core = std::env::var_os("ARIA_CUBISM_CORE").unwrap();
    let inventory = std::env::var_os("ARIA_VTS_INVENTORY").unwrap();
    let rows: serde_json::Value =
        serde_json::from_slice(&std::fs::read(inventory).unwrap()).unwrap();
    for row in rows.as_array().unwrap() {
        let path = Path::new(row["path"].as_str().unwrap());
        let files =
            aria_model::load_files(path).unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        let mut avatar = crate::live2d::Avatar::load(&state, Path::new(&core), files).unwrap();
        let mut saved = SavedRig {
            config: avatar.initial_config.clone(),
            ..Default::default()
        };
        let before = serde_json::to_vec(&saved).unwrap();
        let mut panel = Panel::default();
        panel.offer(path, &avatar, &saved);
        let p = panel
            .preview
            .as_ref()
            .unwrap_or_else(|| panic!("{}: {:?}", path.display(), panel.error));
        println!(
            "{}: actions {}, scenes {}, expressions {}, notices {}",
            avatar.name,
            p.imported.config.actions.len(),
            p.imported.config.scenes.len(),
            p.expressions.len(),
            p.imported.config.notes.len()
        );
        assert_eq!(
            before,
            serde_json::to_vec(&saved).unwrap(),
            "Preview changed profile"
        );
        if path.to_string_lossy().contains("NekoUnix Full Model")
            || path.to_string_lossy().contains("Neko Unix Chibi Model")
        {
            assert_eq!(
                p.imported.config.scenes.len(),
                6,
                "All six supplied scenes, including folder items, must resolve"
            );
        }
        panel.apply(&mut avatar, &mut saved).unwrap();
        saved.validate(avatar.model.parameters()).unwrap();
        let restored: SavedRig =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        restored.validate(avatar.model.parameters()).unwrap();
        let mut expressions = crate::expressions_panel::ExpressionsPanel::default();
        expressions.load(
            &avatar.files.expressions,
            avatar.files.source.parent().unwrap(),
            &saved,
            avatar.model.parameters(),
        );
        expressions.motions.parts = avatar.model.parts.clone();
        let mut items = crate::items::Items::default();
        for action in saved.vts.actions.clone() {
            match action.action {
                Action::Expression(_)
                | Action::Animation { .. }
                | Action::MoveModel(_)
                | Action::TogglePhysics
                | Action::ClearExpressions
                | Action::HideItems => panel.requests.push(action.id),
                Action::ItemScene { file, .. } if saved.vts.scenes.contains_key(&file) => {
                    panel.requests.push(action.id)
                }
                _ => continue,
            };
            panel.advance(
                &eframe::egui::Context::default(),
                &mut avatar,
                &mut saved,
                &mut expressions,
                &Default::default(),
                None,
                1. / 60.,
            );
            assert!(panel.error.is_none(), "{:?}", panel.error);
            items.sync_assets(
                &eframe::egui::Context::default(),
                Some(&state),
                &saved.config,
            );
            items
                .models
                .sync(Some(&state), Path::new(&core), &saved.config);
            items
                .models
                .update(&mut saved.config, &Default::default(), 1. / 60.);
            for item in &saved.config.items {
                assert!(
                    items.error(&item.path).is_none(),
                    "Image item {}: {:?}",
                    item.name,
                    items.error(&item.path)
                );
                assert!(
                    items.models.error(item.id).is_none(),
                    "Live2D item {}: {:?}",
                    item.name,
                    items.models.error(item.id)
                );
            }
            avatar
                .update(
                    &Default::default(),
                    &mut saved.config,
                    &mut expressions,
                    1. / 60.,
                )
                .unwrap();
        }
        panel.undo_import(&mut avatar, &mut saved);
        assert_eq!(
            before,
            serde_json::to_vec(&saved).unwrap(),
            "Undo did not restore profile"
        );
    }
}
