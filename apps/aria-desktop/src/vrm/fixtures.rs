//! Original generated geometry, never a copy of an artist's avatar.
use super::*;
use serde_json::{Value, json};
use std::path::Path;

fn fixture(old: bool) -> (Value, Vec<u8>) {
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut data =
        |bytes: &[u8], component: u32, count: usize, kind: &str, min: Value, max: Value| {
            while bin.len() % 4 != 0 {
                bin.push(0);
            }
            let view = views.len();
            views.push(json!({"buffer":0,"byteOffset":bin.len(),"byteLength":bytes.len()}));
            bin.extend(bytes);
            let id = accessors.len();
            let mut accessor =
                json!({"bufferView":view,"componentType":component,"count":count,"type":kind});
            if !min.is_null() {
                accessor["min"] = min;
                accessor["max"] = max;
            }
            accessors.push(accessor);
            id
        };
    let positions = data(
        bytemuck::cast_slice(&[[-0.3f32, 0., 0.], [0.3, 0., 0.], [0., 1., 0.]]),
        5126,
        3,
        "VEC3",
        json!([-0.3, 0., 0.]),
        json!([0.3, 1., 0.]),
    );
    let normals = data(
        bytemuck::cast_slice(&[[0f32, 0., 1.]; 3]),
        5126,
        3,
        "VEC3",
        Value::Null,
        Value::Null,
    );
    let joints = data(
        bytemuck::cast_slice(&[[0u16; 4]; 3]),
        5123,
        3,
        "VEC4",
        Value::Null,
        Value::Null,
    );
    let weights = data(
        bytemuck::cast_slice(&[[1f32, 0., 0., 0.]; 3]),
        5126,
        3,
        "VEC4",
        Value::Null,
        Value::Null,
    );
    let morph = data(
        bytemuck::cast_slice(&[[0f32, 0., 0.], [0., 0., 0.], [0.2, 0., 0.]]),
        5126,
        3,
        "VEC3",
        Value::Null,
        Value::Null,
    );
    let indices = data(
        bytemuck::cast_slice(&[0u16, 1, 2]),
        5123,
        3,
        "SCALAR",
        Value::Null,
        Value::Null,
    );
    let inverse = data(
        bytemuck::cast_slice(&[Mat4::IDENTITY.to_cols_array()]),
        5126,
        1,
        "MAT4",
        Value::Null,
        Value::Null,
    );
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let primitive = json!({"attributes":{"POSITION":positions,"NORMAL":normals,"JOINTS_0":joints,"WEIGHTS_0":weights},"indices":indices,"material":0,"targets":[{"POSITION":morph}]});
    let mut j = json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":bin.len()}],"bufferViews":views,"accessors":accessors,
        "scenes":[{"nodes":[0]}],"scene":0,
        "nodes":[{"children":[1,4]},{"name":"hips","children":[2]},{"name":"head","translation":[0.,0.6,0.],"children":[3]},{"name":"tip","translation":[0.,0.2,0.]},{"mesh":0,"skin":0}],
        "skins":[{"joints":[1],"inverseBindMatrices":inverse}],"meshes":[{"primitives":[primitive.clone(),primitive]}],
        "materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.8,0.3,0.1,1.]},"doubleSided":true}],"extensions":{},"extensionsUsed":[]});
    if old {
        j["extensionsUsed"] = json!(["VRM"]);
        j["extensions"]["VRM"] = json!({"specVersion":"0.0","meta":{"title":"ARIA original test rig","author":"ARIA","licenseName":"CC0"},"humanoid":{"humanBones":[{"bone":"hips","node":1},{"bone":"head","node":2}]},"blendShapeMaster":{"blendShapeGroups":[{"name":"A","presetName":"a","binds":[{"mesh":0,"index":0,"weight":100.}]}]},"secondaryAnimation":{"colliderGroups":[{"node":1,"colliders":[{"offset":{"x":0.1,"y":0.2,"z":0.3},"radius":0.01}]}],"boneGroups":[{"comment":"Test hair","bones":[2],"stiffiness":1.,"dragForce":0.4,"gravityDir":{"x":0.,"y":-1.,"z":0.},"gravityPower":0.1,"colliderGroups":[0]}]}});
    } else {
        j["extensionsUsed"] = json!(["VRMC_vrm", "VRMC_springBone", "VRMC_materials_mtoon"]);
        j["extensionsRequired"] = json!(["VRMC_vrm"]);
        j["extensions"]["VRMC_vrm"] = json!({"specVersion":"1.0","meta":{"name":"ARIA original test rig","authors":["ARIA"],"licenseUrl":"https://creativecommons.org/publicdomain/zero/1.0/"},"humanoid":{"humanBones":{"hips":{"node":1},"head":{"node":2}}},"expressions":{"preset":{"aa":{"morphTargetBinds":[{"node":4,"index":0,"weight":1.}]}}}});
        j["extensions"]["VRMC_springBone"] = json!({"specVersion":"1.0","colliders":[{"node":1,"shape":{"capsule":{"offset":[0.1,0.2,0.3],"tail":[0.1,0.4,0.3],"radius":0.01}}}],"colliderGroups":[{"colliders":[0]}],"springs":[{"name":"Test hair","joints":[{"node":2,"stiffness":1.,"dragForce":0.4,"gravityPower":0.1},{"node":3}],"colliderGroups":[0]}]});
        j["materials"][0]["extensions"] =
            json!({"VRMC_materials_mtoon":{"specVersion":"1.0","shadeColorFactor":[0.3,0.1,0.1]}});
    }
    (j, bin)
}
fn write(path: &Path, json: &Value, bin: &[u8]) {
    let mut metadata = serde_json::to_vec(json).unwrap();
    while !metadata.len().is_multiple_of(4) {
        metadata.push(b' ');
    }
    let mut out = Vec::new();
    for word in [
        0x46546c67u32,
        2,
        (12 + 8 + metadata.len() + 8 + bin.len()) as u32,
        metadata.len() as u32,
        0x4e4f534a,
    ] {
        out.extend(word.to_le_bytes());
    }
    out.extend(metadata);
    out.extend((bin.len() as u32).to_le_bytes());
    out.extend(0x004e4942u32.to_le_bytes());
    out.extend(bin);
    std::fs::write(path, out).unwrap();
}
#[test]
fn both_versions_share_geometry_and_import_expression_units() {
    let dir = tempfile::tempdir().unwrap();
    for old in [true, false] {
        let (json, bin) = fixture(old);
        let path = dir.path().join(if old { "old.vrm" } else { "new.vrm" });
        write(&path, &json, &bin);
        let asset = asset::load(&path, &|_| Ok(())).unwrap();
        assert_eq!(asset.geometry.len(), 1);
        assert_eq!(asset.parts.len(), 2);
        assert_eq!(asset.expressions[0].binds[0].weight, 1.);
        assert_eq!(asset.springs.len(), 1);
        assert_eq!(
            asset.springs[0].colliders[0].offset.z,
            if old { -0.3 } else { 0.3 }
        );
        let forward = asset.front.transform_vector3(glam::Vec3::Z);
        assert!((forward.z - if old { -1. } else { 1. }).abs() < 1e-5);
        assert!(asset.key.starts_with("vrm:"));
    }
}

#[test]
fn glb_import_keeps_large_morph_sets_defaults_and_rejects_static_props() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("avatar.GLB");
    let (mut j, bin) = fixture(false);
    j["extensions"] = json!({});
    j["extensionsUsed"] = json!([]);
    j["extensionsRequired"] = json!([]);
    j["nodes"][1]["name"] = json!("mixamorig:Hips");
    j["nodes"][2]["name"] = json!("Head");
    j["skins"][0]["joints"][0] = json!(2);
    j["meshes"][0]["name"] = json!("Face");
    let target = j["meshes"][0]["primitives"][0]["targets"][0].clone();
    for p in j["meshes"][0]["primitives"].as_array_mut().unwrap() {
        p["targets"] = json!(vec![target.clone(); 300]);
    }
    let mut names: Vec<_> = (0..300).map(|n| format!("Shape {n}")).collect();
    names[0] = "vrc.v_aa".into();
    names[1] = "eye_close_1_L".into();
    names[2] = "eye_close_1_R".into();
    j["meshes"][0]["extras"]["targetNames"] = json!(names);
    let mut weights = vec![0f32; 300];
    weights[299] = 0.7;
    weights[297] = -0.2;
    weights[298] = 1.3;
    j["meshes"][0]["weights"] = json!(weights);
    write(&path, &j, &bin);
    let a = asset::load(&path, &|_| Ok(())).unwrap();
    assert!(a.summary.is_glb() && a.key.starts_with("glb:"));
    assert_eq!(a.summary.name, "avatar");
    assert_eq!(a.bones["hips"], 1);
    assert_eq!(a.bones["head"], 2);
    assert_eq!(a.geometry.len(), 1);
    assert_eq!(a.expressions.len(), 300);
    assert_eq!(glb_default(&a, &a.expressions[299]), 0.7);
    assert_eq!(glb_default(&a, &a.expressions[297]), -0.2);
    assert_eq!(glb_default(&a, &a.expressions[298]), 1.3);
    let mut bindings = glb::bindings(&a.expressions);
    assert_eq!(
        bindings
            .get_mut(&expression_id(0))
            .unwrap()
            .evaluate(1., 0.016),
        1.
    );
    assert_eq!(
        bindings
            .get_mut(&expression_id(1))
            .unwrap()
            .evaluate(0., 0.016),
        1.
    );
    j["nodes"][4].as_object_mut().unwrap().remove("skin");
    write(&path, &j, &bin);
    assert!(
        asset::load(&path, &|_| Ok(()))
            .err()
            .unwrap()
            .to_string()
            .contains("skinned humanoid")
    );
}
#[test]
fn broken_exports_fail_before_gpu_or_large_allocation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.vrm");
    let (source, bin) = fixture(false);
    for variant in 0..5 {
        let mut j = source.clone();
        match variant {
            0 => j["bufferViews"][0]["byteLength"] = json!(1_000_000),
            1 => j["accessors"][0]["count"] = json!(u32::MAX),
            2 => j["buffers"][0]["uri"] = json!("private.bin"),
            3 => j["nodes"][3]["children"] = json!([0]),
            _ => j["extensionsRequired"] = json!(["KHR_draco_mesh_compression"]),
        };
        write(&path, &j, &bin);
        assert!(
            asset::load(&path, &|_| Ok(())).is_err(),
            "variant {variant}"
        );
    }
}
#[test]
fn spring_step_is_finite_frame_rate_independent_and_freezes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spring.vrm");
    let (j, bin) = fixture(false);
    write(&path, &j, &bin);
    let a = asset::load(&path, &|_| Ok(())).unwrap();
    let settings = aria_core::physics::PhysicsSettings {
        wind: 0.4,
        ..Default::default()
    };
    let run = |dt, n| {
        let mut simulation = spring::Simulation::default();
        let mut rotations: Vec<_> = a.nodes.iter().map(|n| n.rotation).collect();
        let mut world: Vec<_> = a.nodes.iter().map(|n| n.world).collect();
        for _ in 0..n {
            for (r, node) in rotations.iter_mut().zip(&a.nodes) {
                *r = node.rotation;
            }
            spring::world_matrices(&a.nodes, &a.order, &rotations, &mut world);
            simulation.update(
                &a.nodes,
                &a.order,
                &a.springs,
                &mut rotations,
                &mut world,
                &settings,
                &Default::default(),
                dt,
                false,
            );
        }
        let before = rotations.clone();
        simulation.update(
            &a.nodes,
            &a.order,
            &a.springs,
            &mut rotations,
            &mut world,
            &settings,
            &Default::default(),
            dt,
            true,
        );
        assert_eq!(rotations, before);
        assert!(world.iter().all(|m| m.is_finite()));
        rotations
    };
    let at60 = run(1. / 60., 60);
    let at120 = run(1. / 120., 120);
    assert!(at60.iter().zip(at120).all(|(a, b)| a.abs_diff_eq(b, 1e-5)));
    assert!(!at60[2].abs_diff_eq(Quat::IDENTITY, 1e-4));
}
#[test]
#[cfg(windows)]
#[ignore = "requires a DX12 GPU; uses only generated fixture geometry"]
fn both_versions_render_and_morph_on_gpu() {
    let state = crate::spout::tests::gpu_state();
    let dir = tempfile::tempdir().unwrap();
    for old in [true, false] {
        let path = dir.path().join("gpu.vrm");
        let (j, bin) = fixture(old);
        write(&path, &j, &bin);
        let mut a = Avatar::from_asset(&state, asset::load(&path, &|_| Ok(())).unwrap()).unwrap();
        let mut config = a.initial_config.clone();
        config.physics.enabled = false;
        let mut expressions = Default::default();
        a.update(&Inputs::new(), &mut config, &mut expressions, 0.)
            .unwrap();
        let before = a.renderer.read_rgba().unwrap();
        assert!(before.as_chunks::<4>().0.iter().any(|p| p[3] > 0));
        let pin = a
            .pick_pin(eframe::egui::Vec2::ZERO)
            .expect("Pick rendered triangle");
        let pin: aria_core::items::Pin =
            serde_json::from_str(&serde_json::to_string(&pin).unwrap()).unwrap();
        let before_pin = a.resolve_pin(&pin);
        a.update(
            &Inputs::from([("ParamMouthOpenY".into(), 1.)]),
            &mut config,
            &mut expressions,
            1. / 60.,
        )
        .unwrap();
        assert!(
            before != a.renderer.read_rgba().unwrap(),
            "Morph must change rendered fixture"
        );
        let morphed_pin = a.resolve_pin(&pin);
        assert_ne!(before_pin, morphed_pin, "Pin must follow facial morphs");
        config.vrm.yaw = 25.;
        a.update(
            &Inputs::from([("ParamMouthOpenY".into(), 1.)]),
            &mut config,
            &mut expressions,
            1. / 60.,
        )
        .unwrap();
        assert_ne!(
            morphed_pin,
            a.resolve_pin(&pin),
            "Pin follows camera projection"
        );
        let mut broken = pin.clone();
        if let aria_core::items::Pin::VrmSurface { vertices, .. } = &mut broken {
            vertices[0] = u32::MAX;
        }
        assert_eq!(a.resolve_pin(&broken), crate::items::Anchor::Missing);
    }
}

#[test]
fn secondary_detection_protects_humanoid_and_respects_manual_and_exported_chains() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secondary.vrm");
    let (j, bin) = fixture(false);
    write(&path, &j, &bin);
    let mut a = asset::load(&path, &|_| Ok(())).unwrap();
    a.springs.clear();
    let node = a.nodes[3].clone();
    a.nodes[2].children = vec![5];
    a.nodes[3].parent = Some(5);
    a.nodes.push(asset::Node {
        name: "Hair_Root".into(),
        parent: Some(2),
        children: vec![3],
        translation: glam::Vec3::ZERO,
        ..node
    });
    a.order = vec![0, 1, 2, 5, 3, 4];
    a.skins[0].joints.push(3);
    let mut settings = aria_core::vrm::Secondary::default();
    let groups = secondary::groups(&a, &settings);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].joints[0].node, 5);
    assert!(
        !groups
            .iter()
            .flat_map(|g| &g.joints)
            .any(|j| a.bones.values().any(|&n| n == j.node))
    );
    a.springs = groups.clone();
    assert_eq!(secondary::groups(&a, &settings).len(), 1);
    a.springs.clear();
    settings.auto_detect = false;
    assert!(secondary::groups(&a, &settings).is_empty());
    a.nodes[5].name = "unnamed".into();
    settings.manual_roots.insert(5);
    assert_eq!(secondary::groups(&a, &settings).len(), 1);
    settings.manual_roots.clear();
    settings.manual_roots.insert(2); // head is protected
    assert!(secondary::groups(&a, &settings).is_empty());
}

#[test]
fn secondary_world_inertia_lags_centered_head_and_settles_with_bounded_bend() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("inertia.vrm");
    let (j, bin) = fixture(false);
    write(&path, &j, &bin);
    let a = asset::load(&path, &|_| Ok(())).unwrap();
    let mut groups = a.springs.clone();
    groups[0].center = Some(1);
    groups[0].colliders.clear();
    groups[0].joints[0].power = 0.;
    let mut sim = spring::Simulation::default();
    let mut rotations: Vec<_> = a.nodes.iter().map(|n| n.rotation).collect();
    let mut world: Vec<_> = a.nodes.iter().map(|n| n.world).collect();
    let mut settings = aria_core::physics::PhysicsSettings::default();
    let secondary = aria_core::vrm::Secondary::default();
    sim.update(
        &a.nodes,
        &a.order,
        &groups,
        &mut rotations,
        &mut world,
        &settings,
        &secondary,
        1. / 60.,
        false,
    );
    let mut first = 0.;
    for frame in 0..300 {
        for (r, n) in rotations.iter_mut().zip(&a.nodes) {
            *r = n.rotation;
        }
        rotations[1] = Quat::from_rotation_z(25f32.to_radians());
        spring::world_matrices(&a.nodes, &a.order, &rotations, &mut world);
        sim.update(
            &a.nodes,
            &a.order,
            &groups,
            &mut rotations,
            &mut world,
            &settings,
            &secondary,
            1. / 60.,
            false,
        );
        let angle = rotations[2].angle_between(a.nodes[2].rotation).abs();
        assert!(angle.is_finite() && angle <= 45.01f32.to_radians());
        if frame == 0 {
            first = angle;
        }
    }
    assert!(
        first > 5f32.to_radians(),
        "spring should lag a head-centered turn"
    );
    assert!(
        rotations[2].angle_between(a.nodes[2].rotation) < 0.01,
        "spring should settle"
    );
    settings.groups.insert(
        groups[0].id.clone(),
        aria_core::physics::GroupSettings {
            enabled: false,
            ..Default::default()
        },
    );
    for (r, n) in rotations.iter_mut().zip(&a.nodes) {
        *r = n.rotation;
    }
    spring::world_matrices(&a.nodes, &a.order, &rotations, &mut world);
    sim.update(
        &a.nodes,
        &a.order,
        &groups,
        &mut rotations,
        &mut world,
        &settings,
        &secondary,
        0.,
        false,
    );
    assert_eq!(rotations[2], a.nodes[2].rotation);
}
