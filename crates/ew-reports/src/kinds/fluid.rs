// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid report kinds (doc 11 §8.1, §8.2): `tube_list` (FR-FLU-012), `tube_cut_list` and `tube_labels`
//! (FR-FLU-013), `valve_terminal_config` (FR-FLU-015) and `medium_overview` (FR-FLU-016), over the row sources of
//! `ew-derive` (`tube_rows`, `valve_terminal_rows`, `medium_rows`).

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{DeviceState, LengthSource, LibCopy, Medium, Part, ReportBlock, ReportKind, Unit};
use ew_derive::ew_units::Nm;
use ew_derive::{
    EndpointKey, MediumRows, PageRef, Q, TubeEnd, TubeRow, TubeRows, ValveTerminalRows, VtSlot, colour_text,
    line_kind_text, medium_text, part_number, port_role_text, short_tag, tube_spec_text,
};

use super::conn::put;
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, MissingReason, QtyUnit, Row, RowId, RowKey, RowSet};
use crate::words::in_languages;

fn text(t: MlText) -> Cell {
    if t.is_empty() { Cell::Empty } else { Cell::Text(t) }
}

/// A tube end: its `DT:port`, or "tee" in the reader's language for a junction.
fn end_cell(end: &TubeEnd) -> Cell {
    if matches!(end.at, EndpointKey::Junction(_)) {
        text(end.words())
    } else {
        Cell::ident(&end.text)
    }
}

/// The part number of a part copy; empty when the copy is absent.
fn copy_number(q: &Q, part: Option<Id<Part>>) -> String {
    part.and_then(|p| {
        q.table::<LibCopy<Part>>()
            .get(p)
            .map(|c| c.item.part_number.as_str().to_owned())
    })
    .unwrap_or_default()
}

/// A length source in words.
fn source_text(s: &LengthSource) -> MlText {
    match s {
        LengthSource::Manual(_) => in_languages(&text!("cell-manual")),
        LengthSource::FromPart => in_languages(&text!("cell-from-part")),
        LengthSource::Missing | LengthSource::Unknown(_) => in_languages(&text!("cell-missing")),
    }
}

// ---------------------------------------------------------------------------------------------------------
// tube_list

/// The tube list (doc 11 §8.2, FR-FLU-012): one row per physical tube, in `tube_rows` order (natural by tag, ties by
/// the from end, then ID); row ID the tube. `from` is a valve's working port end, else end a. `size` prints od×id in
/// millimetres, `length` in metres with its source; `location` prints "from → to" when the tube crosses a location
/// boundary, empty otherwise; `page` the page of the tube's own connection.
#[derive(Clone, Copy, Debug)]
pub struct TubeList;

impl ReportKindDef for TubeList {
    fn kind(&self) -> ReportKind {
        ReportKind::TubeList
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("tag", CellType::Ident, text!("column-tag")).weighted(8),
            ColumnDef::new("from", CellType::Ident, text!("column-from")).weighted(12),
            ColumnDef::new("from_role", CellType::Text, text!("column-role")).weighted(8),
            ColumnDef::new("to", CellType::Ident, text!("column-to")).weighted(12),
            ColumnDef::new("to_role", CellType::Text, text!("column-role")).weighted(8),
            ColumnDef::new("medium", CellType::Text, text!("column-medium")).weighted(12),
            ColumnDef::new("line_kind", CellType::Text, text!("column-line")).weighted(8),
            ColumnDef::new("size", CellType::Ident, text!("column-od-id")).weighted(7),
            ColumnDef::new("material", CellType::Ident, text!("column-material")).weighted(9),
            ColumnDef::new("colour", CellType::Colour, text!("column-colour")).weighted(7),
            ColumnDef::new("part", CellType::Ident, text!("column-tube-part")).weighted(10),
            ColumnDef::new("length", CellType::Qty, text!("column-length")).weighted(6),
            ColumnDef::new("length_source", CellType::Text, text!("column-source")).weighted(8),
            ColumnDef::new("fitting_a", CellType::Ident, text!("column-fitting-a")).weighted(9),
            ColumnDef::new("fitting_b", CellType::Ident, text!("column-fitting-b")).weighted(9),
            ColumnDef::new("location", CellType::Ident, text!("column-location-change")).weighted(16),
            ColumnDef::new("page", CellType::Folio, text!("column-page")).weighted(5),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for r in q.get::<TubeRows>(()).iter() {
            let row = tube_row(q, &set.columns, r);
            set.rows.push(row);
        }
        set
    }
}

fn tube_row(q: &Q, columns: &[ColumnDef], r: &TubeRow) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    put(columns, &mut cells, "tag", Cell::ident(&r.tag));
    put(columns, &mut cells, "from", end_cell(&r.from));
    put(columns, &mut cells, "to", end_cell(&r.to));
    if let Some(role) = &r.from.role {
        put(columns, &mut cells, "from_role", text(port_role_text(role)));
    }
    if let Some(role) = &r.to.role {
        put(columns, &mut cells, "to_role", text(port_role_text(role)));
    }
    put(columns, &mut cells, "medium", text(medium_text(q, r.net)));
    put(
        columns,
        &mut cells,
        "line_kind",
        text(line_kind_text(&r.line_kind)),
    );
    put(columns, &mut cells, "size", Cell::ident(&r.props.size_text()));
    if let Some(m) = &r.props.material {
        put(columns, &mut cells, "material", Cell::ident(m.value.code()));
    }
    if let Some(c) = &r.props.colour {
        put(columns, &mut cells, "colour", Cell::Colour(c.value.clone()));
    }
    put(
        columns,
        &mut cells,
        "part",
        Cell::ident(&copy_number(q, r.props.part.as_ref().map(|p| p.value))),
    );
    put(
        columns,
        &mut cells,
        "length",
        // No length: a short mark; the Source column says that it is missing.
        r.length.map_or_else(|| Cell::ident("—"), Cell::metres),
    );
    put(
        columns,
        &mut cells,
        "length_source",
        text(source_text(&r.length_source)),
    );
    put(
        columns,
        &mut cells,
        "fitting_a",
        Cell::ident(&copy_number(q, r.from.fitting)),
    );
    put(
        columns,
        &mut cells,
        "fitting_b",
        Cell::ident(&copy_number(q, r.to.fitting)),
    );
    if r.location_change() {
        put(
            columns,
            &mut cells,
            "location",
            Cell::ident(&format!("{} → {}", r.from.location, r.to.location)),
        );
    }
    if let Some(page) = r.page {
        put(columns, &mut cells, "page", Cell::Folio(PageRef::Drawn(page)));
    }
    let mut row = Row::data(RowId::entity(r.id), cells);
    row.sources = r.constituents.iter().map(|c| c.cast()).collect();
    row
}

// ---------------------------------------------------------------------------------------------------------
// tube_cut_list

/// A cut-list line's key: material, size, colour and tube part.
type CutKey = (String, String, String, String);

/// A cut-list line: its first tube, the count, the summed length in nanometres, whether every tube has a length, and
/// the first tube's row (its colour).
type CutLine = (Id<ew_derive::ew_schema::Connection>, i64, i64, bool, TubeRow);

/// The tube cut list (doc 11 §8.2, FR-FLU-013): the tubes grouped by material, size, colour and tube part, each line
/// with the count and the summed length in metres (a tube without a length counts but adds none, and the line's
/// length is then missing). Lines in natural order of material, size, colour and part; row ID the group's first tube.
/// No per-end cut allowance and no order increment (no part data names one).
#[derive(Clone, Copy, Debug)]
pub struct TubeCutList;

impl ReportKindDef for TubeCutList {
    fn kind(&self) -> ReportKind {
        ReportKind::TubeCutList
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("material", CellType::Ident, text!("column-material")).weighted(10),
            ColumnDef::new("size", CellType::Ident, text!("column-od-id")).weighted(10),
            ColumnDef::new("colour", CellType::Colour, text!("column-colour")).weighted(8),
            ColumnDef::new("part", CellType::Ident, text!("column-tube-part")).weighted(16),
            ColumnDef::new("count", CellType::Number, text!("column-pieces")).weighted(8),
            ColumnDef::new("length", CellType::Qty, text!("column-total-length")).weighted(12),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let mut groups: BTreeMap<CutKey, CutLine> = BTreeMap::new();
        for r in q.get::<TubeRows>(()).iter() {
            let key = (
                r.props
                    .material
                    .as_ref()
                    .map(|m| m.value.code().to_owned())
                    .unwrap_or_default(),
                r.props.size_text(),
                r.props
                    .colour
                    .as_ref()
                    .and_then(|c| colour_text(&c.value))
                    .unwrap_or_default(),
                copy_number(q, r.props.part.as_ref().map(|p| p.value)),
            );
            let entry = groups.entry(key).or_insert((r.id, 0, 0, true, r.clone()));
            entry.1 += 1;
            match r.length {
                Some(n) => entry.2 = entry.2.saturating_add(n.0),
                None => entry.3 = false,
            }
        }
        let mut keyed: Vec<_> = groups.into_iter().collect();
        keyed.sort_by(|a, b| {
            let k = |x: &(String, String, String, String)| {
                (
                    ew_derive::ew_mltext::natural_key(&x.0),
                    ew_derive::ew_mltext::natural_key(&x.1),
                    x.2.clone(),
                    ew_derive::ew_mltext::natural_key(&x.3),
                )
            };
            k(&a.0).cmp(&k(&b.0))
        });
        let mut set = RowSet::new(columns);
        for ((material, size, _, part), (first, count, total, complete, sample)) in keyed {
            let mut cells = vec![Cell::Empty; set.columns.len()];
            put(&set.columns, &mut cells, "material", Cell::ident(&material));
            put(&set.columns, &mut cells, "size", Cell::ident(&size));
            if let Some(c) = &sample.props.colour {
                put(&set.columns, &mut cells, "colour", Cell::Colour(c.value.clone()));
            }
            put(&set.columns, &mut cells, "part", Cell::ident(&part));
            put(&set.columns, &mut cells, "count", Cell::Number(count));
            put(
                &set.columns,
                &mut cells,
                "length",
                if complete {
                    Cell::metres(Nm(total))
                } else {
                    Cell::Missing(MissingReason::Length)
                },
            );
            set.rows.push(Row::data(RowId::entity(first), cells));
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------------------
// tube_labels

/// The tube labels (doc 11 §8.2, FR-FLU-013), the label engine's input: one row per tube end, in `tube_rows` order,
/// the from end first; the text is the tube's line tag, else the destination (the other end, `-203Q1:A`). Row ID the
/// tube with the end's index. No marker part (no part data names one).
#[derive(Clone, Copy, Debug)]
pub struct TubeLabels;

impl ReportKindDef for TubeLabels {
    fn kind(&self) -> ReportKind {
        ReportKind::TubeLabels
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("tube", CellType::Ident, text!("column-tube")).weighted(16),
            ColumnDef::new("at", CellType::Ident, text!("column-at")).weighted(14),
            ColumnDef::new("text", CellType::Ident, text!("column-label-text")).weighted(14),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for r in q.get::<TubeRows>(()).iter() {
            let name = r.name();
            for (i, (end, other)) in [(&r.from, &r.to), (&r.to, &r.from)].into_iter().enumerate() {
                let mut cells = vec![Cell::Empty; set.columns.len()];
                put(&set.columns, &mut cells, "tube", text(name.clone()));
                put(&set.columns, &mut cells, "at", end_cell(end));
                let label = if r.tag.is_empty() {
                    end_cell(other)
                } else {
                    Cell::ident(&r.tag)
                };
                put(&set.columns, &mut cells, "text", label);
                let id = RowId::entity(r.id).with(RowKey::Index(u32::try_from(i).unwrap_or(0)));
                set.rows.push(Row::data(id, cells));
            }
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------------------
// valve_terminal_config

/// The valve-terminal configuration (doc 11 §8.2, FR-FLU-015): per valve terminal, a row for the terminal and each
/// child in position order, and for a module with valve positions a row per position: position, module, module part,
/// valve, valve part, solenoids, addresses, function text, process tag and occupancy. Row ID the module (with the
/// position for a position row).
#[derive(Clone, Copy, Debug)]
pub struct ValveTerminalConfig;

impl ReportKindDef for ValveTerminalConfig {
    fn kind(&self) -> ReportKind {
        ReportKind::ValveTerminalConfig
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("module_position", CellType::Number, text!("column-slot")).weighted(5),
            ColumnDef::new("module", CellType::Ident, text!("column-module")).weighted(10),
            ColumnDef::new("module_part", CellType::Ident, text!("column-module-part")).weighted(10),
            ColumnDef::new("position", CellType::Number, text!("column-position")).weighted(6),
            ColumnDef::new("valve", CellType::Ident, text!("column-valve")).weighted(11),
            ColumnDef::new("valve_part", CellType::Ident, text!("column-valve-part")).weighted(10),
            ColumnDef::new("solenoids", CellType::Ident, text!("column-solenoids")).weighted(7),
            ColumnDef::new("address", CellType::Address, text!("column-address")).weighted(9),
            ColumnDef::new("function", CellType::Text, text!("column-function-text")).weighted(12),
            ColumnDef::new("process_tag", CellType::Ident, text!("column-process-tag")).weighted(11),
            ColumnDef::new("occupancy", CellType::Text, text!("column-occupancy")).weighted(12),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for r in q.get::<ValveTerminalRows>(()).iter() {
            let c = &set.columns;
            let mut cells = vec![Cell::Empty; c.len()];
            if let Some(p) = r.module_position {
                put(c, &mut cells, "module_position", Cell::Number(i64::from(p)));
            }
            put(c, &mut cells, "module", Cell::ident(&short_tag(q, r.module)));
            put(
                c,
                &mut cells,
                "module_part",
                Cell::ident(&part_number(q, r.module)),
            );
            let mut id = RowId::entity(r.module);
            if let Some(p) = r.position {
                put(c, &mut cells, "position", Cell::Number(i64::from(p)));
                id = id.with(RowKey::Index(p));
                let occupancy = match (&r.slot, &r.state) {
                    (VtSlot::Valve, Some(DeviceState::Spare)) => in_languages(&text!("cell-valve-spare")),
                    (VtSlot::Valve, Some(DeviceState::Reserved)) => {
                        in_languages(&text!("cell-valve-reserved"))
                    }
                    (VtSlot::Valve, Some(DeviceState::NotFitted)) => {
                        in_languages(&text!("cell-valve-not-fitted"))
                    }
                    (VtSlot::Valve, _) => in_languages(&text!("cell-valve-used")),
                    (VtSlot::Blank, _) => in_languages(&text!("cell-blanking-plate")),
                    (VtSlot::Empty, _) => in_languages(&text!("cell-empty")),
                    (VtSlot::Module, _) => MlText::default(),
                };
                put(c, &mut cells, "occupancy", text(occupancy));
            }
            if let Some(d) = r.device {
                put(c, &mut cells, "valve", Cell::ident(&short_tag(q, d)));
                put(c, &mut cells, "valve_part", Cell::ident(&part_number(q, d)));
            }
            put(c, &mut cells, "solenoids", Cell::ident(&r.solenoids.join(", ")));
            if let Some(a) = &r.addresses {
                put(c, &mut cells, "address", Cell::Address(a.clone()));
            }
            put(c, &mut cells, "function", text(r.function_text.clone()));
            put(
                c,
                &mut cells,
                "process_tag",
                Cell::ident(r.process_tag.as_deref().unwrap_or_default()),
            );
            let mut row = Row::data(id, cells);
            row.sources = r
                .device
                .into_iter()
                .map(|d| d.cast())
                .chain([r.module.cast()])
                .collect();
            set.rows.push(row);
        }
        set
    }
}

// ---------------------------------------------------------------------------------------------------------
// medium_overview

/// The medium overview (doc 11 §8.2, FR-FLU-016): one row per project medium, in natural order of its name: the
/// name, its definition points, the nominal pressure, the quality class, the consumers (valves whose port 1 and actuators
/// whose consumer port lie in a net carrying it) with their count, and the default tube. Row ID the medium.
#[derive(Clone, Copy, Debug)]
pub struct MediumOverview;

impl ReportKindDef for MediumOverview {
    fn kind(&self) -> ReportKind {
        ReportKind::MediumOverview
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("medium", CellType::Text, text!("column-medium")).weighted(16),
            ColumnDef::new("sources", CellType::Ident, text!("column-sources")).weighted(14),
            ColumnDef::new("p_nom", CellType::Qty, text!("column-pressure")).weighted(8),
            ColumnDef::new("quality", CellType::Ident, text!("column-quality")).weighted(8),
            ColumnDef::new("consumers", CellType::Range, text!("column-consumers")).weighted(20),
            ColumnDef::new("consumer_count", CellType::Number, text!("column-count")).weighted(6),
            ColumnDef::new("default_tube", CellType::Ident, text!("column-default-tube")).weighted(12),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let media = q.table::<Medium>();
        for r in q.get::<MediumRows>(()).iter() {
            let Some(m) = media.get(r.medium) else { continue };
            let c = &set.columns;
            let mut cells = vec![Cell::Empty; c.len()];
            put(c, &mut cells, "medium", text(m.name.clone()));
            put(c, &mut cells, "sources", Cell::ident(&r.sources.join(", ")));
            put(
                c,
                &mut cells,
                "p_nom",
                Cell::Qty(m.p_nom, QtyUnit::Unit(Unit::Bar)),
            );
            put(
                c,
                &mut cells,
                "quality",
                Cell::ident(m.quality.as_ref().map(|x| x.as_str()).unwrap_or_default()),
            );
            put(c, &mut cells, "consumers", Cell::Range(r.consumers.clone()));
            put(
                c,
                &mut cells,
                "consumer_count",
                Cell::Number(i64::try_from(r.consumers.len()).unwrap_or(i64::MAX)),
            );
            put(
                c,
                &mut cells,
                "default_tube",
                Cell::ident(&tube_spec_text(q, &m.default_tube)),
            );
            let mut row = Row::data(RowId::entity(r.medium), cells);
            row.sources = vec![r.medium.cast()];
            set.rows.push(row);
        }
        set
    }
}
