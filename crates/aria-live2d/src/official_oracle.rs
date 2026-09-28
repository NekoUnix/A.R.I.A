//! Developer-only differential test against a locally installed official Core.
//! The DLL and model files are never copied into the repository or release.

#[cfg(test)]
mod tests {
    use crate::{
        CubismModel,
        ffi::{Aligned, V2},
    };
    use libloading::Library;
    use std::{
        ffi::{CStr, c_char, c_void},
        path::Path,
    };

    #[test]
    #[ignore = "requires ARIA_CUBISM_CORE and ARIA_TEST_MOC; official DLL stays local"]
    fn official_cubism_matches_current_runtime_on_animated_vertices() {
        let dll = std::env::var_os("ARIA_CUBISM_CORE").expect("ARIA_CUBISM_CORE");
        let path = std::env::var_os("ARIA_TEST_MOC").expect("ARIA_TEST_MOC");
        let bytes = std::fs::read(path).unwrap();
        let rust = aria_model_core::moc::Moc::parse(&bytes).unwrap();
        let mut current = CubismModel::load(Path::new(""), &bytes, 32).unwrap();

        // SAFETY: The local library is used only after its documented exported
        // functions have been resolved. MOC/model buffers have the Core ABI's
        // required alignment and remain alive for the entire comparison.
        unsafe {
            let library = Library::new(dll).unwrap();
            macro_rules! function {
                ($name:literal, $type:ty) => {
                    *library
                        .get::<$type>(concat!($name, "\0").as_bytes())
                        .unwrap()
                };
            }
            let version = function!("csmGetVersion", unsafe extern "C" fn() -> u32);
            let consistent = function!(
                "csmHasMocConsistency",
                unsafe extern "C" fn(*const c_void, u32) -> i32
            );
            let revive = function!(
                "csmReviveMocInPlace",
                unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void
            );
            let model_size = function!(
                "csmGetSizeofModel",
                unsafe extern "C" fn(*const c_void) -> u32
            );
            let initialize = function!(
                "csmInitializeModelInPlace",
                unsafe extern "C" fn(*const c_void, *mut c_void, u32) -> *mut c_void
            );
            let values = function!(
                "csmGetParameterValues",
                unsafe extern "C" fn(*mut c_void) -> *mut f32
            );
            let reset = function!(
                "csmResetDrawableDynamicFlags",
                unsafe extern "C" fn(*mut c_void)
            );
            let update = function!("csmUpdateModel", unsafe extern "C" fn(*mut c_void));
            let count = function!(
                "csmGetDrawableCount",
                unsafe extern "C" fn(*const c_void) -> i32
            );
            let vertex_counts = function!(
                "csmGetDrawableVertexCounts",
                unsafe extern "C" fn(*const c_void) -> *const i32
            );
            let positions = function!(
                "csmGetDrawableVertexPositions",
                unsafe extern "C" fn(*const c_void) -> *const *const V2
            );
            let uvs = function!(
                "csmGetDrawableVertexUvs",
                unsafe extern "C" fn(*const c_void) -> *const *const V2
            );
            let index_counts = function!(
                "csmGetDrawableIndexCounts",
                unsafe extern "C" fn(*const c_void) -> *const i32
            );
            let indices = function!(
                "csmGetDrawableIndices",
                unsafe extern "C" fn(*const c_void) -> *const *const u16
            );
            let ids = function!(
                "csmGetDrawableIds",
                unsafe extern "C" fn(*const c_void) -> *const *const c_char
            );
            let textures = function!(
                "csmGetDrawableTextureIndices",
                unsafe extern "C" fn(*const c_void) -> *const i32
            );
            let parameter_count = function!(
                "csmGetParameterCount",
                unsafe extern "C" fn(*const c_void) -> i32
            );
            let parameter_ids = function!(
                "csmGetParameterIds",
                unsafe extern "C" fn(*const c_void) -> *const *const c_char
            );

            let abi = version();
            assert!(
                (4..=6).contains(&(abi >> 24)),
                "unexpected official ABI {abi:#x}"
            );
            let mut moc_mem = Aligned::new(bytes.len(), 64).unwrap();
            moc_mem.copy_from(&bytes);
            assert_eq!(
                consistent(moc_mem.ptr(), bytes.len() as u32),
                1,
                "official Core rejected the supplied MOC3"
            );
            let moc = revive(moc_mem.ptr(), bytes.len() as u32);
            assert!(!moc.is_null());
            let size = model_size(moc);
            let model_mem = Aligned::new(size as usize, 16).unwrap();
            let model = initialize(moc, model_mem.ptr(), size);
            assert!(!model.is_null());
            assert_eq!(count(model) as usize, current.drawables.len());
            let layouts = rust.mesh_layouts().unwrap();
            assert_eq!(layouts.len(), current.drawables.len());
            let native_ids = std::slice::from_raw_parts(ids(model), layouts.len());
            let native_uvs = std::slice::from_raw_parts(uvs(model), layouts.len());
            let native_indices = std::slice::from_raw_parts(indices(model), layouts.len());
            let native_vertices = std::slice::from_raw_parts(vertex_counts(model), layouts.len());
            let native_index_counts =
                std::slice::from_raw_parts(index_counts(model), layouts.len());
            let native_textures = std::slice::from_raw_parts(textures(model), layouts.len());
            for (i, layout) in layouts.iter().enumerate() {
                assert_eq!(layout.id, CStr::from_ptr(native_ids[i]).to_str().unwrap());
                assert_eq!(i32::from(layout.texture), native_textures[i]);
                assert_eq!(layout.uvs.len(), native_vertices[i] as usize);
                let uv = std::slice::from_raw_parts(native_uvs[i], layout.uvs.len());
                for (decoded, official) in layout.uvs.iter().zip(uv) {
                    assert_eq!(*decoded, [official.x, official.y]);
                }
                assert_eq!(layout.triangles.len(), native_index_counts[i] as usize);
                assert_eq!(
                    layout.triangles,
                    std::slice::from_raw_parts(native_indices[i], layout.triangles.len())
                );
            }
            let params = rust.parameters().unwrap();
            assert_eq!(params.len(), parameter_count(model) as usize);
            let native_param_ids = std::slice::from_raw_parts(parameter_ids(model), params.len());
            for (spec, &id) in params.iter().zip(native_param_ids) {
                assert_eq!(spec.id, CStr::from_ptr(id).to_str().unwrap());
            }

            let param_values = values(model);
            assert!(!param_values.is_null());
            let params: Vec<_> = current.parameters().iter().take(16).cloned().collect();
            let mut worst = 0.0_f32;
            for frame in 0..5 {
                for (i, p) in params.iter().enumerate() {
                    let t = ((frame * 13 + i * 7) % 17) as f32 / 16.0;
                    let value = p.min + (p.max - p.min) * t;
                    param_values.add(i).write(value);
                    current.set_parameter(&p.id, value);
                }
                reset(model);
                update(model);
                current.update().unwrap();
                let counts =
                    std::slice::from_raw_parts(vertex_counts(model), current.drawables.len());
                let positions =
                    std::slice::from_raw_parts(positions(model), current.drawables.len());
                for (i, drawable) in current.drawables.iter().enumerate() {
                    assert_eq!(counts[i] as usize, drawable.positions.len());
                    let source = std::slice::from_raw_parts(positions[i], drawable.positions.len());
                    for (official, aria) in source.iter().zip(&drawable.positions) {
                        worst = worst
                            .max((official.x - aria[0]).abs())
                            .max((official.y - aria[1]).abs());
                    }
                }
            }
            println!("official ABI {abi:#x}, maximum position difference {worst}");
            assert!(
                worst <= 0.001,
                "current runtime diverges from official Cubism"
            );
        }
    }
}
