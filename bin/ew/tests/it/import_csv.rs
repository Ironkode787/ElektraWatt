// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end tests of `ew import csv`: the test tables written as a new project.

#[allow(clippy::wildcard_imports)]
use crate::cli::*;
#[allow(clippy::wildcard_imports)]
use crate::support::*;
use std::path::PathBuf;
use std::process::Output;

/// A copy of the test tables in `t` as `name`.
fn tables_copy(t: &Tmp, name: &str) -> PathBuf {
    let copy = t.0.join(name);
    copy_dir(std::path::Path::new(TABLES), &copy);
    copy
}

/// What `ew import csv` prints on stdout for the import of `tables` in `mode` (the finding lines, from the library
/// itself), ending with `last` when given.
fn import_lines(tables: &std::path::Path, mode: ew_api::import::ImportMode, last: Option<&str>) -> String {
    let library = ew_api::import::StarterLibrary::from_dir(std::path::Path::new(CONTENT)).unwrap();
    let inputs = ew_api::import::read_tables(tables).unwrap();
    let batch = ew_api::import::import_tables(&inputs, &library, mode).unwrap();
    let mut out = String::new();
    for e in &batch.report.entries {
        let severity = match e.severity {
            ew_api::import::Severity::Info => "info",
            ew_api::import::Severity::Warning => "warning",
            ew_api::import::Severity::Error => "error",
        };
        let at = match &e.location {
            Some(l) => match l.line {
                Some(n) => format!("{}:{n}: ", l.file),
                None => format!("{}: ", l.file),
            },
            None => String::new(),
        };
        out.push_str(&format!("{} {severity}: {at}{}\n", e.code, e.message));
    }
    if let Some(last) = last {
        out.push_str(last);
        out.push('\n');
    }
    out
}

/// `ew import csv` with the starter content.
fn import_csv(t: &Tmp, tables: &std::path::Path, out: &std::path::Path, extra: &[&str]) -> Output {
    let content = std::path::Path::new(CONTENT);
    ew_in(
        t,
        &[
            "import".as_ref(),
            "csv".as_ref(),
            tables.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--content".as_ref(),
            content.as_os_str(),
        ],
        extra,
    )
}

/// Through the binary: the test tables import to a new project that is in canonical form and opens at
/// generation 1 (its model and check findings against the bootstrap are `crates/ew-api/tests/import_csv.rs`). Without
/// the starter content, or
/// into an `--out` that holds something, nothing is written (exit 2).
#[test]
fn import_csv_writes_the_test_tables_as_a_new_project() {
    let t = Tmp::new("ew-cli-test");
    let tables = std::path::Path::new(TABLES);
    let out = t.0.join("new.ew");
    let o = import_csv(&t, tables, &out, &[]);
    let n = ew_api::import::read_tables(tables).unwrap().len();
    let library = ew_api::import::StarterLibrary::from_dir(std::path::Path::new(CONTENT)).unwrap();
    let batch = ew_api::import::import_tables(
        &ew_api::import::read_tables(tables).unwrap(),
        &library,
        ew_api::import::ImportMode::Strict,
    )
    .unwrap();
    let last = format!(
        "imported {n} tables into {}: {} records, {} library documents",
        out.display(),
        batch.records.len(),
        batch.lib.len()
    );
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(0),
            import_lines(tables, ew_api::import::ImportMode::Strict, Some(&last)),
            String::new()
        )
    );
    let o = t.ew(&["fmt".as_ref(), out.as_os_str(), "--check".as_ref()]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "the new project is canonical: {}",
        stderr(&o)
    );
    let (s, report) = ew_api::Session::open_reader(&out, "test", "test").unwrap();
    assert_eq!(report.generation, 1);
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    s.close();
    let written = tree(&out);

    // An `--out` that holds something: refused, kept.
    let o = import_csv(&t, tables, &out, &[]);
    assert_eq!(
        (o.status.code(), stderr(&o)),
        (
            Some(2),
            format!(
                "error: {}: not an empty directory; the new project needs a new or empty directory\n",
                out.display()
            )
        )
    );
    assert!(tree(&out) == written, "the project there is kept");
    // Without the starter content.
    let other = t.0.join("other.ew");
    let o = t.ew(&[
        "import".as_ref(),
        "csv".as_ref(),
        tables.as_os_str(),
        "--out".as_ref(),
        other.as_os_str(),
    ]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            "error: `import csv` needs the starter content (`--content <dir>` or EW_CONTENT_DIR): the tables name its \
             symbols, function definitions and forms; nothing was written\n"
                .to_owned()
        )
    );
    assert!(!other.exists());
}

/// A row with a foreign colour code is reported as the library reports it, EW-IMP-002 under `--strict` (the
/// default: nothing is written, exit 1) and EW-IMP-008 under `--lenient` (written, exit 0); an unreadable
/// table exits 2 with its line, and its column where the reader names one, before anything is written.
#[test]
fn import_csv_strict_lenient_and_unreadable_tables() {
    let t = Tmp::new("ew-cli-test");
    let tables = tables_copy(&t, "tables");
    let conductors = tables.join("conductors.csv");
    let text = std::fs::read_to_string(&conductors).unwrap();
    let brown = text.replacen("-106W1:BN,-106W1,BN,BN,", "-106W1:BN,-106W1,BN,BR,", 1);
    assert_ne!(brown, text);
    std::fs::write(&conductors, brown).unwrap();

    let strict = t.0.join("strict.ew");
    let o = import_csv(&t, &tables, &strict, &[]);
    let lines = import_lines(&tables, ew_api::import::ImportMode::Strict, None);
    assert!(lines.contains("EW-IMP-002 error: conductors.csv:6: "), "{lines}");
    // The test tables' -106W1:BN connects nothing, so its row is the one error (the connections dropped with a
    // conductor, EW-IMP-003, are the importer's tests').
    let errors = lines
        .lines()
        .filter(|l| l.split(' ').nth(1) == Some("error:"))
        .count();
    assert_eq!(errors, 1, "{lines}");
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(1),
            lines,
            "error: the import made 1 error finding under `--strict` (the default); nothing was written\n"
                .to_owned()
        )
    );
    assert!(!strict.exists(), "nothing is written");
    let o = import_csv(&t, &tables, &strict, &["--strict"]);
    assert_eq!(o.status.code(), Some(1), "`--strict` is the default");

    let lenient = t.0.join("lenient.ew");
    let o = import_csv(&t, &tables, &lenient, &["--lenient"]);
    let library = ew_api::import::StarterLibrary::from_dir(std::path::Path::new(CONTENT)).unwrap();
    let batch = ew_api::import::import_tables(
        &ew_api::import::read_tables(&tables).unwrap(),
        &library,
        ew_api::import::ImportMode::Lenient,
    )
    .unwrap();
    let n = ew_api::import::read_tables(&tables).unwrap().len();
    let last = format!(
        "imported {n} tables into {}: {} records, {} library documents",
        lenient.display(),
        batch.records.len(),
        batch.lib.len()
    );
    let lines = import_lines(&tables, ew_api::import::ImportMode::Lenient, Some(&last));
    assert!(
        lines.contains("EW-IMP-008 warning: conductors.csv:6: "),
        "{lines}"
    );
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (Some(0), lines, String::new())
    );
    assert!(lenient.join("manifest.json").is_file());

    // Unreadable tables: a ragged row (its line) and a cell over the reader's limit (its line and column).
    let devices = tables.join("devices.csv");
    let original = std::fs::read_to_string(&devices).unwrap();
    let rows = original.lines().count();
    let fields = original.lines().next().unwrap().split(',').count();
    std::fs::write(&devices, format!("{original}a,b\n")).unwrap();
    let out = t.0.join("unreadable.ew");
    let o = import_csv(&t, &tables, &out, &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: devices.csv: line {}: expected {fields} fields, found 2; nothing was imported\n",
                rows + 1
            )
        )
    );
    let long = "x".repeat(40_000);
    std::fs::write(
        &devices,
        format!("{original}-K9,{long}{}\n", ",".repeat(fields - 2)),
    )
    .unwrap();
    let o = import_csv(&t, &tables, &out, &[]);
    assert_eq!(
        (o.status.code(), stdout(&o), stderr(&o)),
        (
            Some(2),
            String::new(),
            format!(
                "error: devices.csv: line {}, column 2: cell longer than 32768 bytes; nothing was imported\n",
                rows + 1
            )
        )
    );
    assert!(!out.exists(), "nothing is written");
}
