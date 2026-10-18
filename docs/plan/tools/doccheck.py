#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Mechanical checks for the ElektraWatt documents (Python 3 standard library only).

  python3 -I docs/plan/tools/doccheck.py [files...]   # default: every .md under docs/ plus README.md and CLAUDE.md

Checks:
  - every relative link resolves to a file, and every #anchor to a heading of the target (GitHub slug rules);
  - every Markdown table has the same number of cells in each row as in its header;
  - every WP ID cited in a plan document exists in docs/plan/work-packages.csv;
  - the plan documents use the gate IDs G-M0 ... G-M6, G-PP and G-SP, never the retired short forms G0 ... G6 or G-1
    (G1 ... G6 only where used like a gate, because G1 ... G13 are also research gap topics).
Exit code 1 on any finding.
"""
import csv, re, sys
from pathlib import Path

DOCS = Path(__file__).resolve().parents[2]
ROOT = DOCS.parent


def slug(h):
    h = re.sub(r"`", "", h.strip().lower())
    h = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", h)
    h = re.sub(r"[^\w\- ]", "", h, flags=re.U)
    return h.replace(" ", "-")


_anchors = {}


def anchors(p):
    if p not in _anchors:
        s, cnt, fence = set(), {}, False
        for line in p.read_text(encoding="utf-8").split("\n"):
            if line.startswith("```"):
                fence = not fence
            if fence:
                continue
            m = re.match(r"^(#{1,6})\s+(.*)", line)
            if m:
                b = slug(m.group(2))
                k = cnt.get(b, 0)
                s.add(b if k == 0 else f"{b}-{k}")
                cnt[b] = k + 1
        _anchors[p] = s
    return _anchors[p]


def cells(line):
    s = re.sub(r"`[^`]*`", lambda m: m.group(0).replace("|", "¦"), line.strip()).replace("\\|", "¦")
    s = s[1:] if s.startswith("|") else s
    s = s[:-1] if s.endswith("|") else s
    return s.split("|")


def check(paths):
    bad = []
    wp_ids = {r["wp_id"] for r in csv.DictReader(open(DOCS / "plan" / "work-packages.csv", encoding="utf-8"))}
    for f in paths:
        txt = f.read_text(encoding="utf-8")
        body = re.sub(r"```.*?```", "", txt, flags=re.S)
        body = re.sub(r"`[^`\n]*`", "", body)
        for m in re.finditer(r"\]\(([^)\s]+)\)", body):
            u = m.group(1)
            if u.startswith(("http:", "https:", "mailto:")):
                continue
            path, _, frag = u.partition("#")
            tgt = (f.parent / path).resolve() if path else f
            if not tgt.exists():
                bad.append(f"{f.relative_to(ROOT)}: link target missing: {u}")
            elif frag and tgt.suffix == ".md" and frag not in anchors(tgt):
                bad.append(f"{f.relative_to(ROOT)}: anchor missing: {u}")
        lines, fence, i = txt.split("\n"), False, 0
        while i < len(lines):
            line = lines[i]
            if line.startswith("```"):
                fence = not fence
            if not fence and line.lstrip().startswith("|") and i + 1 < len(lines) and \
                    re.match(r"^\s*\|\s*:?-{3,}", lines[i + 1]):
                n, j = len(cells(line)), i + 1
                while j < len(lines) and lines[j].lstrip().startswith("|"):
                    if len(cells(lines[j])) != n:
                        bad.append(f"{f.relative_to(ROOT)}:{j + 1}: {len(cells(lines[j]))} cells, header has {n}")
                    j += 1
                i = j
                continue
            i += 1
        if f.parent.name == "plan" or f.parent.parent.name == "plan":
            for m in re.finditer(r"WP-S\d\.\d\d[a-z]?", txt):
                if m.group(0) not in wp_ids:
                    bad.append(f"{f.relative_to(ROOT)}: unknown WP ID {m.group(0)}")
            for n, line in enumerate(txt.split("\n"), 1):
                # G0 and G-1 are unambiguous; G1 … G6 are also research gap topics, so only gate-like uses count
                pat = r"(?<![\w-])(G-1|G0)(?![\w-])|\*\*(G[1-6])\*\*|(?<![\w-])(G[1-6])(?= gate| review|-\d\d|\s*\(M\d)"
                for m in re.finditer(pat, line):
                    g = next(x for x in m.groups() if x)
                    bad.append(f"{f.relative_to(ROOT)}:{n}: retired gate ID {g} (use G-M0 … G-M6, G-SP)")
    return bad


if __name__ == "__main__":
    args = [Path(a).resolve() for a in sys.argv[1:]]
    paths = args or sorted(DOCS.rglob("*.md")) + [p for p in (ROOT / "README.md", ROOT / "CLAUDE.md") if p.exists()]
    found = check(paths)
    for b in found:
        print("doccheck.py:", b)
    print(f"doccheck.py: {len(paths)} files, {len(found)} findings")
    sys.exit(1 if found else 0)
