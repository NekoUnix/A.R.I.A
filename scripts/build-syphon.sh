#!/bin/sh
# Build the official framework from an immutable upstream revision; no root install.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
revision=71351d4b484cd2d1917867f7846a5cdca724552d
work="$root/target/syphon-$revision"
if [ ! -d "$work/.git" ]; then
  git clone https://github.com/Syphon/Syphon-Framework.git "$work"
fi
git -C "$work" checkout --detach "$revision"
test "$(git -C "$work" rev-parse HEAD)" = "$revision"
xcodebuild -project "$work/Syphon.xcodeproj" -scheme Syphon -configuration Release \
  -derivedDataPath "$root/target/syphon-build" \
  ARCHS="$(uname -m)" ONLY_ACTIVE_ARCH=YES MACOSX_DEPLOYMENT_TARGET=13.0 \
  CODE_SIGNING_ALLOWED=NO build
test -d "$root/target/syphon-build/Build/Products/Release/Syphon.framework"
printf '%s\n' 'Syphon built. Set ARIA_SYPHON_FRAMEWORK to target/syphon-build/Build/Products/Release/Syphon.framework for cargo run.'
