#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The checks and the publishing steps of the release workflow (WP-S6.07 D5, D6, D8, D10, D13; doc 18 CI-R-01,
CI-R-02, CI-R-06, CI-R-08, CI-R-11), run by .github/workflows/release.yml and, for `clean`, by package.yml.

  python3 -I ci/release-verify.py tag <tag>                  CI-R-01: prints `version=<X.Y.Z>` (for GITHUB_OUTPUT)
  python3 -I ci/release-verify.py runs <sha> --repo <o/r>    CI-R-02: green ci.yml and deep-nightly.yml runs
  python3 -I ci/release-verify.py clean                      no build step wrote into the checkout
  python3 -I ci/release-verify.py sums <dir> --version <v>   CI-R-06: writes <dir>/SHA256SUMS
  python3 -I ci/release-verify.py draft <tag> <dir> --repo <o/r> --sha <commit>
  python3 -I ci/release-verify.py promote <candidate> <dir> --repo <o/r> (--dry | --publish)
  python3 -I ci/release-verify.py --test                     # its offline unit test

`tag` takes a candidate tag `v<X.Y.Z>-rc.<n>` (D2) and refuses it unless `X.Y.Z` is `[workspace.package].version` of
Cargo.toml and docs/release-notes/<X.Y.Z>.md exists with text beyond its licence comment. `runs` asks the GitHub CLI
(`gh run list --commit <sha> --workflow <file> --json conclusion`) for the runs of ci.yml and deep-nightly.yml on the
commit and refuses unless each has one concluded `success` (perf.yml is report-only and not asked). `clean` refuses
when `git status --porcelain` lists anything. `sums` writes `SHA256SUMS` in sha256sum's format (`<hex>  <name>`, sorted
by name) over the release files of 0.1.0 in <dir>: the CLI archives of x86_64 Windows and Linux, the Windows setup and
the AppImage (ID-476's hold lifted, ID-1331 (2), WP-S6.08 D26), and every artefact's SBOM (`<artefact>.cdx.json`); it
refuses any other file and a missing one.

`draft` creates the draft prerelease <tag> with the release notes of its version and every file of <dir>
(`gh release create --draft --prerelease --verify-tag`, so no tag is created), downloads its files back and verifies
them. `promote` refuses when the tag v<X.Y.Z> exists, finds the draft <candidate>, downloads its files into <dir> and
verifies them; with `--dry` it prints what it would publish and changes nothing; with `--publish` it creates the
release v<X.Y.Z> (published, latest, not a prerelease) with the draft's notes and those files on the candidate's
commit, which creates the tag there, and verifies the published files again. Verifying a release's files: the files
are SHA256SUMS and exactly the files it lists, `sha256sum --check --strict SHA256SUMS` passes and `gh attestation
verify <file> --repo <o/r> --signer-workflow <o/r>/.github/workflows/release.yml --source-digest <commit>` passes
for each file (D10), the commit being the tag's at the draft (`--sha`, the workflow's `$GITHUB_SHA`) and the
candidate's at the promote, so the bytes are bound to the release workflow and to the commit they were built from
(C50-p2#5, FT-1302). Standard library only; the GitHub CLI and sha256sum are the runner's.
"""
import argparse
import contextlib
import hashlib
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CANDIDATE = re.compile(r"v(\d+\.\d+\.\d+)-rc\.\d+")
WORKFLOWS = ("ci.yml", "deep-nightly.yml")
SUMS = "SHA256SUMS"
SIGNER = ".github/workflows/release.yml"
# ID-1313, ID-1331 (2): the CLI archives and the installers, each with its SBOM; the aarch64 archives never at 0.1.0.
ARCHIVES = ("ew-{v}-x86_64-pc-windows-msvc.zip", "ew-{v}-x86_64-unknown-linux-gnu.tar.gz",
            "ElektraWatt-{v}-x64-user-setup.exe", "ElektraWatt-{v}-x86_64.AppImage")


class Refusal(Exception):
    """A failed check, with the message the script prints."""


def run(command, cwd=None):
    """Runs `command`: its standard output as text, or a Refusal that names it and carries its standard error."""
    done = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        raise Refusal(f"`{' '.join(command)}` failed (exit {done.returncode}):\n{done.stderr.strip()}")
    return done.stdout


def candidate_version(tag):
    """The X.Y.Z of the candidate tag v<X.Y.Z>-rc.<n>."""
    match = CANDIDATE.fullmatch(tag)
    if not match:
        raise Refusal(f"the tag {tag} is not a candidate tag v<X.Y.Z>-rc.<n>")
    return match.group(1)


def check_tag(repo, tag):
    """D5: the tag's version is the workspace version, and its release notes exist and are not empty."""
    version = candidate_version(tag)
    with open(os.path.join(repo, "Cargo.toml"), "rb") as f:
        workspace = tomllib.load(f)["workspace"]["package"]["version"]
    if version != workspace:
        raise Refusal(f"the tag {tag} does not match the workspace version {workspace}")
    notes = os.path.join(repo, "docs", "release-notes", f"{version}.md")
    text = ""
    if os.path.isfile(notes):
        with open(notes, encoding="utf-8") as f:
            text = f.read()
    # The licence comment at the top is no note.
    text = re.sub(r"\A\s*<!--.*?-->", "", text, flags=re.S)
    if not text.strip():
        raise Refusal(f"no release notes: docs/release-notes/{version}.md is missing or empty")
    return version


def check_runs(sha, repo, gh=run):
    """D6: a run concluded `success` of each of WORKFLOWS on the commit `sha`."""
    for workflow in WORKFLOWS:
        out = gh(["gh", "run", "list", "--repo", repo, "--commit", sha, "--workflow", workflow,
                  "--json", "conclusion", "--limit", "100"])
        if not any(run.get("conclusion") == "success" for run in json.loads(out or "[]")):
            raise Refusal(f"no successful {workflow} run on {sha[:7]}; run it on this commit before tagging")


def check_clean(git=run, repo=REPO):
    """D5: `git status --porcelain` lists nothing after the builds."""
    out = git(["git", "status", "--porcelain"], cwd=repo)
    if out.strip():
        raise Refusal(f"the build wrote into the checkout (git status --porcelain):\n{out.rstrip()}")


def release_files(directory, version):
    """The release files of `directory`, sorted by name: refuses any file that is no release file of ID-1313."""
    names = sorted(n for n in os.listdir(directory) if n != SUMS)
    archives = [a.format(v=version) for a in ARCHIVES]
    allowed = set(archives) | {f"{a}.cdx.json" for a in archives}
    for name in names:
        if name not in allowed:
            raise Refusal(f"{name} in {directory} is no release file of {version}")
    for archive in archives:
        for name in (archive, f"{archive}.cdx.json"):
            if name not in names:
                raise Refusal(f"{name} is missing from {directory}")
    return names


def write_sums(directory, version, sha=None):
    """D8: SHA256SUMS over the release files of `directory`, in sha256sum's format, sorted by name."""
    names = release_files(directory, version)
    if os.path.exists(os.path.join(directory, SUMS)):
        raise Refusal(f"{directory} holds a {SUMS} already")
    lines = []
    for name in names:
        with open(os.path.join(directory, name), "rb") as f:
            lines.append(f"{hashlib.sha256(f.read()).hexdigest()}  {name}\n")
    with open(os.path.join(directory, SUMS), "w", encoding="utf-8", newline="\n") as f:
        f.writelines(lines)
    return lines


def verify_files(directory, repo, run=run, commit=None):
    """D10, D13: `directory` holds SHA256SUMS and exactly the files it lists, `sha256sum -c` passes and each file's
    build provenance verifies, signed by release.yml (and, given `commit`, built from it). Returns the file names,
    SHA256SUMS last."""
    path = os.path.join(directory, SUMS)
    if not os.path.isfile(path):
        raise Refusal(f"no {SUMS} in {directory}")
    with open(path, encoding="utf-8") as f:
        listed = [line.split("  ", 1)[1].rstrip("\n") for line in f if line.strip()]
    present = sorted(n for n in os.listdir(directory) if n != SUMS)
    for name in present:
        if name not in listed:
            raise Refusal(f"{name} is not listed in {SUMS}")
    for name in listed:
        if name not in present:
            raise Refusal(f"{name} of {SUMS} is missing")
    print(run(["sha256sum", "--check", "--strict", SUMS], cwd=directory), end="")
    for name in present + [SUMS]:
        run(["gh", "attestation", "verify", os.path.join(directory, name), "--repo", repo,
             "--signer-workflow", f"{repo}/{SIGNER}", *(["--source-digest", commit] if commit else [])])
        print(f"release-verify: the build provenance of {name} verifies")
    return present + [SUMS]


def draft(repo_root, tag, directory, repo, commit, run=run):
    """D13: the draft prerelease `tag` with every file of `directory`, then its files downloaded back and verified as
    built from `commit` (FT-1302)."""
    version = candidate_version(tag)
    notes = os.path.join(repo_root, "docs", "release-notes", f"{version}.md")
    files = [os.path.join(directory, n) for n in sorted(os.listdir(directory))]
    run(["gh", "release", "create", tag, "--repo", repo, "--draft", "--prerelease", "--verify-tag",
         "--title", tag, "--notes-file", notes, *files])
    back = tempfile.mkdtemp(prefix="release-back-")
    run(["gh", "release", "download", tag, "--repo", repo, "--dir", back])
    names = verify_files(back, repo, run, commit)
    print(f"release-verify: the draft prerelease {tag} holds {len(names)} verified files; nothing is published")


def promote(candidate, directory, repo, publish, run=run):
    """D13: the candidate's draft, verified, published as v<X.Y.Z> (or, dry, printed)."""
    version = candidate_version(candidate)
    final = f"v{version}"
    refs = json.loads(run(["gh", "api", f"repos/{repo}/git/matching-refs/tags/{final}"]) or "[]")
    if any(ref.get("ref") == f"refs/tags/{final}" for ref in refs):
        raise Refusal(f"{final} exists already; promote a new candidate")
    release = json.loads(run(["gh", "release", "view", candidate, "--repo", repo, "--json", "isDraft,body"]))
    if not release.get("isDraft"):
        raise Refusal(f"the release {candidate} is not a draft; promote the draft of a candidate")
    commit = run(["gh", "api", f"repos/{repo}/commits/{candidate}", "--jq", ".sha"]).strip()
    run(["gh", "release", "download", candidate, "--repo", repo, "--dir", directory])
    names = verify_files(directory, repo, run, commit)
    if not publish:
        print(f"release-verify: dry run: would create the tag {final} on {commit} and publish the release {final} "
              f"(latest) with the notes of {candidate} and its {len(names)} files:")
        for name in names:
            print(f"  {name}")
        print("release-verify: dry run: nothing was changed")
        return
    notes = os.path.join(tempfile.mkdtemp(prefix="release-notes-"), "notes.md")
    with open(notes, "w", encoding="utf-8") as f:
        f.write(release.get("body") or "")
    run(["gh", "release", "create", final, "--repo", repo, "--target", commit, "--title", final,
         "--notes-file", notes, "--latest", *[os.path.join(directory, n) for n in names]])
    back = tempfile.mkdtemp(prefix="release-back-")
    run(["gh", "release", "download", final, "--repo", repo, "--dir", back])
    verify_files(back, repo, run, commit)
    print(f"release-verify: published {final} on {commit} with the {len(names)} files of {candidate}")


def main(argv):
    parser = argparse.ArgumentParser(prog="release-verify.py", description=__doc__.split("\n")[0])
    parser.add_argument("--test", action="store_true", help="run the offline unit test")
    sub = parser.add_subparsers(dest="command")
    p = sub.add_parser("tag", help="CI-R-01: the tag's version and its release notes")
    p.add_argument("tag")
    p = sub.add_parser("runs", help="CI-R-02: green ci.yml and deep-nightly.yml runs on the commit")
    p.add_argument("sha")
    p.add_argument("--repo", required=True)
    sub.add_parser("clean", help="no build step wrote into the checkout")
    p = sub.add_parser("sums", help="CI-R-06: write SHA256SUMS over the release files")
    p.add_argument("dir")
    p.add_argument("--version", required=True)
    p = sub.add_parser("draft", help="CI-R-11: the draft prerelease, verified")
    p.add_argument("tag")
    p.add_argument("dir")
    p.add_argument("--repo", required=True)
    p.add_argument("--sha", required=True)
    p = sub.add_parser("promote", help="CI-R-11: publish a verified candidate as v<X.Y.Z>")
    p.add_argument("candidate")
    p.add_argument("dir")
    p.add_argument("--repo", required=True)
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--dry", action="store_true")
    mode.add_argument("--publish", action="store_true")
    args = parser.parse_args(argv)
    if args.test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(ReleaseVerifyTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    try:
        if args.command == "tag":
            print(f"version={check_tag(REPO, args.tag)}")
        elif args.command == "runs":
            check_runs(args.sha, args.repo)
            print(f"release-verify: {' and '.join(WORKFLOWS)} have a successful run on {args.sha[:7]}")
        elif args.command == "clean":
            check_clean()
            print("release-verify: git status --porcelain is empty; no build step wrote into the checkout")
        elif args.command == "sums":
            lines = write_sums(args.dir, args.version)
            print("".join(lines), end="")
        elif args.command == "draft":
            draft(REPO, args.tag, args.dir, args.repo, args.sha)
        elif args.command == "promote":
            promote(args.candidate, args.dir, args.repo, args.publish)
        else:
            parser.error("give a command or --test")
    except Refusal as refusal:
        print(f"release-verify: {refusal}", file=sys.stderr)
        return 1
    return 0


class FakeRun:
    """Records the commands and answers them from `answers` (a command prefix to its output); `sha256sum` and
    `gh attestation verify` pass unless named in `failing`."""

    def __init__(self, answers=None, failing=()):
        self.answers = answers or {}
        self.failing = failing
        self.commands = []

    def __call__(self, command, cwd=None):
        self.commands.append(command)
        for prefix in self.failing:
            if command[: len(prefix)] == list(prefix):
                raise Refusal(f"`{' '.join(command)}` failed (exit 1):\n")
        for prefix, out in self.answers.items():
            if tuple(command[: len(prefix)]) == prefix:
                return out(command) if callable(out) else out
        return ""


class ReleaseVerifyTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="release-verify-test-")
        self.repo = os.path.join(self.dir, "repo")
        os.makedirs(os.path.join(self.repo, "docs", "release-notes"))
        with open(os.path.join(self.repo, "Cargo.toml"), "w", encoding="utf-8") as f:
            f.write('[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\n')

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def notes(self, text):
        with open(os.path.join(self.repo, "docs", "release-notes", "0.1.0.md"), "w", encoding="utf-8") as f:
            f.write(text)

    def refused(self, call, *args, **kwargs):
        with self.assertRaises(Refusal) as refused:
            call(*args, **kwargs)
        return str(refused.exception)

    def test_a_matching_tag_with_notes_passes(self):
        # REUSE-IgnoreStart: the fixture's notes header is test data, not this file's licence.
        self.notes("<!--\nSPDX-License-Identifier: CC-BY-4.0\n-->\n\n# ElektraWatt 0.1.0\n")
        # REUSE-IgnoreEnd
        self.assertEqual(check_tag(self.repo, "v0.1.0-rc.1"), "0.1.0")
        self.assertEqual(check_tag(self.repo, "v0.1.0-rc.12"), "0.1.0")

    def test_a_tag_of_another_version_is_refused(self):
        self.notes("# 0.1.0\n")
        self.assertEqual(self.refused(check_tag, self.repo, "v0.2.0-rc.1"),
                         "the tag v0.2.0-rc.1 does not match the workspace version 0.1.0")
        self.assertEqual(self.refused(check_tag, self.repo, "v0.1.0"),
                         "the tag v0.1.0 is not a candidate tag v<X.Y.Z>-rc.<n>")

    def test_missing_or_empty_notes_are_refused(self):
        want = "no release notes: docs/release-notes/0.1.0.md is missing or empty"
        self.assertEqual(self.refused(check_tag, self.repo, "v0.1.0-rc.1"), want)
        self.notes("")
        self.assertEqual(self.refused(check_tag, self.repo, "v0.1.0-rc.1"), want)
        self.notes("<!--\nSPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n-->\n\n  \n")
        self.assertEqual(self.refused(check_tag, self.repo, "v0.1.0-rc.1"), want)

    def test_a_commit_without_a_successful_run_is_refused(self):
        sha = "0123456789abcdef0123456789abcdef01234567"

        def runs(conclusions):
            return FakeRun({("gh", "run", "list"): lambda c: conclusions[c[c.index("--workflow") + 1]]})

        green = runs({"ci.yml": '[{"conclusion":"failure"},{"conclusion":"success"}]',
                      "deep-nightly.yml": '[{"conclusion":"success"}]'})
        check_runs(sha, "o/r", green)
        self.assertEqual(green.commands[0], ["gh", "run", "list", "--repo", "o/r", "--commit", sha, "--workflow",
                                             "ci.yml", "--json", "conclusion", "--limit", "100"])
        self.assertEqual(
            self.refused(check_runs, sha, "o/r", runs({"ci.yml": '[{"conclusion":"success"}]',
                                                        "deep-nightly.yml": '[{"conclusion":"cancelled"}]'})),
            "no successful deep-nightly.yml run on 0123456; run it on this commit before tagging")
        self.assertEqual(
            self.refused(check_runs, sha, "o/r", runs({"ci.yml": "[]", "deep-nightly.yml": "[]"})),
            "no successful ci.yml run on 0123456; run it on this commit before tagging")

    def test_a_build_that_wrote_into_the_checkout_is_refused(self):
        check_clean(FakeRun({("git", "status"): ""}))
        self.assertEqual(self.refused(check_clean, FakeRun({("git", "status"): "?? stray.txt\n"})),
                         "the build wrote into the checkout (git status --porcelain):\n?? stray.txt")

    def release_dir(self, names):
        directory = os.path.join(self.dir, "release")
        os.makedirs(directory)
        for name in names:
            with open(os.path.join(directory, name), "wb") as f:
                f.write(name.encode())
        return directory

    FILES = ["ew-0.1.0-x86_64-unknown-linux-gnu.tar.gz", "ew-0.1.0-x86_64-unknown-linux-gnu.tar.gz.cdx.json",
             "ew-0.1.0-x86_64-pc-windows-msvc.zip", "ew-0.1.0-x86_64-pc-windows-msvc.zip.cdx.json",
             "ElektraWatt-0.1.0-x64-user-setup.exe", "ElektraWatt-0.1.0-x64-user-setup.exe.cdx.json",
             "ElektraWatt-0.1.0-x86_64.AppImage", "ElektraWatt-0.1.0-x86_64.AppImage.cdx.json"]

    def test_sums_list_every_release_file_sorted_in_sha256sums_format(self):
        directory = self.release_dir(self.FILES)
        write_sums(directory, "0.1.0")
        with open(os.path.join(directory, SUMS), encoding="utf-8") as f:
            lines = f.read().splitlines()
        self.assertEqual([line.split("  ")[1] for line in lines], sorted(self.FILES))
        self.assertEqual(lines[0], f"{hashlib.sha256(sorted(self.FILES)[0].encode()).hexdigest()}  {sorted(self.FILES)[0]}")
        self.assertEqual(self.refused(write_sums, directory, "0.1.0"), f"{directory} holds a SHA256SUMS already")

    def test_sums_refuse_a_stray_file_and_a_missing_sbom(self):
        directory = self.release_dir(self.FILES + ["ew-0.1.0-aarch64-unknown-linux-gnu.tar.gz"])
        self.assertEqual(self.refused(write_sums, directory, "0.1.0"),
                         f"ew-0.1.0-aarch64-unknown-linux-gnu.tar.gz in {directory} is no release file of 0.1.0")
        os.remove(os.path.join(directory, "ew-0.1.0-aarch64-unknown-linux-gnu.tar.gz"))
        os.remove(os.path.join(directory, "ew-0.1.0-x86_64-pc-windows-msvc.zip.cdx.json"))
        self.assertEqual(self.refused(write_sums, directory, "0.1.0"),
                         f"ew-0.1.0-x86_64-pc-windows-msvc.zip.cdx.json is missing from {directory}")

    def test_sums_require_the_sbom_of_every_artefact(self):
        # C50-p2#4, ID-1313: the setup's and the AppImage's SBOMs are release files as the archives' are; WP-S6.08 D26
        # (ID-1331 (2)): so are the setup and the AppImage themselves.
        for name in ("ElektraWatt-0.1.0-x64-user-setup.exe.cdx.json", "ElektraWatt-0.1.0-x86_64.AppImage.cdx.json",
                     "ElektraWatt-0.1.0-x64-user-setup.exe", "ElektraWatt-0.1.0-x86_64.AppImage"):
            directory = self.release_dir([f for f in self.FILES if f != name])
            self.assertEqual(self.refused(write_sums, directory, "0.1.0"), f"{name} is missing from {directory}")
            shutil.rmtree(directory)

    def test_the_verify_job_requires_nothing_of_main(self):
        # WP-S6.08 D25 (ID-1412): a candidate is tagged on its release branch `release/<X.Y>`, so the `verify` job
        # names no branch: neither `main` nor a branch ref or filter.
        with open(os.path.join(REPO, ".github", "workflows", "release.yml"), encoding="utf-8") as f:
            text = f.read()
        start = text.index("\n  verify:\n")
        job = text[start:text.index("\n  package:\n", start)]
        self.assertIsNone(re.search(r"\bmain\b|refs/heads|branches", job), job)

    def test_verification_needs_exactly_the_listed_files_and_their_provenance(self):
        directory = self.release_dir(self.FILES)
        write_sums(directory, "0.1.0")
        fake = FakeRun()
        self.assertEqual(verify_files(directory, "o/r", fake), sorted(self.FILES) + [SUMS])
        self.assertEqual(fake.commands[0], ["sha256sum", "--check", "--strict", SUMS])
        self.assertEqual(fake.commands[-1], ["gh", "attestation", "verify", os.path.join(directory, SUMS),
                                             "--repo", "o/r", "--signer-workflow", "o/r/.github/workflows/release.yml"])
        self.assertEqual(len(fake.commands), 1 + len(self.FILES) + 1)
        with open(os.path.join(directory, "extra.bin"), "wb") as f:
            f.write(b"x")
        self.assertEqual(self.refused(verify_files, directory, "o/r", FakeRun()), "extra.bin is not listed in SHA256SUMS")
        os.remove(os.path.join(directory, "extra.bin"))
        failing = FakeRun(failing=[("gh", "attestation")])
        self.assertTrue(self.refused(verify_files, directory, "o/r", failing).startswith("`gh attestation verify "))

    def promote_fake(self, refs="[]", draft=True, with_files=True):
        def download(command):
            target = command[command.index("--dir") + 1]
            os.makedirs(target, exist_ok=True)
            if with_files:
                for name in self.FILES:
                    with open(os.path.join(target, name), "wb") as f:
                        f.write(name.encode())
                write_sums(target, "0.1.0")
            return ""

        return FakeRun({
            ("gh", "api", "repos/o/r/git/matching-refs/tags/v0.1.0"): refs,
            ("gh", "release", "view"): '{"isDraft": %s, "body": "notes"}' % ("true" if draft else "false"),
            ("gh", "api", "repos/o/r/commits/v0.1.0-rc.2"): "c0ffee\n",
            ("gh", "release", "download"): download,
        })

    def test_a_dry_promote_changes_nothing(self):
        fake = self.promote_fake()
        directory = os.path.join(self.dir, "files")
        printed = io.StringIO()
        with contextlib.redirect_stdout(printed):
            promote("v0.1.0-rc.2", directory, "o/r", publish=False, run=fake)
        names = sorted(self.FILES) + [SUMS]
        verify = [["gh", "attestation", "verify", os.path.join(directory, n), "--repo", "o/r", "--signer-workflow",
                   "o/r/.github/workflows/release.yml", "--source-digest", "c0ffee"] for n in names]
        self.assertEqual(fake.commands, [
            ["gh", "api", "repos/o/r/git/matching-refs/tags/v0.1.0"],
            ["gh", "release", "view", "v0.1.0-rc.2", "--repo", "o/r", "--json", "isDraft,body"],
            ["gh", "api", "repos/o/r/commits/v0.1.0-rc.2", "--jq", ".sha"],
            ["gh", "release", "download", "v0.1.0-rc.2", "--repo", "o/r", "--dir", directory],
            ["sha256sum", "--check", "--strict", SUMS],
            *verify,
        ])
        self.assertEqual(printed.getvalue(), "".join(
            [f"release-verify: the build provenance of {n} verifies\n" for n in names]
            + ["release-verify: dry run: would create the tag v0.1.0 on c0ffee and publish the release v0.1.0 "
               "(latest) with the notes of v0.1.0-rc.2 and its 9 files:\n"]
            + [f"  {n}\n" for n in names]
            + ["release-verify: dry run: nothing was changed\n"]))

    def test_a_promote_publishes_the_candidates_files_on_its_commit(self):
        fake = self.promote_fake()
        directory = os.path.join(self.dir, "files")
        promote("v0.1.0-rc.2", directory, "o/r", publish=True, run=fake)
        create = [c for c in fake.commands if c[:3] == ["gh", "release", "create"]]
        self.assertEqual(len(create), 1)
        self.assertEqual(create[0][3:10], ["v0.1.0", "--repo", "o/r", "--target", "c0ffee", "--title", "v0.1.0"])
        self.assertIn("--latest", create[0])
        self.assertNotIn("--prerelease", create[0])
        self.assertNotIn("--draft", create[0])
        files = [a for a in create[0] if a.startswith(directory)]
        self.assertEqual(sorted(os.path.basename(f) for f in files), sorted(self.FILES + [SUMS]))

    def test_a_promote_is_refused_when_the_final_tag_exists_or_the_candidate_is_no_draft(self):
        fake = self.promote_fake(refs='[{"ref": "refs/tags/v0.1.0"}]')
        self.assertEqual(self.refused(promote, "v0.1.0-rc.2", os.path.join(self.dir, "a"), "o/r", True, fake),
                         "v0.1.0 exists already; promote a new candidate")
        # A matching prefix (v0.1.0-rc.1) is no final tag.
        promote("v0.1.0-rc.2", os.path.join(self.dir, "b"), "o/r", False,
                self.promote_fake(refs='[{"ref": "refs/tags/v0.1.0-rc.1"}]'))
        self.assertEqual(
            self.refused(promote, "v0.1.0-rc.2", os.path.join(self.dir, "c"), "o/r", True,
                         self.promote_fake(draft=False)),
            "the release v0.1.0-rc.2 is not a draft; promote the draft of a candidate")
        self.assertEqual(
            self.refused(promote, "v0.1.0-rc.2", os.path.join(self.dir, "d"), "o/r", True,
                         self.promote_fake(with_files=False)),
            f"no SHA256SUMS in {os.path.join(self.dir, 'd')}")

    def test_the_draft_is_created_from_the_tag_and_verified_after_the_download(self):
        self.notes("# 0.1.0\n")
        directory = self.release_dir(self.FILES)
        write_sums(directory, "0.1.0")
        fake = self.promote_fake()
        draft(self.repo, "v0.1.0-rc.1", directory, "o/r", "c0ffee", fake)
        create = fake.commands[0]
        self.assertEqual(create[:12], ["gh", "release", "create", "v0.1.0-rc.1", "--repo", "o/r", "--draft",
                                       "--prerelease", "--verify-tag", "--title", "v0.1.0-rc.1", "--notes-file"])
        self.assertEqual(len(create), 13 + len(self.FILES) + 1)
        self.assertEqual(fake.commands[1][:4], ["gh", "release", "download", "v0.1.0-rc.1"])
        self.assertTrue(any(c[:3] == ["sha256sum", "--check", "--strict"] for c in fake.commands))
        # FT-1302: the draft binds each file to the tag's commit, as the promote does.
        verifies = [c for c in fake.commands if c[:3] == ["gh", "attestation", "verify"]]
        self.assertEqual(len(verifies), len(self.FILES) + 1)
        self.assertTrue(all(c[-2:] == ["--source-digest", "c0ffee"] for c in verifies), verifies)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
