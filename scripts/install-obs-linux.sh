#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
config=${XDG_CONFIG_HOME:-"$HOME/.config"}
destination="$config/obs-studio/plugins/aria-canvas/bin/64bit"
test -f "$root/obs-plugin/aria-canvas.so"
mkdir -p "$destination"
install -m 755 "$root/obs-plugin/aria-canvas.so" "$destination/aria-canvas.so"
printf '%s\n' 'ARIA Canvas (Alpha) installed for native OBS. Restart OBS and add ARIA Canvas (Alpha). Flatpak OBS is not supported by this installer.'
