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
| libloading | Explicit native Cubism Core loading | https://github.com/nagisa/rust_libloading |
| bytemuck | Typed GPU buffer serialization | https://github.com/Lokathor/bytemuck |

The VTube Studio transport implementation follows DenchiSoft's published
[third-party iOS UDP protocol](https://github.com/DenchiSoft/VTubeStudioBlendshapeUDPReceiverTest).
The packet fixture is a synthetic example of that public schema, not a recording
of a person's face. This app is not affiliated with or endorsed by DenchiSoft,
Live2D Inc., or Apple.

No Cubism Core DLL, Cubism framework source, Live2D sample model, or third-party
avatar art is redistributed in this copy. The Rust ABI wrapper and WGSL renderer
implement the documented [Cubism Core API](https://cubism.live2d.com/sdk-doc/reference/NativeCoreAPIReference_en_r14.pdf).
Core is loaded from the user's official Native SDK installation and remains under
the [Live2D Proprietary Software License](https://www.live2d.com/eula/live2d-proprietary-software-license-agreement_en.html).
ARIA's MIT license does not grant rights to redistribute Core or model assets.
Consult Live2D's [SDK licensing information](https://www.live2d.com/en/sdk/license/)
before distributing a product with their runtime. The portable ZIP includes no Core DLL.
Live2D's release guidance specifically covers VTuber tracking software as an
Expandable Application; the repository is not evidence of publication-license approval.
