//! Static GLB/glTF/VRM and FBX/OBJ import. Assets are centered for prop rendering.
use anyhow::{Context, Result, ensure};
use base64::Engine;
use glam::{Mat4, Vec3};
use std::path::{Path, PathBuf};
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}
pub struct Part {
    pub vertices: Vec<Vertex>,
    pub image: Option<image::RgbaImage>,
}
pub struct Mesh {
    pub parts: Vec<Part>,
    pub warnings: Vec<String>,
}
const MAX_VERTICES: usize = 600_000;
pub fn cube() -> Mesh {
    let corners = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    let faces = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
        [3, 7, 6, 2],
        [0, 1, 5, 4],
    ];
    let mut vertices = Vec::new();
    for (n, f) in faces.iter().enumerate() {
        let color = if n % 2 == 0 {
            [0.43, 0.91, 0.81, 1.0]
        } else {
            [0.71, 0.52, 0.98, 1.0]
        };
        let a = Vec3::from(corners[f[0]]);
        let normal = (Vec3::from(corners[f[1]]) - a)
            .cross(Vec3::from(corners[f[2]]) - a)
            .normalize()
            .to_array();
        for k in [0, 1, 2, 0, 2, 3] {
            vertices.push(Vertex {
                position: corners[f[k]],
                normal,
                uv: [0.0; 2],
                color,
            });
        }
    }
    Mesh {
        parts: vec![Part {
            vertices,
            image: None,
        }],
        warnings: vec![],
    }
}
pub fn load(path: &Path) -> Result<Mesh> {
    let bytes = aria_model::read_bounded(path, 128 * 1024 * 1024)?;
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let mut mesh = if matches!(ext.as_str(), "glb" | "gltf" | "vrm") {
        load_gltf(path, &bytes)?
    } else {
        load_fbx(path, &bytes)?
    };
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut count = 0;
    for part in &mesh.parts {
        for v in &part.vertices {
            let p = Vec3::from(v.position);
            ensure!(
                p.is_finite() && p.abs().max_element() < 1e10,
                "Invalid mesh positions"
            );
            ensure!(
                v.normal
                    .iter()
                    .chain(&v.uv)
                    .chain(&v.color)
                    .all(|v| v.is_finite()),
                "Non-finite mesh attributes"
            );
            min = min.min(p);
            max = max.max(p);
            count += 1;
        }
    }
    ensure!(
        count > 0 && count <= MAX_VERTICES,
        "Use a static triangle mesh with at most 200,000 triangles"
    );
    let scale = (max - min).max_element();
    ensure!(scale > 1e-8, "Model has no visible size");
    let center = (min + max) * 0.5;
    for part in &mut mesh.parts {
        for v in &mut part.vertices {
            v.position = ((Vec3::from(v.position) - center) / scale * 1.2).to_array();
        }
    }
    Ok(mesh)
}
pub fn local_path(base: &Path, name: &str) -> Result<PathBuf> {
    ensure!(
        !name.contains("://") && !name.starts_with("data:"),
        "Only local companion files are supported"
    );
    let root = base.canonicalize()?;
    let path = root.join(name.replace('\\', "/")).canonicalize()?;
    ensure!(
        path.starts_with(&root),
        "Companion files must stay inside the asset's folder"
    );
    Ok(path)
}
fn uri(base: &Path, s: &str) -> Result<Vec<u8>> {
    if let Some((header, data)) = s.strip_prefix("data:").and_then(|s| s.split_once(',')) {
        ensure!(
            header.ends_with(";base64") && data.len() <= 180 * 1024 * 1024,
            "Unsupported embedded asset data"
        );
        return Ok(base64::engine::general_purpose::STANDARD.decode(data)?);
    }
    // glTF URIs percent-encode spaces. Reject escapes that resolve outside the folder.
    let mut decoded = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            decoded.push(u8::from_str_radix(
                std::str::from_utf8(&b[i + 1..i + 3])?,
                16,
            )?);
            i += 3;
        } else {
            decoded.push(b[i]);
            i += 1;
        }
    }
    aria_model::read_bounded(
        &local_path(base, &String::from_utf8(decoded)?)?,
        128 * 1024 * 1024,
    )
}
fn texture(bytes: &[u8]) -> Result<image::RgbaImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.to_rgba8())
}
fn load_gltf(path: &Path, bytes: &[u8]) -> Result<Mesh> {
    let doc = gltf::Gltf::from_slice(bytes)?;
    let base = path.parent().unwrap();
    let mut buffers = Vec::new();
    let mut used = 0;
    for b in doc.buffers() {
        let data = match b.source() {
            gltf::buffer::Source::Bin => doc.blob.clone().context("GLB binary chunk missing")?,
            gltf::buffer::Source::Uri(s) => uri(base, s)?,
        };
        used += data.len();
        ensure!(used <= 256 * 1024 * 1024, "3D buffer budget exceeded");
        ensure!(data.len() >= b.length(), "Truncated glTF buffer");
        buffers.push(data);
    }
    let mut images = Vec::new();
    let mut decoded = 0;
    for im in doc.images() {
        let bytes = match im.source() {
            gltf::image::Source::Uri { uri: s, .. } => uri(base, s)?,
            gltf::image::Source::View { view, .. } => buffers[view.buffer().index()]
                .get(view.offset()..view.offset() + view.length())
                .context("Image buffer out of bounds")?
                .to_vec(),
        };
        let image = texture(&bytes)?;
        decoded += image.len();
        ensure!(decoded <= 256 * 1024 * 1024, "3D texture budget exceeded");
        images.push(image);
    }
    let mut out = Mesh {
        parts: vec![],
        warnings: vec![],
    };
    let mut count = 0;
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .context("glTF has no scene")?;
    let mut stack: Vec<_> = scene.nodes().map(|n| (n, Mat4::IDENTITY, 0)).collect();
    let mut visited = std::collections::BTreeSet::new();
    let mut material_bytes = 0;
    while let Some((node, parent, depth)) = stack.pop() {
        ensure!(
            visited.insert(node.index()) && visited.len() <= 10000,
            "Cyclic, repeated or excessive glTF nodes"
        );
        ensure!(depth < 128, "3D node hierarchy too deep");
        let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        stack.extend(node.children().map(|n| (n, world, depth + 1)));
        if node.skin().is_some() && !out.warnings.iter().any(|w| w.starts_with("Skinned")) {
            out.warnings.push("Skinned/VRM assets render in their exported rest pose; skeletal animation, spring bones and MToon shaders are not played.".into());
        }
        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                ensure!(
                    primitive.mode() == gltf::mesh::Mode::Triangles,
                    "Export triangle primitives for throw props"
                );
                let reader = primitive.reader(|b| buffers.get(b.index()).map(Vec::as_slice));
                let positions: Vec<_> = reader
                    .read_positions()
                    .context("Mesh has no positions")?
                    .collect();
                let normals: Vec<_> = reader
                    .read_normals()
                    .map(|v| v.collect())
                    .unwrap_or_default();
                let texcoords: Vec<_> = reader
                    .read_tex_coords(0)
                    .map(|v| v.into_f32().collect())
                    .unwrap_or_default();
                let colors: Vec<_> = reader
                    .read_colors(0)
                    .map(|v| v.into_rgba_f32().collect())
                    .unwrap_or_default();
                let indices: Vec<_> = reader
                    .read_indices()
                    .map(|v| v.into_u32().collect())
                    .unwrap_or_else(|| (0..positions.len() as u32).collect());
                count += indices.len();
                ensure!(
                    count <= MAX_VERTICES && indices.len() % 3 == 0,
                    "Too many or incomplete triangles"
                );
                let material = primitive.material().pbr_metallic_roughness();
                let tint = material.base_color_factor();
                let mut vertices = Vec::with_capacity(indices.len());
                let normal_matrix = world.inverse().transpose();
                for face in indices.as_chunks::<3>().0 {
                    let points = face
                        .iter()
                        .map(|&i| {
                            positions
                                .get(i as usize)
                                .map(|p| world.transform_point3(Vec3::from(*p)))
                                .context("Invalid vertex index")
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let flat = (points[1] - points[0])
                        .cross(points[2] - points[0])
                        .normalize_or_zero();
                    for (j, &index) in face.iter().enumerate() {
                        let i = index as usize;
                        let c = colors.get(i).copied().unwrap_or([1.0; 4]);
                        let n = normals
                            .get(i)
                            .map(|n| {
                                normal_matrix
                                    .transform_vector3(Vec3::from(*n))
                                    .normalize_or_zero()
                            })
                            .unwrap_or(flat);
                        vertices.push(Vertex {
                            position: points[j].to_array(),
                            normal: n.to_array(),
                            uv: texcoords.get(i).copied().unwrap_or([0.0; 2]),
                            color: std::array::from_fn(|k| c[k] * tint[k]),
                        });
                    }
                }
                let image = if let Some(t) = material.base_color_texture() {
                    let source = &images[t.texture().source().index()];
                    material_bytes += source.len();
                    ensure!(
                        material_bytes <= 256 * 1024 * 1024,
                        "Material texture budget exceeded"
                    );
                    Some(source.clone())
                } else {
                    None
                };
                out.parts.push(Part { vertices, image });
                ensure!(out.parts.len() <= 256, "Too many material batches");
            }
        }
    }
    Ok(out)
}
fn load_fbx(path: &Path, bytes: &[u8]) -> Result<Mesh> {
    let name = path.to_string_lossy();
    let mut opts = ufbx::LoadOpts {
        filename: name.as_ref().into(),
        generate_missing_normals: true,
        ignore_animation: true,
        load_external_files: false,
        ..Default::default()
    };
    opts.temp_allocator.memory_limit = 256 * 1024 * 1024;
    opts.result_allocator.memory_limit = 256 * 1024 * 1024;
    opts.node_depth_limit = 128;
    // OBJ material text is explicitly read within the model folder, never by an unrestricted callback.
    let mtl = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("obj"))
    {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| s.lines().find_map(|l| l.trim().strip_prefix("mtllib ")))
            .map(|s| local_path(path.parent().unwrap(), s.trim()))
            .transpose()?
            .unwrap_or_else(|| path.with_extension("mtl"))
    } else {
        path.with_extension("mtl")
    };
    let mtl_bytes = if mtl.is_file() {
        aria_model::read_bounded(&mtl, 4 * 1024 * 1024)?
    } else {
        vec![]
    };
    if !mtl_bytes.is_empty() {
        opts.obj_mtl_data = mtl_bytes.as_slice().into();
    }
    let scene =
        ufbx::load_memory(bytes, opts).map_err(|e| anyhow::anyhow!("3D import failed: {e:?}"))?;
    let mut out = Mesh {
        parts: vec![],
        warnings: vec![],
    };
    let mut count = 0;
    let mut material_bytes = 0;
    for node in &scene.nodes {
        if !node.visible {
            continue;
        }
        let Some(mesh) = &node.mesh else {
            continue;
        };
        ensure!(
            mesh.num_triangles * 3 <= MAX_VERTICES,
            "3D mesh exceeds 200,000 triangles"
        );
        let mut groups: std::collections::BTreeMap<usize, Vec<Vertex>> = Default::default();
        let mut tris = Vec::new();
        for (face_index, &face) in mesh.faces.iter().enumerate() {
            if face.num_indices < 3 {
                continue;
            }
            ufbx::triangulate_face_vec(&mut tris, mesh, face);
            count += tris.len();
            ensure!(
                count <= MAX_VERTICES,
                "Combined mesh exceeds 200,000 triangles"
            );
            let mat = mesh.face_material.get(face_index).copied().unwrap_or(0) as usize;
            let mut color = [0.75, 0.8, 0.9, 1.0];
            if let Some(m) = node.materials.get(mat) {
                let map = if m.pbr.base_color.has_value {
                    &m.pbr.base_color
                } else {
                    &m.fbx.diffuse_color
                };
                if map.has_value {
                    let v = map.value_vec4;
                    color = [v.x as f32, v.y as f32, v.z as f32, 1.0];
                }
            }
            let dest = groups.entry(mat).or_default();
            for &index in &tris {
                let i = index as usize;
                let p = ufbx::transform_position(&node.geometry_to_world, mesh.vertex_position[i]);
                let n = if mesh.vertex_normal.exists {
                    ufbx::transform_direction(&node.geometry_to_world, mesh.vertex_normal[i])
                } else {
                    ufbx::Vec3 {
                        x: 0.0,
                        y: 0.0,
                        z: 1.0,
                    }
                };
                let uv = if mesh.vertex_uv.exists {
                    let uv = mesh.vertex_uv[i];
                    [uv.x as f32, 1.0 - uv.y as f32]
                } else {
                    [0.0; 2]
                };
                dest.push(Vertex {
                    position: [p.x as f32, p.y as f32, p.z as f32],
                    normal: Vec3::new(n.x as f32, n.y as f32, n.z as f32)
                        .normalize_or_zero()
                        .to_array(),
                    uv,
                    color,
                });
            }
        }
        for (mat, vertices) in groups {
            let mut image = None;
            if let Some(m) = node.materials.get(mat)
                && let Some(t) = m.pbr.base_color.texture.as_ref().or(m
                    .fbx
                    .diffuse_color
                    .texture
                    .as_ref())
            {
                let result = if !t.content.is_empty() {
                    texture(&t.content)
                } else {
                    local_path(path.parent().unwrap(), t.relative_filename.as_ref())
                        .and_then(|p| aria_model::read_bounded(&p, 32 * 1024 * 1024))
                        .and_then(|b| texture(&b))
                };
                match result {
                    Ok(im) => {
                        material_bytes += im.len();
                        ensure!(
                            material_bytes <= 256 * 1024 * 1024,
                            "Material texture budget exceeded"
                        );
                        image = Some(im);
                    }
                    Err(e) => out
                        .warnings
                        .push(format!("Material texture unavailable: {e}")),
                }
            }
            out.parts.push(Part { vertices, image });
            ensure!(out.parts.len() <= 256, "Too many material batches");
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_template_formats_import_geometry_and_local_companions() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/effects/assets");
        for ext in ["glb", "gltf", "obj", "fbx"] {
            let mesh = load(&root.join(format!("cube.{ext}"))).unwrap();
            assert_eq!(
                mesh.parts.iter().map(|p| p.vertices.len()).sum::<usize>(),
                36,
                "{ext}"
            );
            assert!(
                mesh.parts
                    .iter()
                    .flat_map(|p| &p.vertices)
                    .all(|v| v.position.iter().all(|v| v.abs() <= 0.61))
            );
        }
        // VRM uses a GLB container; this tests that import path, without claiming a rigged VRM template.
        let temp = tempfile::tempdir().unwrap();
        let vrm = temp.path().join("static-container.vrm");
        std::fs::copy(root.join("cube.glb"), &vrm).unwrap();
        assert_eq!(load(&vrm).unwrap().parts[0].vertices.len(), 36);
        assert!(local_path(&root, "../README.md").is_err());
        assert!(local_path(&root, "https://example.invalid/art.png").is_err());
        let mut bad: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("cube.gltf")).unwrap()).unwrap();
        bad["buffers"][0]["uri"] = "../escape.bin".into();
        let path = temp.path().join("bad.gltf");
        std::fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(load(&path).is_err());
    }
}
