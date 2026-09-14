# Third-party notes

A.R.I.A. code and the original Mica vector test puppet are MIT licensed under
the repository LICENSE. Third-party components retain their own terms.
The packaging script copies available dependency license/notice files and a
resolved dependency index into `dependency-licenses/` in the portable bundle.

## Find every package and version

- [Cargo.lock](Cargo.lock) records the complete resolved Rust dependency graph,
  including transitive and platform-specific packages.
- Each Windows download includes `dependency-licenses/INDEX.md`, listing resolved
  package names, exact versions, license identifiers and upstream repositories.
- Linux downloads include `dependency-licenses/INDEX.txt`. On macOS this lives in
  `ARIA Alpha.app/Contents/Resources/dependency-licenses/INDEX.txt` (Finder → Show
  Package Contents). These indexes describe that platform's resolved Rust packages.
- Optional webcam Python dependencies have their own exact
  [requirements lock](tracking/requirements-lock.txt). Native frameworks and
  external SDK terms are described below; they are not all Rust packages.

Graphics/UI dependency review for v0.33 (2026-09-14): wgpu **30.0.1** and
egui/eframe **0.36.2** already match the current stable releases. The pinned Syphon
framework revision is unchanged. Runtime performance changes are documented in
[the performance guide](docs/performance.md); no dependency speedup is claimed.

## Primary libraries

Exact Rust versions are recorded in Cargo.lock:


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
| postcard | Compact private Cubism host protocol | https://github.com/jamesmunns/postcard |
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
| Rust-SDL2 / SDL2 | Background gamepad input, device layouts and hot-plug support (MIT / zlib) | https://github.com/Rust-SDL2/rust-sdl2 / https://github.com/libsdl-org/SDL |

The controller input names and meaning follow the public Nyarupad VTS interface
at https://github.com/maruseu/Nyarupad-VTS. ARIA implements its own signal mapping;
no Nyarupad code, artwork or executable is bundled. SDL2 is statically linked;
its source license is included in the portable dependency notices.

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

## Optional webcam runtime (v0.24)

The standard camera installer separately downloads MediaPipe 1.0.1 (Apache-2.0),
OpenCV contrib Python 4.11.0.86 (Apache-2.0 and bundled dependency notices), NumPy
1.26.4 (BSD-3-Clause), cv2-enumerate-cameras 1.3.3 and the transitive distributions
pinned in `tracking/requirements-lock.txt`. Their license files remain with the
installed Python distributions. Python itself is supplied by the user under the
Python Software Foundation license. The portable app includes setup scripts,
not a redistributed Python environment.

Google's FaceLandmarker task is downloaded separately from the official MediaPipe
model endpoint. Its SHA-256 is
`64184e229b263107bc2b804c6625db1341ff2bb731874b0bcc2fe6544e0bc9ff`.
See [Google's model documentation](https://ai.google.dev/edge/mediapipe/solutions/vision/face_landmarker)
and applicable model/license terms. No model task binary is committed to this repo.

NVIDIA RTX inference requires the user's NVIDIA AR SDK and matching feature/model
packages under NVIDIA's SDK and model terms. They are not bundled or downloaded
by ARIA. The optional bridge is compiled against those headers on the user's PC.
Reference-sample attribution is in [tracking/nvidia/NOTICE.md](tracking/nvidia/NOTICE.md).

Current documentation screenshots include the owner's supplied Odette Live2D,
GIF and VRM artwork at their explicit request. The code license does not license
the depicted art. See [screenshot provenance](docs/images/README.md).

## Native OBS additions (v0.25 Alpha)

macOS bundles the official [Syphon framework](https://github.com/Syphon/Syphon-Framework)
at revision `71351d4b484cd2d1917867f7846a5cdca724552d`. Its BSD-style notices,
including Metal contributors, are in [docs/licenses/syphon.txt](docs/licenses/syphon.txt).
The ARIA Objective-C adapter is original MIT code.

The separate Linux **ARIA Canvas** OBS plugin links libobs and is GPL-2.0-or-later.
Its license, complete source and build instructions accompany every plugin binary
under [native/linux-canvas](native/linux-canvas/README.md). The transport and ARIA
application remain MIT licensed. OBS itself is not bundled.
