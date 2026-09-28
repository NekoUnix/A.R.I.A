# ARIA Model Core: independent Rust replacement

The target runtime is entirely authored in Rust by ARIA. Purism Core is a
temporary **comparison oracle**, not the implementation or a source fork for
this crate. The desktop application still uses the current Purism-backed
`aria-live2d` worker until the replacement meets the gates below. Do not label
the current application as Purism-free, remove its MIT notice, or switch the
worker simply because a file parses successfully.

`crates/aria-model-core` currently implements bounded, immutable MOC3 container
reading, model counts, canvas and parameter metadata, ArtMesh IDs, atlas slots,
UVs, triangle indices and masks. It decodes normal-parameter key tables,
ArtMesh and deformer keyforms, and warp/rotation parent hierarchies. The Rust
geometry evaluator interpolates parameter keyforms and transforms mesh vertices
through mixed warp and rotation chains, including grid extrapolation. A
model-scoped evaluator decodes static bindings, mesh keyforms and blend-shape
delta keyforms into RAM once, then computes each parent deformer once per frame
and applies decoded glue constraints. Its decoded mesh and warp position data
share a 512 MiB budget. The evaluator now applies blend-shape deltas to
ArtMeshes, warp and rotation deformers, and glue intensities. It does
not run C code, relocate pointers in model bytes, load an SDK, or include
copied Purism source. The crate is in the workspace for development and
testing; it is **not** the active evaluator yet. The model-scoped path still
allocates per frame and has no end-to-end frame-time measurement, so it is not
a measured 120 FPS implementation.

`resident::ResidentModel` is the first production connection to this core.
On avatar load, it reads the complete MOC3 source and every declared encoded
texture atlas into bounded, immutable system RAM. The desktop renderer decodes
from those resident bytes, so atlas files are not reread during rendering.
The current evaluator still reads its own MOC copy, and the renderer still
uploads all atlases to the GPU at load time. This change establishes a stable
RAM source for future atlas residency and eviction; it does **not** yet reduce
VRAM or improve frame rate. Atlas eviction must account for every visible
ArtMesh and its mask sources, use a measured budget and prefetch transitions
before dropping GPU resources so an expression or outfit change does not show
missing artwork or stall a frame. Encoded atlases use their compressed file
size in RAM; decoded pixel buffers are temporary during the initial upload.

## Replacement gates

1. Complete versioned MOC3 decoding with explicit bounds and relationship
   checks, including v1–v6 differences, endian handling, parameters, bindings,
   blend shapes, parts, deformers, glues, draw order, colors and offscreen data.
2. Evaluate parameter keys and interpolation, parent hierarchies, warp and
   rotation deformers, mesh positions, opacity, color, visibility and dynamic
   flags in Rust. Preserve the renderer-facing mesh contract and worker crash
   isolation.
3. Compare static data and animated output against both the current runtime
   and an official, locally installed Cubism Core on model exports the user can
   run locally. Cover default, extreme, repeated, blend-shape and physics-driven
   inputs. Investigate disagreements visually instead of assuming either
   reference is always correct.
4. Fuzz malformed MOC3 input, check memory limits, profile large and small
   avatars, and validate rendering and physics at target frame rates. A 120 FPS
   claim requires end-to-end frame measurements, not a core-only benchmark.
5. Switch the desktop and worker to the Rust implementation, remove the
   Purism C source/build dependency and current-runtime test linkage, update
   package notices and documentation, then test shipped builds on supported
   platforms and the supplied local models.

## Current evidence

On 2026-09-27, the Rust container and static-mesh reader passed its own unit
tests and parsed 28 local MOC3 files from the supplied VTube Models and VTube
Studio folders. IDs, parameter ranges, UVs, triangle indices, masks, and atlas
slots matched the current runtime for all 11 Steam models and 16 of 17 models
from Documents. The remaining `Ditto Eevees.moc3` is accepted by the Rust
static reader but rejected by the existing Purism consistency check, so it
has no current-runtime parity result. This is a format-validation issue to
investigate before a production switchover. None of these checks establish
deformation parity or live performance for the Rust evaluator.

On 2026-09-28, a developer-only harness directly loaded the official Cubism
Core 5 DLL installed with VTube Studio. ARIA's Rust-decoded mesh IDs, UVs,
triangle indices, atlas slots and parameter IDs matched that Core on the
large, chibi and Nullas models. Across five animated parameter frames, the
current Purism-backed runtime differed from official Cubism by at most
`0.0000083`, `0.0000073` and `0.0000016` model units respectively. This was the
reference baseline established before the Rust geometry evaluator was added.
The official DLL is neither referenced by production code nor copied to the
repository or release. It is supplied through `ARIA_CUBISM_CORE` only for an
ignored local test.

The Rust geometry work added on 2026-09-28 passed its own interpolation,
reflection, hierarchy and extrapolation tests. Before the blend-shape addition,
the mixed warp/rotation parity test found zero mismatched visible mesh frames
across 27 of the 28 locally supplied MOC3 exports at default and normal
parameter extremes, with the largest coordinate difference below `0.0000015`
model units. `Ditto Eevees.moc3` remains rejected by the existing runtime's
loader. The expanded blend-shape evaluator also passed this three-frame parity
test on the same 27 comparable exports. The direct official comparisons below
include blend-shape parameter extremes; more input combinations still need
checking.
Opacity, draw order, visibility, color, offscreen behavior and final rendered
pixels are not covered by this geometry result. A test passing on these frames
does not make the evaluator production-ready or prove a performance gain.

An optimized local diagnostic on the OILBUN export measured the current
runtime's deformation-only path at roughly `0.71 ms/frame` and the Rust
geometry path at roughly `3.29 ms/frame`. The Rust path now evaluates geometry
for all 284 of that export's ArtMeshes. The Rust implementation is currently
slower at this stage; the measurement gives a baseline for eliminating keyform,
transform and output-allocation costs. It excludes render, GPU upload, capture
and desktop composition time.

The developer-only official Core comparison now includes Rust blend-shape
geometry directly. Across seven animated frames, including blend-shape
parameter extremes, maximum Rust-to-official coordinate differences were
`0.00000363` on OILBUN, `0.00000557` on Hiyori, and `0.00000781` on a supplied
NekoUnix outfit export. Those tests covered 1,909, 912, and 6,123 visible
mesh frames respectively, with zero mismatches above `0.001` model units.
The official DLL remains local and is never linked into the application.
This checks positions only. It does not validate opacity, colors, render order,
visibility, output pixels or end-to-end performance.

The already-Rust ARIA physics solver was also adjusted with an explicit
frequency-controlled return torque for Natural and Bouncy modes. This gives
the Bouncy chain a defined restoring force and measurable overshoot while
retaining a fixed-length, bounded, frame-rate-stable simulation. Per-particle
damping coefficients are now cached until tuning changes, removing repeated
power calculations from every substep. Physics tests cover rebound, 30/60/120
FPS, extreme settings, cache invalidation and deterministic replay. End-to-end
120 FPS remains unverified.

Set `ARIA_TEST_MOC` to a local `.moc3` and run the ignored
`rust_mesh_topology_matches_current_runtime` test in `aria-live2d` to repeat
the static comparison. Assets stay on the user's machine and are never added
to the repository.

Run the ignored `rust_deformer_chain_positions_match_current_runtime` test
with the same variable to repeat the supported animated geometry comparison.
It reports the number of visible mesh frames and the largest coordinate delta.

Set `ARIA_CUBISM_CORE` to an official Core DLL you already have and run the
ignored `official_cubism_matches_current_runtime_on_animated_vertices` test
to repeat the direct Cubism comparison. No DLL path is hardcoded into ARIA.
