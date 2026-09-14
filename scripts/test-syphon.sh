#!/bin/sh
# Native publisher/client regression using the same pinned framework as releases.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
sh "$root/scripts/build-syphon.sh"
clang -fobjc-arc -Wall -Wextra -Werror -framework Foundation -framework Metal \
  "$root/apps/aria-desktop/native/syphon.m" \
  "$root/apps/aria-desktop/native/syphon-test.m" -o "$root/target/syphon-test"
"$root/target/syphon-test" "$root/target/syphon-build/Build/Products/Release/Syphon.framework"
