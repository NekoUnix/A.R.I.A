# Developer control API

![Developer API settings in the current Windows build](images/api-v23.png)

Enable **Settings → Developer API → Enable API on this PC** and click **Copy API key**.
Default base URL: `http://127.0.0.1:39421`. Every request requires
`Authorization: Bearer YOUR_KEY`. The listener is loopback-only and off by default.
Rotate the key to revoke access; changing port/key restarts the server and clears
its queue/results. Keys are stored with local app preferences and are not exposed
by status endpoints. Do not put them in repositories or public chat.

The API is HTTP/1.1 with JSON request/response bodies. Browser Origin headers are
rejected; desktop scripts and stream integrations are supported. There is no
arbitrary file loading, command execution, model import or remote network listener.

For Streamer.bot, use the dedicated [setup guide](streamerbot.md) and
[connector template](../templates/streamerbot/AriaConnector.cs). ARIA's setup window
can generate a script for any saved action without including your API key.

## Discover, queue, confirm

1. `GET /v1/capabilities` returns protocol/app version, actions and limits.
2. `GET /v1/state` returns `version`, `generation` and `state`. State includes native
   parameter IDs, ranges and current values; live inputs; pose/source/status;
   preset indices; expression IDs; output settings; available themes; `imported_actions` with IDs/names/enabled/native action details; and `usage` resource counters.
3. `POST /v1/commands` with the returned generation and an action queues a command.
4. A **202** response includes a `ticket`. Poll `GET /v1/commands/{ticket}` until
   status is `applied` or `rejected`. Queuing is not proof of execution.

```json
{"version":1,"generation":1,"action":{"type":"pose","frozen":true}}
```

The generation changes with the current avatar. Fetch a fresh state before a new
operation; **409** means the generation is stale or state is initializing. Queued
commands are checked again on the UI thread. State is refreshed at roughly 10 Hz
while enabled. An initial **503** means the first state is not available yet.

## Actions

| `type` | Other fields | Behavior |
| --- | --- | --- |
| `set_parameters` | `values`: object of ID → number | Atomically validates 1–64 native values against actual min/max, then holds them in partial override mode. |
| `release_parameters` | `ids`: array of IDs | Releases selected holds; `[]` releases all. Empty holds resume Live mode. |
| `pose` | `frozen`: boolean | Captures final current values or resumes Live mode. |
| `preset` | `index`: integer | Applies a currently saved preset. Indices come from state, and may change after editing the library. |
| `expression` | `id`: string, `enabled`: boolean | Enables/disables an expression belonging to the current avatar. Frozen mode must be resumed to see changes. |
| `output` | `index`, `open`, optional `zoom`, `position` | Indices 0 landscape, 1 portrait, 2 freeform. Zoom 0.25–3; position `[x,y]` each −1 to 1. All fields validate before mutation. |
| `theme` | `name`: string | Selects a built-in or saved custom theme by exact name. |
| `imported_action` | `id`: string from `state.imported_actions` | Runs an enabled, reviewed VTS import action inside ARIA. Experimental. Missing assets, repair-required actions and frozen poses return a rejected ticket. |
| `workspace_action` | `target`: object from `state.workspace_actions`, `mode`: `Toggle`, `On` or `Off` | Run an explicit profile action or graph; poll the ticket until its scheduler finishes. Development build after v0.35. |
| `save_profile` | none | Requests the normal per-avatar profile save. |

Requests reject unknown fields, unsupported action names and invalid values.
Parameter changes enter the same pose/mapping path as UI controls; no out-of-range
native writes are possible. Settings can persist during normal application autosave;
use release/resume to return to live movement after a script finishes.

## Existing effects integrations

`GET /v1/effects` still lists saved effect IDs. `POST /v1/effects/trigger` still accepts
`{"version":1,"id":7}` and returns 202 when queued. Responses now also include a
ticket, so integrations can check execution. It binds the effect to the current
avatar at receipt and rechecks that avatar before applying it.

## Limits and errors

- 20 requests/second, 32 queued commands, 4 KiB request bodies and 8 KiB headers.
- Only the last 128 results are retained; unknown/expired tickets return 404.
- 400: malformed request, missing/invalid key, browser Origin or unsupported framing.
- 404: unknown route, effect or result; 409: stale generation; 429: rate/queue limit.
- An accepted command can later report `rejected` with an `error` message. Retry only
  after checking current state; blindly repeating a trigger can duplicate effects.
- No WebSocket stream; poll state at 5 Hz or slower and budget command/result polls
  within the same 20-request limit. Tokens belong in headers, never query strings.

Start with the runnable [Python client](../templates/api/aria_client.py) or
[PowerShell example](../templates/api/control.ps1). The [request schema](../templates/api/command.schema.json)
documents the action contract. [Effects plugin examples](../templates/effects/plugins/README.md)
remain compatible.

## Resource snapshot (v0.26 source)

`state.usage` adds the bottom-bar counters. Original `cpu_percent`, `ram_bytes`
and `private_bytes` describe the desktop process. `managed_cpu_percent`,
`managed_ram_bytes` and `managed_private_bytes` include directly owned Cubism
and camera/setup workers. `managed_processes` / `readable_processes` describe
coverage. `io_read_bytes_per_second`, `io_write_bytes_per_second`, `handles`,
`system_ram_available_bytes`, `system_ram_total_bytes`, `frame_average_ms`,
`frame_p95_ms` and `slow_frames` provide the expanded details. GPU fields remain
`vram_bytes`, `vram_budget_bytes`, `shared_gpu_bytes`.

Values are numeric or `null` when unavailable (including first-sample rates).
OS counters refresh at 1 Hz even though API state refreshes at 10 Hz. I/O includes
pipes/network; RAM sums can count shared pages more than once. These are local
process counters, not whole-machine CPU or GPU utilization. See
[counter definitions](responsiveness.md#read-the-bottom-bar).

The performance graph update keeps this numeric API contract unchanged. The
rolling graph history and session low/high records are local UI state and are
not included in `state.usage`.

## Numeric tracking extensions

The UDP ARIA JSON tracking format now accepts an optional `parameters` map for
external numeric signals consumed by [VBridger equations](vbridger.md#external-input-example).
This is separate from the HTTP control API. Config import/editing is available
in the desktop UI; existing movement preset actions also restore imported settings.

## Experimental imported actions (v0.29)

Import and repair the avatar's `.vtube.json` in the app first. Fetch `/v1/state`
and choose an ID from `state.imported_actions`. Send:

```json
{"version":1,"generation":1,"action":{"type":"imported_action","id":"ID_FROM_STATE"}}
```

Use the current generation, then poll its result ticket. ARIA validates and applies
the local action before marking it applied. For a scene, applied means the scene
configuration was accepted; GPU/image loading may still report a later asset error
in Objects and the diagnostic report. There is no remote VTube Studio executor.
See the [repair guide](vtube-studio-import.md).

## Multi-avatar workspaces

Since v0.32 Alpha, `state.workspace` includes `editing_profile` and a `profiles`
array with IDs, names, loaded state, tracking followers and load errors. Existing
commands continue to act on the avatar being edited. Output position/zoom commands
move that avatar within the selected shared canvas. Selecting another stage
invalidates the previous model generation; fetch state again before submitting
commands. The development connector adds `state.workspace_actions`, including load/unload/focus/switch targets for saved profiles. Importing new files still happens in ARIA.

## Visual action graphs (v0.34 and later)

Author a graph in **Hotkeys & actions → Action nodes**. `state.action_graphs`
lists its ID, name, node count and whether it is running. Graph validation runs
when you submit a trigger. With the current generation:

```json
{"version":1,"generation":1,"action":{"type":"run_action","id":1}}
```

An applied ticket means the graph was accepted and started, not that delayed
steps or animations have finished. Inspect `state.action_graphs[].running` for
activity; node failures appear in ARIA and exported diagnostic logs. A graph
targets explicit profile IDs; switching the editor does not redirect those nodes.
Load/switch nodes can manage the saved profiles referenced by that authored graph.

```json
{"version":1,"generation":1,"action":{"type":"stop_actions"}}
```

Stopping cancels pending nodes. Already applied changes and launched effects remain.
Both commands use the existing authentication, generation check and ticket mechanism.
Read [Hotkeys and visual actions](actions.md) for graph semantics and limits.


## Explicit workspace actions (development build after v0.35)

Read `state.workspace_actions`: entries contain `name`, `target` and
`supports_on_off`. Copy the exact target rather than deriving it from a display name.

```json
{"version":1,"generation":7,"action":{"type":"workspace_action","target":{"Avatar":{"profile":42,"command":{"Item":3}}},"mode":"Off"}}
```

The profile ID keeps the target stable across editor focus changes. Generation is
checked at acceptance. Once accepted, loading/profile changes do not redirect the
saved target. Unavailable targets reject. `Toggle` also means execute a one-shot
operation; `On`/`Off` are allowed only when `supports_on_off` is true. Graph target
shape is `{"Graph":1}`. Use the same ticket endpoint as other commands.

Unlike legacy `run_action`, `workspace_action` stays queued until its scheduled
steps complete, fail, time out or are cancelled. Successful animation/effect steps
mean playback was initiated, not that it has finished. Errors include missing
assets/targets and cancelled pending work. Key/port/listener changes invalidate
outstanding receipts; old runs cannot write results into a new API session.
Limit: 16 concurrently running actions; model loading may wait up to 60 seconds.
Normal profile loading/asset diagnostics remain available in ARIA.
