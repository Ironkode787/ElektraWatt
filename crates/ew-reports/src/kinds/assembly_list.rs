// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `assembly_list`: where every device goes and per strip the order the fitter mounts it in (doc 13 §6.1,
//! FR-MFG-016), from `ew_derive::layout::rail_assembly`, `surface_items` and `ew_derive::assembly_rows`.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::layout::{
    CarrierLabel, CarrierLabelKind, RailAssembly, RailStart, StripItemKind, Unplaced, cabinet_name,
    carrier_labels, face_of, rail_assembly, surface_datum, surface_items,
};
use ew_derive::{
    AssemblyItem, AssemblyItemKind, BracketFrom, LabelRows, LabelSet, Q, StripAssembly, assembly_rows,
    mm_tenth_short, short_tag, strip_bracket,
};
use ew_ids::Id;
use ew_l10n::{Text, TextArg};
use ew_schema::{
    AccessoryKind, Device, DeviceState, LayoutSpace, MountingSurface, Part, PartRef, ReportBlock, ReportKind,
    SurfaceKind, TechVal,
};
use ew_units::Dec3;

use super::conn::{
    Bridge, Wires, bridge_span_from_to, bridges, measured_cell, put, terminal_designation, terminal_wired,
    wires_by_cp, words_cell,
};
use super::hole_table::surfaces_in_page_order;
use super::parts::part_number_cell;
use super::terminal_diagram::strips_of;
use super::terminal_overview::first_segment_position;
use crate::kind::ReportKindDef;
use crate::rowset::{Cell, CellType, ColumnDef, MissingReason, QtyUnit, Row, RowId, RowKey, RowKind, RowSet};
use crate::words::selecting;

/// The assembly list: for each surface a layout page shows, in the order of its first such page, each rail of it in
/// its legend's order as a block of its own (a heading that says where the rail lies and which end its positions start
/// from, its part, then everything on it from that end with the gaps between, a strip as one row) followed by the
/// blocks of the strips on it; then the surface's devices placed without a rail, with their centres and holes. Then
/// the strips on no rail, and per cabinet the devices not placed, each as a lack. A strip's block: a heading row that
/// says where the strip goes and how long it is, a row per item in mounting order, and its bridges under a heading of
/// their own. A block that chooses its strips lists those and the rails they stand on. Row IDs start with the thing:
/// a rail's heading `(rail, "heading")`, its part `(rail, "rail")`, an item `(rail, n)`, a gap `(rail, "gap n")`; a
/// surface's `(surface, "devices")` and `(surface, n)`; a cabinet's `(space, "unplaced")` and `(space, n)`; a strip's
/// heading `(strip, "heading")`, an item `(strip, n)`, the bridges' heading `(strip, "bridges")` and a bridge `(strip,
/// "bridge", group)`.
#[derive(Clone, Copy, Debug)]
pub struct AssemblyList;

impl ReportKindDef for AssemblyList {
    fn kind(&self) -> ReportKind {
        ReportKind::AssemblyList
    }

    fn workshop(&self) -> bool {
        true
    }

    fn columns(&self) -> Vec<ColumnDef> {
        vec![
            ColumnDef::new("pos", CellType::Number, text!("column-pos")).weighted(3),
            ColumnDef::new("item", CellType::Text, text!("column-item")).weighted(9),
            ColumnDef::new("device", CellType::Ident, text!("column-device")).weighted(8),
            ColumnDef::new("terminal", CellType::Ident, text!("column-terminal")).weighted(5),
            ColumnDef::new("part_number", CellType::Ident, text!("column-part-number")).weighted(12),
            ColumnDef::new("description", CellType::Text, text!("column-description")).weighted(34),
            ColumnDef::new("colour", CellType::Ident, text!("column-colour")).weighted(5),
            ColumnDef::new("width", CellType::Qty, text!("column-width-mm")).weighted(10),
            ColumnDef::new("position", CellType::Text, text!("column-position-mm")).weighted(10),
            ColumnDef::new("marker", CellType::Ident, text!("column-marker")).weighted(5),
            ColumnDef::new("note", CellType::Text, text!("column-note")).weighted(17),
        ]
    }

    fn rows(&self, q: &Q, block: &ReportBlock) -> RowSet {
        let mut set = RowSet::new(self.columns());
        let mut strips = strips_of(q, block);
        // `strips_of` orders by tag; the stable sort keeps that order among the strips of one page.
        strips.sort_by_cached_key(|s| first_segment_position(q, *s));
        let strips: Vec<StripAssembly> = strips.into_iter().filter_map(|s| assembly_rows(q, s)).collect();
        // A block that chooses its strips lists those and the rails they stand on; every other block every rail,
        // every device placed without one and every device not placed.
        let chosen = !block.manual.include.is_empty();
        let ctx = StripContext::of(q);
        let mut listed: BTreeSet<Id<Device>> = BTreeSet::new();
        for (surface, folio) in surfaces_in_page_order(q) {
            for (rail, label) in carrier_labels(q, surface) {
                if label.kind != CarrierLabelKind::Rail {
                    continue;
                }
                let Some(r) = rail_assembly(q, rail) else {
                    continue;
                };
                let mut on: Vec<&StripAssembly> = strips
                    .iter()
                    .filter(|a| a.on.as_ref().is_some_and(|o| o.rail == rail))
                    .collect();
                on.sort_by_key(|a| a.on.as_ref().map(|o| o.from));
                if chosen && on.is_empty() {
                    continue;
                }
                rail_block(q, &mut set, &r, &folio);
                for a in on {
                    listed.insert(a.strip);
                    strip_block(q, &mut set, a, &ctx);
                }
            }
            if !chosen {
                surface_block(q, &mut set, surface, &folio);
            }
        }
        for a in strips.iter().filter(|a| !listed.contains(&a.strip)) {
            strip_block(q, &mut set, a, &ctx);
        }
        if !chosen {
            unplaced_blocks(q, &mut set);
        }
        set
    }
}

/// What every strip's block reads: the terminal markers, the wires by CP, the bridges.
struct StripContext {
    markers: BTreeMap<Id<Device>, String>,
    wires: Wires,
    bridges: Vec<Bridge>,
}

impl StripContext {
    fn of(q: &Q) -> StripContext {
        StripContext {
            markers: terminal_markers(q),
            wires: Wires::of(q),
            bridges: bridges(q),
        }
    }
}

/// A strip's block (doc 13 §6.1): its heading, a row per item in mounting order, its bridges under a heading of their
/// own.
fn strip_block(q: &Q, set: &mut RowSet, a: &StripAssembly, ctx: &StripContext) {
    let strip = a.strip;
    let by_cp = wires_by_cp(&ctx.wires);
    set.rows.push(heading(
        q,
        &set.columns,
        RowId::entity(strip).with(RowKey::Name("heading".to_owned())),
        &heading_words(q, a),
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
                dev.as_ref()
                    .and_then(|d| strip_bracket(q, d))
                    .and_then(|b| match (b.from, b.declared) {
                        (BracketFrom::PartUse(_), _) | (_, None) => None,
                        (_, Some(d)) => with_words(q, &[d.with]),
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
            &ctx.markers,
            note,
        ));
    }
    let own: Vec<&Bridge> = ctx.bridges.iter().filter(|b| b.strip == strip).collect();
    if own.is_empty() {
        return;
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

/// From–to in millimetres as a row prints it: "25–70".
fn from_to(q: &Q, from: i128, to: i128) -> Cell {
    measured_cell(
        q,
        &Text::bare(TextArg::Plain(format!(
            "{}\u{2013}{}",
            mm_tenth_short(from),
            mm_tenth_short(to)
        ))),
    )
}

/// A length in millimetres to the tenth as the Width cell prints it.
fn width_cell(nm: i128) -> Cell {
    let tenths = ew_units::arith::div_round_ties_away(nm, 100_000);
    Cell::Qty(
        Dec3(i64::try_from(tenths.saturating_mul(100)).unwrap_or(i64::MAX)),
        QtyUnit::None,
    )
}

/// The part number and description of `part`, or the lack of a part.
fn part_cells(q: &Q, columns: &[ColumnDef], cells: &mut [Cell], part: Option<&PartRef>) {
    match part.and_then(|p| ew_derive::printed_part(q, p)) {
        Some(copy) => {
            put(
                columns,
                cells,
                "part_number",
                part_number_cell(copy.item.part_number.as_str()),
            );
            put(
                columns,
                cells,
                "description",
                Cell::Text(copy.item.description.clone()),
            );
        }
        None => put(columns, cells, "part_number", Cell::Missing(MissingReason::Part)),
    }
}

/// A data row of `cells` whose source is `source`.
fn sourced(id: RowId, cells: Vec<Cell>, source: ew_schema::AnyId) -> Row {
    let mut row = Row::data(id, cells);
    row.sources = vec![source];
    row
}

/// A rail's block (doc 13 §6.1): its heading (where it lies, its cut length, the end its positions start from), its
/// part, then everything on it from its start with the gaps between, a strip as one row that names its own block.
fn rail_block(q: &Q, set: &mut RowSet, r: &RailAssembly, folio: &str) {
    let surface = face_of(q, r.surface).map_or_else(
        || ew_derive::text::surface_word(&SurfaceKind::Unknown(serde_json::Value::Null), None, 1),
        |f| ew_derive::text::surface_word(&f.kind, f.door, f.doors),
    );
    let corner = q
        .table::<MountingSurface>()
        .get(r.surface)
        .map(|s| surface_datum(q, s))
        .unwrap_or_default();
    let across = matches!(r.start, RailStart::Left | RailStart::Right);
    let words = selecting(key!("cell-rail-heading"))
        .with(
            "rail",
            TextArg::Text(ew_derive::text::carrier_label(CarrierLabel {
                kind: CarrierLabelKind::Rail,
                n: r.number,
            })),
        )
        .with("surface", TextArg::Text(surface))
        .with("folio", folio.to_owned())
        .with(
            "cut",
            r.cut.map_or_else(|| "none".to_owned(), ew_derive::mm_exact),
        )
        .with("axis", if across { "Y" } else { "X" })
        .with("across", mm_tenth_short(r.across))
        .with("along", if across { "X" } else { "Y" })
        .with("lo", mm_tenth_short(r.ends.0))
        .with("hi", mm_tenth_short(r.ends.1))
        .with(
            "corner",
            TextArg::Text(ew_derive::text::corner_in_sentence(&corner)),
        )
        .with("start", TextArg::Text(ew_derive::text::rail_start(r.start)));
    let id = |key: RowKey| RowId::entity(r.rail).with(key);
    set.rows.push(heading(
        q,
        &set.columns,
        id(RowKey::Name("heading".to_owned())),
        &words,
    ));
    let columns = set.columns.clone();
    let mut cells = vec![Cell::Empty; columns.len()];
    put(&columns, &mut cells, "item", words_cell(&text!("cell-rail")));
    part_cells(q, &columns, &mut cells, r.part.as_ref());
    set.rows
        .push(sourced(id(RowKey::Name("rail".to_owned())), cells, r.rail.cast()));
    let gap = |set: &mut RowSet, n: u32, from: i128, to: i128, note: Option<Text>| {
        let mut cells = vec![Cell::Empty; columns.len()];
        put(&columns, &mut cells, "item", words_cell(&text!("cell-gap")));
        put(&columns, &mut cells, "width", width_cell(to - from));
        put(&columns, &mut cells, "position", from_to(q, from, to));
        if let Some(note) = note {
            put(&columns, &mut cells, "note", words_cell(&note));
        }
        set.rows
            .push(Row::data(id(RowKey::Name(format!("gap {n}"))), cells));
    };
    let mut at = 0;
    for (n, item) in r.items.iter().enumerate() {
        let n32 = u32::try_from(n).unwrap_or(u32::MAX);
        if item.from > at {
            gap(
                set,
                n32,
                at,
                item.from,
                (at == 0).then(|| text!("cell-gap-from-start")),
            );
        }
        let strip = q
            .devices()
            .get(item.device)
            .is_some_and(|d| d.aspects.strip.is_some());
        let tag = short_tag(q, item.device);
        let mut cells = vec![Cell::Empty; columns.len()];
        put(&columns, &mut cells, "pos", Cell::Number(i64::from(n32) + 1));
        let kind = if strip {
            text!("cell-strip")
        } else {
            text!("cell-device")
        };
        put(&columns, &mut cells, "item", words_cell(&kind));
        put(&columns, &mut cells, "device", Cell::ident(&tag));
        if strip {
            put(
                &columns,
                &mut cells,
                "note",
                words_cell(&text!("cell-strip-block", tag)),
            );
        } else {
            part_cells(q, &columns, &mut cells, item.part.as_ref());
        }
        let width = if item.unknown {
            words_cell(&text!("cell-width-missing"))
        } else {
            width_cell(item.to - item.from)
        };
        put(&columns, &mut cells, "width", width);
        // Without a width the interval is the page's placeholder: only its centre is where the device stands.
        let position = if item.unknown {
            let centre = mm_tenth_short(item.from + (item.to - item.from) / 2);
            measured_cell(q, &text!("cell-centre-at", centre))
        } else {
            from_to(q, item.from, item.to)
        };
        put(&columns, &mut cells, "position", position);
        set.rows
            .push(sourced(id(RowKey::Index(n32)), cells, item.device.cast()));
        at = at.max(item.to);
    }
    if r.length > at {
        gap(set, u32::MAX, at, r.length, Some(text!("cell-gap-to-end")));
    }
}

/// A surface's block of the devices placed on it without a rail (doc 13 §6.1): its heading, then each device with its
/// centre from the datum corner and the holes it sits in, by the hole table's numbers. Nothing for a surface without
/// such a device.
fn surface_block(q: &Q, set: &mut RowSet, surface: Id<MountingSurface>, folio: &str) {
    let items = surface_items(q, surface);
    let (Some(s), Some(face)) = (q.table::<MountingSurface>().get(surface), face_of(q, surface)) else {
        return;
    };
    if items.is_empty() {
        return;
    }
    let words = text!(
        "cell-surface-heading",
        ew_derive::text::surface_word(&face.kind, face.door, face.doors),
        folio,
        ew_derive::text::surface_seen(&face.kind),
        ew_derive::text::corner_in_sentence(&surface_datum(q, s))
    );
    let id = |key: RowKey| RowId::entity(surface).with(key);
    set.rows.push(heading(
        q,
        &set.columns,
        id(RowKey::Name("devices".to_owned())),
        &words,
    ));
    let columns = set.columns.clone();
    for (n, item) in items.iter().enumerate() {
        let n32 = u32::try_from(n).unwrap_or(u32::MAX);
        let mut cells = vec![Cell::Empty; columns.len()];
        put(&columns, &mut cells, "pos", Cell::Number(i64::from(n32) + 1));
        put(&columns, &mut cells, "item", words_cell(&text!("cell-device")));
        put(
            &columns,
            &mut cells,
            "device",
            Cell::ident(&short_tag(q, item.device)),
        );
        part_cells(q, &columns, &mut cells, item.part.as_ref());
        let width = item.size.map_or_else(
            || words_cell(&text!("cell-width-missing")),
            |(w, _)| width_cell(w),
        );
        put(&columns, &mut cells, "width", width);
        let at = text!("cell-at-xy", mm_tenth_short(item.at.0), mm_tenth_short(item.at.1));
        put(&columns, &mut cells, "position", measured_cell(q, &at));
        if !item.holes.is_empty() {
            let holes = item
                .holes
                .iter()
                .map(|h| format!("H{h}"))
                .collect::<Vec<_>>()
                .join(", ");
            let count = TextArg::Number(i64::try_from(item.holes.len()).unwrap_or(i64::MAX));
            let words = selecting(key!("cell-in-holes"))
                .with("count", count)
                .with("holes", holes);
            put(&columns, &mut cells, "note", words_cell(&words));
        }
        set.rows
            .push(sourced(id(RowKey::Index(n32)), cells, item.device.cast()));
    }
}

/// Per cabinet, the devices of its location that have no place in its layout (`unplaced`), each said as a lack.
fn unplaced_blocks(q: &Q, set: &mut RowSet) {
    let columns = set.columns.clone();
    let mut spaces: Vec<Id<LayoutSpace>> = q.table::<LayoutSpace>().ids().collect();
    spaces.sort();
    for space in spaces {
        let unplaced = q.get::<Unplaced>(space);
        if unplaced.is_empty() {
            continue;
        }
        let id = |key: RowKey| RowId::entity(space).with(key);
        let words = text!("cell-unplaced-heading", cabinet_name(q, space));
        set.rows.push(heading(
            q,
            &columns,
            id(RowKey::Name("unplaced".to_owned())),
            &words,
        ));
        for (n, (device, part)) in unplaced.iter().enumerate() {
            let mut cells = vec![Cell::Empty; columns.len()];
            put(&columns, &mut cells, "item", words_cell(&text!("cell-device")));
            put(
                &columns,
                &mut cells,
                "device",
                Cell::ident(&short_tag(q, *device)),
            );
            part_cells(q, &columns, &mut cells, part.as_ref());
            put(
                &columns,
                &mut cells,
                "position",
                words_cell(&text!("cell-not-placed")),
            );
            let key = RowKey::Index(u32::try_from(n).unwrap_or(u32::MAX));
            set.rows.push(sourced(id(key), cells, device.cast()));
        }
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
    for l in q
        .get::<LabelRows>(())
        .iter()
        .filter(|l| l.set == LabelSet::TerminalMarkers)
    {
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
