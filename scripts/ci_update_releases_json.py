#!/usr/bin/env python3
"""Update site/releases.json for a stable tag. Usage: ci_update_releases_json.py VERSION PATH"""
from __future__ import annotations

import json
import re
import sys
from datetime import date
from pathlib import Path


def changelog_notes(version: str) -> str:
    text = Path("CHANGELOG.md").read_text(encoding="utf-8")
    m = re.search(
        rf"## \[{re.escape(version)}\][^\n]*\n(.*?)(?=\n## |\Z)",
        text,
        re.S,
    )
    if not m:
        return ""
    lines = []
    for ln in m.group(1).splitlines():
        s = ln.strip()
        if not s or s.startswith("#"):
            continue
        s = s.lstrip("-* ").strip()
        if s:
            lines.append(s)
    return "; ".join(lines)[:500]


def main() -> None:
    if len(sys.argv) != 3:
        print("usage: ci_update_releases_json.py VERSION PATH", file=sys.stderr)
        sys.exit(2)
    version, path_s = sys.argv[1], sys.argv[2]
    path = Path(path_s)
    data: dict = {"latest": version, "releases": []}
    if path.exists():
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            pass
    data["latest"] = version
    releases = [r for r in data.get("releases", []) if r.get("version") != version]
    releases.insert(
        0,
        {
            "version": version,
            "url": f"https://boormat.github.io/khanatime/v{version}/",
            "released_at": date.today().isoformat(),
            "channel": "stable",
            "notes": changelog_notes(version),
            "notes_url": f"https://github.com/boormat/khanatime/releases/tag/v{version}",
        },
    )
    data["releases"] = releases
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    main()
