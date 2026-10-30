// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The orthogonal router (WP-S2.04 §8.1, D7; doc 06 §5.4): a pure, integer-only function from two ends, obstacle
//! rectangles, the connection grid and a search area to a route, the full vertex list from the first end to the
//! second. The Wire tool calls it (through `ew_api::routing`) for its ghost and passes the result as
//! `connection.draw`'s `route`, so the command stores what the ghost showed and the router never runs on replay.
//!
//! **The search.** Grid nodes inside the area (the area's nearest grid indices, extended to hold both ends and
//! their stubs), 4-neighbourhood, cost 1 per step and 4 per bend, no move back against the arriving heading (a
//! U-turn would cost one bend only; FT-437). A node whose page point lies strictly inside an
//! obstacle rectangle is blocked, and a move that runs through the open inside of one is not taken (FT-706: a text
//! thinner than a grid step lies between two nodes); the two ends, their nearest grid nodes and their stub nodes
//! are never blocked, nor the move from a CP end to its stub. The nodes of the [`RouteRequest::reserved`] points
//! (the other CPs and their stubs, FT-706) are blocked. A CP end
//! (one with a leaving direction) first leaves one step along its direction (the stub) and is entered along the
//! opposite of its direction; a junction or split end (`dir: None`) has no such constraint. A first end without a
//! direction may carry the heading the route arrived there with ([`RouteRequest::arrive`], a fixed bend's incoming
//! direction; FT-437): the first move never takes its opposite, so the route never doubles back over the segment
//! before the bend; a turn there costs nothing (the bend was fixed to turn at), so the tie rule is otherwise the one
//! of an end without a direction.
//!
//! **The ends' own representations** ([`RouteRequest::own`], FT-706). The rectangles of the two ends' own
//! representations (body, black-box outline, texts) block like the other obstacles, except along a CP end's escape
//! corridor: the nodes from the end along its direction, the stub and every further node while it, or the move on
//! out of it, lies inside one of those rectangles, so that a wire leaves and enters a CP straight out of its own box
//! and texts and never across them.
//!
//! **Other wires** ([`RouteRequest::wires`], FT-706; FR-CON-031: a crossing is not a junction). The route
//! never runs on top of another connection's segment: a unit move parallel to a segment closer than one grid step
//! (`⌊num/den⌋ − 1` nm, so that a coordinate 1 nm off a rational grid still parts by a step) whose extent overlaps
//! it with a positive length is not taken, and the node of every vertex of another connection (its ends and
//! corners) is blocked, so a route crosses another wire only at a right angle through the inside of a segment. The
//! moves of the ends' corridors are exempt (a second wire on a CP leaves along the same stub).
//!
//! **The tie rule** (NFR-REL-07). Among the paths of least cost the result is the least by (bends, length, move
//! sequence), the move sequence being the whole list of unit moves from the first end to the second, each ranked
//! right < down < left < up and compared move by move. It is found exactly: a backward Dijkstra gives every state
//! (node, arriving heading) its least (cost, bends) to the second end, then a forward walk from the first end takes
//! at each node the lowest-ranked move that stays on a least path. (This is the A* of §8.1 with its frontier order
//! made explicit: the result does not depend on the order of the obstacles or of any container.)
//!
//! **The result.** The first end, the interior corners on the grid, the second end, exactly. A corner next to an
//! end takes that end's coordinate on the axis of the segment between them (a CP may sit up to 1 nm off a rational
//! grid; `geometry::require_route` accepts a coordinate equal to an end CP's), collinear and duplicate interior
//! vertices removed. Without a path inside the area (or with an unusable grid or an area of more than
//! [`MAX_NODES`] nodes) the result is the provisional L route of `ew-scene`'s rule: a straight segment for aligned
//! ends, else one corner from the ends' own coordinates.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use ew_units::{Grid, Nm};

/// The largest search area, in grid nodes, beyond which the router falls back to the L route: 2¹⁸ = 262 144, an A0
/// sheet (1189 × 841 mm) at a 2.5 mm grid (476 × 337 = 160 412 nodes) with room to spare (fix round of WP-S2.04
/// part 2, item 3: the distance table is five states per node).
pub const MAX_NODES: usize = 262_144;

/// The cost of a bend, in steps.
pub const BEND_COST: u32 = 4;

/// A heading on the page (y down), in the rank order of the tie rule: right < down < left < up.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Heading {
    /// +x.
    Right,
    /// +y.
    Down,
    /// −x.
    Left,
    /// −y.
    Up,
}

impl Heading {
    /// Every heading in rank order.
    pub const ALL: [Heading; 4] = [Heading::Right, Heading::Down, Heading::Left, Heading::Up];

    /// The unit step in grid indices.
    pub fn step(self) -> (i64, i64) {
        match self {
            Heading::Right => (1, 0),
            Heading::Down => (0, 1),
            Heading::Left => (-1, 0),
            Heading::Up => (0, -1),
        }
    }

    /// The opposite heading.
    pub fn opposite(self) -> Heading {
        match self {
            Heading::Right => Heading::Left,
            Heading::Down => Heading::Up,
            Heading::Left => Heading::Right,
            Heading::Up => Heading::Down,
        }
    }

    /// Whether it runs along y.
    pub fn vertical(self) -> bool {
        matches!(self, Heading::Down | Heading::Up)
    }

    fn index(self) -> usize {
        match self {
            Heading::Right => 0,
            Heading::Down => 1,
            Heading::Left => 2,
            Heading::Up => 3,
        }
    }
}

/// One end of a route: its exact page point and, for a connection point, the direction a wire leaves it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RouteEnd {
    /// The page point, nm.
    pub at: (i64, i64),
    /// The leaving direction of a CP end; `None` for a junction or a split point.
    pub dir: Option<Heading>,
}

/// What the router routes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RouteRequest {
    /// The first end.
    pub from: RouteEnd,
    /// The second end.
    pub to: RouteEnd,
    /// The obstacle rectangles `[x0, y0, x1, y1]` in page nm (any corner order): the bodies of the representations
    /// on the page other than those of the two ends, inflated by one grid step.
    pub obstacles: Vec<[i64; 4]>,
    /// The connection grid `(num, den)` nm.
    pub grid: (i64, i64),
    /// The search area `[x0, y0, x1, y1]` in page nm: the bounding box of both ends grown by 4 grid steps, clipped to
    /// the page's drawing area.
    pub area: [i64; 4],
    /// The heading the route arrived at `from` with, for a first end without a direction (a fixed bend's incoming
    /// direction, FT-437): the first move never takes its opposite; a turn at `from` costs no bend. Ignored for a CP
    /// end (one with `dir`).
    pub arrive: Option<Heading>,
    /// The rectangles `[x0, y0, x1, y1]` of the first and the second end's own representation (body, black-box
    /// outline, texts; page nm, any corner order, inflated as the caller inflates `obstacles`): obstacles, except
    /// along a CP end's escape corridor (module documentation; FT-706). Empty for an end without one.
    pub own: [Vec<[i64; 4]>; 2],
    /// The segments of the other connections on the page, page nm: the route never runs on top of one nor touches
    /// one of their vertices (module documentation; FT-706). Empty: no other wire.
    pub wires: Vec<[(i64, i64); 2]>,
    /// Points no route passes through, page nm: the page's other connection points and their stubs, kept for their
    /// own wires (FT-706). The node nearest to each is blocked (but for the ends and their stubs).
    pub reserved: Vec<(i64, i64)>,
}

/// A route and how it was found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Routed {
    /// The full vertex list from the first end to the second.
    pub points: Vec<(i64, i64)>,
    /// Whether it is the provisional L route of the fallback: no path inside the area (or an unusable grid or an
    /// area over [`MAX_NODES`]), so the route may cross obstacles and other wires (FT-706: the tool says so).
    pub direct: bool,
}

/// The provisional L route of `ew-scene`'s rule (the fallback): `[a, b]` for aligned ends (`[a]` for equal ones),
/// else one corner from the ends' own coordinates, chosen by `a`'s direction, else `b`'s, else `(b.x, a.y)`.
pub fn l_route(a: RouteEnd, b: RouteEnd) -> Vec<(i64, i64)> {
    let (pa, pb) = (a.at, b.at);
    if pa == pb {
        return vec![pa];
    }
    if pa.0 == pb.0 || pa.1 == pb.1 {
        return vec![pa, pb];
    }
    let vertical_first = (pa.0, pb.1);
    let horizontal_first = (pb.0, pa.1);
    let corner = match (a.dir, b.dir) {
        (Some(d), _) if d.vertical() => vertical_first,
        (Some(_), _) => horizontal_first,
        (None, Some(d)) if d.vertical() => horizontal_first,
        (None, Some(_)) => vertical_first,
        (None, None) => horizontal_first,
    };
    vec![pa, corner, pb]
}

/// The search space of one request.
struct Space {
    grid: Grid,
    /// The index bounds, inclusive.
    lo: (i64, i64),
    hi: (i64, i64),
    width: usize,
    blocked: Vec<bool>,
    /// Why the move right from a node is not taken: [`ON_WIRE`], [`OVER_OWN`], [`OVER_OBSTACLE`] (FT-706); 0 when it
    /// is free.
    no_right: Vec<u8>,
    /// Why the move down from a node is not taken.
    no_down: Vec<u8>,
}

/// A move on another wire's track (FT-706).
const ON_WIRE: u8 = 1;
/// A move across one of the ends' own rectangles (FT-706).
const OVER_OWN: u8 = 2;
/// A move across an obstacle rectangle (FT-706).
const OVER_OBSTACLE: u8 = 4;

impl Space {
    /// Whether the unit move `m` from the index point `p` is free: on no other wire's track and across no rectangle
    /// (the target node's own blocking aside).
    fn move_free(&self, p: (i64, i64), m: Heading) -> bool {
        let (from, bits) = match m {
            Heading::Right => (p, &self.no_right),
            Heading::Down => (p, &self.no_down),
            Heading::Left => ((p.0 - 1, p.1), &self.no_right),
            Heading::Up => ((p.0, p.1 - 1), &self.no_down),
        };
        self.node(from).is_some_and(|n| bits[n] == 0)
    }

    /// The flags of the move from the index point `p` to its neighbour `q` (0 outside the bounds).
    fn move_bits(&self, p: (i64, i64), q: (i64, i64)) -> u8 {
        let from = (p.0.min(q.0), p.1.min(q.1));
        self.node(from).map_or(0, |n| {
            if p.1 == q.1 {
                self.no_right[n]
            } else {
                self.no_down[n]
            }
        })
    }

    /// The flags of the move from the index point `p` to its neighbour `q`, for change.
    fn move_flags(&mut self, p: (i64, i64), q: (i64, i64)) -> Option<&mut u8> {
        let from = (p.0.min(q.0), p.1.min(q.1));
        let n = self.node(from)?;
        Some(if p.1 == q.1 {
            &mut self.no_right[n]
        } else {
            &mut self.no_down[n]
        })
    }

    fn node(&self, p: (i64, i64)) -> Option<usize> {
        if p.0 < self.lo.0 || p.0 > self.hi.0 || p.1 < self.lo.1 || p.1 > self.hi.1 {
            return None;
        }
        let x = usize::try_from(p.0 - self.lo.0).ok()?;
        let y = usize::try_from(p.1 - self.lo.1).ok()?;
        Some(y * self.width + x)
    }

    fn coord(&self, k: i64) -> Option<i64> {
        self.grid.at(k).map(|n| n.0)
    }
}

/// The cost of a move with heading `m` from a state that arrived with `h`: `(steps + 4 × bend, bend)`.
fn weight(h: Option<Heading>, m: Heading) -> (u32, u32) {
    let bend = u32::from(h.is_some_and(|h| h != m));
    (1 + BEND_COST * bend, bend)
}

/// Whether a move with heading `m` from a state that arrived with `h` turns back on itself: never taken, so a route
/// never doubles back over its last step (FT-437; a U-turn would cost one bend only).
fn reverses(h: Option<Heading>, m: Heading) -> bool {
    h == Some(m.opposite())
}

fn add(a: (u32, u32), b: (u32, u32)) -> (u32, u32) {
    (a.0.saturating_add(b.0), a.1.saturating_add(b.1))
}

/// The state index of (node, arriving heading); `None` is the fifth heading slot.
fn state(node: usize, h: Option<Heading>) -> usize {
    node * 5 + h.map_or(4, Heading::index)
}

/// The route from `req.from.at` to `req.to.at` (see the module documentation): deterministic, interior vertices on
/// the grid (or on an end's coordinate next to it), the ends exact.
pub fn auto_route(req: &RouteRequest) -> Vec<(i64, i64)> {
    route(req).points
}

/// [`auto_route`] with whether the result is the fallback ([`Routed::direct`]).
pub fn route(req: &RouteRequest) -> Routed {
    match search(req) {
        Some(points) => Routed {
            points,
            direct: false,
        },
        None => Routed {
            points: fallback(req),
            direct: true,
        },
    }
}

/// Whether the given route `points` from `req.from.at` to `req.to.at` is one the router could take: walked on the
/// grid nodes the router searches, every unit move is free and lands on a node that is not blocked, by the router's
/// own rules for `req` (obstacles, the ends' own rectangles but for their corridors, other wires, reserved points;
/// module documentation); a CP end is left along its direction and entered against the second end's; the first move
/// does not reverse [`RouteRequest::arrive`] and no move reverses the one before it. `req.area` only has to hold the
/// route. A route that is not orthogonal on the grid, that does not start and end at the ends, or a request the
/// router would refuse (an unusable grid, an area over [`MAX_NODES`]) is not clear.
///
/// The Wire tool asks it of the direct route through a free terminal in line (FT-705 with FT-706), with the
/// terminal's own body, texts, CPs and stubs left out of `req`.
pub fn clear(req: &RouteRequest, points: &[(i64, i64)]) -> bool {
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else {
        return false;
    };
    if first != req.from.at || last != req.to.at {
        return false;
    }
    let Some(Prepared { space, s, .. }) = prepare(req) else {
        return false;
    };
    // FT-767: a first end without a direction (a fixed bend) starts on its own grid node, which the walk below never
    // tests: it must be one the search may stand on. (`prepare` frees the ends' nodes, so the router starts from a
    // blocked point too; the check keeps `clear` tied to the search space if that rule ever changes.)
    if req.from.dir.is_none() && space.node(s).is_none_or(|n| space.blocked[n]) {
        return false;
    }
    let idx = |p: (i64, i64)| (space.grid.snap(Nm(p.0)), space.grid.snap(Nm(p.1)));
    let mut moves: Vec<Heading> = Vec::new();
    for w in points.windows(2) {
        let (a, b) = (idx(w[0]), idx(w[1]));
        let (h, n) = match (b.0 - a.0, b.1 - a.1) {
            (0, 0) => continue,
            (dx, 0) if dx > 0 => (Heading::Right, dx),
            (dx, 0) => (Heading::Left, -dx),
            (0, dy) if dy > 0 => (Heading::Down, dy),
            (0, dy) => (Heading::Up, -dy),
            _ => return false,
        };
        if usize::try_from(n).map_or(true, |n| moves.len().saturating_add(n) > space.blocked.len()) {
            return false;
        }
        moves.extend(std::iter::repeat_n(h, usize::try_from(n).unwrap_or(0)));
    }
    let (Some(&m0), Some(&mn)) = (moves.first(), moves.last()) else {
        return true;
    };
    if req.from.dir.is_some_and(|d| d != m0)
        || req.to.dir.is_some_and(|d| d.opposite() != mn)
        || (req.from.dir.is_none() && req.arrive.is_some_and(|h| reverses(Some(h), m0)))
    {
        return false;
    }
    let mut p = s;
    let mut heading: Option<Heading> = None;
    for m in moves {
        if reverses(heading, m) {
            return false;
        }
        let q = (p.0 + m.step().0, p.1 + m.step().1);
        let free = space
            .node(q)
            .is_some_and(|n| !space.blocked[n] && space.move_free(p, m));
        if !free {
            return false;
        }
        (p, heading) = (q, Some(m));
    }
    true
}

/// The L route of the fallback; with an arrival heading at a first end without a direction, the corner whose first
/// leg does not reverse it (FT-437; aligned ends behind the arrival stay the straight segment, the only L there is).
pub fn fallback(req: &RouteRequest) -> Vec<(i64, i64)> {
    let (a, b) = (req.from, req.to);
    let (pa, pb) = (a.at, b.at);
    if let (None, Some(h)) = (a.dir, req.arrive)
        && pa.0 != pb.0
        && pa.1 != pb.1
    {
        // The default corner of an end without a direction, unless its first leg runs against `h`.
        let default = l_route(a, b);
        let first_leg = if default[1].1 == pa.1 {
            if pb.0 > pa.0 {
                Heading::Right
            } else {
                Heading::Left
            }
        } else if pb.1 > pa.1 {
            Heading::Down
        } else {
            Heading::Up
        };
        if first_leg == h.opposite() {
            let other = if default[1].1 == pa.1 {
                (pa.0, pb.1)
            } else {
                (pb.0, pa.1)
            };
            return vec![pa, other, pb];
        }
        return default;
    }
    l_route(a, b)
}

/// The search space of a request with every rule of the module documentation applied: the blocked nodes and the
/// moves not taken, with the ends' grid nodes and the first end's stub.
struct Prepared {
    space: Space,
    /// The first end's grid node.
    s: (i64, i64),
    /// The second end's grid node.
    t: (i64, i64),
    /// The first end's stub node (`None` without a direction).
    s1: Option<(i64, i64)>,
}

/// The search space of `req` (`None` with an unusable grid, an index at the edge of `i64` or an area over
/// [`MAX_NODES`]).
fn prepare(req: &RouteRequest) -> Option<Prepared> {
    let (from, to) = (req.from, req.to);
    let num = u64::try_from(req.grid.0).ok()?;
    let den = u64::try_from(req.grid.1).ok()?;
    let grid = Grid::new(num, den).ok()?;
    let idx = |x: i64| grid.snap(Nm(x));
    let s = (idx(from.at.0), idx(from.at.1));
    let t = (idx(to.at.0), idx(to.at.1));
    // The stub node one step along a CP end's direction (`None` without a direction); an index at the edge of `i64`
    // has none, and the router falls back.
    let stub = |p: (i64, i64), d: Option<Heading>| -> Option<Option<(i64, i64)>> {
        match d {
            None => Some(None),
            Some(d) => Some(Some((p.0.checked_add(d.step().0)?, p.1.checked_add(d.step().1)?))),
        }
    };
    let (s1, t1) = (stub(s, from.dir)?, stub(t, to.dir)?);
    // The bounds: the area's nearest indices, extended to the ends and their stubs.
    let (ax0, ax1) = (req.area[0].min(req.area[2]), req.area[0].max(req.area[2]));
    let (ay0, ay1) = (req.area[1].min(req.area[3]), req.area[1].max(req.area[3]));
    let mut lo = (idx(ax0), idx(ay0));
    let mut hi = (idx(ax1), idx(ay1));
    for p in [Some(s), Some(t), s1, t1].into_iter().flatten() {
        lo = (lo.0.min(p.0), lo.1.min(p.1));
        hi = (hi.0.max(p.0), hi.1.max(p.1));
    }
    let width = usize::try_from(hi.0.checked_sub(lo.0)?.checked_add(1)?).ok()?;
    let height = usize::try_from(hi.1.checked_sub(lo.1)?.checked_add(1)?).ok()?;
    let nodes = width.checked_mul(height)?;
    if nodes > MAX_NODES {
        return None;
    }
    let mut space = Space {
        grid,
        lo,
        hi,
        width,
        blocked: vec![false; nodes],
        no_right: vec![0; nodes],
        no_down: vec![0; nodes],
    };
    // A node is blocked when its page point lies strictly inside an obstacle; a move is not taken when it runs
    // through the open inside of one (FT-706: a text thinner than a grid step lies between two nodes).
    let mark = |space: &mut Space, o: &[i64; 4], into: &mut Vec<bool>, flag: u8| {
        let (x0, x1) = (o[0].min(o[2]), o[0].max(o[2]));
        let (y0, y1) = (o[1].min(o[3]), o[1].max(o[3]));
        let (i0, i1) = ((idx(x0) - 1).max(lo.0), (idx(x1) + 1).min(hi.0));
        let (j0, j1) = ((idx(y0) - 1).max(lo.1), (idx(y1) + 1).min(hi.1));
        let inside = |c: Option<i64>, a: i64, b: i64| c.is_some_and(|c| a < c && c < b);
        let across = |c0: Option<i64>, c1: Option<i64>, a: i64, b: i64| match (c0, c1) {
            (Some(c0), Some(c1)) => c0.max(a) < c1.min(b),
            _ => false,
        };
        for j in j0..=j1 {
            let (cy, cy1) = (space.coord(j), space.coord(j + 1));
            for i in i0..=i1 {
                let (cx, cx1) = (space.coord(i), space.coord(i + 1));
                let Some(n) = space.node((i, j)) else { continue };
                if inside(cx, x0, x1) && inside(cy, y0, y1) {
                    into[n] = true;
                }
                if i < hi.0 && inside(cy, y0, y1) && across(cx, cx1, x0, x1) {
                    space.no_right[n] |= flag;
                }
                if j < hi.1 && inside(cx, x0, x1) && across(cy, cy1, y0, y1) {
                    space.no_down[n] |= flag;
                }
            }
        }
    };
    let mut blocked = vec![false; nodes];
    for o in &req.obstacles {
        mark(&mut space, o, &mut blocked, OVER_OBSTACLE);
    }
    // The ends' own representations block too, but for their escape corridors (FT-706): from a CP end along its
    // direction, the stub and every further node while it, or the move on out of it, is inside one of the own
    // rectangles.
    let mut own = vec![false; nodes];
    for o in req.own.iter().flatten() {
        mark(&mut space, o, &mut own, OVER_OWN);
    }
    let corridor = |p: (i64, i64), d: Option<Heading>| -> Vec<(i64, i64)> {
        let Some(d) = d else { return vec![p] };
        let ahead = |q: (i64, i64)| (q.0 + d.step().0, q.1 + d.step().1);
        let over_own = |a: (i64, i64), b: (i64, i64)| space.move_bits(a, b) & OVER_OWN != 0;
        let mut out = vec![p];
        let mut q = p;
        loop {
            let next = ahead(q);
            let Some(n) = space.node(next) else { break };
            out.push(next);
            if !own[n] && !over_own(next, ahead(next)) {
                break;
            }
            q = next;
        }
        out
    };
    let corridors = [corridor(s, from.dir), corridor(t, to.dir)];
    for p in corridors.iter().flatten() {
        if let Some(n) = space.node(*p) {
            own[n] = false;
        }
    }
    // Other wires (FT-706): the node of each of their vertices is blocked, and a move parallel to one of their
    // segments closer than one grid step, overlapping it, is not taken.
    let near = i64::try_from(num / den).unwrap_or(i64::MAX).saturating_sub(1);
    for w in &req.wires {
        let (a, b) = (w[0], w[1]);
        for v in [a, b] {
            if let Some(n) = space.node((idx(v.0), idx(v.1))) {
                blocked[n] = true;
            }
        }
        // `(track coordinate, extent lo, extent hi, horizontal)`.
        let (c, e0, e1, horizontal) = if a.1 == b.1 && a.0 != b.0 {
            (a.1, a.0.min(b.0), a.0.max(b.0), true)
        } else if a.0 == b.0 && a.1 != b.1 {
            (a.0, a.1.min(b.1), a.1.max(b.1), false)
        } else {
            continue;
        };
        let (tlo, thi) = if horizontal { (lo.1, hi.1) } else { (lo.0, hi.0) };
        let (alo, ahi) = if horizontal { (lo.0, hi.0) } else { (lo.1, hi.1) };
        for k in (idx(c) - 1).max(tlo)..=(idx(c) + 1).min(thi) {
            let Some(ck) = space.coord(k) else { continue };
            if (ck - c).abs() >= near {
                continue;
            }
            for i in (idx(e0) - 1).max(alo)..(idx(e1) + 1).min(ahi) {
                let (Some(c0), Some(c1)) = (space.coord(i), space.coord(i + 1)) else {
                    continue;
                };
                if c1.min(e1) - c0.max(e0) <= 0 {
                    continue;
                }
                let (p, q) = if horizontal {
                    ((i, k), (i + 1, k))
                } else {
                    ((k, i), (k, i + 1))
                };
                if let Some(f) = space.move_flags(p, q) {
                    *f |= ON_WIRE;
                }
            }
        }
    }
    // The other connection points and their stubs are reserved for their own wires (FT-706).
    for p in &req.reserved {
        if let Some(n) = space.node((idx(p.0), idx(p.1))) {
            blocked[n] = true;
        }
    }
    // The corridors' moves are exempt from the own rectangles and from other wires (a second wire on a CP leaves
    // along the same stub); the stub move from everything, as the stub node is.
    for c in &corridors {
        for (k, pair) in c.windows(2).enumerate() {
            if let Some(f) = space.move_flags(pair[0], pair[1]) {
                *f &= if k == 0 { 0 } else { !(ON_WIRE | OVER_OWN) };
            }
        }
    }
    for (n, b) in blocked.iter_mut().enumerate() {
        *b |= own[n];
    }
    for p in [Some(s), Some(t), s1, t1].into_iter().flatten() {
        if let Some(n) = space.node(p) {
            blocked[n] = false;
        }
    }
    space.blocked = blocked;
    Some(Prepared { space, s, t, s1 })
}

fn search(req: &RouteRequest) -> Option<Vec<(i64, i64)>> {
    let (from, to) = (req.from, req.to);
    if from.at == to.at {
        return Some(vec![from.at]);
    }
    let Prepared { space, s, t, s1 } = prepare(req)?;
    let (lo, width, nodes) = (space.lo, space.width, space.blocked.len());
    let goal = space.node(t)?;
    // The state the forward walk starts from: the first end's stub with its heading, else the first end with its
    // arrival heading (FT-437), else the first end.
    let arrive = if from.dir.is_none() { req.arrive } else { None };
    let start_state = match (from.dir, s1) {
        (Some(d), Some(p)) => state(space.node(p)?, Some(d)),
        _ => state(space.node(s)?, arrive),
    };
    // The cost of a move with heading `m` from state `st` that arrived with `h`; `None` for a U-turn. A turn at the
    // fixed bend the route starts from is free (the user fixed a bend there), so the arrival only excludes the move
    // back over the last segment.
    let free_turn = arrive.map(|_| start_state);
    let step_cost = |st: usize, h: Option<Heading>, m: Heading| -> Option<(u32, u32)> {
        if reverses(h, m) {
            None
        } else if free_turn == Some(st) {
            Some(weight(None, m))
        } else {
            Some(weight(h, m))
        }
    };
    // The backward Dijkstra: the least (cost, bends) from every state to the second end. It stops once the popped
    // distance exceeds the start state's: every state the forward walk can step to on a least path is settled by
    // then, and an unsettled state's tentative distance is too large to match, so the result is the same.
    const INF: (u32, u32) = (u32::MAX, u32::MAX);
    let mut dist: Vec<(u32, u32)> = vec![INF; nodes * 5];
    let mut heap: BinaryHeap<Reverse<((u32, u32), usize)>> = BinaryHeap::new();
    let goal_headings: Vec<Option<Heading>> = match to.dir {
        Some(d) => vec![Some(d.opposite())],
        None => vec![
            None,
            Some(Heading::Right),
            Some(Heading::Down),
            Some(Heading::Left),
            Some(Heading::Up),
        ],
    };
    for h in goal_headings {
        let st = state(goal, h);
        dist[st] = (0, 0);
        heap.push(Reverse(((0, 0), st)));
    }
    let position = |n: usize| -> (i64, i64) {
        let x = i64::try_from(n % width).unwrap_or(0);
        let y = i64::try_from(n / width).unwrap_or(0);
        (lo.0 + x, lo.1 + y)
    };
    while let Some(Reverse((d, st))) = heap.pop() {
        if d > dist[start_state] {
            break;
        }
        if d > dist[st] {
            continue;
        }
        let (n2, slot) = (st / 5, st % 5);
        if slot == 4 {
            // A state without an arriving heading is only ever a start; nothing arrives at it.
            continue;
        }
        let m = Heading::ALL[slot];
        let p2 = position(n2);
        let p1 = (p2.0 - m.step().0, p2.1 - m.step().1);
        let Some(n1) = space.node(p1) else { continue };
        if space.blocked[n1] || !space.move_free(p1, m) {
            continue;
        }
        for h in [
            None,
            Some(Heading::Right),
            Some(Heading::Down),
            Some(Heading::Left),
            Some(Heading::Up),
        ] {
            let s1 = state(n1, h);
            let Some(w) = step_cost(s1, h, m) else {
                continue;
            };
            let cand = add(w, d);
            if cand < dist[s1] {
                dist[s1] = cand;
                heap.push(Reverse((cand, s1)));
            }
        }
    }
    // The forward walk: the lowest-ranked move that stays on a least path.
    let start = space.node(s)?;
    let mut moves: Vec<Heading> = Vec::new();
    let (mut node, mut heading) = match (from.dir, s1) {
        (Some(d), Some(p)) => {
            let n = space.node(p)?;
            moves.push(d);
            (n, Some(d))
        }
        _ => (start, arrive),
    };
    if dist[state(node, heading)] == INF {
        return None;
    }
    while dist[state(node, heading)] != (0, 0) {
        let here = dist[state(node, heading)];
        let p = position(node);
        let here_state = state(node, heading);
        let next = Heading::ALL.into_iter().find_map(|m| {
            let w = step_cost(here_state, heading, m)?;
            let q = (p.0 + m.step().0, p.1 + m.step().1);
            let n = space.node(q)?;
            if space.blocked[n] || !space.move_free(p, m) {
                return None;
            }
            let there = dist[state(n, Some(m))];
            (there != INF && add(w, there) == here).then_some((n, m))
        });
        let (n, m) = next?;
        moves.push(m);
        node = n;
        heading = Some(m);
        if moves.len() > nodes * 5 {
            return None;
        }
    }
    build(&space, from, to, s, &moves)
}

/// The route of a move list from the grid node `s`: the exact ends, the corners on the grid, the corners next to an
/// end on that end's coordinate, cleaned.
fn build(
    space: &Space,
    from: RouteEnd,
    to: RouteEnd,
    s: (i64, i64),
    moves: &[Heading],
) -> Option<Vec<(i64, i64)>> {
    let (Some(&first), Some(&last)) = (moves.first(), moves.last()) else {
        return Some(vec![from.at, to.at]);
    };
    let mut corners: Vec<(i64, i64)> = Vec::new();
    let mut p = s;
    for (k, m) in moves.iter().enumerate() {
        p = (p.0 + m.step().0, p.1 + m.step().1);
        if moves.get(k + 1).is_some_and(|next| next != m) {
            corners.push((space.coord(p.0)?, space.coord(p.1)?));
        }
    }
    let mut pts: Vec<(i64, i64)> = Vec::with_capacity(corners.len() + 2);
    pts.push(from.at);
    pts.extend(corners);
    pts.push(to.at);
    let n = pts.len();
    if n > 2 {
        // The corner after the first end lies on the first end's coordinate across the first segment; the corner
        // before the last end on the last end's coordinate across the last segment.
        if first.vertical() {
            pts[1].0 = from.at.0;
        } else {
            pts[1].1 = from.at.1;
        }
        if last.vertical() {
            pts[n - 2].0 = to.at.0;
        } else {
            pts[n - 2].1 = to.at.1;
        }
    }
    // A straight route between ends that part by a little across it (two CPs off the grid by different amounts):
    // a jog at the grid coordinate nearest the middle keeps it orthogonal.
    if pts.len() == 2 && from.at.0 != to.at.0 && from.at.1 != to.at.1 {
        let g = space.grid;
        if first.vertical() {
            let ym = g.snap_point(Nm(from.at.1 + (to.at.1 - from.at.1) / 2))?.0;
            pts = vec![from.at, (from.at.0, ym), (to.at.0, ym), to.at];
        } else {
            let xm = g.snap_point(Nm(from.at.0 + (to.at.0 - from.at.0) / 2))?.0;
            pts = vec![from.at, (xm, from.at.1), (xm, to.at.1), to.at];
        }
    }
    Some(clean(pts))
}

/// Removes consecutive duplicate vertices and interior vertices collinear with their neighbours on an axis; the
/// first and last vertex stay.
fn clean(pts: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last() == Some(&p) {
            continue;
        }
        out.push(p);
        while out.len() >= 3 {
            let k = out.len();
            let (a, b, c) = (out[k - 3], out[k - 2], out[k - 1]);
            let collinear = (a.0 == b.0 && b.0 == c.0) || (a.1 == b.1 && b.1 == c.1);
            if !collinear {
                break;
            }
            out.remove(k - 2);
            if out.len() >= 2 && out[out.len() - 1] == out[out.len() - 2] {
                out.pop();
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1 mm grid, so grid index k is k mm.
    const MM: i64 = 1_000_000;

    fn p(x: i64, y: i64) -> (i64, i64) {
        (x * MM, y * MM)
    }

    fn end(x: i64, y: i64, dir: Option<Heading>) -> RouteEnd {
        RouteEnd { at: p(x, y), dir }
    }

    fn req(from: RouteEnd, to: RouteEnd, obstacles: Vec<[i64; 4]>) -> RouteRequest {
        let (x0, x1) = (from.at.0.min(to.at.0), from.at.0.max(to.at.0));
        let (y0, y1) = (from.at.1.min(to.at.1), from.at.1.max(to.at.1));
        RouteRequest {
            from,
            to,
            obstacles,
            grid: (MM, 1),
            area: [x0 - 4 * MM, y0 - 4 * MM, x1 + 4 * MM, y1 + 4 * MM],
            arrive: None,
            own: [Vec::new(), Vec::new()],
            wires: Vec::new(),
            reserved: Vec::new(),
        }
    }

    fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> [i64; 4] {
        [x0 * MM, y0 * MM, x1 * MM, y1 * MM]
    }

    fn orthogonal(r: &[(i64, i64)]) -> bool {
        r.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1)
    }

    #[test]
    fn a_straight_wire_between_facing_cps() {
        let r = auto_route(&req(
            end(0, 0, Some(Heading::Right)),
            end(10, 0, Some(Heading::Left)),
            vec![],
        ));
        assert_eq!(r, vec![p(0, 0), p(10, 0)]);
    }

    #[test]
    fn an_l_between_perpendicular_cps() {
        let r = auto_route(&req(
            end(0, 0, Some(Heading::Right)),
            end(10, 10, Some(Heading::Up)),
            vec![],
        ));
        assert_eq!(r, vec![p(0, 0), p(10, 0), p(10, 10)]);
    }

    #[test]
    fn a_z_between_parallel_cps_takes_the_lowest_ranked_move_sequence() {
        // The first end leaves right, the second is entered moving right, 4 mm lower: a Z with two bends. Among the
        // Z shapes of least cost the move sequence R … R D D D D R is the least (right ranks before down).
        let r = auto_route(&req(
            end(0, 0, Some(Heading::Right)),
            end(10, 4, Some(Heading::Left)),
            vec![],
        ));
        assert!(orthogonal(&r), "{r:?}");
        assert_eq!(r.len(), 4, "a Z: {r:?}");
        assert_eq!(r[0], p(0, 0));
        assert_eq!(r[3], p(10, 4));
        // The least sequence goes right as far as it can before going down: the bend at x = 9 (the last node before
        // the entry stub at x = 9 → 10).
        assert_eq!(r[1], p(9, 0));
        assert_eq!(r[2], p(9, 4));
    }

    #[test]
    fn an_obstacle_is_detoured_around() {
        // A body between the two ends, inflated by one step already: from (2, -3) to (8, 3).
        let r = auto_route(&req(
            end(0, 0, Some(Heading::Right)),
            end(10, 0, Some(Heading::Left)),
            vec![rect(2, -3, 8, 3)],
        ));
        assert!(orthogonal(&r), "{r:?}");
        assert_eq!(r.first(), Some(&p(0, 0)));
        assert_eq!(r.last(), Some(&p(10, 0)));
        // No vertex and no segment passes strictly inside the obstacle.
        for w in r.windows(2) {
            let (a, b) = (w[0], w[1]);
            let inside = |x: i64, y: i64| 2 * MM < x && x < 8 * MM && -3 * MM < y && y < 3 * MM;
            let steps = ((b.0 - a.0).abs() + (b.1 - a.1).abs()) / MM;
            for k in 0..=steps {
                let x = a.0 + (b.0 - a.0).signum() * k * MM;
                let y = a.1 + (b.1 - a.1).signum() * k * MM;
                assert!(!inside(x, y), "{r:?} passes ({x}, {y})");
            }
        }
        // The detour goes down (ranked before up) along the obstacle's lower edge.
        assert!(r.iter().any(|v| v.1 == 3 * MM), "{r:?}");
    }

    #[test]
    fn without_a_path_the_l_route_is_the_fallback() {
        // The second end is walled in on every side inside the area.
        let to = end(10, 10, None);
        let walls = vec![
            rect(7, 7, 13, 9),
            rect(7, 11, 13, 13),
            rect(7, 7, 9, 13),
            rect(11, 7, 13, 13),
        ];
        let r = auto_route(&req(end(0, 0, Some(Heading::Right)), to, walls));
        assert_eq!(r, l_route(end(0, 0, Some(Heading::Right)), to));
        assert_eq!(r, vec![p(0, 0), p(10, 0), p(10, 10)]);
        // An unusable grid falls back too.
        let mut bad = req(end(0, 0, None), end(3, 4, None), vec![]);
        bad.grid = (0, 1);
        assert_eq!(auto_route(&bad), vec![p(0, 0), p(3, 0), p(3, 4)]);
    }

    #[test]
    fn the_tie_rule_prefers_right_before_down() {
        // Junction ends (no direction): R R D D and D D R R cost the same; right ranks first.
        let r = auto_route(&req(end(0, 0, None), end(2, 2, None), vec![]));
        assert_eq!(r, vec![p(0, 0), p(2, 0), p(2, 2)]);
        // Reversed: L L U U against U U L L; left ranks before up.
        let r = auto_route(&req(end(2, 2, None), end(0, 0, None), vec![]));
        assert_eq!(r, vec![p(2, 2), p(0, 2), p(0, 0)]);
    }

    #[test]
    fn fewer_bends_win_over_length_at_equal_cost() {
        // From (0, 0) leaving up to (4, 0) entered from above: up 1, right 4, down 1 has two bends (cost 6 + 8);
        // nothing shorter exists, the result is the U.
        let r = auto_route(&req(
            end(0, 0, Some(Heading::Up)),
            end(4, 0, Some(Heading::Up)),
            vec![],
        ));
        assert_eq!(r, vec![p(0, 0), p(0, -1), p(4, -1), p(4, 0)]);
    }

    #[test]
    fn the_result_does_not_depend_on_the_obstacle_order() {
        let obstacles = vec![
            rect(2, -3, 4, 3),
            rect(6, -1, 8, 6),
            rect(-2, 5, 12, 7),
            rect(3, -8, 5, -5),
        ];
        let base = req(
            end(0, 0, Some(Heading::Right)),
            end(10, 2, Some(Heading::Left)),
            obstacles.clone(),
        );
        let r0 = auto_route(&base);
        assert!(orthogonal(&r0));
        let n = obstacles.len();
        for k in 0..n {
            let mut o = obstacles.clone();
            o.rotate_left(k);
            assert_eq!(
                auto_route(&RouteRequest {
                    obstacles: o.clone(),
                    ..base.clone()
                }),
                r0
            );
            o.reverse();
            assert_eq!(
                auto_route(&RouteRequest {
                    obstacles: o,
                    ..base.clone()
                }),
                r0
            );
        }
    }

    #[test]
    fn ends_off_a_rational_grid_stay_exact_and_corners_take_their_coordinates() {
        // The QET grid: a CP 1 nm off its grid point; the corner next to it takes its x.
        let g = Grid::QET_10PX;
        let at = |k: i64| g.at(k).unwrap().0;
        let a = RouteEnd {
            at: (at(10) + 1, at(10)),
            dir: Some(Heading::Down),
        };
        let b = RouteEnd {
            at: (at(20), at(16) - 1),
            dir: Some(Heading::Left),
        };
        let request = RouteRequest {
            from: a,
            to: b,
            obstacles: vec![],
            grid: (7_937_500, 3),
            area: [at(5), at(5), at(25), at(21)],
            arrive: None,
            own: [Vec::new(), Vec::new()],
            wires: Vec::new(),
            reserved: Vec::new(),
        };
        let r = auto_route(&request);
        assert_eq!(r.first(), Some(&a.at));
        assert_eq!(r.last(), Some(&b.at));
        assert!(orthogonal(&r), "{r:?}");
        assert_eq!(r, vec![a.at, (a.at.0, b.at.1), b.at]);
        // Interior coordinates are grid positions or an end's.
        for v in &r[1..r.len() - 1] {
            for (c, ends) in [(v.0, [a.at.0, b.at.0]), (v.1, [a.at.1, b.at.1])] {
                assert!(g.snap_point(Nm(c)) == Some(Nm(c)) || ends.contains(&c), "{v:?}");
            }
        }
    }

    #[test]
    fn an_area_over_the_node_cap_falls_back_to_the_l_route() {
        // 1 mm grid, an area of 600 × 600 = 360 000 nodes (> 262 144).
        let mut big = req(
            end(0, 0, Some(Heading::Right)),
            end(10, 10, Some(Heading::Up)),
            vec![],
        );
        big.area = [-300 * MM, -300 * MM, 300 * MM, 300 * MM];
        assert_eq!(auto_route(&big), l_route(big.from, big.to));
        // Ends at the edge of `i64` give no stub: the fallback too.
        let edge = RouteRequest {
            from: RouteEnd {
                at: (i64::MAX, 0),
                dir: Some(Heading::Right),
            },
            to: RouteEnd {
                at: (0, 0),
                dir: None,
            },
            obstacles: vec![],
            grid: (1, 1),
            area: [0, 0, 1, 1],
            arrive: None,
            own: [Vec::new(), Vec::new()],
            wires: Vec::new(),
            reserved: Vec::new(),
        };
        assert_eq!(auto_route(&edge), l_route(edge.from, edge.to));
    }

    #[test]
    fn a_stub_into_an_obstacle_is_allowed() {
        // The first end's stub node lies inside another body; the route still leaves along the stub.
        let r = auto_route(&req(
            end(0, 0, Some(Heading::Down)),
            end(6, 4, Some(Heading::Left)),
            vec![rect(-2, 0, 2, 2)],
        ));
        assert_eq!(r.first(), Some(&p(0, 0)));
        assert_eq!(r[1].0, 0, "leaves down along x = 0: {r:?}");
        assert!(r[1].1 > 0);
    }

    fn with_arrive(mut r: RouteRequest, h: Heading) -> RouteRequest {
        r.arrive = Some(h);
        r
    }

    #[test]
    fn a_fixed_bend_s_arrival_is_never_reversed() {
        // FT-437: a bend fixed at (5, 0) after a segment running right, the end at (0, 0) straight behind it. Without
        // the arrival heading the route runs straight back over the last segment.
        let base = req(end(5, 0, None), end(0, 0, None), vec![]);
        assert_eq!(auto_route(&base), vec![p(5, 0), p(0, 0)]);
        // With it the first move turns (down ranks before up), runs left and comes up into the end.
        let r = auto_route(&with_arrive(base, Heading::Right));
        assert_eq!(r, vec![p(5, 0), p(5, 1), p(0, 1), p(0, 0)]);
    }

    #[test]
    fn a_cp_stub_is_never_run_back_over() {
        // FT-437: a CP leaving right with the end straight behind it. A U-turn after the stub costs one bend only,
        // so the search would run back over the stub and through the CP; it goes round instead.
        let r = auto_route(&req(end(0, 0, Some(Heading::Right)), end(-10, 0, None), vec![]));
        assert_eq!(r, vec![p(0, 0), p(1, 0), p(1, 1), p(-10, 1), p(-10, 0)]);
    }

    #[test]
    fn a_turn_at_a_fixed_bend_is_free() {
        // Arriving down at (0, 0): R R D D still ties with D D R R and wins by rank, as without an arrival; the
        // bend was fixed to turn there.
        let base = req(end(0, 0, None), end(2, 2, None), vec![]);
        assert_eq!(auto_route(&base), vec![p(0, 0), p(2, 0), p(2, 2)]);
        assert_eq!(
            auto_route(&with_arrive(base, Heading::Down)),
            vec![p(0, 0), p(2, 0), p(2, 2)]
        );
        // Arriving left, the first move right is excluded (it would run back over the segment before the bend):
        // D D R R.
        let base = req(end(0, 0, None), end(2, 2, None), vec![]);
        assert_eq!(
            auto_route(&with_arrive(base, Heading::Left)),
            vec![p(0, 0), p(0, 2), p(2, 2)]
        );
    }

    #[test]
    fn the_arrival_is_ignored_at_a_cp_end() {
        let base = req(
            end(0, 0, Some(Heading::Right)),
            end(10, 10, Some(Heading::Up)),
            vec![],
        );
        assert_eq!(
            auto_route(&with_arrive(base.clone(), Heading::Left)),
            auto_route(&base)
        );
    }

    #[test]
    fn the_fallback_does_not_reverse_the_arrival() {
        // The end walled in: the L route; its default corner (10, 0) would leave right against an arrival heading
        // left, so the other corner is taken.
        let to = end(10, 10, None);
        let walls = vec![
            rect(7, 7, 13, 9),
            rect(7, 11, 13, 13),
            rect(7, 7, 9, 13),
            rect(11, 7, 13, 13),
        ];
        let base = req(end(0, 0, None), to, walls);
        assert_eq!(auto_route(&base), vec![p(0, 0), p(10, 0), p(10, 10)]);
        assert_eq!(
            auto_route(&with_arrive(base.clone(), Heading::Left)),
            vec![p(0, 0), p(0, 10), p(10, 10)]
        );
        // An arrival the default corner does not reverse keeps it.
        assert_eq!(
            auto_route(&with_arrive(base, Heading::Right)),
            vec![p(0, 0), p(10, 0), p(10, 10)]
        );
    }

    /// The unit moves of a route on the 1 mm grid of the tests, as (from, to) index points.
    fn unit_moves(r: &[(i64, i64)]) -> Vec<((i64, i64), (i64, i64))> {
        let mut out = Vec::new();
        for w in r.windows(2) {
            let (a, b) = ((w[0].0 / MM, w[0].1 / MM), (w[1].0 / MM, w[1].1 / MM));
            let step = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
            let mut p = a;
            while p != b {
                let q = (p.0 + step.0, p.1 + step.1);
                out.push((p, q));
                p = q;
            }
        }
        out
    }

    /// Whether the unit move `a → b` runs on the track of the segment `p → q` with a positive overlap (index points).
    fn on_track(a: (i64, i64), b: (i64, i64), p: (i64, i64), q: (i64, i64)) -> bool {
        if a.1 == b.1 && p.1 == q.1 && a.1 == p.1 {
            a.0.min(b.0).max(p.0.min(q.0)) < a.0.max(b.0).min(p.0.max(q.0))
        } else if a.0 == b.0 && p.0 == q.0 && a.0 == p.0 {
            a.1.min(b.1).max(p.1.min(q.1)) < a.1.max(b.1).min(p.1.max(q.1))
        } else {
            false
        }
    }

    /// The FT-706 violations of a route among the request's wires: a unit move on another wire's track that overlaps
    /// it, or an interior node of the route on another wire's vertex. A CP end's stub move and stub node are exempt.
    fn violations(rq: &RouteRequest, r: &[(i64, i64)]) -> Vec<String> {
        let moves = unit_moves(r);
        let n = moves.len();
        let idx = |v: (i64, i64)| (v.0 / MM, v.1 / MM);
        let mut out = Vec::new();
        for (k, (a, b)) in moves.iter().enumerate() {
            if (k == 0 && rq.from.dir.is_some()) || (k + 1 == n && rq.to.dir.is_some()) {
                continue;
            }
            for w in &rq.wires {
                if on_track(*a, *b, idx(w[0]), idx(w[1])) {
                    out.push(format!("move {a:?}->{b:?} runs on {w:?}"));
                }
            }
        }
        // The interior nodes: every move's target but the last, without the stubs.
        for (k, (_, b)) in moves.iter().enumerate() {
            let stub = (k == 0 && rq.from.dir.is_some()) || (k + 2 == n && rq.to.dir.is_some());
            if k + 1 == n || stub {
                continue;
            }
            for w in &rq.wires {
                if w.iter().any(|v| idx(*v) == *b) {
                    out.push(format!("node {b:?} touches a vertex of {w:?}"));
                }
            }
        }
        out
    }

    fn with_wires(mut r: RouteRequest, polylines: &[&[(i64, i64)]]) -> RouteRequest {
        for line in polylines {
            for w in line.windows(2) {
                r.wires.push([p(w[0].0, w[0].1), p(w[1].0, w[1].1)]);
            }
        }
        r
    }

    #[test]
    fn a_route_never_runs_on_another_wire_s_track() {
        // FT-706 friction 8: the L wire leaves its CP down and runs right at y = 2; the N wire from the next pole
        // would run on that track. It keeps one grid step off it (y = 1).
        let base = req(
            end(3, 0, Some(Heading::Down)),
            end(25, 2, Some(Heading::Left)),
            vec![],
        );
        assert_eq!(
            auto_route(&base),
            vec![p(3, 0), p(3, 2), p(25, 2)],
            "without the wire it takes the track"
        );
        let rq = with_wires(base, &[&[(0, 0), (0, 2), (20, 2)]]);
        let r = auto_route(&rq);
        assert_eq!(r, vec![p(3, 0), p(3, 1), p(24, 1), p(24, 2), p(25, 2)]);
        assert_eq!(violations(&rq, &r), Vec::<String>::new());
    }

    #[test]
    fn a_route_crosses_another_wire_at_a_right_angle() {
        let rq = with_wires(
            req(
                end(0, 0, Some(Heading::Right)),
                end(10, 0, Some(Heading::Left)),
                vec![],
            ),
            &[&[(5, -5), (5, 5)]],
        );
        assert_eq!(auto_route(&rq), vec![p(0, 0), p(10, 0)]);
        assert!(!route(&rq).direct);
    }

    #[test]
    fn a_route_never_touches_another_wire_s_end_or_corner() {
        // A wire ending at (5, 0) on the straight line: passing there would read as a junction.
        let rq = with_wires(
            req(
                end(0, 0, Some(Heading::Right)),
                end(10, 0, Some(Heading::Left)),
                vec![],
            ),
            &[&[(5, -5), (5, 0)]],
        );
        let r = auto_route(&rq);
        assert!(orthogonal(&r), "{r:?}");
        assert_eq!((r.first(), r.last()), (Some(&p(0, 0)), Some(&p(10, 0))));
        assert_eq!(violations(&rq, &r), Vec::<String>::new());
        assert!(!unit_moves(&r).iter().any(|(_, b)| *b == (5, 0)), "{r:?}");
    }

    #[test]
    fn a_second_wire_on_a_cp_shares_only_the_stub() {
        // A wire already leaves the CP down and runs on to (0, 3): the second one leaves along the same stub, then
        // turns off it.
        let rq = with_wires(
            req(end(0, 0, Some(Heading::Down)), end(-10, 6, None), vec![]),
            &[&[(0, 0), (0, 3), (10, 3)]],
        );
        let r = auto_route(&rq);
        assert_eq!(r, vec![p(0, 0), p(0, 1), p(-10, 1), p(-10, 6)]);
        assert_eq!(violations(&rq, &r), Vec::<String>::new());
    }

    /// Whether a unit move of `r` ends strictly inside `o`.
    fn enters(r: &[(i64, i64)], o: [i64; 4]) -> bool {
        unit_moves(r).iter().any(|(_, b)| {
            let (x, y) = (b.0 * MM, b.1 * MM);
            o[0] < x && x < o[2] && o[1] < y && y < o[3]
        })
    }

    #[test]
    fn a_cp_leaves_its_own_box_and_texts_through_its_corridor() {
        // FT-706 friction 7: the second end's CP sits on the top edge of its own box (inflated by a step) and leaves
        // up through its own tag; the first end is below the box. The route goes round the box and comes down into
        // the CP through the tag, never across the box.
        let own_box = rect(-3, -1, 3, 9);
        let tag = rect(-2, -4, 2, 0);
        let mut rq = req(
            end(-8, 12, Some(Heading::Down)),
            end(0, 0, Some(Heading::Up)),
            vec![],
        );
        rq.area = [-14 * MM, -10 * MM, 8 * MM, 18 * MM];
        let skipped = auto_route(&rq);
        assert!(
            enters(&skipped, own_box),
            "without its own box the route crosses it: {skipped:?}"
        );
        rq.own[1] = vec![own_box, tag];
        let routed = route(&rq);
        let r = routed.points.clone();
        assert!(!routed.direct, "{r:?}");
        assert!(orthogonal(&r), "{r:?}");
        assert_eq!(r.last(), Some(&p(0, 0)));
        // The last segment comes straight down through the tag from above it: the corridor (0, -1) … (0, -4).
        let n = r.len();
        assert_eq!(r[n - 2].0, 0, "{r:?}");
        assert!(r[n - 2].1 <= -4 * MM, "{r:?}");
        // Nothing else of the route is inside the box or the tag.
        assert!(!enters(&r[..n - 1], own_box), "{r:?}");
        assert!(!enters(&r[..n - 1], tag), "{r:?}");
    }

    #[test]
    fn a_foreign_obstacle_across_the_corridor_still_blocks() {
        // The same tag as another representation's: the corridor is the stub only, and the CP cannot be left.
        let mut rq = req(
            end(-8, 12, Some(Heading::Down)),
            end(0, 0, Some(Heading::Up)),
            vec![],
        );
        rq.area = [-14 * MM, -10 * MM, 8 * MM, 18 * MM];
        rq.obstacles = vec![rect(-2, -4, 2, 0)];
        rq.own[1] = vec![rect(-3, -1, 3, 9)];
        let routed = route(&rq);
        assert!(routed.direct, "{:?}", routed.points);
        assert_eq!(routed.points, l_route(rq.from, rq.to));
    }

    #[test]
    fn the_route_does_not_depend_on_the_order_of_the_wires() {
        let base = with_wires(
            req(
                end(3, 0, Some(Heading::Down)),
                end(25, 2, Some(Heading::Left)),
                vec![rect(8, -3, 12, 0)],
            ),
            &[
                &[(0, 0), (0, 2), (20, 2)],
                &[(14, -6), (14, 8)],
                &[(-2, 5), (30, 5)],
            ],
        );
        let r0 = auto_route(&base);
        assert_eq!(auto_route(&base), r0, "twice the same");
        let mut wires = base.wires.clone();
        for _ in 0..wires.len() {
            wires.rotate_left(1);
            let mut reversed = wires.clone();
            reversed.reverse();
            for w in [wires.clone(), reversed] {
                assert_eq!(
                    auto_route(&RouteRequest {
                        wires: w,
                        ..base.clone()
                    }),
                    r0
                );
            }
        }
        assert_eq!(violations(&base, &r0), Vec::<String>::new());
    }

    #[test]
    fn the_fallback_says_it_is_direct() {
        let to = end(10, 10, None);
        let walls = vec![
            rect(7, 7, 13, 9),
            rect(7, 11, 13, 13),
            rect(7, 7, 9, 13),
            rect(11, 7, 13, 13),
        ];
        let walled = route(&req(end(0, 0, Some(Heading::Right)), to, walls));
        assert!(walled.direct);
        assert_eq!(walled.points, vec![p(0, 0), p(10, 0), p(10, 10)]);
        assert!(!route(&req(end(0, 0, Some(Heading::Right)), to, vec![])).direct);
    }

    #[test]
    fn a_text_thinner_than_a_step_between_two_nodes_is_not_run_over() {
        // A text from x = 4.2 to 5.8 mm and y = -0.4 to 0.4 mm holds no grid node, yet the straight run crosses it.
        let text = [4 * MM + MM / 5, -2 * MM / 5, 5 * MM + 4 * MM / 5, 2 * MM / 5];
        let rq = req(
            end(0, 0, Some(Heading::Right)),
            end(10, 0, Some(Heading::Left)),
            vec![text],
        );
        let r = route(&rq);
        assert!(!r.direct);
        assert_ne!(r.points, vec![p(0, 0), p(10, 0)]);
        // No segment of the route meets the text's open inside.
        for w in r.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
            let meets = x0 < text[2] && text[0] < x1.max(x0 + 1) && y0 < text[3] && text[1] < y1.max(y0 + 1);
            assert!(!meets, "{:?} meets the text", r.points);
        }
    }

    #[test]
    fn a_reserved_point_is_not_passed() {
        // Another CP's stub at (5, 0) on the straight line: the route goes round it.
        let mut rq = req(
            end(0, 0, Some(Heading::Right)),
            end(10, 0, Some(Heading::Left)),
            vec![],
        );
        assert_eq!(auto_route(&rq), vec![p(0, 0), p(10, 0)]);
        rq.reserved = vec![p(5, 0)];
        let r = route(&rq);
        assert!(!r.direct);
        let passes = |a: (i64, i64), b: (i64, i64), q: (i64, i64)| {
            (a.0.min(b.0)..=a.0.max(b.0)).contains(&q.0) && (a.1.min(b.1)..=a.1.max(b.1)).contains(&q.1)
        };
        assert!(
            !r.points.windows(2).any(|w| passes(w[0], w[1], p(5, 0))),
            "{:?}",
            r.points
        );
        // The ends themselves are never reserved away.
        rq.reserved = vec![p(0, 0), p(10, 0)];
        assert_eq!(auto_route(&rq), vec![p(0, 0), p(10, 0)]);
    }

    #[test]
    fn clear_judges_a_given_route_by_the_router_s_rules() {
        let (a, b) = (end(0, 0, Some(Heading::Down)), end(0, 10, Some(Heading::Up)));
        let straight = vec![p(0, 0), p(0, 10)];
        assert!(clear(&req(a, b, vec![]), &straight));
        // An obstacle across the run, a reserved point on it, another wire on its track: not clear.
        assert!(!clear(&req(a, b, vec![rect(-1, 4, 1, 6)]), &straight));
        let reserved = RouteRequest {
            reserved: vec![p(0, 5)],
            ..req(a, b, vec![])
        };
        assert!(!clear(&reserved, &straight));
        let on_track = RouteRequest {
            wires: vec![[p(0, 3), p(0, 7)]],
            ..req(a, b, vec![])
        };
        assert!(!clear(&on_track, &straight));
        // A wire crossing at a right angle through the inside of its segment is allowed, as in the router.
        let across = RouteRequest {
            wires: vec![[p(-3, 5), p(3, 5)]],
            ..req(a, b, vec![])
        };
        assert!(clear(&across, &straight));
        // A CP end left against its direction, or a route that misses an end or is not orthogonal: not clear.
        assert!(!clear(&req(end(0, 0, Some(Heading::Up)), b, vec![]), &straight));
        assert!(!clear(&req(a, b, vec![]), &[p(0, 0), p(0, 9)]));
        assert!(!clear(&req(a, end(1, 10, None), vec![]), &[p(0, 0), p(1, 10)]));
        // An L between perpendicular CPs.
        let l = req(
            end(0, 0, Some(Heading::Down)),
            end(5, 5, Some(Heading::Left)),
            vec![],
        );
        assert!(clear(&l, &[p(0, 0), p(0, 5), p(5, 5)]));
        assert!(!clear(&l, &[p(0, 0), p(5, 0), p(5, 5)]));
    }

    proptest::proptest! {
        /// The oracle of `clear`: every route the router finds (not the fallback) is clear by `clear`, among
        /// obstacles, other wires and reserved points.
        #[test]
        fn every_found_route_is_clear(
            fx in -6i64..6, fy in -6i64..6, tx in -6i64..6, ty in -6i64..6,
            fd in proptest::option::of(0usize..4), td in proptest::option::of(0usize..4),
            ob in proptest::collection::vec((-8i64..8, -8i64..8, 1i64..4, 1i64..4), 0..3),
            ws in proptest::collection::vec((-9i64..9, -9i64..9, -6i64..6, proptest::bool::ANY), 0..3),
            rs in proptest::collection::vec((-8i64..8, -8i64..8), 0..3),
        ) {
            let obstacles = ob.iter().map(|&(x, y, w, hh)| rect(x, y, x + w, y + hh)).collect();
            let rq = RouteRequest {
                wires: ws
                    .iter()
                    .map(|&(x, y, len, horizontal)| {
                        let b = if horizontal { (x + len, y) } else { (x, y + len) };
                        [p(x, y), p(b.0, b.1)]
                    })
                    .collect(),
                reserved: rs.iter().map(|&(x, y)| p(x, y)).collect(),
                ..req(
                    end(fx, fy, fd.map(|k| Heading::ALL[k])),
                    end(tx, ty, td.map(|k| Heading::ALL[k])),
                    obstacles,
                )
            };
            let routed = route(&rq);
            if !routed.direct {
                proptest::prop_assert!(clear(&rq, &routed.points), "{:?}", routed.points);
            }
        }
    }

    /// The cost of an orthogonal route on the 1 mm grid by the router's weights: one per step, [`BEND_COST`] per bend
    /// between two moves (a turn at the first end is never a bend: a CP end's first move is its direction, and an
    /// end without one turns freely at a fixed bend).
    fn cost(points: &[(i64, i64)]) -> u32 {
        let mut moves: Vec<Heading> = Vec::new();
        for w in points.windows(2) {
            let (dx, dy) = ((w[1].0 - w[0].0) / MM, (w[1].1 - w[0].1) / MM);
            let (h, n) = match (dx.signum(), dy.signum()) {
                (1, 0) => (Heading::Right, dx),
                (-1, 0) => (Heading::Left, -dx),
                (0, 1) => (Heading::Down, dy),
                (0, -1) => (Heading::Up, -dy),
                _ => continue,
            };
            moves.extend(std::iter::repeat_n(h, usize::try_from(n).unwrap()));
        }
        let bends = moves.windows(2).filter(|w| w[0] != w[1]).count();
        u32::try_from(moves.len() + bends * BEND_COST as usize).unwrap()
    }

    proptest::proptest! {
        /// FT-767, the converse of `every_found_route_is_clear`: a straight or L route between the ends that `clear`
        /// accepts is one the router's search accepts. The search finds a route (not the fallback), and of least cost,
        /// so no dearer than that one. The start may lie on a reserved point, another wire's vertex or inside an
        /// obstacle, with and without a direction, with and without an arrival heading.
        #[test]
        fn every_clear_straight_or_l_route_is_one_the_router_finds(
            fx in -6i64..6, fy in -6i64..6, tx in -6i64..6, ty in -6i64..6,
            fd in proptest::option::of(0usize..4), td in proptest::option::of(0usize..4),
            arrive in proptest::option::of(0usize..4),
            ob in proptest::collection::vec((-8i64..8, -8i64..8, 1i64..4, 1i64..4), 0..3),
            ws in proptest::collection::vec((-9i64..9, -9i64..9, -6i64..6, proptest::bool::ANY), 0..3),
            rs in proptest::collection::vec((-8i64..8, -8i64..8), 0..3),
            start_blocked in 0usize..4,
        ) {
            let mut obstacles: Vec<[i64; 4]> = ob.iter().map(|&(x, y, w, hh)| rect(x, y, x + w, y + hh)).collect();
            let mut reserved: Vec<(i64, i64)> = rs.iter().map(|&(x, y)| p(x, y)).collect();
            let mut wires: Vec<[(i64, i64); 2]> = ws
                .iter()
                .map(|&(x, y, len, horizontal)| {
                    let b = if horizontal { (x + len, y) } else { (x, y + len) };
                    [p(x, y), p(b.0, b.1)]
                })
                .collect();
            // Put the start on something the search would otherwise block.
            match start_blocked {
                1 => reserved.push(p(fx, fy)),
                2 => wires.push([p(fx, fy), p(fx + 3, fy)]),
                3 => obstacles.push([p(fx, fy).0 - MM / 2, p(fx, fy).1 - MM / 2, p(fx, fy).0 + MM / 2, p(fx, fy).1 + MM / 2]),
                _ => {}
            }
            let mut rq = RouteRequest {
                wires,
                reserved,
                ..req(
                    end(fx, fy, fd.map(|k| Heading::ALL[k])),
                    end(tx, ty, td.map(|k| Heading::ALL[k])),
                    obstacles,
                )
            };
            rq.arrive = arrive.map(|k| Heading::ALL[k]);
            let (a, b) = (p(fx, fy), p(tx, ty));
            let candidates: Vec<Vec<(i64, i64)>> = if a == b {
                Vec::new()
            } else if a.0 == b.0 || a.1 == b.1 {
                vec![vec![a, b]]
            } else {
                vec![vec![a, (a.0, b.1), b], vec![a, (b.0, a.1), b]]
            };
            for c in candidates {
                if !clear(&rq, &c) {
                    continue;
                }
                let routed = route(&rq);
                proptest::prop_assert!(!routed.direct, "clear accepts {:?}, the search found none", c);
                proptest::prop_assert!(
                    cost(&routed.points) <= cost(&c),
                    "clear accepts {:?} (cost {}), the search took {:?} (cost {})",
                    c, cost(&c), routed.points, cost(&routed.points)
                );
            }
        }
    }

    #[test]
    fn clear_checks_a_start_without_a_direction() {
        // From a fixed bend at (0, 0) without a direction, the straight run down to a CP entered from above.
        let rq = req(end(0, 0, None), end(0, 5, Some(Heading::Up)), vec![]);
        assert!(clear(&rq, &[p(0, 0), p(0, 5)]));
        // The bend on a reserved point or on another wire's vertex: the search frees the ends' own nodes, so the
        // router starts there, and `clear` agrees.
        let reserved = RouteRequest {
            reserved: vec![p(0, 0)],
            ..rq.clone()
        };
        assert!(clear(&reserved, &[p(0, 0), p(0, 5)]));
        assert!(!route(&reserved).direct);
        // Inside an obstacle the move out of it crosses the obstacle: neither takes it.
        let inside = req(
            end(0, 0, None),
            end(0, 5, Some(Heading::Up)),
            vec![rect(-1, -1, 1, 1)],
        );
        assert!(!clear(&inside, &[p(0, 0), p(0, 5)]));
        assert!(route(&inside).direct);
    }

    proptest::proptest! {
        /// FT-706: a route that is not the fallback never runs on another wire's track nor touches one of its
        /// vertices (but for the CP ends' stubs), whatever the wires.
        #[test]
        fn a_found_route_keeps_off_other_wires(
            fx in -6i64..6, fy in -6i64..6, tx in -6i64..6, ty in -6i64..6,
            fd in proptest::option::of(0usize..4), td in proptest::option::of(0usize..4),
            ws in proptest::collection::vec((-9i64..9, -9i64..9, -6i64..6, proptest::bool::ANY), 0..5),
        ) {
            let rq = RouteRequest {
                wires: ws
                    .iter()
                    .map(|&(x, y, len, horizontal)| {
                        let b = if horizontal { (x + len, y) } else { (x, y + len) };
                        [p(x, y), p(b.0, b.1)]
                    })
                    .collect(),
                ..req(end(fx, fy, fd.map(|k| Heading::ALL[k])), end(tx, ty, td.map(|k| Heading::ALL[k])), vec![])
            };
            let routed = route(&rq);
            proptest::prop_assert_eq!(routed.points.first(), Some(&p(fx, fy)));
            proptest::prop_assert_eq!(routed.points.last(), Some(&p(tx, ty)));
            proptest::prop_assert!(orthogonal(&routed.points), "{:?}", routed.points);
            if !routed.direct {
                let v = violations(&rq, &routed.points);
                proptest::prop_assert!(v.is_empty(), "{:?}: {:?}", routed.points, v);
            }
        }
    }

    proptest::proptest! {
        /// FT-437: from a point without a direction and an arrival heading, the first segment never runs against
        /// the arrival (but for the fallback between aligned ends), and the route is orthogonal with exact ends,
        /// among obstacles too.
        #[test]
        fn the_first_segment_never_reverses_the_arrival(
            fx in -6i64..6, fy in -6i64..6, tx in -6i64..6, ty in -6i64..6,
            h in 0usize..4, to_dir in proptest::option::of(0usize..4),
            ob in proptest::collection::vec((-8i64..8, -8i64..8, 1i64..4, 1i64..4), 0..4),
        ) {
            let arrive = Heading::ALL[h];
            let to = end(tx, ty, to_dir.map(|k| Heading::ALL[k]));
            let obstacles = ob.iter().map(|&(x, y, w, hh)| rect(x, y, x + w, y + hh)).collect();
            let rq = with_arrive(req(end(fx, fy, None), to, obstacles), arrive);
            let r = auto_route(&rq);
            // The fallback between aligned ends is the straight segment, the only L there is.
            let aligned_fallback = search(&rq).is_none() && (fx == tx || fy == ty);
            proptest::prop_assert_eq!(r.first(), Some(&p(fx, fy)));
            proptest::prop_assert_eq!(r.last(), Some(&p(tx, ty)));
            proptest::prop_assert!(orthogonal(&r), "{:?}", r);
            if r.len() >= 2 && !aligned_fallback {
                let (a, b) = (r[0], r[1]);
                let first = if b.0 > a.0 { Heading::Right } else if b.0 < a.0 { Heading::Left }
                    else if b.1 > a.1 { Heading::Down } else { Heading::Up };
                proptest::prop_assert_ne!(first, arrive.opposite(), "{:?}", r);
            }
        }
    }
}
