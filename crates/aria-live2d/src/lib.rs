//! Owned, single-threaded Purism Core runtime. Rendering consumes plain Rust mesh data.
//! The MIT runtime is compiled in; no proprietary SDK or external library is loaded.
mod ffi;
pub mod host;
use anyhow::{Context, Result, ensure};
use ffi::{Aligned, Api, V2, array, count};
use std::{collections::BTreeMap, ffi::CStr, marker::PhantomData, path::Path, rc::Rc};

pub use aria_core::rig::RigParameter as Parameter;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Drawable {
    pub id: String,
    pub part: String,
    pub positions: Vec<[f32; 2]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
    pub texture: usize,
    pub masks: Vec<usize>,
    pub masked: bool,
    pub inverted: bool,
    pub double_sided: bool,
    pub visible: bool,
    pub order: i32,
    pub opacity: f32,
    pub multiply: [f32; 4],
    pub screen: [f32; 4],
    pub blend: Blend,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Canvas {
    pub size: [f32; 2],
    pub origin: [f32; 2],
    pub pixels_per_unit: f32,
}

pub struct CubismModel {
    model: *mut ffi::Model,
    // Field drop order: model storage before the MOC storage it references.
    _model_memory: Aligned,
    _moc_memory: Aligned,
    api: Api,
    parameters: Vec<Parameter>,
    parameter_lookup: BTreeMap<String, usize>,
    pub parts: Vec<Parameter>,
    pub canvas: Canvas,
    pub version: String,
    pub drawables: Vec<Drawable>,
    _single_thread: PhantomData<Rc<()>>,
}

impl CubismModel {
    /// The legacy path argument is ignored; retained for source compatibility.
    pub fn load(_legacy_core_path: &Path, bytes: &[u8], texture_count: usize) -> Result<Self> {
        ensure!(
            bytes.len() >= 64
                && bytes.len() <= aria_core::asset_limits::MOC_FILE
                && bytes.starts_with(b"MOC3"),
            "Invalid .moc3 header/size (maximum 1280 MiB)"
        );
        ensure!((1..=32).contains(&texture_count), "Expected 1–32 textures");
        let api = Api::bundled();
        let mut moc_memory = Aligned::new(bytes.len(), 64)?;
        moc_memory.copy_from(bytes);
        // SAFETY: Calls use the documented ABI, aligned owned storage, checked sizes and
        // a model initialized by Core. All returned arrays are copied before any mutation.
        // Native parsing runs in the isolated host in desktop use; it is not an OS sandbox.
        unsafe {
            let version = (api.version)();
            ensure!(
                (4..=6).contains(&(version >> 24)),
                "Unsupported Core version {version:#010x}"
            );
            let moc_version = (api.moc_version)(moc_memory.ptr(), bytes.len() as u32);
            ensure!(
                moc_version != 0 && moc_version <= (api.latest_moc)(),
                "This .moc3 requires a newer Purism Core; update ARIA"
            );
            ensure!(
                (api.consistent)(moc_memory.ptr(), bytes.len() as u32) == 1,
                "Purism Core rejected this .moc3: consistency check failed"
            );
            let moc = (api.revive)(moc_memory.ptr(), bytes.len() as u32);
            ensure!(!moc.is_null(), "Purism Core could not revive .moc3");
            let size = (api.model_size)(moc);
            let model_memory = Aligned::new(size as usize, 16)?;
            let model = (api.initialize)(moc, model_memory.ptr(), size);
            ensure!(
                !model.is_null(),
                "Purism Core could not initialize the model"
            );
            if let Some(offscreens) = api.offscreens {
                ensure!(
                    offscreens(model) == 0,
                    "This avatar uses Cubism 5.3 offscreen parts, which ARIA does not yet render. Export using compatible standard ArtMesh blending."
                );
            }
            let n = count((api.parameter_count)(model), 8192)?;
            let ids = array((api.parameter_ids)(model), n)?;
            let minimum = array((api.minimum)(model), n)?;
            let maximum = array((api.maximum)(model), n)?;
            let defaults = array((api.defaults)(model), n)?;
            let mut parameters = Vec::with_capacity(n);
            for i in 0..n {
                ensure!(!ids[i].is_null(), "Null parameter ID");
                let id = CStr::from_ptr(ids[i])
                    .to_str()
                    .context("Invalid parameter ID")?;
                ensure!(
                    id.len() <= 256
                        && minimum[i].is_finite()
                        && maximum[i].is_finite()
                        && defaults[i].is_finite()
                        && minimum[i] <= maximum[i],
                    "Invalid parameter metadata"
                );
                parameters.push(Parameter {
                    id: id.into(),
                    min: minimum[i],
                    max: maximum[i],
                    default: defaults[i].clamp(minimum[i], maximum[i]),
                    value: defaults[i].clamp(minimum[i], maximum[i]),
                });
            }
            let parameter_lookup = parameters
                .iter()
                .enumerate()
                .map(|(i, p)| (p.id.clone(), i))
                .collect();
            let (mut size, mut origin, mut ppu) = (V2::default(), V2::default(), 0.0);
            (api.canvas)(model, &mut size, &mut origin, &mut ppu);
            ensure!(
                [size.x, size.y, origin.x, origin.y, ppu]
                    .iter()
                    .all(|v| v.is_finite())
                    && size.x > 0.0
                    && size.y > 0.0
                    && ppu > 0.0,
                "Invalid model canvas"
            );
            let mut result = Self {
                model,
                _model_memory: model_memory,
                _moc_memory: moc_memory,
                parameters,
                parameter_lookup,
                parts: {
                    let n = count((api.part_count)(model), 8192)?;
                    let ids = array((api.part_ids)(model), n)?;
                    let opacities = array((api.part_opacities)(model), n)?;
                    let mut parts = Vec::with_capacity(n);
                    for (id, value) in ids.iter().zip(opacities) {
                        ensure!(!id.is_null() && value.is_finite(), "Invalid part opacity");
                        let id = CStr::from_ptr(*id).to_str()?;
                        ensure!(!id.is_empty() && id.len() <= 256, "Invalid part ID");
                        parts.push(Parameter {
                            id: id.into(),
                            min: 0.,
                            max: 1.,
                            default: value.clamp(0., 1.),
                            value: value.clamp(0., 1.),
                        });
                    }
                    parts
                },
                api,
                canvas: Canvas {
                    size: [size.x, size.y],
                    origin: [origin.x, origin.y],
                    pixels_per_unit: ppu,
                },
                version: format!(
                    "Purism Core 1.1.0 (ABI {}.{}.{})",
                    version >> 24,
                    (version >> 16) & 255,
                    version & 65535
                ),
                drawables: Vec::new(),
                _single_thread: PhantomData,
            };
            result.update()?;
            ensure!(
                result.drawables.iter().all(|d| d.texture < texture_count),
                "A texture atlas is missing: model references an index outside the supplied texture list"
            );
            Ok(result)
        }
    }

    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }
    pub fn set_parameter(&mut self, id: &str, value: f32) {
        if value.is_finite()
            && let Some(&index) = self.parameter_lookup.get(id)
        {
            let p = &mut self.parameters[index];
            p.value = value.clamp(p.min, p.max);
        }
    }
    pub fn reset_parameters(&mut self) {
        for p in &mut self.parameters {
            p.value = p.default;
        }
    }

    pub fn update(&mut self) -> Result<()> {
        // SAFETY: self exclusively owns a live model/moc/library. Counts are bounded;
        // C arrays are only borrowed until the next Core call and mesh output is owned.
        unsafe {
            let values = (self.api.values)(self.model);
            ensure!(
                self.parameters.is_empty() || !values.is_null(),
                "Null parameter values"
            );
            for (i, p) in self.parameters.iter().enumerate() {
                values.add(i).write(p.value);
            }
            let opacities = (self.api.part_opacities)(self.model);
            ensure!(
                self.parts.is_empty() || !opacities.is_null(),
                "Null part opacities"
            );
            for (i, p) in self.parts.iter().enumerate() {
                ensure!(p.value.is_finite(), "Invalid part opacity");
                opacities.add(i).write(p.value.clamp(0., 1.));
            }
            (self.api.reset)(self.model);
            (self.api.update)(self.model);
            let a = &self.api;
            let m = self.model;
            let n = count((a.drawable_count)(m), 8192)?;
            macro_rules! read {
                ($field:ident) => {
                    array((a.$field)(m), n)?
                };
            }
            let ids = read!(drawable_ids);
            let parents = read!(drawable_parts);
            let part_count = count((a.part_count)(m), 8192)?;
            let parts = array((a.part_ids)(m), part_count)?;
            let flags = read!(flags);
            let dynamic = read!(dynamic);
            let textures = read!(textures);
            let opacity = read!(opacity);
            let orders = read!(orders);
            let mask_counts = read!(mask_counts);
            let masks = read!(masks);
            let vertex_counts = read!(vertex_counts);
            let positions = read!(positions);
            let uvs = read!(uvs);
            let index_counts = read!(index_counts);
            let indices = read!(indices);
            let multiply = read!(multiply);
            let screen = read!(screen);
            if self.drawables.is_empty()
                && let Some(blends) = a.blend_modes
            {
                for mode in array(blends(m), n)? {
                    ensure!(
                        (0..=2).contains(mode),
                        "Unsupported Cubism 5.3 blend mode {mode}. Use Normal/AddCompatible/MultiplyCompatible ArtMeshes."
                    );
                }
            }
            // Core owns stable mesh topology. Reuse our owned copies instead
            // of allocating positions, UVs, indices and masks for every frame.
            self.drawables.resize_with(n, Drawable::default);
            let mut total_vertices = 0;
            let mut total_indices = 0;
            for i in 0..n {
                let vc = count(vertex_counts[i], 65536)?;
                total_vertices += vc;
                ensure!(
                    total_vertices <= 2_000_000,
                    "Model exceeds 2 million vertices"
                );
                let ic = count(index_counts[i], 1_000_000)?;
                total_indices += ic;
                ensure!(
                    total_indices <= 12_000_000,
                    "Model exceeds 12 million indices"
                );
                ensure!(ic.is_multiple_of(3), "Invalid triangle index count");
                let d = &mut self.drawables[i];
                let first_frame = d.id.is_empty();
                if first_frame {
                    ensure!(!ids[i].is_null(), "Null drawable ID");
                    d.id = std::ffi::CStr::from_ptr(ids[i]).to_str()?.to_owned();
                    ensure!(!d.id.is_empty() && d.id.len() <= 256, "Invalid drawable ID");
                    if parents[i] >= 0 {
                        let parent = count(parents[i], part_count.saturating_sub(1))?;
                        ensure!(
                            parent < parts.len() && !parts[parent].is_null(),
                            "Invalid drawable part"
                        );
                        d.part = std::ffi::CStr::from_ptr(parts[parent]).to_str()?.to_owned();
                        ensure!(d.part.len() <= 256, "Invalid part ID");
                    }
                }
                ensure!(
                    d.positions.is_empty() || d.positions.len() == vc,
                    "Model vertex count changed"
                );
                d.positions.resize(vc, [0.0; 2]);
                for (dst, src) in d.positions.iter_mut().zip(array(positions[i], vc)?) {
                    *dst = [src.x, src.y];
                }
                ensure!(
                    d.positions.iter().flatten().all(|v| v.is_finite()),
                    "Invalid mesh coordinates"
                );
                if first_frame {
                    d.uvs.extend(array(uvs[i], vc)?.iter().map(|v| [v.x, v.y]));
                    d.indices.extend_from_slice(array(indices[i], ic)?);
                    ensure!(
                        d.uvs.iter().flatten().all(|v| v.is_finite())
                            && d.indices.iter().all(|&v| (v as usize) < vc),
                        "Invalid mesh coordinates or triangle index"
                    );
                    let mc = count(mask_counts[i], n)?;
                    for &v in array(masks[i], mc)?.iter().filter(|&&v| v != -1) {
                        d.masks.push(count(v, n.saturating_sub(1))?);
                    }
                    d.texture = count(textures[i], 31)?;
                    d.masked = mc > 0;
                    d.inverted = flags[i] & 8 != 0;
                    d.double_sided = flags[i] & 4 != 0;
                    d.blend = if flags[i] & 1 != 0 {
                        Blend::Add
                    } else if flags[i] & 2 != 0 {
                        Blend::Multiply
                    } else {
                        Blend::Normal
                    };
                } else {
                    ensure!(
                        d.uvs.len() == vc && d.indices.len() == ic,
                        "Model topology changed"
                    );
                }
                let mul = multiply[i];
                let scr = screen[i];
                let mul = [mul.x, mul.y, mul.z, mul.w];
                let scr = [scr.x, scr.y, scr.z, scr.w];
                ensure!(
                    opacity[i].is_finite() && mul.iter().chain(&scr).all(|v| v.is_finite()),
                    "Invalid drawable color"
                );
                d.visible = dynamic[i] & 1 != 0;
                d.order = orders[i];
                d.opacity = opacity[i].clamp(0.0, 1.0);
                d.multiply = mul;
                d.screen = scr;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_purism_api_is_linked_without_a_runtime_path() {
        let api = Api::bundled();
        // SAFETY: These statically linked queries have no pointer arguments.
        unsafe {
            assert_eq!((api.version)(), 0x06000001);
            assert!((api.latest_moc)() >= 5);
        }
    }

    #[test]
    fn truncated_moc_is_rejected_by_bundled_runtime_with_legacy_path_ignored() {
        let mut bytes = [0_u8; 64];
        bytes[..4].copy_from_slice(b"MOC3");
        bytes[4] = 3;
        let error = CubismModel::load(Path::new("never-load-this.dll"), &bytes, 1)
            .err()
            .expect("truncated model must fail");
        assert!(error.to_string().contains("consistency"), "{error:#}");
    }

    #[test]
    fn invalid_moc_is_rejected_before_loading_native_code() {
        assert!(
            CubismModel::load(Path::new("missing.dll"), &[0; 64], 1)
                .err()
                .unwrap()
                .to_string()
                .contains("header")
        );
    }
    #[test]
    fn owned_allocations_have_required_alignment() {
        for align in [16, 64] {
            let mem = Aligned::new(1234, align).unwrap();
            assert_eq!(mem.ptr() as usize % align, 0);
        }
        assert!(Aligned::new(0, 64).is_err());
    }
    #[test]
    #[ignore = "requires ARIA_TEST_MOC; reports a local performance sample"]
    fn real_core_update_benchmark() {
        let path = std::env::var_os("ARIA_TEST_MOC").expect("ARIA_TEST_MOC");
        let bytes = std::fs::read(path).unwrap();
        let mut model = CubismModel::load(Path::new(""), &bytes, 32).unwrap();
        let parameter = model
            .parameters()
            .iter()
            .find(|p| p.max > p.min)
            .unwrap()
            .clone();
        for frame in 0..20 {
            model.set_parameter(
                &parameter.id,
                if frame % 2 == 0 {
                    parameter.min
                } else {
                    parameter.max
                },
            );
            model.update().unwrap();
        }
        let started = std::time::Instant::now();
        for frame in 0..120 {
            model.set_parameter(
                &parameter.id,
                if frame % 2 == 0 {
                    parameter.min
                } else {
                    parameter.max
                },
            );
            model.update().unwrap();
        }
        println!(
            "core model update: {:.2} ms/frame, {} meshes, {} vertices",
            started.elapsed().as_secs_f64() * 1000. / 120.,
            model.drawables.len(),
            model
                .drawables
                .iter()
                .map(|d| d.positions.len())
                .sum::<usize>()
        );
    }
    /// Opt-in integration test, using locally supplied licensed assets. No fixture is redistributed.
    #[test]
    #[ignore = "requires ARIA_TEST_MOC paths"]
    fn real_core_deforms_model_and_reloads() {
        let dll = std::ffi::OsString::new();
        let path = std::env::var_os("ARIA_TEST_MOC").expect("ARIA_TEST_MOC");
        let bytes = std::fs::read(path).unwrap();
        for _ in 0..3 {
            let mut model = CubismModel::load(Path::new(&dll), &bytes, 32).unwrap();
            assert!(!model.drawables.is_empty());
            let before = model
                .drawables
                .iter()
                .flat_map(|d| d.positions.clone())
                .collect::<Vec<_>>();
            let buffers: Vec<_> = model
                .drawables
                .iter()
                .map(|d| {
                    (
                        d.positions.as_ptr(),
                        d.uvs.as_ptr(),
                        d.indices.as_ptr(),
                        d.masks.as_ptr(),
                    )
                })
                .collect();
            for p in model.parameters.clone() {
                model.set_parameter(&p.id, p.max);
            }
            model.update().unwrap();
            assert_eq!(
                buffers,
                model
                    .drawables
                    .iter()
                    .map(|d| (
                        d.positions.as_ptr(),
                        d.uvs.as_ptr(),
                        d.indices.as_ptr(),
                        d.masks.as_ptr()
                    ))
                    .collect::<Vec<_>>(),
                "Native mesh copies should reuse their allocations"
            );
            let after = model
                .drawables
                .iter()
                .flat_map(|d| d.positions.clone())
                .collect::<Vec<_>>();
            assert_ne!(
                before, after,
                "Model parameters must change actual mesh vertices"
            );
        }
    }
}
