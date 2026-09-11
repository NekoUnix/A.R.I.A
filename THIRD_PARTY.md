# Third-party notes

A.R.I.A. code and the original Mica vector test puppet are MIT licensed under
the repository LICENSE. Third-party components retain their own terms.
The packaging script copies available dependency license/notice files and a
resolved dependency index into `dependency-licenses/` in the portable bundle.

Primary dependencies (exact versions are recorded in Cargo.lock):

| Project | Purpose | Upstream |
| --- | --- | --- |
| egui / eframe | Native UI, viewports and 2D drawing integration | https://github.com/emilk/egui |
| wgpu | GPU abstraction | https://github.com/gfx-rs/wgpu |
| serde / serde_json | Structured data and wire parsing | https://github.com/serde-rs/serde / https://github.com/serde-rs/json |
| image | PNG/JPEG decoding | https://github.com/image-rs/image |
| rfd | Native file dialogs | https://github.com/PolyMeilex/rfd |
| clap | CLI parsing | https://github.com/clap-rs/clap |
| anyhow | Error reporting | https://github.com/dtolnay/anyhow |
| tempfile | Isolated test files | https://github.com/Stebalien/tempfile |

The VTube Studio transport implementation follows DenchiSoft's published
[third-party iOS UDP protocol](https://github.com/DenchiSoft/VTubeStudioBlendshapeUDPReceiverTest).
The packet fixture is a synthetic example of that public schema, not a recording
of a person's face. This app is not affiliated with or endorsed by DenchiSoft,
Live2D Inc., or Apple.

No Cubism Core DLL, Cubism framework source, Live2D sample model, or third-party
avatar art is redistributed in this copy. Any later SDK/model integration must
preserve its applicable licensing and attribution separately.
