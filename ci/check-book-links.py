#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Link check of the built user-documentation book (NFR-DOC-01, NFR-MAINT-08; Python 3 standard library only).

  python3 -I ci/check-book-links.py <built-book-dir> [--repo-root <dir>]

The book is built offline, so this check never touches the network. It reads every HTML page of the built book
(`mdbook build docs/book`; `print.html` and `toc.html` repeat the other pages and are skipped) and fails when

  - a relative link, image, stylesheet or script does not resolve to a file of the built book;
  - a `#fragment` does not name an `id` (or `name`) of the target page;
  - a link into this repository (`https://github.com/<owner>/<repo>/blob|tree/<branch>/<path>[#anchor]`, how the book links
    the design documents, which are not part of the book) names a file that is not in the checkout at `--repo-root`
    (default: the current directory), or an anchor that is not a heading of that Markdown file (GitHub slug rules).

Other `http(s)` links are counted and not followed. Exit code 1 on any finding.
"""
import argparse
import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

REPO_BLOB = re.compile(r"^https://github\.com/[^/]+/[^/]+/(?P<kind>blob|tree)/[^/]+/(?P<path>[^#?]+)(?:#(?P<frag>.*))?$")
SKIPPED_PAGES = {"print.html", "toc.html"}
LINK_ATTRS = {"a": "href", "link": "href", "img": "src", "script": "src", "source": "src", "iframe": "src"}


class Page(HTMLParser):
    """The ids of a page and the links it makes."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.links = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        for key in ("id", "name"):
            if attrs.get(key):
                self.ids.add(attrs[key])
        attr = LINK_ATTRS.get(tag)
        if attr and attrs.get(attr) is not None:
            self.links.append(attrs[attr])


def parse(path):
    page = Page()
    page.feed(path.read_text(encoding="utf-8"))
    return page


def slug(heading):
    """GitHub's heading anchor (the rules of docs/plan/tools/doccheck.py)."""
    h = re.sub(r"`", "", heading.strip().lower())
    h = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", h)
    h = re.sub(r"[^\w\- ]", "", h, flags=re.U)
    return h.replace(" ", "-")


def markdown_anchors(path):
    anchors, counts, fence = set(), {}, False
    for line in path.read_text(encoding="utf-8").split("\n"):
        if line.startswith("```"):
            fence = not fence
        if fence:
            continue
        m = re.match(r"^#{1,6}\s+(.*)", line)
        if m:
            base = slug(m.group(1))
            n = counts.get(base, 0)
            anchors.add(base if n == 0 else f"{base}-{n}")
            counts[base] = n + 1
    return anchors


def check(book, repo_root):
    findings, pages, external, repo_links = [], {}, 0, 0
    html_files = sorted(p for p in book.rglob("*.html") if p.name not in SKIPPED_PAGES)
    for path in html_files:
        pages[path.resolve()] = parse(path)
    md_anchors = {}
    for path in html_files:
        page = pages[path.resolve()]
        where = path.relative_to(book).as_posix()
        for link in page.links:
            link = link.strip()
            parts = urlsplit(link)
            if parts.scheme in ("mailto", "javascript", "data", "tel"):
                continue
            if parts.scheme in ("http", "https"):
                m = REPO_BLOB.match(link)
                if not m:
                    external += 1
                    continue
                repo_links += 1
                target = repo_root / unquote(m.group("path"))
                if not (target.is_dir() if m.group("kind") == "tree" else target.is_file()):
                    findings.append(f"{where}: repository file missing: {link}")
                elif m.group("frag") and target.suffix == ".md":
                    anchors = md_anchors.setdefault(target, markdown_anchors(target))
                    if m.group("frag") not in anchors:
                        findings.append(f"{where}: anchor missing in the repository file: {link}")
                continue
            if parts.scheme or parts.netloc:
                findings.append(f"{where}: unsupported link: {link}")
                continue
            if parts.path:
                # A path that starts with `/` is relative to the site root, which is the book directory.
                base = book if parts.path.startswith("/") else path.parent
                target = (base / unquote(parts.path).lstrip("/")).resolve()
                if target.is_dir():
                    target = target / "index.html"
                if not target.is_file() or book.resolve() not in target.parents:
                    findings.append(f"{where}: link target missing: {link}")
                    continue
            else:
                target = path.resolve()
            if parts.fragment and target.suffix == ".html" and target.name not in SKIPPED_PAGES:
                tpage = pages.get(target) or parse(target)
                if unquote(parts.fragment) not in tpage.ids:
                    findings.append(f"{where}: anchor missing: {link}")
    return findings, len(html_files), external, repo_links


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("book", type=Path, help="the built book directory (mdbook build output)")
    ap.add_argument("--repo-root", type=Path, default=Path.cwd(), help="the repository checkout (default: cwd)")
    args = ap.parse_args()
    if not args.book.is_dir() or not (args.book / "index.html").is_file():
        print(f"check-book-links.py: {args.book} is not a built book (no index.html)", file=sys.stderr)
        return 2
    findings, pages, external, repo_links = check(args.book.resolve(), args.repo_root.resolve())
    for f in findings:
        print("check-book-links.py:", f)
    print(
        f"check-book-links.py: {pages} pages, {repo_links} repository links, {external} external links not followed, "
        f"{len(findings)} findings"
    )
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
