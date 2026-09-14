# Themes and custom palettes

![Glass Light workspace with the supplied avatars](images/glass-light-v33.png)

Open **Settings → Appearance & themes**. The eight built-in palettes are **Glass
Dark, Glass Light, Sonoma Dark, Sonoma Light, Sakura, Ocean, Forest and High Contrast**. Selection applies
immediately to panels, cards, controls, headings, help and category accents.
Themes are global UI preferences; switching avatars preserves the selected palette.

Glass Dark is the default for a new installation. Existing installations keep
their saved colors. **Glass surfaces** adds translucent cards, a quiet color wash,
rounded controls and fine edges. The current profile has an accent outline and
an **Editing** label so you can identify the avatar you are changing.

Turn **Glass surfaces** off for solid panels and cards. **High Contrast** always
uses solid surfaces, including when loading an older saved High Contrast palette.
Popups remain nearly opaque to keep underlying content from competing with text.

Expand **Make your own theme**, enter a name, and edit any swatch. The picker exposes
RGB/hex controls; a hex readout is also shown beside every swatch. Set background,
panels, cards, accent, text, secondary text, borders, and the four category colors.
**Light controls** chooses the built-in icon style. A contrast notice appears if
main text against cards falls below a 4.5:1 ratio; check other surfaces visually too.

**Save custom theme** creates or updates the named reusable palette. Up to 32 custom
palettes are stored. Unsaved edits to the active palette also persist through normal
app autosave. **Delete saved custom theme** removes its saved library entry without
surprising you by changing the active colors. **Reset** selects Glass Dark.

**Export theme…** writes a small JSON file. **Import theme…** applies a validated
theme; save it to add it to the picker. Names contain 1–64 characters; files are
limited to 8 KiB and contain RGB triplets and light/glass options, with no code or
external resources. Older theme files work; the new glass option defaults on.
Use [the theme template](../templates/themes/my-theme.json) to create one in an editor.

The glass appearance uses ordinary UI geometry and alpha blending, with one
four-vertex static background wash. It adds no blur pass, desktop capture,
animated decoration or additional render targets. Fonts are loaded once.
This is an Apple-inspired visual style, not native macOS Liquid Glass.
Studio output follows the theme background color;
transparent and chroma-key backgrounds keep their output configuration.
The glass panels and their decoration are excluded from native OBS canvases.

The [control API](api.md) can select any built-in or saved custom theme by name.
