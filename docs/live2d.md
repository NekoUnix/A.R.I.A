# Live2D avatar import on Windows

ARIA v0.2 evaluates real `.moc3` models through Cubism Core and renders their
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

The 12 [standard mapped parameters](tracking.md#mapping) automatically bind when
those IDs exist in the rig. Other parameters remain at their exported defaults.
Missing standard IDs are skipped; values clamp to each model parameter's min/max.

Click **Model parameters…** to search the actual IDs exported by the rig. For any
parameter, choose one of the 12 tracking inputs, or **Manual** to use its slider.
This also provides manual controls for custom toggles/shape parameters. These
assignments and manual values last until the avatar is unloaded; they are not
saved between sessions. VTube Studio's `.vtube.json` mappings and hotkeys are not
imported. Smoothing, gains, mirroring and axis corrections apply before rig binding.

Open **A.R.I.A. Output** for OBS as described in the [Windows guide](windows.md).
The two windows share the same rendered avatar texture and live parameter state.
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
| Tracking | Standard IDs plus manual assignment to custom rig IDs |
| Input limits | 2 MiB manifest; 128 MiB moc; 1–32 atlases; each atlas at most 8192px (also limited by the GPU) and 128 MiB compressed; 1 GiB total decoded atlas storage |
| Render target | Transparent offscreen texture, longest side 2048px, shared with studio/output; UI zoom scales it |

**Not yet implemented:** `.physics3.json` simulation, `.motion3.json` playback,
`.exp3.json` playback, `.pose3.json` processing, motion events/audio, model3 layout
overrides, VTS item attachments, or Cubism 5.3 offscreen parts/advanced blending.
Models using offscreen parts or advanced blend modes are rejected with a compatibility
message. Export using standard compatible ArtMesh blending to use this renderer.
Physics-dependent hair/clothes stay at their parameter defaults unless assigned
or manually adjusted. This is an avatar-rendering foundation, not full VTS feature parity.

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
- **Static hair or missing toggles:** physics and VTS configuration are not applied;
  inspect the relevant manual parameters. This is separate from tracking connectivity.
- **Blank/wrong-size model:** check the model's exported canvas/origin and parameter
  defaults. Models relying on motion/pose/layout setup may need those features added.

See [validation](validation.md) for the actual model and GPU checks performed.
