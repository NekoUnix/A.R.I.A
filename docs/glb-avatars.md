# VRC / GLB avatars — Experimental

Available in **v0.34 Alpha**. Import a
skinned humanoid `.glb` directly into ARIA. No Cubism library or Unity runtime is
required. The GLB is read in place; ARIA saves its settings separately per avatar.

## Import your avatar

1. Open **Workspace → Profiles → + Add avatar…**.
2. Choose **VRC / GLB avatar**, then **Choose GLB avatar…**. You can also drop the
   GLB on the stage to open the guide, or pass its path when starting ARIA.
3. Choose **Review import**. Check the detected bone, shape and material counts.
4. Select **Import GLB avatar** and wait for mesh/texture preparation. Your current
   avatar stays available until import succeeds; Cancel leaves it in place.
5. In the Inspector, open **Avatar → View → GLB rig & tracking**. Review the detected
   assignments before streaming. **Model details** lists import limitations.
6. Choose **Save profile**. Stage position, tracking, expressions, mappings and
   other settings stay with this avatar. Loading another model does not replace them.

Use a GLB exported with **armature, skin weights, shape keys and embedded PNG/JPEG
textures**. A renamed FBX, `.unitypackage`, `.vrca` or Unity prefab is not a GLB.
Ask the author for a permitted GLB export if those are the only files provided.
The avatar author's usage terms still apply.

## Connect the tracking you already use

GLB avatars share the same pipeline as ARIA's other avatars:

```text
Webcam / RTX / VTube Studio / iFacialMocap / microphone
                         ↓
       Calibration, axis correction and input ranges
                         ↓
             GLB bones and facial shape keys
                         ↓
      Your stage → combined OBS preview / native output
```

1. Open **Tracking** and keep or choose your usual source. Follow [webcam setup](webcam.md),
   [iFacialMocap setup](ifacialmocap.md) or the [main tracking instructions](../README.md#first-time-setup).
2. In **Profiles**, use each avatar's own tracker or follow another profile's
   tracking, just as with Live2D/VRM. [Profile guide](profiles.md).
3. Click **Set up personal tracking…** in GLB rig & tracking. Capture neutral,
   turns, nods, tilts, eyelids and mouth using the existing [guided calibration](tracking-setup.md).
4. Test one motion at a time. Check the numeric Inputs values if the source is
   connected but a part of the model stays still.

| Motion | Automatic assignment | How to adjust |
| --- | --- | --- |
| Head turn, nod and tilt | Detected head and neck; shared calibrated head parameters | Humanoid bone assignments; Tracking axis correction/ranges |
| Eyes looking around | Detected left/right eye bones | Assign those bones; use Inputs to bind exported look shapes if needed |
| Talking | Prefer `vrc.v_aa`, then named jaw-open/mouth-A shapes | Face input assignments → Mouth open; Inputs for range and smoothing |
| Blinking | Separate named left/right close shapes when present; otherwise a combined blink shape | Face input assignments → Left/Right eyelid; eye opening is inverted to closure |
| Smile and brows | Recognized named shapes | Face input assignments or Inputs |
| Additional ARKit shapes | Matching named shapes such as cheek puff or tongue out | Inputs; requires a tracker which supplies that channel |
| Resting body and gestures | Detected humanoid bones | Life & idle movement, Gesture animations and Pose controls → Relax arms |

Microphone input drives mouth **opening**, not a phoneme classifier. ARIA does not
invent all 15 VRChat visemes from microphone volume. Other exported visemes remain
available for expressions, input bindings and actions. Extra phone shapes only
move when the current source actually supplies those values. A model with only
a jaw bone can use mouth opening to rotate that bone when no mouth shape is bound.

## Correct a rig inside ARIA

**Humanoid bone assignments** recognizes common Unity, Blender and Mixamo names,
including namespaces and `_L`/`_R` suffixes. It avoids twist bones and ambiguous
duplicate names. For each body part, choose **Auto detect**, a named node, or
**Disabled**. Node numbers distinguish repeated names. If head/hips are unrecognized,
the avatar still loads so you can assign them here; only mapped bones receive motion.
Changes and disabled entries are saved per profile and in movement/pose presets.
If the export faces backward, use **Reverse avatar forward direction**. This corrects
the tracking coordinate basis as well as the view; Camera orbit only changes your view.

**Face input assignments** lets you replace the basic mouth, eyelid, smile and
brow targets. **Edit all shape inputs…** opens the normal Inputs controls, including
custom source selection, ranges, inversion, smoothing, steps and held values.
This supports controller channels and imported tracking equations too.
**Restore detected face inputs** replaces your shape bindings with the automatic
ones; **Restore detected bones** clears only the bone overrides.

Only one default mouth-open shape is chosen per mesh to avoid applying both a
viseme and jaw-open deformation at full strength. All exported shape keys remain
editable. Their labels include the mesh name. Authored starting weights are retained;
you can lower a shape to zero as well as increase it.

Use **Expressions** to combine shapes, the [shortcut recorder](actions.md#record-a-shortcut)
to assign keys, and [action nodes](actions.md) to link expressions, movement presets,
gestures, effects and profile changes. **Pose → Freeze** holds a shot. Existing
PNG/moc3 objects can pin to the GLB surface and follow deformation, using the same
3D pinning implementation as VRM. OBS framing and per-avatar locking also apply.

## Flexible movement and lighting

Open **Avatar → Spring physics** for Medium defaults, individual group controls
and the searchable **Find secondary bones → Bone inspector**. Breast/body bones,
hair and accessories receive automatic physics guesses; review unfamiliar names
with Auto, Simulate and Keep rigid. Weighted single bones and unweighted chain
endpoints are supported. [Physics guide](secondary-motion.md). Use **Avatar
lighting** for color, direction or even light. [Lighting guide](lighting.md).

**Spring physics → Body & self collision** now generates body capsules and moving
flexible-part envelopes by default. Adjust body size and spring thickness and
watch the contact counter. These are approximate bone collisions; exact cloth
surfaces and rigid arm/hand poses can still intersect. See the
[collision controls](secondary-motion.md#body-and-self-collision).

## Compatibility and repair

This imports the **GLB export**, not a running VRChat avatar. GLB contains standard
geometry, skins and morph targets; it does not contain Unity avatar descriptors,
VRChat expression menus, FX controllers or PhysBone components. Rebuild combinations
using ARIA expressions, presets and actions. The provided ICHIGO export includes
no authored spring groups or animation clips. ARIA now generates suitable breast/body,
hair/tail/accessory chains from the rig with Medium defaults. This is an experimental ARIA
simulation, not a reconstruction of Unity PhysBone settings. See [secondary motion](secondary-motion.md).

Exported base-color textures, alpha and emission use ARIA's lighting. Unity custom
shaders, metallic/roughness appearance and material animations are not reproduced
exactly. Embedded glTF animation clips are currently reported but not played. These
limitations appear in Model details. Source conventions: [glTF specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html),
[VRChat lip-sync setup](https://creators.vrchat.com/avatars/creating-your-first-avatar/),
[VRChat expression controls](https://creators.vrchat.com/avatars/expression-menu-and-controls/).

| Problem | Repair |
| --- | --- |
| Static prop rejected | Export the avatar's armature and skin weights. Static GLB props belong in Throws & sprays. |
| Tracking values move but face/body does not | Assign bones/shape targets in GLB rig & tracking. Confirm Live mode rather than a frozen or held pose. |
| Arms intersect clothing | Adjust Relax arms; reduce idle/gesture strength. This is not full-body capture or collision avoidance. |
| Only one breast/body chain moves | Update ARIA for the small-rotation fix. In Spring physics, inspect both branches for Keep rigid, disabled groups or zero strength/bend. Each side has independent saved tuning. See [secondary motion](secondary-motion.md#if-hair-stays-rigid-or-the-wrong-thing-moves). |
| Texture missing or unsupported compression | Re-export as an uncompressed GLB with one embedded binary buffer and PNG/JPEG textures. External URLs/files and required Draco/meshopt/KTX extensions are rejected with an error. |
| Render differs from Unity | Export a base-color/alpha material approximation. Shader-only effects need an exported equivalent. |
| Need help | **Diagnostics / export logs** includes avatar format, detected/overridden bones, morph counts and warnings. Artwork is not included. [Support guide](diagnostics.md). |

Limits: 512 MiB file, 32 MiB JSON metadata, two million unique vertices, six million
triangle indices, 1,024 morphs per mesh and 2,048 total shape controls. Aggregate morph
data remains bounded at 32 million vertex/target entries. Textures are limited to
16,384 pixels per dimension and 2 GiB decoded combined. Lower the avatar canvas
resolution in View if GPU usage is too high. Large morph counts do not cause every
morph to upload each frame: only meshes with changed weights are rebuilt.

## Developer validation

Synthetic fixtures cover ambiguous bone names, large morph sets, starting weights,
static-prop rejection, common tracking inputs, controller remapping and saved mappings.
Windows GPU acceptance uses the supplied private `ICHIGO_v1.04.glb` in place:
23 bones, 742 shapes, 72,032 unique vertices and 29 draw sections. Head movement,
facial changes, gestures, expression hotkeys and frozen-pose restoration passed.
This checks ARIA's tracking-to-render path, not a live session with every physical
camera/phone. macOS/Linux device acceptance remains to be performed.

From a configured Windows Rust development shell:

```powershell
.\scripts\test-glb.ps1 -Glb 'D:\My avatars\avatar.glb'
```

The test reads the asset in place. Private GLBs and rendered artwork are not
committed to this repository.
