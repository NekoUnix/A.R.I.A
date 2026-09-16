# Streamer.bot connector

Open [the complete setup guide](../../docs/streamerbot.md).

`AriaConnector.cs` is a generic Execute C# Code sub-action. Set persisted globals
`ariaApiKey` and `ariaPort`. Without an `ariaAction` argument it tests the connection;
with one it submits an ARIA action and checks its result ticket.

ARIA's **Settings → Streamer.bot** window generates a ready-to-paste copy with your
selected action already filled in. Generated scripts never include your API key.
