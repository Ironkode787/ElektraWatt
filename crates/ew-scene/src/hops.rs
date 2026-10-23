// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Crossings drawn as hops (FR-CON-031, WP-S2.04 §8.6): with the project's drawing setting `crossings: hops`, a
//! horizontal wire segment that properly crosses a vertical segment of another connection is drawn interrupted
//! with a half circle above the crossing.
//!
//! # The rule ([`hop_paths`])
//!
//! A crossing is *proper* when the point is interior to both segments (no shared end point, so a junction or a
//! connection point there is never hopped) and the segments belong to different connections. For every horizontal
//! segment of every connection, the crossings are taken in the order of the crossing connection's ID (ties by the
//! crossing segment's index): a crossing becomes a hop unless it lies closer than `2r` to an end of the horizontal
//! segment or to a hop already taken on that segment, in which case it is drawn plain. A hop interrupts the segment
//! on `[x − r, x + r]` and draws a half circle of radius `r` = [`HOP_RADIUS`] above it (towards −y) as two cubic
//! quarter circles. Vertical segments are never interrupted. Only the horizontal segments' own coordinates and the
//! crossing's x enter the arithmetic: integer nm, no float.
//!
//! The default ([`Crossings::Plain`]) draws every connection as its polyline, byte-equal to a page without the
//! setting.

use ew_dl::{Path, PathEl};
use ew_units::{Nm, Pt2};

/// How crossing wires are drawn (the project's `settings.extra.drawing.crossings`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Crossings {
    /// Plain crossings (the default; absent setting).
    #[default]
    Plain,
    /// The horizontal wire hops over the vertical one.
    Hops,
}

/// The radius of a hop: 1 mm.
pub const HOP_RADIUS: i64 = 1_000_000;

/// The control distance of a quarter circle of radius [`HOP_RADIUS`]: `round(r × 4(√2 − 1)/3)`.
const KAPPA_R: i64 = 552_285;

fn pt(x: i64, y: i64) -> Pt2 {
    Pt2::new(Nm(x), Nm(y))
}

/// The hops of every connection: for connection `k` of `polylines` (in connection-ID order), per segment index the
/// x positions of its hops, ascending. A segment without hops is absent.
pub fn hops(polylines: &[Vec<Pt2>]) -> Vec<Vec<(usize, Vec<i64>)>> {
    let r = i128::from(HOP_RADIUS);
    // Every vertical segment in connection order, then segment order: (connection index, x, y range).
    let mut verticals: Vec<(usize, i64, i64, i64)> = Vec::new();
    for (k, line) in polylines.iter().enumerate() {
        for w in line.windows(2) {
            if w[0].x == w[1].x && w[0].y != w[1].y {
                let (y0, y1) = (w[0].y.0.min(w[1].y.0), w[0].y.0.max(w[1].y.0));
                verticals.push((k, w[0].x.0, y0, y1));
            }
        }
    }
    let mut out = Vec::with_capacity(polylines.len());
    for (k, line) in polylines.iter().enumerate() {
        let mut segments = Vec::new();
        for (i, w) in line.windows(2).enumerate() {
            if !(w[0].y == w[1].y && w[0].x != w[1].x) {
                continue;
            }
            let y = w[0].y.0;
            let (x0, x1) = (w[0].x.0.min(w[1].x.0), w[0].x.0.max(w[1].x.0));
            // The proper crossings, in the crossing connection's order.
            let mut taken: Vec<i64> = Vec::new();
            for &(kv, x, vy0, vy1) in &verticals {
                if kv == k || !(x0 < x && x < x1 && vy0 < y && y < vy1) {
                    continue;
                }
                let near_end = i128::from(x - x0) < 2 * r || i128::from(x1 - x) < 2 * r;
                let near_hop = taken.iter().any(|h| i128::from((h - x).abs()) < 2 * r);
                if !(near_end || near_hop) {
                    taken.push(x);
                }
            }
            if !taken.is_empty() {
                taken.sort_unstable();
                segments.push((i, taken));
            }
        }
        out.push(segments);
    }
    out
}

/// The path of one polyline with the hops of [`hops`] (`hops[segment]` ascending x): straight lines, each hop a gap
/// of `2r` bridged by a half circle above it, in the polyline's direction.
pub fn hop_path(line: &[Pt2], hops: &[(usize, Vec<i64>)]) -> Path {
    let Some(&first) = line.first() else {
        return Path::polyline(line);
    };
    let r = HOP_RADIUS;
    let mut els = vec![PathEl::MoveTo(first)];
    for (i, w) in line.windows(2).enumerate() {
        if let Some((_, xs)) = hops.iter().find(|(s, _)| *s == i) {
            let y = w[0].y.0;
            let forward = w[1].x > w[0].x;
            let mut ordered = xs.clone();
            if !forward {
                ordered.reverse();
            }
            // `s` is +1 travelling towards +x, −1 towards −x: the hop's near side is x − s·r.
            let s = if forward { 1 } else { -1 };
            for x in ordered {
                let (a, b) = (x - s * r, x + s * r);
                els.push(PathEl::LineTo(pt(a, y)));
                // Two quarter circles through the apex (x, y − r).
                els.push(PathEl::CubicTo(
                    pt(a, y - KAPPA_R),
                    pt(x - s * KAPPA_R, y - r),
                    pt(x, y - r),
                ));
                els.push(PathEl::CubicTo(
                    pt(x + s * KAPPA_R, y - r),
                    pt(b, y - KAPPA_R),
                    pt(b, y),
                ));
            }
        }
        els.push(PathEl::LineTo(w[1]));
    }
    Path { els }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: i64 = 1_000_000;

    fn p(x: i64, y: i64) -> Pt2 {
        pt(x * MM, y * MM)
    }

    #[test]
    fn a_proper_crossing_hops_and_the_path_keeps_its_ends() {
        let h = vec![p(0, 10), p(20, 10)];
        let v = vec![p(10, 0), p(10, 20)];
        let all = hops(&[h.clone(), v.clone()]);
        assert_eq!(all[0], vec![(0, vec![10 * MM])]);
        assert!(all[1].is_empty(), "vertical segments are never interrupted");
        let path = hop_path(&h, &all[0]);
        assert_eq!(path.els.first(), Some(&PathEl::MoveTo(p(0, 10))));
        assert_eq!(path.els.last(), Some(&PathEl::LineTo(p(20, 10))));
        assert!(path.els.contains(&PathEl::LineTo(pt(9 * MM, 10 * MM))));
        // The apex lies r above the crossing (towards −y).
        let apex = pt(10 * MM, 9 * MM);
        assert!(
            path.els
                .iter()
                .any(|e| matches!(e, PathEl::CubicTo(_, _, end) if *end == apex))
        );
        // Right to left the gap and the arc follow the direction of travel.
        let back = vec![p(20, 10), p(0, 10)];
        let path = hop_path(&back, &hops(&[back.clone(), v])[0]);
        assert!(path.els.contains(&PathEl::LineTo(pt(11 * MM, 10 * MM))));
        assert_eq!(path.els.last(), Some(&PathEl::LineTo(p(0, 10))));
    }

    #[test]
    fn shared_points_near_ends_and_near_hops_are_drawn_plain() {
        let h = vec![p(0, 10), p(20, 10)];
        // Ends on the horizontal segment (a T at a junction): not proper.
        let t = vec![p(10, 10), p(10, 20)];
        // Closer than 2r to the segment's end.
        let near = vec![p(1, 0), p(1, 20)];
        // Two crossings 1 mm apart: the lower connection ID wins, the other is plain.
        let a = vec![p(15, 0), p(15, 20)];
        let b = vec![p(16, 0), p(16, 20)];
        let all = hops(&[h.clone(), t, near, a, b]);
        assert_eq!(all[0], vec![(0, vec![15 * MM])]);
        // The same connection crossing itself is not hopped.
        let own = vec![p(0, 10), p(20, 10), p(20, 0), p(10, 0), p(10, 20)];
        assert!(hops(&[own])[0].is_empty());
    }

    #[test]
    fn without_hops_the_path_is_the_polyline() {
        let line = vec![p(0, 0), p(5, 0), p(5, 5)];
        assert_eq!(hop_path(&line, &[]), Path::polyline(&line));
    }
}
