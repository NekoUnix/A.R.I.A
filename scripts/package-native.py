"""Stage native Alpha apps with docs/notices; package without private models/SDKs."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import plistlib
import platform
import shutil
import subprocess
import tarfile
import tempfile
import time
import tomllib

ROOT = Path(__file__).resolve().parent.parent
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
DIST = ROOT / "dist"

def run(*args, **kwargs):
    subprocess.run(args, check=True, **kwargs)

def copy(src, dst):
    dst.parent.mkdir(parents=True, exist_ok=True)
    if src.is_dir():
        shutil.copytree(src, dst, symlinks=True, ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.pyo"))
    else:
        shutil.copy2(src, dst)

def contents(stage):
    stage.mkdir(parents=True, exist_ok=True)
    for name in ["aria-desktop", "aria-cli", "aria-cubism-host"]:
        copy(ROOT / "target/release" / name, stage / name)
    for name in ["README.md", "CHANGELOG.md", "LICENSE", "THIRD_PARTY.md", "CONTRIBUTING.md", "SECURITY.md", "CODE_OF_CONDUCT.md", "docs", "templates", "tracking", "native"]:
        copy(ROOT / name, stage / name)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    (stage / "BUILD-INFO.txt").write_text(f"A.R.I.A. Alpha {VERSION}\nCommit: {revision}\nPlatform: {platform.system()} {platform.machine()}\nUnsigned alpha: see docs/platforms.md for setup and validation limits.\n")
    host = next(line.split(": ", 1)[1] for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if line.startswith("host: "))
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", host], cwd=ROOT))
    index = []
    for package in metadata["packages"]:
        if not package.get("source"):
            continue
        folder = Path(package["manifest_path"]).parent
        dest = stage / "dependency-licenses" / f'{package["name"]}-{package["version"]}'
        for notice in folder.rglob("*"):
            if notice.is_file() and any(word in notice.name.upper() for word in ["LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE", "OFL.TXT", "UFL.TXT"]):
                copy(notice, dest / notice.relative_to(folder))
        index.append(f'{package["name"]} {package["version"]}: {package.get("license")} {package.get("repository")}')
    (stage / "dependency-licenses/INDEX.txt").write_text("\n".join(index) + "\n")

def archive_tree(source, path):
    with tarfile.open(path, "w:gz") as archive:
        archive.add(source, arcname=source.name)

def linux(work):
    stage = work / f"aria-{VERSION}-linux-x64"
    contents(stage)
    copy(ROOT / "target/obs-plugin/aria-canvas.so", stage / "obs-plugin/aria-canvas.so")
    copy(ROOT / "native/linux-canvas", stage / "obs-plugin/source")
    copy(ROOT / "scripts/install-obs-linux.sh", stage / "install-obs-linux.sh")
    (stage / "install-obs-linux.sh").chmod(0o755)
    archive_tree(stage, DIST / f"{stage.name}.tar.gz")
    app = work / "app-root"
    copy(stage, app / "opt/aria-alpha")
    # Plugin is optional and packaged separately; no OBS dependency for the app.
    (app / "opt/aria-alpha/obs-plugin/aria-canvas.so").unlink()
    (app / "opt/aria-alpha/install-obs-linux.sh").unlink()
    for name in ["aria-desktop", "aria-cli", "aria-cubism-host"]:
        link = app / "usr/bin" / name
        link.parent.mkdir(parents=True, exist_ok=True)
        link.symlink_to(f"/opt/aria-alpha/{name}")
    copy(ROOT / "packaging/linux/com.nekounix.aria.desktop", app / "usr/share/applications/com.nekounix.aria.desktop")
    copy(ROOT / "packaging/linux/com.nekounix.aria.svg", app / "usr/share/icons/hicolor/scalable/apps/com.nekounix.aria.svg")
    plugin = work / "plugin-root"
    copy(ROOT / "target/obs-arch/aria-canvas.so", plugin / "usr/lib/obs-plugins/aria-canvas.so")
    copy(ROOT / "target/obs-arch/OBS-VERSION.txt", plugin / "usr/share/doc/aria-obs-canvas/OBS-VERSION.txt")
    copy(ROOT / "native/linux-canvas", plugin / "usr/share/doc/aria-obs-canvas/source")
    copy(ROOT / "docs/obs-output.md", plugin / "usr/share/doc/aria-obs-canvas/obs-output.md")
    for name, tree, deps, license_name in [
        ("aria-alpha", app, ["glibc>=2.39", "gcc-libs", "alsa-lib", "libxkbcommon", "wayland", "libx11", "libxcursor", "libxi", "libxrandr", "openssl", "vulkan-icd-loader", "systemd-libs"], "MIT"),
        ("aria-obs-canvas", plugin, ["glibc>=2.39", "obs-studio>=32"], "GPL-2.0-or-later")]:
        arch_package(name, tree, deps, license_name)
        rpm_package(name, tree, license_name, work)

def arch_package(name, tree, deps, license_name):
    version = VERSION.replace("-", "") + "-1"
    size = sum(p.stat().st_size for p in tree.rglob("*") if p.is_file() and not p.is_symlink())
    info = f"pkgname = {name}\npkgbase = {name}\npkgver = {version}\npkgdesc = Avatar Studio native Alpha build\nurl = https://github.com/NekoUnix/A.R.I.A\nbuilddate = {int(time.time())}\npackager = NekoUnix <nekounix@gmail.com>\nsize = {size}\narch = x86_64\nlicense = {license_name}\nxdata = pkgtype=pkg\n"
    info += "".join(f"depend = {dep}\n" for dep in deps)
    (tree / ".PKGINFO").write_text(info)
    dest = DIST / f"{name}-{version}-x86_64.pkg.tar.zst"
    with dest.open("wb") as output:
        tar = subprocess.Popen(["tar", "--owner=0", "--group=0", "-C", str(tree), "-cf", "-", "--", *sorted(p.name for p in tree.iterdir())], stdout=subprocess.PIPE)
        zstd = subprocess.run(["zstd", "-T0", "-10"], stdin=tar.stdout, stdout=output)
        tar.stdout.close()
        if tar.wait() or zstd.returncode:
            raise RuntimeError("Arch archive failed")
    (tree / ".PKGINFO").unlink()

def rpm_package(name, tree, license_name, work):
    top = work / f"rpm-{name}"
    top.mkdir()
    # Fedora uses /usr/lib64/obs-plugins; keep the Arch payload in /usr/lib.
    rpm_tree = work / f"rpm-root-{name}"
    copy(tree, rpm_tree)
    old = rpm_tree / "usr/lib/obs-plugins/aria-canvas.so"
    if old.exists():
        copy(ROOT / "target/obs-fedora/aria-canvas.so", rpm_tree / "usr/lib64/obs-plugins/aria-canvas.so")
        copy(ROOT / "target/obs-fedora/OBS-VERSION.txt", rpm_tree / "usr/share/doc/aria-obs-canvas/OBS-VERSION.txt")
        old.unlink()
    files = sorted('/' + str(p.relative_to(rpm_tree)) for p in rpm_tree.rglob('*') if p.is_file() or p.is_symlink())
    spec = top / f"{name}.spec"
    spec.write_text(f"""Name: {name}
Version: {VERSION.replace('-', '~')}
Release: 1
Summary: Avatar Studio native Alpha build
License: {license_name}
URL: https://github.com/NekoUnix/A.R.I.A
BuildArch: x86_64
{'Requires: obs-studio >= 32' if name == 'aria-obs-canvas' else 'Requires: vulkan-loader, libxkbcommon, libX11, libXcursor, libXi, libXrandr, libwayland-client, libwayland-cursor'}
%description
Native Alpha build. See the bundled documentation for setup and limitations.
%install
mkdir -p %{{buildroot}}
cp -a '{rpm_tree}/.' %{{buildroot}}/
%files
""" + '\n'.join('"' + f + '"' for f in files) + '\n')
    run("rpmbuild", "--define", f"_topdir {top}", "--define", "_build_id_links none", "--define", "__os_install_post %{nil}", "-bb", str(spec))
    for package in top.rglob("*.rpm"):
        copy(package, DIST / package.name)

def macos(work):
    app = work / "ARIA Alpha.app"
    binary = app / "Contents/MacOS"
    payload = work / "payload"
    contents(payload)
    for entry in payload.iterdir():
        target = binary if entry.name in ["aria-desktop", "aria-cli", "aria-cubism-host"] else app / "Contents/Resources"
        copy(entry, target / entry.name)
    framework = ROOT / "target/syphon-build/Build/Products/Release/Syphon.framework"
    copy(framework, app / "Contents/Frameworks/Syphon.framework")
    plist = dict(CFBundleName="ARIA Alpha", CFBundleDisplayName="ARIA Alpha", CFBundleIdentifier="com.nekounix.aria", CFBundleExecutable="aria-desktop", CFBundlePackageType="APPL", CFBundleShortVersionString=VERSION.partition("-")[0], CFBundleVersion=VERSION.partition("-")[0], CFBundleGetInfoString=f"ARIA Alpha {VERSION}", NSHighResolutionCapable=True, LSMinimumSystemVersion="13.0", NSMicrophoneUsageDescription="Use microphone amplitude to animate your avatar.", NSCameraUsageDescription="Use optional camera tracking to animate your avatar.")
    (app / "Contents/Info.plist").write_bytes(plistlib.dumps(plist))
    # Ad-hoc signatures support native Apple Silicon execution; not notarization.
    run("codesign", "--force", "--sign", "-", str(app / "Contents/Frameworks/Syphon.framework"))
    for name in ["aria-cli", "aria-cubism-host"]:
        run("codesign", "--force", "--sign", "-", str(binary / name))
    run("codesign", "--force", "--sign", "-", str(app))
    run("codesign", "--verify", "--deep", "--strict", str(app))
    run(str(binary / "aria-cli"), "--version")
    arch = "arm64" if platform.machine() == "arm64" else "x64"
    run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(DIST / f"aria-{VERSION}-macos-{arch}.zip"))

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("platform", choices=["linux", "macos"])
    args = parser.parse_args()
    assert "alpha" in VERSION, "This script produces Alpha packages only"
    DIST.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="aria-package-", dir=ROOT / "target") as temporary:
        {"linux": linux, "macos": macos}[args.platform](Path(temporary))
    for path in DIST.iterdir():
        if path.is_file() and path.suffix in [".gz", ".zip", ".rpm", ".zst"]:
            print(hashlib.sha256(path.read_bytes()).hexdigest(), path.name)
