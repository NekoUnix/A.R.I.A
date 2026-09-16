# ARIA changelog

All downloads remain Alpha. See [Releases](https://github.com/NekoUnix/A.R.I.A/releases)
for packages and checksums. Features labeled Experimental have compatibility limits.

## 0.35.0-alpha.1 — 2026-09-15

- Mount an additional Live2D model visually using a point on each model. Both
  mesh points stay joined during tracking and physics, including rotation, scale
  and flip. Mounts save with avatar settings/presets and appear in OBS outputs.
  Added models keep independent tracking mappings and physics. Access the new
  two-preview window from Stage → Objects → Add & mount Live2D.

- Fix gentle GLB/VRM spring motion stalling or responding on only one side.
  Preserve very small rotations through simulation and collision recovery instead
  of rounding them to zero. Verify left and right breast chains independently,
  including visible mesh deformation at Medium defaults with wind zero.
  Refine generated collision convergence and remove push-out overshoot so
  close-fitting straps and accessories settle without suppressing small motions.

- Stabilize default VRM/GLB hair and accessory motion during head tracking.
  Advance every rendered frame with bounded 8.3 ms substeps, interpolated tracking
  poses, time-scaled damping and one-time strength blending. Prevent tiny links and approximate
  collisions from launching joints across their bend limits; preserve sliding
  momentum and gradually recover conflicting generated contacts.

- Fix GLB/VRM clothing being forced outward or flipping during arm idle motion.
  Generated collision clearance now follows the current posed skeleton, including
  relaxed arms, and all groups use a consistent collision snapshot. Blend idle
  strength, speed and on/off changes smoothly while retaining the full 0–2 range.

- Add default body and flexible-group collision envelopes for generated GLB/VRM
  physics, fitted from the weighted rig. Add body size, spring thickness, shape
  counts and contact feedback with per-avatar persistence. Retain authored VRM
  shapes and existing rest-pose overlaps. Correct contacts after bend/length
  constraints, check bone segments and catch fast tip crossings.

- Expand VRM/GLB physics guesses to breast/body bones, earrings, animal ears and
  more clothing/accessories. Preserve unweighted chain endpoints and estimate a
  bounded virtual endpoint for weighted single bones. Protect tracking, fingers,
  facial controls and twist helpers from generated physics.
- Add a searchable inspector for all imported skeleton bones, with role/weight
  explanations and per-avatar Auto, Simulate and Keep rigid overrides.

## 0.34.0-alpha.1 — Flexible avatars, lighting and visual actions

- Add Medium-default VRM/GLB secondary motion, automatic hair/tail/ear/clothing
  chains, manual roots, Soft/Medium/Firm presets, damping and bend limits.
  Preserve authored VRM groups and colliders; save all tuning per avatar.
  Fix head-centered springs losing inertia, bound large time steps and reuse
  the solver's scratch buffer. Freeze preserves generated and authored joints.
- Add per-avatar light color/direction and even lighting for all avatar formats.
  3D uses surface normals; Live2D/PNG/GIF use a tint and soft image gradient.
  Preserve alpha, profile/preset settings and shared OBS rendering.
- Update reviewed file-dialog/WebSocket libraries and pinned GitHub cache actions.
  Keep webcam NumPy/OpenCV and SHA-2 upgrades deferred pending compatibility fixes.

- Add **Experimental VRC / GLB humanoid import**, a guided file picker and stage-drop
  support. Share existing webcam, RTX, VTube Studio, iFacialMocap and microphone
  tracking, calibration, profiles, output composition, pins and action nodes.
- Detect common humanoid bones and VRC/ARKit facial shapes; allow per-avatar bone
  overrides and editable face inputs. Keep authored morph weights and expose up to
  1,024 shapes per mesh / 2,048 shape controls, within the existing aggregate memory
  budget. Include format and bone overrides in support reports. Validate with the
  supplied ICHIGO GLB. [Setup and compatibility limits](docs/glb-avatars.md).
- Fix black workspace backgrounds when Glass surfaces is disabled, including Sakura.
- Show clear stage tabs; rename a stage/profile throughout ARIA without changing its identity.
- Collapse stage tools below the stage name and show current numbers and units below resource graphs.
- Replace output framing controls with mouse gestures: Ctrl+Shift+click (Cmd+Shift+click on macOS)
  locks only the clicked avatar in that output. Right-click offers centering, reset and window options.
- Add a dedicated shortcut recorder with conflict checks, global Windows registration and focused
  shortcuts on other platforms. Preserve imported VTS hold/release behavior and existing bindings.
- Add saved visual action graphs: toggles, expressions, presets, images, objects, effects, VRM
  gestures, profile changes, delays, branching and joins. Include validation, cancellation,
  templates, API triggers and per-node repair messages. See [the action guide](docs/actions.md).

## 0.33.0-alpha.1 — Glass workspace, tracking direction and engine efficiency

- Correct VTube Studio UDP head roll before calibration and model mapping.
  Keep pitch/yaw independent, preserve angle wrapping, and apply user Mirror and
  Invert roll controls exactly once. Shared profiles and attached Live2D models
  receive the same corrected tracking input; other protocols are unchanged.
- Migrate saved VTS neutral lean, personal lean ranges and movement-preset
  calibration once, including disabled and shared-tracker workspace profiles.
  Retain authored bindings, explicit inversion, poses, hotkeys and unrelated axes.
  Users who added their own inversion workaround should remove it after updating.
- Refresh the workspace with Apple-inspired glass surfaces, rounded controls,
  quieter category accents and a clear outline around the editing profile.
  Add Glass Dark and Glass Light; preserve existing saved palettes and imports.
  Offer solid surfaces and retain opaque High Contrast. Use static UI geometry
  and blending without blur passes, extra render targets or animation.
- Correct the macOS Syphon publisher's vertical texture-origin conversion so
  OBS receives an upright, unmirrored canvas with preserved alpha and color order.
  Remove old OBS rotation/flip workarounds after upgrading.
- Avoid repeated allocation/copying of camera preferences and workspace output
  maps during live multi-avatar settings handoffs. Leave output layout, target
  frame rate and window policy owned by the workspace.
- Copy tracking snapshots for cross-profile distribution only when a loaded
  follower needs that source; avoid duplicate blendshape maps for unshared input.
- Skip native Syphon GPU submissions for unchanged canvases. Retain pending
  changes while no clients are present, including the latest frozen image for
  late/reconnecting clients. GPU work remains bounded and never waits on OBS.
- Add a reproducible optimized settings-handoff benchmark and native Syphon
  client tests for orientation, left/right order, alpha, BGRA/RGBA, resize,
  late clients and idle submissions. Run Syphon checks in macOS pull requests
  and both Mac release builds.
- Confirm current stable graphics/UI dependencies (wgpu 30.0.1, egui/eframe
  0.36.2); improve ARIA's own runtime without an unnecessary library change.
  Update setup, troubleshooting, performance, dependency and platform guides.
  VTube Studio import and VBridger import remain **Experimental**.

## 0.32.0-alpha.1 — Multiple avatars and Profiles

- Load several Live2D, PNG/GIF and VRM avatars together using Workspace → Profiles.
  Check profiles to load them, uncheck to release their resources, and use Edit or
  stage tabs to choose the avatar being configured. Clearly label the editor,
  inspector and stage with that avatar's name.
- Keep independent live model workers, animation clocks, poses, tracking filters,
  physics, expressions, layers, appearance, image actions, pins and throw effects.
  Changing the editing tab does not reload or freeze the other avatars.
- Combine loaded avatars in all three OBS canvases. Drag and scale them separately,
  choose overlapping avatars by name, change their front/back order, and save an
  independent arrangement for landscape, portrait and Freeform. Native output
  omits preview targeting controls and renders at the configured full resolution.
- Save the profile list, enabled avatars and layouts across restarts. Existing
  per-model rig settings remain usable on import. Loading failures retain the
  catalog entry and leave other avatars running.
- Share one profile's face tracking with other avatars while preserving their
  individual response and calibration. Separate connections remain available;
  connecting a device is still explicit. Matching global shortcuts target all
  loaded profiles that use that shortcut.
- Include all loaded avatar workers in resource counters, all active artwork in
  key-color detection, and each loaded avatar in exported diagnostic reports.
  The existing API acts on the editing profile and reports the workspace roster.
- Update illustrated user instructions, offline help and platform download links.
  VTube Studio import and VBridger import remain **Experimental**.

## 0.31.0-alpha.1 — Highlighted selections and protected Live2D groups

- Open **Select layers…** in a separate resizable window with a frozen model.
  Default Select mode toggles a layer with each click and adds repeated boxes,
  with explicit Remove, Replace and Toggle buttons; no Shift key is needed.
- Tint selected artwork blue using its actual atlas transparency and clipping;
  outline the hovered mesh in gold. Click selected artwork again to deselect it,
  then use the remaining selection for visibility edits or reusable hotkey groups.
  Highlights are preview-only, never saved as avatar colors or sent to outputs.
- Save protected layer groups per avatar. Clicks, boxes, list ranges and search
  selection skip their members unless Include protected layers is enabled.
  Switching the override off removes protected members from the selection.
  Protection is independent of visibility/hotkeys; existing profiles default
  to unprotected groups. Manage it in the frozen window or group settings.
- Preserve fast mouse gestures when press, movement and release arrive between
  rendered frames, and process a gesture once across repeated UI layout passes.
- Hide/restore selections, undo up to 24 local edits, zoom, pan, refresh the pose,
  reveal ARIA-hidden artwork in the preview, and save groups for existing hotkeys.
  Visibility saves per avatar and reaches all outputs while live tracking continues.
- Share uploaded atlases and immutable renderer resources; isolate preview
  geometry, writable buffers, masks and output. Render the frozen pose only when
  its layer settings or selection change, virtualize the selection list and release the
  preview when closed or the model changes.
- Update the mouse guide, contextual help and native screenshot. VTube Studio
  import and VBridger import remain Experimental.
- Add a bread button beside the footer social icons: click to send a small
  bread burst across the stage and outputs. Built-in artwork needs no downloads;
  bursts expire automatically, are rate limited, and do not change avatar settings.
  Bread is also available in the throw designer's built-in asset choices.

## 0.30.0-alpha.1 — Live2D drag selection and customizable avatars

- Drag a rectangle over the stage to select several exported Live2D layers.
  Shift adds, Alt removes, Ctrl/Cmd toggles and Escape cancels; click selects the
  frontmost mesh. Shift-click and dragging across list buttons select ranges.
  Bulk hiding/restoration, named visibility groups and their hotkeys share the selection.
- Add a dedicated **Avatar → Customize** workspace using the loaded model's
  exported parameter names, ranges and nested folders. Suggest appearance controls,
  browse every parameter, organize categories and create sliders, toggles or named choices.
- Hold appearance values without replacing tracking assignments. Release controls
  to restore existing behavior, or change them during a frozen screenshot pose.
- Save appearance-only looks with preset export/import and Windows shortcuts.
  Recall parameter overrides, layer visibility/groups and colors while retaining
  current tracking, physics, props, expressions and curated control names.
- Tint selected layers and restore authored colors. Active-group restoration is
  explicit; stage selection guides never appear in OBS or exported PNGs.
- Include appearance settings, authored parameter folders and layer overrides in
  diagnostic reports. Add illustrated user guides and contextual offline help.
- VTube Studio and VBridger imports remain Experimental. Customization works with
  exported artwork and controls; merged or absent source layers cannot be recovered.

## 0.29.0-alpha.1 — Experimental VTube Studio import and diagnostics

- Experimental, model-owned `.vtube.json` import with category review, Cancel and Undo.
  Transfers tracking ranges, strength/wind and group multipliers, mesh colors,
  expressions, hotkeys, phone buttons, idle/triggered motions and item scenes.
- Native motion3 parameter/part-opacity/model tracks; folder-based Live2D items
  and naturally ordered PNG/JPG frame animations. Approximate saved framing,
  configurable tracking sway, timed toggles, hold expressions and Windows key chords.
- Missing assets and unrecognized actions remain repairable inside ARIA. The
  three-step guide assigns local expressions, motions, item scenes or model controls.
  Tracking repair opens the existing one-step-at-a-time calibration guide.
  No execution is forwarded to VTube Studio.
- Readable diagnostic report export with structured JSON, recent error history,
  bounded local logs and panic recovery, avatar topology/parameters/physics,
  texture metadata and resource counters. Redacts personal paths and credentials.
  Export confirmation links to GitHub issues and NekoUnix’s Discord for tickets.
- Small local-vector social buttons in the footer. Opening links never follows,
  subscribes, joins or modifies accounts automatically.
- Fix browser links across the app: social icons, Cubism downloads, online help,
  support tickets and streaming-service sign-in now open the default browser.
  Restore the UI framework's native link support on Windows, Linux and macOS,
  with a repository check to prevent it being disabled in future builds.
- VTube Studio import and VBridger import are both explicitly Experimental.


## v0.28.0-alpha.1

- Connect an iPhone running iFacialMocap directly over live UDP. Receive facial
  expressions, head motion/position and eye rotations with legacy/v2 support.
- Keep iFacialMocap addresses and ports separate from VTube Studio and save them
  per avatar. Use the existing calibration, illustrated guide, model mappings and
  Experimental VBridger equations with the new source.
- Add connection help, stale-stream recovery, protocol validation and a local
  simulator for developers. Physical phone latency remains unmeasured.
- Rewrite the README around first-time setup, avatar choice, tracking, OBS and
  troubleshooting. Move technical references below user instructions and refresh
  the project's About description and topic tags.
- Add official Cubism Native SDK download links for Windows, Linux, Apple Silicon
  and Intel Macs, with exact library filenames and illustrated-app setup steps.

## v0.27.0-alpha.1

- **Experimental VBridger import and editor:** per-avatar equations, input/output
  curves, weighted tangents, calibration offsets, ranges, delay, smoothing, steps,
  face-loss controls and exports. [Compatibility details](docs/vbridger.md).
- Colorful performance graphs with hover values and session low/high records,
  a compact footer and collapsible details. [Release notes](docs/release-v27.md).

## v0.26.0-alpha.1

- Faster configurable mouth response, expanded resource counters and frame timing.
- Live2D layer visibility groups and hotkeys, independent movable pins, tracking
  for attached Live2D models, and configurable VRM idle motion/gestures.

## v0.25.0-alpha.1

- Native OBS output through Windows Spout2, macOS Syphon and Linux ARIA Canvas.
- Windows/Linux/macOS packages, including Fedora and Arch app/OBS packages.
- Individual guided tracking exercises, an illustrated face and selectable takes.

## Earlier versions

Live2D, VRM and PNG/GIF avatars; webcam tracking; optional Experimental NVIDIA RTX
tracking; editable themes; controller and microphone input; expressions and
hotkeys; pinned objects; throws, sprays and image animations; Twitch/YouTube chat;
and the local control API. Historical validation and limitations are recorded in
the [documentation index](docs/README.md) and older release notes.
