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
//! # Not drawn yet
//!
//! Row links (FR-REP-042: the PDF link annotations and canvas jumps of cells) and group header and footer
//! templates: a group row prints as a heading row.

use ew_dl::{Brush, Color, Item, Path, Stroke};
use ew_schema::{Overflow, ShrinkSpec};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyle, builtin};
use ew_units::{Nm, Pt2, Rect};

use crate::forms::{BoxSpec, CELL_PADDING, OverflowKind, SheetMap, lay_out};

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
    let total: i128 = columns.iter().map(|c| i128::from(c.weight)).sum();
    let width = i128::from(RIGHT.0 - LEFT.0);
    let mut edges = Vec::with_capacity(columns.len() + 1);
    edges.push(LEFT);
    let mut acc: i128 = 0;
    for c in columns {
        acc += i128::from(c.weight);
        let x = if total == 0 { 0 } else { width * acc / total };
        edges.push(Nm(LEFT.0 + i64::try_from(x).unwrap_or(0)));
    }
    if let Some(last) = edges.last_mut()
        && total > 0
    {
        *last = RIGHT;
    }
    edges
}

/// Lowers `view` (see the module documentation). `sheet` is the frame form's page-to-sheet map.
pub fn lower_table(view: &TableView, sheet: SheetMap, fonts: &FontSet) -> LoweredTable {
    let mut out = LoweredTable::default();
    if view.columns.is_empty() {
        return out;
    }
    let style = builtin()
        .resolve(DEFAULT_STYLE_KEY)
        .unwrap_or(TextStyle::regular(Nm(3_175_000)));
    let shrink = Overflow::ShrinkToFit(ShrinkSpec {
        min_cap_height: MIN_CAP_HEIGHT,
        extra: Default::default(),
    });

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
    let text = |out: &mut LoweredTable, s: &str, x0: Nm, x1: Nm, y0: Nm, at: (Option<usize>, usize)| {
        if s.is_empty() {
            return;
        }
        let spec = BoxSpec {
            rect: rect(x0, y0, x1, Nm(y0.0 + ROW_HEIGHT.0)),
            halign: -1,
            valign: 0,
            padding: CELL_PADDING,
            overflow: Some(&shrink),
            max_lines: 1,
            sheet,
        };
        let laid = lay_out(fonts, s, &style, &spec);
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
        out.items.extend(laid.items);
    };
    for (c, col) in view.columns.iter().enumerate() {
        if col.weight > 0 {
            text(&mut out, &col.header, edges[c], edges[c + 1], row_y(0), (None, c));
        }
    }
    if let Some(note) = &view.note {
        text(&mut out, note, LEFT, RIGHT, row_y(1), (None, 0));
    } else {
        for (r, row) in view.rows.iter().take(row_count).enumerate() {
            let y = row_y(r + 1);
            match row.kind {
                TableRowKind::Heading => {
                    let s = row.cells.first().map_or("", String::as_str);
                    text(&mut out, s, LEFT, RIGHT, y, (Some(r), 0));
                }
                TableRowKind::Data => {
                    for (c, col) in view.columns.iter().enumerate() {
                        if col.weight == 0 {
                            continue;
                        }
                        let s = row.cells.get(c).map_or("", String::as_str);
                        text(&mut out, s, edges[c], edges[c + 1], y, (Some(r), c));
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
    fn zero_weights_and_no_columns_do_not_panic() {
        assert_eq!(column_edges(&[]), vec![LEFT]);
        let e = column_edges(&cols(&[0, 0]));
        assert_eq!(e, vec![LEFT, LEFT, LEFT]);
    }
}
