use super::*;
use serde_json::json;
fn imported(h: serde_json::Value) -> Imported {
    parse(&serde_json::to_vec(&json!({"Name":"Synthetic avatar","FileReferences":{"Model":"avatar.model3.json"},"ParameterSettings":[],"Hotkeys":h})).unwrap(),&[],&[]).unwrap()
}
#[test]
fn actions_keys_timers_and_round_trip() {
    let i = imported(json!([
        {"HotkeyID":"e","Name":"Love","Action":"ToggleExpression","File":"expressions/love.exp3.json","Triggers":{"Trigger1":"N1","ScreenButton":1},"DeactivateAfterSeconds":true,"DeactivateAfterSecondsAmount":0.5},
        {"HotkeyID":"s","Name":"Scene","Action":"ToggleItemScene","File":"Pat","Triggers":{"Trigger1":"W","Trigger2":"CapsLock"}},
        {"HotkeyID":"m","Action":"TriggerAnimation","File":"wave.motion3.json","StopsOnLastFrame":true,"Triggers":{"Trigger1":"RightMouseButton"}},
        {"HotkeyID":"c","Action":"RemoveAllExpressions"},{"HotkeyID":"r","Action":"RemoveAllItems"},
        {"HotkeyID":"p","Action":"MoveModel","Position":{"X":200,"Y":-300,"Z":1.5,"Rotation":30}},
        {"HotkeyID":"external","Action":"ChangeBackground"}
    ]));
    assert_eq!(i.config.actions.len(), 7);
    assert_eq!(i.config.actions[0].shortcut.unwrap().key, 0x31);
    assert_eq!(i.config.actions[1].chord, vec![0x57, 0x14]);
    assert_eq!(i.config.actions[2].chord, vec![2]);
    let restored: Config = serde_json::from_slice(&serde_json::to_vec(&i.config).unwrap()).unwrap();
    restored.validate().unwrap();
    let mut r = Runtime::default();
    assert_eq!(r.screen_actions(&restored, 1), vec!["e"]);
    assert!(r.screen_actions(&restored, 1).is_empty());
    r.screen_actions(&restored, -1);
    assert_eq!(r.screen_actions(&restored, 1).len(), 1);
    let mut active = BTreeSet::new();
    assert!(r.expression(&restored.actions[0], &mut active));
    assert!(!r.tick(10., true, &mut active));
    assert_eq!(active.len(), 1);
    r.tick(0.25, false, &mut active);
    assert!(r.tick(0.25, false, &mut active));
    assert!(active.is_empty());
    let mut held = restored.actions[0].clone();
    held.release = true;
    held.seconds = None;
    r.expression(&held, &mut active);
    assert!(r.release("e", &mut active));
    assert!(active.is_empty());
}
#[test]
fn duplicates_reserved_paths_and_unknowns_are_reviewable() {
    let i = imported(
        json!([{ "Action":"RemoveAllExpressions","Triggers":{"Trigger1":"N1"}},{"Action":"RemoveAllItems","Triggers":{"Trigger1":"N1"}},{"Action":"TogglePhysics","Triggers":{"Trigger1":"LeftControl","Trigger2":"LeftAlt","Trigger3":"P"}}]),
    );
    assert!(i.config.actions[1].shortcut.is_none() && i.config.actions[1].chord.is_empty());
    assert!(i.config.actions[2].shortcut.is_none() && i.config.actions[2].chord.is_empty());
    for path in [
        "../private.exp3.json",
        "C:\\secret",
        "/etc/passwd",
        "a/../../b",
        "a//b",
    ] {
        assert!(valid_reference(path).is_err());
    }
    assert!(
        parse(
            br#"{"FileReferences":{"Model":"../other.model3.json"},"ParameterSettings":[]}"#,
            &[],
            &[]
        )
        .is_err()
    );
    assert!(
        shortcut(&["Numpad1"]).unwrap().unwrap().key != shortcut(&["N1"]).unwrap().unwrap().key
    );
}
#[test]
fn physics_and_colors_are_model_specific() {
    let bytes=serde_json::to_vec(&json!({"FileReferences":{"Model":"a.model3.json"},"ParameterSettings":[],"PhysicsSettings":{"Use":true,"PhysicsStrength":60,"WindStrength":40},"PhysicsCustomizationSettings":{"PhysicsMultipliersPerPhysicsGroup":[{"ID":"Hair","Value":0.35}],"WindMultipliersPerPhysicsGroup":[{"ID":"Hair","Value":0.5}]},"ArtMeshDetails":{"ArtMeshMultiplyAndScreenColors":[{"ID":"Face","Value":"FF0000FF|000000FF"}]}})).unwrap();
    let i = parse(&bytes, &[], &[]).unwrap();
    assert_eq!(i.physics.strength, 1.2);
    assert_eq!(i.physics.groups["Hair"].strength, 0.35);
    assert!((i.physics.wind + i.physics.groups["Hair"].wind - 0.2).abs() < 1e-6);
    assert_eq!(i.layers.colors["Face"].multiply, [1., 0., 0., 1.]);
}
