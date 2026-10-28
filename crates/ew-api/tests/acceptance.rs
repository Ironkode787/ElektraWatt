// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The acceptance runner (WP-S6.01, WP-S7.01; doc 20 §21, §22; doc 18 §3.10) for gate M1, headless.
//!
//! It opens the golden project with the oracle on and evaluates AC-01 … AC-14, AC-18, AC-20 and the §19 rows
//! gated M1 (`acceptance/checks.rs`), each as a named entry that passes, fails or is not applicable with a
//! one-line reason (`acceptance/report.rs`). The comparisons with committed files reuse the functions of
//! `tests/page_output.rs` and `tests/connectivity_reports.rs` (text CSV, frame geometry, PNG, PDF structure and
//! SHA-256, report rows); they live in `tests/page_output/` and `tests/golden_common/` and are included here.
//!
//! The page output is the product's since cycle 15 (FT-59): the views, the per-layer page display lists and the
//! PDF and SVG export are the session's export job (`ew_api::output`), which AC-13, AC-14, AC-15 and AC-17 run.
//! The runner stays in this test crate for its shared helpers (`tests/page_output/`, `tests/golden_common/`); it
//! moves to `ew-testkit` (`ew-testkit accept --gate`) with the golden builders (WP-S6.01).
//!
//! - `cargo xtask acceptance <report.md>` runs the gate: the `#[ignore]`d test `gate` writes the report and
//!   fails when any entry fails or is not built (exit status non-zero).
//! - The ordinary tests run the runner without the raster comparison and the measurements, in the three shards of
//!   [`HEADLESS_SHARDS`] that nextest runs in parallel (cycle 32), and compare each shard's rows with the
//!   committed `expected/acceptance_headless.md`; a fourth test shows that the shards evaluate every row of the
//!   report once and that the committed summary line follows from the committed rows. They keep `expected/csv/`
//!   equal to the derived rows.
//!   `EW_BLESS=1 cargo nextest run -p ew-api --test acceptance -E 'test(expected_csv)'` rewrites `expected/csv/`.

// The page-output helpers are shared with `tests/page_output.rs`; not every function is used here.
#![allow(dead_code)]
// Test target (the acceptance runner): bless, report-path and EW_PERF_ASSERT flags, a scratch-directory name from the clock and the open and export timers; the pass or fail of an entry depends on none of them except the opt-in timing bound (doc 18 §3.16).
#![allow(clippy::disallowed_methods)]

mod golden_common;
#[path = "page_output/golden_files.rs"]
mod golden_files;
#[path = "page_output/pdf_read.rs"]
mod pdf_read;
#[path = "page_output/pdf_support.rs"]
mod pdf_support;
#[path = "page_output/view.rs"]
mod view;

#[path = "acceptance/checks.rs"]
mod checks;
#[path = "acceptance/freshness.rs"]
mod freshness;
#[path = "acceptance/report.rs"]
mod report;
#[path = "acceptance/runner.rs"]
mod runner;

use report::Status;
use runner::{Options, evaluated_rows, run, run_shard};

/// The (pass, fail, not_built, not_applicable) counts of the headless report; updated with the committed report.
const EXPECTED: (usize, usize, usize, usize) = (88, 0, 0, 6);

/// The committed report of the headless run without the raster comparison (`expected/acceptance_headless.md`):
/// the ordinary tests run the runner and compare its rows with this file, which pins the statuses and reasons
/// and, by being a file, shows that two runs print the same text. A change of the model or of a golden that
/// moves an entry rewrites it: `EW_BLESS=1 cargo nextest run -p ew-api --test acceptance -E
/// 'test(headless_report)'` (the first shard runs every entry and writes the whole file; the others then do
/// nothing).
fn committed_report() -> std::path::PathBuf {
    golden_common::repo().join("golden/J-E-2416/expected/acceptance_headless.md")
}

/// The ordinary run's options: no raster comparison (the 150 dpi PNGs are compared by `tests/page_output.rs`, and
/// by the full run of `cargo xtask acceptance`), no performance measurements (milestone runs only), and no
/// veraPDF report (the headless run never reads EW_VERAPDF_REPORT: the committed report does not depend on the
/// environment).
fn headless() -> Options {
    Options {
        raster: false,
        perf: false,
        verapdf: None,
    }
}

/// The shards of the headless report: the entry indexes each one evaluates (an `entry_from` row goes with the
/// entry it copies). The SQL pairs (AC-08 to AC-11) and AC-15's SQL paths share the SQL projection, whose
/// pre-delivery check on the oracle session takes about a minute in a debug build, so they are one shard; the
/// criteria AC-01 to AC-13 another; AC-18, AC-20, AC-14, the rest of AC-15, AC-16, AC-17, AC-39 and the §19
/// rows the third (open-ended, so that an entry added at the end is evaluated). Every row is evaluated by
/// exactly one shard (`the_headless_report_shards_cover_the_committed_report`); moving an entry only moves the
/// work between shards.
#[allow(clippy::single_range_in_vec_init)] // a shard is a list of index ranges; the first has one
const HEADLESS_SHARDS: [&[std::ops::Range<usize>]; 3] =
    [&[0..31], &[31..35, 45..46], &[35..45, 46..usize::MAX]];

/// One shard of the ordinary test: its rows equal the committed report's rows, and the oracle sessions found no
/// mismatch.
fn headless_report_shard(shard: usize) {
    if std::env::var_os("EW_BLESS").is_some_and(|v| v == "1") {
        if shard == 0 {
            let report = run(headless());
            assert_eq!(report.oracle_mismatches, 0, "the oracle found memo mismatches");
            std::fs::write(committed_report(), report.markdown().as_bytes()).unwrap();
        }
        return;
    }
    let run = run_shard(headless(), HEADLESS_SHARDS[shard]);
    assert_eq!(
        run.report.oracle_mismatches, 0,
        "the oracle found memo mismatches"
    );
    let path = committed_report();
    let committed = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (EW_BLESS=1 generates it)", path.display()));
    let (_, rows) = split_report(&committed);
    assert_eq!(
        rows.len(),
        run.report.checks.len(),
        "the runner has {} entries, {} has {} rows (EW_BLESS=1 rewrites it)",
        run.report.checks.len(),
        path.display(),
        rows.len()
    );
    let mut evaluated = 0;
    let mut differing = Vec::new();
    for (i, check) in run.report.checks.iter().enumerate() {
        if !run.evaluated[i] {
            continue;
        }
        evaluated += 1;
        let row = check.row();
        if row != rows[i] {
            differing.push(format!("row {i}:\n  now:       {row}\n  committed: {}", rows[i]));
        }
    }
    assert!(evaluated > 0, "shard {shard} evaluates no row");
    assert!(
        differing.is_empty(),
        "shard {shard}: {} row(s) differ from {} (EW_BLESS=1 rewrites it):\n{}",
        differing.len(),
        path.display(),
        differing.join("\n")
    );
}

/// The committed report split into the lines before the entry rows (with their line ends) and the rows.
fn split_report(text: &str) -> (&str, Vec<&str>) {
    const TABLE: &str = "|---|---|---|---|\n";
    let at = text.find(TABLE).expect("the report has a table") + TABLE.len();
    let rows = text[at..].lines().collect();
    (&text[..at], rows)
}

#[test]
fn the_headless_report_equals_the_committed_report_shard_1_of_3() {
    headless_report_shard(0);
}

#[test]
fn the_headless_report_equals_the_committed_report_shard_2_of_3() {
    headless_report_shard(1);
}

#[test]
fn the_headless_report_equals_the_committed_report_shard_3_of_3() {
    headless_report_shard(2);
}

/// The shards together are the former single test: the runner's rows (listed without evaluating anything) are
/// the committed rows by criterion and check, in order; every row is evaluated by exactly one shard; and the
/// committed title, summary line and table header are the ones the runner prints for the committed rows' statuses
/// with no oracle mismatch (each shard asserts its own mismatches are none), with the expected counts.
#[test]
fn the_headless_report_shards_cover_the_committed_report() {
    let listed = run_shard(headless(), &[]);
    assert!(
        listed.evaluated.iter().all(|e| !e),
        "an empty selection evaluates nothing"
    );
    let committed = std::fs::read_to_string(committed_report()).unwrap();
    let (head, rows) = split_report(&committed);
    assert_eq!(
        rows.len(),
        listed.report.checks.len(),
        "rows of the committed report"
    );
    let mut counts = [0usize; 4];
    for (check, row) in listed.report.checks.iter().zip(&rows) {
        let start = check.row_start();
        let rest = row
            .strip_prefix(&start)
            .unwrap_or_else(|| panic!("the committed row `{row}` is not the entry `{start}`"));
        let label = rest.split(" | ").next().unwrap_or_default();
        let status = [
            Status::Pass,
            Status::Fail,
            Status::NotBuilt,
            Status::NotApplicable,
        ]
        .into_iter()
        .position(|s| s.label() == label)
        .unwrap_or_else(|| panic!("`{label}` is not a status: {row}"));
        counts[status] += 1;
    }
    assert_eq!(
        (counts[0], counts[1], counts[2], counts[3]),
        EXPECTED,
        "pass, fail, not_built, not_applicable"
    );
    assert_eq!(
        head,
        report::header(rows.len(), counts, 0),
        "the committed summary follows from its rows"
    );
    assert!(
        committed.ends_with('\n')
            && committed[head.len()..] == rows.iter().map(|r| format!("{r}\n")).collect::<String>()
    );

    let mut times = vec![0usize; rows.len()];
    for select in HEADLESS_SHARDS {
        for (i, e) in evaluated_rows(&listed.origins, select).into_iter().enumerate() {
            times[i] += usize::from(e);
        }
    }
    let wrong: Vec<(usize, usize)> = times
        .iter()
        .enumerate()
        .filter(|(_, n)| **n != 1)
        .map(|(i, n)| (i, *n))
        .collect();
    assert!(
        wrong.is_empty(),
        "(row, shards evaluating it) not once: {wrong:?}"
    );
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
        verapdf: std::env::var_os(checks::VERAPDF_REPORT_ENV)
            .filter(|v| !v.is_empty())
            .map(std::path::PathBuf::from),
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
            // the rows mirror of a block that no other test blesses (the connectivity blocks are written by
            // `connectivity_reports`; the texts are equal by this test's assertion)
            let mirror = dir.join("rows").join(format!("{block}.csv"));
            std::fs::write(&mirror, csv.as_bytes()).unwrap();
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

/// The AC-01 vector comparison sees a frame line moved by more than 0.2 mm, a wrong line width and an extra
/// line in the header band; it accepts the exact frame, a line moved by 0.15 mm and a table border along the
/// drawing area's edge.
#[test]
fn the_pdf_frame_comparison_detects_moved_thin_and_extra_lines() {
    use pdf_read::{PageRead, Segment};
    let frame = checks::frame_figures_json().expect("frame.json");
    let fig = &frame["ew.frame.qet-a4-landscape-grid"];
    let width = fig["line_width"].as_f64().unwrap();
    let lines = checks::frame_lines(fig, true).expect("the figures");
    let page = |segments: Vec<Segment>| PageRead {
        width_mm: 297.0,
        height_mm: 210.0,
        runs: Vec::new(),
        segments,
    };
    let exact: Vec<Segment> = lines.iter().map(|&(a, b)| Segment { a, b, width }).collect();
    let (wrong, checked, worst) = checks::frame_mismatches("t", &page(exact.clone()), fig, true);
    assert!(wrong.is_empty(), "{wrong:?}");
    assert_eq!((checked, worst), (lines.len(), 0.0));
    let nudge = |d: f64| {
        let mut s = exact.clone();
        s[0].a.1 += d;
        s[0].b.1 += d;
        s
    };
    assert!(
        checks::frame_mismatches("t", &page(nudge(0.15)), fig, true)
            .0
            .is_empty()
    );
    let (wrong, _, _) = checks::frame_mismatches("t", &page(nudge(0.3)), fig, true);
    assert!(wrong.iter().any(|w| w.contains("mm off")), "{wrong:?}");
    let mut thin = exact.clone();
    thin[1].width = width / 2.0;
    assert!(
        checks::frame_mismatches("t", &page(thin), fig, true)
            .0
            .iter()
            .any(|w| w.contains("wide"))
    );
    let mut extra = exact.clone();
    extra.push(Segment {
        a: (20.0, 11.0),
        b: (40.0, 11.0),
        width,
    });
    assert!(
        checks::frame_mismatches("t", &page(extra), fig, true)
            .0
            .iter()
            .any(|w| w.contains("extra"))
    );
    let mut table = exact;
    table.push(Segment {
        a: (16.31, 15.01),
        b: (277.27, 15.01),
        width,
    });
    assert!(
        checks::frame_mismatches("t", &page(table), fig, true)
            .0
            .is_empty()
    );
}

/// AC-13's comparison sees a lost space, an unmapped space (U+FFFD) and a missing run.
#[test]
fn the_text_layer_comparison_sees_a_lost_or_unmapped_space() {
    use pdf_read::TextRun;
    let printed = vec!["SK 3110.000".to_owned(), "-101F1".to_owned()];
    let run = |t: &str| TextRun {
        text: t.to_owned(),
        origin: (0.0, 0.0),
    };
    let check = |read: &[TextRun]| checks::text_layer_mismatches("101", &printed, read);
    assert!(check(&[run("SK 3110.000"), run("-101F1")]).is_empty());
    assert_eq!(check(&[run("SK3110.000"), run("-101F1")]).len(), 1);
    assert_eq!(check(&[run("SK\u{fffd}3110.000"), run("-101F1")]).len(), 1);
    assert_eq!(check(&[run("SK 3110.000")]).len(), 1);
}

/// AC-06's comparison counts tags on token boundaries and sees a changed count and a stale token.
#[test]
fn the_tag_comparison_counts_on_token_boundaries_and_sees_stale_tokens() {
    use pdf_read::TextRun;
    assert_eq!(checks::count_tag("-K10 -K1, (-K1)", "-K1"), 2);
    assert_eq!(checks::count_tag("-201U3-K1", "-K1"), 0);
    assert_eq!(checks::count_tag("-201U3-K1", "-201U3"), 0);
    assert_eq!(checks::count_tag("∞ -201U3-K1", "-201U3-K1"), 1);
    let run = |t: &str| TextRun {
        text: t.to_owned(),
        origin: (0.0, 0.0),
    };
    let tags: std::collections::BTreeSet<String> = ["-K1".to_owned(), "-K10".to_owned()].into();
    let printed = vec!["-K1 -K10".to_owned()];
    let (wrong, n) = checks::tag_mismatches("101", &tags, &printed, &[run("-K1 -K10")]);
    assert!(wrong.is_empty(), "{wrong:?}");
    assert_eq!(n, 2);
    let (wrong, _) = checks::tag_mismatches("101", &tags, &printed, &[run("-K10 -K10")]);
    assert_eq!(wrong.len(), 2, "{wrong:?}");
    let (wrong, _) = checks::tag_mismatches("101", &tags, &[], &[run("-4W1")]);
    assert!(wrong.iter().any(|w| w.contains("stale")), "{wrong:?}");
}

/// AC-14's comparisons: two failed exports are not equal, and a link to a missing file or id dangles.
#[test]
fn the_svg_comparison_needs_written_exports_and_resolving_links() {
    use ew_export::{SvgError, SvgFile, SvgOutput};
    let err: Result<SvgOutput, SvgError> = Err(SvgError::NoPages);
    assert!(checks::same_svg(&err, &err).is_err());
    let file = |name: &str, text: &str| SvgFile {
        name: name.to_owned(),
        text: text.to_owned(),
    };
    let mut out = SvgOutput {
        files: vec![
            file(
                "1.svg",
                "<svg id=\"ew-p-1\"><a href=\"2.svg#ew-r-9\"></a><a href=\"2.svg\"></a></svg>",
            ),
            file("2.svg", "<svg id=\"ew-p-2\"><path id=\"ew-r-9\"/></svg>"),
        ],
        report: Default::default(),
    };
    let ok = Ok(out.clone());
    assert!(checks::same_svg(&ok, &ok).is_ok());
    assert!(checks::svg_dangling(&out).is_empty());
    out.files[1].text = out.files[1].text.replace(" id=\"ew-r-9\"", "");
    out.files[0].text.push_str("<a href=\"3.svg\"></a>");
    assert_eq!(checks::svg_dangling(&out).len(), 2);
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

/// A veraPDF XML report (the shape of `verapdf --format xml`) of one job per `(profile, isCompliant)` pair, with
/// a batch summary and the attributes of its `validationReports` count.
fn verapdf_report(files: &[(&str, &str)], counts: &str) -> String {
    let jobs: String = files
        .iter()
        .map(|(profile, compliant)| {
            format!(
                "<job><item size=\"1\"><name>/data/a.pdf</name></item><validationReport jobEndStatus=\"normal\" profileName=\"{profile}\" statement=\"s\" isCompliant=\"{compliant}\"><details passedRules=\"1\" failedRules=\"0\"/></validationReport></job>"
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?><report><jobs>{jobs}</jobs><batchSummary totalJobs=\"{n}\" failedToParse=\"0\" encrypted=\"0\" outOfMemory=\"0\" veraExceptions=\"0\"><validationReports {counts}>{n}</validationReports></batchSummary></report>",
        n = files.len()
    )
}

/// The AC-13 veraPDF entry reads a veraPDF report by the rules of `ci/verapdf-check.py`: compliant PDF/A-2B files
/// only, no file that failed to parse, counts that agree with the jobs.
#[test]
fn the_verapdf_entry_reads_the_report_of_the_job() {
    let good = ("PDF/A-2B validation profile", "true");
    let ok = verapdf_report(
        &[good, good, good],
        "compliant=\"3\" nonCompliant=\"0\" failedJobs=\"0\"",
    );
    let (status, reason) = checks::verapdf_outcome(&ok);
    assert_eq!(status, Status::Pass, "{reason}");
    assert!(reason.contains("3 golden PDF export(s)"), "{reason}");

    let counts_ok = "compliant=\"1\" nonCompliant=\"0\" failedJobs=\"0\"";
    let one = verapdf_report(&[good], counts_ok);
    let bad = [
        // a file that is not compliant, with the counts of such a report
        verapdf_report(
            &[("PDF/A-2B validation profile", "false")],
            "compliant=\"0\" nonCompliant=\"1\" failedJobs=\"0\"",
        ),
        // the wrong profile
        verapdf_report(&[("PDF/A-1B validation profile", "true")], counts_ok),
        // counts that do not match the jobs
        verapdf_report(&[good], "compliant=\"0\" nonCompliant=\"0\" failedJobs=\"1\""),
        // a file that did not parse, was encrypted (the summary says so)
        one.replace("failedToParse=\"0\"", "failedToParse=\"1\""),
        one.replace("encrypted=\"0\"", "encrypted=\"1\""),
        // no job, no summary, no XML
        verapdf_report(&[], "compliant=\"0\" nonCompliant=\"0\" failedJobs=\"0\""),
        "<report><jobs><job><validationReport profileName=\"PDF/A-2B validation profile\" isCompliant=\"true\"/></job></jobs></report>"
            .to_owned(),
        String::new(),
    ];
    for report in &bad {
        let (status, reason) = checks::verapdf_outcome(report);
        assert_eq!(status, Status::Fail, "{report}: {reason}");
        assert!(reason.starts_with("veraPDF cross-check failed: "), "{reason}");
    }
    // `<jobs>` is not a job: an empty `<jobs>` element is a report without a job.
    let (_, reason) = checks::verapdf_outcome(&bad[5]);
    assert!(reason.contains("no job"), "{reason}");
}

/// Without a report path the entry is not applicable and its reason names the job that runs the tool, the text
/// that the committed headless report holds (the headless run passes no path whatever the environment says); with
/// a path it reads the file, and an unreadable one fails.
#[test]
fn the_verapdf_entry_names_the_job_without_a_report() {
    let (status, reason) = checks::verapdf_entry(None);
    assert_eq!(status, Status::NotApplicable);
    let committed = std::fs::read_to_string(committed_report()).unwrap();
    assert!(
        committed.contains(&format!(
            "| AC-13 | veraPDF cross-check of PDF/A-2b | not_applicable | {reason} |"
        )),
        "the committed report holds the entry's text: {reason}"
    );
    assert!(reason.contains(".github/workflows/ci.yml"), "{reason}");
    let (status, reason) = checks::verapdf_entry(Some(std::path::Path::new("no/such/verapdf-report.xml")));
    assert_eq!(status, Status::Fail, "{reason}");
    assert!(reason.contains("cannot be read"), "{reason}");
}
