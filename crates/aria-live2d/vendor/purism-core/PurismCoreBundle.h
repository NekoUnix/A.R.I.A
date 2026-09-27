/*
 * PurismCoreBundle.h - single-file (amalgamated) Purism Core.
 *
 * Usage:
 *   #include "PurismCoreBundle.h"
 * In exactly ONE translation unit, define the implementation first:
 *   #define PURISM_CORE_IMPLEMENTATION
 *   #include "PurismCoreBundle.h"
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PURISM_CORE_BUNDLE_H
#define PURISM_CORE_BUNDLE_H

/* Public API */
/* ===== ../include/PurismCore.h ===== */
/*
 * Purism Core: public API
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PURISM_CORE_H
#define PURISM_CORE_H

#ifdef __cplusplus
extern "C" {
#endif

/* PSM_COMPAT_VERSION is the version reported by csmGetVersion for
   compatibility and determines what public API functions are available. */
#ifndef PSM_COMPAT_VERSION
#  define PSM_COMPAT_VERSION 0x06000001L
#endif

#if PSM_COMPAT_VERSION != 0x06000001L && PSM_COMPAT_VERSION != 0x05010000L
#  error Unsupported Cubism compatibility level
#endif

/* PSM_TRUE_VERSION is the actual Purism Core implementation version. */
#define PSM_TRUE_VERSION 0x01010000L

/* CSM_CORE_WIN32_DLL is an alias for PURISM_CORE_DLL. */
#ifdef CSM_CORE_WIN32_DLL
#  define PURISM_CORE_DLL
#endif

/* PSMDEF specifies the linkage and attributes of public API functions. */
#ifndef PSMDEF
#  if defined(__EMSCRIPTEN__)
#    include <emscripten.h>
#    define PSMDEF EMSCRIPTEN_KEEPALIVE // avoid DCE
#  elif defined(PURISM_CORE_STATIC)
#    define PSMDEF static
#  elif defined(_WIN32) && defined(PURISM_CORE_DLL)
#    define PSMDEF __declspec(dllexport) __stdcall
#  else
#    define PSMDEF
#  endif
#endif

/* PSM_HAS_STDINT determines whether C99 <stdint.h> is available. */
#ifdef PSM_HAS_STDINT
#  if PSM_HAS_STDINT
#    if defined(PSM_STDINT_HEADER)
#      include PSM_STDINT_HEADER
#    else
#      include <stdint.h>
#    endif
#  endif
#elif defined(__has_include)
#  if __has_include(<stdint.h>)
#    include <stdint.h>
#    define PSM_HAS_STDINT 1
#  endif
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 199901L
#  include <stdint.h>
#  define PSM_HAS_STDINT 1
#elif defined(_MSC_VER) && _MSC_VER >= 1600
#  include <stdint.h>
#  define PSM_HAS_STDINT 1
#elif defined(__cplusplus) && __cplusplus >= 201103L
#  include <cstdint>
#  define PSM_HAS_STDINT 1
#endif

#ifndef PSM_HAS_STDINT
#  define PSM_HAS_STDINT 0
#endif

#if defined(PSM_HAS_STDINT) && PSM_HAS_STDINT
typedef int8_t   psm__int8;
typedef uint8_t  psm__u8;
typedef int16_t  psm__i16;
typedef uint16_t psm__u16;
typedef int32_t  psm__i32;
typedef uint32_t psm__u32;
#else
typedef signed char    psm__int8;
typedef unsigned char  psm__u8;
typedef signed short   psm__i16;
typedef unsigned short psm__u16;
typedef signed int     psm__i32;
typedef unsigned int   psm__u32;
#endif

typedef float    psm__f32;
typedef psm__u32 psm_size;

#ifndef psm__static_assert
#  if defined(PSM_HAS_STATIC_ASSERT) && PSM_HAS_STATIC_ASSERT
#    define psm__static_assert(cond, msg) _Static_assert(cond, msg)
#  elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
#    define psm__static_assert(cond, msg) _Static_assert(cond, msg)
#  elif defined(__cplusplus) && __cplusplus >= 201103L
#    define psm__static_assert(cond, msg) static_assert(cond, msg)
#  else
#    ifndef PSM__JOIN
#      define PSM__JOIN_(a, b) a##b
#      define PSM__JOIN(a, b)  PSM__JOIN_(a, b)
#    endif
#    define psm__static_assert(cond, msg) \
      typedef char PSM__JOIN(psm__static_assertion_, __LINE__)[(cond) ? 1 : -1]
#  endif
#endif

/* csmMoc is an opaque handle to a revived MOC3 file. */
typedef struct csmMoc csmMoc;

/* csmModel is an opaque handle to a model instance created from a csmMoc. */
typedef struct csmModel csmModel;

/* csmVersion is a version number. */
typedef psm__u32 csmVersion;

/* csmAlignofMoc is the required alignment for MOC memory (64 bytes).
   csmAlignofModel is the required alignment for model memory (16 bytes). */
enum {
  csmAlignofMoc = 64,
  csmAlignofModel = 16
};

/* csmFlags holds bit flags for drawable properties. */
typedef psm__u8 csmFlags;

/* Constant flags for drawables, obtained via csmGetDrawableConstantFlags.
   csmBlendAdditive and csmBlendMultiplicative are mutually exclusive. */
enum {
  csmBlendAdditive = 1 << 0,       // use additive blending
  csmBlendMultiplicative = 1 << 1, // use multiplicative blending
  csmIsDoubleSided = 1 << 2,       // disable backface culling
  csmIsInvertedMask = 1 << 3       // invert the clipping mask
};

/* Dynamic flags for drawables, obtained via csmGetDrawableDynamicFlags.
   These indicate what changed since the last csmResetDrawableDynamicFlags
   call. */
enum {
  csmIsVisible = 1 << 0,
  csmVisibilityDidChange = 1 << 1,
  csmOpacityDidChange = 1 << 2,
  csmDrawOrderDidChange = 1 << 3,
  csmRenderOrderDidChange = 1 << 4,
  csmVertexPositionsDidChange = 1 << 5,
  csmBlendColorDidChange = 1 << 6
};

#if PSM_COMPAT_VERSION >= 0x06000000L || defined(PSM__BLENDTYPE_V6)
/* Color blend mode values returned by csmGetDrawableBlendModes and
   csmGetOffscreenBlendModes. These define how color channels are combined. */
enum {
  csmColorBlendType_Normal = 0,
  csmColorBlendType_AddCompatible = 1,
  csmColorBlendType_MultiplyCompatible = 2,
  csmColorBlendType_Add = 3,
  csmColorBlendType_AddGlow = 4,
  csmColorBlendType_Darken = 5,
  csmColorBlendType_Multiply = 6,
  csmColorBlendType_ColorBurn = 7,
  csmColorBlendType_LinearBurn = 8,
  csmColorBlendType_Lighten = 9,
  csmColorBlendType_Screen = 10,
  csmColorBlendType_ColorDodge = 11,
  csmColorBlendType_Overlay = 12,
  csmColorBlendType_SoftLight = 13,
  csmColorBlendType_HardLight = 14,
  csmColorBlendType_LinearLight = 15,
  csmColorBlendType_Hue = 16,
  csmColorBlendType_Color = 17
};

/* Alpha blend mode values. These define how alpha channels are combined. */
enum {
  csmAlphaBlendType_Over = 0,
  csmAlphaBlendType_Atop = 1,
  csmAlphaBlendType_Out = 2,
  csmAlphaBlendType_ConjointOver = 3,
  csmAlphaBlendType_DisjointOver = 4
};
#else
/* v5 blend types: Additive and Multiplicative only (no extended blend modes).
   AddCompatible maps to Add, MultiplyCompatible maps to Multiply. */
enum {
  csmColorBlendType_Normal = 0,
  csmColorBlendType_Add = 1,
  csmColorBlendType_Multiply = 2,
  csmColorBlendType_AddCompatible = csmColorBlendType_Add,
  csmColorBlendType_MultiplyCompatible = csmColorBlendType_Multiply
};
#endif

/* csmMocVersion identifies the MOC3 file format version. */
typedef psm__u32 csmMocVersion;

/* MOC3 file format versions returned by csmGetMocVersion. */
enum {
  csmMocVersion_Unknown = 0,
  csmMocVersion_30 = 1,  /* 3.0.00 - 3.2.07 */
  csmMocVersion_33 = 2,  /* 3.3.00 - 3.3.03 */
  csmMocVersion_40 = 3,  /* 4.0.00 - 4.1.05 */
  csmMocVersion_42 = 4,  /* 4.2.00 - 4.2.04 */
  csmMocVersion_50 = 5,  /* 5.0.00 - 5.2.03 */
  csmMocVersion_53 = 6   /* 5.3.00+ */
};

/* Error codes returned by csmGetLastError. */
typedef psm__i32 csmError;
enum {
  csmError_NoError = 0,
  csmError_Failed = 1,
  csmError_ParameterRange = 2,
  csmError_FileUnrecognized = 3,
  csmError_FileCorrupt = 4,
  csmError_InvalidData = 5,
  csmError_InvalidParameter = 6
};

/* csmParameterType distinguishes normal parameters from blend shape
   parameters. */
typedef psm__i32 csmParameterType;

/* Parameter types returned by csmGetParameterTypes. */
enum {
  csmParameterType_Normal = 0,
  csmParameterType_BlendShape = 1
};

/* csmVector2 is a 2D vector with X and Y components. */
typedef struct csmVector2 {
  float X, Y;
} csmVector2;

/* csmVector4 is a 4D vector used for colors (X=R, Y=G, Z=B, W=A). */
typedef struct csmVector4 {
  float X, Y, Z, W;
} csmVector4;

/* csmLogFunction is a callback for receiving log messages from the library. */
typedef void (*csmLogFunction)(const char *message);

/*
 * Version and logging
 */
PSMDEF csmVersion     csmGetVersion(void);
PSMDEF csmVersion     csmGetTrueVersion(void);
PSMDEF const char    *csmGetExtendedVersionString(void);
PSMDEF csmMocVersion  csmGetLatestMocVersion(void);
PSMDEF csmMocVersion  csmGetMocVersion(const void *, unsigned int);
PSMDEF int            csmHasMocConsistency(void *, unsigned int);
PSMDEF csmLogFunction csmGetLogFunction(void);
PSMDEF void           csmSetLogFunction(csmLogFunction);
PSMDEF int            csmGetLogLevel(void);
PSMDEF void           csmSetLogLevel(int);

/*
 * Lifecycle
 *
 * csmReviveMocInPlace: address must be 64-byte aligned (csmAlignofMoc).
 * Memory is modified in place and must outlive all model instances.
 *
 * csmInitializeModelInPlace: address must be 16-byte aligned
 * (csmAlignofModel). The MOC must outlive all its model instances.
 *
 * csmUpdateModel: recalculates vertices after parameter/opacity changes.
 * Call csmResetDrawableDynamicFlags first to track what changed.
 */
PSMDEF csmMoc      *csmReviveMocInPlace(void *, unsigned int);
PSMDEF unsigned int csmGetSizeofModel(const csmMoc *);
PSMDEF csmModel    *csmInitializeModelInPlace(const csmMoc *,
       void *, unsigned int);
PSMDEF void         csmUpdateModel(csmModel *);
PSMDEF void         csmReadCanvasInfo(const csmModel *,
            csmVector2 *, csmVector2 *, float *);

/*
 * Error reporting
 *
 * csmGetMocError and csmGetLastError return the outcome of the most recent
 * csmReviveMocInPlace or csmInitializeModelInPlace on the MOC.
 */
PSMDEF csmError    csmGetMocError(const csmMoc *);
PSMDEF csmError    csmGetLastError(const csmModel *);
PSMDEF const char *csmGetErrorString(csmError);

/*
 * Parameters
 *
 * csmGetParameterValues returns a writable array. Modify then call
 * csmUpdateModel. Values are clamped to [min, max] unless repeat is set.
 */
PSMDEF int                     csmGetParameterCount(const csmModel *);
PSMDEF const char            **csmGetParameterIds(const csmModel *);
PSMDEF const csmParameterType *csmGetParameterTypes(const csmModel *);
PSMDEF const float            *csmGetParameterMinimumValues(const csmModel *);
PSMDEF const float            *csmGetParameterMaximumValues(const csmModel *);
PSMDEF const float            *csmGetParameterDefaultValues(const csmModel *);
PSMDEF float                  *csmGetParameterValues(csmModel *);
PSMDEF const int              *csmGetParameterKeyCounts(const csmModel *);
PSMDEF const float           **csmGetParameterKeyValues(const csmModel *);
#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF const int *csmGetParameterRepeats(const csmModel *);
#endif

/*
 * Parts
 *
 * csmGetPartOpacities returns a writable array. Values are clamped
 * to [0, 1]. Part opacity affects all drawables in that part.
 * Parent index of -1 means no parent.
 */
PSMDEF int          csmGetPartCount(const csmModel *);
PSMDEF const char **csmGetPartIds(const csmModel *);
PSMDEF float       *csmGetPartOpacities(csmModel *);
PSMDEF const int   *csmGetPartParentPartIndices(const csmModel *);
#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF const int *csmGetPartOffscreenIndices(const csmModel *);
#endif

/*
 * Drawables (art meshes)
 *
 * Constant flags: csmBlendAdditive, csmBlendMultiplicative,
 * csmIsDoubleSided, csmIsInvertedMask.
 *
 * Dynamic flags: csmIsVisible, csmVisibilityDidChange, etc.
 * Call csmResetDrawableDynamicFlags before csmUpdateModel to track.
 *
 * Vertex pos are in model space, origin at bottom-left.
 * Triangle indices use counter-clockwise winding.
 * Mask index of -1 means the mask drawable is hidden.
 */
PSMDEF int                    csmGetDrawableCount(const csmModel *);
PSMDEF const char           **csmGetDrawableIds(const csmModel *);
PSMDEF const csmFlags        *csmGetDrawableConstantFlags(const csmModel *);
PSMDEF const csmFlags        *csmGetDrawableDynamicFlags(const csmModel *);
PSMDEF const int             *csmGetDrawableTextureIndices(const csmModel *);
PSMDEF const int             *csmGetDrawableDrawOrders(const csmModel *);
PSMDEF const float           *csmGetDrawableOpacities(const csmModel *);
PSMDEF const int             *csmGetDrawableMaskCounts(const csmModel *);
PSMDEF const int            **csmGetDrawableMasks(const csmModel *);
PSMDEF const int             *csmGetDrawableVertexCounts(const csmModel *);
PSMDEF const csmVector2     **csmGetDrawableVertexPositions(const csmModel *);
PSMDEF const csmVector2     **csmGetDrawableVertexUvs(const csmModel *);
PSMDEF const int             *csmGetDrawableIndexCounts(const csmModel *);
PSMDEF const unsigned short **csmGetDrawableIndices(const csmModel *);
PSMDEF const csmVector4      *csmGetDrawableMultiplyColors(const csmModel *);
PSMDEF const csmVector4      *csmGetDrawableScreenColors(const csmModel *);
PSMDEF const int             *csmGetDrawableParentPartIndices(const csmModel *);
PSMDEF void                   csmResetDrawableDynamicFlags(csmModel *);
#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF const int *csmGetDrawableBlendModes(const csmModel *);
PSMDEF const int *csmGetRenderOrders(const csmModel *);
#else
PSMDEF const int *csmGetDrawableRenderOrders(const csmModel *);
#endif

#if PSM_COMPAT_VERSION >= 0x06000000L
/*
 * Offscreen surfaces (MOC3 v6+)
 *
 * Render-to-texture surfaces for advanced effects.
 * Owner index maps each offscreen to its parent part.
 */
PSMDEF int               csmGetOffscreenCount(const csmModel *);
PSMDEF const int        *csmGetOffscreenBlendModes(const csmModel *);
PSMDEF const float      *csmGetOffscreenOpacities(const csmModel *);
PSMDEF const int        *csmGetOffscreenOwnerIndices(const csmModel *);
PSMDEF const csmVector4 *csmGetOffscreenMultiplyColors(const csmModel *);
PSMDEF const csmVector4 *csmGetOffscreenScreenColors(const csmModel *);
PSMDEF const int        *csmGetOffscreenMaskCounts(const csmModel *);
PSMDEF const int       **csmGetOffscreenMasks(const csmModel *);
PSMDEF const csmFlags   *csmGetOffscreenConstantFlags(const csmModel *);
#endif

#ifdef __cplusplus
}
#endif

#endif

#ifdef PURISM_CORE_IMPLEMENTATION

/* Internal headers */
/* ===== private.h ===== */
/*
 * Purism Core: internal definitions and macros
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__PRIVATE_H
#define PSM__PRIVATE_H

/* Internal code always needs the full v6 blend type set. */
#ifndef PSM__BLENDTYPE_V6
#  define PSM__BLENDTYPE_V6
#endif

#include <stddef.h>

#if defined(__cplusplus)
  /* C++ has bool natively */
#elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 199901L
#  include <stdbool.h>
#elif defined(_MSC_VER) && _MSC_VER >= 1800
#  include <stdbool.h>
#else
typedef unsigned char bool;
#  define true  1
#  define false 0
#endif

#ifndef PSM__DEF
#  if defined(PURISM_CORE_STATIC)
#    define PSM__DEF static
#  elif defined(__GNUC__) || defined(__clang__)
#    define PSM__DEF __attribute__((visibility("hidden")))
#  elif defined(_MSC_VER)
#    define PSM__DEF
#  else
#    define PSM__DEF
#  endif
#endif

enum {
  PSM__FLAG_IS_VISIBLE = 0x01,
  PSM__FLAG_VISIBILITY_CHANGED = 0x02,
  PSM__FLAG_OPACITY_CHANGED = 0x04,
  PSM__FLAG_DRAW_ORDER_CHANGED = 0x08,
  PSM__FLAG_RENDER_ORDER_CHANGED = 0x10,
  PSM__FLAG_VERTEX_CHANGED = 0x20,
  PSM__FLAG_BLEND_COLOR_CHANGED = 0x40,
  PSM__FLAG_ALL_CHANGED = 0x7E,
  PSM__FLAG_ALL = 0x7F,
};

enum {
  PSM__CANVAS_FLAG_Y_REVERSED = 0x01,
};

/* The version constants are `long` (e.g. 0x06000001L); cast the extracted
   components to int so they match PSM__VERFMT's %d. */
#define PSM__VERFMT    "%d.%d.%d"
#define PSM__VERARG(x) (int)((x) >> 24), (int)(((x) >> 16) & 0xFF), (int)((x) & 0xFFFF)

static inline psm__i32
psm__clamp_i32(psm__i32 v, psm__i32 lo, psm__i32 hi)
{
  if (v < lo) return lo;
  if (v > hi) return hi;
  return v;
}

/*
 * Pointer-to-float size ratio. Position keydata stores pointers
 * into the keyform position pool, but the arena is sized in
 * floats. On 64-bit platforms this is 2 (one pointer = two floats).
 */
#define PSM__PTR_FLOAT_RATIO (sizeof(void *) / sizeof(psm__f32))
#define PSM__MAX_KEY_TABLES  20

static inline psm__u32
psm__align_to_16(psm__u32 n)
{
  return (n + 15) & ~15u;
}

#define psm__nop_predicate(...)

#endif /* PSM__PRIVATE_H */
/* ===== error.h ===== */
/*
 * Purism Core: error handling
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__ERROR_H
#define PSM__ERROR_H


// clang-format off
#define PSM__FAILM(cond, v, msg) \
  if (cond) { PSM__LOG(msg); return v; }
#define PSM__FAIL(cond, v, fmt, ...) \
  if (cond) { PSM__LOGF(fmt, __VA_ARGS__); return v; }
// clang-format on

enum {
  PSM__OK,
  PSM__FAILED,
  PSM__ERR_PARAMETER_RANGE_ERROR,
  PSM__ERR_FILE_UNRECOGNIZED,
  PSM__ERR_FILE_CORRUPT,
  PSM__ERR_INVALID_DATA,
  PSM__ERR_INVALID_PARAMETER,
  PSM__ERR_MAX,
};

#endif /* PSM__ERROR_H */
/* ===== debug.h ===== */
/*
 * Purism Core: logging declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__DEBUG_H
#define PSM__DEBUG_H


typedef int psm__log_level;

enum {
  PSM__LOG_VERBOSE,
  PSM__LOG_DEBUG,
  PSM__LOG_INFO,
  PSM__LOG_WARN,
  PSM__LOG_ERR,
  PSM__LOG_OFF,
};

#define PSM__LOG_PREFIX "[PSM] "

#define PSM__LOG_PREFIX_DEBUG PSM__LOG_PREFIX "D: "
#define PSM__LOG_PREFIX_INFO  PSM__LOG_PREFIX "I: "
#define PSM__LOG_PREFIX_WARN  PSM__LOG_PREFIX "W: "
#define PSM__LOG_PREFIX_ERR   PSM__LOG_PREFIX "E: "

#define PSM__LOG(msg) \
  psm__debug_print(PSM__LOG_ERR, PSM__LOG_PREFIX_ERR msg "\n")
#define PSM__LOGF(fmt, ...) \
  psm__debug_print(PSM__LOG_ERR, PSM__LOG_PREFIX_ERR fmt "\n", __VA_ARGS__)
#define PSM__WARN(msg) \
  psm__debug_print(PSM__LOG_WARN, PSM__LOG_PREFIX_WARN msg "\n")
#define PSM__WARNF(fmt, ...) \
  psm__debug_print(PSM__LOG_WARN, PSM__LOG_PREFIX_WARN fmt "\n", __VA_ARGS__)
#define PSM__INFO(msg) \
  psm__debug_print(PSM__LOG_INFO, PSM__LOG_PREFIX_INFO msg "\n")
#define PSM__INFOF(fmt, ...) \
  psm__debug_print(PSM__LOG_INFO, PSM__LOG_PREFIX_INFO fmt "\n", __VA_ARGS__)
#define PSM__DBG(msg) \
  psm__debug_print(PSM__LOG_DEBUG, PSM__LOG_PREFIX_DEBUG msg "\n")
#define PSM__DBGF(fmt, ...) \
  psm__debug_print(PSM__LOG_DEBUG, PSM__LOG_PREFIX_DEBUG fmt "\n", __VA_ARGS__)

PSM__DEF psm__log_level psm__get_log_level(void);
PSM__DEF void           psm__set_log_level(psm__log_level level);
PSM__DEF void           psm__debug_print(int level, const char *format, ...);

#endif /* PSM__DEBUG_H */
/* ===== arena.h ===== */
/*
 * Purism Core: arena allocator
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__ARENA_H
#define PSM__ARENA_H


struct psm__arena {
  psm__u8 *base;
  psm__u32 off;
  psm__u32 cap;
  bool     overflow;
};

#define PSM__ARENA_INIT(buf, cap) \
  ((struct psm__arena){ (psm__u8 *)(buf), 0, (cap), 0 })

static inline psm__u32
psm__arena_safe_mul(struct psm__arena *a, psm__u32 x, psm__u32 y)
{
  psm__u32 r = x * y;
  if (x != 0 && r / x != y) {
    a->overflow = true;
    return 0;
  }
  return r;
}

#define PSM__ARENA_NEW(a, T, n) \
  ((T *)psm__arena_alloc((a), psm__arena_safe_mul((a), sizeof(T), (n))))
#define PSM__ARENA_NEW_SIZE(a, T, sz) \
  ((T *)psm__arena_alloc((a), (sz)))

PSM__DEF void    *psm__arena_alloc(struct psm__arena *, psm__u32);
PSM__DEF psm__u32 psm__arena_total(const struct psm__arena *);
PSM__DEF bool     psm__arena_ok(const struct psm__arena *);
#ifdef PSM_DEBUG_MALLOC
PSM__DEF void psm__dbg_free_all(void);
#endif

#endif /* PSM__ARENA_H */
/* ===== array.h ===== */
/*
 * Purism Core: bounds-checking array macros
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__ARRAY_H
#define PSM__ARRAY_H


static inline int
psm__array_check_index(psm_size i, psm_size n)
{
#ifdef PSM_FAST_AND_DANGEROUS
  (void)i;
  (void)n;
  return PSM__OK;
#else
  return (i < n) ? PSM__OK : PSM__ERR_PARAMETER_RANGE_ERROR;
#endif
}

/*
 * psm__check_idx returns 1 if idx is in [0, max), 0 otherwise.
 * When PSM_FAST_AND_DANGEROUS is defined, always returns 1.
 */
#define psm__check_idx(idx, max) \
  (psm__array_check_index((psm_size)(idx), (max)) == PSM__OK)

/*
 * psm__check_offset_range checks if [offset, offset+count) is within [0, max).
 * Returns 1 if valid, 0 otherwise. Uses unsigned arithmetic to detect overflow.
 * When PSM_FAST_AND_DANGEROUS is defined, always returns 1.
 */
#ifdef PSM_FAST_AND_DANGEROUS
#  define psm__check_offset_range(offset, count, max) (1)
#else
#  define psm__check_offset_range(offset, count, max) \
    ((offset) >= 0 && (count) >= 0 && \
        (psm__u32)(max) - (psm__u32)(offset) >= (psm__u32)(count))
#endif

/*
 * Index relationship validators.
 *
 * psm__valid_idx: required index, must be in [0, max).
 * psm__valid_opt_idx: optional index, -1 means "none",
 *   otherwise [0, max). Returns: 1=valid, 0=none, -1=bad.
 * psm__valid_range: required range [begin, begin+count)
 *   must fit in [0, max).
 * psm__valid_opt_range: optional range, begin<0 with
 *   count==0 means "none". Returns: 1=valid, 0=none, -1=bad.
 */
#ifdef PSM_FAST_AND_DANGEROUS

#  define psm__valid_idx(idx, max)            1
#  define psm__valid_opt_idx(idx, max)        ((idx) >= 0 ? 1 : 0)
#  define psm__valid_range(begin, count, max) 1
#  define psm__valid_opt_range(begin, count, max) \
    ((begin) >= 0 ? 1 : 0)

#else

static inline bool
psm__valid_idx(psm__i32 idx, psm__i32 max)
{
  return (psm__u32)idx < (psm__u32)max;
}

static inline int
psm__valid_opt_idx(psm__i32 idx, psm__i32 max)
{
  if (idx < 0) return 0;
  return (psm__u32)idx < (psm__u32)max ? 1 : -1;
}

static inline bool
psm__valid_range(psm__i32 begin, psm__i32 count, psm__i32 max)
{
  /*
   * begin <= max is required: without it, (u32)max - (u32)begin underflows
   * to a huge value when begin > max and the range check wrongly passes.
   */
  return begin >= 0 && count >= 0 && begin <= max &&
         (psm__u32)max - (psm__u32)begin >= (psm__u32)count;
}

static inline int
psm__valid_opt_range(psm__i32 begin, psm__i32 count, psm__i32 max)
{
  if (begin < 0) return count == 0 ? 0 : -1;
  return psm__valid_range(begin, count, max) ? 1 : -1;
}

#endif

/*
 * psm__clamp_idx clamps idx to [0, max-1] for defensive access.
 * Returns 0 if max <= 0.
 */
static inline psm__i32
psm__clamp_idx(psm__i32 idx, psm__i32 max)
{
  if (max <= 0) return 0;
  return psm__clamp_i32(idx, 0, max - 1);
}

/*
 * psm__safe_order_level computes max_do - min_do + 1 safely, checking for:
 * - max_do < min_do (invalid range)
 * - integer overflow in the subtraction
 * Returns 0 for invalid input, otherwise the positive order level.
 */
static inline psm__i32
psm__safe_order_level(psm__i32 max_do, psm__i32 min_do)
{
  if (max_do < min_do)
    return 0;
  /*
   * Use unsigned arithmetic to safely compute range and detect overflow.
   * This handles all cases including max_do=INT32_MAX with min_do>=0.
   */
  psm__u32 range = (psm__u32)max_do - (psm__u32)min_do;
  if (range > 0x7FFFFFFE)  /* range + 1 would exceed INT32_MAX */
    return 0;
  return (psm__i32)(range + 1);
}

#endif /* PSM__ARRAY_H */
/* ===== math2.h ===== */
/*
 * Purism Core: math utilities
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__MATH2_H
#define PSM__MATH2_H

#include <math.h>
#include <stdint.h>

#define PSM__PI     3.14159265358979323846f
#define PSM__TWO_PI 6.28318530717958647692f

struct psm__vec2 {
  psm__f32 x, y;
};

static inline struct psm__vec2
psm__v2(psm__f32 x, psm__f32 y)
{
  return (struct psm__vec2){ x, y };
}

static inline struct psm__vec2
psm__v2_load(const psm__f32 *arr, psm__i32 idx)
{
  return (struct psm__vec2){ arr[idx * 2], arr[idx * 2 + 1] };
}

static inline void
psm__v2_store(psm__f32 *arr, psm__i32 idx, struct psm__vec2 v)
{
  arr[idx * 2] = v.x;
  arr[idx * 2 + 1] = v.y;
}

static inline struct psm__vec2
psm__v2_add(struct psm__vec2 a, struct psm__vec2 b)
{
  return (struct psm__vec2){ a.x + b.x, a.y + b.y };
}

static inline struct psm__vec2
psm__v2_sub(struct psm__vec2 a, struct psm__vec2 b)
{
  return (struct psm__vec2){ a.x - b.x, a.y - b.y };
}

static inline struct psm__vec2
psm__v2_scale(struct psm__vec2 v, psm__f32 s)
{
  return (struct psm__vec2){ v.x * s, v.y * s };
}

static inline struct psm__vec2
psm__v2_neg(struct psm__vec2 v)
{
  return (struct psm__vec2){ -v.x, -v.y };
}

static inline struct psm__vec2
psm__v2_lerp(struct psm__vec2 a, struct psm__vec2 b, psm__f32 t)
{
  return (struct psm__vec2){ t * (b.x - a.x) + a.x, t * (b.y - a.y) + a.y };
}

static inline struct psm__vec2
psm__v2_bary3(struct psm__vec2 a, struct psm__vec2 b, struct psm__vec2 c,
    psm__f32 wa, psm__f32 wb, psm__f32 wc)
{
  return (struct psm__vec2){ wc * c.x + (wb * b.x + wa * a.x),
    wc * c.y + (wb * b.y + wa * a.y) };
}

static inline struct psm__vec2
psm__v2_bilinear(struct psm__vec2 p00, struct psm__vec2 p10,
    struct psm__vec2 p01, struct psm__vec2 p11, psm__f32 u, psm__f32 v)
{
  psm__f32 inv_u = 1.0f - u;
  psm__f32 x0 = u * p10.x + inv_u * p00.x,
           y0 = u * p10.y + inv_u * p00.y,
           x1 = u * p11.x + inv_u * p01.x,
           y1 = u * p11.y + inv_u * p01.y;
  psm__f32 inv_v = 1.0f - v;
  return (struct psm__vec2){ v * x1 + inv_v * x0, v * y1 + inv_v * y0 };
}

static inline psm__f32
psm__clamp_f32(psm__f32 v, psm__f32 lo, psm__f32 hi)
{
  return fminf(fmaxf(v, lo), hi);
}

static inline psm__f32
psm__clamp_f32_01(psm__f32 v)
{
  return fminf(fmaxf(v, 0.0f), 1.0f);
}

/*
 * Convert float to int32 with clamping. NaN returns 0.
 * Note: (float)INT32_MAX rounds up to 2147483648.0f which overflows int32,
 * so use 2147483520.0f (largest float < 2^31).
 */
static inline psm__i32
psm__f32_to_i32(psm__f32 v)
{
  return (v == v)
             ? (psm__i32)psm__clamp_f32(v, -2147483648.0f, 2147483520.0f)
             : 0;
}

PSM__DEF psm__f32 psm__signed_angle(const psm__f32 *, const psm__f32 *);

#endif /* PSM__MATH2_H */
/* ===== moc3.h ===== */
/*
 * Purism Core: MOC3 format structures
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__MOC3_H
#define PSM__MOC3_H


struct psm__moc3_header;
struct psm__moc3_data;
struct psm__sections;
struct psm__count_info;
struct psm__canvas_info;

struct psm__moc3_header {
  char    magic[4];
  psm__u8 version;
  psm__u8 endian_flag;
  psm__u8 padding1_[2];

  struct psm__moc3_data *data;

  psm__i32 last_error;

  psm__u8 padding2_[52 - sizeof(void *)];
};

psm__static_assert(sizeof(struct psm__moc3_header) == 64,
    "MOC3 header must be 64 bytes");

struct psm__moc3_data {
  struct psm__moc3_header *header;
  psm__u32                *offsets;
  struct psm__sections    *sections;
};

struct psm__id {
  char data[64];
};

struct psm__count_info {
  psm__i32 parts;
  psm__i32 deformers;
  psm__i32 warps;
  psm__i32 rotations;
  psm__i32 art_meshes;
  psm__i32 parameters;
  psm__i32 part_keyforms;
  psm__i32 warp_keyforms;
  psm__i32 rotation_keyforms;
  psm__i32 art_mesh_keyforms;
  psm__i32 keyform_pos;
  psm__i32 key_table_idx;
  psm__i32 bindings;
  psm__i32 key_tables;
  psm__i32 keys;
  psm__i32 uvs;
  psm__i32 idx;
  psm__i32 masks;
  psm__i32 draw_groups;
  psm__i32 draw_items;
  psm__i32 glues;
  psm__i32 glue_info;
  psm__i32 glue_keyforms;
  psm__i32 keyform_mul_colors;
  psm__i32 keyform_scr_colors;
  psm__i32 blend_key_tables;
  psm__i32 blend_bindings;
  psm__i32 bs_warps;
  psm__i32 bs_art_meshes;
  psm__i32 bs_constraint_idx;
  psm__i32 bs_constraints;
  psm__i32 bs_constraint_vals;
  psm__i32 bs_parts;
  psm__i32 bs_rotations;
  psm__i32 bs_glues;
  psm__i32 offscreens;
  psm__i32 offscreen_keyforms;
  psm__i32 bs_offscreens;
  psm__i32 padding_[26];
};

psm__static_assert(sizeof(struct psm__count_info) == 64 * sizeof(psm__i32),
    "count_info must be 256 bytes");

#define PSM__COUNT_INFO_INTS(ver) ((ver) >= csmMocVersion_50 ? 64 : 32)

struct psm__canvas_info {
  psm__f32 pix_per_unit;
  psm__f32 origin_x;
  psm__f32 origin_y;
  psm__f32 width;
  psm__f32 height;
  psm__u8  flag;
};

struct psm__part_src {
  const char    **id_runtime;
  struct psm__id *id;
  psm__i32       *binding_idx;
  psm__i32       *keyform_off;
  psm__i32       *key_len;
  psm__i32       *visible;
  psm__i32       *enable;
  psm__i32       *parent_part_idx;
  psm__i32       *offscreen_idx;
};

struct psm__deformer_src {
  const char    **id_runtime;
  struct psm__id *id;
  psm__i32       *binding_idx;
  psm__i32       *visible;
  psm__i32       *enable;
  psm__i32       *parent_part_idx;
  psm__i32       *parent_deformer_idx;
  psm__i32       *type;
  psm__i32       *local_idx;
};

struct psm__warp_src {
  psm__i32 *binding_idx;
  psm__i32 *keyform_off;
  psm__i32 *key_len;
  psm__i32 *key_color_off;
  psm__i32 *vertex_count;
  psm__i32 *row;
  psm__i32 *col;
  psm__i32 *quad_transform;
};

struct psm__rotation_src {
  psm__i32 *binding_idx;
  psm__i32 *keyform_off;
  psm__i32 *key_len;
  psm__i32 *key_color_off;
  psm__f32 *base_angle;
};

struct psm__art_mesh_src {
  const char     **id_runtime;
  const psm__f32 **uv_runtime;
  const psm__u16 **pos_idx_runtime;
  const psm__i32 **drawable_mask_runtime;
  void            *id;
  psm__i32        *binding_idx;
  psm__i32        *keyform_off;
  psm__i32        *key_len;
  psm__i32        *key_color_off;
  psm__i32        *visible;
  psm__i32        *enable;
  psm__i32        *parent_part_idx;
  psm__i32        *parent_deformer_idx;
  psm__i32        *texture_no;
  psm__u8         *drawable_flag;
  psm__i32        *blend_mode;
  psm__i32        *vertex_count;
  psm__i32        *uv_off;
  psm__i32        *idx_off;
  psm__i32        *idx_len;
  psm__i32        *mask_off;
  psm__i32        *mask_len;
};

struct psm__param_src {
  const char    **id_runtime;
  struct psm__id *id;
  psm__f32       *maximum_value;
  psm__f32       *minimum_value;
  psm__f32       *default_value;
  psm__i32       *repeat;
  psm__i32       *decimal_places;
  psm__i32       *type;
  psm__i32       *key_table_off;
  psm__i32       *key_table_len;
  psm__i32       *blend_key_table_off;
  psm__i32       *blend_key_table_len;
};

struct psm__glue_src {
  const char    **id_runtime;
  struct psm__id *id;
  psm__i32       *binding_idx;
  psm__i32       *keyform_off;
  psm__i32       *key_len;
  psm__i32       *art_mesh_idx_a;
  psm__i32       *art_mesh_idx_b;
  psm__i32       *info_off;
  psm__i32       *info_len;
};

struct psm__part_key_src {
  psm__f32 *draw_order;
  psm__i32 *key_idx;
};

struct psm__warp_key_src {
  psm__f32 *opacity;
  psm__i32 *key_pos_off;
  psm__i32 *key_mul_color_off;
  psm__i32 *key_scr_color_off;
};

struct psm__rotation_key_src {
  psm__f32 *opacity;
  psm__f32 *angle;
  psm__f32 *origin_x;
  psm__f32 *origin_y;
  psm__f32 *scale;
  psm__i32 *reflect_x;
  psm__i32 *reflect_y;
  psm__i32 *key_mul_color_off;
  psm__i32 *key_scr_color_off;
};

struct psm__art_mesh_key_src {
  psm__f32 *opacity;
  psm__f32 *draw_order;
  psm__i32 *key_pos_off;
  psm__i32 *key_mul_color_off;
  psm__i32 *key_scr_color_off;
};

struct psm__glue_key_src {
  psm__f32 *intensity;
};

struct psm__key_pos_src {
  psm__f32 *xy;
};

struct psm__key_table_idx_src {
  psm__i32 *idx;
};

struct psm__binding_src {
  psm__i32 *key_table_idx_off;
  psm__i32 *key_table_idx_len;
};

struct psm__key_table_src {
  psm__i32 *keys_off;
  psm__i32 *keys_len;
};

struct psm__keys_src {
  psm__f32 *key;
};

struct psm__uv_src {
  psm__f32 *xy;
};

struct psm__pos_idx_src {
  psm__u16 *idx;
};

struct psm__mask_src {
  psm__i32 *art_mesh_idx;
};

struct psm__draw_group_src {
  psm__i32 *obj_off;
  psm__i32 *obj_len;
  psm__i32 *obj_total_count;
  psm__i32 *max_order;
  psm__i32 *min_order;
};

struct psm__draw_group_obj_src {
  psm__i32 *type;
  psm__i32 *idx;
  psm__i32 *self_group_idx;
};

struct psm__glue_info_src {
  psm__f32 *weight;
  psm__u16 *pos_idx;
};

struct psm__param_keys_src {
  const psm__f32 **key_runtime;
  psm__i32        *keys_off;
  psm__i32        *keys_len;
};

struct psm__blend_key_table_src {
  psm__i32 *keys_off;
  psm__i32 *keys_len;
  psm__i32 *base_key_idx;
};

struct psm__blend_binding_src {
  psm__i32 *key_table_idx;
  psm__i32 *key_bs_off;
  psm__i32 *key_bs_len;
  psm__i32 *bs_constraint_idx_off;
  psm__i32 *bs_constraint_idx_len;
};

struct psm__blend_src {
  psm__i32 *target_idx;
  psm__i32 *bs_binding_off;
  psm__i32 *bs_binding_len;
};

struct psm__blend_constraint_idx_src {
  psm__i32 *constraint_idx;
};

struct psm__blend_constraint_src {
  psm__i32 *parameter_idx;
  psm__i32 *value_off;
  psm__i32 *value_len;
};

struct psm__blend_constraint_val_src {
  psm__f32 *key;
  psm__f32 *weight;
};

struct psm__offscreen_src {
  const psm__i32 **drawable_mask_runtime;
  psm__i32        *owner_idx;
  psm__u8         *drawable_flag;
  psm__i32        *blend_mode;
  psm__i32        *mask_off;
  psm__i32        *mask_len;
};

struct psm__key_color_src {
  psm__f32 *r;
  psm__f32 *g;
  psm__f32 *b;
};

struct psm__offscreen_key_src {
  psm__f32 *opacity;
  psm__i32 *key_mul_color_off;
  psm__i32 *key_scr_color_off;
};

struct psm__sections {
  struct psm__moc3_data    source;
  struct psm__count_info  *count_info;
  struct psm__canvas_info *canvas_info;

  struct psm__part_src     part_src;
  struct psm__deformer_src deformer_src;
  struct psm__warp_src     warp_src;
  struct psm__rotation_src rotation_src;
  struct psm__art_mesh_src art_mesh_src;

  struct psm__param_src      param_src;
  struct psm__param_keys_src param_keys_src;

  struct psm__part_key_src     part_key_src;
  struct psm__warp_key_src     warp_key_src;
  struct psm__rotation_key_src rotation_key_src;
  struct psm__art_mesh_key_src art_mesh_key_src;

  struct psm__key_pos_src       key_pos_src;
  struct psm__key_table_src     key_table_src;
  struct psm__key_table_idx_src key_table_idx_src;
  struct psm__binding_src       binding_src;

  struct psm__blend_key_table_src      blend_key_table_src;
  struct psm__blend_binding_src        blend_binding_src;
  struct psm__blend_src                bs_part_src;
  struct psm__blend_src                bs_warp_src;
  struct psm__blend_src                bs_rotation_src;
  struct psm__blend_src                bs_art_mesh_src;
  struct psm__blend_src                bs_glue_src;
  struct psm__blend_constraint_idx_src blend_constraint_idx_src;
  struct psm__blend_constraint_src     blend_constraint_src;
  struct psm__blend_constraint_val_src blend_constraint_val_src;

  struct psm__keys_src    keys_src;
  struct psm__uv_src      uv_src;
  struct psm__pos_idx_src idx_src;
  struct psm__mask_src    mask_src;

  struct psm__draw_group_src     draw_group_src;
  struct psm__draw_group_obj_src draw_group_obj_src;

  struct psm__glue_src      glue_src;
  struct psm__glue_info_src glue_info_src;
  struct psm__glue_key_src  glue_key_src;

  struct psm__key_color_src keyform_mul_color_src;
  struct psm__key_color_src keyform_scr_color_src;

  struct psm__offscreen_src     offscreen_src;
  struct psm__offscreen_key_src offscreen_key_src;
  struct psm__blend_src         bs_offscreen_src;
};

/* MOC3 v1-v5: 160 section offsets */
struct psm__moc3_data_v52 {
  struct psm__moc3_header header;
  psm__u32                offsets[160];
  struct psm__sections    sections;
};

/* MOC3 v6+: 480 section offsets */
struct psm__moc3_data_v53 {
  struct psm__moc3_header header;
  psm__u32                offsets[480];
  struct psm__sections    sections;
};

/*
 * FOREACH macros for sections member initialization
 * S(TYPE, MEMBER, COUNT) - static count (compile-time constant)
 * D(TYPE, MEMBER, CNT_MEMBER) - dynamic count from count_info
 */
#define PSM__SECTIONS_V30(S, D) \
  S(struct psm__count_info, count_info, 1) \
  S(struct psm__canvas_info, canvas_info, 1) \
  D(const char *, part_src.id_runtime, parts) \
  D(struct psm__id, part_src.id, parts) \
  D(psm__i32, part_src.binding_idx, parts) \
  D(psm__i32, part_src.keyform_off, parts) \
  D(psm__i32, part_src.key_len, parts) \
  D(psm__i32, part_src.visible, parts) \
  D(psm__i32, part_src.enable, parts) \
  D(psm__i32, part_src.parent_part_idx, parts) \
  D(const char *, deformer_src.id_runtime, deformers) \
  D(struct psm__id, deformer_src.id, deformers) \
  D(psm__i32, deformer_src.binding_idx, deformers) \
  D(psm__i32, deformer_src.visible, deformers) \
  D(psm__i32, deformer_src.enable, deformers) \
  D(psm__i32, deformer_src.parent_part_idx, deformers) \
  D(psm__i32, deformer_src.parent_deformer_idx, deformers) \
  D(psm__i32, deformer_src.type, deformers) \
  D(psm__i32, deformer_src.local_idx, deformers) \
  D(psm__i32, warp_src.binding_idx, warps) \
  D(psm__i32, warp_src.keyform_off, warps) \
  D(psm__i32, warp_src.key_len, warps) \
  D(psm__i32, warp_src.vertex_count, warps) \
  D(psm__i32, warp_src.row, warps) \
  D(psm__i32, warp_src.col, warps) \
  D(psm__i32, rotation_src.binding_idx, rotations) \
  D(psm__i32, rotation_src.keyform_off, rotations) \
  D(psm__i32, rotation_src.key_len, rotations) \
  D(psm__f32, rotation_src.base_angle, rotations) \
  D(const char *, art_mesh_src.id_runtime, art_meshes) \
  D(const psm__f32 *, art_mesh_src.uv_runtime, art_meshes) \
  D(const psm__u16 *, art_mesh_src.pos_idx_runtime, art_meshes) \
  D(const psm__i32 *, art_mesh_src.drawable_mask_runtime, art_meshes) \
  D(struct psm__id, art_mesh_src.id, art_meshes) \
  D(psm__i32, art_mesh_src.binding_idx, art_meshes) \
  D(psm__i32, art_mesh_src.keyform_off, art_meshes) \
  D(psm__i32, art_mesh_src.key_len, art_meshes) \
  D(psm__i32, art_mesh_src.visible, art_meshes) \
  D(psm__i32, art_mesh_src.enable, art_meshes) \
  D(psm__i32, art_mesh_src.parent_part_idx, art_meshes) \
  D(psm__i32, art_mesh_src.parent_deformer_idx, art_meshes) \
  D(psm__i32, art_mesh_src.texture_no, art_meshes) \
  D(psm__u8, art_mesh_src.drawable_flag, art_meshes) \
  D(psm__i32, art_mesh_src.vertex_count, art_meshes) \
  D(psm__i32, art_mesh_src.uv_off, art_meshes) \
  D(psm__i32, art_mesh_src.idx_off, art_meshes) \
  D(psm__i32, art_mesh_src.idx_len, art_meshes) \
  D(psm__i32, art_mesh_src.mask_off, art_meshes) \
  D(psm__i32, art_mesh_src.mask_len, art_meshes) \
  D(const char *, param_src.id_runtime, parameters) \
  D(struct psm__id, param_src.id, parameters) \
  D(psm__f32, param_src.maximum_value, parameters) \
  D(psm__f32, param_src.minimum_value, parameters) \
  D(psm__f32, param_src.default_value, parameters) \
  D(psm__i32, param_src.repeat, parameters) \
  D(psm__i32, param_src.decimal_places, parameters) \
  D(psm__i32, param_src.key_table_off, parameters) \
  D(psm__i32, param_src.key_table_len, parameters) \
  D(psm__f32, part_key_src.draw_order, part_keyforms) \
  D(psm__f32, warp_key_src.opacity, warp_keyforms) \
  D(psm__i32, warp_key_src.key_pos_off, warp_keyforms) \
  D(psm__f32, rotation_key_src.opacity, rotation_keyforms) \
  D(psm__f32, rotation_key_src.angle, rotation_keyforms) \
  D(psm__f32, rotation_key_src.origin_x, rotation_keyforms) \
  D(psm__f32, rotation_key_src.origin_y, rotation_keyforms) \
  D(psm__f32, rotation_key_src.scale, rotation_keyforms) \
  D(psm__i32, rotation_key_src.reflect_x, rotation_keyforms) \
  D(psm__i32, rotation_key_src.reflect_y, rotation_keyforms) \
  D(psm__f32, art_mesh_key_src.opacity, art_mesh_keyforms) \
  D(psm__f32, art_mesh_key_src.draw_order, art_mesh_keyforms) \
  D(psm__i32, art_mesh_key_src.key_pos_off, art_mesh_keyforms) \
  D(psm__f32, key_pos_src.xy, keyform_pos) \
  D(psm__i32, key_table_idx_src.idx, key_table_idx) \
  D(psm__i32, binding_src.key_table_idx_off, bindings) \
  D(psm__i32, binding_src.key_table_idx_len, bindings) \
  D(psm__i32, key_table_src.keys_off, key_tables) \
  D(psm__i32, key_table_src.keys_len, key_tables) \
  D(psm__f32, keys_src.key, keys) \
  D(psm__f32, uv_src.xy, uvs) \
  D(psm__u16, idx_src.idx, idx) \
  D(psm__i32, mask_src.art_mesh_idx, masks) \
  D(psm__i32, draw_group_src.obj_off, draw_groups) \
  D(psm__i32, draw_group_src.obj_len, draw_groups) \
  D(psm__i32, draw_group_src.obj_total_count, draw_groups) \
  D(psm__i32, draw_group_src.max_order, draw_groups) \
  D(psm__i32, draw_group_src.min_order, draw_groups) \
  D(psm__i32, draw_group_obj_src.type, draw_items) \
  D(psm__i32, draw_group_obj_src.idx, draw_items) \
  D(psm__i32, draw_group_obj_src.self_group_idx, draw_items) \
  D(const char *, glue_src.id_runtime, glues) \
  D(struct psm__id, glue_src.id, glues) \
  D(psm__i32, glue_src.binding_idx, glues) \
  D(psm__i32, glue_src.keyform_off, glues) \
  D(psm__i32, glue_src.key_len, glues) \
  D(psm__i32, glue_src.art_mesh_idx_a, glues) \
  D(psm__i32, glue_src.art_mesh_idx_b, glues) \
  D(psm__i32, glue_src.info_off, glues) \
  D(psm__i32, glue_src.info_len, glues) \
  D(psm__f32, glue_info_src.weight, glue_info) \
  D(psm__u16, glue_info_src.pos_idx, glue_info) \
  D(psm__f32, glue_key_src.intensity, glue_keyforms)

#define PSM__SECTIONS_V33(S, D) \
  D(psm__i32, warp_src.quad_transform, warps)

#define PSM__SECTIONS_V42(S, D) \
  D(const psm__f32 *, param_keys_src.key_runtime, parameters) \
  D(psm__i32, param_keys_src.keys_off, parameters) \
  D(psm__i32, param_keys_src.keys_len, parameters) \
  D(psm__i32, warp_src.key_color_off, warps) \
  D(psm__i32, rotation_src.key_color_off, rotations) \
  D(psm__i32, art_mesh_src.key_color_off, art_meshes) \
  D(psm__f32, keyform_mul_color_src.r, keyform_mul_colors) \
  D(psm__f32, keyform_mul_color_src.g, keyform_mul_colors) \
  D(psm__f32, keyform_mul_color_src.b, keyform_mul_colors) \
  D(psm__f32, keyform_scr_color_src.r, keyform_scr_colors) \
  D(psm__f32, keyform_scr_color_src.g, keyform_scr_colors) \
  D(psm__f32, keyform_scr_color_src.b, keyform_scr_colors) \
  D(psm__i32, param_src.type, parameters) \
  D(psm__i32, param_src.blend_key_table_off, parameters) \
  D(psm__i32, param_src.blend_key_table_len, parameters) \
  D(psm__i32, blend_key_table_src.keys_off, blend_key_tables) \
  D(psm__i32, blend_key_table_src.keys_len, blend_key_tables) \
  D(psm__i32, blend_key_table_src.base_key_idx, blend_key_tables) \
  D(psm__i32, blend_binding_src.key_table_idx, blend_bindings) \
  D(psm__i32, blend_binding_src.key_bs_off, blend_bindings) \
  D(psm__i32, blend_binding_src.key_bs_len, blend_bindings) \
  D(psm__i32, blend_binding_src.bs_constraint_idx_off, blend_bindings) \
  D(psm__i32, blend_binding_src.bs_constraint_idx_len, blend_bindings) \
  D(psm__i32, bs_warp_src.target_idx, bs_warps) \
  D(psm__i32, bs_warp_src.bs_binding_off, bs_warps) \
  D(psm__i32, bs_warp_src.bs_binding_len, bs_warps) \
  D(psm__i32, bs_art_mesh_src.target_idx, bs_art_meshes) \
  D(psm__i32, bs_art_mesh_src.bs_binding_off, bs_art_meshes) \
  D(psm__i32, bs_art_mesh_src.bs_binding_len, bs_art_meshes) \
  D(psm__i32, blend_constraint_idx_src.constraint_idx, bs_constraint_idx) \
  D(psm__i32, blend_constraint_src.parameter_idx, bs_constraints) \
  D(psm__i32, blend_constraint_src.value_off, bs_constraints) \
  D(psm__i32, blend_constraint_src.value_len, bs_constraints) \
  D(psm__f32, blend_constraint_val_src.key, bs_constraint_vals) \
  D(psm__f32, blend_constraint_val_src.weight, bs_constraint_vals)

#define PSM__SECTIONS_V50(S, D) \
  D(psm__i32, warp_key_src.key_mul_color_off, warp_keyforms) \
  D(psm__i32, warp_key_src.key_scr_color_off, warp_keyforms) \
  D(psm__i32, rotation_key_src.key_mul_color_off, rotation_keyforms) \
  D(psm__i32, rotation_key_src.key_scr_color_off, rotation_keyforms) \
  D(psm__i32, art_mesh_key_src.key_mul_color_off, art_mesh_keyforms) \
  D(psm__i32, art_mesh_key_src.key_scr_color_off, art_mesh_keyforms) \
  D(psm__i32, bs_part_src.target_idx, bs_parts) \
  D(psm__i32, bs_part_src.bs_binding_off, bs_parts) \
  D(psm__i32, bs_part_src.bs_binding_len, bs_parts) \
  D(psm__i32, bs_rotation_src.target_idx, bs_rotations) \
  D(psm__i32, bs_rotation_src.bs_binding_off, bs_rotations) \
  D(psm__i32, bs_rotation_src.bs_binding_len, bs_rotations) \
  D(psm__i32, bs_glue_src.target_idx, bs_glues) \
  D(psm__i32, bs_glue_src.bs_binding_off, bs_glues) \
  D(psm__i32, bs_glue_src.bs_binding_len, bs_glues)

#define PSM__SECTIONS_V53(S, D) \
  D(psm__i32, part_src.offscreen_idx, parts) \
  D(psm__i32, art_mesh_src.blend_mode, art_meshes) \
  D(const psm__i32 *, offscreen_src.drawable_mask_runtime, offscreens) \
  D(psm__i32, offscreen_src.owner_idx, offscreens) \
  D(psm__u8, offscreen_src.drawable_flag, offscreens) \
  D(psm__i32, offscreen_src.blend_mode, offscreens) \
  D(psm__i32, offscreen_src.mask_off, offscreens) \
  D(psm__i32, offscreen_src.mask_len, offscreens) \
  D(psm__i32, part_key_src.key_idx, part_keyforms) \
  D(psm__f32, offscreen_key_src.opacity, offscreen_keyforms) \
  D(psm__i32, offscreen_key_src.key_mul_color_off, offscreen_keyforms) \
  D(psm__i32, offscreen_key_src.key_scr_color_off, offscreen_keyforms) \
  D(psm__i32, bs_offscreen_src.target_idx, bs_offscreens) \
  D(psm__i32, bs_offscreen_src.bs_binding_off, bs_offscreens) \
  D(psm__i32, bs_offscreen_src.bs_binding_len, bs_offscreens)

static inline struct psm__moc3_data *
psm__moc_to_data(const csmMoc *moc)
{
  return ((struct psm__moc3_header *)moc)->data;
}

#endif /* PSM__MOC3_H */
/* ===== model.h ===== */
/*
 * Purism Core: model runtime structures
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__MODEL_H
#define PSM__MODEL_H


struct psm__color3 {
  psm__f32 *r, *g, *b;
};

struct psm__model;

struct psm__interp {
  psm__i32  object_count;
  psm__i32 *max_blend;
  psm__i32 *blend_count;
  psm__i32  tmp_len;
  psm__f32 *tmp;
  psm__f32 *weights;
};

struct psm__key_table {
  psm__i32  key_count;
  psm__f32 *keys;
  psm__i32  idx;
  psm__f32  weight;
  bool      out_of_range;
  bool      idx_dirty;
  bool      weight_dirty;
};

struct psm__blend_key_table {
  psm__i32  key_count;
  psm__f32 *keys;
  psm__i32  base_key_idx;
  psm__i32  idx;
  psm__f32  weight;
  bool      idx_dirty;
  bool      weight_dirty;
};

struct psm__binding {
  struct psm__key_table **key_tables;
  psm__i32                key_table_len;
  psm__i32                max_blend;
  psm__i32                blend_count;
  psm__i32               *keyform_idx;
  psm__f32               *weights;
  bool                    idx_dirty;
  bool                    weight_dirty;
  bool                    out_of_range;
};

struct psm__blend_constraint {
  struct psm__param *param;
  psm__f32          *keys;
  psm__f32          *weights;
  psm__i32           count;
  psm__f32           weight;
};

struct psm__blend_binding {
  struct psm__blend_key_table   *key_table;
  psm__i32                       key_src_off;
  psm__i32                       blend_count;
  psm__i32                       keyform_idx[2];
  psm__f32                       weights[2];
  bool                           idx_dirty;
  bool                           weight_dirty;
  psm__i32                       constraint_count;
  struct psm__blend_constraint **constraints;
  psm__f32                       weight;
};

struct psm__part {
  struct psm__binding *binding;
  psm__i32             parent_part_idx;
  bool                 local_enable;
};

struct psm__part_keydata {
  struct psm__interp interp;
  psm__f32          *draw_order;
};

struct psm__parts {
  psm__i32                 count;
  struct psm__part        *items;
  struct psm__binding    **bindings; /* items[i].binding, gathered once at load */
  struct psm__part_keydata keydata;
  bool                    *enable;
  psm__i32                *draw_order;
  psm__f32                *opacity;
  psm__f32                *input_opacity;
  psm__i32                *offscreen_src_idx;
};

struct psm__deformer_node {
  struct psm__binding *binding;
  psm__i32             parent_part_idx;
  psm__i32             parent_deformer_idx;
  psm__i32             type;
  psm__i32             local_idx;
  bool                 local_enable;
};

struct psm__warp {
  struct psm__binding *binding;
  psm__i32             row;
  psm__i32             col;
  bool                 quad_transform;
  psm__i32             vertex_count;
};

struct psm__rotation {
  struct psm__binding *binding;
  psm__f32             base_angle;
};

struct psm__warp_keydata {
  struct psm__interp interp;
  psm__f32          *opacity;
  psm__f32         **pos;
  struct psm__color3 mul_color;
  struct psm__color3 scr_color;
};

struct psm__warps {
  psm__i32                 count;
  struct psm__warp        *items;
  struct psm__binding    **bindings; /* items[i].binding, gathered once at load */
  struct psm__warp_keydata keydata;
  bool                    *enable;
  psm__f32                *opacity;
  psm__f32               **pos;
  psm__f32                *mul_color;
  psm__f32                *scr_color;
};

struct psm__rotation_keydata {
  struct psm__interp interp;
  psm__f32          *opacity;
  psm__f32          *angle;
  psm__f32          *origin_x;
  psm__f32          *origin_y;
  psm__f32          *scale;
  struct psm__color3 mul_color;
  struct psm__color3 scr_color;
};

struct psm__rotations {
  psm__i32                     count;
  struct psm__rotation        *items;
  struct psm__binding        **bindings; /* items[i].binding, gathered once at load */
  struct psm__rotation_keydata keydata;
  bool                        *enable;
  psm__f32                    *opacity;
  psm__f32                    *scale;
  psm__f32                    *origin_x;
  psm__f32                    *origin_y;
  psm__f32                    *angle;
  psm__i32                    *reflect_x;
  psm__i32                    *reflect_y;
  psm__f32                    *mul_color;
  psm__f32                    *scr_color;
};

struct psm__deformers {
  struct psm__warps          warps;
  struct psm__rotations      rotations;
  psm__i32                   count;
  struct psm__deformer_node *nodes;
  bool                      *enable;
  psm__f32                  *opacity;
  psm__f32                  *scale;
  psm__f32                  *mul_color;
  psm__f32                  *scr_color;
};

struct psm__art_mesh {
  struct psm__binding *binding;
  psm__i32             parent_part_idx;
  psm__i32             parent_deformer_idx;
  bool                 local_enable;
  psm__i32             vertex_count;
};

struct psm__art_mesh_keydata {
  struct psm__interp interp;
  psm__f32          *opacity;
  psm__f32          *draw_order;
  psm__f32         **pos;
  struct psm__color3 mul_color;
  struct psm__color3 scr_color;
};

struct psm__art_meshes {
  psm__i32                     count;
  struct psm__art_mesh        *meshes;
  struct psm__binding        **bindings; /* meshes[i].binding, gathered once at load */
  struct psm__art_mesh_keydata keydata;
  bool                        *enable;
  bool                         state_changed;
  psm__u8                     *const_flags;
  psm__u8                     *change_flags;
  psm__i32                    *blend_mode;
  psm__i32                    *draw_order;
  psm__f32                   **pos;
  psm__f32                    *opacity;
  psm__f32                    *mul_color;
  psm__f32                    *scr_color;
  psm__i32                    *last_render_order;
  psm__i32                    *last_draw_order;
  psm__f32                    *last_opacity;
  psm__f32                    *last_mul_color;
  psm__f32                    *last_scr_color;
};

struct psm__draw_item {
  psm__i32 object_type;
  psm__i32 object_idx;
  psm__i32 group_idx;
  psm__i32 draw_order;
};

struct psm__draw_group {
  psm__i32               total_count;
  psm__i32               count;
  psm__i32               cursor;
  psm__i32               max_order;
  psm__i32               min_order;
  psm__i32               order_level;
  struct psm__draw_item *items;
};

struct psm__draw_sort {
  psm__i32 *first;
  psm__i32 *next;
  psm__i32 *last;
};

struct psm__draw_groups {
  psm__i32                count;
  struct psm__draw_group *groups;
  struct psm__draw_sort   sort;
};

struct psm__glue {
  struct psm__binding *binding;
  psm__i32             mesh_idx0;
  psm__i32             mesh_idx1;
  psm__i32             glue_info_count;
  bool                 local_enable;
  psm__f32            *weights;
  psm__u16            *pos_idx;
};

struct psm__glue_keydata {
  struct psm__interp interp;
  psm__f32          *intensity;
};

struct psm__glues {
  psm__i32                 count;
  struct psm__glue        *items;
  struct psm__binding    **bindings; /* items[i].binding, gathered once at load */
  struct psm__glue_keydata keydata;
  psm__f32                *intensity;
};

struct psm__offscreen {
  struct psm__binding *binding;
  bool                *owner_enable;
  psm__i32            *keyform_idx;
};

struct psm__offscreen_keydata {
  struct psm__interp interp;
  psm__f32          *opacity;
  struct psm__color3 mul_color;
  struct psm__color3 scr_color;
};

struct psm__offscreens {
  psm__i32                      count;
  struct psm__offscreen        *surfaces;
  struct psm__offscreen_keydata keydata;
  bool                         *enable;
  psm__f32                     *opacity;
  psm__f32                     *mul_color;
  psm__f32                     *scr_color;
};

struct psm__param {
  psm__i32                     type;
  psm__f32                     range[2];
  psm__f32                     range_length;
  bool                         repeat;
  psm__f32                     snap_eps;
  psm__f32                     interp_eps;
  psm__f32                     value;
  bool                         dirty;
  struct psm__key_table       *key_tables;
  psm__i32                     key_table_len;
  struct psm__blend_key_table *blend_key_tables;
  psm__i32                     blend_key_table_len;
};

struct psm__params {
  psm__i32           count;
  struct psm__param *items;
  psm__i32          *type;
  psm__f32          *input_value;
};

struct psm__key_tables {
  psm__i32               count;
  struct psm__key_table *items;
};

struct psm__bindings {
  psm__i32             count;
  struct psm__binding *items;
};

struct psm__blend_shape {
  psm__i32                   target_idx;
  psm__i32                   binding_count;
  struct psm__blend_binding *bindings;
};

struct psm__blend_shapes {
  psm__i32                 count;
  struct psm__blend_shape *items;
};

struct psm__blend_constraints {
  psm__i32                      count;
  struct psm__blend_constraint *items;
};

struct psm__blend_key_tables {
  psm__i32                     count;
  struct psm__blend_key_table *items;
};

struct psm__blend_bindings {
  psm__i32                   count;
  struct psm__blend_binding *items;
};

struct psm__param_keys {
  psm__f32 **keys;
  psm__i32  *key_counts;
};

struct psm__model {
  const struct psm__moc3_data  *source;
  struct psm__parts             parts;
  struct psm__deformers         deformers;
  struct psm__art_meshes        art_meshes;
  struct psm__draw_groups       draw_groups;
  struct psm__glues             glues;
  struct psm__offscreens        offscreens;
  struct psm__params            params;
  struct psm__key_tables        key_tables;
  struct psm__bindings          bindings;
  struct psm__blend_constraints blend_constraints;
  struct psm__blend_key_tables  blend_key_tables;
  struct psm__blend_bindings    blend_bindings;
  struct psm__blend_shapes      bs_parts;
  struct psm__blend_shapes      bs_warps;
  struct psm__blend_shapes      bs_rotations;
  struct psm__blend_shapes      bs_art_meshes;
  struct psm__blend_shapes      bs_glues;
  struct psm__blend_shapes      bs_offscreens;
  struct psm__param_keys        param_keys;
  psm__i32                     *render_order;
  bool                          force_update;
  bool                          y_reversed;
  /* csmGetLastError: code from the most recent update */
  psm__i32 last_error;
};

#endif /* PSM__MODEL_H */
/* ===== verify.h ===== */
/*
 * Purism Core: MOC3 load-time validation
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__VERIFY_H
#define PSM__VERIFY_H


PSM__DEF int psm__verify_count_info(psm__u8 ver,
    const struct psm__count_info           *cnt);

PSM__DEF int psm__verify_sections(struct psm__sections *ms, psm__u8 *p,
    psm__u32 *offsets, psm_size n, psm_size off, psm__u8 ver, bool bounds);

PSM__DEF int psm__verify_idx(psm__u8 ver, const struct psm__sections *src);

#endif /* PSM__VERIFY_H */
/* ===== gather.h ===== */
/*
 * Purism Core: common gather helpers for keyform data
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__GATHER_H
#define PSM__GATHER_H

#include <string.h>

struct psm__gather_channel {
  const psm__f32 *src;
  psm__f32       *dst;
};

/*
 * Gather scalar keyform data into keydata workspace.
 * Common loop for all node types that use a flat keyform_offset array.
 *
 * bindings[i]       = pointer to binding for object i (NULL = skip)
 * keyform_offset[i] = per-object offset into keyform arrays
 */
static inline void
psm__gather_scalars(
    psm__i32                          count,
    struct psm__binding *const       *bindings,
    const psm__i32                   *keyform_offset,
    struct psm__interp               *interp,
    const struct psm__gather_channel *channels,
    psm__i32                          n_channels)
{
  psm__i32 offset = 0;
  for (psm__i32 i = 0; i < count; i++) {
    struct psm__binding *b = bindings[i];
    if (!b)
      continue;
    psm__i32 cc = b->blend_count;

    if (b->idx_dirty || b->weight_dirty)
      interp->blend_count[i] = cc;

    if (b->idx_dirty && cc > 0) {
      for (psm__i32 j = 0; j < cc; j++) {
        psm__i32 kfi = b->keyform_idx[j] + keyform_offset[i];
        for (psm__i32 c = 0; c < n_channels; c++)
          channels[c].dst[offset + j] = channels[c].src[kfi];
      }
    }

    if (b->weight_dirty && cc > 0)
      memcpy(&interp->weights[offset], b->weights, cc * sizeof(psm__f32));

    offset += b->max_blend;
  }
}

/*
 * Gather position pointer keyform data (warps, art meshes).
 *
 * We traverse keyform_idx -> keyform index -> pos_begin -> offset into pos_xy.
 * Each combo entry gets a pointer into the shared position pool.
 */
static inline void
psm__gather_positions(
    psm__i32                    count,
    struct psm__binding *const *bindings,
    const psm__i32             *keyform_offset,
    const psm__f32             *pos_xy,
    const psm__i32             *pos_begin,
    psm__f32                  **pos_dst)
{
  psm__i32 offset = 0;
  for (psm__i32 i = 0; i < count; i++) {
    struct psm__binding *b = bindings[i];
    if (!b)
      continue;
    if (b->idx_dirty && b->blend_count > 0) {
      for (psm__i32 j = 0; j < b->blend_count; j++) {
        psm__i32 kfi = b->keyform_idx[j] + keyform_offset[i];
        psm__i32 pi = pos_begin[kfi];
        pos_dst[offset + j] = (psm__f32 *)&pos_xy[pi];
      }
    }
    offset += b->max_blend;
  }
}

/*
 * Gather color keyform data from the global color pool.
 * Used by warps, rotations, and art meshes.
 */
static inline void
psm__gather_colors(
    psm__i32                         count,
    struct psm__binding *const      *bindings,
    const psm__i32                  *key_color_offset,
    const struct psm__key_color_src *mul_src,
    const struct psm__key_color_src *scr_src,
    struct psm__color3              *mul_dst,
    struct psm__color3              *scr_dst)
{
  psm__i32 offset = 0;
  for (psm__i32 i = 0; i < count; i++) {
    struct psm__binding *b = bindings[i];
    if (!b) continue;
    psm__i32 cc = b->blend_count;

    if (b->idx_dirty && cc > 0) {
      for (psm__i32 j = 0; j < cc; j++) {
        psm__i32 kfi = b->keyform_idx[j] + key_color_offset[i];
        psm__i32 oj = offset + j;
        mul_dst->r[oj] = mul_src->r[kfi];
        mul_dst->g[oj] = mul_src->g[kfi];
        mul_dst->b[oj] = mul_src->b[kfi];
        scr_dst->r[oj] = scr_src->r[kfi];
        scr_dst->g[oj] = scr_src->g[kfi];
        scr_dst->b[oj] = scr_src->b[kfi];
      }
    }

    offset += b->max_blend;
  }
}

/*
 * Gather rotation reflect flags (first keyform only).
 */
static inline void
psm__gather_reflect(
    psm__i32                    count,
    struct psm__binding *const *bindings,
    const psm__i32             *keyform_offset,
    const psm__i32             *rfx_src,
    const psm__i32             *rfy_src,
    psm__i32                   *rfx_dst,
    psm__i32                   *rfy_dst)
{
  for (psm__i32 i = 0; i < count; i++) {
    struct psm__binding *b = bindings[i];
    if (!b || !b->idx_dirty || b->blend_count <= 0)
      continue;
    psm__i32 kfi = b->keyform_idx[0] + keyform_offset[i];
    rfx_dst[i] = rfx_src[kfi];
    rfy_dst[i] = rfy_src[kfi];
  }
}

#endif /* PSM__GATHER_H */
/* ===== interpolate.h ===== */
/*
 * Purism Core: interpolation declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__INTERPOLATE_H
#define PSM__INTERPOLATE_H


PSM__DEF void psm__interp_parts(struct psm__model *model);
PSM__DEF void psm__interp_warps(struct psm__model *model);
PSM__DEF void psm__interp_rotations(struct psm__model *model);
PSM__DEF void psm__interp_art_meshes(struct psm__model *model);
PSM__DEF void psm__interp_glues(struct psm__model *model);
PSM__DEF void psm__interp_offscreens(struct psm__model *model);

#endif /* PSM__INTERPOLATE_H */
/* ===== artmesh.h ===== */
/*
 * Purism Core: art mesh declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__ARTMESH_H
#define PSM__ARTMESH_H


PSM__DEF void psm__enable_art_meshes(struct psm__model *model);
PSM__DEF void psm__gather_art_meshes(struct psm__model *model);
PSM__DEF void psm__apply_parts_to_meshes(struct psm__model *model);

#endif /* PSM__ARTMESH_H */
/* ===== blendshape.h ===== */
/*
 * Purism Core: blend shape declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__BLENDSHAPE_H
#define PSM__BLENDSHAPE_H


PSM__DEF void psm__blend_parts(struct psm__model *model);
PSM__DEF void psm__blend_warps(struct psm__model *model);
PSM__DEF void psm__blend_rotations(struct psm__model *model);
PSM__DEF void psm__blend_art_meshes(struct psm__model *model);
PSM__DEF void psm__blend_glues(struct psm__model *model);
PSM__DEF void psm__blend_offscreens(struct psm__model *model);

#endif /* PSM__BLENDSHAPE_H */
/* ===== deformer.h ===== */
/*
 * Purism Core: deformer declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__DEFORMER_H
#define PSM__DEFORMER_H


#define PSM__DEFORMER_TYPE_WARP     0
#define PSM__DEFORMER_TYPE_ROTATION 1

PSM__DEF void psm__enable_deformers(struct psm__model *);
PSM__DEF void psm__gather_warps(struct psm__model *);
PSM__DEF void psm__gather_rotations(struct psm__model *);
PSM__DEF void psm__apply_transforms(struct psm__model *);
PSM__DEF void psm__apply_transforms_to_meshes(struct psm__model *);

#endif /* PSM__DEFORMER_H */
/* ===== glue.h ===== */
/*
 * Purism Core: glue declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__GLUE_H
#define PSM__GLUE_H


PSM__DEF void psm__gather_glues(struct psm__model *model);
PSM__DEF void psm__apply_glues(struct psm__model *model);

#endif /* PSM__GLUE_H */
/* ===== offscreen.h ===== */
/*
 * Purism Core: offscreen declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__OFFSCREEN_H
#define PSM__OFFSCREEN_H


PSM__DEF void psm__enable_offscreens(struct psm__model *);
PSM__DEF void psm__gather_offscreens(struct psm__model *);

#endif /* PSM__OFFSCREEN_H */
/* ===== param.h ===== */
/*
 * Purism Core: parameter declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__PARAM_H
#define PSM__PARAM_H


PSM__DEF int  psm__resolve_params(struct psm__params *);
PSM__DEF void psm__resolve_key_tables(struct psm__model *);
PSM__DEF void psm__resolve_blend_key_tables(struct psm__model *);
PSM__DEF void psm__resolve_bindings(struct psm__model *);
PSM__DEF void psm__resolve_blend_bindings(struct psm__model *);

#endif /* PSM__PARAM_H */
/* ===== part.h ===== */
/*
 * Purism Core: part declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__PART_H
#define PSM__PART_H


PSM__DEF void psm__enable_parts(struct psm__model *model);
PSM__DEF void psm__gather_parts(struct psm__model *model);
PSM__DEF void psm__apply_part_opacity(struct psm__model *model);

#endif /* PSM__PART_H */
/* ===== render.h ===== */
/*
 * Purism Core: render declarations
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__RENDER_H
#define PSM__RENDER_H


PSM__DEF void psm__sort_render_order(struct psm__model *model);

#endif /* PSM__RENDER_H */
/* ===== update.h ===== */
/*
 * Purism Core: update pipeline declaration
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM__UPDATE_H
#define PSM__UPDATE_H


PSM__DEF void psm__update_model(struct psm__model *model);

#endif /* PSM__UPDATE_H */

/* Implementation */
/* ===== core.c ===== */
/*
 * Purism Core: version and misc functions
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <stdio.h>


#ifndef PSM_GIT_HASH
#  define PSM_GIT_HASH "unknown"
#endif

PSMDEF csmVersion
csmGetVersion(void)
{
  return PSM_COMPAT_VERSION;
}

PSMDEF csmVersion
csmGetTrueVersion(void)
{
  return PSM_TRUE_VERSION;
}

PSMDEF const char *
csmGetExtendedVersionString(void)
{
  static char buf[96];
  if (buf[0] == '\0')
    snprintf(buf, sizeof buf, PSM__VERFMT " (%s)",
        PSM__VERARG(PSM_TRUE_VERSION), PSM_GIT_HASH);
  return buf;
}

PSMDEF csmMocVersion
csmGetLatestMocVersion(void)
{
  return csmMocVersion_53;
}
/* ===== debug.c ===== */
/*
 * Purism Core: logging
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#ifndef PSM_NO_STDIO
#  include <stdarg.h>
#  include <stdio.h>
#endif


static void psm__default_log(const char *);

static csmLogFunction psm__log_fn = psm__default_log;
static psm__log_level psm__my_log_level = PSM__LOG_VERBOSE;

static void
psm__default_log(const char *s)
{
#ifndef PSM_NO_STDIO
  fprintf(stderr, "%s", s);
#else
  (void)s;
#endif
}

PSM__DEF psm__log_level
psm__get_log_level(void)
{
  return psm__my_log_level;
}

PSM__DEF void
psm__set_log_level(psm__log_level level)
{
  psm__my_log_level = level;
}

PSM__DEF void
psm__debug_print(int level, const char *fmt, ...)
{
  if (level < psm__my_log_level || !psm__log_fn)
    return;

#ifndef PSM_NO_STDIO
  char    buf[256];
  va_list args;
  va_start(args, fmt);
  vsnprintf(buf, sizeof(buf), fmt, args);
  va_end(args);
#else
  const char *buf = fmt;
#endif

  psm__log_fn(buf);
}

PSMDEF csmLogFunction
csmGetLogFunction(void)
{
  return psm__log_fn;
}

PSMDEF void
csmSetLogFunction(csmLogFunction f)
{
  psm__log_fn = f;
}

PSMDEF int
csmGetLogLevel(void)
{
  return psm__my_log_level;
}

PSMDEF void
csmSetLogLevel(int level)
{
  psm__my_log_level = level;
}
/* ===== arena.c ===== */
/*
 * Purism Core: arena allocator
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */


#ifdef PSM_DEBUG_MALLOC
#  include <stdlib.h>

static void   **g_dbg_ptrs;
static psm_size g_dbg_count, g_dbg_cap;

static void *
psm__dbg_alloc(psm__u32 n)
{
  void *p = calloc(1, n ? n : 1);
  if (g_dbg_count == g_dbg_cap) {
    psm_size nc = g_dbg_cap ? g_dbg_cap * 2 : 256;
    void   **np = realloc(g_dbg_ptrs, nc * sizeof(void *));
    if (!np)
      return p;
    g_dbg_ptrs = np;
    g_dbg_cap = nc;
  }
  g_dbg_ptrs[g_dbg_count++] = p;
  return p;
}

void
psm__dbg_free_all(void)
{
  for (psm_size i = 0; i < g_dbg_count; i++)
    free(g_dbg_ptrs[i]);
  g_dbg_count = 0;
}
#endif /* PSM_DEBUG_MALLOC */

PSM__DEF void *
psm__arena_alloc(struct psm__arena *a, psm__u32 n)
{
  psm__u32 off = (a->off + 15) & ~15u;
  if (off < a->off) {
    a->overflow = true;
    return NULL;
  }
  psm__u32 end = off + n;
  if (end < off) {
    a->overflow = true;
    return NULL;
  }
  if (a->base == NULL) {
    a->off = end;
    return NULL;
  }
#ifdef PSM_DEBUG_MALLOC
  /* When fuzzing with ASAN, we use real malloc() in order to better catch OOB
     memory access */
  a->off = end;
  return psm__dbg_alloc(n);
#else
  if (end > a->cap) {
    a->overflow = true;
    return NULL;
  }
  void *p = a->base + off;
  a->off = end;
  return p;
#endif
}

PSM__DEF psm__u32
psm__arena_total(const struct psm__arena *a)
{
  return (a->off + 15) & ~15u;
}

PSM__DEF bool
psm__arena_ok(const struct psm__arena *a)
{
  return !a->overflow;
}
/* ===== math2.c ===== */
/*
 * Purism Core: math utilities
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <math.h>

PSM__DEF psm__f32
psm__signed_angle(const psm__f32 *v1, const psm__f32 *v2)
{
  psm__f32 angle1 = atan2f(v1[1], v1[0]);
  psm__f32 angle2 = atan2f(v2[1], v2[0]);
  return remainderf(angle1 - angle2, PSM__TWO_PI);
}
/* ===== moc3.c ===== */
/*
 * Purism Core: MOC3 file format parsing
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <stddef.h>
#include <string.h>

#define PSM__MOC3_MAGIC      "MOC3"
#define PSM__MOC3_MAGIC_SIZE 4

static bool
psm__is_le(void)
{
  unsigned int x = 1;
  return *((unsigned char *)&x) == 1;
}

static inline psm__u16
psm__bswap_16(psm__u16 x)
{
#if defined(__GNUC__) || defined(__clang__)
  return __builtin_bswap16(x);
#elif defined(_MSC_VER)
  return _byteswap_ushort(x);
#else
  return (x >> 8) | (x << 8);
#endif
}

static inline psm__u32
psm__bswap_32(psm__u32 x)
{
#if defined(__GNUC__) || defined(__clang__)
  return __builtin_bswap32(x);
#elif defined(_MSC_VER)
  return _byteswap_ulong(x);
#else
  return ((x >> 24) & 0x000000FF) | ((x >> 8) & 0x0000FF00) |
         ((x << 8) & 0x00FF0000) | ((x << 24) & 0xFF000000);
#endif
}

static void
psm__bswap_many_16(void *data, psm_size count)
{
  psm__u16 *p = (psm__u16 *)data;
  for (psm_size i = 0; i < count; i++)
    p[i] = psm__bswap_16(p[i]);
}

static void
psm__bswap_many_32(void *data, psm_size count)
{
  psm__u32 *p = (psm__u32 *)data;
  for (psm_size i = 0; i < count; i++)
    p[i] = psm__bswap_32(p[i]);
}

static int
psm__get_moc_version(csmMocVersion *v, const psm__u8 *p, psm_size n)
{
  if (!p || !n)
    return PSM__ERR_INVALID_PARAMETER;
  if (n < (PSM__MOC3_MAGIC_SIZE + 1))
    return PSM__ERR_FILE_CORRUPT;
  if (memcmp(p, PSM__MOC3_MAGIC, PSM__MOC3_MAGIC_SIZE) != 0)
    return PSM__ERR_FILE_UNRECOGNIZED;
  *v = (csmMocVersion)(*(p + PSM__MOC3_MAGIC_SIZE));
  return PSM__OK;
}

static void
psm__bswap_model_data(psm__u8 ver, struct psm__sections *src)
{
  struct psm__count_info *cnt = src->count_info;

  /* count_info already swapped above */
  psm__bswap_many_32(&src->canvas_info->pix_per_unit, 1);
  psm__bswap_many_32(&src->canvas_info->origin_x, 1);
  psm__bswap_many_32(&src->canvas_info->origin_y, 1);
  psm__bswap_many_32(&src->canvas_info->width, 1);
  psm__bswap_many_32(&src->canvas_info->height, 1);

#define psm__bswap_predicate(TYPE, MEMBER, COUNT_MEMBER) \
  if (sizeof(TYPE) == 4) \
    psm__bswap_many_32(src->MEMBER, cnt->COUNT_MEMBER); \
  else if (sizeof(TYPE) == 2) \
    psm__bswap_many_16(src->MEMBER, cnt->COUNT_MEMBER);

  PSM__SECTIONS_V30(psm__nop_predicate, psm__bswap_predicate)
  if (ver < csmMocVersion_33) goto done;
  PSM__SECTIONS_V33(psm__nop_predicate, psm__bswap_predicate)
  if (ver < csmMocVersion_42) goto done;
  PSM__SECTIONS_V42(psm__nop_predicate, psm__bswap_predicate)
  if (ver < csmMocVersion_50) goto done;
  PSM__SECTIONS_V50(psm__nop_predicate, psm__bswap_predicate)
  if (ver < csmMocVersion_53) goto done;

  PSM__SECTIONS_V53(psm__nop_predicate, psm__bswap_predicate)

done:;
#undef psm__bswap_predicate
}

static int
psm__init_moc3_sections(struct psm__moc3_data *moc3_data,
    psm__u8 *p, psm_size n, psm_size off, bool needs_bswap)
{
  struct psm__sections *ms = moc3_data->sections;

  psm__u32 *offsets = moc3_data->offsets;
  psm__u8   ver = moc3_data->header->version;
  psm_size  sec_count;

  if (ver >= csmMocVersion_53)
    sec_count =
        sizeof(((struct psm__moc3_data_v53 *)0)->offsets) / sizeof(psm__u32);
  else
    sec_count =
        sizeof(((struct psm__moc3_data_v52 *)0)->offsets) / sizeof(psm__u32);

  if (needs_bswap)
    psm__bswap_many_32(offsets, sec_count);

  psm_size ci_ints = PSM__COUNT_INFO_INTS(ver);

  PSM__FAILM((offsets[0] & 3) != 0,
      PSM__ERR_FILE_CORRUPT, "count_info misaligned");
  PSM__FAILM(offsets[0] > n || n - offsets[0] < ci_ints * sizeof(psm__i32),
      PSM__ERR_FILE_CORRUPT, "count_info offset oob");
  PSM__FAILM(offsets[0] < off,
      PSM__ERR_FILE_CORRUPT, "count_info before header end");

  if (needs_bswap)
    psm__bswap_many_32(p + offsets[0], ci_ints);

#ifndef PSM_FAST_AND_DANGEROUS
  {
    int err = psm__verify_count_info(ver,
        (const struct psm__count_info *)(p + offsets[0]));
    if (err != PSM__OK)
      return err;
  }
  if (psm__verify_sections(ms, p, offsets, n, off, ver, true) != PSM__OK)
    return PSM__ERR_FILE_CORRUPT;
#else
  psm__verify_sections(ms, p, offsets, n, off, ver, false);
#endif

  if (needs_bswap)
    psm__bswap_model_data(ver, ms);

  return PSM__OK;
}

static int
psm__has_moc_consistency(const psm__u8 *p, psm_size n)
{
  psm__u8 ver, endian_flag;
  bool    needs_bswap;

  psm_size  header_size, sec_count, off;
  psm__u32 *offsets;

  struct psm__count_info *cnt = NULL;

  int r = PSM__OK;

  PSM__FAILM(!p || !n, PSM__ERR_INVALID_PARAMETER, "buffer or size is NULL");

  PSM__FAILM(n < sizeof(struct psm__moc3_header),
      PSM__ERR_INVALID_DATA, "buffer too small");
  PSM__FAILM(memcmp(p, PSM__MOC3_MAGIC, PSM__MOC3_MAGIC_SIZE) != 0,
      PSM__ERR_FILE_UNRECOGNIZED, "unknown magic");

  ver = *(p + PSM__MOC3_MAGIC_SIZE);
  PSM__FAILM(ver == csmMocVersion_Unknown,
      PSM__ERR_FILE_CORRUPT, "invalid MOC3 version");
  PSM__FAILM(ver > csmMocVersion_53,
      PSM__ERR_FILE_CORRUPT, "unsupported MOC3 version");

  endian_flag = p[offsetof(struct psm__moc3_header, endian_flag)];
  needs_bswap = psm__is_le() != (endian_flag == 0);

  if (ver >= csmMocVersion_53) {
    header_size = sizeof(struct psm__moc3_data_v53);
    sec_count =
        sizeof(((struct psm__moc3_data_v53 *)0)->offsets) / sizeof(psm__u32);
    off = offsetof(struct psm__moc3_data_v53, sections);
  } else {
    header_size = sizeof(struct psm__moc3_data_v52);
    sec_count =
        sizeof(((struct psm__moc3_data_v52 *)0)->offsets) / sizeof(psm__u32);
    off = offsetof(struct psm__moc3_data_v52, sections);
  }

  PSM__FAILM(n < header_size,
      PSM__ERR_INVALID_DATA, "buffer too small for header");

  offsets = (psm__u32 *)(p + sizeof(struct psm__moc3_header));

  if (needs_bswap)
    psm__bswap_many_32(offsets, sec_count);

  /* Validate section offsets */
  for (psm_size i = 0; i < sec_count; i++) {
    if ((psm__i32)offsets[i] < 0 || offsets[i] > n) {
      PSM__LOGF("section offset [%u] invalid", (unsigned)i);
      r = PSM__ERR_FILE_CORRUPT;
      goto restore;
    }
  }

  /* Validate count_info section first */
  if ((offsets[0] & 3) != 0) {
    PSM__LOG("count_info misaligned");
    r = PSM__ERR_FILE_CORRUPT;
    goto restore;
  }
  if (offsets[0] > n ||
      n - offsets[0] < PSM__COUNT_INFO_INTS(ver) * sizeof(psm__i32)) {
    PSM__LOG("count_info out of bounds");
    r = PSM__ERR_FILE_CORRUPT;
    goto restore;
  }
  if (offsets[0] < off) {
    PSM__LOG("count_info before header end");
    r = PSM__ERR_FILE_CORRUPT;
    goto restore;
  }

  cnt = (struct psm__count_info *)(p + offsets[0]);
  if (needs_bswap)
    psm__bswap_many_32(cnt, PSM__COUNT_INFO_INTS(ver));

  r = psm__verify_count_info(ver, cnt);
  if (r != PSM__OK)
    goto restore;

  {
    struct psm__sections tmp;
    memset(&tmp, 0, sizeof tmp);
    r =
        psm__verify_sections(&tmp, (psm__u8 *)p, offsets, n, off, ver, true);
    if (r != PSM__OK)
      goto restore;

    if (needs_bswap)
      psm__bswap_model_data(ver, &tmp);
    r = psm__verify_idx(ver, &tmp);
    if (needs_bswap)
      psm__bswap_model_data(ver, &tmp);
  }

restore:
  if (needs_bswap) {
    if (cnt)
      psm__bswap_many_32(cnt, PSM__COUNT_INFO_INTS(ver));
    psm__bswap_many_32(offsets, sec_count);
  }

  return r;
}

static int
psm__revive_moc_in_place(struct psm__moc3_data **moc, psm__u8 *p, psm_size n)
{
  struct psm__moc3_data *moc3_data;

  psm_size off = 0;

  PSM__FAILM(!p || !n, PSM__ERR_INVALID_PARAMETER, "buffer or size is NULL");

  PSM__FAILM(n < sizeof(struct psm__moc3_header),
      PSM__ERR_INVALID_DATA, "buffer too small");
  PSM__FAILM(memcmp(p, PSM__MOC3_MAGIC, PSM__MOC3_MAGIC_SIZE) != 0,
      PSM__ERR_FILE_UNRECOGNIZED, "unknown magic");

  psm__u8 ver = *(p + PSM__MOC3_MAGIC_SIZE);
  PSM__FAILM(ver == csmMocVersion_Unknown,
      PSM__ERR_FILE_CORRUPT, "invalid MOC3 version");
  PSM__FAILM(ver > csmMocVersion_53,
      PSM__ERR_FILE_CORRUPT, "unsupported MOC3 version");

  psm__u8 endian_flag = p[offsetof(struct psm__moc3_header, endian_flag)];
  PSM__FAILM(endian_flag != 0 && endian_flag != 1,
      PSM__ERR_FILE_CORRUPT, "invalid endian flag");

  if (ver >= csmMocVersion_53) {
    off = offsetof(struct psm__moc3_data_v53, sections);
    PSM__FAILM(n < off + sizeof(struct psm__moc3_data),
        PSM__ERR_FILE_CORRUPT, "buffer too small");

    struct psm__moc3_data_v53 *layout = (struct psm__moc3_data_v53 *)p;
    moc3_data = &layout->sections.source;

    moc3_data->header = &layout->header;
    moc3_data->offsets = layout->offsets;
    moc3_data->sections = &layout->sections;
    layout->header.data = moc3_data;
  } else {
    off = offsetof(struct psm__moc3_data_v52, sections);
    PSM__FAILM(n < off + sizeof(struct psm__moc3_data),
        PSM__ERR_FILE_CORRUPT, "buffer too small");

    struct psm__moc3_data_v52 *layout = (struct psm__moc3_data_v52 *)p;
    moc3_data = &layout->sections.source;

    moc3_data->header = &layout->header;
    moc3_data->offsets = layout->offsets;
    moc3_data->sections = &layout->sections;
    layout->header.data = moc3_data;
  }

  bool is_le = psm__is_le();
  bool needs_bswap = is_le != (moc3_data->header->endian_flag == 0);
  if (needs_bswap)
    moc3_data->header->endian_flag = !is_le;

  PSM__FAILM(psm__init_moc3_sections(moc3_data, p, n, off,
                 needs_bswap) != PSM__OK,
      PSM__ERR_FILE_CORRUPT, "model data init failed");

  struct psm__sections   *src = moc3_data->sections;
  struct psm__count_info *cnt = src->count_info;

#ifndef PSM_FAST_AND_DANGEROUS
  PSM__FAILM(psm__verify_idx(ver, src) != PSM__OK,
      PSM__ERR_FILE_CORRUPT, "source index validation failed");
#endif

  psm__i32 count = cnt->art_meshes;
  if (count > 0) {
    psm__i32 *mb = src->art_mesh_src.mask_off;
    psm__i32 *mc = src->art_mesh_src.mask_len;
    psm__i32 *mi = src->mask_src.art_mesh_idx;

    if (!mb || !mc || !mi)
      goto skip_mask_processing;

    for (psm__i32 i = 0; i < count; i++) {
      psm__i32 m_cnt = mc[i];
      if (m_cnt <= 0 || !psm__check_offset_range(mb[i], m_cnt, cnt->masks))
        continue;
      psm__i32 *masks = &mi[mb[i]];
      psm__i32  valid = m_cnt;

      if (m_cnt > 1) {
        psm__i32 w = 0;
        for (psm__i32 j = 0; j < valid - 1; j++) {
          while (w < valid - 1 && masks[w] >= 0)
            w++;
          if (w < valid - 1) {
            memmove(&masks[w], &masks[w + 1],
                (valid - w - 1) * sizeof(psm__i32));
            valid--;
          }
        }
      }
      if (valid > 0 && masks[valid - 1] < 0)
        valid--;
      mc[i] = valid;
    }
  }
skip_mask_processing:

  count = cnt->parts;
  if (count > 0 && src->part_src.id && src->part_src.id_runtime) {
    for (psm__i32 i = 0; i < count; i++)
      src->part_src.id_runtime[i] =
          ((struct psm__id *)src->part_src.id)[i].data;
  }

  count = cnt->deformers;
  if (count > 0 && src->deformer_src.id && src->deformer_src.id_runtime) {
    for (psm__i32 i = 0; i < count; i++)
      src->deformer_src.id_runtime[i] =
          ((struct psm__id *)src->deformer_src.id)[i].data;
  }

  count = cnt->art_meshes;
  if (count > 0 && src->art_mesh_src.id && src->art_mesh_src.id_runtime) {
    for (psm__i32 i = 0; i < count; i++) {
      src->art_mesh_src.id_runtime[i] =
          ((struct psm__id *)src->art_mesh_src.id)[i].data;
      if (src->uv_src.xy && src->art_mesh_src.uv_off) {
        psm__i32 ub = src->art_mesh_src.uv_off[i];
        if (psm__check_idx(ub, cnt->uvs))
          src->art_mesh_src.uv_runtime[i] = &src->uv_src.xy[ub];
      }
      if (src->idx_src.idx && src->art_mesh_src.idx_off) {
        psm__i32 io = src->art_mesh_src.idx_off[i];
        if (psm__check_idx(io, cnt->idx))
          src->art_mesh_src.pos_idx_runtime[i] =
              &src->idx_src.idx[io];
      }
      if (src->mask_src.art_mesh_idx && src->art_mesh_src.mask_off) {
        psm__i32 mb2 = src->art_mesh_src.mask_off[i];
        if (psm__check_idx(mb2, cnt->masks))
          src->art_mesh_src.drawable_mask_runtime[i] =
              &src->mask_src.art_mesh_idx[mb2];
      }
    }
  }

  count = cnt->parameters;
  if (count > 0 && src->param_src.id && src->param_src.id_runtime) {
    for (psm__i32 i = 0; i < count; i++)
      src->param_src.id_runtime[i] =
          ((struct psm__id *)src->param_src.id)[i].data;
  }

  count = cnt->glues;
  if (count > 0 && src->glue_src.id_runtime && src->glue_src.id) {
    for (psm__i32 i = 0; i < count; i++)
      src->glue_src.id_runtime[i] =
          ((struct psm__id *)src->glue_src.id)[i].data;
  }

  if (ver >= csmMocVersion_53) {
    count = cnt->offscreens;
    if (count > 0 && src->offscreen_src.drawable_mask_runtime &&
        src->mask_src.art_mesh_idx && src->offscreen_src.mask_off) {
      for (psm__i32 i = 0; i < count; i++) {
        psm__i32 ob = src->offscreen_src.mask_off[i];
        if (psm__check_idx(ob, cnt->masks))
          src->offscreen_src.drawable_mask_runtime[i] =
              &src->mask_src.art_mesh_idx[ob];
      }
    }
  }

  if (src->canvas_info && (src->canvas_info->flag &
                              PSM__CANVAS_FLAG_Y_REVERSED) == 0) {
    psm__u16 *pos_idx = src->idx_src.idx;
    psm__i32 *idx_off = src->art_mesh_src.idx_off;
    psm__i32 *idx_cnt = src->art_mesh_src.idx_len;

    if (!pos_idx || !idx_off || !idx_cnt)
      goto skip_y_reversal;

    count = cnt->art_meshes;
    for (psm__i32 i = 0; i < count; i++) {
      psm__i32 ic = idx_cnt[i];
      psm__i32 ib = idx_off[i];
      if (ic <= 0 || !psm__check_offset_range(ib, ic, cnt->idx))
        continue;
      psm__u16 *idx = &pos_idx[ib];
      for (psm__i32 j = 0; j + 2 < ic; j += 3) {
        psm__u16 tmp = idx[j];
        idx[j] = idx[j + 2];
        idx[j + 2] = tmp;
      }
    }

    psm__f32 *uv_xy = src->uv_src.xy;
    psm__i32 *vert_cnt = src->art_mesh_src.vertex_count;
    psm__i32 *uv_off = src->art_mesh_src.uv_off;

    if (uv_xy && vert_cnt && uv_off) {
      for (psm__i32 i = 0; i < count; i++) {
        psm__i32 vc = vert_cnt[i];
        psm__i32 ub = uv_off[i];
        if (vc <= 0 || ub < 0 ||
            (psm__u32)ub + 2u * (psm__u32)vc > (psm__u32)cnt->uvs)
          continue;
        psm__f32 *uv = &uv_xy[ub];
        for (psm__i32 j = 0; j < vc; j++)
          uv[j * 2 + 1] = 1.0f - uv[j * 2 + 1];
      }
    }
  }
skip_y_reversal:

  *moc = moc3_data;
  return PSM__OK;
}

PSMDEF csmMocVersion
csmGetMocVersion(const void *address, unsigned int size)
{
  csmMocVersion ver;
  if (psm__get_moc_version(&ver, (const psm__u8 *)address, size) != PSM__OK)
    return csmMocVersion_Unknown;
  return ver;
}

PSMDEF int
csmHasMocConsistency(void *address, unsigned int size)
{
  return psm__has_moc_consistency((const psm__u8 *)address, size) == PSM__OK;
}

PSMDEF csmMoc *
csmReviveMocInPlace(void *address, unsigned int size)
{
  static bool first_call = true;
  if (first_call) {
    psm__debug_print(PSM__LOG_OFF,
        "Sakura2D Purism Core version " PSM__VERFMT " (compat " PSM__VERFMT
        ")\n",
        PSM__VERARG(PSM_TRUE_VERSION), PSM__VERARG(PSM_COMPAT_VERSION));
    first_call = false;
  }

  struct psm__moc3_data *moc;

  int err = psm__revive_moc_in_place(&moc, (psm__u8 *)address, size);

  /*
   * Record the outcome in the header's scratch space so a caller can ask
   * csmGetMocError(address) why a load failed, even though we return NULL.
   * Only safe once the buffer is known to hold a full header.
   */
  if (size >= sizeof(struct psm__moc3_header))
    ((struct psm__moc3_header *)address)->last_error = err;

  PSM__FAILM(err != PSM__OK, NULL, "could not revive MOC3");
  return (csmMoc *)address;
}

/* Purism Core extension: see PurismCore.h. */
PSMDEF csmError
csmGetMocError(const csmMoc *moc)
{
  if (!moc)
    return csmError_NoError;
  return (csmError)((const struct psm__moc3_header *)moc)->last_error;
}
/* ===== verify.c ===== */
/*
 * Purism Core: MOC3 load-time validation
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */


PSM__DEF int
psm__verify_count_info(psm__u8 ver, const struct psm__count_info *cnt)
{
  const psm__i32 *field = (const psm__i32 *)cnt;

  psm__i32 n = PSM__COUNT_INFO_INTS(ver);
  for (psm__i32 i = 0; i < n; i++)
    PSM__FAIL(field[i] < 0, PSM__ERR_FILE_CORRUPT,
        "count field[%d] = %d is negative", i, field[i]);

  PSM__FAILM(((psm__u32)cnt->warps + (psm__u32)cnt->rotations) !=
                 (psm__u32)cnt->deformers,
      PSM__ERR_FILE_CORRUPT, "deformer count mismatch");
  return PSM__OK;
}

/* Section bounds: 8-byte aligned, count sane,
 * [offset, offset+size) in [0,n). */
// clang-format off
#define psm__bounds_check_static(TYPE, COUNT, offsets, i, n) \
    if ((offsets[i] & 7) != 0) { \
      PSM__LOGF("section %d misaligned: offset=%u", i, (unsigned)offsets[i]); \
      return PSM__ERR_FILE_CORRUPT; \
    } \
    if ((COUNT) > (psm_size)-1 / sizeof(TYPE)) { \
      PSM__LOGF("section %d count overflow: count=%u", i, (unsigned)(COUNT)); \
      return PSM__ERR_FILE_CORRUPT; \
    } \
    psm_size _sz = sizeof(TYPE) * (COUNT); \
    if (offsets[i] > n || n - offsets[i] < _sz) { \
      PSM__LOGF("section %d out of bounds: offset=%u size=%u n=%u", i, (unsigned)offsets[i], (unsigned)_sz, (unsigned)n); \
      return PSM__ERR_FILE_CORRUPT; \
    }
#define psm__bounds_check_dynamic(TYPE, COUNT_MEMBER, offsets, i, n, cnt) \
    if ((offsets[i] & 7) != 0) { \
      PSM__LOGF("section %d misaligned: offset=%u", i, (unsigned)offsets[i]); \
      return PSM__ERR_FILE_CORRUPT; \
    } \
    if (cnt->COUNT_MEMBER < 0 || (psm_size)cnt->COUNT_MEMBER > (psm_size)-1 / sizeof(TYPE)) { \
      PSM__LOGF("section %d count invalid: count=%d", i, (int)cnt->COUNT_MEMBER); \
      return PSM__ERR_FILE_CORRUPT; \
    } \
    psm_size _sz = sizeof(TYPE) * (psm_size)cnt->COUNT_MEMBER; \
    if (offsets[i] > n || n - offsets[i] < _sz) { \
      PSM__LOGF("section %d out of bounds: offset=%u size=%u n=%u", i, (unsigned)offsets[i], (unsigned)_sz, (unsigned)n); \
      return PSM__ERR_FILE_CORRUPT; \
    }
// clang-format on

PSM__DEF int
psm__verify_sections(struct psm__sections *ms, psm__u8 *p,
    psm__u32 *offsets, psm_size n, psm_size off, psm__u8 ver, bool bounds)
{
  psm_size prev_end = off;
  psm_size ci_bytes = (psm_size)PSM__COUNT_INFO_INTS(ver) * sizeof(psm__i32);

  int i = 0;

  /*
   * count_info (section 0) is padded in-struct to 256 bytes but only ci_bytes
   * are on disk; use ci_bytes for the monotonic cursor. Dynamic sections that
   * are empty get a NULL pointer (the runtime relies on that).
   */
// clang-format off
#define psm__predicate_static(TYPE, MEMBER, COUNT) { \
    if (bounds) { \
      psm__bounds_check_static(TYPE, COUNT, offsets, i, n) \
      psm_size _ssz = (i == 0) ? ci_bytes : sizeof(TYPE) * (COUNT); \
      psm_size _static_end = offsets[i] + _ssz; \
      if (_static_end > prev_end) prev_end = _static_end; \
    } \
    ms->MEMBER = (TYPE *)(p + offsets[i]); \
    i++; \
  }
#define psm__predicate_dynamic(TYPE, MEMBER, COUNT_MEMBER) { \
    if (bounds) { \
      psm__bounds_check_dynamic(TYPE, COUNT_MEMBER, offsets, i, n, ms->count_info) \
      if (offsets[i] < prev_end) { \
        PSM__LOGF("section[%d] not monotonic: off=%u prev=%u sz=%u", \
            i, (unsigned)offsets[i], (unsigned)prev_end, (unsigned)_sz); \
        return PSM__ERR_FILE_CORRUPT; \
      } \
      prev_end = offsets[i] + _sz; \
      ms->MEMBER = _sz ? (TYPE *)(p + offsets[i]) : NULL; \
    } else { \
      ms->MEMBER = (TYPE *)(p + offsets[i]); \
    } \
    i++; \
  }
// clang-format on

  PSM__SECTIONS_V30(psm__predicate_static, psm__predicate_dynamic)
  if (ver < csmMocVersion_33) goto done;
  PSM__SECTIONS_V33(psm__predicate_static, psm__predicate_dynamic)
  if (ver < csmMocVersion_42) goto done;
  PSM__SECTIONS_V42(psm__predicate_static, psm__predicate_dynamic)
  if (ver < csmMocVersion_50) goto done;
  PSM__SECTIONS_V50(psm__predicate_static, psm__predicate_dynamic)
  if (ver < csmMocVersion_53) goto done;
  PSM__SECTIONS_V53(psm__predicate_static, psm__predicate_dynamic)

done:
#undef psm__predicate_static
#undef psm__predicate_dynamic
  return PSM__OK;
}

static psm__u32
psm__binding_keyform_count(const struct psm__sections *src,
    const struct psm__count_info *cnt, psm__i32 bi)
{
  const struct psm__binding_src *bs = &src->binding_src;

  if (bi < 0 || bi >= cnt->bindings || !bs->key_table_idx_off ||
      !bs->key_table_idx_len || !src->key_table_idx_src.idx ||
      !src->key_table_src.keys_len)
    return 1;

  psm__i32 off = bs->key_table_idx_off[bi];
  psm__i32 len = bs->key_table_idx_len[bi];
  if (len <= 0 || !psm__valid_range(off, len, cnt->key_table_idx))
    return 1;

  psm__u32 prod = 1;
  for (psm__i32 k = 0; k < len; k++) {
    psm__i32 kt = src->key_table_idx_src.idx[off + k];
    if (!psm__valid_idx(kt, cnt->key_tables))
      continue;
    psm__i32 kc = src->key_table_src.keys_len[kt];
    if (kc <= 1)
      continue;  /* 0/1-key tables do not extend the keyform grid */
    if (prod > 0x7FFFFFFFu / (psm__u32)kc)
      return 0x7FFFFFFF;  /* saturate: exceeds any valid key_len */
    prod *= (psm__u32)kc;
  }
  return prod;
}

static int
psm__verify_bs_windows(const struct psm__sections *src,
    const struct psm__blend_src *bs, psm__i32 shape_count,
    psm__i32        target_keyforms,
    const psm__i32 *key_pos_off, const psm__i32 *vertex_count,
    psm__i32 target_count, psm__i32 max_pos,
    const psm__i32 *key_mul_off, psm__i32 max_mul_colors,
    const psm__i32 *key_scr_off, psm__i32 max_scr_colors)
{
  const struct psm__count_info *cnt = src->count_info;

  const psm__i32 *kt_idx = src->blend_binding_src.key_table_idx;
  const psm__i32 *ks_off = src->blend_binding_src.key_bs_off;
  const psm__i32 *kc_len = src->blend_key_table_src.keys_len;

  if (!bs->target_idx || !bs->bs_binding_off || !bs->bs_binding_len ||
      !kt_idx || !ks_off || !kc_len)
    return PSM__OK;

  for (psm__i32 i = 0; i < shape_count; i++) {
    psm__i32 bo = bs->bs_binding_off[i];
    psm__i32 bn = bs->bs_binding_len[i];
    if (bn <= 0 || !psm__valid_range(bo, bn, cnt->blend_bindings))
      continue;

    /* pos is gated on the target existing and having vertices */
    psm__i32 vc = 0;
    bool     do_pos = false;
    if (key_pos_off && vertex_count) {
      psm__i32 ti = bs->target_idx[i];
      if (psm__valid_idx(ti, target_count) && vertex_count[ti] > 0) {
        vc = vertex_count[ti];
        do_pos = true;
      }
    }

    for (psm__i32 j = 0; j < bn; j++) {
      psm__i32 bb = bo + j;
      psm__i32 kti = kt_idx[bb];
      if (!psm__valid_idx(kti, cnt->blend_key_tables))
        continue;
      psm__i32 kc = kc_len[kti];
      if (kc < 1) kc = 1;   /* a 0/1-key binding still reads keyform index 0 */
      psm__i32 so = ks_off[bb];

      /* F4: the whole keyform window must fit */
      PSM__FAIL(!psm__valid_range(so, kc, target_keyforms),
          PSM__ERR_FILE_CORRUPT,
          "bs binding[%d] keyform window off=%d kc=%d > max=%d",
          bb, so, kc, target_keyforms);

      if (!do_pos && !key_mul_off && !key_scr_off)
        continue;   /* keyform-only target */

      for (psm__i32 k = 0; k < kc; k++) {
        psm__i32 ki = so + k;
        if (!psm__valid_idx(ki, target_keyforms))
          continue;
        if (do_pos) {
          psm__i32 po = key_pos_off[ki];
          PSM__FAIL(po < 0 || (psm__u32)po + 2u * (psm__u32)vc >
                                  (psm__u32)max_pos,
              PSM__ERR_FILE_CORRUPT,
              "bs pos window ki=%d po=%d vc=%d max=%d", ki, po, vc, max_pos);
        }
        if (key_mul_off) {
          psm__i32 ci = key_mul_off[ki];
          PSM__FAIL(ci >= max_mul_colors, PSM__ERR_FILE_CORRUPT,
              "bs mul color window ki=%d ci=%d max=%d", ki, ci, max_mul_colors);
        }
        if (key_scr_off) {
          psm__i32 ci = key_scr_off[ki];
          PSM__FAIL(ci >= max_scr_colors, PSM__ERR_FILE_CORRUPT,
              "bs scr color window ki=%d ci=%d max=%d", ki, ci, max_scr_colors);
        }
      }
    }
  }
  return PSM__OK;
}

static int
psm__verify_offscreen_window(const struct psm__sections *src,
    psm__i32 offscreen_count, psm__i32 offscreen_keyforms)
{
  const struct psm__count_info *cnt = src->count_info;

  const psm__i32 *owner = src->offscreen_src.owner_idx;
  const psm__i32 *binding_idx = src->part_src.binding_idx;
  const psm__i32 *keyform_off = src->part_src.keyform_off;
  const psm__i32 *key_idx = src->part_key_src.key_idx;
  const psm__i32 *mul_off = src->offscreen_key_src.key_mul_color_off;

  if (!owner || !binding_idx || !keyform_off || !key_idx)
    return PSM__OK;

  for (psm__i32 i = 0; i < offscreen_count; i++) {
    psm__i32 oi = owner[i];
    if (!psm__valid_idx(oi, cnt->parts))
      continue;
    psm__i32 kbi = keyform_off[oi];
    if (!psm__valid_idx(kbi, cnt->part_keyforms))
      continue;
    psm__i32 ki = key_idx[kbi];
    if (ki < 0)
      continue;  /* no offscreen keyforms for this surface */

    psm__i32 prod = (psm__i32)psm__binding_keyform_count(src, cnt,
        binding_idx[oi]);

    PSM__FAIL(!psm__valid_range(ki, prod, offscreen_keyforms),
        PSM__ERR_FILE_CORRUPT,
        "offscreen[%d] keyform window ki=%d prod=%d > max=%d",
        i, ki, prod, offscreen_keyforms);

    if (mul_off) {
      psm__i32 cb = mul_off[ki];
      /*
       * gather_offscreens indexes BOTH the mul and scr color pools with the
       * mul offset (it never reads key_scr_color_off), so cb+prod must fit
       * in both pools. cb < 0 means "no color" and is skipped at runtime.
       */
      PSM__FAIL(cb >= 0 &&
                    (!psm__valid_range(cb, prod, cnt->keyform_mul_colors) ||
                        !psm__valid_range(cb, prod, cnt->keyform_scr_colors)),
          PSM__ERR_FILE_CORRUPT,
          "offscreen[%d] color window cb=%d prod=%d > max(%d,%d)",
          i, cb, prod, cnt->keyform_mul_colors, cnt->keyform_scr_colors);
    }
  }
  return PSM__OK;
}

PSM__DEF int
psm__verify_idx(psm__u8 ver, const struct psm__sections *src)
{
  const struct psm__count_info *cnt = src->count_info;

// clang-format off
#define psm__check_nonnull(TYPE, MEMBER, COUNT_MEMBER) \
  if (cnt->COUNT_MEMBER > 0 && !src->MEMBER) { \
    PSM__LOGF("missing: %s (count=%d)", \
        #MEMBER, cnt->COUNT_MEMBER); \
    return PSM__ERR_FILE_CORRUPT; \
  }

  PSM__SECTIONS_V30(psm__nop_predicate, psm__check_nonnull)
  if (ver < csmMocVersion_33) goto done_nonnull;
  PSM__SECTIONS_V33(psm__nop_predicate, psm__check_nonnull)
  if (ver < csmMocVersion_42) goto done_nonnull;
  PSM__SECTIONS_V42(psm__nop_predicate, psm__check_nonnull)
  if (ver < csmMocVersion_50) goto done_nonnull;
  PSM__SECTIONS_V50(psm__nop_predicate, psm__check_nonnull)
  if (ver < csmMocVersion_53) goto done_nonnull;
  PSM__SECTIONS_V53(psm__nop_predicate, psm__check_nonnull)
done_nonnull:
#undef psm__check_nonnull

#define psm__model_check_index(arr, i, max) \
  PSM__FAIL((arr)[i] < 0 || (arr)[i] >= (max), \
      PSM__ERR_FILE_CORRUPT, \
      "invalid index: %s[%d]=%d (max=%d)", \
      #arr, i, (arr)[i], (max))

#define psm__model_check_index_or_neg1(arr, i, max) \
  PSM__FAIL((arr)[i] < -1 || (arr)[i] >= (max), \
      PSM__ERR_FILE_CORRUPT, \
      "invalid index: %s[%d]=%d (max=%d)", \
      #arr, i, (arr)[i], (max))

#define psm__model_check_range(begin_arr, count_arr, i, max) \
  PSM__FAIL((count_arr)[i] < 0 || \
      ((count_arr)[i] > 0 && ((begin_arr)[i] < 0 || \
          (psm__u32)(begin_arr)[i] + \
          (psm__u32)(count_arr)[i] > (psm__u32)(max))), \
      PSM__ERR_FILE_CORRUPT, \
      "invalid range: %s[%d] begin=%d count=%d (max=%d)", \
      #begin_arr, i, (begin_arr)[i], (count_arr)[i], (max))

  /*
   * Validate the keyform grid covers the full combo span. The combo
   * builder reaches keyform index product(key_counts)-1, so key_len must be
   * at least that product. With the per-object keyform_off+key_len<=*_keyforms
   * range check this bounds every reachable keyform index inside the object's
   * declared keyforms.
   */
#define psm__check_key_combo(obj_src, keyform_total, obj_len) \
  for (psm__i32 _i = 0; _i < (obj_len); _i++) { \
    psm__i32 _bi = (obj_src).binding_idx[_i]; \
    if (_bi < 0 || _bi >= cnt->bindings) continue; \
    psm__u32 _prod = psm__binding_keyform_count(src, cnt, _bi); \
    psm__i32 _kl = (obj_src).key_len[_i]; \
    PSM__FAIL(_kl < 0 || _prod > (psm__u32)_kl, \
        PSM__ERR_FILE_CORRUPT, \
        "%s[%d] combo span %u > key_len %d", \
        #obj_src, _i, _prod, _kl); \
    psm__i32 _mc = 1 << psm__clamp_i32( \
        src->binding_src.key_table_idx_len[_bi], 0, PSM__MAX_KEY_TABLES); \
    PSM__FAIL(!psm__valid_range((obj_src).keyform_off[_i], _mc, (keyform_total)), \
        PSM__ERR_FILE_CORRUPT, \
        "%s[%d] keyform_off=%d + max_blend=%d > total %d", #obj_src, _i, \
        (obj_src).keyform_off[_i], _mc, (keyform_total)); \
  }
// clang-format on

  /* Part sources */
  for (psm__i32 i = 0; i < cnt->parts; i++) {
    psm__model_check_index(src->part_src.binding_idx, i, cnt->bindings);
    psm__model_check_range(src->part_src.keyform_off,
        src->part_src.key_len, i, cnt->part_keyforms);
    psm__model_check_index_or_neg1(
        src->part_src.parent_part_idx, i, cnt->parts);
  }
  psm__check_key_combo(src->part_src, cnt->part_keyforms, cnt->parts);

  /* Deformer sources */
  for (psm__i32 i = 0; i < cnt->deformers; i++) {
    psm__model_check_index(src->deformer_src.binding_idx, i, cnt->bindings);
    psm__model_check_index_or_neg1(
        src->deformer_src.parent_part_idx, i, cnt->parts);
    psm__model_check_index_or_neg1(src->deformer_src.parent_deformer_idx,
        i, cnt->deformers);

    psm__i32 dtype = src->deformer_src.type[i];
    psm__i32 sidx = src->deformer_src.local_idx[i];
    switch (dtype) {
    case PSM__DEFORMER_TYPE_WARP:
      PSM__FAIL(sidx < 0 || sidx >= cnt->warps, PSM__ERR_FILE_CORRUPT,
          "deformer[%d].specific=%d (warp max=%d)", i, sidx, cnt->warps);
      break;
    case PSM__DEFORMER_TYPE_ROTATION:
      PSM__FAIL(sidx < 0 || sidx >= cnt->rotations, PSM__ERR_FILE_CORRUPT,
          "deformer[%d].specific=%d (rot max=%d)", i, sidx, cnt->rotations);
      break;
    default:
      PSM__LOGF("deformer[%d].type=%d invalid", i, dtype);
      return PSM__ERR_FILE_CORRUPT;
    }
  }

  /* Warp deformer sources */
  for (psm__i32 i = 0; i < cnt->warps; i++) {
    psm__model_check_index(src->warp_src.binding_idx,
        i, cnt->bindings);
    psm__model_check_range(src->warp_src.keyform_off,
        src->warp_src.key_len, i, cnt->warp_keyforms);
    psm__i32 row = src->warp_src.row[i];
    psm__i32 col = src->warp_src.col[i];
    psm__i32 vc = src->warp_src.vertex_count[i];
    PSM__FAIL(row <= 0 || col <= 0, PSM__ERR_FILE_CORRUPT,
        "warp[%d] grid row=%d col=%d", i, row, col);
    /* row/col are only bounded > 0, so compute in unsigned (well-defined
     * wraparound); row+1 in int overflows when row == INT_MAX. */
    psm__u32 expect = ((psm__u32)row + 1u) * ((psm__u32)col + 1u);
    PSM__FAIL((psm__u32)vc != expect, PSM__ERR_FILE_CORRUPT,
        "warp[%d] vert_count=%d expected=%u", i, vc, (unsigned)expect);
  }

  psm__check_key_combo(src->warp_src, cnt->warp_keyforms, cnt->warps);

  /* Warp deformer keyform positions */
  for (psm__i32 i = 0; i < cnt->warps; i++) {
    psm__i32 off = src->warp_src.keyform_off[i];
    psm__i32 count = src->warp_src.key_len[i];
    psm__i32 vc = src->warp_src.vertex_count[i];
    for (psm__i32 j = 0; j < count; j++) {
      psm__i32 po = src->warp_key_src.key_pos_off[off + j];
      PSM__FAIL(po < 0 || (psm__u32)po + 2u * (psm__u32)vc >
                              (psm__u32)cnt->keyform_pos,
          PSM__ERR_FILE_CORRUPT,
          "warp[%d] kf[%d] pos_off=%d vc=%d max=%d",
          i, j, po, vc, cnt->keyform_pos);
    }
  }

  /* Rotation deformer sources */
  for (psm__i32 i = 0; i < cnt->rotations; i++) {
    psm__model_check_index(src->rotation_src.binding_idx, i, cnt->bindings);
    psm__model_check_range(src->rotation_src.keyform_off,
        src->rotation_src.key_len, i, cnt->rotation_keyforms);
  }

  psm__check_key_combo(src->rotation_src, cnt->rotation_keyforms,
      cnt->rotations);

  /* Art mesh sources */
  for (psm__i32 i = 0; i < cnt->art_meshes; i++) {
    psm__model_check_index(src->art_mesh_src.binding_idx, i, cnt->bindings);
    psm__model_check_range(src->art_mesh_src.keyform_off,
        src->art_mesh_src.key_len, i, cnt->art_mesh_keyforms);
    psm__model_check_index_or_neg1(
        src->art_mesh_src.parent_part_idx, i, cnt->parts);
    psm__model_check_index_or_neg1(src->art_mesh_src.parent_deformer_idx,
        i, cnt->deformers);
    PSM__FAIL(src->art_mesh_src.vertex_count[i] < 0 ||
                  src->art_mesh_src.uv_off[i] < 0 ||
                  (psm__u32)src->art_mesh_src.uv_off[i] +
                          2u * (psm__u32)src->art_mesh_src.vertex_count[i] >
                      (psm__u32)cnt->uvs,
        PSM__ERR_FILE_CORRUPT,
        "art_mesh[%d]: UV [%d, +%d*2) oob (max %d)",
        i, src->art_mesh_src.uv_off[i],
        src->art_mesh_src.vertex_count[i], cnt->uvs);
    psm__model_check_range(src->art_mesh_src.idx_off,
        src->art_mesh_src.idx_len, i, cnt->idx);
    psm__model_check_range(src->art_mesh_src.mask_off,
        src->art_mesh_src.mask_len, i, cnt->masks);
  }

  psm__check_key_combo(src->art_mesh_src, cnt->art_mesh_keyforms,
      cnt->art_meshes);

  /*
   * Art mesh keyform position indices. Each keyform stores vc vertices
   * (2 floats each), so the readable span is [po, po + 2*vc); validate
   * the full span, not just the start index.
   */
  for (psm__i32 i = 0; i < cnt->art_meshes; i++) {
    psm__i32 off = src->art_mesh_src.keyform_off[i];
    psm__i32 count = src->art_mesh_src.key_len[i];
    psm__i32 vc = src->art_mesh_src.vertex_count[i];
    for (psm__i32 j = 0; j < count; j++) {
      psm__i32 po = src->art_mesh_key_src.key_pos_off[off + j];
      PSM__FAIL(po < 0 || (psm__u32)po + 2u * (psm__u32)vc >
                              (psm__u32)cnt->keyform_pos,
          PSM__ERR_FILE_CORRUPT,
          "art_mesh[%d] kf[%d] pos_off=%d vc=%d max=%d",
          i, j, po, vc, cnt->keyform_pos);
    }
  }

  /*
   * Triangle index VALUES. Each art mesh's index slice
   * [idx_off, idx_off+idx_len) contains
   * holds vertex indices into that mesh's own vertex array, so every index
   * must be < vertex_count. The library never dereferences these, but callers
   * receive them directly from csmGetDrawableIndices, so a malformed index
   * could cause an OOB read in a renderer.
   *
   * We reject at load time.
   */
  if (src->idx_src.idx) {
    for (psm__i32 i = 0; i < cnt->art_meshes; i++) {
      psm__i32 off = src->art_mesh_src.idx_off[i];
      psm__i32 len = src->art_mesh_src.idx_len[i];
      psm__i32 vc = src->art_mesh_src.vertex_count[i];
      for (psm__i32 j = 0; j < len; j++) {
        psm__i32 vi = (psm__i32)src->idx_src.idx[off + j];
        PSM__FAIL(vi >= vc, PSM__ERR_FILE_CORRUPT,
            "art_mesh[%d] index[%d]=%d >= vertex_count %d", i, j, vi, vc);
      }
    }
  }

  /* Parameter sources */
  for (psm__i32 i = 0; i < cnt->parameters; i++) {
    psm__model_check_range(src->param_src.key_table_off,
        src->param_src.key_table_len, i, cnt->key_tables);
  }

  /* Keyform binding sources */
  for (psm__i32 i = 0; i < cnt->bindings; i++) {
    psm__model_check_range(src->binding_src.key_table_idx_off,
        src->binding_src.key_table_idx_len, i, cnt->key_table_idx);
    psm__i32 pc = src->binding_src.key_table_idx_len[i];
    PSM__FAIL(pc < 0 || pc > PSM__MAX_KEY_TABLES, PSM__ERR_FILE_CORRUPT,
        "binding[%d] param_count=%d oob", i, pc);
  }

  /* Parameter binding index sources */
  for (psm__i32 i = 0; i < cnt->key_table_idx; i++) {
    psm__model_check_index(src->key_table_idx_src.idx, i, cnt->key_tables);
  }

  /* Parameter binding sources */
  for (psm__i32 i = 0; i < cnt->key_tables; i++) {
    psm__model_check_range(src->key_table_src.keys_off,
        src->key_table_src.keys_len, i, cnt->keys);
  }

  /* Drawable mask sources */
  for (psm__i32 i = 0; i < cnt->masks; i++) {
    psm__model_check_index_or_neg1(
        src->mask_src.art_mesh_idx, i, cnt->art_meshes);
  }

  psm__check_key_combo(src->glue_src, cnt->glue_keyforms, cnt->glues);

  /* Glue sources */
  for (psm__i32 i = 0; i < cnt->glues; i++) {
    psm__model_check_index(src->glue_src.binding_idx, i, cnt->bindings);
    psm__model_check_range(src->glue_src.keyform_off,
        src->glue_src.key_len, i, cnt->glue_keyforms);
    psm__model_check_index(src->glue_src.art_mesh_idx_a, i, cnt->art_meshes);
    psm__model_check_index(src->glue_src.art_mesh_idx_b, i, cnt->art_meshes);
    psm__model_check_range(src->glue_src.info_off,
        src->glue_src.info_len, i, cnt->glue_info);
    PSM__FAIL((src->glue_src.info_len[i] & 1) != 0, PSM__ERR_FILE_CORRUPT,
        "glue[%d]: odd info_len %d", i, src->glue_src.info_len[i]);
  }

  /* Glue position indices */
  if (src->glue_src.info_off && src->glue_src.info_len &&
      src->glue_src.art_mesh_idx_a && src->glue_src.art_mesh_idx_b &&
      src->glue_info_src.pos_idx && src->art_mesh_src.vertex_count) {
    for (psm__i32 i = 0; i < cnt->glues; i++) {
      psm__i32 m0 = src->glue_src.art_mesh_idx_a[i];
      psm__i32 m1 = src->glue_src.art_mesh_idx_b[i];
      if (m0 < 0 || m0 >= cnt->art_meshes || m1 < 0 || m1 >= cnt->art_meshes)
        continue;
      psm__i32 vc0 = src->art_mesh_src.vertex_count[m0];
      psm__i32 vc1 = src->art_mesh_src.vertex_count[m1];
      psm__i32 ib = src->glue_src.info_off[i];
      psm__i32 ic = src->glue_src.info_len[i];
      if (ib < 0 || ic <= 0 || ib + ic > cnt->glue_info)
        continue;
      for (psm__i32 j = 0; j < ic; j += 2) {
        psm__u16 p0 = src->glue_info_src.pos_idx[ib + j];
        PSM__FAIL(p0 >= (psm__u16)vc0, PSM__ERR_FILE_CORRUPT,
            "glue[%d] pos_idx[%d]=%u OOB (vc=%d)", i, j, p0, vc0);
        if (j + 1 < ic) {
          psm__u16 p1 = src->glue_info_src.pos_idx[ib + j + 1];
          PSM__FAIL(p1 >= (psm__u16)vc1, PSM__ERR_FILE_CORRUPT,
              "glue[%d] pos_idx[%d]=%u OOB (vc=%d)", i, j + 1, p1, vc1);
        }
      }
    }
  }

  /* Draw order group sources */
  for (psm__i32 i = 0; i < cnt->draw_groups; i++) {
    psm__model_check_range(src->draw_group_src.obj_off,
        src->draw_group_src.obj_len, i, cnt->draw_items);
  }

  /* Draw order group object sources */
  if (src->draw_group_obj_src.type && src->draw_group_obj_src.idx) {
    for (psm__i32 i = 0; i < cnt->draw_items; i++) {
      psm__model_check_index_or_neg1(src->draw_group_obj_src.self_group_idx,
          i, cnt->draw_groups);
      psm__i32 t = src->draw_group_obj_src.type[i];
      psm__i32 oi = src->draw_group_obj_src.idx[i];
      PSM__FAIL(t != 0 && t != 1, PSM__ERR_FILE_CORRUPT,
          "draw_item[%d]: bad type %d", i, t);
      psm__i32 max = t ? cnt->parts : cnt->art_meshes;
      PSM__FAIL(oi < 0 || oi >= max, PSM__ERR_FILE_CORRUPT,
          "draw_item[%d]: index %d OOB (type=%d max=%d)", i, oi, t, max);
      /*
       * only part items (type 1) recurse into a child group via
       * self_group_idx, and at runtime that index must be valid. -1 is
       * allowed for art-mesh items (never used) but not for parts.
       */
      PSM__FAIL(t == 1 && src->draw_group_obj_src.self_group_idx[i] < 0,
          PSM__ERR_FILE_CORRUPT,
          "draw_item[%d]: part item has no group", i);
    }
  }

  if (ver < csmMocVersion_42)
    goto done_ver;

  /* Warp deformer color indices */
  for (psm__i32 i = 0; i < cnt->warps; i++) {
    psm__model_check_range(src->warp_src.key_color_off,
        src->warp_src.key_len, i, cnt->keyform_mul_colors);
  }

  /* Rotation deformer color indices */
  for (psm__i32 i = 0; i < cnt->rotations; i++) {
    psm__model_check_range(src->rotation_src.key_color_off,
        src->rotation_src.key_len, i, cnt->keyform_mul_colors);
  }

  /* Art mesh color indices */
  for (psm__i32 i = 0; i < cnt->art_meshes; i++) {
    psm__model_check_range(src->art_mesh_src.key_color_off,
        src->art_mesh_src.key_len, i, cnt->keyform_mul_colors);
  }

  /* Parameter extension sources */
  for (psm__i32 i = 0; i < cnt->parameters; i++) {
    psm__model_check_range(src->param_keys_src.keys_off,
        src->param_keys_src.keys_len, i, cnt->keys);
  }

  /* Blend shape parameter binding sources */
  for (psm__i32 i = 0; i < cnt->blend_key_tables; i++) {
    psm__model_check_range(src->blend_key_table_src.keys_off,
        src->blend_key_table_src.keys_len, i, cnt->keys);
  }

  /* Parameter blend shape binding indices */
  for (psm__i32 i = 0; i < cnt->parameters; i++) {
    psm__model_check_range(src->param_src.blend_key_table_off,
        src->param_src.blend_key_table_len, i, cnt->blend_key_tables);
  }

  /* Blend shape keyform binding sources */
  for (psm__i32 i = 0; i < cnt->blend_bindings; i++) {
    psm__model_check_index(src->blend_binding_src.key_table_idx,
        i, cnt->blend_key_tables);
    psm__model_check_range(src->blend_binding_src.bs_constraint_idx_off,
        src->blend_binding_src.bs_constraint_idx_len,
        i, cnt->bs_constraint_idx);
  }

  /* Blend shape warp deformer sources */
  for (psm__i32 i = 0; i < cnt->bs_warps; i++) {
    psm__model_check_index(src->bs_warp_src.target_idx, i, cnt->warps);
    psm__model_check_range(src->bs_warp_src.bs_binding_off,
        src->bs_warp_src.bs_binding_len, i, cnt->blend_bindings);
  }

  /* Blend shape art mesh sources */
  for (psm__i32 i = 0; i < cnt->bs_art_meshes; i++) {
    psm__model_check_index(src->bs_art_mesh_src.target_idx,
        i, cnt->art_meshes);
    psm__model_check_range(src->bs_art_mesh_src.bs_binding_off,
        src->bs_art_mesh_src.bs_binding_len, i, cnt->blend_bindings);
  }

  /* Blend shape constraint index sources */
  for (psm__i32 i = 0; i < cnt->bs_constraint_idx; i++) {
    psm__model_check_index(src->blend_constraint_idx_src.constraint_idx,
        i, cnt->bs_constraints);
  }

  /* Blend shape constraint sources */
  for (psm__i32 i = 0; i < cnt->bs_constraints; i++) {
    psm__model_check_index(src->blend_constraint_src.parameter_idx,
        i, cnt->parameters);
    psm__model_check_range(src->blend_constraint_src.value_off,
        src->blend_constraint_src.value_len, i, cnt->bs_constraint_vals);
  }

  /* Blend shape keyform-window bounds (v4.2 targets) */
// clang-format off
#define PSM__VERIFY(call) \
    { if ((call) != PSM__OK) return PSM__ERR_FILE_CORRUPT; }
// clang-format on

  PSM__VERIFY(psm__verify_bs_windows(src, &src->bs_warp_src, cnt->bs_warps,
      cnt->warp_keyforms,
      src->warp_key_src.key_pos_off, src->warp_src.vertex_count,
      cnt->warps, cnt->keyform_pos,
      src->warp_key_src.key_mul_color_off, cnt->keyform_mul_colors,
      src->warp_key_src.key_scr_color_off, cnt->keyform_scr_colors));

  PSM__VERIFY(psm__verify_bs_windows(src, &src->bs_art_mesh_src,
      cnt->bs_art_meshes, cnt->art_mesh_keyforms,
      src->art_mesh_key_src.key_pos_off, src->art_mesh_src.vertex_count,
      cnt->art_meshes, cnt->keyform_pos,
      src->art_mesh_key_src.key_mul_color_off, cnt->keyform_mul_colors,
      src->art_mesh_key_src.key_scr_color_off, cnt->keyform_scr_colors));

  if (ver < csmMocVersion_50)
    goto done_ver;

  /* Blend shape part sources */
  for (psm__i32 i = 0; i < cnt->bs_parts; i++) {
    psm__model_check_index(src->bs_part_src.target_idx, i, cnt->parts);
    psm__model_check_range(src->bs_part_src.bs_binding_off,
        src->bs_part_src.bs_binding_len, i, cnt->blend_bindings);
  }

  /* Blend shape rotation deformer sources */
  for (psm__i32 i = 0; i < cnt->bs_rotations; i++) {
    psm__model_check_index(src->bs_rotation_src.target_idx, i, cnt->rotations);
    psm__model_check_range(src->bs_rotation_src.bs_binding_off,
        src->bs_rotation_src.bs_binding_len, i, cnt->blend_bindings);
  }

  /* Blend shape glue sources */
  for (psm__i32 i = 0; i < cnt->bs_glues; i++) {
    psm__model_check_index(src->bs_glue_src.target_idx, i, cnt->glues);
    psm__model_check_range(src->bs_glue_src.bs_binding_off,
        src->bs_glue_src.bs_binding_len, i, cnt->blend_bindings);
  }

  /* Blend shape keyform-window bounds (v5.0 targets) */
  PSM__VERIFY(psm__verify_bs_windows(src, &src->bs_part_src, cnt->bs_parts,
      cnt->part_keyforms, NULL, NULL, 0, 0, NULL, 0, NULL, 0));
  PSM__VERIFY(psm__verify_bs_windows(src, &src->bs_glue_src, cnt->bs_glues,
      cnt->glue_keyforms, NULL, NULL, 0, 0, NULL, 0, NULL, 0));

  PSM__VERIFY(psm__verify_bs_windows(src, &src->bs_rotation_src,
      cnt->bs_rotations, cnt->rotation_keyforms,
      NULL, NULL, 0, 0,
      src->rotation_key_src.key_mul_color_off, cnt->keyform_mul_colors,
      src->rotation_key_src.key_scr_color_off, cnt->keyform_scr_colors));

  if (ver < csmMocVersion_53)
    goto done_ver;

  /* Part offscreen rendering index */
  for (psm__i32 i = 0; i < cnt->parts; i++) {
    psm__model_check_index_or_neg1(src->part_src.offscreen_idx,
        i, cnt->offscreens);
  }

  /* Offscreen rendering sources */
  for (psm__i32 i = 0; i < cnt->offscreens; i++) {
    psm__model_check_index(src->offscreen_src.owner_idx, i, cnt->parts);
    psm__model_check_range(src->offscreen_src.mask_off,
        src->offscreen_src.mask_len, i, cnt->masks);
  }

  /* Blend shape offscreen rendering sources */
  for (psm__i32 i = 0; i < cnt->bs_offscreens; i++) {
    psm__model_check_index(src->bs_offscreen_src.target_idx,
        i, cnt->offscreens);
    psm__model_check_range(src->bs_offscreen_src.bs_binding_off,
        src->bs_offscreen_src.bs_binding_len, i, cnt->blend_bindings);
  }

  /* Blend shape keyform-window bounds (v5.3 targets) */
  PSM__VERIFY(psm__verify_bs_windows(src, &src->bs_offscreen_src,
      cnt->bs_offscreens, cnt->offscreen_keyforms,
      NULL, NULL, 0, 0,
      src->offscreen_key_src.key_mul_color_off, cnt->keyform_mul_colors,
      src->offscreen_key_src.key_scr_color_off, cnt->keyform_scr_colors));

  PSM__VERIFY(psm__verify_offscreen_window(src, cnt->offscreens,
      cnt->offscreen_keyforms));

#undef PSM__VERIFY

done_ver:
#undef psm__model_check_index
#undef psm__model_check_index_or_neg1
#undef psm__model_check_range

  return PSM__OK;
}
/* ===== model.c ===== */
/*
 * Purism Core: model initialization and accessors
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <math.h>
#include <string.h>

/*
 * Remap v6 extended blend modes to the closest v5 equivalents.
 */
static inline psm__i32
psm__remap_blend_mode(psm__i32 mode)
{
#if PSM_COMPAT_VERSION >= 0x06000000L
  return mode;
#else
  switch (mode) {
  case csmColorBlendType_Normal:
    return csmColorBlendType_Normal;
  /* We may not have these enums defined if built as a single-file bundle. */
  case 1: /* csmColorBlendType_AddCompatible */
  case 3: /* csmColorBlendType_Add */
  case 4: /* csmColorBlendType_AddGlow */
  case 9: /* csmColorBlendType_Lighten */
  case 10: /* csmColorBlendType_Screen */
  case 11: /* csmColorBlendType_ColorDodge */
    return csmColorBlendType_AddCompatible;
  case 2: /* csmColorBlendType_MultiplyCompatible */
  case 6: /* csmColorBlendType_Multiply */
  case 5: /* csmColorBlendType_Darken */
  case 7: /* csmColorBlendType_ColorBurn */
  case 8: /* csmColorBlendType_LinearBurn */
    return csmColorBlendType_MultiplyCompatible;
  default:
    return csmColorBlendType_Normal;
  }
#endif
}

/*
 * Number of key tables (parameter axes) bound to object idx_arr[i], clamped to
 * PSM__MAX_KEY_TABLES. Self-bounds-checking: returns 0 for an out-of-range
 * binding index, so it is safe to call from the size pass before validation.
 */
static inline psm__i32
psm__binding_param_count(const struct psm__sections *src,
    const struct psm__count_info *cnt, const psm__i32 *idx_arr, psm__i32 i)
{
  psm__i32 bi = idx_arr[i];
  if (!psm__valid_idx(bi, cnt->bindings))
    return 0;
  return psm__clamp_i32(src->binding_src.key_table_idx_len[bi],
      0, PSM__MAX_KEY_TABLES);
}

static inline psm__u32
psm__acc_add(struct psm__arena *a, psm__u32 acc, psm__u32 add)
{
  psm__u32 r = acc + add;
  if (r < acc) {
    a->overflow = 1;
    return acc;
  }
  return r;
}

static inline psm__u32
psm__pos_bytes(struct psm__arena *a, psm__i32 vc)
{
  psm__u32 v = (psm__u32)vc;
  if (v > (0xFFFFFFFFu - 15u) / (2u * (psm__u32)sizeof(psm__f32))) {
    a->overflow = 1;
    return 0;
  }
  return psm__align_to_16(2u * (psm__u32)sizeof(psm__f32) * v);
}

#define PSM__MAX_COMB(pc) ((psm__u32)1 << (pc))

/*
 * Per-object-type scratch sizing for the arena dry-run. Both use the
 * overflow-checked accumulator, so a bad count saturates the arena rather than
 * wrapping; a zero count or NULL source array contributes nothing.
 */
static psm__u32
psm__sum_max_comb(struct psm__arena *a, const struct psm__sections *src,
    struct psm__count_info *cnt, const psm__i32 *binding_idx, psm__i32 count)
{
  psm__u32 total = 0;
  if (count > 0 && binding_idx)
    for (psm__i32 i = 0; i < count; i++)
      total = psm__acc_add(a, total,
          PSM__MAX_COMB(psm__binding_param_count(src, cnt, binding_idx, i)));
  return total;
}

static psm__u32
psm__sum_pos_bytes(struct psm__arena *a, const psm__i32 *vertex_count,
    psm__i32 count)
{
  psm__u32 total = 0;
  if (count > 0 && vertex_count)
    for (psm__i32 i = 0; i < count; i++)
      if (vertex_count[i] > 0)
        total = psm__acc_add(a, total, psm__pos_bytes(a, vertex_count[i]));
  return total;
}

static struct psm__model *
psm__alloc_model(struct psm__arena *arena, psm__u8 ver,
    const struct psm__sections *src, struct psm__count_info *cnt)
{
// clang-format off
#define psm__alloc_field(field, T, n) do { \
    void *_p = psm__arena_alloc(arena, \
        psm__arena_safe_mul(arena, sizeof(T), (n))); \
    if (_p) (field) = (T *)_p; \
  } while (0)
#define psm__alloc_field_size(field, T, size) do { \
    void *_p = psm__arena_alloc(arena, (size)); \
    if (_p) (field) = (T *)_p; \
  } while (0)

// clang-format on
  psm__u32 part_tmp_total = psm__sum_max_comb(arena, src, cnt,
      src->part_src.binding_idx, cnt->parts);

  psm__u32 warp_tmp_total = psm__sum_max_comb(arena, src, cnt,
      src->warp_src.binding_idx, cnt->warps);
  psm__u32 warp_pos_total = psm__sum_pos_bytes(arena,
      src->warp_src.vertex_count, cnt->warps);

  psm__u32 rot_tmp_total = psm__sum_max_comb(arena, src, cnt,
      src->rotation_src.binding_idx, cnt->rotations);

  psm__u32 am_tmp_total = psm__sum_max_comb(arena, src, cnt,
      src->art_mesh_src.binding_idx, cnt->art_meshes);
  psm__u32 am_pos_total = psm__sum_pos_bytes(arena,
      src->art_mesh_src.vertex_count, cnt->art_meshes);

  psm__u32 kb_ptr_total = 0, kb_idx_total = 0;
  if (cnt->bindings > 0 && src->binding_src.key_table_idx_len) {
    for (psm__i32 i = 0; i < cnt->bindings; i++) {
      psm__i32 pc = psm__clamp_i32(src->binding_src.key_table_idx_len[i],
          0, PSM__MAX_KEY_TABLES);
      kb_ptr_total = psm__acc_add(arena, kb_ptr_total, (psm__u32)pc);
      kb_idx_total = psm__acc_add(arena, kb_idx_total, PSM__MAX_COMB(pc));
    }
  }

  psm__u32 glue_tmp_total = psm__sum_max_comb(arena, src, cnt,
      src->glue_src.binding_idx, cnt->glues);

  psm__i32 do_max_count = 0, do_max_level = 0;
  if (cnt->draw_groups > 0 && src->draw_group_src.obj_len &&
      src->draw_group_src.max_order && src->draw_group_src.min_order) {
    for (psm__i32 i = 0; i < cnt->draw_groups; i++) {
      psm__i32 c = src->draw_group_src.obj_len[i];
      psm__i32 mx = src->draw_group_src.max_order[i];
      psm__i32 mn = src->draw_group_src.min_order[i];
      psm__i32 lvl = psm__safe_order_level(mx, mn);
      if (c > 0 && c > do_max_count)
        do_max_count = c;
      if (lvl > 0 && lvl > do_max_level)
        do_max_level = lvl;
    }
  }

  psm__u32 bs_constr_ptrs_total = 0;
  if (ver >= csmMocVersion_42 &&
      src->blend_binding_src.bs_constraint_idx_len) {
    for (psm__i32 i = 0; i < cnt->blend_bindings; i++) {
      bs_constr_ptrs_total = psm__acc_add(arena, bs_constr_ptrs_total,
          (psm__u32)src->blend_binding_src.bs_constraint_idx_len[i]);
    }
  }

  psm__u32 os_tmp_total = 0;
  if (ver >= csmMocVersion_53 && src->offscreen_src.owner_idx &&
      src->part_src.binding_idx) {
    for (psm__i32 i = 0; i < cnt->offscreens; i++) {
      psm__i32 oi = src->offscreen_src.owner_idx[i];
      if (!psm__valid_idx(oi, cnt->parts)) continue;
      psm__i32 pc = psm__binding_param_count(src, cnt,
          src->part_src.binding_idx, oi);
      os_tmp_total = psm__acc_add(arena, os_tmp_total, PSM__MAX_COMB(pc));
    }
  }

  /* Model struct - must be allocated first */
  struct psm__model dummy, *m, *m2 = NULL;
  m2 = PSM__ARENA_NEW(arena, struct psm__model, 1);
  if (!m2) {
    memset(&dummy, 0, sizeof(dummy));
    m = &dummy;
  } else {
    m = m2;
  }

  /* Parts */
  psm__alloc_field(m->parts.items, struct psm__part, cnt->parts);
  psm__alloc_field(m->parts.bindings, struct psm__binding *, cnt->parts);
  psm__alloc_field(m->parts.opacity, psm__f32, cnt->parts);
  psm__alloc_field(m->parts.draw_order, psm__i32, cnt->parts);
  psm__alloc_field(m->parts.input_opacity, psm__f32, cnt->parts);
  psm__alloc_field(m->parts.enable, bool, cnt->parts);
  if (ver < csmMocVersion_53) {
    psm__alloc_field(m->parts.offscreen_src_idx, psm__i32, cnt->parts);
  }
  struct psm__part_keydata *pk = &m->parts.keydata;
  psm__alloc_field(pk->interp.max_blend, psm__i32, cnt->parts);
  psm__alloc_field(pk->interp.tmp, psm__f32, part_tmp_total);
  psm__alloc_field(pk->interp.blend_count, psm__i32, cnt->parts);
  psm__alloc_field(pk->interp.weights, psm__f32, part_tmp_total);
  psm__alloc_field(pk->draw_order, psm__f32, part_tmp_total);

  /* Deformers (generic) */
  psm__alloc_field(m->deformers.nodes,
      struct psm__deformer_node, cnt->deformers);
  psm__alloc_field(m->deformers.enable, bool, cnt->deformers);
  psm__alloc_field(m->deformers.opacity, psm__f32, cnt->deformers);
  psm__alloc_field(m->deformers.scale, psm__f32, cnt->deformers);
  psm__alloc_field(m->deformers.mul_color, psm__f32, 4 * cnt->deformers);
  psm__alloc_field(m->deformers.scr_color, psm__f32, 4 * cnt->deformers);

  /* Warp deformers */
  struct psm__warps        *w = &m->deformers.warps;
  struct psm__warp_keydata *wk = &w->keydata;
  psm__alloc_field(w->items, struct psm__warp, cnt->warps);
  psm__alloc_field(w->bindings, struct psm__binding *, cnt->warps);
  psm__alloc_field(w->enable, bool, cnt->warps);
  psm__alloc_field(w->opacity, psm__f32, cnt->warps);
  psm__alloc_field(w->pos, psm__f32 *, cnt->warps);
  psm__f32 *warp_pos_data = PSM__ARENA_NEW_SIZE(
      arena, psm__f32, warp_pos_total);
  psm__alloc_field(w->mul_color, psm__f32, 4 * cnt->warps);
  psm__alloc_field(w->scr_color, psm__f32, 4 * cnt->warps);
  psm__alloc_field(wk->interp.max_blend, psm__i32, cnt->warps);
  psm__alloc_field(wk->interp.tmp, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->interp.blend_count, psm__i32, cnt->warps);
  psm__alloc_field(wk->interp.weights, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->opacity, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->pos, psm__f32 *, PSM__PTR_FLOAT_RATIO * warp_tmp_total);
  psm__alloc_field(wk->mul_color.r, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->mul_color.g, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->mul_color.b, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->scr_color.r, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->scr_color.g, psm__f32, warp_tmp_total);
  psm__alloc_field(wk->scr_color.b, psm__f32, warp_tmp_total);

  /* Rotation deformers */
  struct psm__rotations        *r = &m->deformers.rotations;
  struct psm__rotation_keydata *rk = &r->keydata;
  psm__alloc_field(r->items, struct psm__rotation, cnt->rotations);
  psm__alloc_field(r->bindings, struct psm__binding *, cnt->rotations);
  psm__alloc_field(r->enable, bool, cnt->rotations);
  psm__alloc_field(r->opacity, psm__f32, cnt->rotations);
  psm__alloc_field(r->scale, psm__f32, cnt->rotations);
  psm__alloc_field(r->origin_x, psm__f32, cnt->rotations);
  psm__alloc_field(r->origin_y, psm__f32, cnt->rotations);
  psm__alloc_field(r->angle, psm__f32, cnt->rotations);
  psm__alloc_field(r->reflect_x, psm__i32, cnt->rotations);
  psm__alloc_field(r->reflect_y, psm__i32, cnt->rotations);
  psm__alloc_field(r->mul_color, psm__f32, 4 * cnt->rotations);
  psm__alloc_field(r->scr_color, psm__f32, 4 * cnt->rotations);
  psm__alloc_field(rk->interp.max_blend, psm__i32, cnt->rotations);
  psm__alloc_field(rk->interp.tmp, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->interp.blend_count, psm__i32, cnt->rotations);
  psm__alloc_field(rk->interp.weights, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->opacity, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->angle, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->origin_x, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->origin_y, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->scale, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->mul_color.r, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->mul_color.g, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->mul_color.b, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->scr_color.r, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->scr_color.g, psm__f32, rot_tmp_total);
  psm__alloc_field(rk->scr_color.b, psm__f32, rot_tmp_total);

  /* Art meshes */
  struct psm__art_meshes       *am = &m->art_meshes;
  struct psm__art_mesh_keydata *ak = &am->keydata;
  psm__alloc_field(am->meshes, struct psm__art_mesh, cnt->art_meshes);
  psm__alloc_field(am->bindings, struct psm__binding *, cnt->art_meshes);
  psm__alloc_field(am->enable, bool, cnt->art_meshes);
  psm__alloc_field(am->const_flags, psm__u8, cnt->art_meshes);
  psm__alloc_field(am->change_flags, psm__u8, cnt->art_meshes);
  psm__alloc_field(am->blend_mode, psm__i32, cnt->art_meshes);
  psm__alloc_field(am->draw_order, psm__i32, cnt->art_meshes);
  psm__alloc_field(am->pos, psm__f32 *, cnt->art_meshes);
  psm__f32 *am_pos_data = PSM__ARENA_NEW_SIZE(arena, psm__f32, am_pos_total);
  psm__alloc_field(am->opacity, psm__f32, cnt->art_meshes);
  psm__alloc_field(am->mul_color, psm__f32, 4 * cnt->art_meshes);
  psm__alloc_field(am->scr_color, psm__f32, 4 * cnt->art_meshes);
  psm__alloc_field(am->last_render_order, psm__i32, cnt->art_meshes);
  psm__alloc_field(am->last_draw_order, psm__i32, cnt->art_meshes);
  psm__alloc_field(am->last_opacity, psm__f32, cnt->art_meshes);
  psm__alloc_field(am->last_mul_color, psm__f32, 4 * cnt->art_meshes);
  psm__alloc_field(am->last_scr_color, psm__f32, 4 * cnt->art_meshes);
  psm__alloc_field(ak->interp.max_blend, psm__i32, cnt->art_meshes);
  psm__alloc_field(ak->interp.tmp, psm__f32, am_tmp_total);
  psm__alloc_field(ak->interp.blend_count, psm__i32, cnt->art_meshes);
  psm__alloc_field(ak->interp.weights, psm__f32, am_tmp_total);
  psm__alloc_field(ak->opacity, psm__f32, am_tmp_total);
  psm__alloc_field(ak->draw_order, psm__f32, am_tmp_total);
  psm__alloc_field(ak->pos, psm__f32 *, PSM__PTR_FLOAT_RATIO * am_tmp_total);
  psm__alloc_field(ak->mul_color.r, psm__f32, am_tmp_total);
  psm__alloc_field(ak->mul_color.g, psm__f32, am_tmp_total);
  psm__alloc_field(ak->mul_color.b, psm__f32, am_tmp_total);
  psm__alloc_field(ak->scr_color.r, psm__f32, am_tmp_total);
  psm__alloc_field(ak->scr_color.g, psm__f32, am_tmp_total);
  psm__alloc_field(ak->scr_color.b, psm__f32, am_tmp_total);

  /* Parameters */
  psm__alloc_field(m->params.items, struct psm__param, cnt->parameters);
  if (ver < csmMocVersion_42) {
    psm__alloc_field(m->params.type, psm__i32, cnt->parameters);
  }
  psm__alloc_field(m->params.input_value, psm__f32, cnt->parameters);

  /* Parameter bindings */
  psm__alloc_field(m->key_tables.items, struct psm__key_table,
      cnt->key_tables);

  /* Keyform bindings */
  psm__alloc_field(m->bindings.items, struct psm__binding, cnt->bindings);
  struct psm__key_table **kb_ptrs = PSM__ARENA_NEW(
      arena, struct psm__key_table *, kb_ptr_total);
  psm__i32 *kb_indices = PSM__ARENA_NEW(arena, psm__i32, kb_idx_total);
  psm__f32 *kb_weights = PSM__ARENA_NEW(arena, psm__f32, kb_idx_total);

  /* BlendShape (v4.2+) */
  struct psm__blend_constraint **bs_constr_ptrs = NULL;
  if (ver >= csmMocVersion_42) {
    psm__alloc_field(m->blend_constraints.items,
        struct psm__blend_constraint, cnt->bs_constraints);
    psm__alloc_field(m->blend_key_tables.items, struct psm__blend_key_table,
        cnt->blend_key_tables);
    psm__alloc_field(m->blend_bindings.items, struct psm__blend_binding,
        cnt->blend_bindings);
    bs_constr_ptrs = PSM__ARENA_NEW(arena, struct psm__blend_constraint *,
        bs_constr_ptrs_total);
    psm__alloc_field(m->bs_warps.items,
        struct psm__blend_shape, cnt->bs_warps);
    psm__alloc_field(m->bs_art_meshes.items,
        struct psm__blend_shape, cnt->bs_art_meshes);

    if (ver >= csmMocVersion_50) {
      psm__alloc_field(m->bs_parts.items,
          struct psm__blend_shape, cnt->bs_parts);
      psm__alloc_field(m->bs_rotations.items,
          struct psm__blend_shape, cnt->bs_rotations);
      psm__alloc_field(m->bs_glues.items,
          struct psm__blend_shape, cnt->bs_glues);
    }

    if (ver >= csmMocVersion_53) {
      psm__alloc_field(m->bs_offscreens.items,
          struct psm__blend_shape, cnt->bs_offscreens);
    }
  }

  /* Draw order groups */
  struct psm__draw_item *do_items = NULL;
  if (cnt->draw_groups > 0 && src->draw_group_src.obj_len &&
      src->draw_group_src.max_order && src->draw_group_src.min_order) {
    psm__alloc_field(m->draw_groups.groups, struct psm__draw_group,
        cnt->draw_groups);
    do_items = PSM__ARENA_NEW(arena, struct psm__draw_item, cnt->draw_items);
    psm__alloc_field(m->draw_groups.sort.first, psm__i32, do_max_level);
    psm__alloc_field(m->draw_groups.sort.next, psm__i32, do_max_count);
    psm__alloc_field(m->draw_groups.sort.last, psm__i32, do_max_level);
  }

  /* Glues */
  struct psm__glue_keydata *gk = &m->glues.keydata;
  psm__alloc_field(m->glues.items, struct psm__glue, cnt->glues);
  psm__alloc_field(m->glues.bindings, struct psm__binding *, cnt->glues);
  psm__alloc_field(gk->interp.max_blend, psm__i32, cnt->glues);
  psm__alloc_field(gk->interp.blend_count, psm__i32, cnt->glues);
  psm__alloc_field(gk->interp.tmp, psm__f32, glue_tmp_total);
  psm__alloc_field(gk->interp.weights, psm__f32, glue_tmp_total);
  psm__alloc_field(gk->intensity, psm__f32, glue_tmp_total);
  psm__alloc_field(m->glues.intensity, psm__f32, glue_tmp_total);

  /* Offscreen rendering (v5.3+) */
  if (ver >= csmMocVersion_53) {
    struct psm__offscreens        *os = &m->offscreens;
    struct psm__offscreen_keydata *ok = &os->keydata;
    psm__alloc_field(os->surfaces, struct psm__offscreen, cnt->offscreens);
    psm__alloc_field(os->opacity, psm__f32, cnt->offscreens);
    psm__alloc_field(os->enable, bool, cnt->offscreens);
    psm__alloc_field(os->mul_color, psm__f32, 4 * cnt->offscreens);
    psm__alloc_field(os->scr_color, psm__f32, 4 * cnt->offscreens);
    psm__alloc_field(ok->interp.max_blend, psm__i32, cnt->offscreens);
    psm__alloc_field(ok->interp.tmp, psm__f32, os_tmp_total);
    psm__alloc_field(ok->interp.blend_count, psm__i32, cnt->offscreens);
    psm__alloc_field(ok->interp.weights, psm__f32, os_tmp_total);
    psm__alloc_field(ok->opacity, psm__f32, os_tmp_total);
    psm__alloc_field(ok->mul_color.r, psm__f32, os_tmp_total);
    psm__alloc_field(ok->mul_color.g, psm__f32, os_tmp_total);
    psm__alloc_field(ok->mul_color.b, psm__f32, os_tmp_total);
    psm__alloc_field(ok->scr_color.r, psm__f32, os_tmp_total);
    psm__alloc_field(ok->scr_color.g, psm__f32, os_tmp_total);
    psm__alloc_field(ok->scr_color.b, psm__f32, os_tmp_total);
  }

  /* Parameter extensions */
  if (ver < csmMocVersion_42 || !src->param_keys_src.key_runtime) {
    psm__alloc_field(m->param_keys.keys, psm__f32 *, cnt->parameters);
    psm__alloc_field(m->param_keys.key_counts, psm__i32, cnt->parameters);
  }

  /* Render orders */
  psm__i32 ro_count = cnt->art_meshes +
                      (ver >= csmMocVersion_53 ? cnt->offscreens : 0);
  psm__alloc_field(m->render_order, psm__i32, ro_count);

#undef psm__alloc_field
#undef psm__alloc_field_size

  /* In dry-run mode, return NULL */
  if (arena->base == NULL)
    return NULL;

  m->parts.count = cnt->parts;
  m->parts.keydata.interp.object_count = cnt->parts;
  if (ver >= csmMocVersion_53)
    m->parts.offscreen_src_idx = src->part_src.offscreen_idx;

  m->deformers.count = cnt->deformers;
  w->count = cnt->warps;
  wk->interp.object_count = cnt->warps;
  r->count = cnt->rotations;
  rk->interp.object_count = cnt->rotations;

  am->count = cnt->art_meshes;
  ak->interp.object_count = cnt->art_meshes;

  m->params.count = cnt->parameters;
  if (ver >= csmMocVersion_42)
    m->params.type = src->param_src.type;

  m->key_tables.count = cnt->key_tables;
  m->bindings.count = cnt->bindings;

  m->draw_groups.count = cnt->draw_groups;
  m->glues.count = cnt->glues;
  gk->interp.object_count = cnt->glues;

  if (ver >= csmMocVersion_42) {
    m->blend_constraints.count = cnt->bs_constraints;
    if (src->blend_key_table_src.keys_len &&
        src->blend_key_table_src.keys_off) {
      m->blend_key_tables.count = cnt->blend_key_tables;
    }
    if (src->blend_binding_src.key_table_idx &&
        src->blend_binding_src.key_bs_off) {
      m->blend_bindings.count = cnt->blend_bindings;
    }
    m->bs_warps.count = cnt->bs_warps;
    m->bs_art_meshes.count = cnt->bs_art_meshes;

    if (ver >= csmMocVersion_50) {
      m->bs_parts.count = cnt->bs_parts;
      m->bs_rotations.count = cnt->bs_rotations;
      m->bs_glues.count = cnt->bs_glues;
    }

    if (ver >= csmMocVersion_53 && src->bs_offscreen_src.target_idx &&
        src->bs_offscreen_src.bs_binding_len &&
        src->bs_offscreen_src.bs_binding_off) {
      m->bs_offscreens.count = cnt->bs_offscreens;
    }
  }

  if (ver >= csmMocVersion_53) {
    m->offscreens.count = cnt->offscreens;
    m->offscreens.keydata.interp.object_count = cnt->offscreens;
  }

  if (ver >= csmMocVersion_42 && src->param_keys_src.key_runtime) {
    m->param_keys.keys = (psm__f32 **)src->param_keys_src.key_runtime;
    m->param_keys.key_counts = src->param_keys_src.keys_len;
  }

  /* Set up warp position pointers */
  {
    psm__f32 *pos = warp_pos_data;
    for (psm__i32 i = 0; i < cnt->warps; i++) {
      w->pos[i] = pos;
      psm__i32 vc = src->warp_src.vertex_count[i];
      pos = (psm__f32 *)((psm__u8 *)pos +
                         psm__align_to_16(2 * sizeof(psm__f32) * vc));
    }
  }

  /* Set up art mesh position pointers */
  {
    psm__f32 *pos = am_pos_data;
    for (psm__i32 i = 0; i < cnt->art_meshes; i++) {
      am->pos[i] = pos;
      psm__i32 vc = src->art_mesh_src.vertex_count[i];
      pos = (psm__f32 *)((psm__u8 *)pos +
                         psm__align_to_16(2 * sizeof(psm__f32) * vc));
    }
  }

  /* Set up keyform binding internal pointers */
  if (m && kb_ptrs && m->bindings.items) {
    struct psm__key_table **bp = kb_ptrs;

    psm__i32 *ki = kb_indices;
    psm__f32 *kwt = kb_weights;

    for (psm__i32 i = 0; i < cnt->bindings; i++) {
      struct psm__binding *kc = &m->bindings.items[i];

      psm__i32 bc = src->binding_src.key_table_idx_len[i];
      psm__u32 mc = 1 << bc;

      kc->key_tables = bp;
      kc->keyform_idx = ki;
      kc->weights = kwt;
      kc->key_table_len = bc;
      kc->max_blend = mc;

      bp += bc;
      ki += mc;
      kwt += mc;
    }
  }

  /* Set up draw order group items */
  if (cnt->draw_groups > 0 && do_items && src->draw_group_src.obj_len) {
    struct psm__draw_item *ip = do_items;
    for (psm__i32 i = 0; i < cnt->draw_groups; i++) {
      struct psm__draw_group *grp = &m->draw_groups.groups[i];

      psm__i32 count = src->draw_group_src.obj_len[i];
      if (!psm__valid_range(ip - do_items, count, cnt->draw_items)) {
        grp->items = NULL;
        grp->count = 0;
        continue;
      }
      grp->items = ip;
      grp->count = count;
      if (count > 0) ip += count;
    }
  }

  /* Set up blend shape constraint pointers */
  if (ver >= csmMocVersion_42 && bs_constr_ptrs &&
      src->blend_binding_src.bs_constraint_idx_len) {
    struct psm__blend_constraint **ptr = bs_constr_ptrs;
    for (psm__i32 i = 0; i < cnt->blend_bindings; i++) {
      struct psm__blend_binding *bb = &m->blend_bindings.items[i];

      psm__i32 cc = src->blend_binding_src.bs_constraint_idx_len[i];
      bb->constraints = ptr;
      bb->constraint_count = cc;
      ptr += cc;
    }
  }

  return m2;
}

static void
psm__init_mul_color(psm__f32 *buf, psm__i32 count)
{
  for (psm__i32 i = 0; i < count; i++) {
    buf[i * 4 + 0] = 1.0f;
    buf[i * 4 + 1] = 1.0f;
    buf[i * 4 + 2] = 1.0f;
    buf[i * 4 + 3] = 1.0f;
  }
}

static void
psm__init_scr_color(psm__f32 *buf, psm__i32 count)
{
  for (psm__i32 i = 0; i < count; i++)
    buf[i * 4 + 3] = 1.0f;
}

static int
psm__init_model_data(struct psm__model *m, const struct psm__moc3_data *moc)
{
  psm__u8 ver = moc->header->version;

  struct psm__sections   *ms = moc->sections;
  struct psm__count_info *cnt = ms->count_info;

  m->source = moc;
  m->last_error = moc->header->last_error;   /* inherit the revive outcome */
  m->y_reversed =
      (ms->canvas_info->flag & PSM__CANVAS_FLAG_Y_REVERSED) != 0;
  m->force_update = 1;

  /* Parameter bindings */
  /* keys_src.key has count cnt->keys, which can be 0 while key_tables > 0 */
  if (cnt->key_tables > 0 && ms->keys_src.key) {
    for (psm__i32 i = 0; i < cnt->key_tables; i++) {
      struct psm__key_table *c = &m->key_tables.items[i];

      psm__i32 key_cnt = ms->key_table_src.keys_len[i];
      psm__i32 keyform_off = ms->key_table_src.keys_off[i];
      c->key_count = key_cnt;
      c->out_of_range = 1;
      /* keys_off + keys_len <= cnt->keys proved by verify_idx */
      c->keys = &ms->keys_src.key[keyform_off];
    }
  }

  /*
   * Keyform bindings. The key_table_idx_off/len range and every
   * key_table_idx_src.idx entry are already proved valid by verify_idx,
   * so this just wires the pointers.
   */
  if (cnt->bindings > 0) {
    for (psm__i32 i = 0; i < cnt->bindings; i++) {
      struct psm__binding *kc = &m->bindings.items[i];
      psm__i32             bc = ms->binding_src.key_table_idx_len[i];
      psm__i32             kt_off = ms->binding_src.key_table_idx_off[i];

      kc->idx_dirty = 1;
      kc->weight_dirty = 1;
      kc->out_of_range = 1;

      if (!kc->key_tables)
        continue;

      for (psm__i32 j = 0; j < bc; j++) {
        psm__i32 idx = ms->key_table_idx_src.idx[kt_off + j];
        kc->key_tables[j] = &m->key_tables.items[idx];
      }
    }
  }

  /* Parts */
  if (cnt->parts > 0) {
    psm__i32 tmp_len = 0;
    for (psm__i32 i = 0; i < cnt->parts; i++) {
      struct psm__part *part = &m->parts.items[i];
      /* binding_idx valid by verify_idx */
      psm__i32             bi = ms->part_src.binding_idx[i];
      struct psm__binding *binding = &m->bindings.items[bi];

      part->binding = binding;
      m->parts.bindings[i] = binding;
      part->parent_part_idx = ms->part_src.parent_part_idx[i];
      part->local_enable = ms->part_src.enable[i];

      m->parts.input_opacity[i] = ms->part_src.visible[i] ? 1.0f : 0.0f;

      if (ver < csmMocVersion_53)
        m->parts.offscreen_src_idx[i] = -1;

      psm__i32 mc = binding->max_blend;
      m->parts.keydata.interp.max_blend[i] = mc;
      tmp_len += mc;
    }
    m->parts.keydata.interp.tmp_len = tmp_len;
  }

  /* Parameters */
  if (cnt->parameters > 0) {
    for (psm__i32 i = 0; i < cnt->parameters; i++) {
      struct psm__param *param = &m->params.items[i];

      if (ver >= csmMocVersion_42 && ms->param_src.type) {
        param->type = ms->param_src.type[i];
      } else {
        param->type = csmParameterType_Normal;
        if (ver < csmMocVersion_42)
          m->params.type[i] = csmParameterType_Normal;
      }

      param->range[0] = ms->param_src.minimum_value[i];
      param->range[1] = ms->param_src.maximum_value[i];
      param->range_length = param->range[1] - param->range[0];
      param->repeat = ms->param_src.repeat[i];

      psm__i32 dp = ms->param_src.decimal_places[i];
      param->snap_eps = powf(0.1f, (psm__f32)dp);
      param->interp_eps = param->snap_eps * 1.5f;

      psm__i32 kt_off = ms->param_src.key_table_off[i];
      psm__i32 kt_count = ms->param_src.key_table_len[i];

      {
        int rc = psm__valid_opt_range(kt_off, kt_count, cnt->key_tables);
        if (rc < 0)
          PSM__LOGF("param[%d]: binding range "
                    "[%d, %d) OOB (max %d)",
              i, kt_off, kt_off + kt_count,
              cnt->key_tables);
        if (rc == 1) {
          param->key_tables = &m->key_tables.items[kt_off];
          param->key_table_len = kt_count;
        } else {
          param->key_tables = NULL;
          param->key_table_len = 0;
        }
      }

      param->value = ms->param_src.default_value[i];
      m->params.input_value[i] = param->value;
      param->dirty = 1;

      param->blend_key_table_len = 0;
      param->blend_key_tables = NULL;
    }
  }

  /* Deformers */
  if (cnt->deformers > 0) {
    for (psm__i32 i = 0; i < cnt->deformers; i++) {
      struct psm__deformer_node *node = &m->deformers.nodes[i];

      psm__i32 bi = ms->deformer_src.binding_idx[i]; /* valid by verify_idx */
      node->binding = &m->bindings.items[bi];
      node->parent_part_idx = ms->deformer_src.parent_part_idx[i];
      node->parent_deformer_idx = ms->deformer_src.parent_deformer_idx[i];
      node->type = ms->deformer_src.type[i];
      node->local_idx = ms->deformer_src.local_idx[i];
      node->local_enable = ms->deformer_src.enable[i];
    }
  }

  /* Warp deformers */
  {
    struct psm__warp_src *ws = &ms->warp_src;
    struct psm__interp   *interp = &m->deformers.warps.keydata.interp;
    if (cnt->warps > 0) {
      psm__i32 tmp_len = 0;
      for (psm__i32 i = 0; i < cnt->warps; i++) {
        struct psm__warp *warp = &m->deformers.warps.items[i];
        /* binding_idx valid by verify_idx */
        psm__i32             bi = ws->binding_idx[i];
        struct psm__binding *binding = &m->bindings.items[bi];

        warp->binding = binding;
        m->deformers.warps.bindings[i] = binding;
        warp->row = ws->row[i];
        warp->col = ws->col[i];
        warp->vertex_count = ws->vertex_count[i];
        if (ver >= csmMocVersion_33 && ws->quad_transform)
          warp->quad_transform = ws->quad_transform[i];

        psm__i32 mc = binding->max_blend;
        interp->max_blend[i] = mc;
        tmp_len += mc;
      }
      interp->tmp_len = tmp_len;
    }
  }

  /* Rotation deformers */
  {
    struct psm__rotation_src *rs = &ms->rotation_src;
    struct psm__interp       *interp = &m->deformers.rotations.keydata.interp;
    if (cnt->rotations > 0) {
      psm__i32 tmp_len = 0;
      for (psm__i32 i = 0; i < cnt->rotations; i++) {
        struct psm__rotation *rot = &m->deformers.rotations.items[i];

        /* binding_idx valid by verify_idx */
        psm__i32             bi = rs->binding_idx[i];
        struct psm__binding *binding = &m->bindings.items[bi];

        rot->binding = binding;
        m->deformers.rotations.bindings[i] = binding;
        rot->base_angle = rs->base_angle[i];

        psm__i32 mc = binding->max_blend;
        interp->max_blend[i] = mc;
        tmp_len += mc;
      }
      interp->tmp_len = tmp_len;
    }
  }

  /* Art meshes */
  if (cnt->art_meshes > 0) {
    psm__i32 tmp_len = 0;
    for (psm__i32 i = 0; i < cnt->art_meshes; i++) {
      struct psm__art_mesh *mesh = &m->art_meshes.meshes[i];

      /* binding_idx valid by verify_idx */
      psm__i32             bi = ms->art_mesh_src.binding_idx[i];
      struct psm__binding *binding = &m->bindings.items[bi];

      mesh->binding = binding;
      m->art_meshes.bindings[i] = binding;
      mesh->parent_part_idx = ms->art_mesh_src.parent_part_idx[i];
      mesh->parent_deformer_idx = ms->art_mesh_src.parent_deformer_idx[i];
      mesh->vertex_count = ms->art_mesh_src.vertex_count[i];
      mesh->local_enable = ms->art_mesh_src.enable[i];
      m->art_meshes.const_flags[i] =
          ms->art_mesh_src.drawable_flag[i];

      if (ver < csmMocVersion_53) {
        psm__u8  flag = ms->art_mesh_src.drawable_flag[i];
        psm__i32 blend_mode = (flag & csmBlendMultiplicative)
                                  ? csmColorBlendType_MultiplyCompatible
                                  : csmColorBlendType_Normal;
        if (flag & csmBlendAdditive)
          blend_mode = csmColorBlendType_AddCompatible;
        m->art_meshes.blend_mode[i] = blend_mode;
      } else {
        psm__i32 raw = ms->art_mesh_src.blend_mode[i];
        m->art_meshes.blend_mode[i] = psm__remap_blend_mode(raw);
#if PSM_COMPAT_VERSION < 0x06000000L
        /* v5 callers read blend mode from constant flags */
        psm__u8 *cf = &m->art_meshes.const_flags[i];
        *cf &= ~(csmBlendAdditive | csmBlendMultiplicative);
        if (raw == csmColorBlendType_AddCompatible)
          *cf |= csmBlendAdditive;
        else if (raw == csmColorBlendType_MultiplyCompatible)
          *cf |= csmBlendMultiplicative;
#endif
      }

      psm__i32 mc = binding->max_blend;
      m->art_meshes.keydata.interp.max_blend[i] = mc;
      tmp_len += mc;
    }
    m->art_meshes.keydata.interp.tmp_len = tmp_len;
  }

  /* Link keyform color data to MOC3 source (v4.2+). */
  if (ver >= csmMocVersion_42) {
    struct psm__warp_keydata     *wk = &m->deformers.warps.keydata;
    struct psm__rotation_keydata *rk = &m->deformers.rotations.keydata;
    struct psm__art_mesh_keydata *ak = &m->art_meshes.keydata;
    {
      struct psm__key_color_src *mc = &ms->keyform_mul_color_src;
      struct psm__key_color_src *sc = &ms->keyform_scr_color_src;

      /* Warp deformer colors */
      if (mc->r && sc->r && wk->mul_color.r &&
          ms->warp_key_src.key_mul_color_off &&
          ms->warp_key_src.key_scr_color_off) {
        psm__i32  n = wk->interp.tmp_len;
        psm__i32 *mb = ms->warp_key_src.key_mul_color_off;
        psm__i32 *sb = ms->warp_key_src.key_scr_color_off;
        if (n > cnt->warp_keyforms)
          n = cnt->warp_keyforms;
        for (psm__i32 i = 0; i < n; i++) {
          psm__i32 mi = mb[i], si = sb[i];
          if (psm__valid_idx(mi, cnt->keyform_mul_colors)) {
            wk->mul_color.r[i] = mc->r[mi];
            wk->mul_color.g[i] = mc->g[mi];
            wk->mul_color.b[i] = mc->b[mi];
          }
          if (psm__valid_idx(si, cnt->keyform_scr_colors)) {
            wk->scr_color.r[i] = sc->r[si];
            wk->scr_color.g[i] = sc->g[si];
            wk->scr_color.b[i] = sc->b[si];
          }
        }
      }

      /* Rotation deformer colors */
      if (mc->r && sc->r && rk->mul_color.r &&
          ms->rotation_key_src.key_mul_color_off &&
          ms->rotation_key_src.key_scr_color_off) {
        psm__i32  n = rk->interp.tmp_len;
        psm__i32 *mb = ms->rotation_key_src.key_mul_color_off;
        psm__i32 *sb = ms->rotation_key_src.key_scr_color_off;
        if (n > cnt->rotation_keyforms)
          n = cnt->rotation_keyforms;
        for (psm__i32 i = 0; i < n; i++) {
          psm__i32 mi = mb[i], si = sb[i];
          if (psm__valid_idx(mi, cnt->keyform_mul_colors)) {
            rk->mul_color.r[i] = mc->r[mi];
            rk->mul_color.g[i] = mc->g[mi];
            rk->mul_color.b[i] = mc->b[mi];
          }
          if (psm__valid_idx(si, cnt->keyform_scr_colors)) {
            rk->scr_color.r[i] = sc->r[si];
            rk->scr_color.g[i] = sc->g[si];
            rk->scr_color.b[i] = sc->b[si];
          }
        }
      }

      /* Art mesh colors */
      if (mc->r && sc->r && ak->mul_color.r &&
          ms->art_mesh_key_src.key_mul_color_off &&
          ms->art_mesh_key_src.key_scr_color_off) {
        psm__i32  n = ak->interp.tmp_len;
        psm__i32 *mb = ms->art_mesh_key_src.key_mul_color_off;
        psm__i32 *sb = ms->art_mesh_key_src.key_scr_color_off;
        if (n > cnt->art_mesh_keyforms)
          n = cnt->art_mesh_keyforms;
        for (psm__i32 i = 0; i < n; i++) {
          psm__i32 mi = mb[i], si = sb[i];
          if (psm__valid_idx(mi, cnt->keyform_mul_colors)) {
            ak->mul_color.r[i] = mc->r[mi];
            ak->mul_color.g[i] = mc->g[mi];
            ak->mul_color.b[i] = mc->b[mi];
          }
          if (psm__valid_idx(si, cnt->keyform_scr_colors)) {
            ak->scr_color.r[i] = sc->r[si];
            ak->scr_color.g[i] = sc->g[si];
            ak->scr_color.b[i] = sc->b[si];
          }
        }
      }
    }
  }

  /* Draw order groups */
  {
    struct psm__draw_group_src     *gs = &ms->draw_group_src;
    struct psm__draw_group_obj_src *os = &ms->draw_group_obj_src;
    /* os->* have count cnt->draw_items, which differs from cnt->draw_groups */
    if (cnt->draw_groups > 0 &&
        os->type && os->idx && os->self_group_idx) {
      for (psm__i32 i = 0; i < cnt->draw_groups; i++) {
        struct psm__draw_group *grp = &m->draw_groups.groups[i];

        psm__i32 max_order = gs->max_order[i];
        psm__i32 min_order = gs->min_order[i];
        psm__i32 base_idx = gs->obj_off[i];

        grp->total_count = gs->obj_total_count[i];
        grp->max_order = max_order;
        grp->min_order = min_order;
        grp->order_level = psm__safe_order_level(max_order, min_order);
        grp->cursor = 0;

        /* obj_off + obj_len <= cnt->draw_items proved by verify_idx;
         * count>0 implies grp->items != NULL (set together in alloc) */
        for (psm__i32 j = 0; j < grp->count; j++) {
          struct psm__draw_item *item = &grp->items[j];
          item->object_type = os->type[base_idx + j];
          item->object_idx = os->idx[base_idx + j];
          item->group_idx = os->self_group_idx[base_idx + j];
          item->draw_order = 0;
        }
      }
    }
  }

  /* Glues */
  {
    struct psm__glue_src      *gls = &ms->glue_src;
    struct psm__glue_info_src *gis = &ms->glue_info_src;

    psm__i32 *max_combs = m->glues.keydata.interp.max_blend;
    /* gis->* have count cnt->glue_info, which differs from cnt->glues */
    if (cnt->glues > 0 && gis->weight && gis->pos_idx) {
      psm__i32 tmp_len = 0;
      for (psm__i32 i = 0; i < cnt->glues; i++) {
        struct psm__glue *glue = &m->glues.items[i];
        psm__i32          bi = gls->binding_idx[i]; /* valid by verify_idx */

        struct psm__binding *binding = &m->bindings.items[bi];
        psm__i32             info_off = gls->info_off[i];

        glue->binding = binding;
        m->glues.bindings[i] = binding;
        glue->mesh_idx0 = gls->art_mesh_idx_a[i];
        glue->mesh_idx1 = gls->art_mesh_idx_b[i];
        glue->glue_info_count = gls->info_len[i];

        /* info_off + info_len <= cnt->glue_info proved by verify_idx */
        glue->weights = &gis->weight[info_off];
        glue->pos_idx = &gis->pos_idx[info_off];

        psm__i32 mc = binding->max_blend;
        max_combs[i] = mc;
        tmp_len += mc;
      }
      m->glues.keydata.interp.tmp_len = tmp_len;
    }
  }

  /* BlendShape (v4.2+) */
  if (ver >= csmMocVersion_42) {
    struct psm__blend_constraint_src *bscs = &ms->blend_constraint_src;
    struct psm__blend_constraint_val_src
        *bsvs = &ms->blend_constraint_val_src;
    if (bscs->parameter_idx && bscs->value_off &&
        bscs->value_len && bsvs->key && bsvs->weight) {
      for (psm__i32 i = 0; i < cnt->bs_constraints; i++) {
        struct psm__blend_constraint *constr = &m->blend_constraints.items[i];

        psm__i32 pi = bscs->parameter_idx[i];
        psm__i32 vb = bscs->value_off[i];
        psm__i32 vc = bscs->value_len[i];
        if (!psm__valid_idx(pi, cnt->parameters)) {
          constr->param = NULL;
          constr->keys = NULL;
          constr->weights = NULL;
          constr->count = 0;
          constr->weight = 1.0f;
          continue;
        }
        if (!psm__valid_range(vb, vc, cnt->bs_constraint_vals)) {
          constr->param = &m->params.items[pi];
          constr->keys = NULL;
          constr->weights = NULL;
          constr->count = 0;
          constr->weight = 1.0f;
          continue;
        }
        constr->param = &m->params.items[pi];
        constr->keys = &bsvs->key[vb];
        constr->weights = &bsvs->weight[vb];
        constr->count = vc;
        constr->weight = 1.0f;
      }
    }

    struct psm__blend_key_table_src *ba_src = &ms->blend_key_table_src;
    if (ba_src->keys_len && ba_src->keys_off && ba_src->base_key_idx &&
        ms->keys_src.key) {
      for (psm__i32 i = 0; i < cnt->blend_key_tables; i++) {
        struct psm__blend_key_table *ba = &m->blend_key_tables.items[i];

        psm__i32 kc = ba_src->keys_len[i];
        psm__i32 kb = ba_src->keys_off[i];
        if (psm__valid_range(kb, kc, cnt->keys)) {
          ba->key_count = kc;
          ba->keys = &ms->keys_src.key[kb];
        } else {
          PSM__LOGF("blend_pb[%d]: key range "
                    "[%d, %d) OOB (max %d)",
              i, kb, kb + kc, cnt->keys);
          ba->keys = NULL;
          ba->key_count = 0;
        }
        ba->base_key_idx = ba_src->base_key_idx[i];
        ba->idx = 0;
        ba->weight = 0.0f;
        ba->idx_dirty = 1;
        ba->weight_dirty = 1;
      }
    }

    for (psm__i32 i = 0; i < cnt->parameters; i++) {
      struct psm__param *pc = &m->params.items[i];
      if (ms->param_src.blend_key_table_len &&
          ms->param_src.blend_key_table_off &&
          m->blend_key_tables.items) {
        psm__i32 bs_cnt = ms->param_src.blend_key_table_len[i];
        psm__i32 bs_off = ms->param_src.blend_key_table_off[i];
        if (psm__valid_opt_range(bs_off,
                bs_cnt, cnt->blend_key_tables) == 1) {
          pc->blend_key_table_len = bs_cnt;
          pc->blend_key_tables = &m->blend_key_tables.items[bs_off];
        }
      }
    }

    struct psm__blend_binding_src *bb_src = &ms->blend_binding_src;
    if (bb_src->key_table_idx && bb_src->key_bs_off) {
      for (psm__i32 i = 0; i < cnt->blend_bindings; i++) {
        struct psm__blend_binding *bb = &m->blend_bindings.items[i];

        psm__i32 bpi = bb_src->key_table_idx[i];
        if (m->blend_key_tables.items && psm__valid_idx(bpi,
                                             cnt->blend_key_tables))
          bb->key_table = &m->blend_key_tables.items[bpi];
        else
          bb->key_table = NULL;
        bb->key_src_off = bb_src->key_bs_off[i];
        bb->blend_count = 0;
        bb->idx_dirty = 1;
        bb->weight_dirty = 1;
        bb->weight = 1.0f;

        psm__i32 cc = bb->constraint_count;
        if (cc > 0 && bb->constraints && bb_src->bs_constraint_idx_off &&
            ms->blend_constraint_idx_src.constraint_idx) {
          psm__i32 cb = bb_src->bs_constraint_idx_off[i];
          for (psm__i32 j = 0; j < cc; j++) {
            psm__i32 ci = ms->blend_constraint_idx_src.constraint_idx[cb + j];
            if (psm__valid_idx(ci, cnt->bs_constraints))
              bb->constraints[j] = &m->blend_constraints.items[ci];
            else
              bb->constraints[j] = NULL;
          }
        }
      }
    }

    if (ms->bs_warp_src.target_idx && ms->bs_warp_src.bs_binding_len &&
        ms->bs_warp_src.bs_binding_off) {
      for (psm__i32 i = 0; i < cnt->bs_warps; i++) {
        struct psm__blend_shape *shape = &m->bs_warps.items[i];
        shape->target_idx = ms->bs_warp_src.target_idx[i];
        shape->binding_count = ms->bs_warp_src.bs_binding_len[i];
        psm__i32 bb = ms->bs_warp_src.bs_binding_off[i];
        if (psm__valid_range(bb, shape->binding_count, cnt->blend_bindings))
          shape->bindings = &m->blend_bindings.items[bb];
        else
          shape->bindings = NULL;
      }
    }

    if (ms->bs_art_mesh_src.target_idx &&
        ms->bs_art_mesh_src.bs_binding_len &&
        ms->bs_art_mesh_src.bs_binding_off) {
      for (psm__i32 i = 0; i < cnt->bs_art_meshes; i++) {
        struct psm__blend_shape *shape = &m->bs_art_meshes.items[i];
        shape->target_idx = ms->bs_art_mesh_src.target_idx[i];
        shape->binding_count = ms->bs_art_mesh_src.bs_binding_len[i];
        psm__i32 bb = ms->bs_art_mesh_src.bs_binding_off[i];
        if (psm__valid_range(bb, shape->binding_count, cnt->blend_bindings))
          shape->bindings = &m->blend_bindings.items[bb];
        else
          shape->bindings = NULL;
      }
    }
  }

  /* BlendShape (v5.0+) */
  if (ver >= csmMocVersion_50) {
    if (ms->bs_part_src.target_idx && ms->bs_part_src.bs_binding_len &&
        ms->bs_part_src.bs_binding_off) {
      for (psm__i32 i = 0; i < cnt->bs_parts; i++) {
        struct psm__blend_shape *shape = &m->bs_parts.items[i];
        shape->target_idx = ms->bs_part_src.target_idx[i];
        shape->binding_count = ms->bs_part_src.bs_binding_len[i];
        psm__i32 bb = ms->bs_part_src.bs_binding_off[i];
        if (psm__valid_range(bb, shape->binding_count, cnt->blend_bindings))
          shape->bindings = &m->blend_bindings.items[bb];
        else
          shape->bindings = NULL;
      }
    }

    if (ms->bs_rotation_src.target_idx &&
        ms->bs_rotation_src.bs_binding_len &&
        ms->bs_rotation_src.bs_binding_off) {
      for (psm__i32 i = 0; i < cnt->bs_rotations; i++) {
        struct psm__blend_shape *shape = &m->bs_rotations.items[i];
        shape->target_idx = ms->bs_rotation_src.target_idx[i];
        shape->binding_count = ms->bs_rotation_src.bs_binding_len[i];
        psm__i32 bb = ms->bs_rotation_src.bs_binding_off[i];
        if (psm__valid_range(bb, shape->binding_count, cnt->blend_bindings))
          shape->bindings = &m->blend_bindings.items[bb];
        else
          shape->bindings = NULL;
      }
    }

    if (ms->bs_glue_src.target_idx && ms->bs_glue_src.bs_binding_len &&
        ms->bs_glue_src.bs_binding_off) {
      for (psm__i32 i = 0; i < cnt->bs_glues; i++) {
        struct psm__blend_shape *shape = &m->bs_glues.items[i];
        shape->target_idx = ms->bs_glue_src.target_idx[i];
        shape->binding_count = ms->bs_glue_src.bs_binding_len[i];
        psm__i32 bb = ms->bs_glue_src.bs_binding_off[i];
        if (psm__valid_range(bb, shape->binding_count, cnt->blend_bindings))
          shape->bindings = &m->blend_bindings.items[bb];
        else
          shape->bindings = NULL;
      }
    }
  }

  /* Offscreen rendering (v5.3+) */
  if (ver >= csmMocVersion_53) {
    if (cnt->offscreens > 0 && ms->offscreen_src.owner_idx) {
      psm__i32  tmp_len = 0;
      psm__i32 *os_key_idx = ms->part_key_src.key_idx;

      for (psm__i32 i = 0; i < cnt->offscreens; i++) {
        struct psm__offscreen *surf = &m->offscreens.surfaces[i];

        psm__i32 oi = ms->offscreen_src.owner_idx[i];

        if (!psm__valid_idx(oi, cnt->parts)) {
          surf->binding = NULL;
          surf->owner_enable = NULL;
          surf->keyform_idx = NULL;
          m->offscreens.keydata.interp.max_blend[i] = 0;
          continue;
        }

        /* Check source arrays */
        if (!ms->part_src.binding_idx || !ms->part_src.keyform_off) {
          surf->binding = NULL;
          surf->owner_enable = NULL;
          surf->keyform_idx = NULL;
          m->offscreens.keydata.interp.max_blend[i] = 0;
          continue;
        }

        psm__i32 pbi = ms->part_src.binding_idx[oi];
        psm__i32 kbi = ms->part_src.keyform_off[oi];

        if (psm__valid_opt_idx(pbi, cnt->bindings) == 1)
          surf->binding = &m->bindings.items[pbi];
        else
          surf->binding = NULL;

        if (m->parts.enable)
          surf->owner_enable = &m->parts.enable[oi];
        else
          surf->owner_enable = NULL;

        /* Set offscreen keyform index pointer */
        if (os_key_idx && kbi >= 0 && kbi < cnt->part_keyforms) {
          surf->keyform_idx = &os_key_idx[kbi];
        } else {
          surf->keyform_idx = NULL;
        }

        psm__i32 mc = surf->binding ? surf->binding->max_blend : 0;
        m->offscreens.keydata.interp.max_blend[i] = mc;
        tmp_len += mc;
      }
      m->offscreens.keydata.interp.tmp_len = tmp_len;

      for (psm__i32 i = 0; i < 4 * cnt->offscreens; i++) {
        m->offscreens.mul_color[i] = 1.0f;
        m->offscreens.scr_color[i] = 1.0f;
      }
    } else {
      m->offscreens.keydata.interp.tmp_len = 0;
    }

    if (ms->bs_offscreen_src.target_idx &&
        ms->bs_offscreen_src.bs_binding_len &&
        ms->bs_offscreen_src.bs_binding_off) {
      for (psm__i32 i = 0; i < cnt->bs_offscreens; i++) {
        struct psm__blend_shape *shape = &m->bs_offscreens.items[i];
        shape->target_idx = ms->bs_offscreen_src.target_idx[i];
        shape->binding_count = ms->bs_offscreen_src.bs_binding_len[i];
        psm__i32 bb = ms->bs_offscreen_src.bs_binding_off[i];
        if (psm__valid_range(bb, shape->binding_count, cnt->blend_bindings))
          shape->bindings = &m->blend_bindings.items[bb];
        else
          shape->bindings = NULL;
      }
    }
  }

  /* Parameter extensions */
  if (ver >= csmMocVersion_42 && ms->param_keys_src.key_runtime) {
    if (ms->keys_src.key && ms->param_keys_src.keys_off) {
      for (psm__i32 i = 0; i < cnt->parameters; i++) {
        psm__i32 kb = ms->param_keys_src.keys_off[i];
        if (kb < 0 || kb > cnt->keys) {
          PSM__LOGF("param_keys[%d]: keyform_off %d OOB (max %d)",
              i, kb, cnt->keys);
          m->param_keys.keys[i] = NULL;
        } else {
          m->param_keys.keys[i] = kb < cnt->keys ? &ms->keys_src.key[kb] : NULL;
        }
      }
    }
  } else {
    for (psm__i32 i = 0; i < cnt->parameters; i++) {
      psm__i32 kt_off = ms->param_src.key_table_off[i];
      psm__i32 kt_count = ms->param_src.key_table_len[i];

      if (kt_off < 0 || kt_count <= 0 || !ms->keys_src.key ||
          !ms->key_table_src.keys_off || !ms->key_table_src.keys_len) {
        m->param_keys.keys[i] = NULL;
        m->param_keys.key_counts[i] = 0;
        continue;
      }
      if (!psm__valid_range(kt_off, kt_count, cnt->key_tables)) {
        PSM__LOGF("param_keys fallback[%d]: kt_off %d OOB (max %d)",
            i, kt_off, cnt->key_tables);
        m->param_keys.keys[i] = NULL;
        m->param_keys.key_counts[i] = 0;
        continue;
      }
      psm__i32 best = kt_off, best_kc = 0;
      for (psm__i32 j = 0; j < kt_count; j++) {
        psm__i32 idx = kt_off + j;
        if (idx < cnt->key_tables &&
            ms->key_table_src.keys_len[idx] > best_kc) {
          best_kc = ms->key_table_src.keys_len[idx];
          best = idx;
        }
      }
      psm__i32 fkb = ms->key_table_src.keys_off[best];
      if (!psm__valid_range(fkb, best_kc, cnt->keys)) {
        PSM__LOGF("param_keys fallback[%d]: key range [%d, %d) OOB (max %d)",
            i, fkb, fkb + best_kc, cnt->keys);
        m->param_keys.keys[i] = NULL;
        m->param_keys.key_counts[i] = 0;
        continue;
      }
      m->param_keys.keys[i] = &ms->keys_src.key[fkb];
      m->param_keys.key_counts[i] = best_kc;
    }
  }

  /* Render orders */
  for (psm__i32 i = 0; i < cnt->art_meshes; i++)
    m->render_order[i] = i;

  /* Color defaults: mul=(1,1,1,1), scr alpha=1 */
  psm__i32 dc = cnt->deformers;
  psm__i32 wc = cnt->warps;
  psm__i32 rc = cnt->rotations;
  psm__i32 ac = cnt->art_meshes;
  psm__i32 oc = cnt->offscreens;

  if (m->deformers.mul_color)
    psm__init_mul_color(m->deformers.mul_color, dc);
  if (m->deformers.scr_color)
    psm__init_scr_color(m->deformers.scr_color, dc);
  if (m->deformers.warps.mul_color)
    psm__init_mul_color(m->deformers.warps.mul_color, wc);
  if (m->deformers.warps.scr_color)
    psm__init_scr_color(m->deformers.warps.scr_color, wc);
  if (m->deformers.rotations.mul_color)
    psm__init_mul_color(m->deformers.rotations.mul_color, rc);
  if (m->deformers.rotations.scr_color)
    psm__init_scr_color(m->deformers.rotations.scr_color, rc);
  if (m->art_meshes.mul_color)
    psm__init_mul_color(m->art_meshes.mul_color, ac);
  if (m->art_meshes.scr_color)
    psm__init_scr_color(m->art_meshes.scr_color, ac);
  if (m->art_meshes.last_mul_color)
    psm__init_mul_color(m->art_meshes.last_mul_color, ac);
  if (m->art_meshes.last_scr_color)
    psm__init_scr_color(m->art_meshes.last_scr_color, ac);
  if (m->offscreens.mul_color)
    psm__init_mul_color(m->offscreens.mul_color, oc);
  if (m->offscreens.scr_color)
    psm__init_scr_color(m->offscreens.scr_color, oc);

  return PSM__OK;
}

static int
psm__model_size(psm_size *out, const struct psm__moc3_data *moc)
{
  struct psm__arena arena = PSM__ARENA_INIT(NULL, 0);
  psm__alloc_model(&arena, moc->header->version, moc->sections,
      moc->sections->count_info);
  if (!psm__arena_ok(&arena))
    return PSM__ERR_FILE_CORRUPT;
  *out = psm__arena_total(&arena);
  return PSM__OK;
}

static int
psm__init_model(struct psm__model **out,
    const struct psm__moc3_data *moc, void *p, psm_size n)
{
  /* Dry run to calculate required size */
  struct psm__arena arena = PSM__ARENA_INIT(NULL, 0);
  psm__alloc_model(&arena, moc->header->version, moc->sections,
      moc->sections->count_info);
  PSM__FAILM(!psm__arena_ok(&arena), PSM__ERR_FILE_CORRUPT,
      "allocation size overflow");
  psm__u32 required = psm__arena_total(&arena);

  PSM__FAIL(n < required, PSM__ERR_INVALID_DATA,
      "insufficient memory (%u < %u)", (unsigned)n, required);

  /* Zero the buffer, then allocate and assign pointers */
  memset(p, 0, n);
  arena = PSM__ARENA_INIT(p, n);
  struct psm__model *m = psm__alloc_model(&arena,
      moc->header->version, moc->sections, moc->sections->count_info);
  PSM__FAILM(!m || !psm__arena_ok(&arena), PSM__ERR_FILE_CORRUPT,
      "arena allocation failed");

  PSM__FAILM(psm__init_model_data(m, moc) != PSM__OK,
      PSM__ERR_FILE_CORRUPT, "model data init failed");
  psm__update_model(m);
  *out = m;
  return PSM__OK;
}

PSMDEF unsigned int
csmGetSizeofModel(const csmMoc *moc)
{
  psm_size size;
  PSM__FAILM(psm__model_size(&size, psm__moc_to_data(moc)) != PSM__OK,
      0, "could not get model size");
  return size;
}

PSMDEF csmModel *
csmInitializeModelInPlace(const csmMoc *moc, void *address, unsigned int size)
{
  struct psm__model *model;

  int err = psm__init_model(&model, psm__moc_to_data(moc), address, size);

  /*
   * Record the outcome in the moc header so csmGetMocError can explain an
   * init failure, just as it does for a failed revive. Written on success
   * too (PSM__OK), so a later good init clears a prior failure's code.
   */
  ((struct psm__moc3_header *)moc)->last_error = err;

  PSM__FAILM(err != PSM__OK, NULL, "could not init model");
  return (csmModel *)model;
}

PSMDEF void
csmReadCanvasInfo(const csmModel *model, csmVector2 *outSizeInPixels,
    csmVector2 *outOriginInPixels, float *outPixelsPerUnit)
{
  const struct psm__model       *m = (const struct psm__model *)model;
  const struct psm__canvas_info *c = m->source->sections->canvas_info;
  outSizeInPixels->X = c->width;
  outSizeInPixels->Y = c->height;
  outOriginInPixels->X = c->origin_x;
  outOriginInPixels->Y = c->origin_y;
  *outPixelsPerUnit = c->pix_per_unit;
}

/* Purism Core extension: see PurismCore.h. */
PSMDEF csmError
csmGetLastError(const csmModel *model)
{
  if (!model)
    return csmError_NoError;
  return (csmError)((const struct psm__model *)model)->last_error;
}

PSMDEF const char *
csmGetErrorString(csmError error)
{
  switch (error) {
  case csmError_NoError:
    return "no error";
  case csmError_Failed:
    return "operation failed";
  case csmError_ParameterRange:
    return "parameter out of range";
  case csmError_FileUnrecognized:
    return "unrecognized MOC3 file";
  case csmError_FileCorrupt:
    return "corrupt MOC3 file";
  case csmError_InvalidData:
    return "invalid data";
  case csmError_InvalidParameter:
    return "invalid parameter";
  default:
    return "unknown error";
  }
}
/* ===== update.c ===== */
/*
 * Purism Core: update pipeline
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <string.h>

static void
psm__reverse_y(struct psm__model *m)
{
  if (m->y_reversed)
    return;

  psm__i32 count = m->art_meshes.count;
  if (count <= 0)
    return;

  struct psm__art_mesh *meshes = m->art_meshes.meshes;

  bool      *en = m->art_meshes.enable;
  psm__f32 **pos = m->art_meshes.pos;

  for (psm__i32 i = 0; i < count; i++) {
    if (!en[i])
      continue;
    psm__i32 vc = meshes[i].vertex_count;
    if (vc <= 0)
      continue;
    psm__f32 *p = pos[i];
    for (psm__i32 j = 0; j < vc; j++)
      p[2 * j + 1] = -p[2 * j + 1];
  }
}

static void
psm__save_flags(struct psm__model *m)
{
  struct psm__art_meshes *am = &m->art_meshes;
  if (!am->state_changed)
    return;

  psm__u8  ver = m->source->header->version;
  psm__i32 n = am->count;

  memcpy(am->last_render_order, m->render_order, sizeof(psm__i32) * n);
  memcpy(am->last_draw_order, am->draw_order, sizeof(psm__i32) * n);
  memcpy(am->last_opacity, am->opacity, sizeof(psm__f32) * n);

  if (ver < csmMocVersion_42)
    return;

  memcpy(am->last_mul_color, am->mul_color, 4 * sizeof(psm__f32) * n);
  memcpy(am->last_scr_color, am->scr_color, 4 * sizeof(psm__f32) * n);
}

static void
psm__update_flags(struct psm__model *m)
{
  struct psm__art_meshes *am = &m->art_meshes;

  psm__u8  ver = m->source->header->version;
  psm__i32 n = am->count;

  /* Force update: mark everything dirty */
  if (m->force_update) {
    am->state_changed = 0;
    if (n <= 0)
      return;

    bool     *en = am->enable;
    psm__f32 *opacity = am->opacity;
    psm__u8  *df = am->change_flags;

    for (psm__i32 i = 0; i < n; i++) {
      if (!en[i] || opacity[i] == 0.0f)
        df[i] = PSM__FLAG_ALL_CHANGED;
      else
        df[i] = PSM__FLAG_ALL;
    }
    return;
  }

  /* Normal dirty update: compare previous vs current */
  if (am->state_changed) {
    am->state_changed = 0;
    if (n <= 0)
      return;

    bool     *en = am->enable;
    psm__f32 *opacity = am->opacity;
    psm__u8  *df = am->change_flags;
    psm__i32 *ro = m->render_order;
    psm__i32 *last_ro = am->last_render_order;
    psm__i32 *cdo = am->draw_order;
    psm__i32 *last_do = am->last_draw_order;
    psm__f32 *last_op = am->last_opacity;

    psm__f32 *mc = NULL, *lmc = NULL;
    psm__f32 *sc = NULL, *lsc = NULL;
    if (ver >= csmMocVersion_42) {
      mc = am->mul_color;
      lmc = am->last_mul_color;
      sc = am->scr_color;
      lsc = am->last_scr_color;
    }

    for (psm__i32 i = 0; i < n; i++) {
      psm__i32 visible = en[i] && opacity[i] != 0.0f;
      psm__i32 was = (df[i] & PSM__FLAG_IS_VISIBLE) != 0;
      psm__u8  flags = visible;

      if (visible != was)
        flags |= PSM__FLAG_VISIBILITY_CHANGED;
      if (opacity[i] != last_op[i])
        flags |= PSM__FLAG_OPACITY_CHANGED;
      if (cdo[i] != last_do[i])
        flags |= PSM__FLAG_DRAW_ORDER_CHANGED;
      if (ro[i] != last_ro[i])
        flags |= PSM__FLAG_RENDER_ORDER_CHANGED;
      if (en[i])
        flags |= PSM__FLAG_VERTEX_CHANGED;

      if (mc && (memcmp(&mc[i * 4], &lmc[i * 4], 16) != 0 ||
                    memcmp(&sc[i * 4], &lsc[i * 4], 16) != 0))
        flags |= PSM__FLAG_BLEND_COLOR_CHANGED;

      df[i] = flags;
    }
    return;
  }

  /* No dirty update: just refresh visibility bit */
  if (n <= 0)
    return;

  bool     *en = am->enable;
  psm__f32 *opacity = am->opacity;
  psm__u8  *df = am->change_flags;

  for (psm__i32 i = 0; i < n; i++) {
    if (!en[i] || opacity[i] == 0.0f)
      df[i] &= ~PSM__FLAG_IS_VISIBLE;
    else
      df[i] |= PSM__FLAG_IS_VISIBLE;
  }
}

PSM__DEF void
psm__update_model(struct psm__model *m)
{
  int r = PSM__OK;

  m->last_error = r;
  psm__save_flags(m);

  if ((r = psm__resolve_params(&m->params)) != PSM__OK)
    m->last_error = r;
  psm__resolve_key_tables(m);
  psm__resolve_blend_key_tables(m);
  psm__resolve_bindings(m);
  psm__resolve_blend_bindings(m);

  psm__i32 nparts = m->parts.count;
  if (nparts > 0) {
    psm__f32 *opa = m->parts.input_opacity;
    for (psm__i32 i = 0; i < nparts; i++)
      opa[i] = psm__clamp_f32_01(opa[i]);
  }

  psm__enable_parts(m);
  psm__gather_parts(m);
  psm__interp_parts(m);

  psm__enable_deformers(m);
  psm__gather_warps(m);
  psm__gather_rotations(m);
  psm__interp_warps(m);
  psm__interp_rotations(m);

  psm__enable_art_meshes(m);
  psm__gather_art_meshes(m);
  psm__interp_art_meshes(m);

  psm__gather_glues(m);
  psm__interp_glues(m);

  psm__enable_offscreens(m);
  psm__gather_offscreens(m);
  psm__interp_offscreens(m);

  psm__blend_parts(m);
  psm__blend_warps(m);
  psm__blend_rotations(m);
  psm__blend_art_meshes(m);
  psm__blend_glues(m);
  psm__blend_offscreens(m);

  psm__apply_transforms(m);
  psm__apply_transforms_to_meshes(m);
  psm__apply_part_opacity(m);
  psm__apply_parts_to_meshes(m);

  psm__apply_glues(m); /* must come before reverse_y! */

  psm__reverse_y(m);
  psm__sort_render_order(m);
  psm__update_flags(m);

  /* v6+: zero opacity for disabled offscreen surfaces */
  if (m->source->header->version >= csmMocVersion_53) {
    psm__i32  oc = m->offscreens.count;
    bool     *en = m->offscreens.enable;
    psm__f32 *opa = m->offscreens.opacity;
    if (oc > 0 && en && opa) {
      for (psm__i32 i = 0; i < oc; i++) {
        if (!en[i])
          opa[i] = 0.0f;
      }
    }
  }

  m->force_update = 0;
}

PSMDEF void
csmUpdateModel(csmModel *model)
{
  psm__update_model((struct psm__model *)model);
}

PSMDEF void
csmResetDrawableDynamicFlags(csmModel *model)
{
  struct psm__model *m = (struct psm__model *)model;
  psm__i32           count = m->art_meshes.count;
  psm__u8           *flags = m->art_meshes.change_flags;
  for (psm__i32 i = 0; i < count; i++)
    flags[i] &= PSM__FLAG_IS_VISIBLE;
  m->art_meshes.state_changed = 1;
}
/* ===== param.c ===== */
/*
 * Purism Core: parameter handling
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <math.h>
#include <string.h>

struct psm__key_search_result {
  psm__i32 index;   /* Key index (lower bound of segment) */
  psm__f32 weight;  /* Interpolation weight within segment [0,1] */
  bool     is_outside;  /* Value is outside key range */
  bool     needs_check; /* Need to check previous out_of_range state */
};

static struct psm__key_search_result
psm__find_key_segment(psm__f32 value, const psm__f32 *keys, psm__i32 key_count,
    psm__f32 snap_eps, psm__f32 interp_eps)
{
  struct psm__key_search_result r = { 0, 0.0f, false, false };

  if (key_count <= 0) {
    r.needs_check = true;
    return r;
  }

  if (key_count == 1) {
    psm__f32 key0 = keys[0];
    r.is_outside = (value <= key0 - snap_eps) || (value >= key0 + snap_eps);
    r.needs_check = !r.is_outside;
    return r;
  }

  /* key_count >= 2 */
  psm__f32 key0 = keys[0], key1;

  /* Before first key */
  if (value < key0 - snap_eps) {
    r.is_outside = true;
    return r;
  }

  /* Snapped to first key */
  if (value < key0 + snap_eps) {
    r.needs_check = true;
    return r;
  }

  /* Check second key */
  key1 = keys[1];
  if (value < key1 - snap_eps) {
    /* Between first and second key */
    psm__f32 key_diff = key1 - key0;
    if (key_diff >= interp_eps)
      r.weight = (value - key0) / key_diff;
    return r;
  }
  if (value < key1 + snap_eps) {
    /* Snapped to second key */
    r.index = 1;
    r.needs_check = true;
    return r;
  }

  /* Search remaining keys */
  for (psm__i32 k = 2; k < key_count; k++) {
    key0 = key1;
    key1 = keys[k];
    if (value < key1 - snap_eps) {
      /* Between key0 and key1 */
      r.index = k - 1;
      psm__f32 key_diff = key1 - key0;
      if (key_diff >= interp_eps)
        r.weight = (value - key0) / key_diff;
      return r;
    }
    if (value < key1 + snap_eps) {
      /* Snapped to this key */
      r.index = k;
      r.needs_check = true;
      return r;
    }
  }

  /* After last key */
  r.index = key_count - 1;
  r.is_outside = true;
  return r;
}

PSM__DEF int
psm__resolve_params(struct psm__params *parameters)
{
  psm__i32 count = parameters->count;
  if (count <= 0)
    return false;

  struct psm__param *params = parameters->items;

  psm__f32 *input_value = parameters->input_value;
  int       r = PSM__OK;

  for (psm__i32 i = 0; i < count; i++) {
    psm__f32 user_value = input_value[i];
    psm__f32 new_value;

    if (params[i].repeat) {
      psm__f32 range_min = params[i].range[0],
               range_length = params[i].range_length;

      psm__f32 normalized = (user_value - range_min) / range_length;
      psm__f32 wrapped = normalized - floorf(normalized);
      new_value = wrapped * range_length + range_min;

      if (params[i].value != new_value) {
        params[i].value = new_value;
        params[i].dirty = 1;
      } else {
        params[i].dirty = 0;
      }
    } else {
      psm__f32 range_min = params[i].range[0], range_max = params[i].range[1];
      if (user_value < range_min || user_value > range_max)
        r = PSM__ERR_PARAMETER_RANGE_ERROR;
      new_value = psm__clamp_f32(user_value, range_min, range_max);

      if (params[i].value != new_value) {
        params[i].value = new_value;
        params[i].dirty = 1;
      } else {
        params[i].dirty = 0;
      }

      input_value[i] = new_value;
    }
  }

  return r;
}

PSM__DEF void
psm__resolve_key_tables(struct psm__model *m)
{
  psm__i32           param_count = m->params.count;
  struct psm__param *param_items = m->params.items;
  if (!param_items || param_count <= 0)
    return;

  for (psm__i32 i = 0; i < param_count; i++) {
    struct psm__param *param = &param_items[i];

    if (param->type != csmParameterType_Normal)
      continue;
    if (!param->dirty && !m->force_update) {
      /*
       * Unchanged parameter: clear dirty flags on all its bindings
       * so downstream keyform updates don't re-evaluate them.
       */
      psm__i32 bc = param->key_table_len;

      struct psm__key_table *bs = param->key_tables;
      if (bs) {
        for (psm__i32 j = 0; j < bc; j++) {
          bs[j].idx_dirty = 0;
          bs[j].weight_dirty = 0;
        }
      }
      continue;
    }

    psm__i32 binding_count = param->key_table_len;

    struct psm__key_table *bindings = param->key_tables;
    if (!bindings || binding_count <= 0)
      continue;
    psm__f32 value = param->value;
    psm__f32 snap_eps = param->snap_eps, interp_eps = param->interp_eps;

    for (psm__i32 j = 0; j < binding_count; j++) {
      struct psm__key_table *binding = &bindings[j];
      if (!binding->keys || binding->key_count <= 0)
        continue;

      struct psm__key_search_result r = psm__find_key_segment(value,
          binding->keys, binding->key_count, snap_eps, interp_eps);

      bool idx_dirty, weight_dirty;
      if (!r.is_outside && binding->out_of_range) {
        /* Transition from outside to inside: force dirty and clear outside */
        r.is_outside = false;
        idx_dirty = 1;
        weight_dirty = 1;
      } else {
        /* Normal case (including outsideâ†’outside): check value changes */
        idx_dirty = (binding->idx != r.index);
        weight_dirty = (binding->weight != r.weight);
        if (weight_dirty)
          idx_dirty = r.weight == 0.0f || binding->weight == 0.0f || idx_dirty;
      }

      binding->idx_dirty = idx_dirty;
      binding->weight_dirty = weight_dirty;
      binding->idx = r.index;
      binding->weight = r.weight;
      binding->out_of_range = r.is_outside;
    }
  }
}

PSM__DEF void
psm__resolve_blend_key_tables(struct psm__model *m)
{
  psm__u8 version = m->source->header->version;
  if (version < csmMocVersion_42)
    return;

  psm__i32 param_count = m->params.count;
  if (param_count <= 0)
    return;

  struct psm__param *params = m->params.items;
  if (!params)
    return;
  bool force_update = m->force_update;

  for (psm__i32 param_i = 0; param_i < param_count; param_i++) {
    if (params[param_i].type != csmParameterType_BlendShape)
      continue;

    psm__i32 bs_count = params[param_i].blend_key_table_len;
    if (bs_count <= 0)
      continue;

    struct psm__blend_key_table *blend_key_tables =
        params[param_i].blend_key_tables;
    if (!blend_key_tables)
      continue;
    psm__f32 value = params[param_i].value;

    if (force_update || params[param_i].dirty) {
      for (psm__i32 bs_i = 0; bs_i < bs_count; bs_i++) {
        psm__i32 key_count = blend_key_tables[bs_i].key_count;
        psm__u32 index = 0;
        psm__f32 weight = 0.0f;

        if (key_count >= 2) {
          psm__f32 *keys = blend_key_tables[bs_i].keys;
          if (keys && value > keys[0]) {
            /* Find upper bound: first key > value */
            for (index = 1;
                index < (psm__u32)key_count && value >= keys[index]; index++) {
            }
            index--;
            if (index < (psm__u32)key_count - 1)
              weight = (value - keys[index]) / (keys[index + 1] - keys[index]);
          }
        }

        psm__u32 old_index = blend_key_tables[bs_i].idx;
        psm__f32 old_weight = blend_key_tables[bs_i].weight;
        bool     idx_dirty = (old_index != index),
             weight_dirty = (old_weight != weight);
        if (weight_dirty)
          idx_dirty = weight == 0.0f ||
                      old_weight == 0.0f || old_index != index;

        blend_key_tables[bs_i].idx_dirty = idx_dirty;
        blend_key_tables[bs_i].weight_dirty = weight_dirty;
        blend_key_tables[bs_i].weight = weight;
        blend_key_tables[bs_i].idx = index;
      }
    } else {
      for (psm__i32 bs_i = 0; bs_i < bs_count; bs_i++) {
        blend_key_tables[bs_i].idx_dirty = 0;
        blend_key_tables[bs_i].weight_dirty = 0;
      }
    }
  }
}

PSM__DEF void
psm__resolve_bindings(struct psm__model *m)
{
  psm__i32 count = m->bindings.count;
  if (count <= 0)
    return;

  struct psm__binding *binds = m->bindings.items;
  if (!binds)
    return;
  bool force_update = m->force_update;

  struct psm__key_table *kt_base = m->key_tables.items;
  struct psm__key_table *kt_end = kt_base + m->key_tables.count;

  for (psm__i32 bi = 0; bi < count; bi++) {
    psm__i32 binding_count = binds[bi].key_table_len;

    struct psm__key_table **bindings = binds[bi].key_tables;

    bool     idx_dirty = false;
    bool     weight_dirty = false;
    bool     out_of_range = false;
    psm__u32 active_binding_count = 0;

    /* Skip if no bindings */
    if (!bindings) {
      binds[bi].idx_dirty = 0;
      binds[bi].weight_dirty = 0;
      binds[bi].out_of_range = true;
      continue;
    }

    for (psm__i32 i = 0; i < binding_count; i++) {
      struct psm__key_table *binding = bindings[i];

      /* Validate pointer is within expected range */
      if (binding < kt_base || binding >= kt_end) {
        out_of_range = true; /* corrupted ptr */
        break;
      }

      if (binding->out_of_range) {
        out_of_range = true;
        idx_dirty = 0;
        weight_dirty = 0;
        break;
      }

      weight_dirty |= binding->weight_dirty;
      idx_dirty |= binding->idx_dirty;

      if (binding->weight != 0.0f) {
        active_binding_count++;
      }
    }

    if (out_of_range) {
      binds[bi].idx_dirty = 0;
      binds[bi].weight_dirty = 0;
      binds[bi].out_of_range = true;
      continue;
    }

    if (force_update) {
      idx_dirty = 1;
      weight_dirty = 1;
    }

    if (!(idx_dirty || weight_dirty)) {
      binds[bi].idx_dirty = 0;
      binds[bi].weight_dirty = 0;
      binds[bi].out_of_range = false;
      continue;
    }

    psm__u32 blend_count = 1u << active_binding_count;
    binds[bi].blend_count = blend_count;

    /* Skip if keyform_idx or weights arrays are missing */
    if (!binds[bi].keyform_idx || !binds[bi].weights) {
      binds[bi].idx_dirty = 0;
      binds[bi].weight_dirty = 0;
      binds[bi].out_of_range = true;
      continue;
    }

    memset(binds[bi].keyform_idx, 0, blend_count * sizeof(psm__i32));
    for (psm__i32 j = 0; j < (psm__i32)blend_count; j++) {
      binds[bi].weights[j] = 1.0f;
    }

    /*
     * Combo builder. Produces all 2^N keyform
     * index + weight combinations from N key tables.
     * index_stride tracks the key counts.
     * combo_stride tracks which bit selects upper vs lower
     * keyform for each active key table.
     */
    psm__u32 index_stride = 1, combo_stride = 1;

    for (psm__i32 i = 0; i < binding_count; i++) {
      struct psm__key_table *binding = bindings[i];

      psm__i32 index = binding->idx;
      psm__i32 key_count = binding->key_count;
      psm__f32 weight = binding->weight;
      psm__i32 index_offset = index * index_stride;

      if (weight != 0.0f) {
        psm__i32 next_index_offset = (index + 1) * index_stride;
        psm__f32 inv_weight = 1.0f - weight;

        for (psm__i32 j = 0; j < (psm__i32)blend_count; j++) {
          if ((j & combo_stride) == 0) {
            binds[bi].keyform_idx[j] += index_offset;
            binds[bi].weights[j] *= inv_weight;
          } else {
            binds[bi].keyform_idx[j] += next_index_offset;
            binds[bi].weights[j] *= weight;
          }
        }

        combo_stride *= 2;
      } else {
        for (psm__i32 j = 0; j < (psm__i32)blend_count; j++) {
          binds[bi].keyform_idx[j] += index_offset;
        }
      }

      index_stride *= key_count;
    }

    binds[bi].idx_dirty = idx_dirty;
    binds[bi].weight_dirty = weight_dirty;
    binds[bi].out_of_range = false;
  }
}

PSM__DEF void
psm__resolve_blend_bindings(struct psm__model *m)
{
  psm__u8 version = m->source->header->version;
  if (version < csmMocVersion_42)
    return;

  psm__i32 count = m->blend_bindings.count;
  if (count <= 0)
    return;

  struct psm__blend_binding *binds = m->blend_bindings.items;
  if (!binds)
    return;
  bool force_update = m->force_update;

  for (psm__i32 bi = 0; bi < count; bi++) {
    struct psm__blend_key_table *binding = binds[bi].key_table;
    if (!binding)
      continue;

    bool weight_dirty = force_update || binding->weight_dirty;
    bool idx_dirty = force_update || binding->idx_dirty;

    if (weight_dirty || idx_dirty) {
      psm__f32 weight = binding->weight;
      psm__i32 base_key_idx = binding->base_key_idx;
      psm__i32 next_key_index = binding->idx;

      /* Handle special case: weight != 0 and at base key */
      if (weight != 0.0f && next_key_index == base_key_idx) {
        idx_dirty = 1;
        weight_dirty = 1;
        binds[bi].blend_count = 1;
        binds[bi].weights[0] = weight;
        binds[bi].weights[1] = 1.0f - weight;
        next_key_index++;
        binds[bi].keyform_idx[0] = next_key_index;
        binds[bi].keyform_idx[1] = next_key_index + 1;
      } else {
        /* Set combination count */
        if (weight == 0.0f)
          binds[bi].blend_count = next_key_index != base_key_idx;
        else
          binds[bi].blend_count =
              (next_key_index + 1 == base_key_idx) ? 1 : 2;

        /* Update weights if weight changed */
        if (weight_dirty) {
          binds[bi].weights[0] = 1.0f - weight;
          binds[bi].weights[1] = weight;
        }

        /* Update indices if index changed */
        if (idx_dirty) {
          binds[bi].keyform_idx[0] = next_key_index;
          binds[bi].keyform_idx[1] = next_key_index + 1;
        }
      }
    }

    /* Process constraints */
    psm__i32 constraint_count = binds[bi].constraint_count;
    psm__f32 weight = 1.0f;

    for (psm__i32 i = 0; i < constraint_count; i++) {
      struct psm__blend_constraint *constraint = binds[bi].constraints[i];
      struct psm__param            *param = constraint->param;

      psm__f32 constraint_weight = 1.0f;

      if (param != NULL && (force_update || param->dirty)) {
        psm__i32  key_count = constraint->count;
        psm__f32 *keys = constraint->keys, *weights = constraint->weights;

        if (key_count >= 2) {
          psm__f32 value = param->value;
          if (value > keys[0]) {
            /* Find upper bound */
            psm__i32 idx;
            for (idx = 1; idx < key_count && value >= keys[idx]; idx++) {
            }
            idx--;
            if (idx < key_count - 1) {
              psm__f32 t = (value - keys[idx]) / (keys[idx + 1] - keys[idx]);
              constraint_weight = weights[idx] * (1.0f - t) +
                                  weights[idx + 1] * t;
            } else {
              constraint_weight = weights[key_count - 1];
            }
          } else {
            constraint_weight = weights[0];
          }
        } else if (key_count == 1) {
          constraint_weight = weights[0];
        }
        constraint->weight = constraint_weight;
      } else if (param != NULL) {
        constraint_weight = constraint->weight;
      } else {
        constraint->weight = constraint_weight;
      }

      weight = fminf(weight, constraint_weight);
    }

    binds[bi].weight = weight;
    binds[bi].idx_dirty = idx_dirty;
    binds[bi].weight_dirty = weight_dirty;
  }
}

PSMDEF int
csmGetParameterCount(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->params.count;
}

PSMDEF const char **
csmGetParameterIds(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->param_src.id_runtime;
}

PSMDEF const csmParameterType *
csmGetParameterTypes(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmParameterType *)m->params.type;
}

PSMDEF const float *
csmGetParameterMinimumValues(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->param_src.minimum_value;
}

PSMDEF const float *
csmGetParameterMaximumValues(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->param_src.maximum_value;
}

PSMDEF const float *
csmGetParameterDefaultValues(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->param_src.default_value;
}

PSMDEF float *
csmGetParameterValues(csmModel *model)
{
  struct psm__model *m = (struct psm__model *)model;
  return m->params.input_value;
}

#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF const int *
csmGetParameterRepeats(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const int *)m->source->sections->param_src.repeat;
}
#endif

PSMDEF const int *
csmGetParameterKeyCounts(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->param_keys.key_counts;
}

PSMDEF const float **
csmGetParameterKeyValues(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const float **)m->param_keys.keys;
}
/* ===== part.c ===== */
/*
 * Purism Core: part hierarchy and opacity
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <string.h>

PSM__DEF void
psm__enable_parts(struct psm__model *m)
{
  psm__i32 count = m->parts.count;
  if (count <= 0)
    return;

  struct psm__part *items = m->parts.items;
  bool             *enable = m->parts.enable;

  for (psm__i32 i = 0; i < count; i++) {
    struct psm__part *part = &items[i];

    bool     en = part->local_enable;
    psm__i32 parent_part_idx = part->parent_part_idx;

    if (en && parent_part_idx != -1)
      en = enable[parent_part_idx];
    if (en)
      en = !part->binding->out_of_range;

    enable[i] = en;
  }
}

PSM__DEF void
psm__gather_parts(struct psm__model *m)
{
  psm__i32 count = m->parts.count;
  if (count <= 0)
    return;

  struct psm__part *items = m->parts.items;
  if (!items)
    return;
  struct psm__sections *ms = m->source->sections;
  psm__i32             *keyform_off = ms->part_src.keyform_off;
  psm__f32             *draw_order_src = ms->part_key_src.draw_order;

  if (!keyform_off || !draw_order_src)
    return;

  struct psm__binding *const *bindings = m->parts.bindings;

  struct psm__gather_channel ch[] = {
    { draw_order_src, m->parts.keydata.draw_order },
  };
  psm__gather_scalars(count, bindings, keyform_off, &m->parts.keydata.interp, ch, 1);
}

PSM__DEF void
psm__apply_part_opacity(struct psm__model *m)
{
  psm__i32 count = m->parts.count;
  if (count <= 0)
    return;

  struct psm__part *items = m->parts.items;

  psm__i32 *offscreen_indices = m->parts.offscreen_src_idx;
  bool     *enable = m->parts.enable;
  psm__f32 *input_opacity = m->parts.input_opacity,
           *part_opa = m->parts.opacity;

  for (psm__i32 i = 0; i < count; i++) {
    if (!enable[i])
      continue;

    psm__f32 opacity = input_opacity[i];
    part_opa[i] = opacity;

    psm__i32 parent_index = items[i].parent_part_idx;
    if (parent_index != -1 && offscreen_indices[parent_index] == -1) {
      opacity *= part_opa[parent_index];
      part_opa[i] = opacity;
    }

    if (offscreen_indices[i] != -1) {
      psm__i32 offscreen_idx = offscreen_indices[i];
      m->offscreens.opacity[offscreen_idx] *= opacity;
    }
  }
}

PSMDEF int
csmGetPartCount(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->parts.count;
}

PSMDEF const char **
csmGetPartIds(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->part_src.id_runtime;
}

PSMDEF float *
csmGetPartOpacities(csmModel *model)
{
  struct psm__model *m = (struct psm__model *)model;
  return m->parts.input_opacity;
}

PSMDEF const int *
csmGetPartParentPartIndices(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->part_src.parent_part_idx;
}

#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF const int *
csmGetPartOffscreenIndices(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->parts.offscreen_src_idx;
}
#endif
/* ===== deformer.c ===== */
/*
 * Purism Core: warp and rotation deformer transforms
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <math.h>
#include <string.h>

struct psm__warp_basis {
  struct psm__vec2 center;
  struct psm__vec2 dpdv;
  struct psm__vec2 dpdu;
};

struct psm__warp_cell {
  psm__f32         fu, fv;
  struct psm__vec2 p00, p10, p01, p11;
};

static inline struct psm__warp_basis
psm__warp_extrap_basis(const psm__f32 *pos, psm__i32 row, psm__i32 col,
    psm__i32 stride)
{
  struct psm__vec2 c00 = psm__v2(pos[0], pos[1]);
  struct psm__vec2 c10 = psm__v2_load(pos, col);
  struct psm__vec2 c01 = psm__v2_load(pos, row * stride);
  struct psm__vec2 c11 = psm__v2_load(pos, row * stride + col);

  struct psm__vec2 d11_00 = psm__v2_sub(c11, c00);
  struct psm__vec2 d10_01 = psm__v2_sub(c10, c01);

  struct psm__warp_basis b;
  b.dpdv = psm__v2_scale(psm__v2_sub(d11_00, d10_01), 0.5f);
  b.dpdu = psm__v2_scale(psm__v2_add(d10_01, d11_00), 0.5f);

  struct psm__vec2 sum = psm__v2_add(
      psm__v2_add(c00, c10), psm__v2_add(c01, c11));
  b.center = psm__v2_sub(
      psm__v2_scale(sum, 0.25f), psm__v2_scale(d11_00, 0.5f));

  return b;
}

static inline struct psm__warp_cell
psm__warp_extrap_cell(psm__f32 u, psm__f32 v, psm__f32 gu, psm__f32 gv,
    psm__i32 row, psm__i32 col, psm__i32 stride, const psm__f32 *pos,
    const struct psm__warp_basis *basis)
{
  psm__f32         fr = (psm__f32)row, fc = (psm__f32)col;
  struct psm__vec2 cen = basis->center;
  struct psm__vec2 dv = basis->dpdv;
  struct psm__vec2 du = basis->dpdu;

  struct psm__warp_cell cell;

  /*
   * fu/fv and the interior-strip indices depend only on each axis's class
   * (below grid / within a boundary strip / above), so resolve them per axis
   * here; the per-octant switch below builds only the cell corners.
   * uc/un (cu/(cv) normalized) are used by the within-strip octants.
   */
  psm__i32 cu = 0, cv = 0;
  psm__f32 uc = 0.0f, un = 0.0f, vc = 0.0f, vn = 0.0f;

  if (u <= 0.0f)
    cell.fu = (u + 2.0f) * 0.5f;
  else if (u >= 1.0f)
    cell.fu = (u - 1.0f) * 0.5f;
  else {
    cu = (psm__i32)gu;
    if (cu == col) cu = col - 1;
    cell.fu = gu - (psm__f32)cu;
    uc = (psm__f32)cu / fc;
    un = (psm__f32)(cu + 1) / fc;
  }

  if (v <= 0.0f)
    cell.fv = (v + 2.0f) * 0.5f;
  else if (v >= 1.0f)
    cell.fv = (v - 1.0f) * 0.5f;
  else {
    cv = (psm__i32)gv;
    if (cv == row) cv = row - 1;
    cell.fv = gv - (psm__f32)cv;
    vc = (psm__f32)cv / fr;
    vn = (psm__f32)(cv + 1) / fr;
  }

  if (u <= 0.0f) {
    if (v <= 0.0f) {                      /* below-left corner */
      cell.p00 = psm__v2_sub(cen,
          psm__v2_add(psm__v2_scale(dv, 2.0f), psm__v2_scale(du, 2.0f)));
      cell.p10 = psm__v2_sub(cen, psm__v2_scale(dv, 2.0f));
      cell.p01 = psm__v2_sub(cen, psm__v2_scale(du, 2.0f));
      cell.p11 = psm__v2(pos[0], pos[1]);
    } else if (v < 1.0f) {                /* left edge */
      cell.p00 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(du, 2.0f)),
          psm__v2_scale(dv, vc));
      cell.p10 = psm__v2_load(pos, cv * stride);
      cell.p01 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(du, 2.0f)),
          psm__v2_scale(dv, vn));
      cell.p11 = psm__v2_load(pos, (cv + 1) * stride);
    } else {                              /* above-left corner */
      cell.p00 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(du, 2.0f)), dv);
      cell.p10 = psm__v2_load(pos, row * stride);
      cell.p01 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(du, 2.0f)),
          psm__v2_scale(dv, 3.0f));
      cell.p11 = psm__v2_add(cen, psm__v2_scale(dv, 3.0f));
    }
  } else if (u < 1.0f) {
    if (v <= 0.0f) {                      /* top edge */
      cell.p00 = psm__v2_add(psm__v2_scale(du, uc),
          psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)));
      cell.p10 = psm__v2_add(psm__v2_scale(du, un),
          psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)));
      cell.p01 = psm__v2_load(pos, cu);
      cell.p11 = psm__v2_load(pos, cu + 1);
    } else {                              /* bottom edge */
      cell.p00 = psm__v2_load(pos, row * stride + cu);
      cell.p10 = psm__v2_load(pos, row * stride + cu + 1);
      cell.p01 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(du, uc)),
          psm__v2_scale(dv, 3.0f));
      cell.p11 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(du, un)),
          psm__v2_scale(dv, 3.0f));
    }
  } else {
    if (v <= 0.0f) {                      /* below-right corner */
      cell.p00 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)), du);
      cell.p10 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)),
          psm__v2_scale(du, 3.0f));
      cell.p01 = psm__v2_load(pos, col);
      cell.p11 = psm__v2_add(cen, psm__v2_scale(du, 3.0f));
    } else if (v < 1.0f) {                /* right edge */
      cell.p00 = psm__v2_load(pos, col + cv * stride);
      cell.p10 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(du, 3.0f)),
          psm__v2_scale(dv, vc));
      cell.p01 = psm__v2_load(pos, col + (cv + 1) * stride);
      cell.p11 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(du, 3.0f)),
          psm__v2_scale(dv, vn));
    } else {                              /* above-right corner */
      cell.p00 = psm__v2_load(pos, row * stride + col);
      cell.p10 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(du, 3.0f)), dv);
      cell.p01 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(dv, 3.0f)), du);
      cell.p11 = psm__v2_add(cen,
          psm__v2_add(psm__v2_scale(du, 3.0f), psm__v2_scale(dv, 3.0f)));
    }
  }

  return cell;
}

static inline struct psm__vec2
psm__interp_triangle(const struct psm__warp_cell *cell)
{
  psm__f32 fu = cell->fu, fv = cell->fv;
  if (fu + fv <= 1.0f) {
    psm__f32 w00 = 1.0f - fu - fv;
    return psm__v2_bary3(cell->p00, cell->p10, cell->p01, w00, fu, fv);
  } else {
    psm__f32 w10 = 1.0f - fv;
    psm__f32 w11 = fu + fv - 1.0f;
    psm__f32 w01 = 1.0f - fu;
    return psm__v2_bary3(cell->p10, cell->p11, cell->p01, w10, w11, w01);
  }
}

static void
psm__warp_transform(struct psm__model *m, psm__i32 di,
    const psm__f32 *inputs, psm__f32 *outputs, psm__i32 count)
{
  struct psm__deformer_node *dn = m->deformers.nodes;

  psm__i32          si = dn[di].local_idx;
  struct psm__warp *wc = &m->deformers.warps.items[si];
  psm__f32         *pos = m->deformers.warps.pos[si];

  psm__i32 row = wc->row, col = wc->col;
  bool     is_quad = wc->quad_transform;
  psm__i32 stride = col + 1;
  psm__f32 fr = (psm__f32)row, fc = (psm__f32)col;

  bool extrap_setup = false;

  struct psm__warp_basis basis;

  for (psm__i32 i = 0; i < count; i++) {
    struct psm__vec2 uv = psm__v2_load(inputs, i);
    psm__f32         gu = uv.x * fc, gv = uv.y * fr;

    if (uv.x >= 0.0f && uv.x < 1.0f && uv.y >= 0.0f && uv.y < 1.0f) {
      /* Interior: interpolate within grid cell */
      psm__i32 cu = (psm__i32)gu, cv = (psm__i32)gv;
      psm__f32 fu = gu - (psm__f32)cu;
      psm__f32 fv = gv - (psm__f32)cv;

      psm__i32         bi = cv * stride + cu;
      struct psm__vec2 p00 = psm__v2_load(pos, bi);
      struct psm__vec2 p10 = psm__v2_load(pos, bi + 1);
      struct psm__vec2 p01 = psm__v2_load(pos, bi + stride);
      struct psm__vec2 p11 = psm__v2_load(pos, bi + stride + 1);

      struct psm__vec2 result;
      if (is_quad) {
        result = psm__v2_bilinear(p00, p10, p01, p11, fu, fv);
      } else {
        struct psm__warp_cell cell = { fu, fv, p00, p10, p01, p11 };
        result = psm__interp_triangle(&cell);
      }
      psm__v2_store(outputs, i, result);
    } else {
      /* Extrapolation: compute basis if needed */
      if (!extrap_setup) {
        basis = psm__warp_extrap_basis(pos, row, col, stride);
        extrap_setup = true;
      }

      if (uv.x > -2.0f && uv.x < 3.0f && uv.y > -2.0f && uv.y < 3.0f) {
        /* Near-exterior: virtual cell + triangle */
        struct psm__warp_cell cell = psm__warp_extrap_cell(uv.x, uv.y, gu, gv,
            row, col, stride, pos, &basis);
        struct psm__vec2      r = psm__interp_triangle(&cell);
        psm__v2_store(outputs, i, r);
      } else {
        /* Far-exterior: simple affine */
        psm__f32 rx = basis.dpdu.x * uv.x + basis.center.x +
                      basis.dpdv.x * uv.y;
        psm__f32 ry = basis.dpdu.y * uv.x + basis.center.y +
                      basis.dpdv.y * uv.y;
        outputs[i * 2] = rx;
        outputs[i * 2 + 1] = ry;
      }
    }
  }
}

static void
psm__rotation_transform(struct psm__model *m, psm__i32 di,
    const psm__f32 *inputs, psm__f32 *outputs, psm__i32 count)
{
  psm__i32 si = m->deformers.nodes[di].local_idx;

  struct psm__rotation *rc = &m->deformers.rotations.items[si];

  psm__f32         base_angle = rc->base_angle;
  psm__f32         angle = m->deformers.rotations.angle[si];
  psm__f32         scale = m->deformers.rotations.scale[si];
  struct psm__vec2 origin = psm__v2(m->deformers.rotations.origin_x[si],
      m->deformers.rotations.origin_y[si]);
  psm__i32         rx = m->deformers.rotations.reflect_x[si];
  psm__i32         ry = m->deformers.rotations.reflect_y[si];

  psm__f32 angle_rad = (base_angle + angle) * PSM__PI / 180.0f;
  psm__f32 sin_a = sinf(angle_rad);
  psm__f32 cos_a = cosf(angle_rad);

  psm__f32 rxf = rx ? -1.0f : 1.0f;
  psm__f32 ryf = ry ? -1.0f : 1.0f;

  psm__f32 m00 = scale * cos_a * rxf;
  psm__f32 m01 = scale * (-sin_a) * ryf;
  psm__f32 m10 = scale * sin_a * rxf;
  psm__f32 m11 = scale * cos_a * ryf;

  for (psm__i32 i = 0; i < count; i++) {
    struct psm__vec2 p = psm__v2_load(inputs, i);
    struct psm__vec2 r = psm__v2(origin.x + m00 * p.x + m01 * p.y,
        origin.y + m10 * p.x + m11 * p.y);
    psm__v2_store(outputs, i, r);
  }
}

static inline struct psm__vec2
psm__deformer_transform_point(struct psm__model *m,
    psm__i32 deformer_idx, struct psm__vec2 p)
{
  psm__f32 in[2] = { p.x, p.y };
  psm__f32 out[2];
  psm__i32 pt = m->deformers.nodes[deformer_idx].type;
  if (pt == PSM__DEFORMER_TYPE_WARP)
    psm__warp_transform(m, deformer_idx, in, out, 1);
  else
    psm__rotation_transform(m, deformer_idx, in, out, 1);
  return (struct psm__vec2){ out[0], out[1] };
}

static void
psm__propagate_deformer_colors(psm__f32 *mo, psm__f32 *so,
    const psm__f32 *sm, const psm__f32 *ss,
    psm__i32 self_idx, psm__i32 spec_idx, psm__i32 parent_idx)
{
  psm__i32 s4 = self_idx * 4, p4 = spec_idx * 4;

  if (parent_idx == -1) {
    mo[s4 + 0] = sm[p4 + 0];
    mo[s4 + 1] = sm[p4 + 1];
    mo[s4 + 2] = sm[p4 + 2];
    mo[s4 + 3] = 1.0f;

    so[s4 + 0] = ss[p4 + 0];
    so[s4 + 1] = ss[p4 + 1];
    so[s4 + 2] = ss[p4 + 2];
    so[s4 + 3] = 1.0f;
  } else {
    psm__i32        pi4 = parent_idx * 4;
    const psm__f32 *pm = &mo[pi4];
    const psm__f32 *ps = &so[pi4];

    mo[s4 + 0] = sm[p4 + 0] * pm[0];
    mo[s4 + 1] = sm[p4 + 1] * pm[1];
    mo[s4 + 2] = sm[p4 + 2] * pm[2];
    mo[s4 + 3] = 1.0f;

    so[s4 + 0] = ss[p4 + 0] + ps[0] - ss[p4 + 0] * ps[0];
    so[s4 + 1] = ss[p4 + 1] + ps[1] - ss[p4 + 1] * ps[1];
    so[s4 + 2] = ss[p4 + 2] + ps[2] - ss[p4 + 2] * ps[2];
    so[s4 + 3] = 1.0f;
  }
}

static void
psm__apply_warp(struct psm__model *m, psm__i32 di)
{
  struct psm__deformer_node *dn = m->deformers.nodes;
  struct psm__deformer_node *self = &dn[di];

  psm__f32 *d_opa = m->deformers.opacity;
  psm__f32 *d_scl = m->deformers.scale;
  psm__i32  pi = self->parent_deformer_idx;
  psm__i32  si = self->local_idx;

  if (pi == -1) {
    d_opa[di] = m->deformers.warps.opacity[si];
    d_scl[di] = 1.0f;
  } else {
    psm__f32 **pos = m->deformers.warps.pos;
    psm__i32   vc = m->deformers.warps.items[si].vertex_count;
    psm__i32   pt = dn[pi].type;

    switch (pt) {
    case PSM__DEFORMER_TYPE_WARP:
      psm__warp_transform(m, pi, pos[si], pos[si], vc);
      break;
    case PSM__DEFORMER_TYPE_ROTATION:
      psm__rotation_transform(m, pi, pos[si], pos[si], vc);
      break;
    }

    d_opa[di] = m->deformers.warps.opacity[si] * d_opa[pi];
    d_scl[di] = d_scl[pi];
  }

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__propagate_deformer_colors(
      m->deformers.mul_color, m->deformers.scr_color,
      m->deformers.warps.mul_color, m->deformers.warps.scr_color,
      di, si, pi);
}

static void
psm__apply_rotation(struct psm__model *m, psm__i32 di)
{
  struct psm__deformer_node *dn = m->deformers.nodes;
  struct psm__deformer_node *self = &dn[di];

  psm__f32 *d_opa = m->deformers.opacity;
  psm__f32 *d_scl = m->deformers.scale;
  psm__i32  pi = self->parent_deformer_idx;
  psm__i32  si = self->local_idx;

  psm__f32 *r_opa = m->deformers.rotations.opacity;
  psm__f32 *r_scl = m->deformers.rotations.scale;
  psm__f32 *r_ox = m->deformers.rotations.origin_x;
  psm__f32 *r_oy = m->deformers.rotations.origin_y;
  psm__f32 *r_ang = m->deformers.rotations.angle;

  if (pi == -1) {
    d_opa[di] = r_opa[si];
    d_scl[di] = r_scl[si];
  } else {
    struct psm__vec2 origin = psm__v2(r_ox[si], r_oy[si]);
    struct psm__vec2 direction = { 0.0f, 0.0f };
    psm__i32         pt = dn[pi].type;
    psm__f32         dir_delta =
        (pt == PSM__DEFORMER_TYPE_ROTATION) ? -10.0f : -0.1f;

    struct psm__vec2 t_origin = psm__deformer_transform_point(m, pi, origin);

    psm__f32 scale = 1.0f;
    psm__i32 iter;
    for (iter = 0; iter < 16; iter++) {
      struct psm__vec2 tp = psm__v2(origin.x, origin.y + scale * dir_delta);
      struct psm__vec2 tt = psm__deformer_transform_point(m, pi, tp);
      struct psm__vec2 d = psm__v2_sub(tt, t_origin);

      if (d.x != 0.0f || d.y != 0.0f) {
        direction = d;
        break;
      }

      tp = psm__v2(origin.x, origin.y - scale * dir_delta);
      tt = psm__deformer_transform_point(m, pi, tp);
      d = psm__v2_sub(tt, t_origin);

      if (d.x != 0.0f || d.y != 0.0f) {
        direction = psm__v2_neg(d);
        break;
      }

      scale *= 0.1f;
    }

    if (iter >= 16) {
      PSM__WARN("rotation direction did not converge");
    }

    psm__f32 base_dir[2] = { 0.0f, dir_delta };
    psm__f32 dir_arr[2] = { direction.x, direction.y };
    psm__f32 angle_adj =
        (psm__signed_angle(base_dir, dir_arr) * -180.0f) / PSM__PI;

    origin = psm__deformer_transform_point(m, pi, origin);

    r_ox[si] = origin.x;
    r_oy[si] = origin.y;
    r_ang[si] += angle_adj;

    d_opa[di] = r_opa[si] * d_opa[pi];

    psm__f32 cs = r_scl[si] * d_scl[pi];
    d_scl[di] = cs;
    r_scl[si] = cs;
  }

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__propagate_deformer_colors(
      m->deformers.mul_color, m->deformers.scr_color,
      m->deformers.rotations.mul_color,
      m->deformers.rotations.scr_color, di, si, pi);
}

PSM__DEF void
psm__enable_deformers(struct psm__model *m)
{
  psm__i32 count = m->deformers.count;
  if (count <= 0)
    return;

  struct psm__deformer_node *nodes = m->deformers.nodes;

  bool *en = m->deformers.enable;
  bool *pen = m->parts.enable;
  bool *wen = m->deformers.warps.enable;
  bool *ren = m->deformers.rotations.enable;

  for (psm__i32 i = 0; i < count; i++) {
    struct psm__deformer_node *node = &nodes[i];

    bool     e = node->local_enable;
    psm__i32 ppi = node->parent_part_idx;
    psm__i32 pdi = node->parent_deformer_idx;

    if (e && ppi != -1) e = pen[ppi];
    if (e && pdi != -1) e = en[pdi];
    if (e) e = !node->binding->out_of_range;

    en[i] = e;

    psm__i32 dt = node->type;
    psm__i32 si = node->local_idx;

    switch (dt) {
    case PSM__DEFORMER_TYPE_WARP:
      wen[si] = e;
      break;
    case PSM__DEFORMER_TYPE_ROTATION:
      ren[si] = e;
      break;
    default:
      PSM__LOG("unknown deformer type");
      break;
    }
  }
}

PSM__DEF void
psm__gather_warps(struct psm__model *m)
{
  psm__i32 count = m->deformers.warps.count;
  if (count <= 0)
    return;

  struct psm__sections *ms = m->source->sections;
  struct psm__warp     *items = m->deformers.warps.items;
  if (!items)
    return;

  struct psm__warp_keydata *wk = &m->deformers.warps.keydata;

  psm__i32 *kb = ms->warp_src.keyform_off;

  struct psm__binding *const *bindings = m->deformers.warps.bindings;

  struct psm__gather_channel ch[] = {
    { ms->warp_key_src.opacity, wk->opacity },
  };
  psm__gather_scalars(count, bindings, kb, &wk->interp, ch, 1);

  psm__gather_positions(count, bindings, kb,
      ms->key_pos_src.xy, ms->warp_key_src.key_pos_off, wk->pos);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__i32 *ckb = ms->warp_src.key_color_off;
  if (!ckb || !ms->keyform_mul_color_src.r || !ms->keyform_scr_color_src.r)
    return;

  psm__gather_colors(count, bindings, ckb,
      &ms->keyform_mul_color_src, &ms->keyform_scr_color_src,
      &wk->mul_color, &wk->scr_color);
}

PSM__DEF void
psm__gather_rotations(struct psm__model *m)
{
  psm__i32 count = m->deformers.rotations.count;
  if (count <= 0)
    return;

  struct psm__sections *ms = m->source->sections;
  struct psm__rotation *items = m->deformers.rotations.items;
  if (!items)
    return;

  struct psm__rotation_keydata *rk = &m->deformers.rotations.keydata;

  psm__i32 *kb = ms->rotation_src.keyform_off;

  struct psm__binding *const *bindings = m->deformers.rotations.bindings;

  struct psm__gather_channel ch[] = {
    { ms->rotation_key_src.opacity, rk->opacity },
    { ms->rotation_key_src.angle, rk->angle },
    { ms->rotation_key_src.origin_x, rk->origin_x },
    { ms->rotation_key_src.origin_y, rk->origin_y },
    { ms->rotation_key_src.scale, rk->scale },
  };
  psm__gather_scalars(count, bindings, kb, &rk->interp, ch, 5);

  psm__gather_reflect(count, bindings, kb,
      ms->rotation_key_src.reflect_x, ms->rotation_key_src.reflect_y,
      m->deformers.rotations.reflect_x, m->deformers.rotations.reflect_y);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__i32 *ckb = ms->rotation_src.key_color_off;
  if (!ckb || !ms->keyform_mul_color_src.r || !ms->keyform_scr_color_src.r)
    return;

  psm__gather_colors(count, bindings, ckb,
      &ms->keyform_mul_color_src, &ms->keyform_scr_color_src,
      &rk->mul_color, &rk->scr_color);
}

PSM__DEF void
psm__apply_transforms(struct psm__model *m)
{
  psm__i32 count = m->deformers.count;
  if (count <= 0)
    return;

  bool *en = m->deformers.enable;

  struct psm__deformer_node *nodes = m->deformers.nodes;

  /*
   * Iterate in array order. MOC3 stores deformers in
   * topological order (parents before children), so each
   * deformer's parent is already transformed.
   */
  for (psm__i32 i = 0; i < count; i++) {
    if (en[i]) {
      switch (nodes[i].type) {
      case PSM__DEFORMER_TYPE_WARP:
        psm__apply_warp(m, i);
        break;
      case PSM__DEFORMER_TYPE_ROTATION:
        psm__apply_rotation(m, i);
        break;
      }
    }
  }
}

PSM__DEF void
psm__apply_transforms_to_meshes(struct psm__model *m)
{
  psm__i32 count = m->art_meshes.count;
  if (count <= 0)
    return;

  struct psm__art_mesh      *am = m->art_meshes.meshes;
  struct psm__deformer_node *dn = m->deformers.nodes;

  psm__f32  *d_opa = m->deformers.opacity;
  psm__f32 **cp = m->art_meshes.pos;
  psm__f32  *am_opa = m->art_meshes.opacity;
  bool      *en = m->art_meshes.enable;

  for (psm__i32 i = 0; i < count; i++) {
    if (!en[i]) continue;

    psm__i32 pdi = am[i].parent_deformer_idx;
    if (pdi == -1) continue;

    psm__i32 vc = am[i].vertex_count;
    am_opa[i] *= d_opa[pdi];

    psm__i32 dt = dn[pdi].type;
    if (dt == PSM__DEFORMER_TYPE_WARP)
      psm__warp_transform(m, pdi, cp[i], cp[i], vc);
    else
      psm__rotation_transform(m, pdi, cp[i], cp[i], vc);
  }
}
/* ===== artmesh.c ===== */
/*
 * Purism Core: art mesh processing
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <string.h>

PSM__DEF void
psm__enable_art_meshes(struct psm__model *m)
{
  psm__i32 count = m->art_meshes.count;
  if (count <= 0)
    return;

  struct psm__art_mesh *meshes = m->art_meshes.meshes;

  bool *def_en = m->deformers.enable;
  bool *part_en = m->parts.enable;
  bool *enable = m->art_meshes.enable;

  for (psm__i32 i = 0; i < count; i++) {
    struct psm__art_mesh *am = &meshes[i];

    bool     en = am->local_enable;
    psm__i32 pp = am->parent_part_idx;
    psm__i32 pd = am->parent_deformer_idx;

    if (en && pp != -1) en = part_en[pp];
    if (en && pd != -1) en = def_en[pd];
    if (en) en = !am->binding->out_of_range;

    enable[i] = en;
  }
}

PSM__DEF void
psm__gather_art_meshes(struct psm__model *m)
{
  psm__i32 count = m->art_meshes.count;
  if (count <= 0)
    return;

  struct psm__sections *ms = m->source->sections;
  struct psm__art_mesh *meshes = m->art_meshes.meshes;
  if (!meshes)
    return;

  psm__i32 *kb = ms->art_mesh_src.keyform_off;

  struct psm__art_mesh_keydata *kd = &m->art_meshes.keydata;

  struct psm__binding *const *bindings = m->art_meshes.bindings;

  struct psm__gather_channel ch[] = {
    { ms->art_mesh_key_src.opacity, kd->opacity },
    { ms->art_mesh_key_src.draw_order, kd->draw_order },
  };
  psm__gather_scalars(count, bindings, kb, &kd->interp, ch, 2);

  psm__gather_positions(count, bindings, kb,
      ms->key_pos_src.xy, ms->art_mesh_key_src.key_pos_off, kd->pos);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__i32 *kcb = ms->art_mesh_src.key_color_off;
  if (!kcb || !ms->keyform_mul_color_src.r || !ms->keyform_scr_color_src.r)
    return;

  psm__gather_colors(count, bindings, kcb,
      &ms->keyform_mul_color_src, &ms->keyform_scr_color_src,
      &kd->mul_color, &kd->scr_color);
}

PSM__DEF void
psm__apply_parts_to_meshes(struct psm__model *m)
{
  psm__i32 count = m->art_meshes.count;
  if (count <= 0)
    return;

  struct psm__art_mesh *meshes = m->art_meshes.meshes;

  bool     *en = m->art_meshes.enable;
  psm__f32 *part_opa = m->parts.opacity;
  psm__i32 *part_off = m->parts.offscreen_src_idx;
  psm__f32 *am_opa = m->art_meshes.opacity;

  for (psm__i32 i = 0; i < count; i++) {
    if (!en[i])
      continue;
    psm__i32 pp = meshes[i].parent_part_idx;
    if (pp != -1 && part_off[pp] == -1)
      am_opa[i] *= part_opa[pp];
  }

  psm__u8 version = m->source->header->version;
  if (version < csmMocVersion_42)
    return;

  psm__f32 *d_mul = m->deformers.mul_color;
  psm__f32 *d_scr = m->deformers.scr_color;
  psm__f32 *am_mul = m->art_meshes.mul_color;
  psm__f32 *am_scr = m->art_meshes.scr_color;

  for (psm__i32 i = 0; i < count; i++) {
    psm__i32 ci = i * 4;
    if (!en[i])
      continue;
    psm__i32 pd = meshes[i].parent_deformer_idx;
    if (pd == -1)
      continue;

    psm__f32 *pm = &d_mul[pd * 4];
    psm__f32 *ps = &d_scr[pd * 4];

    psm__f32 r = am_mul[ci + 0] * pm[0];
    psm__f32 g = am_mul[ci + 1] * pm[1];
    psm__f32 b = am_mul[ci + 2] * pm[2];

    am_mul[ci + 0] = psm__clamp_f32_01(r);
    am_mul[ci + 1] = psm__clamp_f32_01(g);
    am_mul[ci + 2] = psm__clamp_f32_01(b);
    am_mul[ci + 3] = 1.0f;

    r = am_scr[ci + 0] + ps[0] - am_scr[ci + 0] * ps[0];
    g = am_scr[ci + 1] + ps[1] - am_scr[ci + 1] * ps[1];
    b = am_scr[ci + 2] + ps[2] - am_scr[ci + 2] * ps[2];

    am_scr[ci + 0] = psm__clamp_f32_01(r);
    am_scr[ci + 1] = psm__clamp_f32_01(g);
    am_scr[ci + 2] = psm__clamp_f32_01(b);
    am_scr[ci + 3] = 1.0f;
  }
}

PSMDEF int
csmGetDrawableCount(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->art_meshes.count;
}

PSMDEF const char **
csmGetDrawableIds(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->art_mesh_src.id_runtime;
}

PSMDEF const csmFlags *
csmGetDrawableConstantFlags(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmFlags *)m->art_meshes.const_flags;
}

PSMDEF const csmFlags *
csmGetDrawableDynamicFlags(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmFlags *)m->art_meshes.change_flags;
}

#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF const int *
csmGetDrawableBlendModes(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->art_meshes.blend_mode;
}
#endif

PSMDEF const int *
csmGetDrawableTextureIndices(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->art_mesh_src.texture_no;
}

PSMDEF const int *
csmGetDrawableDrawOrders(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->art_meshes.draw_order;
}

PSMDEF const float *
csmGetDrawableOpacities(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->art_meshes.opacity;
}

PSMDEF const int *
csmGetDrawableMaskCounts(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->art_mesh_src.mask_len;
}

PSMDEF const int **
csmGetDrawableMasks(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->art_mesh_src.drawable_mask_runtime;
}

PSMDEF const int *
csmGetDrawableVertexCounts(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->art_mesh_src.vertex_count;
}

PSMDEF const csmVector2 **
csmGetDrawableVertexPositions(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmVector2 **)m->art_meshes.pos;
}

PSMDEF const csmVector2 **
csmGetDrawableVertexUvs(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmVector2 **)m->source->sections->art_mesh_src.uv_runtime;
}

PSMDEF const int *
csmGetDrawableIndexCounts(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->art_mesh_src.idx_len;
}

PSMDEF const unsigned short **
csmGetDrawableIndices(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const unsigned short **)
      m->source->sections->art_mesh_src.pos_idx_runtime;
}

PSMDEF const csmVector4 *
csmGetDrawableMultiplyColors(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmVector4 *)m->art_meshes.mul_color;
}

PSMDEF const csmVector4 *
csmGetDrawableScreenColors(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmVector4 *)m->art_meshes.scr_color;
}

PSMDEF const int *
csmGetDrawableParentPartIndices(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->art_mesh_src.parent_part_idx;
}
/* ===== glue.c ===== */
/*
 * Purism Core: glue processing
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <string.h>

PSM__DEF void
psm__gather_glues(struct psm__model *m)
{
  psm__i32 count = m->glues.count;
  if (count <= 0)
    return;

  struct psm__glue *items = m->glues.items;
  if (!items)
    return;
  struct psm__sections *ms = m->source->sections;

  psm__i32 *keyform_base_idx = ms->glue_src.keyform_off;
  psm__f32 *intensity_src = ms->glue_key_src.intensity;
  if (!keyform_base_idx || !intensity_src)
    return;

  struct psm__binding *const *bindings = m->glues.bindings;

  struct psm__gather_channel ch[] = {
    { intensity_src, m->glues.keydata.intensity },
  };
  psm__gather_scalars(count, bindings, keyform_base_idx,
      &m->glues.keydata.interp, ch, 1);
}

PSM__DEF void
psm__apply_glues(struct psm__model *m)
{
  psm__i32 count = m->glues.count;
  if (count <= 0)
    return;

  struct psm__glue *items = m->glues.items;

  psm__f32 **pos = m->art_meshes.pos;
  psm__f32  *calc_int = m->glues.intensity;

  if (!items || !pos || !calc_int)
    return;

  for (psm__i32 gi = 0; gi < count; gi++) {
    struct psm__glue *glue = &items[gi];

    psm__i32 ic = glue->glue_info_count;
    if (ic <= 0)
      continue;

    psm__i32 m0 = glue->mesh_idx0, m1 = glue->mesh_idx1;

    psm__f32  intensity = calc_int[gi];
    psm__f32 *p0 = pos[m0], *p1 = pos[m1];
    if (!p0 || !p1)
      continue;

    psm__f32 *wt = glue->weights;
    psm__u16 *pi = glue->pos_idx;
    if (!wt || !pi)
      continue;

    for (psm__i32 i = 0; i + 1 < ic; i += 2) {
      psm__i32 i0 = pi[i], i1 = pi[i + 1];
      psm__f32 w0 = wt[i], w1 = wt[i + 1];

      struct psm__vec2 a = psm__v2_load(p0, i0);
      struct psm__vec2 b = psm__v2_load(p1, i1);
      struct psm__vec2 d = psm__v2_sub(b, a);

      psm__v2_store(p0, i0, psm__v2_add(a, psm__v2_scale(d, intensity * w0)));
      psm__v2_store(p1, i1, psm__v2_sub(b, psm__v2_scale(d, intensity * w1)));
    }
  }
}
/* ===== offscreen.c ===== */
/*
 * Purism Core: offscreen surface rendering
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <string.h>

PSM__DEF void
psm__enable_offscreens(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_53)
    return;

  psm__i32 count = m->offscreens.count;
  if (count <= 0)
    return;

  struct psm__offscreen *surfaces = m->offscreens.surfaces;

  bool *enable = m->offscreens.enable;

  for (psm__i32 i = 0; i < count; i++) {
    bool *oe = surfaces[i].owner_enable;
    enable[i] = oe ? *oe : 0;
  }
}

PSM__DEF void
psm__gather_offscreens(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_53)
    return;

  psm__i32 count = m->offscreens.count;
  if (count <= 0)
    return;

  struct psm__offscreen *surfaces = m->offscreens.surfaces;
  if (!surfaces)
    return;

  struct psm__sections *ms = m->source->sections;
  psm__f32             *opa_src = ms->offscreen_key_src.opacity;
  if (!opa_src)
    return;

  struct psm__offscreen_keydata *kd = &m->offscreens.keydata;

  psm__i32 off = 0;

  /* Opacity and weights */
  for (psm__i32 i = 0; i < count; i++) {
    struct psm__binding *b = surfaces[i].binding;
    if (!b)
      continue;
    if (!b->idx_dirty && !b->weight_dirty) {
      off += b->max_blend;
      continue;
    }

    psm__i32 nc = b->blend_count;
    kd->interp.blend_count[i] = nc;

    if (b->idx_dirty && nc > 0) {
      psm__i32 *kp = surfaces[i].keyform_idx;
      psm__i32  ki = kp ? *kp : -1;
      /* ki < 0 means "no keyforms"; any negative (not just -1) must be
       * skipped, else idx goes out of bounds (matches the color loop and
       * the psm__verify_offscreen_window load check). */
      if (ki >= 0) {
        for (psm__i32 j = 0; j < nc; j++) {
          psm__i32 idx = b->keyform_idx[j] + ki;
          kd->opacity[off + j] = opa_src[idx];
        }
      }
    }

    if (b->weight_dirty && nc > 0) {
      memcpy(&kd->interp.weights[off], b->weights, nc * sizeof(psm__f32));
    }

    off += b->max_blend;
  }

  /* Colors */
  psm__i32 *col_off = ms->offscreen_key_src.key_mul_color_off;
  psm__f32 *mr = ms->keyform_mul_color_src.r;
  psm__f32 *mg = ms->keyform_mul_color_src.g;
  psm__f32 *mb = ms->keyform_mul_color_src.b;
  psm__f32 *sr = ms->keyform_scr_color_src.r;
  psm__f32 *sg = ms->keyform_scr_color_src.g;
  psm__f32 *sb = ms->keyform_scr_color_src.b;

  if (!col_off || !mr || !mg || !mb || !sr || !sg || !sb)
    return;

  off = 0;

  for (psm__i32 i = 0; i < count; i++) {
    struct psm__binding *b = surfaces[i].binding;
    if (!b || !b->idx_dirty) {
      if (b)
        off += b->max_blend;
      continue;
    }

    psm__i32 *kp = surfaces[i].keyform_idx;
    psm__i32  ki = kp ? *kp : -1;
    psm__i32  nc = b->blend_count;

    if (ki >= 0 && nc > 0) {
      psm__i32 cb = col_off[ki];
      if (cb < 0)   /* offscreen keyform has no color override */
        goto skip_color;
      for (psm__i32 j = 0; j < nc; j++) {
        psm__i32 idx = b->keyform_idx[j] + cb;
        kd->mul_color.r[off + j] = mr[idx];
        kd->mul_color.g[off + j] = mg[idx];
        kd->mul_color.b[off + j] = mb[idx];
        kd->scr_color.r[off + j] = sr[idx];
        kd->scr_color.g[off + j] = sg[idx];
        kd->scr_color.b[off + j] = sb[idx];
      }
    }
  skip_color:
    off += b->max_blend;
  }
}

#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF int
csmGetOffscreenCount(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->offscreens.count;
}

PSMDEF const int *
csmGetOffscreenBlendModes(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->offscreen_src.blend_mode;
}

PSMDEF const float *
csmGetOffscreenOpacities(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->offscreens.opacity;
}

PSMDEF const int *
csmGetOffscreenOwnerIndices(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->offscreen_src.owner_idx;
}

PSMDEF const csmVector4 *
csmGetOffscreenMultiplyColors(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmVector4 *)m->offscreens.mul_color;
}

PSMDEF const csmVector4 *
csmGetOffscreenScreenColors(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmVector4 *)m->offscreens.scr_color;
}

PSMDEF const int *
csmGetOffscreenMaskCounts(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->offscreen_src.mask_len;
}

PSMDEF const int **
csmGetOffscreenMasks(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return m->source->sections->offscreen_src.drawable_mask_runtime;
}

PSMDEF const csmFlags *
csmGetOffscreenConstantFlags(const csmModel *model)
{
  const struct psm__model *m = (const struct psm__model *)model;
  return (const csmFlags *)m->source->sections->offscreen_src.drawable_flag;
}
#endif /* PSM_COMPAT_VERSION >= 0x06000000L */
/* ===== blendshape.c ===== */
/*
 * Purism Core: blend shape blending
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <math.h>

static inline psm__f32
psm__blend_interp_f32(const struct psm__blend_binding *binding,
    const psm__f32                                    *keyform_src)
{
  psm__i32 blend_count = binding->blend_count;
  psm__i32 off = binding->key_src_off;
  psm__f32 value;

  switch (blend_count) {
  case 0:
    return 0.0f;
  case 1: {
    psm__i32 idx0 = binding->keyform_idx[0] + off;
    value = keyform_src[idx0] * binding->weights[0];
    break;
  }
  case 2: {
    psm__i32 idx0 = binding->keyform_idx[0] + off;
    psm__i32 idx1 = binding->keyform_idx[1] + off;
    value = keyform_src[idx0] * binding->weights[0] +
            keyform_src[idx1] * binding->weights[1];
    break;
  }
  default:
    PSM__LOGF("invalid blend count %d", blend_count);
    return 0.0f;
  }

  return binding->weight * value;
}

static void
blend_scalar_f32(psm__i32 count, const struct psm__blend_shape *shapes,
    psm__f32 *values, const psm__f32 *keyform_src,
    psm__f32 lo, psm__f32 hi)
{
  for (psm__i32 i = 0; i < count; i++) {
    psm__i32 ti = shapes[i].target_idx;
    psm__i32 bc = shapes[i].binding_count;
    psm__f32 value = values[ti];

    struct psm__blend_binding *binds = shapes[i].bindings;
    if (bc > 0 && binds) {
      for (psm__i32 j = 0; j < bc; j++)
        value += psm__blend_interp_f32(&binds[j], keyform_src);
    }

    values[ti] = psm__clamp_f32(value, lo, hi);
  }
}

static void
blend_scalar_i32(psm__i32 count, const struct psm__blend_shape *shapes,
    psm__i32 *values, const psm__f32 *keyform_src)
{
  for (psm__i32 i = 0; i < count; i++) {
    psm__i32 ti = shapes[i].target_idx;
    psm__i32 bc = shapes[i].binding_count;
    psm__f32 value = (psm__f32)values[ti];

    struct psm__blend_binding *binds = shapes[i].bindings;
    if (bc > 0 && binds) {
      for (psm__i32 j = 0; j < bc; j++)
        value += psm__blend_interp_f32(&binds[j], keyform_src);
    }

    psm__f32 rounded = value + 0.001f;
    rounded = psm__clamp_f32(rounded, 0.0f, 1000.0f);
    values[ti] = (psm__i32)rounded;
  }
}

static void
psm__blend_positions(const struct psm__model *m, psm__i32 count,
    const struct psm__blend_shape *shapes, const psm__i32 *keyform_pos_off,
    psm__f32 **out_positions, const psm__i32 *vertex_counts)
{
  if (count <= 0)
    return;
  if (!shapes || !keyform_pos_off || !out_positions || !vertex_counts)
    return;

  struct psm__sections *ms = m->source->sections;
  psm__f32             *pos_xy = ms->key_pos_src.xy;
  if (!pos_xy)
    return;

  for (psm__i32 i = 0; i < count; i++) {
    psm__i32 ti = shapes[i].target_idx;
    psm__i32 bc = shapes[i].binding_count;
    if (bc <= 0)
      continue;

    psm__i32 vc = vertex_counts[ti];
    if (vc <= 0)
      continue;

    psm__i32  pc = vc * 2;
    psm__f32 *out = out_positions[ti];

    struct psm__blend_binding *binds = shapes[i].bindings;
    if (!out || !binds)
      continue;

    for (psm__i32 j = 0; j < bc; j++) {
      psm__i32 blend_count = binds[j].blend_count;
      if (blend_count == 0)
        continue;

      psm__i32 off = binds[j].key_src_off;
      psm__f32 cw = binds[j].weight;

      switch (blend_count) {
      case 1: {
        psm__i32  ki = binds[j].keyform_idx[0] + off;
        psm__i32  po = keyform_pos_off[ki];
        psm__f32 *p0 = &pos_xy[po];
        psm__f32  w0 = binds[j].weights[0];
        for (psm__i32 k = 0; k < pc; k++)
          out[k] += p0[k] * w0 * cw;
        break;
      }
      case 2: {
        psm__i32  ki0 = binds[j].keyform_idx[0] + off;
        psm__i32  ki1 = binds[j].keyform_idx[1] + off;
        psm__i32  po0 = keyform_pos_off[ki0];
        psm__i32  po1 = keyform_pos_off[ki1];
        psm__f32 *p0 = &pos_xy[po0];
        psm__f32 *p1 = &pos_xy[po1];
        psm__f32  w0 = binds[j].weights[0];
        psm__f32  w1 = binds[j].weights[1];
        for (psm__i32 k = 0; k < pc; k++)
          out[k] += (w0 * p0[k] + p1[k] * w1) * cw;
        break;
      }
      default:
        PSM__LOGF("invalid blend count %d", blend_count);
        break;
      }
    }
  }
}

static void
psm__blend_colors(psm__i32 count, const struct psm__blend_shape *shapes,
    const psm__i32 *keyform_color_off,
    const psm__f32 *src_r, const psm__f32 *src_g, const psm__f32 *src_b,
    psm__f32 *out)
{
  if (count <= 0)
    return;
  if (!shapes || !keyform_color_off || !src_r || !src_g || !src_b || !out)
    return;

  for (psm__i32 i = 0; i < count; i++) {
    psm__i32 ti = shapes[i].target_idx;
    psm__i32 bc = shapes[i].binding_count;
    psm__i32 ob = ti * 4;

    struct psm__blend_binding *binds = shapes[i].bindings;
    if (bc > 0 && binds) {
      for (psm__i32 j = 0; j < bc; j++) {
        psm__i32 blend_count = binds[j].blend_count;
        if (blend_count == 0)
          continue;

        psm__i32 off = binds[j].key_src_off;
        psm__f32 cw = binds[j].weight;
        psm__f32 r, g, b;

        switch (blend_count) {
        case 1: {
          psm__i32 ki = binds[j].keyform_idx[0] + off;
          psm__i32 ci = keyform_color_off[ki];
          if (ci < 0)   /* keyform has no color override */
            continue;
          psm__f32 w0 = binds[j].weights[0];
          r = src_r[ci] * w0;
          g = src_g[ci] * w0;
          b = src_b[ci] * w0;
          break;
        }
        case 2: {
          psm__i32 ki0 = binds[j].keyform_idx[0] + off;
          psm__i32 ki1 = binds[j].keyform_idx[1] + off;
          psm__i32 ci0 = keyform_color_off[ki0];
          psm__i32 ci1 = keyform_color_off[ki1];
          if (ci0 < 0 || ci1 < 0)   /* keyform has no color override */
            continue;
          psm__f32 w0 = binds[j].weights[0];
          psm__f32 w1 = binds[j].weights[1];
          r = w0 * src_r[ci0] + src_r[ci1] * w1;
          g = w0 * src_g[ci0] + src_g[ci1] * w1;
          b = w0 * src_b[ci0] + src_b[ci1] * w1;
          break;
        }
        default:
          PSM__LOGF("invalid blend count %d", blend_count);
          continue;
        }

        out[ob + 0] += r * cw;
        out[ob + 1] += g * cw;
        out[ob + 2] += b * cw;
      }
    }

    out[ob + 0] = psm__clamp_f32_01(out[ob + 0]);
    out[ob + 1] = psm__clamp_f32_01(out[ob + 1]);
    out[ob + 2] = psm__clamp_f32_01(out[ob + 2]);
  }
}

PSM__DEF void
psm__blend_parts(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_50)
    return;

  psm__i32 count = m->bs_parts.count;
  if (count <= 0)
    return;

  struct psm__blend_shape *shapes = m->bs_parts.items;
  if (!shapes)
    return;

  struct psm__sections *ms = m->source->sections;
  psm__i32             *calc_do = m->parts.draw_order;
  psm__f32             *do_src = ms->part_key_src.draw_order;

  if (!calc_do || !do_src)
    return;

  blend_scalar_i32(count, shapes, calc_do, do_src);
}

PSM__DEF void
psm__blend_warps(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_42)
    return;

  struct psm__sections    *ms = m->source->sections;
  struct psm__blend_shape *shapes = m->bs_warps.items;

  psm__i32 count = m->bs_warps.count;

  if (count <= 0 || !shapes)
    return;

  psm__blend_positions(m, count, shapes, ms->warp_key_src.key_pos_off,
      m->deformers.warps.pos, ms->warp_src.vertex_count);

  if (m->source->header->version < csmMocVersion_50)
    return;

  psm__f32 *op_src = ms->warp_key_src.opacity;
  psm__f32 *calc_op = m->deformers.warps.opacity;

  if (!op_src || !calc_op)
    return;

  blend_scalar_f32(count, shapes, calc_op, op_src, 0.0f, 1.0f);

  psm__blend_colors(count, shapes, ms->warp_key_src.key_mul_color_off,
      ms->keyform_mul_color_src.r, ms->keyform_mul_color_src.g,
      ms->keyform_mul_color_src.b,
      m->deformers.warps.mul_color);

  psm__blend_colors(count, shapes, ms->warp_key_src.key_scr_color_off,
      ms->keyform_scr_color_src.r, ms->keyform_scr_color_src.g,
      ms->keyform_scr_color_src.b,
      m->deformers.warps.scr_color);
}

PSM__DEF void
psm__blend_rotations(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_50)
    return;

  struct psm__sections    *ms = m->source->sections;
  struct psm__blend_shape *shapes = m->bs_rotations.items;

  psm__i32 count = m->bs_rotations.count;

  if (count <= 0 || !shapes)
    return;

  psm__f32 *ox_src = ms->rotation_key_src.origin_x;
  psm__f32 *calc_ox = m->deformers.rotations.origin_x;
  if (ox_src && calc_ox)
    blend_scalar_f32(count, shapes, calc_ox, ox_src, -INFINITY, INFINITY);

  psm__f32 *oy_src = ms->rotation_key_src.origin_y;
  psm__f32 *calc_oy = m->deformers.rotations.origin_y;
  if (oy_src && calc_oy)
    blend_scalar_f32(count, shapes, calc_oy, oy_src, -INFINITY, INFINITY);

  psm__f32 *op_src = ms->rotation_key_src.opacity;
  psm__f32 *calc_op = m->deformers.rotations.opacity;
  if (op_src && calc_op)
    blend_scalar_f32(count, shapes, calc_op, op_src, 0.0f, 1.0f);

  psm__blend_colors(count, shapes, ms->rotation_key_src.key_mul_color_off,
      ms->keyform_mul_color_src.r, ms->keyform_mul_color_src.g,
      ms->keyform_mul_color_src.b,
      m->deformers.rotations.mul_color);

  psm__blend_colors(count, shapes, ms->rotation_key_src.key_scr_color_off,
      ms->keyform_scr_color_src.r, ms->keyform_scr_color_src.g,
      ms->keyform_scr_color_src.b,
      m->deformers.rotations.scr_color);

  psm__f32 *ang_src = ms->rotation_key_src.angle;
  psm__f32 *calc_ang = m->deformers.rotations.angle;
  if (ang_src && calc_ang)
    blend_scalar_f32(count, shapes, calc_ang, ang_src, -3600.0f, 3600.0f);

  psm__f32 *sc_src = ms->rotation_key_src.scale;
  psm__f32 *calc_sc = m->deformers.rotations.scale;
  if (sc_src && calc_sc)
    blend_scalar_f32(count, shapes, calc_sc, sc_src, 0.0001f, 100.0f);
}

PSM__DEF void
psm__blend_art_meshes(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_42)
    return;

  struct psm__sections    *ms = m->source->sections;
  struct psm__blend_shape *shapes = m->bs_art_meshes.items;

  psm__i32 count = m->bs_art_meshes.count;
  if (count <= 0 || !shapes)
    return;

  psm__blend_positions(m, count, shapes, ms->art_mesh_key_src.key_pos_off,
      m->art_meshes.pos, ms->art_mesh_src.vertex_count);

  if (m->source->header->version < csmMocVersion_50)
    return;

  psm__f32 *do_src = ms->art_mesh_key_src.draw_order;
  psm__i32 *calc_do = m->art_meshes.draw_order;
  if (do_src && calc_do)
    blend_scalar_i32(count, shapes, calc_do, do_src);

  psm__f32 *op_src = ms->art_mesh_key_src.opacity;
  psm__f32 *calc_op = m->art_meshes.opacity;
  if (op_src && calc_op)
    blend_scalar_f32(count, shapes, calc_op, op_src, 0.0f, 1.0f);

  if (ms->art_mesh_key_src.key_mul_color_off &&
      ms->keyform_mul_color_src.r && ms->keyform_mul_color_src.g &&
      ms->keyform_mul_color_src.b && m->art_meshes.mul_color) {
    psm__blend_colors(count, shapes, ms->art_mesh_key_src.key_mul_color_off,
        ms->keyform_mul_color_src.r, ms->keyform_mul_color_src.g,
        ms->keyform_mul_color_src.b,
        m->art_meshes.mul_color);
  }

  if (ms->art_mesh_key_src.key_scr_color_off &&
      ms->keyform_scr_color_src.r && ms->keyform_scr_color_src.g &&
      ms->keyform_scr_color_src.b && m->art_meshes.scr_color) {
    psm__blend_colors(count, shapes, ms->art_mesh_key_src.key_scr_color_off,
        ms->keyform_scr_color_src.r, ms->keyform_scr_color_src.g,
        ms->keyform_scr_color_src.b,
        m->art_meshes.scr_color);
  }
}

PSM__DEF void
psm__blend_glues(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_50)
    return;

  psm__i32 count = m->bs_glues.count;
  if (count <= 0)
    return;

  struct psm__blend_shape *shapes = m->bs_glues.items;
  if (!shapes)
    return;

  struct psm__sections *ms = m->source->sections;

  psm__f32 *calc_int = m->glues.intensity;
  psm__f32 *int_src = ms->glue_key_src.intensity;
  if (!calc_int || !int_src)
    return;

  blend_scalar_f32(count, shapes, calc_int, int_src, 0.0f, 1.0f);
}

PSM__DEF void
psm__blend_offscreens(struct psm__model *m)
{
  if (m->source->header->version < csmMocVersion_53)
    return;

  struct psm__sections    *ms = m->source->sections;
  struct psm__blend_shape *shapes = m->bs_offscreens.items;

  psm__i32 count = m->bs_offscreens.count;

  if (count <= 0 || !shapes)
    return;

  psm__f32 *op_src = ms->offscreen_key_src.opacity;
  psm__f32 *calc_op = m->offscreens.opacity;
  if (op_src && calc_op)
    blend_scalar_f32(count, shapes, calc_op, op_src, 0.0f, 1.0f);

  psm__blend_colors(count, shapes, ms->offscreen_key_src.key_mul_color_off,
      ms->keyform_mul_color_src.r, ms->keyform_mul_color_src.g,
      ms->keyform_mul_color_src.b,
      m->offscreens.mul_color);

  psm__blend_colors(count, shapes, ms->offscreen_key_src.key_scr_color_off,
      ms->keyform_scr_color_src.r, ms->keyform_scr_color_src.g,
      ms->keyform_scr_color_src.b,
      m->offscreens.scr_color);
}
/* ===== interpolate.c ===== */
/*
 * Purism Core: keyform interpolation
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <math.h>
#include <string.h>

static void
psm__interp_f32(struct psm__interp *interp, const psm__f32 *targets,
    psm__f32 *out, psm__i32 out_stride, const bool *enable)
{
  if (!interp || !targets || !out)
    return;

  psm__i32 obj_count = interp->object_count;
  if (obj_count <= 0)
    return;

  psm__i32 *max_comb = interp->max_blend;
  psm__i32 *comb = interp->blend_count;
  psm__f32 *wt = interp->weights;
  psm__f32 *tmp = interp->tmp;
  psm__i32  tmp_len = interp->tmp_len;

  if (!max_comb || !comb || !wt)
    return;

  /*
   * Pre-multiply targets by weights into scratch buffer.
   * Avoids redundant multiplies when the same weight layout
   * is reused across channels (e.g. R/G/B color interp).
   */
  if (tmp && tmp_len > 0) {
    for (psm__i32 i = 0; i < tmp_len; i++)
      tmp[i] = targets[i] * wt[i];
  }

  psm__i32 off = 0;
  for (psm__i32 i = 0; i < obj_count; i++) {
    psm__i32 mc = max_comb[i];
    if (enable == NULL || enable[i]) {
      psm__i32 n = comb[i];
      psm__f32 sum = 0.0f;
      if (tmp) {
        for (psm__i32 j = 0; j < n; j++)
          sum += tmp[off + j];
      } else {
        for (psm__i32 j = 0; j < n; j++)
          sum += targets[off + j] * wt[off + j];
      }
      out[i * out_stride] = sum;
    }
    off += mc;
  }
}

static void
psm__interp_i32(struct psm__interp *interp, const psm__f32 *targets,
    psm__i32 *out, const bool *enable)
{
  if (!interp || !targets || !out)
    return;

  psm__i32 obj_count = interp->object_count;
  if (obj_count <= 0)
    return;

  psm__i32 *max_comb = interp->max_blend;
  psm__i32 *comb = interp->blend_count;
  psm__f32 *wt = interp->weights;
  psm__f32 *tmp = interp->tmp;
  psm__i32  tmp_len = interp->tmp_len;

  if (!max_comb || !comb || !wt)
    return;

  if (tmp && tmp_len > 0) {
    for (psm__i32 i = 0; i < tmp_len; i++)
      tmp[i] = targets[i] * wt[i];
  }

  psm__i32 off = 0;
  for (psm__i32 i = 0; i < obj_count; i++) {
    psm__i32 mc = max_comb[i];
    if (enable == NULL || enable[i]) {
      psm__i32 n = comb[i];
      psm__f32 sum = 0.0f;
      if (tmp) {
        for (psm__i32 j = 0; j < n; j++)
          sum += tmp[off + j];
      } else {
        for (psm__i32 j = 0; j < n; j++)
          sum += targets[off + j] * wt[off + j];
      }
      out[i] = psm__f32_to_i32(sum + 0.001f);
    }
    off += mc;
  }
}

static void
psm__interp_f32_array(struct psm__interp *interp,
    psm__f32 **targets, psm__f32 **out,
    const psm__i32 *counts, psm__i32 elem_count, const bool *enable)
{
  if (!interp || !targets || !out || !counts)
    return;

  psm__i32  obj_count = interp->object_count;
  psm__i32 *max_comb = interp->max_blend;
  psm__i32 *comb = interp->blend_count;
  psm__f32 *wt = interp->weights;

  if (!max_comb || !comb || !wt || obj_count <= 0)
    return;

  psm__i32 off = 0;
  for (psm__i32 i = 0; i < obj_count; i++) {
    psm__i32 mc = max_comb[i];
    if (enable == NULL || enable[i]) {
      psm__i32 total = counts[i] * elem_count;
      if (total <= 0) {
        off += mc;
        continue;
      }
      psm__i32  n = comb[i];
      psm__f32 *dst = out[i];
      if (dst) {
        memset(dst, 0, total * sizeof(psm__f32));
        for (psm__i32 j = 0; j < n; j++) {
          psm__f32  w = wt[off + j];
          psm__f32 *src = targets[off + j];
          if (src) {
            for (psm__i32 k = 0; k < total; k++)
              dst[k] += src[k] * w;
          }
        }
      }
    }
    off += mc;
  }
}

static void
psm__interp_colors(
    struct psm__interp       *interp,
    const struct psm__color3 *kd_mul,
    const struct psm__color3 *kd_scr,
    psm__f32                 *mul_out,
    psm__f32                 *scr_out,
    const bool               *enable)
{
  psm__interp_f32(interp, kd_mul->r, mul_out + 0, 4, enable);
  psm__interp_f32(interp, kd_mul->g, mul_out + 1, 4, enable);
  psm__interp_f32(interp, kd_mul->b, mul_out + 2, 4, enable);
  psm__interp_f32(interp, kd_scr->r, scr_out + 0, 4, enable);
  psm__interp_f32(interp, kd_scr->g, scr_out + 1, 4, enable);
  psm__interp_f32(interp, kd_scr->b, scr_out + 2, 4, enable);
}

PSM__DEF void
psm__interp_parts(struct psm__model *m)
{
  struct psm__interp *ip = &m->parts.keydata.interp;
  psm__interp_i32(ip, m->parts.keydata.draw_order, m->parts.draw_order,
      m->parts.enable);
}

PSM__DEF void
psm__interp_warps(struct psm__model *m)
{
  struct psm__warps    *w = &m->deformers.warps;
  struct psm__interp   *ip = &w->keydata.interp;
  struct psm__sections *ms = m->source->sections;

  psm__i32 *vc = ms->warp_src.vertex_count;
  bool     *en = w->enable;

  psm__interp_f32(ip, w->keydata.opacity, w->opacity, 1, en);

  if (vc && w->keydata.pos && w->pos)
    psm__interp_f32_array(ip, w->keydata.pos, w->pos, vc, 2, en);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__interp_colors(ip, &w->keydata.mul_color, &w->keydata.scr_color,
      w->mul_color, w->scr_color, en);
}

PSM__DEF void
psm__interp_rotations(struct psm__model *m)
{
  struct psm__rotations *r = &m->deformers.rotations;
  struct psm__interp    *ip = &r->keydata.interp;

  bool *en = r->enable;

  psm__interp_f32(ip, r->keydata.opacity, r->opacity, 1, en);
  psm__interp_f32(ip, r->keydata.angle, r->angle, 1, en);
  psm__interp_f32(ip, r->keydata.origin_x, r->origin_x, 1, en);
  psm__interp_f32(ip, r->keydata.origin_y, r->origin_y, 1, en);
  psm__interp_f32(ip, r->keydata.scale, r->scale, 1, en);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__interp_colors(ip, &r->keydata.mul_color, &r->keydata.scr_color,
      r->mul_color, r->scr_color, en);
}

PSM__DEF void
psm__interp_art_meshes(struct psm__model *m)
{
  struct psm__art_meshes *am = &m->art_meshes;
  struct psm__interp     *ip = &am->keydata.interp;
  struct psm__sections   *ms = m->source->sections;

  psm__i32 *vc = ms->art_mesh_src.vertex_count;
  bool     *en = am->enable;

  psm__interp_f32(ip, am->keydata.opacity, am->opacity, 1, en);
  psm__interp_i32(ip, am->keydata.draw_order, am->draw_order, en);

  if (vc && am->keydata.pos && am->pos)
    psm__interp_f32_array(ip, am->keydata.pos, am->pos, vc, 2, en);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__interp_colors(ip, &am->keydata.mul_color, &am->keydata.scr_color,
      am->mul_color, am->scr_color, en);
}

PSM__DEF void
psm__interp_glues(struct psm__model *m)
{
  struct psm__glues *g = &m->glues;
  if (g->count <= 0 || !g->keydata.intensity || !g->intensity)
    return;
  psm__interp_f32(&g->keydata.interp, g->keydata.intensity, g->intensity, 1,
      NULL);
}

PSM__DEF void
psm__interp_offscreens(struct psm__model *m)
{
  struct psm__offscreens *os = &m->offscreens;
  struct psm__interp     *ip = &os->keydata.interp;

  bool *en = os->enable;

  if (m->source->header->version < csmMocVersion_53)
    return;

  psm__interp_f32(ip, os->keydata.opacity, os->opacity, 1, en);

  psm__interp_colors(ip, &os->keydata.mul_color, &os->keydata.scr_color,
      os->mul_color, os->scr_color, en);
}
/* ===== render.c ===== */
/*
 * Purism Core: render order calculation and dynamic flags
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <string.h>

PSM__DEF void
psm__sort_render_order(struct psm__model *m)
{
  struct psm__draw_groups *dog = &m->draw_groups;

  psm__i32 group_count = dog->count;
  if (group_count <= 0)
    return;

  struct psm__draw_group *groups = dog->groups;
  if (!groups)
    return;

  struct psm__art_meshes *am = &m->art_meshes;
  struct psm__parts      *pt = &m->parts;

  psm__i32 *am_draw = am->draw_order;
  psm__i32 *pt_draw = pt->draw_order;
  bool     *am_en = am->enable;
  bool     *pt_en = pt->enable;
  psm__i32  am_cnt = am->count;

  /* First assign draw orders to items */
  for (psm__i32 gi = 0; gi < group_count; gi++) {
    struct psm__draw_group *c = &groups[gi];

    psm__i32 n = c->count;
    if (n <= 0)
      continue;
    struct psm__draw_item *it = c->items;
    if (!it)
      continue;

    for (psm__i32 j = 0; j < n; j++) {
      struct psm__draw_item *item = &it[j];

      psm__i32 oi = item->object_idx;

      if (item->object_type == 1) {
        if (pt_en[oi])
          item->draw_order = pt_draw[oi];
        else
          item->draw_order = c->min_order;
      } else {
        if (am_en[oi])
          item->draw_order = am_draw[oi];
        else
          item->draw_order = c->min_order;
      }
    }
  }

  /* Now time for sorting and render order assignment */
  psm__i32 *render_order = m->render_order;
  psm__u8   ver = m->source->header->version;

  struct psm__draw_sort *srt = &dog->sort;

  psm__i32 *first = srt->first;
  psm__i32 *last = srt->last;
  psm__i32 *next = srt->next;

  if (!first || !last || !next)
    return;

  /* Compute max values from source for bounds checking */
  psm__i32 max_level = 0, max_items = 0;

  struct psm__sections   *ms = m->source->sections;
  struct psm__count_info *cnt = ms->count_info;

  if (cnt->draw_groups > 0 && ms->draw_group_src.obj_len &&
      ms->draw_group_src.max_order && ms->draw_group_src.min_order) {
    for (psm__i32 i = 0; i < cnt->draw_groups; i++) {
      psm__i32 gc = ms->draw_group_src.obj_len[i];
      psm__i32 hi = ms->draw_group_src.max_order[i];
      psm__i32 lo = ms->draw_group_src.min_order[i];
      psm__i32 lv = psm__safe_order_level(hi, lo);
      if (lv > max_level) max_level = lv;
      if (gc > max_items) max_items = gc;
    }
  }

  if (max_level <= 0 || max_items <= 0)
    return;

  for (psm__i32 gi = 0; gi < group_count; gi++) {
    struct psm__draw_group *c = &groups[gi];

    psm__i32 olevel = c->order_level;
    psm__i32 n = c->count;

    if (olevel <= 0 || n <= 0)
      continue;
    if (olevel > max_level || n > max_items)
      continue;

    /* Clear sorting buckets */
    if ((psm_size)olevel > (psm_size)-1 / sizeof(psm__i32))
      continue;
    psm_size olsz = (psm_size)olevel * sizeof(psm__i32);
    memset(first, 0xFF, olsz);
    memset(last, 0xFF, olsz);

    if ((psm_size)n > (psm_size)-1 / sizeof(psm__i32))
      continue;
    memset(next, 0xFF, (psm_size)n * sizeof(psm__i32));

    /* Bucket items by relative draw order */
    struct psm__draw_item *it = c->items;
    if (!it)
      continue;
    for (psm__i32 j = 0; j < n; j++) {
      psm__i32 rel =
          (psm__i32)((psm__u32)it[j].draw_order - (psm__u32)c->min_order);
      rel = psm__clamp_idx(rel, olevel);

      if (last[rel] == -1)
        first[rel] = j;
      else
        next[last[rel]] = j;
      last[rel] = j;
    }

    /* Walk buckets in order and assign render positions */
    psm__i32 pos = c->cursor;
    bool     has_offscr = (ver >= csmMocVersion_53);

    for (psm__i32 oi = 0; oi < olevel; oi++) {
      psm__i32 di = first[oi];
      while (di != -1 && di < n) {
        struct psm__draw_item *item = &it[di];

        psm__i32 obj = item->object_idx;
        psm__i32 grp = item->group_idx;

        if (item->object_type == 1) {
          if (has_offscr) {
            psm__i32 oidx = ms->part_src.offscreen_idx[obj];
            if (oidx >= 0)
              render_order[am_cnt + oidx] = pos++;
          }
          /* F3: a part draw item (type 1) always has a valid child group;
           * self_group_idx >= 0 is proved at load (psm__verify_idx). */
          if (grp < group_count) {
            struct psm__draw_group *nc = &groups[grp];
            nc->cursor = pos;
            pos += nc->total_count;
          }
        } else {
          render_order[obj] = pos++;
        }

        psm__i32 ni = next[di];
        if (ni <= di)
          break;
        di = ni;
      }
    }
  }
}

#if PSM_COMPAT_VERSION >= 0x06000000L
PSMDEF const int *
csmGetRenderOrders(const csmModel *model)
{
  return ((const struct psm__model *)model)->render_order;
}
#else
PSMDEF const int *
csmGetDrawableRenderOrders(const csmModel *model)
{
  return ((const struct psm__model *)model)->render_order;
}
#endif

#endif /* PURISM_CORE_IMPLEMENTATION */
#endif /* PURISM_CORE_BUNDLE_H */
