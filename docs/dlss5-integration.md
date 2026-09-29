# DLSS 5 for 3D avatars: integration status

ARIA's VRM and imported VRChat GLB avatars share one transparent, offscreen wgpu canvas. The **Graphics** tab controls that canvas now. Its DLSS 5 control is disabled because no neural-rendering pass is connected; enabling a preference alone would misreport the output.

This does not mean custom-engine integration is impossible. [NVIDIA describes DLSS 5](https://developer.nvidia.com/blog/whats-new-for-game-developers-dlss-5-with-3d-guided-neural-rendering-nvidia-ace-updates-and-new-rtx-kit-capabilities) as a final, one-frame-in/one-frame-out neural render stage using color and motion vectors, with model, Structure Intensity, Tone Intensity and mask controls on RTX 50-series GPUs. [Streamline 2.14's changelog](https://github.com/NVIDIA-RTX/Streamline/blob/main/changelog.txt) names `sl.dlss_nr`, but its public source tree does not contain an NR feature header or integration guide. An [experimental Bevy Vulkan integration](https://github.com/AlrikOlson/bevy_dlss5) demonstrates an NGX route with depth and motion prepasses, while documenting an external architecture-specific runtime and setup constraints. That integration has not been ported to or tested in ARIA.

Before a working switch can ship in ARIA:

1. Obtain a redistributable NVIDIA neural-rendering runtime and SDK path applicable to ARIA. Keep proprietary binaries out of the repository unless their terms expressly permit distribution. Do not depend on a renamed executable, patched GPU checks, or another application's copied DLL.
2. Select and validate one graphics API for the native path. ARIA currently uses wgpu and may run D3D12 or Vulkan; a native NGX/Streamline bridge must share the correct device, textures, command queues and synchronization with that backend. Failure must leave the ordinary renderer running.
3. Add per-pixel motion vectors and retain a suitable depth input for the moving skinned/morphed mesh, camera and springs. The current depth attachment is discarded after the avatar pass. Track camera cuts and resolution changes so temporal history resets correctly.
4. Feed the neural pass scene color at the expected color space and keep the transparent output usable in stage and OBS. Test edge alpha, stylized faces, hair, outlines, eye highlights and user-authored colors against the source image. A beauty pass that changes avatar identity is unacceptable even if it runs.
5. Expose enable, model/preset, Structure Intensity, Tone Intensity and masking only after runtime capability queries succeed. Show actual active/inactive state and the reason for failure. Persist settings per 3D avatar without affecting PNG, GIF or Live2D.
6. Benchmark off/on frame time, GPU memory, output FPS and latency on both RTX 5060 and RTX 5090 adapters. DLSS 5 is a neural rendering effect, not a promise of higher FPS. Exercise VRM and VRChat GLB, each output shape, model switches, resize and shutdown.

The existing Graphics tab's Performance/Balanced/Detail presets and resolution, toon light and outline controls are ARIA's own renderer settings. They do not invoke DLSS 5.
