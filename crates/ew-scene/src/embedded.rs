// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded report blocks on a drawn page (FR-REP-028, doc 09 §2.2): the cover's colour and standard-size
//! tables (doc 20 §17.3) and, for every other kind, a boxed report table (WP-S4.01 D16: PART DATA, the layout
//! legend).
//!
//! The caller fills an [`EmbeddedView`] with the **printed** texts of the block; [`lower_embedded`] is a pure function
//! of the view, the page-to-sheet map of the frame form and the font set.
//!
//! # The boxed table ([`EmbeddedContent::Table`])
//!
//! House values in **printed** millimetres (like the generated pages' table, `crate::table`; the block's rectangle is
//! mapped to the sheet with the frame form's map and every item back to the page with its inverse):
//!
//! - a drop shadow of [`SHADOW`] (2.61 mm, doc 20 §17.3's PART DATA box) to the right and below, filled black and
//!   painted first; then the rectangle as a box at [`GRID_WIDTH`] (0.418 mm, the house width of the cover's boxes);
//! - with a title, a title band of 1.5 rows of [`ROW_HEIGHT`] (9.78 mm) in style [`BOX_TITLE_STYLE`] (15.5 pt);
//! - the width split in [`EmbeddedTable::flow_columns`] flow columns with a [`GUTTER`] of 4 mm between them, the last
//!   ending at the box's right edge; the rows fill the first column to its capacity, then the next;
//! - in each flow column that draws rows, with headers: a header row filled [`HEADER_FILL`], then data rows of
//!   [`ROW_HEIGHT`] (6.52 mm) in `text.normal`, the grid ([`LINE_WIDTH`], 0.183 mm: the outline, the row separators and
//!   the column separators, a heading row spanning) under the texts; without headers no inner grid (PART DATA);
//! - the columns split by weight with [`column_edges_in`], each text by the house cell rule (left aligned, shrunk to
//!   the 1.8 mm cap height, then reported, never clipped);
//! - a block whose rows do not derive prints its [`note`](EmbeddedTable::note) in one spanning row of the first flow
//!   column; rows beyond the capacity ([`boxed_capacity`]: whole rows of the space under the title band, less a header
//!   row, per flow column) are not drawn and are reported ([`TableFinding::TooManyRows`], FR-REP-039).
//!
//! # The cover ([`EmbeddedContent::Cover`])
//!
//! The block's
//! rectangle (`ReportBlock.embedded.rect`, page coordinates) is divided in rows of `u = height / 24`; the
//! proportions are those of the reference cover (printed 128.52 to 274.66 mm by 47.63 to 172.89 mm, doc 20
//! §17.3):
//!
//! | Region | x (of 560) | y |
//! |---|---|---|
//! | colour table | 0 to 560; columns 160, 180, 130, 90 | title `1.5 u`, header `1 u`, the data rows (15) of `1 u`, then the caption (`1 u`) |
//! | control sizes | 0 to 270; columns 170, 100 | from `19.5 u`: title `1.5 u`, then the rows (3) of `1 u` |
//! | mains sizes | 290 to 560; columns 170, 100 | as the control sizes |
//!
//! The note of the cover is drawn in style `data.small` (6.66 pt printed) in the row under the rectangle.
//!
//! A title is drawn in style `cover.table` (11.84 pt printed), every other text in `text.normal`, left aligned
//! and centred on the row's midline, inset by [`CELL_PADDING`](crate::forms::CELL_PADDING). The grid (the outline and the row and column
//! separators of the header and data rows, each line once) is [`GRID_WIDTH`] wide (0.418 mm printed, doc 20
//! §17.4: "cover boxes and tables"); titles and captions have no grid. A text that does not fit its cell
//! shrinks to the printed cap height [`MIN_CAP_HEIGHT`](crate::table::MIN_CAP_HEIGHT) and is then reported, never clipped (FR-REP-039).
//! Only the rows given are drawn, so the table ends below its last row.

use ew_dl::{Brush, Item, Path, Stroke};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyle, TextStyleTable};
use ew_units::{Nm, Pt2, Rect};

use crate::forms::SheetMap;
use crate::table::{
    HEADER_FILL, LINE_WIDTH, LoweredTable, ROW_HEIGHT, TableColumn, TableFinding, TableLink, TableRow,
    TableRowKind, column_edges_in, push_cell,
};

/// The grid line width, printed.
pub const GRID_WIDTH: Nm = Nm(418_000);

/// The style of a table title.
pub const TITLE_STYLE: &str = "cover.table";

/// The style of a boxed table's title (doc 20 §17.3: the PART DATA title, 15.53 pt printed).
pub const BOX_TITLE_STYLE: &str = "box.title";

/// The drop shadow of a boxed table, printed (doc 20 §17.3: "shadow 2.61").
pub const SHADOW: Nm = Nm(2_610_000);

/// The gutter between two flow columns of a boxed table, printed.
pub const GUTTER: Nm = Nm(4_000_000);

/// The most flow columns a boxed table draws; a larger count is drawn as this many.
pub const MAX_FLOW_COLUMNS: u32 = 16;

/// The largest coordinate magnitude of a block's page rectangle that [`lower_embedded`] and [`boxed_capacity`] take:
/// 2⁵³ − 1 nm, the bound a record can hold (`ew_commands::geometry::MAX_COORD`, ADR-0005). A rectangle read from a
/// hostile file is clamped to it, so the arithmetic on it stays inside `i64`.
const MAX_COORD: i64 = (1 << 53) - 1;

/// `rect` with each coordinate clamped to ±[`MAX_COORD`].
fn bounded(rect: Rect) -> Rect {
    let c = |n: Nm| Nm(n.0.clamp(-MAX_COORD, MAX_COORD));
    let (a, b) = (rect.min(), rect.max());
    Rect::from_points(Pt2::new(c(a.x), c(a.y)), Pt2::new(c(b.x), c(b.y)))
}

/// One table of an embedded block.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RegionTable {
    /// The title above the table.
    pub title: String,
    /// The column headers; empty for a table without a header row.
    pub header: Vec<String>,
    /// The data rows, one text per column.
    pub rows: Vec<Vec<String>>,
    /// The text under the table (the basis of a colour profile).
    pub caption: Option<String>,
}

/// The three tables of the cover block.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CoverTables {
    /// The colour table: voltage class, function, potential type, colour.
    pub colours: RegionTable,
    /// The standard sizes of the control class: colour, size.
    pub control: RegionTable,
    /// The standard sizes of the mains class: colour, size.
    pub mains: RegionTable,
    /// The note under the size tables, in the row below the block's rectangle (style `data.small`).
    pub note: Option<String>,
}

/// A boxed report table: an embedded block of any kind but the cover.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmbeddedTable {
    /// The title printed in the box's title band; empty: no title band.
    pub title: String,
    /// The printed columns: header text and relative width.
    pub columns: Vec<TableColumn>,
    /// Whether each flow column that draws rows starts with a header row (and draws its grid).
    pub headers: bool,
    /// The number of flow columns (0 is drawn as 1, more than [`MAX_FLOW_COLUMNS`] as that many).
    pub flow_columns: u32,
    /// The rows in order, one text per printed column (a heading row uses the first).
    pub rows: Vec<TableRow>,
    /// Printed in one spanning row instead of `rows`: why the block's rows do not derive.
    pub note: Option<String>,
}

/// What an embedded block draws.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EmbeddedContent {
    /// The colour and standard-size tables of the cover (`profile_tables`).
    Cover(Box<CoverTables>),
    /// A boxed report table (every other kind).
    Table(EmbeddedTable),
}

/// One embedded block on a page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmbeddedView {
    /// The block's rectangle in page coordinates (`ReportBlock.embedded.rect`).
    pub rect: Rect,
    /// The printed content.
    pub content: EmbeddedContent,
}

/// Lowers `view` (see the module documentation). `sheet` is the frame form's page-to-sheet map; texts print in the
/// styles of `styles`.
pub fn lower_embedded(
    view: &EmbeddedView,
    sheet: SheetMap,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> LoweredTable {
    let rect = bounded(view.rect);
    match &view.content {
        EmbeddedContent::Cover(cover) => lower_cover(rect, cover, sheet, fonts, styles),
        EmbeddedContent::Table(table) => lower_boxed(rect, table, sheet, fonts, styles),
    }
}

/// The capacity of a boxed table in `rect` (page coordinates) under the map `sheet`, as [`lower_embedded`] draws it:
/// `(flow columns, data rows per flow column)`. The rectangle is clamped to ±(2⁵³ − 1) nm first.
pub fn boxed_capacity(rect: Rect, table: &EmbeddedTable, sheet: SheetMap) -> (usize, usize) {
    let r = sheet.apply_rect(bounded(rect));
    let top = r.min().y.0 + title_band(table).0;
    let slots = usize::try_from((r.max().y.0 - top).max(0) / ROW_HEIGHT.0).unwrap_or(0);
    let flows = table.flow_columns.clamp(1, MAX_FLOW_COLUMNS) as usize;
    (flows, slots.saturating_sub(usize::from(draws_header(table))))
}

/// Whether the flow columns draw a header row: the table asks for one and has columns (a block whose rows do not
/// derive may have none, and then prints only its note).
fn draws_header(table: &EmbeddedTable) -> bool {
    table.headers && !table.columns.is_empty()
}

/// The height of the title band, printed: 1.5 rows with a title, else none.
fn title_band(table: &EmbeddedTable) -> Nm {
    if table.title.is_empty() {
        Nm(0)
    } else {
        Nm(ROW_HEIGHT.0 * 3 / 2)
    }
}

/// Lowers a boxed table (see the module documentation).
fn lower_boxed(
    rect: Rect,
    t: &EmbeddedTable,
    sheet: SheetMap,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> LoweredTable {
    let mut out = LoweredTable::default();
    // Printed coordinates of the box.
    let r = sheet.apply_rect(rect);
    let (x0, y0, x1, y1) = (r.min().x, r.min().y, r.max().x, r.max().y);
    let to_page = |x: Nm, y: Nm| sheet.unapply(Pt2::new(x, y));
    let page_rect = |ax: Nm, ay: Nm, bx: Nm, by: Nm| Rect::from_points(to_page(ax, ay), to_page(bx, by));
    let style = |key: &str| styles.resolve(key).unwrap_or(TextStyle::regular(Nm(3_175_000)));

    // The rows given and drawn.
    let (flows, per_flow) = boxed_capacity(rect, t, sheet);
    let given = if t.note.is_some() { 1 } else { t.rows.len() };
    let drawn = given.min(flows.saturating_mul(per_flow));
    if given > drawn {
        out.findings.push(TableFinding::TooManyRows { given, drawn });
    }

    // The drop shadow: the visible L of the box offset by the shadow, under the box (the project data box's shape).
    let s = SHADOW.0;
    let shadow = if s < r.width().0 && s < r.height().0 {
        Path::polygon(&[
            to_page(x1, Nm(y0.0 + s)),
            to_page(Nm(x1.0 + s), Nm(y0.0 + s)),
            to_page(Nm(x1.0 + s), Nm(y1.0 + s)),
            to_page(Nm(x0.0 + s), Nm(y1.0 + s)),
            to_page(Nm(x0.0 + s), y1),
            to_page(x1, y1),
        ])
    } else {
        Path::rect(page_rect(Nm(x0.0 + s), Nm(y0.0 + s), Nm(x1.0 + s), Nm(y1.0 + s)))
    };
    out.items.push(Item::path(shadow, None, Some(Brush::BLACK)));
    out.items.push(Item::path(
        Path::rect(page_rect(x0, y0, x1, y1)),
        Some(Stroke::solid(sheet.unscale_len(GRID_WIDTH))),
        None,
    ));

    // The title band.
    let top = Nm(y0.0 + title_band(t).0);
    let title_style = style(BOX_TITLE_STYLE);
    let cell = page_rect(x0, y0, x1, top);
    push_cell(
        &mut out,
        fonts,
        &title_style,
        sheet,
        cell,
        &t.title,
        (None, 0),
        &[],
    );

    // The flow columns.
    let flows_i = i64::try_from(flows).unwrap_or(1);
    let width = (x1.0 - x0.0 - GUTTER.0 * (flows_i - 1)).max(0) / flows_i;
    let text_style = style(DEFAULT_STYLE_KEY);
    let header = usize::from(draws_header(t));
    let row_y = |i: usize| Nm(top.0 + ROW_HEIGHT.0 * i64::try_from(i).unwrap_or(0));
    let stroke = Stroke::solid(sheet.unscale_len(LINE_WIDTH));
    let line = |ax: Nm, ay: Nm, bx: Nm, by: Nm| {
        Item::path(
            Path::line(to_page(ax, ay), to_page(bx, by)),
            Some(stroke.clone()),
            None,
        )
    };
    for k in 0..flows {
        let start = (k * per_flow).min(drawn);
        let end = ((k + 1) * per_flow).min(drawn);
        let count = end - start;
        // A flow column without rows draws nothing, except the first, which keeps its header.
        if count == 0 && (k > 0 || !draws_header(t)) {
            continue;
        }
        let k_i = i64::try_from(k).unwrap_or(0);
        // A box narrower than its gutters: no column starts right of the box, none ends left of its start.
        let left = Nm((x0.0 + k_i * (width + GUTTER.0)).min(x1.0));
        let right = if k + 1 == flows {
            x1
        } else {
            Nm((left.0 + width).min(x1.0))
        };
        let edges = column_edges_in(&t.columns, left, right);
        if draws_header(t) {
            // Whether row `i` of this flow column (0 the header) spans every column.
            let spans = |i: usize| -> bool {
                i > 0
                    && (t.note.is_some()
                        || t.rows
                            .get(start + i - 1)
                            .is_some_and(|r| r.kind == TableRowKind::Heading))
            };
            let rows = 1 + count;
            out.items.push(Item::path(
                Path::rect(page_rect(left, top, right, row_y(1))),
                None,
                Some(Brush::Solid(HEADER_FILL)),
            ));
            out.items.push(Item::path(
                Path::rect(page_rect(left, top, right, row_y(rows))),
                Some(stroke.clone()),
                None,
            ));
            for i in 1..rows {
                out.items.push(line(left, row_y(i), right, row_y(i)));
            }
            for (c, &x) in edges
                .iter()
                .enumerate()
                .skip(1)
                .take(edges.len().saturating_sub(2))
            {
                if t.columns[c - 1].weight == 0 {
                    continue;
                }
                // The separator is drawn in runs of rows that do not span.
                let mut i = 0;
                while i < rows {
                    if spans(i) {
                        i += 1;
                        continue;
                    }
                    let run = i;
                    while i < rows && !spans(i) {
                        i += 1;
                    }
                    out.items.push(line(x, row_y(run), x, row_y(i)));
                }
            }
            for (c, col) in t.columns.iter().enumerate() {
                if col.weight > 0 {
                    let cell = page_rect(edges[c], row_y(0), edges[c + 1], row_y(1));
                    push_cell(
                        &mut out,
                        fonts,
                        &text_style,
                        sheet,
                        cell,
                        &col.header,
                        (None, c),
                        &[],
                    );
                }
            }
        }
        if let Some(note) = &t.note {
            if count > 0 {
                let cell = page_rect(left, row_y(header), right, row_y(header + 1));
                push_cell(&mut out, fonts, &text_style, sheet, cell, note, (None, 0), &[]);
            }
            continue;
        }
        for (j, row) in t.rows[start..end].iter().enumerate() {
            let (ya, yb) = (row_y(header + j), row_y(header + j + 1));
            let at = start + j;
            match row.kind {
                TableRowKind::Heading => {
                    let s = row.cells.first().map_or("", String::as_str);
                    let cell = page_rect(left, ya, right, yb);
                    push_cell(&mut out, fonts, &text_style, sheet, cell, s, (Some(at), 0), &[]);
                }
                TableRowKind::Data => {
                    for (c, col) in t.columns.iter().enumerate() {
                        if col.weight == 0 {
                            continue;
                        }
                        let s = row.cells.get(c).map_or("", String::as_str);
                        let links: Vec<&TableLink> = row.links.iter().filter(|l| l.column == c).collect();
                        let cell = page_rect(edges[c], ya, edges[c + 1], yb);
                        push_cell(
                            &mut out,
                            fonts,
                            &text_style,
                            sheet,
                            cell,
                            s,
                            (Some(at), c),
                            &links,
                        );
                    }
                }
            }
        }
    }
    out
}

/// The coordinate at `num / den` of `len` from `lo`.
fn at(lo: Nm, len: Nm, num: i64, den: i64) -> Nm {
    Nm(lo.0 + i64::try_from(i128::from(len.0) * i128::from(num) / i128::from(den)).unwrap_or(0))
}

fn lower_cover(
    rect: Rect,
    cover: &CoverTables,
    sheet: SheetMap,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> LoweredTable {
    let (x0, y0) = (rect.min().x, rect.min().y);
    let (w, h) = (rect.width(), rect.height());
    // Columns in 560ths of the width; rows in units of a 24th of the height.
    let x = |n: i64| at(x0, w, n, 560);
    let row = Nm(h.0 / 24);
    let mut p = Painter {
        fonts,
        styles,
        sheet,
        row,
        out: LoweredTable::default(),
    };
    p.table(&cover.colours, &[x(0), x(160), x(340), x(470), x(560)], y0);
    // 19.5 rows down.
    let sizes_top = Nm(y0.0 + row.0 * 39 / 2);
    p.table(&cover.control, &[x(0), x(170), x(270)], sizes_top);
    p.table(&cover.mains, &[x(290), x(460), x(560)], sizes_top);
    if let Some(note) = &cover.note {
        let top = rect.max().y;
        p.text(
            note,
            "data.small",
            (x0, top),
            (x(560), Nm(top.0 + row.0)),
            (None, 0),
        );
    }
    p.out
}

/// Draws the tables of one block into a [`LoweredTable`].
struct Painter<'a> {
    fonts: &'a FontSet,
    styles: &'a TextStyleTable,
    sheet: SheetMap,
    /// The height of a row.
    row: Nm,
    out: LoweredTable,
}

impl Painter<'_> {
    fn table(&mut self, t: &RegionTable, edges: &[Nm], top: Nm) {
        let (left, right) = (edges[0], edges[edges.len() - 1]);
        let row = self.row;
        let title_bottom = Nm(top.0 + row.0 * 3 / 2);
        self.text(
            &t.title,
            TITLE_STYLE,
            (left, top),
            (right, title_bottom),
            (None, 0),
        );
        let header_rows = usize::from(!t.header.is_empty());
        let rows = header_rows + t.rows.len();
        if rows == 0 {
            return;
        }
        let row_y = |i: usize| Nm(title_bottom.0 + row.0 * i64::try_from(i).unwrap_or(0));
        let bottom = row_y(rows);
        let stroke = Stroke::solid(self.sheet.unscale_len(GRID_WIDTH));
        let line = |x0: Nm, y0: Nm, x1: Nm, y1: Nm| {
            Item::path(
                Path::line(Pt2::new(x0, y0), Pt2::new(x1, y1)),
                Some(stroke.clone()),
                None,
            )
        };
        let mut grid = vec![Item::path(
            Path::rect(Rect::from_points(
                Pt2::new(left, title_bottom),
                Pt2::new(right, bottom),
            )),
            Some(stroke.clone()),
            None,
        )];
        for i in 1..rows {
            grid.push(line(left, row_y(i), right, row_y(i)));
        }
        for &x in &edges[1..edges.len() - 1] {
            grid.push(line(x, title_bottom, x, bottom));
        }
        self.out.items.extend(grid);
        // Texts: the header, then the rows.
        if header_rows == 1 {
            for (c, s) in t.header.iter().enumerate().take(edges.len() - 1) {
                self.text(
                    s,
                    DEFAULT_STYLE_KEY,
                    (edges[c], row_y(0)),
                    (edges[c + 1], row_y(1)),
                    (None, c),
                );
            }
        }
        for (r, cells) in t.rows.iter().enumerate() {
            let (y0, y1) = (row_y(header_rows + r), row_y(header_rows + r + 1));
            for (c, s) in cells.iter().enumerate().take(edges.len() - 1) {
                self.text(
                    s,
                    DEFAULT_STYLE_KEY,
                    (edges[c], y0),
                    (edges[c + 1], y1),
                    (Some(r), c),
                );
            }
        }
        if let Some(caption) = &t.caption {
            self.text(
                caption,
                DEFAULT_STYLE_KEY,
                (left, bottom),
                (right, Nm(bottom.0 + row.0)),
                (None, 0),
            );
        }
    }

    /// One left-aligned text in the box from `(x0, y0)` to `(x1, y1)`; `at` is `(row, column)` for findings.
    fn text(
        &mut self,
        s: &str,
        style_key: &str,
        (x0, y0): (Nm, Nm),
        (x1, y1): (Nm, Nm),
        at: (Option<usize>, usize),
    ) {
        if s.is_empty() {
            return;
        }
        let style = self
            .styles
            .resolve(style_key)
            .unwrap_or(TextStyle::regular(Nm(3_175_000)));
        let rect = Rect::from_points(Pt2::new(x0, y0), Pt2::new(x1, y1));
        push_cell(&mut self.out, self.fonts, &style, self.sheet, rect, s, at, &[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cover() -> CoverTables {
        let t = |title: &str, rows: &[&[&str]]| RegionTable {
            title: title.to_owned(),
            header: Vec::new(),
            rows: rows
                .iter()
                .map(|r| r.iter().map(|s| (*s).to_owned()).collect())
                .collect(),
            caption: None,
        };
        CoverTables {
            colours: RegionTable {
                header: ["Voltage", "Function", "Potential", "Colour"]
                    .map(str::to_owned)
                    .to_vec(),
                caption: Some("basis".to_owned()),
                ..t("COLOURS", &[&["24 VDC", "Positive", "L+", "Dark Blue"]])
            },
            control: t("CONTROL", &[&["Dark Blue", "0.75 mm²"], &["White", "0.75 mm²"]]),
            mains: t("MAINS", &[&["Black", "1.5 mm²"]]),
            note: None,
        }
    }

    fn view() -> EmbeddedView {
        EmbeddedView {
            rect: Rect::from_points(
                Pt2::new(Nm(120_000_000), Nm(38_000_000)),
                Pt2::new(Nm(268_000_000), Nm(165_000_000)),
            ),
            content: EmbeddedContent::Cover(Box::new(cover())),
        }
    }

    #[test]
    fn the_tables_stay_inside_the_block_and_are_deterministic() {
        let v = view();
        let a = lower_embedded(&v, SheetMap::IDENTITY, ew_text::bundled(), ew_text::builtin());
        let b = lower_embedded(&v, SheetMap::IDENTITY, ew_text::bundled(), ew_text::builtin());
        assert_eq!(a, b);
        assert!(!a.items.is_empty());
        assert!(a.findings.is_empty(), "{:?}", a.findings);
    }

    #[test]
    fn a_missing_table_draws_nothing() {
        let v = EmbeddedView {
            content: EmbeddedContent::Cover(Box::default()),
            ..view()
        };
        let out = lower_embedded(&v, SheetMap::IDENTITY, ew_text::bundled(), ew_text::builtin());
        assert!(out.items.is_empty() && out.findings.is_empty());
    }

    fn boxed(flow_columns: u32, rows: usize) -> EmbeddedTable {
        let column = |h: &str| TableColumn {
            header: h.to_owned(),
            weight: 1,
        };
        EmbeddedTable {
            title: "PART DATA".to_owned(),
            columns: vec![column("Item"), column("Value")],
            headers: true,
            flow_columns,
            rows: (0..rows)
                .map(|i| TableRow {
                    kind: TableRowKind::Data,
                    cells: vec![format!("r{i}"), "v".to_owned()],
                    links: Vec::new(),
                })
                .collect(),
            note: None,
        }
    }

    /// A rectangle read from a hostile file at the `i64` extremes lowers without a panic (the arithmetic
    /// stays inside `i64`), for both contents, and its capacity is computed on the clamped rectangle.
    #[test]
    fn a_block_at_the_i64_extremes_lowers_without_a_panic() {
        let rects = [
            Rect::from_points(
                Pt2::new(Nm(i64::MIN), Nm(i64::MIN)),
                Pt2::new(Nm(i64::MAX), Nm(i64::MAX)),
            ),
            Rect::from_points(
                Pt2::new(Nm(i64::MAX), Nm(i64::MAX)),
                Pt2::new(Nm(i64::MAX), Nm(i64::MAX)),
            ),
            Rect::from_points(
                Pt2::new(Nm(i64::MIN), Nm(i64::MIN)),
                Pt2::new(Nm(i64::MIN), Nm(i64::MIN)),
            ),
        ];
        let sheets = [
            SheetMap::IDENTITY,
            SheetMap {
                scale_ppb: u32::MAX,
                offset: Pt2::new(Nm(9_790_000), Nm(9_790_000)),
            },
        ];
        for rect in rects {
            for sheet in sheets {
                for content in [
                    EmbeddedContent::Cover(Box::new(cover())),
                    EmbeddedContent::Table(boxed(3, 4)),
                ] {
                    let v = EmbeddedView { rect, content };
                    let _ = lower_embedded(&v, sheet, ew_text::bundled(), ew_text::builtin());
                }
                let _ = boxed_capacity(rect, &boxed(3, 4), sheet);
            }
        }
        let whole = rects[0];
        assert_eq!(
            boxed_capacity(whole, &boxed(1, 0), SheetMap::IDENTITY),
            boxed_capacity(bounded(whole), &boxed(1, 0), SheetMap::IDENTITY)
        );
        assert_eq!(
            bounded(whole),
            Rect::from_points(
                Pt2::new(Nm(-MAX_COORD), Nm(-MAX_COORD)),
                Pt2::new(Nm(MAX_COORD), Nm(MAX_COORD))
            )
        );
    }

    /// A 3-flow table in a box 5 mm wide (narrower than its two gutters) draws every flow column inside
    /// the box: no column's left lies right of the box, so none has `right < left`.
    #[test]
    fn flow_columns_in_a_box_narrower_than_the_gutters_stay_inside_it() {
        let (x0, x1) = (Nm(100_000_000), Nm(105_000_000));
        let rect = Rect::from_points(Pt2::new(x0, Nm(50_000_000)), Pt2::new(x1, Nm(80_000_000)));
        let t = boxed(3, 0);
        let (flows, per_flow) = boxed_capacity(rect, &t, SheetMap::IDENTITY);
        assert_eq!(flows, 3);
        let v = EmbeddedView {
            rect,
            content: EmbeddedContent::Table(boxed(3, 3 * per_flow)),
        };
        let out = lower_embedded(&v, SheetMap::IDENTITY, ew_text::bundled(), ew_text::builtin());
        let headers: Vec<(Nm, Nm)> = out
            .items
            .iter()
            .filter_map(|i| match &i.kind {
                ew_dl::ItemKind::Path(p) if p.fill == Some(Brush::Solid(HEADER_FILL)) => {
                    p.path.bounds().map(|b| (b.min().x, b.max().x))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            headers,
            vec![(x0, x0), (Nm(104_000_000), Nm(104_000_000)), (x1, x1)]
        );
    }
}
