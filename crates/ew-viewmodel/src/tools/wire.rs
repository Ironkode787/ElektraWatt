// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Wire tool (`wire`; WP-S2.04 §8.2, doc 06 §4.4, FR-UI-015, FR-CON-001, FR-CON-006): a connection drawn from a
//! connection point, a junction or a point on a wire to another, routed by the pure router of §8.1 and committed as
//! `connection.draw` with the route the ghost showed.
//!
//! **Targets** in priority order within 8 screen points of the pointer or the grid cursor: a connection point of the
//! page tables, a junction, a point on a connection segment (the grid point of the segment nearest to the pointer;
//! a coordinate of the segment that is off the grid is the segment's own, the CP-exact coordinate
//! `require_split_point` accepts), else the grid point. Alt suppresses the object snaps. Ties by the smaller
//! distance, then for a CP the first in the page tables' order (natural label, its bytes, (y, x), ID; FT-439), for a
//! junction by the smaller ID (two junctions are never at one point); for a segment the larger connection ID,
//! which is later in paint order (connections paint in ID order, so the one on top); WP-S2.02's pick breaks a tie
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
//! | routing | `Press` or `Activate` on a CP, junction or segment other than the start | armed | `Commit(connection.draw {a, b, page, route})`, or `connection.draw_many` for a route through representations ([`pass_through`], FT-705) |
//! | armed | `Key(Finish)` (a double click's second press) on a CP, junction or segment | routing | as `Press` there (FT-708) |
//! | routing | `Escape` | armed | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! The committed route is the fixed vertices joined with the router's route from the last fixed vertex to the end,
//! exactly the ghost (the router never runs on replay); a free terminal in line between the last fixed vertex and the
//! end is connected through on the direct route instead when nothing else is in its way (`through_route`, FT-705
//! with FT-706). The connection kind is the page type's (doc 06 §2.2), given
//! by the host ([`WireTool::with_kind`]): a wire on a multi-line page (`connection.draw`'s default, so no `kind` is
//! sent), `overview` on single-line, overview and assembly-overview pages; on the other page types the host refuses
//! `tool.wire` until their kinds are drawn (fix round of WP-S2.04 part 2, item 1).

use ew_api::AnyCommand;
use ew_api::geometry::Dir;
use ew_api::output::Uuid;
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::routing::{RouteEnd, RouteRequest, route};
use serde_json::{Value, json};

use super::{
    Anchor, Button, CP_SNAP_PT, CanvasEvent, MarkerKind, Mods, Overlay, OverlayItem, RouteInfo, Style, Token,
    Tool, ToolCx, ToolKey, ToolOutput, segment_dist2,
};
use crate::canvas::mm3;
use crate::strings;

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
    /// (`@101-C12 F55:2 -> E55:L`, an end that is a junction by its name; FT-485 item 7: two wires can run through one
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

/// `num / den` rounded to the nearest integer, ties toward zero (`ew_units::Grid`'s rule).
fn div_round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() > den { q + num.signum() } else { q }
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
        i64::try_from(div_round(i128::from(v) * self.den, self.num)).unwrap_or(0)
    }

    /// The page coordinate of grid index `k`.
    pub fn coord(self, k: i64) -> i64 {
        i64::try_from(div_round(i128::from(k) * self.num, self.den)).unwrap_or(0)
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
    // A tie keeps the first CP in the page tables' order (natural label, its bytes, (y, x), ID; FT-439).
    let mut best: Option<(i128, &ew_api::output::TableCp)> = None;
    for cp in &tables.cps {
        let d = d2(cp.at);
        if d <= limit && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, cp));
        }
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
    // 2. A junction.
    let mut best: Option<(i128, EntityRef, (i64, i64))> = None;
    for j in &tables.junctions {
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
            let point = if a.1 == b.1 && a.0 != b.0 {
                on_axis(grid, page.0, a.0.min(b.0), a.0.max(b.0)).map(|x| (x, a.1))
            } else if a.0 == b.0 && a.1 != b.1 {
                on_axis(grid, page.1, a.1.min(b.1), a.1.max(b.1)).map(|y| (a.0, y))
            } else {
                None
            };
            let Some(p) = point.filter(|p| *p != first && *p != last) else {
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

/// The grid coordinate nearest to `v` inside `[lo, hi]`; `None` when no grid coordinate lies inside.
fn on_axis(grid: PageGridMap, v: i64, lo: i64, hi: i64) -> Option<i64> {
    let k = grid.index(v.clamp(lo, hi));
    [k, k - 1, k + 1]
        .into_iter()
        .map(|k| grid.coord(k))
        .filter(|c| (lo..=hi).contains(c))
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

/// One connection of a route that passes through representations (FT-705): from the wire's start or a CP to a CP
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

/// The pieces of a route from `start` to `end` that passes through representations (FT-705, the cycle-38 ruling):
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
/// arrival direction (FT-437), the end with its direction, and the ends' own representations (FT-706).
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

/// The margins of the router's search area around the two ends, in grid steps, tried in turn until a clear route is
/// found (FT-706): WP-S2.04 §8.1's 4, then a wider 16 for a route that has to go round a box or other wires.
const AREA_MARGINS: [i64; 2] = [4, 16];

/// The obstacle rectangles of every representation of the page, page nm (FT-706): its body and a black box's outline
/// inflated by one grid step `g`, and every text and mark the scene draws for it (the tag, the slot texts, a black
/// box's pin texts: the page's entity rectangles of the representation that are not a point) inflated by a quarter
/// step, a little clearance; the router refuses a move across a rectangle, so a text thinner than a step counts too.
/// Without `texts`, only the bodies and the black-box outlines.
fn rep_rects(cx: &ToolCx<'_>, g: i64, texts: bool) -> std::collections::BTreeMap<Uuid, Vec<[i64; 4]>> {
    let grow = |r: [i64; 4], d: i64| [r[0] - d, r[1] - d, r[2] + d, r[3] + d];
    let mut out: std::collections::BTreeMap<Uuid, Vec<[i64; 4]>> = std::collections::BTreeMap::new();
    for r in &cx.page.tables.reps {
        let rects = out.entry(r.id).or_default();
        rects.push(grow(r.body, g));
        if let Some(b) = &r.black_box {
            let xs = b.outline.iter().map(|p| p.0);
            let ys = b.outline.iter().map(|p| p.1);
            if let (Some(x0), Some(x1), Some(y0), Some(y1)) =
                (xs.clone().min(), xs.max(), ys.clone().min(), ys.max())
            {
                rects.push(grow([x0, y0, x1, y1], g));
            }
        }
    }
    if !texts {
        return out;
    }
    let clearance = g / 4;
    for (e, rect) in &cx.page.entities {
        let Some(rects) = out.get_mut(&e.0) else {
            continue;
        };
        let (a, b) = (rect.min(), rect.max());
        if a.x == b.x || a.y == b.y {
            continue;
        }
        let p = cx.snapper.sheet_to_page((a.x.0, a.y.0));
        let q = cx.snapper.sheet_to_page((b.x.0, b.y.0));
        rects.push(grow(
            [p.0.min(q.0), p.1.min(q.1), p.0.max(q.0), p.1.max(q.1)],
            clearance,
        ));
    }
    out
}

/// The obstacle rectangles of the page's frame (FT-758): the sheet outside the frame grid's drawing area, that is
/// the header bands with their labels, the title block and its text fields, in page nm, the drawing area's edge grown
/// inward by one grid step `g` as a box's outline is. On a side where one of the `ends` (the route's ends and their
/// stubs) stands within that step, the edge is not grown: an interruption point one step inside the area, leaving
/// towards the sheet's edge, turns on the area's edge as before. Empty for a page without a frame grid.
fn frame_rects(cx: &ToolCx<'_>, g: i64, ends: &[(i64, i64)]) -> Vec<[i64; 4]> {
    let Some([x0, y0, x1, y1]) = super::ip::drawing_area(cx.page) else {
        return Vec::new();
    };
    let a = cx.snapper.sheet_to_page((0, 0));
    let b = cx.snapper.sheet_to_page(cx.page.sheet);
    let (sx0, sy0, sx1, sy1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
    let inset = |near: &dyn Fn(&(i64, i64)) -> bool| if ends.iter().any(near) { 0 } else { g };
    let ix0 = x0 + inset(&|p| p.0 < x0 + g);
    let iy0 = y0 + inset(&|p| p.1 < y0 + g);
    let ix1 = x1 - inset(&|p| p.0 > x1 - g);
    let iy1 = y1 - inset(&|p| p.1 > y1 - g);
    if ix0 >= ix1 || iy0 >= iy1 {
        return Vec::new();
    }
    // Above, below, left of and right of the drawing area, each reaching a step past the sheet so that its far side
    // blocks nothing the area holds.
    vec![
        [sx0 - g, sy0 - g, sx1 + g, iy0],
        [sx0 - g, iy1, sx1 + g, sy1 + g],
        [sx0 - g, iy0, ix0, iy1],
        [ix1, iy0, sx1 + g, iy1],
    ]
}

/// What a routing pass keeps clear (FT-706). The Wire tool tries the passes in turn and takes the first clear route;
/// only when none is found does it draw the provisional L route and say so.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pass {
    /// Every rule: the other representations' bodies, black boxes and texts, the ends' own but for their escape
    /// corridors, the other wires, the other CPs and their stubs.
    Clear,
    /// As [`Pass::Clear`] without the texts: a crowded spot where only a run over a text is left.
    OverTexts,
    /// The router of WP-S2.04 §8.1 before FT-706: the other representations' bodies only.
    Bodies,
}

impl Pass {
    const ALL: [Pass; 3] = [Pass::Clear, Pass::OverTexts, Pass::Bodies];
}

/// The route request of `pass` from `from` (leaving along `from_dir`, or arrived at along `arrive`) to `to`, the area
/// left for the caller; `own` are the representations of the two ends (FT-706). The representations `through` (the
/// ones a direct route passes through, FT-705) are left out: their bodies, texts, CPs and stubs.
fn request(
    cx: &ToolCx<'_>,
    g: i64,
    pass: Pass,
    (from, from_dir, arrive): ((i64, i64), Option<Dir>, Option<Dir>),
    own: [Option<EntityRef>; 2],
    to: &WireTarget,
    through: &[Uuid],
) -> RouteRequest {
    let tables = &cx.page.tables;
    let t = to.at();
    let mut rects = rep_rects(cx, g, pass == Pass::Clear);
    let mut own_rects = own.map(|r| r.and_then(|r| rects.get(&r.0).cloned()).unwrap_or_default());
    for r in own.into_iter().flatten() {
        rects.remove(&r.0);
    }
    for r in through {
        rects.remove(r);
    }
    let mut obstacles: Vec<[i64; 4]> = rects.into_values().flatten().collect();
    let stub = |p: (i64, i64), d: Option<Dir>| {
        d.map_or(p, |d| {
            let v = d.vector();
            (p.0 + v.0 * g, p.1 + v.1 * g)
        })
    };
    let ends = [from, stub(from, from_dir), t, stub(t, to.dir())];
    obstacles.extend(frame_rects(cx, g, &ends));
    let mut wires: Vec<[(i64, i64); 2]> = Vec::new();
    // Every other CP and its stub is kept for its own wire.
    let mut reserved: Vec<(i64, i64)> = Vec::new();
    if pass == Pass::Bodies {
        own_rects = [Vec::new(), Vec::new()];
    } else {
        for c in &tables.connections {
            let line = super::autoconnect::dedup(&c.polyline);
            wires.extend(line.windows(2).map(|w| [w[0], w[1]]));
        }
        for cp in &tables.cps {
            if cp.at == from || cp.at == t || through.contains(&cp.rep) {
                continue;
            }
            reserved.push(cp.at);
            if let Some(d) = cp.dir {
                let v = d.vector();
                reserved.push((cp.at.0 + v.0 * g, cp.at.1 + v.1 * g));
            }
        }
    }
    RouteRequest {
        from: RouteEnd {
            at: from,
            dir: from_dir,
        },
        to: RouteEnd { at: t, dir: to.dir() },
        obstacles,
        grid: tables.grid,
        area: drawing_area(cx),
        arrive,
        own: own_rects,
        wires,
        reserved,
    }
}

/// The direct route of a tail from `from` to `to` when it passes through a free representation in line (FT-705 with
/// FT-706, the cycle-38 window-fix ruling): the router reserves every other CP and its stub, so it never routes into
/// a free terminal and [`pass_through`] would never see one; a terminal standing in line between the ends is to be
/// connected through, not avoided.
///
/// The direct route is the straight run between aligned ends, else the router's own provisional L
/// ([`ew_api::routing::direct_route`]). It is taken when [`pass_through`]'s rule finds at least one representation
/// passed on it (the CPs of `own` and at the two ends not counted), every passed CP lying on the route itself (not
/// beside it, a jog away), and the router's own tests
/// ([`ew_api::routing::clear`], [`Pass::Clear`]: other boxes and texts, other wires, other CPs and stubs) find it
/// clear with the passed representations' bodies, texts, CPs and stubs left out. Otherwise `None`, and the router
/// runs as before.
fn through_route(
    cx: &ToolCx<'_>,
    g: i64,
    (from, from_dir, arrive): ((i64, i64), Option<Dir>, Option<Dir>),
    own: [Option<EntityRef>; 2],
    to: &WireTarget,
) -> Option<Vec<(i64, i64)>> {
    let t = to.at();
    if from == t {
        return None;
    }
    let mut req = request(cx, g, Pass::Clear, (from, from_dir, arrive), own, to, &[]);
    let direct = clean(ew_api::routing::direct_route(&req));
    let found = passes(cx, own, [from, t], &direct)?;
    // In line: the route runs over the passed CPs themselves, not beside them.
    if found
        .passes
        .iter()
        .any(|(entry, exit)| entry.proj != entry.cp.at || exit.proj != exit.cp.at)
    {
        return None;
    }
    let mut through: Vec<Uuid> = found.passes.iter().map(|(entry, _)| entry.cp.rep).collect();
    through.sort_unstable();
    through.dedup();
    req = request(cx, g, Pass::Clear, (from, from_dir, arrive), own, to, &through);
    let xs = direct.iter().map(|p| p.0);
    let ys = direct.iter().map(|p| p.1);
    req.area = [
        xs.clone().min()? - g,
        ys.clone().min()? - g,
        xs.max()? + g,
        ys.max()? + g,
    ];
    ew_api::routing::clear(&req, &direct).then_some(direct)
}

/// The Wire tool.
#[derive(Clone, Debug)]
pub struct WireTool {
    state: State,
    /// The last routed tail, whether it is the router's fallback, and its key (one route per changed snapped point).
    cache: Option<CachedTail>,
    /// Whether the last route the tool computed is the fallback drawn direct (FT-706): the status hint says so until
    /// the next wire starts.
    direct: bool,
    /// How many times the router ran (the exact-count test, §12 item 4).
    routes: u64,
    /// The labels of the start and the end, for the transcript.
    labels: (Option<String>, Option<String>),
    /// The JSON value of `connection.draw`'s `kind` for the page type; `None` for a wire (the default).
    kind: Option<&'static str>,
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
            routes: 0,
            labels: (None, None),
            kind: None,
        }
    }

    /// The tool drawing connections of `kind` (`connection.draw`'s JSON value; `None` for a wire).
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
    /// through the cache; it sets `self.direct` to whether the tail is the router's fallback.
    ///
    /// The obstacles are every representation of the page ([`rep_rects`]); those of `own` (the start's while the
    /// tail starts at it, the end's) are the ends' own, left only along a CP's escape corridor; the other wires are
    /// every connection on the page, and the other CPs and their stubs are reserved (FT-706). The passes ([`Pass`])
    /// are tried in turn, each with the search area grown by [`AREA_MARGINS`], until a route is found. Before them,
    /// the direct route through a free terminal in line is taken when it qualifies ([`through_route`], FT-705).
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
        let g = PageGridMap::new(cx.page.tables.grid).step();
        let t = to.at();
        let page = drawing_area(cx);
        if let Some(points) = through_route(cx, g, (from, from_dir, arrive), own, to) {
            self.routes += 1;
            self.direct = false;
            self.cache = Some((key, points.clone(), false));
            return points;
        }
        let mut routed = None;
        'passes: for pass in Pass::ALL {
            let req = request(cx, g, pass, (from, from_dir, arrive), own, to, &[]);
            for m in AREA_MARGINS {
                let area = [
                    (from.0.min(t.0) - m * g).max(page[0]),
                    (from.1.min(t.1) - m * g).max(page[1]),
                    (from.0.max(t.0) + m * g).min(page[2]),
                    (from.1.max(t.1) + m * g).min(page[3]),
                ];
                let r = route(&RouteRequest { area, ..req.clone() });
                let found = !r.direct;
                routed = Some(r);
                if found {
                    break 'passes;
                }
            }
        }
        let (points, direct) = routed.map_or_else(|| (vec![from, t], true), |r| (r.points, r.direct));
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
        // After a fixed bend the router takes the last segment's direction, so it never doubles back over it
        // (FT-437).
        let arrive = match fixed {
            [.., a, b] => heading_between(*a, *b),
            _ => None,
        };
        // The start's representation is the tail's own only while the tail starts at it; after a bend it is an
        // obstacle like any other (FT-706).
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
                        text: strings::format("wire-readout", &[&mm3(length.round() as i64)]),
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
    /// representations (FT-705, [`pass_through`]) is `connection.draw_many` with one entry per piece, in one
    /// transaction; any other is one `connection.draw`.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        let State::Routing {
            start, end, route, ..
        } = &self.state
        else {
            return None;
        };
        let end = end.as_ref().filter(|e| e.is_end() && !e.same_end(start))?;
        let draw = |a: Value, b: Value, route: &[(i64, i64)]| {
            let mut params = json!({
                "a": a,
                "b": b,
                "page": cx.page.tables.page.to_string(),
                "route": route.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
            });
            if let Some(kind) = self.kind {
                params["kind"] = Value::from(kind);
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

    /// `Key(EndInIp)` (P, WP-S2.04 §9.2): ends the routing wire in a new interruption point drawn with `shape` (the
    /// IP symbol, one CP) at the end target's grid point, oriented among A–D so that its CP faces the arriving
    /// segment (the CP leaves opposite to the last segment's direction), its insertion point the grid point that puts
    /// the CP there (the route then ends exactly at the CP, re-routed when the grid rounds the CP off the point);
    /// `name_of` gives the name and the direction from the start and the insertion point. Answers the
    /// `connection.draw_to_ip` commit with the name and direction, the tool armed again; a refusal key while not
    /// routing to a grid point (`reason-ip-end-grid`), before the wire has a segment (`reason-ip-end-segment`) or for a
    /// symbol without exactly one CP (`reason-ip-symbol`).
    pub fn end_in_ip(
        &mut self,
        cx: &ToolCx<'_>,
        shape: &ew_api::output::SymbolShape,
        name_of: impl FnOnce(&WireTarget, (i64, i64)) -> (String, &'static str),
    ) -> Result<(ToolOutput, String, &'static str), &'static str> {
        let State::Routing {
            start,
            fixed,
            end,
            route,
            ..
        } = self.state.clone()
        else {
            return Err("reason-ip-end-grid");
        };
        let Some(WireTarget::Grid { at: end }) = end else {
            return Err("reason-ip-end-grid");
        };
        let [cp] = shape.cps.as_slice() else {
            return Err("reason-ip-symbol");
        };
        // The arriving direction: the last segment of the routed ghost.
        let line = super::autoconnect::dedup(&route);
        let [.., a, b] = line.as_slice() else {
            return Err("reason-ip-end-segment");
        };
        let arrive =
            Dir::of_vector(((b.0 - a.0).signum(), (b.1 - a.1).signum())).ok_or("reason-ip-end-segment")?;
        let away = Dir::of_vector((-arrive.vector().0, -arrive.vector().1)).ok_or("reason-ip-end-segment")?;
        let orient = (0..4u8)
            .find(|o| ew_api::geometry::orient_dir(*o, cp.dir) == away)
            .ok_or("reason-ip-symbol")?;
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
            dir: Some(away),
            label: String::new(),
        };
        let route = self.full_route(cx, &start, &fixed, &target);
        let (name, direction) = name_of(&start, pos);
        let Some(a) = start.endpoint() else {
            return Err("reason-ip-end-grid");
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
            params["kind"] = Value::from(kind);
        }
        self.state = State::Armed { target: None };
        self.cache = None;
        let cmd = Box::new(AnyCommand {
            name: "connection.draw_to_ip".into(),
            params,
        });
        Ok((ToolOutput::Commit(cmd), name, direction))
    }

    /// The start of the wire being routed.
    pub fn routing_start(&self) -> Option<&WireTarget> {
        match &self.state {
            State::Routing { start, .. } => Some(start),
            State::Armed { .. } => None,
        }
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
            // A double click is `Finish` in place of its second press (the window's rule, WP-S2.04 §6.1). Armed, it
            // is that second press: a click that ends a wire on a CP and a second one there at once start the next
            // wire from that CP, as two slower clicks do (FT-708: the wire was silently not drawn). Routing, it is
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

    /// FT-706: while the ghost is the router's fallback, and after a wire drawn so until the next one starts, the
    /// hint says that no clear route was found and the wire is drawn direct (not a refusal).
    fn status_hint(&self) -> &'static str {
        if self.direct {
            return "hint-wire-direct";
        }
        match self.state {
            State::Armed { .. } => "hint-wire-armed",
            State::Routing { .. } => "hint-wire-routing",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    /// A double click ends the wire on its target and arms the next one there (FT-708; doc 16's keymap).
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

    #[test]
    fn clean_removes_duplicates_and_collinear_vertices() {
        assert_eq!(
            clean(vec![(0, 0), (0, 0), (5, 0), (10, 0), (10, 5)]),
            vec![(0, 0), (10, 0), (10, 5)]
        );
    }
}
