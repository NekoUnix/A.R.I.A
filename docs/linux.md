# Install ARIA Alpha on Linux

This guide covers the **v0.26.0-alpha.1** downloads for Ubuntu 24.04, Fedora 44
and current Arch Linux on **x86_64 / amd64** PCs. Use the
[published Alpha release](https://github.com/NekoUnix/A.R.I.A/releases/tag/v0.26.0-alpha.1).
Linux ARM64 packages, a Debian `.deb`, an AppImage and Flatpak/Snap packages are
not included in this release.

ARIA needs a graphical desktop, glibc **2.39 or newer**, and a working Vulkan
driver for your GPU. The Vulkan loader alone does not supply a GPU driver. Use
your distribution's supported graphics driver. Native output works independently
of whether your desktop uses Wayland or X11.

## Choose your downloads

| Environment | Application | Optional native OBS plugin |
| --- | --- | --- |
| Ubuntu 24.04 | `aria-0.26.0-alpha.1-linux-x64.tar.gz` | Included in the archive; install separately with its script |
| Fedora 44 | `aria-alpha-0.26.0.alpha.1-1.x86_64.rpm` | `aria-obs-canvas-0.26.0.alpha.1-1.x86_64.rpm` |
| Current Arch Linux | `aria-alpha-0.26.0alpha.1-1-x86_64.pkg.tar.zst` | `aria-obs-canvas-0.26.0alpha.1-1-x86_64.pkg.tar.zst` |

Also download **SHA256SUMS.txt** from that same release. OBS is optional: you can
install and run ARIA without installing its OBS plugin. These are downloadable
community Alpha packages; there is no ARIA APT/DNF repository or AUR installation
step in this release.

The Fedora download names above contain **`.alpha.1`**. GitHub replaces the `~`
in the original RPM filenames with a dot during upload. The internal RPM version
is still `0.26.0~alpha.1`, and the package contents are unchanged. Use the actual
download names above when following the commands below.

## Verify the files

Open a terminal in the folder containing your downloads. All installation
commands below start from that folder unless stated otherwise.

```sh
sha256sum --check --ignore-missing SHA256SUMS.txt
```

Each downloaded package must report **OK**. `--ignore-missing` lets you verify
only the operating-system packages you downloaded. A failed checksum or a result
with no files verified is not a successful check; download the matching files
again before installing. Keep the original download filenames for verification.

## Ubuntu installation

These runtime package names apply to Ubuntu **24.04**. Install them using APT:

```sh
sudo apt update
sudo apt install libasound2t64 libudev1 libssl3t64 libxkbcommon0 \
  libwayland-client0 libwayland-cursor0 libx11-6 libxcursor1 \
  libxi6 libxrandr2 libvulkan1
```

Ubuntu 24.04 uses the `t64` package names for
[ALSA](https://packages.ubuntu.com/noble/libasound2t64) and OpenSSL. Keep your GPU's
Vulkan driver installed as well. Ubuntu 22.04's standard glibc is too old for this
portable build; do not replace the system glibc to make it load.

Extract the complete archive into a folder you own and launch the app:

```sh
mkdir -p "$HOME/Applications"
tar -xzf aria-0.26.0-alpha.1-linux-x64.tar.gz -C "$HOME/Applications"
"$HOME/Applications/aria-0.26.0-alpha.1-linux-x64/aria-cli" --version
"$HOME/Applications/aria-0.26.0-alpha.1-linux-x64/aria-desktop"
```

The CLI should print `aria-cli 0.26.0-alpha.1`. Keep the extracted files together.
The portable archive does not add an application-menu entry automatically. Run
ARIA as your desktop user; it does not need `sudo`.

For native OBS output, install Ubuntu's native OBS package, close OBS, then run
the included plugin installer **without sudo**:

```sh
sudo apt install obs-studio
sh "$HOME/Applications/aria-0.26.0-alpha.1-linux-x64/install-obs-linux.sh"
```

This installs the source for your user under
`${XDG_CONFIG_HOME:-$HOME/.config}/obs-studio/plugins/aria-canvas/bin/64bit/`.
It targets Ubuntu 24.04's distribution OBS 30 with `libobs.so.0`. A PPA, Flatpak,
Snap or differently built OBS may require a different plugin build; see
[OBS compatibility](#obs-compatibility-and-first-capture).

## Fedora installation

The RPMs were installation-tested on **Fedora 44**. These steps target DNF-based
Fedora installations. From your verified download
folder, install the application with DNF so its dependencies are resolved:

```sh
sudo dnf install ./aria-alpha-0.26.0.alpha.1-1.x86_64.rpm
aria-cli --version
aria-desktop
```

You can also launch **ARIA Alpha** from the application menu. The app is installed
under `/opt/aria-alpha`, with launch links in `/usr/bin`.

To add native OBS output, close OBS and install the optional source package:

```sh
sudo dnf install ./aria-obs-canvas-0.26.0.alpha.1-1.x86_64.rpm
```

DNF also installs the native `obs-studio` dependency if needed. The source is
installed under `/usr/lib64/obs-plugins/aria-canvas.so`; it was built against
Fedora's OBS 32 libraries. Restart OBS after installation. Do not copy the Ubuntu
portable plugin over this Fedora build. DNF's official
[local-package installation reference](https://dnf5.readthedocs.io/en/latest/commands/install.8.html)
describes dependency handling.

## Arch installation

Use an up-to-date **x86_64 Arch Linux** system. Update the whole system before
installing the downloaded local package; do not perform a partial upgrade with
`pacman -Sy` alone.

```sh
sudo pacman -Syu
sudo pacman -U ./aria-alpha-0.26.0alpha.1-1-x86_64.pkg.tar.zst
aria-cli --version
aria-desktop
```

The app installs under `/opt/aria-alpha` and adds an **ARIA Alpha** menu entry and
launch links in `/usr/bin`. To add native OBS output, close OBS and install:

```sh
sudo pacman -U ./aria-obs-canvas-0.26.0alpha.1-1-x86_64.pkg.tar.zst
```

Pacman resolves dependencies, including native `obs-studio`. The plugin installs
under `/usr/lib/obs-plugins/aria-canvas.so`; this release was built and tested with
Arch's OBS 32.2.2 package. Restart OBS after installation. The
[pacman manual](https://pacman.archlinux.page/pacman.8.html) documents local package
installation with `-U`, system upgrades and removal. No AUR helper is needed.

## OBS compatibility and first capture

Use the plugin built for your OBS installation:

| OBS installation | ARIA source to use |
| --- | --- |
| Ubuntu 24.04 native OBS 30 (`libobs.so.0`) | Plugin from the Linux portable archive |
| Fedora 44 native OBS 32 | Fedora `aria-obs-canvas` RPM |
| Current Arch native OBS 32 | Arch `aria-obs-canvas` package |
| Custom OBS build or another distribution | Build the [included plugin source](../native/linux-canvas/README.md) against that installation's libobs development files |
| Flatpak or Snap OBS | No matching extension is included; the native installer does not install into these environments |

1. Run ARIA and OBS as the same desktop user.
2. In ARIA's **Output** page, enable landscape, portrait and/or Freeform, choose
   the canvas resolution, then enable **Send full resolution to OBS (ARIA Canvas)**.
3. Restart OBS after installing the plugin. Add **ARIA Canvas (Alpha)** to your
   scene and choose the matching live sender.
4. Choose **Transparent** in ARIA for alpha, or choose a studio/chroma background.
   The OBS source receives the full canvas while the preview window stays small.
5. After restarting ARIA, select its new process-ID sender in OBS. Reopen the
   source properties to refresh the sender list. Canvas resizes reconnect
   automatically.

All three outputs can run together. Linux uses bounded asynchronous GPU readback,
local shared memory and an OBS texture upload. It costs more memory bandwidth than
GPU-only Spout/Syphon sharing; start with 1080p at 30 FPS on slower hardware.
See the [output guide](obs-output.md#linux-aria-canvas) for framing and transparency.

## Import your avatar and tracking

The built-in puppet starts without an SDK. Open **Avatar → Avatar & appearance →
Import avatar** to select PNG/GIF, VRM or Live2D. Live2D requires the official
Linux x86_64 Cubism Core library, usually
`Core/dll/linux/x86_64/libLive2DCubismCore.so`, and your exported model files.
Choose the library or extracted SDK folder in guided import. A Windows DLL does
not work as the Linux Core library. PNG/GIF and VRM do not need Cubism Core.

For iPhone VTube Studio or external JSON input, follow [tracking setup](tracking.md)
and [personal calibration](tracking-setup.md). The [platform capability table](platforms.md#what-runs-where)
records remaining Linux limits, including Windows-only priority/hotkey controls,
credential storage and camera/NVIDIA setup tooling.

## Updates and removal

Close ARIA and OBS before replacing installed files. These downloads do not add
an automatic ARIA update repository. For a newer Alpha, download its matching app,
optional plugin and checksum file; verify them and repeat the DNF or pacman
installation command with the **new filenames**. For the portable version,
extract into a new versioned folder and run its plugin installer again if used.
Back up your avatar files and profiles before upgrading.

To uninstall, use the commands for the components you actually installed:

| Environment | Remove the app | Remove the optional OBS source |
| --- | --- | --- |
| Fedora | `sudo dnf remove aria-alpha` | `sudo dnf remove aria-obs-canvas` |
| Arch | `sudo pacman -R aria-alpha` | `sudo pacman -R aria-obs-canvas` |
| Portable | Delete only the extracted ARIA version folder in your file manager | Delete only its `aria-canvas.so` from the per-user OBS path listed above |

Package removal does not delete your per-user profiles or original avatar files.
Review the package manager's removal list before confirming.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Fedora download cannot be found | Use `.alpha.1` in the downloaded RPM filename, not `~alpha.1`; open the terminal in your download folder. |
| `GLIBC_2.39` missing / executable format error | Check `getconf GNU_LIBC_VERSION` and `uname -m`. These binaries need glibc 2.39+ and x86_64. Other systems need a compatible [source build](platforms.md#build-on-linux-or-macos). |
| App exits with a graphics error | Launch `aria-desktop` in a terminal and check your Vulkan driver. `vulkaninfo --summary`, if installed, can identify whether a Vulkan device is available. |
| Shared library is missing | For the trusted portable download, run `ldd ./aria-desktop` inside its extracted folder and install the missing runtime dependency. For RPM/Arch installs, use DNF/pacman to resolve dependencies. |
| OBS has no ARIA Canvas source | Restart native OBS; confirm you installed the matching plugin, not just the app. Check **Help → Log Files → View Current Log** for load errors. A previous per-user portable plugin can conflict with a system package; remove that old ARIA plugin file before switching methods. |
| `libobs.so.0` or `.30` missing | The plugin and OBS installation do not match. Use your distro's plugin or rebuild against your libobs; do not create a library-name symlink. |
| Source is blank after restarting ARIA | Enable native output in ARIA, refresh OBS source properties, and select ARIA's current process-ID sender. Both apps must run as the same user. |
| Shared-memory allocation fails | Check `df -h /dev/shm`, lower the canvas resolution or close unused outputs, then use **Retry OBS output**. A 4096×4096 canvas alone needs 64 MiB of shared memory. |

The release's [native validation run](https://github.com/NekoUnix/A.R.I.A/actions/runs/34749991532)
includes Fedora/Arch clean-container installation, Linux readback and libobs
rendering tests. See [validation limits](validation.md) for physical-device and
model coverage; installation success does not establish compatibility with every
GPU, model or OBS variant.
