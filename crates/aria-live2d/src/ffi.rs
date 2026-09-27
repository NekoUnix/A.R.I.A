//! C ABI declarations for the vendored MIT-licensed Purism Core v6 API.
//! Signatures follow vendor/purism-core/PurismCoreBundle.h.
use anyhow::{Context, Result, ensure};
use std::{
    alloc::{Layout, alloc_zeroed, dealloc},
    ffi::{c_char, c_void},
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

// Static C linkage to the vendored MIT implementation; never load a user DLL.
macro_rules! api {
    ($($field:ident : unsafe extern "C" fn($($arg:ident: $arg_ty:ty),*) $(-> $ret:ty)? = $symbol:ident),* $(,)?) => {
        unsafe extern "C" {
            $(fn $symbol($($arg: $arg_ty),*) $(-> $ret)?;)*
            fn csmGetRenderOrders(model: *const Model) -> *const i32;
            fn csmGetOffscreenCount(model: *const Model) -> i32;
            fn csmGetDrawableBlendModes(model: *const Model) -> *const i32;
        }
        pub struct Api {
            $(pub $field: unsafe extern "C" fn($($arg_ty),*) $(-> $ret)?,)*
            pub orders: unsafe extern "C" fn(a0: *const Model) -> *const i32,
            pub offscreens: Option<unsafe extern "C" fn(a0: *const Model) -> i32>,
            pub blend_modes: Option<unsafe extern "C" fn(a0: *const Model) -> *const i32>,
        }
        impl Api {
            pub fn bundled() -> Self {
                Self {
                    $($field: $symbol,)*
                    orders: csmGetRenderOrders,
                    offscreens: Some(csmGetOffscreenCount),
                    blend_modes: Some(csmGetDrawableBlendModes),
                }
            }
        }
    }
}
api! {
    version: unsafe extern "C" fn() -> u32 = csmGetVersion,
    latest_moc: unsafe extern "C" fn() -> u32 = csmGetLatestMocVersion,
    moc_version: unsafe extern "C" fn(a0: *const c_void, a1: u32) -> u32 = csmGetMocVersion,
    consistent: unsafe extern "C" fn(a0: *const c_void, a1: u32) -> i32 = csmHasMocConsistency,
    revive: unsafe extern "C" fn(a0: *mut c_void, a1: u32) -> *mut Moc = csmReviveMocInPlace,
    model_size: unsafe extern "C" fn(a0: *const Moc) -> u32 = csmGetSizeofModel,
    initialize: unsafe extern "C" fn(a0: *const Moc, a1: *mut c_void, a2: u32) -> *mut Model = csmInitializeModelInPlace,
    update: unsafe extern "C" fn(a0: *mut Model) = csmUpdateModel,
    reset: unsafe extern "C" fn(a0: *mut Model) = csmResetDrawableDynamicFlags,
    canvas: unsafe extern "C" fn(a0: *const Model, a1: *mut V2, a2: *mut V2, a3: *mut f32) = csmReadCanvasInfo,
    parameter_count: unsafe extern "C" fn(a0: *const Model) -> i32 = csmGetParameterCount,
    parameter_ids: unsafe extern "C" fn(a0: *const Model) -> *const *const c_char = csmGetParameterIds,
    minimum: unsafe extern "C" fn(a0: *const Model) -> *const f32 = csmGetParameterMinimumValues,
    maximum: unsafe extern "C" fn(a0: *const Model) -> *const f32 = csmGetParameterMaximumValues,
    defaults: unsafe extern "C" fn(a0: *const Model) -> *const f32 = csmGetParameterDefaultValues,
    values: unsafe extern "C" fn(a0: *mut Model) -> *mut f32 = csmGetParameterValues,
    drawable_ids: unsafe extern "C" fn(a0: *const Model) -> *const *const c_char = csmGetDrawableIds,
    part_count: unsafe extern "C" fn(a0: *const Model) -> i32 = csmGetPartCount,
    part_ids: unsafe extern "C" fn(a0: *const Model) -> *const *const c_char = csmGetPartIds,
    part_opacities: unsafe extern "C" fn(a0: *mut Model) -> *mut f32 = csmGetPartOpacities,
    drawable_parts: unsafe extern "C" fn(a0: *const Model) -> *const i32 = csmGetDrawableParentPartIndices,
    drawable_count: unsafe extern "C" fn(a0: *const Model) -> i32 = csmGetDrawableCount,
    flags: unsafe extern "C" fn(a0: *const Model) -> *const u8 = csmGetDrawableConstantFlags,
    dynamic: unsafe extern "C" fn(a0: *const Model) -> *const u8 = csmGetDrawableDynamicFlags,
    textures: unsafe extern "C" fn(a0: *const Model) -> *const i32 = csmGetDrawableTextureIndices,
    opacity: unsafe extern "C" fn(a0: *const Model) -> *const f32 = csmGetDrawableOpacities,
    mask_counts: unsafe extern "C" fn(a0: *const Model) -> *const i32 = csmGetDrawableMaskCounts,
    masks: unsafe extern "C" fn(a0: *const Model) -> *const *const i32 = csmGetDrawableMasks,
    vertex_counts: unsafe extern "C" fn(a0: *const Model) -> *const i32 = csmGetDrawableVertexCounts,
    positions: unsafe extern "C" fn(a0: *const Model) -> *const *const V2 = csmGetDrawableVertexPositions,
    uvs: unsafe extern "C" fn(a0: *const Model) -> *const *const V2 = csmGetDrawableVertexUvs,
    index_counts: unsafe extern "C" fn(a0: *const Model) -> *const i32 = csmGetDrawableIndexCounts,
    indices: unsafe extern "C" fn(a0: *const Model) -> *const *const u16 = csmGetDrawableIndices,
    multiply: unsafe extern "C" fn(a0: *const Model) -> *const V4 = csmGetDrawableMultiplyColors,
    screen: unsafe extern "C" fn(a0: *const Model) -> *const V4 = csmGetDrawableScreenColors,
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
