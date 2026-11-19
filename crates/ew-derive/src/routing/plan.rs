// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page solve ([`route_page`]) and one route against a page as drawn ([`route_one`]): the search
//! of [`super::route`] fed with a page's geometry under the hard rules, the costs, the order,
//! the mates and the areas and rip-up rounds, deterministic.
//!
//! **One job's request.** The ends are the connection's CPs (with their leaving directions) or junctions. Every
//! representation's body and black-box outline grown by one grid step, and every text box grown by a quarter step,
//! block; the ends' own representations' bodies block but for the escape corridors, and their texts block as any
//! other text (rule 4 has no exemption for a wire's own texts, nor for its stubs). The frame's
//! bands block, the drawing area's edge grown inward by one step but on a side where an end or its stub stands
//! within that step. The routed connections that are not re-routed, and the jobs routed so far, are the
//! other wires (an unrouted connection outside the solve is no obstacle). Every other connection point and its stub,
//! and every
//! junction the job does not end at, is reserved. A crossing costs [`CROSSING_COST`]; a step beside a routed mate of
//! the job's group costs [`MATE_STEP_COST`](super::MATE_STEP_COST).
//!
//! **Four arms.** A job ending at a junction that the other routed connections already leave in three directions is
//! not routed (its route would be the fourth arm, rule 7).
//!
//! **Areas.** The ends' bounding box grown by 4 grid steps, then by 16, then the whole drawing area (the sheet on a
//! page without a frame grid), each clipped to it; the first area with a route that crosses no text wins, else the
//! cheapest route across a text of all three.
//!
//! **Across the texts** (ADR-0033 amendment 2). Entering a text costs [`TEXT_COST`] and each step on it a step more;
//! the best route across a text is taken where every text-clear route is more than 8/5 of its length (round a symbol
//! group, round the sheet) or none exists.
//! A route across a text must keep every other hard rule by the layout measures; where it does not, the job is
//! routed with the texts as obstacles. The plan lists the routes across a text in `over_texts`.
//!
//! **The half grid, last** (ADR-0033 amendment 3). Where the page's grid gives no route, or one more than 8/5 of the
//! direct way between the ends, the same search runs on half the grid (bodies and frame grown by half a step, a free
//! pin's half step reserved too, a step between the grid lines only between the ends); its route is taken where the
//! whole grid's is more than 8/5 of it or there is none. A stub point that another pin's wire leaves through is that
//! wire's, also where it is an end's own stub (two poles wired cross-over).
//!
//! **One number for a grid position** (ADR-0033 amendment 4). The solve and [`route_one`] read the settled page
//! ([`PageGeometry::settled`]): a coordinate within 1 nm of a grid position is the grid's own number, so a step of
//! 1 nm in a wire between two pins of one grid line is no stretch of wire. A route found there takes its ends' stored
//! numbers again. A vertex another wire runs straight through is no corner to a route.
//!
//! **A reroute** (amendment 4). Jobs with stored routes are not only routed from nothing: the drawing as it is, each
//! wire routed again against all the others and the new route taken where the old one breaks a hard rule, crosses a
//! text the new one keeps clear of (amendment 5) or costs more as drawn, is rated against the jobs routed from nothing,
//! the shortest first, and the better is taken. A page
//! routed for the first time (no job has a route) is routed from nothing twice, the longest first and the shortest
//! first; the shortest-first drawing is taken where it rates better with no more wires without a good route, bends,
//! crossings and texts crossed, else the longest-first one.
//!
//! **Reasons.** A job left unrouted names the first rule its free route (against the fixed obstacles alone) breaks:
//! the search is tried again without the other wires, then also without the texts, then also without the reserved
//! points, then also without the bodies and the frame; the first relaxation that finds a route names the rule. With
//! none, the reason is the symbols.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;
use ew_schema::Dir4;

use super::measures::{Measures, TouchedPoint, measure, meet, overlap};
use super::page::{PageGeometry, Point, Rect};
use super::search::{
    BEND_COST, CROSSING_COST, Heading, RouteEnd, RouteRequest, STEP_COST, TEXT_COST, fit, route,
};

/// Why a job of a solve is left unrouted (§5.6): the first rule its free route broke.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Unrouted {
    /// No route keeps clear of the other wires.
    Wires,
    /// No route keeps clear of the symbols (bodies, black boxes, the frame).
    Symbols,
    /// No route keeps clear of the texts.
    Texts,
    /// No route keeps clear of the other connection points (and the junctions).
    Points,
    /// The junction at an end would get a fourth arm.
    FourArms,
}

/// The result of a solve: the routes in D2's form (both ends included) and the jobs left unrouted with their reasons,
/// each list in the order of the connection IDs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RoutePlan {
    /// The routed jobs: connection and route.
    pub routes: Vec<(Uuid, Vec<Point>)>,
    /// The jobs left unrouted: connection and reason.
    pub unrouted: Vec<(Uuid, Unrouted)>,
    /// The routed jobs whose route crosses a text: no route kept clear of the texts, and one through them is better
    /// than none (the persona re-test: a tube through a valve's body), in the order of the connection IDs.
    pub over_texts: Vec<Uuid>,
}

/// The order of the supply potentials (§5.3, §5.4): L1, L2, L3, L, N, PEN, PE, FE, L+, M, L−, as the rank in
/// [`DrawnConnection::supply`](super::DrawnConnection::supply).
pub const SUPPLY_ORDER: [&str; 11] = ["L1", "L2", "L3", "L", "N", "PEN", "PE", "FE", "L+", "M", "L-"];

/// One end of a route against a page ([`route_one`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OneEnd {
    /// Its point.
    pub at: Point,
    /// A CP end's leaving direction; `None` for a junction or a free point.
    pub dir: Option<Heading>,
    /// The representation that draws a CP end, whose rectangles are the end's own.
    pub rep: Option<Uuid>,
}

/// An end of a job, from the page's CPs and junctions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct End {
    at: Point,
    dir: Option<Heading>,
    rep: Option<Uuid>,
    /// The CP or junction ID.
    id: Uuid,
    junction: bool,
}

/// The heading of a known direction.
fn heading(d: &Dir4) -> Option<Heading> {
    match d {
        Dir4::Up => Some(Heading::Up),
        Dir4::Down => Some(Heading::Down),
        Dir4::Left => Some(Heading::Left),
        Dir4::Right => Some(Heading::Right),
        Dir4::Unknown(_) => None,
    }
}

/// The grid step in nm, rounded.
fn step(page: &PageGeometry) -> i64 {
    let (num, den) = page.grid;
    if den <= 0 {
        return num.max(1);
    }
    ((num + den / 2) / den).max(1)
}

/// The area the search may use at most: the drawing area, else the sheet.
fn whole(page: &PageGeometry) -> Rect {
    page.drawing_area.unwrap_or(page.sheet)
}

fn grow(r: Rect, d: i64) -> Rect {
    [r[0] - d, r[1] - d, r[2] + d, r[3] + d]
}

/// The ends of connection `id`: the CPs or junctions its `ends` name.
fn ends_of(page: &PageGeometry, id: Uuid) -> Option<[End; 2]> {
    let c = page.connections.iter().find(|c| c.id == id)?;
    let end = |e: Uuid| -> Option<End> {
        if let Some(cp) = page.cps.iter().find(|cp| cp.id == e) {
            return Some(End {
                at: cp.at,
                dir: cp.dir.as_ref().and_then(heading),
                rep: Some(cp.rep),
                id: e,
                junction: false,
            });
        }
        let j = page.junctions.iter().find(|j| j.id == e)?;
        Some(End {
            at: j.at,
            dir: None,
            rep: None,
            id: e,
            junction: true,
        })
    };
    Some([end(c.ends[0])?, end(c.ends[1])?])
}

/// What a request leaves out, to name the rule a free route breaks.
#[derive(Clone, Copy, Default)]
struct Relax {
    wires: bool,
    texts: bool,
    bodies: bool,
    points: bool,
}

/// The frame's obstacles: the sheet outside the drawing area grown inward by `g`, but on a side where one
/// of `ends` stands within `g`.
fn frame_rects(page: &PageGeometry, g: i64, ends: &[Point]) -> Vec<Rect> {
    let Some([x0, y0, x1, y1]) = page.drawing_area else {
        return Vec::new();
    };
    let [sx0, sy0, sx1, sy1] = page.sheet;
    let inset = |near: &dyn Fn(&Point) -> bool| if ends.iter().any(near) { 0 } else { g };
    let ix0 = x0 + inset(&|p| p.0 < x0 + g);
    let iy0 = y0 + inset(&|p| p.1 < y0 + g);
    let ix1 = x1 - inset(&|p| p.0 > x1 - g);
    let iy1 = y1 - inset(&|p| p.1 > y1 - g);
    if ix0 >= ix1 || iy0 >= iy1 {
        return Vec::new();
    }
    vec![
        [sx0.min(x0) - g, sy0.min(y0) - g, sx1.max(x1) + g, iy0],
        [sx0.min(x0) - g, iy1, sx1.max(x1) + g, sy1.max(y1) + g],
        [sx0.min(x0) - g, iy0, ix0, iy1],
        [ix1, iy0, sx1.max(x1) + g, iy1],
    ]
}

/// The request of a route from `a` to `b` on `page` against the `wires` and with the `mates`' segments; with `half`,
/// on half the page's grid (ADR-0033 amendment 3): the bodies and the frame grown by half a step, a step between the
/// page's grid lines only between the ends ([`RouteRequest::half_grid`]), the other connection points' stubs and the
/// texts as on the whole grid.
fn request(
    page: &PageGeometry,
    (a, b): (&OneEnd, &OneEnd),
    skip: (&[Uuid], &[Point]),
    wires: &[[Point; 2]],
    mates: &[[Point; 2]],
    relax: Relax,
    half: bool,
) -> RouteRequest {
    let (skip_points, skip_at) = skip;
    let g = step(page);
    // The growth of the bodies and the frame: one step of the grid searched.
    let gb = if half { g / 2 } else { g };
    let stub = |e: &OneEnd| {
        e.dir.map_or(e.at, |d| {
            let v = d.step();
            (e.at.0 + v.0 * g, e.at.1 + v.1 * g)
        })
    };
    let mut obstacles: Vec<Rect> = Vec::new();
    let mut own: [Vec<Rect>; 2] = [Vec::new(), Vec::new()];
    // A representation that draws a CP on a CP end's point (an interruption point drawn on a pin) is that end's own
    // too: the wire leaves the pin through it, as through the pin's own symbol.
    let on_end =
        |e: &OneEnd, rep: Uuid| e.dir.is_some() && page.cps.iter().any(|c| c.rep == rep && c.at == e.at);
    let place = |rep: Uuid, r: Rect, obstacles: &mut Vec<Rect>, own: &mut [Vec<Rect>; 2]| {
        let mut mine = false;
        for (k, e) in [a, b].into_iter().enumerate() {
            if e.rep == Some(rep) || on_end(e, rep) {
                own[k].push(r);
                mine = true;
            }
        }
        if !mine {
            obstacles.push(r);
        }
    };
    if !relax.bodies {
        // A body that holds an end of another representation is an outline drawn around it (a valve terminal's box
        // around its valves, the persona round), never an obstacle: a route could not leave it.
        let inside = |r: Rect, e: &OneEnd| r[0] < e.at.0 && e.at.0 < r[2] && r[1] < e.at.1 && e.at.1 < r[3];
        for body in &page.bodies {
            if [a, b]
                .into_iter()
                .any(|e| e.rep != Some(body.rep) && inside(body.rect, e))
            {
                continue;
            }
            place(body.rep, grow(body.rect, gb), &mut obstacles, &mut own);
        }
        obstacles.extend(frame_rects(page, gb, &[a.at, stub(a), b.at, stub(b)]));
    }
    // Rule 4: every text blocks, the ends' own included, across a stub too; the escape corridors
    // pass bodies only.
    let texts = if relax.texts {
        Vec::new()
    } else {
        page.texts.iter().map(|t| grow(t.rect, g / 4)).collect()
    };
    let mut reserved: Vec<Point> = Vec::new();
    // An end's own stub is the end's way out: another CP facing it two steps away (the stacked valves of the persona
    // re-test, port 4 under port 5) has its stub on the same point, which reserves it for neither. Unless that point
    // has a wire of its own: then the point is the other wire's way out, and a corner there would meet it (two
    // poles wired cross-over, ADR-0033 amendment 3); on the half grid each takes a half step instead.
    // Two stubs are one point when they snap to one grid node (a CP may sit up to 1 nm off a rational grid).
    let grid = ew_units::Grid::new(
        u64::try_from(page.grid.0).unwrap_or(0),
        u64::try_from(page.grid.1).unwrap_or(0),
    )
    .ok();
    let node = |p: Point| grid.map(|gr| (gr.snap(ew_units::Nm(p.0)), gr.snap(ew_units::Nm(p.1))));
    let own_stubs = [a, b].map(|e| e.dir.map(|_| node(stub(e))));
    if !relax.points {
        for cp in &page.cps {
            if skip_points.contains(&cp.id) || skip_at.contains(&cp.at) {
                continue;
            }
            reserved.push(cp.at);
            if let Some(d) = cp.dir.as_ref().and_then(heading) {
                let v = d.step();
                let at = (cp.at.0 + v.0 * g, cp.at.1 + v.1 * g);
                if !own_stubs.contains(&Some(node(at))) || !cp.connections.is_empty() {
                    reserved.push(at);
                }
                // On the half grid, a free connection point's half step too: a wire passing right in front of a pin
                // reads as joined to it.
                if half && cp.connections.is_empty() {
                    reserved.push((cp.at.0 + v.0 * g / 2, cp.at.1 + v.1 * g / 2));
                }
            }
        }
        for j in &page.junctions {
            if !skip_points.contains(&j.id) && !skip_at.contains(&j.at) {
                reserved.push(j.at);
            }
        }
    }
    RouteRequest {
        from: RouteEnd { at: a.at, dir: a.dir },
        to: RouteEnd { at: b.at, dir: b.dir },
        obstacles,
        texts,
        grid: if half {
            (page.grid.0, page.grid.1.saturating_mul(2))
        } else {
            page.grid
        },
        area: whole(page),
        arrive: None,
        own,
        wires: if relax.wires { Vec::new() } else { wires.to_vec() },
        reserved,
        cross_cost: CROSSING_COST,
        text_cost: Some(TEXT_COST),
        mates: mates.to_vec(),
        half_grid: half,
    }
}

/// Whether the polyline `line` runs through the open inside of one of the rectangles `texts`.
fn through_any(texts: &[Rect], line: &[Point]) -> bool {
    let meets = |lo: i64, hi: i64, rlo: i64, rhi: i64| {
        if lo == hi {
            rlo < lo && lo < rhi
        } else {
            lo < rhi && rlo < hi
        }
    };
    segments(line).any(|[a, b]| {
        texts.iter().any(|r| {
            meets(a.0.min(b.0), a.0.max(b.0), r[0], r[2]) && meets(a.1.min(b.1), a.1.max(b.1), r[1], r[3])
        })
    })
}

/// Whether `route` runs through a text box of `page` with the router's clearance (grown by a quarter step, as a request
/// grows it): a route across a text.
pub fn crosses_text(page: &PageGeometry, route: &[Point]) -> bool {
    let g = step(page);
    let texts: Vec<Rect> = page.texts.iter().map(|t| grow(t.rect, g / 4)).collect();
    through_any(&texts, route)
}

/// The route of `req` in the areas of §5.5 (4 and 16 grid steps around the ends, then the whole area), with its
/// cost; `None` when no area holds one.
fn search_areas(page: &PageGeometry, req: &RouteRequest) -> Option<(Vec<Point>, u32)> {
    let g = step(page);
    let w = whole(page);
    let (a, b) = (req.from.at, req.to.at);
    let around = |m: i64| {
        [
            (a.0.min(b.0) - m * g).max(w[0]),
            (a.1.min(b.1) - m * g).max(w[1]),
            (a.0.max(b.0) + m * g).min(w[2]),
            (a.1.max(b.1) + m * g).min(w[3]),
        ]
    };
    // A route across a text may be beaten by a text-clear one in a larger area.
    let mut across: Option<(Vec<Point>, u32)> = None;
    for area in [around(4), around(16), w] {
        let r = route(&RouteRequest { area, ..req.clone() });
        if r.direct {
            continue;
        }
        if req.text_cost.is_none() || !through_any(&req.texts, &r.points) {
            if across.as_ref().is_none_or(|(_, c)| r.cost <= *c) {
                return Some((r.points, r.cost));
            }
            break;
        }
        if across.as_ref().is_none_or(|(_, c)| r.cost < *c) {
            across = Some((r.points, r.cost));
        }
    }
    across
}

/// How much longer, at the least, every text-clear route must be than a route across a text for the route across to
/// be taken (ADR-0033 amendment 2): 8/5. A way round of a few steps and a bend or two stays; a way round a symbol
/// group or the sheet does not. (A cost alone cannot tell them apart: a bend weighs four steps, so a short way round
/// with two bends costs as much as a long one without.)
const FAR: (i64, i64) = (8, 5);

/// The length of a polyline, nm.
fn length(line: &[Point]) -> i64 {
    segments(line)
        .map(|[a, b]| (b.0 - a.0).abs() + (b.1 - a.1).abs())
        .sum()
}

/// The route of the request `make` builds, on the page's grid ([`search_grid`]), else on its half grid (ADR-0033
/// amendment 3): the half grid is tried last, where the whole grid gives no route or one more than [`FAR`] of the
/// direct way, and its route is taken where the whole grid's is more than [`FAR`] of it or there is none.
fn search_checked(
    page: &PageGeometry,
    make: impl Fn(bool) -> Option<RouteRequest>,
    drawn: impl Fn() -> (PageGeometry, Uuid),
) -> Option<(Vec<Point>, u32)> {
    let req = make(false)?;
    let whole = search_grid(page, &req, &drawn);
    let (a, b) = (req.from.at, req.to.at);
    let direct = (a.0 - b.0).abs() + (a.1 - b.1).abs();
    let far = |longer: &[Point], than: i64| length(longer) * FAR.1 > than * FAR.0;
    if whole.as_ref().is_some_and(|(r, _)| !far(r, direct)) {
        return whole;
    }
    let half = make(true).and_then(|req| search_grid(page, &req, &drawn));
    match (whole, half) {
        (Some(w), Some(h)) if !far(&w.0, length(&h.0)) => Some(w),
        (w, None) => w,
        (_, h) => h,
    }
}

/// [`search_areas`] with a route across a text taken only where every text-clear route is far longer ([`FAR`]) or
/// none exists, and where it keeps every other hard rule on `drawn` as connection `id` ([`keeps_all_but_texts`]);
/// else the route with the texts as obstacles.
fn search_grid(
    page: &PageGeometry,
    req: &RouteRequest,
    drawn: &impl Fn() -> (PageGeometry, Uuid),
) -> Option<(Vec<Point>, u32)> {
    let found = search_areas(page, req)?;
    if !crosses_text(page, &found.0) {
        return Some(found);
    }
    let clear = search_areas(
        page,
        &RouteRequest {
            text_cost: None,
            ..req.clone()
        },
    );
    let far = clear
        .as_ref()
        .is_none_or(|(c, _)| length(c) * FAR.1 > length(&found.0) * FAR.0);
    if far && {
        let (drawn, id) = drawn();
        keeps_all_but_texts(&drawn, id, &found.0)
    } {
        return Some(found);
    }
    clear
}

/// The segments of a polyline, without empty ones.
fn segments(line: &[Point]) -> impl Iterator<Item = [Point; 2]> + '_ {
    line.windows(2).filter(|w| w[0] != w[1]).map(|w| [w[0], w[1]])
}

/// The straight runs of a polyline, as another route meets it: its segments, those that run straight on along x or y
/// joined into one. A vertex a wire runs straight through (a stored point on a straight stretch, or the place of a
/// 1 nm step once the page is settled) is no corner: another wire may cross there as anywhere on the run.
fn runs(line: &[Point]) -> Vec<[Point; 2]> {
    let on = |p: Point, q: Point, r: Point| {
        (p.0 == q.0 && q.0 == r.0 && (q.1 - p.1).signum() == (r.1 - q.1).signum())
            || (p.1 == q.1 && q.1 == r.1 && (q.0 - p.0).signum() == (r.0 - q.0).signum())
    };
    let mut out: Vec<[Point; 2]> = Vec::new();
    for [a, b] in segments(line) {
        match out.last_mut() {
            Some(last) if on(last[0], last[1], b) => last[1] = b,
            _ => out.push([a, b]),
        }
    }
    out
}

/// The directions in which the polylines of `lines` leave the point `j` (reduced to unit steps).
fn arms(j: Point, lines: &[&[Point]]) -> BTreeSet<(i64, i64)> {
    let mut out = BTreeSet::new();
    for line in lines {
        let line: Vec<Point> = line.iter().copied().fold(Vec::new(), |mut v, p| {
            if v.last() != Some(&p) {
                v.push(p);
            }
            v
        });
        if line.len() < 2 {
            continue;
        }
        let next = if line[0] == j {
            line[1]
        } else if line[line.len() - 1] == j {
            line[line.len() - 2]
        } else {
            continue;
        };
        out.insert(((next.0 - j.0).signum(), (next.1 - j.1).signum()));
    }
    out
}

/// Whether the two polylines meet against a hard rule: a shared stretch, or a point where a vertex of one lies on
/// the other (a right-angle crossing through two segments' insides is allowed).
fn conflict(p: &[Point], q: &[Point]) -> bool {
    let (sp, sq): (Vec<[Point; 2]>, Vec<[Point; 2]>) = (segments(p).collect(), segments(q).collect());
    let vertex = |x: Point| p.contains(&x) || q.contains(&x);
    sp.iter().any(|s| {
        sq.iter().any(|t| {
            overlap((s[0], s[1]), (t[0], t[1])).is_some()
                || meet((s[0], s[1]), (t[0], t[1])).is_some_and(vertex)
        })
    })
}

/// The solve's state: the routes found so far with their costs.
#[derive(Clone, Default)]
struct State {
    routes: BTreeMap<Uuid, (Vec<Point>, u32)>,
}

/// What a wire breaks of the hard rules by the layout measures, the rules the checks report: the points it touches,
/// the wires it lies on and the bodies it runs through (the grave ones), and apart from them the texts it runs across.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
struct Faults {
    /// `(rule, what)`: 0 a touched point, 1 a wire it lies on, 2 a body it runs through.
    grave: BTreeSet<(u8, Uuid)>,
    /// The representations whose texts it runs across.
    texts: BTreeSet<Uuid>,
}

impl Faults {
    /// Each wire's faults in the measures `m`.
    fn of(m: &Measures) -> BTreeMap<Uuid, Faults> {
        let mut out: BTreeMap<Uuid, Faults> = BTreeMap::new();
        for t in &m.touches {
            let point = match t.point {
                TouchedPoint::Cp(id) | TouchedPoint::Junction(id) => id,
            };
            out.entry(t.connection).or_default().grave.insert((0, point));
        }
        for o in &m.overlaps {
            let [a, b] = o.connections;
            out.entry(a).or_default().grave.insert((1, b));
            out.entry(b).or_default().grave.insert((1, a));
        }
        for t in &m.through_bodies {
            out.entry(t.connection).or_default().grave.insert((2, t.rep));
        }
        for t in &m.through_texts {
            out.entry(t.connection).or_default().texts.insert(t.rep);
        }
        out
    }

    /// Whether a wire with these faults, drawn anew, breaks no hard rule it kept with the faults `before`: no point,
    /// wire or body it did not touch, lie on or run through before, and no text it did not run across before, unless
    /// it is rid of a grave fault for it (a page is never worse after a reroute, rc.5).
    fn no_worse(&self, before: &Faults) -> bool {
        self.grave.is_subset(&before.grave)
            && (self.texts.is_subset(&before.texts) || self.grave.len() < before.grave.len())
    }
}

/// The page solve's fixed context.
struct Solve<'a> {
    page: &'a PageGeometry,
    /// The jobs in routing order.
    order: Vec<Uuid>,
    ends: BTreeMap<Uuid, [End; 2]>,
    /// The segments of the routed connections outside the solve.
    fixed: Vec<[Point; 2]>,
    /// The fixed routed polylines (for the junctions' arms).
    fixed_lines: Vec<&'a [Point]>,
    /// Each job's mates, routed before it in its group.
    mates: BTreeMap<Uuid, Vec<Uuid>>,
    /// The jobs found to have no route even with no other job routed: no state of the solve gives them one (another
    /// job's route only takes room), so they are not searched for again, each search of theirs covering the whole
    /// page and its half grid in vain.
    hopeless: RefCell<BTreeSet<Uuid>>,
}

impl Solve<'_> {
    fn one(e: &End) -> OneEnd {
        OneEnd {
            at: e.at,
            dir: e.dir,
            rep: e.rep,
        }
    }

    /// Whether a job ending at a junction would make its fourth arm in `state`.
    fn four_arms(&self, job: Uuid, state: &State) -> bool {
        let Some(ends) = self.ends.get(&job) else {
            return false;
        };
        ends.iter().filter(|e| e.junction).any(|e| {
            let mut lines: Vec<&[Point]> = self.fixed_lines.clone();
            lines.extend(
                state
                    .routes
                    .iter()
                    .filter(|(id, _)| **id != job)
                    .map(|(_, (r, _))| r.as_slice()),
            );
            arms(e.at, &lines).len() >= 3
        })
    }

    /// The request of `job` against the fixed wires and the routes of `state` (none with `alone`), on the half grid
    /// with `half`.
    fn request(
        &self,
        job: Uuid,
        state: &State,
        alone: bool,
        relax: Relax,
        half: bool,
    ) -> Option<RouteRequest> {
        let [a, b] = self.ends.get(&job)?;
        let mut wires = self.fixed.clone();
        if !alone {
            for (id, (r, _)) in &state.routes {
                if *id != job {
                    wires.extend(runs(r));
                }
            }
        }
        let mates: Vec<[Point; 2]> = self
            .mates
            .get(&job)
            .into_iter()
            .flatten()
            .filter_map(|m| state.routes.get(m))
            .flat_map(|(r, _)| runs(r))
            .collect();
        Some(request(
            self.page,
            (&Self::one(a), &Self::one(b)),
            (&[a.id, b.id], &[a.at, b.at]),
            &wires,
            &mates,
            relax,
            half,
        ))
    }

    /// Routes `job` in `state`; whether it found a route. With `defer`, a route across a text is not taken yet: the
    /// jobs that cross a text are routed after the others, so that a text-clear route keeps its way.
    fn route(&self, job: Uuid, state: &mut State, defer: bool) -> bool {
        if self.hopeless.borrow().contains(&job) || self.four_arms(job, state) {
            return false;
        }
        let found = search_checked(
            self.page,
            |half| self.request(job, state, false, Relax::default(), half),
            || (self.drawn(state), job),
        );
        if found.is_none() {
            // Without a way between its ends on the page's grid or its half grid, whatever it costs and with no
            // other job routed, the job has none in any state.
            let way = |half: bool| {
                self.request(job, state, true, Relax::default(), half)
                    .and_then(|req| search_areas(self.page, &req))
                    .is_some()
            };
            if !way(false) && !way(true) {
                self.hopeless.borrow_mut().insert(job);
            }
        }
        let found = found.filter(|(r, _)| !defer || !crosses_text(self.page, r));
        match found {
            Some(r) => {
                state.routes.insert(job, r);
                true
            }
            None => false,
        }
    }

    /// The page as `state` draws it: the routes found so far stored on their connections, a job without one unrouted.
    fn drawn(&self, state: &State) -> PageGeometry {
        let mut drawn = self.page.clone();
        for c in &mut drawn.connections {
            if let Some((route, _)) = state.routes.get(&c.id) {
                c.polyline.clone_from(route);
                c.routed = true;
            } else if self.ends.contains_key(&c.id) {
                c.routed = false;
            }
        }
        drawn
    }

    /// Routes the jobs `state` has no route for: the first pass in routing order, then at most two rip-up rounds,
    /// each kept only when it scores better, then the jobs left, across a text where that is their way.
    fn construct(&self, mut state: State) -> State {
        for &j in &self.order {
            if !state.routes.contains_key(&j) {
                self.route(j, &mut state, true);
            }
        }
        for _ in 0..2 {
            if self.failed(&state).is_empty() {
                break;
            }
            let next = self.round(&state);
            if self.score(&next) < self.score(&state) {
                state = next;
            } else {
                break;
            }
        }
        for j in self.failed(&state) {
            self.route(j, &mut state, false);
        }
        state
    }

    /// `state` with each job routed again against all the others, in routing order, and the new route taken where
    /// the old one breaks a hard rule, where the old one crosses a text and the new one keeps clear of every text, or
    /// where the new one is drawn at less cost ([`Self::cost`]); at most two passes, the second only where the first
    /// changed a route. A job without a better route keeps the one it has. (The search offers a route clear of the texts
    /// wherever it is not far longer than the best across one; weighed as drawn, a text costs as much as a bend and six
    /// steps, so a stored wire through a text kept its route where the Wire tool would have gone round: rc.5.) A new
    /// route is never taken where it breaks a hard rule the old one kept ([`Self::no_worse`]).
    fn improve(&self, mut state: State) -> State {
        for _ in 0..2 {
            let bad = self.bad(&state);
            let mut changed = false;
            for &j in &self.order {
                let Some(old) = state.routes.remove(&j) else {
                    continue;
                };
                let found = if self.four_arms(j, &state) {
                    None
                } else {
                    search_checked(
                        self.page,
                        |half| self.request(j, &state, false, Relax::default(), half),
                        || (self.drawn(&state), j),
                    )
                };
                match found {
                    Some(new)
                        if new.0 != old.0
                            && (bad.contains(&j)
                                || (crosses_text(self.page, &old.0) && !crosses_text(self.page, &new.0))
                                || self.cost(j, &new.0, &state) < self.cost(j, &old.0, &state))
                            && self.no_worse(j, &new.0, &old.0, &state) =>
                    {
                        state.routes.insert(j, new);
                        changed = true;
                    }
                    _ => {
                        state.routes.insert(j, old);
                    }
                }
            }
            if !changed {
                break;
            }
        }
        state
    }

    /// What `route` costs as the wire of `job` beside the other wires of `state`, by the weights of the search and
    /// read off the drawing: its length in steps, its bends, the wires it crosses and the texts it runs across.
    fn cost(&self, job: Uuid, route: &[Point], state: &State) -> u64 {
        let [steps, bends, crossings, texts] = self.counts(job, route, state);
        steps
            + u64::from(BEND_COST) * bends
            + u64::from(CROSSING_COST) * crossings
            + u64::from(TEXT_COST) * texts
    }

    /// What [`Self::cost`] weighs: the route's length in step costs, its bends, the wires it crosses and the texts
    /// it runs across.
    fn counts(&self, job: Uuid, route: &[Point], state: &State) -> [u64; 4] {
        let g = step(self.page);
        let steps = (length(route) * i64::from(STEP_COST) + g / 2) / g;
        let mine = runs(route);
        let (first, last) = (route.first(), route.last());
        let others = self.fixed.iter().copied().chain(
            state
                .routes
                .iter()
                .filter(|(id, _)| **id != job)
                .flat_map(|(_, (r, _))| runs(r)),
        );
        let mut crossings = 0_u64;
        for t in others {
            for s in &mine {
                let (s, t) = ((s[0], s[1]), (t[0], t[1]));
                if overlap(s, t).is_none()
                    && meet(s, t).is_some_and(|p| Some(&p) != first && Some(&p) != last)
                {
                    crossings += 1;
                }
            }
        }
        let texts = self
            .page
            .texts
            .iter()
            .filter(|t| through_any(&[grow(t.rect, g / 4)], route))
            .count();
        [
            u64::try_from(steps).unwrap_or(0),
            mine.len().saturating_sub(1) as u64,
            crossings,
            texts as u64,
        ]
    }

    /// Whether `state` draws the jobs better than `than` at no price a reader sees: a better rating
    /// ([`Self::rating`]) with no more wires without a good route, no more bends, no more crossings and no more
    /// texts crossed. (A better rating alone may buy fewer crossings with more bends and ways round the back of a
    /// symbol, which reads worse than the crossings it saves.)
    fn better_at_no_price(&self, state: &State, than: &State) -> bool {
        let seen = |s: &State| {
            let mut sum = [0_u64; 3];
            for (id, (r, _)) in &s.routes {
                let [_, bends, crossings, texts] = self.counts(*id, r, s);
                sum = [sum[0] + bends, sum[1] + crossings, sum[2] + texts];
            }
            sum
        };
        let (new, old) = (self.rating(state), self.rating(than));
        new < old && new.0 <= old.0 && seen(state).iter().zip(seen(than)).all(|(a, b)| *a <= b)
    }

    /// The jobs of `state` whose route breaks a hard rule but (d), a text, by the layout measures: it touches a
    /// point it does not end at, lies on another wire or runs through a body. (A corner on another wire's corner,
    /// ADR-0033 amendment 3, breaks none of them: it counts as crossings in [`Self::cost`], which a route clear of
    /// it undercuts.)
    fn bad(&self, state: &State) -> BTreeSet<Uuid> {
        self.faults(state)
            .into_iter()
            .filter(|(id, f)| !f.grave.is_empty() && state.routes.contains_key(id))
            .map(|(id, _)| id)
            .collect()
    }

    /// What every routed wire of `state` breaks by the layout measures ([`Faults`]).
    fn faults(&self, state: &State) -> BTreeMap<Uuid, Faults> {
        let mut drawn = self.drawn(state);
        drawn.connections.retain(|c| c.routed);
        Faults::of(&measure(&drawn))
    }

    /// Whether the wire of `job` drawn along `new` instead of `old`, beside the other wires of `state`, breaks no hard
    /// rule that it kept along `old` ([`Faults::no_worse`]).
    fn no_worse(&self, job: Uuid, new: &[Point], old: &[Point], state: &State) -> bool {
        let along = |route: &[Point]| {
            let mut s = state.clone();
            s.routes.insert(job, (route.to_vec(), 0));
            self.faults(&s).remove(&job).unwrap_or_default()
        };
        along(new).no_worse(&along(old))
    }

    /// How well `state` draws the jobs (less is better): the jobs without a route or with one that breaks a hard
    /// rule, then what the routes cost together ([`Self::cost`]).
    fn rating(&self, state: &State) -> (usize, u64) {
        (
            self.failed(state).len() + self.bad(state).len(),
            state
                .routes
                .iter()
                .map(|(id, (r, _))| self.cost(*id, r, state))
                .sum(),
        )
    }

    /// The jobs of `state` left without a route, in routing order.
    fn failed(&self, state: &State) -> Vec<Uuid> {
        self.order
            .iter()
            .copied()
            .filter(|j| !state.routes.contains_key(j))
            .collect()
    }

    /// The score of a state: its failed jobs, then its total cost (less is better).
    fn score(&self, state: &State) -> (usize, u64) {
        (
            self.failed(state).len(),
            state.routes.values().map(|(_, c)| u64::from(*c)).sum(),
        )
    }

    /// One rip-up round (§5.5 step 3).
    fn round(&self, before: &State) -> State {
        let mut state = before.clone();
        for f in self.failed(before) {
            if state.routes.contains_key(&f) || self.hopeless.borrow().contains(&f) {
                continue;
            }
            let free = self
                .request(f, &state, true, Relax::default(), false)
                .and_then(|req| search_areas(self.page, &req));
            let Some((free, _)) = free else { continue };
            // The routed jobs the free route meets, and every routed job after the first of them in the order: they
            // were routed around it, so they are routed again too (step 6's tuning).
            let first = self
                .order
                .iter()
                .position(|j| state.routes.get(j).is_some_and(|(r, _)| conflict(&free, r)));
            let ripped: Vec<Uuid> = first
                .map(|k| {
                    self.order[k..]
                        .iter()
                        .copied()
                        .filter(|j| *j != f && state.routes.contains_key(j))
                        .collect()
                })
                .unwrap_or_default();
            for j in &ripped {
                state.routes.remove(j);
            }
            self.route(f, &mut state, true);
            for j in ripped {
                self.route(j, &mut state, true);
            }
        }
        state
    }

    /// The first rule the free route of `job` breaks (see the module documentation).
    fn reason(&self, job: Uuid, state: &State) -> Unrouted {
        if self.four_arms(job, state) {
            return Unrouted::FourArms;
        }
        let tries = [
            (
                Relax {
                    wires: true,
                    ..Relax::default()
                },
                Unrouted::Wires,
            ),
            (
                Relax {
                    wires: true,
                    texts: true,
                    ..Relax::default()
                },
                Unrouted::Texts,
            ),
            (
                Relax {
                    wires: true,
                    texts: true,
                    bodies: false,
                    points: true,
                },
                Unrouted::Points,
            ),
            (
                Relax {
                    wires: true,
                    texts: true,
                    bodies: true,
                    points: true,
                },
                Unrouted::Symbols,
            ),
        ];
        for (relax, why) in tries {
            let found = self
                .request(job, state, false, relax, false)
                .and_then(|req| search_areas(self.page, &req));
            if found.is_some() {
                return why;
            }
        }
        Unrouted::Symbols
    }
}

/// The groups of mates among `jobs` (§5.4): two connections whose ends pair up side by side (each end on one
/// horizontal or vertical line with an end of the other, at most two grid steps apart, facing the same way), closed
/// transitively. Each group in its fixed order: supply rank (none last), then the `a` end top to bottom, then left
/// to right, then the ID.
fn mate_groups(page: &PageGeometry, jobs: &[Uuid], ends: &BTreeMap<Uuid, [End; 2]>) -> Vec<Vec<Uuid>> {
    let g = step(page);
    let beside = |p: &End, q: &End| {
        let aligned = (p.at.0 == q.at.0 && (p.at.1 - q.at.1).abs() <= 2 * g)
            || (p.at.1 == q.at.1 && (p.at.0 - q.at.0).abs() <= 2 * g);
        p.at != q.at && aligned && p.dir == q.dir && p.junction == q.junction
    };
    let mates = |x: &[End; 2], y: &[End; 2]| {
        (beside(&x[0], &y[0]) && beside(&x[1], &y[1])) || (beside(&x[0], &y[1]) && beside(&x[1], &y[0]))
    };
    // Union-find by the smallest member.
    let mut root: BTreeMap<Uuid, Uuid> = jobs.iter().map(|j| (*j, *j)).collect();
    fn find(root: &BTreeMap<Uuid, Uuid>, mut x: Uuid) -> Uuid {
        while let Some(&r) = root.get(&x) {
            if r == x {
                break;
            }
            x = r;
        }
        x
    }
    for (i, x) in jobs.iter().enumerate() {
        for y in &jobs[i + 1..] {
            let (Some(ex), Some(ey)) = (ends.get(x), ends.get(y)) else {
                continue;
            };
            if mates(ex, ey) {
                let (rx, ry) = (find(&root, *x), find(&root, *y));
                let (lo, hi) = (rx.min(ry), rx.max(ry));
                root.insert(hi, lo);
            }
        }
    }
    let supply = |id: &Uuid| {
        page.connections
            .iter()
            .find(|c| c.id == *id)
            .and_then(|c| c.supply)
            .unwrap_or(u8::MAX)
    };
    let mut groups: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for j in jobs {
        groups.entry(find(&root, *j)).or_default().push(*j);
    }
    groups
        .into_values()
        .map(|mut members| {
            members.sort_by_key(|m| {
                let a = ends.get(m).map(|e| e[0].at);
                (supply(m), a.map(|p| (p.1, p.0)), *m)
            });
            members
        })
        .collect()
}

/// The page solve (see the module documentation): routes the connections `jobs` of `page` (the
/// others are fixed), deterministic for any order of the geometry's lists and of the jobs. A job whose ends are not
/// both on the page is left out; a job whose ends meet needs no route and is left out.
pub fn route_page(page: &PageGeometry, jobs: &[Uuid]) -> RoutePlan {
    // The solve reads every grid position as one number; its routes then take their ends' own numbers.
    let exact = page;
    let page = &*page.settled();
    let mut jobs: Vec<Uuid> = jobs.to_vec();
    jobs.sort_unstable();
    jobs.dedup();
    let ends: BTreeMap<Uuid, [End; 2]> = jobs
        .iter()
        .filter_map(|j| ends_of(page, *j).map(|e| (*j, e)))
        .filter(|(_, e)| e[0].at != e[1].at)
        .collect();
    let jobs: Vec<Uuid> = jobs.into_iter().filter(|j| ends.contains_key(j)).collect();
    // The fixed wires: routed connections outside the solve, in ID order.
    let mut fixed_conns: Vec<&super::DrawnConnection> = page
        .connections
        .iter()
        .filter(|c| c.routed && !jobs.contains(&c.id))
        .collect();
    fixed_conns.sort_by_key(|c| c.id);
    let fixed: Vec<[Point; 2]> = fixed_conns.iter().flat_map(|c| runs(&c.polyline)).collect();
    let fixed_lines: Vec<&[Point]> = fixed_conns.iter().map(|c| c.polyline.as_slice()).collect();
    // §5.3: class, span (falling, or rising with `short_first`), mates together, ties by the ends' points and the ID.
    let supply = |id: &Uuid| {
        page.connections
            .iter()
            .find(|c| c.id == *id)
            .and_then(|c| c.supply)
    };
    let groups = mate_groups(page, &jobs, &ends);
    let group_of: BTreeMap<Uuid, usize> = groups
        .iter()
        .enumerate()
        .flat_map(|(k, g)| g.iter().map(move |m| (*m, k)))
        .collect();
    let order_by = |short_first: bool| -> Vec<Uuid> {
        let key = |id: &Uuid| {
            let e = &ends[id];
            let span = (e[0].at.0 - e[1].at.0).abs() + (e[0].at.1 - e[1].at.1).abs();
            (
                u8::from(supply(id).is_none()),
                if short_first { span } else { -span },
                (e[0].at.1, e[0].at.0),
                (e[1].at.1, e[1].at.0),
                *id,
            )
        };
        let mut sorted = jobs.clone();
        sorted.sort_by_key(key);
        let mut order: Vec<Uuid> = Vec::with_capacity(sorted.len());
        let mut placed: BTreeSet<usize> = BTreeSet::new();
        for j in &sorted {
            let k = group_of[j];
            if placed.insert(k) {
                order.extend(groups[k].iter().copied());
            }
        }
        order
    };
    let [longest_first, shortest_first] = [order_by(false), order_by(true)];
    let mut mates: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for g in &groups {
        for (i, m) in g.iter().enumerate() {
            mates.insert(*m, g[..i].to_vec());
        }
    }
    let mut solve = Solve {
        page,
        order: longest_first,
        ends,
        fixed,
        fixed_lines,
        mates,
        hopeless: RefCell::default(),
    };
    // The drawing as it is: each job's stored route between its two ends. A reroute starts from it, so that it mends
    // what is badly drawn and never leaves the page worse than it found it.
    let mut kept = State::default();
    for c in page.connections.iter().filter(|c| c.routed) {
        let mut line = c.polyline.clone();
        line.dedup();
        if let Some(e) = solve.ends.get(&c.id)
            && line.len() >= 2
            && line.first() == Some(&e[0].at)
            && line.last() == Some(&e[1].at)
        {
            kept.routes.insert(c.id, (line, 0));
        }
    }
    // The candidates, the first of the best rating taken (ADR-0033 amendment 4). A reroute: the drawing as it is,
    // each wire then routed again where that is better; and the jobs routed from nothing, the shortest first (on a
    // full page the long wires else take the straight tracks between the pins that the short ones need). A page
    // routed for the first time: the jobs routed from nothing, the longest first (decision 5's order) and the
    // shortest first, which is taken where it is better at no price a reader sees (no more bends, crossings or
    // texts crossed).
    let state = if kept.routes.is_empty() {
        let longest = solve.construct(State::default());
        if shortest_first == solve.order {
            longest
        } else {
            let longest_first = std::mem::replace(&mut solve.order, shortest_first);
            let shortest = solve.construct(State::default());
            if solve.better_at_no_price(&shortest, &longest) {
                shortest
            } else {
                solve.order = longest_first;
                longest
            }
        }
    } else {
        solve.order = shortest_first;
        let before = solve.faults(&kept);
        let stored: Vec<Uuid> = kept.routes.keys().copied().collect();
        let drawn = solve.improve(solve.construct(kept));
        let fresh = solve.construct(State::default());
        // The jobs routed from nothing are taken only where no wire breaks a hard rule it kept as drawn (rc.5: a
        // drawing with nine crossings fewer rated better, and one of its wires ran across a text the old one kept clear
        // of, a check's warning more).
        let after = solve.faults(&fresh);
        let kept_rules = stored.iter().all(|id| {
            after
                .get(id)
                .is_some_and(|a| a.no_worse(&before.get(id).cloned().unwrap_or_default()))
        });
        if kept_rules && solve.rating(&fresh) < solve.rating(&drawn) {
            fresh
        } else {
            drawn
        }
    };
    let over_texts: Vec<Uuid> = state
        .routes
        .iter()
        .filter(|(_, (r, _))| crosses_text(page, r))
        .map(|(id, _)| *id)
        .collect();
    let unrouted = solve
        .failed(&state)
        .into_iter()
        .map(|j| (j, solve.reason(j, &state)))
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();
    RoutePlan {
        routes: state
            .routes
            .into_iter()
            .map(|(id, (r, _))| match ends_of(exact, id) {
                Some([a, b]) => (id, onto_ends(page, r, a.at, b.at)),
                None => (id, r),
            })
            .collect(),
        unrouted,
        over_texts,
    }
}

/// `route`, found on the settled page ([`PageGeometry::settled`]), with its ends on the points `a` and `b` as the
/// page stores them: the corner next to an end takes that end's coordinate, and a straight route between two numbers
/// of one grid line its jog ([`fit`]). Unchanged where the ends are the settled ones.
fn onto_ends(page: &PageGeometry, mut route: Vec<Point>, a: Point, b: Point) -> Vec<Point> {
    let n = route.len();
    if n < 2 || (route[0] == a && route[n - 1] == b) {
        return route;
    }
    let vertical = (route[0].0 == route[1].0, route[n - 2].0 == route[n - 1].0);
    (route[0], route[n - 1]) = (a, b);
    let grid = ew_units::Grid::new(
        u64::try_from(page.grid.0).unwrap_or(0),
        u64::try_from(page.grid.1).unwrap_or(0),
    );
    match grid {
        Ok(grid) => fit(route.clone(), vertical, grid).unwrap_or(route),
        Err(_) => route,
    }
}

/// One route from `from` to `to` against `page` as drawn (the Wire tool's ghost): every routed
/// connection a fixed wire, every other CP with its stub and every junction not at an end reserved, the ends'
/// own representations' rectangles left along their escape corridors, the areas of §5.5, a text crossed at its cost
/// (ADR-0033 amendment 2: [`crosses_text`] tells); `arrive` the heading a free first end was arrived at with. `None`
/// when no route keeps every hard rule.
pub fn route_one(
    page: &PageGeometry,
    from: OneEnd,
    to: OneEnd,
    arrive: Option<Heading>,
) -> Option<Vec<Point>> {
    // As the page solve: routed on the settled page, the route then on the ends' own numbers.
    let exact = (from.at, to.at);
    let page = &*page.settled();
    let settle = |e: OneEnd| OneEnd {
        at: page.settle(e.at),
        ..e
    };
    let (from, to) = (settle(from), settle(to));
    let wires = routed_wires(page);
    let make = |half| {
        let mut req = request(
            page,
            (&from, &to),
            (&[], &[from.at, to.at]),
            &wires,
            &[],
            Relax::default(),
            half,
        );
        req.arrive = arrive;
        Some(req)
    };
    // The route as a connection between the ends' CPs or junctions, so that the measures judge it.
    let drawn = || {
        let id = Uuid::nil();
        let end = |e: &OneEnd| {
            page.cps
                .iter()
                .find(|c| c.at == e.at && Some(c.rep) == e.rep)
                .map(|c| c.id)
                .or_else(|| page.junctions.iter().find(|j| j.at == e.at).map(|j| j.id))
                .unwrap_or_else(Uuid::nil)
        };
        let ends = [end(&from), end(&to)];
        let mut drawn = page.clone();
        for c in &mut drawn.cps {
            if ends.contains(&c.id) {
                c.connections.push(id);
            }
        }
        for j in &mut drawn.junctions {
            if ends.contains(&j.id) {
                j.connections.push(id);
            }
        }
        drawn.connections.push(super::DrawnConnection {
            id,
            ends,
            routed: true,
            supply: None,
            polyline: Vec::new(),
        });
        (drawn, id)
    };
    search_checked(page, make, drawn).map(|(points, _)| onto_ends(page, points, exact.0, exact.1))
}

/// Whether `route` as the polyline of connection `id` of `page` breaks no hard rule but (d), a text: no touch, no
/// overlap, no stretch through a body (the routes across the texts are judged so, since the search's corridors alone
/// do not keep (c) once the texts no longer shape them).
fn keeps_all_but_texts(page: &PageGeometry, id: Uuid, route: &[Point]) -> bool {
    let mut drawn = page.clone();
    for c in &mut drawn.connections {
        if c.id == id {
            c.polyline = route.to_vec();
            c.routed = true;
        }
    }
    drawn.connections.retain(|c| c.routed);
    let m = measure(&drawn);
    !m.touches.iter().any(|t| t.connection == id)
        && !m.overlaps.iter().any(|o| o.connections.contains(&id))
        && !m.through_bodies.iter().any(|t| t.connection == id)
}

/// The segments of the page's routed connections, in ID order: [`route_one`]'s other wires.
fn routed_wires(page: &PageGeometry) -> Vec<[Point; 2]> {
    let mut lines: Vec<&super::DrawnConnection> = page.connections.iter().filter(|c| c.routed).collect();
    lines.sort_by_key(|c| c.id);
    lines.iter().flat_map(|c| runs(&c.polyline)).collect()
}

/// Whether `points` is a route [`route_one`] could take from `from` to `to` on `page` (`search::clear` on its
/// request), with the representations `through` left out: their bodies, texts and connection points. The Wire tool
/// asks it of the direct route through a free terminal in line.
pub fn clear_one(
    page: &PageGeometry,
    (from, to): (OneEnd, OneEnd),
    arrive: Option<Heading>,
    through: &[Uuid],
    points: &[Point],
) -> bool {
    let mut page = page.settled().into_owned();
    let settle = |e: OneEnd| OneEnd {
        at: page.settle(e.at),
        ..e
    };
    let (from, to) = (settle(from), settle(to));
    let points: Vec<Point> = points.iter().map(|p| page.settle(*p)).collect();
    let points = points.as_slice();
    page.bodies.retain(|b| !through.contains(&b.rep));
    page.texts.retain(|t| !through.contains(&t.rep));
    page.cps.retain(|c| !through.contains(&c.rep));
    let mut req = request(
        &page,
        (&from, &to),
        (&[], &[from.at, to.at]),
        &routed_wires(&page),
        &[],
        Relax::default(),
        false,
    );
    req.arrive = arrive;
    // The direct route through a terminal keeps clear of the texts as before.
    req.text_cost = None;
    let g = step(&page);
    let (xs, ys) = (points.iter().map(|p| p.0), points.iter().map(|p| p.1));
    let (Some(x0), Some(x1), Some(y0), Some(y1)) = (xs.clone().min(), xs.max(), ys.clone().min(), ys.max())
    else {
        return false;
    };
    req.area = [x0 - g, y0 - g, x1 + g, y1 + g];
    super::search::clear(&req, points)
}
