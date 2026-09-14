# Engine performance

ARIA uses its own Rust avatar/runtime engine, with wgpu for GPU rendering and
egui/eframe for the native UI. v0.33 improves ARIA's per-frame work; its current
stable graphics libraries were already up to date when reviewed on 2026-09-14
([wgpu](https://crates.io/crates/wgpu), [eframe](https://crates.io/crates/eframe)).

## What changed

- **Multi-avatar settings handoffs:** exchange owned per-avatar settings instead
  of allocating/copying camera preferences, strings and all three output layouts.
  Workspace layouts, frame-rate target and window policy remain in place.
- **Unshared tracking:** avoid making an extra snapshot and full blendshape-map
  copy for a tracker that has no loaded followers. Shared tracking still takes
  the source snapshot before advancing the independent avatar runtimes.
- **Unchanged Syphon frames:** retain the previously published image instead of
  submitting another GPU copy/draw. Keep pending changes when OBS disconnects,
  and publish the newest canvas when a client arrives, even if it is now frozen.
- **Syphon orientation:** convert the image origin in Syphon's existing GPU
  publication pass. There is no CPU pixel readback or additional ARIA staging
  texture for the orientation fix. A vertical conversion requires a shader pass
  instead of the old direct blit, so it is a correctness fix, not a claimed
  improvement in the cost of each changing frame.

- **Glass workspace:** static geometry and alpha blending in the existing UI
  renderer. No blur pass, backdrop capture or additional render target. Native
  OBS canvases exclude the glass decoration.

## Reproduce the CPU measurement

From a configured source checkout:

```sh
cargo test --locked --release -p aria-desktop profile_settings_handoff_benchmark -- --ignored --nocapture
```

This opt-in benchmark models four avatar placements in all three canvases. It
compares the previous settings path with the new ownership exchange, alternates
run order, and reports the median of seven runs of 20,000 handoffs each. It opens
no windows and accesses no camera or avatar files. Exact timings depend on the
CPU, build mode and other running programs. This isolates settings overhead;
its percentage is **not an overall frame-rate improvement**.

Release measurements are recorded in [Validation](validation.md). The ownership
test also checks that settings return to their original avatar, their string
allocations are reused, and output layouts stay unchanged.

## Check native macOS output work

```sh
sh scripts/test-syphon.sh
```

On a Mac with Xcode and Metal, this builds the pinned framework and runs a real
Syphon publisher/client regression. It checks asymmetric pixels using OBS's
bottom-left row convention, both BGRA and RGBA sources, transparency, all three
canvas shapes, resizing and a late client. A counted command queue verifies that
120 unchanged sends submit **zero additional GPU command buffers**. This is a
native transport test, not an end-to-end recording in the OBS desktop app.

## Practical limits

More live avatars, high-resolution textures, animations, physics and effects
still require more CPU/GPU work. The Syphon saving applies when the combined
canvas is unchanged; one moving avatar keeps that output active. It does not
disable tracking or animation. Windows Spout and Linux ARIA Canvas keep their
existing transport behavior. Use the bottom-bar graphs to compare the same
models, effects, output resolutions and frame-rate target in your own session.
