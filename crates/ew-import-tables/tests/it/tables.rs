// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The committed golden tables: generator parity, file hygiene, key order and the library keys they reference.

use crate::common;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::OnceLock;

use ew_import_tables::csv::{self, CsvTable, TableLimits};
use ew_import_tables::table_names;

fn committed() -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = std::fs::read_dir(common::tables_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv"))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

fn parsed(name: &str) -> (Vec<u8>, CsvTable) {
    let bytes = std::fs::read(common::tables_dir().join(name)).unwrap();
    let table = csv::parse(&bytes, &TableLimits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
    (bytes, table)
}

/// The distinct non-empty values of column `column` of table file `name`.
fn column_values(name: &str, column: &str) -> BTreeSet<String> {
    let (bytes, t) = parsed(name);
    let i = t
        .header
        .iter()
        .position(|h| h == column)
        .unwrap_or_else(|| panic!("{name} has no column {column}"));
    (0..t.row_count())
        .map(|r| t.cell(&bytes, r, i).to_owned())
        .filter(|c| !c.is_empty())
        .collect()
}

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

/// Every committed CSV file is a table the importer knows, and every known table is committed.
#[test]
fn committed_files_and_known_tables_coincide() {
    let files: BTreeSet<String> = committed().into_iter().map(|(n, _)| n).collect();
    let known: BTreeSet<String> = table_names().collect();
    assert_eq!(files, known);
}

/// Every table is BOM-free, CR-free, ends in exactly one LF and parses under the default limits.
#[test]
fn every_table_is_of_the_dialect() {
    for (name, bytes) in committed() {
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "{name}: BOM");
        assert!(!bytes.contains(&b'\r'), "{name}: CR");
        assert!(bytes.ends_with(b"\n"), "{name}: no final LF");
        assert!(!bytes.ends_with(b"\n\n"), "{name}: more than one final LF");
        csv::parse(&bytes, &TableLimits::default()).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

/// The `key` column of every table is non-empty, unique and strictly increasing in natural order.
#[test]
fn keys_are_unique_and_in_natural_order() {
    for (name, _) in committed() {
        let (bytes, t) = parsed(&name);
        let i = t
            .header
            .iter()
            .position(|h| h == "key")
            .unwrap_or_else(|| panic!("{name}: no key column"));
        let mut prev: Option<&str> = None;
        for row in 0..t.row_count() {
            let key = t.cell(&bytes, row, i);
            assert!(!key.is_empty(), "{name} line {}: empty key", t.line(row));
            if let Some(p) = prev {
                assert!(
                    ew_mltext::natural_cmp(p, key) == std::cmp::Ordering::Less,
                    "{name} line {}: key {key:?} does not follow {p:?} in natural order",
                    t.line(row)
                );
            }
            prev = Some(key);
        }
    }
}

/// Every FnDef, symbol and form key the tables reference exists in the built-in library.
#[test]
fn content_keys_exist_in_the_builtin_library() {
    let lib = common::library();
    for (file, column) in [("functions.csv", "fndef"), ("part_templates.csv", "fndef")] {
        for key in column_values(file, column) {
            assert!(lib.fndef(&key).is_some(), "{file}: unknown FnDef {key}");
        }
    }
    for key in column_values("representations.csv", "symbol") {
        assert!(
            lib.symbol(&key).is_some(),
            "representations.csv: unknown symbol {key}"
        );
    }
    for (file, column) in [("pages.csv", "form"), ("report_blocks.csv", "form")] {
        for key in column_values(file, column) {
            assert!(lib.form(&key).is_some(), "{file}: unknown form {key}");
        }
    }
}
