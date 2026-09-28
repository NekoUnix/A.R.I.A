# ARIA Core runtime and distribution

ARIA's Live2D-compatible `.moc3` evaluator is implemented in Rust in
`crates/aria-model-core`; the desktop adapter lives in `crates/aria-live2d`.
The desktop uses it by default. It retains the MOC3 and encoded atlas files in
system RAM for the avatar lifetime, decodes the textures for GPU upload, and
tries GPU geometry evaluation first. If GPU plan creation fails, the same Rust
evaluator computes geometry on the CPU. `ARIA_DISABLE_GPU_MOC3=1` forces that
CPU route for diagnostics. Surface pin editing and attached surface objects
also update CPU mesh positions so the current stage layout can track them.

The release check rejects proprietary Cubism Core binaries, removed runtime
source, and user-supplied model assets. ARIA does not grant rights to any
imported model, textures, motions or character artwork; follow the model
author's license. Live2D and Cubism are trademarks of Live2D Inc. ARIA's
compatibility work does not imply affiliation or endorsement.

The Rust core supports the tested MOC3 features described in
[model core](aria-model-core.md). Some valid exports may use features that are
not yet supported; the importer reports an error rather than silently loading
a different runtime. An end-to-end 120 FPS guarantee has not been established
for every model and GPU.
