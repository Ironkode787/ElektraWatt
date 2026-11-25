// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A text a wire runs through stays readable (ADR-0034 amendment 3): the wires layer leaves out the stretch
//! of every line that runs through the box of a text of the labels layer, and [`KNOCKOUT_GAP`] on each side of it
//! along the line. One rule in the scene, so the window, the PDF, the PNG, the SVG and the DXF all draw the cut line.
//!
//! - **The texts** are those the labels layer prints, each by the box of its glyph runs as placed (their layout boxes,
//!   from the ascent to the descent, over the advance), but the texts of a mark: a wire marker's text and a cable mark's
//!   core names stand across their wires by the marker's rule, as the layout's measure counts them (doc 06a, C4). The
//!   labels layer lists the boxes a line of the page runs through ([`crate::page::LayerList::knockouts`]); the wires
//!   layer reads them from the view ([`crate::page::PageView::knockouts`]).
//! - **The lines** are the connections' straight stretches that run along a page axis. A stretch runs through a box
//!   when it lies strictly between the box's two edges across it and overlaps it along it by more than a point: a
//!   line along a text's edge, or one that ends where a box begins, is not cut. Slanted stretches (a bevel) and
//!   curves (a hop, a hose's arc) are drawn whole.
//! - **The cut** keeps the line's item, its entity and its stroke: the line becomes subpaths, and a dash pattern starts
//!   again on each, as PDF draws it. Where no line runs through a text, the layer is the one drawn before.

use ew_dl::{Path, PathEl};
use ew_units::{Nm, Pt2, Rect};

/// How far a cut line stops short of the text it runs through, beyond the text's box along the line: 0.25 mm.
pub const KNOCKOUT_GAP: Nm = Nm(250_000);

/// The coordinate along an axis-parallel stretch `a`–`b` and the one across it, and whether it is horizontal; `None`
/// for a slanted stretch or a point.
fn axes(a: Pt2, b: Pt2) -> Option<bool> {
    match (a.x == b.x, a.y == b.y) {
        (false, true) => Some(true),
        (true, false) => Some(false),
        _ => None,
    }
}

/// The span of `r` along (`horizontal`: x) and across a stretch.
fn spans(r: Rect, horizontal: bool) -> ((i64, i64), (i64, i64)) {
    let (lo, hi) = (r.min(), r.max());
    if horizontal {
        ((lo.x.0, hi.x.0), (lo.y.0, hi.y.0))
    } else {
        ((lo.y.0, hi.y.0), (lo.x.0, hi.x.0))
    }
}

/// Whether the axis-parallel stretch `a`–`b` runs through the inside of `r` (see the module documentation).
fn crosses(a: Pt2, b: Pt2, r: Rect) -> bool {
    let Some(horizontal) = axes(a, b) else {
        return false;
    };
    let (along, across) = spans(r, horizontal);
    let (s, e, c) = if horizontal {
        (a.x.0, b.x.0, a.y.0)
    } else {
        (a.y.0, b.y.0, a.x.0)
    };
    across.0 < c && c < across.1 && s.min(e) < along.1 && along.0 < s.max(e)
}

/// The boxes of `boxes` that a stretch of one of `lines` runs through, each once, in their order.
pub(crate) fn crossed(lines: &[Vec<Pt2>], boxes: &[Rect]) -> Vec<Rect> {
    let mut out: Vec<Rect> = Vec::new();
    for r in boxes {
        let through = lines
            .iter()
            .any(|l| l.windows(2).any(|w| crosses(w[0], w[1], *r)));
        if through && !out.contains(r) {
            out.push(*r);
        }
    }
    out
}

/// The pieces of the stretch `a`–`b` outside every box of `boxes` it runs through, grown by [`KNOCKOUT_GAP`] along
/// it, in the order from `a` to `b`; pieces of no length are left out. `[(a, b)]` where it runs through none.
fn clear(a: Pt2, b: Pt2, boxes: &[Rect]) -> Vec<(Pt2, Pt2)> {
    let Some(horizontal) = axes(a, b) else {
        return vec![(a, b)];
    };
    let (s, e) = if horizontal {
        (a.x.0, b.x.0)
    } else {
        (a.y.0, b.y.0)
    };
    let (lo, hi) = (s.min(e), s.max(e));
    let mut gaps: Vec<(i64, i64)> = boxes
        .iter()
        .filter(|r| crosses(a, b, **r))
        .map(|r| {
            let (along, _) = spans(*r, horizontal);
            (
                along.0.saturating_sub(KNOCKOUT_GAP.0).max(lo),
                along.1.saturating_add(KNOCKOUT_GAP.0).min(hi),
            )
        })
        .collect();
    if gaps.is_empty() {
        return vec![(a, b)];
    }
    gaps.sort_unstable();
    let mut pieces: Vec<(i64, i64)> = Vec::new();
    let mut from = lo;
    for (g0, g1) in gaps {
        if g0 > from {
            pieces.push((from, g0));
        }
        from = from.max(g1);
    }
    if hi > from {
        pieces.push((from, hi));
    }
    let at = |t: i64| {
        if horizontal {
            Pt2::new(Nm(t), a.y)
        } else {
            Pt2::new(a.x, Nm(t))
        }
    };
    if s > e {
        pieces.iter().rev().map(|&(p0, p1)| (at(p1), at(p0))).collect()
    } else {
        pieces.iter().map(|&(p0, p1)| (at(p0), at(p1))).collect()
    }
}

/// `path` with every axis-parallel straight stretch cut round the boxes of `boxes` it runs through (module
/// documentation); `None` where no stretch runs through one, where nothing of it would be left, or where the path
/// closes a subpath (an outline, which no wire is): the caller draws it whole.
pub(crate) fn knock_out(path: &Path, boxes: &[Rect]) -> Option<Path> {
    if boxes.is_empty() || path.els.contains(&PathEl::ClosePath) {
        return None;
    }
    let mut els: Vec<PathEl> = Vec::with_capacity(path.els.len());
    // The point the source path stands at, and the point the output's open subpath ends at (none after a move).
    let mut cur = Pt2::ORIGIN;
    let mut pen: Option<Pt2> = None;
    let mut cut = false;
    for el in &path.els {
        match *el {
            PathEl::MoveTo(p) => {
                cur = p;
                pen = None;
            }
            PathEl::LineTo(p) => {
                let pieces = clear(cur, p, boxes);
                cut |= pieces != [(cur, p)];
                for (s, e) in pieces {
                    if pen != Some(s) {
                        els.push(PathEl::MoveTo(s));
                    }
                    els.push(PathEl::LineTo(e));
                    pen = Some(e);
                }
                cur = p;
            }
            curve => {
                if pen != Some(cur) {
                    els.push(PathEl::MoveTo(cur));
                }
                els.push(curve);
                cur = curve.end().unwrap_or(cur);
                pen = Some(cur);
            }
        }
    }
    // A line that lies wholly in texts is drawn whole: a wire is never drawn as nothing.
    (cut && !els.is_empty()).then_some(Path { els })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i64, y: i64) -> Pt2 {
        Pt2::new(Nm(x * 1_000_000), Nm(y * 1_000_000))
    }

    fn r(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        Rect::from_points(p(x0, y0), p(x1, y1))
    }

    /// A vertical wire through a text's box is cut round it with the gap; one along the box's edge, one that ends at
    /// it and one beside it are not.
    #[test]
    fn a_stretch_is_cut_where_it_runs_through_a_box() {
        let text = r(0, 10, 20, 14);
        let gap = KNOCKOUT_GAP.0;
        let mm = |v: i64| v * 1_000_000;
        assert_eq!(
            clear(p(5, 0), p(5, 30), &[text]),
            vec![
                (p(5, 0), Pt2::new(Nm(mm(5)), Nm(mm(10) - gap))),
                (Pt2::new(Nm(mm(5)), Nm(mm(14) + gap)), p(5, 30)),
            ]
        );
        // Drawn the other way, the pieces run the other way too.
        assert_eq!(
            clear(p(5, 30), p(5, 0), &[text]),
            vec![
                (p(5, 30), Pt2::new(Nm(mm(5)), Nm(mm(14) + gap))),
                (Pt2::new(Nm(mm(5)), Nm(mm(10) - gap)), p(5, 0)),
            ]
        );
        for (a, b) in [
            (p(0, 0), p(0, 30)),
            (p(5, 0), p(5, 10)),
            (p(25, 0), p(25, 30)),
            (p(-5, 12), p(-1, 12)),
        ] {
            assert_eq!(clear(a, b, &[text]), vec![(a, b)], "{a:?} {b:?}");
        }
    }

    /// A hop's half circle stays whole, the straight stretches round it are cut, and a line that would vanish in a
    /// text is drawn whole.
    #[test]
    fn a_path_keeps_its_curves_and_is_never_cut_to_nothing() {
        let text = r(0, 10, 20, 14);
        let gap = KNOCKOUT_GAP.0;
        let hop = PathEl::CubicTo(p(9, 4), p(11, 4), p(12, 6));
        let path = Path {
            els: vec![
                PathEl::MoveTo(p(30, 12)),
                PathEl::LineTo(p(10, 12)),
                PathEl::LineTo(p(10, 6)),
                hop,
                PathEl::LineTo(p(12, 0)),
            ],
        };
        let x = |mm: i64| Nm(mm * 1_000_000);
        assert_eq!(
            knock_out(&path, &[text]).expect("cut").els,
            [
                PathEl::MoveTo(p(30, 12)),
                PathEl::LineTo(Pt2::new(Nm(x(20).0 + gap), x(12))),
                PathEl::MoveTo(Pt2::new(x(10), Nm(x(10).0 - gap))),
                PathEl::LineTo(p(10, 6)),
                hop,
                PathEl::LineTo(p(12, 0)),
            ]
        );
        let inside = Path::line(p(2, 12), p(18, 12));
        assert_eq!(knock_out(&inside, &[text]), None);
    }
}
