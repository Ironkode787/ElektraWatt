// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `assembly_list`: per strip the order the fitter mounts it in (doc 13 §6.1, FR-MFG-016), from
//! `ew_derive::assembly_rows`.

use std::collections::BTreeMap;

use ew_derive::layout::StripItemKind;
use ew_derive::{
    AssemblyItem, AssemblyItemKind, BracketFrom, LabelRows, LabelSet, Q, StripAssembly,
    assembly_rows, mm_tenth_short, short_tag, strip_bracket,
};
use ew_ids::Id;
use ew_l10n::{Text, TextArg};
use ew_schema::{AccessoryKind, Device, DeviceState, Part, ReportBlock, ReportKind, TechVal};
use ew_units::Dec3;

use super::conn::{
    Bridge, Wires, bridge_span_from_to, bridges, measured_cell, put, terminal_designation, terminal_wired,
    wires_by_cp, words_cell,
};
use super::parts::part_number_cell;
use super::terminal_diagram::strips_of;
use super::terminal_overview::first_segment_position;
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, MissingReason, QtyUnit, Row, RowId, RowKey, RowKind, RowSet};
use crate::words::selecting;

/// The assembly list: per strip of the block's selection (else every strip), in the order of the strip's first segment
/// in the drawn page order, then by tag, a heading row that says where the strip goes and how long it is, a row per
/// item in mounting order, and its bridges under a heading of their own. Row IDs start with the strip: the heading
/// `(strip, "heading")`, an item `(strip, n)`, the bridges' heading `(strip, "bridges")` and a bridge `(strip, "bridge",
/// group)`.
#[derive(Clone, Copy, Debug)]
pub struct AssemblyList;

impl ReportKindDef for AssemblyList {
    fn kind(&self) -> ReportKind {
        ReportKind::AssemblyList
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("pos", CellType::Number, text!("column-pos")).weighted(4),
            ColumnDef::new("item", CellType::Text, text!("column-item")).weighted(10),
            ColumnDef::new("terminal", CellType::Ident, text!("column-terminal")).weighted(8),
            ColumnDef::new("part_number", CellType::Ident, text!("column-part-number")).weighted(14),
            ColumnDef::new("description", CellType::Text, text!("column-description")).weighted(28),
            ColumnDef::new("colour", CellType::Ident, text!("column-colour")).weighted(6),
            ColumnDef::new("width", CellType::Qty, text!("column-width-mm")).weighted(6),
            ColumnDef::new("marker", CellType::Ident, text!("column-marker")).weighted(8),
            ColumnDef::new("note", CellType::Text, text!("column-note")).weighted(16),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let mut strips = strips_of(q, block);
        // `strips_of` orders by tag; the stable sort keeps that order among the strips of one page.
        strips.sort_by_cached_key(|s| first_segment_position(q, *s));
        let markers = terminal_markers(q);
        let wires = Wires::of(q);
        let by_cp = wires_by_cp(&wires);
        let bridges = bridges(q);
        for strip in strips {
            let Some(a) = assembly_rows(q, strip) else {
                continue;
            };
            set.rows.push(heading(
                q,
                &set.columns,
                RowId::entity(strip).with(RowKey::Name("heading".to_owned())),
                &heading_words(q, &a),
            ));
            let dev = q.devices().get(strip).cloned();
            for (n, item) in a.items.iter().enumerate() {
                let note = match &item.kind {
                    AssemblyItemKind::Item(StripItemKind::Terminal(t)) => {
                        let spare = q.devices().get(*t).is_some_and(|d| d.state == DeviceState::Spare);
                        match (spare, terminal_wired(q, *t, &by_cp)) {
                            (true, true) => Some(text!("cell-spare-fed")),
                            (true, false) => Some(text!("cell-spare")),
                            _ => None,
                        }
                    }
                    AssemblyItemKind::Item(StripItemKind::Accessory(acc)) => with_words(q, &acc.with),
                    AssemblyItemKind::Item(StripItemKind::Own(_)) => Some(text!("cell-added")),
                    AssemblyItemKind::EndBracket => {
                        dev.as_ref().and_then(|d| strip_bracket(q, d)).and_then(|b| {
                            match (b.from, b.declared) {
                                (BracketFrom::PartUse(_), _) | (_, None) => None,
                                (_, Some(d)) => with_words(q, &[d.with]),
                            }
                        })
                    }
                };
                let id = RowId::entity(strip).with(RowKey::Index(u32::try_from(n).unwrap_or(u32::MAX)));
                set.rows.push(item_row(
                    q,
                    &set.columns,
                    id,
                    n,
                    item,
                    dev.as_deref(),
                    &markers,
                    note,
                ));
            }
            let own: Vec<&Bridge> = bridges.iter().filter(|b| b.strip == strip).collect();
            if own.is_empty() {
                continue;
            }
            set.rows.push(heading(
                q,
                &set.columns,
                RowId::entity(strip).with(RowKey::Name("bridges".to_owned())),
                &text!("cell-bridges-of", short_tag(q, strip)),
            ));
            for b in own {
                set.rows.push(bridge_row(q, &set.columns, b));
            }
        }
        set
    }
}

/// A heading row: `words` in the first cell, their decimals as the list writes them.
fn heading(q: &Q, columns: &[ColumnDef], id: RowId, words: &Text) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    cells[0] = measured_cell(q, words);
    Row {
        id,
        kind: RowKind::Heading,
        cells,
        links: Vec::new(),
        sources: Vec::new(),
    }
}

/// The heading of a strip (doc 13 §6.1): its tag, where it goes, the rail's cut length, the strip it abuts, whether it is
/// turned, its terminals and its length with its end brackets, as one text.
fn heading_words(q: &Q, a: &StripAssembly) -> Text {
    let none = || "none".to_owned();
    let (rail, from, to, cut, abuts, turned) = match &a.on {
        Some(on) => (
            i64::from(on.number),
            mm_tenth_short(on.from),
            mm_tenth_short(on.to),
            on.cut.map_or_else(none, ew_derive::mm_exact),
            on.abuts.map_or_else(none, |s| short_tag(q, s)),
            if on.turned { "yes" } else { "no" },
        ),
        None => (0, String::new(), String::new(), none(), none(), "no"),
    };
    let length = match (a.width_missing, a.bracketed) {
        (false, true) => "brackets",
        (true, true) => "least-brackets",
        (false, false) => "exact",
        (true, false) => "least",
    };
    selecting(key!("cell-assembly-heading"))
        .with("strip", short_tag(q, a.strip))
        .with("rail", TextArg::Number(rail))
        .with("from", from)
        .with("to", to)
        .with("cut", cut)
        .with("abuts", abuts)
        .with("turned", turned)
        .with(
            "terminals",
            TextArg::Number(i64::try_from(a.terminals).unwrap_or(i64::MAX)),
        )
        .with("length", mm_tenth_short(a.length))
        .with("form", length)
}

/// "with EW-GEN-TRM-1, EW-GEN-TRM-2": the parts an accessory comes with, by number; `None` for none.
fn with_words(q: &Q, with: &[Id<Part>]) -> Option<Text> {
    let numbers: Vec<String> = with
        .iter()
        .filter_map(|p| q.parts().get(*p).map(|c| c.item.part_number.as_str().to_owned()))
        .collect();
    (!numbers.is_empty()).then(|| text!("cell-with", numbers.join(", ")))
}

/// The terminal markers' texts by terminal (`label_rows`, doc 13 §5.3), their lines joined by " / ", and so the markers
/// of a terminal's levels.
fn terminal_markers(q: &Q) -> BTreeMap<Id<Device>, String> {
    let mut out: BTreeMap<Id<Device>, String> = BTreeMap::new();
    for l in q.get::<LabelRows>(()).iter().filter(|l| l.set == LabelSet::TerminalMarkers) {
        if let Some(d) = l.target.device() {
            let text = out.entry(d).or_default();
            if !text.is_empty() {
                text.push_str(" / ");
            }
            text.push_str(&l.lines.join(" / "));
        }
    }
    out
}

/// One item's row.
#[allow(clippy::too_many_arguments)]
fn item_row(
    q: &Q,
    columns: &[ColumnDef],
    id: RowId,
    n: usize,
    item: &AssemblyItem,
    strip: Option<&Device>,
    markers: &BTreeMap<Id<Device>, String>,
    note: Option<Text>,
) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    let mut set = |key: &str, cell: Cell| put(columns, &mut cells, key, cell);
    set("pos", Cell::Number(i64::try_from(n + 1).unwrap_or(i64::MAX)));
    let kind = match &item.kind {
        AssemblyItemKind::EndBracket => Some(AccessoryKind::EndClamp),
        AssemblyItemKind::Item(StripItemKind::Terminal(_)) => None,
        AssemblyItemKind::Item(StripItemKind::Accessory(a)) => Some(
            a.kind
                .clone()
                .unwrap_or(AccessoryKind::Unknown(serde_json::Value::Null)),
        ),
        AssemblyItemKind::Item(StripItemKind::Own(i)) => strip
            .and_then(|s| s.aspects.strip.as_ref())
            .and_then(|s| s.accessories.get(*i))
            .map(|a| a.kind.clone()),
    };
    set(
        "item",
        words_cell(&kind.map_or_else(|| text!("cell-terminal"), |k| ew_derive::text::accessory_kind(&k))),
    );
    let mut sources = Vec::new();
    if let AssemblyItemKind::Item(StripItemKind::Terminal(t)) = &item.kind {
        sources.push(t.cast());
        set("terminal", Cell::ident(&terminal_designation(q, *t)));
        if let Some(TechVal::Ident(c)) = ew_derive::tech_value(q, *t, "colour") {
            set("colour", Cell::ident(c.as_str()));
        }
        if let Some(m) = markers.get(t) {
            set("marker", Cell::ident(m));
        }
    }
    match item.part.as_ref().and_then(|p| ew_derive::printed_part(q, p)) {
        Some(copy) => {
            set("part_number", part_number_cell(copy.item.part_number.as_str()));
            set("description", Cell::Text(copy.item.description.clone()));
        }
        None => set("part_number", Cell::Missing(MissingReason::Part)),
    }
    set(
        "width",
        match item.width {
            Some(w) => Cell::Qty(Dec3(i64::try_from(w / 1_000).unwrap_or(i64::MAX)), QtyUnit::None),
            None => words_cell(&text!("cell-width-missing")),
        },
    );
    if let Some(note) = note {
        set("note", words_cell(&note));
    }
    let mut row = Row::data(id, cells);
    row.sources = sources;
    row
}

/// A bridge's row in its strip's bridge block: its label, the terminals it joins, its part and its poles.
fn bridge_row(q: &Q, columns: &[ColumnDef], b: &Bridge) -> Row {
    let mut cells = vec![Cell::Empty; columns.len()];
    let mut set = |key: &str, cell: Cell| put(columns, &mut cells, key, cell);
    set("item", words_cell(&text!("cell-bridge-label", b.label.clone())));
    set("terminal", words_cell(&bridge_span_from_to(q, b)));
    match b.part.as_ref().and_then(|p| ew_derive::printed_part(q, p)) {
        Some(copy) => {
            set("part_number", part_number_cell(copy.item.part_number.as_str()));
            set("description", Cell::Text(copy.item.description.clone()));
        }
        // A bridge without a part is still to be decided (FR-CHK-034, EW-MFG-021).
        None => set("part_number", Cell::Tbd),
    }
    set(
        "note",
        words_cell(&selecting(key!("cell-poles")).with(
            "count",
            TextArg::Number(i64::try_from(b.terminals.len()).unwrap_or(i64::MAX)),
        )),
    );
    let id = RowId::entity(b.strip)
        .with(RowKey::Name("bridge".to_owned()))
        .with(RowKey::Index(u32::from(b.group)));
    let mut row = Row::data(id, cells);
    row.sources = b.terminals.iter().map(|t| t.cast()).collect();
    row
}
