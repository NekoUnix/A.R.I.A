//! Developer-only differential test against a locally installed official Core.
//! The DLL and model files are never copied into the repository or release.

#[cfg(test)]
mod tests {
    use crate::rust_model::RustModel;
    use libloading::Library;
    use std::alloc::{Layout, alloc_zeroed, dealloc};
    use std::{
        ffi::{CStr, c_char, c_void},
        ptr::NonNull,
    };

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct V2 {
        x: f32,
        y: f32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct V4 {
        x: f32,
        y: f32,
        z: f32,
        w: f32,
    }

    struct Aligned {
        ptr: NonNull<u8>,
        layout: Layout,
    }
    impl Aligned {
        fn new(size: usize, alignment: usize) -> anyhow::Result<Self> {
            let layout = Layout::from_size_align(size, alignment)?;
            let ptr = NonNull::new(unsafe { alloc_zeroed(layout) })
                .ok_or_else(|| anyhow::anyhow!("Cannot allocate oracle model memory"))?;
            Ok(Self { ptr, layout })
        }
        fn ptr(&self) -> *mut c_void {
            self.ptr.as_ptr().cast()
        }
        fn copy_from(&mut self, bytes: &[u8]) {
            assert!(bytes.len() <= self.layout.size());
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr.as_ptr(), bytes.len());
            }
        }
    }
    impl Drop for Aligned {
        fn drop(&mut self) {
            unsafe {
                dealloc(self.ptr.as_ptr(), self.layout);
            }
        }
    }

    #[test]
    #[ignore = "requires ARIA_CUBISM_CORE and ARIA_TEST_MOC; official DLL stays local"]
    fn official_cubism_matches_rust_on_animated_vertices() {
        let dll = std::env::var_os("ARIA_CUBISM_CORE").expect("ARIA_CUBISM_CORE");
        let path = std::env::var_os("ARIA_TEST_MOC").expect("ARIA_TEST_MOC");
        let bytes = std::fs::read(path).unwrap();
        let rust = aria_model_core::moc::Moc::parse(&bytes).unwrap();
        let evaluator = aria_model_core::geometry::GeometryEvaluator::new(&rust).unwrap();
        let orderer = aria_model_core::draw_order::RenderOrderEvaluator::new(&rust).unwrap();
        let reverse_y = rust.canvas().unwrap().reverse_y;
        let mut rust_adapter = RustModel::load(&bytes, 32).unwrap();

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
            let constant_flags = function!(
                "csmGetDrawableConstantFlags",
                unsafe extern "C" fn(*const c_void) -> *const u8
            );
            let dynamic_flags = function!(
                "csmGetDrawableDynamicFlags",
                unsafe extern "C" fn(*const c_void) -> *const u8
            );
            let drawable_parts = function!(
                "csmGetDrawableParentPartIndices",
                unsafe extern "C" fn(*const c_void) -> *const i32
            );
            let drawable_opacities = function!(
                "csmGetDrawableOpacities",
                unsafe extern "C" fn(*const c_void) -> *const f32
            );
            let drawable_draw_orders = function!(
                "csmGetDrawableDrawOrders",
                unsafe extern "C" fn(*const c_void) -> *const i32
            );
            let drawable_render_orders = function!(
                "csmGetDrawableRenderOrders",
                unsafe extern "C" fn(*const c_void) -> *const i32
            );
            let drawable_multiply = function!(
                "csmGetDrawableMultiplyColors",
                unsafe extern "C" fn(*const c_void) -> *const V4
            );
            let drawable_screen = function!(
                "csmGetDrawableScreenColors",
                unsafe extern "C" fn(*const c_void) -> *const V4
            );
            let part_count = function!(
                "csmGetPartCount",
                unsafe extern "C" fn(*const c_void) -> i32
            );
            let part_ids = function!(
                "csmGetPartIds",
                unsafe extern "C" fn(*const c_void) -> *const *const c_char
            );
            let part_opacities = function!(
                "csmGetPartOpacities",
                unsafe extern "C" fn(*mut c_void) -> *mut f32
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
            assert_eq!(count(model) as usize, rust_adapter.drawables.len());
            let layouts = rust.mesh_layouts().unwrap();
            assert_eq!(layouts.len(), rust_adapter.drawables.len());
            let native_ids = std::slice::from_raw_parts(ids(model), layouts.len());
            let native_uvs = std::slice::from_raw_parts(uvs(model), layouts.len());
            let native_indices = std::slice::from_raw_parts(indices(model), layouts.len());
            let native_vertices = std::slice::from_raw_parts(vertex_counts(model), layouts.len());
            let native_index_counts =
                std::slice::from_raw_parts(index_counts(model), layouts.len());
            let native_textures = std::slice::from_raw_parts(textures(model), layouts.len());
            let native_flags = std::slice::from_raw_parts(constant_flags(model), layouts.len());
            let native_parents = std::slice::from_raw_parts(drawable_parts(model), layouts.len());
            for (i, layout) in layouts.iter().enumerate() {
                assert_eq!(layout.id, CStr::from_ptr(native_ids[i]).to_str().unwrap());
                assert_eq!(i32::from(layout.texture), native_textures[i]);
                assert_eq!(layout.constant_flags, native_flags[i]);
                assert_eq!(
                    layout.parent_part.map(|index| index as i32).unwrap_or(-1),
                    native_parents[i]
                );
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
            let parts = rust.part_layouts().unwrap();
            assert_eq!(parts.len(), part_count(model) as usize);
            let native_part_ids = std::slice::from_raw_parts(part_ids(model), parts.len());
            for (part, &id) in parts.iter().zip(native_part_ids) {
                assert_eq!(part.id, CStr::from_ptr(id).to_str().unwrap());
            }

            let param_values = values(model);
            assert!(!param_values.is_null());
            let params: Vec<_> = rust_adapter.parameters().iter().take(16).cloned().collect();
            let mut rust_worst = 0.0_f32;
            let mut opacity_worst = 0.0_f32;
            let mut color_worst = 0.0_f32;
            let mut draw_order_mismatches = 0_usize;
            let mut render_order_mismatches = 0_usize;
            let mut rust_mesh_frames = 0_usize;
            let mut rust_mismatches = 0_usize;
            for frame in 0..7 {
                if frame < 5 {
                    for (i, p) in params.iter().enumerate() {
                        let t = ((frame * 13 + i * 7) % 17) as f32 / 16.0;
                        let value = p.min + (p.max - p.min) * t;
                        param_values.add(i).write(value);
                        rust_adapter.set_parameter(&p.id, value);
                    }
                } else {
                    for (i, spec) in rust.parameters().unwrap().iter().enumerate() {
                        if spec.kind == aria_model_core::moc::ParameterKind::BlendShape {
                            let value = if frame == 5 {
                                spec.maximum
                            } else {
                                spec.minimum
                            };
                            param_values.add(i).write(value);
                            rust_adapter.set_parameter(&spec.id, value);
                        }
                    }
                }
                let input_parts = parts
                    .iter()
                    .enumerate()
                    .map(|(index, part)| {
                        if !part.visible {
                            0.0
                        } else if frame == 0 {
                            1.0
                        } else {
                            ((frame * 7 + index * 5) % 13) as f32 / 12.0
                        }
                    })
                    .collect::<Vec<_>>();
                for (i, value) in input_parts.iter().enumerate() {
                    part_opacities(model).add(i).write(*value);
                    rust_adapter.parts[i].value = *value;
                }
                reset(model);
                update(model);
                rust_adapter.update().unwrap();
                let counts =
                    std::slice::from_raw_parts(vertex_counts(model), rust_adapter.drawables.len());
                let positions =
                    std::slice::from_raw_parts(positions(model), rust_adapter.drawables.len());
                let native_opacities = std::slice::from_raw_parts(
                    drawable_opacities(model),
                    rust_adapter.drawables.len(),
                );
                let native_draw_orders = std::slice::from_raw_parts(
                    drawable_draw_orders(model),
                    rust_adapter.drawables.len(),
                );
                let native_render_orders = std::slice::from_raw_parts(
                    drawable_render_orders(model),
                    rust_adapter.drawables.len(),
                );
                let native_dynamic =
                    std::slice::from_raw_parts(dynamic_flags(model), rust_adapter.drawables.len());
                let native_multiply = std::slice::from_raw_parts(
                    drawable_multiply(model),
                    rust_adapter.drawables.len(),
                );
                let native_screen = std::slice::from_raw_parts(
                    drawable_screen(model),
                    rust_adapter.drawables.len(),
                );
                let rust_values = rust_adapter
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.value)
                    .collect::<Vec<_>>();
                let rust_frames = evaluator
                    .frame_with_parts(&rust_values, &input_parts)
                    .unwrap();
                let rust_orders = orderer.frame(&rust_values, &rust_frames).unwrap();
                for (index, independent) in rust_adapter.drawables.iter().enumerate() {
                    assert_eq!(independent.visible, native_dynamic[index] & 1 != 0);
                    assert_eq!(independent.order, native_render_orders[index]);
                    if native_render_orders[index] != rust_orders[index] {
                        render_order_mismatches += 1;
                        if render_order_mismatches <= 8 {
                            eprintln!(
                                "render order mesh {index} {} frame {frame}: official {}, Rust {}",
                                independent.id, native_render_orders[index], rust_orders[index]
                            );
                        }
                    }
                }
                for (i, independent) in rust_adapter.drawables.iter().enumerate() {
                    assert_eq!(
                        independent.visible,
                        rust_frames[i]
                            .as_ref()
                            .is_some_and(|frame| frame.opacity != 0.0),
                        "Rust visibility differs for mesh {i} {} at frame {frame}: official dynamic {}, official opacity {}, Rust opacity {:?}, mesh flags {}, enabled {}, parent part {:?}, default visible {}",
                        independent.id,
                        native_dynamic[i],
                        native_opacities[i],
                        rust_frames[i].as_ref().map(|f| f.opacity),
                        layouts[i].constant_flags,
                        layouts[i].enabled,
                        layouts[i].parent_part,
                        layouts[i].default_visible
                    );
                    assert_eq!(counts[i] as usize, independent.positions.len());
                    let source =
                        std::slice::from_raw_parts(positions[i], independent.positions.len());
                    if independent.visible {
                        let rust_frame = rust_frames[i].as_ref().unwrap_or_else(|| {
                            panic!("Visible mesh {i} {} has no Rust frame", independent.id)
                        });
                        rust_mesh_frames += 1;
                        opacity_worst =
                            opacity_worst.max((native_opacities[i] - rust_frame.opacity).abs());
                        let multiply = [
                            native_multiply[i].x,
                            native_multiply[i].y,
                            native_multiply[i].z,
                            native_multiply[i].w,
                        ];
                        let screen = [
                            native_screen[i].x,
                            native_screen[i].y,
                            native_screen[i].z,
                            native_screen[i].w,
                        ];
                        for channel in 0..4 {
                            color_worst = color_worst
                                .max((multiply[channel] - rust_frame.multiply[channel]).abs())
                                .max((screen[channel] - rust_frame.screen[channel]).abs());
                        }
                        if native_draw_orders[i] != rust_frame.integer_draw_order() {
                            draw_order_mismatches += 1;
                            eprintln!(
                                "draw order mesh {i} {} frame {frame}: official {}, rust {}",
                                independent.id, native_draw_orders[i], rust_frame.draw_order
                            );
                        }
                        let mut mesh_difference = 0.0_f32;
                        for (official, aria) in source.iter().zip(&rust_frame.positions) {
                            let y = if reverse_y { aria[1] } else { -aria[1] };
                            mesh_difference = mesh_difference
                                .max((official.x - aria[0]).abs())
                                .max((official.y - y).abs());
                        }
                        rust_worst = rust_worst.max(mesh_difference);
                        if mesh_difference > 0.001 {
                            rust_mismatches += 1;
                            eprintln!(
                                "Rust mismatch frame {frame} mesh {i} {}: {mesh_difference}",
                                independent.id
                            );
                        }
                    }
                }
            }
            println!(
                "official ABI {abi:#x}, Rust delta {rust_worst}, opacity delta {opacity_worst}, color delta {color_worst}, draw-order mismatches {draw_order_mismatches}, render-order mismatches {render_order_mismatches} across {rust_mesh_frames} supported visible mesh frames ({rust_mismatches} mismatched)"
            );
            assert!(
                rust_mesh_frames > 0,
                "No supported Rust geometry was compared"
            );
            assert!(
                rust_worst <= 0.001,
                "Rust geometry diverges from official Cubism"
            );
            assert!(
                opacity_worst <= 0.001,
                "Rust opacity diverges from official Cubism"
            );
            assert!(
                color_worst <= 0.001,
                "Rust drawable colors diverge from official Cubism"
            );
            assert_eq!(
                draw_order_mismatches, 0,
                "Rust ArtMesh draw orders diverge from official Cubism"
            );
            assert_eq!(
                render_order_mismatches, 0,
                "Rust final render orders diverge from official Cubism"
            );
        }
    }
}
