"""Reject proprietary SDK, copied core, and user model assets in source or releases."""
from pathlib import Path
import argparse
import subprocess

ROOT = Path(__file__).resolve().parent.parent


def prohibited(path):
    name = path.name.lower()
    return ("live2dcubismcore" in name or any("purism" in part.lower() for part in path.parts)
            or path.suffix.lower() in {".moc", ".moc3", ".cmo3"}
            or name.endswith((".model3.json", ".physics3.json", ".motion3.json", ".exp3.json")))


def check(stage=None):
    errors = []
    if stage is None:
        names = subprocess.check_output(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
        ).decode().split("\0")
        files = [ROOT / name for name in names if name and (ROOT / name).is_file()]
    else:
        stage = Path(stage)
        if not stage.is_dir():
            return [f"Missing release directory: {stage}"]
        files = list(stage.rglob("*"))
    errors.extend(f"Prohibited runtime/model release path: {p}" for p in files if prohibited(p))
    return errors


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", type=Path, help="Application resource root before packaging")
    errors = check(parser.parse_args().stage)
    if errors:
        raise SystemExit("\n".join(errors))
    print("Rust-only runtime and model distribution checks passed.")
