// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid report kinds (doc 11 §8.1, §8.2): `tube_list` (FR-FLU-012), `tube_cut_list` and `tube_labels`
//! (FR-FLU-013), `valve_terminal_config` (FR-FLU-015) and `medium_overview` (FR-FLU-016), over the row sources of
//! `ew-derive` (`tube_rows`, `valve_terminal_rows`, `medium_rows`).

use std::collections::BTreeMap;

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{DeviceState, LengthSource, LibCopy, Medium, Part, ReportBlock, Unit};
use ew_derive::ew_units::{Dec3, Nm};
use ew_derive::{
    MediumRows, PageRef, Q, TubeRow, TubeRows, ValveTerminalRows, VtSlot, colour_text, langs, line_kind_text,
    medium_text, part_number, port_role_text, short_tag, tube_spec_text,
};

use super::conn::put;
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, MissingReason, QtyUnit, Row, RowId, RowKey, RowSet};

fn ident(text: &str) -> Cell {
    if text.is_empty() {
        Cell::Empty
    } else {
        Cell::Ident(text.to_owned())
    }
}

fn text(t: MlText) -> Cell {
    if t.is_empty() { Cell::Empty } else { Cell::Text(t) }
}

/// A length in metres (millimetre precision), as the cable overview prints it.
fn metres(n: Nm) -> Cell {
    Cell::Qty(Dec3(n.0 / 1_000_000), QtyUnit::Unit(Unit::M))
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
        LengthSource::Manual(_) => langs("manual", "handmatig"),
        LengthSource::FromPart => langs("from part", "uit artikel"),
        LengthSource::Missing | LengthSource::Unknown(_) => langs("missing", "ontbreekt"),
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
    fn name(&self) -> &'static str {
        "tube_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("tag", CellType::Ident, "Tag").weighted(8),
            ColumnDef::new("from", CellType::Ident, "From").weighted(14),
            ColumnDef::new("from_role", CellType::Text, "Role").weighted(8),
            ColumnDef::new("to", CellType::Ident, "To").weighted(14),
            ColumnDef::new("to_role", CellType::Text, "Role").weighted(8),
            ColumnDef::new("medium", CellType::Text, "Medium").weighted(14),
            ColumnDef::new("line_kind", CellType::Text, "Line").weighted(8),
            ColumnDef::new("size", CellType::Ident, "OD×ID").weighted(7),
            ColumnDef::new("material", CellType::Ident, "Material").weighted(7),
            ColumnDef::new("colour", CellType::Colour, "Colour").weighted(6),
            ColumnDef::new("part", CellType::Ident, "Tube part").weighted(10),
            ColumnDef::new("length", CellType::Qty, "Length").weighted(7),
            ColumnDef::new("length_source", CellType::Text, "Source").weighted(7),
            ColumnDef::new("fitting_a", CellType::Ident, "Fitting A").weighted(9),
            ColumnDef::new("fitting_b", CellType::Ident, "Fitting B").weighted(9),
            ColumnDef::new("location", CellType::Ident, "Location change").weighted(14),
            ColumnDef::new("page", CellType::Folio, "Page").weighted(5),
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
    put(columns, &mut cells, "tag", ident(&r.tag));
    put(columns, &mut cells, "from", ident(&r.from.text));
    put(columns, &mut cells, "to", ident(&r.to.text));
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
    put(columns, &mut cells, "size", ident(&r.props.size_text()));
    if let Some(m) = &r.props.material {
        put(columns, &mut cells, "material", ident(m.value.code()));
    }
    if let Some(c) = &r.props.colour {
        put(columns, &mut cells, "colour", Cell::Colour(c.value.clone()));
    }
    put(
        columns,
        &mut cells,
        "part",
        ident(&copy_number(q, r.props.part.as_ref().map(|p| p.value))),
    );
    put(
        columns,
        &mut cells,
        "length",
        r.length.map_or(Cell::Missing(MissingReason::Length), metres),
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
        ident(&copy_number(q, r.from.fitting)),
    );
    put(
        columns,
        &mut cells,
        "fitting_b",
        ident(&copy_number(q, r.to.fitting)),
    );
    if r.location_change() {
        put(
            columns,
            &mut cells,
            "location",
            ident(&format!("{} → {}", r.from.location, r.to.location)),
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
    fn name(&self) -> &'static str {
        "tube_cut_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("material", CellType::Ident, "Material").weighted(10),
            ColumnDef::new("size", CellType::Ident, "OD×ID").weighted(10),
            ColumnDef::new("colour", CellType::Colour, "Colour").weighted(8),
            ColumnDef::new("part", CellType::Ident, "Tube part").weighted(16),
            ColumnDef::new("count", CellType::Number, "Pieces").weighted(8),
            ColumnDef::new("length", CellType::Qty, "Total length").weighted(12),
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
            put(&set.columns, &mut cells, "material", ident(&material));
            put(&set.columns, &mut cells, "size", ident(&size));
            if let Some(c) = &sample.props.colour {
                put(&set.columns, &mut cells, "colour", Cell::Colour(c.value.clone()));
            }
            put(&set.columns, &mut cells, "part", ident(&part));
            put(&set.columns, &mut cells, "count", Cell::Number(count));
            put(
                &set.columns,
                &mut cells,
                "length",
                if complete {
                    metres(Nm(total))
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
    fn name(&self) -> &'static str {
        "tube_labels"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("tube", CellType::Ident, "Tube").weighted(16),
            ColumnDef::new("at", CellType::Ident, "At").weighted(14),
            ColumnDef::new("text", CellType::Ident, "Label text").weighted(14),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        for r in q.get::<TubeRows>(()).iter() {
            let name = if r.tag.is_empty() {
                format!("{} → {}", r.from.text, r.to.text)
            } else {
                r.tag.clone()
            };
            for (i, (end, other)) in [(&r.from, &r.to), (&r.to, &r.from)].into_iter().enumerate() {
                let mut cells = vec![Cell::Empty; set.columns.len()];
                put(&set.columns, &mut cells, "tube", ident(&name));
                put(&set.columns, &mut cells, "at", ident(&end.text));
                let label = if r.tag.is_empty() {
                    other.text.clone()
                } else {
                    r.tag.clone()
                };
                put(&set.columns, &mut cells, "text", ident(&label));
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
    fn name(&self) -> &'static str {
        "valve_terminal_config"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("module_position", CellType::Number, "Slot").weighted(5),
            ColumnDef::new("module", CellType::Ident, "Module").weighted(10),
            ColumnDef::new("module_part", CellType::Ident, "Module part").weighted(10),
            ColumnDef::new("position", CellType::Number, "Position").weighted(6),
            ColumnDef::new("valve", CellType::Ident, "Valve").weighted(11),
            ColumnDef::new("valve_part", CellType::Ident, "Valve part").weighted(10),
            ColumnDef::new("solenoids", CellType::Ident, "Solenoids").weighted(7),
            ColumnDef::new("address", CellType::Address, "Address").weighted(9),
            ColumnDef::new("function", CellType::Text, "Function text").weighted(12),
            ColumnDef::new("process_tag", CellType::Ident, "Process tag").weighted(11),
            ColumnDef::new("occupancy", CellType::Text, "Occupancy").weighted(12),
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
            put(c, &mut cells, "module", ident(&short_tag(q, r.module)));
            put(c, &mut cells, "module_part", ident(&part_number(q, r.module)));
            let mut id = RowId::entity(r.module);
            if let Some(p) = r.position {
                put(c, &mut cells, "position", Cell::Number(i64::from(p)));
                id = id.with(RowKey::Index(p));
                let occupancy = match (&r.slot, &r.state) {
                    (VtSlot::Valve, Some(DeviceState::Spare)) => langs("valve, spare", "ventiel, reserve"),
                    (VtSlot::Valve, Some(DeviceState::Reserved)) => {
                        langs("valve, reserved", "ventiel, gereserveerd")
                    }
                    (VtSlot::Valve, Some(DeviceState::NotFitted)) => {
                        langs("valve, not fitted", "ventiel, niet geplaatst")
                    }
                    (VtSlot::Valve, _) => langs("valve, used", "ventiel, in gebruik"),
                    (VtSlot::Blank, _) => langs("blanking plate", "blindplaat"),
                    (VtSlot::Empty, _) => langs("empty", "leeg"),
                    (VtSlot::Module, _) => MlText::default(),
                };
                put(c, &mut cells, "occupancy", text(occupancy));
            }
            if let Some(d) = r.device {
                put(c, &mut cells, "valve", ident(&short_tag(q, d)));
                put(c, &mut cells, "valve_part", ident(&part_number(q, d)));
            }
            put(c, &mut cells, "solenoids", ident(&r.solenoids.join(", ")));
            if let Some(a) = &r.addresses {
                put(c, &mut cells, "address", Cell::Address(a.clone()));
            }
            put(c, &mut cells, "function", text(r.function_text.clone()));
            put(
                c,
                &mut cells,
                "process_tag",
                ident(r.process_tag.as_deref().unwrap_or_default()),
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
/// name, its definition points, the nominal pressure, the quality class, the consumers (devices whose port 1 lies in a
/// net carrying it) with their count, and the default tube. Row ID the medium.
#[derive(Clone, Copy, Debug)]
pub struct MediumOverview;

impl ReportKindDef for MediumOverview {
    fn name(&self) -> &'static str {
        "medium_overview"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("medium", CellType::Text, "Medium").weighted(16),
            ColumnDef::new("sources", CellType::Ident, "Sources").weighted(14),
            ColumnDef::new("p_nom", CellType::Qty, "Pressure").weighted(8),
            ColumnDef::new("quality", CellType::Ident, "Quality").weighted(8),
            ColumnDef::new("consumers", CellType::Range, "Consumers").weighted(20),
            ColumnDef::new("consumer_count", CellType::Number, "Count").weighted(6),
            ColumnDef::new("default_tube", CellType::Ident, "Default tube").weighted(12),
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
            put(c, &mut cells, "sources", ident(&r.sources.join(", ")));
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
                ident(m.quality.as_ref().map(|x| x.as_str()).unwrap_or_default()),
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
                ident(&tube_spec_text(q, &m.default_tube)),
            );
            let mut row = Row::data(RowId::entity(r.medium), cells);
            row.sources = vec![r.medium.cast()];
            set.rows.push(row);
        }
        set
    }
}
