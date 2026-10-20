// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The migration report of an import (doc 14 §2.5): findings grouped by code with counts, as Markdown, CSV
//! or JSON (`ew import … --report md|csv|json`). (STUB: implemented by the report work item.)

use ew_interop_api::ImportReport;

/// The migration report of one import (STUB).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct MigrationReport {
    /// The number of findings.
    pub total: u32,
}

impl MigrationReport {
    /// The report of `report` (STUB).
    pub fn from_report(report: &ImportReport) -> MigrationReport {
        MigrationReport {
            total: u32::try_from(report.entries.len()).unwrap_or(u32::MAX),
        }
    }
}
