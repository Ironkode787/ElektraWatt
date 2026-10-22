#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Line coverage of the core crates from a cargo-llvm-cov summary (CI-PR-13, FT-296, NFR-MAINT-06; stdlib only).

  python3 -I ci/coverage-summary.py <summary.json> [--threshold 80] [--out <report.md>]
  python3 -I ci/coverage-summary.py --selftest     # run the offline checks

`<summary.json>` is `cargo llvm-cov report --json --summary-only` (the llvm-cov export format) of a run of the
workspace tests. The report lists, for each core crate, the lines of its `crates/<name>/src/` files that the run
covered, against the threshold of NFR-MAINT-06 (80 % of lines), and the total of the core crates. Files under
`tests/` and `benches/` are not counted: they are the tests, not the code under test.

REPORT-ONLY: a crate below the threshold is listed and does not fail the check. The check fails (exit code 1) only
when the measurement itself failed: the file is not an llvm-cov summary, or a core crate has no instrumented line
(the run did not measure it). The owner sets the dated plan that makes the threshold gate (gate G-M1-15).
"""
import argparse
import json
import re
import sys

# The core crates of CI-PR-13 (18 §5.2, NFR-MAINT-06): the model, the commands, the memo engine, the derived-data
# engine and the two persistence crates.
CORE_CRATES = ("ew-model", "ew-commands", "ew-memo", "ew-derive", "ew-format", "ew-store")
SOURCE = re.compile(r"/crates/(?P<crate>[^/]+)/src/")


def measure(summary):
    """Returns {crate: (covered, count)} over the `src/` files of the core crates, or raises ValueError."""
    try:
        files = summary["data"][0]["files"]
    except (KeyError, IndexError, TypeError):
        raise ValueError("not an llvm-cov summary: no data[0].files")
    totals = {crate: [0, 0] for crate in CORE_CRATES}
    for entry in files:
        name = str(entry.get("filename", "")).replace("\\", "/")
        m = SOURCE.search(name)
        if not m or m.group("crate") not in totals:
            continue
        lines = entry.get("summary", {}).get("lines", {})
        totals[m.group("crate")][0] += int(lines.get("covered", 0))
        totals[m.group("crate")][1] += int(lines.get("count", 0))
    return {crate: (c, n) for crate, (c, n) in totals.items()}


def pct(covered, count):
    return 100.0 * covered / count if count else 0.0


def report(measured, threshold):
    """Returns (errors, markdown): the reasons the measurement failed and the report."""
    errors = [f"{crate}: no instrumented line (the run did not measure it)" for crate, (_, n) in measured.items() if n == 0]
    rows = ["| Crate | Lines | Covered | Line coverage | Threshold |", "|---|---:|---:|---:|---|"]
    below = []
    for crate in CORE_CRATES:
        covered, count = measured[crate]
        p = pct(covered, count)
        met = count > 0 and p >= threshold
        if count > 0 and not met:
            below.append(crate)
        rows.append(f"| {crate} | {count} | {covered} | {p:.1f} % | {'met' if met else 'below'} |")
    covered = sum(c for c, _ in measured.values())
    count = sum(n for _, n in measured.values())
    rows.append(f"| **core crates** | {count} | {covered} | **{pct(covered, count):.1f} %** | |")
    text = [f"### Line coverage of the core crates (threshold {threshold:g} %, report-only)", ""] + rows + [""]
    if below:
        text.append(f"Below the threshold: {', '.join(below)}. Report-only until the owner sets the dated plan (G-M1-15).")
    else:
        text.append("Every core crate meets the threshold.")
    return errors, "\n".join(text) + "\n"


def selftest():
    def entry(path, covered, count):
        return {"filename": path, "summary": {"lines": {"count": count, "covered": covered, "percent": 0}}}

    files = [
        entry("/w/crates/ew-model/src/a.rs", 90, 100),
        entry("/w/crates/ew-model/src/b/c.rs", 10, 100),
        entry("/w/crates/ew-model/tests/t.rs", 100, 100),  # a test file is not counted
        entry("/w/crates/ew-commands/src/a.rs", 8, 10),
        entry("/w/crates/ew-memo/src/a.rs", 1, 1),
        entry("/w/crates/ew-derive/src/a.rs", 5, 10),
        entry("C:\\w\\crates\\ew-format\\src\\a.rs", 9, 10),  # a Windows path
        entry("/w/crates/ew-store/src/a.rs", 10, 10),
        entry("/w/crates/ew-api/src/a.rs", 0, 1000),  # not a core crate
        entry("/w/bin/ew/src/main.rs", 0, 1000),
    ]
    summary = {"type": "llvm.coverage.json.export", "data": [{"files": files, "totals": {}}]}
    measured = measure(summary)
    assert measured["ew-model"] == (100, 200), measured
    assert measured["ew-format"] == (9, 10), measured
    errors, text = report(measured, 80)
    assert errors == [], errors
    assert "| ew-model | 200 | 100 | 50.0 % | below |" in text, text
    assert "| ew-commands | 10 | 8 | 80.0 % | met |" in text, text
    assert "| ew-derive | 10 | 5 | 50.0 % | below |" in text, text
    assert "Below the threshold: ew-model, ew-derive." in text, text
    assert "| **core crates** | 241 | 133 | **55.2 %** | |" in text, text

    # all met
    ok = {"data": [{"files": [entry(f"/w/crates/{c}/src/a.rs", 9, 10) for c in CORE_CRATES]}]}
    errors, text = report(measure(ok), 80)
    assert errors == [] and "Every core crate meets the threshold." in text, (errors, text)

    # a crate that was not measured fails the check; a file that is not a summary raises
    partial = {"data": [{"files": [entry("/w/crates/ew-model/src/a.rs", 1, 1)]}]}
    errors, _ = report(measure(partial), 80)
    assert len(errors) == len(CORE_CRATES) - 1 and "ew-store: no instrumented line" in errors[-1], errors
    for bad in ({}, {"data": []}, {"data": [{}]}, [], None):
        try:
            measure(bad)
        except ValueError:
            continue
        raise AssertionError(f"{bad!r} was accepted")
    print("coverage-summary selftest: ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("summary", nargs="?", help="cargo llvm-cov report --json --summary-only output")
    parser.add_argument("--threshold", type=float, default=80.0, help="the line threshold in percent (default 80)")
    parser.add_argument("--out", help="write the Markdown report to this file")
    parser.add_argument("--selftest", action="store_true", help="run the offline checks and exit")
    args = parser.parse_args()
    if args.selftest:
        selftest()
        return 0
    if not args.summary:
        parser.error("the summary file is required")
    try:
        with open(args.summary, encoding="utf-8") as f:
            measured = measure(json.load(f))
    except (OSError, ValueError) as e:  # json.JSONDecodeError is a ValueError
        print(f"coverage-summary: cannot read {args.summary}: {e}", file=sys.stderr)
        return 1
    errors, text = report(measured, args.threshold)
    print(text)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(text)
    for error in errors:
        print(f"coverage-summary: {error}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
