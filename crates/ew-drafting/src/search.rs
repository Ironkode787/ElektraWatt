// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The exact grid search of one route (docs/design/06a-drafting.md §3.6), moved from the router of ADR-0033 and
//! reading the occupancy ([`Occupancy`]): a backward Dijkstra over (node, arriving heading) gives every state its
//! least (cost, bends) to the second end, and a forward walk from the first end takes at each node the lowest-ranked
//! move on a least path (right, down, left, up), so the result is the same for the same page whatever the order of its
//! lists.
//!
//! **The lattice.** The page's grid (a node every [`TICKS_PER_STEP`]), or its half grid (every [`HALF`]); a move of
//! the lattice is one or two moves of the occupancy. An end off the lattice takes its nearest node; the route's first
//! and last vertex are the ends' exact points, and the corner next to an end takes that end's coordinate across the
//! segment between them.
//!
//! **Prices** (§1.2, in ticks, the measure's unit): a step its length; on the half grid a move along a line between
//! the grid's lines [`HALF_STEP_EXTRA`] more per tick; a turn [`BEND`] steps; each wire crossed [`CROSSING`] steps;
//! onto a text that cannot move [`TEXT_CROSSED`] steps and [`TEXT_STEP`] per tick on it; onto a label the text placer
//! may move [`LABEL_RANK`] steps (it will step aside), unless the request names it at its full price.
//!
//! **Not taken:** a node strictly inside a body grown by a step (half a step on the half grid), but along an end's
//! escape corridor out of its own part (the nodes from a pin along its side while inside its own grown body, and the
//! first one outside) and inside a body that holds a first end that is a bend reached over one of its pins a step behind
//! it (the person led the wire through the part); a node that a
//! pin, a pin's stub or a junction
//! reserves that is not the route's (a stub of a pin no wire ends at is free to an end whose own stub lies there); a
//! node of another wire's vertex; a move along another wire; a turn back; on the half grid, a move along a line
//! between the grid's lines but strictly between the two ends, inside their box.
//!
//! **Shared runs** (§3.4). A wire of a pin may start (or end) on the run of a wire laid from the same pin: the search
//! starts from each node of that run with the run's price so far, and so ends where it parts from it; it never runs
//! along it after it has parted.
//!
//! **Ends.** A pin end leaves along its side by [`STUB`] (one move on the half grid) and is entered against it; no
//! move enters the first pin end's node or leaves the second end's. A first end without a side may carry the heading
//! the route arrived there with (a fixed bend): the first move never takes its opposite, and a turn there is free.
//!
//! **Areas:** the box of the two ends grown by each of [`WINDOWS`], then the drawing area; the first with a route
//! wins. An area of more than [`MAX_NODES`] lattice nodes has none.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use ew_ids::Uuid;

use crate::consts::{
    BEND, BODY_GAP, CROSSING, DETOUR_MOST, HALF, HALF_STEP_EXTRA, LABEL_RANK, MAX_NODES, STEP, STUB,
    TEXT_CROSSED, TEXT_STEP, TICKS_PER_STEP, WINDOWS,
};
use crate::geom;
use crate::model::{JunctionId, PinId, Pt, Rect, Side, Tick};
use crate::occupancy::{Holder, Move, Node, Occupancy, node_of};

/// A heading, in the rank order of the tie rule: right < down < left < up.
const HEADINGS: [Side; 4] = [Side::Right, Side::Down, Side::Left, Side::Up];

fn rank(s: Side) -> usize {
    match s {
        Side::Right => 0,
        Side::Down => 1,
        Side::Left => 2,
        Side::Up => 3,
    }
}

/// The heading from `a` to `b` on one line; `None` for one point or a slant.
pub fn heading(a: Pt, b: Pt) -> Option<Side> {
    match (b.x.cmp(&a.x), b.y.cmp(&a.y)) {
        (std::cmp::Ordering::Greater, std::cmp::Ordering::Equal) => Some(Side::Right),
        (std::cmp::Ordering::Less, std::cmp::Ordering::Equal) => Some(Side::Left),
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Greater) => Some(Side::Down),
        (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => Some(Side::Up),
        _ => None,
    }
}

/// What an end of a route is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TipKind {
    /// A pin.
    Pin(PinId),
    /// A junction.
    Junction(JunctionId),
    /// A free point: a fixed bend, or a point on a wire where a junction will be made.
    Free,
}

/// An end of a route as the search reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tip {
    /// Its point.
    pub at: Pt,
    /// The side a wire leaves it on, for a pin with one.
    pub side: Option<Side>,
    /// What it is.
    pub owner: TipKind,
    /// Its own part's ID and body on the page, for a pin.
    pub part: Option<(Uuid, Rect)>,
    /// The routes of the wires laid from the same pin, each from the pin outward: the route may share their run.
    pub shared: Vec<Vec<Pt>>,
}

impl Tip {
    /// A free point.
    pub fn free(at: Pt) -> Tip {
        Tip {
            at,
            side: None,
            owner: TipKind::Free,
            part: None,
            shared: Vec::new(),
        }
    }
}

/// What a search leaves out, to name what is in a wire's way (§3.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Relax {
    /// The other wires.
    pub wires: bool,
    /// The pins, stubs and junctions of other wires.
    pub points: bool,
}

/// One search.
#[derive(Clone, Debug)]
pub struct Ask<'a> {
    /// The page.
    pub occ: &'a Occupancy,
    /// The first end.
    pub from: Tip,
    /// The second end.
    pub to: Tip,
    /// The heading the route arrived at a first end without a side with (a fixed bend).
    pub arrive: Option<Side>,
    /// The drawing area.
    pub area: Rect,
    /// The texts priced in full although the text placer may move them (the second text round, §3.7).
    pub full: &'a [u32],
    /// What it leaves out.
    pub relax: Relax,
}

/// A route found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Found {
    /// Every vertex from the first end to the second.
    pub route: Vec<Pt>,
    /// Its price.
    pub cost: u32,
}

/// The lattice of one search: its step in ticks, its window in lattice indices.
struct Space<'a> {
    ask: &'a Ask<'a>,
    /// The lattice's step, in nodes of the occupancy (2 on the grid, 1 on the half grid).
    f: i32,
    lo: Node,
    hi: Node,
    width: usize,
    /// The first and the second end's lattice nodes.
    s: Node,
    t: Node,
    /// The corridors' nodes of the occupancy, per end.
    corridors: [Vec<Node>; 2],
    /// The outlines drawn round other parts that hold an end's point, per end: its corridor leaves them.
    frames: [Vec<Rect>; 2],
    /// The bodies that hold a first end that is a bend reached over one of their pins a step behind it (the person led the
    /// wire through the part): the route may leave them.
    held: Vec<Rect>,
}

/// Least (cost, bends).
type Dist = (u32, u32);

const INF: Dist = (u32::MAX, u32::MAX);

fn add(a: Dist, b: Dist) -> Dist {
    (a.0.saturating_add(b.0), a.1.saturating_add(b.1))
}

fn price(weight: i64) -> u32 {
    u32::try_from(weight.saturating_mul(i64::from(TICKS_PER_STEP))).unwrap_or(u32::MAX)
}

impl<'a> Space<'a> {
    fn node(&self, p: Node) -> Option<usize> {
        if p.0 < self.lo.0 || p.0 > self.hi.0 || p.1 < self.lo.1 || p.1 > self.hi.1 {
            return None;
        }
        let x = usize::try_from(p.0 - self.lo.0).ok()?;
        let y = usize::try_from(p.1 - self.lo.1).ok()?;
        Some(y * self.width + x)
    }

    fn position(&self, k: usize) -> Node {
        let x = i32::try_from(k % self.width).unwrap_or(0);
        let y = i32::try_from(k / self.width).unwrap_or(0);
        (self.lo.0 + x, self.lo.1 + y)
    }

    /// The occupancy's node of lattice node `p`.
    fn occ(&self, p: Node) -> Node {
        (p.0.saturating_mul(self.f), p.1.saturating_mul(self.f))
    }

    fn half(&self) -> bool {
        self.f == 1
    }

    /// Whether the occupancy's node `n` lies strictly inside `r` grown by the search's body gap.
    fn inside(&self, n: Node, r: Rect) -> bool {
        let g = if self.half() { HALF } else { BODY_GAP };
        let r = r.grown(g);
        let p = crate::occupancy::point_of(n);
        r.contains_strictly(p)
    }

    /// Whether the occupancy's node `n` is closed to the route.
    fn closed(&self, n: Node) -> bool {
        let ask = self.ask;
        let tips = [&ask.from, &ask.to];
        let at_tip = tips.iter().any(|t| node_of(t.at) == n);
        if at_tip {
            return false;
        }
        let mut bodies = i32::from(ask.occ.bodies_at(n, self.half()));
        if bodies > 0 {
            for r in &self.held {
                if self.inside(n, *r) {
                    bodies -= 1;
                }
            }
            for (k, tip) in tips.iter().enumerate() {
                if let Some((_, r)) = tip.part
                    && self.corridors[k].contains(&n)
                    && self.inside(n, r)
                {
                    bodies -= 1;
                }
                for r in &self.frames[k] {
                    if self.corridors[k].contains(&n) && self.inside(n, *r) {
                        bodies -= 1;
                    }
                }
            }
            if bodies > 0 {
                return true;
            }
        }
        if !ask.relax.points {
            for h in ask.occ.holders_at(n) {
                let own = match h {
                    Holder::Pin { pin, .. } => tips.iter().any(|t| t.owner == TipKind::Pin(*pin)),
                    Holder::Junction(j) => tips.iter().any(|t| t.owner == TipKind::Junction(*j)),
                };
                // A free pin's stub on an end's own stub (a second connection point on the same point) is the end's way
                // out, its half step as well as its full one.
                let shared_stub = matches!(
                    h,
                    Holder::Pin {
                        stub: true,
                        wired: false,
                        ..
                    }
                ) && tips.iter().any(|t| {
                    t.side.is_some_and(|s| {
                        let u = s.unit();
                        [HALF, STUB]
                            .iter()
                            .any(|d| node_of(t.at.plus(Pt::new(u.x * d, u.y * d))) == n)
                    })
                });
                if !own && !shared_stub {
                    return true;
                }
            }
        }
        !ask.relax.wires && ask.occ.vertices_at(n) > 0
    }

    /// The moves of the occupancy that lattice move `m` from `p` is made of.
    fn half_moves(&self, p: Node, m: Side) -> impl Iterator<Item = (Move, Node)> + '_ {
        let start = self.occ(p);
        let u = m.unit();
        (0..self.f).map(move |k| {
            let a = (start.0 + u.x * k, start.1 + u.y * k);
            let b = (a.0 + u.x, a.1 + u.y);
            let mv = match m {
                Side::Right => (a, false),
                Side::Down => (a, true),
                Side::Left => (b, false),
                Side::Up => (b, true),
            };
            (mv, b)
        })
    }

    /// Whether lattice move `m` from `p` may be taken (the far node's own closure aside).
    fn free(&self, p: Node, m: Side) -> bool {
        let ask = self.ask;
        // On the half grid, a move between the grid's lines only between the ends.
        if self.half() {
            let track = if m.vertical() { p.0 } else { p.1 };
            if track.rem_euclid(2) != 0 {
                let (lo, hi) = (
                    (self.s.0.min(self.t.0), self.s.1.min(self.t.1)),
                    (self.s.0.max(self.t.0), self.s.1.max(self.t.1)),
                );
                let q = (p.0 + m.unit().x, p.1 + m.unit().y);
                let inside = |v: Node| lo.0 <= v.0 && v.0 <= hi.0 && lo.1 <= v.1 && v.1 <= hi.1;
                let (a, b) = if m.vertical() { (lo.0, hi.0) } else { (lo.1, hi.1) };
                if !(a < track && track < b && inside(p) && inside(q)) {
                    return false;
                }
            }
        }
        for (k, (mv, b)) in self.half_moves(p, m).enumerate() {
            if !ask.relax.wires && ask.occ.along(mv) > 0 {
                return false;
            }
            // The node inside a move of the grid is closed as a node is.
            if k + 1 < usize::try_from(self.f).unwrap_or(1) && self.closed(b) {
                return false;
            }
        }
        true
    }

    /// The texts lattice move `m` from `p` runs through, with the ticks inside each; a text of an end's own part on its
    /// corridor left out.
    fn texts(&self, p: Node, m: Side) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = Vec::new();
        for (mv, b) in self.half_moves(p, m) {
            for &(k, ticks) in self.ask.occ.texts_on(mv) {
                let Some(t) = self.ask.occ.texts.get(usize::try_from(k).unwrap_or(usize::MAX)) else {
                    continue;
                };
                let own = [&self.ask.from, &self.ask.to].iter().enumerate().any(|(e, tip)| {
                    tip.part.is_some_and(|(id, _)| id == t.owner) && self.corridors[e].contains(&b)
                });
                if own {
                    continue;
                }
                match out.iter_mut().find(|(x, _)| *x == k) {
                    Some(e) => e.1 += u32::from(ticks),
                    None => out.push((k, u32::from(ticks))),
                }
            }
        }
        out
    }

    /// The price of lattice move `m` from `p` after the move `before` (the heading the route arrived at `p` with, `None`
    /// at its start), its turn aside.
    fn cost(&self, p: Node, m: Side, before: Option<Side>) -> u32 {
        let ask = self.ask;
        let len = (HALF * self.f).unsigned_abs();
        let mut c = len.saturating_mul(u32::try_from(STEP).unwrap_or(1));
        if self.half() {
            let track = if m.vertical() { p.0 } else { p.1 };
            if track.rem_euclid(2) != 0 {
                c = c.saturating_add(len.saturating_mul(u32::try_from(HALF_STEP_EXTRA).unwrap_or(1)));
            }
        }
        if !ask.relax.wires {
            for (mv, _) in self.half_moves(p, m) {
                c = c.saturating_add(price(CROSSING).saturating_mul(u32::from(ask.occ.across(mv))));
            }
        }
        let texts = self.texts(p, m);
        if !texts.is_empty() {
            let came: Vec<u32> = match before {
                Some(h) => {
                    let back = (p.0 - h.unit().x, p.1 - h.unit().y);
                    self.texts(back, h).into_iter().map(|(k, _)| k).collect()
                }
                None => Vec::new(),
            };
            for (k, ticks) in texts {
                let movable = ask
                    .occ
                    .texts
                    .get(usize::try_from(k).unwrap_or(usize::MAX))
                    .is_some_and(|t| t.label.is_some())
                    && !ask.full.contains(&k);
                let enter = !came.contains(&k);
                if movable {
                    if enter {
                        c = c.saturating_add(price(LABEL_RANK));
                    }
                } else {
                    c = c.saturating_add(ticks.saturating_mul(u32::try_from(TEXT_STEP).unwrap_or(1)));
                    if enter {
                        c = c.saturating_add(price(TEXT_CROSSED));
                    }
                }
            }
        }
        c
    }
}

/// The weight of move `m` from a state that arrived with `h` and costs `c` without its turn.
fn weight(h: Option<Side>, m: Side, c: u32) -> Dist {
    let bend = u32::from(h.is_some_and(|h| h != m));
    (c.saturating_add(price(BEND).saturating_mul(bend)), bend)
}

/// The state index of (node, arriving heading); `None` is the fifth slot.
fn state(node: usize, h: Option<Side>) -> usize {
    node * 5 + h.map_or(4, rank)
}

/// A start of the forward walk: its state, its price so far and the route up to it.
struct Start {
    node: Node,
    heading: Option<Side>,
    dist: Dist,
    route: Vec<Pt>,
    /// The order among equal starts: the stub first, then the shared runs by how far they go.
    order: (usize, usize),
}

/// The nodes of the lattice (step `f` nodes of the occupancy) along `run` from its first point, each with the heading
/// it was reached with and the route up to it; it stops where the run leaves the lattice.
fn along_run(run: &[Pt], f: i32) -> Vec<(Node, Side, Vec<Pt>)> {
    let pts = geom::clean(run);
    let step = HALF * f;
    let mut out = Vec::new();
    let mut so_far: Vec<Pt> = pts.first().copied().into_iter().collect();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let Some(h) = heading(a, b) else { break };
        let on = |p: Pt| p.x.rem_euclid(step) == 0 && p.y.rem_euclid(step) == 0;
        let u = h.unit();
        let len = (b.x - a.x).abs() + (b.y - a.y).abs();
        let mut d = 0;
        while d < len {
            d += 1;
            let p = Pt::new(a.x + u.x * d, a.y + u.y * d);
            if on(p) {
                let mut r = so_far.clone();
                r.push(p);
                out.push(((p.x / step, p.y / step), h, r));
            }
        }
        if !on(b) {
            break;
        }
        so_far.push(b);
    }
    out
}

/// The route of `ask` on the grid (`half: false`) or the half grid, in the areas of the module documentation.
pub fn search(ask: &Ask<'_>, half: bool) -> Option<Found> {
    if ask.from.at == ask.to.at {
        return Some(Found {
            route: vec![ask.from.at],
            cost: 0,
        });
    }
    let (a, b) = (ask.from.at, ask.to.at);
    let ends = Rect::new(a, b);
    for grow in WINDOWS.iter().map(|g| Some(*g)).chain([None]) {
        let window = match grow {
            Some(g) => {
                let w = ends.grown(g);
                Rect {
                    x0: w.x0.max(ask.area.x0),
                    y0: w.y0.max(ask.area.y0),
                    x1: w.x1.min(ask.area.x1),
                    y1: w.y1.min(ask.area.y1),
                }
            }
            None => ask.area,
        };
        if let Some(found) = search_in(ask, half, window) {
            return Some(found);
        }
    }
    None
}

/// The space of `ask` in `window` (the drawing area kept clear by a step at its edge), extended to hold the ends,
/// their stubs and their shared runs; `None` for an area over [`MAX_NODES`].
fn space<'a>(ask: &'a Ask<'a>, half: bool, window: Rect) -> Option<Space<'a>> {
    let f = if half { 1 } else { 2 };
    let step = HALF * f;
    let near = |v: Tick| {
        i32::try_from((i64::from(v) + i64::from(step / 2)).div_euclid(i64::from(step))).unwrap_or(0)
    };
    let lat = |p: Pt| (near(p.x), near(p.y));
    let (s, t) = (lat(ask.from.at), lat(ask.to.at));
    let inset = window.grown(-BODY_GAP);
    let ceil = |v: Tick| i32::try_from(i64::from(v).div_euclid(i64::from(step)) + 1).unwrap_or(0);
    let floor = |v: Tick| i32::try_from(i64::from(v).div_euclid(i64::from(step))).unwrap_or(0);
    let (mut lo, mut hi) = if inset.x0 < inset.x1 && inset.y0 < inset.y1 {
        (
            (ceil(inset.x0 - 1), ceil(inset.y0 - 1)),
            (floor(inset.x1), floor(inset.y1)),
        )
    } else {
        (s, s)
    };
    let mut hold = |p: Node| {
        lo = (lo.0.min(p.0), lo.1.min(p.1));
        hi = (hi.0.max(p.0), hi.1.max(p.1));
    };
    for tip in [&ask.from, &ask.to] {
        let n = lat(tip.at);
        hold(n);
        if let Some(side) = tip.side {
            hold((n.0 + side.unit().x, n.1 + side.unit().y));
        }
        for run in &tip.shared {
            for (n, _, _) in along_run(run, f) {
                hold(n);
            }
        }
    }
    let width = usize::try_from(i64::from(hi.0) - i64::from(lo.0) + 1).ok()?;
    let height = usize::try_from(i64::from(hi.1) - i64::from(lo.1) + 1).ok()?;
    if width.checked_mul(height)? > MAX_NODES {
        return None;
    }
    let mut sp = Space {
        ask,
        f,
        lo,
        hi,
        width,
        s,
        t,
        corridors: [Vec::new(), Vec::new()],
        frames: [&ask.from, &ask.to].map(|tip| {
            // (And a part of one pin set on the end's own point, a mark on a connection point: it does not wall the
            // pin in.)
            let rides =
                ask.occ.bodies.iter().filter(|(id, _, pins)| {
                    pins[..] == [tip.at] && tip.part.is_some_and(|(own, _)| own != id.0)
                });
            ask.occ
                .frames
                .iter()
                .chain(rides.map(|(_, r, _)| r))
                .filter(|r| r.contains(tip.at))
                .copied()
                .collect()
        }),
        held: ask
            .occ
            .bodies
            .iter()
            .filter(|(_, r, pins)| {
                let t = &ask.from;
                let inside = r.contains_strictly(t.at);
                // Reached over one of its pins, a step behind the bend: the person led the wire through the part.
                let over_pin = ask.arrive.is_some_and(|h| {
                    let u = h.unit();
                    pins.iter()
                        .any(|p| (1..=STUB).any(|d| t.at.minus(Pt::new(u.x * d, u.y * d)) == *p))
                });
                t.owner == TipKind::Free && inside && over_pin
            })
            .map(|(_, r, _)| *r)
            .collect(),
    };
    for (k, tip) in [&ask.from, &ask.to].into_iter().enumerate() {
        let Some(side) = tip.side else { continue };
        let u = side.unit();
        // From the end's node of the lattice searched: an end off the grid takes the lattice's nearest line.
        let mut n = sp.occ(if k == 0 { sp.s } else { sp.t });
        let mut out = vec![n];
        for _ in 0..64 {
            n = (n.0 + u.x, n.1 + u.y);
            out.push(n);
            let inside = tip.part.is_some_and(|(_, r)| sp.inside(n, r))
                || sp.frames[k].iter().any(|r| sp.inside(n, *r));
            let stub = crate::occupancy::point_of(n);
            let within = (stub.x - tip.at.x).abs() + (stub.y - tip.at.y).abs() < STUB;
            if !inside && !within {
                break;
            }
        }
        sp.corridors[k] = out;
    }
    Some(sp)
}

fn search_in(ask: &Ask<'_>, half: bool, window: Rect) -> Option<Found> {
    let sp = space(ask, half, window)?;
    let (from, to) = (&ask.from, &ask.to);
    let nodes = sp.width * usize::try_from(sp.hi.1 - sp.lo.1 + 1).ok()?;
    let goal = sp.node(sp.t)?;
    let f = sp.f;
    // The starts: the stub of a pin end (or the end itself), and each node of a run it may share past its stub.
    let mut starts: Vec<Start> = Vec::new();
    match from.side {
        Some(d) => {
            let q = (sp.s.0 + d.unit().x, sp.s.1 + d.unit().y);
            if sp.node(q).is_some() && sp.free(sp.s, d) && (q == sp.t || !sp.closed(sp.occ(q))) {
                starts.push(Start {
                    node: q,
                    heading: Some(d),
                    dist: weight(None, d, sp.cost(sp.s, d, None)),
                    route: vec![from.at],
                    order: (0, 0),
                });
            }
        }
        None => starts.push(Start {
            node: sp.s,
            heading: ask.arrive,
            dist: (0, 0),
            route: vec![from.at],
            order: (0, 0),
        }),
    }
    for (r, run) in from.shared.iter().enumerate() {
        let mut dist: Dist = (0, 0);
        let mut before: Option<Side> = None;
        let mut p = sp.s;
        for (k, (n, h, route)) in along_run(run, f).into_iter().enumerate() {
            dist = add(dist, weight(before, h, sp.cost(p, h, before)));
            before = Some(h);
            p = n;
            if sp.node(n).is_some() {
                starts.push(Start {
                    node: n,
                    heading: Some(h),
                    dist,
                    route,
                    order: (1 + r, k),
                });
            }
        }
    }
    let start_states: Vec<usize> = starts
        .iter()
        .filter_map(|s| sp.node(s.node).map(|k| state(k, s.heading)))
        .collect();
    // The goal states: the second end entered against its side (any heading at an end without one), and each node of
    // a run it may share, with the price of that run back to the end.
    let mut dist: Vec<Dist> = vec![INF; nodes * 5];
    let mut seeds: BTreeMap<usize, (Dist, Vec<Pt>)> = BTreeMap::new();
    let mut heap: BinaryHeap<Reverse<(Dist, usize)>> = BinaryHeap::new();
    let mut put = |st: usize, d: Dist, tail: Vec<Pt>, dist: &mut Vec<Dist>, heap: &mut BinaryHeap<_>| {
        if d < dist[st] {
            dist[st] = d;
            heap.push(Reverse((d, st)));
            seeds.insert(st, (d, tail));
        }
    };
    match to.side {
        Some(d) => put(
            state(goal, Some(d.opposite())),
            (0, 0),
            vec![to.at],
            &mut dist,
            &mut heap,
        ),
        None => {
            for h in [
                None,
                Some(Side::Right),
                Some(Side::Down),
                Some(Side::Left),
                Some(Side::Up),
            ] {
                put(state(goal, h), (0, 0), vec![to.at], &mut dist, &mut heap);
            }
        }
    }
    for run in &to.shared {
        for (n, _, route) in along_run(run, f) {
            let Some(idx) = sp.node(n) else { continue };
            // The run from `n` back to the end: the route's tail.
            let mut tail = route;
            tail.reverse();
            let mut cost: Dist = (0, 0);
            let mut before: Option<Side> = None;
            let mut first: Option<Side> = None;
            let mut p = n;
            for w in tail.windows(2) {
                let Some(h) = heading(w[0], w[1]) else { break };
                first.get_or_insert(h);
                let len = (w[1].x - w[0].x).abs() + (w[1].y - w[0].y).abs();
                for _ in 0..len / (HALF * f) {
                    cost = add(cost, weight(before, h, sp.cost(p, h, before)));
                    before = Some(h);
                    p = (p.0 + h.unit().x, p.1 + h.unit().y);
                }
            }
            let Some(g0) = first else { continue };
            for h in HEADINGS {
                if h == g0.opposite() {
                    continue;
                }
                let turn = u32::from(h != g0);
                let d = add(cost, (price(BEND).saturating_mul(turn), turn));
                put(state(idx, Some(h)), d, tail.clone(), &mut dist, &mut heap);
            }
        }
    }
    // A closed node is passed by no route; it may only be a start (in that start's state) or an end.
    let open =
        |p: Node, st: usize| p == sp.s || p == sp.t || !sp.closed(sp.occ(p)) || start_states.contains(&st);
    let first_pin = from.side.and_then(|_| sp.node(sp.s)).filter(|&n| n != goal);
    let free_turn = |st: usize| {
        from.side.is_none() && ask.arrive.is_some() && sp.node(sp.s).map(|k| state(k, ask.arrive)) == Some(st)
    };
    let best = |dist: &Vec<Dist>| {
        starts
            .iter()
            .zip(&start_states)
            .filter(|(_, st)| dist[**st] != INF)
            .map(|(s, st)| add(s.dist, dist[*st]))
            .min()
            .unwrap_or(INF)
    };
    // The backward Dijkstra: the least (cost, bends) from every state to the second end.
    while let Some(Reverse((d, st))) = heap.pop() {
        if d > dist[st] {
            continue;
        }
        if d > best(&dist) {
            break;
        }
        let (n2, slot) = (st / 5, st % 5);
        if slot == 4 {
            continue;
        }
        let m = HEADINGS[slot];
        let p2 = sp.position(n2);
        let p1 = (p2.0 - m.unit().x, p2.1 - m.unit().y);
        let Some(n1) = sp.node(p1) else { continue };
        if n1 == goal || first_pin == Some(n1) || !sp.free(p1, m) {
            continue;
        }
        for h in [
            None,
            Some(Side::Right),
            Some(Side::Down),
            Some(Side::Left),
            Some(Side::Up),
        ] {
            if h == Some(m.opposite()) {
                continue;
            }
            let s1 = state(n1, h);
            if !open(p1, s1) {
                continue;
            }
            let c = sp.cost(p1, m, h);
            let w = if free_turn(s1) {
                weight(None, m, c)
            } else {
                weight(h, m, c)
            };
            let cand = add(w, d);
            if cand < dist[s1] {
                dist[s1] = cand;
                heap.push(Reverse((cand, s1)));
            }
        }
    }
    // The start of least price; among equal ones the stub, then the shorter shared run.
    let (total, _, s) = starts
        .iter()
        .zip(&start_states)
        .filter(|(_, st)| dist[**st] != INF)
        .map(|(s, st)| (add(s.dist, dist[*st]), s.order, s))
        .min_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)))?;
    // The forward walk: the lowest-ranked move on a least path, until a goal state's own price is reached.
    let mut pts: Vec<Pt> = s.route.clone();
    let (mut node, mut heading) = (s.node, s.heading);
    let mut moves = 0usize;
    let tail: Vec<Pt> = loop {
        let k = sp.node(node)?;
        let here_state = state(k, heading);
        let here = dist[here_state];
        if let Some((v, tail)) = seeds.get(&here_state)
            && *v == here
        {
            break tail.clone();
        }
        let next = HEADINGS.into_iter().find_map(|m| {
            if heading == Some(m.opposite()) {
                return None;
            }
            let q = (node.0 + m.unit().x, node.1 + m.unit().y);
            let kq = sp.node(q)?;
            if first_pin == Some(kq) || !sp.free(node, m) {
                return None;
            }
            let there_state = state(kq, Some(m));
            let there = dist[there_state];
            let stops = seeds.get(&there_state).is_some_and(|(v, _)| *v == there);
            if !stops && !open(q, there_state) {
                return None;
            }
            let c = sp.cost(node, m, heading);
            let w = if free_turn(here_state) {
                weight(None, m, c)
            } else {
                weight(heading, m, c)
            };
            (there != INF && add(w, there) == here).then_some((q, m))
        });
        let (q, m) = next?;
        if heading != Some(m) {
            pts.push(crate::occupancy::point_of(sp.occ(node)));
        }
        node = q;
        heading = Some(m);
        moves += 1;
        if moves > nodes * 5 {
            return None;
        }
    };
    // The tail: the second end itself (in place of its node on the lattice), or the shared run from where the walk
    // ended to it.
    pts.extend(tail);
    if pts.last() != Some(&to.at) {
        pts.push(to.at);
    }
    Some(Found {
        route: fit(pts),
        cost: total.0,
    })
}

/// `pts` cleaned, its corners next to the ends on the ends' coordinates (an end off the lattice): the corner after the
/// first end takes the end's coordinate across the first segment, the corner before the last end the last end's. A
/// straight route between two ends that are not on one line (each off the lattice beside it) takes a jog at the grid
/// line nearest its middle, so that it stays square.
pub fn fit(pts: Vec<Pt>) -> Vec<Pt> {
    let mut pts = geom::clean(&pts);
    if let [a, b] = pts[..]
        && a.x != b.x
        && a.y != b.y
    {
        let near = |v: i64| -> Tick {
            let s = i64::from(TICKS_PER_STEP);
            Tick::try_from((v + s / 2).div_euclid(s) * s).unwrap_or_default()
        };
        let mid = |p: Tick, q: Tick| near((i64::from(p) + i64::from(q)).div_euclid(2));
        pts = if (b.y - a.y).abs() >= (b.x - a.x).abs() {
            let y = mid(a.y, b.y);
            vec![a, Pt::new(a.x, y), Pt::new(b.x, y), b]
        } else {
            let x = mid(a.x, b.x);
            vec![a, Pt::new(x, a.y), Pt::new(x, b.y), b]
        };
    }
    let n = pts.len();
    if n >= 3 {
        let (a, b, c) = (pts[0], pts[1], pts[2]);
        // The first segment runs along the second's perpendicular: the corner takes the end's coordinate across it.
        if b.x == c.x {
            pts[1].y = a.y;
        } else if b.y == c.y {
            pts[1].x = a.x;
        }
        let (z, y, x) = (pts[n - 1], pts[n - 2], pts[n - 3]);
        if y.x == x.x {
            pts[n - 2].y = z.y;
        } else if y.y == x.y {
            pts[n - 2].x = z.x;
        }
    }
    clean_collinear(&pts)
}

/// The route without equal neighbours and without interior vertices it runs straight through.
pub fn clean_collinear(route: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(route.len());
    for &p in route {
        if out.last() == Some(&p) {
            continue;
        }
        out.push(p);
        while out.len() >= 3 {
            let k = out.len();
            let (a, b, c) = (out[k - 3], out[k - 2], out[k - 1]);
            let on_line = (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y);
            let straight = on_line && heading(a, b).is_some() && heading(a, b) == heading(b, c);
            if !straight {
                break;
            }
            out.remove(k - 2);
        }
    }
    out
}

/// The price of `route` from `ask.from` to `ask.to` as the search pays it, read on the half grid (on the grid's lines
/// the same price): each move of its runs on the half grid's lines priced as a move of the search, a stretch off them
/// (next to an end off the grid) by its length, each corner a bend; `None` for a slanted route.
pub fn price_of(ask: &Ask<'_>, route: &[Pt]) -> Option<u32> {
    let pts = clean_collinear(route);
    let mut window = Rect::new(ask.from.at, ask.to.at);
    for p in &pts {
        window = window.union(Rect::new(*p, *p));
    }
    let sp = space(ask, true, window.grown(BODY_GAP + HALF))?;
    let mut total: u32 = 0;
    let mut before: Option<Side> = None;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let h = heading(a, b)?;
        let len = (b.x - a.x).abs() + (b.y - a.y).abs();
        let on_line = if h.vertical() {
            a.x.rem_euclid(HALF) == 0
        } else {
            a.y.rem_euclid(HALF) == 0
        };
        let along_start = if h.vertical() { a.y } else { a.x };
        if !on_line || along_start.rem_euclid(HALF) != 0 || len.rem_euclid(HALF) != 0 {
            let (c, _) = weight(before, h, len.unsigned_abs());
            total = total.saturating_add(c);
            before = Some(h);
            continue;
        }
        let mut p = (a.x / HALF, a.y / HALF);
        for _ in 0..len / HALF {
            let (c, _) = weight(before, h, sp.cost(p, h, before));
            total = total.saturating_add(c);
            before = Some(h);
            p = (p.0 + h.unit().x, p.1 + h.unit().y);
        }
    }
    Some(total)
}

/// The route of `ask` by the half-grid rule (§3.6): on the grid; where it has none, or one more than 8/5 of the
/// city-block distance of the ends, on the half grid, whose route is taken where the grid's is more than 8/5 of it or
/// there is none. A debug build asserts that the route is tidy (`geom::tidy`, FT-2097), so that the phase that would
/// make one that is not is found; a route put back from the page or kept is not the search's and is not asserted.
pub fn route(ask: &Ask<'_>) -> Option<Found> {
    let whole = search(ask, false);
    let length = |r: &[Pt]| -> i64 { geom::segments(r).iter().map(|s| geom::length(*s)).sum() };
    let direct = geom::length((ask.from.at, ask.to.at));
    let far = |longer: i64, than: i64| longer * DETOUR_MOST.1 > than * DETOUR_MOST.0;
    let found = if whole.as_ref().is_some_and(|w| !far(length(&w.route), direct)) {
        whole
    } else {
        match (whole, search(ask, true)) {
            (Some(w), Some(h)) if !far(length(&w.route), length(&h.route)) => Some(w),
            (w, None) => w,
            (_, h) => h,
        }
    };
    debug_assert!(
        found.as_ref().is_none_or(|f| geom::tidy(&f.route)),
        "the search found {found:?}"
    );
    found
}

/// Whether `route` from `ask.from` to `ask.to` is one the search could take: walked on the grid's lattice where every
/// vertex lies on it (else the half grid's), every move free and every node it passes open, a pin end left along its
/// side and entered against it, no turn back; a stretch off the lattice next to an end (a pin off the grid) is not
/// judged.
pub fn clear(ask: &Ask<'_>, route: &[Pt]) -> bool {
    let pts = clean_collinear(route);
    let (Some(&first), Some(&last)) = (pts.first(), pts.last()) else {
        return false;
    };
    if first != ask.from.at || last != ask.to.at {
        return false;
    }
    let on = |p: Pt, s: Tick| p.x.rem_euclid(s) == 0 && p.y.rem_euclid(s) == 0;
    let half = !pts.iter().all(|p| on(*p, TICKS_PER_STEP));
    let mut window = Rect::new(first, last);
    for p in &pts {
        window = window.union(Rect::new(*p, *p));
    }
    let Some(sp) = space(ask, half, window.grown(BODY_GAP + HALF)) else {
        return false;
    };
    let step = HALF * sp.f;
    let n = pts.len();
    let mut before: Option<Side> = None;
    for (k, w) in pts.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let Some(h) = heading(a, b) else { return false };
        if before == Some(h.opposite()) {
            return false;
        }
        if k == 0 && ask.from.side.is_some_and(|s| s != h) {
            return false;
        }
        if k + 2 == n && ask.to.side.is_some_and(|s| s.opposite() != h) {
            return false;
        }
        before = Some(h);
        if !on(a, step) || !on(b, step) {
            continue;
        }
        let mut p = (a.x / step, a.y / step);
        let q_end = (b.x / step, b.y / step);
        while p != q_end {
            if !sp.free(p, h) {
                return false;
            }
            let q = (p.0 + h.unit().x, p.1 + h.unit().y);
            if q != sp.s && q != sp.t && sp.closed(sp.occ(q)) {
                return false;
            }
            p = q;
        }
    }
    true
}
