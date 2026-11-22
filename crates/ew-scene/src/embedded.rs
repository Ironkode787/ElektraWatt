// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded report blocks on a drawn page (FR-REP-028, doc 09 §2.2): the cover's colour and standard-size
//! tables (doc 20 §17.3) and, for every other kind, a boxed report table (PART DATA, the layout
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
//!   row, per flow column) are not drawn and are reported ([`TableFinding::TooManyRows`], FR-REP-039);
//! - a lookup table ([`EmbeddedTable::lookup`], the legend a person adds) draws one flow column with its columns sized
//!   to their texts, its identifiers whole and its prose wrapped where the box is narrow ([`Lookup`]), and its box
//!   ending under its last row ([`boxed_box`]); it draws no drop shadow, and its header row is filled
//!   [`LOOKUP_HEADER_FILL`], lighter than the house's.
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

use ew_dl::{Brush, Color, Item, Link, Path, Stroke};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyle, TextStyleTable};
use ew_units::{Nm, Pt2, Rect};

use crate::forms::{BoxSpec, CELL_PADDING, SheetMap, lay_out, min_size};
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

/// The header fill of a lookup table (the legend a person adds): a light grey, so the header reads as the table's head
/// and not as its heaviest part.
pub const LOOKUP_HEADER_FILL: Color = Color::from_rgb8(232, 232, 232);

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
    /// A lookup table (the legend a person adds beside a layout, doc 09 §2.2): `Some` prints the rows in one flow
    /// column whose columns are as wide as their widest texts, the last taking what is left, and ends the box under
    /// its last row ([`boxed_box`]); where the box is narrower than the texts, the columns give way as [`Lookup`]
    /// says. `None`: the house layout (the weights, [`EmbeddedTable::flow_columns`], the whole rectangle).
    pub lookup: Option<Lookup>,
}

/// How the columns of a lookup table give way where its box is narrower than its texts (doc 12 §5.4): an identifier (a
/// position, a tag, a quantity, a part number) prints whole, a text of prose (a description) wraps.
///
/// 1. Each column shrinks its texts down to the smallest size (the 1.8 mm cap height), the prose first, then the
///    identifiers in the order of `order`.
/// 2. Each prose column narrows down to its widest word at that size, a text that no longer fits wrapping at its words
///    onto further lines.
/// 3. Each prose column narrows further, its texts cut.
/// 4. Last, each identifier column, in the order of `order`, is cut, down to nothing.
///
/// A row whose prose wraps grows by a line for each further line. Where the rows, so grown, need more height than the
/// box has, no text wraps and each prose text is cut in its row instead: the box holds its rows before their whole
/// descriptions.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Lookup {
    /// The order in which the identifier columns give way (indices of [`EmbeddedTable::columns`]).
    pub order: Vec<usize>,
    /// The columns of prose (indices of [`EmbeddedTable::columns`]); every other column is an identifier.
    pub prose: Vec<usize>,
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
    let flows = if table.lookup.is_some() {
        1
    } else {
        table.flow_columns.clamp(1, MAX_FLOW_COLUMNS) as usize
    };
    (flows, slots.saturating_sub(usize::from(draws_header(table))))
}

/// The box a boxed table draws in `rect` (page coordinates) under the map `sheet`, its shadow left out: the rectangle,
/// or for a lookup table its top part down to its last row (the title band, the header and the rows it draws, a
/// wrapped row as high as its lines), never below the rectangle. What a click on the block hits.
pub fn boxed_box(
    rect: Rect,
    table: &EmbeddedTable,
    sheet: SheetMap,
    fonts: &FontSet,
    styles: &TextStyleTable,
) -> Rect {
    let rect = bounded(rect);
    if table.lookup.is_none() {
        return rect;
    }
    let r = sheet.apply_rect(rect);
    let header = i64::from(draws_header(table));
    let rows: i64 = match lookup_plan(rect, table, sheet, fonts, &text_style(styles)) {
        Some(plan) => plan.heights.iter().sum(),
        None => {
            let (_, per_flow) = boxed_capacity(rect, table, sheet);
            let given = if table.note.is_some() { 1 } else { table.rows.len() };
            ROW_HEIGHT
                .0
                .saturating_mul(i64::try_from(given.min(per_flow)).unwrap_or(0))
        }
    };
    let bottom = (r.min().y.0 + title_band(table).0 + ROW_HEIGHT.0 * header)
        .saturating_add(rows)
        .min(r.max().y.0);
    Rect::from_points(rect.min(), sheet.unapply(Pt2::new(r.max().x, Nm(bottom))))
}

/// The style of a boxed table's header and rows.
fn text_style(styles: &TextStyleTable) -> TextStyle {
    styles
        .resolve(DEFAULT_STYLE_KEY)
        .unwrap_or(TextStyle::regular(Nm(3_175_000)))
}

/// How a lookup table stands in its box: its column edges and the rows it draws, each with its lines and its height.
struct Plan {
    /// The column edges, printed.
    edges: Vec<Nm>,
    /// The lines of each row drawn: more than 1 where its prose wraps.
    lines: Vec<usize>,
    /// The height of each row drawn, printed.
    heights: Vec<i64>,
}

/// The plan of a lookup table in `rect` (page coordinates, clamped) whose rows derive ([`Lookup`]); `None` for the house
/// layout and for a block whose rows do not derive (its note prints in one row of the house height).
fn lookup_plan(
    rect: Rect,
    t: &EmbeddedTable,
    sheet: SheetMap,
    fonts: &FontSet,
    style: &TextStyle,
) -> Option<Plan> {
    let lookup = t.lookup.as_ref().filter(|_| t.note.is_none())?;
    let r = sheet.apply_rect(rect);
    let (_, per_flow) = boxed_capacity(rect, t, sheet);
    let uniform = t.rows.len().min(per_flow);
    let edges = lookup_edges(
        t,
        &t.rows[..uniform],
        lookup,
        (r.min().x, r.max().x),
        fonts,
        style,
        sheet,
    );
    // The room under the title band and the header.
    let top = r.min().y.0 + title_band(t).0 + ROW_HEIGHT.0 * i64::from(draws_header(t));
    let room = r.max().y.0 - top;
    let small = smallest(fonts, style, sheet);
    let pitch = sheet.scale_len(fonts.vertical_metrics(&small).line_pitch).0;
    let height = |n: usize| ROW_HEIGHT.0 + pitch * i64::try_from(n - 1).unwrap_or(0);
    let lines: Vec<usize> = t
        .rows
        .iter()
        .map(|row| {
            lookup
                .prose
                .iter()
                .filter_map(|&c| wrapped(row, c, &edges, fonts, &small, sheet).map(|s| s.lines.len()))
                .max()
                .unwrap_or(1)
                .max(1)
        })
        .collect();
    let wrapped_height = lines.iter().fold(0i64, |a, n| a.saturating_add(height(*n)));
    // Wrapped where every row then fits the box; else every row in one line, as many as fit.
    let lines = if lines.iter().any(|n| *n > 1) && wrapped_height <= room {
        lines
    } else {
        vec![1; uniform]
    };
    let heights = lines.iter().map(|n| height(*n)).collect();
    Some(Plan {
        edges,
        lines,
        heights,
    })
}

/// `style` at the smallest size a cell shrinks to (the printed cap height [`MIN_CAP_HEIGHT`](crate::table::MIN_CAP_HEIGHT)),
/// the size a wrapped text prints in.
fn smallest(fonts: &FontSet, style: &TextStyle, sheet: SheetMap) -> TextStyle {
    style.with_size(min_size(fonts, style, crate::table::MIN_CAP_HEIGHT, sheet).min(style.size))
}

/// The text of `row`'s cell in column `c` shaped in `small` (the smallest size), wrapped at its words within the
/// column between `edges` (printed), when it does not fit the column in one line at that size; `None` for a heading
/// row, an empty cell, a column without width, a text that fits in one line (and prints as a cell does), and a text
/// with a word wider than the column (which cannot wrap and is cut as a cell is).
fn wrapped(
    row: &TableRow,
    c: usize,
    edges: &[Nm],
    fonts: &FontSet,
    small: &TextStyle,
    sheet: SheetMap,
) -> Option<ew_text::Shaped> {
    if row.kind != TableRowKind::Data {
        return None;
    }
    let s = row.cells.get(c).filter(|s| !s.is_empty())?;
    let (a, b) = (*edges.get(c)?, *edges.get(c + 1)?);
    // The width a cell's text has inside its insets, in page coordinates, as `lay_out` measures it.
    let page = |x: Nm| sheet.unapply(Pt2::new(x, Nm(0))).x;
    let avail = Nm(page(b).0 - page(a).0 - 2 * CELL_PADDING.0);
    // Wrapped only where one line does not fit and no word is wider than the column (a line's advance counts the space
    // it ends with, so the words are measured, not the lines).
    if avail <= Nm::ZERO
        || fonts.shape(s, small, None).advance() <= avail
        || fonts.shape(s, small, Some(Nm::ZERO)).advance() > avail
    {
        return None;
    }
    Some(fonts.shape(s, small, Some(avail)))
}

/// The column edges of a lookup table between `left` and `right` (printed): each column as wide as its widest text
/// (the header and the rows `rows`) with its padding, the last taking what is left; where that is more than the
/// width, the columns give way as [`Lookup`] says (a column with no width prints nothing), until the columns fit.
#[allow(clippy::too_many_arguments)]
fn lookup_edges(
    t: &EmbeddedTable,
    rows: &[TableRow],
    lookup: &Lookup,
    (left, right): (Nm, Nm),
    fonts: &FontSet,
    style: &TextStyle,
    sheet: SheetMap,
) -> Vec<Nm> {
    let n = t.columns.len();
    // Both insets, and a quarter of a millimetre for the rounding of the shaping at a smaller size.
    let pad = sheet.scale_len(CELL_PADDING).0.saturating_mul(2) + 250_000;
    let floor = min_size(fonts, style, crate::table::MIN_CAP_HEIGHT, sheet).min(style.size);
    // The texts of column `c`: its header and its rows' cells.
    let texts = |c: usize| {
        let header = draws_header(t).then(|| t.columns[c].header.as_str());
        let cells = rows
            .iter()
            .filter(|r| r.kind == TableRowKind::Data)
            .filter_map(move |r| r.cells.get(c).map(String::as_str));
        header.into_iter().chain(cells)
    };
    // The printed advance of the widest text of each column at the style's size, and of its widest word at the
    // smallest size (the widest line of its texts broken at every place a line may break).
    let widest_at = |c: usize, style: &TextStyle, wrap: Option<Nm>| {
        texts(c)
            .map(|s| sheet.scale_len(fonts.shape(s, style, wrap).advance()).0)
            .max()
            .unwrap_or(0)
    };
    let widest: Vec<i64> = (0..n).map(|c| widest_at(c, style, None)).collect();
    let shrunk = |c: usize| {
        let w = i128::from(widest[c]) * i128::from(floor.0) / i128::from(style.size.0.max(1));
        i64::try_from(w).unwrap_or(widest[c]).saturating_add(pad)
    };
    let small = style.with_size(floor);
    let word = |c: usize| widest_at(c, &small, Some(Nm::ZERO)).saturating_add(pad);
    let mut width: Vec<i64> = widest.iter().map(|w| w.saturating_add(pad)).collect();
    for (c, col) in t.columns.iter().enumerate() {
        if col.weight == 0 {
            width[c] = 0;
        }
    }
    let total = right.0 - left.0;
    let mut excess = width.iter().fold(0i64, |a, w| a.saturating_add(*w)) - total;
    let prose: Vec<usize> = lookup.prose.iter().copied().filter(|c| *c < n).collect();
    let identifiers: Vec<usize> = lookup
        .order
        .iter()
        .copied()
        .filter(|c| *c < n && !prose.contains(c))
        .collect();
    let mut give = |width: &mut Vec<i64>, columns: &[usize], to: &dyn Fn(usize) -> i64| {
        for &c in columns {
            if excess <= 0 {
                break;
            }
            let r = (width[c] - to(c).min(width[c])).min(excess);
            width[c] -= r;
            excess -= r;
        }
    };
    give(&mut width, &prose, &shrunk);
    give(&mut width, &identifiers, &shrunk);
    give(&mut width, &prose, &word);
    give(&mut width, &prose, &|_| 0);
    give(&mut width, &identifiers, &|_| 0);
    let mut edges = Vec::with_capacity(n + 1);
    edges.push(left);
    let mut x = left.0;
    for w in &width[..n.saturating_sub(1)] {
        x = x.saturating_add(*w).min(right.0);
        edges.push(Nm(x));
    }
    if n > 0 {
        edges.push(right);
    }
    edges
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
    let style = |key: &str| styles.resolve(key).unwrap_or(TextStyle::regular(Nm(3_175_000)));
    let text_style = style(DEFAULT_STYLE_KEY);
    // Printed coordinates of the box.
    let r = sheet.apply_rect(boxed_box(rect, t, sheet, fonts, styles));
    let (x0, y0, x1, y1) = (r.min().x, r.min().y, r.max().x, r.max().y);
    let to_page = |x: Nm, y: Nm| sheet.unapply(Pt2::new(x, y));
    let page_rect = |ax: Nm, ay: Nm, bx: Nm, by: Nm| Rect::from_points(to_page(ax, ay), to_page(bx, by));

    // The rows given and drawn.
    let (flows, per_flow) = boxed_capacity(rect, t, sheet);
    let plan = lookup_plan(rect, t, sheet, fonts, &text_style);
    let given = if t.note.is_some() { 1 } else { t.rows.len() };
    let drawn = plan
        .as_ref()
        .map_or(given.min(flows.saturating_mul(per_flow)), |p| p.lines.len());
    let per_flow = if plan.is_some() { drawn } else { per_flow };
    if given > drawn {
        out.findings.push(TableFinding::TooManyRows { given, drawn });
    }

    // The drop shadow: the visible L of the box offset by the shadow, under the box (the project data box's shape). A
    // lookup table has none.
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
    if t.lookup.is_none() {
        out.items.push(Item::path(shadow, None, Some(Brush::BLACK)));
    }
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
    let header = usize::from(draws_header(t));
    let small = smallest(fonts, &text_style, sheet);
    let header_fill = if t.lookup.is_some() {
        LOOKUP_HEADER_FILL
    } else {
        HEADER_FILL
    };
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
        let edges = match &plan {
            Some(p) => p.edges.clone(),
            None => column_edges_in(&t.columns, left, right),
        };
        // The tops of the flow column's rows (0 the header's, when drawn), then its bottom: a wrapped row is as high as
        // its lines, every other row one house row.
        let mut tops = vec![top.0];
        for i in 0..header + count {
            let h = match (&plan, i.checked_sub(header)) {
                (Some(p), Some(j)) => p.heights[start + j],
                _ => ROW_HEIGHT.0,
            };
            tops.push(tops[i].saturating_add(h));
        }
        let row_y = |i: usize| Nm(tops[i]);
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
                Some(Brush::Solid(header_fill)),
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
                        // A prose text of a wrapped row prints its lines (doc 12 §5.4).
                        let lines = plan.as_ref().map_or(1, |p| p.lines[at]);
                        let prose = t.lookup.as_ref().is_some_and(|l| l.prose.contains(&c));
                        if lines > 1 && prose && wrapped(row, c, &edges, fonts, &small, sheet).is_some() {
                            push_wrapped(&mut out, fonts, &small, sheet, cell, s, lines, &links);
                            continue;
                        }
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

/// A text of prose wrapped at its words in `rect` (page coordinates; at most `lines` lines, which [`lookup_plan`] made
/// the row high enough for), left aligned and centred on the row's midline as a cell is; characters without a glyph are
/// reported, and the cell's link without a span is set on every line.
#[allow(clippy::too_many_arguments)]
fn push_wrapped(
    out: &mut LoweredTable,
    fonts: &FontSet,
    style: &TextStyle,
    sheet: SheetMap,
    rect: Rect,
    s: &str,
    lines: usize,
    links: &[&TableLink],
) {
    let spec = BoxSpec {
        rect,
        halign: -1,
        valign: 0,
        padding: CELL_PADDING,
        overflow: None,
        max_lines: lines,
        sheet,
        pixel: None,
    };
    let laid = lay_out(fonts, s, style, &spec);
    if !laid.shaped.missing.is_empty() {
        out.findings.push(TableFinding::MissingGlyphs {
            text: s.to_owned(),
            chars: laid.shaped.missing.clone(),
        });
    }
    match links.iter().find(|l| l.span.is_none()) {
        Some(l) => out
            .items
            .extend(laid.items.into_iter().map(|i| i.with_link(Link::Source(l.dest)))),
        None => out.items.extend(laid.items),
    }
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
            lookup: None,
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
