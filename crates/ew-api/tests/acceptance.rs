// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The acceptance runner (WP-S6.01, WP-S7.01; doc 20 §21, §22; doc 18 §3.10) for gate M1, headless.
//!
//! It opens the golden project with the oracle on and evaluates AC-01 … AC-13, AC-18, AC-20 and the §19 rows
//! gated M1 (`acceptance/checks.rs`), each as a named entry that passes, fails or is not applicable with a
//! one-line reason (`acceptance/report.rs`). The comparisons with committed files reuse the functions of
//! `tests/page_output.rs` and `tests/connectivity_reports.rs` (text CSV, frame geometry, PNG, PDF structure and
//! SHA-256, report rows); they live in `tests/page_output/` and `tests/golden_common/` and are included here.
//!
//! The runner lives in this test crate because the page output pipeline (display lists, PNG, PDF) is test
//! code in v0: `ew-scene` reads derived data through a caller-filled view that only the session layer fills
//! (`tests/page_output/view.rs`). It moves to `ew-testkit` (`ew-testkit accept --gate`) with that view.
//!
//! - `cargo xtask acceptance <report.md>` runs the gate: the `#[ignore]`d test `gate` writes the report and
//!   fails when any entry fails or is not built (exit status non-zero).
//! - The ordinary tests run the runner once, without the raster comparison and the measurements, and compare
//!   its report with the committed `expected/acceptance_headless.md`; they keep `expected/csv/` equal to the
//!   derived rows.
//!   `EW_BLESS=1 cargo nextest run -p ew-api --test acceptance -E 'test(expected_csv)'` rewrites `expected/csv/`.

// The page-output helpers are shared with `tests/page_output.rs`; not every function is used here.
#![allow(dead_code)]

mod golden_common;
#[path = "page_output/golden_files.rs"]
mod golden_files;
#[path = "page_output/pdf_support.rs"]
mod pdf_support;
#[path = "page_output/view.rs"]
mod view;

#[path = "acceptance/checks.rs"]
mod checks;
#[path = "acceptance/report.rs"]
mod report;
#[path = "acceptance/runner.rs"]
mod runner;

use report::Status;
use runner::{Options, run};

/// The (pass, fail, not_built, not_applicable) counts of the headless report; updated with the committed report.
const EXPECTED: (usize, usize, usize, usize) = (37, 12, 26, 6);

/// The committed report of the headless run without the raster comparison (`expected/acceptance_headless.md`):
/// the ordinary test runs the runner once and compares its report with this file, which pins the statuses and
/// reasons and, by being a file, shows that two runs print the same text. A change of the model or of a golden
/// that moves an entry rewrites it: `EW_BLESS=1 cargo nextest run -p ew-api --test acceptance -E
/// 'test(headless_report)'`.
fn committed_report() -> std::path::PathBuf {
    golden_common::repo().join("golden/J-E-2416/expected/acceptance_headless.md")
}

/// The ordinary test: one run without the raster comparison (the 150 dpi PNGs are compared by
/// `tests/page_output.rs`, and by the full run of `cargo xtask acceptance`) and without the performance
/// measurements (milestone runs only).
#[test]
fn the_headless_report_equals_the_committed_report() {
    let report = run(Options {
        raster: false,
        perf: false,
    });
    assert_eq!(report.oracle_mismatches, 0, "the oracle found memo mismatches");
    let text = report.markdown();
    let path = committed_report();
    if std::env::var_os("EW_BLESS").is_some_and(|v| v == "1") {
        std::fs::write(&path, text.as_bytes()).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 generates it)", path.display()));
    assert!(
        committed == text,
        "the report differs from {} (EW_BLESS=1 rewrites it):\n{text}",
        path.display()
    );
    let counts = (
        report.count(Status::Pass),
        report.count(Status::Fail),
        report.count(Status::NotBuilt),
        report.count(Status::NotApplicable),
    );
    assert_eq!(counts, EXPECTED, "pass, fail, not_built, not_applicable");
}

/// The gate itself, run by `cargo xtask acceptance`: the full run with the raster comparison, writing the
/// report to `EW_ACCEPTANCE_REPORT`; it fails when an entry fails or is not built, or the oracle finds a
/// mismatch. `EW_PERF_ASSERT=1` adds the AC-17 measurements.
#[test]
#[ignore = "run by `cargo xtask acceptance`"]
fn gate() {
    let report = run(Options {
        raster: true,
        perf: std::env::var_os("EW_PERF_ASSERT").is_some(),
    });
    let path = std::env::var_os("EW_ACCEPTANCE_REPORT").expect("EW_ACCEPTANCE_REPORT names the report file");
    std::fs::write(path, report.markdown()).expect("the report path is writable");
    let blocking: Vec<String> = report
        .blocking()
        .map(|c| format!("{} {} {}", c.id, c.status_name(), c.name))
        .collect();
    assert!(
        report.green(),
        "{} blocking entries (fail or not_built), {} oracle mismatches:\n{}",
        blocking.len(),
        report.oracle_mismatches,
        blocking.join("\n")
    );
}

/// `expected/csv/<name>.csv` is the derived rows of the report block, and equals `expected/rows/<folio>.csv`.
#[test]
fn expected_csv_is_the_derived_rows_and_mirrors_expected_rows() {
    let batch = golden_common::batch();
    let session = golden_common::session_with_profiles(&batch);
    let q = golden_common::plain(&session);
    let dir = golden_common::repo().join("golden/J-E-2416/expected");
    let bless = std::env::var_os("EW_BLESS").is_some_and(|v| v == "1");
    for (name, block) in checks::ROW_PAIRS {
        let csv = checks::row_pair_text(&batch, &q, name).expect("the rows derive");
        let path = dir.join("csv").join(format!("{name}.csv"));
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, csv.as_bytes()).unwrap();
            continue;
        }
        let committed = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 generates it)", path.display()));
        assert!(
            committed == csv.as_bytes(),
            "{} differs from the derived rows (EW_BLESS=1 rewrites it)",
            path.display()
        );
        let rows = std::fs::read(dir.join("rows").join(format!("{block}.csv"))).unwrap();
        assert!(committed == rows, "{name}.csv differs from rows/{block}.csv");
    }
}

#[test]
fn the_markdown_has_one_line_per_entry_and_escapes_pipes() {
    use report::{Check, Report};
    let report = Report {
        checks: vec![
            Check {
                id: "AC-99",
                name: "a | b".into(),
                status: Status::Fail,
                reason: "two\nlines".into(),
            },
            Check {
                id: "AC-98",
                name: "c".into(),
                status: Status::NotApplicable,
                reason: "later".into(),
            },
            Check {
                id: "AC-97",
                name: "d".into(),
                status: Status::NotBuilt,
                reason: "missing".into(),
            },
        ],
        oracle_mismatches: 0,
    };
    let text = report.markdown();
    assert!(text.contains("3 entries: 0 pass, 1 fail, 1 not_built, 1 not_applicable."));
    assert!(text.contains("| AC-99 | a \\| b | fail | two lines |"));
    assert!(text.contains("| AC-98 | c | not_applicable | later |"));
    assert!(text.contains("| AC-97 | d | not_built | missing |"));
    assert!(!report.green());
    assert_eq!(
        report.blocking().count(),
        2,
        "fail and not_built block, not_applicable does not"
    );
}
