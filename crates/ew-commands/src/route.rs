// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The orthogonal router (WP-S2.04 §8.1, D7; doc 06 §5.4): a pure, integer-only function from two ends, obstacle
//! rectangles, the connection grid and a search area to a route, the full vertex list from the first end to the
//! second. The Wire tool calls it (through `ew_api::routing`) for its ghost and passes the result as
//! `connection.draw`'s `route`, so the command stores what the ghost showed and the router never runs on replay.
//!
//! **The search.** Grid nodes inside the area (the area's nearest grid indices, extended to hold both ends and
//! their stubs), 4-neighbourhood, cost 1 per step and 4 per bend. A node whose page point lies strictly inside an
//! obstacle rectangle is blocked; the two ends, their nearest grid nodes and their stub nodes never are. A CP end
//! (one with a leaving direction) first leaves one step along its direction (the stub) and is entered along the
//! opposite of its direction; a junction or split end (`dir: None`) has no such constraint.
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
}

impl Space {
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
    search(req).unwrap_or_else(|| l_route(req.from, req.to))
}

fn search(req: &RouteRequest) -> Option<Vec<(i64, i64)>> {
    let (from, to) = (req.from, req.to);
    if from.at == to.at {
        return Some(vec![from.at]);
    }
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
    };
    // A node is blocked when its page point lies strictly inside an obstacle.
    for o in &req.obstacles {
        let (x0, x1) = (o[0].min(o[2]), o[0].max(o[2]));
        let (y0, y1) = (o[1].min(o[3]), o[1].max(o[3]));
        let (i0, i1) = (idx(x0).max(lo.0), idx(x1).min(hi.0));
        let (j0, j1) = (idx(y0).max(lo.1), idx(y1).min(hi.1));
        for j in j0..=j1 {
            let Some(cy) = space.coord(j) else { continue };
            if !(y0 < cy && cy < y1) {
                continue;
            }
            for i in i0..=i1 {
                let Some(cx) = space.coord(i) else { continue };
                if x0 < cx && cx < x1 {
                    if let Some(n) = space.node((i, j)) {
                        space.blocked[n] = true;
                    }
                }
            }
        }
    }
    for p in [Some(s), Some(t), s1, t1].into_iter().flatten() {
        if let Some(n) = space.node(p) {
            space.blocked[n] = false;
        }
    }
    let goal = space.node(t)?;
    // The state the forward walk starts from: the first end's stub with its heading, else the first end.
    let start_state = match (from.dir, s1) {
        (Some(d), Some(p)) => state(space.node(p)?, Some(d)),
        _ => state(space.node(s)?, None),
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
        if space.blocked[n1] {
            continue;
        }
        for h in [
            None,
            Some(Heading::Right),
            Some(Heading::Down),
            Some(Heading::Left),
            Some(Heading::Up),
        ] {
            let cand = add(weight(h, m), d);
            let s1 = state(n1, h);
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
        _ => (start, None),
    };
    if dist[state(node, heading)] == INF {
        return None;
    }
    while dist[state(node, heading)] != (0, 0) {
        let here = dist[state(node, heading)];
        let p = position(node);
        let next = Heading::ALL.into_iter().find_map(|m| {
            let q = (p.0 + m.step().0, p.1 + m.step().1);
            let n = space.node(q)?;
            if space.blocked[n] {
                return None;
            }
            let there = dist[state(n, Some(m))];
            (there != INF && add(weight(heading, m), there) == here).then_some((n, m))
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
}
