# Remaining Studio systems

This is an implementation plan, not a list of shipped features. The
[feature matrix](studio-feature-matrix.md) records implemented behavior and
verification separately. Work stays in development until its acceptance checks
pass; installing a reference application is not evidence of feature parity.

## Audio and VST3

Music already has streaming decoding, playlists, ducking and explicit output
selection. The next layer is a shared audio bus for music, microphone monitoring,
effects and guest audio, with independent monitoring and broadcast destinations.
Build bounded queues, resampling and measured peak/RMS meters before a patch editor.
Changing endpoints must stop/reconnect deliberately without unexpectedly routing
private monitoring to speakers or to the broadcast bus.

The first VST3 slice now runs one Windows music effect in a separate worker with
bounded queues, probe/processing timeouts, explicit mono/stereo negotiation,
normalized parameter persistence and visible failure. See the
[music guide](studio-production.md#experimental-vst3-music-effect-windows).
Extend this with opaque plugin state, native editors and explicit chain/bus
configuration. Add measured gain staging and cross-bus latency compensation before
connecting microphone/guest inputs to a live effect graph. The existing worker
stops music on failure; it does not silently bypass the selected effect.

Use a pinned current SDK and preserve its notices. Steinberg's current
[VST3 SDK](https://github.com/steinbergmedia/vst3sdk) and
[file-specific licensing guidance](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/Which%2Bfiles%2Bfall%2Bunder%2Bwhich%2Blicense.html)
remain the source of truth. The initial host pins MIT `vst3-host` 0.9.0 with
`vst3` 0.3.0 bindings; dependency notices accompany packages. Do not silently pull in
an old differently licensed SDK or redistribute users' commercial plugins.

Acceptance: mono/stereo and sample-rate changes; silent-input stability; impulse
latency; clipping protection; state restore; missing/bad plugins; worker crash;
device unplug/replug; one-hour playback; two installed effects. Native checks cover
LoudMax and Elgato EQ, mono/stereo and 44.1/48/96 kHz, impulse latency, parameter
restoration, silent settling, missing files, cancellation and worker termination.
Buffered playback has a short headless acceptance test. One-hour playback,
audible quality, opaque state and device-unplug tests with effects remain open.

## Remote guests

Implement invitations with explicit host admission, separate guest mute and
monitor controls, reconnect and removal. A guest must never acquire local action
API access simply by joining. Keep guest media distinct from avatar tracking
state so video-only guests do not require an imported model. Show frozen/absent
media clearly rather than displaying stale frames indefinitely.

Use WebRTC media with authenticated signaling and a deployment-owned TURN service
for networks where direct peers cannot connect. These are distinct components,
as described in the official [peer-connection guide](https://webrtc.org/getting-started/peer-connections).
A browser window alone is not remote-guest support. Join URLs should be usable
without a guest installing ARIA or creating API credentials. Service deployment
and operating costs must be decided before advertising an Internet-ready relay.

Acceptance: two independent clients, both directions of audio/video, host-only
admission/removal, expired invitations, NAT/TURN-only path, reconnect, packet loss,
bounded queues, A/V sync, and capture into each output canvas.

## Browser and media sources

Finish native browser navigation, focus, downloads and persistence acceptance.
Then add download progress/cancel/resume and a source-specific lifecycle for
browser textures. Preserve separate editing and capture controls; browser UI and
account dialogs must not become part of the stream texture by accident.

Spout/NDI receivers require sender discovery, dimensions/format changes,
disconnect/reconnect and independently owned textures. Scene video requires a
decoder clock, seek/loop controls and audio routing, then crossfades and complete
scene snapshots covering media, props and loaded actors.

Acceptance: resize, high DPI, hidden/visible tabs, 12-tab cap, download cancellation,
missing WebView2, stale sender frames, resolution change, repeated source removal,
and bounded GPU memory over a long session.

## Avatar, effects and automation

Extend VMC from head/face to explicit humanoid and finger retargeting. Treat
SteamVR, Leap and RhyLive as separate adapters with their own calibration and
disconnect tests. Composite gesture conditions precede recorded finger-pose
recognition. Add arbitrary spring/stretch chain editing only with bone validation,
collision/bend constraints and a live reset control.

Implement postprocessing as an output pass with alpha preservation and a bypass;
start with grading/bloom before distortion/glitch and scene-depth-dependent AO.
Compare premultiplied-alpha edges in OBS, not only an opaque preview.

Graphs now include labelled True/False paths and bounded timed repetition of one
action, with tests for inactive paths, joins, cancellation and independent runs.
Whole-subgraph repetition still needs an explicit iteration boundary and join
semantics; arbitrary cycles remain rejected. Add native provider event clients
only after publisher OAuth registration, scopes and reconnect/replay handling are
verified. Then expand chat badges/emotes, moderation and composition.

The Studio event workspace now subscribes to local Streamer.bot Twitch/YouTube
events and creates disabled reaction rules from received examples. Sound targets
use background decoding and a bounded worker. Quick-start templates, disabled
copies, session dispatch limits and skip-reason history are implemented. Continue with per-user permissions
and cooldowns, explicit named blocking queues, completion/error history, reward
catalog management and fulfillment/refund receipts. A received event is not proof
that an action played successfully. Keep duplicate suppression and gift-bundle
normalization across all event adapters. TikTok/X currently require external
adapters; they do not have native account connections in ARIA.

Acceptance: supplied Live2D and all five VRM fixtures, low/high frame rates, tracking
loss, action cancellation, burst events, absent assets and profile migration.

## Portability and release

Add unified backup/restore with a manifest, missing-file repair and a preview of
changes. Keep credentials, browser cookies and private model artwork out of
portable exports by default. Define plugin boundaries before a plugin catalog;
do not imply VNyan plugin compatibility.

Remaining platform/hardware gates are explicit in the feature matrix. A release
needs matching documentation, clean lint/tests, native acceptance for changed
systems, distribution checks and CI artifacts. No unverified feature is described
as complete merely because its controls exist.
