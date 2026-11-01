// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `tables` layer of a generated report page: the house report table of doc 20 §16 (FT-53).
//!
//! The caller (which holds the tracked read context) fills a [`TableView`] with the **printed** texts of
//! one page's rows: the cell values are typed [`ew_reports::Cell`]s rendered late (doc 09 §3.6), so this
//! module only draws strings. [`lower_table`] is a pure function of the view, the form's page-to-sheet
//! map and the font set.
//!
//! # Geometry
//!
//! The house form of the reference drawing, in printed (sheet) millimetres, converted to page coordinates
//! with the inverse of the frame form's map ([`SheetMap::unapply`]): table x 16.31 to 277.27 mm, top edge
//! at 15.01 mm, header and data rows [`ROW_HEIGHT`] (6.52 mm), header fill [`HEADER_FILL`] (#C0C0C0), grid
//! lines [`LINE_WIDTH`] (0.183 mm), text style `text.normal` (8.88 pt), left aligned and centred on the row's
//! midline, inset by [`CELL_PADDING`]. Column widths split the table width by the columns' weights
//! (the integer rounding of a cumulative share, so the edges are exact and the last edge is the right
//! edge).
//!
//! # Paint order
//!
//! The header fill; then the grid, each line once: the outline, the row separators and the column
//! separators (a heading row, which spans every column, has none); then the header texts and the row texts
//! in row order. Only the rows given are drawn: the table ends below the last row, as on the reference.
//!
//! # Overflow
//!
//! Text that does not fit its cell shrinks to a printed cap height of [`MIN_CAP_HEIGHT`] and is then
//! reported ([`TableFinding::Overflow`], carried by [`PageFinding::Table`](crate::page::PageFinding::Table)); it is never clipped
//! (FR-REP-039).
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
use ew_schema::{Overflow, ShrinkSpec};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyle, TextStyleTable};
use ew_units::{Nm, Pt2, Rect};

use crate::forms::{BoxSpec, CELL_PADDING, OverflowKind, SheetMap, lay_out};
use crate::page::span_quads;

/// The left edge of the table, printed.
pub const LEFT: Nm = Nm(16_310_000);
/// The right edge of the table, printed.
pub const RIGHT: Nm = Nm(277_270_000);
/// The top edge of the table (the top of the header row), printed.
pub const TOP: Nm = Nm(15_010_000);
/// The height of the header and of every row, printed (doc 20 §16).
pub const ROW_HEIGHT: Nm = Nm(6_520_000);
/// The grid line width, printed.
pub const LINE_WIDTH: Nm = Nm(183_000);
/// The header fill.
pub const HEADER_FILL: Color = Color::from_rgb8(192, 192, 192);
/// The most rows below the header the house form holds (doc 20 §16: 23 data rows per page).
pub const MAX_ROWS: usize = ew_reports::layout::HOUSE_ROWS_PER_PAGE as usize;
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

/// One link of a table cell (FR-REP-042; WP-S3.06 D10).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TableLink {
    /// The column of the cell.
    pub column: usize,
    /// The byte span of the linked item in the cell's text, or `None` for the whole text.
    pub span: Option<Range<usize>>,
    /// Where the link goes.
    pub dest: Destination,
}

impl TableLink {
    /// A link over the whole text of the cell in `column`.
    pub fn whole(column: usize, dest: Destination) -> TableLink {
        TableLink {
            column,
            span: None,
            dest,
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
    /// More rows than the house form holds ([`MAX_ROWS`]): the first `drawn` are drawn.
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

/// The column edges, printed: `columns.len() + 1` values from [`LEFT`] to [`RIGHT`], split by weight.
pub fn column_edges(columns: &[TableColumn]) -> Vec<Nm> {
    column_edges_in(columns, LEFT, RIGHT)
}

/// The column edges of a table spanning `left` to `right`: `columns.len() + 1` values split by weight, each edge the
/// integer rounding of its cumulative share, the last exactly `right` (the house rule of [`column_edges`], shared with
/// the embedded tables of [`crate::embedded`]).
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
/// midline and inset by [`CELL_PADDING`], shrunk to the printed cap height [`MIN_CAP_HEIGHT`] when it does not fit and
/// then reported ([`TableFinding::Overflow`] at `at`, `(row, column)`), never clipped; characters without a glyph are
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
    let spec = BoxSpec {
        rect,
        halign: -1,
        valign: 0,
        padding: CELL_PADDING,
        overflow: Some(&shrink),
        max_lines: 1,
        sheet,
    };
    let laid = lay_out(fonts, s, style, &spec);
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
    // One call for all the cell's spans: the line is shaped once, never once per span end (C44-p3#1).
    let spanned: Vec<(&Range<usize>, Destination)> = links
        .iter()
        .filter_map(|l| Some((l.span.as_ref()?, l.dest)))
        .collect();
    let ranges: Vec<&Range<usize>> = spanned.iter().map(|(r, _)| *r).collect();
    let carriers: Vec<Item> = span_quads(fonts, &final_style, s, &laid.shaped, &laid.items, &ranges)
        .into_iter()
        .zip(&spanned)
        .filter_map(|(quad, (_, dest))| {
            Some(Item::path(Path::polygon(&quad?), None, None).with_link(Link::Source(*dest)))
        })
        .collect();
    match links.iter().find(|l| l.span.is_none()) {
        Some(l) => out
            .items
            .extend(laid.items.into_iter().map(|i| i.with_link(Link::Source(l.dest)))),
        None => out.items.extend(laid.items),
    }
    out.items.extend(carriers);
}

/// Lowers `view` (see the module documentation). `sheet` is the frame form's page-to-sheet map; the rows print in
/// the default style of `styles`.
pub fn lower_table(
    view: &TableView,
    sheet: SheetMap,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> LoweredTable {
    let mut out = LoweredTable::default();
    if view.columns.is_empty() {
        return out;
    }
    let style = styles
        .resolve(DEFAULT_STYLE_KEY)
        .unwrap_or(TextStyle::regular(Nm(3_175_000)));

    let edges: Vec<Nm> = column_edges(&view.columns);
    // The table never runs below the form's data area: rows beyond the page's capacity are not drawn and
    // reported, instead of relying on the pagination's row count (FR-REP-039).
    let given = if view.note.is_some() { 1 } else { view.rows.len() };
    let row_count = given.min(MAX_ROWS);
    if given > row_count {
        out.findings.push(TableFinding::TooManyRows {
            given,
            drawn: row_count,
        });
    }
    let bottom = Nm(TOP.0 + ROW_HEIGHT.0 * (i64::try_from(row_count).unwrap_or(0) + 1));
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
    let row_y = |i: usize| Nm(TOP.0 + ROW_HEIGHT.0 * i64::try_from(i).unwrap_or(0));

    // Header fill, then the grid.
    out.items.push(Item::path(
        Path::rect(rect(LEFT, TOP, RIGHT, row_y(1))),
        None,
        Some(Brush::Solid(HEADER_FILL)),
    ));
    out.items.push(Item::path(
        Path::rect(rect(LEFT, TOP, RIGHT, bottom)),
        Some(stroke.clone()),
        None,
    ));
    for i in 1..=row_count {
        out.items.push(line(LEFT, row_y(i), RIGHT, row_y(i)));
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

    // Texts: the header, then the rows (or the note).
    let text = |out: &mut LoweredTable,
                s: &str,
                x0: Nm,
                x1: Nm,
                y0: Nm,
                at: (Option<usize>, usize),
                links: &[&TableLink]| {
        let cell = rect(x0, y0, x1, Nm(y0.0 + ROW_HEIGHT.0));
        push_cell(out, fonts, &style, sheet, cell, s, at, links);
    };
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
        text(&mut out, note, LEFT, RIGHT, row_y(1), (None, 0), &[]);
    } else {
        for (r, row) in view.rows.iter().take(row_count).enumerate() {
            let y = row_y(r + 1);
            match row.kind {
                TableRowKind::Heading => {
                    let s = row.cells.first().map_or("", String::as_str);
                    text(&mut out, s, LEFT, RIGHT, y, (Some(r), 0), &[]);
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use ew_dl::ItemKind;
    use ew_text::{Shaped, bundled};

    /// The oracle of C44-p3#1: the span box as it was computed before, with one shaping of the line's text up to each
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

    /// The span boxes of `text` laid into `rect` (as `push_cell` lays a cell) by `span_quads` and by the oracle.
    fn both(text: &str, spans: &[Range<usize>], rect: Rect, max_lines: usize) -> (Quads, Quads) {
        let fonts = bundled();
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
        };
        let laid = lay_out(fonts, text, &style, &spec);
        let style = style.with_size(laid.shaped.run.size);
        let ranges: Vec<&Range<usize>> = spans.iter().collect();
        let new = span_quads(fonts, &style, text, &laid.shaped, &laid.items, &ranges);
        let old = spans
            .iter()
            .map(|r| span_quad_per_end(fonts, &style, text, &laid.shaped, &laid.items, r))
            .collect();
        (new, old)
    }

    fn mm(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        let n = |v: i64| Nm(v * 1_000_000);
        Rect::from_points(Pt2::new(n(x0), n(y0)), Pt2::new(n(x1), n(y1)))
    }

    /// C44-p3#1: the spans of a 300-folio cell take their ends from one shaping of the line, and every box equals the
    /// one a shaping per span end gives, in a cell that shrinks it and in one wide enough for it.
    #[test]
    fn the_spans_of_a_300_folio_cell_equal_a_shaping_per_span_end() {
        let (text, spans) = folios_300();
        assert!(text.contains("100, 102, 104, 106, 108–111, 113"), "{text}");
        // Every character takes its pen travel from the line's one shaping: no span end shapes again.
        let style = ew_text::builtin()
            .resolve(DEFAULT_STYLE_KEY)
            .expect("the default style");
        assert_eq!(
            crate::page::pens_mapped(bundled(), &style, &text),
            text.chars().count()
        );
        for rect in [mm(0, 0, 30, 6), mm(0, 0, 20_000, 6)] {
            let (new, old) = both(&text, &spans, rect, 1);
            assert!(new.iter().all(Option::is_some));
            assert_eq!(new, old);
        }
    }

    /// C44-p3#1: a text of several lines and spans the shaping cannot map one to one (a combining mark, a ligature-prone
    /// pair, a right-to-left word) keep the boxes of a shaping per span end; a span across a line break has none.
    #[test]
    fn spans_over_several_lines_and_unmapped_text_equal_a_shaping_per_span_end() {
        let text = "1.2 /3.4\nfi e\u{301}f\n\u{5d0}\u{5d1} 7";
        let spans = [
            0..3,
            4..8,
            5..8,
            9..11,
            12..15,
            15..16,
            6..12,
            17..19,
            19..21,
            22..23,
        ];
        let (new, old) = both(text, &spans, mm(0, 0, 500, 40), 3);
        assert!(new[..6].iter().all(Option::is_some));
        assert_eq!(new[6], None);
        assert_eq!(new, old);
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
        let e = column_edges(&cols(&[1, 1, 2]));
        assert_eq!(e.first(), Some(&LEFT));
        assert_eq!(e.last(), Some(&RIGHT));
        let w = RIGHT.0 - LEFT.0;
        assert_eq!(e[1].0 - e[0].0, w / 4);
        assert_eq!(e[3].0 - e[2].0, w - w / 2);
    }

    #[test]
    fn the_toc_column_widths_of_the_reference_reproduce() {
        // Doc 09 §2.5: 16.31 | 79.73 | 214.90 | 277.27 mm.
        let e = column_edges(&cols(&[63_420, 135_170, 62_370]));
        assert_eq!(e[1], Nm(79_730_000));
        assert_eq!(e[2], Nm(214_900_000));
    }

    #[test]
    fn column_edges_in_a_span_follow_the_same_rule() {
        let c = cols(&[63_420, 135_170, 62_370]);
        assert_eq!(column_edges_in(&c, LEFT, RIGHT), column_edges(&c));
        // D17's first flow column, 108.00 to 191.00 mm at 10 : 22 : 16 : 35: 10, 22, 16 and 35 mm.
        let e = column_edges_in(&cols(&[10, 22, 16, 35]), Nm(108_000_000), Nm(191_000_000));
        assert_eq!(
            e,
            [108_000_000, 118_000_000, 140_000_000, 156_000_000, 191_000_000].map(Nm)
        );
    }

    #[test]
    fn zero_weights_and_no_columns_do_not_panic() {
        assert_eq!(column_edges(&[]), vec![LEFT]);
        let e = column_edges(&cols(&[0, 0]));
        assert_eq!(e, vec![LEFT, LEFT, LEFT]);
    }
}
