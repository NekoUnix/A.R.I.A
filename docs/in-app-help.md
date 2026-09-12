# ARIA in-app help

This guide is bundled into the application and works offline. The topic headers also provide the short hover descriptions for the circled question-mark buttons.

## effects | Throws & liquid sprays | Create reusable effect designs for the current avatar, then trigger them with a button, your own shortcut, or an event from a stream tool. Pause playback for screenshots.

### Start with a design

Open Throws & liquid sprays in the right panel. Every avatar starts with Star toss, Soft ball volley, 3D cube tumble, Water spray and Paint splash. Select a name to edit that design; the Throw or Spray button beside it immediately queues it. Search filters names. New throw and New spray add independent designs with new IDs. Duplicate copies the selected design and clears its hotkey. Delete design removes only the saved definition; Clear active effects stops particles, recoil and sounds already playing.

The name is your label, up to 80 characters. Mode chooses a bouncing thrown object or a liquid particle that attempts to stick on impact. Multiple designs can play together. There is no fixed count limit on saved designs. Each trigger emits at most 1,000 particles, up to 256 particles can exist at once, and up to 32 bursts can wait in the queue. When playback is full, emission waits for space. Cooldown can reject rapid repeat triggers of the same design. Status text reports missing assets, cooldowns and queue limits.

### Save, pose and capture

Edits save with this avatar after editing settles. Save effects writes immediately. Loading another avatar restores its own library, volumes and shortcuts, and clears the previous avatar's transient effects. Moving or renaming an asset file requires updating its reference or importing a corrected design. The default Mica puppet and PNG avatars also have separate profiles.

Pause effects holds particles and recoil while the avatar can keep moving; attached splats still follow its surface. Freeze pose in Pose holds both avatar and effect playback. Use either before Save transparent PNG for an image containing the current effects. Movement and pose presets store avatar poses and stage objects; they do not serialize a transient particle timeline. All three OBS outputs include effects, with each output's framing, zoom and background. Effect audio needs separate OBS audio capture.

## effect-assets | Throw and spray artwork | Mix transparent PNG artwork, complete Live2D exports and static 3D props in a design. Asset selection controls which file is emitted and how many copies are made.

### Files and selection

Add asset files accepts PNG, moc3 or model3.json, GLB, glTF, VRM, FBX and OBJ. Select multiple files to build a pool. Star, Ball, 3D cube and Droplet add built-in artwork. Up and Down change the pool order; Remove removes a reference, leaving its file untouched. Keep at least one asset. Random chooses from the pool for each particle. Cycle walks the pool in order until Number to emit is reached. All emits Copies of every asset; for example, 3 assets and 4 copies produces 12 particles. Each design holds up to 256 asset references and each trigger is limited to 1,000 total particles.

PNG artwork should be tightly cropped with transparent padding where needed. Maximum dimensions are 4096 × 4096, with a combined active PNG budget of 256 MiB decoded. A moc3 needs its matching model3.json and all referenced atlas PNGs beside it; select Cubism Core under Avatar setup first. Live2D throw props use independent instances at their authored default pose. They never replace or reconfigure the main avatar. Up to four different Live2D throw assets can be loaded together, with a combined 1 GiB atlas budget. Repeated particles of the same asset share its rendered image.

### 3D import and resource limits

GLB/glTF and VRM use glTF triangle geometry; FBX and OBJ use the native ufbx importer. These are static prop imports: mesh transforms, UVs, normals and base-color materials render with simple lighting and a tumble animation. Skeletal animation, blendshape playback, VRM spring bones, MToon, full PBR and proprietary material graphs are not evaluated. Export the desired rest pose and bake complex materials to a PNG or JPEG base-color texture. Unsupported compressed geometry should be exported as ordinary triangles. A .blend, .max or .ma project must be exported to a supported interchange format first.

Keep local buffer, MTL and texture companions within the asset folder. Remote texture URLs and references outside it are rejected. Use at most 200,000 triangles, 256 material batches, 128 MiB per model file and 4096 pixels per texture edge. Active 3D resources have a 512 MiB budget; each prop is rendered to a shared 512 × 512 transparent texture. All copies share that asset's 3D orientation, with individual flight and screen-space spin. Reload cached assets / sounds clears playback and retries edited or missing files. Small exports load faster and consume less VRAM.

## effect-motion | Emission, flight and impact | Set the burst count, trajectory, size, timing and recoil. Positions are measured relative to the avatar canvas so a design scales with every output resolution.

### Coordinates and size

Launch position and Aim position are X/Y pairs in avatar-canvas-height units, measured from its center. X increases rightward and Y downward. An origin of (-0.8, -0.25) starts 0.8 canvas heights left and 0.25 above center. An aim of (0, -0.16) targets the upper center. Target spread adds random horizontal and vertical offsets around that point, up to one canvas height. Aim at visible artwork for sprays to find a surface. Transparent padding belongs to the avatar canvas too.

Item size is the particle image's height divided by avatar canvas height: 0.10 means one tenth of that height. Size variation adds a random fractional variation, so 0.25 gives 75–125 percent of the chosen size. Aspect ratio is preserved. These values scale with the model when the mouse wheel changes zoom in any output.

### Timing and response

Interval is the time between emissions, from 0 to 5 seconds. Zero starts a rapid burst, subject to the particle limit. Flight time is seconds from launch to the aim plane. Flight arc adds upward curvature when positive and downward curvature when negative; zero flies straight. Spin is degrees per second in screen space, with negative values reversing direction. Bounce controls the sideways/upward rebound of thrown objects after impact. Time after impact controls the fade duration, from 0.1 to 15 seconds.

Avatar recoil applies a damped displacement to the avatar composition after impacts, then settles back to zero. It moves the avatar and its accessories together without overwriting your tracking parameters or rig physics. Zero disables it. Impact timing is a 2D aim-plane effect, not a 3D collision simulation; liquid additionally checks the avatar surface when it lands. Cooldown is the minimum real time between accepted triggers of this design, up to 60 seconds. Pause/Freeze stops simulation time. A fixed-step simulation keeps normal 15–120 FPS playback consistent; long suspend gaps are discarded to avoid a burst of overdue work.

## sprays | Liquid spray, splats and color | Spray particles fly toward the avatar, expand into splats on a visible mesh, follow that surface, then drip and fade. Customize the artwork and tint for water, paint or your own liquid.

### Make a spray

Choose Water spray or Paint splash, or click New spray. Select a droplet PNG or use the built-in white Droplet; a white source makes tint colors easy to control. Liquid & color contains the RGBA picker. RGB multiplies the artwork's color channels and A controls transparency. White with full alpha preserves the original artwork. Dark or colored artwork cannot become brighter just by tinting it. This tint also works for thrown assets.

Splat expansion adds growth just after impact: 0 keeps the incoming size and 1 doubles it. Expansion ranges from 0 to 2. Time after impact determines how long the coating lasts. Smaller Item size, more particles, short intervals and some Target spread create a stream; larger particles and higher expansion create paint blobs. Launch and impact clips can be replaced separately. Disable the impact sound for a continuous spray if individual splat sounds become too busy.

### Surface attachment and limits

On landing, ARIA looks for a rendered Live2D triangle beneath the aim point and stores a surface pin. The splat then follows that triangle while slowly drifting downward and fading. A particle that misses the mesh falls away. For Mica and PNG puppets, attachment follows the puppet's coordinate system. Pins live only for this effect's lifetime and do not add permanent Stage objects. This is a stylized particle coating, not a fluid solver, texture painting system or wet-material shader; it does not edit the avatar's texture files.

Pause effects keeps the current coating visible while tracking continues. Freeze pose stops both for an image. Clear active effects removes all current coating and recoil immediately. The same coating appears in landscape, portrait and freeform outputs and transparent PNG export. Use transparent output for soft alpha edges; colored liquid may conflict with a chroma key. Choose a key after checking the colors in your effects as well as the avatar.

## effect-sounds | Replaceable launch and impact audio | Choose local WAV, MP3, Ogg Vorbis or FLAC clips for each effect, preview them, and mix their volume. OBS captures this audio separately from the video canvas.

### Select and mix clips

Launch sound plays once when the burst begins emitting. Impact sound plays once per particle at the aim plane. Choose audio selects a file, Preview plays the selected clip at design and master volume, and None disables that event. Built-in whoosh, pop, spray and splat buttons restore synthesized starter sounds. Design volume multiplies Master volume. Mute effects silences playback, and Pause effects, Freeze pose and Clear active effects stop current sounds. Preview is an explicit audition even when automatic effects are muted.

Clips may be mono or stereo, up to 192 kHz, 10 seconds and 16 MiB per compressed file. WAV PCM, MP3, Ogg Vorbis and FLAC are supported. ARIA normalizes decoded clip peaks and limits simultaneous voices to 16; new sounds retire the oldest voice when full. It caches up to 64 MiB of decoded audio. Use a short, quiet impact clip for large volleys. Missing clips or unavailable devices show an error while visual playback continues. Reload cached assets / sounds retries the audio device and rereads edited clips.

### Windows and OBS

Playback uses Windows' default audio output when the audio device is first opened. Choose the desired device in Windows before starting ARIA. In OBS, add Application Audio Capture for ARIA, or use Desktop Audio for that device. Avoid capturing both at once because this doubles the sound. Spout shares only the image texture and carries no audio. Test a quiet built-in preview while watching OBS's meter before going live. The template kit contains an editable WAV and synthesis script; you can replace it with your own recording without changing code.

## effect-designs | Editable designs and asset templates | Import and export a design as JSON, duplicate it for variations, and use the bundled artwork, 3D, Live2D layout, audio and plugin templates to create your own effects.

### Save and share a design

Export this design writes an .aria-effect.json file with aria_effect version 1 and a design object. It stores the selected asset paths, sounds, count, timing, movement and color. It does not embed the assets. For a portable kit, put files in a folder beside the design JSON and replace absolute paths with relative paths such as assets/star.png. Import design resolves those paths relative to the JSON file. Built-in references such as builtin:star and builtin:whoosh work without companion files.

Import gives the design a new local ID and clears its hotkey, preventing someone else's assignment from replacing your shortcuts. IDs are displayed in the design list and used by the plugin API. Use names for people and IDs for automation. Duplicate similarly creates a new ID with no hotkey. Changes save in the current avatar profile, independently of movement presets. Keep a backup export before restructuring a library used by stream events.

### Template kit

Open the templates/effects folder beside the portable executable, or in the repository. Its README explains the editable SVG and transparent PNG artwork, OBJ/MTL, GLB/glTF and FBX prop examples, WAV audio, JSON designs, schema and plugin adapters. generate_templates.py regenerates the sample assets from their source. You can use the starter artwork under ARIA's MIT license and replace it with your own design.

Live2D templates provide the folder layout and a model3.json template, not a fabricated moc3. Export your own model from Cubism, keep its atlases and manifest together, then select it as a visual asset. The 3D templates are small static props; export VRM characters from a VRM-capable tool when needed. The import renderer uses their rest geometry. Design files allow paths to local assets only; the event API selects an already saved design and cannot import files or run commands.

## effect-api | Stream events and plugin bridge | Enable the local API to trigger saved effects from Streamer.bot, Touch Portal or a custom tool. Use the supplied adapters to connect Twitch events without changing ARIA's code.

### Enable and connect

Expand Stream events & plugins and enable the local plugin API. ARIA generates a random API key. Copy API key puts it on the clipboard; paste it into your local tool's configuration. The default address is http://127.0.0.1:39421. Loopback TCP port changes the port if another program uses it. Generate new key replaces the current key, so update each adapter afterward. The API binds only to this PC and starts disabled. It is separate from iPhone tracking and does not require a tracking firewall rule.

Send Authorization: Bearer YOUR_KEY on every request. GET /v1/effects returns the loaded avatar's design IDs, names and kinds. POST /v1/effects/trigger accepts a JSON body containing version: 1 and id: the saved design ID. HTTP 202 means queued, not guaranteed playback: cooldown, resource limits or missing assets may still reject it, with details in the ARIA panel. The bridge accepts up to 20 requests per second and queues up to 32 commands. Changing avatars discards commands queued for the old profile; query the catalogue again because the new avatar can use the same ID for a different effect.

### Bind stream and desktop events

The templates/effects/plugins folder contains a Streamer.bot Execute C# Code adapter, a PowerShell client and request examples. In Streamer.bot, set the ariaKey, ariaPort and ariaEffectId arguments before the C# sub-action, then attach any event you want: channel-point redemption, cheer, subscription, chat command, timer or another installed integration. Streamer.bot owns the Twitch login, permissions and event conditions. ARIA receives only the resulting local trigger; it does not connect directly to Twitch or store a Twitch token.

For Touch Portal, configure an HTTP POST action with the same endpoint, Authorization and Content-Type: application/json headers, and JSON body. A custom plugin can list IDs and send the identical request. The adapters run on the same PC as ARIA. Browser-origin requests, remote network access, arbitrary asset paths and command execution are not supported. Keep the key in local settings, out of shared screenshots, public action exports and repositories. Error responses distinguish invalid requests, unknown effects and queue/rate limits. Detailed copyable examples are in the bundled guide.

## png-items | Stage objects & avatar accessories | Drop PNG images or Live2D exports onto Your stage to add accessories. Each item has its own placement, pin, visibility toggle and optional input rule, saved with this avatar.

### Add and select items

Drag .png, .moc3 or .model3.json files from File Explorer into the central stage, or open Stage objects & toggles in the right panel and click Add objects. Stage drops add accessories to the current avatar. Use Avatar & appearance to replace the main avatar. See Pinnable Live2D objects for model texture setup and independent parameter controls.

Click an accessory or its name in the item list to select it. Drag its rectangle on the stage to position it. Transparent padding is part of that rectangle, so tightly cropped PNGs are easier to handle. The highlighted border and name are editing aids; they do not appear in clean outputs or exported images. Hidden items remain selectable in the list. The checkbox beside a name controls master visibility.

### Files, performance and saving

Use a stable local folder for your objects. Profiles and presets reference the asset path; they do not embed or copy the file. If an image is moved, choose Replace object / locate file. Reload assets retries missing files and rereads edited artwork. Removing an item removes its settings and shortcut, leaving the original file on disk.

Each avatar can have up to 32 items, including at most four Live2D objects. Each PNG image must be a real PNG, at most 4096 pixels in either dimension and 32 MiB on disk. The combined decoded PNG image budget is 256 MiB, counting shared paths once. Texture assets are cached; moving or toggling an item does not decode or upload the PNG again. Changes save locally after editing settles; Save item settings saves immediately. Missing or oversized files show an error beside the selected item without preventing other items from working.

### Presets, screenshots and outputs

Movement and pose presets include the item list, placement, pins, visibility and input rules. Shortcut assignments belong to the avatar's profile; importing a preset does not install someone else's shortcuts. A shortcut is active only when its item is present in the current layout. Deleting an item clears its shortcut even if an older preset still contains the item.

All three output canvases include the same accessories, with each canvas's framing, zoom and background. Mouse-wheel avatar scaling in an output scales its accessories too. The automatic key-color suggestion includes loaded object colors, including hidden items; Live2D object colors are sampled from their current rendered pose. Save transparent PNG in Pose saves the avatar and visible accessories on the model's rendering canvas (maximum model edge 2048 pixels); Mica and PNG puppets use a 1200 × 1400 image. Artwork outside that canvas is clipped. Selection outlines and the studio grid are excluded.

## model-items | Pinnable Live2D objects | Drop a moc3 or model3.json onto Your stage to add an independent Live2D object. It has its own pose and can follow a pin without replacing the main avatar.

### Drop an export

Drag the .moc3 into Your stage, or use Stage objects & toggles → Add objects. Keep the matching .model3.json beside it and preserve all referenced texture folders. ARIA finds the manifest that references that exact moc3 and loads its atlases in the authored order. Dropping the .model3.json itself works too. Drop only one of those two files to create one object; dropping both creates two independent objects. A moc3 contains compiled geometry and parameters, not the texture images.

Each object has a separate Cubism instance, parameter list and optional physics simulation. Adding, removing or posing it does not replace the main avatar, change the main profile identity, or edit its mappings, expressions or physics. The Cubism Core DLL selected under Avatar & appearance → Cubism runtime setup is shared as a runtime library. Set that DLL once before loading models. Main-avatar selection still uses Open Live2D avatar in the left panel.

### Bare moc3 texture setup

If no unique matching manifest is found, select the object in the list and expand Live2D object → Object texture setup. Add atlas PNGs in the model's texture-index order: index 0 first, then index 1, and so on. Up and Down reorder the pending list; Remove removes an entry from this pending list, leaving the file untouched. Apply texture order commits the list and reloads only this object. Use matching manifest instead clears the explicit list and retries automatic discovery.

Atlas order comes from the export or its creator; ARIA does not guess from filenames. Missing atlases or incorrect order can cause errors or mismatched artwork. Manual atlas loading provides geometry and textures; optional tracking, physics and display metadata require a complete manifest export. Atlas PNGs added here are textures inside this Live2D object. Dropping those PNGs directly on the stage would create separate image accessories.

### Object pose and movement

Animate object from tracking starts off. In this mode the object uses its authored defaults plus the values in Object parameters. It can still follow its pin on a moving main avatar. Enable animation to feed the current processed tracking inputs through this object's own imported or standard mappings. Use object's physics enables this object's imported physics file and secondary motion while animation is on. This checkbox does not change the main avatar's physics settings. Missing optional files are reported in the object's status.

Object parameters lists this object's actual IDs, labels and authored limits. Search filters IDs and labels. Check Override to hold the current value, or drag the slider to choose and hold a new value. A slider edit enables the override automatically. Uncheck Override to return that parameter to its authored default in static mode, or to its normal evaluation in animated mode. Overrides take precedence over tracking and physics. Reset object parameters clears all overrides; in a frozen pose it also clears the stored object snapshot back to defaults. Turning animation off returns unheld values to authored defaults.

Freeze pose in the Pose tab freezes the main avatar and the final parameter values of its objects. Object overrides can still be edited for screenshots. Movement and pose presets store object files, texture order, overrides, animation/physics options, pins, placement and visibility. A frozen preset restores its object pose even while tracking changes. Resume pose restores each object's selected animation mode. Shortcut assignments remain per-avatar settings.

### Pin, show and capture

Select and drag the object on Your stage. Use Pin here at its center, or Choose pin point and click the main avatar. Pinning follows the chosen main-model surface; it moves the object's entire canvas while its own internal deformation remains independent. The same size, opacity, layer, rotation, stretch and visibility-follow controls work for PNG and Live2D objects. Model canvases can include substantial transparent padding, which counts toward object size and hit testing. Increase Size if the visible artwork is small.

Input toggle reads the main avatar's processed tracking inputs or final parameters, and controls this object's visibility. Keyboard toggle gives the object a user-defined visibility hotkey. All three outputs, full-resolution Spout canvases and transparent PNG export include visible objects. Their selection outlines stay in the editor. Pinned objects and the main avatar share each output's framing and wheel zoom.

### Resources and recovery

There can be at most four Live2D objects within an avatar's 32-object limit. Object atlases have a combined decoded budget of 1 GiB, separate from the main avatar and PNG accessory budgets. Each model instance has its own atlases; using the same file twice still consumes two instances' memory. GPU targets, masks and other working memory add to atlas usage. Use lower-resolution exports for small accessories. Static unchanged objects reuse their rendered image; animated objects update from their own parameter changes. Hidden objects remain loaded so toggling them does not reload artwork.

Files are referenced locally, not embedded in profiles or presets. Keep the full export in a stable folder. Replace object / locate file changes the selected asset and resets its object-specific pose and texture setup, while preserving placement and pin controls. Reload this object retries an error or rereads edited files for just this object. Reload assets reloads all accessories. A failed object remains in the list with its error and is omitted from rendering; it does not unload the main avatar. Remove item removes its settings and shortcut, leaving original files on disk.

## png-placement | Object size, position & layers | Position accessories by dragging or editing their offsets. Size is relative to the avatar canvas height, so the same layout follows every output resolution and zoom.

### Size, rotation and opacity

Size is the object's image/canvas height divided by the main avatar canvas height. For example, 0.20 means one fifth of that height; it does not mean 20 pixels. The object keeps its aspect ratio. Rotation is an added clockwise angle in screen space, between -180 and 180 degrees. Opacity ranges from 0 (invisible) to 1 (fully visible); the object's own alpha still applies.

X offset increases toward the right; Y offset increases downward. Both use canvas-height units, independent of desktop DPI or OBS resolution. A free item's offset is measured from the avatar canvas center (Mica uses its drawing origin). A pinned item's offset is measured from its pin. Follow pin rotation rotates this offset with the surface; Follow surface stretch scales it with the surface. Dragging a pinned item edits the offset without removing its pin. Reset offset returns it to the pin, or to the canvas origin if it is free.

### Layers and locking

Behind the avatar draws the item before the avatar. Otherwise it draws after the avatar. Backward and Forward change its order relative to other items on the same side of the avatar; later items are in front. Accessories cannot be inserted between individual Live2D ArtMeshes in this version. Lock stage dragging prevents accidental movement while leaving sidebar editing, toggles and pin following available. Size, rotation and visibility still work when dragging is locked.

Free items stay in the avatar's canvas framing but do not follow face motion. Pin an item when it should follow a head, hair strand or other animated part. Changing the output's position or wheel zoom moves the complete avatar-and-accessory composition.

## png-pins | Pin an object to a moving surface | Pin here attaches at the item's center. Choose pin point lets you click a moving part of the avatar. Live2D pins follow that mesh's animated vertices.

@diagram pin

### Pick a surface

Select the object. Choose pin point, then click the model where its center should attach. The object moves to that point and its offset becomes zero. Pin here uses the current object center; place it over the avatar first. Escape or Cancel pin leaves the existing placement unchanged. After pinning, drag the accessory to adjust its offset.

Live2D picking selects the frontmost visible triangle at the clicked location. A pin stores that triangle's vertices and a weighted position inside it. Each frame, the updated mesh supplies the anchor, including changes caused by tracking, expressions and physics. ArtMesh numbers identify the selected mesh in this avatar. Picking uses mesh triangles, not individual texture alpha or clipping-mask pixels; if the wrong layer moves the item, try a nearby point on the intended part. Invisible and zero-opacity meshes are skipped during selection.

### Following options

Follow pin rotation uses the triangle's longest edge as an orientation reference at pin time. Turning it off keeps the object and its offset upright while its center still follows the anchor. Follow surface stretch changes accessory size and offset with that edge's length; its multiplier is limited to 0.25–4 to prevent extreme deformation. This is rigid attachment movement, not a warp of the accessory itself. A Live2D object still renders its own internal deformation independently.

Follow surface visibility multiplies item opacity by the selected mesh's visibility and opacity. Disable it to keep an accessory visible when an expression hides that mesh. This does not apply that mesh's clipping mask to the accessory. If a pin's geometry is missing or collapsed, the accessory is hidden until a usable surface returns or you repin it.

Mica and PNG puppets do not have Cubism meshes, so their pins follow the puppet's head translation and rotation. Surface stretch and mesh-visibility options have no extra effect on those puppets. Unpin keeps the accessory's current position, scale and orientation, then stops motion following. The pin is stored with this avatar's content-based profile, not with the test model or a global mapping.

## png-toggles | Named toggles, input rules & hotkeys | Each object has a named visibility toggle. Show it manually, assign a custom keyboard shortcut, or use a tracking input or model parameter to drive its visibility.

@diagram item-toggle

### Choose a behavior

Rename Toggle name to describe the action, such as Sunglasses or Talking sparkle. Manual / hotkey uses the master checkbox and assigned shortcut. Visible while in range shows the item only while master visibility is on and the chosen signal is inside the range. Toggle on entering range flips master visibility once when the signal enters the range from outside. Staying inside does not keep flipping it.

Select Tracking input for a processed input such as MouthOpen, FaceAngleX or EyeOpenLeft. Select Model parameter for an actual parameter on the current avatar, including the final effects of mappings, expressions, held values and physics. The list comes from the current signals or model. You can type an exact custom ID; spelling and case matter. The current signal value helps choose thresholds. If an ID is unavailable, a range-gated item hides and a latched toggle keeps its last master state. ARIA's standard tracking inputs may return neutral values when tracking disconnects, so use thresholds appropriate to those values.

### Ranges, stepping and hysteresis

Start and End are inclusive and use the selected signal's units. For a sparkle that appears while talking, choose MouthOpen, Visible while in range, Start 0.35, End 1.0 and Hysteresis 0.05. The signal enters at 0.35, but must fall below 0.30 (or rise above 1.05) to leave. This margin reduces flicker near a threshold. It does not change the signal or the avatar mapping.

To toggle sunglasses with a blink, use EyeOpenLeft, Toggle on entering range, Start 0, End 0.2 and Hysteresis 0.05. A first sample establishes the baseline; opening a profile or reconnecting an unavailable signal does not fabricate a toggle. Leave the range before entering it again. For stepped model-parameter triggers, configure that parameter's Step in Inputs; the rule reads its final stepped value. The rule itself uses the inclusive range and hysteresis, without an extra stepping filter.

### Keyboard shortcuts and frozen poses

Choose Ctrl, Alt, Shift or Win modifiers and a main key, then Assign shortcut. ARIA rejects duplicates assigned to expressions, presets, other objects or the reserved pose shortcut. Enable global hotkeys applies to all actions for this avatar. Clear shortcut removes only this item's assignment. A shortcut flips master visibility; in Visible while in range mode, the input condition must also match. A key without modifiers can intercept normal typing in other applications. Windows reserves F12, and another application can own a shortcut; registration failures appear in the panel.

Freeze pose pauses input rules and captures the current gated visibility with the pose preset. Manual master toggles and hotkeys still work so you can choose accessories for screenshots without moving the model. Resume live reevaluates the input conditions. Save a pose or movement preset to keep multiple accessory layouts for the same avatar.

## welcome | Getting started & finding help | Hover a circled question mark for a quick explanation. Click it to open a separate, searchable help window without pausing your avatar.

### Your first session

1. Leave Tracking & connection on Demo to check that the built-in Mica puppet moves.
2. Open Avatar & appearance to load your own PNG or Live2D export. Live2D requires the official x64 Cubism Core DLL.
3. Choose your tracking source and connect. Face the camera naturally and calibrate a neutral pose.
4. In the Input Monitor, adjust mappings, pose controls, physics or expressions for this avatar.
5. Open an output under Capture & performance. For a full-resolution OBS source with a small desktop preview, use Spout2 Capture.
6. Use Save profile to preserve the current avatar's settings. Use presets to keep named movement or pose variants.

### How this interface is organized

The left panel contains studio setup. The center is your stage. The right Input Monitor contains avatar controls and diagnostics. Collapse categories to make room; scroll either side panel independently and drag its inside edge to resize it. Categories are organizational labels, not restrictions on which models can be loaded.

Every question mark opens a topic here. Search matches words in the full guide, not just titles. Back returns to the previous topic and any context captured with it. Close this window to return to the studio; tracking and output windows keep running. Use Tab to focus a question mark, then Enter or Space to open it. Diagrams are illustrations, not controls.

## profiles | Saving & per-avatar profiles | Save profile stores this avatar's current studio settings and rig. Presets store named movement or pose variants; machine runtime settings are shared.

### What is saved

Save profile stores the active avatar's tracking source, IP and ports, mapping and calibration, FPS target, studio zoom, input bindings, manual values, steps, pose, physics groups, expression state, shortcuts, presets and output layouts locally. Output layouts include each canvas's resolution, background, framing and preview size. Opening or closing an output is session state; previews start closed on launch.

The Core DLL path and Windows process priority belong to this PC rather than an individual avatar. Saving a profile does not copy model assets, textures, expression files or the Core DLL into a portable project. Keep referenced files available. Live2D identity is based on moc3 content and image-puppet identity on decoded image content, so moving unchanged artwork preserves its identity. Replacing that content may create a different profile identity; external expression references still need usable paths.

### When to save

Edits take effect immediately unless a control says Apply or Assign. Explicit Save buttons write the profile; presets, expression assignments and output changes also request saves. Output dragging is debounced so it does not write to disk every frame. Normal shutdown and periodic application persistence also save settings, but use Save profile before experimenting or force-closing the app. A crash can lose changes not yet written.

### Profiles versus presets

A profile is the current workspace for one avatar. A movement preset saves rig tuning, mapping, physics and active expressions; a pose preset also captures the final parameter values. Presets do not include connection settings, output layouts, SDK paths or the complete model asset folder. Export a preset to share a compatible configuration, not an avatar.

## tracking | Tracking sources & connection | Demo animates locally. iPhone VTube Studio sends face tracking over your network. ARIA JSON accepts supported packets from an external tool.

@diagram network

### Choose a source

Demo generates synthetic movement without a phone or network. It is useful for checking artwork, physics and capture, but does not represent your real face. iPhone · VTube Studio subscribes to the phone's tracking stream. External tool · ARIA JSON receives ARIA JSON v1 UDP packets; it is not a general OSC, VMC or VTube Studio desktop WebSocket endpoint.

### Connect an iPhone

Put the phone and PC on a network that lets them communicate. In VTube Studio on the phone, enable 3rd Party PC Clients and use its IPv4 address in ARIA. The default phone request port is 21412 and the default PC receive port is 11125. Match any ports you changed in your setup, then click Connect tracking. ARIA sends subscription requests and listens for replies. Keep VTube Studio tracking your face.

### Connection controls and status

Connect starts the receiver; Disconnect releases it. Address and port fields are disabled while connected, so disconnect before editing them. Changing the source resets the receiver and tracking pipeline. A connected socket does not by itself mean that valid face data is arriving: check the face status, packet counters and age in Connection diagnostics & export.

If the model does not move, try Demo first, then check the sender address, phone setting, network isolation and Windows firewall access for ARIA on your intended network. Another app may already own the receive port. Do not assume forwarding ports on your router is necessary for local tracking. Missing, stale or face-lost input eases toward neutral rather than holding an old face indefinitely.

## network | IP addresses & UDP ports | The sender IPv4 identifies the phone or external tool. The phone request port and PC receive port are separate destinations, each from 1 to 65535.

### Address fields

iPhone IPv4 address means the phone's local IPv4 address, not the PC's address, a web URL or a public internet address. Allowed sender IPv4 limits accepted external packets to that sender. Use 127.0.0.1 for a tool running on the same PC. If DHCP gives a device a new address, disconnect, update the field and reconnect.

### Ports

Phone request port is where ARIA sends VTube Studio subscription requests; 21412 is the default. PC receive port is where ARIA listens for tracking packets; 11125 is the default. The JSON source only needs the receive port. Values must be whole numbers between 1 and 65535. Changing a port in ARIA does not reconfigure the sending tool or a firewall.

### Diagnose a connection

If Connect fails immediately, read the status text for an invalid address, unavailable port or socket error. If requests increase but valid packets do not, verify the phone's address and third-party client setting. Ignored packets can come from a different sender; rejected packets failed validation. Check both devices' network access and any firewall prompt. Disconnect before editing fields or retrying a different receive port.

## json | External tools: ARIA JSON packets | External tools can send UTF-8 ARIA JSON v1 tracking over UDP. Select the allowed sender IP, connect ARIA, and send to its PC receive port.

### Required packet fields

Required fields are version, face_found, rotation and blend_shapes. For a local sender, use allowed IP 127.0.0.1 and send each JSON object as one UDP datagram to 127.0.0.1:11125, or your selected receive port. Example:

{"version":1,"face_found":true,"rotation":{"x":4,"y":-10,"z":2},"blend_shapes":{"jawOpen":0.7,"eyeBlinkLeft":0,"eyeBlinkRight":0}}

Rotation uses degrees: X is pitch/up-down, Y is yaw/left-right and Z is roll/tilt. Blendshape names are case-insensitive after decoding and weights are clamped to 0…1. Missing weights act as zero. ARIA JSON uses its own schema; other tracking protocols need an adapter. This mode does not send phone subscription requests.

### Optional fields and validation

Optional fields include timestamp as unsigned Unix milliseconds, position, eye_left and eye_right as x/y/z objects, and hotkey as an integer. Omitting timestamp or using zero disables sender-order checking. Position and eye rotations retain sender coordinates; raw eye rotation is diagnostic while current gaze mapping uses blendshapes. The packet hotkey field does not automatically trigger ARIA's keyboard actions.

Packets are limited to 16 KiB, 128 blendshapes and names of 1–64 bytes; numbers must be finite. Unknown fields are tolerated. Only the configured sender IP is accepted, but source ports may vary. Check Connection diagnostics & export for rejected or ignored packets. For a local test, the bundled command aria-cli.exe send-json --target 127.0.0.1:11125 --seconds 120 generates a stream. Stop that tool when finished.

## calibration | Neutral calibration & movement | Calibrate while facing forward to define your neutral head rotation. Global mapping affects tracking before individual avatar bindings are evaluated.

### Calibrate neutral pose

Look naturally at the camera with your head centered, wait for a detected face, then click Calibrate neutral pose. ARIA records the current rotation as the center for future head motion. The button is unavailable without valid face tracking. Recalibrate after moving your phone or changing your seated position. Calibration does not rearrange the model's artwork, change its authored parameter limits or swap its axes.

### Mapping controls

Smooth ms filters the common tracking values. Head gain scales calibrated head rotation; Mouth gain scales mouth opening. Mirror movement reverses horizontal motion and swaps paired eye/brow tracking where appropriate. Individual Inputs bindings then choose which signal drives each avatar parameter. A binding can add its own range, response curve and smoothing after these global controls.

Reset mapping and calibration restores the common mapping defaults and clears the neutral rotation and pipeline state. It does not delete your per-parameter bindings, physics groups or presets. Use Save profile to keep the calibration and mapping for this avatar.

@diagram pipeline

## smoothing | Smoothing & response delay | Smoothing reduces jitter by easing toward new values. Larger millisecond values look steadier but add more lag; 0 responds immediately.

### Global and individual smoothing

Movement & calibration offers global Smooth ms from 0 to 300 ms. Each mapped input also has Smoothing ms from 0 to 500 ms. The individual filter is applied after its range and curve mapping. If both are high, their delays accumulate. A useful starting point is moderate global smoothing and little extra smoothing on responsive controls such as eyes or mouth.

### What milliseconds mean

The value is an exponential filter time constant, not a fixed delay or an animation duration. With a stable new target, a filter reaches about 63% of the change after one time constant. At 100 ms, most of the change takes several tenths of a second to settle. A 0 ms setting removes that filter's easing.

If motion feels late, reduce smoothing before raising gain. If it jitters, verify tracking quality and add smoothing in small increments. Physics momentum is separate: smoothing steadies a driving signal, while physics lets authored parts react with secondary motion. Frozen pose bypasses live animation, so changing smoothing will not make a frozen model move.

## gain | Head & mouth gain | Gain multiplies tracking intensity before avatar mapping. 1 is unchanged, below 1 reduces movement, and above 1 amplifies it within supported limits.

### Head gain

Head gain ranges from 0.1 to 3.0. It scales calibrated head rotation and related body movement in the common pipeline. For example, a 10-degree turn with gain 1.5 produces a 15-degree head target before limits and per-input mapping. If a small turn immediately reaches the model's maximum, reduce gain or widen that binding's input range.

### Mouth gain

Mouth gain also ranges from 0.1 to 3.0. It scales the mouth-open signal after mouth-close compensation. Increase it if you must open your real mouth too far to open the avatar's mouth; reduce it if the avatar is always wide open. The supported common mouth-open value is still bounded, and the avatar's own range may be different.

Gain cannot create deformation that the artist did not rig. If one custom parameter needs different sensitivity, edit its Input start / end and Output start / end instead of changing gain for the whole avatar. Save these adjustments with the avatar's profile or a movement preset.

## axes | Mirror, yaw, pitch & roll | Yaw turns left/right, pitch looks up/down, and roll tilts the head. Invert flips a direction; it does not exchange two axes.

V0.12 corrects VTube Studio vertical direction at the input boundary: wire Y becomes negative internal pitch. ARIA JSON is unchanged. Existing VTS neutral-pitch offsets migrate once. If you previously compensated with Invert pitch, review it and recalibrate while facing forward. Your custom per-model mapping ranges and inversion settings are preserved.

### Correct directions

ARIA reads head yaw from the tracking rotation's Y component and pitch from X. FaceAngleX normally drives the avatar's left/right head angle; FaceAngleY drives up/down; FaceAngleZ drives tilt. Tracking component names and model parameter names therefore do not use identical axis naming conventions.

Mirror movement reverses horizontal head, gaze and mouth motion and swaps paired eye/brow tracking. Invert yaw, Invert pitch and Invert roll independently reverse those directions. Mirror and inversion can cancel one another for a given axis. Test one setting at a time while moving only that axis.

### When looking up moves the model sideways

Open Inputs, find the affected head parameter and inspect its Source. Verify the avatar's authored behavior with a manual or held value. Map its sideways deformation to a sideways signal and its vertical deformation to a vertical signal. Inversion alone cannot repair a wrong source assignment. Custom avatars can use arbitrary parameter IDs; use the actual model's behavior rather than guessing from the name.

## avatar | Loading avatars & PNG puppets | Open a Live2D model3 manifest or moc3 export, or use an image puppet. Controls and physics are discovered from the loaded avatar's data.

### Live2D exports

Prefer Open Live2D avatar with the exported .model3.json file. It describes the .moc3 geometry, texture atlases and optional physics, expressions and display metadata. Keep the exported folder structure intact. Dropping a .model3.json or .moc3 onto Your stage adds a separate pinnable object and preserves the main avatar. Use Open Live2D avatar to change the main model. A bare .moc3 can be used with its correct textures, but cannot provide all manifest metadata by itself.

Select the official Windows x64 Cubism Core DLL in Cubism runtime setup before loading Live2D. ARIA does not bundle the proprietary Core or your model. A .cmo3 editor project is not the same as a runtime .moc3 export. Re-export from Cubism when needed. The model's own metadata and supported VTS profile assignments populate the rig; unrecognized controls remain available for manual mapping.

### Image puppets

Open PNG loads an image puppet; PNG, JPG and JPEG are supported by the picker. Use transparent PNG artwork for clean alpha edges. Set talking image adds an optional image that replaces the idle image when the common mouth-open value exceeds 0.18. Both images move with the head; this is not mesh deformation. Reset returns to the built-in Mica puppet without deleting your source files or saved avatar profiles.

### Studio zoom and model details

Zoom from 0.5 to 1.5 changes the studio preview. Each output has its own Model scale and position. Model details reports meshes, tracked assignments, decoded atlas memory and Core version. Configure avatar physics opens that avatar's discovered groups; Model parameters opens Inputs. A model can have many parameters without all of them having tracking assignments.

## runtime | Cubism Core DLL setup | Select Live2DCubismCore.dll from the official Native SDK's Windows x86_64 folder. This local runtime path is shared across avatars.

### Select the runtime

Use Select Core DLL or enter the complete file path. For the official Native SDK, the expected location is Core/dll/windows/x86_64/Live2DCubismCore.dll. Use the 64-bit Windows DLL, not the x86 build, a Unity library or a library for another OS. Then open the avatar again. The SDK download link opens your browser; the help text itself works offline.

### Files and compatibility

The Core DLL evaluates the exported model; the model's texture atlases and manifest must also be present. Changing the saved path does not copy the DLL or reinstall a runtime. Use a trusted official SDK and comply with its license. ARIA packages omit this DLL and private avatar assets. A moc3 created with a newer unsupported format may require a compatible Core release.

If loading fails, check the reported error, the DLL architecture, file availability and manifest references. The asset inspector checks referenced files but does not validate Core compatibility or prove that the model can render. Reload the avatar after changing runtime or asset paths.

## textures | Bare moc3 & texture atlas order | A bare moc3 needs its exported texture atlases in texture-index order. Prefer the matching model3 manifest whenever it is available.

### Supplying textures

Add texture atlases selects one or more PNG atlases. The list indices must match the model's exported texture indices: 0, 1, 2 and so on. Use Up and Down to reorder a selected file and Remove to take an incorrect entry out of this import list. Removing an entry does not delete the source image. Load avatar becomes available once textures have been supplied.

### Why order matters

Mesh UV coordinates refer to a particular atlas index. The wrong order can paint body parts with unrelated artwork even when every file loads successfully. An ordinary portrait image is not a replacement for an exported atlas. Keep original atlas sizes and content; do not crop or rearrange them.

If the avatar appears scrambled, verify index order against the exporter or open the matching .model3.json instead. A manifest also supplies references to physics, expressions and other model metadata. A bare import may therefore expose less information than the complete export. The dialog changes the active import only and does not rewrite the moc3 or generate a manifest.

## inspection | Model file inspection | Inspect model files lists referenced assets, sizes and file problems. It is a file check, not a render or moc3 compatibility test.

### Reading the report

Inspect Live2D .model3.json opens a report for that manifest. Type identifies the asset role, File shows its relative reference, and Result shows its size or a problem such as a missing file. Summary counts include textures, expressions and motions referenced by the export. Disk size is compressed file storage, not decoded RAM or VRAM.

### What it does not prove

The inspector does not load or draw the Cubism model, validate its internal moc3 data, or guarantee every optional file's runtime behavior. A manifest can list motions without the current ARIA UI offering a motion player. Use Open Live2D avatar for actual loading and rendering. Closing the inspector leaves your current avatar unchanged.

For missing assets, restore the expected exported folder structure or export the model again. Do not rename individual atlases without updating the manifest. The report is read-only and does not repair, copy or modify source assets.

## inputs | Input Monitor & parameter categories | Inputs maps live signals to avatar parameters. Pose holds values, Physics tunes secondary motion, Expressions toggles files, Presets stores variants, and Raw inspects tracking.

### Find a control

Search matches the parameter ID, its display label and its assigned source, ignoring letter case. Mapped inputs only hides parameters with no tracking binding. Turn it off to discover custom clothing, accessory and expression controls. Clearing search restores the unfiltered categories. Head & body, Eyes & brows and other categories are inferred from labels for navigation only; they do not alter mappings or restrict supported avatars.

### Expand and edit

Expand a parameter to choose its source, input and output ranges, clamps, smoothing, dead zone, response curve and step. Its number and progress bar show the current resulting value relative to the avatar's authored range. Click its question mark for a snapshot of that exact parameter's ID, limits, default and value. Changes affect the active avatar immediately; Save input configuration persists them locally.

### Trace unexpected movement

Check the source value first, then its mapping, then active expressions, holds and physics. A moving source does not guarantee visible artwork if the parameter is held, frozen, overridden or not used by the rig. Raw shows the received packet before ARIA's mapping; Inputs shows the model-facing controls. Use Demo to separate network issues from rigging issues.

@diagram pipeline

## parameter | Avatar parameter values & holds | Each avatar supplies its own parameter IDs, minimum, maximum and default. A parameter's visual effect comes from its rig, not its display category.

### Reading the context

The context card records the control you clicked, including authored limits and its value at that moment. These numbers use the model's parameter units; a name containing Angle does not guarantee physical degrees. Reopen the question mark to refresh the snapshot after editing or switching models. ARIA never guesses a custom control's visual meaning from its ID alone.

### Editing values

In Inputs, choose Manual to replace the tracking binding with a constant value. In Pose, Freeze all lets you edit every parameter while the model remains still. With partial manual pose mode, check Hold for only the controls you want to override. Unheld controls remain live. The slider follows the authored minimum and maximum and any configured step.

Changing a value has an effect only if the model's artwork uses it. Expressions and physics may target the same parameter; holds are reapplied to keep held values stable. To test an unfamiliar control, capture a frozen pose, move one slider within its allowed range, then Resume live. Save the result as a pose preset when you want to reuse it.

## source | Tracking source versus Manual | A source binding drives this avatar parameter from a named input. Manual removes that binding and uses a constant value instead.

### Choosing a signal

The source list includes ARIA's common signals, model-facing common parameters and inputs discovered from tracking. FaceAngleX is left/right head rotation, FaceAngleY is up/down and FaceAngleZ is tilt. Eye, mouth and brow signals use their own ranges. ARKit-prefixed names are blendshapes present in received tracking; a source absent from the current input set reads as zero.

Selecting a new source creates a fresh binding with that source's default range and the destination parameter's authored range. Review ranges, smoothing and curve again after switching sources. Source value is the current signal before that individual binding's mapping, although common signals may already be calibrated and globally smoothed.

### Common signal ranges

FaceAngleX/Y/Z start with −30…30 angle ranges. EyeLeftX/Y and EyeRightX/Y use −0.5…0.5 gaze ranges. MouthX, MouthPressLipOpen and individual BrowLeftY/BrowRightY use −1…1. Many one-sided signals such as eye opening, mouth opening and cheek puff use 0…1. Standard Param-prefixed sources use their defined common parameter ranges. These defaults are starting points; imported profiles or your edits may use different endpoints.

Breath is a generated smooth four-second 0…1 breathing cycle. AutoBlink supplies periodic brief eyelid closures. These are animation signals rather than measurements from the phone and can continue when a face is lost. ARKit-prefixed weights expose decoded blendshapes directly. VTS named-input behavior is approximated from the public tracking stream; it does not include VTube Studio's private filtering algorithms.

### Manual and held controls

Manual removes the tracking binding and starts its constant value at the current parameter value. It is useful for persistent accessory choices or testing a rig. Physics and expressions can still affect that parameter. For a guaranteed final value, use Hold in partial pose mode or Freeze all for a still image. Choose a source again to restore tracking, then save the input configuration.

## ranges | Input & output start / end | Input endpoints define the tracker motion span; output endpoints define the avatar values it becomes. Reverse output endpoints to invert only this binding.

@diagram mapping

### Input start and end

Input start corresponds to Output start; Input end corresponds to Output end. For a linear mapping from input −10…10 to output −30…30, input 0 produces output 0 and input 5 produces output 15. These examples assume no smoothing, dead zone, curve adjustment or clamping beyond the endpoints. Set input endpoints to a comfortable range of real motion, using Source value as a reference.

Input fields accept −1000 to 1000 in the source's units. Start and end must differ; equal endpoints would divide by zero, so the previous pair is restored. Reversing input endpoints also reverses the mapping. Narrowing the input span makes the avatar reach its output extremes with less real motion; widening it makes movement gentler.

### Output start and end

Output endpoints are constrained to the destination parameter's authored minimum and maximum. They may be reversed to invert direction or equal to create a constant target. Invert output swaps these endpoints without changing the selected source. A smaller output span limits how much the avatar moves; it does not change the artist's rig.

For example, map a 0…1 smile input to a custom parameter's 0…0.6 interval if the full expression looks too strong. Test both endpoints, midpoint and intermediate positions. Save input configuration or a movement preset when satisfied. Pose holds and active expressions may mask the result until you resume live or release them.

## clamps | Clamp input & clamp output | Clamps prevent the mapped value from extrapolating beyond chosen endpoints. The model's final authored limits still apply even with both clamps off.

### Clamp input

With Clamp input enabled, tracker values beyond Input start / end are limited to that interval before the dead zone and curve. For input −10…10, a raw value of 20 behaves like 10. This prevents tracking spikes or larger-than-expected motion from overshooting the selected mapping.

### Clamp output

With Clamp output enabled, the mapped target is limited to the smaller and larger of the two output endpoints, even when the endpoints are reversed. Disabling both clamps permits extrapolation from the mapping, but ARIA still clamps the final model value to its authored parameter range. It cannot make the model deform beyond its export limits.

Keep clamps on for predictable calibration. If a value seems stuck at an endpoint, inspect the source range before disabling clamps. An equal output start and end intentionally creates a constant value. A hold, frozen pose, expression or physics output can also determine the final value independently of these controls.

## dead-zone | Dead zone around the midpoint | Dead zone ignores a fraction of the input span on each side of its midpoint, reducing small center movements. It is a fraction, not degrees.

### What the number means

The slider ranges from 0 to 0.45. Zero disables the dead zone. For input −10…10, a dead zone of 0.10 ignores input between −2 and +2: 10% of the full 20-unit span on either side of the midpoint. That entire region maps to the output midpoint. The remaining regions are rescaled so the output endpoints remain reachable.

### When to use it

A small dead zone can steady a neutral head or gaze. Increase it gradually while watching Source value. It is centered on the arithmetic midpoint of Input start / end, which may differ from the avatar's authored default. Adjust your input endpoints or calibration if the desired neutral position is not there.

Avoid a large dead zone on controls where subtle motion matters. On a one-sided 0…1 mouth signal, the midpoint is 0.5, so this setting flattens the middle of the mouth motion rather than suppressing tiny openings near zero. Smoothing addresses jitter over time; a dead zone changes the mapping itself.

@diagram mapping

## curve | Response curve | Curve 1 is linear. Above 1 softens motion near the midpoint; below 1 makes the center more sensitive while retaining the endpoints.

### Shape the response

Response curve ranges from 0.1 to 4.0 and applies symmetrically around the normalized input midpoint after the dead zone. For a −10…10 to −30…30 mapping without a dead zone, input 5 gives output 15 at curve 1, but 7.5 at curve 2. Full input 10 still reaches output 30. Reversed endpoints work with the same response shape.

### Choose a value

Use values above 1 when small face movements feel too strong but you still want the full range when turning farther. Values below 1 emphasize smaller motions and may reveal jitter. Begin at 1, calibrate endpoints, then adjust the curve in small steps. It is not a speed or time setting; smoothing controls how quickly the mapped target is reached.

Curve, dead zone and endpoints are saved per binding and included in movement presets. A frozen pose or held parameter can hide live mapping changes. Resume live before judging the movement response.

@diagram mapping

## step | Stepping & continuous values | Step 0 keeps movement continuous. A positive step snaps final values to increments measured from the parameter's authored minimum.

### How snapping works

For an authored range −30…30 and step 5, available values include −30, −25, −20 and so on. A mapped value of 12 snaps to 10. Steps are in the parameter's own units, not degrees unless its rig uses degrees. A step does not have to divide the range exactly; values remain clamped to the authored limits.

### Where it applies

The Step editor is shared between Inputs and Pose for this avatar parameter. It affects live resulting values and manual/held edits. Positive steps can create deliberate stop-motion movement or switch-like controls. For fluid face tracking, leave Step at 0 or use a very small increment. A large step can look like tracking stutter even when packets arrive smoothly.

Drag the number to adjust it or edit its numeric text. The allowed step extends from zero to the parameter's span. Reset it to zero to restore continuous movement. Save input configuration or a preset to retain the change. Stepping cannot add rigged states that the model does not contain.

## pose | Pose mode, freezing & Hold | Freeze captures all final parameter values for a still image. Partial pose mode holds selected controls while other tracking and physics remain live.

### Freeze a complete pose

Enable Pose mode / manual inputs to capture the current pose. Freeze all animation for a picture keeps tracking, expressions, breathing and physics from changing the captured parameter values. Adjust the sliders to compose a still image. Capture current pose takes a new full snapshot. Resume live returns to live evaluation and resets motion filters for the transition.

### Hold individual controls

With pose mode active, turn off Freeze all animation to use partial overrides. Check Hold on each parameter you want to pin; uncheck it to release that control. A newly held parameter begins at its current value. Unheld controls continue responding to tracking, expressions and physics. The final Hold is reapplied after physics, so secondary movement cannot dislodge a held parameter.

### Save or export

Save pose as preset opens Presets, where Save pose stores a named frozen snapshot along with the rig settings and stage objects. Ctrl+Alt+P toggles freeze/resume when global hotkeys are enabled. Save transparent PNG exports the rendered avatar and visible stage objects without studio UI or the selected output background. Use output framing and OBS when you need a particular composition or canvas format.

If expressions seem unavailable or physics is still, check for POSE FROZEN before changing mappings. Frozen mode intentionally pauses live animation. Holds and frozen values belong to the active avatar's rig and can be included in its saved profile.

## png | Saving a transparent avatar PNG | Save transparent PNG writes the current avatar and visible stage objects with alpha, excluding studio controls and output backgrounds.

### Make a still image

Capture a frozen pose, adjust the desired parameters and accessory visibility, then use Save transparent PNG. Choose a destination and PNG filename in the save dialog. Canceling the dialog leaves the app unchanged. The export composes visible accessories with the avatar; it is not a screenshot of the studio window or an independently framed OBS canvas.

### Transparency and availability

The PNG preserves transparent pixels around the model. Live2D exports use the model render target dimensions, with a maximum edge of 2048 pixels. Image puppets and the built-in Mica preview export to a 1200 × 1400 canvas. Neither the compact preview size nor a selected output resolution changes this export. Accessories outside the export canvas are clipped. Use OBS and an output canvas for a different framing or resolution.

For a posed landscape or portrait composition, frame an output and capture that source in OBS instead. A frozen pose makes repeated image exports predictable. Check the app's status if the destination cannot be written or GPU readback fails; choosing a path does not guarantee a successful save.

## presets | Movement & pose presets | Presets store named configurations for the active avatar. Movement keeps tuning for animation; Pose also captures the final model values as a frozen picture.

### Create a preset

Enter a unique name of up to 80 characters, choose No hotkey or Ctrl+Alt+F1–F11, then click Save movement or Save pose. Movement saves current input bindings, manual values, steps, mapping, physics, active expressions and stage-object layouts. It clears full frozen mode for live movement, but partial Hold overrides remain part of the rig. Pose captures the current final parameter values and accessory visibility and freezes them. Up to 128 presets can be stored per avatar.

### Use and edit the library

Click a preset row to select it; selection alone does not apply it. Apply selected replaces the current rig configuration and mapping with the preset. Replace selected writes your current settings over that entry while retaining its movement/pose kind. Rename / assign key changes the selected entry's name and shortcut without replacing its saved tuning. Delete selected removes that saved entry; it does not delete the avatar or undo values already applied.

### Import and export

Export selected preset writes an ARIA JSON preset containing its model identity and rig configuration. Import preset validates compatibility with the current avatar and adds it to the library; click Apply selected to activate it. Duplicate imported names gain a suffix and imported hotkeys are cleared so they do not unexpectedly intercept keys. Source textures, moc3 data, external expression files, output layouts and connection settings are not embedded in the preset.

Use Save profile for the entire avatar workspace. Presets are intended for variants such as gentle movement, energetic movement or portrait poses, rather than transferring unrelated avatars' parameter IDs.

## hotkeys | Global hotkeys & conflicts | Global shortcuts work while ARIA is unfocused. Presets use Ctrl+Alt+F1–F11; Ctrl+Alt+P freezes/resumes; expressions accept a chosen modifier/key combination.

### Enable and assign

Enable global hotkeys (Windows) controls preset, pose and expression shortcuts together for the active avatar. Assigning a preset key or expression shortcut enables the switch. Disabling it releases registrations while retaining assignments; closing ARIA also releases them. Only the active avatar's registrations are used.

In the expression editor, choose any desired Ctrl, Alt, Shift and Win modifiers and a main key, then Assign shortcut. Press once to toggle on and again to toggle off. Clear shortcut removes the assignment, not the expression. Selecting modifiers alone does not save a new shortcut. F12 is reserved by Windows and is not offered.

### Conflicts and typing

ARIA rejects a shortcut already assigned to another active-avatar preset or expression, and reserves Ctrl+Alt+P for pose mode. Windows can also refuse a combination already registered by another application; read the registration error and choose another key. Unmodified letters or digits intercept ordinary typing when globally registered. Modifier combinations reduce accidental activation.

If a key does nothing, check the global switch, active avatar, registration status and frozen pose. Expressions do not change a frozen model. The tracking packet's hotkey diagnostic field is separate from these Windows keyboard shortcuts. Buttons remain usable if global registration is unavailable.

## expressions | Expression files, blending & toggles | Expressions apply values from this avatar's .exp3.json or .exp3 files. Toggle several independently, inspect their parameters, and assign global shortcuts.

### Library and selection

ARIA loads expressions referenced by the manifest, discovers supported expression files in the avatar export, and retains files you import explicitly. Search matches display names and file IDs. The checkbox toggles an expression; clicking its name selects it for details and shortcut editing. All off clears active toggles. Reload files rereads files already known to the library and resets their blending player; reopen the model to rediscover newly added files or import them explicitly.

### How values blend

Expressions fade in and out using their exported times. Blend percentage shows the current weight during that transition. Add contributes an offset, Multiply scales the underlying value, and Overwrite moves it toward a specified value. Multiple expressions are applied in the displayed file-ID order, so overlapping parameter changes can produce different results from using each expression alone. Final values remain within the avatar's authored limits.

### Interaction with the rig

Tracking and manual base values are evaluated before expressions. Holds and physics can affect the same destinations afterwards. A full frozen pose pauses expression changes; Resume live before toggling. A missing parameter is reported and skipped, so an expression exported for another avatar may load without visibly changing this one. The parameter list shows the actual exported IDs, blend modes and values.

Expression toggles, imported file references and shortcut assignments are saved for this avatar. Active expression IDs are also part of movement/pose presets. ARIA does not edit the source expression file when you toggle it or assign a shortcut.

## expression-files | Importing & reloading expressions | Import expression files adds compatible .exp3.json or .exp3 exports by local path. File errors and missing parameters explain why an expression may not work.

### Import and reload

Use Import expression files to choose one or more files exported for this avatar. ARIA reads and validates each supported file and avoids duplicate entries. The library is limited to 256 expressions per avatar. File contents are bounded in size to avoid loading an unexpectedly large document as an expression. Reload files retries the known library after you edit or restore those files.

### References, not copies

Imported external files are referenced at their local paths; they are not copied into the avatar directory or embedded in presets. Moving or deleting one causes a load error until it is restored or imported from a usable path. The selected expression's file ID identifies it, and hovering the ID shows the source path. Changes to exported fade times or parameter values require reloading the file.

Files needing attention lists parse or file-access failures. Missing parameters in an otherwise valid expression are shown separately and skipped. Verify the destination IDs against this avatar's Inputs list with Mapped inputs only disabled. A similarly named expression from a different model is not necessarily compatible.

## physics | Overall avatar physics | Physics lets authored particle groups turn motion into secondary movement such as hair or clothing sway. Overall tuning combines with each group's settings.

@diagram physics

### What appears here

ARIA reads groups from the loaded avatar's physics3 export. Groups and their input/output parameter IDs come from that file, so different avatars show different controls. With no physics export, this panel explains how to load one; ARIA cannot infer a complete particle rig from artwork alone. Supported VTS profile multipliers may be imported with the avatar.

### Overall controls

Enable avatar physics switches the simulation's effect on or off. Strength adjusts output amplitude. Inertia changes retained momentum, Response speed changes particle response, Gravity changes the restoring force, and Wind adds horizontal force. Overall and per-group multipliers combine, while wind offsets add. Tuning cannot invent missing deformation or a connection the artist never authored.

### Save and reset

Save physics settings persists the overall controls and group overrides with this avatar. Save as preset opens Presets so you can save a complete movement or pose variant. Settle motion resets simulation/filter state to remove accumulated movement and lets it settle again; it does not rewrite tuning. Reset overall restores imported/default overall values while keeping per-group overrides. Reset all groups restores group defaults while keeping overall tuning.

Try subtle head motion, tune one property at a time and compare before/after. Excessive strength can drive parameters to their limits. A full frozen pose pauses physics, and held output parameters cannot be displaced by it. If movement is absent, check pose mode, overall and group switches, strength, and authored inputs/outputs.

## physics-group | Individual physics groups | A group contains authored input parameters, a particle chain and output parameters. Its overrides affect only that group and are saved with this avatar.

### Find and inspect a group

Search matches group names, IDs and connected parameter IDs. Only modified groups hides groups with neutral tuning. Each card shows enabled state and counts of inputs, outputs and particles. Click the question mark beside a group for its exact connections and imported multiplier. Authored inputs & outputs lists which parameters drive it and which parameters it moves.

### Tune and combine

Enable the overall physics switch and the group's checkbox to make its sliders active. Tune group exposes Strength, Inertia, Response speed, Gravity and Wind. Overall settings multiply each group's corresponding settings; wind is additive. Two groups can target the same output, so changing one may not be the only influence on the artwork. Parameter holds and final authored limits still apply.

### Group actions

Reset this group restores its imported/default override. Enable all and Disable all change every discovered group's switch. Reset all groups restores the group settings, leaving overall tuning intact. Overrides for absent group IDs are retained but inactive, so replacing a physics export does not silently attach them to unrelated groups. Use Save physics settings after editing; the original physics3 file is not modified.

## strength | Physics strength | Strength controls how much a physics group's output contributes. Overall strength × group strength × imported output scale determines amplitude.

### Range and effect

Both overall and group Strength range from 0 to 2. A value of 1 keeps that multiplier neutral, 0 removes its physics contribution, and 2 doubles it before other multipliers and final limits. Overall 0.5 and group 1.5 produce a 0.75 multiplier before the avatar's imported scale. This changes amplitude, not the rate of simulation.

### Tuning advice

Reduce strength when hair or accessories hit their parameter limits abruptly. Increase it gently when authored secondary motion is too subtle. If increasing it produces no change, inspect whether the group receives moving inputs, is enabled, has usable output parameters, or is blocked by pose holds. A static pose cannot demonstrate secondary motion.

Settings are local to this avatar and can be saved in a movement preset. Reset this group returns its tuning to the imported/default values; Reset overall changes the global multiplier without deleting group overrides. The artist's output scaling and parameter range also affect the visible result.

## inertia | Physics inertia | Inertia scales retained particle momentum. Lower values settle sooner; higher values tend to swing longer, subject to the authored particle mobility cap.

### Range and interaction

Inertia ranges from 0 to 2 at both overall and group levels. A value of 1 keeps that factor neutral. The factors combine with the particle's authored mobility, which remains capped at 1. Increasing the slider does not guarantee proportionally longer motion once that cap is reached.

### Tune with movement

Move the head, stop, and watch how long the affected hair, ears or accessory continues to move. Reduce inertia if it appears to drift or oscillate excessively. Increase it cautiously for a softer follow-through. Response speed and gravity also influence the resulting motion, so change one setting at a time.

Use Settle motion after large edits when stored momentum makes comparisons difficult. Full pose freeze stops the simulation path. A group must be enabled and have authored connections to visibly react. Save physics settings or a movement preset to retain the tuning for this avatar.

## response | Physics response speed | Response speed scales the particle chain's authored response. Below 1 reacts more slowly; above 1 reacts faster. It does not change the FPS target.

### Range and combination

Overall and per-group response each range from 0.25 to 2.0. A factor of 1 preserves the authored response at that level. Their factors combine, so an overall value of 1.5 and a group value of 0.5 yield 0.75 of the authored response scale. The resulting motion still depends on particle delay, inertia and forces.

### Practical use

Use a lower group response for an accessory that should follow the head gently. Raise it when a group trails too far behind the driving movement. This is a physics tuning factor rather than a frame-rate control, input smoothing duration or playback speed for the whole avatar.

If the rig appears unstable or too sharp, return to neutral tuning, settle motion, and make smaller adjustments. Check the group's authored inputs and outputs to make sure you are tuning the intended part. Save physics settings or a movement preset when the behavior is right.

## gravity | Physics gravity | Gravity scales the restoring force toward the avatar physics rig's gravity direction. It changes the simulation, not the avatar's position on the canvas.

### Range and effect

Overall and group Gravity range from 0 to 2, with 1 as the neutral multiplier. The factors combine with the authored particle force. Lower values weaken that restoring force; higher values strengthen it. Zero removes this contribution, but other forces and particle constraints can still affect the chain.

### Judge the result

Observe both a resting pose and movement. A stronger restoring force can change how rapidly an accessory returns toward its authored resting direction, but the exact result depends on the rig's delay, mobility, radius and input normalization. It is not a universal real-world gravity value and has no meter-per-second units.

Use Settle motion to clear accumulated motion during comparisons. Wind adds a different, horizontal force. Output framing is independent: to move the entire avatar down the canvas, drag it in an output window instead. Save the physics settings with the active avatar.

## wind | Physics wind | Wind adds a horizontal force to the rig's authored wind. Overall and group wind add together; 0 adds no extra force at that level.

### Signed values

Each Wind slider ranges from −1 to +1. Negative and positive values push in opposite horizontal directions in the simulation's coordinates. The visible direction can depend on the rig's input/output reflection and how the artist connected parameters, so verify the result on the loaded avatar rather than assuming screen-left or screen-right.

### Combining settings

Overall wind 0.2 and group wind −0.2 cancel ARIA's additional wind for that group; any authored wind remains. Wind is additive, unlike Strength, Inertia, Response speed and Gravity multipliers. Use small values first to avoid pushing outputs against their limits.

Wind can make a rig lean or react even without strong tracking motion, but it does not create random gusts or new physics connections. Frozen poses and held output parameters remain fixed. Save physics settings to retain the effect or capture it in a named movement preset.

## outputs | Landscape, portrait & Freeform outputs | Open up to three independent previews: landscape 16:9, portrait 9:16 and resizable Freeform. Each has its own framing, background and OBS resolution.

### Open and select

Open Landscape, Open Portrait and Open Freeform toggle independent native windows; all three can be active together. Edit selects which canvas the controls below affect. Selecting a canvas does not open or close it. Its settings are restored per avatar, while window-open state is temporary for the current session. Closing a preview stops that output's sender; minimizing it keeps the sender running.

### Keep previews small

Each output has a full-resolution canvas texture and a smaller desktop preview. Set OBS canvas resolution for recording detail, then use Preview size or Freeform window width/height for desktop space. Resizing a Freeform preview does not resize its OBS canvas. The preview fits the canvas without stretching; mismatched aspect ratios leave unused space around it.

### Choose a capture path

Enable Send full resolution to OBS (Spout), install the separate OBS Spout2 plugin, then add a Spout2 Capture source with the matching ARIA sender. Ordinary OBS Window Capture records only the visible small preview size. Use the full-resolution Spout path when you want a large recording canvas without a screen-filling window.

@diagram capture

## spout | Full-resolution OBS capture | Spout shares the full canvas texture with OBS on Windows. Use the separate Spout2 Capture plugin and keep ARIA and OBS on the same GPU.

### Set up OBS

Open the desired output and enable Send full resolution to OBS (Spout). Install the OBS Spout2 plugin using the provided release link, restarting OBS if required by the plugin. In OBS, add Spout2 Capture and select ARIA Landscape, ARIA Portrait or ARIA Freeform. If another ARIA process owns a name, the live status may show a PID suffix; select the actual name in OBS. Copy sender copies the standard name shown in the controls.

### Resolution and alpha

The sender uses OBS canvas resolution even if the desktop preview is small or minimized. Window Capture cannot obtain this larger texture from the small preview. Keep both programs on the same graphics adapter. For a Transparent background, enable transparency in the source and use premultiplied alpha if offered. A color-key background instead needs a matching OBS Chroma Key filter.

### Missing or stalled source

Check the output's live status text, open switch and Send switch. Retry OBS output recreates sender resources after a failure; it may briefly interrupt all active senders. The Windows DirectX 12 renderer supports ARIA's current Spout bridge; a different backend may report that sharing is unavailable. Keep the output open while using it. Closing it releases its capture texture and sender, while minimizing preserves full-resolution frames.

## resolution | Canvas resolution & resource cost | Canvas resolution sets the actual pixels sent to OBS. Preview size controls desktop space separately; increasing resolution raises GPU work and memory use.

### Fixed aspect ratios

Landscape stays 16:9 and portrait stays 9:16. Choose a resolution from the menu; supported long edges are 640, 960, 1280, 1920, 2560 and 3840 pixels. A 1920 long edge means 1920×1080 in landscape and 1080×1920 in portrait. Read the complete dimensions shown in the menu rather than treating the long edge as width in both formats.

### Freeform canvas

Freeform accepts an independent width and height from 64 to 4096 pixels each. These values define the sender texture and can differ from the preview window's dimensions. Resizing the window does not alter them. The preview fits the chosen canvas aspect without deforming the avatar.

### Performance tradeoffs

Doubling both dimensions creates four times as many pixels. Larger canvases and several simultaneous outputs increase GPU memory and compositing work. Start with the actual resolution you need in OBS; a larger canvas cannot restore artwork detail absent from the model textures. Resolution edits are briefly debounced to avoid reallocating GPU textures on every drag increment. Save output layouts retains the choice for this avatar.

## background | Studio, color key & transparency | Each output can use a studio background, an opaque custom color key, or transparent pixels. Choose the method that matches your OBS source.

### Three modes

Studio includes ARIA's studio-style background in that canvas. Green screen / color key fills the background with the selected RGB color; despite its name, the color can be any valid hex key. Transparent keeps alpha around the avatar so a compatible capture source can composite it directly over other content.

### Which to choose

Use Studio when you want its backdrop included. Use Transparent with Spout2 Capture for alpha without removing a color from the artwork. Enable transparency and premultiplied alpha in the OBS source if offered. Use a color key when your capture workflow expects an opaque background and an OBS Chroma Key filter. Desktop Window Capture transparency varies by capture method.

The background is saved independently for each output and each avatar. Changing it does not recolor the avatar's textures or change the PNG export background. Check translucent hair and clothing edges in your actual OBS composition; the desktop preview alone does not demonstrate how OBS is configured.

## chroma | Hex color key & automatic detection | Enter a six-digit RGB hex color or detect a color separated from the avatar's artwork. OBS must use the same custom key color to remove the background.

### Choose and apply a color

Use the color swatch picker or type a six-digit value such as #00FF00 and click Apply hex. The hash prefix is optional. Each pair represents red, green and blue from 00 to FF. The field is a draft until Apply succeeds; invalid text leaves the previous color unchanged. Copy hex copies the applied key rather than an invalid draft.

### Detect safer color

Detection scans nontransparent atlas colors, including hidden artwork, and the current rendered Live2D model. PNG puppets include both idle and talking artwork; Mica has a built-in palette. ARIA searches saturated colors for the greatest chroma separation from the sampled artwork and applies its best candidate. This can be more expensive than a normal frame, so it runs only when requested.

### Match OBS and inspect edges

In OBS, add a Chroma Key filter, select Custom, paste the same hex color and begin with low similarity. Increase carefully while checking hair, eyes and clothing. Color separation is a best-fit suggestion, not a guarantee: translucency, antialiasing, later color effects and high filter similarity can still remove model detail. Recheck after changing artwork or expressions. A warning means no clearly separated candidate was found; consider Transparent with Spout instead.

## framing | Moving, scaling & locking an output | Drag the avatar to move it, scroll over the canvas to scale it, and double-click the avatar to center it. Each output keeps independent framing.

### Mouse controls

Start a primary-button drag on the avatar's bounds to move it. Wheel scrolling over the canvas changes its size between 0.25× and 3×; scrolling over a letterboxed area outside the actual canvas does not zoom. Double-click the avatar to center it. Position offsets are normalized to the canvas so they remain meaningful when its resolution changes.

### Framing controls

Model scale sets the same scale as the wheel. Center model resets position while preserving scale. Reset framing restores centered position and 1× scale. Lock model framing blocks drag and wheel interactions to protect a composition; explicit controls in the studio can still change its framing. Keep this output on top affects only that preview's desktop stacking order, not OBS resolution or process priority.

### Recovery and persistence

If the model is out of view, use Center model or Reset framing from the studio controls. It is possible to move part of the avatar off the canvas intentionally. Output framing is independent from Avatar & appearance's studio Zoom. Edits are saved per avatar with a short debounce; Save output layouts requests an explicit save. Output windows contain no help icons or control overlays, keeping capture artwork clean.

## preview | Compact preview & Freeform window size | Preview dimensions determine how much desktop space an output uses. They do not lower the full canvas resolution that Spout sends to OBS.

### Fixed previews

Landscape and portrait offer compact preview sizes with the same fixed aspect ratio as their canvas. These dimensions describe the preview's client area in physical pixels; Windows borders and the title bar add a little space. A small preview is convenient for framing, while OBS can still receive a much larger canvas.

### Freeform window

Set Freeform window width and height from 160 to 1600 pixels in the studio or resize the native window directly. The canvas is fitted inside it without stretching. When the window and canvas aspect ratios differ, empty preview margins are expected and do not become extra pixels in the Spout canvas.

Minimize a preview to keep the full-resolution sender active without occupying your desktop. Close it to stop that output and release its sender texture. Ordinary Window Capture sees only the small preview; choose Spout2 Capture for the independent full-resolution canvas. Changing a preview's size does not change the avatar's per-output scale setting.

## performance | FPS target & rendering efficiency | FPS target limits model simulation and rendering. Lower targets save work; higher targets need more CPU/GPU capacity and do not increase the phone's tracking rate.

### Choosing a target

Choose 30, 60 or 120 FPS. Start at 60 for smooth motion and reduce to 30 if resource use is too high. A 120 target is useful only if your hardware and capture pipeline can sustain it. This is a target, not a guarantee. The footer reports measured model FPS separately from the selected limit.

### Where work goes

Large avatar texture atlases use GPU memory even when the preview is small. High canvas resolutions and several simultaneous outputs add compositing work. ARIA limits simulation independently of UI events, reuses buffers, skips unchanged model updates and caches static canvas results. A frozen pose can therefore reduce model work even though UI windows remain responsive.

If FPS is low, inspect CPU, RAM and VRAM, reduce output resolution or close unused outputs, and check other GPU-heavy apps. Smaller preview windows mainly save desktop space; they do not shrink the full-resolution sender. High process priority addresses CPU scheduling contention and cannot fix a saturated GPU. Save profile retains the FPS target for this avatar.

## priority | Windows High process priority | High gives ARIA CPU scheduling preference over normal-priority apps during contention. It may reduce scheduling hitches but cannot guarantee smooth rendering.

### Enable or disable

The switch sets this ARIA process to Windows High priority and reports whether the change succeeded. Turning it off restores Normal. This preference is saved for ARIA on this PC, not separately for each avatar. If Windows refuses the change, the UI restores the previous selection and shows the error.

### What it can help

When multiple programs compete for CPU time, scheduling priority can give ARIA more opportunity to run. It does not increase hardware speed, reserve a CPU core, raise GPU priority or fix missing tracking packets, disk stalls, GPU overload or thermal throttling. Check the FPS and resource counters before and after changing it.

High is the strongest priority offered here. Realtime is not offered because it can starve Windows, input processing and OBS. Turn High off if other programs become less responsive. You can often improve overall streaming performance more effectively by choosing an appropriate FPS target, closing unused outputs and avoiding oversized canvases.

## metrics | CPU, RAM, VRAM & frame counters | The bottom bar reports ARIA's process usage and rendering-adapter memory once per second. N/A means unavailable, not zero usage.

### CPU and frame timing

CPU is ARIA's process CPU time normalized across available logical processors. 100% means all available processors are busy with ARIA; one saturated thread on a many-core CPU can show a much smaller percentage. Model FPS measures model update cadence. UI work is CPU time spent in the application update, not full GPU frame latency or a measurement of OBS encoding.

### RAM

RAM is ARIA's resident working set in MiB. The hover detail also shows private committed memory, which can include pages not currently resident. MiB means 1,048,576 bytes. Texture decoding, model data and native libraries contribute to memory use. Compressed files on disk can be much smaller than their decoded memory footprint.

### VRAM

VRAM reports this process's local GPU memory use on ARIA's rendering adapter via DXGI. Hover details include the budget and shared/non-local memory. The counter includes driver allocations; integrated GPUs may use system memory for local GPU allocations. It is not the entire GPU's utilization percentage or other applications' memory. N/A means the backend or driver cannot report the counter.

Large atlases and output textures can consume significant memory even with small preview windows. Compare counters while opening or closing outputs or changing resolution. The once-per-second sampling avoids expensive continuous polling, so brief spikes may not appear and numbers can lag a recent action.

## stage | The studio stage | The center stage previews the active avatar and connection state. Independent output windows provide movable, scalable compositions for OBS.

### What you see

The stage displays Mica, your PNG puppet or the loaded Live2D model with the current tracking and rig settings. Avatar & appearance's Zoom affects this studio view. The selected output's background can be reflected in the studio preview, while its framing remains independent. The connection status distinguishes demo movement from live tracking conditions.

### Studio versus outputs

To compose landscape, portrait or Freeform content, open that output under Capture & performance. Its canvas supports dragging the avatar, mouse-wheel scaling and double-click centering. The studio is a working interface, so capturing it includes controls; a dedicated output or Spout sender provides the clean composition.

If the model is static, check Demo/live source, valid face data, pose freeze and holds before changing the renderer. If a loaded avatar appears incorrect, review its texture order, file warnings and parameter assignments. Opening a help window leaves the stage and tracking running.

## diagnostics | Raw tracking, connection counters & export | Raw shows received tracking before avatar mapping. Diagnostics separates valid, rejected and ignored packets; Export mapped values writes a final parameter snapshot.

### Raw tab

Head values show decoded rotation in degrees before neutral calibration and avatar mapping: X is pitch, Y is yaw and Z is roll. The VTube Studio adapter converts the phone's wire axes into this convention, so these are not a byte-for-byte dump of its Rotation fields. Position is shown in the sender's coordinates; ARIA does not promise that every source uses the same physical units. Blendshape meters show named received weights, generally in 0…1. The hotkey value is a packet field and is separate from ARIA's Windows global shortcuts.

### Connection counters

Valid packets passed decoding and sender checks. Rejected packets failed validation. Ignored packets were not accepted for the configured source/sender. Requests counts outgoing subscription requests. Packets per second indicates input arrival rate, not render FPS. Last valid packet age uses the PC's monotonic clock so it remains meaningful even if the phone's timestamp differs.

### Export mapped values

Export mapped values writes a JSON object of parameter IDs and their current final values for inspection or another tool. It is a single snapshot, not a recording, preset or full configuration backup. Choose Import/Export preset under Presets for a restorable movement or pose configuration. File export errors appear in the app's status text.

When troubleshooting, compare Raw, Source value and the final parameter. This identifies whether motion was absent at the sender, changed by mapping, or overridden by expressions, holds or physics. A healthy network can still produce a frozen picture when pose freeze is active.
