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
| image | PNG/JPEG/GIF decoding | https://github.com/image-rs/image |
| rfd | Native file dialogs | https://github.com/PolyMeilex/rfd |
| clap | CLI parsing | https://github.com/clap-rs/clap |
| anyhow | Error reporting | https://github.com/dtolnay/anyhow |
| tempfile | Isolated test files | https://github.com/Stebalien/tempfile |
| libloading | Explicit native Cubism Core loading | https://github.com/nagisa/rust_libloading |
| bytemuck | Typed GPU buffer serialization | https://github.com/Lokathor/bytemuck |
| windows-rs | Windows process and DXGI memory counters | https://github.com/microsoft/windows-rs |
| Spout2 SDK protocol | Sender registry, synchronization and D3D11On12 sharing conventions | https://github.com/leadedge/Spout2 |
| gltf-rs | Static GLB/glTF and VRM geometry import | https://github.com/gltf-rs/gltf |
| ufbx | Static FBX and OBJ parsing | https://github.com/ufbx/ufbx |
| glam | 3D prop transforms | https://github.com/bitshifter/glam-rs |
| rodio / cpal / Symphonia | Windows microphone amplitude capture, audio playback and WAV, MP3, Vorbis and FLAC decoding | https://github.com/RustAudio/rodio / https://github.com/RustAudio/cpal / https://github.com/pdeljanov/Symphonia |
| getrandom / base64 | Local API keys and embedded glTF buffers | https://github.com/rust-random/getrandom / https://github.com/marshallpierce/rust-base64 |
| reqwest / rustls | HTTPS OAuth and YouTube API requests | https://github.com/seanmonstar/reqwest / https://github.com/rustls/rustls |
| tungstenite / native-tls | Twitch secure IRC WebSocket connection using Windows TLS | https://github.com/snapview/tungstenite-rs / https://github.com/sfackler/rust-native-tls |
| sha2 | SHA-256 PKCE challenge generation | https://github.com/RustCrypto/hashes |

Twitch and YouTube are separate services. Their APIs require user-configured OAuth
applications and account authorization; ARIA includes no publisher-owned client
credentials. The native chat preview contains synthetic examples, not downloaded
chat histories. OAuth passwords and browser cookies are handled by the providers.
See [chat setup and protocol references](docs/streaming-chat.md).

Effect starter artwork, static cube files, WAV and synthesis/export templates in
`templates/effects` and `templates/images` are original ARIA materials under the repository MIT license.
The Streamer.bot adapter is original integration code using its public C# API;
Streamer.bot and Twitch are separate applications/services and are not bundled.

The Rust Spout sender follows the Spout2 SDK's `SharedTextureInfo` layout,
sender-name maps, named mutexes, frame-count semaphore and DirectX 12 bridge.
Copyright (c) 2020-2024, Lynn Jarvis. The BSD 2-Clause notice is reproduced in
[docs/licenses/spout.txt](docs/licenses/spout.txt) and included in portable builds.
The OBS Spout2 plugin is installed separately from its upstream project.

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
The independent Rust particle solver interprets exported `physics3.json` settings;
see Live2D's [physics integration documentation](https://docs.live2d.com/en/cubism-sdk-manual/physics/).
It does not embed Cubism Framework or reproduce VTube Studio's proprietary solver
and tracking filters. Profile imports read the user's adjacent data files locally.
Consult Live2D's [SDK licensing information](https://www.live2d.com/en/sdk/license/)
before distributing a product with their runtime. The portable ZIP includes no Core DLL.
Live2D's release guidance specifically covers VTuber tracking software as an
Expandable Application; the repository is not evidence of publication-license approval.
