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

Host VST3 effects in a separate worker process, with plugin discovery and probing
outside the render/audio callbacks. Persist class IDs, plugin state and explicit
bus/channel layouts. Reject unsupported instruments or layouts with a repairable
message. Add bypass, gain staging and latency accounting before connecting an
effect chain to live output. A worker timeout/crash must release the plugin and
leave a visible bypass/failure state; it must not hang the avatar renderer.

Use a pinned current SDK and preserve its notices. Steinberg's current
[VST3 SDK](https://github.com/steinbergmedia/vst3sdk) and
[file-specific licensing guidance](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/Which%2Bfiles%2Bfall%2Bunder%2Bwhich%2Blicense.html)
are the source of truth for an eventual host integration. Do not silently pull in
an old differently licensed SDK or redistribute users' commercial plugins.

Acceptance: mono/stereo and sample-rate changes; silent-input stability; impulse
latency; clipping protection; state restore; missing/bad plugins; worker crash;
device unplug/replug; one-hour playback; two installed effects. Native VST3
processing is not implemented by the current output-device selector.

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

Extend graphs with explicitly labelled branches and bounded repetition; test joins,
cancel/restart, delays and concurrent variables. Add native provider event clients
only after publisher OAuth registration, scopes and reconnect/replay handling are
verified. Then expand chat badges/emotes, moderation and composition.

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
