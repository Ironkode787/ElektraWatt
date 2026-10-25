#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: MIT OR Apache-2.0

"""The path filter of CI-PR-08, the per-pull-request fuzz run (FT-335; stdlib only).

  git diff --name-only <base> HEAD | python3 -I ci/fuzz-smoke-targets.py
  python3 -I ci/fuzz-smoke-targets.py --all          # every target (no usable base: a dispatch, a new branch)
  python3 -I ci/fuzz-smoke-targets.py --selftest     # run the offline checks

Reads the changed paths of the change from standard input, one per line, and prints the cargo-fuzz targets of `fuzz/`
that the change can affect, as `<target>:<max_len>` words in the order of the table, on one line (an empty line:
nothing to run). The table is `ci/fuzz-smoke-targets.toml`: for each target its `-max_len` (the value of the deep
nightly's matrix) and the path-dependency closure of the crates it uses, which a test of xtask recomputes from the
Cargo.toml files, so the mapping cannot drift from the dependency graph.

- A change under `crates/<name>/` runs every target whose closure holds `<name>`, except what is not built into a fuzz
  target: the crate's `tests/`, `benches/` and `examples/` directories and its Markdown files. The one exception is the
  command generator of the nightly command fuzzer, `crates/ew-testkit/tests/command_fuzz/`, which `fuzz/` includes as
  modules.
- A change inside `fuzz/` runs the target it names (`fuzz_targets/<t>.rs`, `corpus/<t>/`, `regressions/<t>/`) and,
  anywhere else in the crate (the harness in `src/`, `Cargo.toml`, `Cargo.lock`, the toolchain file, the replay test),
  all targets; its README, its deny policy and what the tools write (`target/`, `artifacts/`, `coverage/`) run nothing.
- This script and its table are inputs of every target.
"""
import argparse
import os
import re
import sys
import tomllib

HERE = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(HERE, "fuzz-smoke-targets.toml"), "rb") as _f:
    TARGETS = tomllib.load(_f)["targets"]

# The targets in run order, with the `-max_len` of each.
MAX_LEN = {name: spec["max_len"] for name, spec in TARGETS.items()}

CRATE = re.compile(r"^crates/(?P<crate>[^/]+)/(?P<rest>.*)$")
# inside a crate: what no fuzz target is built from
NOT_BUILT = re.compile(r"^(?:tests|benches|examples)/|\.md$")
# the crate-internal path that a fuzz target includes as modules
INCLUDED = ("ew-testkit", "tests/command_fuzz/")
# fuzz/<kind>/<target>[.rs|/…]: a change that belongs to one target
OWN = re.compile(r"^fuzz/(?:fuzz_targets|corpus|regressions)/(?P<target>[A-Za-z0-9_]+)(?:\.rs|/)")
# fuzz/ paths that no run reads
IGNORED = re.compile(r"^fuzz/(?:README\.md|deny\.toml|\.gitignore|target/|artifacts/|coverage/)")
SELF = ("ci/fuzz-smoke-targets.py", "ci/fuzz-smoke-targets.toml")


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
    if p in SELF:
        return set(MAX_LEN)
    m = CRATE.match(p)
    if m:
        crate, rest = m.group("crate"), m.group("rest")
        if NOT_BUILT.search(rest) and not (crate == INCLUDED[0] and rest.startswith(INCLUDED[1])):
            return set()
        return {t for t, spec in TARGETS.items() if crate in spec["closure"]}
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
    assert list(MAX_LEN) == ["jsonl_loader", "migration_chain", "command_sequence", "png_pdf"], list(MAX_LEN)
    loader, chain, command = "jsonl_loader:65536", "migration_chain:65", "command_sequence:512"
    # nothing relevant
    assert run() == ""
    assert run("README.md", "docs/design/10-plc-and-bus.md", "crates/ew-ui/src/lib.rs") == ""
    assert run("fuzz/README.md", "fuzz/deny.toml", "fuzz/target/corpus/x", "fuzz/artifacts/y/crash-1") == ""
    # a crate's tests, benches, examples and Markdown are not built into a target
    assert run("crates/ew-format/tests/roundtrip.rs", "crates/ew-store/benches/b.rs", "crates/ew-derive/README.md") == ""
    # the closure of each target: the format crate is in all three (the command target saves through ew-store)
    assert run("crates/ew-format/src/lib.rs") == f"{loader} {chain} {command}"
    assert run("crates/ew-schema/src/plc.rs") == f"{loader} {chain} {command}"
    assert run("crates/ew-store/src/open.rs") == f"{chain} {command}"
    assert run("crates/ew-model/src/index.rs") == f"{chain} {command}"
    assert run("crates/ew-commands/src/tx.rs") == command
    assert run("crates/ew-derive/src/plc.rs") == command
    assert run("crates/ew-api/src/verify.rs") == command
    assert run("crates/ew-import-tables/Cargo.toml") == command
    assert run("crates/ew-testkit/src/lib.rs") == command
    assert run("crates/ew-testkit/tests/command_fuzz/arms.rs") == command, "the generator is included by fuzz/"
    assert run("crates/ew-testkit/tests/other.rs") == ""
    assert run("crates/ew-format-extra/x.rs") == "", "a prefix of a directory name is not the crate"
    # the fuzz crate
    assert run("fuzz/fuzz_targets/jsonl_loader.rs") == loader
    assert run("fuzz/corpus/migration_chain/seed-1") == chain
    assert run("fuzz/regressions/command_sequence/crash-x") == command
    assert run("fuzz/src/loader.rs") == every
    assert run("fuzz/Cargo.lock") == every
    assert run("fuzz/rust-toolchain.toml") == every
    assert run("fuzz/fuzz_targets/unknown.rs") == every, "a target the table does not know is a harness change"
    assert run("ci/fuzz-smoke-targets.py") == every
    assert run("ci/fuzz-smoke-targets.toml") == every
    # several paths: the union, in run order, once each
    assert (
        run("crates/ew-commands/a.rs", "crates/ew-format/b.rs", "crates/ew-dl/src/lib.rs", "crates/ew-commands/c.rs")
        == every
    )
    # Windows separators and a leading ./ (a local run)
    assert run("crates\\ew-store\\src\\lib.rs") == f"{chain} {command}"
    assert run("./crates/ew-commands/src/tx.rs") == command

    # the table agrees with the crate and the deep nightly's matrix (the dependency closure is checked by xtask)
    for spec in TARGETS.values():
        assert set(spec["roots"]) <= set(spec["closure"]), spec
    try:
        with open(os.path.join(HERE, "..", "fuzz", "Cargo.toml"), encoding="utf-8") as f:
            manifest = f.read()
        with open(os.path.join(HERE, "..", ".github", "workflows", "deep-nightly.yml"), encoding="utf-8") as f:
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
