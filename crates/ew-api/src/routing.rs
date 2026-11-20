// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The router as the window, the CLI and the tools reach it (docs/design/06a-drafting.md §2.7, §7): [`Session::route_plan`]
//! plans Route on a page or a selection with `ew_drafting::route` on the page as the layout sees it
//! ([`Session::page_draft`]) and gives the plan as the parameters of the one `layout.apply` that stores it;
//! [`route_wire`] and [`wire_clear`] are the Wire tool's route between two ends, in page nm; [`unrouted_line`] the line
//! of a wire without a route.
//!
//! **Back from ticks** (§2.1): a vertex on the grid takes the grid's own number for its position; a route's first and
//! last vertex are its ends' stored points (a junction's new place where the plan moves it), and the corner next to an
//! end takes that end's coordinate across the segment between them, so a route meets a pin that lies 1 nm off the grid.

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_units::Grid;
use ew_drafting::consts::TICKS_PER_STEP;
use ew_drafting::{JunctionId, PinId, Pt, Side, WireEnd};
pub use ew_drafting::{DraftPage, Jobs, Plan, Quality, Reason, WireId, measure, route as route_on};
use serde_json::{Value, json};

use crate::Session;
use crate::geometry::Dir;
use crate::output::{PageTables, TableEnd};

/// What a Route plans.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RouteScope {
    /// The page: its unrouted wires, those with a fault as stored, and those the program routed (§3.2).
    Page,
    /// The selection: the wires named, and every wire of the page with an end on a part named, whoever shaped them.
    Selection(Vec<Uuid>),
}

/// A plan of Route on one page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RoutePlan {
    /// The plan, in the layout's ticks.
    pub plan: Plan,
    /// The parameters of the `layout.apply` that stores it; `None` for a plan that changes nothing.
    pub params: Option<Value>,
}

impl RoutePlan {
    /// The wires whose routes the plan changes, by ID.
    pub fn changed(&self) -> Vec<Uuid> {
        self.plan.routes.iter().map(|(w, _)| w.0).collect()
    }
}

/// The conversion of one page between nm and the layout's ticks.
#[derive(Clone, Copy, Debug)]
pub struct Ticks(Grid);

impl Ticks {
    /// The conversion of a page with the grid `(num, den)` (a 4 mm step for numbers that make no grid).
    pub fn new(grid: (i64, i64)) -> Ticks {
        let g = u64::try_from(grid.0)
            .ok()
            .zip(u64::try_from(grid.1).ok())
            .and_then(|(n, d)| Grid::new(n, d).ok())
            .unwrap_or(Grid::MM4);
        Ticks(g)
    }

    /// A tick as page nm.
    pub fn nm(self, t: i32) -> i64 {
        self.0
            .nm_of_tick(i64::from(t), TICKS_PER_STEP.unsigned_abs())
            .map_or(0, |n| n.0)
    }

    /// A point in page nm as ticks (a point within 1 nm of a half-grid position takes its tick).
    pub fn pt(self, p: (i64, i64)) -> Pt {
        let one = |v: i64| {
            let (t, _) = self
                .0
                .tick_of(ew_derive::ew_units::Nm(v), TICKS_PER_STEP.unsigned_abs());
            i32::try_from(t).unwrap_or(if t < 0 { i32::MIN } else { i32::MAX })
        };
        Pt::new(one(p.0), one(p.1))
    }

    /// A route in ticks as page nm between the ends' stored points `a` and `b` (module documentation).
    pub fn route(self, route: &[Pt], a: (i64, i64), b: (i64, i64)) -> Vec<(i64, i64)> {
        let mut pts: Vec<(i64, i64)> = route.iter().map(|p| (self.nm(p.x), self.nm(p.y))).collect();
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
                let y = self.nm(i32::try_from((i64::from(route[0].y) + i64::from(route[1].y)) / 2).unwrap_or(0));
                vec![(a.0, y), (b.0, y)]
            } else {
                let x = self.nm(i32::try_from((i64::from(route[0].x) + i64::from(route[1].x)) / 2).unwrap_or(0));
                vec![(x, a.1), (x, b.1)]
            };
            pts.splice(1..1, mid);
        }
        pts.dedup();
        pts
    }
}

/// The stored point of a wire's end on the page of `tables`, with the junctions at `moved` places.
fn end_point(tables: &PageTables, e: &TableEnd, moved: &[(Uuid, (i64, i64))]) -> Option<(i64, i64)> {
    match e {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.at),
        TableEnd::Junction { id } => moved
            .iter()
            .find(|(j, _)| j == id)
            .map(|(_, p)| *p)
            .or_else(|| tables.junctions.iter().find(|j| j.id == *id).map(|j| j.at)),
    }
}

/// The routes of `plan` in page nm, by connection: each between its ends' stored points.
pub fn plan_routes(tables: &PageTables, plan: &Plan) -> Vec<(Uuid, Vec<(i64, i64)>)> {
    let ticks = Ticks::new(tables.grid);
    let moved: Vec<(Uuid, (i64, i64))> = plan
        .junctions
        .iter()
        .map(|(j, p)| (j.0, (ticks.nm(p.x), ticks.nm(p.y))))
        .collect();
    plan.routes
        .iter()
        .filter_map(|(w, route)| {
            let c = tables.connections.iter().find(|c| c.id == w.0)?;
            let (a, b) = (end_point(tables, &c.a, &moved)?, end_point(tables, &c.b, &moved)?);
            Some((w.0, ticks.route(route, a, b)))
        })
        .collect()
}

/// The `layout.apply` that stores `plan` on the page of `tables`; `None` for a plan that changes nothing. A marker's
/// anchor is read from `marker_at`.
pub fn layout_apply_params(
    tables: &PageTables,
    plan: &Plan,
    marker_at: &dyn Fn(Uuid) -> Option<(i64, i64)>,
) -> Option<Value> {
    if plan.is_empty() {
        return None;
    }
    let ticks = Ticks::new(tables.grid);
    let junctions: Vec<Value> = plan
        .junctions
        .iter()
        .map(|(j, p)| json!({"junction": j.0.to_string(), "at": [ticks.nm(p.x), ticks.nm(p.y)]}))
        .collect();
    let routes: Vec<Value> = plan_routes(tables, plan)
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
        let params = self.plan_params(&tables, &plan);
        Some(RoutePlan { plan, params })
    }

    /// The `layout.apply` of `plan` on the page of `tables` ([`layout_apply_params`]), the markers' anchors read from
    /// the project.
    pub fn plan_params(&self, tables: &PageTables, plan: &Plan) -> Option<Value> {
        let q = self.query();
        let items = q.page_items();
        let marker_at = |id: Uuid| match &items.get(Id::from_uuid(id))?.kind {
            ew_derive::ew_schema::PageItemKind::WireMarker(m) => Some((m.at.x.0, m.at.y.0)),
            _ => None,
        };
        layout_apply_params(tables, plan, &marker_at)
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
            let cp: Id<()> = ew_derive::ew_ids::v5::cp_id(Id::<ew_derive::ew_schema::Function>::from_uuid(function), u32::from(idx));
            WireEnd::Pin(PinId(cp.as_uuid()))
        }
        ToolEnd::Junction(j) => WireEnd::Junction(JunctionId(j)),
        ToolEnd::OnWire(w, p) => WireEnd::OnWire(WireId(w), ticks.pt(p)),
        ToolEnd::Point(p) => WireEnd::Point(ticks.pt(p)),
    }
}

/// The Wire tool's route from `from` (at the page point `a`) to `to` (at `b`) on `draft`, a page of grid `grid`
/// (§7.4): `ew_drafting::route_wire` back in page nm between the two points, and whether it runs across a text that
/// cannot step aside; `arrive` the direction a free first end was arrived at with. `None` where no route keeps free
/// of faults.
pub fn route_wire(
    draft: &DraftPage,
    grid: (i64, i64),
    (from, a): (ToolEnd, (i64, i64)),
    (to, b): (ToolEnd, (i64, i64)),
    arrive: Option<Dir>,
) -> Option<(Vec<(i64, i64)>, bool)> {
    let ticks = Ticks::new(grid);
    let found = ew_drafting::route_wire(draft, wire_end(ticks, from), wire_end(ticks, to), arrive.map(side))?;
    Some((ticks.route(&found.route, a, b), found.over_text))
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
