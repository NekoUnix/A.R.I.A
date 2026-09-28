# ARIA changelog

## Unreleased

- Add a resident GPU warp-chain pass after keyframe and blend-shape evaluation. Rust precomputes immutable work by hierarchy depth, and the GPU resolves child warp control points before transforming mesh vertices without an intermediate readback. Native parity passed on three private MOC3 models, including 276,025 hierarchy points on the large outfit and direct comparison with full Rust geometry for eligible meshes. Mixed rotation branches, glue and final render hookup remain.

- Add a second resident GPU geometry pass for MOC3 blend-shape position deltas. Rust builds a fixed per-point schedule at load time, retains all delta positions on the GPU, and uploads only active blend weights/counts per pose. The combined normal-key and delta passes match the Rust decoder across three poses on the large outfit, OILBUN and Ditto. Hierarchical deformers, glue, final bounds and active renderer connection still remain.

- Replace the pose-specific GPU keyframe test packing with a reusable Rust load-time plan and desktop compute evaluator. Mesh positions lead the output buffer for direct vertex binding; resident key positions and per-vertex work remain fixed while only active offsets, weights and counts upload per pose. Three-pose DX12 parity passed on the large outfit, OILBUN and Ditto. The large outfit uses 24.03 MiB static keys, 15.94 MiB static work and about 41 KiB changing data per pose. Production blend shapes, hierarchy, glue and renderer hookup remain unfinished.

- Add a renderer entry point that binds GPU-computed MOC3 positions directly as the vertex source, skipping the per-frame CPU position upload and any GPU readback. A native masked-mesh fixture generated its vertices in compute and matched the CPU-rendered image pixel for pixel, even with deliberately incorrect CPU coordinates. The full GPU evaluator is not yet connected to this entry point.

- Add a conflict-aware Rust glue scheduler and test-only GPU glue pass. The scheduler runs independent pairs together, preserves source order where glues share vertices, and signals a CPU fallback for within-glue overlap. GPU parity passed on the large outfit's 1,365 pairs in one pass and OILBUN's 2,298 pairs in two ordered passes. This is not yet wired into the live renderer.

- Add a GPU compute pass for additive warp and ArtMesh blend-shape deltas after normal keyframe blending. The large 90s outfit matched Rust with 1,477,244 resident delta points affecting 458,945 positions; OILBUN also passed. A generated chain carries a mesh delta through parent warps and rotation without readback. This remains test-only until complete hierarchy, glue and renderer integration pass.

- Add a test-only GPU rotation pass that consumes GPU-resident mesh positions after warp hierarchy resolution. Authored local rotation frames from the large 90s outfit, OILBUN and Ditto match Rust, including angle, scale and reflection coefficients. Hierarchical rotation state and live-renderer integration still need work.

- Extend the batched GPU keyframe parity path to ArtMesh base positions. The large model matched Rust on all 1,210 meshes and 225,448 vertices in addition to its warp grids; OILBUN and Ditto passed too. A generated GPU-only hierarchy carries blended points through two parent warps into final mesh positions. Blend shapes, rotations, glue and renderer integration remain open.

- Add a test-only GPU compute stage that blends resident MOC3 warp keyforms in one batched dispatch. A non-default pose of the supplied large model matched Rust across 680 warp nodes and 819,275 points, including 327 multi-key nodes. A generated two-depth chain then consumed blended control points directly in GPU storage without intermediate readback. This targets a known CPU cost but is not connected to the live renderer yet.

- Prototype a batched DX12 compute warp sampler for the independent Rust MOC3 core. It matches ARIA's affine, bent, quad, triangular and exterior sampling on generated grids and decoded control grids from three supplied models. A two-level child/grandchild dispatch resolves parent control points in GPU storage without intermediate CPU readback. This is a tested primitive, not yet the active Live2D renderer; keyform blending, rotation parents, glue and final mesh output still need GPU integration.

- Let VRM/GLB meshes run up to 32 simultaneous expression morphs in the GPU vertex shader when the shared 64 MiB budget and device limits permit, instead of falling back after eight. Handle valid position-only morphs by supplying zero normal deltas. Generated DX12 twelve-morph and local NekoUnity2 render checks pass. A model survey found that simple unparented-mesh GPU interpolation would cover none of the large 90s outfit's 225,448 vertices, so Live2D needs a hierarchical GPU evaluator.

- Reuse unchanged Rust MOC3 deformer states across frames with explicit normal-key and blend-shape parameter dependencies, propagating changes through parent deformers. This reduces large-model CPU evaluation for narrow parameter updates while preserving official-Core output across all 28 supplied exports. A broad 32-axis animation remains slower than the transitional runtime; GPU MOC3 deformation is still pending.

- Add an opt-in in-process ARIA Rust MOC3 path for profiling without worker serialization. Adopt evaluated mesh position buffers directly in the Rust adapter and decoded worker frames instead of copying every vertex into an older buffer. Its large-model default-pose GPU render matched the Rust worker pixel for pixel, but a 120-FPS-target desktop run was slower than the current runtime, so the default remains unchanged.

- Match the official Core directly on all 28 supplied MOC3 exports, including an export the transitional Purism loader rejects. Reduce Rust geometry time by recognizing affine warp grids while retaining full interpolation for bent grids. Validate draw-group graph ownership and cycles before rendering; the large-model worker remains slower than the current runtime.

- Add an opt-in, independently authored Rust MOC3 worker adapter with hierarchical final render ordering; keep the default runtime unchanged while compatibility and performance gates remain open. Move Live2D canvas projection into the GPU vertex shader and reduce CPU vertex preparation. Local GPU render and official Core differential checks cover the tested models; the Rust worker is still slower on the measured large export.

- Move common VRM/GLB expression morphing into the GPU vertex shader, with up to eight active targets per mesh and a 64 MiB model budget. Fall back to CPU morphing for larger active sets and keep picking accurate without deforming every vertex on the CPU. Compute shared skin palettes once per frame. Expand the independent Rust MOC3 evaluator with RAM-compiled deformer keyforms, part/mesh binding visibility, colors and integer draw orders. Bundle the Windows runtime DLLs needed by portable builds.

- Make the Windows source launcher use the optimized release executable and warn when it is missing. Mark debug builds in the Studio footer, add opt-in Live2D and VRM/GLB timing diagnostics, and stop re-copying static Live2D mesh UVs, indices and masks on every update. Keep UVs in a separate GPU buffer so only changing positions upload per frame, bind geometry once per render pass, and use a single-channel mask target. Avoid repeated descendant-tree walks in VRM/GLB spring simulation. Document the measured limits of a large model at a 120 FPS target.

- Replace the Studio header wordmark with a Vaelari AI model chibi mascot and the exact A.R.I.A. / Avatar Studio lockup. Bundle a transparent, optimized header icon and full-resolution art master without including the user's private reference sheet.

- Refine Studio navigation with compact grouped rows, left-aligned labels and an active accent marker. Add collapsible reaction cards and subtle lower card shading. Replace per-frame reaction JSON serialization with direct change detection and avoid building gradients for offscreen cards.

- Add quick-start audience reaction templates, disabled rule duplication, per-rule session dispatch limits and reset controls. Explain skipped events in bounded recent history, including cooldown, minimum amount, duplicate, provider-test and session-limit decisions. Explicit previews preserve live counts.

- Add a local Streamer.bot WebSocket event connection for Twitch rewards, follows, subscriptions, gift bundles, cheers and raids, plus YouTube membership and support events. Include challenge authentication, bounded queues, platform filters, provider-test opt-in and shared-chat/gift-bundle duplicate suppression.
- Add a workspace sound library and sound/stop-sound action targets, with background decoding, volume controls, file repair and bounded playback. Events, hotkeys and action graphs can combine sounds with avatar reactions.
- Organize audience reactions into Reactions, Connections and Sounds tabs with model-colored gradient cards. Create disabled rules from received events and preview a selected reaction before enabling live triggers.
- Document the explicit custom-event adapter contract for other sources; TikTok/X website connections and native reward creation remain separate work.

- Add True/False action-graph branches, inactive-path-aware joins, timed repetition of a single action and a Choose and repeat recipe. Bound repeats, preserve cancellation and prevent catch-up bursts after delayed frames.

- Add an experimental Windows VST3 music effect with separate-process probing and processing, mono/stereo negotiation, saved normalized controls, bounded buffering and visible timeout/crash recovery. Preserve bounded reported tails; keep native editors, effect chains and shared audio routing marked unfinished.

- Connect opt-in live Twitch/YouTube !commands to event rules, with bounded queues, backlog suppression, duplicate protection and pending-command moderation removal.

- Extend gesture rules with up to eight All/Any input conditions, upper/lower thresholds and release hysteresis; clear partial holds after disabling or editing a rule.

- Add a Windows Studio browser window using WebView2, with tabs, local bookmarks/history, per-tab zoom and prompted downloads. Isolate the browser in a separate ARIA process with no web-to-avatar command bridge.
- Add persistent music output selection, including installed virtual audio devices, with explicit missing-device errors and release of the previous endpoint on route changes.

- Add named scene layouts with smooth movement transitions, capture/update controls, stable IDs and hotkey/action integration. Preserve output resolution and sender state when recalling a layout.
- Add streaming local music, multiple playlists, seek/pause/next, repeat/shuffle, missing-file repair and speech ducking. Keep file opening off the UI thread and prevent cancelled loads from restarting playback.
- Add opt-in stream-event mappings, simulation, duplicate protection and cooldowns; add avatar-scoped held-input gestures. Direct native provider event subscriptions remain pending.
- Extend action graphs with per-run variables, arithmetic, tracking-input reads and stop conditions.
- Add Bouncy/Natural styles for VRM and GLB spring chains; retain legacy behavior on older profiles. Add gradient panel shading and popup shadows while retaining solid/high-contrast themes.
- Add an experimental bounded VMC/OSC head and facial-input receiver. Full-body/finger retargeting is not yet implemented.

- Introduce a studio workspace with persistent navigation, Home readiness links, contextual controls, searchable tools, focus mode, local notes and a session timer. Preserve existing profiles, saved themes and output settings.
- Add Neko Studio as the default palette, with pink, mint and lavender colors, workspace tabs, an icon sidebar and an original studio backdrop. Retain Midnight Studio and existing saved palettes.
- Replace end-user chat API setup with website Connect buttons. Supply publisher-owned OAuth registrations during release builds; clearly identify builds where sign-in is not yet enabled.
- Add timed and parallel action recipes with explicit target assignment.
- Add optional momentum-based throw rebounds and authored/left/right/alternating/random directions. Preserve legacy motion for existing designs.
- Prevent emission timing debt from producing catch-up barrages after the particle budget is full; show remaining queued objects.
- Document the reference-app audit and [workspace guide](docs/studio-workspace.md).

## 0.37.0-alpha.1 — 2026-09-27

- Bundle pinned MIT Purism Core 1.1.0 so compatible models load without downloading
  or selecting the proprietary Cubism Core. Ignore old runtime paths and remove
  the runtime setup gate. Keep worker isolation and advance its protocol to v4.
- Add Bouncy and Natural physics with a fixed 120 Hz simulation, time-based
  damping, bounded momentum, pause recovery and reusable scratch buffers. New
  avatars default to Bouncy; saved profiles retain Authored until the user opts in
  through Avatar → Physics and saves the settings.
- Include Purism's full MIT notice and add source-checksum and distribution guards
  to prevent accidental proprietary Core/model inclusion in release archives.
- Renderer limits and model/artwork permissions remain unchanged.
- Validate 11 local model renders and eight physics rigs across 57,600 stress
  frames, with 1,944 exact native/worker geometry matches. Complete 288 standard
  tests and 33 local native checks, including multi-avatar rendering, controllers,
  microphone lifecycle, layers, VTS import/undo and VRM/GLB secondary motion.
  See [validation](docs/validation.md) for test inputs, platform status and limits.
- Update the release overview, upgrade instructions, Linux package filenames and
  runtime/physics help. Retain the v0.36 Streamer.bot connector.

All downloads remain Alpha. See [Releases](https://github.com/NekoUnix/A.R.I.A/releases)
for packages and checksums. Features labeled Experimental have compatibility limits.

## 0.36.0-alpha.1 — 2026-09-16

- Add a dedicated Streamer.bot setup window with secret-free generated C# actions,
  a read-only connection test and a complete setup/troubleshooting guide. Expose
  every saved hotkey/node target through the authenticated API, including explicit
  avatar selection, layer/object/image toggles, effects and 3D gestures. Return
  execution results after scheduled workspace actions complete or fail, and reject
  cancelled or stale-session work.

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
