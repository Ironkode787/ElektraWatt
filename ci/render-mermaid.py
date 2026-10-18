# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Render every Mermaid block of the repository's Markdown and fail on any error (CI-PR-15).

GitHub shows a broken Mermaid diagram as an error box without failing anything, so this job renders each
block with a pinned mermaid-cli container image and fails on any parse or render error.

  python3 -I ci/render-mermaid.py --image <ghcr.io/...@sha256:...>   # extract and render (needs docker)
  python3 -I ci/render-mermaid.py --extract-only                       # extract and list, no rendering

Sources: every *.md under docs/ (and book/ once it exists) and the *.md files in the repository root.
Python 3 standard library only.
"""
import argparse
import concurrent.futures
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_DIRS = ["docs", "book"]
FENCE_OPEN = re.compile(r"^(\s*)(`{3,}|~{3,})\s*mermaid\s*$")
# Markers mermaid puts into an SVG when it renders an error diagram instead of failing.
ERROR_MARKERS = ('aria-roledescription="error"', "Syntax error in text")


def markdown_files():
    files = sorted(p for p in ROOT.glob("*.md") if p.is_file())
    for d in SOURCE_DIRS:
        if (ROOT / d).is_dir():
            files += sorted((ROOT / d).rglob("*.md"))
    return files


def extract(path):
    """Yield (line number of the opening fence, block text) for each Mermaid block of a Markdown file."""
    lines = path.read_text(encoding="utf-8").split("\n")
    i = 0
    while i < len(lines):
        m = FENCE_OPEN.match(lines[i])
        if not m:
            i += 1
            continue
        indent, fence = m.group(1), m.group(2)
        start, body = i + 1, []
        i += 1
        while i < len(lines) and not lines[i].strip().startswith(fence[0] * len(fence)):
            line = lines[i]
            body.append(line[len(indent):] if line.startswith(indent) else line)
            i += 1
        if i == len(lines):
            raise SystemExit(f"{path.relative_to(ROOT)}:{start}: unterminated mermaid block")
        yield start, "\n".join(body) + "\n"
        i += 1


def render(image, workdir, name):
    """Render workdir/name.mmd to name.svg in a container; return an error message or None."""
    user = f"{os.getuid()}:{os.getgid()}" if hasattr(os, "getuid") else None
    cmd = ["docker", "run", "--rm", "--network", "none", "-v", f"{workdir}:/data"]
    if user:
        cmd += ["-u", user]
    cmd += [image, "-i", f"/data/{name}.mmd", "-o", f"/data/{name}.svg"]
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
    svg = Path(workdir) / f"{name}.svg"
    if proc.returncode != 0:
        return (proc.stderr or proc.stdout).strip() or f"mermaid-cli exited with {proc.returncode}"
    if not svg.is_file() or svg.stat().st_size == 0:
        return "mermaid-cli produced no SVG"
    text = svg.read_text(encoding="utf-8", errors="replace")
    if any(marker in text for marker in ERROR_MARKERS):
        return "mermaid rendered an error diagram"
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--image", help="mermaid-cli container image, pinned by digest")
    ap.add_argument("--extract-only", action="store_true", help="extract and list the blocks, do not render")
    ap.add_argument("--jobs", type=int, default=4, help="parallel renders (default 4)")
    args = ap.parse_args()
    if not args.extract_only and not args.image:
        ap.error("--image is required unless --extract-only is given")
    if args.image and "@sha256:" not in args.image:
        ap.error("--image must be pinned by digest (name@sha256:...)")

    workdir = tempfile.mkdtemp(prefix="mermaid-")
    blocks = []  # (name, "path:line")
    for path in markdown_files():
        for line, text in extract(path):
            name = f"d{len(blocks) + 1:04d}"
            Path(workdir, f"{name}.mmd").write_text(text, encoding="utf-8", newline="\n")
            blocks.append((name, f"{path.relative_to(ROOT).as_posix()}:{line}"))
    print(f"render-mermaid: {len(blocks)} blocks extracted to {workdir}")

    if args.extract_only:
        for name, where in blocks:
            print(f"  {name}  {where}")
        return 0

    failures = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        futures = {pool.submit(render, args.image, workdir, name): where for name, where in blocks}
        for future in concurrent.futures.as_completed(futures):
            where = futures[future]
            try:
                error = future.result()
            except Exception as exc:  # timeout or docker failure counts as a render error
                error = f"{type(exc).__name__}: {exc}"
            if error:
                failures.append((where, error))
    for where, error in sorted(failures):
        print(f"::error file={where.split(':')[0]},line={where.split(':')[1]}::mermaid render failed")
        print(f"{where}: {error}", file=sys.stderr)
    print(f"render-mermaid: {len(blocks) - len(failures)} of {len(blocks)} blocks rendered")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
