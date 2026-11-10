// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The committed golden tables: generator parity.

use crate::common;

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::OnceLock;

/// The generator script, relative to the repository root.
const GENERATOR: &str = "golden/J-E-2416/tools/tables_from_doc20.py";

/// The Python 3 command of this machine: the first of `python3`, `python` that answers `--version`. CI runners may
/// have only one of the two.
fn python() -> &'static str {
    static PYTHON: OnceLock<&'static str> = OnceLock::new();
    PYTHON.get_or_init(|| {
        ["python3", "python"]
            .into_iter()
            .find(|cmd| {
                Command::new(cmd)
                    .arg("--version")
                    .output()
                    .is_ok_and(|o| o.status.success())
            })
            .expect(
                "the golden generator needs Python 3, but neither `python3` nor `python` runs `--version`",
            )
    })
}

/// Runs the generator (isolated mode) from the repository root with `args`.
fn generator(args: &[&OsStr]) -> Output {
    Command::new(python())
        .args(["-I", GENERATOR])
        .args(args)
        .current_dir(common::repo())
        .output()
        .unwrap_or_else(|e| panic!("running `{} {GENERATOR}`: {e}", python()))
}

fn report(out: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The committed tables equal what the doc-20 generator writes (`--check`).
#[test]
fn the_committed_tables_equal_the_generator_output() {
    let out = generator(&[OsStr::new("--check")]);
    assert!(out.status.success(), "generator check failed\n{}", report(&out));
}

/// Doc 20, the only data source of the generator.
fn doc_20() -> (PathBuf, String) {
    let path = common::repo().join("docs/design/20-golden-project.md");
    let text = std::fs::read_to_string(&path).unwrap();
    (path, text)
}

/// Writes `text` as a copy of doc 20 into a fresh directory of `CARGO_TARGET_TMPDIR`.
fn write_doc_copy(name: &str, text: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("golden-doc20-{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("20-golden-project.md");
    std::fs::write(&path, text).unwrap();
    path
}

/// A copy of doc 20 that is not mutated passes the check: the `--doc` option reads the same data.
#[test]
fn an_unmutated_copy_of_doc_20_passes_the_check() {
    let (_, text) = doc_20();
    let copy = write_doc_copy("control", &text);
    let out = generator(&[OsStr::new("--doc"), copy.as_os_str(), OsStr::new("--check")]);
    assert!(
        out.status.success(),
        "the unmutated copy fails the check\n{}",
        report(&out)
    );
}

/// `(name, substring of doc 20, replacement)`: every hard-coded value of the generator is tied to a doc cell, so
/// changing that cell must fail the check (by a doc check or by a committed table that differs).
const MUTATIONS: &[(&str, &str, &str)] = &[
    ("cp-list-2904602", "+#1, +#2, −#1", "+#1, −#1, +#2"),
    ("cp-list-mico-4-10", "Out1 … Out4", "Out1 … Out3"),
    ("pin-550208", "4 24V El", "4 0V El"),
    (
        "potential-type",
        "| +24V | L+, 24 V DC, control |",
        "| +24V | M, 24 V DC, control |",
    ),
    (
        "potential-first-member",
        "| -102G1:+#1, +#2, -102F1:1 |",
        "| -102G1:+#2, +#1, -102F1:1 |",
    ),
    (
        "junction-sequence",
        "W24, -102F1:2, W25 (far ends in order:",
        "W25, -102F1:2, W24 (far ends in order:",
    ),
    (
        "part-number",
        "Phoenix Contact 2904602 | L, N, PE",
        "Phoenix Contact 2904603 | L, N, PE",
    ),
    (
        "bus-potential-members",
        "| -201U1:DP, -3X1, -3W1 A/B",
        "| -3X1, -3W1 A/B",
    ),
    (
        "bus-cable-drawn-single-line",
        "drawn single-line (FR-CBL-008)",
        "drawn per core (FR-CBL-008)",
    ),
    (
        "plc-station-role",
        "role valve terminal / DP slave",
        "role valve terminal / DP master",
    ),
    (
        "plc-modules",
        "-201U3 … -201U8, -202U2 in manifold slots",
        "-201U3 … -201U7, -202U2 in manifold slots",
    ),
    ("channel-binding-position", "| Q2 TA05-V-33 |", "| Q3 TA05-V-33 |"),
    (
        "device-link-driver",
        "| -203Q5 … -203Q8 | TA05-V-36, TA05-V-38, TA05-V-16, TA05-V-37 | +FIELD ▲ | -201U4-Q1 … Q4 |",
        "| -203Q5 … -203Q8 | TA05-V-36, TA05-V-38, TA05-V-16, TA05-V-37 | +FIELD ▲ | -201U4-Q1 … Q3 |",
    ),
    (
        "strip-numbering-format",
        "strips `{letter}{page}{counter}`",
        "strips `{page}{letter}{counter}`",
    ),
];

/// Each mutation of a doc-20 cell the generator consumes makes `--check` exit non-zero.
#[test]
fn doc_20_mutations_fail_the_check() {
    let (_, text) = doc_20();
    for (name, from, to) in MUTATIONS {
        assert_eq!(
            text.matches(from).count(),
            1,
            "mutation {name}: the substring {from:?} must occur exactly once in doc 20"
        );
        let mutated = text.replacen(from, to, 1);
        assert_ne!(mutated, text, "mutation {name} changes nothing");
        let copy = write_doc_copy(name, &mutated);
        let out = generator(&[OsStr::new("--doc"), copy.as_os_str(), OsStr::new("--check")]);
        assert!(
            !out.status.success(),
            "mutation {name} ({from:?} -> {to:?}) passes the check\n{}",
            report(&out)
        );
    }
}
