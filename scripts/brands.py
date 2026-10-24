#!/usr/bin/env python3
"""Brand icons for the codes, from Simple Icons (https://simpleicons.org).

    npm pack simple-icons && tar xzf simple-icons-*.tgz
    scripts/brands.py package

writes crates/app/assets/brands.tsv.gz: one brand a line, tab-separated:
its slug, its name, its color (hex), its other names (|-separated), and
its icon, one 24x24 SVG path. The first line says which Simple Icons it is.

Only the icons under Simple Icons' own CC0 are taken: those with a license
of their own (custom, CC-BY-*, GPL, MIT, Apache, ...) are left out, so that
using them asks nothing of authwing. The brands' names and marks are their
owners' trademarks: authwing shows one only beside the user's own code for
that service.
"""

import gzip
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "crates" / "app" / "assets" / "brands.tsv.gz"


def main(package: pathlib.Path) -> None:
    meta = json.loads((package / "package.json").read_text())
    data = json.loads((package / "data" / "simple-icons.json").read_text())
    icons = data["icons"] if isinstance(data, dict) else data
    lines = [f"# simple-icons {meta['version']}"]
    left_out = 0
    for icon in icons:
        license = (icon.get("license") or {}).get("type")
        if license not in (None, "CC0-1.0"):
            left_out += 1
            continue
        slug = icon["slug"]
        svg = (package / "icons" / f"{slug}.svg").read_text()
        paths = re.findall(r'<path d="([^"]+)"', svg)
        if len(paths) != 1:
            left_out += 1
            continue
        aliases = icon.get("aliases") or {}
        others = list(aliases.get("aka", []))
        others += [d["title"] for d in aliases.get("dup", [])]
        others += list((aliases.get("loc") or {}).values())
        clean = lambda s: s.replace("\t", " ").replace("|", " ").replace("\n", " ")
        lines.append(
            "\t".join(
                [slug, clean(icon["title"]), icon["hex"], "|".join(map(clean, others)), paths[0]]
            )
        )
    OUT.write_bytes(gzip.compress(("\n".join(lines) + "\n").encode(), 9, mtime=0))
    print(f"{OUT.relative_to(ROOT)}: {len(lines) - 1} brands, {left_out} left out")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(pathlib.Path(sys.argv[1]))
