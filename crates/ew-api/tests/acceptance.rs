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
//!   fails when any entry fails (exit status non-zero).
//! - The ordinary tests pin the report (counts, determinism) so that a change of the model or of a golden
//!   moves them, and keep `expected/csv/` equal to the derived rows.
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

/// The pass, fail and not-applicable counts that the golden fixture gives today. A model or golden change
/// that moves a count updates this line together with the README of the golden project.
const EXPECTED: (usize, usize, usize) = (31, 12, 18);

#[test]
fn the_golden_report_has_the_counts_of_the_fixture() {
    let report = run(Options { raster: true });
    if let Some(path) = std::env::var_os("EW_ACCEPTANCE_REPORT") {
        std::fs::write(path, report.markdown()).expect("the report path is writable");
    }
    assert_eq!(report.oracle_mismatches, 0, "the oracle found memo mismatches");
    let counts = (
        report.count(Status::Pass),
        report.count(Status::Fail),
        report.count(Status::NotApplicable),
    );
    assert_eq!(counts, EXPECTED, "\n{}", report.markdown());
}

#[test]
fn the_report_is_deterministic() {
    // Without the raster comparison (about a minute): the rest of the report is what can differ.
    let a = run(Options { raster: false });
    let b = run(Options { raster: false });
    assert_eq!(a.markdown(), b.markdown());
    assert_eq!(a, b);
}

/// The gate itself, run by `cargo xtask acceptance <report.md>`: writes the report to `EW_ACCEPTANCE_REPORT`
/// and fails when an entry fails or the oracle finds a mismatch.
#[test]
#[ignore = "run by `cargo xtask acceptance`"]
fn gate() {
    let report = run(Options { raster: true });
    let path = std::env::var_os("EW_ACCEPTANCE_REPORT").expect("EW_ACCEPTANCE_REPORT names the report file");
    std::fs::write(path, report.markdown()).expect("the report path is writable");
    let failed: Vec<String> = report
        .failures()
        .map(|c| format!("{} {}", c.id, c.name))
        .collect();
    assert!(
        report.green(),
        "{} failed entries, {} oracle mismatches:\n{}",
        failed.len(),
        report.oracle_mismatches,
        failed.join("\n")
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
        ],
        oracle_mismatches: 0,
    };
    let text = report.markdown();
    assert!(text.contains("2 entries: 0 pass, 1 fail, 1 not applicable."));
    assert!(text.contains("| AC-99 | a \\| b | FAIL | two lines |"));
    assert!(text.contains("| AC-98 | c | n/a | later |"));
    assert!(!report.green());
}
