//! Private ABI definitions for the documented Cubism Core C API.
//! No SDK headers or proprietary implementation are included.
use anyhow::{Context, Result, ensure};
use libloading::Library;
use std::{
    alloc::{Layout, alloc_zeroed, dealloc},
    ffi::{c_char, c_void},
    path::Path,
    ptr::NonNull,
};

pub type Model = c_void;
pub type Moc = c_void;
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct V4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

macro_rules! api {
    ($($field:ident : $ty:ty = $symbol:literal),* $(,)?) => {
        pub struct Api {
            $(pub $field: $ty,)*
            pub orders: unsafe extern "system" fn(*const Model) -> *const i32,
            pub offscreens: Option<unsafe extern "system" fn(*const Model) -> i32>,
            pub blend_modes: Option<unsafe extern "system" fn(*const Model) -> *const i32>,
            _library: Library,
        }
        impl Api {
            // The selected library is executable native code. Only load an official SDK DLL.
            pub fn load(path: &Path) -> Result<Self> {
                let path = crate::platform::resolve(path)?;
                // SAFETY: Explicit absolute path; Core's documented ABI is checked by symbol lookup.
                // Windows searches only the DLL's folder and System32 for its dependencies.
                unsafe {
                    #[cfg(windows)]
                    let library: Library = libloading::os::windows::Library::load_with_flags(&path, 0x100 | 0x800)
                        .context("Cannot load Cubism Core. Use the Windows x86_64 DLL, not the x86 DLL.")?.into();
                    #[cfg(not(windows))]
                    let library = Library::new(&path).context("Cannot load Cubism Core")?;
                    Ok(Self {
                        $($field: *library.get::<$ty>(concat!($symbol, "\0").as_bytes()).with_context(|| format!("Missing {}. Install a current Cubism Native SDK.", $symbol))?,)*
                        orders: *library.get(b"csmGetRenderOrders\0").or_else(|_| library.get(b"csmGetDrawableRenderOrders\0")).context("Core has no render-order API")?,
                        offscreens: library.get(b"csmGetOffscreenCount\0").ok().map(|s| *s),
                        blend_modes: library.get(b"csmGetDrawableBlendModes\0").ok().map(|s| *s),
                        _library: library,
                    })
                }
            }
        }
    }
}
api! {
    version: unsafe extern "system" fn() -> u32 = "csmGetVersion",
    latest_moc: unsafe extern "system" fn() -> u32 = "csmGetLatestMocVersion",
    moc_version: unsafe extern "system" fn(*const c_void, u32) -> u32 = "csmGetMocVersion",
    consistent: unsafe extern "system" fn(*const c_void, u32) -> i32 = "csmHasMocConsistency",
    revive: unsafe extern "system" fn(*mut c_void, u32) -> *mut Moc = "csmReviveMocInPlace",
    model_size: unsafe extern "system" fn(*const Moc) -> u32 = "csmGetSizeofModel",
    initialize: unsafe extern "system" fn(*const Moc, *mut c_void, u32) -> *mut Model = "csmInitializeModelInPlace",
    update: unsafe extern "system" fn(*mut Model) = "csmUpdateModel",
    reset: unsafe extern "system" fn(*mut Model) = "csmResetDrawableDynamicFlags",
    canvas: unsafe extern "system" fn(*const Model, *mut V2, *mut V2, *mut f32) = "csmReadCanvasInfo",
    parameter_count: unsafe extern "system" fn(*const Model) -> i32 = "csmGetParameterCount",
    parameter_ids: unsafe extern "system" fn(*const Model) -> *const *const c_char = "csmGetParameterIds",
    minimum: unsafe extern "system" fn(*const Model) -> *const f32 = "csmGetParameterMinimumValues",
    maximum: unsafe extern "system" fn(*const Model) -> *const f32 = "csmGetParameterMaximumValues",
    defaults: unsafe extern "system" fn(*const Model) -> *const f32 = "csmGetParameterDefaultValues",
    values: unsafe extern "system" fn(*mut Model) -> *mut f32 = "csmGetParameterValues",
    drawable_ids: unsafe extern "system" fn(*const Model) -> *const *const c_char = "csmGetDrawableIds",
    part_count: unsafe extern "system" fn(*const Model) -> i32 = "csmGetPartCount",
    part_ids: unsafe extern "system" fn(*const Model) -> *const *const c_char = "csmGetPartIds",
    drawable_parts: unsafe extern "system" fn(*const Model) -> *const i32 = "csmGetDrawableParentPartIndices",
    drawable_count: unsafe extern "system" fn(*const Model) -> i32 = "csmGetDrawableCount",
    flags: unsafe extern "system" fn(*const Model) -> *const u8 = "csmGetDrawableConstantFlags",
    dynamic: unsafe extern "system" fn(*const Model) -> *const u8 = "csmGetDrawableDynamicFlags",
    textures: unsafe extern "system" fn(*const Model) -> *const i32 = "csmGetDrawableTextureIndices",
    opacity: unsafe extern "system" fn(*const Model) -> *const f32 = "csmGetDrawableOpacities",
    mask_counts: unsafe extern "system" fn(*const Model) -> *const i32 = "csmGetDrawableMaskCounts",
    masks: unsafe extern "system" fn(*const Model) -> *const *const i32 = "csmGetDrawableMasks",
    vertex_counts: unsafe extern "system" fn(*const Model) -> *const i32 = "csmGetDrawableVertexCounts",
    positions: unsafe extern "system" fn(*const Model) -> *const *const V2 = "csmGetDrawableVertexPositions",
    uvs: unsafe extern "system" fn(*const Model) -> *const *const V2 = "csmGetDrawableVertexUvs",
    index_counts: unsafe extern "system" fn(*const Model) -> *const i32 = "csmGetDrawableIndexCounts",
    indices: unsafe extern "system" fn(*const Model) -> *const *const u16 = "csmGetDrawableIndices",
    multiply: unsafe extern "system" fn(*const Model) -> *const V4 = "csmGetDrawableMultiplyColors",
    screen: unsafe extern "system" fn(*const Model) -> *const V4 = "csmGetDrawableScreenColors",
}

pub struct Aligned {
    ptr: NonNull<u8>,
    layout: Layout,
}
impl Aligned {
    pub fn new(size: usize, alignment: usize) -> Result<Self> {
        ensure!(
            size > 0 && size as u64 <= aria_core::asset_limits::CORE_ALLOCATION,
            "Invalid or excessive Core allocation ({size} bytes)"
        );
        let layout = Layout::from_size_align(size, alignment)?;
        // SAFETY: Nonzero checked layout; this allocation is freed with the same layout.
        let ptr = NonNull::new(unsafe { alloc_zeroed(layout) })
            .context("Cannot allocate Cubism model memory")?;
        Ok(Self { ptr, layout })
    }
    pub fn ptr(&self) -> *mut c_void {
        self.ptr.as_ptr().cast()
    }
    pub fn copy_from(&mut self, bytes: &[u8]) {
        assert!(bytes.len() <= self.layout.size());
        // SAFETY: The allocation is exclusively owned and large enough, source is disjoint.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.ptr.as_ptr(), bytes.len());
        }
    }
}
impl Drop for Aligned {
    fn drop(&mut self) {
        // SAFETY: We own the live allocation and its exact original layout.
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

/// Caller must establish that ptr refers to count initialized elements owned by a live Core model.
pub unsafe fn array<'a, T>(ptr: *const T, count: usize) -> Result<&'a [T]> {
    if count == 0 {
        return Ok(&[]);
    }
    ensure!(
        !ptr.is_null() && (ptr as usize).is_multiple_of(std::mem::align_of::<T>()),
        "Invalid Core array pointer"
    );
    // SAFETY: Caller establishes the allocation/lifetime; nonzero null/alignment checked here.
    Ok(unsafe { std::slice::from_raw_parts(ptr, count) })
}

pub fn count(value: i32, max: usize) -> Result<usize> {
    ensure!(
        value >= 0 && value as usize <= max,
        "Invalid/excessive Core count: {value}"
    );
    Ok(value as usize)
}
