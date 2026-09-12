//! Bounded, self-contained VRM 0.x / 1.0 import. Shared glTF accessors stay shared.
use anyhow::{Context, Result, ensure};
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub const FILE_LIMIT: usize = 512 * 1024 * 1024;
const VERTEX_LIMIT: usize = 2_000_000;
const INDEX_LIMIT: usize = 6_000_000;
const TEXTURE_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}
#[derive(Clone)]
pub struct Node {
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub world: Mat4,
}
pub struct Skin {
    pub joints: Vec<usize>,
    pub inverse: Vec<Mat4>,
}
pub struct Morph {
    pub position: Vec<Vec3>,
    pub normal: Vec<Vec3>,
}
pub struct Geometry {
    pub node: usize,
    pub mesh: usize,
    pub skin: Option<usize>,
    pub vertices: Vec<Vertex>,
    pub morphs: Vec<Morph>,
    pub weights: Vec<f32>,
}
pub struct Part {
    pub geometry: usize,
    pub indices: Vec<u32>,
    pub material: usize,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub color: [f32; 4],
    pub shade: [f32; 4],
    pub emission: [f32; 4],
    pub rim: [f32; 4],
    // shade shift, tooniness, alpha cutoff, unlit
    pub params: [f32; 4],
    // normal scale, rim power, rim lift, matcap multiplier
    pub extra: [f32; 4],
    // base UV scale and offset
    pub uv: [f32; 4],
    pub outline: [f32; 4],
    // outline world width, alpha mode, UV cosine, UV sine
    pub mode: [f32; 4],
}
pub struct Material {
    pub uniform: MaterialUniform,
    pub textures: [Option<usize>; 5],
    pub cull: u32,
    pub queue: i32,
}
#[derive(Clone)]
pub struct MorphBind {
    pub node: Option<usize>,
    pub mesh: Option<usize>,
    pub index: usize,
    pub weight: f32,
}
#[derive(Clone)]
pub struct Expression {
    pub name: String,
    pub preset: String,
    pub binds: Vec<MorphBind>,
    pub binary: bool,
    pub overrides: [String; 3],
}
#[derive(Clone, Debug)]
pub struct Summary {
    pub version: String,
    pub name: String,
    pub author: String,
    pub license: String,
    pub bones: usize,
    pub expressions: usize,
    pub springs: usize,
    pub materials: usize,
}
pub struct Asset {
    pub path: PathBuf,
    pub key: String,
    pub summary: Summary,
    pub nodes: Vec<Node>,
    pub order: Vec<usize>,
    pub skins: Vec<Skin>,
    pub bones: BTreeMap<String, usize>,
    pub geometry: Vec<Geometry>,
    pub parts: Vec<Part>,
    pub materials: Vec<Material>,
    pub images: Vec<Option<image::RgbaImage>>,
    pub expressions: Vec<Expression>,
    pub springs: Vec<super::spring::Group>,
    pub warnings: Vec<String>,
    pub front: Mat4,
    pub bounds: [Vec3; 2],
    pub look_expression: bool,
    pub eye_ranges: [f32; 4],
}
pub fn number(v: &Value, default: f32) -> f32 {
    v.as_f64()
        .map(|x| x as f32)
        .filter(|x| x.is_finite())
        .unwrap_or(default)
}
pub fn index(v: &Value) -> Option<usize> {
    v.as_u64().and_then(|x| usize::try_from(x).ok())
}
pub fn array(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn vector(v: &Value, default: Vec3) -> Vec3 {
    if v.is_array() {
        Vec3::new(
            number(&v[0], default.x),
            number(&v[1], default.y),
            number(&v[2], default.z),
        )
    } else {
        Vec3::new(
            number(&v["x"], default.x),
            number(&v["y"], default.y),
            number(&v["z"], default.z),
        )
    }
}
fn color(v: &Value, default: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| number(&v[i], default[i]))
}
fn read(path: &Path) -> Result<(Vec<u8>, Value)> {
    let bytes = aria_model::read_bounded(path, FILE_LIMIT)
        .context("VRM file exceeds 512 MiB or cannot be read")?;
    let glb = gltf::binary::Glb::from_slice(&bytes).context("Choose a binary .vrm export")?;
    ensure!(glb.header.version == 2, "VRM requires glTF 2.0");
    ensure!(
        glb.json.len() <= 32 * 1024 * 1024,
        "VRM metadata exceeds 32 MiB"
    );
    let json: Value = serde_json::from_slice(&glb.json)?;
    summary(&json)?;
    Ok((bytes, json))
}
pub fn inspect(path: &Path) -> Result<Summary> {
    let (_, j) = read(path)?;
    summary(&j)
}
fn summary(j: &Value) -> Result<Summary> {
    let old = &j["extensions"]["VRM"];
    let new = &j["extensions"]["VRMC_vrm"];
    ensure!(
        old.is_object() || new.is_object(),
        "This GLB has no VRM humanoid extension. Export VRM 0.x or VRM 1.0."
    );
    let is_old = old.is_object();
    let v = if is_old { old } else { new };
    ensure!(
        is_old || v["specVersion"] == "1.0",
        "Only VRM 0.x and VRM 1.0 are supported"
    );
    let meta = &v["meta"];
    let text = |x: &Value| {
        x.as_str()
            .unwrap_or("")
            .chars()
            .take(256)
            .collect::<String>()
    };
    Ok(Summary {
        version: if is_old { "0.x" } else { "1.0" }.into(),
        name: text(&meta[if is_old { "title" } else { "name" }]),
        author: if is_old {
            text(&meta["author"])
        } else {
            array(&meta["authors"])
                .iter()
                .map(text)
                .collect::<Vec<_>>()
                .join(", ")
        },
        license: text(&meta[if is_old { "licenseName" } else { "licenseUrl" }]),
        bones: if is_old {
            array(&v["humanoid"]["humanBones"]).len()
        } else {
            v["humanoid"]["humanBones"]
                .as_object()
                .map_or(0, |o| o.len())
        },
        expressions: if is_old {
            array(&v["blendShapeMaster"]["blendShapeGroups"]).len()
        } else {
            ["preset", "custom"]
                .iter()
                .map(|k| v["expressions"][k].as_object().map_or(0, |o| o.len()))
                .sum()
        },
        springs: if is_old {
            array(&v["secondaryAnimation"]["boneGroups"]).len()
        } else {
            array(&j["extensions"]["VRMC_springBone"]["springs"]).len()
        },
        materials: array(&j["materials"]).len(),
    })
}

pub fn load(path: &Path, progress: &dyn Fn(&str) -> Result<()>) -> Result<Asset> {
    progress("Reading VRM metadata")?;
    let (bytes, mut json) = read(path)?;
    let summary = summary(&json)?;
    let old = summary.version == "0.x";
    let vrm = json["extensions"][if old { "VRM" } else { "VRMC_vrm" }].clone();
    let front = if old {
        Mat4::from_rotation_y(std::f32::consts::PI)
    } else {
        Mat4::IDENTITY
    };
    let mut warnings = Vec::new();
    // gltf-rs validates the core schema; VRM extensions are validated here.
    if let Some(required) = json
        .get_mut("extensionsRequired")
        .and_then(Value::as_array_mut)
    {
        for ext in required.iter() {
            ensure!(
                matches!(
                    ext.as_str(),
                    Some(
                        "VRM"
                            | "VRMC_vrm"
                            | "VRMC_materials_mtoon"
                            | "VRMC_springBone"
                            | "KHR_materials_unlit"
                            | "KHR_texture_transform"
                            | "KHR_materials_emissive_strength"
                    )
                ),
                "Unsupported required VRM extension: {ext}"
            );
        }
        required.clear();
    }
    let doc = gltf::Gltf::from_slice(&serde_json::to_vec(&json)?)
        .context("Invalid glTF structure in VRM")?;
    let glb = gltf::binary::Glb::from_slice(&bytes)?;
    let blob = glb
        .bin
        .as_deref()
        .context("VRM has no embedded binary buffer")?;
    ensure!(
        doc.buffers().len() == 1
            && doc.buffers().all(
                |b| matches!(b.source(), gltf::buffer::Source::Bin) && b.length() <= blob.len()
            ),
        "VRM must contain one embedded buffer; external resources are not loaded"
    );
    ensure!(
        doc.nodes().len() <= 4096
            && doc.materials().len() <= 256
            && doc.images().len() <= 256
            && doc.skins().len() <= 256,
        "VRM exceeds node, skin or material limits"
    );
    for view in doc.views() {
        ensure!(
            view.offset()
                .checked_add(view.length())
                .is_some_and(|end| end <= blob.len()),
            "VRM buffer view is truncated"
        );
    }
    for accessor in doc.accessors() {
        ensure!(
            accessor.count() > 0 && accessor.count() <= INDEX_LIMIT,
            "Invalid or excessive VRM accessor count"
        );
        let check = |view: gltf::buffer::View<'_>,
                     offset: usize,
                     count: usize,
                     size: usize|
         -> Result<()> {
            let stride = view.stride().unwrap_or(size);
            ensure!(
                stride >= size
                    && count > 0
                    && stride
                        .checked_mul(count - 1)
                        .and_then(|n| n.checked_add(offset))
                        .and_then(|n| n.checked_add(size))
                        .is_some_and(|end| end <= view.length()),
                "VRM accessor exceeds its buffer view"
            );
            Ok(())
        };
        if let Some(view) = accessor.view() {
            check(view, accessor.offset(), accessor.count(), accessor.size())?;
        }
        if let Some(sparse) = accessor.sparse() {
            ensure!(
                sparse.count() <= accessor.count(),
                "Invalid sparse accessor count"
            );
            let indices = sparse.indices();
            let values = sparse.values();
            check(
                indices.view(),
                indices.offset(),
                sparse.count(),
                indices.index_type().size(),
            )?;
            check(
                values.view(),
                values.offset(),
                sparse.count(),
                accessor.size(),
            )?;
        }
    }
    let mut nodes: Vec<_> = doc
        .nodes()
        .map(|n| {
            let (t, r, s) = n.transform().decomposed();
            Node {
                parent: None,
                children: n.children().map(|c| c.index()).collect(),
                translation: Vec3::from(t),
                rotation: Quat::from_array(r).normalize(),
                scale: Vec3::from(s),
                world: Mat4::IDENTITY,
            }
        })
        .collect();
    for i in 0..nodes.len() {
        ensure!(
            nodes[i].translation.is_finite()
                && nodes[i].rotation.is_finite()
                && nodes[i].scale.is_finite()
                && nodes[i].scale.abs().min_element() > 1e-6,
            "Invalid VRM node transform"
        );
        for child in nodes[i].children.clone() {
            ensure!(
                child < nodes.len() && nodes[child].parent.is_none(),
                "VRM node has multiple parents"
            );
            nodes[child].parent = Some(i);
        }
    }
    let mut order = Vec::new();
    let mut stack: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.parent.is_none())
        .map(|(i, _)| (i, 0))
        .collect();
    while let Some((i, depth)) = stack.pop() {
        ensure!(
            depth < 128 && order.len() < 4096,
            "VRM hierarchy is too deep"
        );
        let local = Mat4::from_scale_rotation_translation(
            nodes[i].scale,
            nodes[i].rotation,
            nodes[i].translation,
        );
        nodes[i].world = nodes[i].parent.map_or(local, |p| nodes[p].world * local);
        order.push(i);
        stack.extend(nodes[i].children.iter().map(|&c| (c, depth + 1)));
    }
    ensure!(
        order.len() == nodes.len(),
        "VRM contains a cyclic node hierarchy"
    );
    let mut bones = BTreeMap::new();
    if old {
        for b in array(&vrm["humanoid"]["humanBones"]) {
            if let (Some(name), Some(node)) = (b["bone"].as_str(), index(&b["node"])) {
                ensure!(node < nodes.len(), "Invalid humanoid bone");
                bones.insert(name.into(), node);
            }
        }
    } else if let Some(map) = vrm["humanoid"]["humanBones"].as_object() {
        for (name, b) in map {
            let node = index(&b["node"]).context("Missing humanoid node")?;
            ensure!(node < nodes.len(), "Invalid humanoid bone");
            bones.insert(name.clone(), node);
        }
    }
    ensure!(
        bones.contains_key("hips") && bones.contains_key("head"),
        "VRM must map at least hips and head bones"
    );
    let mut skins = Vec::new();
    for s in doc.skins() {
        let joints: Vec<_> = s.joints().map(|n| n.index()).collect();
        ensure!(
            !joints.is_empty() && joints.len() <= 1024,
            "Invalid or excessive VRM skin joints"
        );
        let inverse = s
            .reader(|_| Some(blob))
            .read_inverse_bind_matrices()
            .map(|r| r.map(|m| Mat4::from_cols_array_2d(&m)).collect())
            .unwrap_or_else(|| vec![Mat4::IDENTITY; joints.len()]);
        ensure!(
            inverse.len() == joints.len()
                && inverse
                    .iter()
                    .all(|m| m.is_finite() && m.determinant().abs() > 1e-12),
            "Invalid VRM inverse bind matrices"
        );
        skins.push(Skin { joints, inverse });
    }
    let mut expressions = Vec::new();
    if old {
        for e in array(&vrm["blendShapeMaster"]["blendShapeGroups"]) {
            if !array(&e["materialValues"]).is_empty() {
                warnings.push(
                    "Material-color expression binds are not evaluated in this build.".into(),
                );
            }
            let binds = array(&e["binds"])
                .iter()
                .map(|b| {
                    Ok(MorphBind {
                        node: None,
                        mesh: Some(index(&b["mesh"]).context("Missing expression mesh")?),
                        index: index(&b["index"]).context("Missing morph index")?,
                        weight: number(&b["weight"], 0.0) / 100.0,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            expressions.push(Expression {
                name: e["name"]
                    .as_str()
                    .unwrap_or("Expression")
                    .chars()
                    .take(128)
                    .collect(),
                preset: e["presetName"].as_str().unwrap_or("").to_ascii_lowercase(),
                binds,
                binary: e["isBinary"].as_bool().unwrap_or(false),
                overrides: Default::default(),
            });
        }
    } else {
        for section in ["preset", "custom"] {
            if let Some(map) = vrm["expressions"][section].as_object() {
                for (name, e) in map {
                    if !array(&e["materialColorBinds"]).is_empty()
                        || !array(&e["textureTransformBinds"]).is_empty()
                    {
                        warnings.push("Material-color and texture-transform expression binds are not evaluated in this build.".into());
                    }
                    let binds = array(&e["morphTargetBinds"])
                        .iter()
                        .map(|b| {
                            Ok(MorphBind {
                                node: Some(index(&b["node"]).context("Missing expression node")?),
                                mesh: None,
                                index: index(&b["index"]).context("Missing morph index")?,
                                weight: number(&b["weight"], 0.0),
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    expressions.push(Expression {
                        name: name.chars().take(128).collect(),
                        preset: if section == "preset" {
                            name.to_ascii_lowercase()
                        } else {
                            String::new()
                        },
                        binds,
                        binary: e["isBinary"].as_bool().unwrap_or(false),
                        overrides: ["overrideBlink", "overrideLookAt", "overrideMouth"]
                            .map(|k| e[k].as_str().unwrap_or("none").into()),
                    });
                }
            }
        }
    }
    ensure!(
        expressions.len() <= 256 && expressions.iter().all(|e| e.binds.len() <= 4096),
        "Too many VRM expression binds"
    );
    let mut materials = Vec::new();
    for (i, m) in array(&json["materials"]).iter().enumerate() {
        materials.push(material(m, &vrm["materialProperties"][i], old));
    }
    // glTF allows a primitive with no assigned material.
    materials.push(material(&Value::Null, &Value::Null, false));
    let mut images: Vec<Option<image::RgbaImage>> = (0..doc.images().len()).map(|_| None).collect();
    let textures: Vec<_> = doc.textures().map(|t| t.source().index()).collect();
    let mut texture_bytes = 0u64;
    for mat in &mut materials {
        for tex in mat.textures.iter_mut().flatten() {
            let im = *textures
                .get(*tex)
                .context("Invalid VRM material texture index")?;
            *tex = im;
            if images[im].is_some() {
                continue;
            }
            progress(&format!("Decoding texture {} of {}", im + 1, images.len()))?;
            let source = doc.images().nth(im).unwrap();
            let data = match source.source() {
                gltf::image::Source::View { view, .. } => blob
                    .get(
                        view.offset()
                            ..view
                                .offset()
                                .checked_add(view.length())
                                .context("Invalid image length")?,
                    )
                    .context("VRM image buffer is truncated")?,
                _ => anyhow::bail!(
                    "VRM textures must be embedded; external resources are not loaded"
                ),
            };
            let (w, h) = image::ImageReader::new(std::io::Cursor::new(data))
                .with_guessed_format()?
                .into_dimensions()?;
            texture_bytes = texture_bytes
                .checked_add(u64::from(w) * u64::from(h) * 4)
                .context("Image size overflow")?;
            ensure!(
                w > 0 && h > 0 && w <= 16384 && h <= 16384 && texture_bytes <= TEXTURE_LIMIT,
                "VRM textures exceed 16384px or 2 GiB decoded; reduce texture sizes"
            );
            let mut reader =
                image::ImageReader::new(std::io::Cursor::new(data)).with_guessed_format()?;
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(16384);
            limits.max_image_height = Some(16384);
            limits.max_alloc = Some(1024 * 1024 * 1024);
            reader.limits(limits);
            images[im] = Some(reader.decode()?.to_rgba8());
        }
    }
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .context("VRM has no scene")?;
    let mut active = vec![false; nodes.len()];
    let mut stack: Vec<_> = scene.nodes().map(|n| n.index()).collect();
    while let Some(n) = stack.pop() {
        ensure!(!active[n], "Repeated VRM scene node");
        active[n] = true;
        stack.extend(&nodes[n].children);
    }
    let mut geometry = Vec::<Geometry>::new();
    let mut parts = Vec::new();
    let mut cache = BTreeMap::new();
    let mut vertices_used = 0usize;
    let mut indices_used = 0usize;
    let mut morph_used = 0usize;
    for node in doc.nodes().filter(|n| active[n.index()]) {
        let Some(mesh) = node.mesh() else {
            continue;
        };
        progress(&format!(
            "Preparing mesh {}",
            mesh.name().unwrap_or("avatar")
        ))?;
        for prim in mesh.primitives() {
            ensure!(
                prim.mode() == gltf::mesh::Mode::Triangles,
                "VRM primitives must use triangles"
            );
            let pos = prim
                .get(&gltf::Semantic::Positions)
                .context("VRM mesh has no positions")?;
            let count = pos.count();
            ensure!(count > 0 && count <= VERTEX_LIMIT, "VRM mesh is too large");
            ensure!(
                prim.attributes().all(|(_, a)| a.count() == count),
                "Mismatched VRM vertex attribute counts"
            );
            for m in prim.morph_targets() {
                ensure!(
                    m.positions().is_none_or(|a| a.count() == count)
                        && m.normals().is_none_or(|a| a.count() == count),
                    "Mismatched VRM morph counts"
                );
            }
            let mut key = vec![node.index(), pos.index()];
            key.extend(prim.attributes().map(|(_, a)| a.index()));
            for m in prim.morph_targets() {
                key.extend([
                    m.positions().map_or(usize::MAX, |a| a.index()),
                    m.normals().map_or(usize::MAX, |a| a.index()),
                ]);
            }
            let reader = prim.reader(|_| Some(blob));
            let geometry_index = if let Some(&existing) = cache.get(&key) {
                existing
            } else {
                vertices_used += count;
                ensure!(
                    vertices_used <= VERTEX_LIMIT,
                    "VRM exceeds two million unique vertices"
                );
                let positions: Vec<_> = reader
                    .read_positions()
                    .context("Invalid position accessor")?
                    .collect();
                let normals: Vec<_> = reader
                    .read_normals()
                    .map(|v| v.collect())
                    .unwrap_or_else(|| vec![[0., 1., 0.]; count]);
                let uv: Vec<_> = reader
                    .read_tex_coords(0)
                    .map(|v| v.into_f32().collect())
                    .unwrap_or_else(|| vec![[0.; 2]; count]);
                let colors: Vec<_> = reader
                    .read_colors(0)
                    .map(|v| v.into_rgba_f32().collect())
                    .unwrap_or_else(|| vec![[1.; 4]; count]);
                let joints: Vec<_> = reader
                    .read_joints(0)
                    .map(|v| v.into_u16().map(|v| v.map(u32::from)).collect())
                    .unwrap_or_else(|| vec![[0; 4]; count]);
                let weights: Vec<_> = reader
                    .read_weights(0)
                    .map(|v| v.into_f32().collect())
                    .unwrap_or_else(|| vec![[1., 0., 0., 0.]; count]);
                ensure!(
                    [
                        positions.len(),
                        normals.len(),
                        uv.len(),
                        colors.len(),
                        joints.len(),
                        weights.len()
                    ]
                    .iter()
                    .all(|&n| n == count),
                    "Mismatched VRM vertex attributes"
                );
                let skin = node.skin().map(|s| s.index());
                let joint_count = skin.map_or(1, |s| skins[s].joints.len());
                let mut vertices = Vec::with_capacity(count);
                for i in 0..count {
                    ensure!(
                        positions[i]
                            .iter()
                            .chain(normals[i].iter())
                            .chain(uv[i].iter())
                            .chain(colors[i].iter())
                            .chain(weights[i].iter())
                            .all(|v| v.is_finite()),
                        "Non-finite VRM vertex data"
                    );
                    ensure!(
                        joints[i].iter().all(|&j| (j as usize) < joint_count)
                            && weights[i].iter().all(|&w| w >= 0.),
                        "Invalid VRM skin weights or joints"
                    );
                    let sum: f32 = weights[i].iter().sum();
                    vertices.push(Vertex {
                        position: positions[i],
                        normal: normals[i],
                        uv: uv[i],
                        color: colors[i],
                        joints: joints[i],
                        weights: if sum > 1e-6 {
                            weights[i].map(|w| w / sum)
                        } else {
                            [1., 0., 0., 0.]
                        },
                    });
                }
                let target_count = prim.morph_targets().len();
                ensure!(
                    target_count <= 256,
                    "VRM exceeds 256 morph targets per mesh"
                );
                morph_used = morph_used
                    .checked_add(count * target_count)
                    .context("Morph size overflow")?;
                ensure!(morph_used <= 32_000_000, "VRM morph data is too large");
                let mut morphs = Vec::new();
                for (position, normal, _) in reader.read_morph_targets() {
                    let position: Vec<_> = position
                        .map(|v| v.map(Vec3::from).collect())
                        .unwrap_or_default();
                    let normal: Vec<_> = normal
                        .map(|v| v.map(Vec3::from).collect())
                        .unwrap_or_default();
                    ensure!(
                        (position.is_empty() || position.len() == count)
                            && (normal.is_empty() || normal.len() == count)
                            && position.iter().chain(normal.iter()).all(|v| v.is_finite()),
                        "Invalid VRM morph accessor"
                    );
                    morphs.push(Morph { position, normal });
                }
                let mut weights = node
                    .weights()
                    .or_else(|| mesh.weights())
                    .unwrap_or(&[])
                    .to_vec();
                weights.resize(morphs.len(), 0.);
                ensure!(
                    weights.iter().all(|w| w.is_finite()),
                    "Invalid morph weights"
                );
                let id = geometry.len();
                geometry.push(Geometry {
                    node: node.index(),
                    mesh: mesh.index(),
                    skin,
                    vertices,
                    morphs,
                    weights,
                });
                cache.insert(key, id);
                id
            };
            if let Some(a) = prim.indices() {
                ensure!(a.count() <= INDEX_LIMIT, "VRM index accessor is too large");
            }
            let indices: Vec<_> = reader
                .read_indices()
                .map(|v| v.into_u32().collect())
                .unwrap_or_else(|| (0..count as u32).collect());
            indices_used += indices.len();
            ensure!(
                indices_used <= INDEX_LIMIT
                    && indices.len() % 3 == 0
                    && indices.iter().all(|&i| (i as usize) < count),
                "Invalid or excessive VRM triangle indices"
            );
            parts.push(Part {
                geometry: geometry_index,
                indices,
                material: prim.material().index().unwrap_or(materials.len() - 1),
            });
            ensure!(parts.len() <= 1024, "VRM exceeds 1024 draw parts");
        }
    }
    ensure!(!parts.is_empty(), "VRM has no drawable triangle meshes");
    for e in &expressions {
        for b in &e.binds {
            ensure!(
                b.weight.is_finite() && (0.0..=1.0).contains(&b.weight),
                "Invalid expression weight"
            );
            let matching: Vec<_> = geometry
                .iter()
                .filter(|g| {
                    b.node.is_none_or(|n| g.node == n) && b.mesh.is_none_or(|m| g.mesh == m)
                })
                .collect();
            ensure!(
                !matching.is_empty() && matching.iter().all(|g| b.index < g.morphs.len()),
                "Expression {} references an absent morph",
                e.name
            );
        }
    }
    let springs = super::spring::parse(&json, &vrm, old, &nodes)?;
    if array(&json["nodes"])
        .iter()
        .any(|n| n["extensions"]["VRMC_node_constraint"].is_object())
    {
        warnings.push("VRMC_node_constraint is not evaluated in this build.".into());
    }
    if doc.animations().len() > 0 {
        warnings
            .push("Embedded animation clips are not played; tracking drives the humanoid.".into());
    }
    warnings.sort();
    warnings.dedup();
    let fp = if old {
        &vrm["firstPerson"]
    } else {
        &vrm["lookAt"]
    };
    let look_expression = if old {
        fp["lookAtTypeName"] == "BlendShape"
    } else {
        fp["type"] == "expression"
    };
    let eye_ranges = if old {
        [
            "lookAtHorizontalInner",
            "lookAtHorizontalOuter",
            "lookAtVerticalUp",
            "lookAtVerticalDown",
        ]
        .map(|k| number(&fp[k]["yRange"], 10.0).clamp(0., 90.))
    } else {
        [
            "rangeMapHorizontalInner",
            "rangeMapHorizontalOuter",
            "rangeMapVerticalUp",
            "rangeMapVerticalDown",
        ]
        .map(|k| number(&fp[k]["outputScale"], 10.0).clamp(0., 90.))
    };
    let mut bounds = [Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)];
    for g in &geometry {
        for v in &g.vertices {
            let p = if let Some(s) = g.skin {
                let skin = &skins[s];
                let mut p = Vec3::ZERO;
                for k in 0..4 {
                    let j = v.joints[k] as usize;
                    p += (nodes[skin.joints[j]].world * skin.inverse[j])
                        .transform_point3(Vec3::from(v.position))
                        * v.weights[k];
                }
                p
            } else {
                nodes[g.node].world.transform_point3(Vec3::from(v.position))
            };
            let p = front.transform_point3(p);
            bounds[0] = bounds[0].min(p);
            bounds[1] = bounds[1].max(p);
        }
    }
    ensure!(
        bounds.iter().all(|v| v.is_finite())
            && (bounds[1] - bounds[0]).y > 0.01
            && (bounds[1] - bounds[0]).max_element() < 1000.,
        "Invalid VRM world bounds"
    );
    progress("Preparing the renderer")?;
    Ok(Asset {
        path: path.to_owned(),
        key: format!("vrm:{}", aria_core::movement::model_key(&bytes)),
        summary,
        nodes,
        order,
        skins,
        bones,
        geometry,
        parts,
        materials,
        images,
        expressions,
        springs,
        warnings,
        front,
        bounds,
        look_expression,
        eye_ranges,
    })
}

fn material(m: &Value, v: &Value, old: bool) -> Material {
    let old = old && v.is_object();
    let p = &m["pbrMetallicRoughness"];
    let mt = &m["extensions"]["VRMC_materials_mtoon"];
    let f = &v["floatProperties"];
    let c = &v["vectorProperties"];
    let t = &v["textureProperties"];
    let toon = if old {
        v["shader"].as_str().is_some_and(|s| s.contains("MToon"))
    } else {
        mt.is_object()
    };
    let mode = if old && v.is_object() {
        number(&f["_BlendMode"], 0.0) as u32
    } else {
        match m["alphaMode"].as_str() {
            Some("MASK") => 1,
            Some("BLEND") => 2,
            _ => 0,
        }
    };
    let base_tex = index(&t["_MainTex"])
        .filter(|_| old)
        .or_else(|| index(&p["baseColorTexture"]["index"]));
    let main_uv = color(&c["_MainTex"], [0., 0., 1., 1.]);
    let transform = &p["baseColorTexture"]["extensions"]["KHR_texture_transform"];
    let uv = if old {
        [main_uv[2], main_uv[3], main_uv[0], main_uv[1]]
    } else {
        [
            number(&transform["scale"][0], 1.),
            number(&transform["scale"][1], 1.),
            number(&transform["offset"][0], 0.),
            number(&transform["offset"][1], 0.),
        ]
    };
    let angle = number(&transform["rotation"], 0.);
    let unlit = if toon {
        false
    } else {
        m["extensions"]["KHR_materials_unlit"].is_object()
            || v["shader"].as_str().is_some_and(|s| s.contains("Unlit"))
    };
    let width = if old {
        if number(&f["_OutlineWidthMode"], 0.) > 0. {
            number(&f["_OutlineWidth"], 0.) * 0.01
        } else {
            0.
        }
    } else {
        number(&mt["outlineWidthFactor"], 0.)
    };
    Material {
        uniform: MaterialUniform {
            color: if old {
                color(&c["_Color"], color(&p["baseColorFactor"], [1.; 4]))
            } else {
                color(&p["baseColorFactor"], [1.; 4])
            },
            shade: if old {
                color(&c["_ShadeColor"], [0.8, 0.8, 0.8, 1.])
            } else {
                color(&mt["shadeColorFactor"], [0.8, 0.8, 0.8, 1.])
            },
            emission: if old {
                color(&c["_EmissionColor"], [0.; 4])
            } else {
                color(&m["emissiveFactor"], [0.; 4])
            },
            rim: if old {
                color(&c["_RimColor"], [0.; 4])
            } else {
                color(&mt["parametricRimColorFactor"], [0.; 4])
            },
            params: [
                if old {
                    number(&f["_ShadeShift"], 0.)
                } else {
                    number(&mt["shadingShiftFactor"], 0.)
                },
                if old {
                    number(&f["_ShadeToony"], 0.6)
                } else {
                    number(&mt["shadingToonyFactor"], 0.9)
                },
                if old {
                    number(&f["_Cutoff"], 0.5)
                } else {
                    number(&m["alphaCutoff"], 0.5)
                },
                f32::from(unlit),
            ],
            extra: [
                if old {
                    number(&f["_BumpScale"], 1.)
                } else {
                    number(&m["normalTexture"]["scale"], 1.)
                },
                if old {
                    number(&f["_RimFresnelPower"], 5.)
                } else {
                    number(&mt["parametricRimFresnelPowerFactor"], 5.)
                },
                if old {
                    number(&f["_RimLift"], 0.)
                } else {
                    number(&mt["parametricRimLiftFactor"], 0.)
                },
                1.,
            ],
            uv,
            outline: if old {
                color(&c["_OutlineColor"], [0., 0., 0., 1.])
            } else {
                color(&mt["outlineColorFactor"], [0., 0., 0., 1.])
            },
            mode: [width.clamp(0., 0.03), mode as f32, angle.cos(), angle.sin()],
        },
        textures: [
            base_tex,
            if old {
                index(&t["_ShadeTexture"])
            } else {
                index(&mt["shadeMultiplyTexture"]["index"])
            },
            if old {
                index(&t["_EmissionMap"])
            } else {
                index(&m["emissiveTexture"]["index"])
            },
            if old {
                index(&t["_BumpMap"])
            } else {
                index(&m["normalTexture"]["index"])
            },
            if old {
                index(&t["_SphereAdd"])
            } else {
                index(&mt["matcapTexture"]["index"])
            },
        ],
        cull: if old {
            number(&f["_CullMode"], 2.) as u32
        } else if m["doubleSided"].as_bool().unwrap_or(false) {
            0
        } else {
            2
        },
        queue: if old {
            v["renderQueue"]
                .as_i64()
                .unwrap_or(if mode >= 2 { 3000 } else { 2000 }) as i32
        } else {
            (if mode >= 2 { 3000 } else { 2000 })
                + mt["renderQueueOffsetNumber"].as_i64().unwrap_or(0) as i32
        },
    }
}
