# Studio reference audit

Work in progress, 2026-09-27. This audit separates observed UI, bundled documentation, source inspection, inference, and runtime testing. A documented control is not a verified integration. Reference programs and private models stay outside distributable ARIA files.

## Builds and methods

| Reference | Supplied build | Evidence |
|---|---|---|
| Stella Browser | ZIP 1.0.0, built-in guide dated 2026-09-23 | Executable extracted from installer without installing; UI and complete built-in guide; no proprietary source decompilation |
| VNyan | 1.7.2e | Portable ARIAResearch profile; UI; 331 bundled English help articles (10,922 words), public documentation |
| KarasuBonk | package.json 1.2.5 | Bundled readable JavaScript/HTML/default data; public MIT repository and release notes; native UI audit in progress |
| Streamer.bot | Supplied x64 ZIP 1.0.7; 148 entries | Isolated portable extraction, native UI and official event schemas; actual local server subscription accepted; no proprietary source decompilation |

The research uses the user's local avatar collection for local validation only. No reference artwork, installers, application code, or private avatars are included in ARIA.

## Streamer.bot: event automation audit

Archive SHA-256: `47A4E56F7A8A3E5F09EC16A5BA14C50E034DD98D67D2DC1EC89A671994C1F5C5`.
The provided distribution is a compiled .NET/WPF application with audio, WebView2,
Roslyn, storage and network dependencies. Dependency presence is architectural
evidence, not proof of internal implementation details. Accounts were not logged
in and no real channel rewards, messages or monetized events were generated.

Native pages observed: Home; Actions & Queues overview; Actions (separate action,
trigger and sub-action panes); Queues (pending/completed/paused/blocking columns);
Commands (location, global/user cooldown columns); Platforms overview;
Twitch overview/settings and Channel Point Rewards; Servers/Clients overview;
WebSocket Server; Integrations catalog. Authenticated platform screens, every
sub-action dialog, hidden settings and all integrations have **not** been fully
exercised. The inventory is explicitly incomplete rather than a claim of exhaustive
page-by-page acceptance.

| System | Strength observed or documented | Friction / boundary | ARIA implementation or follow-up |
|---|---|---|---|
| Actions and triggers | Reusable action definitions with multiple event sources | Large trigger/sub-action catalogs can overwhelm first-time setup | Common reactions in a task-focused Events workspace; graphs for sequences |
| Queues | Explicit blocking, pause and completion counts | Default queue was nonblocking; media completion needs deliberate waiting | Bounded ingress/expiry now; named blocking queues and completion history remain |
| Commands | Platform/location plus global and per-user cooldown concepts | Permissions and argument semantics add setup complexity | Existing opt-in commands and rule cooldowns; per-user restrictions remain |
| Twitch rewards | Cost, enabled, paused and ownership columns separate channel state | Reward catalog requires a connected broadcaster | Map reward titles in ARIA; native catalog management remains |
| Website account login | Broadcaster/bot roles and website-based login | Publisher registrations and platform scopes still matter | Reuse user's Streamer.bot website logins; ARIA direct chat OAuth stays separate |
| WebSocket transport | Explicit status, host, endpoint, authentication and connected clients | Documentation defaults differed: fresh 1.0.7 showed Auto Start off | Explicit Connect, actual subscription acknowledgement, password challenge support |
| Gift bundles | Twitch settings identify bundled gift-child behavior | Counting bundle and child notifications can double reactions | Normalize GiftBomb once and ignore GiftSub fromCommunitySubGift children |
| Integration catalog | Clearly separated third-party services with status markers | Installed integration is not a native platform guarantee | TikTok/X labeled adapter-only; no implied direct login |
| Audio actions (documented) | Play files/folders, named playback, stop controls and wait behavior | Completion and overlapping playback need intentional queue design | Local sound library, background decode, bounded voices, stop action; completion nodes remain |

Native compatibility: started only the isolated reference app's default loopback
server (`127.0.0.1:8080/`), left authentication and all account settings unchanged,
ran ARIA's actual Hello/Subscribe client successfully, then stopped the server.
Password authentication and failure cases were separately exercised against a
local protocol fixture. This confirms transport compatibility, not real-account
delivery or provider permissions.

Primary references: [core actions](https://docs.streamer.bot/guide/core/actions),
[blocking queues](https://docs.streamer.bot/faq/action-blocking),
[WebSocket configuration](https://docs.streamer.bot/api/websocket/guide/configuration),
[authentication](https://docs.streamer.bot/api/websocket/guide/authentication),
[platform support](https://docs.streamer.bot/faq/platform-support),
[TikFinity integration](https://tikfinity.zerody.one/streamerbot-integration),
and the provider event schemas linked in [ARIA's connector guide](streamerbot.md).

## Stella Browser: architecture and workflows

Observed: an Electron shell combines a browser with dedicated studio pages. Persistent left navigation, a central task surface, and right utilities maintain context. Stage introduces an additional source/library, canvas, inspector arrangement. Rounded dark surfaces, lavender selection and cyan status distinguish navigation and activity. These are general interaction patterns; ARIA implements its own native egui design.

| Page / system | Strength | Friction or constraint | ARIA response |
|---|---|---|---|
| Home | Quick access, notes, timer and resource status support preparation | Browser history, weather, account and news compete with production tasks | Task-focused Home, local notes and timer, model/tracking/output readiness links |
| Stage | Chat/Game/OP/ED modes; drag layers, lock/crop, preview; distinct send status | Deep tool collection and several sidebars can crowd preview | Persistent navigation, contextual inspector, resizable panel and Focus stage |
| Avatar Link | Source discovery, dim unavailable senders, explicit alpha/resolution/FPS, preview before adding | Source count and partially translated messages were inconsistent in inspected UI | Keep actual connection status visible; do not label a preview as a live stream |
| Voice Graph | Searchable effects, graph presets, separate stream and monitor paths, input/output meters | VST hosting and audio clocks are complex; several built-ins are paid | Reuse library/canvas/inspector concepts; do not add an untested audio host |
| BGM | Documented playlist groups, multi-select, repeat/seek and scene-aware ducking | File references need repair after migration | Existing file validation/reload remains; full BGM system is separate work |
| Overlay | Documented streamer-only HUD with capture exclusion and saved layout | Borderless/windowed constraints; capture exclusion is platform-specific | Keep session utilities outside composed avatar outputs |
| Collaboration | Guest video or fallback portrait; per-guest framing | Separate Discord audio route can duplicate sound; paid guest limits | Preserve ARIA's existing multi-avatar workspace; remote guest audio not implemented here |
| Smart Link | Documented temporary pairing and separate camera/tracking roles | Requires phone/network and Pro; not runtime verified | No claim of phone-browser pairing support |
| Settings migration | Documented preview of missing files; excludes credentials | Import replaces all settings and restarts | Preserve profile IDs and legacy design defaults; do not silently replace a workspace |
| Manual | Illustrated, task-ordered guidance; preflight and recovery instructions | Some development-build labels differ or remain Japanese | Add offline workspace help and a concrete migration guide |

Manual review additionally covered source persistence per scene, missing-file indicators, video/HTML interaction mode, comment text scaling, recording destination, temporary performance adjustments, TimeShield limitations, platform-specific send semantics, monitor split/popout, muted monitor behavior, Spout alpha recommendations, VST routing, VB-CABLE directions, browser import exclusions, theme/language, update choices, VR Deck, and troubleshooting. These features were documented, not all executed.

## VNyan: architecture and workflows

Observed: Unity application with a largely unobstructed render view, floating feature windows, a compact menu and a graph editor. Portable profile selection appears in the title. Initial mode selection distinguishes 3D avatars from the VTube Studio overlay. The avatar wizard is skippable; the graph palette is searchable and categorized. Windows expose little accessibility text, requiring screenshot inspection. At the tested desktop scaling the controls are small and several simultaneous panels overlap.

The bundled node documentation describes callbacks, actions, conditional routing, arrays/dictionaries, math and typed values. Named triggers can be shared across graphs; queued triggers specify spacing; cooldown drops excess signals while Wait delays only its branch. This is substantially more programmable than ARIA's current bounded action sequences. ARIA does not claim VNyan graph compatibility.

Useful lessons: starter recipes reduce the blank-canvas problem; explicit target assignment avoids surprising imported actions; graph run/stop status matters; named resources and completion callbacks are preferable to blind delays. ARIA already has graph validation, 16 concurrent-run limits, execution budgets, load completion waits and cancellation. This change adds timed and parallel starter layouts rather than duplicating those systems.

### Settings inspected so far

- Tracking/base: arm angle, root rotation, arm tracking during animation.
- Tracking effects: expressive body tilt rotation/tilt/lift; breathing enable, heart-rate link, speed/rotation/lift/scale; arm sway enable, axes, breathing/shoulder/arm/hand/finger multipliers and finger override.
- Webcam: MediaPipe mode/device, eye bones, eye blendshapes, head/body/arms/hands/blendshapes, mirror, ARKit conversion, linked blinks and threshold, movement axes, gaze, rotation/nod/tilt ranges, torso stiffness, locomotion, stationary hip, planted feet, hand height; adjustment import/export and eye calibration.
- Node graphs: add/delete, graph tabs, searchable categorized nodes, active switch, rename/load/export and canvas zoom.

### Documented capabilities and limitations

The full help inventory covers tracking layers and 32 VMC tracker slots; ARKit requirements; LeapMotion calibration; camera positions and transitions; bone/object transforms; expression priorities, constraints, smoothing and curves; tagged collisions; throwable force/target/lifetime/flinch; sticky bubbles; ragdoll reset; props; lighting and postprocessing; motion recording/playback; parameters and file I/O; Twitch/YouTube/Kick and other callbacks; Crowd Control; OSC/REST/WebSocket; OBS state and actions; VTube Studio overlay actions; VNyanNet RPC.

Notable documented pitfalls: tracking layers cannot be enabled by a node unless initialized at startup; node branch order needs an Ordered node when order matters; some object operations require unique Unity GameObject names; speech recognition is explicitly experimental and can pause on focus changes; audio device changes can break the documented volume-filter workflow. These are documentation findings, not reproduced defects in the supplied runtime.

## KarasuBonk: throw pipeline

The readable bundled implementation divides responsibilities between main.js (Electron lifecycle, settings, Twitch events, WebSocket bridge), renderer.js/index.html (configuration), and bonker.js/bonker.html (browser-source rendering and VTube Studio communication). Default data exposes 16 throw images, nine impact sounds, event mappings, custom throws, redemptions and commands. A throw has independent artwork, scale, impact strength and sound; custom presets override groups of behavior.

Strengths: dedicated test buttons; model calibration; reusable named throws; explicit directional choices; barrage spacing; per-event cooldowns; minimum/maximum donation/raid counts; impact decals; launch/impact sound selection; pixel-art rendering; portable browser-source integration.

Source findings in the supplied 1.2.5 bundle (not claims about newer versions):

- simulatePhysics adds horizontal velocity directly each timer callback and applies gravity using 1/FPS, so changing physics FPS changes displacement behavior.
- bonk's loading wait combines OR and AND without grouping all readiness/queue conditions. Queue delay is not independently required whenever assets are ready. Missing audio/decal readiness has no explicit timeout in that wait.
- Each bonk assigns the shared VTS socket's onmessage handler; overlapping requests can replace the earlier handler. Request IDs are reused.
- Physics gravity and simulation rate are globals shared by spawned objects; a later throw can change the environment for earlier throws.
- WebSocket parsing shown in createServer has no local JSON parse guard; server construction omits a host restriction. This is a source-level robustness review, not an exploit test.
- Per-model min/max calibration is clever but introduces manual setup and dependence on VTS placement, unlike ARIA's native model coordinates.

ARIA changes: optional momentum rebound uses the incoming arc tangent and an analytical linear-drag trajectory; old designs retain their old rebound. Side selection supports authored/left/right/alternating/random origins around each aim point. The existing fixed-step simulation now clears emission timing debt while saturated, and the panel reports remaining queued objects. Tests cover capacity recovery, side selection, impact continuity, speed scaling, drag, gravity and legacy deserialization.

## Public user needs

Public reports are anecdotal, dated and not a representative survey. Recurring themes worth testing are discoverability, tracking-status clarity, predictable expression interactions, settings portability and capture performance. Examples: [saving VNyan settings](https://www.reddit.com/r/vtubertech/comments/1cltp31/), [tracking setup confusion](https://www.reddit.com/r/vtubertech/comments/1rw2two/trouble_setting_up_vnyan/), [expression/blink interaction](https://www.reddit.com/r/vtubertech/comments/1vdsby1/issues_with_vnyan/), and [capture lag discussion](https://www.reddit.com/r/vtubertech/comments/1tt6nbj/bit_confused_on_how_to_fix_this/). These inform test cases, not comparative performance claims.

Primary references: [Stella](https://www.stella-vs.com/stella_browser/), [VNyan documentation](https://github.com/Suvidriel/VNyanDoc/wiki), [VNyan](https://suvidriel.itch.io/vnyan), [KarasuBonk source](https://github.com/typeou/karasubonk), [KarasuBonk releases](https://github.com/typeou/karasubonk/releases).

## Validation status

Initial workspace run: 294 standard tests passed; 35 tests requiring native/runtime fixtures were skipped by their existing gates. Workspace Clippy with all targets/features and warnings denied passed. Visual smoke review found and prompted correction of misencoded navigation labels. Further page coverage and native validation remain in progress; this document must not be presented as an exhaustive completed audit yet.

### Additional VNyan tracking coverage

Phone/ARKit exposes MeowFace, VTube Studio (recommended) and FaceMotion choices; IP/connection indicator; eye bones, eye blendshapes, linked blinks/threshold, mirror, simplified ARKit, planted feet, stationary hips, tracked blendshapes and idle-on-loss; movement/rotation/tilt/nod/locomotion/gaze and per-body-part application weights. Per-blendshape adjustment exposes a weight, editable response curve, copy/paste curve and adjustment import/export.

RhyLive has enable, receiver port and head/body/arms/hands/blendshape masks. LeapMotion has placement/orientation, device visualization, height/distance/lateral offset, hand separation, scale, XYZ rotation, hand-up/down speeds, mirroring and tracking mode. SteamVR has enable/IK targets/tracker visibility, mapping and calibration slots. Mapping assigns head/chest/hips/hands/elbows/feet/knees, tracking-space offsets/scale/floor calibration, mirror, body-part masks and smoothing. Calibration exposes head/arm/pelvis/feet adjustments, per-tracker XYZ offsets and leg swivel, with contextual help. Physical behavior remains untested without those devices.

Native rendering check passed: seven custom throw assets including PNG/GIF/3D and an independently hosted Live2D asset, using a model from the authorized local collection. This is rendering evidence, not proof of every external stream integration.

Further KarasuBonk source finding: renderer.js loadImage says it will preserve different same-name files, but its loop increments the suffix when contents are equal and breaks when they differ, before copying over the destination. The inspected branch therefore appears to invert the intended duplicate protection. ARIA references source artwork and does not copy it into a shared same-name gallery in this workflow. This finding is not a report that any user file was overwritten during research; no reference-app import was performed.

VNyan output/graphics/audio/integration pages inspected: Spout sender enable/name; NDI enable; virtual camera enable/mirror/FPS/background/install entry; VMC sender address/port. Graphics includes window size, anti-aliasing, SMAA, FPS cap, UI scale, theme/reload and hotkey help. Audio includes master/bonk/confetti volume, AudioLink input/gain/four band thresholds, and lip-sync device/gain/smoothing. Four VMC receivers expose independent ports, per-body-part masks, smoothing, blendshape tracking and clear-on-timeout, plus tracker mapping.

Connection pages inspected individually: Twitch emote dropping/allowlist and redeem creation entry; YouTube live-video ID; Kick username; Tiltify campaign; OBS address/port/password, connect-on-startup, reconnect and separate stream/record/virtual-camera indicators; Crowd Control authorization gate; VRChat OSC/virtual trackers; VTube Studio API port/overlay mode; Chaturbate event-token setup; Lovense local game-mode address/port; Fansly account name. Authentication was not performed. None of the device, account, stream or recording toggles was activated.

### Native UI coverage continuation

KarasuBonk native screens inspected: default image gallery and item detail (scale, pixel scaling, impact weight, volume, per-item sound); sound gallery; Custom list and Hydrate editor with every override, lower gallery overrides and impact-decal gallery; Events with redeems, commands/moderator-only, follow/sub/gift/charity, bits and raids including thresholds, maximum counts and cooldowns; Settings top and bottom including every throw, model, network and physics control; Test Bonks menu; Calibrate readiness. Calibration is explicitly gated on Twitch authentication in this supplied build; no login was performed. Help opens the external KBonk help page. These UI observations complement, rather than replace, the source analysis above.

VNyan successfully loaded the user-supplied NekoUnity2.vrm after the user completed the native file picker. The model rendered visibly; collider visualization was enabled and its head/torso/hand volumes displayed. Gesture editor inspected: avatar hand, record, five finger-angle thresholds, held milliseconds and import/export. Expression editor inspected: recording, blendshape min/max conditions, output Set Value/Multiply, value/multiplier, clamps and smoothing. Blank research-only gesture and expression entries were created to reveal the editors.

All nine Effects subpages inspected: Bloom (intensity, threshold, diffusion, HSV/color intensity); Color Grading (temperature, tint, hue, saturation, brightness, contrast); Ambient Occlusion (intensity, thickness, color); Lens Distortion (intensity, XY multipliers/center, scale); Chromatic Aberration (intensity); Grain (colored, intensity, size, luminance); Analog Glitch (scanline jitter, vertical jump, horizontal shake, color drift); Digital Glitch (intensity); Binary (dither type/scale, color, opacity). Every page has an explicit Active switch. Merely inspecting a control does not establish its visual behavior across all values.

Stella BGM page inspected in the running app: autosave, JSON import/export, named playlists, multi-track drag/reorder, local-track and music-pack subpages, file import and empty-state guidance. No copyrighted music was imported.

VNyan additional pages: Lights (directional/ambient HSV and hex, intensity, shadow bias, affect-world/shadows/contact-shadows, direction from camera, monitor-driven display lighting); Monitor (searchable live model blendshape values); Props (file, Spout2, text, browser, NDI sources; text prop linked bone, XYZ position/rotation/scale, face-camera, clone/show-hide); Pendulums (chain count, input source/name/multiplier/axis, bone/blendshape/negative-blendshape/parameter/object outputs, offset, preview, damping/elasticity/stiffness/inertia); Stretch Bones (anchor/target/stretch objects, visualizers, target position/rotation, stretch min/max, scale multipliers and live values; lower scrolled region needs further coverage); Stickers (file/clipboard, distance); Plugins (empty in supplied build); Help (about, shortcuts, capture, community/credits); Resources (search and item/prop/world/graph/plugin/tutorial/Unity-asset filters, populated external catalog); VNyanNet (server/lobby credentials, username, create/join, host controls positions, client list); Spout cameras (active, name, Spout2/NDI/both, resolution/focal length, main-camera link, layer masks, position/rotation, current-position capture). A research camera starts Active immediately when added; it was disabled after inspection. Resource links unexpectedly opened behind overlapping editor clicks; this observation suggests investigating click-through/modal input isolation, not a proven general defect.

Stella remaining primary pages were opened individually: BGM Music packs reports the missing Manager-chan integration; Overlay exposes six HUD elements, drag/resize/transparency, display/resolution, default-layout reset and Start; Collaboration exposes local avatar status, chroma colors, guest name, VDO URL/image fallback, transparency/fringe controls and Free/Pro limits; Smart Link is a Pro gate. Browser Bookmarks, Downloads, History and application Menu were inspected in the empty research profile. Import opens a separate panel explaining local profile reads and exclusion of cookies/passwords/sign-in; no personal browser data was imported.

Every Stella Settings category was opened: Performance (memory saver and exemptions, acceleration, video decode, GPU details); Privacy/security (time-range deletion, local-list protection and cache limitations, read-only); Data collection (usage/crash controls and disclosed schema fields, read-only); Autofill (empty vault landing only, no credential operations); Languages (UI language, ordered Accept-Language, translation prompt); Settings migration (secret-excluding export and explicit replace-all/restart import). The entire shared lower settings page was read and scrolled through: 24 appearance choices, background/panels/darkness, search/session/bookmarks/download destination, recording folder, Manager-chan executable, Veil/ad-guard disclosures, webcam-source visibility, browser migration, paid-feature list, version/update status, edition and VR Deck gates, and support form with optional redacted logs. No privacy/security setting was changed and no support message was submitted.

### Stage and audio detail checks

Stella's Generic Spout input discovered VNyanSpout while the supplied VRM was visible in VNyan. It reported Receiving at 1280×720, but the avatar preview remained blank. Auto alpha showed transparency; Opaque showed a black rectangle. Manually selecting Avatar Link in Stage reproduced the empty rectangle. This is an unsuccessful visual transfer despite a receiving status; the cause has not been isolated.

Stage inspector coverage includes Avatar input/opacity/position, Comments presets/text size/display/opacity, Background opening/ending video and layered assets, BGM playback/repeat/ducking, both paid effects gates, and all four Items tabs (images, slides, paid video and HTML). Stream quality separates adaptive preview/render/capture settings from active stream resolution/FPS/bitrate; its advanced game-performance guide was also expanded. Layer order includes visibility and locked room/content layers. Collaboration members shows an empty-state link to setup and independent Discord-audio volume/capture. VR Deck exposes its paid gate, wrist visibility, quality choices and three lost-panel recovery controls.

Choose what to show was expanded through Game, Smart Link, browser tabs, running apps and less-common sources. Capture-device setup separates video and audio, offers refresh, and disables adding an incomplete source. Full-display/browser/settings sources are grouped separately with an explanation. No personal application was selected for capture. Chat, Game, opening and ending modes were visited; missing opening/ending media explicitly produced a black-screen warning. Studio monitor offers a resizable split, full-page/video-only modes and popout, with a selected-broadcast prerequisite.

YouTube, Twitch and TwitCasting destination panels were each inspected, including manual connection forms. YouTube requires a selected scheduled stream; Twitch and TwitCasting display immediate-publication warnings and service-specific ingest/key fields. No credentials were entered or broadcast started. The Stage status still said YouTube monitoring standby when another destination was selected, an example of platform-specific status wording that can confuse users.

Voice Graph was inspected beyond its landing view: microphone identity retention on disconnect, plugin library and paid gates, signal connectors, separate monitoring gain, monitor-delay recovery, synchronization toggle, latency/CPU/dropout metrics and the full audio-clock diagnostics dialog. Clock diagnostics report drift, cumulative difference, jitter, confidence, samples and rate separately for each source and explicitly say they do not alter processing latency. Adding an unconnected VB-CABLE node revealed input/output device names and stopped/disconnected behavior; Voice Graph remained stopped.

VNyan's remaining lower Stretch Bones region was reached using the scrollbar track: movement amount/offset, axis selection, blendshape name, multiplier, inversion and live stretch/scale/movement values. Props were expanded separately for Browser, NDI and Spout2: each exposes a source field plus shared transforms and camera-facing behavior. No remote URL or sender was connected. Attachment choices include head, hands, upper chest, world, hips and 32 tracker slots. Blank research props were used to reveal these controls.

KarasuBonk's Bits image tab was inspected separately (1/100/1000/5000/10000 tiers, replacement artwork and individual scale), as was its Bits sound-selection tab. All four custom gallery categories were opened: images, impact sounds, impact decals and windup sounds. Impact-sound override was temporarily enabled to reveal the selector, then restored to inheritance. No artwork or sound files were imported.

The updated ARIA build passed another complete standard workspace run: 294 passed, zero failed, 35 gated tests ignored. A separate native mixed-workspace test passed with two supplied Live2D models, the supplied Neko VRM and a GIF: three output formats rendered all four avatars, switching reused the independent workers, and unloading one preserved the others. Landscape and portrait rendered images and the avatar-library UI were visually reviewed. The render-rate test now also compares actual momentum-rebound positions and alternating origins between 15 and 120 FPS; it passes. This does not establish end-to-end external broadcast readiness.
