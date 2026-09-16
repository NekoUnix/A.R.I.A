# Streamer.bot → ARIA setup

This connector is included in v0.36 Alpha. It lets a
Streamer.bot command, channel-point reward, button, timer or other supported
trigger run ARIA actions. ARIA and Streamer.bot must run on the **same computer**.
The connection uses ARIA's authenticated local HTTP API; you do not need to enable
Streamer.bot's WebSocket server, open firewall ports or install a separate bridge.

```text
Chat / reward / event
        ↓
Streamer.bot action → ARIA connector → ARIA result ticket
        ↓                                     ↓
Saved avatar ID and action              Applied / rejected
```

## 1. Prepare ARIA

1. Open ARIA and load the avatars you want to control. Save their profiles.
2. Open **Settings → Streamer.bot → Set up Streamer.bot…**.
3. Under **Enable ARIA's local connection**, check **Enable API on this PC**.
4. Leave the port at **39421**, unless another program already uses it. ARIA will
   display a listener error if it cannot bind the port. Choose a free port and use
   the same number in Streamer.bot.
5. Click **Copy API key**. You will paste this into Streamer.bot in the next step.

This is the same listener as **Settings → Developer API**. Turning off either
switch disables the connection. **Rotate key** revokes the old key; update
Streamer.bot's saved key afterward. A running listener is not proof that
Streamer.bot is connected—the test action below checks the complete connection.

## 2. Save the connection details in Streamer.bot

1. Open Streamer.bot's **Global Variables** window from its toolbar.
2. Select the **Persisted Globals** category. Right-click and choose **Add Variable**.
3. Name it **`ariaApiKey`**, paste the copied key as its value, and leave **Auto Type**
   off so it stays text. Do not include quotes, spaces or `%` around the name/value.
4. Add **`ariaPort`** with value **`39421`**, or your chosen ARIA port. Text or an
   integer value works. These names are case-sensitive.

The globals are read by the connector; you do not need a Get Global Variable
sub-action. They persist across Streamer.bot restarts. Treat the key as a local
password. It is deliberately absent from the generated scripts. Do not share a
backup/export containing your global variables. If exposed, rotate it in ARIA.
See Streamer.bot's [global-variable guide](https://docs.streamer.bot/guide/core/variables).

## 3. Test the connection without changing the model

1. In ARIA's setup window click **Copy connection-test C#**.
2. In Streamer.bot's **Actions** area, create an action named **ARIA – Test connection**.
3. Add **Core → C# → Execute C# Code** (some versions label the category **C# Code**).
4. Replace the editor contents with the copied code. Compile it and save it.
   The connector uses `Newtonsoft.Json`, which Streamer.bot includes. If the
   compiler reports a missing namespace, use its reference lookup/Find References
   to add `Newtonsoft.Json` and the standard networking assemblies, then compile again.
5. Run the action manually. You should see **ARIA connection test passed** in the
   Streamer.bot log. Under **Action Queues → Action History**, inspect variables
   after the run: `ariaSuccess` is true and `ariaStatus` is `connected`.

You can instead paste [AriaConnector.cs](../templates/streamerbot/AriaConnector.cs)
for the generic connector. With no `ariaAction` argument it performs the same
read-only test. Streamer.bot documents [Execute C# Code](https://docs.streamer.bot/api/sub-actions/core/csharp/execute-csharp-code)
and its [included JSON library](https://docs.streamer.bot/api/csharp/guide/variables).
No Twitch/YouTube message is sent by the test.

## 4. Connect a real action

1. Return to ARIA's Streamer.bot setup window. Search the action list and select
   the avatar action you want. For example, select **Odette · Expression · Smile**.
2. For stateful actions, choose **Toggle**, **On** or **Off**. Prefer On/Off for
   rewards that should leave a known state. One-shot actions run once per trigger.
3. Click **Copy action C#**.
4. Create another Streamer.bot action, such as **ARIA – Smile**, add an
   **Execute C# Code** sub-action and paste the generated code. Compile and save it.
5. Test it manually, then add your desired Streamer.bot trigger: a command,
   channel-point reward, event, timer or button. Configure permissions and cooldowns
   in Streamer.bot. Do not add a trigger to the connection-test action by mistake.
6. Use a dedicated **sequential ARIA action queue**. Avoid concurrent execution
   of bursts of ARIA requests. The shared API allows 20 requests/second.

You can make as many Streamer.bot actions as needed using the same two globals.
A renamed avatar keeps its profile ID. Reimporting it as a new profile, removing a
saved action, or renaming a preset may require copying a fresh action from ARIA.
The generated code contains IDs and the chosen action, not your key or model files.

## What you can control

| ARIA action | How to select it | Toggle / On / Off |
| --- | --- | --- |
| Freeze/resume pose | Avatar's Freeze / resume pose entry | Yes; On freezes, Off resumes |
| Movement/appearance preset | Avatar's Preset entry | One-shot; preset name must still exist |
| Live2D expression | Avatar's Expression entry | Yes; resume a frozen pose to see movement |
| Saved layer group | Avatar's Layer group entry | Yes; On activates the group's saved effect |
| PNG/GIF/Live2D mounted object | Avatar's Object entry | Yes; controls visibility, preserving its mount |
| PNG/GIF artwork state | Avatar's Image entry | Yes; On holds it, Off releases that hold |
| Throws, sprays and other saved effects | Avatar's Effect entry | One-shot |
| Repaired/imported VTS action | Avatar's Imported action entry | One-shot; disabled or broken imports reject |
| VRM/GLB gesture/animation | Avatar's 3D animation entry | One-shot; starts the animation |
| Load/unload/edit/switch avatar | Saved profile's workspace entry | One-shot |
| Whole node graph | Action graph entry | One-shot; result waits for its nodes |
| Stop pending actions | Stop all pending actions | Cancels remaining nodes; already applied changes remain |
| Open/close output canvas | Output entry | Separate Open and Close entries |
| Release held parameters / save profile | Current avatar entries | One-shot |
| Set individual parameter values, output position/zoom, theme and legacy API commands | Advanced JSON, using the [API reference](api.md) | Depends on the command |

The list is the same catalogue used by ARIA's hotkeys and action nodes. It includes
loaded avatars even when you are editing another stage. **Profile-targeted actions
keep their saved target** when editing focus changes. Unloaded profiles offer
load/switch controls; load them to discover their per-avatar controls. To load an
avatar and then change an expression, create a graph with **Load avatar → Expression**
and trigger that graph. Profile switching replaces the stage that was selected
when the request was accepted; it does not remove other loaded avatars.

This is action control, not remote file management or a way to change every editor
setting. Import, repair, rig calibration and asset selection stay in ARIA. It does
not create a chat command/reward in your streaming account automatically.

## Reuse one connector or provide dynamic values

Instead of generating a script per action, use the generic template once and place
**Core → Arguments → Set Argument** before the C# sub-action. Name the argument
**`ariaAction`** and paste the JSON from **Copy action JSON**. The argument overrides
any generated default. An empty value requests a connection test.

Advanced example: set **`ariaAction`** to `{"type":"pose","frozen":true}` to freeze
the *currently edited avatar*. The connector fetches a fresh model generation
before submitting. Current-avatar API commands do not pin a saved profile; choose
a workspace action from the list if the target must be independent of editing focus.

For parameter values and output framing, follow [the request schema](../templates/api/command.schema.json).
Supply only the inner `action` object; the connector adds `version` and `generation`.
Use validated, creator-controlled values. Do not put arbitrary viewer chat text
into `ariaAction`. See Streamer.bot's [Set Argument documentation](https://docs.streamer.bot/api/sub-actions/core/arguments/set-argument).

## Results, delays and errors

The C# sub-action returns true only for a successful connection test or an
**applied** result ticket. Normally false stops the Streamer.bot action. If you
choose Streamer.bot's **Save Result to Variable**, handle false yourself before
running dependent sub-actions. The connector sets:

| Argument | Meaning |
| --- | --- |
| `ariaSuccess` | true only after confirmed success |
| `ariaStatus` | `connected`, `applied`, `rejected`, `pending`, or `error` |
| `ariaTicket` | Result ticket after a command was accepted, otherwise 0 |
| `ariaError` | Readable reason for failure; never contains the saved API key |
| `ariaState` | Latest state JSON, without the API key, useful for advanced scripts |

The default wait is 75 seconds. Set an **`ariaWaitSeconds`** argument to 1–600 for
longer graphs. A pending timeout does **not** cancel the command. Do not repeat a
throw/toggle after a timeout: it may already have executed. Poll its existing
ticket through the API or inspect ARIA. **Stop all pending actions** cancels
unfinished nodes, not running animation playback or already thrown objects.

A workspace-action ticket completes after its scheduler steps finish. For a graph,
this includes delays and avatar loading. It confirms configuration/trigger application,
not that a gesture animation has finished or all thrown objects have left the scene.
Asset/GPU/audio loading errors can still occur later and appear in ARIA diagnostics.
The older `run_action` API command retains its original "graph started" semantics.

| Problem | Fix |
| --- | --- |
| Connection refused / timeout | Open ARIA, enable the API, confirm the same port and same computer. Check ARIA's listener error. Do not configure Streamer.bot's WebSocket port as ariaPort. |
| Invalid/missing key | Copy the current 48-character key again into persisted `ariaApiKey`; leave Auto Type off. |
| HTTP 400 | Check the key and JSON. Update from v0.35 or earlier for `workspace_action` support. |
| HTTP 409 | The model changed before acceptance. Wait for loading and use a fresh action/state. The rejected request was not applied. |
| HTTP 429 | Reduce event frequency, add cooldowns and use one sequential ARIA queue. Other API clients share the limit. |
| HTTP 503 | State is initializing; wait for the avatar and test again. |
| Target missing | Load the avatar, repair its files or choose a current action in ARIA and copy it again. |
| Rejected imported action | Repair it in ARIA and enable it; resume Live2D if frozen. |
| Pending / unknown outcome | Inspect the existing ticket or ARIA. The connector never resends a POST automatically. |
| Lost ticket after restart/key rotation | Existing tickets cannot be recovered. Check avatar state before retrying. |
| Compiler error | Paste the complete C# file, check references, and use a current Streamer.bot version. |

For a persistent issue use **Diagnostics / export logs** in ARIA, include the
`ariaStatus`, `ariaTicket` and `ariaError` from Streamer.bot's action history, and
follow the GitHub/Discord support links. Remove API keys from any screenshots or
Streamer.bot backup you share. The connector does not log the Authorization header.

## Compatibility and validation

The generated script uses Streamer.bot's documented CPH argument/global methods
and its included Newtonsoft JSON library. It adds no Rust runtime dependency and
opens no new listener beyond ARIA's existing opt-in local API. Streamer.bot itself
is a separate application; platform support follows Streamer.bot's own requirements.
This connector does not support connecting from a second computer.

Automated checks cover ARIA routing, profile targeting, generation checks,
result cancellation, stale sessions and C# HTTP success/error handling using a
CPH test adapter. A native ARIA smoke check also loaded a Live2D avatar and verified
that workspace Pose On and Off commands reached applied results. That adapter is
not a live Streamer.bot/Twitch account test.
Test your own event/reward once before using it on stream.

