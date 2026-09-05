#!/usr/bin/env python3
"""Write site/index.html — human catalog of published Khanatime builds.

Usage: ci_write_pages_index.py SITE_DIR
Scans SITE_DIR/v* and SITE_DIR/main, merges notes from releases.json when present.
"""
from __future__ import annotations

import html
import json
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: ci_write_pages_index.py SITE_DIR", file=sys.stderr)
        sys.exit(2)
    site = Path(sys.argv[1])
    site.mkdir(parents=True, exist_ok=True)

    notes: dict[str, str] = {}
    latest = ""
    releases_path = site / "releases.json"
    if releases_path.exists():
        try:
            data = json.loads(releases_path.read_text(encoding="utf-8"))
            latest = str(data.get("latest") or "")
            for r in data.get("releases") or []:
                ver = str(r.get("version") or "")
                if ver:
                    notes[ver] = str(r.get("notes") or "")
        except json.JSONDecodeError:
            pass

    versions = sorted(
        (p.name[1:] for p in site.glob("v*") if p.is_dir() and p.name[1:2].isdigit()),
        key=lambda v: [int(x) if x.isdigit() else 0 for x in v.split(".")],
        reverse=True,
    )
    if not latest and versions:
        latest = versions[0]

    has_main = (site / "main").is_dir()
    has_latest = (site / "latest").is_dir()

    rows = []
    for ver in versions:
        note = html.escape(notes.get(ver, ""))
        mark = " ← latest" if ver == latest else ""
        rows.append(
            f'<li><a href="v{html.escape(ver)}/"><strong>v{html.escape(ver)}</strong></a>'
            f"{html.escape(mark)}"
            + (f' — <span class="notes">{note}</span>' if note else "")
            + "</li>"
        )

    if latest:
        latest_block = (
            f'<p class="lead"><a class="button" href="v{html.escape(latest)}/">'
            f"Open latest stable (v{html.escape(latest)})</a></p>"
        )
        if has_latest:
            latest_block += (
                f'<p>Also at <a href="latest/"><code>/latest/</code></a> '
                f"(same bits as v{html.escape(latest)}).</p>"
            )
    elif has_latest:
        latest_block = (
            '<p class="lead"><a class="button" href="latest/">Open /latest/</a></p>'
        )
    else:
        latest_block = ""

    preview_block = ""
    if has_main:
        preview_block = (
            "<h2>Prerelease</h2>"
            "<ul><li><a href=\"main/\"><strong>main</strong></a> "
            "(tip of <code>main</code>; Help shows <code>dev-&lt;sha&gt;</code>)</li></ul>"
        )

    body = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Khanatime — releases</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 40rem; margin: 2rem auto; padding: 0 1rem; line-height: 1.45; }}
    h1 {{ font-size: 1.5rem; }}
    .lead {{ margin: 1.25rem 0; }}
    .button {{ display: inline-block; background: #3273dc; color: #fff; text-decoration: none;
               padding: 0.5rem 1rem; border-radius: 4px; font-weight: 600; }}
    .notes {{ color: #555; font-size: 0.9rem; }}
    code {{ font-size: 0.9em; }}
    footer {{ margin-top: 2rem; color: #666; font-size: 0.85rem; }}
  </style>
</head>
<body>
  <h1>Khanatime</h1>
  <p>Pick a <strong>pinned</strong> build for event invite QRs
     (<code>/vX.Y.Z/</code>). Do not use this catalog URL or <code>/main/</code>
     as a publish invite base.</p>
  {latest_block}
  <h2>Stable releases</h2>
  <ul>
    {"".join(rows) if rows else "<li><em>No tagged releases published yet.</em></li>"}
  </ul>
  {preview_block}
  <footer>
    Machine-readable: <a href="releases.json">releases.json</a>
  </footer>
</body>
</html>
"""
    (site / "index.html").write_text(body, encoding="utf-8")
    print(f"wrote {site / 'index.html'} ({len(versions)} stable, main={has_main})")


if __name__ == "__main__":
    main()
