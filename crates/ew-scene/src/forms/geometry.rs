// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame geometry from a [`FrameGrid`]: the outer rectangle, the drawing area, the column and row edges,
//! the labels and the title-block origin, all in page coordinates (nm, y down).

use ew_dl::Pen;
use ew_schema::{FormDef, FrameGrid};
use ew_units::{Grid, Nm, Pt2, Rect};
use serde_json::Value;

/// `FrameGrid.extra` member: the width in nm of the generated frame lines (outer rectangle, header bands,
/// title-block separator). A layout hint; default [`DEFAULT_FRAME_LINE`].
pub const EXTRA_LINE_WIDTH: &str = "ew.frame.line_width";
/// `FrameGrid.extra` member: `false` hides the header bands, ticks and labels while keeping the grid's
/// geometry (the no-grid variant of FR-PAG-017). Default `true`.
pub const EXTRA_SHOW_GRID: &str = "ew.frame.show_grid";
/// The width of generated frame lines without [`EXTRA_LINE_WIDTH`]: the 0.26 mm house pen.
pub const DEFAULT_FRAME_LINE: Nm = Pen::P026.width();

/// The geometry of a frame form with a grid.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FrameGeometry {
    /// The frame's outer rectangle: from the origin to the grid's far edges, extended down (and right)
    /// by the title block placed below the drawing area.
    pub outer: Rect,
    /// The drawing area: inside the header bands, above the title block.
    pub drawing_area: Rect,
    /// The header band thickness (0 without bands).
    pub header_band: Nm,
    /// The `count + 1` column edges, left to right: `header + k × width`.
    pub column_edges: Vec<Nm>,
    /// The `count + 1` row edges, top to bottom: `header + k × height`.
    pub row_edges: Vec<Nm>,
    /// The column labels.
    pub column_labels: Vec<String>,
    /// The row labels.
    pub row_labels: Vec<String>,
    /// Where the title block's origin (its top-left corner) lands: `(0, last row edge)`.
    pub title_block_origin: Option<Pt2>,
    /// Whether bands, ticks and labels are drawn ([`EXTRA_SHOW_GRID`]).
    pub show_grid: bool,
    /// The width of the generated lines ([`EXTRA_LINE_WIDTH`]).
    pub line_width: Nm,
}

impl FrameGeometry {
    /// The geometry of `form`, whose title block (if any) is `title_block`; `None` if the form has no
    /// grid.
    ///
    /// Every edge is computed exactly on the rational grid and rounded once to whole nm, ties toward zero:
    /// `round((hn/hd) + k × (wn/wd))`. The title block's size is its own `sheet`.
    pub fn of(form: &FormDef, title_block: Option<&FormDef>) -> Option<FrameGeometry> {
        let grid = form.grid.as_ref()?;
        let header = grid.header_band;
        let column_edges: Vec<Nm> = (0..=i64::from(grid.columns.count))
            .map(|k| edge(header, grid.columns.width, k))
            .collect();
        let row_edges: Vec<Nm> = (0..=i64::from(grid.rows.count))
            .map(|k| edge(header, grid.rows.height, k))
            .collect();
        let header_nm = edge(header, grid.columns.width, 0);
        let right = *column_edges.last().unwrap_or(&Nm::ZERO);
        let bottom = *row_edges.last().unwrap_or(&Nm::ZERO);
        let (tb_w, tb_h) = title_block.map_or((Nm::ZERO, Nm::ZERO), |tb| (tb.sheet.width, tb.sheet.height));
        let outer = Rect::from_points(Pt2::ORIGIN, Pt2::new(right.max(tb_w), bottom + tb_h));
        Some(FrameGeometry {
            outer,
            drawing_area: Rect::from_points(Pt2::new(header_nm, header_nm), Pt2::new(right, bottom)),
            header_band: header_nm,
            column_edges,
            row_edges,
            column_labels: column_labels(grid),
            row_labels: row_labels(grid),
            title_block_origin: title_block.map(|_| Pt2::new(Nm::ZERO, bottom)),
            show_grid: grid
                .extra
                .get(EXTRA_SHOW_GRID)
                .and_then(Value::as_bool)
                .unwrap_or(true),
            line_width: grid
                .extra
                .get(EXTRA_LINE_WIDTH)
                .and_then(Value::as_i64)
                .filter(|&w| w > 0)
                .map_or(DEFAULT_FRAME_LINE, Nm),
        })
    }
}

/// Where a title block lands on a form: below the drawing area of a grid frame
/// ([`FrameGeometry::title_block_origin`]); on a form without grid, at the bottom-left of the sheet
/// (`(0, sheet.height − title_block.sheet.height)`; such forms are native, scale-1 frames).
pub fn title_block_origin(form: &FormDef, title_block: &FormDef) -> Pt2 {
    match FrameGeometry::of(form, Some(title_block)).and_then(|g| g.title_block_origin) {
        Some(p) => p,
        None => Pt2::new(Nm::ZERO, form.sheet.height - title_block.sheet.height),
    }
}

/// `round(header + k × step)` nm, exact, ties toward zero; header 0 if absent.
fn edge(header: Option<Grid>, step: Grid, k: i64) -> Nm {
    let (hn, hd) = header.map_or((0i128, 1i128), |h| (i128::from(h.num()), i128::from(h.den())));
    let (sn, sd) = (i128::from(step.num()), i128::from(step.den()));
    let num = hn * sd + i128::from(k) * sn * hd;
    let den = hd * sd;
    let q = num / den;
    let r = num % den;
    let q = if 2 * r.abs() > den { q + num.signum() } else { q };
    Nm(i64::try_from(q).unwrap_or(i64::MAX))
}

fn column_labels(grid: &FrameGrid) -> Vec<String> {
    let c = &grid.columns;
    (0..usize::from(c.count))
        .map(|k| {
            c.labels.get(k).map_or_else(
                || (u32::from(c.start_index) + k as u32).to_string(),
                |l| l.as_str().to_owned(),
            )
        })
        .collect()
}

fn row_labels(grid: &FrameGrid) -> Vec<String> {
    let r = &grid.rows;
    let letters: Vec<char> = ('A'..='Z')
        .filter(|c| !(r.skip_io && (*c == 'I' || *c == 'O')))
        .collect();
    (0..usize::from(r.count))
        .map(|k| {
            r.labels
                .get(k)
                .map_or_else(|| letter_label(&letters, k), |l| l.as_str().to_owned())
        })
        .collect()
}

/// `A … Z`, then `AA, AB …` (bijective base-n over `letters`).
fn letter_label(letters: &[char], mut k: usize) -> String {
    let n = letters.len();
    let mut out = Vec::new();
    loop {
        out.push(letters[k % n]);
        if k < n {
            break;
        }
        k = k / n - 1;
    }
    out.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters() {
        let l: Vec<char> = ('A'..='Z').collect();
        assert_eq!(letter_label(&l, 0), "A");
        assert_eq!(letter_label(&l, 7), "H");
        assert_eq!(letter_label(&l, 25), "Z");
        assert_eq!(letter_label(&l, 26), "AA");
        assert_eq!(letter_label(&l, 27), "AB");
    }

    #[test]
    fn edges_are_exact() {
        let header = Some(Grid::new(15_875_000, 3).unwrap());
        let col = Grid::new(15_875_000, 1).unwrap();
        // 20 px + k × 60 px; 20 px = 5_291_666.67 nm rounds to 5_291_667.
        assert_eq!(edge(header, col, 0), Nm(5_291_667));
        assert_eq!(edge(header, col, 1), Nm(21_166_667));
        assert_eq!(edge(header, col, 17), Nm(275_166_667));
        assert_eq!(edge(None, col, 2), Nm(31_750_000));
    }
}
