// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame geometry from a [`FrameGrid`]: the outer rectangle, the drawing area, the column and row edges,
//! the labels and the title-block origin, all in page coordinates (nm, y down).

use ew_dl::Pen;
use ew_schema::FormDef;
use ew_units::{Grid, Nm, Pt2, Rect, arith};
use serde_json::Value;

/// `FrameGrid.extra` member: the width in nm of the generated frame lines (outer rectangle, header bands,
/// title-block separator). A layout hint; default [`DEFAULT_FRAME_LINE`].
pub const EXTRA_LINE_WIDTH: &str = "ew.frame.line_width";
/// `FrameGrid.extra` member: the frame's margin in nm, the distance of its outer rectangle from the page
/// origin on both axes (a QET border lies 5 px from the scene origin, doc 14 §4.9). The header band and the
/// column and row edges keep counting from the page origin, so the bands are `header_band − margin` thick.
/// Default 0.
pub const EXTRA_MARGIN: &str = "ew.frame.margin";
/// The width of generated frame lines without [`EXTRA_LINE_WIDTH`]: the 0.26 mm house pen.
pub const DEFAULT_FRAME_LINE: Nm = Pen::P026.width();

/// The geometry of a frame form with a grid.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct FrameGeometry {
    /// The frame's outer rectangle: from `(margin, margin)` to the grid's far edges, extended down (and
    /// right) by the title block placed below the drawing area.
    pub outer: Rect,
    /// The drawing area: inside the header bands, above the title block.
    pub drawing_area: Rect,
    /// Where the header bands end: the band thickness plus the margin (0 without bands).
    pub header_band: Nm,
    /// The margin ([`EXTRA_MARGIN`]): where the outer rectangle starts on both axes.
    pub margin: Nm,
    /// The `count + 1` column edges, left to right: `header + k × width`.
    pub column_edges: Vec<Nm>,
    /// The `count + 1` row edges, top to bottom: `header + k × height`.
    pub row_edges: Vec<Nm>,
    /// The column labels.
    pub column_labels: Vec<String>,
    /// The row labels.
    pub row_labels: Vec<String>,
    /// Where the title block's origin (its top-left corner) lands: `(margin, last row edge)`.
    pub title_block_origin: Option<Pt2>,
    /// Whether bands, ticks and labels are drawn (`FrameGrid::shows_grid`).
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
            .map(|k| grid_edge(header, grid.columns.width, k))
            .collect();
        let row_edges: Vec<Nm> = (0..=i64::from(grid.rows.count))
            .map(|k| grid_edge(header, grid.rows.height, k))
            .collect();
        let header_nm = grid_edge(header, grid.columns.width, 0);
        let right = *column_edges.last().unwrap_or(&Nm::ZERO);
        let bottom = *row_edges.last().unwrap_or(&Nm::ZERO);
        let (tb_w, tb_h) = title_block.map_or((Nm::ZERO, Nm::ZERO), |tb| (tb.sheet.width, tb.sheet.height));
        let margin = grid
            .extra
            .get(EXTRA_MARGIN)
            .and_then(Value::as_i64)
            .filter(|&m| m > 0)
            .map_or(Nm::ZERO, Nm)
            .min(header_nm);
        let outer = Rect::from_points(
            Pt2::new(margin, margin),
            Pt2::new(right.max(margin + tb_w), bottom + tb_h),
        );
        Some(FrameGeometry {
            outer,
            drawing_area: Rect::from_points(Pt2::new(header_nm, header_nm), Pt2::new(right, bottom)),
            header_band: header_nm,
            margin,
            column_edges,
            row_edges,
            column_labels: (0..usize::from(grid.columns.count))
                .map(|k| ew_derive::column_label(grid, k))
                .collect(),
            row_labels: (0..usize::from(grid.rows.count))
                .map(|k| ew_derive::row_label(grid, k))
                .collect(),
            title_block_origin: title_block.map(|_| Pt2::new(margin, bottom)),
            show_grid: grid.shows_grid(),
            line_width: grid
                .extra
                .get(EXTRA_LINE_WIDTH)
                .and_then(Value::as_i64)
                .filter(|&w| w > 0)
                .map_or(DEFAULT_FRAME_LINE, Nm),
        })
    }
}

/// What a person points at when a frame is changed on its page (doc 09 §3.9), in page nm: the scene answers where
/// everything is, so the frame tool reads these and computes nothing itself.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FrameHits {
    /// The title block's cells, each with its index in the title block's `cells`.
    pub cells: Vec<(usize, Rect)>,
    /// The title block's lines between its cells (`FormDef::cell_edges`), each with its index in that list.
    pub lines: Vec<(Pt2, Pt2)>,
    /// The band of the column numbers along the top, while the rulers are drawn.
    pub columns: Option<Rect>,
    /// The band of the row letters along the left, while the rulers are drawn.
    pub rows: Option<Rect>,
    /// The line between the drawing area and the title block.
    pub block_top: Option<(Pt2, Pt2)>,
    /// The drawing area (the page's drawing fades here while the frame is changed).
    pub drawing_area: Option<Rect>,
}

impl FrameHits {
    /// The hit data of `form` and its title block `title_block`.
    pub fn of(form: &FormDef, title_block: Option<&FormDef>) -> FrameHits {
        let geometry = FrameGeometry::of(form, title_block);
        let mut hits = FrameHits {
            drawing_area: geometry.as_ref().map(|g| g.drawing_area),
            ..FrameHits::default()
        };
        if let Some(g) = &geometry
            && g.show_grid
            && g.header_band > g.margin
        {
            let (m, h, area) = (g.margin, g.header_band, g.drawing_area);
            hits.columns = Some(Rect::from_points(Pt2::new(h, m), Pt2::new(area.max().x, h)));
            hits.rows = Some(Rect::from_points(Pt2::new(m, h), Pt2::new(h, area.max().y)));
        }
        if let Some(tb) = title_block {
            let origin = title_block_origin(form, tb);
            let at = |p: Pt2| p + origin;
            hits.cells = tb
                .cells
                .iter()
                .enumerate()
                .map(|(i, c)| (i, Rect::from_points(at(c.rect.0.min()), at(c.rect.0.max()))))
                .collect();
            hits.lines = tb.cell_edges().into_iter().map(|(a, b)| (at(a), at(b))).collect();
            hits.block_top = Some((origin, Pt2::new(origin.x + tb.sheet.width, origin.y)));
        }
        hits
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

/// Edge `k` of a frame's columns or rows on the page: `round(header + k × step)` nm, exact, ties toward zero; header 0
/// if absent. Where the page draws the edge, and where Cell-at and Go to read it.
pub fn grid_edge(header: Option<Grid>, step: Grid, k: i64) -> Nm {
    let (hn, hd) = header.map_or((0i128, 1i128), |h| (i128::from(h.num()), i128::from(h.den())));
    let (sn, sd) = (i128::from(step.num()), i128::from(step.den()));
    let num = hn * sd + i128::from(k) * sn * hd;
    let den = hd * sd;
    let q = arith::div_round_ties_toward_zero(num, den);
    Nm(i64::try_from(q).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_exact() {
        let header = Some(Grid::new(15_875_000, 3).unwrap());
        let col = Grid::new(15_875_000, 1).unwrap();
        // 20 px + k × 60 px; 20 px = 5_291_666.67 nm rounds to 5_291_667.
        assert_eq!(grid_edge(header, col, 0), Nm(5_291_667));
        assert_eq!(grid_edge(header, col, 1), Nm(21_166_667));
        assert_eq!(grid_edge(header, col, 17), Nm(275_166_667));
        assert_eq!(grid_edge(None, col, 2), Nm(31_750_000));
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        /// A grid step `num / den` nm of at least 1 nm.
        fn grid() -> impl Strategy<Value = Grid> {
            (1u64..=2_000_000_000, 1u64..=1_000)
                .prop_filter_map("grid at least 1 nm", |(n, d)| Grid::new(n, d).ok())
        }

        /// Round-half-down on the magnitude: a different route than `edge` takes to the same rounding.
        fn round_ties_toward_zero(n: i128, d: i128) -> i128 {
            let q = (2 * n.abs() + d - 1) / (2 * d);
            if n < 0 { -q } else { q }
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

            /// `grid_edge(header, step, k) = round(header + k × step)` over exact rationals, ties toward zero.
            #[test]
            fn edge_is_the_exact_rational(
                header in proptest::option::of(grid()),
                step in grid(),
                k in -10_000i64..=10_000,
            ) {
                let (hn, hd) = header.map_or((0i128, 1i128), |h| (i128::from(h.num()), i128::from(h.den())));
                let (sn, sd) = (i128::from(step.num()), i128::from(step.den()));
                // (hn / hd) + k × (sn / sd) over the common denominator hd × sd.
                let want = round_ties_toward_zero(hn * sd + i128::from(k) * sn * hd, hd * sd);
                prop_assert_eq!(i128::from(grid_edge(header, step, k).0), want);
            }

            /// Edges of a grid are non-decreasing in `k`.
            #[test]
            fn edges_are_monotonic(header in proptest::option::of(grid()), step in grid(), k in 0i64..=10_000) {
                prop_assert!(grid_edge(header, step, k) <= grid_edge(header, step, k + 1));
            }
        }
    }
}
