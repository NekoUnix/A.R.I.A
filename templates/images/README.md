# PNG/GIF action template

1. Open `artwork/idle.png` using **Avatar & appearance → Open PNG / GIF**.
2. Open **PNG / GIF actions → Import actions** and choose `starter.aria-images.json`.
3. The example has Idle, Talking, Blink and a manual Excited GIF action.
4. Enable the microphone in its tab, or use tracking for the talking image.
5. Select an action to change its artwork, input range, priority, motion, GIF playback
   and transition. Assign a hotkey to Excited GIF, or use **Activate / hold image**.
6. Save actions. Export actions writes your own reusable configuration.

Replace these original example drawings with your own artwork. Keep all states on
the same canvas with the avatar aligned to the same position for smooth swaps.
`avatar-template.svg` provides editable eye/mouth groups. Export your drawings as
PNG or GIF. GIF frames should use a consistent logical canvas. All template artwork
is provided under the repository's MIT license.

The JSON paths resolve relative to the configuration. You may use absolute local
paths as well. Import clears hotkeys and manual selection; assign your own keys.
The configuration references artwork instead of copying or embedding it.

Each state supports `trigger` (`Idle`, `Talking`, `Quiet`, `Blink`, `Input`, `Manual`),
`priority`, `rule` (tracking/parameter source, start, end, hysteresis), `hotkey`,
`transition` (`Cut`, `Crossfade`, `FadeThrough`, `Slide`), `fade_in`, `fade_out`,
`minimum_hold`, `motion`, `gif_speed`, `gif_loop`, and `restart_gif`.

Motion `kind` is `None`, `Shake`, `Jump`, `Blip`, `Pulse`, `Wobble` or `Bob`, with
`strength`, `duration`, `frequency` and `repeat`. All units and controls are described
in the circled **?** help inside the app and in [offline help](../../docs/in-app-help.md).
Use MicLevel or MicTalking in a custom input rule to build reactions to volume.

PNG/GIF files also work as pinned stage objects and throw/spray assets. Animated
objects retain transparency and pause with the relevant scene/effect clock.
Limits: 40960px source edges, 320 MiB per file, 256 GIF frames, 1280 MiB decoded per GIF,
and 2560 MiB per image collection. There may be up to 128 actions per avatar.

To regenerate the example artwork, run `python generate_templates.py` with Pillow
installed. Editing the supplied files in your normal drawing app does not require
Python or Rust.

Source dimensions and decoded budgets both apply. Textures above the GPU edge limit are fitted once when imported; original artwork is unchanged. Larger assets require more memory.
