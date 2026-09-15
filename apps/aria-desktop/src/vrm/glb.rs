//! GLB humanoids use the same render, tracking and profile pipeline as VRM.
//! Bone names are hints, never an assertion that a Unity avatar descriptor exists.
use super::asset::{Expression, MorphBind, Node, Summary, array, index};
use anyhow::{Result, ensure};
use aria_core::rig::Binding;
use glam::{Mat4, Vec3};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const BONES: &[(&str, &[&str])] = &[
    ("hips", &["hips", "hip", "pelvis"]),
    ("spine", &["spine", "spine1"]),
    ("chest", &["chest", "spine2"]),
    ("upperChest", &["upperchest", "spine3"]),
    ("neck", &["neck"]),
    ("head", &["head"]),
    ("jaw", &["jaw"]),
    ("leftEye", &["lefteye", "eyel"]),
    ("rightEye", &["righteye", "eyer"]),
    (
        "leftShoulder",
        &["leftshoulder", "shoulderl", "lshoulder", "claviclel"],
    ),
    (
        "rightShoulder",
        &["rightshoulder", "shoulderr", "rshoulder", "clavicler"],
    ),
    (
        "leftUpperArm",
        &["leftupperarm", "upperarml", "leftarm", "arml"],
    ),
    (
        "rightUpperArm",
        &["rightupperarm", "upperarmr", "rightarm", "armr"],
    ),
    (
        "leftLowerArm",
        &["leftlowerarm", "lowerarml", "leftforearm", "forearml"],
    ),
    (
        "rightLowerArm",
        &["rightlowerarm", "lowerarmr", "rightforearm", "forearmr"],
    ),
    ("leftHand", &["lefthand", "handl"]),
    ("rightHand", &["righthand", "handr"]),
    (
        "leftUpperLeg",
        &["leftupperleg", "upperlegl", "leftupleg", "thighl"],
    ),
    (
        "rightUpperLeg",
        &["rightupperleg", "upperlegr", "rightupleg", "thighr"],
    ),
    (
        "leftLowerLeg",
        &["leftlowerleg", "lowerlegl", "leftleg", "calfl", "shinl"],
    ),
    (
        "rightLowerLeg",
        &["rightlowerleg", "lowerlegr", "rightleg", "calfr", "shinr"],
    ),
    ("leftFoot", &["leftfoot", "footl"]),
    ("rightFoot", &["rightfoot", "footr"]),
    (
        "leftToes",
        &["lefttoes", "lefttoebase", "toebasel", "toesl"],
    ),
    (
        "rightToes",
        &["righttoes", "righttoebase", "toebaser", "toesr"],
    ),
];
fn normalized(name: &str) -> String {
    name.rsplit([':', '|'])
        .next()
        .unwrap_or(name)
        .to_lowercase()
        .trim_start_matches("mixamorig")
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}
fn active_nodes(j: &Value) -> BTreeSet<usize> {
    let scene = index(&j["scene"]).unwrap_or(0);
    let mut stack: Vec<_> = array(&j["scenes"][scene]["nodes"])
        .iter()
        .filter_map(index)
        .collect();
    let mut active = BTreeSet::new();
    while let Some(n) = stack.pop() {
        if n < array(&j["nodes"]).len() && active.len() < 4096 && active.insert(n) {
            stack.extend(array(&j["nodes"][n]["children"]).iter().filter_map(index));
        }
    }
    active
}
pub fn detect_bones(j: &Value) -> BTreeMap<String, usize> {
    let active = active_nodes(j);
    let nodes = array(&j["nodes"]);
    // Include unweighted parents of skin joints (often hips/spine/neck).
    let mut eligible: BTreeSet<_> = array(&j["skins"])
        .iter()
        .flat_map(|s| array(&s["joints"]).iter().filter_map(index))
        .filter(|n| active.contains(n))
        .collect();
    let mut parents = BTreeMap::new();
    for (n, node) in nodes.iter().enumerate() {
        for child in array(&node["children"]).iter().filter_map(index) {
            parents.insert(child, n);
        }
    }
    for n in eligible.clone() {
        let mut next = n;
        for _ in 0..128 {
            let Some(&p) = parents.get(&next) else {
                break;
            };
            if !eligible.insert(p) {
                break;
            }
            next = p;
        }
    }
    let names: Vec<_> = nodes
        .iter()
        .map(|n| normalized(n["name"].as_str().unwrap_or("")))
        .collect();
    let mut bones = BTreeMap::new();
    for &(bone, aliases) in BONES {
        for alias in aliases {
            let matches: Vec<_> = eligible
                .iter()
                .copied()
                .filter(|&n| names.get(n).is_some_and(|name| name == alias))
                .collect();
            if matches.len() == 1 {
                bones.insert(bone.into(), matches[0]);
                break;
            }
            // Ambiguous exact names must be assigned by the user, not guessed.
            if matches.len() > 1 {
                break;
            }
        }
    }
    bones
}
pub fn summary(j: &Value) -> Result<Summary> {
    ensure!(
        array(&j["nodes"]).len() <= 4096 && array(&j["meshes"]).len() <= 1024,
        "GLB exceeds avatar node or mesh limits"
    );
    ensure!(
        active_nodes(j)
            .iter()
            .any(|&n| index(&j["nodes"][n]["skin"]).is_some()
                && index(&j["nodes"][n]["mesh"]).is_some()),
        "Choose a skinned humanoid GLB with its armature. Static GLB props can be used in Throws & sprays."
    );
    Ok(Summary {
        version: "glTF 2.0".into(),
        name: String::new(),
        author: "Not declared in GLB".into(),
        license: "Consult the avatar author's license".into(),
        bones: detect_bones(j).len(),
        expressions: expressions(j)?.len(),
        springs: 0,
        materials: array(&j["materials"]).len(),
    })
}
pub fn front(nodes: &[Node], bones: &BTreeMap<String, usize>) -> Mat4 {
    let p = |name| {
        bones
            .get(name)
            .and_then(|&n| nodes.get(n))
            .map(|n| n.world.transform_point3(Vec3::ZERO))
    };
    if let (Some(left), Some(right), Some(head), Some(hips)) = (
        p("leftUpperArm").or_else(|| p("leftUpperLeg")),
        p("rightUpperArm").or_else(|| p("rightUpperLeg")),
        p("head"),
        p("hips"),
    ) && (left - right).cross(head - hips).z < 0.
    {
        return Mat4::from_rotation_y(std::f32::consts::PI);
    }
    Mat4::IDENTITY
}
pub fn warnings(bones: &BTreeMap<String, usize>) -> Vec<String> {
    let mut notes = vec!["Experimental VRC / GLB import: review detected bones and face inputs in Avatar → View → GLB rig & tracking. Named shape keys remain editable in Inputs and Expressions.".into(),
        "GLB carries meshes, skinning and morphs, not the Unity avatar descriptor, expression menus, FX controllers or VRChat PhysBones. Use ARIA expressions, presets and action nodes to rebuild behavior. Exported base color/alpha/emission use ARIA lighting; Unity shaders are not reproduced.".into()];
    let missing: Vec<_> = [
        "hips",
        "head",
        "neck",
        "leftUpperArm",
        "rightUpperArm",
        "leftEye",
        "rightEye",
    ]
    .into_iter()
    .filter(|n| !bones.contains_key(*n))
    .collect();
    if !missing.is_empty() {
        notes.push(format!("Bones needing review: {}. Missing optional bones are skipped; assign unnamed bones in GLB rig & tracking.", missing.join(", ")));
    }
    notes
}
pub fn expressions(j: &Value) -> Result<Vec<Expression>> {
    let active_meshes: BTreeSet<_> = active_nodes(j)
        .iter()
        .filter_map(|&n| index(&j["nodes"][n]["mesh"]))
        .collect();
    let mut out = Vec::new();
    for mesh in active_meshes {
        let m = &j["meshes"][mesh];
        let count = array(&m["primitives"])
            .first()
            .map_or(0, |p| array(&p["targets"]).len());
        ensure!(
            count <= 1024 && out.len() + count <= 2048,
            "GLB exceeds 1024 morphs per mesh or 2048 total morph controls"
        );
        ensure!(
            array(&m["primitives"])
                .iter()
                .all(|p| array(&p["targets"]).len() == count),
            "GLB mesh primitives have inconsistent morph counts"
        );
        for i in 0..count {
            let raw = m["extras"]["targetNames"][i]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Shape {i}"));
            out.push(Expression {
                name: format!("{} / {}", m["name"].as_str().unwrap_or("Mesh"), raw)
                    .chars()
                    .take(256)
                    .collect(),
                preset: raw.chars().take(128).collect(),
                binds: vec![MorphBind {
                    node: None,
                    mesh: Some(mesh),
                    index: i,
                    weight: 1.,
                }],
                binary: false,
                overrides: Default::default(),
            });
        }
    }
    Ok(out)
}

/// Use calibrated common parameters. Raw ARKit shapes remain available in Inputs.
pub fn bindings(expressions: &[Expression]) -> BTreeMap<String, Binding> {
    let mut result = BTreeMap::new();
    let meshes: BTreeSet<_> = expressions.iter().filter_map(|e| e.binds[0].mesh).collect();
    for mesh in meshes {
        let shapes: Vec<_> = expressions
            .iter()
            .enumerate()
            .filter(|(_, e)| e.binds[0].mesh == Some(mesh))
            .collect();
        let find = |aliases: &[&str]| {
            aliases.iter().find_map(|alias| {
                shapes
                    .iter()
                    .find(|(_, e)| normalized(&e.preset) == *alias)
                    .map(|(i, _)| *i)
            })
        };
        let mut add = |i: usize, input: &str, inverted: bool| {
            let mut binding = Binding::direct(input, 0., 1.);
            if inverted {
                binding.output_min = 1.;
                binding.output_max = 0.;
            }
            result.insert(super::expression_id(i), binding);
        };
        if let Some(i) = find(&["vrcvaa", "jawopen", "moutha", "mouthopen", "aa"]) {
            add(i, "ParamMouthOpenY", false);
        }
        let left = find(&["eyeblinkleft", "eyeclose1l", "blinkleft", "blinkl"]);
        let right = find(&["eyeblinkright", "eyeclose1r", "blinkright", "blinkr"]);
        if let (Some(l), Some(r)) = (left, right) {
            add(l, "ParamEyeLOpen", true);
            add(r, "ParamEyeROpen", true);
        } else if let Some(i) =
            find(&["vrcblink30", "vrcblink", "blink", "eyesclosed", "eyeclose1"])
        {
            add(i, "ParamEyeLOpen", true);
        }
        for (aliases, input) in [
            (&["mouthsmileleft", "mouthsmile1l"][..], "MouthSmile"),
            (&["mouthsmileright", "mouthsmile1r"][..], "MouthSmile"),
            (&["browouterupleft", "browup1l"][..], "Brows"),
            (&["browouterupright", "browup1r"][..], "Brows"),
        ] {
            if let Some(i) = find(aliases) {
                add(i, input, false);
            }
        }
        for (i, e) in shapes {
            let name = normalized(&e.preset);
            if !result.contains_key(&super::expression_id(i))
                && super::ARKIT.split_whitespace().any(|a| a == name)
                && name != "jawopen"
                && !name.starts_with("eyeblink")
                && !name.starts_with("eyelook")
            {
                result.insert(
                    super::expression_id(i),
                    Binding::direct(&format!("ARKit:{name}"), 0., 1.),
                );
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn bone_matching_ignores_twists_ambiguous_names_and_other_scenes() {
        let mut j = json!({"scenes":[{"nodes":[0]}], "nodes":[
            {"name":"root","children":[1,2,3,4,5,6]}, {"name":"Hips"}, {"name":"ns:Head"},
            {"name":"UpperArm_L"}, {"name":"UpperArm_twist_L"}, {"name":"mixamorig:RightArm"}, {"name":"Head.001"}, {"name":"Head"}],
            "skins":[{"joints":[1,2,3,4,5,6,7]}]});
        let map = detect_bones(&j);
        assert_eq!(map["head"], 2);
        assert_eq!(map["leftUpperArm"], 3);
        assert_eq!(map["rightUpperArm"], 5);
        j["nodes"][6]["name"] = json!("Head");
        assert!(!detect_bones(&j).contains_key("head"));
    }
    #[test]
    fn common_tracking_pipeline_drives_glb_and_allows_remapping() {
        let j = json!({"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"name":"Face","primitives":[{"targets":[{},{},{},{}]}],"extras":{"targetNames":["vrc.v_aa","eye_close_1_L","eye_close_1_R","cheekPuff"]}}]});
        let expressions = expressions(&j).unwrap();
        let mut parameters: Vec<_> = (0..4)
            .map(|i| aria_core::rig::RigParameter {
                id: super::super::expression_id(i),
                min: 0.,
                max: 1.,
                default: 0.,
                value: 0.,
            })
            .collect();
        let mut rig = aria_core::movement::RigConfig::from_parameters(&parameters);
        rig.bindings.extend(bindings(&expressions));
        // All phone, webcam and RTX receivers feed TrackingFrame -> common mapper.
        let mut frame = aria_core::TrackingFrame {
            face_found: true,
            ..Default::default()
        };
        frame.blend_shapes.insert("cheekpuff".into(), 0.65);
        let mut p = aria_core::Parameters::default();
        p.0[5] = 0.8;
        p.0[3] = 0.;
        p.0[4] = 1.;
        let inputs = aria_core::rig::tracking_inputs(Some(&frame), p, false, 1.);
        rig.evaluate(&inputs, &mut parameters, 1. / 60., None);
        assert_eq!(
            parameters.iter().map(|p| p.value).collect::<Vec<_>>(),
            [0.8, 1., 0., 0.65]
        );
        // Microphone-only sessions supply the same calibrated mouth parameter.
        p.0[5] = 0.3;
        rig.evaluate(
            &aria_core::rig::tracking_inputs(None, p, false, 1.),
            &mut parameters,
            1. / 60.,
            None,
        );
        assert!((parameters[0].value - 0.3).abs() < 0.0001);
        rig.bindings.insert(
            super::super::expression_id(0),
            Binding::direct("NP_L2", 0., 1.),
        );
        rig.evaluate(
            &BTreeMap::from([("NP_L2".into(), 0.9)]),
            &mut parameters,
            1. / 60.,
            None,
        );
        assert!((parameters[0].value - 0.9).abs() < 0.0001);
        rig.vrm.bone_map.insert("head".into(), Some(7));
        rig.vrm.bone_map.insert("neck".into(), None);
        rig.validate(&parameters).unwrap();
        let saved: aria_core::movement::RigConfig =
            serde_json::from_slice(&serde_json::to_vec(&rig).unwrap()).unwrap();
        assert_eq!(saved.vrm.bone_map, rig.vrm.bone_map);
    }
}
