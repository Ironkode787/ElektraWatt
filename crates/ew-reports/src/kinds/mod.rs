// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in report kinds (doc 09 §2.4).
//!
//! Derived now (Derivation I): `toc`, `parts_summary`, `parts_list`, `part_data`, `structure_overview`, and
//! `profile_tables` (the cover's colour and size tables, from the profiles in the project settings).
//! Derived now from the Derivation II queries (potential partition, physical wires, effective wire
//! properties): `wire_list`, `potential_overview`, `terminal_diagram`, `cable_overview_diagrams`. Those
//! queries read the drawn page order, which has no report pages, so these kinds derive their rows inside the
//! pagination like the others.
//! Derived from the PLC queries (`plc_rows`, doc 10 §2.10): `plc_overview`. From the tags and wire numbers:
//! `numbering_report` (frozen values with their rule values, FR-REP-022). From the revision records:
//! `revision_overview` (`revision_rows`, FT-207; WP-S1.18 part 2). From the layout: `enclosure_legend`
//! (`layout_legend`, WP-S4.01 part 4; registered, drawn on no page until part 4b). Other schema kinds are not registered; their blocks
//! report [`NotDerived::Unsupported`]. [`Pending`] registers a kind with its columns before its row source exists.

use ew_derive::Q;
use ew_derive::ew_schema::ReportBlock;

use crate::kind::ReportKindDef;
use crate::rowset::{CellType, ColumnDef, NotDerived, RowSet};

mod cable_overview;
mod conn;
mod enclosure_legend;
mod numbering_report;
mod part_data;
mod parts;
mod plc_overview;
mod potential_overview;
mod profile_tables;
mod revision_overview;
mod structure_overview;
mod terminal_diagram;
mod toc;
mod wire_list;

pub use cable_overview::CableOverview;
pub use conn::bridge_groups;
pub use enclosure_legend::EnclosureLegend;
pub use numbering_report::NumberingReport;
pub use part_data::PartData;
pub use parts::{PartsList, PartsSummary};
pub use plc_overview::PlcOverview;
pub use potential_overview::PotentialOverview;
pub use profile_tables::{GROUPS as PROFILE_TABLE_GROUPS, ProfileTables};
pub use revision_overview::{RevisionOverview, revision_rows};
pub use structure_overview::StructureOverview;
pub use terminal_diagram::TerminalDiagram;
pub use toc::Toc;
pub use wire_list::WireList;

/// Every built-in kind of this build.
pub(crate) fn builtin() -> Vec<&'static dyn ReportKindDef> {
    vec![
        &Toc,
        &PartsSummary,
        &PartsList,
        &PartData,
        &StructureOverview,
        &TerminalDiagram,
        &CableOverview,
        &WireList,
        &PotentialOverview,
        &PlcOverview,
        &NumberingReport,
        &ProfileTables,
        &RevisionOverview,
        &EnclosureLegend,
    ]
}

/// A kind registered with its columns whose row source does not exist yet.
#[derive(Debug)]
pub struct Pending {
    name: &'static str,
    columns: &'static [(&'static str, CellType, &'static str)],
    reason: NotDerived,
}

impl Pending {
    /// Why the kind has no rows.
    pub fn reason(&self) -> &NotDerived {
        &self.reason
    }
}

impl ReportKindDef for Pending {
    fn name(&self) -> &'static str {
        self.name
    }

    fn columns(&self) -> Vec<ColumnDef> {
        self.columns
            .iter()
            .map(|(k, t, l)| ColumnDef::new(k, *t, l))
            .collect()
    }

    fn rows(&self, _q: &Q, _block: &ReportBlock) -> RowSet {
        RowSet::not_derived(self.columns(), self.reason.clone())
    }
}
