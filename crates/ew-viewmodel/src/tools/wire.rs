// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Wire tool (`wire`; doc 06 §4.4, FR-UI-015, FR-CON-001, FR-CON-006): a connection drawn from a
//! connection point, a junction or a point on a wire to another, routed by the pure router of §8.1 and committed as
//! `connection.draw` with the route the ghost showed.
//!
//! **Targets** in priority order within 8 screen points of the pointer or the grid cursor: a connection point of the
//! page tables (anywhere on an interruption point's arrow, its one CP), a junction, a point on a connection segment (the grid point of the segment nearest to the pointer
//! that is not its connection's end, so a press beside a rail's end joins the rail at the next grid point along it
//! rather than fixing a bend; a coordinate of the segment that is off the grid is the segment's own, the CP-exact coordinate
//! `require_split_point` accepts), else the grid point. Alt suppresses the object snaps. Ties by the smaller
//! distance, then for a CP the first in the page tables' order (natural label, its bytes, (y, x), ID), for a
//! junction by the smaller ID (two junctions are never at one point); for a segment the larger connection ID,
//! which is later in paint order (connections paint in ID order, so the one on top); the pick breaks a tie
//! the same way. This is a paint-order contract, not a creation-order guarantee: UUIDv7 IDs are not guaranteed to
//! be chronological or monotonic within the clock's resolution or across clock movement. A split's halves meet
//! at their junction, which is a target of its own before segments.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | `PointerMove`, `CursorMove` | armed | `Transient`: the snap marker of the target under the point |
//! | armed | `Press` or `Activate` on a CP, junction or segment | routing | the start is fixed; `Transient` |
//! | armed | `Press` or `Activate` on the grid | armed | `None` (a wire needs two endpoints) |
//! | routing | move | routing | `Transient`: the routed ghost from the last fixed vertex to the target (one `auto_route` per changed target), the fixed bends, the target's marker, the length readout |
//! | routing | `Press` or `Activate` on the grid | routing | a bend is fixed there (the route so far becomes fixed vertices) |
//! | routing | `Back` | routing | the last fixed bend is removed; without one `None` |
//! | routing | `Dwell { ms ≥ 80 }` on a valid end | routing | `Preview(connection.draw)` |
//! | routing | `Press` or `Activate` on a CP, junction or segment other than the start | armed | `Commit(connection.draw {a, b, page, route})`, or `connection.draw_many` for a route through representations ([`pass_through`]) |
//! | armed | `Key(Finish)` (a double click's second press) on a CP, junction or segment | routing | as `Press` there |
//! | routing | `Escape` | armed | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! The committed route is the fixed vertices joined with the router's route from the last fixed vertex to the end,
//! exactly the ghost (the router never runs on replay); a free terminal in line between the last fixed vertex and the
//! end is connected through on the direct route instead when nothing else is in its way (`through_route`). The connection kind is the page type's (doc 06 §2.2), given
//! by the host ([`WireTool::with_kind`]): a wire on a multi-line page (`connection.draw`'s default, so no `kind` is
//! sent), `overview` on single-line, overview and assembly-overview pages, a working fluid line (`{"fluid": {}}`) on a
//! fluid page; on the other page types the host refuses `tool.wire` until their kinds are drawn.

use ew_api::AnyCommand;
use ew_api::geometry::Dir;
use ew_api::output::Uuid;
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::routing::{OneEnd, RouteEnd, clear_one, heading, route_one, unrouted_line};
use serde_json::{Value, json};

use super::{
    Anchor, Button, CP_SNAP_PT, CanvasEvent, MarkerKind, Mods, Overlay, OverlayItem, RouteInfo, Style, Token,
    Tool, ToolCx, ToolKey, ToolOutput, div_round_toward_zero, segment_dist2,
};
use crate::canvas::mm3;
use crate::key;
use crate::strings;
use crate::strings::Key;

/// What the Wire tool snaps to.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum WireTarget {
    /// A connection point of the page.
    Cp {
        /// The function.
        function: EntityRef,
        /// The function's CP index.
        idx: u16,
        /// The representation that draws it.
        rep: EntityRef,
        /// Its point on the page.
        at: (i64, i64),
        /// The direction a wire leaves it.
        dir: Option<Dir>,
        /// Its label (`-102F1:2`).
        label: String,
    },
    /// A junction.
    Junction {
        /// The junction.
        id: EntityRef,
        /// Its point on the page.
        at: (i64, i64),
    },
    /// A point on a connection segment (a junction is created there and the connection split).
    Segment {
        /// The connection.
        connection: EntityRef,
        /// The point on the page.
        at: (i64, i64),
    },
    /// A grid point (a bend, never an end).
    Grid {
        /// The point on the page.
        at: (i64, i64),
    },
}

impl WireTarget {
    /// The point on the page.
    pub fn at(&self) -> (i64, i64) {
        match self {
            WireTarget::Cp { at, .. }
            | WireTarget::Junction { at, .. }
            | WireTarget::Segment { at, .. }
            | WireTarget::Grid { at } => *at,
        }
    }

    /// The leaving direction of a CP end.
    fn dir(&self) -> Option<Dir> {
        match self {
            WireTarget::Cp { dir, .. } => *dir,
            _ => None,
        }
    }

    /// The representation of a CP end.
    fn rep(&self) -> Option<EntityRef> {
        match self {
            WireTarget::Cp { rep, .. } => Some(*rep),
            _ => None,
        }
    }

    /// Whether it can end a wire.
    pub fn is_end(&self) -> bool {
        !matches!(self, WireTarget::Grid { .. })
    }

    /// The endpoint parameter of `connection.draw`; `None` for a grid point.
    fn endpoint(&self) -> Option<Value> {
        match self {
            WireTarget::Cp { function, idx, .. } => {
                Some(json!({"cp": {"function": function.0.to_string(), "idx": idx}}))
            }
            WireTarget::Junction { id, .. } => Some(json!({"junction": {"junction": id.0.to_string()}})),
            WireTarget::Segment { connection, at } => {
                Some(json!({"on_connection": {"connection": connection.0.to_string(), "at": [at.0, at.1]}}))
            }
            WireTarget::Grid { .. } => None,
        }
    }

    /// The snap marker's shape: a ring on a CP, a square on a junction or a split point, a cross on the grid.
    fn marker(&self) -> MarkerKind {
        match self {
            WireTarget::Cp { .. } => MarkerKind::Cp,
            WireTarget::Junction { .. } | WireTarget::Segment { .. } => MarkerKind::Junction,
            WireTarget::Grid { .. } => MarkerKind::Grid,
        }
    }

    /// The label the transcript and the live region use: a CP's label; a junction `@` and its name (`@C11`, `@C11#2`
    /// when the cell holds several); a split point `@` and its cell, then the wire it splits by its two ends
    /// (`@101-C12 F55:2 -> E55:L`, an end that is a junction by its name; two wires can run through one
    /// cell); a grid point `None`.
    pub fn label(&self, cx: &ToolCx<'_>) -> Option<String> {
        let cell_of = |p: (i64, i64)| cx.page.cell_text(cx.snapper.page_to_sheet(p)).unwrap_or_default();
        match self {
            WireTarget::Cp { label, .. } => Some(label.clone()),
            WireTarget::Junction { id, .. } => Some(format!(
                "@{}",
                super::junction_name(&cx.page.tables, *id, &cell_of)
            )),
            WireTarget::Segment { at, connection } => {
                let tables = &cx.page.tables;
                let end = |e: &ew_api::output::TableEnd| match e {
                    ew_api::output::TableEnd::Cp { function, idx } => tables
                        .cps
                        .iter()
                        .find(|c| c.function == *function && c.idx == *idx)
                        .map_or_else(String::new, |c| c.label.clone()),
                    ew_api::output::TableEnd::Junction { id } => {
                        format!("@{}", super::junction_name(tables, EntityRef(*id), &cell_of))
                    }
                };
                let mut label = format!("@{}", cell_of(*at));
                if let Some(c) = tables.connections.iter().find(|c| c.id == connection.0) {
                    label.push_str(&format!(" {} -> {}", end(&c.a), end(&c.b)));
                }
                Some(label)
            }
            WireTarget::Grid { .. } => None,
        }
    }

    /// Whether two targets name the same endpoint.
    fn same_end(&self, other: &WireTarget) -> bool {
        match (self, other) {
            (
                WireTarget::Cp {
                    function: f, idx: i, ..
                },
                WireTarget::Cp {
                    function: g, idx: j, ..
                },
            ) => f == g && i == j,
            (WireTarget::Junction { id: a, .. }, WireTarget::Junction { id: b, .. }) => a == b,
            (WireTarget::Segment { connection: a, .. }, WireTarget::Segment { connection: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// The grid of the page tables `(num, den)`: the index of a page coordinate and the coordinate of an index.
#[derive(Clone, Copy, Debug)]
pub struct PageGridMap {
    num: i128,
    den: i128,
}

impl PageGridMap {
    /// The map of the grid `(num, den)` nm.
    pub fn new(grid: (i64, i64)) -> PageGridMap {
        PageGridMap {
            num: i128::from(grid.0.max(1)),
            den: i128::from(grid.1.max(1)),
        }
    }

    /// The grid index nearest to the page coordinate `v`.
    pub fn index(self, v: i64) -> i64 {
        i64::try_from(div_round_toward_zero(i128::from(v) * self.den, self.num)).unwrap_or(0)
    }

    /// The page coordinate of grid index `k`.
    pub fn coord(self, k: i64) -> i64 {
        i64::try_from(div_round_toward_zero(i128::from(k) * self.num, self.den)).unwrap_or(0)
    }

    /// One grid step, nm (rounded).
    pub fn step(self) -> i64 {
        self.coord(1) - self.coord(0)
    }
}

/// The drawing area of the focused page in page nm: the sheet's rectangle mapped to the page.
pub fn drawing_area(cx: &ToolCx<'_>) -> [i64; 4] {
    let a = cx.snapper.sheet_to_page((0, 0));
    let b = cx.snapper.sheet_to_page(cx.page.sheet);
    [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]
}

/// The target at the sheet point `at` (see the module documentation).
pub fn resolve_target(cx: &ToolCx<'_>, at: (i64, i64), alt: bool) -> WireTarget {
    let grid_point = || WireTarget::Grid {
        at: cx.snapper.snap_grid(at).page_nm,
    };
    if alt {
        return grid_point();
    }
    let tables = &cx.page.tables;
    let reach = cx.nm_of_points(CP_SNAP_PT).floor().min(9.0e15) as i128;
    let limit = reach * reach;
    let d2 = |p: (i64, i64)| super::dist2(cx.snapper.page_to_sheet(p), at);
    // 1. A connection point.
    // A tie keeps the first CP in the page tables' order (natural label, its bytes, (y, x), ID).
    let mut best: Option<(i128, &ew_api::output::TableCp)> = None;
    for cp in &tables.cps {
        let d = d2(cp.at);
        if d <= limit && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, cp));
        }
    }
    // An interruption point's arrow stands for its one connection point, which sits a grid step behind the tip
    // (FT-1660: a click on the arrow of an outgoing point snapped to the grid beside it, and no wire reached it).
    if best.is_none() {
        let inside = |r: &&ew_api::output::TableRep| {
            let (a, b) = (
                cx.snapper.page_to_sheet((r.body[0], r.body[1])),
                cx.snapper.page_to_sheet((r.body[2], r.body[3])),
            );
            (a.0.min(b.0)..=a.0.max(b.0)).contains(&at.0) && (a.1.min(b.1)..=a.1.max(b.1)).contains(&at.1)
        };
        best = tables
            .reps
            .iter()
            .filter(|r| r.ip.is_some())
            .find(inside)
            .and_then(|r| tables.cps.iter().find(|cp| cp.rep == r.id))
            .map(|cp| (0, cp));
    }
    if let Some((_, cp)) = best {
        return WireTarget::Cp {
            function: EntityRef(cp.function),
            idx: cp.idx,
            rep: EntityRef(cp.rep),
            at: cp.at,
            dir: cp.dir,
            label: cp.label.clone(),
        };
    }
    // 2. A junction, but one that already joins three wires (a fourth would make a four-way junction).
    let mut best: Option<(i128, EntityRef, (i64, i64))> = None;
    for j in &tables.junctions {
        if full_junction(tables, j.id) {
            continue;
        }
        let d = d2(j.at);
        let id = EntityRef(j.id);
        if d <= limit && best.is_none_or(|(bd, bid, _)| d < bd || (d == bd && id < bid)) {
            best = Some((d, id, j.at));
        }
    }
    if let Some((_, id, at)) = best {
        return WireTarget::Junction { id, at };
    }
    // 3. A point on a connection segment.
    let grid = PageGridMap::new(tables.grid);
    let page = cx.snapper.sheet_to_page(at);
    let mut best: Option<(i128, EntityRef, (i64, i64))> = None;
    for c in &tables.connections {
        let line = super::autoconnect::dedup(&c.polyline);
        let (Some(first), Some(last)) = (line.first().copied(), line.last().copied()) else {
            continue;
        };
        for w in line.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = segment_dist2(at, cx.snapper.page_to_sheet(a), cx.snapper.page_to_sheet(b));
            if d > limit {
                continue;
            }
            // The nearest grid point that is not the connection's end, so a press beside a rail's
            // end joins it at the next grid point along it rather than fixing a bend there.
            let end = |p: (i64, i64)| p == first || p == last;
            let point = if a.1 == b.1 && a.0 != b.0 {
                on_axis_except(grid, page.0, a.0.min(b.0), a.0.max(b.0), |x| end((x, a.1))).map(|x| (x, a.1))
            } else if a.0 == b.0 && a.1 != b.1 {
                on_axis_except(grid, page.1, a.1.min(b.1), a.1.max(b.1), |y| end((a.0, y))).map(|y| (a.0, y))
            } else {
                None
            };
            let Some(p) = point else {
                continue;
            };
            let id = EntityRef(c.id);
            if best.is_none_or(|(bd, bid, _)| d < bd || (d == bd && id > bid)) {
                best = Some((d, id, p));
            }
        }
    }
    if let Some((_, connection, at)) = best {
        return WireTarget::Segment { connection, at };
    }
    grid_point()
}

/// Whether the junction `id` already joins three wires or more: it is no end of a new wire.
fn full_junction(tables: &ew_api::output::PageTables, id: Uuid) -> bool {
    let at = ew_api::output::TableEnd::Junction { id };
    tables
        .connections
        .iter()
        .filter(|c| c.a == at || c.b == at)
        .count()
        >= 3
}

/// Whether the sheet point `at` is within reach of a junction that already joins three wires and of no connection
/// point (the Wire tool's hint).
fn near_full_junction(cx: &ToolCx<'_>, at: (i64, i64), alt: bool) -> bool {
    let tables = &cx.page.tables;
    let reach = cx.nm_of_points(CP_SNAP_PT).floor().min(9.0e15) as i128;
    let near = |p: (i64, i64)| super::dist2(cx.snapper.page_to_sheet(p), at) <= reach * reach;
    !alt && !tables.cps.iter().any(|cp| near(cp.at))
        && tables
            .junctions
            .iter()
            .any(|j| near(j.at) && full_junction(tables, j.id))
}

/// The grid coordinate nearest to `v` inside `[lo, hi]`; `None` when no grid coordinate lies inside.
#[cfg(test)]
fn on_axis(grid: PageGridMap, v: i64, lo: i64, hi: i64) -> Option<i64> {
    on_axis_except(grid, v, lo, hi, |_| false)
}

/// The grid coordinate nearest to `v` inside `[lo, hi]` that `skip` does not refuse; `None` when there is none. A
/// skipped coordinate is an end of the range, so the next one along is at most one step further.
fn on_axis_except(grid: PageGridMap, v: i64, lo: i64, hi: i64, skip: impl Fn(i64) -> bool) -> Option<i64> {
    let k = grid.index(v.clamp(lo, hi));
    [k, k - 1, k + 1, k - 2, k + 2]
        .into_iter()
        .map(|k| grid.coord(k))
        .filter(|c| (lo..=hi).contains(c) && !skip(*c))
        .min_by_key(|c| ((*c - v).abs(), *c))
}

/// The direction from `a` to `b` along an axis; `None` for equal or diagonal points.
fn heading_between(a: (i64, i64), b: (i64, i64)) -> Option<Dir> {
    Dir::of_vector(((b.0 - a.0).signum(), (b.1 - a.1).signum()))
}

/// Removes consecutive duplicate vertices and interior vertices collinear with their neighbours; the ends stay.
pub(super) fn clean(pts: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
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

/// A function CP as `(function, index)`.
type CpKey = (Uuid, u16);

/// One connection of a route that passes through representations: from the wire's start or a CP to a CP
/// or the wire's end.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Piece {
    /// The CP it starts at; `None` for the wire's start.
    pub from: Option<CpKey>,
    /// The CP it ends at; `None` for the wire's end.
    pub to: Option<CpKey>,
    /// Its route, ends exact.
    pub route: Vec<(i64, i64)>,
}

/// The endpoint parameter of `connection.draw` for a function CP.
fn cp_endpoint((function, idx): CpKey) -> Value {
    json!({"cp": {"function": function.to_string(), "idx": idx}})
}

/// A free CP the route meets: where along the route and at which point of it.
#[derive(Clone, Copy, Debug)]
struct Hit<'t> {
    cp: &'t ew_api::output::TableCp,
    /// The distance along the route from its start to `proj`.
    pos: i64,
    /// The point of the route the CP is met at (the CP itself, or its foot on the segment for a CP beside it).
    proj: (i64, i64),
    /// The direction the route runs there.
    travel: Option<Dir>,
}

/// The pieces of a route from `start` to `end` that passes through representations:
/// the route meets two CPs of one other representation that have no connection on the page, the first where the run
/// enters it and the second where it leaves, so the wire is drawn as two connections, start to the first and the
/// second to the end, and the representation is in the circuit. Empty when the route passes through none, which is
/// one `connection.draw`.
///
/// - **Met.** A CP is met when its foot on a segment of the route (the perpendicular one, inside the segment) is
///   within the CP snap reach ([`CP_SNAP_PT`], the hit tolerance of the targets) of the CP on the sheet. A CP beside
///   the route (not on it) is reached by a perpendicular jog from its foot, and only when the CP and the foot are on
///   the connection grid, so that the pieces are routes `connection.draw` accepts.
/// - **Counted.** CPs with a connection on the page, CPs of the start's or the end's representation and CPs at the
///   route's ends are not counted. A representation is passed through when exactly two of its CPs are met, at two
///   different places strictly inside the route, the first CP leaving against the run (or without a direction) and
///   the second along it (or without one): a terminal in a vertical run. One CP met (a T) or a run along two CPs that
///   face the same way (the top pins of a two-pole device) is not a pass.
/// - **Several.** Every representation passed through is in the chain in the order of the route. When two of them
///   overlap along the route, or a pass begins at the route's start or ends at its end, the route is one wire.
pub fn pass_through(
    cx: &ToolCx<'_>,
    start: &WireTarget,
    end: &WireTarget,
    route: &[(i64, i64)],
) -> Vec<Piece> {
    let Some(Passes {
        line,
        cum,
        total,
        passes,
    }) = passes(cx, [start.rep(), end.rep()], [start.at(), end.at()], route)
    else {
        return Vec::new();
    };
    // The route between two places along it: the first point, the vertices strictly between, the second point.
    let between = |s0: i64, q0: (i64, i64), s1: i64, q1: (i64, i64)| -> Vec<(i64, i64)> {
        let mut pts = vec![q0];
        pts.extend(
            (0..line.len())
                .filter(|&k| s0 < cum[k] && cum[k] < s1)
                .map(|k| line[k]),
        );
        pts.push(q1);
        pts
    };
    let key = |h: &Hit<'_>| (h.cp.function, h.cp.idx);
    let mut pieces = Vec::with_capacity(passes.len() + 1);
    let (mut from, mut s0, mut q0, mut head) = (None, 0, line[0], Vec::new());
    for (entry, exit) in &passes {
        let mut pts = head;
        pts.extend(between(s0, q0, entry.pos, entry.proj));
        pts.push(entry.cp.at);
        pieces.push(Piece {
            from,
            to: Some(key(entry)),
            route: clean(pts),
        });
        from = Some(key(exit));
        (s0, q0, head) = (exit.pos, exit.proj, vec![exit.cp.at]);
    }
    let mut pts = head;
    pts.extend(between(s0, q0, total, line[line.len() - 1]));
    pieces.push(Piece {
        from,
        to: None,
        route: clean(pts),
    });
    pieces
}

/// The representations a route passes through ([`pass_through`]'s rule) and where.
struct Passes<'t> {
    /// The route, duplicates removed.
    line: Vec<(i64, i64)>,
    /// The distance along the route to each vertex of `line`.
    cum: Vec<i64>,
    /// The route's length.
    total: i64,
    /// Each pass's entry and exit, in the order of the route; never empty.
    passes: Vec<(Hit<'t>, Hit<'t>)>,
}

/// The passes of `route` by [`pass_through`]'s rule, the CPs of the representations `own` and at the points `ends`
/// not counted; `None` when it passes through none.
fn passes<'t>(
    cx: &'t ToolCx<'_>,
    own: [Option<EntityRef>; 2],
    ends: [(i64, i64); 2],
    route: &[(i64, i64)],
) -> Option<Passes<'t>> {
    let line = super::autoconnect::dedup(route);
    if line.len() < 2 {
        return None;
    }
    let tables = &cx.page.tables;
    let reach = cx.nm_of_points(CP_SNAP_PT).floor().min(9.0e15) as i128;
    let limit = reach * reach;
    let grid = PageGridMap::new(tables.grid);
    let on_grid = |p: (i64, i64)| grid.coord(grid.index(p.0)) == p.0 && grid.coord(grid.index(p.1)) == p.1;
    let sheet = |p: (i64, i64)| cx.snapper.page_to_sheet(p);
    // The distance along the route to each vertex.
    let mut cum = vec![0i64; line.len()];
    for k in 1..line.len() {
        let (a, b) = (line[k - 1], line[k]);
        cum[k] = cum[k - 1].saturating_add((b.0 - a.0).abs().saturating_add((b.1 - a.1).abs()));
    }
    let total = cum[line.len() - 1];
    let mut by_rep: std::collections::BTreeMap<Uuid, Vec<Hit<'_>>> = std::collections::BTreeMap::new();
    for cp in &tables.cps {
        if !cp.connections.is_empty() || own.contains(&Some(EntityRef(cp.rep))) || ends.contains(&cp.at) {
            continue;
        }
        let mut best: Option<(i128, Hit<'_>)> = None;
        for (i, w) in line.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let proj = if a.1 == b.1 && a.0 != b.0 && (a.0.min(b.0)..=a.0.max(b.0)).contains(&cp.at.0) {
                (cp.at.0, a.1)
            } else if a.0 == b.0 && a.1 != b.1 && (a.1.min(b.1)..=a.1.max(b.1)).contains(&cp.at.1) {
                (a.0, cp.at.1)
            } else {
                continue;
            };
            let d = super::dist2(sheet(cp.at), sheet(proj));
            if d > limit || (proj != cp.at && !(on_grid(proj) && on_grid(cp.at))) {
                continue;
            }
            if best.is_none_or(|(bd, _)| d < bd) {
                let along = (proj.0 - a.0).abs() + (proj.1 - a.1).abs();
                let hit = Hit {
                    cp,
                    pos: cum[i] + along,
                    proj,
                    travel: heading_between(a, b),
                };
                best = Some((d, hit));
            }
        }
        if let Some((_, hit)) = best {
            by_rep.entry(cp.rep).or_default().push(hit);
        }
    }
    let opposite = |d: Dir| Dir::of_vector((-d.vector().0, -d.vector().1));
    let mut passes: Vec<(Hit<'_>, Hit<'_>)> = by_rep
        .into_values()
        .filter_map(|mut hits| {
            let [_, _] = hits.as_slice() else {
                return None;
            };
            hits.sort_by_key(|h| h.pos);
            let (entry, exit) = (hits[0], hits[1]);
            let enters = entry.cp.dir.is_none() || entry.travel.and_then(opposite) == entry.cp.dir;
            let leaves = exit.cp.dir.is_none() || exit.travel == exit.cp.dir;
            (entry.pos < exit.pos && enters && leaves).then_some((entry, exit))
        })
        .collect();
    if passes.is_empty() {
        return None;
    }
    passes.sort_by_key(|(entry, _)| entry.pos);
    let mut last = 0;
    for (entry, exit) in &passes {
        if entry.pos <= last {
            return None;
        }
        last = exit.pos;
    }
    if last >= total {
        return None;
    }
    Some(Passes {
        line,
        cum,
        total,
        passes,
    })
}

/// The tool's state.
#[derive(Clone, Debug)]
enum State {
    Armed {
        /// The target under the point.
        target: Option<WireTarget>,
    },
    Routing {
        /// The start.
        start: WireTarget,
        /// The fixed vertices from the start (the start's point first).
        fixed: Vec<(i64, i64)>,
        /// `fixed` as it was before each bend (Backspace restores it; a bend that continues straight is cleaned out
        /// of `fixed`, so a length would not do).
        marks: Vec<Vec<(i64, i64)>>,
        /// The target under the point.
        end: Option<WireTarget>,
        /// The ghost route from the start to the end target's point.
        route: Vec<(i64, i64)>,
    },
}

/// The key of a computed route: the page content's revision, the last fixed vertex with its leaving direction and its
/// arrival direction, the end with its direction, and the ends' own representations.
type RouteKey = (
    u64,
    (i64, i64),
    Option<Dir>,
    Option<Dir>,
    (i64, i64),
    Option<Dir>,
    [Option<EntityRef>; 2],
);

/// A routed tail in the cache: its key, its points, and whether it is the router's fallback.
type CachedTail = (RouteKey, Vec<(i64, i64)>, bool);

/// The router's end of a point with the direction a wire leaves it and the representation that draws it.
fn one_end(at: (i64, i64), dir: Option<Dir>, rep: Option<EntityRef>) -> OneEnd {
    OneEnd {
        at,
        dir: dir.map(heading),
        rep: rep.map(|r| r.0),
    }
}

/// A tool's end as the router takes it.
fn route_end(at: (i64, i64), dir: Option<Dir>) -> RouteEnd {
    RouteEnd {
        at,
        dir: dir.map(heading),
    }
}

/// The user's tail from `from` (leaving along `from_dir`, or arrived at along `arrive`) to `to` (entered against
/// `to_dir`), taken without a clear route after a fixed bend: the straight segment for aligned ends,
/// else one corner from the ends' own coordinates, the first leg along `from_dir`'s axis, else the last along
/// `to_dir`'s, else across first; a first leg that would double back over `arrive` takes the other corner.
fn l_tail(
    (from, from_dir, arrive): ((i64, i64), Option<Dir>, Option<Dir>),
    to: (i64, i64),
    to_dir: Option<Dir>,
) -> Vec<(i64, i64)> {
    if from == to {
        return vec![from];
    }
    // The corner of the unrouted line's rule.
    let line = unrouted_line(route_end(from, from_dir), route_end(to, to_dir));
    let [_, mut corner, _] = line[..] else {
        return line;
    };
    let (down_first, across_first) = ((from.0, to.1), (to.0, from.1));
    if from_dir.is_none()
        && let Some(h) = arrive
        && heading_between(from, corner).map(Dir::vector) == Some((-h.vector().0, -h.vector().1))
    {
        corner = if corner == down_first {
            across_first
        } else {
            down_first
        };
    }
    vec![from, corner, to]
}

/// The direct route of a tail from `from` to `to` when it passes through a free representation in line: the
/// router reserves every other CP and its stub, so it never routes into
/// a free terminal and [`pass_through`] would never see one; a terminal standing in line between the ends is to be
/// connected through, not avoided.
///
/// The direct route is the straight run between aligned ends, else the L of [`l_tail`]. It is taken when
/// [`pass_through`]'s rule finds at least one representation passed on it (the CPs of `own` and at the two ends not
/// counted), every one passed a terminal (a fuse, a contact or a coil standing in line is gone round, not put in series
/// by accident: dropped on a wire it is put in series), every passed CP lying on the route itself (not beside it, a
/// jog away), and the router's own tests
/// ([`clear_one`]: the page's bodies and texts, other wires, other CPs and stubs) find it clear with the passed
/// representations' bodies, texts and CPs left out. Otherwise `None`, and the router runs as before.
fn through_route(
    cx: &ToolCx<'_>,
    (from, from_dir, arrive): ((i64, i64), Option<Dir>, Option<Dir>),
    own: [Option<EntityRef>; 2],
    to: &WireTarget,
) -> Option<Vec<(i64, i64)>> {
    let t = to.at();
    if from == t {
        return None;
    }
    let direct = clean(l_tail((from, from_dir, arrive), t, to.dir()));
    let found = passes(cx, own, [from, t], &direct)?;
    // In line: the route runs over the passed CPs themselves, not beside them; and every one passed is a terminal.
    let terminal = |rep: Uuid| cx.page.tables.reps.iter().any(|r| r.id == rep && r.terminal);
    if found
        .passes
        .iter()
        .any(|(entry, exit)| entry.proj != entry.cp.at || exit.proj != exit.cp.at || !terminal(entry.cp.rep))
    {
        return None;
    }
    let mut through: Vec<Uuid> = found.passes.iter().map(|(entry, _)| entry.cp.rep).collect();
    through.sort_unstable();
    through.dedup();
    let ends = (one_end(from, from_dir, own[0]), one_end(t, to.dir(), own[1]));
    clear_one(&cx.page.geometry, ends, arrive.map(heading), &through, &direct).then_some(direct)
}

/// The Wire tool.
#[derive(Clone, Debug)]
pub struct WireTool {
    state: State,
    /// The last routed tail, whether it is the router's fallback, and its key (one route per changed snapped point).
    cache: Option<CachedTail>,
    /// Whether the last tail the tool computed found no clear route: the status hint says so until
    /// the next wire starts.
    direct: bool,
    /// Whether the point is on a junction that already joins three wires: the hint says so.
    full_junction: bool,
    /// How many times the router ran (the exact-count test, §12 item 4).
    routes: u64,
    /// The labels of the start and the end, for the transcript.
    labels: (Option<String>, Option<String>),
    /// The JSON value of `connection.draw`'s `kind` for the page type; `None` for a wire (the default).
    kind: Option<&'static str>,
}

/// The JSON value of a connection kind name: `fluid` is a working fluid line (`{"fluid": {}}`), any other name its
/// own string.
fn kind_value(kind: &str) -> Value {
    if kind == "fluid" {
        json!({"fluid": {}})
    } else {
        Value::from(kind)
    }
}

impl Default for WireTool {
    fn default() -> WireTool {
        WireTool::new()
    }
}

impl WireTool {
    /// The tool, armed.
    pub fn new() -> WireTool {
        WireTool {
            state: State::Armed { target: None },
            cache: None,
            direct: false,
            full_junction: false,
            routes: 0,
            labels: (None, None),
            kind: None,
        }
    }

    /// The tool drawing connections of `kind` (`connection.draw`'s kind name; `None` for a wire).
    pub fn with_kind(mut self, kind: Option<&'static str>) -> WireTool {
        self.kind = kind;
        self
    }

    /// How many times the router ran since the tool was made.
    pub fn route_calls(&self) -> u64 {
        self.routes
    }

    /// The start's label while routing (the live region's text).
    pub fn start_label(&self) -> Option<&str> {
        match self.state {
            State::Routing { .. } => self.labels.0.as_deref(),
            State::Armed { .. } => None,
        }
    }

    /// The router's tail from `from` (leaving along `from_dir`, or arrived at along `arrive`) to the end target,
    /// through the cache; it sets `self.direct` to whether no clear route was found.
    ///
    /// The tail is [`route_one`]'s against the page as drawn (the host's page geometry): every
    /// routed connection a fixed wire, every other CP with its stub and every junction not at an end reserved, the
    /// ends' own bodies left only along a CP's escape corridor (`own`: the start's while the tail starts at it, the
    /// end's). Before it, the direct route through a free terminal in line is taken when it qualifies
    /// ([`through_route`]). Without a clear route the tail is the right-angled line an unrouted connection is
    /// drawn with ([`unrouted_line`], the page's own rule, so the ghost is what a click stores), or after a fixed
    /// bend the user's L ([`l_tail`]).
    fn tail(
        &mut self,
        cx: &ToolCx<'_>,
        (from, from_dir, arrive): ((i64, i64), Option<Dir>, Option<Dir>),
        own: [Option<EntityRef>; 2],
        to: &WireTarget,
    ) -> Vec<(i64, i64)> {
        let key: RouteKey = (cx.page.rev, from, from_dir, arrive, to.at(), to.dir(), own);
        if let Some((k, r, direct)) = &self.cache
            && *k == key
        {
            self.direct = *direct;
            return r.clone();
        }
        let t = to.at();
        if let Some(points) = through_route(cx, (from, from_dir, arrive), own, to) {
            self.routes += 1;
            self.direct = false;
            self.cache = Some((key, points.clone(), false));
            return points;
        }
        let routed = route_one(
            &cx.page.geometry,
            one_end(from, from_dir, own[0]),
            one_end(t, to.dir(), own[1]),
            arrive.map(heading),
        );
        let (points, direct) = match routed {
            Some(points) => (points, false),
            None if arrive.is_some() => (l_tail((from, from_dir, arrive), t, to.dir()), true),
            None => (
                unrouted_line(route_end(from, from_dir), route_end(t, to.dir())),
                true,
            ),
        };
        self.routes += 1;
        self.direct = direct;
        self.cache = Some((key, points.clone(), direct));
        points
    }

    /// The full route from the start's fixed vertices to `to`.
    fn full_route(
        &mut self,
        cx: &ToolCx<'_>,
        start: &WireTarget,
        fixed: &[(i64, i64)],
        to: &WireTarget,
    ) -> Vec<(i64, i64)> {
        let Some(&last) = fixed.last() else {
            return Vec::new();
        };
        let from_dir = if fixed.len() == 1 { start.dir() } else { None };
        // After a fixed bend the router takes the last segment's direction, so it never doubles back over it.
        let arrive = match fixed {
            [.., a, b] => heading_between(*a, *b),
            _ => None,
        };
        // The start's representation is the tail's own only while the tail starts at it; after a bend it is an
        // obstacle like any other.
        let own = [start.rep().filter(|_| fixed.len() == 1), to.rep()];
        let tail = self.tail(cx, (last, from_dir, arrive), own, to);
        let mut pts: Vec<(i64, i64)> = fixed[..fixed.len() - 1].to_vec();
        pts.extend(tail);
        clean(pts)
    }

    /// The overlay of the current state.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let mut items = Vec::new();
        match &self.state {
            State::Armed { target } => {
                if let Some(t) = target {
                    items.push(OverlayItem::Marker {
                        at: cx.snapper.page_to_sheet(t.at()),
                        kind: t.marker(),
                    });
                }
            }
            State::Routing {
                start, end, route, ..
            } => {
                let pts: Vec<(i64, i64)> = route.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect();
                if pts.len() >= 2 {
                    items.push(OverlayItem::Polyline {
                        points: pts.clone(),
                        style: Style::dashed(Token::Ghost),
                    });
                }
                items.push(OverlayItem::Marker {
                    at: cx.snapper.page_to_sheet(start.at()),
                    kind: start.marker(),
                });
                if let (Some(t), Some(&at)) = (end, pts.last()) {
                    let length: f64 = pts
                        .windows(2)
                        .map(|w| ((w[1].0 - w[0].0) as f64).hypot((w[1].1 - w[0].1) as f64))
                        .sum();
                    items.push(OverlayItem::Label {
                        at,
                        text: strings::format(key!("wire-readout"), &[&mm3(length.round() as i64)]),
                        anchor: Anchor::Right,
                    });
                    items.push(OverlayItem::Marker {
                        at: cx.snapper.page_to_sheet(t.at()),
                        kind: t.marker(),
                    });
                }
            }
        }
        Overlay::new(items)
    }

    /// The command of the current route to `end`; `None` without a valid end. A route that passes through
    /// representations ([`pass_through`]) is `connection.draw_many` with one entry per piece, in one
    /// transaction; any other is one `connection.draw`.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        let State::Routing {
            start, end, route, ..
        } = &self.state
        else {
            return None;
        };
        let end = end.as_ref().filter(|e| e.is_end() && !e.same_end(start))?;
        // Without a clear route and without a fixed bend, the wire is stored unrouted.
        let route: &[(i64, i64)] = if self.direct && self.bends() == 0 {
            &[]
        } else {
            route
        };
        let draw = |a: Value, b: Value, route: &[(i64, i64)]| {
            let mut params = json!({
                "a": a,
                "b": b,
                "page": cx.page.tables.page.to_string(),
                "route": route.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
            });
            if let Some(kind) = self.kind {
                params["kind"] = kind_value(kind);
            }
            params
        };
        let (a, b) = (start.endpoint()?, end.endpoint()?);
        let pieces = pass_through(cx, start, end, route);
        if pieces.is_empty() {
            return Some(Box::new(AnyCommand {
                name: "connection.draw".into(),
                params: draw(a, b, route),
            }));
        }
        let connections: Vec<Value> = pieces
            .iter()
            .map(|p| {
                let from = p.from.map_or_else(|| a.clone(), cp_endpoint);
                let to = p.to.map_or_else(|| b.clone(), cp_endpoint);
                draw(from, to, &p.route)
            })
            .collect();
        Some(Box::new(AnyCommand {
            name: "connection.draw_many".into(),
            params: json!({ "connections": connections }),
        }))
    }

    /// The target under `at` moved to: re-route while routing.
    fn moved(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        let target = resolve_target(cx, at, mods.alt);
        self.full_junction = near_full_junction(cx, at, mods.alt);
        match self.state.clone() {
            State::Armed { .. } => {
                self.state = State::Armed { target: Some(target) };
            }
            State::Routing {
                start, fixed, marks, ..
            } => {
                let route = self.full_route(cx, &start, &fixed, &target);
                self.labels.1 = target.label(cx);
                self.state = State::Routing {
                    start,
                    fixed,
                    marks,
                    end: Some(target),
                    route,
                };
            }
        }
        ToolOutput::Transient(self.overlay(cx))
    }

    /// A click (a press or Enter) at `at`.
    fn click(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        let target = resolve_target(cx, at, mods.alt);
        self.full_junction = near_full_junction(cx, at, mods.alt);
        match self.state.clone() {
            State::Armed { .. } => {
                if !target.is_end() {
                    self.state = State::Armed { target: Some(target) };
                    return ToolOutput::None;
                }
                self.labels = (target.label(cx), None);
                self.direct = false;
                self.state = State::Routing {
                    fixed: vec![target.at()],
                    marks: Vec::new(),
                    end: None,
                    route: vec![target.at()],
                    start: target,
                };
                ToolOutput::Transient(self.overlay(cx))
            }
            State::Routing {
                start,
                mut fixed,
                mut marks,
                ..
            } => {
                if target.is_end() {
                    if target.same_end(&start) {
                        return ToolOutput::None;
                    }
                    let route = self.full_route(cx, &start, &fixed, &target);
                    self.labels.1 = target.label(cx);
                    self.state = State::Routing {
                        start,
                        fixed,
                        marks,
                        end: Some(target),
                        route,
                    };
                    let cmd = self.command(cx);
                    self.state = State::Armed { target: None };
                    self.cache = None;
                    return match cmd {
                        Some(c) => ToolOutput::Commit(c),
                        None => ToolOutput::None,
                    };
                }
                // A bend: the route so far to the grid point becomes fixed.
                let route = self.full_route(cx, &start, &fixed, &target);
                marks.push(fixed);
                fixed = route.clone();
                self.state = State::Routing {
                    start,
                    fixed,
                    marks,
                    end: Some(target),
                    route,
                };
                ToolOutput::Transient(self.overlay(cx))
            }
        }
    }

    /// The number of fixed bends while routing.
    fn bends(&self) -> usize {
        match &self.state {
            State::Routing { marks, .. } => marks.len(),
            State::Armed { .. } => 0,
        }
    }

    /// P: ends the routing wire in a new interruption point drawn with `shape` (the
    /// IP symbol, one CP) at the end target's grid point, oriented among A–D so that its CP faces the arriving
    /// segment (the CP leaves opposite to the last segment's direction), its insertion point the grid point that puts
    /// the CP there (the route then ends exactly at the CP, re-routed when the grid rounds the CP off the point);
    /// `name_of` gives the name and the direction from the start and the insertion point. Answers the
    /// `connection.draw_to_ip` commit with the name and direction, the tool armed again; a refusal key while not
    /// routing to a grid point (`reason-ip-end-grid`), before the wire has a segment (`reason-ip-end-segment`) or for a
    /// symbol without exactly one CP (`reason-ip-symbol`).
    ///
    /// With EW's two forms (`shapes`: the source form and the target form) the point is never turned:
    /// `name_of` gives the name and the direction from the start and the end's grid point first, and the direction's
    /// form is drawn there, its CP where the grid puts it, the route ending at it.
    pub fn end_in_ip(
        &mut self,
        cx: &ToolCx<'_>,
        shapes: (&ew_api::output::SymbolShape, Option<&ew_api::output::SymbolShape>),
        name_of: impl FnOnce(&WireTarget, (i64, i64)) -> (String, &'static str),
    ) -> Result<(ToolOutput, String, &'static str), Key<'static>> {
        let State::Routing {
            start,
            fixed,
            end,
            route,
            ..
        } = self.state.clone()
        else {
            return Err(key!("reason-ip-end-grid"));
        };
        let Some(WireTarget::Grid { at: end }) = end else {
            return Err(key!("reason-ip-end-grid"));
        };
        let mut name_of = Some(name_of);
        let (shape, named) = match (shapes, name_of.take_if(|_| shapes.1.is_some())) {
            ((source, Some(target)), Some(name_of)) => {
                let (name, direction) = name_of(&start, end);
                let shape = if direction == "target" { target } else { source };
                (shape, Some((name, direction)))
            }
            ((shape, _), _) => (shape, None),
        };
        let [cp] = shape.cps.as_slice() else {
            return Err(key!("reason-ip-symbol"));
        };
        // The arriving direction: the last segment of the routed ghost.
        let line = super::autoconnect::dedup(&route);
        let [.., a, b] = line.as_slice() else {
            return Err(key!("reason-ip-end-segment"));
        };
        // Along the segment's longer axis: an unrouted line from a CP 1 nm off the grid is not quite straight.
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let v = if dx.abs() >= dy.abs() {
            (dx.signum(), 0)
        } else {
            (0, dy.signum())
        };
        let arrive = Dir::of_vector(v).ok_or(key!("reason-ip-end-segment"))?;
        let away =
            Dir::of_vector((-arrive.vector().0, -arrive.vector().1)).ok_or(key!("reason-ip-end-segment"))?;
        let orient = match named {
            Some(_) => 0,
            None => (0..4u8)
                .find(|o| ew_api::geometry::orient_dir(*o, cp.dir) == away)
                .ok_or(key!("reason-ip-symbol"))?,
        };
        let off = ew_api::geometry::orient_point(orient, cp.at);
        let grid = PageGridMap::new(cx.page.tables.grid);
        let pos = (
            grid.coord(grid.index(end.0 - off.0)),
            grid.coord(grid.index(end.1 - off.1)),
        );
        let cp_at = (pos.0 + off.0, pos.1 + off.1);
        let target = WireTarget::Cp {
            function: EntityRef(cx.page.tables.page),
            idx: cp.fn_cp,
            rep: EntityRef(cx.page.tables.page),
            at: cp_at,
            dir: Some(ew_api::geometry::orient_dir(orient, cp.dir)),
            label: String::new(),
        };
        let mut route = self.full_route(cx, &start, &fixed, &target);
        if self.direct && fixed.len() == 1 {
            // Without a clear route and without a fixed bend, the wire is stored unrouted.
            route.clear();
        }
        let (name, direction) = match (named, name_of) {
            (Some(named), _) => named,
            (None, Some(name_of)) => name_of(&start, pos),
            (None, None) => return Err(key!("reason-ip-symbol")),
        };
        let Some(a) = start.endpoint() else {
            return Err(key!("reason-ip-end-grid"));
        };
        let mut params = json!({
            "a": a,
            "page": cx.page.tables.page.to_string(),
            "route": route.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
            "ip": {
                "symbol": shape.copy.to_string(),
                "pos": [pos.0, pos.1],
                "orient": super::place::ORIENT_NAMES[usize::from(orient)],
                "name": name,
                "direction": direction,
            },
        });
        if let Some(kind) = self.kind {
            params["kind"] = kind_value(kind);
        }
        self.state = State::Armed { target: None };
        self.cache = None;
        let cmd = Box::new(AnyCommand {
            name: "connection.draw_to_ip".into(),
            params,
        });
        Ok((ToolOutput::Commit(cmd), name, direction))
    }

    /// The committed route the ghost shows (tests).
    pub fn ghost_route(&self) -> Option<&[(i64, i64)]> {
        match &self.state {
            State::Routing { route, .. } => Some(route),
            State::Armed { .. } => None,
        }
    }
}

impl Tool for WireTool {
    fn name(&self) -> &'static str {
        "wire"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "armed",
            State::Routing { .. } => "routing",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, mods } => self.moved(*at, *mods, cx),
            CanvasEvent::CursorMove { at } => self.moved(*at, Mods::default(), cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => self.click(*at, *mods, cx),
            // A double click is `Finish` in place of its second press (the window's rule). Armed, it
            // is that second press: a click that ends a wire on a CP and a second one there at once start the next
            // wire from that CP, as two slower clicks do (the wire was silently not drawn). Routing, it is
            // nothing (the first press fixed the bend or ended the wire).
            CanvasEvent::Key(ToolKey::Finish) if matches!(self.state, State::Armed { .. }) => {
                match &self.state {
                    State::Armed { target: Some(t) } if t.is_end() => {
                        let at = cx.snapper.page_to_sheet(t.at());
                        self.click(at, Mods::default(), cx)
                    }
                    _ => ToolOutput::None,
                }
            }
            CanvasEvent::Activate => {
                let at = match &self.state {
                    State::Armed { target: Some(t) } => Some(cx.snapper.page_to_sheet(t.at())),
                    State::Routing { end: Some(t), .. } => Some(cx.snapper.page_to_sheet(t.at())),
                    _ => None,
                };
                match at {
                    Some(at) => self.click(at, Mods::default(), cx),
                    None => ToolOutput::None,
                }
            }
            CanvasEvent::Back => match self.state.clone() {
                State::Routing {
                    start,
                    mut marks,
                    end,
                    ..
                } => {
                    let Some(fixed) = marks.pop() else {
                        return ToolOutput::None;
                    };
                    let route = match &end {
                        Some(t) => self.full_route(cx, &start, &fixed, t),
                        None => fixed.clone(),
                    };
                    self.state = State::Routing {
                        start,
                        fixed,
                        marks,
                        end,
                        route,
                    };
                    ToolOutput::Transient(self.overlay(cx))
                }
                State::Armed { .. } => ToolOutput::None,
            },
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Escape => match self.state {
                State::Routing { .. } => {
                    self.state = State::Armed { target: None };
                    self.cache = None;
                    self.direct = false;
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    /// While a wire is drawn to a junction that already joins three wires, the hint says it is no end (not before the
    /// wire starts: a wire just ended on a wire left the pointer on the junction it made, and the hint read as a
    /// refusal of that wire, FT-1660); once a click placed a bend, that a bend is placed; while no clear route is
    /// found, and after a wire drawn so until the next one starts, that the wire stays unrouted (not a refusal).
    fn status_hint(&self) -> super::Hint {
        if self.full_junction && matches!(self.state, State::Routing { .. }) {
            return super::Hint::plain(key!("hint-wire-full-junction"));
        }
        // Once a click placed a bend instead of ending on a connection point, the hint says so and what
        // ends the wire (a click that missed a connection point snaps to the grid and bends there).
        if self.bends() > 0 {
            return super::Hint::plain(key!("hint-wire-bend"));
        }
        if self.direct {
            return super::Hint::plain(key!("hint-wire-direct"));
        }
        match self.state {
            State::Armed { .. } => super::Hint::new(key!("hint-wire-armed"), &[super::ENTER]),
            State::Routing { .. } => {
                super::Hint::new(key!("hint-wire-routing"), &[super::BACK, super::ESCAPE])
            }
        }
    }

    /// A double click ends the wire on its target and arms the next one there (doc 16's keymap).
    fn finishes_on_double_click(&self) -> bool {
        true
    }

    fn route_info(&self) -> Option<RouteInfo> {
        match &self.state {
            State::Routing { route, .. } => Some(RouteInfo {
                from: self.labels.0.clone().unwrap_or_default(),
                bends: self.bends(),
                points: route.len(),
                end: self.labels.1.clone(),
            }),
            State::Armed { .. } => None,
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// The sheet rectangle of a page rectangle (tests and the host's picking).
pub fn sheet_rect(cx: &ToolCx<'_>, r: [i64; 4]) -> Rect {
    super::rect_of(
        cx.snapper.page_to_sheet((r[0], r[1])),
        cx.snapper.page_to_sheet((r[2], r[3])),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_map_rounds_as_the_commands_grid_does() {
        let g = PageGridMap::new((7_937_500, 3));
        for k in [-7, -1, 0, 1, 2, 3, 100] {
            assert_eq!(g.index(g.coord(k)), k);
        }
        assert_eq!(g.step(), 2_645_833);
    }

    #[test]
    fn on_axis_takes_the_nearest_grid_coordinate_inside_the_segment() {
        let g = PageGridMap::new((1_000_000, 1));
        assert_eq!(on_axis(g, 2_400_000, 0, 10_000_000), Some(2_000_000));
        assert_eq!(on_axis(g, -5_000_000, 0, 10_000_000), Some(0));
        assert_eq!(on_axis(g, 500_000, 100_000, 900_000), None);
    }

    /// A refused end gives way to the next grid coordinate along; a range of its two ends alone has none.
    #[test]
    fn on_axis_except_takes_the_next_coordinate_past_a_refused_end() {
        let g = PageGridMap::new((1_000_000, 1));
        let ends = |c: i64| c == 0 || c == 10_000_000;
        assert_eq!(on_axis_except(g, 400_000, 0, 10_000_000, ends), Some(1_000_000));
        assert_eq!(
            on_axis_except(g, -5_000_000, 0, 10_000_000, ends),
            Some(1_000_000)
        );
        assert_eq!(on_axis_except(g, 9_700_000, 0, 10_000_000, ends), Some(9_000_000));
        assert_eq!(
            on_axis_except(g, 400_000, 0, 1_000_000, |c| c == 0 || c == 1_000_000),
            None
        );
    }

    #[test]
    fn clean_removes_duplicates_and_collinear_vertices() {
        assert_eq!(
            clean(vec![(0, 0), (0, 0), (5, 0), (10, 0), (10, 5)]),
            vec![(0, 0), (10, 0), (10, 5)]
        );
    }
}
