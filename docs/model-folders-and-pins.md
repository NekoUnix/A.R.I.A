# Model folders, full-body framing and accessories

ARIA v0.20 adds a folder picker for nested Live2D exports and shared attachment
controls for Live2D, PNG/GIF and VRM avatars. Settings stay with each avatar profile.

## Import a model library on Windows

1. Extract downloaded ZIP/RAR archives. Keep their internal folders intact.
2. For OneDrive folders, select **Always keep on this device** in Explorer and
   wait until the download completes.
3. In **Avatar & appearance**, choose **Change avatar / type → Live2D**.
4. Click **Choose model folder…** and choose the library or an individual model
   folder. ARIA searches its nested folders for `.model3.json` exports.
5. Select the desired relative path, choose your Cubism Core DLL, review and import.
   Files with spaces, apostrophes, Unicode names and texture subfolders work as-is.

You can also drop a folder onto **Your stage**, or start the app with a folder:

```powershell
.\aria-desktop.exe "C:\path\to\Friend Models"
```

A library can contain multiple avatars, but the guide imports the one you select.
It never guesses between duplicate exports. Each manifest determines its own atlas
order, expressions, physics and tracking sidecars. The scan stays within the chosen
folder and stops at 20,000 entries, 256 exports or 16 levels. It does not open archives.

## Live2D framing

The renderer now fits the visible mesh geometry with a small margin, including
parts extending outside the export's declared canvas. This projection is shared
by the stage, outputs, PNG export and pin picking. Hidden meshes parked away from
the body do not shrink the initial avatar. If movement or a revealed expression
needs more room, the view expands and holds that size to prevent zoom jitter.
Reimporting refits the starting pose. Texture resolution and render target count
are unchanged.

Output positioning and zoom still apply separately. A deliberately enlarged or
off-center avatar can extend outside an OBS window; reduce its zoom or reset its
placement. This does not restore art omitted from the export, or override parts
the rig itself hides with opacity or clipping masks.

## Pin and move accessories on any avatar type

1. Drop a PNG, GIF or Live2D object on **Your stage**, or use **Objects → Add objects**.
2. Select it and drag it into place. Use **Placement & appearance** for size,
   rotation, opacity, X/Y offset and front/behind ordering.
3. In **Pin to avatar**, click **Choose pin point**, then click the avatar. Or use
   **Pin here** with the object's center already over the desired point.
4. Drag again to adjust its offset while keeping the pin. Turn off **Lock stage
   dragging** if it is locked. **Unpin** retains the visible placement.
5. Save the item settings or a pose/movement preset. Existing input toggles and
   user-defined hotkeys control accessories on all three avatar types.

| Avatar | What the pin follows |
| --- | --- |
| Live2D | The clicked ArtMesh triangle, including tracking, physics and expressions |
| PNG/GIF | The incoming artwork layer, tracking and action motion such as shake, jump or blip |
| VRM | The clicked projected triangle, including GPU skinning, morphs, spring motion and camera changes |

**Follow pin rotation** rotates the accessory and its offset. **Follow surface
stretch** follows the triangle edge's scale, or the geometric mean of the two
PNG/GIF animation scales. **Follow surface visibility** includes the image action's
fade/opacity for GIFs. Disable it if an accessory should stay visible through a fade.
Crossfades use the incoming artwork layer as the attachment reference.

Picking uses triangle geometry rather than individual texture-alpha pixels; if a
transparent region picks the wrong surface, choose a nearby solid part. VRM items
are flat overlays with front/behind layering, not 3D props with depth occlusion.
You can move/rotate a pinned Live2D object independently of its own internal rig.

## Workspace appearance

The compact macOS-style dark workspace uses rounded controls, blue selections and
distinct teal, purple, orange and pink category accents. Colors are solid fills;
there are no blur passes, background animation or additional GPU render targets.

## Local GPU verification

From a source checkout with the Windows toolchain installed:

```powershell
.\scripts\test-model-library.ps1 -Folder "C:\path\to\Friend Models" -Core "C:\path\to\Live2DCubismCore.dll"
.\scripts\test-vrm.ps1 -Vrm "C:\path\to\avatar.vrm"
```

The library test imports each discovered export in place, renders it and checks
that visible geometry remains inside the view at head/body parameter extremes.
Private avatars and local test screenshots are excluded from the repository/package.
