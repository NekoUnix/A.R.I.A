# Native platforms and ARIA Core

The v0.37.0-alpha.1 release provides Windows x64, Linux x64, macOS Apple Silicon
and macOS Intel packages. Every app and archive is labeled **Alpha**. Native builds
and protocol tests do not replace acceptance testing on physical GPUs, webcams,
phones and every model. See [validation](validation.md) for the tested boundary.

## What runs where

| Component | Windows | Linux | macOS |
| --- | --- | --- | --- |
| Main Rust application | DX12; Vulkan fallback | Vulkan | Metal |
| ARIA Rust model core | Built in | Built in | Built in |
| PNG/GIF and VRM | Implemented and Windows-tested | Native implementation; hardware validation pending | Native implementation; hardware validation pending |
| iPhone/JSON tracking protocol | Tested with local sender | Portable UDP implementation | Portable UDP implementation |
| Native OBS canvas output | Spout2 GPU sharing | ARIA Canvas plugin; asynchronous readback/shared memory | Syphon Metal GPU sharing |
| Global hotkeys, High process priority, usage counters | Windows integrations | Not implemented; use app controls | Not implemented; use app controls |
| Persistent Twitch/YouTube credentials | Windows DPAPI | OS credential store planned | Keychain support planned |
| Camera setup helper / NVIDIA bridge | Windows tooling | Native installer/adapter validation planned | Native installer planned; NVIDIA RTX unavailable |

## Model evaluation

```mermaid
flowchart LR
    UI[ARIA: tracking and settings] --> Core[ARIA Rust model evaluator]
    Core --> GPU[GPU geometry and rendering]
    Core --> CPU[CPU geometry for fallback and surface pins]
```

The desktop evaluates Live2D-compatible models in-process. It retains model
sources in RAM and uploads atlases and geometry to the GPU as needed. A separate
Rust-only worker remains available for integration tests; normal imports do not
start it. The GPU path may fall back to Rust CPU geometry for unsupported plans.

## Built-in runtime

All builds use ARIA's Rust model core. No SDK
selection or platform-specific DLL copying is required. Old saved Core paths
are ignored. See [runtime distribution notes](aria-core-runtime.md).

## Build on Linux or macOS

Install Rust through rustup, Git and CMake. The repository pins the compiler;
`cargo build --locked --workspace` builds the desktop, CLI and runtime host.

On Ubuntu 24.04 install native build dependencies first:

```sh
sudo apt-get update
sudo apt-get install -y build-essential cmake pkg-config libasound2-dev libudev-dev libssl-dev libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev
git clone --branch v0.37.0-alpha.1 https://github.com/NekoUnix/A.R.I.A.git
cd A.R.I.A
cargo build --locked --release --workspace
./target/release/aria-desktop
```

On macOS install Xcode Command Line Tools (`xcode-select --install`) and CMake,
then clone and run the same Cargo commands. A native graphical session and suitable
GPU driver are needed; a successful headless CI build is not a graphics test.
Developer ID signing/notarization remains planned. Alpha bundles have local ad-hoc signatures.

Developers can set `ARIA_CUBISM_HOST` to an absolute, trusted `aria-cubism-host`
executable from the same build for native integration tests. Normal users can
leave it unset. See [validation](validation.md) for the tested boundary.

## Install an Alpha release

Download the archive/package matching your architecture from
[Releases](https://github.com/NekoUnix/A.R.I.A/releases). Verify its SHA-256 against
`SHA256SUMS.txt`. These are unsigned community Alpha packages, not distribution
repository packages. Cubism Core and private avatar files are not included.

| System | Artifact | Launch |
| --- | --- | --- |
| Windows x64 | `aria-0.37.0-alpha.1-windows-x64.zip` | Extract, run `aria-desktop.exe` |
| Linux x64 | `aria-0.37.0-alpha.1-linux-x64.tar.gz` | Extract, run `./aria-desktop` |
| macOS Apple Silicon | `aria-0.37.0-alpha.1-macos-arm64.zip` | Extract, open `ARIA Alpha.app` |
| macOS Intel | `aria-0.37.0-alpha.1-macos-x64.zip` | Extract, open `ARIA Alpha.app` |
| Fedora x64 | `aria-alpha-0.37.0.alpha.1-1.x86_64.rpm` | Install with DNF; launch ARIA Alpha |
| Arch x64 | `aria-alpha-0.37.0alpha.1-1-x86_64.pkg.tar.zst` | Install with pacman; launch ARIA Alpha |

For complete [Ubuntu, Fedora and Arch installation instructions](linux.md),
including runtime dependencies, checksum verification, optional OBS installation,
updates and removal, use the Linux guide. The examples below install both the app
and its optional OBS plugin. GitHub normalizes `~` to `.` in Fedora download
filenames; the RPM's internal version remains `0.37.0~alpha.1`.

Linux binaries target glibc 2.39 or newer (Ubuntu 24.04+, recent Fedora and current
Arch), with ALSA, OpenSSL 3, Wayland/X11 libraries, libxkbcommon and a Vulkan driver.
Fedora and Arch packages declare runtime dependencies. OBS is optional for the app;
its **aria-obs-canvas** package is installed separately. Plugin packages are built
against Fedora 44 and current Arch OBS 32+ respectively. The portable plugin is
built against Ubuntu 24.04's native OBS 30 (`libobs.so.0`); other distributions
should use their matching package or compile the included source against their OBS.

```sh
# Fedora: run in the downloaded release folder
sudo dnf install ./aria-alpha-0.37.0.alpha.1-1.x86_64.rpm ./aria-obs-canvas-0.37.0.alpha.1-1.x86_64.rpm

# Arch: run in the downloaded release folder
sudo pacman -Syu
sudo pacman -U ./aria-alpha-0.37.0alpha.1-1-x86_64.pkg.tar.zst ./aria-obs-canvas-0.37.0alpha.1-1-x86_64.pkg.tar.zst
```

The application installs under `/opt/aria-alpha`, with CLI/desktop launch links in
`/usr/bin`. The native OBS plugin installs under `/usr/lib64/obs-plugins` on Fedora
or `/usr/lib/obs-plugins` on Arch. Existing profiles stay in your user data folder;
package removal does not delete them. Upgrade using the same package-manager command.
For portable installs, run `sh install-obs-linux.sh` to add the plugin for your user.

macOS requires macOS 13+ and Metal. Move **ARIA Alpha.app** into Applications.
Because it is not notarized, macOS may require **System Settings → Privacy &
Security → Open Anyway** after your first attempt. Keep the app's internal
`Contents/Frameworks/Syphon.framework` intact. Audio/camera permission prompts are
specific to your chosen inputs. Windows-only RTX tooling, global hotkeys and
credential encryption are not implemented on macOS/Linux.

## Reproduce native packages

After `cargo build --locked --release --workspace` on Ubuntu 24.04, build the
portable Linux OBS plugin as shown in [its README](../native/linux-canvas/README.md).
Install `rpm`, `zstd` and Docker, then run `sh scripts/build-linux-obs-packages.sh`
to compile separate plugins against Fedora 44 and current Arch OBS headers and
libraries. Finally run `python3 scripts/package-native.py linux` (Python 3.11+).
OBS library filenames differ between distributions; copying Ubuntu's plugin into
all packages is insufficient. Each plugin package records its build's OBS version.

For macOS run `sh scripts/build-syphon.sh`, then `python3 scripts/package-native.py
macos`. This builds Syphon at an immutable revision and bundles it; it does not
install a global framework. For source execution set `ARIA_SYPHON_FRAMEWORK` to
the absolute `target/syphon-build/Build/Products/Release/Syphon.framework` path.
The **Alpha packages** workflow builds all four OS/architecture combinations and
retains the installer artifacts. Publishing a release requires all packages to
finish successfully and have checksums; an incomplete matrix is not a release.
