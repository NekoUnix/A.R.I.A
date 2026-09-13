#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
config=${XDG_CONFIG_HOME:-"$HOME/.config"}
destination="$config/obs-studio/plugins/aria-canvas/bin/64bit"
test -f "$root/obs-plugin/aria-canvas.so"
if ! dependencies=$(ldd "$root/obs-plugin/aria-canvas.so" 2>&1) || printf '%s\n' "$dependencies" | grep -q 'not found'; then
    printf '%s\n' "$dependencies" >&2
    printf '%s\n' 'This portable plugin requires Ubuntu 24.04-compatible native OBS (libobs.so.0). Use the Fedora/Arch plugin package or build native/linux-canvas against your OBS version; do not create a libobs symlink.' >&2
    exit 1
fi
mkdir -p "$destination"
install -m 755 "$root/obs-plugin/aria-canvas.so" "$destination/aria-canvas.so"
printf '%s\n' 'ARIA Canvas (Alpha) installed for native OBS. Restart OBS and add ARIA Canvas (Alpha). Flatpak OBS is not supported by this installer.'
