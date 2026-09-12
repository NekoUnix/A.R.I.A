# Validation

This file records checks for the development builds. The Windows CI workflow
is the repeatable MSVC build/test path; its status belongs to a specific commit.

## Local verification on 2026-09-12

### v0.15 image actions, GIFs and microphone

- Formatting and strict Clippy across all targets/features passed. **104 default
  tests passed**, with ten environment-dependent tests ignored. New checks cover
  GIF frame selection/timing, decoded budgets, action priorities, input ranges,
  interrupted fades, freeze, animation endpoints, template import/export, shortcut
  conflicts and microphone gate/routing. The real eframe RON storage test also
  restores distinct image and microphone settings for two avatar profiles.
- A separate Windows microphone test opened the default input, received **151 audio
  callbacks**, measured finite amplitudes and closed the stream on disable. Samples
  were not saved. The native microphone panel showed the live level and talking gate.
- Native DX12 previews observed all **eight frames** of the starter GIF, plus a GIF
  stage object and independently animated thrown GIFs. Freeze produced byte-identical
  transparent PNGs before and after a pause, including animated objects and throws.
- OBS 32.2.1/Spout2 1.12 received the image scene at **1920×1080**, **1080×1920** and
  **1536×1024**, preserving transparent corners and visible artwork. The short check
  reported zero skipped render frames; this is not a general performance guarantee.
- PNG/GIF action artwork is a flat image puppet. Tracking-driven Live2D parameters
  still use the existing rig, and microphone input feeds the same mapping path.
  GIF disposal/transparency uses image-rs; GIF audio and speech recognition are absent.

### v0.14 impact deformation

- Formatting and strict Clippy across all targets/features passed. **98 default
  tests passed**, with nine environment-dependent tests ignored. New coverage
  exercises legacy settings migration, finite bounds, overlapping dent geometry,
  independent copies, contact timing, exact recovery, freeze, clear, dent lifetime
  after particle removal, resolution scaling, texture coordinates and alpha.
- Native Windows/DX12 editor previews verified Mica and the local Live2D avatar,
  including a separate thrown moc3 copy. The Live2D comparison changed **438,419
  avatar pixels** and **104,853 object pixels**. Clearing deformation restored
  the original render byte-for-byte; transparent corners remained alpha zero.
  Public documentation screenshots use only Mica; private artwork stays local.
- A stage run rendered **148 effect particles**, including PNG, independent moc3,
  GLB, glTF, OBJ and FBX props. Assertions confirmed avatar dents, independent
  object deformation and both throwing and spray attachment behavior.
- Isolated OBS 32.2.1 with Spout2 1.12 captured the deformed scene at **1920×1080**,
  **1080×1920** and **1536×1024**, retaining transparent corners and opaque avatar
  pixels. This short run reported zero skipped render frames; it is not a general
  performance guarantee. Source geometry and rig parameters are unchanged: these
  are reversible display dents, including on the rendered appearance of 3D props.

## Local verification on 2026-09-11

### v0.9 contextual documentation

- Formatting and Clippy across all targets/features with warnings denied passed.
  **58 default tests** passed. Five new help tests cover topic links, lazy context
  creation, hover text, mouse/keyboard activation, navigation history, independent
  native-window closing, and vertical layout/wrapping of all **46 articles** at
  both the 700-point minimum and 880-point default help-window widths.
- Native Windows/wgpu screenshots at 175% desktop scaling verified the studio,
  expanded input controls, capture controls, range and physics diagrams, plus
  actual parameter context and physics/expression panels with the local Live2D
  test avatar. The initial article layout defect found by screenshots was fixed
  and covered by the article-layout regression test.
- The offline guide is compiled into the executable. Avatar-specific context is
  generated when clicked, not every frame. The help viewport is separate from
  the three capture viewports; closing help does not issue a root/output close.
  Public screenshots use Mica or generic help text; private model art remains local.
- This release changes documentation and UI layout. It does not reimplement
  tracking, physics, expression blending, rendering or the Spout bridge. The v0.8
  OBS interoperability checks below remain historical evidence, not a new v0.9
  end-to-end OBS run.

### v0.8 compact previews and full-resolution OBS output

- Formatting, Clippy for every target/feature with warnings denied, and **53 default
  tests** passed on Windows x64. New cases cover wheel scaling in all three aspect
  ratios, framing locks, multiple layout passes, resolution/preview independence,
  Freeform fitting, legacy two-canvas RON migration, per-model Freeform settings,
  and capping simulation during rapid UI repaints.
- A separate D3D11 receiver opened ARIA's DX12/Spout GPU textures at **1920×1080**,
  **1080×1920**, and **1536×1024**. It verified dimensions, BGRA values and alpha,
  plus unique sender names and cleanup without removing another sender.
- An isolated **OBS 32.2.1** with the upstream **Spout2 plugin 1.12.0** captured all
  three simultaneous senders with the real test avatar. Native source screenshots
  retained those full dimensions, alpha-zero corners, and opaque avatar pixels,
  using **Premultiplied Alpha** composite mode. Only test-owned portable settings,
  sources and processes were used. Private model screenshots remain local.
- All three native preview windows were then minimized (verified by viewport
  state), while OBS continued receiving full-resolution avatar frames and alpha.
  Changing Freeform controls resized the native client from **480×320 to 600×400**;
  a simultaneous canvas change reached OBS at **2048×1024** without reselecting
  the sender. This caught and fixed an old-callback overwrite of requested sizes.
- The real model's compact Freeform preview was **480×320** while its OBS source
  was **1536×1024**. The source receives an offscreen GPU canvas, not a scaled
  window screenshot. The shared Live2D artwork render remains capped at 2048 pixels.
- Optional native checks passed for clipping, normal/add/multiply blending,
  colors, masks, culling, draw order, real Core deformation and mesh allocation
  reuse. A frozen-avatar check skipped 60 unchanged Core/GPU updates, refreshed
  after editing a held parameter, then skipped the unchanged pose again.
- The Windows priority test successfully applied High and Normal to its own test
  process and restored the original priority afterward. High remains off by default.
- These tests validate the changed behavior, not a universal performance multiplier
  or a guarantee against dropped frames under arbitrary CPU/GPU load.

### v0.7 dual OBS output and custom key colors

- Formatting, Clippy across all targets/features with warnings denied, and all
  **48 default tests** passed on Windows x64. The optional native GPU rendering
  test also passed after sharing the PNG readback path with color analysis.
- Real egui pointer-event tests drag both landscape and portrait canvases through
  repeated layout passes, verify one normalized movement with no accumulated jump,
  report drag release for persistence, and preserve placement while locked.
- Both layouts retain independent offsets, backgrounds, keys and scale through
  actual eframe RON profile persistence. Model switching restores distinct layouts.
  Tests cover old settings migration, invalid numeric values, window independence,
  and all supported 16:9/9:16 dimensions.
- Key-color tests cover mixed-case and invalid hex, transparent padding, inclusion
  of additional artwork, avoidance of existing artwork colors and the case where
  no well-separated key exists. The color scan runs only on request, with a compact
  table collected during atlas loading.
- Native release screenshots with both windows enabled verified **960×540**
  landscape and **540×960** portrait clients with different model placement and
  scale. The supplied model's automatic key was **#FF3C00**; screenshot pixels
  matched that exact opaque color, while portrait retained its dark studio color.
  The Mica controls screenshot uses only the original built-in artwork.
- Both transparent output clients also retained their exact dimensions, alpha-zero
  background pixels and visible opaque model pixels in native GPU screenshots.
- Actual OBS filter tuning/capture-method acceptance remains a user/device check;
  automatic key selection is explicitly explained as a best-fit suggestion.

### v0.6 expressions and custom keyboard shortcuts

- Formatting, Clippy across all targets/features with warnings denied, and all
  **42 default tests** passed on Windows x64. Five optional tests require a GPU,
  an interactive Windows session and/or locally supplied Cubism files.
- Expression tests verify Add/Multiply/Overwrite math, native range clamping,
  default blend/fade values, interrupted fades, clean release without cumulative
  drift, partial-hold priority and exact frozen poses. Invalid files are rejected
  and missing target IDs are reported. Discovery merges manifest references with
  unlisted expression files, preserves display names, deduplicates files and
  rejects references outside the avatar directory.
- The supplied model's **three unlisted expressions** loaded and each changed its
  corresponding native parameter and rendered mesh state. The test used the same
  expression actions as keyboard shortcuts, verified saved selections/assignments,
  preserved vertex/color/opacity state while frozen, and restored the underlying
  values and appearance after each expression faded out.
- A temporary **Ctrl+Shift+F24** Windows registration dispatched an expression action
  through the owned worker queue, detected a competing registration and released
  the shortcut on shutdown. The test injected only a message into its own worker;
  it did not synthesize keyboard input into another app.
- Custom modifier/key combinations, expression selections and imported paths
  remain separate for two models after actual eframe RON storage round-trips.
  Duplicate expression/preset/pose shortcut assignments are rejected.
- Release UI checks exercised the real model's Expressions tab, its active blend,
  shortcut editor, the no-Live2D empty state and the built-in Mica studio. Private
  avatar assets and screenshots are excluded from the repository and app package.

### v0.5 studio organization and per-avatar physics

- Formatting, Clippy across all targets/features with warnings denied, and all
  **36 default tests** passed on Windows x64.
- Synthetic rigs with arbitrary group IDs/display names verify metadata discovery,
  independent group amplitude/muting, global amplitude, and changes to inertia,
  response speed, gravity and wind. Unrelated group trajectories remain identical.
  Extreme supported settings and repeated enable/disable transitions remain finite
  and bounded by native parameter limits. Duplicate group IDs are rejected.
- Two distinct model profiles round-trip through eframe's actual RON storage format
  and retain separate tracking sources/addresses, gains, zoom, backgrounds, FPS,
  calibration, poses and group tuning. Physics controls also round-trip in movement
  presets. Legacy v0.4 physics fields load with neutral defaults for new controls.
- The supplied Live2D rig still passes its native integration test: 25 imported
  assignments, 29 groups, 93 output assignments and 92 moving physics parameters.
  Serialized screenshot poses remain vertex-identical under changing live inputs.
- Direct3D 12 screenshots verify the themed studio, parameter categories and
  global/group physics cards using the real model. Private model screenshots remain
  local. Names shown in those cards come from the avatar's own physics dictionary.
- These checks cover the implemented model format and controls, not every possible
  avatar export or device; remaining compatibility/phone acceptance is listed below.

### v0.4 input controls, screenshot poses and presets

- All **31 default tests** passed on Windows x64. Formatting and Clippy across
  all targets/features passed with warnings denied. Four optional tests require
  native Windows hotkeys, a GPU and/or user-supplied Cubism runtime/model files.
- Control tests cover input/output endpoints, dead zones, response curves,
  stepping, exact full freezes, partial holds and returning to live movement.
  Preset serialization and actual input-monitor shortcut dispatch restore tuning,
  global mapping and complete poses. Invalid ranges, model mismatches and duplicate
  hotkey assignments are rejected.
- The supplied rig's 25 imported VTS assignments and 29 physics groups still pass
  the native integration test. After serializing and restoring its complete final
  pose, changing tracking/physics inputs across 120 updates left Core vertices
  exactly identical. Editing a frozen head parameter then deformed the meshes.
- The opt-in Windows hotkey test registered a temporary Ctrl+Alt+F11 shortcut,
  dispatched a message to its own worker queue, detected a conflicting second
  registration, and verified registration succeeds after the first worker closes.
  It does not synthesize keystrokes or type into other applications.
- Direct3D 12 smoke runs with the real model verified the expanded Inputs editor,
  Pose and Presets panels, including readable controls under a light Windows theme,
  and actual GPU PNG readback. The exported **1757 × 2048** PNG contains transparent
  background pixels, translucent edge pixels and opaque artwork. Independent
  launches exporting the same frozen values produced identical PNG hashes.
- Private model assets, screenshots and local settings are excluded from the
  repository and portable bundle. Physical-phone and broader device acceptance
  checks remain below.

### v0.3 rig mapping, physics and resource bar

- Formatting, Clippy across all targets/features with warnings denied, and all
  **26 default tests** passed on Windows x64. Three optional tests require a real
  GPU and/or user-supplied Cubism runtime/model files.
- The supplied model imported **25 VTS assignments** with no missing or unsupported
  inputs. Every assignment's input endpoints changed its actual native parameter
  within that parameter's range, including custom mouth and cheek controls.
- Its authored physics file loaded **29 groups / 93 output assignments**, with
  no missing input/output IDs. During ten seconds of simulated movement all **92
  distinct physics output parameters** varied; every value remained finite and
  within the native limits, and applying the final pose deformed actual Core meshes.
- Independent single-axis VTS packets verify horizontal, vertical and roll output
  isolation and calibration. ARIA JSON's existing pitch/yaw/roll contract is preserved.
- Physics tests verify a push continues after the driver stops, eventually settles,
  disables cleanly, and remains close at 30/60/120 Hz. Invalid particle indices and
  delay values are rejected. Importer tests cover optional sidecar discovery/errors.
- Windows CPU-time normalization and real working-set/private-memory queries passed.
  A release screenshot on the RTX 5090 showed **CPU 0.7%, RAM 285 MiB, VRAM 958 MiB**
  at **59 UI FPS** with the user's model, mappings and physics enabled. These are
  one-second samples from a smoke run, not a benchmark or guaranteed resource budget.
- Release screenshot checks also verified the separate green output window and
  live loopback VTS tracking applied to the model: **491 valid packets, zero
  rejected, 9 subscription requests** at capture. Smoke tracking uses an available
  loopback port so it can coexist with a running app, and fails without a fresh face frame.
- Private model assets and screenshots remain local and excluded from distribution.

### v0.2 moc3 implementation

- Rust 1.98.1 Windows x64: formatting, Clippy with all targets/features and warnings
  denied, and **17** default tests passed. Two extra tests require local runtime/GPU
  dependencies and are explicitly ignored in ordinary CI.
- The opt-in native integration test passed with official **Cubism Native 5-r.5 /
  Core 6.0.1** and a user-supplied avatar. It loaded, changed parameters, verified
  actual vertex deformation, and unloaded/reloaded three times.
- The opt-in GPU test passed on Direct3D 12. Synthetic quads verify draw ordering,
  regular/inverted clipping (including invisible/zero-opacity mask sources), normal
  alpha blending, additive/multiplicative blending, multiply/screen colors and culling.
  It reads back GPU pixels and compares them with expected results. No proprietary
  assets are needed for this test.
- The user's actual model rendered **441 meshes and ten 4096px atlases**, totaling
  640 MiB decoded atlas storage. All 12 standard tracking IDs were detected.
- Native screenshot smoke runs verified `.model3.json` import with demo animation,
  direct `.moc3` selection with matching-manifest discovery, real loopback VTS UDP
  tracking applied to the avatar, and the separate green-screen output viewport.
  The private model and its screenshots are excluded from the repository/package.
- Synthetic importer tests cover exact texture ordering, differently named matching
  manifests, ambiguous manifests, missing textures, bounded reads and path escapes.

### v0.1 foundation checks

- Rust 1.98.1, Windows x64 GNU toolchain: format check, Clippy (all targets and
  all features, warnings denied), and all **13** core/model/tracking tests passed.
- Separate CLI processes: VTS phone simulator to receiver delivered **169** valid
  packets and **3** subscription requests during a three-second observation.
- Separate ARIA JSON sender to receiver delivered **171** valid packets and **0**
  subscription requests during a three-second observation.
- The desktop started on a real Windows Direct3D 12 adapter. Native GPU screenshots
  verified the demo studio, studio with simulated VTS input, and separate output
  viewport. The default 60 FPS target was observed in the studio after correcting
  egui's predicted-frame-time adjustment. This is a smoke check, not a benchmark.
- [Studio screenshot](images/studio.png) and [green output screenshot](images/output.png)
  show the actual app. Physical-phone and OBS acceptance remain listed below.
- The optimized desktop/CLI release build and portable ZIP packaging completed
  locally. The ZIP includes documentation and dependency license files. GitHub CI
  independently builds the documented Windows MSVC configuration.

## Automated coverage

- VTS decoding from a fixture spelling the documented PascalCase keys, `{k,v}`
  blendshapes, raw eye rotations and unknown future fields.
- Malformed/unrelated JSON, unsupported schema versions, excessive packet size,
  coefficient clamping and non-finite number rejection.
- Actual loopback UDP request/reply, recurring subscription requests, malformed
  packet handling, stale detection, recovery and prompt receiver shutdown/rebind.
- Old packet ordering and occupied-port errors.
- Neutral calibration across Euler 359°/1° wrapping, mirror/asymmetric blink,
  range clamping, face-loss neutral pose and frame-rate-independent smoothing.
- Model3 version validation, missing files and external/path-traversal references.

Additional local integration checks (not run by normal CI):

```powershell
# Uses synthetic quads; requires a working GPU, no SDK.
cargo test --locked -p aria-desktop gpu_clipping -- --ignored --nocapture

# Uses your local SDK and licensed model; neither is downloaded by the test.
$env:ARIA_CUBISM_CORE = 'C:\Tools\CubismSdkForNative-5-r.5\Core\dll\windows\x86_64\Live2DCubismCore.dll'
$env:ARIA_TEST_MOC = 'C:\Avatars\MyAvatar\MyAvatar.moc3'
cargo test --locked -p aria-live2d real_core -- --ignored --nocapture

# Requires a model with adjacent VTS profile and referenced physics file.
$env:ARIA_TEST_MODEL = 'C:\Avatars\MyAvatar\MyAvatar.model3.json'
cargo test --locked -p aria-desktop local_rig_assignments -- --ignored --nocapture

# Registers a temporary Ctrl+Alt+F11 key; requires interactive Windows.
cargo test --locked -p aria-desktop native_registration -- --ignored --nocapture
```

Run from the repository root:

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace
cargo build --locked --release --workspace
```

For a screenshot smoke build (development aid, not a hardware-independent test):

```powershell
cargo build --locked -p aria-desktop --features screenshots
$env:ARIA_SCREENSHOT_TO = "$PWD\aria-smoke.png"
.\target\debug\aria-desktop.exe
Remove-Item Env:ARIA_SCREENSHOT_TO
```

The feature enables A.R.I.A.'s wgpu screenshot-and-exit hook. A real graphics adapter and
interactive Windows desktop are required. Ordinary release packages do not enable
this feature. Check the resulting image for layout and the reported adapter.
Use `ARIA_SMOKE_SCENARIO=help-ranges` or `help-physics` for the separate native help
viewport; `help-parameter` includes the first loaded parameter's actual context.
Any bundled topic ID can follow `help-` to select another article. These tests use
the screenshot feature's repaint hook; the normal help window repaints on demand.
Set `ARIA_SMOKE_SCENARIO=vts` to connect to a running loopback VTS simulator, or
`ARIA_SMOKE_SCENARIO=output` to capture the separate green-screen output window.
The VTS smoke scenario selects an available receive port and advertises that port
to the simulator; normal app connections still use the user's configured port.
Smoke runs use default settings and do not overwrite saved app preferences.
For Live2D smoke runs, also set `ARIA_CUBISM_CORE` and `ARIA_TEST_MODEL` (a manifest
or moc3 path). A model-load failure fails the run instead of capturing the fallback
puppet. `ARIA_SMOKE_DELAY_SECONDS=7` allows additional settling time after import.
The `inputs`, `pose` and `presets` scenarios prepare example configurations and
open the corresponding monitor tab. Set `ARIA_SMOKE_AVATAR_PNG` to a local PNG
path to exercise transparent model export as well as the UI screenshot. These
scenarios do not register global shortcuts or overwrite normal saved settings.
The `physics` scenario opens overall/group controls for the loaded avatar;
`physics-group` expands its first actual group with overall controls collapsed.
Omitting `ARIA_TEST_MODEL` exercises the built-in puppet and empty physics state.

Output smoke scenarios also include `output-freeform`, `output-resize` (changes
the Freeform preview to 600×400 and canvas to 2048×1024 after two seconds), and
`capture-minimized` (opens and then minimizes all three previews, asserting native
minimized state while senders continue). `ARIA_SMOKE_OUTPUT=0`, `1`, or `2` selects
the output screenshot target. These hooks require the `screenshots` feature;
normal portable builds do not run them or change user layouts automatically.

## Manual acceptance still required on the intended setup

1. Physical iPhone running the user's VTube Studio version, permissions and Wi-Fi.
   Confirm valid packets, head directions, blinks, mouth, gaze, calibration and
   recovery after backgrounding/reopening the phone app.
2. Windows Firewall helper on the user's Private network, then removal of that rule.
   The app never applies it automatically.
3. Final OBS scenes/filter settings on the intended streaming setup. Full-resolution
   Spout was tested above; Window Capture fallback alpha and custom chroma-key
   similarity still depend on capture method and artwork.
4. Different DPI/display combinations and older supported GPUs.

5. Additional real rigs and export versions, particularly models requiring pose,
   complex physics, motions, custom layout or Cubism 5.3 advanced rendering. Unsupported
   features are listed in [the compatibility guide](live2d.md#compatibility-and-limits).

Simulator success is evidence for the documented protocol and receiver lifecycle;
it is not evidence of a completed physical iPhone test. The model and GPU smoke
checks are not a performance benchmark or a full Cubism conformance suite.
