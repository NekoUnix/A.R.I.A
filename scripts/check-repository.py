"""Offline checks for repository documentation and shipped development contracts."""
from pathlib import Path
import json
import re
import subprocess
import sys
import tomllib
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parent.parent


def prose(text):
    """Skip fenced examples so example links are not treated as navigation."""
    fence = None
    for line in text.splitlines():
        marker = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if marker:
            if fence is None:
                fence = marker[1]
            elif marker[1][0] == fence[0] and len(marker[1]) >= len(fence):
                fence = None
            continue
        if fence is None:
            yield line


def anchors(text):
    counts, result = {}, set()
    for line in prose(text):
        heading = re.match(r"^ {0,3}#{1,6}\s+(.+?)(?:\s+#+)?$", line)
        if not heading:
            continue
        title = re.sub(r"\[([^]]+)\]\([^)]*\)", r"\1", heading[1])
        title = re.sub(r"<[^>]+>", "", title).replace("`", "").replace("*", "")
        slug = re.sub(r"[^\w\-\s]", "", title.lower()).replace(" ", "-")
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        result.add(slug if count == 0 else f"{slug}-{count}")
    result.update(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)', text))
    return result


def check():
    files = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT
    ).decode("utf-8").split("\0")
    files = sorted({name for name in files if name and (ROOT / name).is_file()})
    errors, markdown_count, json_count = [], 0, 0
    for name in files:
        path = ROOT / name
        if path.suffix == ".json":
            try:
                json.loads(path.read_text(encoding="utf-8-sig"))
                json_count += 1
            except (ValueError, UnicodeError) as error:
                errors.append(f"{name}: invalid JSON: {error}")
        if path.suffix != ".md":
            continue
        markdown_count += 1
        content = path.read_text(encoding="utf-8-sig")
        for line in prose(content):
            for target in re.findall(r"\]\((<[^>]+>|[^)\n]+)\)", line):
                target = re.sub(r'\s+"[^"]*"$', "", target).strip("<>")
                url = urlsplit(target)
                if url.scheme or url.netloc:
                    continue
                linked = (ROOT / unquote(url.path).lstrip("/") if url.path.startswith("/")
                          else path.parent / unquote(url.path)) if url.path else path
                linked = linked.resolve()
                if not linked.is_relative_to(ROOT):
                    errors.append(f"{name}: link escapes repository: {target}")
                elif not linked.exists():
                    errors.append(f"{name}: missing link target: {target}")
                elif linked.suffix == ".md" and url.fragment:
                    if unquote(url.fragment) not in anchors(linked.read_text(encoding="utf-8-sig")):
                        errors.append(f"{name}: missing heading: {target}")

    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    version = cargo["workspace"]["package"]["version"]
    desktop = tomllib.loads((ROOT / "apps/aria-desktop/Cargo.toml").read_text(encoding="utf-8"))
    if "links" not in desktop["dependencies"]["eframe"].get("features", []):
        errors.append("Desktop eframe must enable links for native browser navigation")
    package = (ROOT / "scripts/build-windows.ps1").read_text(encoding="utf-8-sig")
    workflow = (ROOT / ".github/workflows/windows.yml").read_text(encoding="utf-8")
    if f"$ariaVersion = '{version}'" not in package:
        errors.append("Package version differs from Cargo workspace version")
    if f"aria-{version}-windows-x64.zip" not in workflow:
        errors.append("Windows artifact path differs from Cargo workspace version")
    locked = {}
    for line in (ROOT / "tracking/requirements-lock.txt").read_text().splitlines():
        if "==" in line:
            name, value = line.split("==", 1)
            locked[re.sub(r"[-_.]+", "-", name).lower()] = value
    for line in (ROOT / "tracking/requirements.txt").read_text().splitlines():
        if "==" in line:
            name, value = line.split("==", 1)
            if locked.get(re.sub(r"[-_.]+", "-", name).lower()) != value:
                errors.append(f"Camera direct dependency differs from lock: {name}")
    for name in files:
        if name.startswith(".github/workflows/") and name.endswith((".yml", ".yaml")):
            for action in re.findall(r"uses:\s*([^\s#]+)", (ROOT / name).read_text()):
                if not re.fullmatch(r"[\w.-]+/[\w./-]+@[0-9a-f]{40}", action):
                    errors.append(f"{name}: action must use a full commit SHA: {action}")
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Validated {markdown_count} Markdown files, {json_count} JSON files, "
          "local paths/anchors, dependency pins and package/workflow consistency.")
    return 0


if __name__ == "__main__":
    sys.exit(check())
