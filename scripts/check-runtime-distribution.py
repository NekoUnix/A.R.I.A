"""Check Purism provenance and reject SDK binaries/raw models in release trees."""
from pathlib import Path
import argparse
import hashlib
import subprocess

ROOT = Path(__file__).resolve().parent.parent
BUNDLE = Path("crates/aria-live2d/vendor/purism-core/PurismCoreBundle.h")
SHA256 = "68e57128c18a489d01fb66497738abfeae448b266c302d25cef118012a264603"
NOTICE = Path("docs/licenses/purism-core.txt")


def prohibited(path):
    name = path.name.lower()
    return ("live2dcubismcore" in name or path.suffix.lower() in {".moc", ".moc3", ".cmo3"}
            or name.endswith((".model3.json", ".physics3.json", ".motion3.json", ".exp3.json")))


def check(stage=None):
    errors = []
    source_notice = ROOT / "crates/aria-live2d/vendor/purism-core/LICENSE"
    if hashlib.sha256((ROOT / BUNDLE).read_bytes()).hexdigest() != SHA256:
        errors.append("Purism source checksum changed; review and update pinned provenance")
    if (ROOT / NOTICE).read_bytes() != source_notice.read_bytes():
        errors.append("Shipped Purism notice differs from the vendored MIT license")
    if stage is None:
        names = subprocess.check_output(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
        ).decode().split("\0")
        files = [ROOT / name for name in names if name and (ROOT / name).is_file()]
    else:
        stage = Path(stage)
        if not stage.is_dir():
            return [f"Missing release directory: {stage}"]
        notice = stage / NOTICE
        if not notice.is_file() or notice.read_bytes() != source_notice.read_bytes():
            errors.append("Release is missing the exact Purism MIT notice at docs/licenses/purism-core.txt")
        files = list(stage.rglob("*"))
    errors.extend(f"Prohibited runtime/model release path: {p}" for p in files if prohibited(p))
    return errors


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, help="Application resource root before packaging")
    errors = check(parser.parse_args().stage)
    if errors:
        raise SystemExit("\n".join(errors))
    print("Purism source, MIT notice and runtime/model distribution checks passed.")
