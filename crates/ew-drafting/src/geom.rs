// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact geometry on ticks: segments against points, rectangles and each other (exact on integers, a slanted segment
//! of an old drawing included), and the bucket grid over the page that the text placer and the measure ask what
//! stands near a place.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::model::{Pt, Rect, Tick};

/// A segment from its first point to its second.
pub type Seg = (Pt, Pt);

/// The route without consecutive equal points.
pub fn clean(route: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(route.len());
    for &p in route {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

/// Whether a route the search finds is tidy (FT-2097): no point twice in a row, and no turn back along the line it came
/// by.
pub fn tidy(route: &[Pt]) -> bool {
    let twice = route.windows(2).any(|w| w[0] == w[1]);
    let back = route.windows(3).any(|w| {
        let (a, b, c) = (w[0], w[1], w[2]);
        let on_line = (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y);
        on_line
            && (b.x - a.x).signum() == -(c.x - b.x).signum()
            && (b.y - a.y).signum() == -(c.y - b.y).signum()
    });
    !twice && !back
}

/// The segments of a cleaned route; a route of one point is one segment of no length.
pub fn segments(line: &[Pt]) -> Vec<Seg> {
    match line {
        [] => Vec::new(),
        [p] => vec![(*p, *p)],
        _ => line.windows(2).map(|w| (w[0], w[1])).collect(),
    }
}

fn wide(p: Pt) -> (i128, i128) {
    (i128::from(p.x), i128::from(p.y))
}

/// The cross product `(b − a) × (c − a)`.
fn orient(a: Pt, b: Pt, c: Pt) -> i128 {
    let (a, b, c) = (wide(a), wide(b), wide(c));
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Whether `p` lies on the closed segment `s`, exactly.
pub fn on(p: Pt, (a, b): Seg) -> bool {
    orient(a, b, p) == 0
        && a.x.min(b.x) <= p.x
        && p.x <= a.x.max(b.x)
        && a.y.min(b.y) <= p.y
        && p.y <= a.y.max(b.y)
}

/// The length of a segment, in ticks: exact on a horizontal or vertical one, the city-block length of a slanted one.
pub fn length((a, b): Seg) -> i64 {
    (i64::from(a.x) - i64::from(b.x)).abs() + (i64::from(a.y) - i64::from(b.y)).abs()
}

/// A non-negative fraction `n / d` of a segment's length, `d > 0`.
#[derive(Clone, Copy, Debug)]
pub struct Frac {
    n: i128,
    d: i128,
}

impl Frac {
    const ZERO: Frac = Frac { n: 0, d: 1 };
    const ONE: Frac = Frac { n: 1, d: 1 };

    fn new(n: i128, d: i128) -> Frac {
        if d < 0 {
            Frac { n: -n, d: -d }
        } else {
            Frac { n, d }
        }
    }
}

impl PartialEq for Frac {
    fn eq(&self, other: &Frac) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Frac {}

impl PartialOrd for Frac {
    fn partial_cmp(&self, other: &Frac) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Frac {
    fn cmp(&self, other: &Frac) -> Ordering {
        (self.n * other.d).cmp(&(other.n * self.d))
    }
}

/// `n / d` rounded to the nearest integer, halves away from zero, `d > 0`.
fn div_round(n: i128, d: i128) -> i128 {
    if n >= 0 {
        (2 * n + d) / (2 * d)
    } else {
        -((-2 * n + d) / (2 * d))
    }
}

fn tick(v: i128) -> Tick {
    Tick::try_from(v).unwrap_or(if v < 0 { Tick::MIN } else { Tick::MAX })
}

/// The point at the fraction `t` of the segment `a`–`b`, rounded to the tick.
pub fn at((a, b): Seg, t: Frac) -> Pt {
    let (a, b) = (wide(a), wide(b));
    let (mut n, mut d) = (t.n, t.d);
    while d > 1 << 62 {
        (n, d) = (n >> 1, d >> 1);
    }
    Pt::new(
        tick(a.0 + div_round((b.0 - a.0) * n, d)),
        tick(a.1 + div_round((b.1 - a.1) * n, d)),
    )
}

/// The fractions `(lo, hi)`, `lo < hi`, of the segment that lie inside the open rectangle `r`; `None` when it does
/// not enter it (a segment along an edge, or touching a corner, stays outside).
pub fn inside((a, b): Seg, r: Rect) -> Option<(Frac, Frac)> {
    let (a, b) = (wide(a), wide(b));
    let (mut lo, mut hi) = (Frac::ZERO, Frac::ONE);
    for (p, d, min, max) in [
        (a.0, b.0 - a.0, i128::from(r.x0), i128::from(r.x1)),
        (a.1, b.1 - a.1, i128::from(r.y0), i128::from(r.y1)),
    ] {
        match d.cmp(&0) {
            Ordering::Equal if min < p && p < max => {}
            Ordering::Equal => return None,
            Ordering::Greater => {
                lo = lo.max(Frac::new(min - p, d));
                hi = hi.min(Frac::new(max - p, d));
            }
            Ordering::Less => {
                lo = lo.max(Frac::new(p - max, -d));
                hi = hi.min(Frac::new(p - min, -d));
            }
        }
    }
    (lo < hi).then_some((lo, hi))
}

/// The stretch of the segment inside the open rectangle, from its first point's side.
pub fn stretch_inside(s: Seg, r: Rect) -> Option<Seg> {
    inside(s, r).map(|(lo, hi)| (at(s, lo), at(s, hi)))
}

/// `(lo, hi)` of the segment without the fractions on the ray from `p` along `v`; unchanged when the segment is not
/// on the ray's line.
pub fn minus_ray(s: Seg, (lo, hi): (Frac, Frac), p: Pt, v: Pt) -> Option<(Frac, Frac)> {
    let ((a, b), p, v) = (s, wide(p), wide(v));
    let (a, b) = (wide(a), wide(b));
    let d = (b.0 - a.0, b.1 - a.1);
    let w = (a.0 - p.0, a.1 - p.1);
    if v.0 * d.1 - v.1 * d.0 != 0 || v.0 * w.1 - v.1 * w.0 != 0 {
        return Some((lo, hi));
    }
    let (s0, sd) = (w.0 * v.0 + w.1 * v.1, d.0 * v.0 + d.1 * v.1);
    if sd == 0 {
        return Some((lo, hi));
    }
    let edge = Frac::new(-s0, sd);
    let (lo, hi) = if sd > 0 {
        (lo, hi.min(edge))
    } else {
        (lo.max(edge), hi)
    };
    (lo < hi).then_some((lo, hi))
}

/// The stretch two segments with a length share on one line, from its smaller end to its larger; `None` when they
/// are not on one line or share at most a point.
pub fn overlap(s: Seg, t: Seg) -> Option<Seg> {
    let ((a, b), (c, d)) = (s, t);
    if a == b || c == d || orient(a, b, c) != 0 || orient(a, b, d) != 0 {
        return None;
    }
    let key = |p: Pt| if a.x != b.x { p.x } else { p.y };
    let sort = |p: Pt, q: Pt| if key(p) <= key(q) { (p, q) } else { (q, p) };
    let ((s0, s1), (t0, t1)) = (sort(a, b), sort(c, d));
    let lo = if key(s0) >= key(t0) { s0 } else { t0 };
    let hi = if key(s1) <= key(t1) { s1 } else { t1 };
    (key(lo) < key(hi)).then_some((lo, hi))
}

/// The point two segments meet at, when they share exactly one (rounded to the tick for slanted segments); `None`
/// when they are apart or share a stretch.
pub fn meet(s: Seg, t: Seg) -> Option<Pt> {
    let ((a, b), (c, d)) = (s, t);
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1.signum() * o2.signum() < 0 && o3.signum() * o4.signum() < 0 {
        let (wa, wc, wd) = (wide(a), wide(c), wide(d));
        let num = (wc.0 - wa.0) * (wd.1 - wc.1) - (wc.1 - wa.1) * (wd.0 - wc.0);
        return Some(at(s, Frac::new(num, o2 - o1)));
    }
    [c, d]
        .into_iter()
        .find(|p| on(*p, s))
        .or_else(|| [a, b].into_iter().find(|p| on(*p, t)))
}

/// Whether a segment meets the open inside of a rectangle.
pub fn crosses(s: Seg, r: Rect) -> bool {
    inside(s, r).is_some()
}

/// The direction of a segment with a length, reduced (a unit step on a horizontal or vertical one).
pub fn direction((a, b): Seg) -> (i64, i64) {
    let (dx, dy) = (i64::from(b.x) - i64::from(a.x), i64::from(b.y) - i64::from(a.y));
    let g = gcd(dx, dy).max(1);
    (dx / g, dy / g)
}

fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    i64::try_from(a).unwrap_or(i64::MAX)
}

/// The vertices of a cleaned route where it changes direction (a reversal is one), in order.
pub fn corners(line: &[Pt]) -> Vec<Pt> {
    line.windows(3)
        .filter(|w| direction((w[0], w[1])) != direction((w[1], w[2])))
        .map(|w| w[1])
        .collect()
}

/// The runs of a cleaned route: its segments with the vertices it runs straight through left out.
pub fn runs(line: &[Pt]) -> Vec<Seg> {
    let mut out: Vec<Seg> = Vec::new();
    for s in segments(line) {
        match out.last_mut() {
            Some(last) if last.0 != last.1 && s.0 != s.1 && direction(*last) == direction(s) => last.1 = s.1,
            _ => out.push(s),
        }
    }
    out
}

/// The size of a bucket of [`Buckets`]: four steps.
const BUCKET: i64 = 64;

/// A grid of buckets over the page: each holds the numbers of the boxes that reach into it, so that a question about
/// a place reads the things near it only.
#[derive(Clone, Debug, Default)]
pub struct Buckets {
    cells: BTreeMap<(i64, i64), Vec<usize>>,
}

impl Buckets {
    fn span(lo: Tick, hi: Tick) -> std::ops::RangeInclusive<i64> {
        i64::from(lo).div_euclid(BUCKET)..=i64::from(hi).div_euclid(BUCKET)
    }

    /// Files `r` under the number `n`.
    pub fn insert(&mut self, n: usize, r: Rect) {
        for x in Buckets::span(r.x0, r.x1) {
            for y in Buckets::span(r.y0, r.y1) {
                self.cells.entry((x, y)).or_default().push(n);
            }
        }
    }

    /// Takes the number `n` filed under `r` out again.
    pub fn remove(&mut self, n: usize, r: Rect) {
        for x in Buckets::span(r.x0, r.x1) {
            for y in Buckets::span(r.y0, r.y1) {
                if let Some(v) = self.cells.get_mut(&(x, y)) {
                    v.retain(|k| *k != n);
                }
            }
        }
    }

    /// The numbers filed near `r`, each once, in increasing order.
    pub fn near(&self, r: Rect) -> Vec<usize> {
        let mut out = Vec::new();
        for x in Buckets::span(r.x0, r.x1) {
            for y in Buckets::span(r.y0, r.y1) {
                if let Some(v) = self.cells.get(&(x, y)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// Whether two boxes of texts (or a text's and a body's) meet: their insides overlap by more than the tick each was
/// rounded outward by (§2.1).
pub fn texts_meet(a: Rect, b: Rect) -> bool {
    a.grown(-1).overlaps(b.grown(-1))
}

/// Whether two boxes stand one over the other rather than side by side: their heights overlap by at most half the
/// lower one's.
pub fn stacked(a: Rect, b: Rect) -> bool {
    let shared = i64::from(a.y1.min(b.y1)) - i64::from(a.y0.max(b.y0));
    2 * shared <= i64::from(a.h().min(b.h()))
}

/// The gap between two boxes: their distances across and down summed, 0 where they meet.
pub fn gap(a: Rect, b: Rect) -> Tick {
    let dx = (a.x0 - b.x1).max(b.x0 - a.x1).max(0);
    let dy = (a.y0 - b.y1).max(b.y0 - a.y1).max(0);
    dx.saturating_add(dy)
}

/// The box of a segment.
pub fn seg_box((a, b): Seg) -> Rect {
    Rect::new(a, b)
}
