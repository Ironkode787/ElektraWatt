#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The UI stack's lag behind its newest releases (WP-S6.06 D17; NFR-MAINT-04; doc 18 §6.3, CI-W-03's lag half).

  python3 -I ci/ui-stack-lag.py [--lock Cargo.lock]
  python3 -I ci/ui-stack-lag.py --test

Reads the locked versions of the UI stack (CRATES) from Cargo.lock and, from the crates.io sparse index
(https://index.crates.io), the newest release of each that is not yanked and not a pre-release, and prints a Markdown
table: crate, locked, newest, lag. A crate locked at several versions counts with its highest; one that Cargo.lock does
not hold reads "not locked". It fails (exit 1, each failure on a line after the table) when egui is more than two
minor releases behind or wgpu more than one major release behind: the limits of NFR-MAINT-04 that doc 18 §6.3's
upgrade train keeps (.github/ISSUE_TEMPLATE/upgrade-train.yml). For a 0.x crate the minor is its breaking line.
`--test` is the offline unit test, over a fixture lock and index. Standard library only.
"""
import argparse
import json
import os
import sys
import time
import tomllib
import unittest
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATES = ("egui", "eframe", "egui-wgpu", "egui_tiles", "egui_table", "egui_ltreeview", "egui_kittest", "wgpu", "winit")
# NFR-MAINT-04: (crate, the part of the version that counts, how many releases behind it may be).
LIMITS = (("egui", "minor", 2), ("wgpu", "major", 1))
INDEX = "https://index.crates.io"


def version_key(version):
    """(major, minor, patch) of a release version such as `0.37.1`."""
    major, minor, patch = (version.split("+", 1)[0].split(".") + ["0", "0"])[:3]
    return int(major), int(minor), int(patch)


def index_path(name):
    """The crate's path in the sparse index (the Cargo registry layout)."""
    name = name.lower()
    if len(name) <= 2:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def fetch_index(name):
    """The sparse index's lines of the crate `name` (with network; three attempts)."""
    request = urllib.request.Request(f"{INDEX}/{index_path(name)}", headers={"User-Agent": "elektrawatt-ui-stack-lag"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                return response.read().decode("utf-8").splitlines()
        except OSError:
            if attempt == 2:
                raise
            time.sleep(5 * (attempt + 1))
    return []


def newest(lines):
    """The newest version of the index `lines` that is neither yanked nor a pre-release, or None."""
    releases = []
    for line in lines:
        if line.strip():
            entry = json.loads(line)
            if not entry.get("yanked") and "-" not in entry["vers"].split("+", 1)[0]:
                releases.append(entry["vers"])
    return max(releases, key=version_key, default=None)


def locked(lock_text):
    """{crate: highest locked version} of the crates of CRATES in the Cargo.lock text."""
    found = {}
    for package in tomllib.loads(lock_text).get("package", []):
        name = package["name"]
        if name in CRATES and (name not in found or version_key(package["version"]) > version_key(found[name])):
            found[name] = package["version"]
    return found


def lag(have, latest):
    """How far `have` is behind `latest`: (majors, minors), the minors counted within the same major only."""
    h, n = version_key(have), version_key(latest)
    if n[0] != h[0]:
        return n[0] - h[0], 0
    return 0, n[1] - h[1]


def describe(majors, minors):
    """The lag in words."""
    if majors:
        return f"{majors} major{'s' if majors != 1 else ''} behind"
    if minors:
        return f"{minors} minor{'s' if minors != 1 else ''} behind"
    return "current"


def report(lock_text, fetch):
    """The Markdown table and the failures, from the Cargo.lock text and `fetch(name)`, the index lines of a crate."""
    have = locked(lock_text)
    rows = ["| Crate | Locked | Newest | Lag |", "|---|---|---|---|"]
    failures = []
    for name in CRATES:
        latest = newest(fetch(name))
        if name not in have or latest is None:
            rows.append(f"| {name} | {have.get(name, 'not locked')} | {latest or 'no release'} | - |")
            continue
        majors, minors = lag(have[name], latest)
        rows.append(f"| {name} | {have[name]} | {latest} | {describe(majors, minors)} |")
        for crate, part, limit in LIMITS:
            if crate != name:
                continue
            # A new major of a minor-limited crate is beyond any number of minors.
            over = majors > limit if part == "major" else (majors > 0 or minors > limit)
            if over:
                word = "major" if part == "major" else "minor"
                failures.append(
                    f"{name} {have[name]} is {describe(majors, minors)} {latest}: more than {limit} {word}"
                    f"{'s' if limit != 1 else ''} behind (NFR-MAINT-04); run the upgrade train"
                )
    return rows, failures


def main(argv):
    parser = argparse.ArgumentParser(prog="ui-stack-lag.py", description=__doc__.split("\n")[0])
    parser.add_argument("--test", action="store_true", help="run the offline unit test")
    parser.add_argument("--lock", default=os.path.join(REPO, "Cargo.lock"), help="the Cargo.lock to read")
    args = parser.parse_args(argv)
    if args.test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(LagTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    with open(args.lock, encoding="utf-8") as f:
        rows, failures = report(f.read(), fetch_index)
    print("\n".join(rows))
    if failures:
        print()
        print("\n".join(f"**Failed:** {failure}" for failure in failures))
        return 1
    return 0


class LagTest(unittest.TestCase):
    """IN-19 over a fixture lock and index."""

    def lock(self, **versions):
        packages = {"egui": "0.37.0", "eframe": "0.37.0", "egui_tiles": "0.18.0", "wgpu": "27.0.1", "winit": "0.30.12"}
        packages.update(versions)
        text = 'version = 4\n\n[[package]]\nname = "wgpu"\nversion = "25.0.2"\n\n'
        text += "".join(f'[[package]]\nname = "{n}"\nversion = "{v}"\n\n' for n, v in packages.items())
        return text

    def index(self, **newest_of):
        """An index whose crates have the releases of `newest_of` (default: the fixture lock's) and older ones."""
        releases = {
            "egui": ["0.36.0", "0.37.0", "0.38.0", "0.39.0", "0.40.0-alpha.1"],
            "eframe": ["0.37.0", "0.38.0"],
            "egui-wgpu": ["0.37.0"],
            "egui_tiles": ["0.18.0"],
            "egui_table": [],
            "egui_ltreeview": ["0.9.0"],
            "egui_kittest": ["0.37.0"],
            "wgpu": ["26.0.0", "27.0.1", "28.0.0"],
            "winit": ["0.30.12", "0.31.0"],
        }
        releases.update(newest_of)

        def fetch(name):
            lines = [json.dumps({"name": name, "vers": v, "yanked": False}) for v in releases[name]]
            if name == "winit":
                lines.append(json.dumps({"name": name, "vers": "0.32.0", "yanked": True}))
            return lines

        return fetch

    def test_the_fixture_gives_the_expected_table(self):
        rows, failures = report(self.lock(), self.index())
        self.assertEqual(
            rows,
            [
                "| Crate | Locked | Newest | Lag |",
                "|---|---|---|---|",
                "| egui | 0.37.0 | 0.39.0 | 2 minors behind |",
                "| eframe | 0.37.0 | 0.38.0 | 1 minor behind |",
                "| egui-wgpu | not locked | 0.37.0 | - |",
                "| egui_tiles | 0.18.0 | 0.18.0 | current |",
                "| egui_table | not locked | no release | - |",
                "| egui_ltreeview | not locked | 0.9.0 | - |",
                "| egui_kittest | not locked | 0.37.0 | - |",
                "| wgpu | 27.0.1 | 28.0.0 | 1 major behind |",
                "| winit | 0.30.12 | 0.31.0 | 1 minor behind |",
            ],
        )
        self.assertEqual(failures, [])

    def test_egui_three_minors_behind_fails(self):
        rows, failures = report(self.lock(egui="0.36.0"), self.index())
        self.assertIn("| egui | 0.36.0 | 0.39.0 | 3 minors behind |", rows)
        self.assertEqual(
            failures,
            ["egui 0.36.0 is 3 minors behind 0.39.0: more than 2 minors behind (NFR-MAINT-04); run the upgrade train"],
        )

    def test_wgpu_two_majors_behind_fails(self):
        _, failures = report(self.lock(wgpu="26.0.0"), self.index())
        self.assertEqual(
            failures,
            ["wgpu 26.0.0 is 2 majors behind 28.0.0: more than 1 major behind (NFR-MAINT-04); run the upgrade train"],
        )

    def test_a_new_egui_major_fails(self):
        _, failures = report(self.lock(), self.index(egui=["1.0.0"]))
        self.assertEqual(
            failures,
            ["egui 0.37.0 is 1 major behind 1.0.0: more than 2 minors behind (NFR-MAINT-04); run the upgrade train"],
        )

    def test_a_yanked_newest_release_is_skipped(self):
        fetch = self.index()
        self.assertEqual(newest(fetch("winit")), "0.31.0")
        self.assertEqual(newest(fetch("egui")), "0.39.0")
        self.assertIsNone(newest([]))

    def test_the_index_path_is_the_registry_layout(self):
        self.assertEqual(
            [index_path(n) for n in ("a", "ab", "abc", "egui", "Egui_Tiles")],
            ["1/a", "2/ab", "3/a/abc", "eg/ui/egui", "eg/ui/egui_tiles"],
        )


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
