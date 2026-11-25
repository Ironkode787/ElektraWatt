// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The describe text of a page: its wires, its terms in sentences, its table lines and the page described whole.

#[allow(clippy::wildcard_imports)]
use super::*;

/// A label or title of the page text as it is quoted: a double quote becomes a single one and a line break a space.
pub(super) fn page_quoted(text: &str) -> String {
    let t: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    format!("\"{}\"", t.replace('"', "'"))
}

/// `at X Y mm` of a page point in page nm, printed as the sheet point a pointer or the grid cursor reaches it at
/// (`canvas.press`, `canvas.pointer`, `cursor.set`; item 5): the page's own coordinates are offset from the
/// sheet's by the frame margin, so a page coordinate printed as is missed every click by that margin.
fn page_at(page: &CanvasPage, p: (i64, i64)) -> String {
    let s = page.snapper.page_to_sheet(p);
    format!("at {} {} mm", mm3(s.0), mm3(s.1))
}

/// The candidate points of [`wire_point`] along a segment, in 32nds: the middle first, then outward (the nearer to the
/// middle first, the lower first).
const FRACTIONS: [i64; 31] = [
    16, 15, 17, 14, 18, 13, 19, 12, 20, 11, 21, 10, 22, 9, 23, 8, 24, 7, 25, 6, 26, 5, 27, 4, 28, 3, 29, 2,
    30, 1, 31,
];

/// A sheet point on the drawn polyline of `wire` that the Select tool's press picks as this wire (the pick rule where routes overlap),
/// so a reader of the page text can hover, select or delete it: with the pick reach
/// at 50 % zoom (else at 90 %), the wire is first in the pick stack there ([`crate::tools::pick_stack`]) and no other
/// connection is in it, and no link is under it (a press on a link follows it). The candidates are the points at n/32 of each segment ([`FRACTIONS`]: the middle first, then
/// outward), longer segments first (ties: the segment's order); `None` when no candidate is unambiguous (an unrouted
/// line that runs along other wires for its whole length; a repeated click there walks the stack).
fn wire_point(page: &CanvasPage, wire: &ew_api::output::TableConnection) -> Option<(i64, i64)> {
    let own: Vec<(i64, i64)> = wire
        .polyline
        .iter()
        .map(|p| page.snapper.page_to_sheet(*p))
        .collect();
    let mut segments: Vec<(i128, usize)> = own
        .windows(2)
        .enumerate()
        .map(|(i, w)| (crate::tools::dist2(w[0], w[1]), i))
        .filter(|(len, _)| *len > 0)
        .collect();
    segments.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let wires: std::collections::BTreeSet<_> = page.tables.connections.iter().map(|c| c.id).collect();
    let me = ew_dl::EntityRef(wire.id);
    // The pick reach of 4 points at 50 % and at 90 % zoom, nm (1 pt = 25.4 mm / 72).
    for reach in [25_400_000 * 4 * 2 / 72, 25_400_000 * 4 * 10 / 72 / 9] {
        for &(_, i) in &segments {
            let (a, b) = (own[i], own[i + 1]);
            for n in FRACTIONS {
                let p = (a.0 + (b.0 - a.0) * n / 32, a.1 + (b.1 - a.1) * n / 32);
                let stack = crate::tools::pick_stack(page, &page.snapper, p, f64::from(reach));
                let alone = stack.iter().filter(|e| wires.contains(&e.0)).count() == 1;
                let link =
                    crate::tools::link_at(&page.links, p, crate::pointer::HOVER_SLOP_PT / f64::from(reach))
                        .is_some();
                if stack.first() == Some(&me) && alone && !link {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// A terminal's tag (`-X1011:3`) as a sentence names it: "-X1011 terminal 3", so that the strip and the
/// number read apart. The designation `-X1011:3` stays in tables, Properties and printed labels; a tag without a colon
/// is returned as it is.
pub(crate) fn terminal_in_sentence(tag: &str) -> String {
    match tag.rsplit_once(':') {
        Some((strip, number)) if !strip.is_empty() && !number.is_empty() => {
            crate::strings::format(key!("terminal-in-sentence"), &[strip, number])
        }
        _ => tag.to_owned(),
    }
}

/// A connection point's label (`-1A4:-#2`) as a sentence names it: a designation the device
/// shares between pins carries its position in words, "-1A4 pin - (2nd)", never the label's `#2`. Any other label
/// (an unshared pin `-1A4:L+`, a terminal's clamp `-X1:1/a#2`) is returned as it is; tags,
/// tables and printed labels keep the label.
pub(crate) fn pin_in_sentence(label: &str) -> String {
    let Some((head, n)) = label.rsplit_once('#') else {
        return label.to_owned();
    };
    let Ok(n) = n.parse::<u32>() else {
        return label.to_owned();
    };
    match head.rsplit_once(':') {
        Some((tag, pin)) if !tag.is_empty() && !pin.is_empty() && !pin.contains('/') && n > 0 => {
            crate::strings::format(key!("pin-in-sentence"), &[tag, pin, &ordinal(n)])
        }
        _ => label.to_owned(),
    }
}

/// The English ordinal suffixes, for a number ending in 1, 2 or 3 and for every other number (the teens 11 to 13
/// take the last). English only for now: this table is what the shell's Fluent tables localise,
/// where a language's ordinal rule replaces the choice in [`ordinal`].
const ORDINAL_SUFFIXES: [&str; 4] = ["st", "nd", "rd", "th"];

/// An English ordinal: `1st`, `2nd`, `3rd`, `4th`, `11th`, `12th`, `13th`, `21st` ([`ORDINAL_SUFFIXES`]).
fn ordinal(n: u32) -> String {
    let [st, nd, rd, th] = ORDINAL_SUFFIXES;
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => th,
        (1, _) => st,
        (2, _) => nd,
        (3, _) => rd,
        _ => th,
    };
    format!("{n}{suffix}")
}

/// The variant letter of an orientation `0..=7`.
fn variant_letter(orient: u8) -> char {
    char::from(b'A' + orient.min(7))
}

/// The tag a representation names: its device's short tag, read off the labels of its connection points (a CP's
/// label is `tag:designation`, a terminal's `tag/clamp` or, without clamps, its tag alone, an interruption point's
/// its name); `None` without one. A terminal's tag is the strip's and its own designation (`-X1011:2`), so that two
/// terminals of one strip never read alike.
pub(crate) fn rep_tag(rep: &TableRep, cps: &[&TableCp]) -> Option<String> {
    let label = &cps.first()?.label;
    // A terminal's label is `-X1031:11/a` (its tag is `-X1031:11`), or `-X1011:2` when its function names no clamps
    // (a part's template); a device's is `-101F1:2` or `-101F1:2#2`.
    if let Some((tag, _clamp)) = label.rsplit_once('/') {
        return Some(tag.to_owned());
    }
    if rep.terminal {
        return Some(label.clone());
    }
    Some(label.split(':').next().unwrap_or(label).to_owned())
}

/// The reading key of a page point: its frame cell's row then column (a point outside every cell last), then `y`, `x`.
fn reading(page: &CanvasPage, p: (i64, i64)) -> (u32, u32, i64, i64) {
    let (row, column) = page
        .cells
        .as_ref()
        .and_then(|c| c.cell_at(page.snapper.page_to_sheet(p)))
        .map_or((u32::MAX, u32::MAX), |c| (c.row_index, c.column_index));
    (row, column, p.1, p.0)
}

/// The cell of a page point as the line prints it: the quoted cell text, or `none` outside the frame.
fn cell_word(page: &CanvasPage, p: (i64, i64)) -> String {
    page.cell_text(page.snapper.page_to_sheet(p))
        .map_or_else(|| "none".to_owned(), |c| page_quoted(&c))
}

/// The junctions of the page text in their numbered order (indices into the page tables' junctions): reading order of
/// the position, then style, wiring, the far-end CP labels of the sequence and the ID.
fn junction_order(page: &CanvasPage) -> Vec<usize> {
    let t: &PageTables = &page.tables;
    let cp_label = |function, idx| {
        t.cps
            .iter()
            .find(|c| c.function == function && c.idx == idx)
            .map_or_else(String::new, |c| c.label.clone())
    };
    let end_cp_only = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => cp_label(*function, *idx),
        TableEnd::Junction { .. } => String::new(),
    };
    let far_cp_labels = |j: &ew_api::output::TableJunction| -> Vec<String> {
        j.sequence
            .iter()
            .filter_map(|id| t.connections.iter().find(|c| c.id == *id))
            .map(|c| match (c.a, c.b) {
                (TableEnd::Junction { id }, other) if id == j.id => end_cp_only(&other),
                (other, _) => end_cp_only(&other),
            })
            .collect()
    };
    let mut order: Vec<usize> = (0..t.junctions.len()).collect();
    order.sort_by_cached_key(|i| {
        let j = &t.junctions[*i];
        // The IDs one transaction mints are not in creation order, so an ID decides only between junctions that the
        // position, the style, the wiring and the sequence cannot tell apart.
        (
            reading(page, j.at),
            j.style.clone(),
            j.wiring.clone(),
            far_cp_labels(j),
            j.id,
        )
    });
    order
}

/// The drawn connections of `page` with their two ends as the page text names them, without quotes: a CP label or
/// `junction <n>` (item 4: `select.set` takes `wire:<a> -> <b>`).
pub fn page_wires(page: &CanvasPage) -> Vec<(ew_dl::EntityRef, String, String)> {
    let t: &PageTables = &page.tables;
    let order = junction_order(page);
    let end = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map_or_else(String::new, |c| c.label.clone()),
        TableEnd::Junction { id } => format!(
            "junction {}",
            order
                .iter()
                .position(|i| t.junctions[*i].id == *id)
                .map_or(0, |n| n + 1)
        ),
    };
    t.connections
        .iter()
        .map(|c| (ew_dl::EntityRef(c.id), end(&c.a), end(&c.b)))
        .collect()
}

/// The lines `page.describe` adds for a generated page: its printed table, so a text-only reader can read a
/// parts list or any other list page.
///
/// ```text
/// table columns 3 rows 2
///   columns "Item" "Quantity" "Designation"
///   row "1" "2" "-101F1"
///   row group "Group heading"
/// note "No cables yet"                (instead of the rows, when the page prints a note)
/// ```
///
/// The cells are the folio's printed texts, quoted as user text; a heading row (a group header or footer) prints
/// `group` and its spanning text.
pub fn page_table_lines(table: &ew_api::output::PageListTable) -> Vec<String> {
    let mut lines = vec![format!(
        "table columns {} rows {}",
        table.columns.len(),
        table.rows.len()
    )];
    let quoted = |cells: &[String]| {
        cells
            .iter()
            .map(|c| format!("\"{}\"", user(c)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    if !table.columns.is_empty() {
        lines.push(format!("  columns {}", quoted(&table.columns)));
    }
    for row in &table.rows {
        if row.heading {
            let text = row.cells.first().map_or("", String::as_str);
            lines.push(format!("  row group \"{}\"", user(text)));
        } else {
            lines.push(format!("  row {}", quoted(&row.cells)));
        }
    }
    if let Some(note) = &table.note {
        lines.push(format!("note \"{}\"", user(note)));
    }
    lines
}

/// The text of one page for `page.describe`: the header with the exact counts, then the
/// representations with their connection points, the connections, the junctions, the page items and the structure
/// boxes, each in a named order (reading order of the anchor cell, then the position and the symbol key; connections
/// by their end labels; junctions in reading order; items and boxes in the page tables' order). The header's
/// `items <n> boxes <m>` follows `junctions <j>` only when the page has one of them, so a page without items prints
/// as before. No record ID appears. A representation whose bound pins print PLC addresses on the canvas ends its line
/// with `address "<pin>=<address>" …`.
pub fn page_describe(page: &CanvasPage) -> String {
    let t: &PageTables = &page.tables;
    let mut header = format!(
        "page {} {} reps {} connections {} junctions {}",
        page_quoted(&page.folio),
        page_quoted(&page.title),
        t.reps.len(),
        t.connections.len(),
        t.junctions.len()
    );
    if !t.items.is_empty() || !t.structure_boxes.is_empty() {
        header.push_str(&format!(
            " items {} boxes {}",
            t.items.len(),
            t.structure_boxes.len()
        ));
    }
    let mut lines = vec![header];

    // The junctions are numbered first: connections and sequences name them by number.
    let order = junction_order(page);
    let cp_label = |function, idx| {
        t.cps
            .iter()
            .find(|c| c.function == function && c.idx == idx)
            .map_or_else(String::new, |c| c.label.clone())
    };
    let number = |id| {
        order
            .iter()
            .position(|i| t.junctions[*i].id == id)
            .map_or(0, |n| n + 1)
    };
    let end_text = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => page_quoted(&cp_label(*function, *idx)),
        TableEnd::Junction { id } => format!("junction {}", number(*id)),
    };

    // The representations.
    let mut reps: Vec<_> = t
        .reps
        .iter()
        .map(|r| {
            let mut cps: Vec<&TableCp> = t.cps.iter().filter(|c| c.rep == r.id).collect();
            cps.sort_by(|a, b| (a.idx, &a.label).cmp(&(b.idx, &b.label)));
            let tag = rep_tag(r, &cps);
            let key = (
                reading(page, r.pos),
                r.symbol_key.clone(),
                tag.clone().unwrap_or_default(),
                r.orient,
            );
            (key, r, cps, tag)
        })
        .collect();
    reps.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, r, cps, tag) in &reps {
        let who = match (&r.ip, tag) {
            // Where the point continues, its printed cross-reference, when it has a partner.
            (Some(ip), _) if !ip.continuation.is_empty() => format!(
                "ip {} direction {} continues {}",
                page_quoted(&ip.name),
                ew_api::output::ip_direction_word(&ip.direction),
                page_quoted(&ip.continuation)
            ),
            (Some(ip), _) => format!(
                "ip {} direction {}",
                page_quoted(&ip.name),
                ew_api::output::ip_direction_word(&ip.direction)
            ),
            (None, Some(tag)) => page_quoted(tag),
            (None, None) => "none".to_owned(),
        };
        let mut line = format!(
            "  rep {who} cell {} symbol {} variant {} {}",
            cell_word(page, r.pos),
            page_quoted(&r.symbol_key),
            page_quoted(&variant_letter(r.orient).to_string()),
            page_at(page, r.pos)
        );
        // The PLC addresses the canvas prints beside the representation's bound pins, on
        // its own line as `address "<pin>=<address>" …` in CP order (the pin is the CP label after its last `:`).
        let addresses: Vec<String> = cps
            .iter()
            .filter_map(|cp| {
                let address = cp.plc_address.as_deref()?;
                let pin = cp.label.rsplit_once(':').map_or(cp.label.as_str(), |(_, p)| p);
                Some(page_quoted(&format!("{pin}={address}")))
            })
            .collect();
        if !addresses.is_empty() {
            line.push_str(" address ");
            line.push_str(&addresses.join(" "));
        }
        lines.push(line);
        for cp in cps {
            // G-PP triage row 21: a terminal CP names its side beside its clamp (`"1X1:1/a" internal`).
            let side = cp.side.map_or_else(String::new, |s| format!(" {s}"));
            let mut line = format!(
                "    cp {}{side} {} wires {}",
                page_quoted(&cp.label),
                page_at(page, cp.at),
                cp.connections.len()
            );
            if let Some(name) = cp.potential.as_ref().map(|p| &p.name) {
                line.push_str(&format!(" potential {}", page_quoted(name)));
            }
            lines.push(line);
        }
    }

    // The connections, by their end labels.
    let mut wires: Vec<(String, String, bool, usize, String)> = t
        .connections
        .iter()
        .map(|c| {
            let (a, b) = (end_text(&c.a), end_text(&c.b));
            // The vertices where the drawn polyline turns (a stored route is drawn as stored, so a vertex
            // it runs straight through is no bend).
            let corners: Vec<(i64, i64)> = c
                .polyline
                .windows(3)
                .filter(|w| {
                    let (u, v) = (
                        (w[1].0 - w[0].0, w[1].1 - w[0].1),
                        (w[2].0 - w[1].0, w[2].1 - w[1].1),
                    );
                    i128::from(u.0) * i128::from(v.1) != i128::from(u.1) * i128::from(v.0)
                        || i128::from(u.0) * i128::from(v.0) + i128::from(u.1) * i128::from(v.1) < 0
                })
                .map(|w| w[1])
                .collect();
            let bends = corners.len();
            // Where the wire runs: its corners in order from its first end, as `x,y` in sheet mm (the bend
            // count alone did not say whether a route ran round a symbol or along another wire).
            let via = if corners.is_empty() {
                String::new()
            } else {
                let points: Vec<String> = corners
                    .iter()
                    .map(|p| {
                        let s = page.snapper.page_to_sheet(*p);
                        format!("{},{}", mm3(s.0), mm3(s.1))
                    })
                    .collect();
                format!(" via {} mm", points.join(" "))
            };
            let route = if c.stored_route { "stored" } else { "unrouted" };
            // A point that picks this wire and nothing else (item 4), where there is one.
            let point =
                wire_point(page, c).map_or_else(String::new, |p| format!(" at {} {} mm", mm3(p.0), mm3(p.1)));
            (
                a.clone(),
                b.clone(),
                c.stored_route,
                bends,
                format!("  wire {a} -> {b} route {route} bends {bends}{via}{point}"),
            )
        })
        .collect();
    wires.sort();
    lines.extend(wires.into_iter().map(|w| w.4));

    // The junctions, numbered.
    for (n, i) in order.iter().enumerate() {
        let j = &t.junctions[*i];
        let members: Vec<String> = j
            .sequence
            .iter()
            .filter_map(|id| t.connections.iter().find(|c| c.id == *id))
            .map(|c| match (c.a, c.b) {
                (TableEnd::Junction { id }, other) if id == j.id => end_text(&other),
                (other, _) => end_text(&other),
            })
            .collect();
        let mut line = format!(
            "  junction {} cell {} {} style {} wiring {} sequence",
            n + 1,
            cell_word(page, j.at),
            page_at(page, j.at),
            j.style,
            j.wiring
        );
        for m in members {
            line.push(' ');
            line.push_str(&m);
        }
        lines.push(line);
    }

    // The page items, then the structure boxes, in the page tables' order.
    for item in &t.items {
        let word = match item.kind.as_str() {
            "graphic" => "graphic",
            "free_text" => "text",
            "wire_marker" => "marker",
            "image" => "image",
            // `ui.describe` names a surface view "surface view".
            "surface_view" => "surface view",
            _ => "unknown",
        };
        // G-PP triage row 12: the middle of the bounds, a point that picks the item, like a rep's or a wire's `at`.
        let [x0, y0, x1, y1] = item.bounds;
        let mid = (x0 + (x1 - x0) / 2, y0 + (y1 - y0) / 2);
        let mut line = format!(
            "  item {word} cell {} {}",
            cell_word(page, (x0, y0)),
            page_at(page, mid)
        );
        if matches!(word, "text" | "marker")
            && let Some(text) = &item.text
        {
            line.push(' ');
            line.push_str(&page_quoted(text));
        }
        lines.push(line);
    }
    for b in &t.structure_boxes {
        let top_left = b
            .outline
            .iter()
            .copied()
            .min_by_key(|(x, y)| (*y, *x))
            .unwrap_or_default();
        lines.push(format!(
            "  box structure cell {} {}",
            cell_word(page, top_left),
            page_quoted(&b.caption)
        ));
    }
    lines.join("\n")
}

/// The bridge lines `page.describe` appends: one `  bridge "BR<n>" "<terminal>" …` per bridge group of which
/// the strip or a terminal is drawn on `page`, in `BR<n>` order, its terminals in (`seq`, ID) order. Bridges have no page (they are
/// defined in the strip editor), so the page lists the bridges of the strips it draws.
pub fn page_bridge_lines(page: &CanvasPage, bridges: &[ew_api::output::BridgeRow]) -> Vec<String> {
    bridges
        .iter()
        .filter(|b| {
            // A strip segment draws the strip's device; a terminal symbol draws the terminal's.
            page.tables.reps.iter().any(|r| {
                r.device
                    .is_some_and(|d| d == b.strip || b.terminals.iter().any(|(t, _)| *t == d))
            })
        })
        .map(|b| {
            let mut line = format!("  bridge {}", page_quoted(&b.label));
            for (_, name) in &b.terminals {
                line.push(' ');
                line.push_str(&page_quoted(name));
            }
            line
        })
        .collect()
}
