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
       rig bindings -> breath -> physics
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
| `aria-core` | Tracking/parameter data, calibration, VTS profiles, response/ranges, stepping, fixed-step physics3, serializable configurations and poses |
| `aria-tracking` | VTS/ARIA decoding, subscription renewal, bounded latest-frame snapshot |
| `aria-model` | Bounded model3 parsing, path validation, moc-to-manifest discovery, ordered textures and optional rig sidecars |
| `aria-live2d` | Private Core ABI, explicit DLL loading, aligned memory ownership, consistency/version checks, parameter metadata and owned mesh output |
| `aria-desktop` | Input/pose/preset UI, model import, wgpu renderer/PNG export, Windows process metrics/global hotkeys and output viewport |
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

Tracking binds by exact output ID, preferring adjacent VTS profile assignments.
Each update starts from native defaults/manual overrides, applies named tracking and
breathing through each binding's ranges/filter, layers active expressions, then
evaluates authored physics. Partial holds apply before and after physics.
Physics uses the rig's fixed FPS, input interpolation and previous/current output
interpolation, preserving group order for chained dependencies. Particle state retains
velocity; pause gaps and re-enabling reset it. Values clamp to native parameter limits
before resetting drawable flags, evaluating Core and rendering. Expression fades
are per-model transient state; selections live in RigConfig and shortcuts/import
paths in SavedRig. The Expressions panel discovers metadata and feeds the same
runtime from buttons and shortcut actions. Full frozen poses bypass this entire
pipeline. Motion3/pose3 sequencing remains future work. Rig math uses owned Rust parameter data, independent
of native ABI pointers, so unit tests need neither Core nor licensed assets.

Optional profile, physics and display-info reads are bounded to 2 MiB each and
stay within the model directory. Invalid optional data produces a visible warning;
the avatar still loads. Physics group/particle/input/output counts and numerical
ranges are validated before constructing runtime chains.

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

## Windows process metrics

The footer samples once per second. GetProcessTimes supplies combined kernel/user
100-ns ticks; delta time divided by elapsed monotonic time and available processor
count yields process CPU %. GetProcessMemoryInfo supplies working-set/private bytes.
For Direct3D 12, a scoped wgpu HAL adapter guard exposes the exact DXGI adapter for
read-only QueryVideoMemoryInfo calls (local usage/budget and non-local usage, node 0).
No arbitrary GPU matching, retained raw object, WMI subprocess or system-wide counter
is used. Failed queries and non-DX12 backends have explicit unavailable values.

## Next milestones

- Expressions, motions and pose in an explicit update order.
- Animated preset transitions and richer shortcut customization.
- Cubism 5.3 offscreen parts and advanced color/alpha blending.
- Per-model resource budgeting, async import, minimized-window limits and performance work.
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

## Input configuration and pose boundary

The Input Monitor owns a serializable `SavedRig`: active `RigConfig`, named presets,
and the model's hotkey enable setting. A stable moc-content fingerprint separates
avatars without writing to their folders. PNG/JPEG sprites use separate decoded
image-content identities; Mica keeps the built-in preview identity.
Model switches remember the outgoing rig. Explicit saves and preset edits flush
eframe storage immediately; regular app autosave/close also saves the active controls.
Preset files validate version, model identity, IDs, limits and finite ranges before
changing the list. Import clears the shortcut and does not apply the preset.

ModelPreferences stores tracking source/ports/address, calibration, mapping gains,
output preferences, zoom and target FPS for each model identity alongside SavedRig.
Switches restore the incoming profile and disconnect the old tracking receiver.
The SDK path remains a machine preference. Old serialized rig settings use serde
defaults for newly added physics tuning; the original v0.4 field layout is preserved.

PhysicsSettings contains overall controls and a map of GroupSettings keyed by actual
physics group ID. Metadata comes from PhysicsSettings plus Meta.PhysicsDictionary,
with ID fallback. Output amplitude, mobility, particle response and restoring force
use overall × group multipliers; wind offsets add. Mobility is capped at 1. Disabled
groups do not write outputs; re-enabled chains initialize without stale momentum.
Authored group order and dependencies are retained. Unknown saved groups remain
inactive and are reported in the UI. No physics/model sidecar is modified.

Live evaluation is defaults/manual values → bindings/response/smoothing → holds and
stepping → physics → held-output enforcement/stepping → Core. Partial holds therefore
drive unheld physics without letting physics overwrite held parameters. Full freeze
takes a separate branch: restore all final parameters exactly and skip filters and
physics. Breathing time also stops. Mode changes reset particle momentum. Movement
presets release a full freeze; pose presets restore complete frozen values.

Transparent PNG export copies the existing render texture to a padded readback buffer,
waits with a bounded GPU timeout, removes row padding and converts premultiplied RGB
to straight-alpha RGBA before encoding PNG. The export is processed after model update
so the latest pose edits reach the saved image.

Windows global shortcuts use an owned thread and RegisterHotKey/WM_HOTKEY. A command
channel replaces registrations; generation numbers reject stale events. MOD_NOREPEAT
prevents repeated triggering while a key is held. Conflicts are surfaced in the UI;
shutdown joins the worker after unregistering only its own shortcuts. The message
thread requests an egui repaint when a shortcut arrives, including with another app
focused. It does not use keyboard hooks or record arbitrary key input.
