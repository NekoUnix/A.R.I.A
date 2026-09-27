# Scenes, music and event rules (development)

These features are in the development branch, not the v0.37 package. They are
workspace settings: changing the avatar being edited does not replace a playlist,
scene library or event rule. Playback and event actions do not start automatically
from opening a music file or importing a model.

## Scenes

Arrange avatars in the landscape, portrait and freeform output windows. Open
**Scenes**, enter a name such as Chat, Gameplay, Opening or Ending, then **Capture
current layout**. **Go to scene** recalls all three layouts; **Update from current**
replaces the saved arrangement. Rename a scene without breaking its action ID.

A scene recalls positions, sizes, background colors and locks. It preserves live
output resolution, sender enablement and open/closed windows. A transition of zero
seconds cuts immediately; up to five seconds smoothly moves avatar transforms.
Background changes cut at the start. Interrupting a transition starts from the
current interpolated layout. Missing or unloaded avatars are not loaded by recall.
Video playback, crossfades and complete world/prop scene snapshots are not included.

Scenes appear in **Hotkeys & actions** and as action-node targets, so a timed graph
can recall a layout, wait and return. Deleted scene targets report a repairable
error rather than switching to an unrelated scene.

## Music

Open **Music & playlists**, choose **Add music files**, and select local WAV, MP3,
OGG or FLAC audio. Up to 32 playlists hold 256 paths each. Rename playlists, reorder
tracks or use **Repair** to replace a moved file. The current playing queue is a
snapshot: editing a playlist changes the next explicitly started queue.

Use Play/Pause, Stop, Next and the position slider. Repeat supports Off, Playlist
or Track. Shuffle avoids immediate repeats; with Repeat Off it stops after every
track in the current queue has played. A missing or unsupported track stops with
an error instead of silently skipping through the whole library. Long tracks are
decoded as a stream; opening a file runs off the UI thread. Stop invalidates an
unfinished open, so a late decoder result cannot restart playback.

**Lower music while speaking** uses live face tracking or an already enabled
microphone. It never enables a microphone. The control has a short attack, a hold
between syllables and a slower release. **Applied gain** shows the volume setting
after ducking; it is not a measured audio peak meter.

Music can use the system default or a saved output device, including an installed
virtual audio cable. Refresh the device list after connecting hardware. Changing
output stops playback and releases the previous endpoint. A missing saved output
reports an error instead of falling back to speakers. Capture that output in OBS.
Spout/Syphon canvases do not carry audio. If a device stops,
ARIA stops playback and displays a retry instruction. Play/Pause, Next and Stop are
also available as hotkey, action-graph and authenticated integration targets.

### Experimental VST3 music effect (Windows)

Expand **VST3 music effect**, choose a locally installed `.vst3` file or bundle,
and wait for inspection. Enable **Process music through VST3**, then press Play.
ARIA hosts one matching mono/stereo audio effect in a separate worker process.
Inspection and processing stay off the avatar UI and audio callback. Generic
normalized controls apply live; the plugin path, class ID and parameter values
are saved locally. Changing the selected plugin or enable switch stops playback.
Disable the effect and press Play to return to ordinary music playback.

Processing uses bounded 512-frame blocks and up to eight queued blocks. Missing
plugins, unsupported buses, invalid output, crashes and timeouts stop music with
an error; there is no automatic unprocessed fallback. Buffer starvation inserts
complete silent frames and displays an underrun count. Reported latency is drained
at track endings, followed by the reported tail capped at five seconds. A live
latency change requires restarting playback. Output samples are bounded to the
digital range; this is clipping protection, not a loudness-normalization service.

Seeking while processed, native plugin editors, opaque preset/state chunks,
instruments/MIDI, sidechains, multi-effect chains, microphone processing and a
shared routing graph remain unfinished. ARIA does not create virtual audio
devices or redistribute installed plugin binaries. Process separation contains
plugin crashes; it is not an operating-system security sandbox.

## Events and gestures

Open **Events & gestures**, add a rule, assign an action and enable the rule plus
the master switch. Stream rules match Command, Reward, Follow, Subscription, Gift,
Bits or Raid, optionally an exact name and a minimum amount. Each rule has its own
cooldown. **Simulate event** runs the same matching logic and can trigger every
matching enabled rule; it is a real action preview, not a dry run.

Gesture rules select the currently edited avatar and up to eight input conditions,
each with an at-least or at-most threshold. Choose All or Any conditions, then set
the hold time. The primary input remains compatible with older saved rules. A
held condition fires once and must release before firing again. A release margin
provides hysteresis around each threshold, preventing jitter from retriggering;
the margin uses the input's units. Missing/nonfinite inputs do not match. Disabling
a rule or editing its conditions discards a partially accumulated hold.
Tracking loss and demo input do not trigger gestures. This is a numeric condition
recognizer, not a finger-pose recording system.

Enable **Accept !commands from connected Twitch / YouTube chat** to feed new live
messages into Command rules. Match names such as `!bonk`; only the first token is
used and it must contain ASCII letters, digits or underscores after `!`. Arguments
never become action IDs or code. This opt-in applies to all viewers; moderator-only
command permissions are not yet implemented. Existing history and the first YouTube
poll are skipped. Each service queues at most 64 commands, drains at most eight
per frame, and discards entries older than five seconds. Moderation deletes remove
commands that have not dispatched yet; completed actions cannot be undone by deletion.

Other incoming stream events currently use the existing authenticated loopback
integration. There is no direct native Twitch EventSub connection yet; chat-read
authorization alone does not grant reward/subscription permissions. Explicit
configured mappings choose actions; event names are never interpreted as code or
arbitrary target IDs. Duplicate event IDs are retained in a bounded 1,024-entry
window, up to 128 rules are evaluated and at most 16 actions are dispatched per
event. The existing action scheduler applies its additional concurrency limits.

Example `action` payload inside a version-1 `/v1/commands` request:

```json
{"type":"stream_event","event":{"id":"unique-provider-event-id","kind":"Bits","name":"cheer","amount":100}}
```

Names and amounts should come from the integration's verified event, not an
untrusted chat message claiming to be a subscription. The recent-results list
reports matching and dispatch; individual action failures appear in Hotkeys &
actions. **Disable event and gesture actions** immediately prevents new dispatch.

## Variables and conditions in graphs

Variable nodes support Set, Add, Subtract, Multiply, Divide, Minimum and Maximum.
Read input snapshots a tracking value into a variable. Variables are private to
each run and do not leak across concurrent graphs. Values must stay finite within
±1e12; division by zero and missing inputs report errors. There are at most 128
variables per run.

A Condition compares a variable with a number. When false, it finishes the run
without executing remaining steps. Put a condition before the actions it guards;
already completed parallel actions are not undone.

Use **Branch** to choose a path without stopping the whole run. Connect two
outputs; the first connection becomes **True**, the other **False**. Select the
node to swap the True destination. Each card displays both destination IDs.
The comparison reads the current run's variable at that node. A skipped path
does not execute its actions or delays. Joins wait for every incoming path to
finish or be skipped, then run once if any incoming path is active. A node shared
with an independent active path still runs. Missing variables stop with an error.

**Repeat action** runs one selected action 1–100 times, with 0.05–300 seconds
between successful executions. The first execution is immediate. Later waits
start after each success, so a delayed frame does not release a catch-up burst.
Downstream nodes wait for the last repetition. **Stop all actions** cancels
remaining repetitions; actions already completed are not undone. Each repetition
uses the same captured avatar origin, and asynchronous avatar loads retain their
existing timeout handling. Runs retain the shared 32-step-per-frame budget.

The **Choose and repeat** recipe demonstrates a tracking input, a True/False
branch and a repeated action. Assign both action targets before running. Saved
and imported graphs retain the layout, comparisons and repeat counts; imported
action targets must be reassigned, including repeated actions. Whole-subgraph
loops, arbitrary scripts and VNyan graph imports remain unsupported.
