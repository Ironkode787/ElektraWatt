// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout measures of a page ([`measure`]): exact integer geometry over [`PageGeometry`], each measure listing
//! its offending items. The definitions are the module documentation of [`crate::routing`].

use std::cmp::Ordering;
use std::collections::BTreeSet;

use ew_ids::Uuid;
use ew_schema::Dir4;
use ew_units::{Nm, Pt2};

use super::page::{DrawnConnection, PageGeometry, Point, Rect};

/// What a connection touches without ending there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchedPoint {
    /// A connection point, by its ID.
    Cp(Uuid),
    /// A junction, by its ID.
    Junction(Uuid),
}

/// (a) A connection's polyline touching a connection point or a junction it does not end at.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Touch {
    /// The connection.
    pub connection: Uuid,
    /// What it touches.
    pub point: TouchedPoint,
    /// Where.
    pub at: Point,
}

/// (b) Two connections running along each other.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Overlap {
    /// The two connections, in the order of [`PageGeometry::connections`].
    pub connections: [Uuid; 2],
    /// Each stretch they share, from its smaller end to its larger (by x, on a vertical line by y), in the order of
    /// the first connection's segments.
    pub stretches: Vec<(Point, Point)>,
}

/// (c) A connection through a representation's body, or (d) through one of its texts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Through {
    /// The connection.
    pub connection: Uuid,
    /// The representation whose body or text it runs through.
    pub rep: Uuid,
    /// Each stretch inside, in the connection's direction, in the order of its segments (rounded to the nm for a
    /// segment that is neither horizontal nor vertical).
    pub stretches: Vec<(Point, Point)>,
}

/// (e) Two connections meeting at a point.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Crossing {
    /// The two connections, in the order of [`PageGeometry::connections`].
    pub connections: [Uuid; 2],
    /// Where (rounded to the nm for segments that are neither horizontal nor vertical).
    pub at: Point,
}

/// (f) A connection's bends.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bends {
    /// The connection.
    pub connection: Uuid,
    /// Its bends, at least one.
    pub bends: usize,
}

/// (g) A junction with four or more arms.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FourWay {
    /// The junction.
    pub junction: Uuid,
    /// Its arms, at least four.
    pub arms: usize,
    /// Where.
    pub at: Point,
}

/// The measures of one page, each list in the order of the page's connections (then of their segments) or junctions.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Measures {
    /// (a) Hard rule: connections touching a connection point or a junction they do not end at.
    pub touches: Vec<Touch>,
    /// (b) Hard rule: pairs of connections running along each other.
    pub overlaps: Vec<Overlap>,
    /// (c) Hard rule: connections through a representation's body.
    pub through_bodies: Vec<Through>,
    /// (d) Hard rule: connections through a representation's text.
    pub through_texts: Vec<Through>,
    /// (e) Cost: the points where two connections meet.
    pub crossings: Vec<Crossing>,
    /// (f) Cost: the connections with bends.
    pub bends: Vec<Bends>,
    /// (g) Hard rule: junctions with four or more arms.
    pub four_way: Vec<FourWay>,
}

impl Measures {
    /// The items that break a hard rule: (a) to (d) and (g).
    pub fn hard(&self) -> usize {
        self.touches.len()
            + self.overlaps.len()
            + self.through_bodies.len()
            + self.through_texts.len()
            + self.four_way.len()
    }

    /// The bends of every connection together.
    pub fn bend_total(&self) -> usize {
        self.bends.iter().map(|b| b.bends).sum()
    }
}

/// The measures of `page` (see the module documentation of [`crate::routing`]).
pub fn measure(page: &PageGeometry) -> Measures {
    let lines: Vec<Vec<Point>> = page.connections.iter().map(|c| clean(&c.polyline)).collect();
    let mut m = Measures::default();
    for (c, line) in page.connections.iter().zip(&lines) {
        touches(page, c, line, &mut m.touches);
        through(page, c, line, &mut m.through_bodies, &mut m.through_texts);
        let bends = bend_count(line);
        if bends > 0 {
            m.bends.push(Bends {
                connection: c.id,
                bends,
            });
        }
    }
    for i in 0..page.connections.len() {
        for j in i + 1..page.connections.len() {
            pair(page, [i, j], &lines, &mut m.overlaps, &mut m.crossings);
        }
    }
    for j in &page.junctions {
        let arms = arms(j.at, &j.connections, page, &lines);
        if arms >= 4 {
            m.four_way.push(FourWay {
                junction: j.id,
                arms,
                at: j.at,
            });
        }
    }
    m
}

/// The polyline without consecutive equal points.
fn clean(polyline: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(polyline.len());
    for &p in polyline {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

/// The segments of a cleaned polyline; a single point is one segment of no length.
fn segments(line: &[Point]) -> Vec<(Point, Point)> {
    match line {
        [] => Vec::new(),
        [p] => vec![(*p, *p)],
        _ => line.windows(2).map(|w| (w[0], w[1])).collect(),
    }
}

fn pt2(p: Point) -> Pt2 {
    Pt2::new(Nm(p.0), Nm(p.1))
}

/// Whether `p` lies on the closed segment `a`–`b`, exactly ([`crate::on_segment`]).
fn on(p: Point, (a, b): (Point, Point)) -> bool {
    crate::on_segment(pt2(p), pt2(a), pt2(b))
}

/// (a): the CPs and junctions on `line` that `c` does not end at.
fn touches(page: &PageGeometry, c: &DrawnConnection, line: &[Point], out: &mut Vec<Touch>) {
    let segs = segments(line);
    let hit = |at: Point| segs.iter().any(|s| on(at, *s));
    for cp in &page.cps {
        if !cp.connections.contains(&c.id) && hit(cp.at) {
            out.push(Touch {
                connection: c.id,
                point: TouchedPoint::Cp(cp.id),
                at: cp.at,
            });
        }
    }
    for j in &page.junctions {
        if !j.connections.contains(&c.id) && hit(j.at) {
            out.push(Touch {
                connection: c.id,
                point: TouchedPoint::Junction(j.id),
                at: j.at,
            });
        }
    }
}

/// A non-negative fraction `n / d` of a segment's length, `d > 0`.
#[derive(Clone, Copy, Debug)]
struct Frac {
    n: i128,
    d: i128,
}

impl Frac {
    const ZERO: Frac = Frac { n: 0, d: 1 };
    const ONE: Frac = Frac { n: 1, d: 1 };

    /// `n / d` for `d != 0`, its sign moved to the numerator.
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

fn wide(p: Point) -> (i128, i128) {
    (i128::from(p.0), i128::from(p.1))
}

/// `n / d` rounded to the nearest integer, halves away from zero, `d > 0`.
fn div_round(n: i128, d: i128) -> i128 {
    if n >= 0 {
        (2 * n + d) / (2 * d)
    } else {
        -((-2 * n + d) / (2 * d))
    }
}

/// The point at the fraction `t` (`0 <= t <= 1`) of the segment `a`–`b`, rounded to the nm. A fraction of more than 62
/// bits (a crossing of two slanted segments far out, ADR-0005's ±(2⁵³ − 1) nm) is first shortened, both terms shifted
/// alike, so that a difference of two coordinates times its numerator stays inside `i128`.
fn at(a: Point, b: Point, t: Frac) -> Point {
    let (a, b) = (wide(a), wide(b));
    let (mut n, mut d) = (t.n, t.d);
    while d > 1 << 62 {
        (n, d) = (n >> 1, d >> 1);
    }
    let x = a.0 + div_round((b.0 - a.0) * n, d);
    let y = a.1 + div_round((b.1 - a.1) * n, d);
    (
        i64::try_from(x).unwrap_or(i64::MAX),
        i64::try_from(y).unwrap_or(i64::MAX),
    )
}

/// The fractions `(lo, hi)`, `lo < hi`, of the segment `a`–`b` that lie inside the open rectangle `r`; `None` when
/// the segment does not enter it (a segment along an edge, or touching a corner, stays outside).
fn inside(a: Point, b: Point, r: Rect) -> Option<(Frac, Frac)> {
    let (a, b) = (wide(a), wide(b));
    let (mut lo, mut hi) = (Frac::ZERO, Frac::ONE);
    for (p, d, min, max) in [
        (a.0, b.0 - a.0, i128::from(r[0]), i128::from(r[2])),
        (a.1, b.1 - a.1, i128::from(r[1]), i128::from(r[3])),
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

/// The unit vector of a known direction (y down).
fn vector(d: &Dir4) -> Option<(i128, i128)> {
    match d {
        Dir4::Up => Some((0, -1)),
        Dir4::Down => Some((0, 1)),
        Dir4::Left => Some((-1, 0)),
        Dir4::Right => Some((1, 0)),
        Dir4::Unknown(_) => None,
    }
}

/// `(lo, hi)` of the segment `a`–`b` without the fractions that lie on the escape `e`, the ray from a CP along its
/// leaving direction; unchanged when the segment is not on the ray's line.
fn minus_escape(
    a: Point,
    b: Point,
    (lo, hi): (Frac, Frac),
    e: (Point, (i128, i128)),
) -> Option<(Frac, Frac)> {
    let (a, b, (p, v)) = (wide(a), wide(b), (wide(e.0), e.1));
    let d = (b.0 - a.0, b.1 - a.1);
    let w = (a.0 - p.0, a.1 - p.1);
    if v.0 * d.1 - v.1 * d.0 != 0 || v.0 * w.1 - v.1 * w.0 != 0 {
        return Some((lo, hi));
    }
    // Along the line, (p(t) − e) · v = s0 + t · sd ≥ 0 on the ray; sd ≠ 0 for a segment with a length.
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

/// (c) and (d): the bodies and texts `line` runs through, a body of a representation `c` ends at less the escapes of
/// its end CPs there.
fn through(
    page: &PageGeometry,
    c: &DrawnConnection,
    line: &[Point],
    bodies: &mut Vec<Through>,
    texts: &mut Vec<Through>,
) {
    let escapes: Vec<(Uuid, Point, (i128, i128))> = page
        .cps
        .iter()
        .filter(|cp| cp.connections.contains(&c.id))
        .filter_map(|cp| Some((cp.rep, cp.at, vector(cp.dir.as_ref()?)?)))
        .collect();
    let add = |out: &mut Vec<Through>, rep: Uuid, stretch: (Point, Point)| match out
        .iter_mut()
        .find(|t| t.connection == c.id && t.rep == rep)
    {
        Some(t) => {
            if !t.stretches.contains(&stretch) {
                t.stretches.push(stretch);
            }
        }
        None => out.push(Through {
            connection: c.id,
            rep,
            stretches: vec![stretch],
        }),
    };
    for (a, b) in segments(line) {
        if a == b {
            continue;
        }
        for body in &page.bodies {
            let mut span = inside(a, b, body.rect);
            for &(rep, p, v) in &escapes {
                if rep == body.rep {
                    span = span.and_then(|s| minus_escape(a, b, s, (p, v)));
                }
            }
            if let Some((lo, hi)) = span {
                add(bodies, body.rep, (at(a, b, lo), at(a, b, hi)));
            }
        }
        for text in &page.texts {
            if let Some((lo, hi)) = inside(a, b, text.rect) {
                add(texts, text.rep, (at(a, b, lo), at(a, b, hi)));
            }
        }
    }
}

/// The cross product `(b − a) × (c − a)`.
fn orient(a: Point, b: Point, c: Point) -> i128 {
    let (a, b, c) = (wide(a), wide(b), wide(c));
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// The stretch two segments with a length share on one line, from its smaller end to its larger (by x, on a
/// vertical line by y); `None` when they are not on one line or share at most a point.
fn overlap(s: (Point, Point), t: (Point, Point)) -> Option<(Point, Point)> {
    let ((a, b), (c, d)) = (s, t);
    if a == b || c == d || orient(a, b, c) != 0 || orient(a, b, d) != 0 {
        return None;
    }
    let key = |p: Point| if a.0 != b.0 { p.0 } else { p.1 };
    let sort = |p: Point, q: Point| if key(p) <= key(q) { (p, q) } else { (q, p) };
    let ((s0, s1), (t0, t1)) = (sort(a, b), sort(c, d));
    let lo = if key(s0) >= key(t0) { s0 } else { t0 };
    let hi = if key(s1) <= key(t1) { s1 } else { t1 };
    (key(lo) < key(hi)).then_some((lo, hi))
}

/// The point two segments meet at, when they share exactly one (rounded to the nm for a crossing of segments that
/// are neither horizontal nor vertical); `None` when they are apart or share a stretch.
fn meet(s: (Point, Point), t: (Point, Point)) -> Option<Point> {
    let ((a, b), (c, d)) = (s, t);
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    if o1.signum() * o2.signum() < 0 && o3.signum() * o4.signum() < 0 {
        // A proper crossing: a + (b − a) · ((c − a) × (d − c)) / ((b − a) × (d − c)).
        let (wa, wc, wd) = (wide(a), wide(c), wide(d));
        let num = (wc.0 - wa.0) * (wd.1 - wc.1) - (wc.1 - wa.1) * (wd.0 - wc.0);
        let den = o2 - o1;
        return Some(at(a, b, Frac::new(num, den)));
    }
    [c, d]
        .into_iter()
        .find(|p| on(*p, s))
        .or_else(|| [a, b].into_iter().find(|p| on(*p, t)))
}

/// (b) and (e) for the connections `i` and `j`: the stretches they share and the points they meet at, but where
/// either ends (a shared end, or a touch of (a)) and on a shared stretch.
fn pair(
    page: &PageGeometry,
    [i, j]: [usize; 2],
    lines: &[Vec<Point>],
    overlaps: &mut Vec<Overlap>,
    crossings: &mut Vec<Crossing>,
) {
    let (ci, cj) = (&page.connections[i], &page.connections[j]);
    let (si, sj) = (segments(&lines[i]), segments(&lines[j]));
    let mut stretches: Vec<(Point, Point)> = Vec::new();
    let mut points: Vec<Point> = Vec::new();
    for &s in &si {
        for &t in &sj {
            if let Some(o) = overlap(s, t) {
                if !stretches.contains(&o) {
                    stretches.push(o);
                }
            } else if let Some(p) = meet(s, t)
                && !points.contains(&p)
            {
                points.push(p);
            }
        }
    }
    let ends = |p: Point| {
        let ends_at = |list: &[Uuid]| list.contains(&ci.id) || list.contains(&cj.id);
        page.cps.iter().any(|cp| cp.at == p && ends_at(&cp.connections))
            || page
                .junctions
                .iter()
                .any(|jn| jn.at == p && ends_at(&jn.connections))
    };
    for p in points {
        if !ends(p) && !stretches.iter().any(|s| on(p, *s)) {
            crossings.push(Crossing {
                connections: [ci.id, cj.id],
                at: p,
            });
        }
    }
    if !stretches.is_empty() {
        overlaps.push(Overlap {
            connections: [ci.id, cj.id],
            stretches,
        });
    }
}

/// The bends of a cleaned polyline: the vertices where it changes direction (a vertex it runs straight through is
/// none; a reversal is one).
fn bend_count(line: &[Point]) -> usize {
    line.windows(3)
        .filter(|w| {
            let (a, b, c) = (wide(w[0]), wide(w[1]), wide(w[2]));
            let (u, v) = ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1));
            u.0 * v.1 - u.1 * v.0 != 0 || u.0 * v.0 + u.1 * v.1 < 0
        })
        .count()
}

/// The greatest common divisor of `a` and `b` (non-negative).
fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// (g): the distinct directions the connections `ending` leave the junction at `j` by, each the direction of the
/// polyline's first segment from its end at `j`.
fn arms(j: Point, ending: &[Uuid], page: &PageGeometry, lines: &[Vec<Point>]) -> usize {
    let mut dirs: BTreeSet<(i128, i128)> = BTreeSet::new();
    for (c, line) in page.connections.iter().zip(lines) {
        if !ending.contains(&c.id) || line.len() < 2 {
            continue;
        }
        let next = if line[0] == j {
            line[1]
        } else if line[line.len() - 1] == j {
            line[line.len() - 2]
        } else {
            continue;
        };
        let (dx, dy) = (
            i128::from(next.0) - i128::from(j.0),
            i128::from(next.1) - i128::from(j.1),
        );
        let g = gcd(dx, dy).max(1);
        dirs.insert((dx / g, dy / g));
    }
    dirs.len()
}
