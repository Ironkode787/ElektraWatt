// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The columns of the navigators' tables (doc 16 §5.2, §7.1): a column's key and label, the routes of the edit mapping (doc 16 §7.2) that make
//! a cell editable, the derived flags and the strip editor's columns.
//!
//! Not here: the rows (each provider's own file beside this one) and the table types (`navigators.rs`).

use ew_derive::ew_schema::ReportKind;

use super::bus::BUS_COLUMNS;
use super::libraries;
use super::messages::MESSAGE_COLUMNS;
use super::plc::PLC_COLUMNS;
use super::{NavColumn, NavKind};

/// A column of a report kind or of this module: its label, no route, read-only with "no command yet" unless a route
/// is given.
pub(super) fn column(key: &str, label: &str, key_column: bool, route: Option<(&str, &str)>) -> NavColumn {
    NavColumn {
        key: key.to_owned(),
        label: label.to_owned(),
        derived: false,
        key_column,
        route: route.map(|(k, p)| (k.to_owned(), p.to_owned())),
        read_only: route.is_none().then(|| "no command yet".to_owned()),
        strip_cell: None,
    }
}

/// A column key as a label: the first letter in capitals and `_` read as a space (`page_cell` → "Page cell").
pub(super) fn key_label(key: &str) -> String {
    let text = key.replace('_', " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The columns of a report kind, the first one the key column.
fn kind_columns(kind: &ReportKind) -> Vec<NavColumn> {
    ew_reports::Registry::builtin()
        .get(kind)
        .map(|k| {
            k.columns()
                .iter()
                .enumerate()
                .map(|(i, c)| column(&c.key, &crate::output::view::neutral(&c.label), i == 0, None))
                .collect()
        })
        .unwrap_or_default()
}

/// The editable columns of table mode (D10): (navigator, column key, route kind, route path).
/// Every other column is read-only.
pub(super) const EDITABLE_COLUMNS: [(NavKind, &str, &str, &str); 36] = [
    (NavKind::Pages, "title", "page", "title"),
    (NavKind::Pages, "type", "page", "kind"),
    (NavKind::Devices, "function_text", "device", "function_text"),
    (NavKind::Connections, "colour", "connection", "overrides.colour"),
    (
        NavKind::Connections,
        "cross_section",
        "connection",
        "overrides.csa",
    ),
    (NavKind::Strips, "terminal", "device", "terminal.designation"),
    (NavKind::Strips, "label", "device", "function_text"),
    (NavKind::Strips, "state", "device", "state"),
    (NavKind::Cables, "colour", "conductor", "colour"),
    (NavKind::Cables, "cross_section", "conductor", "csa"),
    // FR-CBL-016: a core's name (an identifier of its cable), shield and pair; a cable's length
    // through its typed editor (FT-429).
    (NavKind::Cables, "core", "conductor", "core"),
    (NavKind::Cables, "shield", "conductor", "shield"),
    (NavKind::Cables, "pair", "conductor", "pair"),
    (NavKind::Cables, "length", "device", "cable.length"),
    (NavKind::Potentials, "potential", "potential_def", "name"),
    // The G-PP triage's row 8: Type and Voltage set a definition's `ptype` and `voltage`; on a potential without one
    // the Name defines it, with Type and Voltage typed in the same gesture (the view model compiles
    // `potential.define` at `NavNode::define_at`).
    (NavKind::Potentials, "kind", "potential_def", "ptype"),
    (NavKind::Potentials, "voltage", "potential_def", "voltage"),
    (NavKind::Potentials, "class", "potential_def", "circuit_class"),
    (NavKind::Potentials, "colour", "potential_def", "props.colour"),
    (NavKind::Potentials, "cross_section", "potential_def", "props.csa"),
    // FR-PLC-021: a channel's members through `fields.set`; Address and Target through
    // their typed editors (`Session::parse_field_input`), Slot the module's through its typed editor.
    (NavKind::Plc, "slot", "device", "plc_module.slot"),
    (NavKind::Plc, "designation", "channel", "designation"),
    (NavKind::Plc, "address", "channel", "address"),
    (NavKind::Plc, "symbol", "channel", "symbol"),
    (NavKind::Plc, "state", "channel", "state"),
    (NavKind::Plc, "target", "channel", "target"),
    (NavKind::Plc, "comment", "channel", "comment"),
    // FT-737: a channel's data type and signal range in the cell; its safety data in Properties (the cell says so).
    (NavKind::Plc, "data_type", "channel", "data_type"),
    (NavKind::Plc, "signal_range", "channel", "signal_range"),
    (NavKind::Plc, "safety", "channel", "safety"),
    // FT-856: a bus port's address through its typed editor, its termination as its choice.
    (NavKind::Bus, "address", "function", "bus_port.address"),
    (NavKind::Bus, "termination", "function", "bus_port.termination"),
    // The tube table's tube part and length, the line's (WP-S2.13).
    (NavKind::Tubes, "part", "connection", "tube_part"),
    (NavKind::Tubes, "length", "connection", "tube_length"),
    (NavKind::Structure, "designation", "structure_node", "code"),
    (
        NavKind::Structure,
        "function_text",
        "structure_node",
        "description",
    ),
];

/// The derived columns: cells the derivation computes from other records rather than a
/// field of the row's record (a connection's or terminal's potential, the folios a row is drawn on, counts). Drawn
/// italic in table mode and read-only with the reason "derived".
const DERIVED_COLUMNS: [(NavKind, &str); 17] = [
    (NavKind::Pages, "changed_in"),
    (NavKind::Connections, "potential"),
    (NavKind::Cables, "potential"),
    (NavKind::Connections, "pages"),
    (NavKind::Strips, "potential"),
    (NavKind::Potentials, "aliases"),
    (NavKind::Potentials, "members"),
    (NavKind::Potentials, "pages"),
    (NavKind::Structure, "usage"),
    (NavKind::Blocks, "pages"),
    // A channel's process tag comes from its target, its page from the bound coil. Its function text, its target's
    // too, is typed in its cell (`table_edit::channel_text_cell`).
    (NavKind::Plc, "process_tag"),
    (NavKind::Plc, "placed_on"),
    // The rack is the module's slot's (edited in the Slot cell).
    (NavKind::Plc, "rack"),
    // The Unplaced devices navigator's cells are the device's tag and its primary part's data.
    (NavKind::Unplaced, "device"),
    (NavKind::Unplaced, "part"),
    (NavKind::Unplaced, "size"),
    (NavKind::Unplaced, "mount"),
];

/// The strip-editor columns of the Terminal strips navigator: (column key, its compile).
const STRIP_CELLS: [(&str, crate::output::strip_edit::StripCell); 3] = [
    ("group", crate::output::strip_edit::StripCell::Group),
    ("bridge_part", crate::output::strip_edit::StripCell::BridgePart),
    ("part", crate::output::strip_edit::StripCell::AccessoryPart),
];

/// The columns of a navigator with the editable-cell routes and the derived flags.
pub(super) fn columns_of(kind: NavKind) -> Vec<NavColumn> {
    // The PLC grid has the PLC navigator's columns.
    let kind = kind.source();
    let mut columns = plain_columns_of(kind);
    for c in &mut columns {
        if kind == NavKind::Strips
            && let Some((_, cell)) = STRIP_CELLS.iter().find(|(k, _)| *k == c.key)
        {
            c.strip_cell = Some(*cell);
        }
        if DERIVED_COLUMNS.contains(&(kind, c.key.as_str())) {
            c.derived = true;
            c.route = None;
            c.read_only = Some("derived".to_owned());
        } else if let Some((_, _, k, p)) = EDITABLE_COLUMNS
            .iter()
            .find(|(n, key, _, _)| *n == kind && *key == c.key)
        {
            c.route = Some(((*k).to_owned(), (*p).to_owned()));
            c.read_only = None;
        }
    }
    columns
}

/// The columns of a navigator before the editable-cell routes.
fn plain_columns_of(kind: NavKind) -> Vec<NavColumn> {
    let own = |keys: &[&str]| -> Vec<NavColumn> {
        keys.iter()
            .enumerate()
            .map(|(i, k)| column(k, &key_label(k), i == 0, None))
            .collect()
    };
    match kind.source() {
        NavKind::Pages => vec![
            column("folio", "Folio", true, None),
            column("title", "Title", false, None),
            column("type", "Type", false, None),
            column("frame", "Frame", false, None),
            column("generated", "Generated", false, None),
            // The page change status of doc 15 §5.6.
            column("changed_in", "Changed in", false, None),
        ],
        NavKind::Devices => kind_columns(&ReportKind::StructureOverview),
        NavKind::Connections => kind_columns(&ReportKind::WireList),
        // `pos` after `strip`, `bridge_part` after `bridge`; the list's own `strip` column is the first, and its `level`
        // column is the printed list's alone.
        NavKind::Strips => {
            let mut c = own(&["strip"]);
            c.push(column("pos", "Pos", false, None));
            for mut k in kind_columns(&ReportKind::TerminalDiagram)
                .into_iter()
                .filter(|k| k.key != "strip" && k.key != "level")
            {
                k.key_column = false;
                let bridge = k.key == "bridge";
                c.push(k);
                if bridge {
                    c.push(column("bridge_part", "Bridge part", false, None));
                }
            }
            c
        }
        // The cable overview's columns (a core row's From and To are the far ends of its
        // wires, as the cable diagram prints them), then a core's derived potential, its shield flag and pair.
        NavKind::Cables => {
            let mut c = kind_columns(&ReportKind::CableOverviewDiagrams);
            c.extend(["potential", "shield", "pair"].map(|k| column(k, &key_label(k), false, None)));
            c
        }
        // The PLC grid's own columns; the first, Address, is the key column.
        NavKind::Plc | NavKind::PlcGrid => PLC_COLUMNS
            .iter()
            .enumerate()
            .map(|(i, (k, l))| column(k, l, i == 0, None))
            .collect(),
        NavKind::Bus => BUS_COLUMNS
            .iter()
            .enumerate()
            .map(|(i, (k, l))| column(k, crate::text_english(l).unwrap_or(l), i == 0, None))
            .collect(),
        // The fluid navigator's table mode lists its tubes (`NavKind::source`).
        NavKind::Fluid | NavKind::Tubes => kind_columns(&ReportKind::TubeList),
        NavKind::Media => kind_columns(&ReportKind::MediumOverview),
        NavKind::Potentials => kind_columns(&ReportKind::PotentialOverview),
        NavKind::Ips => own(&["name", "page_cell", "direction", "partner", "external"]),
        NavKind::Structure => {
            let mut c = kind_columns(&ReportKind::StructureOverview);
            c.push(column("usage", "Usage", false, None));
            c
        }
        NavKind::Parts => kind_columns(&ReportKind::PartsSummary),
        NavKind::Blocks => own(&["title", "kind", "first_folio", "section", "pages"]),
        // The device (the key column, as the tree row names it in table mode), Part,
        // W × H (mm), Mount.
        NavKind::Unplaced => vec![
            column("device", "Device", true, None),
            column("part", "Part", false, None),
            column("size", "W × H (mm)", false, None),
            column("mount", "Mount", false, None),
        ],
        // The Macros and Libraries navigators' own columns.
        NavKind::Macros => libraries::columns(&libraries::MACRO_COLUMNS),
        NavKind::Libraries => libraries::columns(&libraries::LIBRARY_COLUMNS),
        // Every column is derived and read-only; `rule` is the key column.
        NavKind::Messages => MESSAGE_COLUMNS
            .iter()
            .map(|k| NavColumn {
                key: (*k).to_owned(),
                label: key_label(k),
                derived: true,
                key_column: *k == "rule",
                route: None,
                read_only: Some("derived".to_owned()),
                strip_cell: None,
            })
            .collect(),
    }
}
