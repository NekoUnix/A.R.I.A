# Trigger ARIA from stream events, buttons and plugins

![Current API controls](../../../docs/images/api-v23.png)

1. In ARIA load the intended avatar, create/select an effect, and note its ID.
2. Expand **Throws & liquid sprays → Stream events & plugins** and enable the
   local API. Default port: **39421**. Click **Copy API key**.
3. Configure your tool on the same PC with this key and port. No firewall rule is
   needed for loopback. The API cannot open files, run programs or import designs.

## Streamer.bot and Twitch

Create an action named for the effect. Add **Set Argument** sub-actions for
`ariaKey` (the copied key), `ariaPort` (`39421`) and `ariaEffectId` (the number from
ARIA). Add an **Execute C# Code** sub-action and paste `StreamerBot.cs`, then compile.
Run the action manually once to verify the effect before attaching an event.

Connect Twitch in Streamer.bot's platform settings. Add a trigger to that action
for a channel-point redemption, cheer, subscription, chat command, timer or other
event you use. Apply event filters and cooldowns in Streamer.bot, plus ARIA's own
effect cooldown if needed. You can make many actions, each with a different saved
effect ID. Use a dedicated action queue for network-trigger actions if you do not
want a three-second timeout to delay unrelated actions. Other supported streaming
services can use the same action through their Streamer.bot triggers.

Streamer.bot owns authorization and event handling; ARIA has no direct Twitch
OAuth connection. Keep the API key in local arguments/settings and remove it from
public action exports. Streamer.bot versions may place these menus differently.
API reference: [Streamer.bot TryGetArg](https://docs.streamer.bot/api/csharp/methods/core/arguments/try-get-arg).

## Touch Portal and other HTTP tools

Create an HTTP request action with these values:

| Setting | Value |
| --- | --- |
| Method | POST |
| URL | `http://127.0.0.1:39421/v1/effects/trigger` |
| Authorization header | `Bearer YOUR_COPIED_KEY` |
| Content-Type header | `application/json` |
| Body | `{"version":1,"id":1}` (replace the ID) |

Choose a native HTTP action or an installed HTTP-request plugin appropriate to
your tool. Browser JavaScript is not supported: requests containing an Origin
header are rejected. Desktop HTTP clients need no CORS handling.

## PowerShell client

Read the key interactively rather than storing it in your shell history:

```powershell
$ariaKey = Read-Host 'Paste your ARIA API key'
.\Trigger-AriaEffect.ps1 -ApiKey $ariaKey -List
.\Trigger-AriaEffect.ps1 -ApiKey $ariaKey -EffectId 1
```

The supplied script uses `Invoke-RestMethod` and has a three-second timeout.
The port can be changed using `-Port 39422`. Changing the key or port in ARIA
requires updating your tools. Do not use the API port as an iPhone UDP tracking port.

## HTTP contract, version 1

Every request needs `Authorization: Bearer KEY`. `GET /v1/effects` returns
`{"version":1,"effects":[{"id":1,"name":"Star toss","kind":"Throw"}]}`.
`POST /v1/effects/trigger` accepts only `version` and `id`, not arbitrary assets.

| HTTP status | Meaning |
| --- | --- |
| 200 | Catalogue returned |
| 202 | Command queued; ARIA still checks cooldowns and asset/resource availability |
| 400 | Invalid request, absent/invalid key, browser Origin, body/header limit |
| 404 | Unknown endpoint, ID or protocol version |
| 429 | Rate limit (20 requests/second), or 32-command queue full |

The HTTP response is not a final playback acknowledgement. Inspect ARIA's effect
status for later failures. Loading another avatar discards queued commands for the
previous profile. Query the catalogue after changing avatars: IDs are local to a
profile and ID 1 can represent a different design there. Disable the API to stop
accepting events. Header limit: 8 KiB; body limit: 4 KiB; no chunked transfer.

For all actions, profile targeting and execution results, use the [full Streamer.bot connector](../../streamerbot/README.md). The older effect-only example remains compatible.
