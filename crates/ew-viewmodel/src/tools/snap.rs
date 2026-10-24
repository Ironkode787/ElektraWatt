// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Snapping (WP-S2.03 §7.5, FR-UI-018, doc 06 §5.7): to the connection points drawn on a page within 8 screen
//! points, else to the page's connection grid, always on grid indices (ADR-0005).
//!
//! A [`Snapper`] is built per page content from its [`PageGrid`] and `Session::page_cps`, once per (revision,
//! folio), and evaluated without a `Session` call. A sheet point maps to the page by the inverse of the frame's
//! page-to-sheet map (`page = (sheet − offset) × 10⁹ / scale_ppb`, per axis in `i128`, halves away from zero); the
//! grid index of a page coordinate is `k = round(page × den / num)`; the page point a command receives is
//! `round(k × num / den)` (so a tool never meets the grid rule's rejection, ID-54), and its sheet point
//! `offset + round(k × num × scale_ppb / (den × 10⁹))`, the point the grid overlay and the grid cursor use.

use ew_api::output::{PageCp, PageGrid};

/// What a point snapped to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SnapKind {
    /// A point of the page's connection grid.
    Grid,
    /// A connection point.
    Cp,
}

impl SnapKind {
    /// The transcript's and the describe text's word: `grid` or `cp`.
    pub fn word(self) -> &'static str {
        match self {
            SnapKind::Grid => "grid",
            SnapKind::Cp => "cp",
        }
    }
}

/// The result of [`Snapper::snap`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Snap {
    /// The snapped point on the sheet, nm.
    pub at_nm: (i64, i64),
    /// The snapped point on the page, nm: what a command receives, on the page's connection grid.
    pub page_nm: (i64, i64),
    /// Its index on the page's connection grid.
    pub grid_index: (i64, i64),
    /// What it snapped to.
    pub kind: SnapKind,
    /// The connection point's label (`-102F1:2`) for [`SnapKind::Cp`].
    pub label: Option<String>,
}

/// What a fine snap places (WP-S2.04 §6.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FineKind {
    /// A graphic's point.
    Graphic,
    /// A text's anchor.
    Text,
}

/// How far a connection point attracts the pointer, in screen points.
pub const CP_SNAP_PT: f64 = 8.0;

/// `num / den` rounded to the nearest integer, halves away from zero (`den > 0`).
pub(super) fn div_round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() >= den { q + num.signum() } else { q }
}

/// An `i128` as `i64`, saturating.
fn sat(v: i128) -> i64 {
    i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })
}

/// A connection point as the snapper keeps it.
#[derive(Clone, PartialEq, Eq, Debug)]
struct SnapCp {
    at: (i64, i64),
    label: String,
    grid_index: Option<(i64, i64)>,
}

/// The snapping of one page's content.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Snapper {
    /// The grid pitch `num / den` nm on the page.
    num: i128,
    den: i128,
    /// The page-to-sheet scale in parts per 10⁹ and the sheet point of the page origin.
    scale: i128,
    offset: (i128, i128),
    /// The page's connection points in reading order.
    cps: Vec<SnapCp>,
}

impl Default for Snapper {
    fn default() -> Snapper {
        Snapper::new(None, &[])
    }
}

impl Snapper {
    /// The snapper of a page with `grid` (a 1 mm grid at scale 1 without one) and its connection points in
    /// reading order.
    pub fn new(grid: Option<&PageGrid>, cps: &[PageCp]) -> Snapper {
        let (num, den, scale, offset) = match grid {
            Some(g) if g.num_nm > 0 && g.den > 0 && g.sheet_scale_ppb > 0 => (
                i128::from(g.num_nm),
                i128::from(g.den),
                i128::from(g.sheet_scale_ppb),
                (i128::from(g.sheet_offset_nm.0), i128::from(g.sheet_offset_nm.1)),
            ),
            _ => (1_000_000, 1, 1_000_000_000, (0, 0)),
        };
        Snapper {
            num,
            den,
            scale,
            offset,
            cps: cps
                .iter()
                .map(|c| SnapCp {
                    at: c.at_nm,
                    label: c.label.clone(),
                    grid_index: c.grid_index,
                })
                .collect(),
        }
    }

    /// The grid pitch on the page, `(num, den)` nm.
    pub fn grid(&self) -> (i64, i64) {
        (sat(self.num), sat(self.den))
    }

    fn off(&self, axis: usize) -> i128 {
        if axis == 0 { self.offset.0 } else { self.offset.1 }
    }

    /// The page coordinate of the sheet coordinate `s` on `axis` (0 x, 1 y).
    pub fn to_page(&self, s: i64, axis: usize) -> i64 {
        sat(div_round(
            (i128::from(s) - self.off(axis)) * 1_000_000_000,
            self.scale,
        ))
    }

    /// The grid index of the page coordinate `p`.
    fn index(&self, p: i64) -> i64 {
        sat(div_round(i128::from(p) * self.den, self.num))
    }

    /// The page coordinate of grid index `k`: `round(k × num / den)`.
    pub fn page_of_index(&self, k: i64) -> i64 {
        sat(div_round(i128::from(k) * self.num, self.den))
    }

    /// The sheet coordinate of grid index `k` on `axis`.
    pub fn sheet_of_index(&self, k: i64, axis: usize) -> i64 {
        sat(self.off(axis) + div_round(i128::from(k) * self.num * self.scale, self.den * 1_000_000_000))
    }

    /// The grid point nearest to the sheet point `at`.
    pub fn snap_grid(&self, at: (i64, i64)) -> Snap {
        let k = (
            self.index(self.to_page(at.0, 0)),
            self.index(self.to_page(at.1, 1)),
        );
        Snap {
            at_nm: (self.sheet_of_index(k.0, 0), self.sheet_of_index(k.1, 1)),
            page_nm: (self.page_of_index(k.0), self.page_of_index(k.1)),
            grid_index: k,
            kind: SnapKind::Grid,
            label: None,
        }
    }

    /// The connection point nearest to `at` within `CP_SNAP_PT` screen points at `points_per_nm` (squared distance
    /// in `i128`; ties to the earlier in reading order), as a snap.
    pub fn snap_cp(&self, at: (i64, i64), points_per_nm: f64) -> Option<Snap> {
        let reach = CP_SNAP_PT / points_per_nm;
        if !(reach.is_finite() && reach >= 0.0) {
            return None;
        }
        let reach = reach.floor().min(9.0e15) as i128;
        let limit = reach * reach;
        let mut best: Option<(i128, &SnapCp)> = None;
        for cp in &self.cps {
            let (dx, dy) = (
                i128::from(cp.at.0) - i128::from(at.0),
                i128::from(cp.at.1) - i128::from(at.1),
            );
            let d = dx * dx + dy * dy;
            if d <= limit && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, cp));
            }
        }
        let (_, cp) = best?;
        let k = cp.grid_index.unwrap_or_else(|| {
            (
                self.index(self.to_page(cp.at.0, 0)),
                self.index(self.to_page(cp.at.1, 1)),
            )
        });
        let page_nm = match cp.grid_index {
            Some(k) => (self.page_of_index(k.0), self.page_of_index(k.1)),
            None => (self.to_page(cp.at.0, 0), self.to_page(cp.at.1, 1)),
        };
        Some(Snap {
            at_nm: cp.at,
            page_nm,
            grid_index: k,
            kind: SnapKind::Cp,
            label: Some(cp.label.clone()),
        })
    }

    /// The sheet coordinate of the page coordinate `p` on `axis`: `offset + round(p × scale_ppb / 10⁹)`.
    pub fn to_sheet(&self, p: i64, axis: usize) -> i64 {
        sat(self.off(axis) + div_round(i128::from(p) * self.scale, 1_000_000_000))
    }

    /// The sheet point of the page point `p`.
    pub fn page_to_sheet(&self, p: (i64, i64)) -> (i64, i64) {
        (self.to_sheet(p.0, 0), self.to_sheet(p.1, 1))
    }

    /// The page point of the sheet point `s`.
    pub fn sheet_to_page(&self, s: (i64, i64)) -> (i64, i64) {
        (self.to_page(s.0, 0), self.to_page(s.1, 1))
    }

    /// Whether the page's connection grid is the QET grid `7_937_500/3` nm.
    pub fn is_qet(&self) -> bool {
        (self.num, self.den) == (7_937_500, 3)
    }

    /// The grid index of the page coordinate `p` (`round(p × den / num)`, halves away from zero).
    pub fn grid_index_of(&self, p: i64) -> i64 {
        self.index(p)
    }

    /// The fine snap of graphics and texts (WP-S2.04 §6.5, doc 06 §2.4): the nearest point of the fine grid, on a
    /// QET-grid page `793_750/3` nm (1 px) for both, elsewhere 500 000 nm for graphics and 1 000 000 nm for texts;
    /// with `alt` the exact page point of the pointer. Returns the sheet point and the page point, computed on
    /// grid indices in `i128`.
    pub fn snap_fine(&self, at: (i64, i64), kind: FineKind, alt: bool) -> ((i64, i64), (i64, i64)) {
        let page = self.sheet_to_page(at);
        if alt {
            return (self.page_to_sheet(page), page);
        }
        let (num, den): (i128, i128) = if self.is_qet() {
            (793_750, 3)
        } else {
            match kind {
                FineKind::Graphic => (500_000, 1),
                FineKind::Text => (1_000_000, 1),
            }
        };
        let snap = |p: i64| sat(div_round(div_round(i128::from(p) * den, num) * num, den));
        let page = (snap(page.0), snap(page.1));
        (self.page_to_sheet(page), page)
    }

    /// Snaps the sheet point `at` (WP-S2.03 §7.5): unless `alt` is held, the connection point nearest to it within
    /// 8 screen points wins; else the nearest point of the page's connection grid.
    pub fn snap(&self, at: (i64, i64), points_per_nm: f64, alt: bool) -> Snap {
        if !alt && let Some(s) = self.snap_cp(at, points_per_nm) {
            return s;
        }
        self.snap_grid(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qet() -> PageGrid {
        PageGrid {
            num_nm: 7_937_500,
            den: 3,
            sheet_scale_ppb: 986_282_578,
            sheet_offset_nm: (9_790_000, 9_790_000),
        }
    }

    fn cp(n: u8, label: &str, at: (i64, i64)) -> PageCp {
        PageCp {
            id: format!("00000000-0000-0000-0000-0000000000{n:02x}")
                .parse()
                .unwrap(),
            label: label.to_owned(),
            at_nm: at,
            grid_index: Some((n.into(), 7)),
            connected: false,
        }
    }

    #[test]
    fn a_cp_within_8_points_wins_alt_suppresses_it_and_ties_go_to_reading_order() {
        let a = cp(1, "-1A1:1", (100_000_000, 100_000_000));
        let b = cp(2, "-1A1:2", (100_000_000, 104_000_000));
        let s = Snapper::new(Some(&qet()), &[a, b]);
        // At 100 % one point is 0.2646 mm, so 8 points are 2.117 mm.
        let ppn = 96.0 / 25_400_000.0;
        let near = s.snap((101_500_000, 100_500_000), ppn, false);
        assert_eq!(near.kind, SnapKind::Cp);
        assert_eq!(near.label.as_deref(), Some("-1A1:1"));
        assert_eq!(near.at_nm, (100_000_000, 100_000_000));
        assert_eq!(near.grid_index, (1, 7));
        assert_eq!(near.page_nm, (s.page_of_index(1), s.page_of_index(7)));
        // Beyond 8 points: the grid.
        assert_eq!(
            s.snap((102_200_000, 100_000_000), ppn, false).kind,
            SnapKind::Grid
        );
        // Zoomed in to 400 % the reach shrinks to 0.53 mm.
        assert_eq!(
            s.snap((101_000_000, 100_000_000), 4.0 * ppn, false).kind,
            SnapKind::Grid
        );
        // Alt suppresses connection points.
        assert_eq!(s.snap((100_000_000, 100_000_000), ppn, true).kind, SnapKind::Grid);
        // Equidistant from both: the earlier in reading order.
        let mid = s.snap((100_000_000, 102_000_000), ppn, false);
        assert_eq!(mid.label.as_deref(), Some("-1A1:1"));
    }

    #[test]
    fn grid_snaps_are_exact_grid_indices_under_the_sheet_map() {
        let s = Snapper::new(Some(&qet()), &[]);
        for x in (0..400_000_000).step_by(7_654_321) {
            for y in [12_345_678_i64, 99_999_999, 200_000_001] {
                let snap = s.snap((x, y), 1.0, false);
                assert_eq!(snap.kind, SnapKind::Grid);
                // The page point lies within 1 nm of k × g (here exactly round(k × g)).
                for (axis, k) in [snap.grid_index.0, snap.grid_index.1].into_iter().enumerate() {
                    let page = [snap.page_nm.0, snap.page_nm.1][axis];
                    let exact = k as f64 * 7_937_500.0 / 3.0;
                    assert!((page as f64 - exact).abs() <= 0.5 + 1e-6, "{k}: {page}");
                }
                // The snapped grid point is the nearest one to the pointer's page point.
                let page = s.to_page(x, 0);
                let k = snap.grid_index.0;
                let d = |k: i64| (page as f64 - k as f64 * 7_937_500.0 / 3.0).abs();
                assert!(d(k) <= d(k - 1) && d(k) <= d(k + 1));
            }
        }
    }
}
