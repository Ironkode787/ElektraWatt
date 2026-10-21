// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Embedded report blocks on a drawn page (FR-REP-028, doc 09 §2.2): the cover's colour and standard-size
//! tables (doc 20 §17.3).
//!
//! The caller fills an [`EmbeddedView`] with the **printed** texts of the block's groups; [`lower_embedded`]
//! is a pure function of the view, the page-to-sheet map of the frame form and the font set. The block's
//! rectangle (`ReportBlock.embedded.rect`, page coordinates) is divided in rows of `u = height / 24`; the
//! proportions are those of the reference cover (printed 128.52 to 274.66 mm by 47.63 to 172.89 mm, doc 20
//! §17.3):
//!
//! | Region | x (of 560) | y |
//! |---|---|---|
//! | colour table | 0 to 560; columns 160, 180, 130, 90 | title `1.5 u`, header `1 u`, the data rows (15) of `1 u`, then the basis text (`1 u`) |
//! | control sizes | 0 to 270; columns 170, 100 | from `19.5 u`: title `1.5 u`, then the rows (3) of `1 u` |
//! | mains sizes | 290 to 560; columns 170, 100 | as the control sizes |
//!
//! A title is drawn in style `cover.table` (11.84 pt printed), every other text in `text.normal`, left aligned
//! and centred on the row's midline, inset by [`CELL_PADDING`]. The grid (the outline and the row and column
//! separators of the header and data rows, each line once) is [`GRID_WIDTH`] wide (0.418 mm printed, doc 20
//! §17.4: "cover boxes and tables"); titles and captions have no grid. A text that does not fit its cell
//! shrinks to the printed cap height [`MIN_CAP_HEIGHT`] and is then reported, never clipped (FR-REP-039).
//! Only the rows given are drawn, so the table ends below its last row.

use ew_dl::{Item, Path, Stroke};
use ew_schema::{Overflow, ShrinkSpec};
use ew_text::{DEFAULT_STYLE_KEY, FontSet, TextStyle, builtin};
use ew_units::{Nm, Pt2, Rect};

use crate::forms::{BoxSpec, CELL_PADDING, SheetMap, lay_out};
use crate::table::{LoweredTable, MIN_CAP_HEIGHT, TableFinding};

/// The grid line width, printed.
pub const GRID_WIDTH: Nm = Nm(418_000);

/// The style of a table title.
pub const TITLE_STYLE: &str = "cover.table";

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
}

/// What an embedded block draws.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EmbeddedContent {
    /// The colour and standard-size tables of the cover (`profile_tables`).
    Cover(CoverTables),
}

/// One embedded block on a page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EmbeddedView {
    /// The block's rectangle in page coordinates (`ReportBlock.embedded.rect`).
    pub rect: Rect,
    /// The printed content.
    pub content: EmbeddedContent,
}

/// Lowers `view` (see the module documentation). `sheet` is the frame form's page-to-sheet map.
pub fn lower_embedded(view: &EmbeddedView, sheet: SheetMap, fonts: &FontSet) -> LoweredTable {
    match &view.content {
        EmbeddedContent::Cover(cover) => lower_cover(view.rect, cover, sheet, fonts),
    }
}

/// The coordinate at `num / den` of `len` from `lo`.
fn at(lo: Nm, len: Nm, num: i64, den: i64) -> Nm {
    Nm(lo.0 + i64::try_from(i128::from(len.0) * i128::from(num) / i128::from(den)).unwrap_or(0))
}

fn lower_cover(rect: Rect, cover: &CoverTables, sheet: SheetMap, fonts: &FontSet) -> LoweredTable {
    let (x0, y0) = (rect.min().x, rect.min().y);
    let (w, h) = (rect.width(), rect.height());
    // Columns in 560ths of the width; rows in units of a 24th of the height.
    let x = |n: i64| at(x0, w, n, 560);
    let row = Nm(h.0 / 24);
    let mut p = Painter {
        fonts,
        sheet,
        row,
        out: LoweredTable::default(),
    };
    p.table(&cover.colours, &[x(0), x(160), x(340), x(470), x(560)], y0);
    // 19.5 rows down.
    let sizes_top = Nm(y0.0 + row.0 * 39 / 2);
    p.table(&cover.control, &[x(0), x(170), x(270)], sizes_top);
    p.table(&cover.mains, &[x(290), x(460), x(560)], sizes_top);
    p.out
}

/// Draws the tables of one block into a [`LoweredTable`].
struct Painter<'a> {
    fonts: &'a FontSet,
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
        let style = builtin()
            .resolve(style_key)
            .unwrap_or(TextStyle::regular(Nm(3_175_000)));
        let shrink = Overflow::ShrinkToFit(ShrinkSpec {
            min_cap_height: MIN_CAP_HEIGHT,
            extra: Default::default(),
        });
        let spec = BoxSpec {
            rect: Rect::from_points(Pt2::new(x0, y0), Pt2::new(x1, y1)),
            halign: -1,
            valign: 0,
            padding: CELL_PADDING,
            overflow: Some(&shrink),
            max_lines: 1,
            sheet: self.sheet,
        };
        let laid = lay_out(self.fonts, s, &style, &spec);
        if !laid.shaped.missing.is_empty() {
            self.out.findings.push(TableFinding::MissingGlyphs {
                text: s.to_owned(),
                chars: laid.shaped.missing.clone(),
            });
        }
        if let Some(kind) = laid.overflow {
            self.out.findings.push(TableFinding::Overflow {
                row: at.0,
                column: at.1,
                text: s.to_owned(),
                kind,
            });
        }
        self.out.items.extend(laid.items);
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
        }
    }

    fn view() -> EmbeddedView {
        EmbeddedView {
            rect: Rect::from_points(
                Pt2::new(Nm(120_000_000), Nm(38_000_000)),
                Pt2::new(Nm(268_000_000), Nm(165_000_000)),
            ),
            content: EmbeddedContent::Cover(cover()),
        }
    }

    #[test]
    fn the_tables_stay_inside_the_block_and_are_deterministic() {
        let v = view();
        let a = lower_embedded(&v, SheetMap::IDENTITY, ew_text::bundled());
        let b = lower_embedded(&v, SheetMap::IDENTITY, ew_text::bundled());
        assert_eq!(a, b);
        assert!(!a.items.is_empty());
        assert!(a.findings.is_empty(), "{:?}", a.findings);
    }

    #[test]
    fn a_missing_table_draws_nothing() {
        let v = EmbeddedView {
            content: EmbeddedContent::Cover(CoverTables::default()),
            ..view()
        };
        let out = lower_embedded(&v, SheetMap::IDENTITY, ew_text::bundled());
        assert!(out.items.is_empty() && out.findings.is_empty());
    }
}
