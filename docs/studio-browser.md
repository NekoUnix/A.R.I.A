# Studio browser (Windows development build)

Choose **Open studio browser** in the studio rail. ARIA opens its own browser
window using the installed Microsoft Edge WebView2 Runtime. It runs in a separate
ARIA process so browser work does not run on the avatar render loop. Closing the
browser does not close the studio; closing the studio leaves the browser window
available until you close it.

The browser supports up to 12 tabs, HTTP/HTTPS addresses, search, back/forward,
reload and per-tab zoom. Use the star to add or remove a bookmark. Bookmarks and
the latest 500 distinct visited addresses persist locally; open tabs are not
restored automatically. History can be cleared independently of cookies.

Each download asks where to save the file. Cancel the dialog to reject it. The
Downloads page shows up to 100 completion/failure results from the current window;
live progress, pause/resume and persistent download history are not implemented.
Downloaded files are never executed automatically. New-window links become tabs
subject to the tab limit; sites requiring a JavaScript popup/opener relationship
may require the system browser instead.

The browser uses a separate local WebView2 profile, not your Edge/Chrome profile.
It defaults to `%LOCALAPPDATA%/ARIA/browser`, or `browser` beneath an explicit
`ARIA_PROFILE_DIR`. Website cookies and sign-ins stay in that profile. Browser
history is not included in avatar exports or support reports. Websites receive no
ARIA action API or native command bridge. File URLs, script URLs, custom protocol
navigation and credentials embedded in URLs are rejected. Camera/microphone and
other website permission prompts remain the runtime's responsibility.

If WebView2 cannot start, ARIA displays a runtime repair instruction. The browser
is currently Windows-only; this addition does not implement browser textures in
avatar outputs, extensions, remote guest media, native stream encoding or VST
hosting. Streaming-chat account authorization continues to use the system browser.

Official Windows MSVC builds link the WebView2 loader statically. Developers using
GNU/LLVM Windows targets must make the matching `WebView2Loader.dll` from the
`webview2-com-sys` dependency available alongside the executable or on PATH. The
WebView2 Runtime is installed and serviced separately by Microsoft, and is not
bundled by ARIA. See [Microsoft's runtime distribution guide](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution)
and [Wry documentation](https://docs.rs/wry/0.57.0/wry/).
