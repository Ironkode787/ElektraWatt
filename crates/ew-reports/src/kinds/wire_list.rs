// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `wire_list`: the wire and connection list (FR-REP-014, FR-CON-034; golden 907, 907.1, 907.2).

use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::MlText;
use ew_derive::ew_schema::{Device, ReportBlock};
use ew_derive::{CpRef, Q, WireNumbers};

use super::conn::{
    Bridge, WireRec, Wires, bridge_span, bridges, colour_cell, csa_cell, device_of, end_cells, ip_name,
    join_texts, pages_cell, put, source_text, tag_short, text_cell,
};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, Row, RowId, RowKey, RowKind, RowSet};

/// The wire list.
///
/// One row per **physical wire**, then a heading and a separate block of **bridge** rows (bridges are never
/// wire rows, doc 08 §6.5). The wires come in the **numbering order** of FR-NUM-028 (doc 08 §5.2): the page
/// of the From end, the From end's frame cell (column, then row), the To end's cell, then the wire ID. The
/// order and the numbers are the `wire_numbers` query of `ew-derive`; cable cores sort in the same order but
/// carry no number (FR-NUM-019).
///
/// The From end is the first of the wire's two ends after orientation (`ew_derive::orient_wire`, doc 08
/// §3.8): the end on the earlier page, else the one with the smaller anchor cell, then the smaller CP ID. It is
/// not always `PhysicalWire::ends[0]`, the end the constituents start from.
///
/// A wire's potential is its name, or `A ⇄ B` for a bond wire. `source` names where the effective
/// cross-section and colour come from (`cdp`, `rule`, `bond`, `core`, `potential`, `project`, `built-in`;
/// size first, then colour, joined when they differ). `via` names, joined by "; ", the cable core of a core
/// wire (`-106W1 BK`), the junction the wire passes by its place and wiring (`101-C11 star at
/// F55:2`; `102-C6 chain on -102F1:1`), its daisy chains (`chain on -101F1:1`, two conductors on one
/// clamp) and the interruption points it passes (`IP 0VDC-AIR2`; doc 08 §3.2, D-20).
/// `no` is the wire number `{page}.{counter}` of the connection-oriented scheme (counter scope page), empty
/// for a cable core; a tie between wires of equal cells is decided by the wire ID and not printed. `part` is
/// the wire part the wire carries explicitly (empty at MVP, FR-CON-033).
#[derive(Clone, Copy, Debug)]
pub struct WireList;

impl ReportKindDef for WireList {
    fn name(&self) -> &'static str {
        "wire_list"
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("no", CellType::Ident, "No.").weighted(7),
            ColumnDef::new("from", CellType::Tag, "From").weighted(13),
            ColumnDef::new("from_cp", CellType::Ident, "CP").weighted(6),
            ColumnDef::new("to", CellType::Tag, "To").weighted(13),
            ColumnDef::new("to_cp", CellType::Ident, "CP").weighted(6),
            ColumnDef::new("potential", CellType::Ident, "Potential").weighted(11),
            ColumnDef::new("colour", CellType::Colour, "Colour").weighted(6),
            ColumnDef::new("cross_section", CellType::Qty, "mm\u{00B2}").weighted(6),
            ColumnDef::new("source", CellType::Ident, "Source").weighted(10),
            ColumnDef::new("via", CellType::Text, "Via").weighted(22),
            ColumnDef::new("part", CellType::Ident, "Wire part").weighted(8),
            ColumnDef::new("pages", CellType::Folios, "Pages").weighted(8),
        ]
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let wires = Wires::of(q);
        let numbers = q.get::<WireNumbers>(());
        let mut keyed: Vec<(usize, Row)> = wires
            .wires
            .iter()
            .map(|rec| {
                let at = numbers.position(rec.wire.id).unwrap_or(usize::MAX);
                let label = numbers.number(rec.wire.id).map(|n| n.label.as_str());
                (at, wire_row(q, &wires, rec, label))
            })
            .collect();
        keyed.sort_by_key(|(at, _)| *at);

        let mut set = RowSet::new(columns);
        set.rows = keyed.into_iter().map(|(_, r)| r).collect();

        let bridges = bridges(q);
        if !bridges.is_empty() {
            let mut heading = vec![Cell::Empty; set.columns.len()];
            put(&set.columns, &mut heading, "no", text_cell("Bridges"));
            set.rows.push(Row {
                id: RowId(vec![RowKey::Name("bridges".to_owned())]),
                kind: RowKind::Heading,
                cells: heading,
                links: Vec::new(),
                sources: Vec::new(),
            });
            set.rows.extend(bridges.iter().map(|b| bridge_row(q, b)));
        }
        set
    }
}

fn wire_row(q: &Q, wires: &Wires, rec: &WireRec, number: Option<&str>) -> Row {
    let w = &rec.wire;
    let (from, from_cp) = end_cells(q, &w.ends[0]);
    let (to, to_cp) = end_cells(q, &w.ends[1]);
    let mut via: Vec<String> = Vec::new();
    if let Some(core) = w.core.and_then(|c| q.conductors().get(c)) {
        via.push(format!("{} {}", tag_short(q, core.cable), core.core.as_str()));
    }
    let (junction, daisy) = join_texts(q, w);
    via.extend(junction);
    via.extend(daisy);
    // The interruption points the wire passes, by name (the two points of a pair share one), not the
    // external point it ends at.
    let mut ips: Vec<String> = Vec::new();
    for f in w
        .ips
        .iter()
        .filter(|f| w.ends.iter().all(|e| e.cp.function != **f))
    {
        let name = ip_name(q, *f);
        if !name.is_empty() && !ips.contains(&name) {
            ips.push(name);
        }
    }
    if !ips.is_empty() {
        via.push(format!("IP {}", ips.join(", ")));
    }
    let part = rec
        .props
        .wire_part
        .as_ref()
        .and_then(|p| q.parts().get(p.value.copy))
        .map_or(Cell::Empty, |c| {
            super::parts::part_number_cell(c.item.part_number.as_str())
        });
    let mut r = Row::data(
        RowId::entity(w.id),
        vec![
            number.map_or(Cell::Empty, |n| Cell::Ident(n.to_owned())),
            from,
            from_cp,
            to,
            to_cp,
            text_ident(wires.potential_text(w)),
            colour_cell(rec.props.colour.as_ref().map(|p| &p.value)),
            csa_cell(rec.props.csa.as_ref().map(|p| &p.value)),
            text_ident(source_text(&rec.props)),
            text_cell(via.join("; ")),
            part,
            pages_cell(&w.pages),
        ],
    );
    r.sources = w
        .ends
        .iter()
        .filter_map(|e| device_of(q, e.cp.function))
        .map(|d: Id<Device>| d.cast())
        .collect();
    r
}

fn text_ident(s: String) -> Cell {
    if s.is_empty() { Cell::Empty } else { Cell::Ident(s) }
}

fn bridge_row(q: &Q, b: &Bridge) -> Row {
    let first = b.terminals.first().map_or(Cell::Empty, |t| Cell::Tag(*t));
    let last = b.terminals.last().map_or(Cell::Empty, |t| Cell::Tag(*t));
    let via = format!("{} bridge, terminals {}", b.kind, bridge_span(q, b));
    let id = RowId::entity(b.strip)
        .with(RowKey::Name("bridge".to_owned()))
        .with(RowKey::Index(u32::from(b.group)));
    let columns = WireList.columns();
    let mut cells = vec![Cell::Empty; columns.len()];
    let mut set = |key: &str, cell: Cell| put(&columns, &mut cells, key, cell);
    set("no", Cell::Ident(b.label.clone()));
    set("from", first);
    set("to", last);
    set("potential", bridge_potential(q, b));
    set("via", Cell::Text(MlText::neutral(&via)));
    set("part", Cell::Tbd);
    set("pages", b.page.map_or(Cell::Empty, |p| pages_cell(&[p])));
    let mut r = Row::data(id, cells);
    r.sources = b.terminals.iter().map(|t| t.cast()).collect();
    r
}

/// The potential a bridge joins, by the first terminal's first CP.
fn bridge_potential(q: &Q, b: &Bridge) -> Cell {
    use ew_derive::{PotentialNameOf, PotentialPartition};
    let Some(t) = b.terminals.first() else {
        return Cell::Empty;
    };
    let partition = q.get::<PotentialPartition>(());
    for f in q.idx().functions_of_device(*t) {
        for cp in q.get::<ew_derive::FunctionCps>(f).iter() {
            if let Some(p) = partition.potential_at(CpRef::new(f, cp.idx).key()) {
                return Cell::Ident(q.get::<PotentialNameOf>(p.id).name.text.clone());
            }
        }
    }
    Cell::Empty
}
