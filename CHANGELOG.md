# ARIA changelog

All downloads remain Alpha. See [Releases](https://github.com/NekoUnix/A.R.I.A/releases)
for packages and checksums. Features labeled Experimental have compatibility limits.

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
