# v0.27.0-alpha.1 — Experimental VBridger tracking and performance graphs

**The VBridger import and editor are Experimental.** Config compatibility and
motion can differ from VBridger. Review imported outputs before applying them;
the editor reports unsupported settings and missing tracking signals.

## Changes

- Import legacy and V2 `.vbridger` files, review their equations and model
  assignments, and apply the supported outputs to the selected avatar.
- Edit input calibration, input/output curves with weighted tangents, ranges,
  delay, smoothing, steps, face-loss behavior and axis corrections. Configs save
  per avatar and travel with movement presets and their hotkeys.
- Export complete ARIA JSON configs or V2 `.vbridger` output configs. Optional
  numeric tracking channels let external tools supply visemes and other inputs.
- Colorful performance graphs show hover readings and session low/high values.
  The footer stays compact and expanded counter categories collapse. Two-minute
  histories have bounded storage; extrema last until the app closes.

Open **Tracking → Import / edit VBridger config** to begin. See the
[import and editor guide](vbridger.md) and [graph controls](responsiveness.md).

## Compatibility and validation

The supplied AdvancedARKitSettings file imports all 34 outputs. Ten installed
default configs were also parsed and evaluated locally without skipped equations.
An actual native Cubism/Odette check verified imported mouth movement changes
model geometry and frozen poses remain exact. These tests do not establish
identical motion, physical phone latency or universal model compatibility.

Automated checks cover import bounds, expression parsing, dependencies, curves,
modifiers, input axes, profile/preset persistence, numeric tracking channels and
UI Apply/Cancel/Undo behavior. Native UI captures use the owner's Odette avatar
with an isolated profile. Avatar files and proprietary SDKs are not distributed.

VBridger's standalone input-curve format and native application reimport of ARIA
exports remain unverified. ARIA does not provide an audio phoneme recognizer,
native VMC receiver or automatic VMC bone routing. Some private/future modifiers
are unsupported. Read [the detailed compatibility table](vbridger.md#compatibility-and-limits).

## Packages and installation

Packages target Windows x64, Linux x64, macOS Apple Silicon and Intel. Linux also
includes separate Fedora and Arch app/OBS packages. Verify each download against
**SHA256SUMS.txt** from the same release and follow [Windows](windows.md),
[Linux](linux.md) or [macOS](platforms.md#install-an-alpha-release) instructions.

All builds are **Alpha**. Verified GitHub source commits do not sign downloadable
applications: Windows packages are unsigned and macOS uses an ad-hoc signature
without Developer ID notarization. Native platform CI is separate from physical
camera, controller, GPU and OBS acceptance testing.
