# Live2D avatar import on Windows

ARIA v0.6 evaluates real `.moc3` models through Cubism Core and renders their
ArtMeshes with wgpu/Direct3D 12. You need **the model, its texture images, and the
official Cubism Core DLL**. A moc3 contains the rig, not the texture artwork.

## 1. Install the runtime

1. Visit the official [Cubism SDK for Native download page](https://www.live2d.com/en/sdk/download/native/),
   review its terms, download the SDK, and extract it to a stable folder such as
   `C:\Tools\CubismSdkForNative-5-r.5`.
2. Run ARIA. In the left **AVATAR** section, expand **Cubism runtime setup**.
   You may need to scroll down the controls panel.
3. Click **Select Core DLL…** and select:

   ```text
   C:\Tools\CubismSdkForNative-5-r.5\Core\dll\windows\x86_64\Live2DCubismCore.dll
   ```

Use **x86_64**, not x86. Select the `.dll`, not a `.lib` in `Core/lib`. ARIA loads
the DLL directly; no C++ wrapper, SDK compilation, Unity, or desktop VTube Studio
is required. The DLL path is saved in ARIA's local preferences. Changes take effect
on the next avatar import. Keep the SDK folder in place.

The SDK used for local verification was Native **5-r.5**, whose Core reports
**6.0.1**; SDK and Core version numbers are different. The loader supports both
the older drawable render-order API and the newer combined render-order API.
It requires the Core consistency-check and multiply/screen-color APIs. Prefer the
current official SDK when an older DLL reports missing symbols or a newer moc version.

Core is executable native software. Select an official SDK DLL, not an arbitrary
DLL received with an avatar. ARIA only loads the explicitly selected absolute path;
it does not search the model folder for executable code. Neither Core nor licensed
model artwork is included in ARIA's ZIP or MIT source license.

## 2. Open the avatar

Keep the exported folder structure intact. A typical export looks like:

```text
MyAvatar/
  MyAvatar.model3.json
  MyAvatar.moc3
  MyAvatar.4096/
    texture_00.png
    texture_01.png
  MyAvatar.physics3.json
```

Select **Open Live2D avatar…** and choose `MyAvatar.model3.json`. Alternatively,
drag it onto the ARIA studio window. The manifest supplies the exact texture index
order. ARIA does not copy, modify, or upload your model files.

Selecting/dropping `MyAvatar.moc3` also works: ARIA searches the same folder for a
single `.model3.json` that references that moc3. The filenames do not need to match.
If multiple manifests reference it, open the desired manifest explicitly.

**A bare moc3 without a manifest:** the import dialog offers **Add texture atlases…**.
Select the atlas PNGs, then use **Up / Down** to put them in the model's index order
(0, 1, 2, …). Review the numbered list and click **Load avatar**. File-picker ordering
is not assumed to be correct. ARIA detects missing atlas indices, but cannot infer
whether you supplied the correct artwork in the correct order.

Successful imports show the avatar name, mesh count, mapped parameter count, Core
version and decoded atlas size. The studio badge changes to **LIVE2D / CUBISM**.
Loading errors preserve the current avatar and appear in the controls panel.
**Reset** unloads the avatar and restores Mica. **Open PNG…** switches to a flat puppet.
Opening another Live2D avatar replaces the current one after loading succeeds.

## 3. Animate it

**Demo** animates the model immediately. The existing [iPhone VTube Studio setup](tracking.md#iphone-vtube-studio)
feeds the same rig: choose the phone source, enter the phone address, connect and
calibrate. No desktop VTS WebSocket authentication token is involved.

ARIA looks for `<moc filename stem>.vtube.json` beside the manifest and imports
its tracking assignments automatically. Keep this file when moving a model from
VTube Studio. Supported input names include face angles, individual eyes, brows,
mouth open/smile, MouthX, MouthFunnel, MouthShrug, MouthPressLipOpen, MouthPucker,
CheekPuff, and automatic breathing/blinking. Custom output IDs use the saved
input/output ranges, including reversed ranges, clamps and smoothing amounts.
Warnings identify unsupported inputs or missing rig parameters.

Without a profile, the 12 [standard parameters](tracking.md#mapping) bind by ID,
with additional body Y/Z and breathing bindings where present. Other parameters
keep their exported defaults. Values always clamp to the native rig's limits.

Click **Model parameters…** to open **Input Monitor → Inputs**. Search IDs or names
from `.cdi3.json`, edit ranges/clamps/smoothing/stepping/curves, or choose **Manual**.
Raw `ARKit:` inputs are also available. Edits save with this model and override its
initial VTS assignments when reopened; the original profile is never modified.
**Pose** holds individual parameters or freezes the complete avatar for PNG export.
**Presets** saves movement configurations and screenshot poses with Windows hotkeys.
See the [input controls guide](input-controls.md). **Expressions** discovers and
plays the avatar's `.exp3.json` / `.exp3` files with per-model custom shortcuts;
see [expressions](expressions.md). VTS hotkey assignments, VTS's active expression
state and items are not imported. Calibration, global smoothing, gains and mirroring
precede binding.

These are ARIA filters over raw tracking, not an exact recreation of VTS's
proprietary face processing. VTS smoothing amounts become a time constant of
4 ms per unit (0–100). See [input formulas](tracking.md#model-specific-inputs).

### Secondary motion

The manifest's `.physics3.json` loads automatically. ARIA evaluates its authored
weighted inputs, normalization, particle lengths, delay, mobility, acceleration,
output scale/reflection and weight. Chains retain momentum: a head turn pushes
hair/ears/tail/clothing, which follow and settle. The authored FPS sets a fixed
simulation step with interpolated inputs and outputs, independent of UI FPS.
Later groups can use earlier groups' outputs. Automatic breathing continues at
rest, including when tracking is lost, and feeds physics where the rig connects it.

Use **Input Monitor → Physics** for global and individual group enable, strength,
inertia, response speed, gravity and wind. Groups and their names come from each
avatar's export. **Settle motion** clears momentum. To hold a physics output, use
**Pose** controls; physics otherwise runs after tracking. Physics controls persist
with each model and are included in movement/pose presets. See [the physics guide](physics.md).
The profile's physics enable flag and per-group strength multipliers are honored;
VTS's global strength/wind/dragging settings and legacy solver mode are not reproduced.

Models need an authored physics file for secondary motion. ARIA does not guess
which custom rig IDs mean hair or clothing. Missing/invalid files produce a visible
warning and leave the avatar usable. Bare moc imports have no physics/profile metadata.

Open **A.R.I.A. Output** for OBS as described in the [Windows guide](windows.md).
The studio and all output windows share the rendered avatar texture and live parameter state.
Use green screen plus OBS Chroma Key when your capture method does not retain alpha.

## Command-line launch

You can set the DLL for the current PowerShell session and pass the model path:

```powershell
$env:ARIA_CUBISM_CORE = 'C:\Tools\CubismSdkForNative-5-r.5\Core\dll\windows\x86_64\Live2DCubismCore.dll'
.\aria-desktop.exe 'C:\Avatars\MyAvatar\MyAvatar.model3.json'
```

From source:

```powershell
cargo run --locked --release -p aria-desktop -- 'C:\Avatars\MyAvatar\MyAvatar.moc3'
```

`ARIA_CUBISM_CORE` takes precedence over the saved DLL path. Launching with a model
argument does not automatically connect to a phone. The app builds and its demo
works without the SDK installed.

## Compatibility and limits

| Supported | Details |
| --- | --- |
| Native rig evaluation | Checked moc version, consistency validation, aligned model storage |
| Mesh rendering | Ordered triangles, atlas UVs, opacity, single/double-sided culling |
| Clipping | Multiple mask sources, regular/inverted masks; invisible mask sources still contribute texture alpha |
| Colors/blending | Multiply/screen colors; normal, legacy additive and multiplicative blending |
| Tracking | VTS profile import, named/custom/raw inputs, editable ranges, manual controls |
| Physics | Version 3 particle chains; authored FPS, weights, normalization, reflection and per-group multipliers |
| Input limits | 2 MiB manifest; 128 MiB moc; 1–32 atlases; each atlas at most 8192px (also limited by the GPU) and 128 MiB compressed; 1 GiB total decoded atlas storage |
| Render target | Transparent offscreen texture, longest side 2048px, shared with studio/output; UI zoom scales it |

**Not yet implemented:** `.motion3.json` playback,
`.exp3.json` playback, `.pose3.json` processing, motion events/audio, model3 layout
overrides, VTS item attachments, or Cubism 5.3 offscreen parts/advanced blending.
Models using offscreen parts or advanced blend modes are rejected with a compatibility
message. Export using standard compatible ArtMesh blending to use this renderer.
The independent Rust physics solver is not a byte-for-byte Cubism Framework or
VTS implementation. Expression values apply before physics; motion3/pose3 playback
remains separate from physics and is not implemented.

Atlas MiB is `width × height × 4` summed over textures. It is not measured process
VRAM; model memory, output/mask textures, uploads, and driver overhead are additional.
Large models can briefly pause the UI while importing; use a release build for normal use.

## Troubleshooting

- **Choose Core first / missing symbol / wrong architecture:** select the official
  current x64 DLL above. Install the [Microsoft Visual C++ x64 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)
  if Windows reports a missing runtime dependency. Restart after replacing a loaded DLL.
- **Consistency check failed:** re-export the avatar with Cubism Editor or obtain a
  valid export from its author. ARIA does not attempt to revive rejected files.
- **Missing texture or unexpected artwork:** retain the manifest's subfolders;
  for bare imports verify every atlas index. Do not substitute the model thumbnail.
- **Mouth/eyes do not respond:** inspect actual IDs and assignments under
  **Model parameters…**, then verify tracking meters and face-found status.
- **Static hair:** check that the referenced physics file loaded, secondary motion
  is enabled, and strength is above zero. The group/output counts should be visible.
- **Missing toggles:** open **Expressions** and import missing `.exp3.json` files.
  Assign shortcuts in ARIA; VTS keyboard assignments are not imported. If the avatar
  has no expression file for a toggle, use manual parameter controls.
- **Blank/wrong-size model:** check the model's exported canvas/origin and parameter
  defaults. Models relying on motion/pose/layout setup may need those features added.

See [validation](validation.md) for the actual model and GPU checks performed.
