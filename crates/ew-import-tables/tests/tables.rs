// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The committed golden tables: generator parity, file hygiene, key order and the library keys they reference.

mod common;

use std::collections::BTreeSet;
use std::process::Command;

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

fn parsed(name: &str) -> CsvTable {
    let bytes = std::fs::read(common::tables_dir().join(name)).unwrap();
    csv::parse(&bytes, &TableLimits::default()).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The distinct non-empty values of column `column` of table file `name`.
fn column_values(name: &str, column: &str) -> BTreeSet<String> {
    let t = parsed(name);
    let i = t
        .header
        .iter()
        .position(|h| h == column)
        .unwrap_or_else(|| panic!("{name} has no column {column}"));
    t.rows
        .iter()
        .map(|r| r.cells[i].clone())
        .filter(|c| !c.is_empty())
        .collect()
}

/// The committed tables equal what the doc-20 generator writes (`--check`).
#[test]
fn the_committed_tables_equal_the_generator_output() {
    let out = Command::new("python")
        .args(["-I", "golden/J-E-2416/tools/tables_from_doc20.py", "--check"])
        .current_dir(common::repo())
        .output()
        .expect("python is required to run the generator check");
    assert!(
        out.status.success(),
        "generator check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
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
        let t = parsed(&name);
        let i = t
            .header
            .iter()
            .position(|h| h == "key")
            .unwrap_or_else(|| panic!("{name}: no key column"));
        let mut prev: Option<&str> = None;
        for row in &t.rows {
            let key = row.cells[i].as_str();
            assert!(!key.is_empty(), "{name} line {}: empty key", row.line);
            if let Some(p) = prev {
                assert!(
                    ew_mltext::natural_cmp(p, key) == std::cmp::Ordering::Less,
                    "{name} line {}: key {key:?} does not follow {p:?} in natural order",
                    row.line
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
