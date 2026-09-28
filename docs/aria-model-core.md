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
model-scoped evaluator decodes static bindings, mesh and deformer keyforms, and
blend-shape delta keyforms into RAM once. It then computes each parent deformer
once per frame and applies decoded glue constraints. Its decoded mesh and warp
position data share a 512 MiB budget. The evaluator applies blend-shape deltas
to ArtMeshes, warp and rotation deformers, and glue intensities. It also
resolves part hierarchy, caller-controlled part opacity, binding-range
visibility, authored multiply/screen colors, integer ArtMesh draw order and
hierarchical final render order. The draw-group evaluator validates unique
ownership, acyclic relationships and declared descendant counts, then visits
groups in parent-before-child order even if they are stored differently. A
renderer-facing adapter is available in the
isolated worker when `ARIA_EXPERIMENTAL_RUST_CORE=1` is set. An additional
`ARIA_EXPERIMENTAL_DIRECT_RUST_CORE=1` path evaluates the same Rust core in the
desktop process without worker serialization. Both are opt-in comparison paths;
the default and distributed runtime still use Purism. The direct path loses
worker crash isolation and must remain experimental until the replacement gates
are met.
After compilation,
frame evaluation no longer borrows or reads the encoded MOC3 bytes. It does
not run C code, relocate pointers in model bytes, load an SDK, or include
copied Purism source. The model-scoped path still allocates per frame and is
slower than the current runtime on the measured large export. It is not a
measured 120 FPS implementation.

The evaluator now retains completed deformer states in RAM. It tracks the
normal key tables and blend-shape/constraint parameters that can affect each
deformer, then recomputes a node only when one of those inputs or its parent
state changes. Part activation is checked every frame. Disabled or newly
enabled deformers invalidate their descendants; parameter values are compared
after range and repeat handling. A failed frame leaves the cache incomplete,
forcing full evaluation on the next attempt. Mesh keyforms, glue and final
render ordering are still evaluated each frame. `ARIA_PERF_RUST_CORE=1` prints
120-frame averages for axes, deformer keyforms/resolution, meshes and glue to
stderr for local profiling.

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
from Documents. `Ditto Eevees.moc3` is accepted by the Rust static reader but
rejected by the existing Purism consistency check, so it has no Purism parity
result. A later direct official-Core comparison is recorded below. These early
checks alone did not establish deformation parity or live performance.

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
model units. `Ditto Eevees.moc3` is rejected by the existing runtime's
loader. The expanded blend-shape evaluator also passed this three-frame parity
test on the same 27 comparable exports. The direct official comparisons below
include blend-shape parameter extremes; more input combinations still need
checking.
All dynamic-flag transitions and offscreen behavior are not covered by this
geometry result. A test passing on these frames does not make the evaluator
production-ready or prove a performance gain.

An optimized local diagnostic on the OILBUN export measured the current
runtime's deformation-only path at roughly `0.64 ms/frame` and the Rust
geometry path at roughly `2.54 ms/frame`. Decoding normal deformer keyforms
into RAM reduced the Rust diagnostic from `3.29` to `2.54 ms/frame` on this
machine. The Rust path evaluates geometry
for all 284 of that export's ArtMeshes. The Rust implementation is currently
slower at this stage; the measurement gives a baseline for eliminating keyform,
transform and output-allocation costs. It excludes render, GPU upload, capture
and desktop composition time.

The developer-only official Core comparison includes Rust blend-shape geometry
directly. Across seven animated frames, including blend-shape parameter
extremes and varied part opacities, maximum Rust-to-official coordinate differences were
`0.00000363` on OILBUN, `0.00000557` on Hiyori, and `0.00000781` on a supplied
NekoUnix outfit export. Those tests covered 1,526, 851, and 4,834 visible
mesh frames respectively, with zero mismatches above `0.001` model units.
Part IDs matched. Maximum drawable-opacity differences were below `0.00000006`
on all three, and sampled visibility, multiply/screen colors and integer
ArtMesh draw orders matched. The extended local sweep passed direct official
comparisons on all 27 exports also accepted by the current runtime. It exposed
and resolved binding-range visibility and integer draw-order edge cases.
The official DLL remains local and is never linked into the application.
The same 27 comparable exports subsequently matched official Cubism's final
render-order ranks across seven frames after ARIA decoded hierarchical draw
groups. Opt-in Rust worker rendering was tested on OILBUN and a large 90s
outfit export. Their default-pose images differed from the current renderer on
3,017 of 3,854,336 and 1,963 of 2,779,136 pixels respectively, mostly
subpixel edges; each had five pixels with channel difference above 32. This
does not validate every dynamic flag, output pose, or end-to-end performance.
The direct harness now permits the transitional runtime to reject an export
while comparing Rust to official Core. All 28 supplied MOC3 exports passed
seven animated comparison frames, including `Ditto Eevees.moc3`: its maximum
coordinate delta was below `0.00000047`, with zero visibility, color, opacity,
draw-order or final render-order mismatches across 51 visible mesh frames.
The current runtime still rejects that export.

The in-process adapter was tested with two independent Ditto Eevees instances.
The large 90s outfit rendered with the same pixels as the Rust worker at its
default pose (1,357 × 2,048). The evaluator now hands its completed position
vectors to the renderer-facing drawable without another full vertex copy; the
host also adopts the decoded worker frame vector. These changes reduce CPU
memory copying but do not move MOC3 geometry or physics evaluation onto the
GPU. The direct adapter is a profiling path, not a production switch.

A local topology survey also bounded a tempting but insufficient GPU shortcut:
the large 90s outfit has 1,210 ArtMeshes and 225,448 vertices, but no ArtMesh
is both unparented and free of glue. It has 740 deformers, a maximum parent
depth of 19 and 819,275 warp control points. OILBUN has only 92 of 74,580
vertices in the equivalent unparented/glue-free set. Sending only those simple
meshes to a shader would not address the measured CPU bottleneck. The GPU
replacement must evaluate normal and blend keyforms, parent warp/rotation
chains, mesh deformation and glue without a synchronous full-frame GPU readback;
the current renderer-facing CPU geometry contract still needs to be evolved.

A test-only WGPU compute stage also blends load-time warp keyforms from one
resident GPU buffer into a control-point buffer in a batched dispatch. The
large 90s outfit needed 19.75 MiB of warp key positions. At an interior
parameter pose, all 680 warp nodes and 819,275 points matched the CPU Rust
evaluator within its coordinate-relative tolerance; 327 nodes used multiple
keys. This has no per-frame key-position upload, but active key weights and
the work schedule still need efficient runtime updates. A generated
root/child/grandchild test now blends local points on the GPU and resolves two
parent levels from that GPU buffer without intermediate readback. Mesh output
still needs to remain on the GPU. No frame-time or FPS improvement has been
established from this test path.

The same keyframe batch now includes ordinary ArtMesh base positions. A large
model interior pose matched Rust for all 1,210 meshes and 225,448 vertices,
including 358 multi-key meshes; combined warp and mesh keys occupied 24.03 MiB.
OILBUN and Ditto passed the equivalent native GPU check. A generated chain
consumed GPU-blended local points through two parent warps into final mesh
positions without intermediate readback. Per-model blend shapes, rotation
parents and glue are still CPU-only, so this path is not a complete renderer.

A test-only affine rotation pass can now transform those GPU-resident points
after warp resolution. It matched the Rust transform for all 60 authored
rotation nodes in the large 90s outfit, 22 in OILBUN and 14 in Ditto at an
interior parameter pose. The generated chain also applied reflected, scaled
rotation after two parent warps without readback. Rotation coefficients are
still prepared on the CPU; inherited orientation, origin probing, blend
shapes and glue have not been ported into the GPU evaluator.

The test path now applies additive warp and ArtMesh blend-shape deltas in a
second compute dispatch after normal key blending. At an interior pose, the
large 90s outfit matched Rust with 1,477,244 resident delta points affecting
458,945 positions; OILBUN matched with 356,777 delta points across 82,468
positions. Ditto has no position-delta keys in this check. The generated
chain also carried an active mesh delta through two parent warps and a
reflection/scale rotation without CPU readback. These figures establish math
parity only; the active renderer still evaluates these steps on the CPU.

Glue now has a conflict-aware Rust pass planner. It places independent glues
in the same GPU dispatch and assigns later passes when a vertex is reused,
preserving source-order updates. A glue that reuses a vertex within itself
still requires finer scheduling or CPU fallback. The large 90s outfit's
1,365 authored pairs fit one pass; OILBUN's 2,298 pairs need two passes
because four vertices are shared across glues. A test-only compute pass
matched sequential Rust updates on synthetic positions using each rig's
actual pair topology, weights and pose-derived intensities. This does not
yet validate glue on final rendered poses or improve the live frame rate.

The desktop renderer now has an explicit GPU-position entry point. It binds
the caller's compute-produced vertex buffer directly for mask and color
passes, skipping the CPU position upload and an intermediate GPU copy. The
caller must provide a fitted view canvas and normalized visible bounds;
undersized buffers and invalid view/bounds values are rejected. A native
two-mesh fixture, including a
hidden mask mesh, matched the CPU-rendered image pixel for pixel while all
CPU drawable coordinates were deliberately wrong. This verifies the
compute-to-render buffer contract, not a live MOC3 model update: the full
GPU evaluator and its bounds calculation still need to call this path.

The normal-key GPU stage now has a reusable load-time Rust plan and desktop
compute evaluator. Mesh positions occupy the start of its output buffer for
the renderer's direct vertex binding; warp points follow. Static source keys
and per-vertex work remain GPU-resident across poses. Only selected key
offsets/weights and one count per node change each frame. Native DX12 checks
matched the Rust decoder across default, interior and varied poses on the
large 90s outfit, OILBUN and Ditto. For the large model, source keys use
24.03 MiB, static work 15.94 MiB and dynamic keys/counts 41.34 KiB per pose.
The evaluator now applies a second resident pass for blend-shape position
deltas to that same buffer. Rust builds its fixed work schedule at load time;
the GPU keeps delta points and updates only selected weights/counts. Combined
normal-key and blend-delta parity passed on all three models across three
poses, including 458,945 affected points on the large outfit. That model uses
11.27 MiB resident deltas and 11.12 KiB changing blend keys/counts per pose.
The remaining hierarchy transforms, glue, metadata and bounds must still be
composed before the renderer can use the buffer for a real avatar frame.

A third resident pass now resolves warp-only hierarchy branches in depth
order. Static grid descriptors and per-point work stay on the GPU; child warp
control points are transformed before their descendant meshes. It uses the
same output buffer as normal keys and blend deltas, without an intermediate
readback. Native DX12 parity passed at three poses on the large outfit
(276,025 hierarchy points, 118 supported warps and 480 meshes), OILBUN
(112,068 points) and Ditto (2,309 points). Eligible, non-glued mesh vertices
also matched the full Rust geometry evaluator. This warp-only pass remains as
an isolated parity check for the more complete stage below.

The resident evaluator now also has a complete depth-ordered warp/rotation
hierarchy and source-ordered GPU glue stage. Rust prepares local rotation
origin, angle, scale and reflection plus glue intensities each pose; GPU
compute resolves inherited transforms (including warp-to-rotation and
rotation-to-warp links), mesh vertices and glue in the same position buffer.
Native DX12 comparisons with the full Rust geometry evaluator passed at three
poses on the large outfit (740 deformers, 19 depths, 1,365 glue pairs and
676,344 active vertex-poses), OILBUN (271 deformers, 11 depths, 2,298 glue
pairs and 223,740 vertex-poses) and Ditto (109 deformers, 13 depths and 1,980
vertex-poses). Maximum observed errors were below 0.000015 model units. This
validates geometry math, not live rendering: dynamic bounds, renderer hookup
and frame-time measurements remain.

The Rust evaluator now has a separate metadata-only frame path. It computes
activation, inherited part/deformer opacity and color, mesh opacity/color,
and draw order while leaving positions empty. The direct Rust hosted adapter
updates its drawable state without changing the stored vertex arrays. Local
three-pose comparisons on the large outfit, OILBUN and Ditto matched the full
Rust frame's metadata and render order exactly. This removes the need to run
CPU vertex deformation solely to feed GPU rendering; dynamic bounds and the
actual Studio renderer switch are still pending.

A test-only WGPU compute primitive now samples multiple warp grids in a single
dispatch. It implements the Rust evaluator's affine shortcut, bent-grid quad
or triangular cells and exterior continuation. Native DX12 comparisons passed
for generated grids and default-pose control grids decoded from the large 90s
outfit, OILBUN and Ditto. A separate two-depth test writes child control points
to GPU storage, then samples that GPU-produced child grid for its grandchild in
the next dispatch without an intermediate readback. The live renderer does not
use this shader yet. GPU keyform/blend evaluation, rotation chains, glue,
visibility/order and mesh output must be integrated and checked end to end
before the CPU geometry path can be removed.

Optimized 120-frame worker tests measured OILBUN at about 2.58 ms/frame with
the current runtime and 4.09 ms/frame with Rust. The large 90s outfit measured
about 10.20 ms/frame current and 18.38 ms/frame Rust. These figures include
worker frame handling, not complete desktop rendering. The Rust path must be
accelerated before it can replace the default. In the desktop renderer, model
coordinates are now projected in the GPU vertex shader; this removes CPU
projection of every vertex before upload but does not move MOC3 deformation
or worker serialization to the GPU. No whole-frame FPS improvement is claimed.

Profiling the large 90s outfit located most remaining Rust geometry time in
parent warp transformations. The evaluator now recognizes warp grids whose
control points agree with an affine map within `0.000001` model units and uses
the precomputed map for vertex and child-deformer transforms. Bent grids still
use full cell interpolation. Internal finite-validated transforms also avoid
repeating per-vertex input checks; the completed geometry frame is checked for
non-finite vertices. All 28 direct official-Core comparisons passed after
these changes. In optimized local 120-frame worker samples, the Rust path fell
from about `18.38` to `13.85 ms/frame`; the transitional worker measured about
`9.99 ms/frame` in the same follow-up series. These are workload-specific
CPU/transport timings and still fall short of the required performance gate.

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
ignored `official_cubism_matches_rust_on_animated_vertices` test
to repeat the direct Cubism comparison. No DLL path is hardcoded into ARIA.
Set `ARIA_COMPARE_TRANSITIONAL_CORE=1` to include the temporary Purism runtime
in that diagnostic; direct Rust-to-official comparison works without it.
