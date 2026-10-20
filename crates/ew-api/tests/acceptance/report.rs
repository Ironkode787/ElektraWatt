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
    /// The criterion does not hold; the reason says what differs. Blocks the gate.
    Fail,
    /// The feature the criterion needs does not exist yet; the reason names it. Blocks the gate exactly like
    /// a failure and is listed separately, so progress stays visible.
    NotBuilt,
    /// The criterion cannot be evaluated headless or at this delivery (the GUI, the pilot, an external tool
    /// that is not installed, a milestone-only measurement). Does not block; the reason is required.
    NotApplicable,
}

impl Status {
    /// The name in the report.
    pub fn label(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "fail",
            Status::NotBuilt => "not_built",
            Status::NotApplicable => "not_applicable",
        }
    }

    /// Whether the status blocks the gate.
    pub fn blocks(self) -> bool {
        matches!(self, Status::Fail | Status::NotBuilt)
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

impl Check {
    /// The status as the report prints it.
    pub fn status_name(&self) -> &'static str {
        self.status.label()
    }
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

/// A check whose feature does not exist yet; blocks the gate like a failure.
pub fn not_built(reason: impl Into<String>) -> Outcome {
    (Status::NotBuilt, reason.into())
}

/// A check that cannot be evaluated here (GUI, pilot, external tool, milestone-only measurement).
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

    /// The entries that block the gate: failed and not built.
    pub fn blocking(&self) -> impl Iterator<Item = &Check> + '_ {
        self.checks.iter().filter(|c| c.status.blocks())
    }

    /// Whether the run passes the gate: no failed and no not-built entry, no oracle mismatch.
    pub fn green(&self) -> bool {
        self.blocking().next().is_none() && self.oracle_mismatches == 0
    }

    /// The report as Markdown: a summary line and a table with one row per entry.
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Acceptance report: golden project J-E-2416, gate M1 (headless)\n\n");
        let _ = writeln!(
            out,
            "{} entries: {} pass, {} fail, {} not_built, {} not_applicable. Oracle mismatches: {}. `fail` and `not_built` block the gate; `not_applicable` does not.\n",
            self.checks.len(),
            self.count(Status::Pass),
            self.count(Status::Fail),
            self.count(Status::NotBuilt),
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
