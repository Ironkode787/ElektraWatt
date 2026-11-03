#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The cross-platform digests of the release `ew` on Golden A (WP-S6.06 D14; doc 18 CI-N-04, AC-14; FT-234).

  python3 -I ci/golden-digests.py <ew> <project> <out-dir>
  python3 -I ci/golden-digests.py --compare <a> <b> --expected <dir>
  python3 -I ci/golden-digests.py --test                                 # its offline unit test

The first form runs <ew> (a release build, D5 and D6) on <project> and writes <out-dir>/digests.txt: `<sha256>  <name>`
of every output, sorted by name, then the line `fmt-check clean`:

  pdf/threads-1.pdf, pdf/threads-4.pdf       ew export pdf --threads 1 and 4
  svg/serial/<folio>.svg                     ew export svg --threads 1 (36 files on Golden A)
  svg/threads-4/<folio>.svg                  ew export svg --threads 4
  report/<kind>.csv                          ew export report --kind <kind> --format csv, for each kind of D12 (the
                                             names of golden/J-E-2416/expected/csv that this `ew` takes as a kind)
  rows/wires.csv, rows/terminals.csv,        ew export wires, terminals, cables and bom
  rows/cables.csv, rows/bom.csv
  fmt-check clean                            ew fmt --check finds no file to rewrite

The programs run without EW_CONTENT_DIR and SOURCE_DATE_EPOCH, so the bytes are the project's alone. <out-dir> must
not exist; the outputs stay there.

`--compare` reads two such manifests (one per OS) and fails, naming each file, on any line that differs or that only
one holds, and on a PDF whose digest is not <dir>/pdf/sha256.txt (with both digests); else it prints how many files it
compared. `--test` is the offline unit test. Standard library only.
"""
import argparse
import hashlib
import os
import subprocess
import sys
import tempfile
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXPECTED_CSV = os.path.join(REPO, "golden", "J-E-2416", "expected", "csv")
CLEAN = "fmt-check clean"
ROWS = ("wires", "terminals", "cables", "bom")
DROPPED = ("EW_CONTENT_DIR", "SOURCE_DATE_EPOCH")


class Refusal(Exception):
    """A failed check, with the message the script prints."""


def run_ew(command):
    """Runs `command` (the `ew` path first) without the variables of DROPPED: (exit code, stderr text)."""
    env = {k: v for k, v in os.environ.items() if k not in DROPPED}
    done = subprocess.run(command, env=env, capture_output=True, check=False)
    return done.returncode, done.stderr.decode("utf-8", "replace")


def sha256(path):
    """The SHA-256 of the file at `path`, in hex."""
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def report_kinds(csv_dir):
    """The candidate report kinds: the names of the CSV files in `csv_dir`, sorted."""
    return sorted(n[: -len(".csv")] for n in os.listdir(csv_dir) if n.endswith(".csv"))


def write_manifest(ew, project, out, run=run_ew, csv_dir=EXPECTED_CSV):
    """Runs `ew` on `project` into the new directory `out` and writes `out/digests.txt`; its lines."""
    if os.path.exists(out):
        raise Refusal(f"the output directory exists: {out}")
    for sub in ("pdf", "svg", "report", "rows"):
        os.makedirs(os.path.join(out, sub))

    def must(command, what):
        code, err = run([ew] + command)
        if code != 0:
            raise Refusal(f"{what} failed with exit code {code}: {err.strip()}")

    for threads in ("1", "4"):
        name = os.path.join(out, "pdf", f"threads-{threads}.pdf")
        must(["export", "pdf", project, "--out", name, "--threads", threads], f"ew export pdf --threads {threads}")
    for folder, threads in (("serial", "1"), ("threads-4", "4")):
        name = os.path.join(out, "svg", folder)
        must(["export", "svg", project, "--out", name, "--threads", threads], f"ew export svg --threads {threads}")
    for kind in report_kinds(csv_dir):
        name = os.path.join(out, "report", f"{kind}.csv")
        code, err = run([ew, "export", "report", "--kind", kind, "--format", "csv", project, "--out", name])
        if code != 0 and f"unknown report kind `{kind}`" in err:
            continue
        if code != 0:
            raise Refusal(f"ew export report --kind {kind} failed with exit code {code}: {err.strip()}")
    for rows in ROWS:
        must(["export", rows, project, "--out", os.path.join(out, "rows", f"{rows}.csv")], f"ew export {rows}")
    code, err = run([ew, "fmt", "--check", project])
    if code != 0:
        raise Refusal(f"ew fmt --check finds files to rewrite (exit code {code}): {err.strip()}")
    files = []
    for base, dirnames, filenames in os.walk(out):
        dirnames.sort()
        for n in filenames:
            path = os.path.join(base, n)
            files.append((os.path.relpath(path, out).replace(os.sep, "/"), path))
    lines = [f"{sha256(path)}  {name}" for name, path in sorted(files, key=lambda f: f[0].encode("utf-8"))]
    lines.append(CLEAN)
    with open(os.path.join(out, "digests.txt"), "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    return lines


def read_manifest(path):
    """{name: digest} of the manifest at `path`, and whether it holds the line `fmt-check clean`."""
    entries, clean = {}, False
    with open(path, encoding="utf-8") as f:
        for line in f.read().splitlines():
            if line == CLEAN:
                clean = True
            elif line:
                digest, _, name = line.partition("  ")
                entries[name] = digest
    return entries, clean


def compare(a, b, expected):
    """The number of files compared; refused with every difference between the manifests `a` and `b`, and every PDF
    whose digest is not `expected`/pdf/sha256.txt."""
    with open(os.path.join(expected, "pdf", "sha256.txt"), encoding="utf-8") as f:
        want = f.read().strip()
    (left, left_clean), (right, right_clean) = read_manifest(a), read_manifest(b)
    problems = []
    for name in sorted(set(left) | set(right), key=lambda n: n.encode("utf-8")):
        if name not in right:
            problems.append(f"{name} is only in {a}")
        elif name not in left:
            problems.append(f"{name} is only in {b}")
        elif left[name] != right[name]:
            problems.append(f"{name} differs: {left[name]} in {a}, {right[name]} in {b}")
    for path, entries, clean in ((a, left, left_clean), (b, right, right_clean)):
        for name, digest in sorted(entries.items()):
            if name.startswith("pdf/") and digest != want:
                problems.append(f"{name} of {path} has the SHA-256 {digest}, not the {want} of the expected PDF")
        if not clean:
            problems.append(f"{path} lacks the line `{CLEAN}`")
    if not any(name.startswith("pdf/") for name in left):
        problems.append(f"{a} holds no PDF")
    if problems:
        raise Refusal("the digests of the two platforms differ:\n" + "\n".join(problems))
    return len(left)


def main(argv):
    parser = argparse.ArgumentParser(prog="golden-digests.py", description=__doc__.split("\n")[0])
    parser.add_argument("--test", action="store_true", help="run the offline unit test")
    parser.add_argument("--compare", nargs=2, metavar=("A", "B"), help="compare two manifests")
    parser.add_argument("--expected", help="the expected folder of Golden A (with --compare)")
    parser.add_argument("args", nargs="*", metavar="ew project out-dir")
    args = parser.parse_args(argv)
    if args.test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(DigestsTest)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    try:
        if args.compare:
            if not args.expected or args.args:
                parser.error("--compare <a> <b> needs --expected <dir> and nothing else")
            count = compare(args.compare[0], args.compare[1], args.expected)
            print(f"golden-digests: the two manifests agree on {count} files, the PDFs on the expected digest")
        elif len(args.args) == 3:
            lines = write_manifest(*args.args)
            print(f"golden-digests: {len(lines) - 1} files and `{CLEAN}` in {os.path.join(args.args[2], 'digests.txt')}")
        else:
            parser.error("give <ew> <project> <out-dir>, --compare <a> <b> --expected <dir>, or --test")
    except Refusal as refusal:
        print(refusal, file=sys.stderr)
        return 1
    return 0


class DigestsTest(unittest.TestCase):
    """IN-17 over manifests and a stand-in `ew` in a temporary directory."""

    PDF = "a" * 64

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = self.tmp.name
        self.expected = os.path.join(self.dir, "expected")
        os.makedirs(os.path.join(self.expected, "pdf"))
        with open(os.path.join(self.expected, "pdf", "sha256.txt"), "w", encoding="utf-8") as f:
            f.write(self.PDF + "\n")

    def tearDown(self):
        self.tmp.cleanup()

    def manifest(self, name, changed=None):
        lines = {
            "pdf/threads-1.pdf": self.PDF,
            "pdf/threads-4.pdf": self.PDF,
            "report/toc.csv": "1" * 64,
            "svg/serial/101.svg": "2" * 64,
            "svg/threads-4/101.svg": "2" * 64,
        }
        lines.update(changed or {})
        path = os.path.join(self.dir, name)
        with open(path, "w", encoding="utf-8", newline="\n") as f:
            f.write("".join(f"{d}  {n}\n" for n, d in sorted(lines.items())) + CLEAN + "\n")
        return path

    def test_equal_manifests_pass(self):
        a, b = self.manifest("digests-linux.txt"), self.manifest("digests-windows.txt")
        self.assertEqual(compare(a, b, self.expected), 5)

    def test_a_manifest_that_differs_in_one_svg_fails_naming_it(self):
        a = self.manifest("digests-linux.txt")
        b = self.manifest("digests-windows.txt", {"svg/threads-4/101.svg": "3" * 64})
        with self.assertRaises(Refusal) as refused:
            compare(a, b, self.expected)
        self.assertEqual(
            str(refused.exception),
            "the digests of the two platforms differ:\n"
            f"svg/threads-4/101.svg differs: {'2' * 64} in {a}, {'3' * 64} in {b}",
        )

    def test_a_pdf_digest_other_than_the_expected_one_fails_with_both(self):
        other = "b" * 64
        a = self.manifest("digests-linux.txt", {"pdf/threads-1.pdf": other})
        b = self.manifest("digests-windows.txt", {"pdf/threads-1.pdf": other})
        with self.assertRaises(Refusal) as refused:
            compare(a, b, self.expected)
        self.assertEqual(
            str(refused.exception),
            "the digests of the two platforms differ:\n"
            f"pdf/threads-1.pdf of {a} has the SHA-256 {other}, not the {self.PDF} of the expected PDF\n"
            f"pdf/threads-1.pdf of {b} has the SHA-256 {other}, not the {self.PDF} of the expected PDF",
        )

    def test_a_file_in_one_manifest_only_and_a_missing_clean_line_fail(self):
        a = self.manifest("digests-linux.txt", {"rows/bom.csv": "4" * 64})
        b = self.manifest("digests-windows.txt")
        with open(b, encoding="utf-8") as f:
            text = f.read()
        with open(b, "w", encoding="utf-8", newline="\n") as f:
            f.write(text.replace(CLEAN + "\n", ""))
        with self.assertRaises(Refusal) as refused:
            compare(a, b, self.expected)
        self.assertEqual(
            str(refused.exception),
            f"the digests of the two platforms differ:\nrows/bom.csv is only in {a}\n{b} lacks the line `{CLEAN}`",
        )

    def stand_in(self, refused=()):
        """A stand-in `ew` that writes each output from its arguments, and the commands it ran."""
        ran = []

        def run(command):
            ran.append(command[1:])
            args = command[1:]
            if args[0] == "fmt":
                return 0, ""
            out = args[args.index("--out") + 1]
            if args[:2] == ["export", "report"] and args[3] in refused:
                return 2, f"error: unknown report kind `{args[3]}`\n"
            if args[:2] == ["export", "svg"]:
                os.makedirs(out)
                for folio in ("101", "2"):
                    with open(os.path.join(out, f"{folio}.svg"), "w", encoding="utf-8") as f:
                        f.write(f"<svg>{folio}</svg>")
                return 0, ""
            with open(out, "w", encoding="utf-8") as f:
                f.write(" ".join(a for a in args if a != out))
            return 0, ""

        return run, ran

    def test_the_manifest_lists_every_output_sorted_and_the_clean_line(self):
        csv_dir = os.path.join(self.dir, "csv")
        os.makedirs(csv_dir)
        for kind in ("toc", "wires", "device_list"):
            with open(os.path.join(csv_dir, f"{kind}.csv"), "w", encoding="utf-8") as f:
                f.write("x\n")
        run, ran = self.stand_in(refused=("wires",))
        out = os.path.join(self.dir, "out")
        lines = write_manifest("ew", "golden", out, run=run, csv_dir=csv_dir)
        self.assertEqual(
            [line.split("  ", 1)[1] if "  " in line else line for line in lines],
            [
                "pdf/threads-1.pdf",
                "pdf/threads-4.pdf",
                "report/device_list.csv",
                "report/toc.csv",
                "rows/bom.csv",
                "rows/cables.csv",
                "rows/terminals.csv",
                "rows/wires.csv",
                "svg/serial/101.svg",
                "svg/serial/2.svg",
                "svg/threads-4/101.svg",
                "svg/threads-4/2.svg",
                CLEAN,
            ],
        )
        self.assertEqual(lines[0], f"{sha256(os.path.join(out, 'pdf', 'threads-1.pdf'))}  pdf/threads-1.pdf")
        with open(os.path.join(out, "digests.txt"), encoding="utf-8") as f:
            self.assertEqual(f.read(), "\n".join(lines) + "\n")
        self.assertEqual(ran[0], ["export", "pdf", "golden", "--out", os.path.join(out, "pdf", "threads-1.pdf"), "--threads", "1"])
        self.assertEqual(ran[-1], ["fmt", "--check", "golden"])
        with self.assertRaises(Refusal) as refused:
            write_manifest("ew", "golden", out, run=run, csv_dir=csv_dir)
        self.assertEqual(str(refused.exception), f"the output directory exists: {out}")

    def test_a_failing_export_or_fmt_check_is_refused(self):
        def run(command):
            if command[1] == "fmt":
                return 1, "would rewrite pages/1.jsonl\n"
            return (2, "error: cannot open\n") if command[2] == "cables" else (0, "")

        csv_dir = os.path.join(self.dir, "csv")
        os.makedirs(csv_dir)
        with self.assertRaises(Refusal) as refused:
            write_manifest("ew", "golden", os.path.join(self.dir, "a"), run=run, csv_dir=csv_dir)
        self.assertEqual(str(refused.exception), "ew export cables failed with exit code 2: error: cannot open")
        with self.assertRaises(Refusal) as refused:
            write_manifest("ew", "golden", os.path.join(self.dir, "b"), run=lambda c: (1, "x") if c[1] == "fmt" else (0, ""), csv_dir=csv_dir)
        self.assertEqual(str(refused.exception), "ew fmt --check finds files to rewrite (exit code 1): x")


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
