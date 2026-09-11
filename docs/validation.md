# Validation

This file records checks for the development builds. The Windows CI workflow
is the repeatable MSVC build/test path; its status belongs to a specific commit.

## Local verification on 2026-09-11

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

Additional local v0.2 checks (not run by normal CI):

```powershell
# Uses synthetic quads; requires a working GPU, no SDK.
cargo test --locked -p aria-desktop gpu_clipping -- --ignored --nocapture

# Uses your local SDK and licensed model; neither is downloaded by the test.
$env:ARIA_CUBISM_CORE = 'C:\Tools\CubismSdkForNative-5-r.5\Core\dll\windows\x86_64\Live2DCubismCore.dll'
$env:ARIA_TEST_MOC = 'C:\Avatars\MyAvatar\MyAvatar.moc3'
cargo test --locked -p aria-live2d real_core -- --ignored --nocapture
```

Run from the repository root:

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
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
Set `ARIA_SMOKE_SCENARIO=vts` to connect to a running loopback VTS simulator, or
`ARIA_SMOKE_SCENARIO=output` to capture the separate green-screen output window.
Smoke runs use default settings and do not overwrite saved app preferences.
For Live2D smoke runs, also set `ARIA_CUBISM_CORE` and `ARIA_TEST_MODEL` (a manifest
or moc3 path). A model-load failure fails the run instead of capturing the fallback
puppet. `ARIA_SMOKE_DELAY_SECONDS=7` allows additional settling time after import.

## Manual acceptance still required on the intended setup

1. Physical iPhone running the user's VTube Studio version, permissions and Wi-Fi.
   Confirm valid packets, head directions, blinks, mouth, gaze, calibration and
   recovery after backgrounding/reopening the phone app.
2. Windows Firewall helper on the user's Private network, then removal of that rule.
   The app never applies it automatically.
3. OBS Window Capture on the user's OBS/GPU setup: output selection, green key,
   resize/minimize behavior, and whether their capture method retains alpha.
4. Different DPI/display combinations and older supported GPUs.

5. Additional real rigs and export versions, particularly models requiring pose,
   physics, motions, custom layout or Cubism 5.3 advanced rendering. Unsupported
   features are listed in [the compatibility guide](live2d.md#compatibility-and-limits).

Simulator success is evidence for the documented protocol and receiver lifecycle;
it is not evidence of a completed physical iPhone test. The model and GPU smoke
checks are not a performance benchmark or a full Cubism conformance suite.
