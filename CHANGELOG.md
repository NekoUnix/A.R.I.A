# ARIA changelog

All downloads remain Alpha. See [Releases](https://github.com/NekoUnix/A.R.I.A/releases)
for packages and checksums. Features labeled Experimental have compatibility limits.

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
