// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page solve of WP-S2.19 §5 ([`route_page`]) and one route against a page as drawn ([`route_one`]): the search
//! of [`super::route`] fed with a page's geometry under the hard rules of §5.1, the costs of §5.2, the order of §5.3,
//! the mates of §5.4 and the areas and rip-up rounds of §5.5, deterministic (§5.6).
//!
//! **One job's request.** The ends are the connection's CPs (with their leaving directions) or junctions. Every
//! representation's body and black-box outline grown by one grid step, and every text box grown by a quarter step,
//! block; the ends' own representations' bodies block but for the escape corridors, and their texts block as any
//! other text (rule 4 has no exemption for a wire's own texts; ID-1502). The frame's bands block,
//! the drawing area's edge grown inward by one step but on a side where an end or its stub stands within that step
//! (FT-758). The routed connections that are not re-routed, and the jobs routed so far, are the other wires (an
//! unrouted connection outside the solve is no obstacle). Every other connection point and its stub, and every
//! junction the job does not end at, is reserved. A crossing costs [`CROSSING_COST`]; a step beside a routed mate of
//! the job's group costs [`MATE_STEP_COST`](super::MATE_STEP_COST).
//!
//! **Four arms.** A job ending at a junction that the other routed connections already leave in three directions is
//! not routed (its route would be the fourth arm, rule 7).
//!
//! **Areas.** The ends' bounding box grown by 4 grid steps, then by 16, then the whole drawing area (the sheet on a
//! page without a frame grid), each clipped to it; the first area with a route wins.
//!
//! **Reasons.** A job left unrouted names the first rule its free route (against the fixed obstacles alone) breaks:
//! the search is tried again without the other wires, then also without the texts, then also without the reserved
//! points, then also without the bodies and the frame; the first relaxation that finds a route names the rule. With
//! none, the reason is the symbols.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;
use ew_schema::Dir4;

use super::measures::{meet, overlap};
use super::page::{PageGeometry, Point, Rect};
use super::search::{CROSSING_COST, Heading, RouteEnd, RouteRequest, route};

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

/// The frame's obstacles (FT-758): the sheet outside the drawing area grown inward by `g`, but on a side where one
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

/// The request of a route from `a` to `b` on `page` against the `wires` and with the `mates`' segments.
fn request(
    page: &PageGeometry,
    (a, b): (&OneEnd, &OneEnd),
    skip: (&[Uuid], &[Point]),
    wires: &[[Point; 2]],
    mates: &[[Point; 2]],
    relax: Relax,
) -> RouteRequest {
    let (skip_points, skip_at) = skip;
    let g = step(page);
    let stub = |e: &OneEnd| {
        e.dir.map_or(e.at, |d| {
            let v = d.step();
            (e.at.0 + v.0 * g, e.at.1 + v.1 * g)
        })
    };
    let mut obstacles: Vec<Rect> = Vec::new();
    let mut own: [Vec<Rect>; 2] = [Vec::new(), Vec::new()];
    let place = |rep: Uuid, r: Rect, obstacles: &mut Vec<Rect>, own: &mut [Vec<Rect>; 2]| {
        let mut mine = false;
        for (k, e) in [a, b].into_iter().enumerate() {
            if e.rep == Some(rep) {
                own[k].push(r);
                mine = true;
            }
        }
        if !mine {
            obstacles.push(r);
        }
    };
    if !relax.bodies {
        for body in &page.bodies {
            place(body.rep, grow(body.rect, g), &mut obstacles, &mut own);
        }
        obstacles.extend(frame_rects(page, g, &[a.at, stub(a), b.at, stub(b)]));
    }
    // Rule 4 (ID-1502): every text blocks, the ends' own included; the escape corridors pass bodies only.
    if !relax.texts {
        obstacles.extend(page.texts.iter().map(|t| grow(t.rect, g / 4)));
    }
    let mut reserved: Vec<Point> = Vec::new();
    if !relax.points {
        for cp in &page.cps {
            if skip_points.contains(&cp.id) || skip_at.contains(&cp.at) {
                continue;
            }
            reserved.push(cp.at);
            if let Some(d) = cp.dir.as_ref().and_then(heading) {
                let v = d.step();
                reserved.push((cp.at.0 + v.0 * g, cp.at.1 + v.1 * g));
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
        grid: page.grid,
        area: whole(page),
        arrive: None,
        own,
        wires: if relax.wires { Vec::new() } else { wires.to_vec() },
        reserved,
        cross_cost: CROSSING_COST,
        mates: mates.to_vec(),
    }
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
    for area in [around(4), around(16), w] {
        let r = route(&RouteRequest { area, ..req.clone() });
        if !r.direct {
            return Some((r.points, r.cost));
        }
    }
    None
}

/// The segments of a polyline, without empty ones.
fn segments(line: &[Point]) -> impl Iterator<Item = [Point; 2]> + '_ {
    line.windows(2).filter(|w| w[0] != w[1]).map(|w| [w[0], w[1]])
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

    /// The request of `job` against the fixed wires and the routes of `state` (none with `alone`).
    fn request(&self, job: Uuid, state: &State, alone: bool, relax: Relax) -> Option<RouteRequest> {
        let [a, b] = self.ends.get(&job)?;
        let mut wires = self.fixed.clone();
        if !alone {
            for (id, (r, _)) in &state.routes {
                if *id != job {
                    wires.extend(segments(r));
                }
            }
        }
        let mates: Vec<[Point; 2]> = self
            .mates
            .get(&job)
            .into_iter()
            .flatten()
            .filter_map(|m| state.routes.get(m))
            .flat_map(|(r, _)| segments(r).collect::<Vec<_>>())
            .collect();
        Some(request(
            self.page,
            (&Self::one(a), &Self::one(b)),
            (&[a.id, b.id], &[a.at, b.at]),
            &wires,
            &mates,
            relax,
        ))
    }

    /// Routes `job` in `state`; whether it found a route.
    fn route(&self, job: Uuid, state: &mut State) -> bool {
        if self.four_arms(job, state) {
            return false;
        }
        let found = self
            .request(job, state, false, Relax::default())
            .and_then(|req| search_areas(self.page, &req));
        match found {
            Some(r) => {
                state.routes.insert(job, r);
                true
            }
            None => false,
        }
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
            if state.routes.contains_key(&f) {
                continue;
            }
            let free = self
                .request(f, &state, true, Relax::default())
                .and_then(|req| search_areas(self.page, &req));
            let Some((free, _)) = free else { continue };
            // The routed jobs the free route meets, and every routed job after the first of them in the order: they
            // were routed around it, so they are routed again too (ID-1502, step 6's tuning).
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
            self.route(f, &mut state);
            for j in ripped {
                self.route(j, &mut state);
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
                .request(job, state, false, relax)
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

/// The page solve of WP-S2.19 §5 (see the module documentation): routes the connections `jobs` of `page` (the
/// others are fixed), deterministic for any order of the geometry's lists and of the jobs. A job whose ends are not
/// both on the page is left out; a job whose ends meet needs no route and is left out.
pub fn route_page(page: &PageGeometry, jobs: &[Uuid]) -> RoutePlan {
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
    let fixed: Vec<[Point; 2]> = fixed_conns
        .iter()
        .flat_map(|c| segments(&c.polyline).collect::<Vec<_>>())
        .collect();
    let fixed_lines: Vec<&[Point]> = fixed_conns.iter().map(|c| c.polyline.as_slice()).collect();
    // §5.3: class, span falling, mates together, ties by the ends' points and the ID.
    let supply = |id: &Uuid| {
        page.connections
            .iter()
            .find(|c| c.id == *id)
            .and_then(|c| c.supply)
    };
    let key = |id: &Uuid| {
        let e = &ends[id];
        let span = (e[0].at.0 - e[1].at.0).abs() + (e[0].at.1 - e[1].at.1).abs();
        (
            u8::from(supply(id).is_none()),
            std::cmp::Reverse(span),
            (e[0].at.1, e[0].at.0),
            (e[1].at.1, e[1].at.0),
            *id,
        )
    };
    let mut sorted = jobs.clone();
    sorted.sort_by_key(key);
    let groups = mate_groups(page, &jobs, &ends);
    let group_of: BTreeMap<Uuid, usize> = groups
        .iter()
        .enumerate()
        .flat_map(|(k, g)| g.iter().map(move |m| (*m, k)))
        .collect();
    let mut order: Vec<Uuid> = Vec::with_capacity(sorted.len());
    let mut placed: BTreeSet<usize> = BTreeSet::new();
    for j in &sorted {
        let k = group_of[j];
        if placed.insert(k) {
            order.extend(groups[k].iter().copied());
        }
    }
    let mut mates: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
    for g in &groups {
        for (i, m) in g.iter().enumerate() {
            mates.insert(*m, g[..i].to_vec());
        }
    }
    let solve = Solve {
        page,
        order,
        ends,
        fixed,
        fixed_lines,
        mates,
    };
    // The first pass, then at most two rounds, each kept only when it scores better.
    let mut state = State::default();
    for j in solve.order.clone() {
        solve.route(j, &mut state);
    }
    for _ in 0..2 {
        if solve.failed(&state).is_empty() {
            break;
        }
        let next = solve.round(&state);
        if solve.score(&next) < solve.score(&state) {
            state = next;
        } else {
            break;
        }
    }
    let unrouted = solve
        .failed(&state)
        .into_iter()
        .map(|j| (j, solve.reason(j, &state)))
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();
    RoutePlan {
        routes: state.routes.into_iter().map(|(id, (r, _))| (id, r)).collect(),
        unrouted,
    }
}

/// One route from `from` to `to` against `page` as drawn (WP-S2.19 D4, the Wire tool's ghost): every routed
/// connection a fixed wire, every other CP with its stub and every junction not at an end reserved, the ends'
/// own representations' rectangles left along their escape corridors, the areas of §5.5; `arrive` the heading a
/// free first end was arrived at with (FT-437). `None` when no route keeps every hard rule.
pub fn route_one(
    page: &PageGeometry,
    from: OneEnd,
    to: OneEnd,
    arrive: Option<Heading>,
) -> Option<Vec<Point>> {
    let mut req = request(
        page,
        (&from, &to),
        (&[], &[from.at, to.at]),
        &routed_wires(page),
        &[],
        Relax::default(),
    );
    req.arrive = arrive;
    search_areas(page, &req).map(|(points, _)| points)
}

/// The segments of the page's routed connections, in ID order: [`route_one`]'s other wires.
fn routed_wires(page: &PageGeometry) -> Vec<[Point; 2]> {
    let mut lines: Vec<&super::DrawnConnection> = page.connections.iter().filter(|c| c.routed).collect();
    lines.sort_by_key(|c| c.id);
    lines
        .iter()
        .flat_map(|c| segments(&c.polyline).collect::<Vec<_>>())
        .collect()
}

/// Whether `points` is a route [`route_one`] could take from `from` to `to` on `page` (`search::clear` on its
/// request), with the representations `through` left out: their bodies, texts and connection points. The Wire tool
/// asks it of the direct route through a free terminal in line (FT-705).
pub fn clear_one(
    page: &PageGeometry,
    (from, to): (OneEnd, OneEnd),
    arrive: Option<Heading>,
    through: &[Uuid],
    points: &[Point],
) -> bool {
    let mut page = page.clone();
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
    );
    req.arrive = arrive;
    let g = step(&page);
    let (xs, ys) = (points.iter().map(|p| p.0), points.iter().map(|p| p.1));
    let (Some(x0), Some(x1), Some(y0), Some(y1)) = (xs.clone().min(), xs.max(), ys.clone().min(), ys.max())
    else {
        return false;
    };
    req.area = [x0 - g, y0 - g, x1 + g, y1 + g];
    super::search::clear(&req, points)
}
