# A.R.I.A.

A Windows-first, native Rust foundation for a modular avatar runtime. This first
development build connects directly to **VTube Studio on iPhone**, maps facial
tracking into avatar parameters, and animates a built-in 2D test puppet or your PNG
artwork. The UI and preview run on **egui + wgpu**, without Unity or Godot.

> **v0.1 scope:** this is a working tracking and desktop-output prototype.
> It can **inspect** Live2D `.model3.json` exports, but **cannot render `.moc3`
> models yet**. Cubism Core integration, mesh clipping, physics, motions and
> expressions are future work. No Cubism SDK binaries or third-party model art
> are included. See [architecture and roadmap](docs/architecture.md).

![A.R.I.A. Windows studio with its original Mica test puppet](docs/images/studio.png)

## Run on Windows

**Without a Rust installation:** open [Windows build artifacts](https://github.com/NekoUnix/A.R.I.A/actions/workflows/windows.yml),
select a successful run for `main`, download `aria-windows-x64`, and extract both
the artifact ZIP and the app ZIP inside it. Run `aria-desktop.exe`. GitHub requires
sign-in to download workflow artifacts. Artifacts expire after 30 days; a new
workflow run creates a new one.

**From source:** install Rust and the Visual Studio **Desktop development with
C++** workload, including MSVC x64 tools and a Windows SDK. Then open PowerShell:

```powershell
git clone https://github.com/NekoUnix/A.R.I.A.git
cd A.R.I.A
cargo run --locked --release -p aria-desktop
```

Cargo installs the compiler pinned in `rust-toolchain.toml` through rustup on first
use. Initial compilation downloads dependencies and takes several minutes.

**[Detailed Windows installation, build, OBS and troubleshooting guide →](docs/windows.md)**

## Connect an iPhone

1. Put the Windows PC and iPhone on the same trusted local network. Ethernet on
   the PC and Wi-Fi on the phone are fine if they share that network.
2. In VTube Studio on iPhone, start face tracking and enable **3rd Party PC
   Clients** in the first settings tab. Find the phone's IPv4 address and request
   port in the app. The documented default request port is `21412`.
3. In A.R.I.A., choose **iPhone · VTube Studio**, enter that phone address, leave
   **PC receive port** at `11125` unless another app uses it, and select **Connect
   tracking**. Permit the app on **Private networks** if Windows asks.
4. Wait for **Tracking live**, look straight ahead, and select **Calibrate neutral
   pose**. Blink, open your mouth and turn your head to check the preview and meters.

This uses the documented [VTube Studio third-party UDP protocol](https://github.com/DenchiSoft/VTubeStudioBlendshapeUDPReceiverTest),
not its desktop WebSocket plugin API. VTube Studio on the PC, Steam, a VTS plugin
token, and USB pairing are not required. This integration uses LAN networking;
it does not implement VTS's proprietary USB transport.

**[Tracking setup, ports, protocol, simulator and troubleshooting →](docs/tracking.md)**

## Included in this first copy

- Native desktop app with an original animated test puppet, PNG/JPEG loading and
  an optional talking image, neutral-pose calibration, smoothing, gains and axis correction.
- Direct VTS iOS UDP subscription and renewal; all incoming blendshapes retained
  for inspection, with 12 standard Cubism-style parameters mapped for the preview.
- Versioned **ARIA JSON v1** UDP input for other tools and custom bridges.
- Separate **A.R.I.A. Output** window for OBS Window Capture, green-screen mode,
  experimental transparent background, and optional always-on-top behavior.
- Connection diagnostics, packet age/rate, input validation, disconnect recovery,
  GPU adapter/backend information, UI frame rate and UI CPU time.
- Live2D manifest/asset presence inspection and mapped-parameter JSON export.
- Saved connection/mapping/output preferences. Connections always require an
  explicit click after startup. PNG artwork is reselected each session.
- Headless CLI, local phone simulator, automated tests, Windows build scripts and CI.

Frame time is not total process CPU utilization. Asset bytes on disk are not VRAM
usage. GPU load, per-model VRAM accounting, webcam tracking, Spout/shared textures,
an OBS plugin, and a stable public plugin SDK are not implemented in v0.1.

## Test without a phone

The default **Demo** source needs no other process. To exercise the real UDP
subscription path, run this in a second PowerShell terminal:

```powershell
cargo run --locked -p aria-cli -- simulate-vts --seconds 120
```

In the desktop app select **iPhone · VTube Studio**, set the phone address to
`127.0.0.1`, request port `21412`, receive port `11125`, then connect. The simulator
only sends frames while it has a valid subscription, just like the phone protocol.

For a packaged build, use `./aria-cli.exe simulate-vts --seconds 120` instead.

## Development

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release --workspace
```

To build a portable bundle with its documentation:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
```

The script runs tests and writes `dist/aria-0.1.0-windows-x64.zip`. See
[architecture](docs/architecture.md) for crate boundaries and
[validation](docs/validation.md) for what has actually been exercised.

## License

A.R.I.A.'s source and original test puppet are [MIT licensed](LICENSE). Dependencies
keep their own licenses. The MIT license does not cover the Live2D SDK or user
artwork. [Third-party notes](THIRD_PARTY.md) identify the main libraries and protocol
references.
