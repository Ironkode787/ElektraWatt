// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The acceptance report: one entry per named check, with its status and a one-line reason, rendered as a
//! Markdown table. The text depends on the checks only (no clock, no path, no host), so two runs over the
//! same fixture print the same report.

use std::fmt::Write as _;

/// The outcome of one check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    /// The criterion holds.
    Pass,
    /// The criterion does not hold; the reason says what differs.
    Fail,
    /// The criterion cannot be evaluated here (needs the GUI, the pilot, a BM runner, a feature that does not
    /// exist yet); the reason says what is missing. Never a silent skip.
    NotApplicable,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "FAIL",
            Status::NotApplicable => "n/a",
        }
    }
}

/// One named check of the report.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Check {
    /// The criterion: `AC-01` … `AC-13`, `D-01` … (doc 20 §21, §19).
    pub id: &'static str,
    /// What this entry checks (a criterion has several entries when it has several parts).
    pub name: String,
    /// The outcome.
    pub status: Status,
    /// One line: why.
    pub reason: String,
}

/// The result of one check function: a status and its reason.
pub type Outcome = (Status, String);

/// A passed check.
pub fn pass(reason: impl Into<String>) -> Outcome {
    (Status::Pass, reason.into())
}

/// A failed check.
pub fn fail(reason: impl Into<String>) -> Outcome {
    (Status::Fail, reason.into())
}

/// A check that cannot be evaluated: `not applicable at M1 headless: <reason>`.
pub fn na(reason: impl Into<String>) -> Outcome {
    (Status::NotApplicable, reason.into())
}

/// The report of one run.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Report {
    /// The entries in the order the runner defines them.
    pub checks: Vec<Check>,
    /// Memoised reads that the oracle found to differ from a cold run (a memo-engine bug); none expected.
    pub oracle_mismatches: usize,
}

impl Report {
    /// The number of entries with a status.
    pub fn count(&self, status: Status) -> usize {
        self.checks.iter().filter(|c| c.status == status).count()
    }

    /// The failed entries.
    pub fn failures(&self) -> impl Iterator<Item = &Check> + '_ {
        self.checks.iter().filter(|c| c.status == Status::Fail)
    }

    /// Whether the run passes the gate: no failed entry and no oracle mismatch.
    pub fn green(&self) -> bool {
        self.count(Status::Fail) == 0 && self.oracle_mismatches == 0
    }

    /// The report as Markdown: a summary line and a table with one row per entry.
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Acceptance report: golden project J-E-2416, gate M1 (headless)\n\n");
        let _ = writeln!(
            out,
            "{} entries: {} pass, {} fail, {} not applicable. Oracle mismatches: {}.\n",
            self.checks.len(),
            self.count(Status::Pass),
            self.count(Status::Fail),
            self.count(Status::NotApplicable),
            self.oracle_mismatches
        );
        out.push_str("| Criterion | Check | Status | Reason |\n|---|---|---|---|\n");
        for c in &self.checks {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                c.id,
                cell(&c.name),
                c.status.label(),
                cell(&c.reason)
            );
        }
        out
    }
}

/// A table cell: one line, `|` escaped.
fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}
