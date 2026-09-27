# Twitch and YouTube chat on Windows

![ Chat setup with the supplied Odette avatar](images/chat-v23.png)

Open **Streaming chat** in the studio's left panel. Enable either service or both,
then **Open chats below portrait**. The native chat companion follows the 9:16
preview and stacks the enabled panels beneath it. Turn **Follow portrait preview**
off to position the chat window independently.

Each service has its own **Transparency & colors** section:

- **Whole chat opacity** fades text and background together.
- **Background opacity** fades only the background; text stays readable.
- **Background** offers a color picker and `#RRGGBB` input.
- **Text**, username colors, text size and panel height control readability.
- **Chat width** sets the common width; **Keep chat on top** is independent of
  the avatar output's window preference.

The content can be fully transparent. The Windows title bar remains separate.
These appearance settings save per avatar. Streaming accounts are shared across
avatars and never enter model presets or templates.

## Connect Twitch

Click **Connect Twitch**. ARIA opens Twitch's website for authorization. No API
application or client-ID setup is required from streamers. The publisher supplies
ARIA's app registration with the release build. Builds without registration show
a clear unavailable state; see [publisher setup](publisher-sign-in.md) for maintainers.

Complete the official activation page in your
normal browser. ARIA displays the device code and a link to reopen the page.
It requests `chat:read` to read chat. A blank channel selects your own Twitch
channel; enter a channel name or Twitch URL to read a different channel.

The implementation uses Twitch's documented
[Public device authorization flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow)
and [secure IRC WebSocket interface](https://dev.twitch.tv/docs/chat/irc/).
Access tokens validate on connection and at least hourly; refresh tokens rotate.

## Connect YouTube

1. Click **Connect YouTube**. Choose the Google account/channel and consent in
   the official browser page. ARIA requests `youtube.readonly`. Google Cloud
   registration and consent configuration are handled by ARIA's publisher.
2. Leave the target blank for your single active broadcast, or paste a live
   video's URL/ID. Watch URLs, `youtu.be` and `/live/` links work. A channel URL
   does not identify a particular live chat. With simultaneous broadcasts,
   choose one explicitly.

Google sign-in follows its
[desktop OAuth flow with PKCE and loopback callbacks](https://developers.google.com/identity/protocols/oauth2/native-app).
The temporary listener binds only to `127.0.0.1` on an available port and checks
a random state. Account passwords and browser cookies never pass through ARIA.
Chat availability is resolved through
[liveBroadcasts](https://developers.google.com/youtube/v3/live/docs/liveBroadcasts/list)
or the selected video's `activeLiveChatId`.

YouTube API calls use the publisher project's quota (or the existing registration
on a previously configured installation). The current client uses
[liveChatMessages.list](https://developers.google.com/youtube/v3/live/docs/liveChatMessages/list),
honors `pollingIntervalMillis` and uses a minimum five-second gap. Long sessions
can exhaust project quota. Quota, permission, disabled-chat and ended-broadcast
errors appear in the panel and stop further polling. The newer server-streaming
`streamList` transport is not implemented in this version.

## Remember, disconnect and sign out

**Remember login** stores tokens encrypted with Windows DPAPI for the current
Windows user. Credentials
are stored in local application preferences, never in exported avatar presets,
artwork templates, repository files or portable ZIPs. Settings copied to another
Windows account cannot unlock the login.

After restarting, click **Connect Twitch** or **Connect YouTube** to reuse a remembered login. **Switch account** opens a fresh browser sign-in. There is no
automatic account connection merely from launching ARIA or loading an avatar.
Uncheck Remember login to clear persisted tokens while retaining the current
session until app exit or sign-out. **Disconnect / cancel** stops reception or
pending sign-in; **Connect** resumes it. After changing a target, disconnect and
reconnect to apply the target.

**Sign out** clears the local service session and messages. To revoke provider
permission too, use [Twitch Connections](https://www.twitch.tv/settings/connections)
or [Google third-party connections](https://myaccount.google.com/connections).
Expired/revoked credentials may require a fresh sign-in.

## Display and OBS behavior

The companion is a read-only native viewer with up to 200 retained messages per
service. It processes message deletions and user-ban events. Chat text is plain
text, including textual emote codes; it cannot invoke avatar actions. Native emote
images, badges, polls, message composition and moderation UI remain available
through **Open official chat to type / moderate** in the platform's browser chat.
Hiding the viewer keeps connections alive; Disconnect or app exit stops them.

The companion is separate from the avatar's Spout canvas. It does not alter the
portrait sender's resolution, alpha or artwork. For an on-stream chat overlay,
add **A.R.I.A. Chat — Twitch + YouTube** as a separate OBS Window Capture source
and crop its title bar. Window Capture alpha depends on the selected capture
method, so check it in OBS. The avatar can continue using its existing Spout source.

The circled **?** controls in Streaming chat open detailed offline explanations.
The development screenshot scenario uses labeled sample messages and does not
sign in or transmit messages. Real-account verification requires registered
client credentials, provider access and the account owner's consent.
