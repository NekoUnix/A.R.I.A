# Reference feature implementation checklist

Development audit, 2026-09-27. This is a scope checklist, not a claim of complete
feature parity. The [system plan](studio-system-plan.md) defines remaining integration and acceptance work. See [research evidence](studio-research.md) for the inspected
builds and [release status](README.md#current-and-planned-features) for platform limits.

| Feature family | ARIA implementation | Remaining work or verification |
|---|---|---|
| Persistent studio navigation, tabs and inspector | New Studio shell and command search; six pages visually checked at minimum window size | Expanded keyboard/accessibility acceptance remains |
| Model-themed visual style | Neko Studio palette, gradient cards, depth highlights and original stage backdrop; minimum-size views checked | Further Stella visual fidelity/polish remains; custom palettes retained |
| Home readiness and quick tools | Avatar/tracking/output links, notes and elapsed-time timer | Verify notes persistence and timer controls interactively |
| Multiple avatars and layouts | Existing independent profiles, three output formats | Native four-avatar test passed with supplied collection |
| Transparent avatar output | Existing Spout/Syphon/Linux canvas | Platform/GPU acceptance remains separate from rendering tests |
| External Spout/NDI avatar input | Not implemented as an avatar source | Design receiver lifecycle, missing-sender state and actual-frame preview |
| Website chat authorization | Browser flows and new publisher-configured Connect UI | Publisher registration, consent and real-account acceptance |
| Chat display, appearance and popout | Existing native companion and per-service styling | Native emote images, badges and moderation/composition are not implemented |
| Native stream transmission and recording | Not implemented; ARIA supplies OBS canvases | Encoder, audio mix, credentials, service lifecycle and recording recovery |
| Chat/Game/opening/ending scenes | New named layout snapshots, movement transitions and hotkeys/action targets | Video playback, crossfades and complete world/prop snapshots remain |
| BGM playlists and audio ducking | New streaming player, 32 playlists, repeat/shuffle, seek, file repair, speech ducking and saved output-device routing; native route release passed with silent audio | Audible mix, additional endpoint/hot-unplug acceptance remain |
| VST audio graph and virtual-audio output | Experimental Windows single-effect music worker, normalized controls and existing output-device routing | Full routing graph, microphone/guest buses, native editors, opaque state, chains and long-session acceptance remain |
| Remote guest collaboration | Local multi-avatar support exists | Remote media, consent, latency, guest reconnect and audio routing |
| Streamer-only overlay HUD | Local editor metrics/utilities exist | Dedicated HUD positioning and capture-exclusion acceptance |
| VR wrist controls | Not implemented | SteamVR integration, positioning and device acceptance |
| Tracking and calibration | Phone JSON/VTS, iFacialMocap, MediaPipe, microphone and controllers | Hardware acceptance and additional protocols remain |
| Leap/SteamVR/VMC/RhyLive inputs | Experimental VMC head/facial receiver added with OSC bundle and local UDP tests | Full-body/finger VMC, Leap, SteamVR and RhyLive adapters and hardware acceptance remain |
| Expressions, mapping and response curves | Existing expression/mapping/appearance systems | Validate reference-inspired priorities and conflicts with more rigs |
| Custom gesture recognition | Avatar-scoped All/Any combinations of up to eight thresholds, hold duration, release hysteresis and cooldown | Recorded finger-pose gestures and hardware acceptance remain |
| Secondary physics | Bouncy/Natural Live2D physics plus new 3D styles for VRM/GLB springs | VRM0/1 synthetic tests pass at 15/30/60/120 FPS; all five supplied VRM files passed native checks; arbitrary rigs remain unverified |
| Custom pendulum and stretch-bone editors | Not equivalent to VNyan's arbitrary chains/outputs | General object bindings, chain graph, stretch mappings and preview |
| Props and attachments | Images/GIF/Live2D/3D assets and pins | Browser/NDI/Spout texture props and animated imported prop behavior |
| Postprocessing | Lighting exists | Reference bloom, grading, AO, distortion and glitch effects are not all implemented |
| Logic graphs | Bounded graphs, recipes, per-run variables/math, input reads, stop conditions, True/False branching and timed action repetition | Whole-subgraph loops and VNyan's full callback/node library remain |
| Stream events | Reactions/Connections/Sounds workspace, platform filters, learn-from-event rule creation, cooldowns and deduplication; local Streamer.bot Twitch/YouTube subscriptions with challenge authentication | Direct provider EventSub clients, publisher registration, live-account acceptance, native reward creation and TikTok/X connections remain |
| Sound reaction library | Saved local clips, volume/preview/repair, sound and stop-sound targets for rules/hotkeys/graphs; bounded background decoding | Shared audio buses, selectable sound output, per-clip completion nodes and audible acceptance remain |
| Throws and projectiles | Existing designer/assets/sounds/deformation; new side modes and momentum rebound | Additional event presets and visual tuning; no blanket superiority claim |
| Throw queue reliability | Capacity bound, cadence recovery and queue count | Expanded long-session/asset-failure acceptance |
| Throw frame-rate independence | Fixed-step emissions and analytical rebound | 15/120 FPS position comparison passed |
| Per-item sound, windup and impact choices | Existing effect design support | Audit all designer controls against source-app presets |
| Plugins/resource catalog | No VNyan plugin compatibility | Define supported extension boundaries before loading third-party code |
| Multiplayer avatar state | Not implemented | Transport, ownership, bounded state and reconnect design |
| Settings portability and diagnostics | Existing profiles, presets, theme exports and support reports | Unified workspace backup/missing-file repair experience |
| Full browser, bookmarks, history, downloads | Windows WebView2 window, tabs, bookmarks/history, zoom and prompted downloads; navigation, Back, new-window tabs, bookmark restart persistence and actual fixture download checked | Download cancel/progress/resume, long-session/high-DPI variants, browser-texture output, extensions and non-Windows support remain |

Reference program installers, proprietary code and artwork are not copied into
ARIA. Private model renders remain local test evidence. An unavailable provider,
paid feature or device is recorded as unverified, never as a passing test.
