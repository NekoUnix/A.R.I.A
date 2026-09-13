# Windows setup and operation

![ The current Windows build with Odette](images/workspace-v23.png)

ARIA v0.23 adds webcam tracking, the optional NVIDIA RTX adapter, themes and the control API. Guided tracking setup remains available for every avatar. See [Tracking setup](tracking-setup.md). Direct gamepad input and Live2D `NP_*` assignments remain available.
Open **Inspector → Tracking → Controller** to select a device and adjust response.
Xbox, PlayStation, Switch and other SDL-mapped controllers work alongside iPhone
tracking; unrecognized joysticks can use a custom SDL2 layout.
[Setup, input meanings and troubleshooting](controllers.md).

ARIA v0.19 adds animated VRM 0.x / 1.0 avatars to guided import.
Choose **Avatar & appearance → Import VRM avatar**, select your `.vrm`, review
its metadata, then Import. No Cubism SDK is needed for VRM. Phone tracking,
microphone talking, expression hotkeys and spring settings use its own profile.
See [VRM setup, controls and compatibility](vrm.md).

Guided PNG/GIF and Live2D import includes background loading of large GIFs,
and Inspector categories that match the imported avatar. The graphite palette
and blue accent use the same renderer and Windows font, with no blur or UI animation.
[See the import limits and workspace guide](in-app-help.md).

For Twitch/YouTube sign-in and a chat window below the portrait preview,
open **Chat → Streaming chat** in the left panel; each service has independent opacity,
background color and font settings. [Account setup and operation](streaming-chat.md)
explains the required OAuth registration and Windows-protected saved logins.
PNG/GIF image actions and microphone talking input are also available.
Open **PNG / GIF actions** to assign artwork to talking, blinking, custom input
ranges or hotkeys. Configure transitions, shake/jump/blip and GIF playback per action.
Open **Microphone** for device selection, sensitivity, smoothing and talking thresholds.
Image states, microphone settings and presets save with each avatar.
See the [step-by-step designer guide](../templates/effects/README.md),
[template kit](../templates/effects/README.md) and [plugin guide](../templates/effects/plugins/README.md).
Settings save per avatar. The previous vertical iPhone tracking correction remains.
Existing VTS neutral-pitch calibrations migrate once. If you manually enabled
Invert pitch to compensate for the earlier bug, review that setting and recalibrate
while looking straight ahead. Per-model mapping ranges and custom inversions remain yours.

ARIA includes an offline help window: hover a circled **?** beside a control
for a short explanation, or click it for detailed instructions and diagrams.
**Help & documentation** in the studio header opens the searchable guide. The
window can stay open while tracking continues; closing it leaves ARIA running.
Parameter, range, expression and physics-group help includes context from the
loaded avatar, captured when clicked. See the [bundled reference](in-app-help.md).

## 1. Choose a way to run

The target for this first build is **Windows 10/11, x64**, with an up-to-date
graphics driver. A Direct3D 12-capable GPU is the normal path through wgpu. Vulkan
can be selected as a fallback if supported by the installed driver. Other operating
systems and Windows ARM64 are not verified release targets yet.

### Download a compiled build

1. Visit [the Windows workflow](https://github.com/NekoUnix/A.R.I.A/actions/workflows/windows.yml).
2. Select a green, successful run on `main` for the version you want.
3. Under **Artifacts**, download **aria-windows-x64**. Sign in to GitHub if asked.
4. Extract the downloaded artifact. Extract `aria-0.23.0-windows-x64.zip` inside it
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
   Include **C++ CMake tools for Windows**, or install CMake separately and add it
   to PATH (`cmake --version`). SDL2 builds from its pinned dependency source and
   links statically; end users need no separate SDL DLL. The repository supplies
   CMake 4 compatibility and a GNU C11 flag for newer MinGW compilers.
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
- **Inspector → Tracking → Inputs / Diagnostics:** compare the model's final values with incoming
  tracking; expand controls to edit ranges, stepping, curves and dead zones.
- **Pose / Presets:** hold individual inputs or freeze the whole avatar for images,
  save model configurations and assign Windows global hotkeys. See the
  [input controls guide](input-controls.md) for the full workflow.
- **Physics:** tune enable, strength, inertia, response speed, gravity and wind
  for the whole avatar or each authored group. [Physics/profile guide](physics.md).
- **Expressions:** toggle `.exp3.json` files and assign your own keyboard shortcuts,
  saved per avatar. [Expression and custom shortcut guide](expressions.md).
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

Select **Avatar & appearance → Import PNG / GIF avatar…**. Choose files or a folder,
mark one image **Idle / base**, and review the roles and fitted playback dimensions.
Import prepares the base in the background, reports progress and offers Cancel;
the old avatar stays active until success. Remaining actions load next. Open
**Artwork & actions** to add Talking, Quiet, Blink, a custom tracking/parameter range,
or a manual hotkey. Use a low-priority Idle state as the fallback. Configure fades,
transition style, motion on change and GIF playback separately for each image.
Keep the canvas and subject position consistent between states for smooth swaps.
The last image avatar reopens on startup, and reopening the same base image restores
its saved action library. **Use built-in puppet** restores Mica.

Choose a per-GIF playback budget of 64–1024 MiB (default 256 MiB). The tested
3500 × 2500, 63-frame GIFs would occupy about 2103 MiB each at full resolution;
the default retains all frames at 1221 × 872. Files up to 512 MiB and 40960px source
edges are accepted, subject to the 4096 MiB source-canvas limit. GIFs can contain
4096 frames and 256 GiB of accumulated source data. These are ceilings, not startup
allocations; large canvases still need RAM for source-frame working buffers.
Collections retain at most 2560 MiB of playback textures. The review blocks a new
selection that exceeds that total. Source artwork is never resized on disk.

For microphone-only use, choose **Microphone / manual · no tracker** as the tracking
source. In the **Microphone** tab, enable input, select your device, and adjust noise
floor and talking thresholds while watching the meter. Enabling the microphone from
Demo switches to local input automatically. Phone/JSON tracking can also be combined
with the microphone. Samples are measured locally and discarded; voice is not recorded.

**Activate / hold image** or an assigned hotkey holds an action; press that key again
or **Resume automatic actions** to release it. **Pose → Freeze** holds the model,
GIF frame, fades and motion together for export. Save actions/profile to persist.
Import/export action configurations and editable artwork are supplied in
[templates/images](../templates/images/README.md). Circled **?** controls explain
all units, priority rules, GIF limits and microphone troubleshooting offline.

**Import Live2D avatar…** guides you through selecting an exported .model3.json
or .moc3 and the official Cubism Core x64 DLL. Review the detected atlas textures,
physics and expressions, then import. Follow the [Live2D guide](live2d.md) for details.

After import, **Model details** summarizes the current Live2D export. Live2D
shows Physics and Expressions, while PNG/GIF avatars show Artwork & actions.
**Change avatar / type** opens the type chooser; **Guided import tour** reopens
preparation for the current type. Tracking, outputs, chat and independent scene
objects remain available for either avatar format.

### Test your local assets

From a Rust-enabled PowerShell in the repository, run:

```powershell
.\scripts\test-local-assets.ps1 `
  -GifDirectory 'C:\Avatars\My GIF Tuber' `
  -Live2DModel 'C:\Avatars\My Live2D\Avatar.model3.json' `
  -CubismCore 'C:\Tools\CubismSdkForNative-5-r.5\Core\dll\windows\x86_64\Live2DCubismCore.dll'
```

Use the actual export directory. VTube Studio normally stores it under
`VTube Studio_Data\StreamingAssets\Live2DModels`, without an extra `VTube Studio`
directory before `_Data`. The script loads every GIF in place on the Windows GPU,
checks retained texture budgets and timing, tests a temporary copy padded to ten
times the largest GIF's file size, then checks the selected Live2D model through
the native Core and renderer. Padding tests byte-size handling, not extra image
detail. Sources and the SDK remain local. These optional tests require enough
RAM/VRAM for the selected assets and are excluded from ordinary CI.

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

1. Install the [OBS Spout2 plugin](https://github.com/Off-World-Live/obs-spout2-plugin/releases)
   using its Windows installer, then restart OBS. ARIA already includes its sender.
2. Expand **Capture & performance**. Enable Landscape, Portrait and/or Freeform;
   all three can stay open together. Leave **Send full resolution to OBS** checked.
   Select a canvas resolution independently of its small preview window.
   Drag the avatar to position it and scroll the wheel to resize it.
3. Choose **Studio background**, **Green screen / color key**, or **Transparent**
   separately for each canvas. For a key, enter hex and press **Apply hex**, or
   use **Detect safer color** to analyze the avatar's colors.
4. In OBS add a **Spout2 Capture** source for each output. Select **ARIA Landscape**,
   **ARIA Portrait**, or **ARIA Freeform**. For transparent output, choose
   **Composite mode → Premultiplied Alpha**. Keep OBS and ARIA on the same GPU.
5. For color-key output, add **Filters → Effect Filters → Chroma Key**, choose
   **Custom**, and paste that canvas's hex. Begin with low similarity and inspect
   hair, eyes and clothing. Framing and colors save per window, per avatar.

See the [complete OBS output guide](obs-output.md) for automatic-color limitations,
matching filters, saved layouts and canvas sizes.

**The preview stays small to save desktop space. Spout sends the actual selected
canvas resolution to OBS, even when the preview is minimized.** Window Capture
is still available without a plugin, but only captures the preview's pixel size;
enlarging that source in OBS does not add image detail. Window Capture alpha
depends on the capture method. The main stage uses a dark placeholder in
transparent mode. Virtual camera, click-through and borderless overlay are not included.

### Windows priority and performance

In **Capture & performance → Windows performance**, enable **High process priority**
to give ARIA scheduling preference over normal-priority processes during CPU
contention. This sets Microsoft's `HIGH_PRIORITY_CLASS` for ARIA only; disabling it
restores Normal. The setting is saved for this PC and is off by default.
It can reduce scheduling delays but does not guarantee hitch-free output or change
GPU priority. High can reduce responsiveness in OBS or other CPU-heavy programs;
disable it if that happens. Realtime is intentionally not offered because it can
preempt important OS work, including input handling. See Microsoft's
[SetPriorityClass documentation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass).

Use 30 or 60 FPS and a smaller OBS canvas if the GPU is overloaded. Simulation is
capped even during UI input; unchanged Live2D poses reuse the last render, mesh
buffers are reused, and closed outputs release their textures. Resolution and
layout edits settle briefly before reallocating capture textures or saving to disk.

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
| OBS Spout source blank | Open the ARIA output, enable sending, select its named sender and use the same GPU in both apps. Check ARIA's sender status and **Retry OBS output**. Vulkan fallback cannot send Spout. |
| Window Capture looks low resolution | It sees the compact preview. Use Spout2 Capture for the separate full-resolution canvas. |
| Transparent edges look dark | Select **Premultiplied Alpha** in Spout2 Capture, with no Chroma Key filter on transparent output. |

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

For a separate testing or portable profile, set `ARIA_PROFILE_DIR` to a writable
folder before launching ARIA. Both the app configuration and window layout use
that directory. Unset it to return to your normal Windows profile. Screenshot
test builds use their own temporary profile automatically.

Close the app before replacing binaries. Download a new successful artifact, or
update your checkout and rebuild. Preserve any uncommitted work before pulling.
Settings are separate from the executable. To uninstall the portable app, remove
its extracted folder and any helper-created firewall rule you added.
