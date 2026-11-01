#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Mechanical checks for the ElektraWatt documents (Python 3 standard library only).

  python3 -I docs/plan/tools/doccheck.py [files...]   # default: every .md under docs/ plus README.md and CLAUDE.md
  python3 -I docs/plan/tools/doccheck.py --test       # offline unit test over a fixture repository in a temp dir

Checks:
  - every relative link resolves to a file, and every #anchor to a heading of the target (GitHub slug rules); a
    missing target that git ignores is built, not committed (the book's reference pages, `cargo xtask docs-gen`),
    so it is no finding (one batched `git check-ignore --stdin` call);
  - every Markdown table has the same number of cells in each row as in its header;
  - every WP ID cited in a plan document exists in docs/plan/work-packages.csv;
  - the plan documents use the gate IDs G-M0 ... G-M6, G-PP and G-SP, never the retired short forms G0 ... G6 or G-1
    (G1 ... G6 only where used like a gate, because G1 ... G13 are also research gap topics).
Exit code 1 on any finding.
"""
import csv, re, subprocess, sys
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


def rel_of(root, p):
    """The repo-relative `/`-separated path of `p` (absolute), or None outside `root`."""
    try:
        return p.relative_to(root).as_posix()
    except ValueError:
        return None


def ignored(root, paths):
    """The repo-relative `/`-separated paths of `paths` (absolute) that git ignores at `root`: one batched
    `git check-ignore --stdin` call; none when git is missing or `root` is no repository."""
    rels = sorted({r for r in (rel_of(root, p) for p in paths) if r})
    if not rels:
        return set()
    try:
        # NUL-separated bytes both ways: no newline translation, no quoting of unusual names.
        out = subprocess.run(["git", "-C", str(root), "check-ignore", "-z", "--stdin"],
                             input="\0".join(rels).encode("utf-8") + b"\0", capture_output=True)
    except OSError:
        return set()
    # Exit code 1: none is ignored; 128: an error (no repository), so nothing counts as ignored.
    return {r for r in out.stdout.decode("utf-8").split("\0") if r} if out.returncode == 0 else set()


def check(paths, root=ROOT):
    bad, missing = [], []
    wp_ids = None
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
                missing.append((tgt, f"{rel_of(root, f)}: link target missing: {u}"))
            elif frag and tgt.suffix == ".md" and frag not in anchors(tgt):
                bad.append(f"{rel_of(root, f)}: anchor missing: {u}")
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
                        bad.append(f"{rel_of(root, f)}:{j + 1}: {len(cells(lines[j]))} cells, header has {n}")
                    j += 1
                i = j
                continue
            i += 1
        if f.parent.name == "plan" or f.parent.parent.name == "plan":
            if wp_ids is None:
                with open(root / "docs" / "plan" / "work-packages.csv", encoding="utf-8") as table:
                    wp_ids = {r["wp_id"] for r in csv.DictReader(table)}
            for m in re.finditer(r"WP-S\d\.\d\d[a-z]?", txt):
                if m.group(0) not in wp_ids:
                    bad.append(f"{rel_of(root, f)}: unknown WP ID {m.group(0)}")
            for n, line in enumerate(txt.split("\n"), 1):
                # G0 and G-1 are unambiguous; G1 … G6 are also research gap topics, so only gate-like uses count
                pat = r"(?<![\w-])(G-1|G0)(?![\w-])|\*\*(G[1-6])\*\*|(?<![\w-])(G[1-6])(?= gate| review|-\d\d|\s*\(M\d)"
                for m in re.finditer(pat, line):
                    g = next(x for x in m.groups() if x)
                    bad.append(f"{rel_of(root, f)}:{n}: retired gate ID {g} (use G-M0 … G-M6, G-SP)")
    skip = ignored(root, [tgt for tgt, _ in missing])
    bad.extend(msg for tgt, msg in missing if rel_of(root, tgt) not in skip)
    return bad


def selftest():
    import os, tempfile, unittest

    class T(unittest.TestCase):
        def setUp(self):
            self._tmp = tempfile.TemporaryDirectory()
            self.root = Path(self._tmp.name).resolve() / "repo"
            self.root.mkdir()
            env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
            subprocess.run(["git", "init", "--quiet", str(self.root)], check=True, env=env)
            (self.root / ".gitignore").write_text("/docs/built/\n/docs/page.md\n", encoding="utf-8")
            (self.root / "docs").mkdir()

        def tearDown(self):
            self._tmp.cleanup()

        def doc(self, text):
            f = self.root / "docs" / "a.md"
            f.write_text(text, encoding="utf-8")
            return f

        def test_an_ignored_missing_target_is_no_finding(self):
            f = self.doc("[x](built/one.md) [y](page.md#top) [z](built/sub/two.md)\n")
            self.assertEqual(check([f], self.root), [])

        def test_a_missing_target_git_does_not_ignore_is_a_finding(self):
            f = self.doc("[x](built/one.md) [y](gone.md) [z](../../outside.md#a)\n")
            self.assertEqual(check([f], self.root), [
                "docs/a.md: link target missing: gone.md",
                "docs/a.md: link target missing: ../../outside.md#a",
            ])

        def test_an_existing_ignored_target_is_still_checked(self):
            (self.root / "docs" / "built").mkdir()
            (self.root / "docs" / "built" / "one.md").write_text("# One\n", encoding="utf-8")
            f = self.doc("[x](built/one.md#one) [y](built/one.md#two)\n")
            self.assertEqual(check([f], self.root), ["docs/a.md: anchor missing: built/one.md#two"])

        def test_outside_a_repository_nothing_is_ignored(self):
            outside = self.root.parent
            self.assertEqual(ignored(outside, [outside / "docs" / "built" / "one.md"]), set())

    suite = unittest.defaultTestLoader.loadTestsFromTestCase(T)
    return 0 if unittest.TextTestRunner(verbosity=1).run(suite).wasSuccessful() else 1


if __name__ == "__main__":
    if "--test" in sys.argv:
        sys.exit(selftest())
    args = [Path(a).resolve() for a in sys.argv[1:]]
    paths = args or sorted(DOCS.rglob("*.md")) + [p for p in (ROOT / "README.md", ROOT / "CLAUDE.md") if p.exists()]
    found = check(paths)
    for b in found:
        print("doccheck.py:", b)
    print(f"doccheck.py: {len(paths)} files, {len(found)} findings")
    sys.exit(1 if found else 0)
