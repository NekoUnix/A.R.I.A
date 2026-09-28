# Avatar performance investigation

ARIA's FPS target is a scheduling cap, not a promise that every asset reaches
that rate. At 120 FPS, the complete frame has 8.33 ms. Measure with the same
window size, output canvases, tracking source and background workload before
comparing models. The release executable and its runtime DLLs must be together;
an unoptimized debug executable or a standalone test binary without the DLL
directory on `PATH` is not a valid performance sample.

Set `ARIA_PERF_LOG=1` for two-second `PERF_FRAME` and `PERF_AVATAR` records in the
local diagnostics directory. `PERF_FRAME` reports the UI/update interval;
`PERF_AVATAR` separates rig/physics evaluation, Purism host roundtrip and
Live2D mesh rendering. `PERF_CUBISM_RENDER` further splits view fitting, vertex
preparation, GPU writes and command encoding, including mask/color pass counts.
These are CPU wall times, not GPU execution timings or end-to-end tracking
latency. A busy system, multiple ARIA instances or GPU backpressure can change
them substantially.

## Local supplied-asset samples (2026-09-27)

An optimized CPU-only test alternated one parameter for 120 frames after 20
warm-up frames. It measured the open-source Purism runtime directly, including
mesh extraction in the first number:

| Model | Meshes | Vertices | Full local update | Purism deformation only |
| --- | ---: | ---: | ---: | ---: |
| NekoUnix 90S OUTFIT | 1,174 | 218,693 | 4.99 ms | 4.53 ms |
| NekoUnix chibi | 441 | 25,101 | 0.28 ms | 0.27 ms |

The large model also has 17 4096-pixel texture atlases. Its separate native
desktop sample reached about 55–59 FPS at a 120 target, with roughly 8–11 ms
in the host roundtrip and 3–5 ms in rendering on sampled frames. The direct
benchmark establishes that just copying Rust mesh data is less than half a
millisecond of the local Core update. It does not isolate host serialization,
scheduling or GPU time. Smaller Live2D assets need their own native checks;
the numbers above do not justify treating every Live2D model the same way.

An instrumented release run found 378 visible meshes, 60 mask passes and 61
color passes. View fitting and CPU vertex preparation were usually below 1 ms
each, while the call through GPU submission ranged about 1.8–4.6 ms in sampled
frames. Those submission wall times are not GPU timestamps. ARIA now stores
unchanging UVs in a separate GPU buffer and uploads only deformed positions;
the change halves dynamic vertex transfer, but does not remove the many mask
passes or the full Core evaluation. Compare the same asset before and after to
determine its actual FPS effect.

The renderer now uploads deformed model-space vertices and applies the canvas
projection in its GPU vertex shader. CPU preparation copies positions directly;
it reduces visible bounds to model-space extrema rather than projecting every
vertex for that calculation. The GPU clipping/blending test and an OILBUN
native render passed. Compared with the preceding OILBUN image, 1,101 of
3,854,336 pixels changed, with 21 pixels differing by more than one channel
level and two by more than eight, at rasterized edges. This work leaves MOC3
deformation and worker transport on the CPU, so it does not establish a 120 FPS
gain. An initial experimental Rust worker sample took about 18.38 ms/frame on
the large 90s outfit where the current worker took about 10.20 ms/frame.
After affine-warp and finite-validated transform fast paths, a follow-up sample
measured about 13.85 ms/frame for Rust and 9.99 ms/frame for the current worker.
The two runs are local
measurements, not a guaranteed FPS gain. Making Rust the default requires
further geometry acceleration and end-to-end profiling.

An in-process Rust adapter removes worker transport and one position-vector
copy per updated mesh. On the large 90s outfit, an optimized single-parameter
hosted test measured about 8.74 ms/frame, while a 32-parameter animation
measured about 10.93 ms/frame for Rust versus 5.70 ms/frame for the current
runtime's direct model update. In matched 20-second hidden Studio smokes at a
120 FPS target, the direct Rust path settled near 43.6 FPS (about 13.3 ms host
and 3.9 ms renderer wall time), while the default path on the rebuilt binary
settled near 53.9 FPS (about 11.0 ms host and 3.8 ms renderer). These are local
samples, not GPU timestamps. The direct path stays opt-in. A synchronously
awaited Rust background thread was also profiled and discarded because its
desktop result remained near 43 FPS. The next performance work needs to reduce
Rust keyform/deformer evaluation and mask pass costs; simply changing thread or
process boundaries does not meet the 8.33 ms whole-frame budget.

The next Rust-core change caches deformer states by their normal and blend
parameter dependencies. In optimized large-outfit tests, alternating one
parameter fell from roughly 8.74 to 1.98 ms per hosted update. Three 32-axis
samples measured about 8.9–9.4 ms for Rust versus 5.3–5.4 ms for the current
runtime's direct update. With stage profiling enabled, roughly 5.8 ms of the
Rust 32-axis frame remained in deformers (about 2.0 ms keyform/blend work and
3.7 ms hierarchy resolution), 2.4 ms in meshes and 0.5 ms in glue. The
profiling itself adds overhead; the uninstrumented totals above are the
performance comparison. This change reduces CPU work but does not execute MOC3
deformation on the GPU. A matching desktop FPS check is recorded separately.

The experimental GPU warp kernel is a correctness and scheduling prototype,
not a desktop FPS change. It batches independent grids in one compute dispatch
and resolves a child then grandchild grid in successive GPU passes without
reading the parent back to the CPU. The current Studio path still computes
deformer keyforms, hierarchy, meshes and glue on the CPU and uploads final
vertices, so the measured Live2D frame rates above remain the baseline.

In matched 20-second hidden Studio physics smokes on the same optimized
executable, the cached direct Rust path settled near 42.3 FPS by the last six
sampled intervals, versus 53.9 FPS for the default runtime. The Rust sample's
host wall times ranged roughly 11–23 ms in the sampled intervals; the default
was usually about 8–15 ms. Rendering was around 3–6 ms on both. The single-axis
gain therefore does not solve broad physics-driven updates or the whole-frame
120 FPS target. The default renderer remains unchanged.

The supplied NekoUnity2 VRM spent about 1–1.3 ms in model update at its saved
60 FPS target. With the smoke cap held at 120 after import, it subsequently ran
at 120.1–120.3 FPS in three sampled intervals, with 0.8–1.4 ms model updates.
One 44 MB GIF decoded asynchronously and played at about 120 FPS before its
profile restored a saved 60 FPS setting. A later four-GIF import reached its
ready screenshot and exited normally in about 49 seconds; sampled playback
stayed about 120 FPS with approximately 0.1 ms of model update. PNG image
actions and a 160-particle effects smoke also reached about 120 FPS in steady
samples, with occasional 110 FPS intervals around screenshot/UI work. These
results distinguish long GIF import latency from inexpensive playback. The
smoke harness now reapplies a requested FPS cap after asynchronous profile
loads so comparisons use the intended target.

The newer GPU morph path keeps a bounded set of active expression deltas in
GPU storage and combines them in the vertex shader alongside skinning. The
supplied NekoUnity2 check verified an active face morph used this path and the
rendered tracking/pose tests passed. A matching before/after full-frame sample
has not yet been recorded, so the earlier FPS figures above are baselines,
not measured gains from this change.

VRM/GLB morph allocation now selects up to 32 active slots per mesh according
to vertex count, the remaining 64 MiB model budget and the device's storage
buffer limits. That extends GPU execution beyond the former eight-morph cutoff
for models that fit, while larger active sets still use the CPU fallback.
Position-only morphs supply zero normal deltas on the GPU, matching their CPU
behavior. A generated 12-active-morph DX12 render matched the CPU fallback to
within 1% of image channels; NekoUnity2 also passed its native render, tracking
and spring check. This is more GPU work for 3D expressions, not a measured FPS
gain or a GPU solution for Live2D deformation.

The supplied ICHIGO VRC/GLB export rendered successfully but settled around
113 FPS in this short native sample, with roughly 5 ms model updates. It uses
the VRM/GLB renderer, not Purism Core. `PERF_VRM` separates parameter/pose,
spring simulation and GPU rendering to guide further tuning. A GLB exported
from a different Unity/VRChat rig may behave differently; no Unity runtime or
PhysBone simulation is assumed by this measurement.

For ICHIGO, instrumented spring/world-matrix work ranged from 1.3 to 6.1 ms
on sampled frames. The solver previously refreshed every descendant of each
spring joint before processing the next one. It now reconstructs only the
current joint's ancestor path and rebuilds the full world once per substep.
This preserves a current parent transform for chained springs while avoiding
repeated whole-subtree walks. The full desktop spring suite, GPU clipping
render test and a native ICHIGO import/render test passed after the change;
the final release smoke remained about 109–114 FPS for this asset. It did not
establish a frame-rate gain on ICHIGO. NekoUnity2 remained near 119–120 FPS.

After the separate UV buffer, once-per-pass geometry binds and single-channel
mask target, the large Live2D smoke remained about 54–63 FPS. The chibi model,
which uses 74 mask and 75 color passes despite only 25,101 vertices, reached
120 FPS in its last two sampled intervals after the mask change; earlier
intervals were 114–118 FPS. Both screenshots retained expected clipping and
the native GPU clipping/blending test passed. The large model did not approach
120 FPS; its Core/host and mask-pass costs remain the main constraints.

## Core and licensing

An experimental renderer entry point can now draw directly from a GPU
compute-produced position buffer. A native masked-mesh pixel comparison
passed with CPU vertex coordinates deliberately poisoned. The full MOC3
deformation pipeline is not yet connected to it, so current desktop FPS and
CPU-use figures should not be interpreted as gains from this entry point.

Normal mesh and warp key positions now use a resident GPU plan in native
parity checks. On the large outfit, the static key and work buffers total
39.97 MiB, while parameter changes upload about 41 KiB per pose. This
removes a planned full-frame geometry upload for that stage. A second resident
pass applies blend-shape position deltas to the same output buffer, including
458,945 affected points on that model, with only active blend weights/counts
uploaded per pose (11.12 KiB on the large outfit, with 11.27 MiB resident
deltas). Whole-frame GPU timing and active Studio FPS have not been
measured because later deformation stages and metadata are still on the CPU path.

The resident path now also transforms warp-only hierarchy branches in GPU
storage, using fixed depth-ordered work and no intermediate readback. It
covered 276,025 control/mesh points on the large outfit. The newer full GPU
evaluator also resolves mixed warp/rotation branches and glue, with full-Rust
final-position parity on three local models. Only compact rotation frames,
active keys and glue intensities upload per pose; the large source arrays and
final mesh positions stay GPU-resident. The active Studio renderer is still on
its existing path, so no FPS gain is claimed until metadata, bounds and
renderer integration are measured.

The developing Rust `aria-model-core` now retains MOC3 source bytes and all
declared encoded atlas files in system RAM. The current desktop renderer reads
these resident bytes, but still uploads every atlas to the GPU. RAM residency
alone does not lower VRAM or remove the measured Live2D mask-pass cost; GPU
atlas residency and frame-time changes need separate measurement.

ARIA builds the MIT-licensed Purism Core source pinned under
`crates/aria-live2d/vendor/purism-core`. Its generated bundle remains upstream
unchanged. Performance work should be measured against that open source and
validated on licensed, local model files. No proprietary Live2D binary or
model artwork is part of the repository. Replacing the entire runtime is a
larger compatibility task. ARIA's independent Rust replacement is being
validated separately against local Cubism exports before it becomes active;
renderer and transport costs still need measurement alongside that work.

## Reproducing the CPU test

Run `real_core_update_benchmark` as an ignored **release** test with
`ARIA_TEST_MOC` pointing to your own `.moc3`. On Windows GNU LLVM, add
`target/release` to `PATH` before launching the test executable directly so
`libunwind.dll` resolves. Cargo's configured toolchain normally supplies that
runtime when invoking tests. This test uses local assets only and logs timing;
it does not compare the rendering visually or verify a GPU frame rate.
