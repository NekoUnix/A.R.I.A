# Expressions and custom hotkeys

ARIA v0.6 loads Live2D `.exp3.json` expressions, including JSON expression files
named `.exp3`. Expressions can control facial features, clothing, accessories or
any other parameters exported in the avatar. The controls use each loaded model's
files and parameter IDs; they are not tied to the development test avatar.

## Load and toggle expressions

1. Open the avatar's `.model3.json` or `.moc3` as usual.
2. Open **Inspector → Avatar → Expressions**. ARIA reads the manifest's named expression
   references and discovers unlisted `.exp3.json` / `.exp3` files in the avatar
   folder and its subfolders. The same file appears only once.
3. Check an expression to turn it on; uncheck it to fade it out. Select its name to
   see its file, blend amount, fade times and parameter values. Multiple expressions
   can be enabled together. **All off** releases them all.
4. Use **Import expression files…** for exports stored elsewhere. You may select
   several files. ARIA remembers those file paths for this avatar. Keep the files
   at those paths; ARIA does not modify or copy the author's files.
5. After editing an existing expression on disk, choose **Reload files**. This
   rereads the listed files and restarts their fades. Reopen the avatar to discover
   newly added files in its folder, or import the new files directly.

Missing or malformed files appear under **Files needing attention**. They do not
prevent the avatar or its other expressions from loading. Unknown parameter IDs
are displayed under the selected expression and skipped. An expression needs
matching parameter IDs to affect a model; an arbitrary expression from another
avatar will not necessarily work.

## Assign your own keyboard shortcut

1. Select an expression's name in the library.
2. Under **Selected expression → Keyboard shortcut**, choose any combination of
   **Ctrl**, **Alt**, **Shift** and **Win**, then choose the main key from the menu.
   Supported keys include letters, digits, function keys, numpad keys, arrows and
   navigation keys. For example, choose **Ctrl + Shift + H** for a hoodie toggle.
3. Click **Assign shortcut**. The assignment is saved and enables global hotkeys.
4. Press the combination once to turn the expression on and again to turn it off.
   Holding the key does not repeatedly toggle it. The shortcut also works while
   another app is focused. **Clear shortcut** removes just this assignment.

ARIA rejects a shortcut already assigned to another expression or preset.
**Ctrl+Alt+P** remains reserved for freezing/resuming a pose. F12 is excluded because
Windows reserves it for debuggers. If another application or Windows owns a key,
the registration error appears in the panel; choose another combination or disable
and re-enable global hotkeys after freeing it. These are owned
[Windows RegisterHotKey registrations](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey),
without a keyboard hook. Windows-reserved combinations may be unavailable.

Modifiers are optional. An unmodified shortcut, such as `H`, also intercepts that
key during normal typing while ARIA's global hotkeys are enabled. A modifier
combination is usually more convenient. **Enable global hotkeys (Windows)** controls
expression, preset and pose shortcuts together. Disabling it or closing ARIA
releases the registrations. Buttons still work when global hotkeys are disabled.

VTube Studio phone hotkey numbers and desktop VTS shortcut assignments are not
imported. Set the desired keyboard combination in ARIA.

## Blending, movement and screenshots

ARIA applies the file's **Add**, **Multiply** and **Overwrite** parameter operations,
following the [Cubism expression model](https://docs.live2d.com/en/cubism-sdk-manual/expression/).
An omitted Blend uses Add. Omitted FadeInTime/FadeOutTime values use one second;
zero means immediate. Fades ease smoothly, and toggling during a fade starts the
new transition from the current blend amount.

Each frame starts from the avatar's default/manual values and current tracking.
Expression layers then apply in the displayed file order, before authored physics.
This lets changed driver parameters push secondary movement. Values clamp to the
rig's limits, and Add/Multiply values do not accumulate from one frame to the next.
If expressions overlap, later layers can change the result of earlier ones;
later Overwrite layers take priority as their blend reaches 100%.

Individual held parameters keep priority over expressions and physics. A full
**Frozen** pose preserves the exact final appearance and pauses expression fades.
Expression toggles are disabled while frozen; press **Resume live** before changing
them. To make an image, turn on the desired expressions, let the fades settle,
then freeze the pose and export a transparent PNG from **Pose**.

## Saved profiles and presets

On/off selections, expression file paths and shortcuts save with the avatar's
existing profile. Switching models releases the outgoing shortcuts and loads the
incoming model's assignments. Reopening the same model restores its selections;
active expressions fade in again unless its saved pose is frozen.

Movement presets include the active expression selection. Pose presets capture
the final appearance, including expressions, and freeze it. Shortcut assignments
and imported expression paths belong to the model profile, so applying a preset
does not replace them. Exported preset JSON includes expression identities, not
the expression files themselves. Keep the matching files with the same avatar.

Discovery is bounded to 4,096 directory entries and eight subfolder levels, and
does not follow files outside the model directory. Up to 256 expressions are
loaded per avatar, with a 1 MiB limit per file and 4,096 parameter entries per
expression. Motion3 and pose3 playback remain separate, unimplemented features.
