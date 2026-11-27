// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `wire_list`: the wire and connection list (FR-REP-014, FR-CON-034; folio 907 of the standard layout).

use ew_derive::{CpRef, EndTreatment, Q, WireLength, WireRecord, short_tag};
use ew_ids::Id;
use ew_l10n::{List, TextArg};
use ew_schema::{Device, ReportBlock, ReportKind, Unit};
use ew_units::Dec3;

use super::conn::{
    Bridge, Wires, bridge_span, bridges, colour_cell, csa_cell, device_of, end_cells, ip_name, join_texts,
    pages_cell, put, source_text, words_cell,
};
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, QtyUnit, Row, RowId, RowKey, RowKind, RowSet};

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
/// size first, then colour, joined when they differ). `via` names, joined by "; ", the junction the wire passes by
/// its place and wiring (`101-C11 star at F55:2`; `102-C6 chain on -102F1:1`), its daisy chains (`chain on
/// -101F1:1`, two conductors on one clamp) and the interruption points it passes (`IP 0VDC-AIR2`; doc 08 §3.2,
/// D-20). `no` is the wire number `{page}.{counter}` of the connection-oriented scheme (counter scope page); a cable
/// core, which has none, prints its cable and core (`-106W1 BK`, doc 13 §3.1); a tie between wires of equal cells is
/// decided by the wire ID and not printed. `part` is the wire part the wire is cut from: the one its chain names, else
/// the workshop stock's (`ew_derive::wire_part_of`, FR-CON-033).
///
/// An end (doc 13 §3.2) prints its device or terminal without the structure the two ends share, so an end in
/// another location prints it (`+FLD-104B1`), and its pin in the CP column: a repeated pin with `#n` (`+#1`), a
/// terminal's clamp letter (`a`). A bridge prints its kind and span in its From end cell.
///
/// `length` is the wire's length (doc 13 §3.3): typed, in millimetres; `cable` for a core, cut with its cable; `—`
/// without one; `length_source` says which (typed, cable, missing).
#[derive(Clone, Copy, Debug)]
pub struct WireList;

impl ReportKindDef for WireList {
    fn kind(&self) -> ReportKind {
        ReportKind::WireList
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("no", CellType::Ident, text!("column-no")).weighted(7),
            ColumnDef::new("from", CellType::Tag, text!("column-from")).weighted(13),
            ColumnDef::new("from_cp", CellType::Ident, text!("column-cp")).weighted(6),
            ColumnDef::new("to", CellType::Tag, text!("column-to")).weighted(13),
            ColumnDef::new("to_cp", CellType::Ident, text!("column-cp")).weighted(6),
            ColumnDef::new("potential", CellType::Ident, text!("column-potential")).weighted(11),
            ColumnDef::new("colour", CellType::Colour, text!("column-colour")).weighted(6),
            ColumnDef::new("cross_section", CellType::Qty, text!("column-mm2")).weighted(6),
            ColumnDef::new("source", CellType::Text, text!("column-source")).weighted(10),
            ColumnDef::new("via", CellType::Text, text!("column-via")).weighted(22),
            ColumnDef::new("part", CellType::Ident, text!("column-wire-part")).weighted(8),
            ColumnDef::new("pages", CellType::Folios, text!("column-pages")).weighted(8),
            ColumnDef::new("length", CellType::Qty, text!("column-length")).weighted(7),
            ColumnDef::new("length_source", CellType::Text, text!("column-length-from")).weighted(7),
            ColumnDef::new("from_end", CellType::Text, text!("column-from-end")).weighted(12),
            ColumnDef::new("to_end", CellType::Text, text!("column-to-end")).weighted(12),
            ColumnDef::new("from_part", CellType::Ident, text!("column-from-part")).weighted(9),
            ColumnDef::new("to_part", CellType::Ident, text!("column-to-part")).weighted(9),
            ColumnDef::new("from_strip", CellType::Qty, text!("column-from-strip")).weighted(6),
            ColumnDef::new("to_strip", CellType::Qty, text!("column-to-strip")).weighted(6),
            ColumnDef::new("from_marker", CellType::Ident, text!("column-from-marker")).weighted(11),
            ColumnDef::new("to_marker", CellType::Ident, text!("column-to-marker")).weighted(11),
        ]
    }

    /// What the wire list prints when its block chooses no columns (doc 13 §3.1): the fitter's columns, No., From, CP,
    /// To, CP, Colour, mm², Length, From end, To end, From marker, To marker and Pages; the engineering columns (Potential, Source, Via, Wire
    /// part) and the ends' parts and strip lengths stay in the list's exports and are ticked in List columns….
    fn printed(&self) -> Option<&'static [&'static str]> {
        Some(&[
            "no",
            "from",
            "from_cp",
            "to",
            "to_cp",
            "colour",
            "cross_section",
            "length",
            "from_end",
            "to_end",
            "from_marker",
            "to_marker",
            "pages",
        ])
    }

    fn rows(&self, q: &Q, _block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let wires = Wires::of(q);
        let mut set = RowSet::new(columns);
        set.rows = wires.wires.iter().map(|rec| wire_row(q, &wires, rec)).collect();

        let bridges = bridges(q);
        if !bridges.is_empty() {
            let mut heading = vec![Cell::Empty; set.columns.len()];
            put(
                &set.columns,
                &mut heading,
                "no",
                words_cell(&text!("cell-bridges")),
            );
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

/// The wire list's row of one wire.
pub(super) fn wire_row(q: &Q, wires: &Wires, rec: &WireRecord) -> Row {
    let w = &rec.wire;
    let (from, from_cp) = end_cells(q, &w.ends[0], &w.ends[1]);
    let (to, to_cp) = end_cells(q, &w.ends[1], &w.ends[0]);
    // The parts of the Via column: only the junction's and the chains' words are in a language.
    let (junction, daisy) = join_texts(q, w);
    let mut via: Vec<TextArg> = junction.into_iter().chain(daisy).map(TextArg::from).collect();
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
        via.push(format!("IP {}", ips.join(", ")).into());
    }
    let via = if via.is_empty() {
        Cell::Empty
    } else {
        let between = || text!("cell-via-between");
        words_cell(&ew_l10n::Text::bare(List::new(via, between(), between())))
    };
    let part = rec
        .wire_part
        .as_ref()
        .and_then(|p| ew_derive::printed_part(q, p))
        .map_or(Cell::Empty, |c| {
            super::parts::part_number_cell(c.item.part_number.as_str())
        });
    let no = wire_no(q, rec).map_or(Cell::Empty, Cell::Ident);
    let columns = WireList.columns();
    let mut cells = vec![Cell::Empty; columns.len()];
    let mut set = |key: &str, cell: Cell| put(&columns, &mut cells, key, cell);
    set("no", no);
    set("from", from);
    set("from_cp", from_cp);
    set("to", to);
    set("to_cp", to_cp);
    set("potential", Cell::ident(&wires.potential_text(w)));
    set("colour", colour_cell(rec.colour.as_ref().map(|p| &p.value)));
    set("cross_section", csa_cell(rec.csa.as_ref().map(|p| &p.value)));
    set("source", source_text(rec));
    set("via", via);
    set("part", part);
    set("pages", pages_cell(&w.pages));
    set("length", length_cell(rec.length));
    set(
        "length_source",
        Cell::Text(ew_derive::text::in_languages(&ew_derive::text::wire_length_from(
            q, rec,
        ))),
    );
    for (end, key) in rec.ends.iter().zip(["from_marker", "to_marker"]) {
        // The lines a marker label carries, joined by ` / ` (doc 13 §5.2); none where the end carries none.
        if let ew_derive::MarkerText::Lines(lines) = &end.marker {
            set(key, Cell::Ident(lines.join(" / ")));
        }
    }
    for (end, (treatment, part, strip)) in rec.ends.iter().zip([
        ("from_end", "from_part", "from_strip"),
        ("to_end", "to_part", "to_strip"),
    ]) {
        let copy = end.part.and_then(|p| q.parts().get(p));
        let ferrule = copy.and_then(|c| c.item.sections.ferrule.as_ref());
        let partner = match end.treatment {
            EndTreatment::TwinFerrule {
                partner: Some(partner),
                ..
            } => wires.get(partner).and_then(|r| wire_no(q, r)),
            _ => None,
        };
        let words = ew_derive::text::end_treatment(
            &end.treatment,
            ferrule.and_then(|f| f.colour.as_ref()),
            ferrule.and_then(|f| f.stud.as_ref()).map(|s| s.as_str()),
            partner.as_deref(),
        );
        set(
            treatment,
            words.map_or_else(
                || Cell::ident("—"),
                |t| Cell::Text(ew_derive::text::in_languages(&t)),
            ),
        );
        set(
            part,
            copy.map_or(Cell::Empty, |c| {
                super::parts::part_number_cell(c.item.part_number.as_str())
            }),
        );
        set(
            strip,
            ferrule.and_then(|f| f.sleeve).map_or(Cell::Empty, |n| {
                Cell::Qty(Dec3(n.0 / 1_000), QtyUnit::Unit(Unit::Mm))
            }),
        );
    }
    let mut r = Row::data(RowId::entity(w.id), cells);
    r.sources = w
        .ends
        .iter()
        .filter_map(|e| device_of(q, e.cp.function))
        .map(|d: Id<Device>| d.cast())
        .collect();
    r
}

/// The No. of a wire: its number; a core, which has none, its cable and core (`-103W1 BN`).
pub(super) fn wire_no(q: &Q, rec: &WireRecord) -> Option<String> {
    match (&rec.number, rec.wire.core.and_then(|c| q.conductors().get(c))) {
        (Some(n), _) => Some(n.clone()),
        (None, Some(core)) => Some(format!("{} {}", short_tag(q, core.cable), core.core.as_str())),
        (None, None) => None,
    }
}

/// A wire's length as the Length cell prints it (doc 13 §3.3, doc 12 §9): in millimetres as typed or routed, after "≈"
/// along a way that no longer holds, `cable` for a core, `—` without one.
fn length_cell(length: WireLength) -> Cell {
    match length {
        WireLength::Typed(n) | WireLength::Routed { length: n, .. } | WireLength::Stale { length: n, .. } => {
            Cell::stale_if(
                Cell::Qty(Dec3(n.0 / 1_000), QtyUnit::Unit(Unit::Mm)),
                matches!(length, WireLength::Stale { .. }),
            )
        }
        WireLength::WithCable(_) => Cell::Text(ew_derive::text::in_languages(
            &ew_derive::text::wire_length_source(&length),
        )),
        WireLength::Missing => Cell::ident("—"),
    }
}

fn bridge_row(q: &Q, b: &Bridge) -> Row {
    let first = b.terminals.first().map_or(Cell::Empty, |t| Cell::Tag(*t));
    let last = b.terminals.last().map_or(Cell::Empty, |t| Cell::Tag(*t));
    let span = bridge_span(q, b);
    let kind = words_cell(&text!("cell-bridge-span", b.kind.clone(), span));
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
    set("from_end", kind);
    // A bridge without a part prints TBD (doc 13 §6.1), as the assembly list's bridge block does.
    set(
        "part",
        b.part
            .as_ref()
            .and_then(|p| ew_derive::printed_part(q, p))
            .map_or(Cell::Tbd, |c| {
                super::parts::part_number_cell(c.item.part_number.as_str())
            }),
    );
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
