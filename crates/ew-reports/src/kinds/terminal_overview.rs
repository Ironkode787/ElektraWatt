// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `terminal_overview`: the terminal strip overview (FR-REP-010).

use ew_derive::{
    DevicePartLines, DrawnPageOrder, MainRep, PageOrderV, PageRef, Q, RepAnchorCell, StructureOf, short_tag,
};
use ew_ids::Id;
use ew_schema::{Device, RepType, ReportBlock, ReportKind, Representation};

use super::conn::{
    TerminalState, Wires, bridges, put, terminal_state, terminal_wired, text_cell, wires_by_cp,
};
use super::terminal_diagram::strips_of;
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, LinkRef, LinkTarget, Row, RowId, RowSet};

/// The terminal strip overview.
///
/// One row per strip in `terminal_diagram`'s strip order (the strips of the block's manual selection, else every
/// strip by the natural order of its tag; one shared function); row ID `entity(strip)`, sources the strip.
///
/// - `strip`: the short tag, as the terminal-diagram titles print it (`-X1031`).
/// - `location`: the strip's location designation, as `structure_overview` prints it.
/// - `terminals`, `used`, `spare`, `other`: the strip's terminals and their states by `terminal_diagram`'s one state
///   rule; `other` counts `reserved`, `not fitted` and `unused`, so terminals = used + spare + other.
/// - `parts`: each distinct terminal part as `<count> × <part number>`, in the order of first appearance by `seq`,
///   joined by ", " (a terminal's part is its first part line, as `terminal_diagram`'s Part column).
/// - `segments`: the frame cells (`103-A3`) of the strip's segment representations in drawing order (page order,
///   then row, then column, then ID), joined by ", "; a strip with no segment representation prints the cell of its
///   main representation (the strip fallback of `MainRep`, its first placed terminal, such as
///   `101-D2`), and a strip with neither prints nothing.
#[derive(Clone, Copy, Debug)]
pub struct TerminalOverview;

impl ReportKindDef for TerminalOverview {
    fn kind(&self) -> ReportKind {
        ReportKind::TerminalOverview
    }

    /// The weights are millimetres of the house table's 260.96 mm: each column holds the widest text of its kind (the
    /// header or a cell: "-X1031", "Location (+)", "Terminals", "Used", "Spare", "Other", "2 × UK 5 N, 1 × USLKG 5",
    /// "103-A3, 103-A10") at the house cap height, the free-text columns Parts and Segments with the most room.
    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("strip", CellType::Ident, text!("column-strip")).weighted(25),
            ColumnDef::new("location", CellType::Ident, text!("column-location-aspect")).weighted(20),
            ColumnDef::new("terminals", CellType::Number, text!("column-terminals")).weighted(22),
            ColumnDef::new("used", CellType::Number, text!("column-used")).weighted(16),
            ColumnDef::new("spare", CellType::Number, text!("column-spare")).weighted(16),
            ColumnDef::new("other", CellType::Number, text!("column-other")).weighted(16),
            ColumnDef::new("parts", CellType::Text, text!("column-parts")).weighted(112),
            ColumnDef::new("segments", CellType::Text, text!("column-segments")).weighted(34),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let columns = self.columns();
        let wires = Wires::of(q);
        let by_cp = wires_by_cp(&wires);
        let bridges = bridges(q);
        let mut set = RowSet::new(columns);
        for strip in strips_of(q, block) {
            let mut terminals: Vec<(u32, Id<Device>)> = q
                .idx()
                .terminals_of_strip(strip)
                .filter_map(|t| Some((q.devices().get(t)?.aspects.terminal.as_ref()?.seq, t)))
                .collect();
            terminals.sort_unstable();
            let (mut used, mut spare, mut other) = (0_i64, 0_i64, 0_i64);
            let mut parts: Vec<(String, i64)> = Vec::new();
            for (_, t) in &terminals {
                let Some(dev) = q.devices().get(*t) else { continue };
                let bridged = bridges.iter().any(|b| b.terminals.contains(t));
                match terminal_state(&dev.state, terminal_wired(q, *t, &by_cp), bridged) {
                    TerminalState::Used => used += 1,
                    TerminalState::Spare => spare += 1,
                    _ => other += 1,
                }
                let number = q
                    .get::<DevicePartLines>(*t)
                    .iter()
                    .find(|l| !l.is_accessory())
                    .and_then(|l| l.part_number(q));
                if let Some(number) = number {
                    match parts.iter_mut().find(|(n, _)| *n == number) {
                        Some((_, count)) => *count += 1,
                        None => parts.push((number, 1)),
                    }
                }
            }
            let parts: Vec<String> = parts
                .into_iter()
                .map(|(number, count)| format!("{count} \u{d7} {number}"))
                .collect();
            let location = q
                .get::<StructureOf>(strip)
                .designation(|a| a.as_str() == "location")
                .to_string();
            let set_columns = &set.columns;
            let mut cells = vec![Cell::Empty; set_columns.len()];
            put(
                set_columns,
                &mut cells,
                "strip",
                Cell::ident(&short_tag(q, strip)),
            );
            put(set_columns, &mut cells, "location", Cell::ident(&location));
            put(
                set_columns,
                &mut cells,
                "terminals",
                Cell::Number(used + spare + other),
            );
            put(set_columns, &mut cells, "used", Cell::Number(used));
            put(set_columns, &mut cells, "spare", Cell::Number(spare));
            put(set_columns, &mut cells, "other", Cell::Number(other));
            put(set_columns, &mut cells, "parts", text_cell(parts.join(", ")));
            put(
                set_columns,
                &mut cells,
                "segments",
                text_cell(segments_text(q, strip).join(", ")),
            );
            let mut r = Row::data(RowId::entity(strip), cells);
            r.sources = vec![strip.cast()];
            // The strip links its main representation.
            if let Some(c) = set_columns.iter().position(|c| c.key == "strip") {
                r.links = vec![LinkRef::whole(c, LinkTarget::MainRep(strip))];
            }
            set.rows.push(r);
        }
        set
    }
}

/// The cells of the strip's segment representations in drawing order, else the cell of its main representation.
fn segments_text(q: &Q, strip: Id<Device>) -> Vec<String> {
    segments(q, strip).into_iter().map(|p| p.text).collect()
}

/// The position in the drawn page order of the strip's first segment representation, else of its main
/// representation (`usize::MAX` without either): the assembly list's order of the strips (doc 13 §6.1).
pub(super) fn first_segment_position(q: &Q, strip: Id<Device>) -> usize {
    segments(q, strip).first().map_or(usize::MAX, |p| p.position)
}

/// The strip's segment representations in drawing order, else its main representation.
fn segments(q: &Q, strip: Id<Device>) -> Vec<Placed> {
    let order = q.get::<DrawnPageOrder>(());
    let reps = q.reps();
    let mut segments: Vec<Placed> = Vec::new();
    for f in q.idx().functions_of_device(strip) {
        for id in q.idx().reps_of_function(f) {
            if reps.get(id).is_some_and(|r| r.rep_type == RepType::StripSegment)
                && let Some(placed) = placed_cell(q, &order, id)
            {
                segments.push(placed);
            }
        }
    }
    segments.sort();
    segments.dedup();
    if segments.is_empty() {
        return (*q.get::<MainRep>(strip))
            .and_then(|rep| placed_cell(q, &order, rep))
            .into_iter()
            .collect();
    }
    segments
}

/// A representation's place in drawing order and its printed cell; ordered by page position in the drawn page order,
/// then the cell's row and column, then the representation's ID.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Placed {
    position: usize,
    row: u32,
    column: u32,
    rep: Id<Representation>,
    /// `<folio>-<cell>` (`103-A3`).
    text: String,
}

/// Where a representation is drawn; `None` without a drawn page or a frame cell.
fn placed_cell(q: &Q, order: &PageOrderV, rep: Id<Representation>) -> Option<Placed> {
    let page = PageRef::Drawn(q.reps().get(rep)?.page);
    let position = order.position(page)?;
    let folio = &order.entries[position].folio;
    let cell = q.get::<RepAnchorCell>(rep).as_ref().clone()?;
    Some(Placed {
        position,
        row: cell.row,
        column: cell.column,
        rep,
        text: format!("{folio}-{cell}"),
    })
}
