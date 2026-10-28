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
//! | routing | `Press` or `Activate` on a CP, junction or segment other than the start | armed | `Commit(connection.draw {a, b, page, route})` |
//! | routing | `Escape` | armed | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! The committed route is the fixed vertices joined with the router's route from the last fixed vertex to the end,
//! exactly the ghost (the router never runs on replay). The connection kind is the page type's (doc 06 §2.2), given
//! by the host ([`WireTool::with_kind`]): a wire on a multi-line page (`connection.draw`'s default, so no `kind` is
//! sent), `overview` on single-line, overview and assembly-overview pages; on the other page types the host refuses
//! `tool.wire` until their kinds are drawn (fix round of WP-S2.04 part 2, item 1).

use ew_api::AnyCommand;
use ew_api::geometry::Dir;
use ew_api::output::ew_dl::EntityRef;
use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::routing::{RouteEnd, RouteRequest, auto_route};
use serde_json::{Value, json};

use super::{
    Anchor, Button, CP_SNAP_PT, CanvasEvent, MarkerKind, Mods, Overlay, OverlayItem, RouteInfo, Style, Token,
    Tool, ToolCx, ToolOutput, segment_dist2,
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

/// The key of a computed route: the page content's revision, the last fixed vertex with its leaving direction and the
/// end with its direction.
type RouteKey = (u64, (i64, i64), Option<Dir>, (i64, i64), Option<Dir>);

/// The Wire tool.
#[derive(Clone, Debug)]
pub struct WireTool {
    state: State,
    /// The last routed tail and its key (one `auto_route` per changed snapped point).
    cache: Option<(RouteKey, Vec<(i64, i64)>)>,
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

    /// The router's tail from `from` (leaving along `from_dir`) to the end target, through the cache.
    fn tail(
        &mut self,
        cx: &ToolCx<'_>,
        from: (i64, i64),
        from_dir: Option<Dir>,
        skip: [Option<EntityRef>; 2],
        to: &WireTarget,
    ) -> Vec<(i64, i64)> {
        let key: RouteKey = (cx.page.rev, from, from_dir, to.at(), to.dir());
        if let Some((k, r)) = &self.cache
            && *k == key
        {
            return r.clone();
        }
        let tables = &cx.page.tables;
        let grid = PageGridMap::new(tables.grid);
        let g = grid.step();
        let obstacles: Vec<[i64; 4]> = tables
            .reps
            .iter()
            .filter(|r| !skip.contains(&Some(EntityRef(r.id))))
            .map(|r| [r.body[0] - g, r.body[1] - g, r.body[2] + g, r.body[3] + g])
            .collect();
        let t = to.at();
        let page = drawing_area(cx);
        let area = [
            (from.0.min(t.0) - 4 * g).max(page[0]),
            (from.1.min(t.1) - 4 * g).max(page[1]),
            (from.0.max(t.0) + 4 * g).min(page[2]),
            (from.1.max(t.1) + 4 * g).min(page[3]),
        ];
        let route = auto_route(&RouteRequest {
            from: RouteEnd {
                at: from,
                dir: from_dir,
            },
            to: RouteEnd { at: t, dir: to.dir() },
            obstacles,
            grid: tables.grid,
            area,
        });
        self.routes += 1;
        self.cache = Some((key, route.clone()));
        route
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
        let skip = [start.rep(), to.rep()];
        let tail = self.tail(cx, last, from_dir, skip, to);
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

    /// The `connection.draw` of the current route to `end`; `None` without a valid end.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        let State::Routing {
            start, end, route, ..
        } = &self.state
        else {
            return None;
        };
        let end = end.as_ref().filter(|e| e.is_end() && !e.same_end(start))?;
        let mut params = json!({
            "a": start.endpoint()?,
            "b": end.endpoint()?,
            "page": cx.page.tables.page.to_string(),
            "route": route.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
        });
        if let Some(kind) = self.kind {
            params["kind"] = Value::from(kind);
        }
        Some(Box::new(AnyCommand {
            name: "connection.draw".into(),
            params,
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
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "hint-wire-armed",
            State::Routing { .. } => "hint-wire-routing",
        }
    }

    fn wants_hover(&self) -> bool {
        false
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
