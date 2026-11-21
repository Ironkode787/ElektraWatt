// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where a list's table stands on its frame, and how many rows a page of it holds (doc 09 §3.8): one exact,
//! integer rule that the pagination of `ew-reports` and the table `ew-scene` draws both read, so a page never holds a
//! row it cannot draw.
//!
//! **The area** is the frame's drawing area: on a frame with a grid, inside its header bands and above its title
//! block (the grid's far edges); on a form without a grid, the sheet less `PLAIN_MARGIN` on every side and less its
//! title block's height at the bottom. It is taken onto the sheet through the form's page-to-sheet transform, exactly
//! (one rounding, ties toward zero).
//!
//! **The table** keeps the house table's distances to the drawing area of the starter A4 frame: its left edge
//! `INSET_LEFT` in, its right edge `INSET_RIGHT` in, its top `INSET_TOP` down. On that frame it is the house table
//! of doc 20 §16 to the nanometre: x 16.31 to 277.27 mm, its top at 15.01 mm, 23 rows.
//!
//! **The rows** are [`HOUSE_ROW_HEIGHT`] high, printed: an optional caption row, the header row, the data rows, an
//! optional legend row and one row kept free (where the window marks a list that runs on). A page holds as many data
//! rows as fit, at least one.

use ew_units::Nm;

use crate::library::{FormDef, HOUSE_ROW_HEIGHT, HOUSE_ROWS_PER_PAGE};

/// The margin of the area of a form without a grid, on every side: 10 mm.
const PLAIN_MARGIN: Nm = Nm(10_000_000);

/// How far the table's left edge stands inside the drawing area, printed: the house table's 16.31 mm less the starter
/// A4 frame's drawing area's left edge (15.009 079 mm).
const INSET_LEFT: Nm = Nm(1_300_921);
/// How far the table's right edge stands inside the drawing area, printed: the starter A4 frame's right edge
/// (281.182 089 mm) less the house table's 277.27 mm.
const INSET_RIGHT: Nm = Nm(3_912_089);
/// How far the table's top stands below the drawing area's top, printed: the house table's 15.01 mm less the starter A4
/// frame's top edge (15.009 079 mm).
const INSET_TOP: Nm = Nm(921);

/// Where a list's table stands on a frame, printed (sheet nm, y down).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ListArea {
    /// The table's left edge.
    pub left: Nm,
    /// The table's right edge.
    pub right: Nm,
    /// The top of the first row: the caption row's with a caption, else the header row's.
    pub top: Nm,
    /// Whether a caption row stands above the header row.
    pub caption: bool,
    /// The data rows one page holds, at least 1.
    pub rows: i64,
}

impl ListArea {
    /// The list area of a list page without a frame form (a project that holds none): the house table of the starter
    /// A4 frame, x 16.31 to 277.27 mm, its top at 15.01 mm, [`HOUSE_ROWS_PER_PAGE`] rows less a caption's and a
    /// legend's.
    pub fn house(caption: bool, legend: bool) -> ListArea {
        ListArea {
            left: Nm(16_310_000),
            right: Nm(277_270_000),
            top: Nm(15_010_000),
            caption,
            rows: (HOUSE_ROWS_PER_PAGE - i64::from(caption) - i64::from(legend)).max(1),
        }
    }

    /// The top of the header row.
    pub fn header_top(&self) -> Nm {
        if self.caption {
            Nm(self.top.0 + HOUSE_ROW_HEIGHT.0)
        } else {
            self.top
        }
    }

    /// The top of data row `i` (0-based) below the header; `i = rows` is the row under the last one.
    pub fn row_top(&self, i: i64) -> Nm {
        Nm(self.header_top().0 + HOUSE_ROW_HEIGHT.0 * (i + 1))
    }
}

impl FormDef {
    /// Where a list's table stands on this frame, whose title block is `title_block`, with a caption row and a legend
    /// row as asked (see the module documentation).
    pub fn list_area(&self, title_block: Option<&FormDef>, caption: bool, legend: bool) -> ListArea {
        let (scale, offset) = self
            .page_to_sheet
            .as_ref()
            .map_or((1_000_000_000_i128, (0, 0)), |t| {
                (i128::from(t.scale_ppb), (t.offset.x.0, t.offset.y.0))
            });
        // A page coordinate `num / den` nm on the sheet, along an axis with offset `off`.
        let printed = |num: i128, den: i128, off: i64| -> Nm {
            let v = round(num * scale, den * 1_000_000_000);
            Nm(off.saturating_add(i64::try_from(v).unwrap_or(i64::MAX)))
        };
        let (left, right, top, bottom) = match &self.grid {
            Some(grid) => {
                let (hn, hd) = grid
                    .header_band
                    .map_or((0, 1), |h| (i128::from(h.num()), i128::from(h.den())));
                // `header + count × step` over the common denominator.
                let far = |count: u16, num: u64, den: u64| {
                    let (sn, sd) = (i128::from(num), i128::from(den));
                    (hn * sd + i128::from(count) * sn * hd, hd * sd)
                };
                let (rn, rd) = far(grid.columns.count, grid.columns.width.num(), grid.columns.width.den());
                let (bn, bd) = far(grid.rows.count, grid.rows.height.num(), grid.rows.height.den());
                (
                    printed(hn, hd, offset.0),
                    printed(rn, rd, offset.0),
                    printed(hn, hd, offset.1),
                    printed(bn, bd, offset.1),
                )
            }
            None => {
                let tb = title_block.map_or(0, |t| t.sheet.height.0);
                let m = PLAIN_MARGIN.0;
                let nm = |v: i64, off: i64| printed(i128::from(v), 1, off);
                (
                    nm(m, offset.0),
                    nm(self.sheet.width.0 - m, offset.0),
                    nm(m, offset.1),
                    nm(self.sheet.height.0 - tb - m, offset.1),
                )
            }
        };
        let top = Nm(top.0 + INSET_TOP.0);
        let fit = (bottom.0 - top.0).max(0) / HOUSE_ROW_HEIGHT.0;
        // The header row, the row kept free, a caption and a legend.
        let other = 2 + i64::from(caption) + i64::from(legend);
        ListArea {
            left: Nm(left.0 + INSET_LEFT.0),
            right: Nm(right.0 - INSET_RIGHT.0),
            top,
            caption,
            rows: (fit - other).max(1),
        }
    }
}

/// `num / den` rounded to the nearest integer, exact ties toward zero (`den > 0`).
fn round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() > den { q + num.signum() } else { q }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LibCopy, LibDocument};

    fn form(text: &str) -> FormDef {
        match LibDocument::from_json(text).expect("a form") {
            LibDocument::Form(LibCopy { item, .. }) => item,
            other => panic!("not a form: {other:?}"),
        }
    }

    /// On the starter A4 frame the table is the house table of doc 20 §16: x 16.31 to 277.27 mm, its top at
    /// 15.01 mm, 23 rows; a caption and a legend each take one of them.
    #[test]
    fn the_starter_a4_frame_holds_the_house_table() {
        let frame = form(include_str!("../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"));
        let tb = form(include_str!("../../../content/forms/ew.titleblock.qet-a4.json"));
        let a = frame.list_area(Some(&tb), false, false);
        assert_eq!((a.left, a.right, a.top), (Nm(16_310_000), Nm(277_270_000), Nm(15_010_000)));
        assert_eq!(a.rows, HOUSE_ROWS_PER_PAGE);
        assert_eq!(a.header_top(), a.top);
        let both = frame.list_area(Some(&tb), true, true);
        assert_eq!(both.rows, a.rows - 2);
        assert_eq!(both.header_top(), Nm(a.top.0 + HOUSE_ROW_HEIGHT.0));
        // A list page without a frame takes the same area.
        for (caption, legend) in [(false, false), (true, false), (true, true)] {
            assert_eq!(
                ListArea::house(caption, legend),
                frame.list_area(Some(&tb), caption, legend)
            );
        }
    }

    /// On the starter A3 frame (FT-1826) the table keeps the house distances to its drawing area and takes its width and
    /// height: 12 rows of 21.17 mm print 259.29 mm high at 98.6 %, from 15.01 to 265.52 mm, 38 rows of 6.52 mm, 36 of
    /// them data rows.
    #[test]
    fn the_starter_a3_frame_holds_a_wider_and_longer_table() {
        let a4 = form(include_str!("../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"))
            .list_area(None, false, false);
        let frame = form(include_str!("../../../content/forms/ew.frame.qet-a3-landscape-grid.json"));
        let tb = form(include_str!("../../../content/forms/ew.titleblock.qet-a3.json"));
        let a3 = frame.list_area(Some(&tb), false, false);
        assert_eq!((a3.left, a3.top), (a4.left, a4.top));
        assert!(a3.right.0 - a3.left.0 > a4.right.0 - a4.left.0);
        assert_eq!(a3.rows, 36);
    }

    /// A form without a grid takes the sheet less 10 mm and its title block; a frame too small for one row still
    /// holds one, and nothing panics.
    #[test]
    fn a_form_without_a_grid_takes_its_sheet_and_a_tiny_one_holds_one_row() {
        let mut plain = form(include_str!("../../../content/forms/ew.frame.qet-a4-landscape-nogrid.json"));
        plain.grid = None;
        plain.page_to_sheet = None;
        let a = plain.list_area(None, false, false);
        assert_eq!(a.left, Nm(PLAIN_MARGIN.0 + INSET_LEFT.0));
        assert_eq!(a.right, Nm(297_000_000 - PLAIN_MARGIN.0 - INSET_RIGHT.0));
        // (210 − 20 mm) / 6.52 mm = 29 rows, less the header and the free row.
        assert_eq!(a.rows, 27);
        plain.sheet.height = Nm(1);
        assert_eq!(plain.list_area(None, true, true).rows, 1);
    }
}
