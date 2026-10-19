// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in report kinds (doc 09 §2.4).
//!
//! Derived now (Derivation I): `toc`, `parts_summary`, `parts_list`, `part_data`, `structure_overview`.
//! Registered with their columns but **not derived yet** (an empty row set marked
//! [`NotDerived`]): the connectivity kinds, which wait for the Derivation II queries
//! (`terminal_diagram`, `cable_overview_diagrams`, `wire_list`, `potential_overview`, `plc_overview`), and
//! the kinds whose source records have no shape in this format (`profile_tables`: colour and size profiles;
//! `revision_overview`: revisions). Other schema kinds are not registered; their blocks report
//! [`NotDerived::Unsupported`].

use ew_derive::Q;
use ew_derive::ew_schema::ReportBlock;

use crate::kind::ReportKindDef;
use crate::rowset::{CellType, ColumnDef, NotDerived, RowSet};

mod part_data;
mod parts;
mod structure_overview;
mod toc;

pub use part_data::PartData;
pub use parts::{PartsList, PartsSummary};
pub use structure_overview::StructureOverview;
pub use toc::Toc;

/// Every built-in kind of this build.
pub(crate) fn builtin() -> Vec<&'static dyn ReportKindDef> {
    vec![
        &Toc,
        &PartsSummary,
        &PartsList,
        &PartData,
        &StructureOverview,
        &TERMINAL_DIAGRAM,
        &CABLE_OVERVIEW_DIAGRAMS,
        &WIRE_LIST,
        &POTENTIAL_OVERVIEW,
        &PLC_OVERVIEW,
        &PROFILE_TABLES,
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

// The columns below are provisional: the row content of the connectivity reports is owned by docs 08, 10
// and 20 §9–§11, and lands with the Derivation II queries.

/// Terminal diagram per strip (FR-REP-009; golden 903–905).
pub static TERMINAL_DIAGRAM: Pending = Pending {
    name: "terminal_diagram",
    columns: &[
        ("strip", CellType::Tag, "Strip"),
        ("terminal", CellType::Ident, "Terminal"),
        ("level", CellType::Number, "Level"),
        ("inside", CellType::Tag, "Inside target"),
        ("inside_cp", CellType::Ident, "Inside CP"),
        ("outside", CellType::Tag, "Outside target"),
        ("outside_cp", CellType::Ident, "Outside CP"),
        ("bridge", CellType::Ident, "Bridge"),
        ("page", CellType::CellRef, "Page"),
    ],
    reason: NotDerived::AwaitsQuery("terminal_rows"),
};

/// Cable overview and per-cable diagrams (FR-REP-012, FR-REP-013; golden 906).
pub static CABLE_OVERVIEW_DIAGRAMS: Pending = Pending {
    name: "cable_overview_diagrams",
    columns: &[
        ("cable", CellType::Tag, "Cable"),
        ("part", CellType::Ident, "Cable type"),
        ("core", CellType::Ident, "Core"),
        ("colour", CellType::Colour, "Colour"),
        ("cross_section", CellType::Qty, "Cross-section"),
        ("from", CellType::Tag, "From"),
        ("to", CellType::Tag, "To"),
        ("length", CellType::Qty, "Length"),
    ],
    reason: NotDerived::AwaitsQuery("cable_rows"),
};

/// Wire list (FR-REP-014; golden 907–907.2).
pub static WIRE_LIST: Pending = Pending {
    name: "wire_list",
    columns: &[
        ("wire", CellType::Ident, "Wire"),
        ("from", CellType::Tag, "From"),
        ("from_cp", CellType::Ident, "From CP"),
        ("to", CellType::Tag, "To"),
        ("to_cp", CellType::Ident, "To CP"),
        ("colour", CellType::Colour, "Colour"),
        ("cross_section", CellType::Qty, "Cross-section"),
        ("potential", CellType::Ident, "Potential"),
        ("part", CellType::Ident, "Wire part"),
        ("pages", CellType::Folios, "Pages"),
    ],
    reason: NotDerived::AwaitsQuery("wire_rows"),
};

/// Potential overview (FR-REP-016; golden 909).
pub static POTENTIAL_OVERVIEW: Pending = Pending {
    name: "potential_overview",
    columns: &[
        ("potential", CellType::Ident, "Potential"),
        ("kind", CellType::Text, "Type"),
        ("members", CellType::Number, "Members"),
        ("colour", CellType::Colour, "Colour"),
        ("pages", CellType::Folios, "Pages"),
    ],
    reason: NotDerived::AwaitsQuery("potential_partition"),
};

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

/// The cover's colour and standard-size tables (FR-REP-028, FR-REP-038; golden folio 1).
pub static PROFILE_TABLES: Pending = Pending {
    name: "profile_tables",
    columns: &[
        ("voltage_class", CellType::Text, "Voltage"),
        ("function", CellType::Text, "Function"),
        ("potential_type", CellType::Text, "Potential type"),
        ("colour", CellType::Text, "Colour"),
        ("code", CellType::Colour, "Code"),
    ],
    reason: NotDerived::NoRecordShape("colour_profile"),
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
