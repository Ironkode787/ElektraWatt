#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The staging script of the installed tree and the CLI archives (WP-S6.06 D3, D4, D18; standard library only).

  python3 -I packaging/stage.py tree --bin <dir> --book <dir> --version <v> --out <root>
  python3 -I packaging/stage.py archive --tree <root> --kind zip|tar.gz --out <dir>/ew-<v>-<target>.<kind>
  python3 -I packaging/stage.py pin <name>            # the version of a component of packaging/components.toml
  python3 -I packaging/stage.py version [--sha <sha>] # the workspace version, or D8's <version>-dev.<short sha>
  python3 -I packaging/stage.py --test                # the offline unit test, over a fixture repository

`tree` assembles D3's installed tree, flat, every resource beside the executables:

  elektrawatt[.exe], ew[.exe]           from --bin (on Windows also elektrawatt.pdb and ew.pdb, D5)
  THIRD-PARTY-NOTICES.txt               from --bin, where the job writes it beside the binaries (cargo-about)
  content/                              the repository's content/, as is
  samples/J-E-2416.ew/                  a copy of golden/J-E-2416.ew, which is only read
  docs/                                 the built book, from --book
  NOTICE, PRIVACY.md, SECURITY.md       from the repository
  LICENSES/                             the repository's licence texts (at least the six of D3)
  README.txt                            what the tree is, its version, "unsigned" and where to verify it

The Windows layout is chosen when --bin holds `ew.exe`. Every input is checked before anything is written, and a
missing one is refused with "missing input: <path>", never skipped; the output directory must not exist.

`archive` writes a CLI archive of D3 from a staged tree: under the one root folder `ew-<v>-<target>/` (the archive's
file name without its extension) exactly `ew[.exe]`, `content/`, `samples/`, `THIRD-PARTY-NOTICES.txt`, `NOTICE`,
`LICENSES/`, `PRIVACY.md`, `SECURITY.md` and `README.md` (ci/cli-artefact-README.md); never `elektrawatt`, a `.pdb`
or `docs/` (ID-476's hold). Entries are files only, in byte order of their names, with Unix modes 0755 for the
executable and 0644 for the rest, owner 0, and a fixed time (1980-01-01), so two runs give identical bytes. The file
is written beside its final name and renamed into place.
"""
import argparse
import gzip
import io
import os
import shutil
import sys
import tarfile
import tempfile
import unittest
import zipfile

if sys.version_info < (3, 11):
    sys.exit("stage.py needs Python 3.11 or newer (tomllib)")
import tomllib  # noqa: E402

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

LINUX_BINARIES = ("elektrawatt", "ew")
WINDOWS_BINARIES = ("elektrawatt.exe", "ew.exe", "elektrawatt.pdb", "ew.pdb")
EXECUTABLES = ("elektrawatt", "ew", "elektrawatt.exe", "ew.exe")
NOTICES = "THIRD-PARTY-NOTICES.txt"
# The licences that the tree's files carry (ci/licence-map.toml); LICENSES/ holds every text of the repository.
LICENCES = ("GPL-3.0-or-later", "CC-BY-SA-4.0", "CC0-1.0", "OFL-1.1", "MIT", "CC-BY-4.0")
SAMPLE = "J-E-2416.ew"
CLI_README = os.path.join("ci", "cli-artefact-README.md")
# The CLI archive's entries at the root of a tree, besides the executable (D3).
CLI_ENTRIES = ("content", "samples", NOTICES, "NOTICE", "LICENSES", "PRIVACY.md", "SECURITY.md")
KINDS = {"zip": ".zip", "tar.gz": ".tar.gz"}
# 1980-01-01 00:00:00 UTC, the earliest time a zip entry can hold; the tar entries take the same.
ZIP_TIME = (1980, 1, 1, 0, 0, 0)
TAR_TIME = 315532800


class Refusal(Exception):
    """An input or an output that the script refuses, with the message it prints."""


def need(path):
    """`path`, refused when it does not exist."""
    if not os.path.exists(path):
        raise Refusal(f"missing input: {path}")
    return path


def readme(version):
    """The tree's README.txt."""
    return (
        f"ElektraWatt {version}\n"
        "\n"
        "This folder is an installation of ElektraWatt: the program elektrawatt, the command-line program ew, the\n"
        "starter content (content), the sample project (samples/J-E-2416.ew, opened read-only; Save as makes a copy),\n"
        "the user guide (docs/index.html), the licence texts (LICENSES), NOTICE, the third-party notices\n"
        "(THIRD-PARTY-NOTICES.txt), PRIVACY.md and SECURITY.md.\n"
        "\n"
        "The programs are unsigned: no signature tells you who built them. Verify a download against the checksums\n"
        "published with its release on the project's repository, https://github.com/Ironkode787/ElektraWatt/releases.\n"
        "A version that ends in -dev.<commit> is a development build of that commit, not a release.\n"
        "\n"
        "ElektraWatt is free software under the GPL-3.0-or-later (LICENSES/GPL-3.0-or-later.txt).\n"
    )


def stage_tree(repo, bin_dir, book, version, out):
    """Assembles D3's tree at `out` (see the module's description)."""
    windows = os.path.exists(os.path.join(bin_dir, "ew.exe"))
    binaries = WINDOWS_BINARIES if windows else LINUX_BINARIES
    files = [(need(os.path.join(bin_dir, name)), name) for name in binaries + (NOTICES,)]
    files += [(need(os.path.join(repo, name)), name) for name in ("NOTICE", "PRIVACY.md", "SECURITY.md")]
    licences = need(os.path.join(repo, "LICENSES"))
    for licence in LICENCES:
        need(os.path.join(licences, f"{licence}.txt"))
    need(os.path.join(book, "index.html"))
    dirs = [
        (need(os.path.join(repo, "content")), "content"),
        (need(os.path.join(repo, "golden", SAMPLE)), os.path.join("samples", SAMPLE)),
        (book, "docs"),
        (licences, "LICENSES"),
    ]
    if os.path.exists(out):
        raise Refusal(f"output exists: {out}")
    os.makedirs(out)
    for source, name in files:
        shutil.copy2(source, os.path.join(out, name))
    for source, name in dirs:
        shutil.copytree(source, os.path.join(out, name))
    with open(os.path.join(out, "README.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write(readme(version))


def walk(path, arc):
    """(arcname, file) of every file under `path`, `arc` standing for `path`, `/`-separated."""
    if os.path.isfile(path):
        return [(arc, path)]
    found = []
    for base, dirnames, filenames in os.walk(path):
        dirnames.sort()
        rel = os.path.relpath(base, path).replace(os.sep, "/")
        for name in filenames:
            inner = name if rel == "." else f"{rel}/{name}"
            found.append((f"{arc}/{inner}", os.path.join(base, name)))
    return found


def archive_entries(repo, tree, root):
    """The CLI archive's entries under `root`, in byte order: (arcname, file, mode)."""
    exes = [name for name in ("ew", "ew.exe") if os.path.exists(os.path.join(tree, name))]
    if len(exes) != 1:
        raise Refusal(f"missing input: {os.path.join(tree, 'ew')}")
    exe = f"{root}/{exes[0]}"
    entries = [(exe, os.path.join(tree, exes[0]))]
    for name in CLI_ENTRIES:
        entries += walk(need(os.path.join(tree, name)), f"{root}/{name}")
    entries.append((f"{root}/README.md", need(os.path.join(repo, CLI_README))))
    entries.sort(key=lambda e: e[0].encode("utf-8"))
    return [(arc, path, 0o755 if arc == exe else 0o644) for arc, path in entries]


def write_archive(repo, tree, kind, out):
    """Writes the CLI archive of `tree` to `out` (see the module's description)."""
    suffix = KINDS[kind]
    name = os.path.basename(out)
    if not name.endswith(suffix) or len(name) == len(suffix):
        raise Refusal(f"the archive's name must end in {suffix}: {out}")
    entries = archive_entries(repo, tree, name[: -len(suffix)])
    data = io.BytesIO()
    if kind == "zip":
        with zipfile.ZipFile(data, "w") as z:
            for arc, path, mode in entries:
                info = zipfile.ZipInfo(arc, date_time=ZIP_TIME)
                info.create_system = 3
                info.external_attr = (0o100000 | mode) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                with open(path, "rb") as f:
                    z.writestr(info, f.read(), compresslevel=9)
    else:
        with gzip.GzipFile(filename="", mode="wb", fileobj=data, mtime=0, compresslevel=9) as gz:
            with tarfile.open(fileobj=gz, mode="w", format=tarfile.PAX_FORMAT) as t:
                for arc, path, mode in entries:
                    info = tarfile.TarInfo(arc)
                    info.size = os.path.getsize(path)
                    info.mode = mode
                    info.mtime = TAR_TIME
                    info.uid = info.gid = 0
                    info.uname = info.gname = ""
                    with open(path, "rb") as f:
                        t.addfile(info, f)
    partial = out + ".partial"
    try:
        with open(partial, "wb") as f:
            f.write(data.getvalue())
        os.replace(partial, out)
    finally:
        if os.path.exists(partial):
            os.remove(partial)
    return [arc for arc, _, _ in entries]


def pin(repo, name):
    """The version of the component `name` of packaging/components.toml."""
    with open(need(os.path.join(repo, "packaging", "components.toml")), "rb") as f:
        components = tomllib.load(f)["component"]
    for component in components:
        if component["name"] == name:
            return component["version"]
    raise Refusal(f"no component {name} in packaging/components.toml")


def workspace_version(repo, sha=None):
    """The version of the workspace's Cargo.toml, with D8's `-dev.<short sha>` when `sha` is given."""
    with open(need(os.path.join(repo, "Cargo.toml")), "rb") as f:
        version = tomllib.load(f)["workspace"]["package"]["version"]
    return f"{version}-dev.{sha[:7]}" if sha else version


def main(argv):
    parser = argparse.ArgumentParser(prog="stage.py", description=__doc__.split("\n")[0])
    parser.add_argument("--test", action="store_true", help="run the offline unit test")
    parser.add_argument("--repo", default=REPO, help=argparse.SUPPRESS)
    sub = parser.add_subparsers(dest="command")
    p = sub.add_parser("tree", help="stage the installed tree")
    for flag in ("--bin", "--book", "--version", "--out"):
        p.add_argument(flag, required=True)
    p = sub.add_parser("archive", help="write a CLI archive from a staged tree")
    p.add_argument("--tree", required=True)
    p.add_argument("--kind", required=True, choices=sorted(KINDS))
    p.add_argument("--out", required=True)
    p = sub.add_parser("pin", help="print the version of a component")
    p.add_argument("name")
    p = sub.add_parser("version", help="print the workspace version, or D8's default version")
    p.add_argument("--sha")
    args = parser.parse_args(argv)
    if args.test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(StageTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    try:
        if args.command == "tree":
            stage_tree(args.repo, args.bin, args.book, args.version, args.out)
        elif args.command == "archive":
            for arc in write_archive(args.repo, args.tree, args.kind, args.out):
                print(arc)
        elif args.command == "pin":
            print(pin(args.repo, args.name))
        elif args.command == "version":
            print(workspace_version(args.repo, args.sha))
        else:
            parser.error("name a command, or --test")
    except Refusal as refusal:
        print(refusal, file=sys.stderr)
        return 1
    return 0


class StageTest(unittest.TestCase):
    """IN-1 over a fixture repository in a temporary directory."""

    VERSION = "1.2.3-dev.abc1234"

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = self.tmp.name
        self.repo = os.path.join(self.dir, "repo")
        files = {
            "content/symbols/a.json": "{}\n",
            "content/libraries/p.ewlib/manifest.json": "{}\n",
            "golden/J-E-2416.ew/project.json": "{}\n",
            "golden/J-E-2416.ew/pages/1.json": "{}\n",
            "NOTICE": "notice\n",
            "PRIVACY.md": "privacy\n",
            "SECURITY.md": "security\n",
            "ci/cli-artefact-README.md": "readme\n",
            "Cargo.toml": '[workspace.package]\nversion = "1.2.3"\n',
            "packaging/components.toml": '[[component]]\nname = "gcc"\nversion = "4:11"\n',
            "LICENSES/Apache-2.0.txt": "text\n",
        }
        files.update({f"LICENSES/{licence}.txt": "text\n" for licence in LICENCES})
        files.update({f"book/{name}": "<html>\n" for name in ("index.html", "css/a.css")})
        for name in LINUX_BINARIES + (NOTICES, "ew.d"):
            files[f"linux/{name}"] = name
        for name in WINDOWS_BINARIES + (NOTICES,):
            files[f"windows/{name}"] = name
        for name, text in files.items():
            path = os.path.join(self.repo if not name.startswith(("book/", "linux/", "windows/")) else self.dir, name)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
        self.book = os.path.join(self.dir, "book")

    def tearDown(self):
        self.tmp.cleanup()

    def stage(self, os_name, out="tree"):
        out = os.path.join(self.dir, out)
        stage_tree(self.repo, os.path.join(self.dir, os_name), self.book, self.VERSION, out)
        return out

    def listing(self, root):
        return sorted(arc for arc, _ in walk(root, "."))

    COMMON = [
        "LICENSES/Apache-2.0.txt",
        "LICENSES/CC-BY-4.0.txt",
        "LICENSES/CC-BY-SA-4.0.txt",
        "LICENSES/CC0-1.0.txt",
        "LICENSES/GPL-3.0-or-later.txt",
        "LICENSES/MIT.txt",
        "LICENSES/OFL-1.1.txt",
        "NOTICE",
        "PRIVACY.md",
        "README.txt",
        "SECURITY.md",
        "THIRD-PARTY-NOTICES.txt",
        "content/libraries/p.ewlib/manifest.json",
        "content/symbols/a.json",
        "docs/css/a.css",
        "docs/index.html",
        "samples/J-E-2416.ew/pages/1.json",
        "samples/J-E-2416.ew/project.json",
    ]

    def test_the_tree_is_d3s_listing(self):
        linux = self.listing(self.stage("linux", "linux-tree"))
        self.assertEqual(linux, sorted(f"./{n}" for n in self.COMMON + ["elektrawatt", "ew"]))
        windows = self.listing(self.stage("windows", "windows-tree"))
        self.assertEqual(
            windows,
            sorted(f"./{n}" for n in self.COMMON + ["elektrawatt.exe", "elektrawatt.pdb", "ew.exe", "ew.pdb"]),
        )
        with open(os.path.join(self.dir, "linux-tree", "README.txt"), encoding="utf-8") as f:
            self.assertEqual(
                f.read(),
                "ElektraWatt 1.2.3-dev.abc1234\n"
                "\n"
                "This folder is an installation of ElektraWatt: the program elektrawatt, the command-line program ew,"
                " the\n"
                "starter content (content), the sample project (samples/J-E-2416.ew, opened read-only; Save as makes"
                " a copy),\n"
                "the user guide (docs/index.html), the licence texts (LICENSES), NOTICE, the third-party notices\n"
                "(THIRD-PARTY-NOTICES.txt), PRIVACY.md and SECURITY.md.\n"
                "\n"
                "The programs are unsigned: no signature tells you who built them. Verify a download against the"
                " checksums\n"
                "published with its release on the project's repository,"
                " https://github.com/Ironkode787/ElektraWatt/releases.\n"
                "A version that ends in -dev.<commit> is a development build of that commit, not a release.\n"
                "\n"
                "ElektraWatt is free software under the GPL-3.0-or-later (LICENSES/GPL-3.0-or-later.txt).\n",
            )

    def test_a_missing_input_is_refused_with_its_path(self):
        cases = [
            os.path.join(self.repo, "PRIVACY.md"),
            os.path.join(self.repo, "LICENSES", "OFL-1.1.txt"),
            os.path.join(self.repo, "golden", SAMPLE),
            os.path.join(self.dir, "linux", NOTICES),
            os.path.join(self.dir, "linux", "ew"),
            os.path.join(self.book, "index.html"),
        ]
        for missing in cases:
            moved = missing + ".away"
            os.rename(missing, moved)
            out = os.path.join(self.dir, "refused")
            with self.assertRaises(Refusal) as refused:
                self.stage("linux", "refused")
            self.assertEqual(str(refused.exception), f"missing input: {missing}")
            self.assertFalse(os.path.exists(out), "nothing is written before every input is there")
            os.rename(moved, missing)
        os.remove(os.path.join(self.dir, "windows", "ew.pdb"))
        with self.assertRaises(Refusal) as refused:
            self.stage("windows")
        self.assertEqual(str(refused.exception), f"missing input: {os.path.join(self.dir, 'windows', 'ew.pdb')}")
        self.stage("linux")
        with self.assertRaises(Refusal) as refused:
            self.stage("linux")
        self.assertEqual(str(refused.exception), f"output exists: {os.path.join(self.dir, 'tree')}")

    def read_archive(self, path):
        """(name, mode, bytes) of each entry, in the archive's order, and the tar members' owners and times."""
        if path.endswith(".zip"):
            with zipfile.ZipFile(path) as z:
                return [(i.filename, (i.external_attr >> 16) & 0o7777, z.read(i)) for i in z.infolist()], set()
        with tarfile.open(path, "r:gz") as t:
            members = t.getmembers()
            meta = {(m.uid, m.gid, m.uname, m.gname, m.mtime, m.type) for m in members}
            return [(m.name, m.mode, t.extractfile(m).read()) for m in members], meta

    def test_an_archive_holds_d3s_cli_list_in_byte_order(self):
        for os_name, kind, exe in (("linux", "tar.gz", "ew"), ("windows", "zip", "ew.exe")):
            tree = self.stage(os_name, f"{os_name}-tree")
            root = f"ew-{self.VERSION}-target"
            out = os.path.join(self.dir, f"{root}{KINDS[kind]}")
            write_archive(self.repo, tree, kind, out)
            entries, meta = self.read_archive(out)
            names = [name for name, _, _ in entries]
            expected = [
                f"{root}/{n}"
                for n in [exe] + [n for n in self.COMMON if not n.startswith(("docs/", "README.txt"))] + ["README.md"]
            ]
            self.assertEqual(names, sorted(expected, key=lambda n: n.encode("utf-8")))
            self.assertEqual(names, sorted(names, key=lambda n: n.encode("utf-8")))
            for name, mode, data in entries:
                self.assertEqual(mode, 0o755 if name == f"{root}/{exe}" else 0o644, name)
            self.assertEqual(dict((n, d) for n, _, d in entries)[f"{root}/README.md"], b"readme\n")
            if kind == "tar.gz":
                self.assertEqual(meta, {(0, 0, "", "", TAR_TIME, tarfile.REGTYPE)})
            self.assertFalse(os.path.exists(out + ".partial"))

    def test_two_runs_give_identical_archives(self):
        for kind in KINDS:
            made = []
            for run in ("a", "b"):
                tree = self.stage("linux", f"tree-{run}")
                os.makedirs(os.path.join(self.dir, run))
                out = os.path.join(self.dir, run, f"ew-1-x{KINDS[kind]}")
                write_archive(self.repo, tree, kind, out)
                with open(out, "rb") as f:
                    made.append(f.read())
                shutil.rmtree(tree)
                shutil.rmtree(os.path.join(self.dir, run))
            self.assertEqual(made[0], made[1], kind)

    def test_an_archive_name_needs_its_kinds_extension(self):
        tree = self.stage("linux")
        out = os.path.join(self.dir, "ew-1-x.tar.gz")
        with self.assertRaises(Refusal) as refused:
            write_archive(self.repo, tree, "zip", out)
        self.assertEqual(str(refused.exception), f"the archive's name must end in .zip: {out}")
        os.remove(os.path.join(tree, "PRIVACY.md"))
        with self.assertRaises(Refusal) as refused:
            write_archive(self.repo, tree, "tar.gz", out)
        self.assertEqual(str(refused.exception), f"missing input: {os.path.join(tree, 'PRIVACY.md')}")
        self.assertFalse(os.path.exists(out))

    def test_pins_and_versions(self):
        self.assertEqual(pin(self.repo, "gcc"), "4:11")
        with self.assertRaises(Refusal) as refused:
            pin(self.repo, "nsis")
        self.assertEqual(str(refused.exception), "no component nsis in packaging/components.toml")
        self.assertEqual(workspace_version(self.repo), "1.2.3")
        self.assertEqual(workspace_version(self.repo, "0123456789abcdef"), "1.2.3-dev.0123456")

    def test_the_repositorys_components_carry_every_field(self):
        with open(os.path.join(REPO, "packaging", "components.toml"), "rb") as f:
            components = tomllib.load(f)["component"]
        fields = {"name", "version", "licence", "url", "sha256", "checked_by", "used_for"}
        for component in components:
            self.assertEqual(set(component), fields, component.get("name"))
            sha = component["sha256"]
            self.assertTrue(sha == "" or (len(sha) == 64 and set(sha) <= set("0123456789abcdef")), sha)
        for name in ("gcc", "libc6-dev", "cargo-about", "python"):
            self.assertTrue(pin(REPO, name))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
