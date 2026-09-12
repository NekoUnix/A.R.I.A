//! Owned, single-threaded Cubism Core runtime. Rendering consumes plain Rust mesh data.
//! Core must be supplied separately under Live2D's license.
mod ffi;
use anyhow::{Context, Result, ensure};
use ffi::{Aligned, Api, V2, array, count};
use std::{collections::BTreeMap, ffi::CStr, marker::PhantomData, path::Path, rc::Rc};

pub use aria_core::rig::RigParameter as Parameter;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
}

#[derive(Clone, Debug, Default)]
pub struct Drawable {
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

#[derive(Clone, Copy, Debug)]
pub struct Canvas {
    pub size: [f32; 2],
    pub origin: [f32; 2],
    pub pixels_per_unit: f32,
}

pub struct CubismModel {
    model: *mut ffi::Model,
    // Field drop order: model storage before moc storage, then the DLL.
    _model_memory: Aligned,
    _moc_memory: Aligned,
    api: Api,
    parameters: Vec<Parameter>,
    parameter_lookup: BTreeMap<String, usize>,
    pub canvas: Canvas,
    pub version: String,
    pub drawables: Vec<Drawable>,
    _single_thread: PhantomData<Rc<()>>,
}

impl CubismModel {
    pub fn load(core_path: &Path, bytes: &[u8], texture_count: usize) -> Result<Self> {
        ensure!(
            bytes.len() >= 64
                && bytes.len() <= aria_core::asset_limits::MOC_FILE
                && bytes.starts_with(b"MOC3"),
            "Invalid .moc3 header/size (maximum 1280 MiB)"
        );
        ensure!((1..=32).contains(&texture_count), "Expected 1–32 textures");
        let api = Api::load(core_path)?;
        let mut moc_memory = Aligned::new(bytes.len(), 64)?;
        moc_memory.copy_from(bytes);
        // SAFETY: Calls use the documented ABI, aligned owned storage, checked sizes and
        // a model initialized by Core. All returned arrays are copied before any mutation.
        // A genuine, current Core DLL is a trust boundary, not a sandbox for arbitrary DLLs.
        unsafe {
            let version = (api.version)();
            ensure!(
                (4..=6).contains(&(version >> 24)),
                "Unsupported Core version {version:#010x}"
            );
            let moc_version = (api.moc_version)(moc_memory.ptr(), bytes.len() as u32);
            ensure!(
                moc_version != 0 && moc_version <= (api.latest_moc)(),
                "This .moc3 requires a newer Cubism Core SDK"
            );
            ensure!(
                (api.consistent)(moc_memory.ptr(), bytes.len() as u32) == 1,
                "Cubism rejected this .moc3: consistency check failed"
            );
            let moc = (api.revive)(moc_memory.ptr(), bytes.len() as u32);
            ensure!(!moc.is_null(), "Cubism could not revive .moc3");
            let size = (api.model_size)(moc);
            let model_memory = Aligned::new(size as usize, 16)?;
            let model = (api.initialize)(moc, model_memory.ptr(), size);
            ensure!(!model.is_null(), "Cubism could not initialize the model");
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
                api,
                parameters,
                parameter_lookup,
                canvas: Canvas {
                    size: [size.x, size.y],
                    origin: [origin.x, origin.y],
                    pixels_per_unit: ppu,
                },
                version: format!(
                    "{}.{}.{}",
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
            if let Some(blends) = a.blend_modes {
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
                d.positions.clear();
                d.positions
                    .extend(array(positions[i], vc)?.iter().map(|v| [v.x, v.y]));
                d.uvs.clear();
                d.uvs.extend(array(uvs[i], vc)?.iter().map(|v| [v.x, v.y]));
                d.indices.clear();
                d.indices.extend_from_slice(array(indices[i], ic)?);
                ensure!(
                    d.positions
                        .iter()
                        .chain(&d.uvs)
                        .flatten()
                        .all(|v| v.is_finite())
                        && d.indices.iter().all(|&v| (v as usize) < vc),
                    "Invalid mesh coordinates or triangle index"
                );
                let mc = count(mask_counts[i], n)?;
                d.masks.clear();
                for &v in array(masks[i], mc)?.iter().filter(|&&v| v != -1) {
                    d.masks.push(count(v, n.saturating_sub(1))?);
                }
                let mul = multiply[i];
                let scr = screen[i];
                let mul = [mul.x, mul.y, mul.z, mul.w];
                let scr = [scr.x, scr.y, scr.z, scr.w];
                ensure!(
                    opacity[i].is_finite() && mul.iter().chain(&scr).all(|v| v.is_finite()),
                    "Invalid drawable color"
                );
                d.texture = count(textures[i], 31)?;
                d.masked = mc > 0;
                d.inverted = flags[i] & 8 != 0;
                d.double_sided = flags[i] & 4 != 0;
                d.visible = dynamic[i] & 1 != 0;
                d.order = orders[i];
                d.opacity = opacity[i].clamp(0.0, 1.0);
                d.multiply = mul;
                d.screen = scr;
                d.blend = if flags[i] & 1 != 0 {
                    Blend::Add
                } else if flags[i] & 2 != 0 {
                    Blend::Multiply
                } else {
                    Blend::Normal
                };
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    /// Opt-in integration test, using locally supplied licensed assets. No fixture is redistributed.
    #[test]
    #[ignore = "requires ARIA_CUBISM_CORE and ARIA_TEST_MOC paths"]
    fn real_core_deforms_model_and_reloads() {
        let dll = std::env::var_os("ARIA_CUBISM_CORE").expect("ARIA_CUBISM_CORE");
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
