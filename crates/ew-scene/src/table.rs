// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `tables` layer of a generated report page: the house report table of doc 20 §16.
//!
//! The caller (which holds the tracked read context) fills a [`TableView`] with the **printed** texts of
//! one page's rows: the cell values are typed `Cell`s of `ew-reports` rendered late (doc 09 §3.6), so this
//! module only draws strings. [`lower_table`] is a pure function of the view, the form's page-to-sheet
//! map and the font set.
//!
//! # Geometry
//!
//! The table stands where the frame's list area puts it ([`ListArea`], doc 09 §3.8), in printed (sheet)
//! millimetres, converted to page coordinates with the inverse of the frame form's map ([`SheetMap::unapply`]); on
//! the starter A4 frame that is the house form of the reference drawing: table x 16.31 to 277.27 mm, top edge at
//! 15.01 mm. Header and data rows [`ROW_HEIGHT`] (6.52 mm), header fill [`HEADER_FILL`] (#C0C0C0), grid
//! lines [`LINE_WIDTH`] (0.183 mm), text style `text.normal` (8.88 pt), left aligned and centred on the row's
//! midline, inset by [`CELL_PADDING`]. Column widths split the table width by the columns' weights
//! (the integer rounding of a cumulative share, so the edges are exact and the last edge is the right
//! edge). Where a text of the page would be cut (see "Overflow"), the columns with room to spare give it to the
//! columns that cut, up to the width their widest text takes at the full size ([`widened`]).
//!
//! A caption ([`TableView::caption`]) stands in its own row above the header in style [`CAPTION_STYLE`], a legend
//! ([`TableView::legend`]) in a row directly under the last row drawn, in the rows' style; both left aligned, without
//! lines, shrunk, cut and reported as a cell is.
//!
//! # Paint order
//!
//! The header fill; then the grid, each line once: the outline, the row separators and the column
//! separators (a heading row, which spans every column, has none); then the caption, the header texts, the row
//! texts in row order and the legend. Only the rows given are drawn: the table ends below the last row, as on the
//! reference.
//!
//! # Overflow
//!
//! Text that does not fit its cell shrinks to a printed cap height of [`MIN_CAP_HEIGHT`]; still too wide, it is cut
//! with an ellipsis at that size (doc 09 §7 item 5) and reported ([`TableFinding::Overflow`], carried by
//! [`PageFinding::Table`](crate::page::PageFinding::Table)). A cell never prints over its neighbour, and never
//! silently (FR-REP-039).
//!
//! # Row links
//!
//! A [`TableLink`] of a data cell without a span carries a [`Link::Source`] to its destination on the cell's text,
//! which the PDF writes as a link annotation over the text (FR-REP-042). A link with a span (one listed item of the
//! cell, the "101" of "101, 103") is drawn as a cross-reference text's item link is (ADR-0015 amendment 9): one
//! invisible path over the box of that span's glyphs, after the cell's text items, carrying the link; the text
//! items then carry none. Not drawn yet: group header and footer templates (a group row prints as a heading row).

use std::ops::Range;

use ew_dl::{Brush, Color, Destination, Item, Link, Path, Stroke};
use ew_schema::{ListArea, Overflow, ShrinkSpec};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyle, TextStyleTable};
use ew_units::{Nm, Pt2, Rect};

use crate::forms::{BoxSpec, CELL_PADDING, OverflowKind, SheetMap, lay_out, min_size};
use crate::page::span_quads;

/// The height of the header and of every row, printed (doc 20 §16).
pub const ROW_HEIGHT: Nm = ew_schema::HOUSE_ROW_HEIGHT;
/// The grid line width, printed.
pub const LINE_WIDTH: Nm = Nm(183_000);
/// The header fill.
pub const HEADER_FILL: Color = Color::from_rgb8(192, 192, 192);
/// The style of a caption: the title style of the embedded boxes.
pub const CAPTION_STYLE: &str = crate::embedded::BOX_TITLE_STYLE;
/// The smallest printed cap height of shrink-to-fit (doc 09 §2.5; the 1.8 mm of REF-088).
pub const MIN_CAP_HEIGHT: Nm = Nm(1_800_000);

/// One column of the table.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableColumn {
    /// The printed header.
    pub header: String,
    /// The relative width; a column with weight 0 takes no width (and draws nothing).
    pub weight: u32,
}

/// The kind of a printed row.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TableRowKind {
    /// A data row: one text per column.
    #[default]
    Data,
    /// A heading (also a group header or footer): the first text spans every column.
    Heading,
}

/// One printed row.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableRow {
    /// The kind.
    pub kind: TableRowKind,
    /// One text per column (a heading row uses the first).
    pub cells: Vec<String>,
    /// The links of the row's cells (FR-REP-042).
    pub links: Vec<TableLink>,
}

/// One link of a table cell (FR-REP-042).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableLink {
    /// The column of the cell.
    pub column: usize,
    /// The byte span of the linked item in the cell's text, or `None` for the whole text.
    pub span: Option<Range<usize>>,
    /// Where the link goes.
    pub dest: Destination,
    /// The cell is where links to `dest` land ([`Link::Target`]: a legend row its device's label links to, doc 12
    /// §5.4), not a link itself.
    pub target: bool,
}

impl TableLink {
    /// A link over the whole text of the cell in `column`.
    pub fn whole(column: usize, dest: Destination) -> TableLink {
        TableLink {
            column,
            span: None,
            dest,
            target: false,
        }
    }
}

/// The table of one generated page.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TableView {
    /// The columns.
    pub columns: Vec<TableColumn>,
    /// The rows of the page, in order.
    pub rows: Vec<TableRow>,
    /// A text printed in one row spanning every column instead of `rows` (the placeholder of a row
    /// source that is not derived yet, or an empty block's text).
    pub note: Option<String>,
    /// The line above the header (the block's caption), `None` for none; drawn where the list area has a caption row.
    pub caption: Option<String>,
    /// The line under the last row drawn (the block's legend), `None` for none.
    pub legend: Option<String>,
}

/// What lowering a table found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TableFinding {
    /// A text does not fit its cell cleanly (FR-REP-039).
    Overflow {
        /// The row (0-based, in `rows`; `None` for the header or the note).
        row: Option<usize>,
        /// The column.
        column: usize,
        /// The full text.
        text: String,
        /// What happened.
        kind: OverflowKind,
    },
    /// More rows than the page's list area holds ([`ListArea::rows`]): the first `drawn` are drawn.
    TooManyRows {
        /// The rows given.
        given: usize,
        /// The rows drawn.
        drawn: usize,
    },
    /// Characters without a glyph in the font set (FR-OUT-027).
    MissingGlyphs {
        /// The text.
        text: String,
        /// The characters, each once.
        chars: Vec<char>,
    },
}

/// The lowered table: items in page coordinates and what to report.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LoweredTable {
    /// The items in paint order.
    pub items: Vec<Item>,
    /// The findings in paint order.
    pub findings: Vec<TableFinding>,
}

/// The column edges of a table spanning `left` to `right`: `columns.len() + 1` values split by weight, each edge the
/// integer rounding of its cumulative share, the last exactly `right` (the house rule, shared with the embedded tables
/// of [`crate::embedded`]).
pub fn column_edges_in(columns: &[TableColumn], left: Nm, right: Nm) -> Vec<Nm> {
    let total: i128 = columns.iter().map(|c| i128::from(c.weight)).sum();
    let width = i128::from(right.0 - left.0);
    let mut edges = Vec::with_capacity(columns.len() + 1);
    edges.push(left);
    let mut acc: i128 = 0;
    for c in columns {
        acc += i128::from(c.weight);
        let x = if total == 0 { 0 } else { width * acc / total };
        edges.push(Nm(left.0 + i64::try_from(x).unwrap_or(0)));
    }
    if let Some(last) = edges.last_mut()
        && total > 0
    {
        *last = right;
    }
    edges
}

/// The house cell rule (shared with [`crate::embedded`]): `s` left aligned in `rect` (page coordinates), centred on its
/// midline and inset by [`CELL_PADDING`], shrunk to the printed cap height [`MIN_CAP_HEIGHT`] when it does not fit, then
/// cut with an ellipsis at that size and reported ([`TableFinding::Overflow`] at `at`,
/// `(row, column)`; a cut text carries no span links); characters without a glyph are
/// reported too. An empty text draws nothing. `links` are the cell's links: the first without a span is set on every
/// item of the text; each with a span adds an invisible path over that span's glyphs ([`span_quads`]) carrying it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_cell(
    out: &mut LoweredTable,
    fonts: &FontSet,
    style: &TextStyle,
    sheet: SheetMap,
    rect: Rect,
    s: &str,
    at: (Option<usize>, usize),
    links: &[&TableLink],
) {
    if s.is_empty() {
        return;
    }
    let shrink = Overflow::ShrinkToFit(ShrinkSpec {
        min_cap_height: MIN_CAP_HEIGHT,
        extra: Default::default(),
    });
    let ellipsis = Overflow::Ellipsis;
    let spec = |overflow| BoxSpec {
        rect,
        halign: -1,
        valign: 0,
        padding: CELL_PADDING,
        overflow: Some(overflow),
        max_lines: 1,
        sheet,
        pixel: None,
    };
    let mut laid = lay_out(fonts, s, style, &spec(&shrink));
    // Still too wide at the smallest size: cut with an ellipsis at that size (doc 09 §7 item 5), so that a cell never
    // prints over its neighbour. Two lines are no way out: at the smallest size their pitch exceeds the row.
    let cut = laid.overflow == Some(OverflowKind::ShrinkLimit);
    if cut {
        let floor = style.with_size(laid.shaped.run.size);
        laid = lay_out(fonts, s, &floor, &spec(&ellipsis));
    }
    if !laid.shaped.missing.is_empty() {
        out.findings.push(TableFinding::MissingGlyphs {
            text: s.to_owned(),
            chars: laid.shaped.missing.clone(),
        });
    }
    if let Some(kind) = laid.overflow {
        out.findings.push(TableFinding::Overflow {
            row: at.0,
            column: at.1,
            text: s.to_owned(),
            kind,
        });
    }
    // The style at the size the text was laid out in (shrink-to-fit may have made it smaller).
    let final_style = style.with_size(laid.shaped.run.size);
    // One call for all the cell's spans: the boxes come from the drawn shaping, which `span_quads` cannot redo (it takes
    // no fonts).
    // A cut text has no boxes for the spans of the whole one.
    let spanned: Vec<(&Range<usize>, Destination)> = links
        .iter()
        .filter(|_| !cut)
        .filter_map(|l| Some((l.span.as_ref()?, l.dest)))
        .collect();
    let ranges: Vec<&Range<usize>> = spanned.iter().map(|(r, _)| *r).collect();
    let carriers: Vec<Item> = span_quads(
        fonts.vertical_metrics(&final_style),
        s,
        &laid.shaped,
        &laid.items,
        &ranges,
    )
    .into_iter()
    .zip(&spanned)
    .filter_map(|(quad, (_, dest))| {
        Some(Item::path(Path::polygon(&quad?), None, None).with_link(Link::Source(*dest)))
    })
    .collect();
    match links.iter().find(|l| l.span.is_none()) {
        Some(l) => {
            let link = if l.target {
                Link::Target(l.dest)
            } else {
                Link::Source(l.dest)
            };
            out.items
                .extend(laid.items.into_iter().map(|i| i.with_link(link)));
        }
        None => out.items.extend(laid.items),
    }
    out.items.extend(carriers);
}

/// The column edges with room given to a column whose text would be cut (the parts list's descriptions read
/// "PLC CPU with PROFINET and …" in a smaller size beside columns with room to spare). Where a text of the page's
/// header or data rows would be cut with an ellipsis at the smallest size in its column, each column whose texts
/// all fit with room to spare gives some of it, in proportion to its spare room, so that the cut columns get the
/// width their widest text takes at the full size (in proportion to what they lack, as far as the spare room goes);
/// where the room spared at the full size is not enough, the cut columns get what they lack at the smallest size from
/// the room the others spare at that size. Without a cut text the edges stay those of the weights.
fn widened(
    view: &TableView,
    edges: Vec<Nm>,
    area: &ListArea,
    fonts: &FontSet,
    style: &TextStyle,
    sheet: SheetMap,
) -> Vec<Nm> {
    if view.note.is_some() || view.columns.len() < 2 {
        return edges;
    }
    let max_rows = usize::try_from(area.rows).unwrap_or(usize::MAX);
    let top = area.header_top();
    let texts = |c: usize| {
        std::iter::once(view.columns[c].header.as_str()).chain(
            view.rows
                .iter()
                .take(max_rows)
                .filter(|r| r.kind == TableRowKind::Data)
                .filter_map(move |r| r.cells.get(c).map(String::as_str)),
        )
    };
    let pad = 2 * CELL_PADDING.0;
    let n = view.columns.len();
    let width = |c: usize| edges[c + 1].0 - edges[c].0;
    // The widest text of each column in `style`, with its padding.
    let widest = |c: usize, style: &TextStyle| {
        texts(c)
            .filter(|s| !s.is_empty())
            .map(|s| fonts.shape(s, style, None).advance().0 + pad)
            .max()
            .unwrap_or(0)
    };
    // At the full size, and at the smallest size shrink-to-fit goes to.
    let need: Vec<i64> = (0..n).map(|c| widest(c, style)).collect();
    let floor = style.with_size(min_size(fonts, style, MIN_CAP_HEIGHT, sheet).min(style.size));
    let need_floor: Vec<i64> = (0..n).map(|c| widest(c, &floor)).collect();
    let cut = |c: usize| {
        let rect = Rect::from_points(
            sheet.unapply(Pt2::new(edges[c], top)),
            sheet.unapply(Pt2::new(edges[c + 1], Nm(top.0 + ROW_HEIGHT.0))),
        );
        let shrink = Overflow::ShrinkToFit(ShrinkSpec {
            min_cap_height: MIN_CAP_HEIGHT,
            extra: Default::default(),
        });
        let spec = BoxSpec {
            rect,
            halign: -1,
            valign: 0,
            padding: CELL_PADDING,
            overflow: Some(&shrink),
            max_lines: 1,
            sheet,
            pixel: None,
        };
        texts(c).any(|s| {
            !s.is_empty() && lay_out(fonts, s, style, &spec).overflow == Some(OverflowKind::ShrinkLimit)
        })
    };
    let cuts: Vec<bool> = (0..n).map(|c| view.columns[c].weight > 0 && cut(c)).collect();
    let lacking_at = |need: &[i64]| -> Vec<i64> {
        (0..n)
            .map(|c| if cuts[c] { (need[c] - width(c)).max(0) } else { 0 })
            .collect()
    };
    let spare_at = |need: &[i64]| -> Vec<i64> {
        (0..n)
            .map(|c| {
                if view.columns[c].weight > 0 && !cuts[c] {
                    (width(c) - need[c]).max(0)
                } else {
                    0
                }
            })
            .collect()
    };
    let mut lacking = lacking_at(&need);
    let mut lack: i64 = lacking.iter().sum();
    if lack == 0 {
        return edges;
    }
    let mut spare = spare_at(&need);
    // Where the room the other columns spare at the full size is not enough (a table of many columns, whose texts
    // print smaller than the full size), each cut column takes what it lacks at the smallest size, from the room the
    // others spare at that size: no text is cut while the columns hold them all at that size (FT-1923).
    if spare.iter().sum::<i64>() < lack {
        lacking = lacking_at(&need_floor);
        lack = lacking.iter().sum();
        spare = spare_at(&need_floor);
    }
    let room: i64 = spare.iter().sum();
    let moved = lack.min(room);
    if moved == 0 {
        return edges;
    }
    let share = |part: i64, whole: i64| {
        i64::try_from(i128::from(moved) * i128::from(part) / i128::from(whole)).unwrap_or(0)
    };
    let mut out = Vec::with_capacity(n + 1);
    out.push(edges[0]);
    for c in 0..n {
        let grow = if lacking[c] > 0 {
            share(lacking[c], lack)
        } else {
            0
        };
        let give = if spare[c] > 0 { share(spare[c], room) } else { 0 };
        let last = *out.last().unwrap_or(&edges[0]);
        out.push(Nm(last.0 + width(c) + grow - give));
    }
    // The rounding's remainder stays in the last column: the right edge is the table's.
    if let Some(last) = out.last_mut() {
        *last = edges[n];
    }
    out
}

/// Lowers `view` (see the module documentation) into `area`, the list area of the page's frame. `sheet` is the frame
/// form's page-to-sheet map; the rows print in the default style of `styles`.
pub fn lower_table(
    view: &TableView,
    area: &ListArea,
    sheet: SheetMap,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> LoweredTable {
    let mut out = LoweredTable::default();
    if view.columns.is_empty() {
        return out;
    }
    let style = styles.resolve_or_default(DEFAULT_STYLE_KEY);
    let (left, right, top) = (area.left, area.right, area.header_top());

    let edges: Vec<Nm> = widened(
        view,
        column_edges_in(&view.columns, left, right),
        area,
        fonts,
        &style,
        sheet,
    );
    // The table never runs below the form's data area: rows beyond the page's capacity are not drawn and
    // reported, instead of relying on the pagination's row count (FR-REP-039).
    let given = if view.note.is_some() { 1 } else { view.rows.len() };
    let row_count = given.min(usize::try_from(area.rows).unwrap_or(usize::MAX));
    if given > row_count {
        out.findings.push(TableFinding::TooManyRows {
            given,
            drawn: row_count,
        });
    }
    let bottom = Nm(top.0 + ROW_HEIGHT.0 * (i64::try_from(row_count).unwrap_or(0) + 1));
    let to_page = |x: Nm, y: Nm| sheet.unapply(Pt2::new(x, y));
    let rect = |x0: Nm, y0: Nm, x1: Nm, y1: Nm| Rect::from_points(to_page(x0, y0), to_page(x1, y1));
    let stroke = Stroke::solid(sheet.unscale_len(LINE_WIDTH));
    let line = |x0: Nm, y0: Nm, x1: Nm, y1: Nm| {
        Item::path(
            Path::line(to_page(x0, y0), to_page(x1, y1)),
            Some(stroke.clone()),
            None,
        )
    };
    let row_y = |i: usize| Nm(top.0 + ROW_HEIGHT.0 * i64::try_from(i).unwrap_or(0));

    // Header fill, then the grid.
    out.items.push(Item::path(
        Path::rect(rect(left, top, right, row_y(1))),
        None,
        Some(Brush::Solid(HEADER_FILL)),
    ));
    out.items.push(Item::path(
        Path::rect(rect(left, top, right, bottom)),
        Some(stroke.clone()),
        None,
    ));
    for i in 1..=row_count {
        out.items.push(line(left, row_y(i), right, row_y(i)));
    }
    let spans: Vec<bool> = (0..=row_count)
        .map(|i| {
            // Row 0 is the header; a heading or note row spans.
            if i == 0 {
                false
            } else if view.note.is_some() {
                true
            } else {
                view.rows[i - 1].kind == TableRowKind::Heading
            }
        })
        .collect();
    for (k, &x) in edges
        .iter()
        .enumerate()
        .skip(1)
        .take(edges.len().saturating_sub(2))
    {
        if view.columns[k - 1].weight == 0 {
            continue;
        }
        // The separator is drawn in runs of rows that do not span.
        let mut i = 0;
        while i <= row_count {
            if spans[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i <= row_count && !spans[i] {
                i += 1;
            }
            out.items.push(line(x, row_y(start), x, row_y(i)));
        }
    }

    // Texts: the caption, the header, then the rows (or the note), then the legend.
    let styled = |out: &mut LoweredTable,
                  s: &str,
                  style: &TextStyle,
                  (x0, x1, y0): (Nm, Nm, Nm),
                  at: (Option<usize>, usize),
                  links: &[&TableLink]| {
        let cell = rect(x0, y0, x1, Nm(y0.0 + ROW_HEIGHT.0));
        push_cell(out, fonts, style, sheet, cell, s, at, links);
    };
    let text = |out: &mut LoweredTable,
                s: &str,
                x0: Nm,
                x1: Nm,
                y0: Nm,
                at: (Option<usize>, usize),
                links: &[&TableLink]| styled(out, s, &style, (x0, x1, y0), at, links);
    if let Some(caption) = view.caption.as_deref().filter(|_| area.caption) {
        let caption_style = styles.resolve(CAPTION_STYLE).unwrap_or(style);
        styled(
            &mut out,
            caption,
            &caption_style,
            (left, right, area.top),
            (None, 0),
            &[],
        );
    }
    for (c, col) in view.columns.iter().enumerate() {
        if col.weight > 0 {
            text(
                &mut out,
                &col.header,
                edges[c],
                edges[c + 1],
                row_y(0),
                (None, c),
                &[],
            );
        }
    }
    if let Some(note) = &view.note {
        text(&mut out, note, left, right, row_y(1), (None, 0), &[]);
    } else {
        for (r, row) in view.rows.iter().take(row_count).enumerate() {
            let y = row_y(r + 1);
            match row.kind {
                TableRowKind::Heading => {
                    let s = row.cells.first().map_or("", String::as_str);
                    text(&mut out, s, left, right, y, (Some(r), 0), &[]);
                }
                TableRowKind::Data => {
                    for (c, col) in view.columns.iter().enumerate() {
                        if col.weight == 0 {
                            continue;
                        }
                        let s = row.cells.get(c).map_or("", String::as_str);
                        let links: Vec<&TableLink> = row.links.iter().filter(|l| l.column == c).collect();
                        text(&mut out, s, edges[c], edges[c + 1], y, (Some(r), c), &links);
                    }
                }
            }
        }
    }
    if let Some(legend) = &view.legend {
        text(
            &mut out,
            legend,
            left,
            right,
            row_y(row_count + 1),
            (None, 0),
            &[],
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use ew_dl::ItemKind;
    use ew_text::{Shaped, bundled};

    /// The oracle: the span box as it was computed before, with one shaping of the line's text up to each
    /// span end.
    fn span_quad_per_end(
        fonts: &FontSet,
        style: &TextStyle,
        text: &str,
        shaped: &Shaped,
        lines: &[Item],
        range: &Range<usize>,
    ) -> Option<[Pt2; 4]> {
        let m = fonts.vertical_metrics(style);
        let (k, line) = shaped
            .lines
            .iter()
            .enumerate()
            .find(|(_, l)| l.text.start <= range.start && range.end <= l.text.end)?;
        let travel = |end: usize| {
            let prefix = text.get(line.text.start..end)?;
            Some(fonts.shape(prefix, style, None).advance())
        };
        let (x0, x1) = (travel(range.start)?, travel(range.end)?);
        let to_parent = |q: Pt2| match &lines.get(k)?.kind {
            ItemKind::Glyphs(run) => Some(run.to_parent(q)),
            ItemKind::Group(g) => Some(g.transform.unwrap_or(ew_dl::Transform::IDENTITY).apply(q)),
            _ => None,
        };
        let corner = |x: Nm, y: Nm| to_parent(Pt2::new(x, y));
        Some([
            corner(x0, Nm(-m.ascent.0))?,
            corner(x0, m.descent)?,
            corner(x1, m.descent)?,
            corner(x1, Nm(-m.ascent.0))?,
        ])
    }

    /// A Folios text of 300 pages as `folios_spans` prints it, lists and runs mixed, and the spans of its linked pages
    /// (a run's two ends; "–" is 3 bytes).
    fn folios_300() -> (String, Vec<Range<usize>>) {
        let mut text = String::new();
        let mut spans = Vec::new();
        let mut page = 100;
        let mut listed = 0;
        let mut n = 0;
        while listed < 300 {
            if !text.is_empty() {
                text.push_str(", ");
            }
            let first = page.to_string();
            spans.push(text.len()..text.len() + first.len());
            text.push_str(&first);
            if n % 5 == 4 {
                // A run of four pages: its two ends span.
                let last = (page + 3).to_string();
                text.push('–');
                spans.push(text.len()..text.len() + last.len());
                text.push_str(&last);
                page += 3;
                listed += 4;
            } else {
                listed += 1;
            }
            page += 2;
            n += 1;
        }
        (text, spans)
    }

    /// One box per span, `None` where a span has none.
    type Quads = Vec<Option<[Pt2; 4]>>;

    /// `text` laid into `rect` as `push_cell` lays a cell (left-aligned), with the style at the size it was laid out in.
    fn lay(text: &str, rect: Rect, max_lines: usize) -> (Vec<Item>, Shaped, TextStyle) {
        let style = ew_text::builtin()
            .resolve(DEFAULT_STYLE_KEY)
            .expect("the default style");
        let shrink = Overflow::ShrinkToFit(ShrinkSpec {
            min_cap_height: MIN_CAP_HEIGHT,
            extra: Default::default(),
        });
        let spec = BoxSpec {
            rect,
            halign: -1,
            valign: 0,
            padding: CELL_PADDING,
            overflow: Some(&shrink),
            max_lines,
            sheet: SheetMap::IDENTITY,
            pixel: None,
        };
        let laid = lay_out(bundled(), text, &style, &spec);
        let style = style.with_size(laid.shaped.run.size);
        (laid.items, laid.shaped, style)
    }

    /// The span boxes of `text` laid into `rect` (as `push_cell` lays a cell) by `span_quads` and by the oracle.
    fn both(text: &str, spans: &[Range<usize>], rect: Rect, max_lines: usize) -> (Quads, Quads) {
        let fonts = bundled();
        let (items, shaped, style) = lay(text, rect, max_lines);
        let ranges: Vec<&Range<usize>> = spans.iter().collect();
        let new = span_quads(fonts.vertical_metrics(&style), text, &shaped, &items, &ranges);
        let old = spans
            .iter()
            .map(|r| span_quad_per_end(fonts, &style, text, &shaped, &items, r))
            .collect();
        (new, old)
    }

    /// The drawn extent of the glyphs of `span` in `text` laid into `rect`, through its line item, read from the drawn
    /// shaping alone: the glyphs of the span's line whose clusters start from the span's own cluster up to its end, each
    /// reaching to the next glyph's x (the last to the line's advance). `None` when the span covers no glyph.
    fn drawn_extent((items, shaped): (&[Item], &Shaped), span: &Range<usize>) -> Option<(Nm, Nm)> {
        let (k, line) = shaped
            .lines
            .iter()
            .enumerate()
            .find(|(_, l)| l.text.start <= span.start && span.end <= l.text.end)?;
        let ids: Vec<usize> = line.glyphs.clone().collect();
        let lo = ids
            .iter()
            .map(|&g| shaped.clusters[g])
            .filter(|&c| c <= span.start)
            .max()
            .unwrap_or(span.start);
        let inside: Vec<usize> = ids
            .iter()
            .copied()
            .filter(|&g| (lo..span.end).contains(&shaped.clusters[g]))
            .collect();
        let x = |g: usize| shaped.run.glyphs[g].x;
        let right = |g: usize| {
            if g + 1 < line.glyphs.end {
                x(g + 1)
            } else {
                line.advance
            }
        };
        let x0 = inside.iter().map(|&g| x(g)).min()?;
        let x1 = inside.iter().map(|&g| right(g)).max()?;
        let ItemKind::Glyphs(run) = &items[k].kind else {
            panic!("one face per line");
        };
        let at = |x: Nm| run.to_parent(Pt2::new(x, Nm::ZERO)).x;
        Some((at(x0), at(x1)))
    }

    fn mm(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        let n = |v: i64| Nm(v * 1_000_000);
        Rect::from_points(Pt2::new(n(x0), n(y0)), Pt2::new(n(x1), n(y1)))
    }

    /// The x extent of a box (the tests' cells are not turned).
    fn xs(q: &Option<[Pt2; 4]>) -> (Nm, Nm) {
        let q = q.expect("a box");
        (q[0].x, q[2].x)
    }

    /// In digits (no kerning, one glyph per character) every box of a 300-folio cell equals the one a
    /// shaping per span end gives, in a cell that shrinks it and in one wide enough for it.
    #[test]
    fn the_spans_of_a_300_folio_cell_equal_a_shaping_per_span_end() {
        let (text, spans) = folios_300();
        assert!(text.contains("100, 102, 104, 106, 108–111, 113"), "{text}");
        for rect in [mm(0, 0, 30, 6), mm(0, 0, 20_000, 6)] {
            let (new, old) = both(&text, &spans, rect, 1);
            assert!(new.iter().all(Option::is_some));
            assert_eq!(new, old);
        }
    }

    /// A span's box is where its glyphs are drawn, kerning included. In "LT, Y, r, 7" the box of "L" ends at
    /// the x of the "T" kerned towards it, the box of "Y" at the "," kerned towards it, and the box of "r" at its ","
    /// likewise; each ends before the advance of its text shaped alone up to the span's end.
    #[test]
    fn a_kerned_pair_ends_the_box_at_the_next_glyph() {
        let text = "LT, Y, r, 7";
        let spans = [0..1, 4..5, 7..8];
        let (new, old) = both(text, &spans, mm(0, 0, 500, 6), 1);
        let style = ew_text::builtin()
            .resolve(DEFAULT_STYLE_KEY)
            .expect("the default style");
        let line = bundled().shape(text, &style, None);
        let widths: Vec<Nm> = new.iter().map(|q| Nm(xs(q).1.0 - xs(q).0.0)).collect();
        let drawn: Vec<Nm> = spans
            .iter()
            .map(|r| Nm(line.run.glyphs[r.end].x.0 - line.run.glyphs[r.start].x.0))
            .collect();
        assert_eq!(widths, drawn);
        assert_eq!(widths, [Nm(1_600_200), Nm(1_606_550), Nm(803_275)]);
        let alone: Vec<Nm> = old.iter().map(|q| Nm(xs(q).1.0 - xs(q).0.0)).collect();
        assert_eq!(alone, [Nm(1_663_700), Nm(1_797_050), Nm(1_311_275)]);
    }

    /// A span gives the same box whatever else its line holds: a "fi" ligature, a CJK character or a
    /// combining mark later in the line change nothing.
    #[test]
    fn a_span_keeps_its_box_next_to_a_ligature_or_a_cjk_character() {
        let spans = [0..1, 4..5, 7..8, 10..11];
        let (plain, _) = both("LT, Y, r, 7", &spans, mm(0, 0, 500, 6), 1);
        for text in [
            "LT, Y, r, 7, fi",
            "LT, Y, r, 7, \u{4e2d}",
            "LT, Y, r, 7, e\u{301}",
        ] {
            let (new, _) = both(text, &spans, mm(0, 0, 500, 6), 1);
            assert_eq!(new, plain, "{text:?}");
        }
    }

    /// A 300-folio line with a CJK folio label among its digits: every span has its box at its glyphs' drawn
    /// extent, the boxes follow each other along the line, and the digit labels' boxes are those a shaping per end gives
    /// (no kerning between digits). `span_quads` takes no fonts, so it cannot shape per span end.
    #[test]
    fn a_folio_line_with_a_cjk_label_takes_its_boxes_from_the_drawn_glyphs() {
        let (digits, spans) = folios_300();
        let label = "\u{4e2d}7";
        let text = format!("{label}, {digits}");
        let shift = label.len() + 2;
        let spans: Vec<Range<usize>> = std::iter::once(0..label.len())
            .chain(spans.iter().map(|r| r.start + shift..r.end + shift))
            .collect();
        let (new, old) = both(&text, &spans, mm(0, 0, 20_000, 6), 1);
        let (items, shaped, _) = lay(&text, mm(0, 0, 20_000, 6), 1);
        let drawn: Vec<Option<(Nm, Nm)>> = spans.iter().map(|s| drawn_extent((&items, &shaped), s)).collect();
        assert_eq!(new.iter().map(|q| Some(xs(q))).collect::<Vec<_>>(), drawn);
        assert!(new.windows(2).all(|w| xs(&w[0]).1 <= xs(&w[1]).0));
        assert_eq!(new[1..], old[1..]);
    }

    /// A span inside a cluster covers the whole cluster: "f" of a "fi" ligature the ligature's glyph, "e" of "é" written
    /// with a combining mark the base and its mark; a span across a line break has no box.
    #[test]
    fn a_span_inside_a_cluster_covers_the_whole_cluster() {
        let text = "1.2 /3.4\nfi e\u{301}f";
        let spans = [0..3, 9..10, 9..11, 12..13, 12..15, 15..16, 6..12];
        let (new, _) = both(text, &spans, mm(0, 0, 500, 40), 3);
        assert!(new[..6].iter().all(Option::is_some));
        assert_eq!(new[1], new[2]);
        assert_eq!(new[3], new[4]);
        assert_eq!(new[6], None);
    }

    /// A cross-reference-like text of two lines that starts right-to-left is drawn with one base direction:
    /// its first line right-aligned in the run, its second reordered ("90" drawn first). Every span's box is its glyphs'
    /// drawn extent, not where its line would lie shaped alone.
    #[test]
    fn a_right_to_left_first_text_boxes_its_spans_where_they_are_drawn() {
        let text = "\u{5d0}\u{5d1} 1\n12345678 90";
        let spans = [0..2, 2..4, 0..4, 5..6, 7..15, 16..18];
        let (new, _) = both(text, &spans, mm(0, 0, 500, 40), 2);
        let (items, shaped, _) = lay(text, mm(0, 0, 500, 40), 2);
        let drawn: Vec<Option<(Nm, Nm)>> = spans.iter().map(|s| drawn_extent((&items, &shaped), s)).collect();
        assert!(drawn.iter().all(Option::is_some));
        assert_eq!(new.iter().map(|q| Some(xs(q))).collect::<Vec<_>>(), drawn);
        // The second line is reordered: "90" is drawn left of "12345678".
        assert!(xs(&new[5]).1 <= xs(&new[4]).0);
    }

    /// In a right-to-left line a span's box covers its glyphs' extent ("\u{5d0}" is drawn right of
    /// "\u{5d1}"), the two letters' boxes together make the word's, and a span that holds no glyph has no box.
    #[test]
    fn a_right_to_left_span_covers_its_glyphs_and_an_empty_span_has_no_box() {
        let text = "\u{5d0}\u{5d1} 7";
        let spans = [0..2, 2..4, 0..4, 2..2];
        let (new, _) = both(text, &spans, mm(0, 0, 500, 6), 1);
        let (alef, bet, word) = (xs(&new[0]), xs(&new[1]), xs(&new[2]));
        assert!(bet.1 <= alef.0, "{alef:?} {bet:?}");
        assert_eq!(word, (bet.0, alef.1));
        assert_eq!(new[3], None);
    }

    fn cols(weights: &[u32]) -> Vec<TableColumn> {
        weights
            .iter()
            .map(|&weight| TableColumn {
                header: String::new(),
                weight,
            })
            .collect()
    }

    #[test]
    fn column_edges_span_the_table_and_follow_the_weights() {
        // The house table, 16.31 to 277.27 mm.
        let (left, right) = (Nm(16_310_000), Nm(277_270_000));
        let column_edges = |c: &[TableColumn]| column_edges_in(c, left, right);
        let e = column_edges(&cols(&[1, 1, 2]));
        assert_eq!(e.first(), Some(&left));
        assert_eq!(e.last(), Some(&right));
        let w = right.0 - left.0;
        assert_eq!(e[1].0 - e[0].0, w / 4);
        assert_eq!(e[3].0 - e[2].0, w - w / 2);
        // Doc 09 §2.5: 16.31 | 79.73 | 214.90 | 277.27 mm.
        let toc = cols(&[63_420, 135_170, 62_370]);
        let e = column_edges(&toc);
        assert_eq!(e[1], Nm(79_730_000));
        assert_eq!(e[2], Nm(214_900_000));
        // D17's first flow column, 108.00 to 191.00 mm at 10 : 22 : 16 : 35: 10, 22, 16 and 35 mm.
        let e = column_edges_in(&cols(&[10, 22, 16, 35]), Nm(108_000_000), Nm(191_000_000));
        assert_eq!(
            e,
            [108_000_000, 118_000_000, 140_000_000, 156_000_000, 191_000_000].map(Nm)
        );
        // Zero weights and no columns do not panic.
        assert_eq!(column_edges(&[]), vec![left]);
        let e = column_edges(&cols(&[0, 0]));
        assert_eq!(e, vec![left, left, left]);
    }
}
