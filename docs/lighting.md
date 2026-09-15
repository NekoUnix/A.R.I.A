# Avatar lighting

Select a stage tab, then open **Avatar → Avatar lighting** in the Workspace.
Lighting belongs to that avatar, including its profiles and movement/pose presets.
Every OBS output uses the same result; the capture background color stays separate.

1. Check **Custom lighting**. Leaving it off preserves the original rendering.
2. Choose **Directional** for shading from one side, or **Even / all-over light**
   to light the avatar uniformly without a single light direction.
3. Click **Light color** to open the color picker. Use **White light** to remove tint.
4. For directional light, move **Left / right direction** and **Above / below**.
   Zero horizontal angle faces the viewer; positive angles light from the right.
   Positive elevation lights from above. Increase **Fill light** to brighten shadows.
5. Click **Save profile**. Edits save automatically with the selected avatar.
   Repeat on other loaded avatars to match the lighting across your composition.

VRM/GLB uses the actual skinned surface normals; even mode removes directional
surface shading. Authored emission, rim and matcap effects may remain visible.
The 3D **Light intensity** setting still controls brightness. This is an avatar
light, without cast shadows, environment maps or a separate lighting scene.

Live2D, PNG and GIF have no 3D normals. They use an inexpensive color tint and
soft directional gradient over their artwork. Even mode applies the same tint
everywhere. Lighting preserves opacity and animated GIF transitions. Pinned items,
throws and the background keep their own appearance. Direction for 2D artwork is
relative to its image, while 3D light is relative to the stage world.

If a model looks too dark, try white light and raise Fill light, or use Even mode.
Disable Custom lighting to compare with the original. Light color can also change
which chroma-key background works best; recheck your key against the lit result.
