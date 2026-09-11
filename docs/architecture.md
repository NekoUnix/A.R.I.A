# Architecture and next steps

## Current vertical slice

```text
iPhone VTS UDP / ARIA JSON / deterministic demo
                    |
            aria-tracking
     socket worker + validation + latest-frame mailbox
                    |
               aria-core
      normalized frame → calibration → mapping → smoothing
                    |
              aria-desktop
        egui controls + wgpu drawing
           /                 \
  studio preview       Windows output viewport → OBS Window Capture

aria-cli uses the same receiver and mapping without a GPU.
aria-model inspects model3 manifests and their local asset references.
```

| Package | Responsibility |
| --- | --- |
| `aria-core` | Plain tracking/parameter data, calibration, mapping and deterministic smoothing; no OS, network or rendering dependencies |
| `aria-tracking` | VTS/ARIA wire formats, subscription renewal, socket lifecycle, sender filtering, bounded latest-value snapshot |
| `aria-model` | Model3 manifest parsing and safe local asset inspection; no native SDK dependency |
| `aria-desktop` | Windows-focused app shell, GPU-backed 2D test rendering, image loading, diagnostics and capture viewport |
| `aria-cli` | Simulator, JSON sender, headless receiver and manifest inspector |

There is one network worker per active receiver. The UI reads a shared snapshot;
it cannot accumulate an unbounded queue of old tracking frames. Socket reads have
a timeout so closing/reconnecting joins the worker promptly. A full-size UDP
receive buffer prevents accidental truncation into an apparently valid packet;
the decoder enforces the smaller application limit.

The receiver is not a general plugin host. A second protocol is selected explicitly;
packets are not guessed from arbitrary JSON. All renderers consume the same
parameter set, so subsequent output backends can remain independent of tracking.
There is no unsafe FFI code in this initial workspace.

The versioned JSON schema is the first extension point. A stable binary Rust plugin
ABI is deliberately not promised: Rust type layout and compiler ABI are not a
cross-version plugin contract. A future public API should use a versioned IPC
protocol, C ABI, or sandboxed plugin interface with explicit permissions.

## Live2D integration

The engine discussion proposed a Rust core with wgpu and a native Cubism bridge.
That is still the intended direction. The working v0.1 preview is an original
vector/PNG puppet; the `.moc3` evaluation and rendering boundary is **not implemented**.
An asset report must never be presented as a successfully loaded Live2D model.

A complete next milestone needs:

1. Obtain the official [Cubism SDK for Native](https://www.live2d.com/en/sdk/download/native/)
   under its applicable terms and establish which SDK assets can be distributed.
   Keep it outside the MIT-only assumption for this repository.
2. Add an isolated `aria-live2d` wrapper around the native core with audited
   ownership, alignment, lifetime and version checks. Expose owned Rust render data
   rather than raw core pointers throughout the application.
3. Build a wgpu mesh renderer with texture atlases, drawable ordering, opacity,
   premultiplied-alpha behavior, clipping masks, blend modes and device-loss handling.
4. Map IDs and parameter ranges from each model rather than assuming all models
   use the 12 preview IDs. Add editable per-model mappings and model-specific defaults.
5. Integrate motions, expressions, physics and pose in a defined update order.
6. Test against appropriately licensed sample models, including clipped eyes/hair,
   additive/multiplicative blending, many masks, large textures and missing files.

This is substantial renderer work. Native platform support or wgpu portability
alone does not make Cubism integration automatic.

## Windows milestones

- **v0.1, here:** native UI, actual UDP input, PNG/vector preview, model file
  inspection, ordinary OBS Window Capture, portable ZIP, tests and build instructions.
- **Next:** actual Cubism playback with the renderer and conformance checks above;
  saved per-avatar settings and configurable parameter mapping.
- **After basic model correctness:** Windows resource accounting, measured process
  CPU/RAM and GPU/VRAM data, frame pacing and inactive-window limits.
- **Streaming output:** offscreen render target and Windows shared GPU texture
  transport, with synchronization and an OBS source plugin. Define the platform-neutral
  control protocol separately from the Direct3D handle transport.
- **Broader inputs:** named adapters for webcam/OpenSeeFace/other phone apps and
  recording/replay with explicit opt-in. These are not currently supported formats.
- **Cross-platform:** test Linux/macOS desktop builds and implement equivalent output
  transports only after Windows behavior is stable. Cargo crate boundaries are a
  starting point, not a claim of tested platform parity.

## Build and dependency choices

The desktop uses eframe 0.33.0, with dependency resolution committed in Cargo.lock.
wgpu chooses the real adapter/backend and the UI reports it. Windows normally
uses Direct3D 12; `WGPU_BACKEND` can select an available fallback. Build/test tooling
is pinned by `rust-toolchain.toml`; the Windows CI job uses the MSVC host toolchain.

No generated image service, browser runtime, Unity player, Godot project, cloud
tracking service or model asset is required to run the shipped preview.
