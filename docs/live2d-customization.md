# Customize a Live2D avatar

![The native Live2D appearance workspace](images/customization-v30.png)

Use **Inspector → Avatar → Customize** for a model's outfits, hair styles,
accessories and other appearance options. Use **Avatar → Layers** to select
exported artwork on the stage and hide, fade or tint it.

These tools adapt to the avatar you load. They read its exported parameter IDs,
names, ranges and folders, rather than expecting the controls of a particular
test model. Keep the complete model export together, including its
`.model3.json`, `.moc3`, textures, and optional `.cdi3.json` display information.
See [Live2D setup](live2d.md) if you have not imported the avatar yet.

## Select several layers with your mouse

The frozen selector described here is in the next source build after v0.30.0-alpha.1.
The published v0.30 package has the earlier on-stage selection tools.

![The separate frozen model layer selector](images/frozen-layer-editor.png)

1. Load your Live2D avatar, then click **Select layers…** above **Your stage**
   or in **Inspector → Avatar → Layers**. A separate window opens with a frozen
   copy of its current pose. You can move and resize this window.
2. Leave **Mouse selection: Add** selected. Click artwork to select its frontmost
   mesh, or hold the left mouse button and draw a box over several layers.
   Release the button to keep the selection. Repeat as often as you like:
   **plain clicks and boxes keep earlier selections, without Shift**.
3. Check the selection count and the list on the left. Uncheck any unwanted row,
   or click **Remove** and click/draw over layers to subtract them.
4. Click **Hide selected**. The artwork disappears from the preview, your live
   stage and all outputs. **Restore selected** removes individual opacity
   overrides. Edits save automatically to this avatar; **Done** keeps them.
5. Use **Undo** to reverse the last visibility/group edit made in this window.
   It remembers up to 24 edits while open and skips undo if layer settings have
   since changed elsewhere, so it cannot overwrite another change silently.

| Control | What it does |
| --- | --- |
| Add | Plain clicks and boxes accumulate layers. This is the default. |
| Remove | Plain clicks and boxes subtract layers. |
| Replace | Starts a fresh selection for each click or box. |
| Toggle | Flips membership of the clicked layer or boxed layers. |
| Clear selection | Deselects everything; visibility stays as it is. |
| Wheel / Zoom slider | Enlarges or reduces the frozen preview only. |
| Right or middle mouse drag | Pans around the preview. |
| Fit model | Resets preview zoom and pan. |
| Refresh frozen pose | Takes a fresh snapshot of the live model's current pose. |
| Reveal ARIA-hidden layers in this preview | Shows artwork hidden by ARIA opacity/groups, for selection and restoration. The live model stays unchanged until you edit it. |
| Escape | Cancels the current box and restores the previous selection. |

The live model keeps tracking while you work. Zooming, panning and refreshing
this window do not reposition the model in OBS. Loading a different avatar
closes the old preview. Closing the selector releases its extra render target;
it shares the loaded textures and does not start another Cubism process.

Enter a group name on the left, then click **Save selection as group** to reuse
these layers later. **Manage groups & hotkeys…** brings the main Layers
inspector forward so you can assign its shortcut.

The optional **On stage** switch retains the earlier moving-stage picker. In
that mode a box replaces the selection; Shift adds, Alt removes and Ctrl/Cmd
toggles. Turn it off to resume dragging pinned objects on the live stage.
Search the layer list by ArtMesh ID, part ID or exported part name. List ranges
and **Select results** also work, including matches outside the visible rows.

Selection uses exported mesh triangles. Transparent texture holes, clipping
masks and overlapping artwork can make a box include more layers than the
visible pixels suggest. Review the selected names before hiding a large area.
The preview's Reveal checkbox cannot reveal artwork hidden by the frozen
pose's own outfit/expression parameters. Change those controls, then refresh
the pose. Completely collapsed layers can be selected from the main list.
Selection guides never appear in OBS or exported images.

## Save layer groups and restore visibility

Select the layers, enter a name under **Saved layer groups**, then choose
**Create group from selection**. A new group starts inactive. Check its name to
apply the group's opacity; uncheck it to restore the underlying individual
settings. Expand its settings to edit membership, opacity and shortcut.

An active group can still hide an individually restored layer. **Disable groups
affecting selected layers** turns off those entire groups, including their other
members. **Show all layers** clears individual opacity overrides and disables
all groups. Neither button can reveal artwork hidden by the model's own outfit
or expression parameters: change those in Customize or Expressions.

## Set up your model's appearance controls

1. Open **Avatar → Customize** and expand **Set up appearance controls**.
2. Click **Add this model's suggested controls**. ARIA uses the creator's names
   and folders to suggest toggles, outfit switches and similar controls. Known
   physics outputs are excluded from suggestions. Adding a control does not
   change its value or turn it on.
3. If something is missing, open **Browse all exported parameters**. Search by
   name, folder or ID and check the controls you want in your workspace.
4. Check an appearance control to lock its value, then adjust the slider or
   numeric field. Other parameters continue following tracking.
5. Uncheck that control to release it. **Release all appearance values** removes
   every appearance override while keeping your curated list of controls.

For example, one model may export a `HAIR SWITCH` and an `OUTFIT SWAP`; another
may expose unnamed parameters with completely different ranges. Both can be
added and named in this workspace. A model without display information uses
raw IDs until you give its controls friendly labels.

An enabled appearance value takes priority over that parameter's tracking,
expressions, pose hold and physics output. Its value still drives any dependent
physics chains. It can be changed while a screenshot pose is frozen; other
frozen values remain fixed. Releasing it restores the existing behavior, so you
do not need to rebuild a tracking assignment.

## Make controls easier to use

Expand **Control name, category & options** below a control:

| Setting | What it does |
| --- | --- |
| Display name | Gives a raw ID a friendly label; blank uses the exported name. |
| Category | Organizes controls into your own collapsible folders. |
| Continuous slider | Allows intermediate values within the exported range. |
| On / off | Uses the parameter's maximum for On and minimum for Off. |
| Named choices | Maps names such as “Short hair” to explicit numeric values. |
| Step | Quantizes slider adjustments; zero is continuous, one suits numbered variants. |
| Authored default | Holds the value supplied by the model creator. |

Use named choices for reversed toggles or more than two variants. The creator's
rig determines what each number does and how intermediate values blend. ARIA
does not invent missing costume artwork or reconstruct layers merged during
export. Removing a control in Browse all also releases its override.

## Tint selected artwork

In **Layers**, expand **Selected layer colors**, choose a multiply tint, then
click **Apply tint to selected layers**. White preserves the original texture
colors. Other colors tint and darken them; multiplication cannot recolor black
pixels or recover missing detail. **Restore selected colors** restores the
creator's multiply and screen colors. The texture files stay intact.

## Save looks and assign shortcuts

Under **Customize → Saved appearance looks**, enter a name and choose **Save
appearance look**. A look captures locked customization values, layer visibility
and groups, and layer colors. Click **Apply** to recall it with tracking still
running. Tracking calibration, physics, props, current expressions and microphone
settings retain their current values.

Choose **Manage looks, shortcuts & exports** to open the preset library. There
you can select a look, rename or replace it, delete it, export it, or assign an
available **Ctrl+Alt+F1–F11** shortcut on Windows. Enable global hotkeys for the
avatar to use those shortcuts. Conflicting shortcuts are rejected. Imported
looks must belong to the same avatar content identity; imported keyboard
assignments are cleared.

For outfits implemented as `.exp3.json` expressions, use **Expression-based
outfits** to open the expression library. Appearance looks do not change active
expressions; a full **Movement** preset also remembers expression toggles.

The workspace, overrides and looks save per avatar. They affect the stage, all
outputs and exported PNGs. Keep a backup of important presets and your original
model export before experimenting with appearance combinations.

[Preset and input guide](input-controls.md) · [Live2D compatibility](live2d.md) ·
[Export a diagnostic report](diagnostics.md)
