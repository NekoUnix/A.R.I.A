# ARIA Canvas OBS source — Alpha

The Linux native OBS plugin receives the full landscape, portrait or Freeform
canvas independently of the desktop preview. It uses local shared memory with
premultiplied BGRA/RGBA; there is no video encoding or network listener. ARIA uses
one asynchronous GPU readback buffer per canvas and skips work while it is busy,
up to 60 FPS. This is not GPU zero-copy and has higher memory bandwidth cost than
Windows Spout or macOS Syphon. Start with 1080p at 30 FPS on slower hardware.

Build with CMake, pkg-config and your distribution's libobs development package:

```sh
cmake -S native/linux-canvas -B target/obs-plugin -DCMAKE_BUILD_TYPE=Release
cmake --build target/obs-plugin
ctest --test-dir target/obs-plugin --output-on-failure
```

The release includes source and Fedora/Arch plugin packages. For the portable
tarball run `sh install-obs-linux.sh`, restart native OBS, add **ARIA Canvas
(Alpha)**, and select an open sender. Reopen source properties to refresh the list.
The sender name includes ARIA's process ID; after restarting ARIA, select its new
sender. Resolution changes reconnect automatically without changing the selection.
An absent sender becomes transparent. All three outputs can run together.

Install the plugin built for your native OBS ABI. Flatpak/Snap sandbox packages
require their own compatible extension and are not supported by this installer.
No root service, virtual camera module, PipeWire portal or compositor capture is
used. Files are exclusively created with mode 0600 under `/dev/shm`; both programs
must run as the same user. The transport reserves tmpfs space before mapping it,
rejects malformed sizes/formats, uses nonblocking advisory locks and removes its
owned file on normal shutdown. After a crash a stale file can remain until reboot;
close OBS/ARIA before removing a stale `aria-canvas-UID-PID-*` file manually.

`transport.[ch]` and its tests are MIT licensed (LICENSE-transport-MIT).
`obs-plugin.c` links libobs and is GPL-2.0-or-later (LICENSE). Its complete buildable
source accompanies the plugin. This license applies to the OBS plugin, which is
a separate program from ARIA. No OBS executable is redistributed.
