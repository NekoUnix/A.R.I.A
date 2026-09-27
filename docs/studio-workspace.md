# Studio workspace (development)

The navigation rail keeps Home, Stage, Scenes, Music & playlists, Events & gestures, Avatar library, Avatar settings, Tracking, Outputs & OBS, Streaming chat and Settings in one place. Production pages use the full workspace; avatar settings appear beside the stage. The Stage inspector holds input, physics, expression, object and pose tools. Drag the inspector divider to give the stage more room.

## Start a session

1. Open **Home** for avatar, tracking and output checks. Demo input is explicitly identified; it does not mean your phone is connected.
2. Import an avatar or reopen one from **Avatar library**. Existing profiles retain their settings.
3. Open **Tracking** to connect and calibrate. The setup guide remains reopenable.
4. Open **Outputs & OBS** to configure and preview a canvas. The footer counts open output windows; it does not report a platform broadcast as live.
5. Use **Save profile** after editing. ARIA also retains its existing autosave behavior.

**Find a tool** searches names and common terms such as hair, bounce, lipsync and OBS. Ctrl+K (Cmd+K on macOS) opens it; arrows select, Enter opens, Escape closes. Existing custom Ctrl+K bindings take precedence; use the button in that case. Tools incompatible with the current avatar format are omitted.

**Focus stage** hides the navigation and inspector. Use **Exit focus** or Escape when no text field owns keyboard focus. This affects the editor layout, not the configured output canvas.

Home includes local notes (2,000 characters), a pauseable session timer and session health. Notes use separate local app storage and are not added to support exports. The timer uses elapsed time, not rendered frames, and never starts or stops a broadcast. Performance graphs can be expanded in the footer.

New installations use **Neko Studio**, with pink, mint and lavender colors. Workspace tabs, an icon sidebar, a full-width tool search and rounded panels follow the reference studio workflow. Existing saved palettes remain unchanged; select Neko Studio from Settings to switch. Community links and the built-in bread throw remain available below the navigation tools.

## Action recipes

See [production tools](studio-production.md) for scenes, playlists, speech ducking,
event rules, held-expression triggers, variables and graph conditions. See
[VMC facial input](vmc.md) for the experimental OSC receiver.

Open **Hotkeys & actions → Action nodes → Start from a recipe**. **Timed sequence** creates two actions separated by a two-second delay. **Together** creates two branches that join before finishing. Select each Action node and assign a current avatar action. The recipe cannot run with missing targets. Change delay and On/Off/Toggle behavior as needed; imported graphs keep the existing explicit reassignment workflow.

## More expressive throws

Open **Stage**, select the inspector's **Stage → Throws & sprays** tab, create or edit a throw, and use its motion controls:

- **Momentum rebound** reflects the incoming flight direction and arc. Faster flight produces a stronger rebound; gravity and air resistance shape the post-impact path. New throws enable it. Existing saved designs retain legacy motion until enabled.
- **Direction** uses drawn paths, left, right, alternating sides, or a random 50/50 side. Side modes mirror the launch point around each route's aim point. The path editor continues to display authored paths; preview playback shows the actual selected direction.
- The effects panel reports active particles and queued objects. When the particle limit is full, the queue waits and then resumes its configured interval. Clear active effects clears the queue too.

These changes do not import reference-app assets, replace ARIA's model physics with another app's code, or add VNyan graph compatibility. See the [research audit](studio-research.md) for evidence, comparisons and remaining investigations.
