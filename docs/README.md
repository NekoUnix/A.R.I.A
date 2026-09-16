# ARIA documentation

![Odette in the Live2D appearance workspace](images/customization-v30.png)

Next release in development: [recorded hotkeys and visual actions](actions.md),
clear stage tabs, shared stage/profile names, individual preview locks and
[Experimental VRC / GLB humanoid avatars](glb-avatars.md) using the existing tracking pipeline.

New to ARIA? Start with the [beginner setup walkthrough](../README.md#first-time-setup)
and [changelog](../CHANGELOG.md). For system-specific steps use [Windows setup](windows.md), [Linux installation](linux.md) or
[macOS package setup](platforms.md#install-an-alpha-release), import your avatar, then follow
[personal tracking setup](tracking-setup.md). The left navigation is **Avatar /
Tracking / Output / Chat / Settings**. Inspector tools stay grouped by tracking,
avatar, stage and poses. Hover or click any circled **?** for offline explanations.

v0.30 adds [drag layer selection and customizable Live2D controls](live2d-customization.md),
with model-owned appearance looks, tinting and shortcuts.

v0.29 added [Experimental VTube Studio import and repair](vtube-studio-import.md)
and [avatar-specific support reports](diagnostics.md).

v0.28 added [iFacialMocap phone tracking](ifacialmocap.md), with direct live UDP,
separate per-avatar connection settings and guided setup. The README puts user
instructions first; [technical references](../README.md#technical-guides-and-development)
are collected below them.

v0.27 Alpha adds **experimental** [VBridger config import and editing](vbridger.md), with
per-avatar equations, curves, modifiers and portable exports.

v0.27 also includes [colorful performance graphs](responsiveness.md#read-the-bottom-bar)
with hover readings and session low/high records. Read the [v0.27 release notes](release-v27.md).

## Current and planned features

**Working** means implemented with the validation described below, primarily on
Windows. **Partial / experimental** identifies an implementation with known limits
or missing real-device validation. **Planned** means it is not available yet.
This is a release status grid, not a promise of universal model/device compatibility.

| Feature | Status | What works / what remains |
| --- | --- | --- |
| Built-in demo avatar | Working | Starts without extra model files or SDKs. |
| VRC / GLB avatars | Experimental · v0.34 | Skinned GLB, editable humanoid mapping, named facial shapes, shared tracking, profiles, pinning and output. ICHIGO Windows GPU validation passed. Unity controllers/shaders and VRChat PhysBones are not imported. [Guide](glb-avatars.md). |
| iFacialMocap live UDP input | Implemented in v0.28 Alpha | Legacy/v2 parsing, head/eye/blendshape data, scoped receiver and stale recovery; physical phone acceptance remains unverified. No TCP/Bluetooth/recording transfer. |
| Live2D model3/moc3 imports and nested folders | Working with limits | Standard ArtMeshes, authored mappings, full-body framing and exported textures; advanced Cubism 5.3 offscreen rendering is unsupported. |
| Separate Cubism runtime process | Implemented | Native Core worker, private binary pipes, bounded messages and failure timeouts; use the library matching the OS. |
| PNG/GIF avatars | Working | Guided import, talking/blinking states, transitions and bounded GIF playback; large sources may be downscaled to the playback budget. |
| Secondary bone physics | Experimental for generated chains | Medium defaults, per-avatar/group tuning, authored VRM colliders, manual roots. [Guide](secondary-motion.md). |
| Avatar lighting | Working with limits | Per-avatar color/direction or even light. 2D uses a gradient; 3D uses normals. [Guide](lighting.md). |
| VRM 0.x / 1.0 | Working with limits | Native GPU rendering, supported expressions/skinning/springs and pins; this is not a complete reference VRM/MToon renderer. |
| iPhone VTube Studio / external JSON | Partial validation | Protocol, axes, loopback packets and calibration tested; physical phone/network acceptance remains. |
| VTube Studio customization import | **Experimental in v0.29 Alpha** | Tracking, physics, expressions/hotkeys, motion/part opacity, mesh colors and local item scenes; missing or incompatible actions are repaired inside ARIA. No VTS execution fallback. |
| Avatar-specific support report | Implemented in v0.29 Alpha | Bounded local errors and rig metadata, readable JSON/text export, prior-session recovery and GitHub/Discord ticket links; no automatic upload. |
| VBridger import and editor | Experimental in v0.27 Alpha | Per-avatar equations, weighted curves, delays, smoothing, steps and exports; exact VBridger behavior is not guaranteed. |
| Performance graphs | Working in v0.27 Alpha | Hover samples and session extrema with bounded history; OS counters depend on platform support. |
| Responsive mouth input / resource details | Implemented in v0.26 Alpha | Single speech filter, preset persistence, owned-process counters and frame/packet timing; physical phone latency remains unmeasured. |
| Webcam MediaPipe tracking | Partial validation | Clean/repair installation and real image inference tested; physical camera tracking remains to be checked. |
| NVIDIA RTX webcam tracking | Experimental | Adapter/setup code exists; NVIDIA SDK bridge build and real GPU inference remain unverified. |
| Microphone talking controls | Implemented | Audio levels drive image/model controls; device-specific acceptance remains. |
| Xbox / PlayStation / Switch controller inputs | Partial validation | SDL mappings and virtual-controller tests; physical controllers/adapters still need coverage. |
| Per-avatar mapping, ranges, poses and presets | Working | Individual guided exercises, illustrated tracking face, selectable takes, calibration, inversion, holds/freeze, profiles and Windows hotkeys. |
| Live2D expressions and physics groups | Working with limits | exp3 blending and per-model/group settings; only the documented Cubism subset is supported. |
| Live2D layer selection and visibility groups | Updated in v0.30 Alpha | Stage marquee, list ranges, reversible hiding/tinting, named groups, presets and Windows hotkeys; picks mesh geometry rather than texture pixels. |
| Customizable Live2D workspace | Implemented in v0.30 Alpha | Exported parameter folders, curated sliders/toggles/choices, appearance locks and looks; only artwork exported by the creator is available. |
| VRM idle motion and gestures | Implemented in v0.26 Alpha | Configurable sway, breathing and arms; six procedural gestures, blending and frozen poses. |
| Streamer.bot connector | Development build | Generated actions, all saved action targets, result tickets and [setup guide](streamerbot.md). |
| Pinned PNG/GIF and Live2D objects | Working | Two-model visual mounts, independent anchor editing and tracking on both models; up to four attached Live2D instances. [Mounting guide](in-app-help.md#model-mount--mount-two-live2d-models--choose-a-point-on-each-model-to-join-them-while-both-receive-tracking). |
| Custom throw assets | Updated | Replace/add controls, exact quantities, background PNG/GIF loading, file-specific errors and save validation; large 3D/Live2D props still have cold-load costs. |
| 3D throw assets | Working with limits | Static GLB/glTF, VRM, FBX and OBJ; native .blend/.max/.ma project files must be exported first. |
| Throws, sounds, bounce/stick and visual dents | Working with limits | Saved visual designs, routes, lifetime and 2D impact deformation; not full soft-body simulation. |
| Anime liquid sprays | Working with limits | Stylized configurable droplets, gloss, foam, trails and drips; not a physically accurate fluid simulation. |
| OBS 16:9 / 9:16 / Freeform outputs | Alpha native outputs | Windows Spout2, macOS Syphon and Linux ARIA Canvas; full resolution, alpha and small previews. Linux uses asynchronous readback; native OBS acceptance varies by GPU. |
| Twitch / YouTube chat | Partial validation | OAuth/chat/style implementation and local tests; real account/provider acceptance remains. |
| Themes and contextual documentation | Working | Six palettes, custom colors, import/export and offline help with diagrams. |
| Authenticated local control API | Working | Native model values, presets, expressions, outputs and effects verified; arbitrary file/program execution is not exposed. |
| Linux/macOS desktop builds | Experimental | Native build/test workflow, Vulkan/Metal and native Core selection; graphical and SDK execution on those machines needs acceptance testing. |
| Linux/macOS global hotkeys and account key storage | Planned | Platform integrations still needed; Windows-specific controls are not cross-platform guarantees. |
| Windows DLL execution under Wine on Linux/macOS | Not implemented | Use the official native Core library. The runtime process is not a Windows emulator. |
| Native distribution packages | Alpha | Windows ZIP, macOS app ZIPs, Linux tarball, Fedora RPM and Arch packages; matching OBS plugin included. |
| Developer signing / macOS notarization | Planned | Alpha builds are unsigned or locally ad-hoc signed; no Developer ID notarization. |
| Full Cubism advanced rendering / full VRM shader parity | Planned | Unsupported features remain documented; model coverage will expand through reproducible compatibility reports. |

See [validation evidence](validation.md), [native platform setup](platforms.md),
and [compatibility reports](https://github.com/NekoUnix/A.R.I.A/issues/new/choose).

## Find a guide

| I want to… | Open this guide |
| --- | --- |
| Install on Ubuntu, Fedora or Arch, including the OBS plugin | [Linux installation](linux.md) |
| Install on an Intel or Apple Silicon Mac | [Native packages](platforms.md#install-an-alpha-release) |
| Use a webcam or NVIDIA RTX facial inference | [Webcam tracking](webcam.md) |
| Connect iPhone VTube Studio or a tracking tool | [Tracking protocol](tracking.md) |
| Connect iFacialMocap on my iPhone | [iFacialMocap setup and troubleshooting](ifacialmocap.md) |
| Import or customize VBridger equations | [VBridger config editor](vbridger.md) |
| Match tracking to my face and rig | [Personal calibration](tracking-setup.md) |
| Use a gamepad with my avatar | [Controllers](controllers.md) |
| Tune ranges, hold parameters, create presets | [Input controls](input-controls.md) |
| Load exported Live2D files | [Live2D](live2d.md) |
| Configure outfits, hair and appearance looks | [Customizable Live2D models](live2d-customization.md) |
| Hide Live2D layers and save groups with hotkeys | [Layer visibility](live2d-layers.md) |
| Import and tune a 3D VRM | [VRM](vrm.md) |
| Choose from nested model folders or pin items | [Folders and pins](model-folders-and-pins.md) |
| Configure secondary motion | [Physics](physics.md) |
| Use expressions and hotkeys | [Expressions](expressions.md) |
| Record shortcuts and connect avatar actions (development) | [Hotkeys and action nodes](actions.md) |
| Capture landscape, portrait or Freeform in OBS | [Outputs](obs-output.md) |
| Sign into Twitch / YouTube chat | [Streaming chat](streaming-chat.md) |
| Change colors or share a custom theme | [Themes](themes.md) |
| Control ARIA with code | [API](api.md) |
| Make throws, sprays and sound effects | [Effect templates](../templates/effects/README.md) |
| Animate talking PNG/GIF states | [Image templates](../templates/images/README.md) |
| Read every contextual explanation | [In-app help reference](in-app-help.md) |
| Understand code or verified limitations | [Architecture](architecture.md), [validation](validation.md) |
| Contribute code or review a pull request | [Contribution guide](../CONTRIBUTING.md), [development workflow](development.md) |

The tracking screenshot shows v0.28 Alpha with a labeled local iFacialMocap simulator.
Other guides retain earlier images
of controls that remain available, using the owner's supplied avatars.
They demonstrate UI layout; camera configuration screens do not imply an active
camera connection. [Artwork provenance and reproduction notes](images/README.md).

## Avatar customization included in v0.26 Alpha

- [Live2D customization](live2d-customization.md): authored controls, categories, locks and appearance-only looks.
- [Live2D layers](live2d-layers.md): reversible hiding/transparency, model-owned groups and user-defined hotkeys.
- [Stage anchors](model-folders-and-pins.md): move a pin independently of its object; new Live2D attachments receive tracking automatically.
- [VRM motion](vrm.md#natural-movement-and-gesture-animations): configurable sway, breathing and arms, plus six clickable gestures and screenshot freezing.

These features remain included in v0.27.0-alpha.1. Download the matching package from
[Releases](https://github.com/NekoUnix/A.R.I.A/releases/tag/v0.27.0-alpha.1).
