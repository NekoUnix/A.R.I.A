# Make your own ARIA throws and sprays

Open **Throws & liquid sprays → Import design** and choose one of the three
`.aria-effect.json` files here. Select the imported design and click **Trigger now**.
All assets in this kit are original ARIA examples under the repository MIT license.
Keep this folder together: design paths are relative to their JSON file.

1. Copy this folder to a stable location for your own artwork.
2. Edit the SVG in a vector editor or create a transparent PNG. ARIA loads the PNG;
   SVG is the editable source. Replace `assets/star.png` or `assets/droplet.png`.
3. Open `cube.obj`, `cube.glb`, `cube.gltf` or `cube.fbx` in your 3D editor, replace
   its geometry, then export a static triangle mesh. Keep buffer/texture/MTL files
   beneath that asset's folder. ARIA centers and scales the mesh automatically.
4. Replace `pop.wav` with a short mono/stereo sound, or select WAV, MP3, Ogg Vorbis
   or FLAC in ARIA. Clips may be at most 10 seconds, 16 MiB and 192 kHz.
5. Import a design, customize it in the panel, assign a hotkey and save.
   Export the design again to back up your changes. Import assigns a fresh ID and
   clears the hotkey; configure integrations using the new ID shown in the list.

Run `python generate_templates.py` to regenerate the **original samples**, replacing
edits to those sample files. It uses Python 3's standard library only. Edit the
generator to change its mathematical artwork and sound synthesis; SVG files can
also be edited and exported manually without Python.

## Live2D and VRM

`live2d/MyProp.model3.json.template` shows the export layout. Export your own model
with Live2D Cubism and use its actual `.model3.json`, `.moc3` and texture atlases.
A JSON file cannot substitute for a compiled moc3. No Cubism binary or private
avatar is included. Throw props use the default pose and remain independent of
the main avatar. Four distinct Live2D throw assets can be loaded at once.

VRM 0.x/1.0 files use glTF geometry and can be selected directly as static props.
Export a real VRM from your VRM authoring tool; renaming a cube GLB does not create
a rigged VRM avatar. ARIA renders the exported rest geometry and base color, without
VRM animation, spring bones or MToon shaders. FBX/OBJ also use static geometry and
base-color materials. Export unsupported project formats (`.blend`, `.max`, `.ma`)
to GLB, glTF, FBX or OBJ. Compressed mesh extensions need a regular triangle export.

## Design fields

The envelope is `{"aria_effect":1,"design":{...}}`. `effect.schema.json` describes
the values; the application validates them again on import. Missing design fields
use the Star toss defaults. `id` is reassigned on import. `name` is 1–80 characters.

| Field | Meaning |
| --- | --- |
| `kind` | `Throw` bounces; `Spray` pins a splat at a visible surface |
| `assets` | 1–256 file paths or `builtin:star`, `builtin:ball`, `builtin:cube`, `builtin:drop` |
| `selection`, `count` | `Random` / `Cycle`: total particles; `All`: copies of each asset; at most 1,000 per trigger |
| `interval`, `flight` | Seconds between emissions and seconds to the aim plane |
| `origin`, `target` | `[X,Y]` offsets from avatar center in canvas-height units; right/down are positive |
| `size`, `size_variance` | Canvas-height fraction and fractional random size variation |
| `spread`, `arc`, `spin` | Aim randomness, upward curve, clockwise degrees per second |
| `bounce`, `lifetime`, `impact` | Rebound amount, seconds after impact, damped avatar composition recoil |
| `tint`, `splash` | Multiplicative RGBA bytes and spray expansion after landing |
| `launch_sound`, `impact_sound` | Local clip, `builtin:whoosh/pop/spray/splat`, or empty string for silence |
| `volume`, `cooldown` | Design volume 0–1 and minimum seconds between accepted triggers |
| `hotkey` | Set in ARIA after import; sharing a design never installs a shortcut |

There is no fixed number of saved designs. Runtime caps are 256 active particles,
32 queued bursts and 16 audio voices. A queue-full or asset error appears in ARIA.
Effects pause with Freeze pose and appear in PNG export and all output canvases.
Transient particles are not serialized into movement presets. OBS captures ARIA
audio separately; Spout carries video only.

## Stream events and plugins

See [the plugin setup guide](plugins/README.md) for Streamer.bot, Twitch, Touch
Portal, PowerShell and HTTP. These trigger the same saved designs as buttons and
hotkeys. ARIA does not log in to Twitch; your stream tool provides that connection.
