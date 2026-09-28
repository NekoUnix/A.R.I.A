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

ARIA builds the MIT-licensed Purism Core source pinned under
`crates/aria-live2d/vendor/purism-core`. Its generated bundle remains upstream
unchanged. Performance work should be measured against that open source and
validated on licensed, local model files. No proprietary Live2D binary or
model artwork is part of the repository. Replacing the entire runtime is a
larger compatibility task and is not required to investigate the measured
renderer/transport costs.

## Reproducing the CPU test

Run `real_core_update_benchmark` as an ignored **release** test with
`ARIA_TEST_MOC` pointing to your own `.moc3`. On Windows GNU LLVM, add
`target/release` to `PATH` before launching the test executable directly so
`libunwind.dll` resolves. Cargo's configured toolchain normally supplies that
runtime when invoking tests. This test uses local assets only and logs timing;
it does not compare the rendering visually or verify a GPU frame rate.
