// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The router as the window, the CLI and the tools reach it (docs/design/06a-drafting.md §2.7, §7): [`Session::route_plan`]
//! plans Route on a page or a selection with `ew_drafting::route` on the page as the layout sees it
//! ([`Session::page_draft`]), [`Session::arrange_plan`] Arrange and route with `ew_drafting::arrange`, and each gives
//! its plan as the parameters of the one `layout.apply` that stores it;
//! [`route_wire`] and [`wire_clear`] are the Wire tool's route between two ends, in page nm; [`unrouted_line`] the line
//! of a wire without a route.
//!
//! **Back from ticks** (doc 06a §2.1): a vertex on the grid takes the grid's own number for its position; a route's first and
//! last vertex are its ends' stored points (a junction's new place where the plan moves it), and the corner next to an
//! end takes that end's coordinate across the segment between them, so a route meets a pin that lies 1 nm off the grid.

pub use ew_drafting::Pt;
pub use ew_drafting::consts::TICKS_PER_STEP;
pub use ew_drafting::{
    DraftPage, InTheWay, Jobs, PartId, Plan, Quality, Reason, Side, StripHeld, WireId, measure,
    route as route_on,
};
use ew_drafting::{JunctionId, PinId, WireEnd};
use ew_ids::{Id, Uuid};
/// The conversion of a page between nm and the layout's ticks, which the window's tools use too.
pub use ew_scene::ticks::Ticks;
use ew_units::Grid;
use serde_json::{Value, json};

use crate::Session;
use crate::geometry::Dir;
use crate::output::{PageTables, TableEnd};

/// What a Route plans.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RouteScope {
    /// The page: its unrouted wires, those with a fault as stored, and those the program routed (doc 06a §3.2).
    Page,
    /// The selection: the wires named, and every wire of the page with an end on a part named, whoever shaped them.
    Selection(Vec<Uuid>),
}

/// A plan of Route or of Arrange and route on one page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RoutePlan {
    /// The plan, in the layout's ticks.
    pub plan: Plan,
    /// The symbols Arrange and route moves, each by its move in ticks (whole grid steps); none for Route.
    pub moves: Vec<(PartId, Pt)>,
    /// The symbols Arrange and route leaves where they stand against a rule, each with the side of its host's pin
    /// where its place is and what was in the way of that place; none for Route.
    pub left: Vec<(PartId, Side, InTheWay)>,
    /// The parameters of the `layout.apply` that stores it; `None` for a plan that changes nothing.
    pub params: Option<Value>,
    /// The page's routed wires a page-wide Route leaves alone as the person's (no `computed` mark, no fault), by ID;
    /// empty for a selection.
    pub kept: Vec<Uuid>,
    /// The strips of terminals over or under a card that Arrange and route could not open as one, each by its first
    /// terminal, with what held it; none for Route.
    pub held: Vec<(PartId, StripHeld)>,
}

impl RoutePlan {
    /// The wires whose routes the plan changes, by ID.
    pub fn changed(&self) -> Vec<Uuid> {
        self.plan.routes.iter().map(|(w, _)| w.0).collect()
    }
}

/// A route in ticks as page nm between the ends' stored points `a` and `b` (module documentation).
pub fn route_nm(ticks: Ticks, route: &[Pt], a: (i64, i64), b: (i64, i64)) -> Vec<(i64, i64)> {
    let mut pts: Vec<(i64, i64)> = route.iter().map(|p| (ticks.nm(p.x), ticks.nm(p.y))).collect();
    let n = pts.len();
    if n == 0 {
        return pts;
    }
    let vertical = |p: Pt, q: Pt| p.x == q.x;
    if n >= 3 {
        if vertical(route[0], route[1]) {
            pts[1].0 = a.0;
        } else {
            pts[1].1 = a.1;
        }
        if vertical(route[n - 2], route[n - 1]) {
            pts[n - 2].0 = b.0;
        } else {
            pts[n - 2].1 = b.1;
        }
    }
    pts[0] = a;
    pts[n - 1] = b;
    if n == 2 && a.0 != b.0 && a.1 != b.1 {
        // A straight route between two numbers of one grid line: a jog at the grid line nearest its middle.
        let mid = if vertical(route[0], route[1]) {
            let y = ticks.nm(ew_drafting::jog_line(route[0].y, route[1].y));
            vec![(a.0, y), (b.0, y)]
        } else {
            let x = ticks.nm(ew_drafting::jog_line(route[0].x, route[1].x));
            vec![(x, a.1), (x, b.1)]
        };
        pts.splice(1..1, mid);
    }
    pts.dedup();
    pts
}

/// The stored point of a wire's end on the page of `tables`, with the junctions at `moved` places and the connection
/// points of the representations of `shifted` where those stand after their moves (grid steps and page nm).
fn end_point(
    tables: &PageTables,
    e: &TableEnd,
    moved: &[(Uuid, (i64, i64))],
    shifted: &[(Uuid, [i64; 2], (i64, i64))],
) -> Option<(i64, i64)> {
    match e {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| match shifted.iter().find(|(r, _, _)| *r == c.rep) {
                Some((_, steps, d)) => {
                    let grid = Ticks::new(tables.grid).grid();
                    (
                        moved_coordinate(grid, c.at.0, steps[0], d.0),
                        moved_coordinate(grid, c.at.1, steps[1], d.1),
                    )
                }
                None => c.at,
            }),
        TableEnd::Junction { id } => moved
            .iter()
            .find(|(j, _)| j == id)
            .map(|(_, p)| *p)
            .or_else(|| tables.junctions.iter().find(|j| j.id == *id).map(|j| j.at)),
    }
}

/// A coordinate `x` of a connection point after its symbol moved by `k` grid steps, `by` nm: where it is a number of
/// the grid, the grid's own number `k` positions on, as the moved symbol then draws it (a connection point on a grid
/// position has that position's number, `Grid::settle_point`; on a grid whose step is no whole number of nm the
/// coordinate plus the symbol's move can lie 1 nm beside it); else the coordinate moved by `by`.
fn moved_coordinate(grid: Grid, x: i64, k: i64, by: i64) -> i64 {
    use ew_units::Nm;
    let own = grid
        .snap_point(Nm(x))
        .filter(|p| p.0 == x)
        .and_then(|p| grid.at(grid.snap(p).checked_add(k)?));
    own.map_or(grid.settle(Nm(x + by)).0, |p| p.0)
}

/// The move in page nm of a representation at `pos` moved by `steps` grid steps, as `rep.move` and `layout.apply` move
/// it: its position at its grid index plus the steps; `None` for a position off the grid.
fn shift_of(grid: Grid, pos: (i64, i64), steps: [i64; 2]) -> Option<(i64, i64)> {
    use ew_units::Nm;
    let one = |x: i64, k: i64| {
        let at = grid.snap_point(Nm(x)).filter(|p| p.0 == x)?;
        Some(grid.at(grid.snap(at).checked_add(k)?)?.0 - x)
    };
    Some((one(pos.0, steps[0])?, one(pos.1, steps[1])?))
}

/// The representations of `tables` that `moves` moves, each with its grid steps and its move in page nm.
fn part_moves(tables: &PageTables, moves: &[(PartId, Pt)]) -> Vec<(Uuid, [i64; 2], (i64, i64))> {
    let grid = Ticks::new(tables.grid).grid();
    moves
        .iter()
        .filter_map(|(p, d)| {
            let rep = tables.reps.iter().find(|r| r.id == p.0)?;
            let steps = [i64::from(d.x / TICKS_PER_STEP), i64::from(d.y / TICKS_PER_STEP)];
            Some((p.0, steps, shift_of(grid, rep.pos, steps)?))
        })
        .collect()
}

/// The routes of `plan` in page nm, by connection: each between its ends' stored points, those of the symbols that
/// `moves` moves where they go.
pub fn plan_routes(tables: &PageTables, plan: &Plan, moves: &[(PartId, Pt)]) -> Vec<(Uuid, Vec<(i64, i64)>)> {
    let ticks = Ticks::new(tables.grid);
    let shifted = part_moves(tables, moves);
    let moved: Vec<(Uuid, (i64, i64))> = plan
        .junctions
        .iter()
        .map(|(j, p)| (j.0, (ticks.nm(p.x), ticks.nm(p.y))))
        .collect();
    routes_nm(tables, plan, |e| end_point(tables, e, &moved, &shifted))
}

/// The routes of `plan` in page nm, by connection, each between the points `ends` gives its ends.
fn routes_nm(
    tables: &PageTables,
    plan: &Plan,
    ends: impl Fn(&TableEnd) -> Option<(i64, i64)>,
) -> Vec<(Uuid, Vec<(i64, i64)>)> {
    let ticks = Ticks::new(tables.grid);
    plan.routes
        .iter()
        .filter_map(|(w, route)| {
            let c = tables.connections.iter().find(|c| c.id == w.0)?;
            Some((w.0, route_nm(ticks, route, ends(&c.a)?, ends(&c.b)?)))
        })
        .collect()
}

/// A move's re-solve (doc 06a §7.4) on the page of `tables`, `before` the page as the layout sees it: the symbols of
/// `moves` displaced by their move in page nm, each connection of `followed` along the route it follows the move by
/// (page nm; unrouted where it has no stored route), then [`ew_drafting::resolve_move`]. The connections it lays
/// again, with their routes in page nm between the points `ends` gives their ends after the move.
pub fn move_routes(
    tables: &PageTables,
    before: &DraftPage,
    moves: &[(Uuid, (i64, i64))],
    followed: &[(Uuid, Vec<(i64, i64)>)],
    ends: impl Fn(&TableEnd) -> Option<(i64, i64)>,
) -> Vec<(Uuid, Vec<(i64, i64)>)> {
    let ticks = Ticks::new(tables.grid);
    let mut after = before.clone();
    // The preview moves a part's place, its pins with it; its fixed texts stay where they stood (the placer's
    // `moved` moves them too): the form the move's re-solve has always judged its jobs on.
    for p in &mut after.parts {
        if let Some((_, d)) = moves.iter().find(|(rep, _)| *rep == p.id.0) {
            p.at = p.at.plus(ticks.pt(*d));
        }
    }
    for (id, route) in followed {
        let stored = tables.connections.iter().any(|c| c.id == *id && c.stored_route);
        if let Some(w) = after.wires.iter_mut().find(|w| w.id.0 == *id) {
            w.route = if stored {
                route.iter().map(|p| ticks.pt(*p)).collect()
            } else {
                Vec::new()
            };
        }
    }
    let affected: Vec<WireId> = followed.iter().map(|(id, _)| WireId(*id)).collect();
    let plan = ew_drafting::resolve_move(before, &after, &affected);
    routes_nm(tables, &plan, ends)
}

/// The `layout.apply` that stores `plan` with the symbol moves `moves` on the page of `tables`; `None` for a plan that
/// changes nothing. A marker's anchor is read from `marker_at`.
pub fn layout_apply_params(
    tables: &PageTables,
    plan: &Plan,
    moves: &[(PartId, Pt)],
    marker_at: &dyn Fn(Uuid) -> Option<(i64, i64)>,
) -> Option<Value> {
    if plan.is_empty() && moves.is_empty() {
        return None;
    }
    let ticks = Ticks::new(tables.grid);
    let parts: Vec<Value> = part_moves(tables, moves)
        .into_iter()
        .map(|(r, steps, _)| json!({"rep": r.to_string(), "steps": steps}))
        .collect();
    let junctions: Vec<Value> = plan
        .junctions
        .iter()
        .map(|(j, p)| json!({"junction": j.0.to_string(), "at": [ticks.nm(p.x), ticks.nm(p.y)]}))
        .collect();
    let routes: Vec<Value> = plan_routes(tables, plan, moves)
        .into_iter()
        .map(|(id, route)| {
            let points: Vec<Value> = route.iter().map(|p| json!([p.0, p.1])).collect();
            json!({"connection": id.to_string(), "route": points})
        })
        .collect();
    let markers: Vec<Value> = plan
        .marks
        .iter()
        .filter_map(|(m, d)| {
            let at = marker_at(m.0)?;
            Some(json!({"item": m.0.to_string(), "at": [at.0 + ticks.nm(d.x), at.1 + ticks.nm(d.y)]}))
        })
        .collect();
    let mut params = json!({"page": tables.page.to_string()});
    if !parts.is_empty() {
        params["parts"] = Value::Array(parts);
    }
    if !junctions.is_empty() {
        params["junctions"] = Value::Array(junctions);
    }
    if !routes.is_empty() {
        params["routes"] = Value::Array(routes);
    }
    if !markers.is_empty() {
        params["markers"] = Value::Array(markers);
    }
    Some(params)
}

impl Session {
    /// The plan of Route on `scope` of the page labelled `folio` (docs/design/06a-drafting.md §3, §7.2), with the
    /// `layout.apply` that stores it; `None` for an unknown folio or a page that is not drawn.
    pub fn route_plan(&self, folio: &str, scope: &RouteScope) -> Option<RoutePlan> {
        let draft = self.page_draft(folio)?;
        let tables = self.page_tables(folio)?;
        let jobs = match scope {
            RouteScope::Page => ew_drafting::jobs(&draft, ew_drafting::Scope::Page),
            RouteScope::Selection(ids) => ew_drafting::jobs(&draft, ew_drafting::Scope::Selection(ids)),
        };
        let plan = ew_drafting::route(&draft, &jobs);
        let params = self.plan_params(&tables, &plan, &[]);
        let kept = match scope {
            RouteScope::Page => draft
                .wires
                .iter()
                .filter(|w| !w.route.is_empty() && !jobs.wires.contains(&w.id))
                .map(|w| w.id.0)
                .collect(),
            RouteScope::Selection(_) => Vec::new(),
        };
        Some(RoutePlan {
            plan,
            moves: Vec::new(),
            left: Vec::new(),
            params,
            kept,
            held: Vec::new(),
        })
    }

    /// The plan of Arrange and route on `scope` of the page labelled `folio` (docs/design/06a-drafting.md §4, §7.3),
    /// with the symbols and the wires of `fixed` kept as they stand (the rows unticked in the preview), with the
    /// `layout.apply` that stores it; `None` for an unknown folio or a page that is not drawn. A symbol whose position
    /// is not exactly a point of the page's grid stays where it stands (a move by grid steps counts from it).
    pub fn arrange_plan(&self, folio: &str, scope: &RouteScope, fixed: &[Uuid]) -> Option<RoutePlan> {
        let draft = self.page_draft(folio)?;
        let tables = self.page_tables(folio)?;
        let grid = Ticks::new(tables.grid).grid();
        let mut kept: Vec<Uuid> = fixed.to_vec();
        kept.extend(
            tables
                .reps
                .iter()
                .filter(|r| shift_of(grid, r.pos, [0, 0]).is_none())
                .map(|r| r.id),
        );
        let arranged = match scope {
            RouteScope::Page => ew_drafting::arrange(&draft, ew_drafting::Scope::Page, &kept),
            RouteScope::Selection(ids) => {
                ew_drafting::arrange(&draft, ew_drafting::Scope::Selection(ids), &kept)
            }
        };
        let params = self.plan_params(&tables, &arranged.plan, &arranged.moves);
        Some(RoutePlan {
            plan: arranged.plan,
            moves: arranged.moves,
            left: arranged.left,
            params,
            kept: Vec::new(),
            held: arranged.held,
        })
    }

    /// A symbol's name in words, for a plan's rows: its device's printed tag, an interruption point's name; `None` for
    /// one without either.
    pub fn symbol_name(&self, id: Uuid) -> Option<String> {
        match self.selection_words(id)? {
            crate::output::SelectionWords::Device { tag, .. } => Some(tag).filter(|t| !t.is_empty()),
            crate::output::SelectionWords::InterruptionPoint { name, .. } => {
                Some(name).filter(|n| !n.is_empty())
            }
            _ => None,
        }
    }

    /// The `layout.apply` of `plan` with the symbol moves `moves` on the page of `tables` ([`layout_apply_params`]),
    /// the markers' anchors read from the project.
    pub fn plan_params(&self, tables: &PageTables, plan: &Plan, moves: &[(PartId, Pt)]) -> Option<Value> {
        let q = self.query();
        let items = q.page_items();
        let marker_at = |id: Uuid| match &items.get(Id::from_uuid(id))?.kind {
            ew_schema::PageItemKind::WireMarker(m) => Some((m.at.x.0, m.at.y.0)),
            _ => None,
        };
        layout_apply_params(tables, plan, moves, &marker_at)
    }
}

/// The side of a tool's direction.
pub fn side(d: Dir) -> Side {
    match d {
        Dir::Up => Side::Up,
        Dir::Down => Side::Down,
        Dir::Left => Side::Left,
        Dir::Right => Side::Right,
    }
}

/// An end of the Wire tool's route, in page nm.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolEnd {
    /// Connection point `idx` of `function`.
    Cp {
        /// The function.
        function: Uuid,
        /// The CP index.
        idx: u16,
    },
    /// A junction.
    Junction(Uuid),
    /// A point on a connection, where a junction will be made.
    OnWire(Uuid, (i64, i64)),
    /// A free point (a fixed bend).
    Point((i64, i64)),
}

fn wire_end(ticks: Ticks, e: ToolEnd) -> WireEnd {
    match e {
        ToolEnd::Cp { function, idx } => {
            let cp: Id<()> =
                ew_ids::v5::cp_id(Id::<ew_schema::Function>::from_uuid(function), u32::from(idx));
            WireEnd::Pin(PinId(cp.as_uuid()))
        }
        ToolEnd::Junction(j) => WireEnd::Junction(JunctionId(j)),
        ToolEnd::OnWire(w, p) => WireEnd::OnWire(WireId(w), ticks.pt(p)),
        ToolEnd::Point(p) => WireEnd::Point(ticks.pt(p)),
    }
}

/// The Wire tool's route from `from` (at the page point `a`) to `to` (at `b`) on `draft`, a page of grid `grid`
/// (doc 06a §7.4): `ew_drafting::route_wire` back in page nm between the two points, and whether it runs across a text that
/// cannot step aside; `before` the wire's route so far (page nm), ending at a free first end (its fixed bends), which
/// the route goes on from and does not run back over. `None` where no route keeps free of faults.
pub fn route_wire(
    draft: &DraftPage,
    grid: (i64, i64),
    (from, a): (ToolEnd, (i64, i64)),
    (to, b): (ToolEnd, (i64, i64)),
    before: &[(i64, i64)],
) -> Option<(Vec<(i64, i64)>, bool)> {
    let ticks = Ticks::new(grid);
    let before: Vec<Pt> = before.iter().map(|p| ticks.pt(*p)).collect();
    let found = ew_drafting::route_wire(draft, wire_end(ticks, from), wire_end(ticks, to), &before)?;
    Some((route_nm(ticks, &found.route, a, b), found.over_text))
}

/// Whether `route` (page nm) from `from` to `to` is one the Wire tool's search could take on `draft` with the parts
/// `through` left out: the direct route through a free terminal in line.
pub fn wire_clear(
    draft: &DraftPage,
    grid: (i64, i64),
    (from, to): (ToolEnd, ToolEnd),
    arrive: Option<Dir>,
    through: &[Uuid],
    route: &[(i64, i64)],
) -> bool {
    let ticks = Ticks::new(grid);
    let route: Vec<Pt> = route.iter().map(|p| ticks.pt(*p)).collect();
    let through: Vec<ew_drafting::PartId> = through.iter().map(|p| ew_drafting::PartId(*p)).collect();
    ew_drafting::wire_clear(
        draft,
        (wire_end(ticks, from), wire_end(ticks, to)),
        arrive.map(side),
        &through,
        &route,
    )
}

/// The line of a wire without a route from `a` (leaving along `a_dir`) to `b` (`ew_drafting::unrouted_line`).
pub fn unrouted_line(
    (a, a_dir): ((i64, i64), Option<Dir>),
    (b, b_dir): ((i64, i64), Option<Dir>),
) -> Vec<(i64, i64)> {
    ew_drafting::unrouted_line((a, a_dir.map(side)), (b, b_dir.map(side)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight route in ticks between two ends 1 nm apart across it (ends at 0 and 40 ticks) takes its jog at the
    /// grid line nearest its middle, tick 16, as the search's `fit` does: on the half grid, so that F1 does not take
    /// the stored route again.
    #[test]
    fn a_straight_route_between_ends_off_one_line_jogs_on_the_grid() {
        let ticks = Ticks::of(Grid::MM4);
        let route = [Pt::new(0, 0), Pt::new(0, 40)];
        let (a, b) = ((0, 0), (1, ticks.nm(40)));
        let y = ticks.nm(16);
        assert_eq!(route_nm(ticks, &route, a, b), [a, (0, y), (1, y), b]);
    }
}
