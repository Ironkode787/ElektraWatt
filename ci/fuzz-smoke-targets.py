#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The path filter of CI-PR-08, the per-pull-request fuzz run (FT-335; stdlib only).

  git diff --name-only <base> HEAD | python3 -I ci/fuzz-smoke-targets.py
  python3 -I ci/fuzz-smoke-targets.py --all          # every target (no usable base: a dispatch, a new branch)
  python3 -I ci/fuzz-smoke-targets.py --selftest     # run the offline checks

Reads the changed paths of the change from standard input, one per line, and prints the cargo-fuzz targets of `fuzz/`
that the change can affect, as `<target>:<max_len>` words in the order of the table below, on one line (an empty line:
nothing to run). `max_len` is the libFuzzer `-max_len` of the target, the value of the deep nightly's matrix.

The mapping follows what each target reaches (fuzz/README.md):

- `jsonl_loader` feeds `ew-format` alone;
- `migration_chain` opens a project directory with `ew-store`, which reads and writes through `ew-format`;
- `command_sequence` drives the seeded command generator of `ew-testkit` (`crates/ew-testkit/tests/command_fuzz`)
  through `ew-commands` over `ew-model`, with a save-and-reload round trip through `ew-store`.

So a change of `crates/ew-format` runs the first two, of `crates/ew-store` the last two, of `crates/ew-commands`,
`crates/ew-model` or the generator the last one. A change inside `fuzz/` runs the target it names (`fuzz_targets/<t>.rs`,
`corpus/<t>/`, `regressions/<t>/`) and, anywhere else in the crate (the harness in `src/`, `Cargo.toml`, `Cargo.lock`,
the toolchain file, the replay test), all three; its README, its deny policy and what the tools write (`target/`,
`artifacts/`, `coverage/`) run nothing. This script is itself an input.
"""
import argparse
import re
import sys

# The targets in run order, with the `-max_len` of each (the same numbers as the matrix of deep-nightly.yml; the
# selftest compares them).
MAX_LEN = {"jsonl_loader": 65536, "migration_chain": 65, "command_sequence": 512}

# crate directory prefix -> the targets that reach it
CRATES = (
    ("crates/ew-format/", ("jsonl_loader", "migration_chain")),
    ("crates/ew-store/", ("migration_chain", "command_sequence")),
    ("crates/ew-commands/", ("command_sequence",)),
    ("crates/ew-model/", ("command_sequence",)),
    ("crates/ew-testkit/tests/command_fuzz/", ("command_sequence",)),
    # this script and the test of its mapping: a change of the filter runs everything it can select
    ("ci/fuzz-smoke-targets.py", tuple(MAX_LEN)),
)

# fuzz/<kind>/<target>[.rs|/…]: a change that belongs to one target
OWN = re.compile(r"^fuzz/(?:fuzz_targets|corpus|regressions)/(?P<target>[A-Za-z0-9_]+)(?:\.rs|/)")
# fuzz/ paths that no run reads
IGNORED = re.compile(r"^fuzz/(?:README\.md|deny\.toml|\.gitignore|target/|artifacts/|coverage/)")


def targets_for(path):
    """The set of target names that a change of `path` can affect."""
    p = path.strip().replace("\\", "/")
    if p.startswith("./"):
        p = p[2:]
    if p.startswith("fuzz/"):
        if IGNORED.match(p):
            return set()
        m = OWN.match(p)
        if m and m.group("target") in MAX_LEN:
            return {m.group("target")}
        return set(MAX_LEN)
    for prefix, targets in CRATES:
        if p.startswith(prefix):
            return set(targets)
    return set()


def select(paths):
    """The `target:max_len` words for the changed `paths`, in run order."""
    chosen = set()
    for path in paths:
        chosen |= targets_for(path)
    return [f"{t}:{MAX_LEN[t]}" for t in MAX_LEN if t in chosen]


def selftest():
    def run(*paths):
        return " ".join(select(paths))

    every = " ".join(f"{t}:{n}" for t, n in MAX_LEN.items())
    # nothing relevant
    assert run() == ""
    assert run("README.md", "crates/ew-derive/src/plc.rs", "docs/design/10-plc-and-bus.md") == ""
    assert run("fuzz/README.md", "fuzz/deny.toml", "fuzz/target/corpus/x", "fuzz/artifacts/y/crash-1") == ""
    # the crates and what they reach
    assert run("crates/ew-format/src/lib.rs") == "jsonl_loader:65536 migration_chain:65"
    assert run("crates/ew-store/src/open.rs") == "migration_chain:65 command_sequence:512"
    assert run("crates/ew-commands/src/tx.rs") == "command_sequence:512"
    assert run("crates/ew-model/src/index.rs") == "command_sequence:512"
    assert run("crates/ew-testkit/tests/command_fuzz/arms.rs") == "command_sequence:512"
    assert run("crates/ew-testkit/tests/other.rs") == ""
    assert run("crates/ew-format-extra/x.rs") == "", "a prefix of a directory name is not the crate"
    # the fuzz crate
    assert run("fuzz/fuzz_targets/jsonl_loader.rs") == "jsonl_loader:65536"
    assert run("fuzz/corpus/migration_chain/seed-1") == "migration_chain:65"
    assert run("fuzz/regressions/command_sequence/crash-x") == "command_sequence:512"
    assert run("fuzz/src/loader.rs") == every
    assert run("fuzz/Cargo.lock") == every
    assert run("fuzz/rust-toolchain.toml") == every
    assert run("fuzz/fuzz_targets/unknown.rs") == every, "a target the table does not know is a harness change"
    assert run("ci/fuzz-smoke-targets.py") == every
    # several paths: the union, in run order, once each
    assert (
        run("crates/ew-commands/a.rs", "crates/ew-format/b.rs", "crates/ew-commands/c.rs")
        == "jsonl_loader:65536 migration_chain:65 command_sequence:512"
    )
    # Windows separators and a leading ./ (a local run)
    assert run("crates\\ew-format\\src\\lib.rs") == "jsonl_loader:65536 migration_chain:65"
    assert run("./crates/ew-commands/src/tx.rs") == "command_sequence:512"

    # the table agrees with the crate and the deep nightly's matrix
    try:
        with open("fuzz/Cargo.toml", encoding="utf-8") as f:
            manifest = f.read()
        with open(".github/workflows/deep-nightly.yml", encoding="utf-8") as f:
            nightly = f.read()
    except OSError as e:
        print(f"fuzz-smoke-targets selftest: skipping the cross-checks ({e})")
    else:
        bins = set(re.findall(r'(?m)^name = "([a-z_]+)"\npath = "fuzz_targets/', manifest))
        assert bins == set(MAX_LEN), f"the targets of fuzz/Cargo.toml are {sorted(bins)}"
        matrix = dict(re.findall(r'(?m)^\s*- target: (\w+)\n\s*max_len: "(\d+)"', nightly))
        assert {t: int(n) for t, n in matrix.items()} == MAX_LEN, f"the matrix of deep-nightly.yml is {matrix}"
    print("fuzz-smoke-targets selftest: ok")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--all", action="store_true", help="print every target without reading paths")
    parser.add_argument("--selftest", action="store_true", help="run the offline checks and exit")
    args = parser.parse_args()
    if args.selftest:
        selftest()
        return 0
    if args.all:
        print(" ".join(f"{t}:{n}" for t, n in MAX_LEN.items()))
        return 0
    print(" ".join(select(line for line in sys.stdin.read().splitlines() if line.strip())))
    return 0


if __name__ == "__main__":
    sys.exit(main())
