// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in report kinds (doc 09 §2.4).
//!
//! Derived now (Derivation I): `toc`, `parts_summary`, `parts_list`, `part_data`, `structure_overview`, `device_list`
//! (FR-REP-008, the device rows of `structure_overview`), and `profile_tables` (the cover's colour and
//! size tables, from the profiles in the project settings).
//! Derived now from the Derivation II queries (potential partition, physical wires, effective wire
//! properties): `wire_list`, `potential_overview`, `terminal_diagram`, `terminal_overview` (FR-REP-010, one row per
//! strip), `cable_overview_diagrams`. Those
//! queries read the drawn page order, which has no report pages, so these kinds derive their rows inside the
//! pagination like the others.
//! Derived from the PLC queries (`plc_rows`, doc 10 §2.10): `plc_overview` and `io_list` (FR-PLC-023); from the PLC
//! and bus queries of 1.0 the lists of doc 10 §5: `plc_card_list`, `network_address_list`, `bus_overview`,
//! `bus_cable_list`, `iolink_port_list` and `safety_io_list`. From the tags and wire numbers:
//! `numbering_report` (frozen values with their rule values, FR-REP-022). From the revision records:
//! `revision_overview` (`revision_rows`). From the layout: `enclosure_legend`
//! (`layout_legend`, registered, drawn on no page). From the check engine:
//! `open_items` (FR-REP-021, the open findings of the open-item rules, `ew_checks::open_items`; none
//! of those rules reads the page order, so the list paginates inside it). Other schema kinds are not registered; their blocks
//! report [`NotDerived::Unsupported`](crate::rowset::NotDerived::Unsupported).

use crate::kind::ReportKindDef;

mod cable_overview;
mod conn;
mod device_list;
mod enclosure_legend;
mod io_list;
mod numbering_report;
mod open_items;
mod part_data;
mod parts;
mod plc_bus_lists;
mod plc_overview;
mod potential_overview;
mod profile_tables;
mod revision_overview;
mod structure_overview;
mod terminal_diagram;
mod terminal_overview;
mod toc;
mod wire_list;

pub use cable_overview::CableOverview;
pub use conn::bridge_groups;
pub use device_list::DeviceList;
pub use enclosure_legend::EnclosureLegend;
pub use io_list::IoList;
pub use numbering_report::NumberingReport;
pub use open_items::OpenItems;
pub use part_data::PartData;
pub use parts::{PartsList, PartsSummary};
pub use plc_bus_lists::{
    BusCableList, BusOverview, IolinkPortList, NetworkAddressList, PlcCardList, SafetyIoList,
};
pub use plc_overview::PlcOverview;
pub use potential_overview::PotentialOverview;
pub use profile_tables::{GROUPS as PROFILE_TABLE_GROUPS, ProfileTables};
pub use revision_overview::{RevisionOverview, revision_rows};
pub use structure_overview::StructureOverview;
pub use terminal_diagram::TerminalDiagram;
pub use terminal_overview::TerminalOverview;
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
        &IoList,
        &NumberingReport,
        &ProfileTables,
        &RevisionOverview,
        &EnclosureLegend,
        &DeviceList,
        &TerminalOverview,
        &OpenItems,
        &PlcCardList,
        &NetworkAddressList,
        &BusOverview,
        &BusCableList,
        &IolinkPortList,
        &SafetyIoList,
    ]
}
