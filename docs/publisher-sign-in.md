# Publisher-owned website sign-in

This page is for ARIA release maintainers. Streamers connect from **Streaming chat** and authorize on Twitch or Google's website. They do not create API applications, enter client IDs or import credential files.

## One-time registration

- Register an ARIA-owned **Public** application with Twitch. Supply its public client ID as the build variable `ARIA_TWITCH_CLIENT_ID`. Do not ship a Twitch confidential client secret. ARIA uses Twitch's [device authorization flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow) with chat-read permission.
- Register an ARIA-owned Google **Desktop app** OAuth client with YouTube Data API v3 enabled. Configure consent branding, permitted users during testing, and Google's verification requirements before general distribution. Supply `ARIA_GOOGLE_CLIENT_ID` and `ARIA_GOOGLE_DESKTOP_CLIENT_SECRET`. The installed-app client value is distributed with the executable; it is not a confidential server credential. Never substitute a Web application secret. ARIA uses [desktop PKCE and a loopback callback](https://developers.google.com/youtube/v3/live/guides/auth/installed-apps).

The Alpha package workflow reads the client IDs from repository variables and the Google desktop value from a repository secret. Local maintainers can supply the same environment variables before compiling. The client IDs identify ARIA; copying IDs from another application is not supported.

## Release readiness

Registration values have not been supplied in this checkout. A build without an appropriate registration disables its new-account Connect button and explains that the publisher must enable sign-in. It does not send users to developer consoles. Existing installations retain previously configured registrations and encrypted sessions.

Before advertising working website sign-in, verify a real account on both platforms: initial consent, remembered reconnect, cancel, declined consent, expired/revoked session, switching account, different channel/video, and sign-out. Use a channel with active chat for message verification. Unit tests cover PKCE, callback state/host validation, session rotation and error handling; they cannot prove provider registration or consent approval.

Passwords stay in the external browser. Session tokens use Windows current-user DPAPI and are excluded from avatar exports and support data. The temporary Google callback binds to loopback only. No browser cookies are imported.
