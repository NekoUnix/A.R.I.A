# Architecture and next steps

## Current flow

```text
iPhone VTS UDP / ARIA JSON / deterministic demo
                    |
             aria-tracking
       validation + latest-frame mailbox
                    |
                aria-core
       calibration -> mapping -> smoothing
                    |
             aria-desktop bindings
                    |
       aria-live2d -> Cubism Core DLL
          owned animated mesh data
                    |
          wgpu ArtMesh renderer
       transparent shared render target
           /                   \
  studio preview       Windows output -> OBS Window Capture
```

| Package | Responsibility |
| --- | --- |
| `aria-core` | Tracking/parameter data, calibration, mapping, deterministic smoothing |
| `aria-tracking` | VTS/ARIA decoding, subscription renewal, bounded latest-frame snapshot |
| `aria-model` | Bounded model3 parsing, path validation, moc-to-manifest discovery, ordered texture references |
| `aria-live2d` | Private Core ABI, explicit DLL loading, aligned memory ownership, consistency/version checks, parameter metadata and owned mesh output |
| `aria-desktop` | Native UI, model import/bindings, wgpu renderer, diagnostics and output viewport |
| `aria-cli` | Simulator, JSON sender, headless receiver and manifest inspector |

One worker serves each active tracking receiver. The UI consumes the most recent
frame without queuing old frames. Read timeouts allow prompt shutdown/reconnect.
The CLI shares the receiver and mapping without requiring a GPU or the SDK.

## Native model boundary

`aria-live2d::CubismModel` owns model storage, moc storage, and the loaded library
in that destruction order. Allocations satisfy Core's 16/64-byte alignment requirements.
Before revival, the loader checks the header, size, moc version and Core consistency.
The object is intentionally neither Send nor Sync. No raw Core pointers cross the
crate's public API; updates copy mesh data into owned Rust vectors.

DLLs are executable code and must come from the official SDK. The Windows loader
uses a canonical absolute path and limits dependency searches to the DLL folder and
System32. Core itself is a trusted native parser; these checks are not process isolation.
API symbols and counts are checked, including both old/new render-order entry points.
Unsupported offscreen parts and advanced blend modes fail explicitly.

Tracking binds by exact ID, with optional assignments to custom IDs. Unbound parameters
use model defaults or manual overrides. Updates clamp values, reset drawable dynamic
flags, evaluate Core, then render. Physics/motion/pose/expression sequencing is future work.

## GPU rendering

`cubism_render.rs` uploads atlas textures once and reuses geometry/style buffers.
Each update writes positions and per-drawable colors/opacity. Drawing follows Core's
render order, including changes caused by parameters. A mask pass combines texture
alpha from all referenced meshes, using their culling but ignoring their opacity and
visibility. A following pass applies the regular/inverted mask and the proper
blend state. Adjacent compatible draws share a pass. The mask target is reused, so
many mask groups do not allocate many full-size textures.

Colors are composed in gamma space using a premultiplied RGBA8 target, matching
egui's native-texture convention. Multiply/screen colors are applied before alpha.
The resulting 2048px-longest-side texture is registered with egui and displayed in
both native windows. Its registration is freed on model replacement/unload.
Separate deferred viewports allow ordinary Windows capture and screenshot requests.

This initial renderer uses one reusable mask target and CPU mesh copies each frame.
Mask atlasing, dirty-geometry uploads, asynchronous imports, device-loss recovery,
resolution controls and measured resource budgeting remain follow-up work.

## Next milestones

- Physics, expressions, motions and pose in an explicit update order.
- Saved per-avatar mappings and configurable input/output parameter ranges.
- Cubism 5.3 offscreen parts and advanced color/alpha blending.
- Resource measurements, async import, minimized-window limits and performance work.
- Windows shared GPU textures/Spout and an OBS source plugin, with synchronization.
- Additional tracking adapters and opt-in recording/replay.
- Linux/macOS verification and platform output transports.

Rust's binary ABI is not a stable plugin contract. Future plugins should use versioned
IPC, a C ABI or a sandboxed interface. The current ARIA JSON protocol is the first
extension point, not a general plugin host.

## Dependencies

The desktop uses eframe 0.33.0 and wgpu 27, resolved in Cargo.lock. Windows defaults
to Direct3D 12; `WGPU_BACKEND=vulkan` selects the available fallback. The toolchain
is pinned in rust-toolchain.toml. CI builds on Windows MSVC without proprietary
assets. Core and model files are supplied at runtime under their separate licenses.
