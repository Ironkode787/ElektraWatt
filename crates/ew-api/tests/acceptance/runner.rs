// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The acceptance runner: opens the golden project with the oracle on, evaluates the M1 criteria of doc 20 §21
//! (AC-01 … AC-14, AC-18, AC-20) and the defect regression rows of §19 that gate at M1, and returns the
//! [`Report`]. The checks live in `checks.rs`; this file builds their context.
//!
//! **What a check compares with.** Doc 20's expected outputs where they are committed files
//! (`expected/text`, `expected/png`, `expected/geometry`, `expected/pdf`, `expected/csv`), through the
//! functions that `tests/page_output.rs` and `tests/connectivity_reports.rs` use to write them; doc 20's own
//! tables where it states a value (§5 pages, §15 cross-references); the model through `Q`.
//!
//! **The oracle.** The sessions of the checks run with [`MemoMode::Oracle`]: memoised reads are also computed
//! cold and compared (every read in the gate and the deep nightly, a fixed sample in the per-gate tests:
//! [`Options::oracle`]), and the mismatches are in the report. The cross-reference scans (AC-05, D-01,
//! D-14, AC-18) run on a memoised session: the oracle recomputes all 135 cross-reference texts cold, which
//! takes about 25 s in a debug build.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ew_api::Session;
use ew_derive::Q;
use ew_dl::FontKey;
use ew_export::{Parallelism, PdfOutput, PdfPage, pdf};
use ew_import_tables::ew_interop_api::ImportBatch;
use ew_render_cpu::FontBytes;

use crate::golden_common;
use crate::golden_files::text_csv;
use crate::pdf_read::{PageRead, PdfFile};
use crate::pdf_support::{pdf_options, pdf_pages};
use crate::view::{GoldenPage, fonts, golden_pages, golden_session};

use super::checks;
use super::freshness;
use super::report::{Check, Outcome, Report, Status, fail};

/// Options of a run.
#[derive(Clone, Debug)]
pub struct Options {
    /// Compare the 150 dpi PNGs of the AC-02 folios (about a minute in a debug build). Off, the PNG entry is
    /// reported as not applicable.
    pub raster: bool,
    /// Measure and assert the AC-17 times (a milestone run: `cargo xtask acceptance --milestone`, which sets
    /// `EW_PERF_ASSERT=1`). Off, AC-17 is reported as not applicable and no clock is read.
    pub perf: bool,
    /// The memo oracle of the two oracle sessions (`oracle`, `profiled`) once they are open: `Oracle::Every` in the
    /// gate (`cargo xtask acceptance`) and the deep nightly, a fixed sample per gate in the ordinary tests (test
    /// audit g7). The mismatch count is in the report either way.
    pub oracle: ew_derive::Oracle,
    /// The veraPDF XML report that AC-13's veraPDF entry reads (the gate passes `EW_VERAPDF_REPORT` here; the
    /// headless run passes `None`, so the committed report never depends on the environment).
    pub verapdf: Option<std::path::PathBuf>,
}

/// Everything a check reads, built once per run.
pub struct Ctx {
    pub raster: bool,
    pub perf: bool,
    pub verapdf: Option<std::path::PathBuf>,
    pub batch: ImportBatch,
    /// Source key (`devices|-201U1`, `pages|3`, …) to record ID text.
    pub ids: BTreeMap<String, String>,
    /// The golden model, oracle on.
    pub oracle: Session,
    /// The golden model with the colour and size profiles of doc 20 §7.1, §7.2, oracle on.
    pub profiled: Session,
    /// The golden model, memoised without the oracle (the cross-reference scans).
    pub memoised: Session,
    /// The pages of `page_order` with their views.
    pub pages: Vec<GoldenPage>,
    /// The text CSV of every folio as the page output generates it.
    pub text: BTreeMap<String, String>,
    pub pdf_pages: Vec<PdfPage>,
    pub pdf: PdfOutput,
    /// The written PDF read back (text layer and stroked segments per page), or why it does not read.
    pub pdf_read: Result<Vec<PageRead>, String>,
    pub fonts: BTreeMap<FontKey, FontBytes>,
    /// The SQL projection of the profiled model with its pre-delivery check run (WP-S1.10), or why it did
    /// not build: [`Ctx::sql`], built by the first entry that reads it (about a minute in a debug build: the
    /// check runs on the oracle session).
    sql: std::cell::OnceCell<Result<ew_api::sql::Projection, String>>,
    /// The AC-16 run (`freshness.rs`), computed by the first entry that reads it.
    pub ac16: std::cell::OnceCell<Result<freshness::Run, String>>,
}

impl Ctx {
    fn new(options: Options) -> Ctx {
        let batch = golden_common::batch();
        let ids = golden_common::ids_by_key(&batch);
        let oracle = golden_common::session_oracle(&batch);
        let profiled = golden_common::session_oracle_with(&batch, |p, _| golden_common::with_profiles(p));
        for session in [&oracle, &profiled] {
            session
                .memo()
                .expect("an oracle session memoises")
                .set_oracle(options.oracle);
        }
        let memoised = golden_common::session(&batch);
        let view = golden_session(None);
        let q = view.query();
        let pages = golden_pages(&q);
        // The page text as the shipped export job draws it (the memoised layers of `ew_api::output`).
        let text = pages
            .iter()
            .map(|g| {
                let scene = ew_api::output::page_scene(&q, &g.entry);
                (g.entry.folio.clone(), text_csv(&scene))
            })
            .collect();
        let fonts = fonts();
        let pdf_pages = pdf_pages(&q, Parallelism::Serial);
        let pdf = pdf(&pdf_pages, &pdf_options(&q), &fonts, None).expect("the golden PDF is written");
        let pdf_read = PdfFile::parse(&pdf.bytes).and_then(|f| f.pages());
        Ctx {
            sql: std::cell::OnceCell::new(),
            ac16: std::cell::OnceCell::new(),
            raster: options.raster,
            perf: options.perf,
            verapdf: options.verapdf,
            batch,
            ids,
            oracle,
            profiled,
            memoised,
            pages,
            text,
            pdf_pages,
            pdf,
            pdf_read,
            fonts,
        }
    }

    /// The SQL projection of the profiled model with its pre-delivery check run, or why it did not build.
    pub fn sql(&self) -> &Result<ew_api::sql::Projection, String> {
        self.sql.get_or_init(|| {
            let checks = self
                .profiled
                .check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Release);
            self.profiled
                .sql_projection(Some(&checks))
                .map_err(|e| e.to_string())
        })
    }

    /// A `Q` without the report engine over the profiled model (the row tests' recipe).
    pub fn rows_q(&self) -> Q {
        golden_common::plain(&self.profiled)
    }
}

/// Collects the entries; a check that panics is a failed entry with the panic message as its reason.
///
/// A run evaluates every entry, or (a shard of the ordinary test, [`run_shard`]) the entries whose index is in a
/// selection, plus every [`Runner::entry_from`] row whose source entry it evaluated; the other rows are listed
/// by ID and name without being evaluated. The context is built by the first entry evaluated, so a run that
/// evaluates nothing builds nothing.
pub struct Runner {
    options: Options,
    ctx: std::cell::OnceCell<Ctx>,
    report: Report,
    /// The entry indexes this run evaluates; `None` is all.
    select: Option<Vec<std::ops::Range<usize>>>,
    /// Per row: whether it was evaluated.
    evaluated: Vec<bool>,
    /// Per row: the row an `entry_from` row copies, `None` for an ordinary entry.
    origins: Vec<Option<usize>>,
}

/// The rows that a selection evaluates, from the rows' origins (`None`: an ordinary entry, evaluated when its
/// index is selected; `Some(j)`: an `entry_from` row, evaluated with row `j`). The rule of [`Runner::entry`] and
/// [`Runner::entry_from`], for the test that the shards cover every row once.
pub fn evaluated_rows(origins: &[Option<usize>], select: &[std::ops::Range<usize>]) -> Vec<bool> {
    let mut out: Vec<bool> = Vec::with_capacity(origins.len());
    for (i, origin) in origins.iter().enumerate() {
        let e = match origin {
            None => select.iter().any(|r| r.contains(&i)),
            Some(j) => out[*j],
        };
        out.push(e);
    }
    out
}

impl Runner {
    fn new(options: Options, select: Option<Vec<std::ops::Range<usize>>>) -> Runner {
        Runner {
            options,
            ctx: std::cell::OnceCell::new(),
            report: Report::default(),
            select,
            evaluated: Vec::new(),
            origins: Vec::new(),
        }
    }

    fn ctx(&self) -> &Ctx {
        self.ctx.get_or_init(|| Ctx::new(self.options.clone()))
    }

    /// Whether this run measures the AC-17 times.
    pub fn perf(&self) -> bool {
        self.options.perf
    }

    /// Adds one entry: `check` runs now (when the run evaluates it), and a panic inside it becomes a failure.
    pub fn entry(&mut self, id: &'static str, name: &str, check: impl FnOnce(&Ctx) -> Outcome) {
        let index = self.report.checks.len();
        let selected = self
            .select
            .as_ref()
            .is_none_or(|s| s.iter().any(|r| r.contains(&index)));
        let (status, reason) = if !selected {
            not_evaluated()
        } else {
            match catch_unwind(AssertUnwindSafe(|| check(self.ctx()))) {
                Ok(outcome) => outcome,
                Err(payload) => {
                    let message = payload
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                        .unwrap_or_else(|| "panic".to_owned());
                    fail(format!("the check panicked: {message}"))
                }
            }
        };
        self.push(
            Check {
                id,
                name: name.to_owned(),
                status,
                reason,
            },
            selected,
            None,
        );
    }

    /// Adds one entry whose outcome is the status of an earlier entry of the same run (a regression row that
    /// the criterion's check covers): `via` names that entry by criterion and name. It is evaluated when that
    /// entry is.
    pub fn entry_from(&mut self, id: &'static str, name: &str, via: (&str, &str), why: &str) {
        let source = self
            .report
            .checks
            .iter()
            .position(|c| c.id == via.0 && c.name == via.1)
            .unwrap_or_else(|| panic!("no entry {} / {}", via.0, via.1));
        let found = self.report.checks[source].clone();
        let selected = self.evaluated[source];
        let (status, reason) = if selected {
            let reason = format!(
                "{why} Same check as {} \"{}\": {}",
                via.0, found.name, found.reason
            )
            .trim_start()
            .to_owned();
            (found.status, reason)
        } else {
            not_evaluated()
        };
        self.push(
            Check {
                id,
                name: name.to_owned(),
                status,
                reason,
            },
            selected,
            Some(source),
        );
    }

    fn push(&mut self, check: Check, evaluated: bool, origin: Option<usize>) {
        self.report.checks.push(check);
        self.evaluated.push(evaluated);
        self.origins.push(origin);
    }

    /// The report, with the oracle mismatches of the context's sessions if it was built.
    fn finish(self) -> Shard {
        let mut report = self.report;
        report.oracle_mismatches = self.ctx.get().map_or(0, |ctx| {
            [&ctx.oracle, &ctx.profiled]
                .iter()
                .map(|s| s.take_oracle_mismatches().len())
                .sum()
        });
        Shard {
            report,
            evaluated: self.evaluated,
            origins: self.origins,
        }
    }
}

/// The outcome of a row that the run does not evaluate (never compared with anything).
fn not_evaluated() -> Outcome {
    (Status::NotBuilt, "not evaluated in this shard".to_owned())
}

/// The result of [`run_shard`].
pub struct Shard {
    /// Every row; the rows not evaluated are placeholders (`not_built`, never compared).
    pub report: Report,
    /// Per row: whether this run evaluated it.
    pub evaluated: Vec<bool>,
    /// Per row: the row an `entry_from` row copies.
    pub origins: Vec<Option<usize>>,
}

/// Runs the acceptance checks over the golden fixture and returns the report.
pub fn run(options: Options) -> Report {
    let mut runner = Runner::new(options, None);
    // The SQL projection first, as before the shards existed: the profiled session's pre-delivery check runs
    // before every entry, so a full run reads that session in the same order.
    let _ = runner.ctx().sql();
    checks::all(&mut runner);
    runner.finish().report
}

/// Runs the entries with an index in `select` (and the `entry_from` rows of those entries) and lists the
/// others; see [`Runner`]. An empty selection evaluates nothing and builds no context.
pub fn run_shard(options: Options, select: &[std::ops::Range<usize>]) -> Shard {
    let mut runner = Runner::new(options, Some(select.to_vec()));
    checks::all(&mut runner);
    runner.finish()
}
