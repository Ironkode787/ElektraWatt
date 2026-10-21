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
//! Registered with their columns but **not derived yet** (an empty row set marked
//! [`NotDerived`]): `plc_overview` (no channel records), and `revision_overview` (the revision records are
//! reserved, not typed). Other schema kinds are not registered; their blocks report
//! [`NotDerived::Unsupported`].

use ew_derive::Q;
use ew_derive::ew_schema::ReportBlock;

use crate::kind::ReportKindDef;
use crate::rowset::{CellType, ColumnDef, NotDerived, RowSet};

mod cable_overview;
mod conn;
mod part_data;
mod parts;
mod potential_overview;
mod profile_tables;
mod structure_overview;
mod terminal_diagram;
mod toc;
mod wire_list;

pub use cable_overview::CableOverview;
pub use part_data::PartData;
pub use parts::{PartsList, PartsSummary};
pub use potential_overview::PotentialOverview;
pub use profile_tables::{GROUPS as PROFILE_TABLE_GROUPS, ProfileTables};
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
        &PLC_OVERVIEW,
        &ProfileTables,
        &REVISION_OVERVIEW,
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

// The columns below are provisional: the row content of the PLC overview is owned by doc 10 and doc 20 §11,
// and lands with the PLC work package.

/// PLC overview per station or module (FR-REP-019; golden 908–908.2).
pub static PLC_OVERVIEW: Pending = Pending {
    name: "plc_overview",
    columns: &[
        ("module", CellType::Tag, "Module"),
        ("channel", CellType::Ident, "Channel"),
        ("address", CellType::Address, "Address"),
        ("symbol", CellType::Ident, "Symbol"),
        ("function", CellType::Text, "Function text"),
        ("page", CellType::CellRef, "Page"),
    ],
    reason: NotDerived::AwaitsQuery("plc_rows"),
};

/// Revision overview (FR-REP-018; golden 911).
pub static REVISION_OVERVIEW: Pending = Pending {
    name: "revision_overview",
    columns: &[
        ("label", CellType::Ident, "Revision"),
        ("date", CellType::Date, "Date"),
        ("author", CellType::Text, "Author"),
        ("checker", CellType::Text, "Checker"),
        ("state", CellType::Text, "State"),
        ("pages", CellType::Folios, "Changed pages"),
    ],
    reason: NotDerived::NoRecordShape("revision"),
};
