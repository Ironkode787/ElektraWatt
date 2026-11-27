// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in report kinds (doc 09 §2.4): [`builtin`] lists every kind this build derives, one module a kind or a
//! family. A kind reads the queries it needs (the drawn page order for the connection lists, which has no report pages,
//! so those kinds derive their rows inside the pagination like the others). Other schema kinds are not registered;
//! their blocks report [`NotDerived::Unsupported`](crate::rowset::NotDerived::Unsupported).

use crate::kind::ReportKindDef;

mod assembly_list;
mod cable_overview;
mod conn;
mod consumables;
mod cut_list;
mod device_list;
mod enclosure_legend;
mod fluid;
mod hole_table;
pub use hole_table::owner_words as hole_owner_words;
mod io_list;
mod labels;
mod numbering_report;
mod open_items;
mod part_data;
mod parts;
mod plc_bus_lists;
mod plc_overview;
mod potential_overview;
mod profile_tables;
mod revision_overview;
mod rework;
mod structure_overview;
mod terminal_diagram;
mod terminal_overview;
mod toc;
mod wire_list;

pub use assembly_list::AssemblyList;
pub use cable_overview::{CableOverview, cable_length_words, length_source_cell};
pub use consumables::Consumables;
pub use cut_list::{CutList, TubeCutList};
pub use device_list::DeviceList;
pub use enclosure_legend::EnclosureLegend;
pub use fluid::{MediumOverview, TubeLabels, TubeList, ValveTerminalConfig};
pub use hole_table::HoleTable;
pub use io_list::IoList;
pub use labels::{LabelFile, Labels, label_column_words, label_files};
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
pub use rework::rework_rows;
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
        &TubeList,
        &TubeCutList,
        &TubeLabels,
        &ValveTerminalConfig,
        &MediumOverview,
        &Labels,
        &AssemblyList,
        &CutList,
        &Consumables,
        &HoleTable,
    ]
}
