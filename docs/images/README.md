# Documentation screenshots

## v0.33 glass workspace

`profiles-v33.png` and `glass-light-v33.png` are direct renders of the real v0.33
application UI through egui/wgpu, using Glass Dark and Glass Light respectively.
They use the owner's full Odette, OilBun, Odette GIF and NekoUnity2 VRM together,
isolated test state and synthetic Demo tracking. Resource readings are not
performance benchmarks. Only these unretouched UI renders are distributed, not
model sources, Core binaries or private settings.

## v0.32 multiple avatars and Profiles

`profiles-v32.png` renders the real application workspace UI through egui/wgpu.
`multi-avatar-landscape.png` is the same full-resolution GPU composition used for
native OBS output, saved as a transparent PNG. Both use isolated test state and
the owner-provided full Odette, OilBun, Odette GIF and NekoUnity2 VRM assets.
Face input is synthetic Demo input shared across the four independent runtimes;
the UI counters are not performance benchmarks. These are direct renders without
image retouching. Only the images are distributed, not model sources, SDKs or
private configuration files.

## v0.30 Live2D customization and drag selection

`customization-v30.png` and `layer-selection-v30.png` are unedited native Windows
captures of the v0.30.0-alpha.1 screenshot build, using the owner's full Odette
export and isolated profiles. The appearance workspace holds exported hair/outfit
switches; the layer image uses injected pointer events through normal egui stage
interaction and asserts a multi-layer selection before capture. The automatic
import-preview window is closed so the editing surface is visible and interactive.
Tracking is visibly labeled Demo. Debug counters are not performance benchmarks.
Only the UI images are distributed; no model sources, SDKs or profiles are bundled.

## v0.29 Experimental import and diagnostic reports

`workspace-v29.png`, `vts-import-v29.png`, `vts-repair-v29.png` and `diagnostics-v29.png` are unedited
native Windows captures with the owner's Odette export and an isolated profile.
The repair screenshot demonstrates the local guide using an existing action.
The diagnostic report exercise records a clearly labeled synthetic test event,
not an actual model failure. Demo tracking and debug counters are not device
acceptance tests or performance benchmarks. Only screenshots are distributed;
private profiles, exported reports, model assets and SDK binaries remain local.

## Avatar customization source upgrade — 2026-09-13

`live2d-layers-upgrade.png` shows the owner-provided Odette 90s outfit with the
exported ArtMesh/part list. `vrm-motion-upgrade.png` shows the owner-provided
NekoUnity2 VRM performing the procedural Wave gesture beside its motion controls.
Both are unedited native Windows screenshots from the development build using
isolated profiles, within the owner's documentation-image authorization.
Neither demonstrates physical tracking. Debug resource readings are not benchmarks.
Only screenshots are included; the original models, SDK and profiles remain private.

## Earlier captures

The `*-v23.png` images were captured from ARIA v0.23 on Windows on 2026-09-12.
The owner explicitly requested current-build screenshots using previously supplied
models. These rendered illustrations are included for that purpose; raw avatar
exports, texture atlases, SDK binaries and private settings are excluded.

- Live2D: the owner's **NekoUnix Full Model / Odette**, 90s outfit export.
- PNG/GIF: the owner's **Odette GIF Tuber** artwork, supplied at 3500 × 2500.
- VRM: the owner's **NekoUnity2.vrm**, displayed with its embedded model name.
- Legacy screenshots show the application's original Mica test puppet; current
  guides use the v0.23 captures. Historical images are retained for old links.

Artwork copyright and any model-specific usage terms remain with their respective
creators/rightsholders. ARIA's MIT license covers its code, not the depicted avatar
art. These screenshots do not grant model download or reuse rights.

Screenshots use the feature-gated native screenshot harness with an isolated
`ARIA_PROFILE_DIR`; no personal app profile is modified. Camera/RTX panels show
configuration, not fabricated live inference. Guided tracking review uses a
deterministic calibration rehearsal. Demo movement is labeled in the interface.
No login credentials, API keys or raw camera images are included.

## v0.24 renderer update

`workspace-v24.png` is an unedited native Windows capture of the v0.24 screenshot
build on 2026-09-13, with the isolated Cubism process, using the same owner-provided Odette Live2D export with permission. It uses
an isolated test profile and Demo input. Resource numbers from this debug capture
are not a release benchmark. Raw model files and SDK binaries are not included.

`tracking-takes-v24.png` shows the individual-exercise tracking guide from the
2026-09-13 development build, with the owner's Odette avatar behind it. The face
and two takes use a deterministic synthetic rehearsal, labeled in the UI. This
illustrates controls and layout, not a physical camera or phone recording.

- `outputs-alpha-v25.png`: v0.25 Alpha Windows output controls with the owner-provided Odette model and synthetic demo tracking; records Alpha branding and native-output wording. Captured from an isolated profile. This does not represent Linux/macOS hardware acceptance.

## v0.26 speech and resource controls

`responsive-speech-v26.png` and `performance-details-v26.png` show the v0.26 Alpha
native Windows screenshot build, using the same owner-provided full Odette Live2D
model and visibly labeled Demo input. The isolated profile allows the model and
OS counters to settle before capture. The Details image includes real local
machine counters from an optimized build; this short UI check is not a controlled performance benchmark.
Use screenshot scenarios `responsiveness` and `performance-details` to reproduce
the panels. No phone recording, account data, Core binaries or avatar source
files are included.

## Performance graph development update

`performance-graphs.png` and `performance-footer.png` are unedited native Windows
captures of the optimized performance-graph source build on 2026-09-13. The
owner-provided full Odette model uses an isolated profile and visibly labeled
Demo input. CPU/memory/frame histories are actual local one-second samples;
tracking graphs correctly show N/A without a connected tracking source. These
short UI checks are not controlled performance benchmarks. The expanded view
shows the Overview plus collapsed detail categories. Raw avatars and SDK
binaries are not included.

## v0.27 experimental VBridger update

`vbridger-import.png` is an unedited native Windows capture of the optimized
v0.27.0-alpha.1 screenshot build. It shows the owner's supplied
AdvancedARKitSettings import draft with 34 outputs and the explicit Experimental
label. The owner-provided Odette model is behind the floating editor. Tracking
uses visibly labeled Demo input; this is an import/layout check, not a phone
recording. An isolated profile protects personal settings. The screenshot
contains no raw config, model files or SDK binaries.

## v0.28 iFacialMocap update

`ifacialmocap-v28.png` is an unedited native Windows capture of the optimized
v0.28.0-alpha.1 screenshot build with the owner's full Odette model. A separate
loopback CLI simulator sends real UDP packets through the new adapter; the UI
explicitly labels the source as simulated. Temporary test ports avoid collisions
with a user's tracker. This validates connection/layout/rendering, not physical
iPhone capture or latency. An isolated profile protects saved app settings.
The README's Cubism instructions were checked against the official Native SDK
page/library list and the locally extracted SDK's Windows/Linux/macOS layout.
No private avatar files, config exports or SDK binaries are included.

## Frozen selector and bread development update

`frozen-layer-editor.png` was refreshed for v0.31.0-alpha.1 on 2026-09-14 with
the owner's full Odette avatar and an isolated profile. Actual mouse interaction
created a protected group of 178 head-area meshes; a full-model box then selected
200 other meshes, shown blue. Turning the protection override on, reselecting,
and turning it off verified counts of 378 then 200. The image is an unedited
native Windows capture; blue is the editor's selection highlight, not a model tint.
The picture was captured through the desktop because eframe's immediate wgpu
viewport does not handle its Screenshot command. The live stage continued
demo tracking while this preview stayed frozen. No avatar sources are included.

`bread-footer.png` shows the built-in bread action in the native Windows app.
The isolated screenshot run pauses the transient burst after 0.8 seconds to
make it visible in the capture; normal playback crosses the screen and expires.

## Action editor development update — 2026-09-14

`sakura-solid-development.png`, `action-nodes-development.png` and
`hotkeys-development.png` render the actual egui workspace through ARIA's native
Windows GPU renderer. They use isolated test settings, synthetic Demo tracking,
the owner's supplied full Odette model, OilBun model, Odette GIF and NekoUnity2 VRM.
No private model files or SDK binaries are included. Only the main Odette stage
is visible; all four profiles are loaded. The test also renders each avatar into
all three native canvas formats and verifies their workers survive stage changes.
These screenshots show unreleased changes based on v0.33; their version label
does not claim a published new release. Slow stepping and resource readings in
this debug screenshot test are not a performance benchmark. No image editing
was applied to these captures.
