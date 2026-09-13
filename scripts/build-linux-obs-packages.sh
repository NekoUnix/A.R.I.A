#!/bin/sh
# Match each distribution's libobs SONAME and headers; Ubuntu's portable plugin
# cannot be assumed to load against Fedora/Arch's libobs.so.30.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$root/target/obs-fedora" "$root/target/obs-arch"
docker run --rm -v "$root/native/linux-canvas:/src:ro" -v "$root/target/obs-fedora:/out" fedora:44 sh -c '
  set -eu
  dnf install -y gcc cmake make pkgconf-pkg-config obs-studio-devel simde-devel
  cmake -S /src -B /out -DCMAKE_BUILD_TYPE=Release
  cmake --build /out --parallel 2
  ctest --test-dir /out --output-on-failure
  pkg-config --modversion libobs > /out/OBS-VERSION.txt
'
docker run --rm -v "$root/native/linux-canvas:/src:ro" -v "$root/target/obs-arch:/out" archlinux:base sh -c '
  set -eu
  pacman -Syu --noconfirm --needed gcc cmake make pkgconf obs-studio simde
  cmake -S /src -B /out -DCMAKE_BUILD_TYPE=Release
  cmake --build /out --parallel 2
  ctest --test-dir /out --output-on-failure
  pkg-config --modversion libobs > /out/OBS-VERSION.txt
'
