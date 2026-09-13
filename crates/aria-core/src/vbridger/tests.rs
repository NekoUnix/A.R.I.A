use super::*;
use serde_json::json;

fn entry(name: &str, equation: &str) -> Value {
    json!({"output":name,"equation":equation,"min":"0","max":"1","defaultValue":"0","on":"true"})
}
fn profile(rows: Vec<Value>) -> Config {
    import(&serde_json::to_vec(&json!({"store":rows})).unwrap(), "test").unwrap()
}
fn face(jaw: f32) -> TrackingFrame {
    TrackingFrame {
        face_found: true,
        blend_shapes: BTreeMap::from([("jawopen".into(), jaw)]),
        ..Default::default()
    }
}
fn tick(c: &Config, r: &mut Runtime, f: Option<&TrackingFrame>, dt: f32) -> Inputs {
    let mut v = Inputs::new();
    c.apply(f, Vec3::default(), &mut v, r, dt);
    v
}

#[test]
fn plain_encoded_legacy_and_portable_roundtrip() {
    let raw =
        serde_json::to_vec(&json!({"store":[entry("MouthOpen","jawOpen - mouthClose")]})).unwrap();
    for prefix in [b"".as_slice(), b"vbridgerV2"] {
        let bytes = [prefix, &raw].concat();
        let encoded: Vec<_> = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ MASK[i % MASK.len()])
            .collect();
        for b in [&bytes, &encoded] {
            let c = import(b, "sample.vbridger").unwrap();
            assert_eq!(c.outputs.len(), 1);
            assert!(c.notes.is_empty());
            let loaded = import(&c.export().unwrap(), "roundtrip").unwrap();
            assert_eq!(
                tick(&loaded, &mut Runtime::default(), Some(&face(0.75)), 0.016)["MouthOpen"],
                0.75
            );
        }
    }
}
#[test]
fn formula_precedence_conditionals_and_math_are_bounded() {
    for (source, value) in [
        ("-2^2 + 3 * 4", 8.0),
        ("2^3^2", 512.0),
        (
            "if('(jawOpen>0.5)&&(jawOpen<1)', 'max(0.3, 0.8)', '1/0')",
            0.8,
        ),
        ("clamp(lerp(0,2,0.6),0,1)", 1.0),
        ("round(2.5)+sign(-2)+approx(1,1.02,0.1)", 2.0),
    ] {
        let e = Equation::parse(source).unwrap();
        assert!(
            (e.evaluate(|_| Some(0.7)).unwrap() - value).abs() < 1e-6,
            "{source}"
        );
    }
    for source in [
        "exec('whoami')",
        "include(file)",
        "1; 2",
        "rand(1)",
        "if(1,2)",
        "1e999",
    ] {
        assert!(Equation::parse(source).is_err(), "{source}");
    }
    assert!(Equation::parse(&format!("{}1{}", "(".repeat(1000), ")".repeat(1000))).is_err());
    assert!(Equation::parse(&"1+".repeat(1500)).is_err());
    for source in ["1/0", "sqrt(-1)", "clamp(1,2,0)"] {
        assert!(
            Equation::parse(source)
                .unwrap()
                .evaluate(|_| None)
                .is_none()
        );
    }
}
#[test]
fn time_and_random_are_reproducible_at_equal_times() {
    assert!(
        (Equation::parse("time(0.1,1)")
            .unwrap()
            .evaluate_at(|_| None, 0.1)
            .unwrap()
            - 0.6)
            .abs()
            < 1e-9
    );
    let e = Equation::parse("rand(-2,4)").unwrap();
    let a = e.evaluate_at(|_| None, 0.123).unwrap();
    assert!((-2.0..=4.0).contains(&a));
    assert_eq!(a, e.evaluate_at(|_| None, 0.123).unwrap());
    assert_ne!(a, e.evaluate_at(|_| None, 0.5).unwrap());
}
#[test]
fn dependencies_are_sorted_and_cycles_or_unknowns_are_reported() {
    let c = profile(vec![
        entry("Result", "Mid*0.5"),
        entry("Mid", "jawOpen"),
        entry("Bad", "Unknown+0.1"),
        entry("CycleA", "CycleB"),
        entry("CycleB", "CycleA"),
    ]);
    assert_eq!(
        c.outputs
            .iter()
            .map(|o| o.name.as_str())
            .collect::<Vec<_>>(),
        ["Mid", "Result"]
    );
    assert_eq!(c.notes.len(), 3);
    assert_eq!(
        tick(&c, &mut Runtime::default(), Some(&face(0.8)), 0.016)["Result"],
        0.4
    );
    let duplicate = json!({"store":[entry("A","1"),entry("A","0")]});
    assert!(import(&serde_json::to_vec(&duplicate).unwrap(), "bad").is_err());
}
#[test]
fn disabled_outputs_keep_their_state_and_defaults_without_overwriting_inputs() {
    let mut row = entry("First", "jawOpen");
    row["on"] = json!(false);
    row["defaultValue"] = json!(0.2);
    let c = profile(vec![row, entry("Result", "First+0.1")]);
    let v = tick(&c, &mut Runtime::default(), Some(&face(0.8)), 0.016);
    assert!(!v.contains_key("First"));
    assert!((v["Result"] - 0.3).abs() < 1e-6);
}
#[test]
fn vector_outputs_expand_into_independent_axes() {
    let mut v = entry("FaceAngle", "-headRotY");
    v["equationY"] = json!("-headRotX");
    v["equationZ"] = json!("headRotZ");
    v["vectorMode"] = json!(true);
    v["min"] = json!(-50);
    v["max"] = json!(50);
    let c = profile(vec![v]);
    assert_eq!(c.outputs.len(), 3);
    for (rotation, index, value) in [
        (
            Vec3 {
                x: 12.0,
                y: 0.0,
                z: 0.0,
            },
            "FaceAngleY",
            12.0,
        ),
        (
            Vec3 {
                x: 0.0,
                y: -17.0,
                z: 0.0,
            },
            "FaceAngleX",
            -17.0,
        ),
        (
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: 8.0,
            },
            "FaceAngleZ",
            8.0,
        ),
    ] {
        let mut f = face(0.0);
        f.rotation = rotation;
        let out = tick(&c, &mut Runtime::default(), Some(&f), 0.016);
        assert_eq!(out[index], value);
        for n in ["FaceAngleX", "FaceAngleY", "FaceAngleZ"]
            .into_iter()
            .filter(|n| *n != index)
        {
            assert_eq!(out[n], 0.0);
        }
    }
}
#[test]
fn delay_smoothing_steps_and_face_loss_reset() {
    let mut c = profile(vec![entry("MouthOpen", "jawOpen")]);
    let o = &mut c.outputs[0];
    o.delay_on = true;
    o.delay_ms = 100.0;
    let mut r = Runtime::default();
    let f = face(1.0);
    assert_eq!(tick(&c, &mut r, Some(&f), 0.02)["MouthOpen"], 0.0);
    for _ in 0..6 {
        tick(&c, &mut r, Some(&f), 0.02);
    }
    assert_eq!(tick(&c, &mut r, Some(&f), 0.02)["MouthOpen"], 1.0);
    c.outputs[0].delay_on = false;
    c.outputs[0].smooth_on = true;
    c.outputs[0].smoothing = 0.5;
    let a = tick(&c, &mut Runtime::default(), Some(&f), 1.0 / 60.0)["MouthOpen"];
    let mut r = Runtime::default();
    tick(&c, &mut r, Some(&f), 1.0 / 120.0);
    let b = tick(&c, &mut r, Some(&f), 1.0 / 120.0)["MouthOpen"];
    assert!((a - b).abs() < 1e-6);
    c.outputs[0].smooth_on = false;
    c.face_loss_ms = 100.0;
    tick(&c, &mut r, Some(&f), 0.016);
    assert_eq!(tick(&c, &mut r, None, 0.05)["MouthOpen"], 1.0);
    assert_eq!(tick(&c, &mut r, None, 0.06)["MouthOpen"], 0.0);
    assert_eq!(tick(&c, &mut r, Some(&face(0.2)), 0.016)["MouthOpen"], 0.2);
}
#[test]
fn steps_use_absolute_falling_thresholds_and_minimum_hold_times() {
    let mut row = entry("MouthOpen", "jawOpen");
    row["stepOn"] = json!(true);
    row["stepDetails2"] = json!([
        [0.0, 0.0, 0.0, 0.0],
        [0.3, 0.4, 0.2, 100.0],
        [0.6, 1.0, 0.5, 0.0]
    ]);
    let c = profile(vec![row]);
    let mut r = Runtime::default();
    assert_eq!(tick(&c, &mut r, Some(&face(0.4)), 0.016)["MouthOpen"], 0.4);
    assert_eq!(tick(&c, &mut r, Some(&face(0.1)), 0.05)["MouthOpen"], 0.4);
    assert_eq!(tick(&c, &mut r, Some(&face(0.25)), 0.06)["MouthOpen"], 0.4);
    assert_eq!(tick(&c, &mut r, Some(&face(0.19)), 0.016)["MouthOpen"], 0.0);
    assert_eq!(tick(&c, &mut r, Some(&face(0.8)), 0.016)["MouthOpen"], 1.0);
}
#[test]
fn curves_support_legacy_weights_wraps_and_input_calibration() {
    let legacy = json!({"m_Curve":[{"time":0,"value":0,"inSlope":0,"outSlope":0},{"time":1,"value":1,"inSlope":0,"outSlope":0}],"m_PreInfinity":2,"m_PostInfinity":2});
    let mut row = entry("MouthOpen", "jawOpen");
    row["curve"] = legacy;
    let mut c = profile(vec![row]);
    assert_eq!(c.outputs.len(), 1);
    let curve = &mut c.outputs[0].curve;
    assert!((curve.evaluate(0.25) - 0.15625).abs() < 1e-6);
    curve.keys[0].weighted = 2;
    curve.keys[0].out_weight = 0.1;
    assert!((curve.evaluate(0.25) - 0.15625).abs() > 0.01);
    curve.post_wrap = 2;
    assert!((curve.evaluate(1.25) - curve.evaluate(0.25)).abs() < 1e-6);
    curve.post_wrap = 4;
    assert!((curve.evaluate(1.25) - curve.evaluate(0.75)).abs() < 1e-6);
    c.outputs[0].curve = Curve::default();
    c.calibrate_blends(&face(0.2));
    assert!(
        (tick(&c, &mut Runtime::default(), Some(&face(0.7)), 0.016)["MouthOpen"] - 0.5).abs()
            < 1e-6
    );
    let restored = import(&c.export().unwrap(), "saved").unwrap();
    assert_eq!(restored.offsets, c.offsets);
}
#[test]
fn no_face_procedural_and_volume_inputs_work_without_faking_visemes() {
    let mut row = entry("Loudness", "volume");
    row["faceSend"] = json!(false);
    let c = profile(vec![row, entry("Silence", "viseme_SIL_abs")]);
    let mut inputs = Inputs::from([("MicLevel".into(), 0.8)]);
    c.apply(
        None,
        Vec3::default(),
        &mut inputs,
        &mut Runtime::default(),
        0.016,
    );
    assert_eq!(inputs["Loudness"], 0.8);
    let f = face(0.3);
    assert!(c.missing_inputs(&f).contains("viseme_SIL_abs"));
    assert_eq!(
        tick(&c, &mut Runtime::default(), Some(&f), 0.016)["Silence"],
        1.0
    );
}
#[test]
fn malformed_and_excessive_imports_cannot_change_a_valid_profile() {
    for bytes in [
        vec![],
        vec![0; MAX_FILE + 1],
        b"vbridgerV99{}".to_vec(),
        b"{\"store\":[]}".to_vec(),
    ] {
        assert!(import(&bytes, "bad").is_err());
    }
    let mut row = entry("Bad", "jawOpen");
    row["unexpectedModifier"] = json!(true);
    let c = profile(vec![row]);
    assert!(c.outputs.is_empty());
    assert!(c.notes[0].contains("unexpectedModifier"));
    let mut c = profile(vec![entry("A", "jawOpen")]);
    c.outputs[0].min = 1.0;
    assert!(c.validate().is_err());
    assert!(Output::new("Bad", "1", 5.0, 0.0).is_err());
}
#[test]
fn model_profiles_and_presets_retain_imports_without_sharing_runtime() {
    let params = vec![crate::rig::RigParameter {
        id: "ParamMouthOpenY".into(),
        min: 0.0,
        max: 1.0,
        default: 0.0,
        value: 0.0,
    }];
    let mut rig = crate::movement::RigConfig::from_parameters(&params);
    rig.vbridger = profile(vec![entry("MouthOpen", "jawOpen*0.5")]);
    let bytes = serde_json::to_vec(&rig).unwrap();
    let restored: crate::movement::RigConfig = serde_json::from_slice(&bytes).unwrap();
    restored.validate(&params).unwrap();
    let inputs = tick(
        &restored.vbridger,
        &mut Runtime::default(),
        Some(&face(0.8)),
        0.016,
    );
    assert_eq!(inputs["ParamMouthOpenY"], 0.4);
    let mut values = params.clone();
    rig.evaluate(&inputs, &mut values, 0.016, None);
    assert!((values[0].value - 0.4).abs() < 1e-6);
    assert!(
        !crate::movement::RigConfig::from_parameters(&params)
            .vbridger
            .enabled
    );
}
#[test]
fn local_user_and_installed_configs_import_and_drive_finite_values() {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("ARIA_TEST_VBRIDGER") {
        paths.push(std::path::PathBuf::from(path));
    }
    if let Some(dir) = std::env::var_os("ARIA_TEST_VBRIDGER_DIR") {
        paths.extend(
            std::fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "vbridger")),
        );
    }
    if paths.is_empty() {
        return;
    }
    for path in paths {
        let c = import(
            &std::fs::read(&path).unwrap(),
            &path.file_name().unwrap().to_string_lossy(),
        )
        .unwrap();
        assert!(!c.outputs.is_empty(), "{}: {:?}", path.display(), c.notes);
        assert!(
            c.notes
                .iter()
                .all(|n| n.contains("merged an identical duplicate")),
            "{}: {:?}",
            path.display(),
            c.notes
        );
        let mut r = Runtime::default();
        for i in 0..120 {
            let mut f = face(i as f32 / 120.0);
            f.rotation = Vec3 {
                x: 10.0,
                y: 20.0,
                z: 5.0,
            };
            let v = tick(&c, &mut r, Some(&f), 1.0 / 60.0);
            assert!(v.values().all(|v| v.is_finite()));
        }
        eprintln!(
            "{}: {} outputs, no skipped settings",
            path.file_name().unwrap().to_string_lossy(),
            c.outputs.len()
        );
    }
}

#[test]
fn native_export_roundtrip_and_external_channels_without_face() {
    let mut c = profile(vec![entry("Result", "volume")]);
    c.outputs[0].requires_face = false;
    let bytes = c.export_vbridger().unwrap();
    let decoded = import(&bytes, "export").unwrap();
    assert_eq!(decoded.outputs[0].equation.source(), "volume");
    c.external_inputs.insert("MyInput".into(), 0.2);
    c.outputs[0].equation = Equation::parse("MyInput + volume").unwrap();
    c.prepare().unwrap();
    let frame = TrackingFrame {
        parameters: BTreeMap::from([("MyInput".into(), 0.4), ("volume".into(), 0.3)]),
        ..Default::default()
    };
    assert!((tick(&c, &mut Runtime::default(), Some(&frame), 0.016)["Result"] - 0.7).abs() < 1e-6);
}
