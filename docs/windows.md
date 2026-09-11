# Windows setup and operation

## 1. Choose a way to run

The target for this first build is **Windows 10/11, x64**, with an up-to-date
graphics driver. A Direct3D 12-capable GPU is the normal path through wgpu. Vulkan
can be selected as a fallback if supported by the installed driver. Other operating
systems and Windows ARM64 are not verified release targets yet.

### Download a compiled build

1. Visit [the Windows workflow](https://github.com/NekoUnix/A.R.I.A/actions/workflows/windows.yml).
2. Select a green, successful run on `main` for the version you want.
3. Under **Artifacts**, download **aria-windows-x64**. Sign in to GitHub if asked.
4. Extract the downloaded artifact. Extract `aria-0.5.0-windows-x64.zip` inside it
   into a normal writable folder, for example `C:\Apps\ARIA`.
5. Double-click **aria-desktop.exe**. The default source is Demo and Mica should move.

Do not run the app from inside the ZIP viewer. The bundle contains the desktop
app, CLI, documentation, license, and an optional firewall helper. It has no
installer, auto-start task or background service. This first executable is unsigned;
Windows may display its normal publisher/reputation prompt. Only run a build you
downloaded from this repository and intend to trust.

If the app reports a missing Microsoft runtime DLL, install Microsoft's current
[Visual C++ x64 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist).

### Build from source

The documented development toolchain is **Rust MSVC x64**. RustRover or an editor
is optional; neither replaces the compiler/linker.

1. Install [Git for Windows](https://git-scm.com/downloads/win) if `git --version`
   does not work.
2. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022).
   In the installer select **Desktop development with C++**. Include the MSVC x64/x86
   compiler and a Windows 10 or Windows 11 SDK. The full Visual Studio IDE is optional.
3. Install Rust using the official [Windows rustup installer](https://rust-lang.org/install.html).
   Keep the `x86_64-pc-windows-msvc` default. Rustup may offer to install the C++
   prerequisites itself. See the official [Windows Rust setup guide](https://learn.microsoft.com/en-us/windows/dev-environment/rust/setup).
4. Close and reopen PowerShell so it sees the updated PATH. Verify:

   ```powershell
   git --version
   rustup show
   cargo --version
   ```

5. Clone and build:

   ```powershell
   git clone https://github.com/NekoUnix/A.R.I.A.git
   cd A.R.I.A
   cargo build --locked --release --workspace
   .\target\release\aria-desktop.exe
   ```

   If the repository is already cloned, open PowerShell in its root instead of
   cloning again. A folder name containing spaces is supported; quote it when
   using `cd`. This workspace may already be named `Live2D Alternative`.

The compiler version is pinned in `rust-toolchain.toml`; rustup will download it
when first needed. `Cargo.lock` pins dependency resolution. Internet access is
needed on the initial build; cached dependencies can be used offline afterward.
The demo, PNG puppet and tracking work without an SDK. Live2D avatars require the official Cubism Core DLL; see [Live2D setup](live2d.md).

For a quicker edit/compile loop:

```powershell
cargo run --locked -p aria-desktop
```

For a tested ZIP:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
```

This process-level execution-policy option applies to this invocation only. It
does not change your machine's saved policy. The script writes the package path
at the end and stops on test or build errors. Use `-SkipTests` only if you have
already run the checks on the same source revision.

## 2. Check the desktop app

On launch, **Demo input** produces synthetic values. Mica is an original vector
test puppet with moving eyes, eyelids, brows, mouth and head. This verifies drawing
and mapping without requiring a phone or licensed model.

- **Tracking source:** Demo, direct VTube Studio iOS, or an external ARIA JSON sender.
- **Calibrate neutral pose:** captures head rotation while a face is present.
  Calibration saves with the current model. Changing the tracking source resets it;
  recalibrate when moving the phone or changing its orientation.
- **Smooth ms:** higher values reduce jitter but add response delay. Start at 75 ms.
- **Head / mouth gain:** scale tracking response. Parameters are clamped to their ranges.
- **Axis correction:** invert axes if the device orientation produces reversed motion.
- **Input Monitor → Inputs / Raw:** compare the model's final values with incoming
  tracking; expand controls to edit ranges, stepping, curves and dead zones.
- **Pose / Presets:** hold individual inputs or freeze the whole avatar for images,
  save model configurations and assign Windows global hotkeys. See the
  [input controls guide](input-controls.md) for the full workflow.
- **Physics:** tune enable, strength, inertia, response speed, gravity and wind
  for the whole avatar or each authored group. [Physics/profile guide](physics.md).
- **Export mapped values:** save the current parameter snapshot as JSON. This is
  a snapshot, not continuous recording or network output.

Only the allowed sender's IP can supply UDP frames. Face data is processed locally;
there is no analytics, cloud upload, webcam access or recording. IP filtering is
not encryption/authentication: use a trusted LAN. Help links open only on a click.

The app saves its preferences through eframe's per-user Windows application-data
storage, under the A.R.I.A. app identity. Window size and settings are local to that
user. It does not write settings into the source repository. Network connections
and the OBS output window are never automatically opened on the next launch.
Each model now restores its own tracking, mapping, calibration, appearance, parameter
and physics profile. **Save profile** writes it immediately. Studio sections and
parameter categories are collapsible; expand only what you need.

### Resource bar

The bottom bar samples **ARIA's process usage once per second**:

- **CPU %:** kernel + user CPU time, normalized across available logical processors.
  100% means all processors busy. This differs from the adjacent UI-work time.
- **RAM:** resident working set. Hover to see private committed memory as well.
- **VRAM:** DXGI local-memory usage for this process on the exact Direct3D 12
  adapter used by wgpu. Hover for the OS budget and shared/non-local usage. This
  includes driver allocations, not just atlas textures. On integrated GPUs the
  local segment uses system memory.

The first CPU sample and unavailable counters show **N/A**, not zero. VRAM is N/A
on the Vulkan fallback or when the driver cannot report it. These are process
counters, not machine-wide totals or GPU utilization %. Atlas MiB in the Avatar
panel remains a separate decoded-texture estimate.

## 3. Use your own artwork

Select **Open PNG…** and choose a PNG or JPEG. Transparent PNGs work best.
Images are limited to 4096 × 4096 and 32 MiB on disk. The image rotates/translates
with head tracking. It is a flat puppet; this does not infer a facial rig.

Optionally choose **Set talking image…** for an open-mouth variant. Use the same
dimensions, transparent margins and subject placement for both images. The app
switches to that image above mapped mouth-open value 0.18. Blink deformation is
implemented for Mica, not for a single imported PNG. **Reset** restores Mica.
Avatar selections are not persisted between sessions.

**Open Live2D avatar…** loads a real exported .model3.json or .moc3 avatar with its texture atlases. Configure the official Cubism Core x64 DLL first. Follow the [detailed Live2D guide](live2d.md) for import, custom tracking assignments, compatibility and troubleshooting.

**Inspect Live2D .model3.json…** remains available as a separate asset report. It checks references and file sizes; only Open Live2D avatar performs native model evaluation and rendering.

## 4. Connect iPhone tracking

Follow [the tracking guide](tracking.md#iphone-vtube-studio) for the device steps.
There are two ports with different jobs:

| Setting | Default | Destination |
| --- | --- | --- |
| Phone request port | UDP 21412 | A.R.I.A. sends subscriptions to the iPhone |
| PC receive port | UDP 11125 | The iPhone sends tracking to A.R.I.A. |

Do not configure a router port forward. The PC and phone communicate on the LAN.
Do not put the PC's address in the **iPhone IPv4 address** box. The phone's address
is required there.

### Windows Firewall

If Windows prompts for `aria-desktop.exe`, permit it on the **Private** profile
for your trusted home network. If no prompt appears and data is blocked, the repo
includes a narrowly scoped helper. In an **Administrator PowerShell**:

```powershell
cd "C:\path\to\A.R.I.A"
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\allow-tracking-firewall.ps1 -AppPath .\target\release\aria-desktop.exe -Port 11125
```

For an extracted bundle:

```powershell
cd C:\Apps\ARIA
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\allow-tracking-firewall.ps1 -AppPath .\aria-desktop.exe -Port 11125
```

The helper allows **inbound UDP only**, for that **exact executable and port**,
on the **Private** profile, from **LocalSubnet**. It does not disable the firewall,
allow public networks, or open all application ports. A.R.I.A. does not run this
script or request administrator access automatically. If your phone is on a routed
separate subnet, configure the exact required scope in Windows Firewall yourself.

When changing the executable location, debug/release build, or receive port,
update the corresponding firewall rule. A CLI listener is a different executable
and needs its own rule for physical-phone testing; a loopback simulator does not.

To remove a helper-created rule (use the same port):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\allow-tracking-firewall.ps1 -Port 11125 -Remove
```

## 5. Capture in OBS

1. Enable **Open OBS capture window** in A.R.I.A. Leave this separate window open
   and not minimized. It is titled **A.R.I.A. Output** and contains only the avatar.
2. Select **Green screen** in A.R.I.A.'s output settings.
3. In OBS add a **Window Capture** source and choose **A.R.I.A. Output**. Select
   the Windows 10 capture method if OBS's automatic choice is blank. Capture the
   client area or crop the native title bar as needed.
4. On that OBS source add **Filters → Effect Filters → Chroma Key**, choose green,
   and adjust similarity/smoothness for your artwork. Green pixels in artwork may
   also be removed; use artwork/colors suitable for chroma key.
5. Resize/crop the source in OBS. **Keep output on top** is optional.

**Transparent (experimental)** asks Windows for a transparent native viewport.
Desktop transparency does not guarantee the chosen OBS capture method retains
alpha. Use green screen when capture alpha is black or inconsistent. The main
preview uses a dark placeholder background for transparent mode. This release
does not install an OBS plugin and does not implement Spout, shared GPU textures,
virtual camera output, click-through, or a borderless desktop overlay.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| `cargo` is not recognized | Install rustup, reopen PowerShell, check `%USERPROFILE%\.cargo\bin` is on PATH. |
| `link.exe` missing or Windows libraries cannot be found | Install the C++ workload **and Windows SDK**; open a fresh terminal. VS Code/RustRover alone is insufficient. |
| Blank window / graphics initialization error | Update the GPU driver. Try Vulkan as shown below. A remote desktop or VM may expose a different adapter. |
| `Cannot bind UDP port` | Another A.R.I.A. instance or tracker owns that receive port. Choose another PC receive port and update its firewall rule. |
| `Waiting for data` | Verify phone IPv4, request port, VTS third-party switch, iOS Local Network permission, and Private firewall scope. Test the simulator to separate network issues from app issues. |
| Requests increase but valid packets stay at zero | Sending a UDP request does not prove it arrived. Check both devices' network paths and the receive firewall rule. |
| `Face not found` | The phone is reachable but reports no face. Check the camera, lighting, and VTS tracking state. |
| `Signal lost` | No valid frame for one second; the avatar returns toward neutral. The receiver keeps renewing and recovers automatically. |
| Rejected packets rise | The sender is not using the selected schema, or values/size are invalid. See Raw view and the last error. |
| Head snaps around 0/360 | Signed Euler wrapping is handled. Calibrate facing the phone; use axis correction if needed. |
| OBS source blank | Select the **output** window, not the controls window. Do not minimize it. Try OBS's Windows 10 capture method and green screen. |

Vulkan fallback for a packaged build:

```powershell
$env:WGPU_BACKEND = 'vulkan'
.\aria-desktop.exe
Remove-Item Env:WGPU_BACKEND
```

For a source build replace the executable path with `./target/release/aria-desktop.exe`.
If the driver does not support Vulkan, use the default Direct3D path on supported
hardware. The UI footer identifies the actual adapter/backend; frame-rate targets
are subject to display refresh, GPU capacity and extra UI events.

## Updating

Close the app before replacing binaries. Download a new successful artifact, or
update your checkout and rebuild. Preserve any uncommitted work before pulling.
Settings are separate from the executable. To uninstall the portable app, remove
its extracted folder and any helper-created firewall rule you added.
