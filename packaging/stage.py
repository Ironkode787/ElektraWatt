#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The staging script of the installed tree and the CLI archives (WP-S6.06 D3, D4, D13, D18; standard library only).

  python3 -I packaging/stage.py tree --bin <dir> --book <dir> --version <v> --out <root>
  python3 -I packaging/stage.py archive --tree <root> --kind zip|tar.gz --out <dir>/ew-<v>-<target>.<kind>
  python3 -I packaging/stage.py pin <name> [--field version|url|sha256]   # a component of packaging/components.toml
  python3 -I packaging/stage.py snapshot              # the Ubuntu archive snapshot of the Linux container (FT-1222)
  python3 -I packaging/stage.py debs <dir>            # the downloaded .deb files against components.toml (FT-1223)
  python3 -I packaging/stage.py listing <root>        # `<sha256>  <path>` of every file under <root>
  python3 -I packaging/stage.py resources --tree <root> --work <dir>   # D13's installed-resources runs
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

`debs` compares the `.deb` files that the Linux container's apt downloaded into <dir> with packaging/components.toml
before they are installed: every file is a listed package (`<name>_<version>_amd64.deb`, `:` written `%3a` as apt
names it) with its SHA-256, and every listed package was downloaded; anything else is refused, naming each file.

`resources` proves D13 from the installed executable: `elektrawatt --headless --script
packaging/tests/installed.uiscript --config-dir <fresh> --keep-going`, run in a fresh directory of <work> on the tree
as it is, then once more on a copy of the tree without each resource of RESOURCES; every step of the first run is
done, the sample opens read-only with its 36 pages, and each resource's step reads differently in the run without
that resource. The tree itself is only read.
"""
import argparse
import gzip
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
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
COMPONENTS = os.path.join("packaging", "components.toml")
FIELDS = ("version", "url", "sha256")
# A snapshot of snapshot.ubuntu.com: a UTC timestamp to the second.
SNAPSHOT = re.compile(r"[0-9]{8}T[0-9]{6}Z")
UISCRIPT = os.path.join("packaging", "tests", "installed.uiscript")
# D13: each resource of the tree that a step of the installed-resources script proves, by the step's action, and the
# word for the run without it. The guide has no step: no action of the headless mode shows whether it was found (F1's
# request goes to the window only).
RESOURCES = (
    ("samples", "file.open_sample", "sample"),
    ("content", "file.import_qet", "content"),
    (NOTICES, "ui.describe", "notices"),
)
# The variables that would name a resource or a settings directory other than the tree's and the run's own.
RESOURCE_VARIABLES = ("EW_SAMPLE_DIR", "EW_CONTENT_DIR", "EW_DOCS_DIR", "EW_CONFIG_DIR", "EW_DATA_DIR", "EW_CACHE_DIR")
# The QElectroTech project that the content step imports: one page, no element; the import's banner says when the
# starter content is missing.
QET = (
    '<project version="0.100" title="Installed">\n'
    '<diagram title="First page" folio="1" cols="17" colsize="60" rows="8" rowsize="80" height="640"/>\n'
    "</project>\n"
)


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


def components(repo):
    """packaging/components.toml, read."""
    with open(need(os.path.join(repo, COMPONENTS)), "rb") as f:
        return tomllib.load(f)


def pin(repo, name, field="version"):
    """The `field` (version, url or sha256) of the component `name` of packaging/components.toml."""
    for component in components(repo)["component"]:
        if component["name"] == name:
            return component[field]
    raise Refusal(f"no component {name} in packaging/components.toml")


def snapshot(repo):
    """The timestamp of the Ubuntu archive snapshot that the Linux container's apt reads (FT-1222)."""
    value = components(repo).get("apt", {}).get("snapshot", "")
    if not SNAPSHOT.fullmatch(value):
        raise Refusal(f"the apt snapshot of packaging/components.toml is not a time such as 20261102T000000Z: {value}")
    return value


def deb_file(name, version):
    """The file name that apt gives the amd64 package `name` at `version` in its archive cache."""
    return f"{name}_{version.replace(':', '%3a')}_amd64.deb"


def sha256(path):
    """The SHA-256 of the file at `path`, in hex."""
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def check_debs(repo, directory):
    """`<sha256>  <file>` of each package in `directory`, refused unless the `.deb` files there are exactly the
    packages of packaging/components.toml (the components whose URL is a `.deb`, and `[[apt.deb]]`) with their
    SHA-256 (FT-1223)."""
    table = components(repo)
    listed = [c for c in table["component"] if c["url"].endswith(".deb")] + table.get("apt", {}).get("deb", [])
    want = {deb_file(p["name"], p["version"]): p["sha256"] for p in listed}
    got = {n: sha256(os.path.join(directory, n)) for n in os.listdir(need(directory)) if n.endswith(".deb")}
    problems = []
    for name in sorted(set(want) | set(got)):
        if name not in got:
            problems.append(f"{name} is listed in packaging/components.toml but was not downloaded")
        elif name not in want:
            problems.append(f"{name} was downloaded but is not listed in packaging/components.toml")
        elif got[name] != want[name]:
            problems.append(f"{name} has the SHA-256 {got[name]}, not the {want[name]} of packaging/components.toml")
    if problems:
        raise Refusal("the downloaded packages are not those of packaging/components.toml:\n" + "\n".join(problems))
    return [f"{got[name]}  {name}" for name in sorted(got)]


def listing(root):
    """`<sha256>  <path>` of every file under `root`, the paths `/`-separated, in byte order of the paths."""
    files = sorted(walk(need(root), "."), key=lambda e: e[0].encode("utf-8"))
    return [f"{sha256(path)}  {arc[2:]}" for arc, path in files]


def copy_without(tree, out, resource):
    """A copy of `tree` at `out` without its top-level entry `resource`; the files keep their modes, the directories
    stay writable."""
    for base, dirnames, filenames in os.walk(tree):
        rel = os.path.relpath(base, tree)
        if rel == ".":
            dirnames[:] = [d for d in dirnames if d != resource]
            filenames = [f for f in filenames if f != resource]
        os.makedirs(os.path.join(out, rel), exist_ok=True)
        for name in filenames:
            shutil.copy(os.path.join(base, name), os.path.join(out, rel, name))


def run_script(exe, script, run_dir):
    """Runs the installed-resources script with `exe` in the fresh directory `run_dir` (its settings in `config`, its
    working directory `cwd` with the project to import); the exit code and the transcript's step lines by action."""
    cwd, config = os.path.join(run_dir, "cwd"), os.path.join(run_dir, "config")
    os.makedirs(cwd)
    os.makedirs(config)
    with open(os.path.join(cwd, "example.qet"), "w", encoding="utf-8", newline="\n") as f:
        f.write(QET)
    env = {k: v for k, v in os.environ.items() if k not in RESOURCE_VARIABLES}
    # The settings directory through EW_CONFIG_DIR: `--config-dir` would drop the resources that the program found
    # beside itself (the headless mode starts a configuration of that directory alone).
    env["EW_CONFIG_DIR"] = os.path.abspath(config)
    command = [exe, "--headless", "--script", os.path.abspath(script), "--keep-going"]
    done = subprocess.run(command, cwd=cwd, env=env, capture_output=True, check=False)
    for name, data in (("transcript.jsonl", done.stdout), ("stderr.txt", done.stderr)):
        with open(os.path.join(run_dir, name), "wb") as f:
            f.write(data)
    lines = [json.loads(line) for line in done.stdout.decode("utf-8").splitlines() if line.strip()]
    return done.returncode, {line["action"]: line for line in lines[1:]}


def resource_problems(runs):
    """The problems of D13's runs: `runs` maps `installed` and the word of each resource of RESOURCES to the exit code
    and the step lines (by action) of the run on the tree and on the copy without that resource."""
    problems = []
    code, steps = runs["installed"]
    if code != 0:
        problems.append(f"the run on the tree exited with {code}, not 0")
    for action, line in steps.items():
        if line["outcome"] != "done":
            problems.append(f"{action} on the tree: {line['outcome']}, {line['message'] or line['reason_text']}")
    state = steps.get("file.open_sample", {}).get("state") or {}
    if (state.get("access"), state.get("page_count")) != ("read-only", 36):
        problems.append(
            f"the sample opened {state.get('access')} with {state.get('page_count')} pages, not read-only with 36"
        )
    for _, action, word in RESOURCES:
        code, without = runs[word]
        if code not in (0, 1):
            problems.append(f"the run without the {word} exited with {code}, not 0 or 1")
        if action not in steps or action not in without:
            problems.append(f"the step {action} of the {word} is missing from a transcript")
        elif without[action] == steps[action]:
            problems.append(f"{action} reads the same with and without the {word}: the {word} is not proven")
    return problems


def check_resources(repo, tree, work):
    """D13 over `tree` in the fresh directory `work`: the report lines, refused with the problems."""
    names = [n for n in ("elektrawatt.exe", "elektrawatt") if os.path.isfile(os.path.join(tree, n))]
    if not names:
        raise Refusal(f"missing input: {os.path.join(tree, 'elektrawatt')}")
    script = need(os.path.join(repo, UISCRIPT))
    if os.path.exists(work):
        raise Refusal(f"output exists: {work}")
    runs = {"installed": run_script(os.path.join(tree, names[0]), script, os.path.join(work, "installed"))}
    for resource, _, word in RESOURCES:
        run_dir = os.path.join(work, f"without-{word}")
        copy_without(tree, os.path.join(run_dir, "tree"), resource)
        runs[word] = run_script(os.path.join(run_dir, "tree", names[0]), script, run_dir)
    problems = resource_problems(runs)
    if problems:
        raise Refusal(f"the installed resources are not proven (transcripts in {work}):\n" + "\n".join(problems))
    report = []
    for _, action, word in RESOURCES:
        with_it, without = runs["installed"][1][action]["outcome"], runs[word][1][action]["outcome"]
        report.append(f"{word}: {action} {with_it} on the tree, {without} without the {word}; its line differs")
    return report


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
    p = sub.add_parser("pin", help="print the version (or another field) of a component")
    p.add_argument("name")
    p.add_argument("--field", default="version", choices=FIELDS)
    sub.add_parser("snapshot", help="print the Ubuntu archive snapshot of the Linux container")
    p = sub.add_parser("debs", help="check the downloaded .deb files against components.toml")
    p.add_argument("dir")
    p = sub.add_parser("listing", help="print the SHA-256 and path of every file under a directory")
    p.add_argument("root")
    p = sub.add_parser("resources", help="run D13's installed-resources script on a tree and on copies without each")
    p.add_argument("--tree", required=True)
    p.add_argument("--work", required=True)
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
            print(pin(args.repo, args.name, args.field))
        elif args.command == "snapshot":
            print(snapshot(args.repo))
        elif args.command == "debs":
            print("\n".join(check_debs(args.repo, args.dir)))
        elif args.command == "listing":
            print("\n".join(listing(args.root)))
        elif args.command == "resources":
            print("\n".join(check_resources(args.repo, args.tree, args.work)))
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
    # The fixture's packages: what a downloaded file of each holds, and so its SHA-256.
    GCC, CRYPT = b"gcc package\n", b"libcrypt-dev package\n"
    COMPONENTS = (
        '[[component]]\nname = "gcc"\nversion = "4:11"\n'
        'url = "https://snapshot.ubuntu.com/ubuntu/20261102T000000Z/pool/main/g/gcc-defaults/gcc_11_amd64.deb"\n'
        f'sha256 = "{hashlib.sha256(GCC).hexdigest()}"\n'
        '[[component]]\nname = "mdbook"\nversion = "0.5.4"\nurl = "https://example.org/mdbook.tar.gz"\n'
        f'sha256 = "{"0" * 64}"\n'
        '[apt]\nsnapshot = "20261102T000000Z"\n'
        '[[apt.deb]]\nname = "libcrypt-dev"\nversion = "1:4.4.27-1"\n'
        f'sha256 = "{hashlib.sha256(CRYPT).hexdigest()}"\n'
    )

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
            "packaging/components.toml": self.COMPONENTS,
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
        self.assertEqual(pin(self.repo, "mdbook", "url"), "https://example.org/mdbook.tar.gz")
        self.assertEqual(pin(self.repo, "mdbook", "sha256"), "0" * 64)
        self.assertEqual(snapshot(self.repo), "20261102T000000Z")
        with self.assertRaises(Refusal) as refused:
            pin(self.repo, "nsis")
        self.assertEqual(str(refused.exception), "no component nsis in packaging/components.toml")
        self.assertEqual(workspace_version(self.repo), "1.2.3")
        self.assertEqual(workspace_version(self.repo, "0123456789abcdef"), "1.2.3-dev.0123456")

    def test_the_repositorys_components_carry_every_field(self):
        table = components(REPO)
        fields = {"name", "version", "licence", "url", "sha256", "checked_by", "used_for"}
        hexdigest = re.compile(r"[0-9a-f]{64}")
        for component in table["component"]:
            self.assertEqual(set(component), fields, component.get("name"))
            sha = component["sha256"]
            self.assertTrue(sha == "" or hexdigest.fullmatch(sha), sha)
        for name in ("gcc", "libc6-dev", "cargo-about", "python", "setup-python", "mdbook", "nsis"):
            self.assertTrue(pin(REPO, name))
        # FT-1222, FT-1223: the packages come from the one snapshot, each with its version and SHA-256.
        pool = f"https://snapshot.ubuntu.com/ubuntu/{snapshot(REPO)}/pool/"
        for component in table["component"]:
            if component["url"].endswith(".deb"):
                self.assertTrue(component["url"].startswith(pool), component["url"])
                self.assertTrue(hexdigest.fullmatch(component["sha256"]), component["name"])
        for deb in table["apt"]["deb"]:
            self.assertEqual(set(deb), {"name", "version", "sha256"}, deb.get("name"))
            self.assertTrue(hexdigest.fullmatch(deb["sha256"]), deb["name"])

    def test_the_workflow_pins_follow_the_components(self):
        # FT-1225: the Windows job's setup-python is the version of components.toml, and so is its Python.
        with open(os.path.join(REPO, ".github", "workflows", "package.yml"), encoding="utf-8") as f:
            workflow = f.read()
        uses = re.findall(r"uses: actions/setup-python@[0-9a-f]{40} # v(\S+)", workflow)
        self.assertEqual(uses, [pin(REPO, "setup-python")])
        self.assertEqual(re.findall(r'python-version: "([^"]+)"', workflow), [pin(REPO, "python")])

    def test_downloaded_packages_are_compared_with_the_list(self):
        debs = os.path.join(self.dir, "archives")
        os.makedirs(os.path.join(debs, "partial"))
        files = {"gcc_4%3a11_amd64.deb": self.GCC, "libcrypt-dev_1%3a4.4.27-1_amd64.deb": self.CRYPT, "lock": b""}
        for name, data in files.items():
            with open(os.path.join(debs, name), "wb") as f:
                f.write(data)
        gcc, crypt = hashlib.sha256(self.GCC).hexdigest(), hashlib.sha256(self.CRYPT).hexdigest()
        self.assertEqual(
            check_debs(self.repo, debs),
            [f"{gcc}  gcc_4%3a11_amd64.deb", f"{crypt}  libcrypt-dev_1%3a4.4.27-1_amd64.deb"],
        )
        os.remove(os.path.join(debs, "gcc_4%3a11_amd64.deb"))
        with open(os.path.join(debs, "extra_1_amd64.deb"), "wb") as f:
            f.write(b"extra\n")
        with open(os.path.join(debs, "libcrypt-dev_1%3a4.4.27-1_amd64.deb"), "wb") as f:
            f.write(b"another build\n")
        other = hashlib.sha256(b"another build\n").hexdigest()
        with self.assertRaises(Refusal) as refused:
            check_debs(self.repo, debs)
        self.assertEqual(
            str(refused.exception),
            "the downloaded packages are not those of packaging/components.toml:\n"
            "extra_1_amd64.deb was downloaded but is not listed in packaging/components.toml\n"
            "gcc_4%3a11_amd64.deb is listed in packaging/components.toml but was not downloaded\n"
            f"libcrypt-dev_1%3a4.4.27-1_amd64.deb has the SHA-256 {other}, not the {crypt} of"
            " packaging/components.toml",
        )

    def test_the_snapshot_is_a_timestamp(self):
        path = os.path.join(self.repo, COMPONENTS)
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write(self.COMPONENTS.replace("20261102T000000Z", "2026-11-02"))
        with self.assertRaises(Refusal) as refused:
            snapshot(self.repo)
        self.assertEqual(
            str(refused.exception),
            "the apt snapshot of packaging/components.toml is not a time such as 20261102T000000Z: 2026-11-02",
        )

    def test_a_listing_and_a_copy_without_one_resource(self):
        tree = self.stage("linux")
        lines = listing(tree)
        self.assertEqual([line.split("  ", 1)[1] for line in lines], [n[2:] for n in self.listing(tree)])
        notice = hashlib.sha256(b"notice\n").hexdigest()
        self.assertIn(f"{notice}  NOTICE", lines)
        os.chmod(os.path.join(tree, "ew"), 0o755)
        copy = os.path.join(self.dir, "copy")
        copy_without(tree, copy, "content")
        self.assertEqual(listing(copy), [line for line in lines if not line.split("  ", 1)[1].startswith("content/")])
        if os.name == "posix":
            self.assertEqual(os.stat(os.path.join(copy, "ew")).st_mode & 0o777, 0o755)

    def transcript(self, **changed):
        """The step lines (by action) of a run of the installed-resources script, with `changed` lines."""
        sample = {"access": "read-only", "page_count": 36}
        steps = {
            "help.notices": {"outcome": "done", "state": {}},
            "ui.describe": {"outcome": "done", "describe": "  notices: 88 lines"},
            "ui.dismiss": {"outcome": "done"},
            "file.import_qet": {"outcome": "done", "state": {"banner": "Imported example.qet."}},
            "file.open_sample": {"outcome": "done", "state": sample},
        }
        for action, line in changed.items():
            steps[action.replace("__", ".")] = line
        for line in steps.values():
            line.setdefault("message", None)
            line.setdefault("reason_text", None)
        return steps

    def test_each_resource_step_reads_differently_without_its_resource(self):
        runs = {
            "installed": (0, self.transcript()),
            "sample": (1, self.transcript(file__open_sample={"outcome": "failed", "message": "Not installed."})),
            "content": (0, self.transcript(file__import_qet={"outcome": "done", "state": {"banner": "No content."}})),
            "notices": (0, self.transcript(ui__describe={"outcome": "done", "describe": "  notices: 85 lines"})),
        }
        self.assertEqual(resource_problems(runs), [])
        runs["installed"] = (
            1,
            self.transcript(
                file__open_sample={"outcome": "refused", "reason_text": "No sample.", "state": {"access": "edit"}}
            ),
        )
        runs["content"] = (0, self.transcript())
        runs["notices"] = (101, self.transcript(ui__describe={"outcome": "done", "describe": "  notices: 85 lines"}))
        self.assertEqual(
            resource_problems(runs),
            [
                "the run on the tree exited with 1, not 0",
                "file.open_sample on the tree: refused, No sample.",
                "the sample opened edit with None pages, not read-only with 36",
                "file.import_qet reads the same with and without the content: the content is not proven",
                "the run without the notices exited with 101, not 0 or 1",
            ],
        )

    def test_the_repositorys_installed_resources_script_has_each_resource_step_once(self):
        with open(os.path.join(REPO, UISCRIPT), encoding="utf-8") as f:
            lines = [json.loads(line) for line in f if line.strip()]
        self.assertEqual(lines[0]["$schema"], "ew.uiscript/1")
        actions = [line["action"] for line in lines[1:]]
        for _, action, _ in RESOURCES:
            self.assertEqual(actions.count(action), 1, action)
        imported = next(line for line in lines[1:] if line["action"] == "file.import_qet")
        self.assertEqual(imported["params"], {"file": "example.qet", "dir": "imported.ew"})

    def test_the_setup_removes_d3s_entries_by_name(self):
        # The uninstall section of the setup deletes exactly the top-level entries of the Windows tree, and itself.
        with open(os.path.join(REPO, "packaging", "windows", "elektrawatt.nsi"), encoding="utf-8") as f:
            script = f.read()
        uninstall = script.split('Section "Uninstall"', 1)[1].split("SectionEnd", 1)[0]
        removed = re.findall(r'^\s*(?:Delete|RMDir /r) "\$INSTDIR\\([^"\\]+)"$', uninstall, re.MULTILINE)
        tree = self.stage("windows")
        self.assertEqual(sorted(removed), sorted(os.listdir(tree) + ["uninstall.exe"]))


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
