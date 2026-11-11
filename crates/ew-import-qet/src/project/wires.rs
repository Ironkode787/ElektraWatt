// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Conductors → connections and junctions (doc 14 §4.7).
//!
//! A conductor end names its element by UUID (`element{i}`) and its terminal by a definition terminal UUID or
//! by a per-folio integer terminal ID; the element mapper keys placed elements with
//! [`element_keys`]/[`terminal_ids`], so this module resolves ends against the same keys. A conductor with an
//! end that does not resolve to a connection point (or a junction) is dropped with EW-IMP-003; the others
//! become connections whose IDs derive from the pair of end keys, so a re-import reproduces them. A connection's
//! route is the conductor's stored path, else QET's automatic path ([`wire_route`]).

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_interop_api::{Code, Severity};
use ew_mltext::Ident;
use ew_schema::{
    Connection, ConnectionKind, Dir4, Endpoint, Junction, JunctionStyle, JunctionWiring, NumberSpec,
    WireOverrides,
};
use ew_units::{Grid, Nm, Pt2};
use serde_json::Value;

use super::cx::{ElementKey, Findings, Ids, Placed, Registry, Target, element_keys, terminal_ids};
use super::model::{End, QetConductor, QetDiagram, QetProject, Segment, TerminalRef};

/// What the conductor mapper emits.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct WireOut {
    /// The connections, in diagram then conductor document order.
    pub connections: Vec<Connection>,
    /// The junctions, one per [`super::cx::Target::Junction`] element, in registry key order.
    pub junctions: Vec<Junction>,
    /// Conductors dropped because an end did not resolve or both ends resolved to the same CP.
    pub lost: u32,
}

/// The number of conductor ends attached to each placed element, by element key, before mapping: the
/// element mapper turns a junction element (one terminal, at least three conductors) into a junction.
pub fn conductor_counts(project: &QetProject) -> BTreeMap<ElementKey, u32> {
    let mut counts = BTreeMap::new();
    for d in &project.diagrams {
        let keys = element_keys(d);
        let known: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
        let tids = terminal_ids(d, &keys);
        for c in &d.conductors {
            for end in [&c.end1, &c.end2] {
                let key = match (&end.element, &end.terminal) {
                    (Some(u), _) => known.contains(u.as_str()).then(|| u.clone()),
                    (None, TerminalRef::Id(id)) => tids.get(id.trim()).map(|(k, _)| k.clone()),
                    (None, _) => None,
                };
                if let Some(key) = key {
                    *counts.entry((d.index, key)).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

/// The lookups of one diagram for resolving conductor ends.
struct Lookup<'a> {
    diagram: &'a QetDiagram,
    /// Element key → element index.
    by_key: BTreeMap<&'a str, usize>,
    /// Terminal ID → (element key, terminal index in the instance list).
    tids: &'a BTreeMap<String, (String, usize)>,
}

/// A resolved conductor end.
struct Resolved<'a> {
    /// The definition terminal index.
    index: usize,
    placed: &'a Placed,
    endpoint: Endpoint,
}

/// An unresolved conductor end: the reason and the placed element when one was found.
struct Unresolved<'a> {
    reason: &'static str,
    placed: Option<&'a Placed>,
}

/// One candidate binding of a conductor end.
type EndResult<'a> = Result<Resolved<'a>, Unresolved<'a>>;

fn unresolved<'a>(reason: &'static str, placed: Option<&'a Placed>) -> Unresolved<'a> {
    Unresolved { reason, placed }
}

/// The definition terminal index of the instance terminal `inst` of element `key`: the unique definition
/// terminal at the instance terminal's coordinates, else the list position.
fn index_of_id(lk: &Lookup<'_>, placed: &Placed, key: &str, inst: usize) -> Option<usize> {
    let terminal = lk
        .by_key
        .get(key)
        .and_then(|&e| lk.diagram.elements.get(e))
        .and_then(|e| e.terminals.get(inst));
    if let Some((Some(x), Some(y))) = terminal.map(|t| (t.x, t.y))
        && let (Ok(x), Ok(y)) = (x.px_to_nm(), y.px_to_nm())
    {
        let want = Pt2::new(x, y);
        let mut matching = placed
            .terminals
            .iter()
            .enumerate()
            .filter(|(_, t)| t.local == want);
        if let (Some((j, _)), None) = (matching.next(), matching.next()) {
            return Some(j);
        }
    }
    (inst < placed.terminals.len()).then_some(inst)
}

/// The endpoint of the terminal `index` of `placed`.
fn bind(placed: &Placed, index: usize) -> EndResult<'_> {
    let endpoint = match &placed.target {
        Target::Function { function, cps } => match cps.get(index) {
            Some(Some(idx)) => Endpoint::Cp(*function, *idx),
            _ => {
                return Err(unresolved("terminal maps to no connection point", Some(placed)));
            }
        },
        Target::Junction { id, .. } => Endpoint::Junction(*id),
        Target::Dropped => return Err(unresolved("element was not imported", Some(placed))),
    };
    Ok(Resolved {
        index,
        placed,
        endpoint,
    })
}

/// Resolves the terminal of an end that names the element `key` (the placed element `placed`).
fn resolve_at<'a>(terminal: &TerminalRef, lk: &Lookup<'_>, key: &str, placed: &'a Placed) -> EndResult<'a> {
    let index = match terminal {
        TerminalRef::Uuid(t) => placed
            .terminals
            .iter()
            .position(|p| p.uuid.as_deref() == Some(t.as_str()))
            .ok_or_else(|| unresolved("terminal not found", Some(placed)))?,
        TerminalRef::Id(id) => {
            let inst = match lk.tids.get(id.trim()) {
                Some((k, inst)) if k == key => *inst,
                _ => {
                    return Err(unresolved(
                        "terminal id does not belong to the element",
                        Some(placed),
                    ));
                }
            };
            index_of_id(lk, placed, key, inst)
                .ok_or_else(|| unresolved("terminal not found", Some(placed)))?
        }
        TerminalRef::Missing | TerminalRef::Other(_) => {
            if placed.terminals.len() == 1 {
                0
            } else {
                return Err(unresolved("terminal not specified", Some(placed)));
            }
        }
    };
    bind(placed, index)
}

/// The most position probes a conductor whose two ends both name repeated UUIDs may make; beyond it the
/// first copies are bound with EW-IMP-120, so no conductor costs more than a constant (doc 14 §11).
pub const MAX_PAIR_PROBES: usize = 16;

/// The copies of every UUID repeated on one folio and their terminal-position indexes, built once per folio
/// and UUID (and terminal reference), never per conductor.
struct CopyIndex {
    /// UUID → the element keys of its copies in document order (`u`, `u#2`, …), for UUIDs with ≥ 2 copies.
    copies: BTreeMap<String, Vec<String>>,
    /// (UUID, terminal reference) → the index of that terminal over the copies.
    positions: BTreeMap<(String, String), Positions>,
}

/// The copies of one UUID whose terminal resolves, and where that terminal sits on the page.
struct Positions {
    /// Copy numbers (positions in `CopyIndex::copies`) that resolve, in document order.
    resolving: Vec<usize>,
    /// Page position → copy numbers there.
    by_pos: BTreeMap<Pt2, Vec<usize>>,
}

impl CopyIndex {
    /// The repeated UUIDs of the folio `keys` (its [`element_keys`]).
    fn new(keys: &[String]) -> CopyIndex {
        let mut copies: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for k in keys {
            if k.starts_with('#') {
                continue;
            }
            let u = k.split('#').next().unwrap_or(k);
            copies.entry(u.to_owned()).or_default().push(k.clone());
        }
        copies.retain(|_, v| v.len() > 1);
        CopyIndex {
            copies,
            positions: BTreeMap::new(),
        }
    }

    /// The position index of terminal `terminal` over the copies of `u`, built on first use: one resolution
    /// and one insertion per copy, counted in `comparisons`.
    fn positions(
        &mut self,
        u: &str,
        terminal: &TerminalRef,
        lk: &Lookup<'_>,
        reg: &Registry,
        comparisons: &mut u64,
    ) -> &Positions {
        let tkey = format!("{terminal:?}");
        let copies = self.copies.get(u).map(Vec::as_slice).unwrap_or_default();
        self.positions.entry((u.to_owned(), tkey)).or_insert_with(|| {
            let mut resolving = Vec::new();
            let mut by_pos: BTreeMap<Pt2, Vec<usize>> = BTreeMap::new();
            for (n, key) in copies.iter().enumerate() {
                *comparisons += 1;
                let Some(placed) = reg.get(&(lk.diagram.index, key.clone())) else {
                    continue;
                };
                if let Ok(r) = resolve_at(terminal, lk, key, placed) {
                    resolving.push(n);
                    if let Some(pos) = end_pos(&r) {
                        by_pos.entry(pos).or_default().push(n);
                    }
                }
            }
            Positions { resolving, by_pos }
        })
    }
}

/// One conductor end before a copy is chosen.
enum Cand<'a> {
    /// Decided (or failed).
    One(EndResult<'a>),
    /// The end names the UUID `u`, repeated on the folio, with terminal reference `terminal`.
    Copies { u: String, terminal: TerminalRef },
}

/// The candidate of a conductor end: decided, unless it names a UUID repeated on the folio.
fn resolve_end<'a>(end: &End, lk: &Lookup<'_>, reg: &'a Registry, ix: &CopyIndex) -> Cand<'a> {
    match (&end.element, &end.terminal) {
        (Some(u), terminal) => {
            if ix.copies.contains_key(u) && !matches!(terminal, TerminalRef::Id(_)) {
                return Cand::Copies {
                    u: u.clone(),
                    terminal: terminal.clone(),
                };
            }
            match reg.get(&(lk.diagram.index, u.clone())) {
                Some(placed) => Cand::One(resolve_at(terminal, lk, u, placed)),
                None => Cand::One(Err(unresolved("element not found", None))),
            }
        }
        (None, TerminalRef::Id(id)) => {
            let Some((key, inst)) = lk.tids.get(id.trim()) else {
                return Cand::One(Err(unresolved("terminal id not found", None)));
            };
            let Some(placed) = reg.get(&(lk.diagram.index, key.clone())) else {
                return Cand::One(Err(unresolved("element not found", None)));
            };
            let Some(index) = index_of_id(lk, placed, key, *inst) else {
                return Cand::One(Err(unresolved("terminal not found", Some(placed))));
            };
            Cand::One(bind(placed, index))
        }
        (None, _) => Cand::One(Err(unresolved("no element and no terminal id", None))),
    }
}

/// The page position of a resolved end, if known.
fn end_pos(r: &Resolved<'_>) -> Option<Pt2> {
    match &r.placed.target {
        Target::Junction { at, .. } => Some(*at),
        _ => r.placed.terminals.get(r.index).and_then(|t| t.page_pos),
    }
}

/// The page position of a resolved end with the direction a wire leaves it (a junction's: its terminal's).
fn end_at(r: &Resolved<'_>) -> Option<(Pt2, Option<Dir4>)> {
    let dir = r.placed.terminals.get(r.index).and_then(|t| t.dir.clone());
    end_pos(r).map(|p| (p, dir))
}

/// The points of a conductor drawn from `start` through `segments`: the point after each segment. `None` when
/// a length does not convert or the arithmetic overflows.
fn points_of(start: Pt2, segments: &[Segment]) -> Option<Vec<Pt2>> {
    let mut p = start;
    let mut points = Vec::with_capacity(segments.len());
    for s in segments {
        let len = s.length.px_to_nm().ok()?;
        p = if s.horizontal {
            Pt2::new(p.x.checked_add(len)?, p.y)
        } else {
            Pt2::new(p.x, p.y.checked_add(len)?)
        };
        points.push(p);
    }
    Some(points)
}

/// How far, in nm, the page position of a terminal on a whole QET px may lie from it: 1 nm from adding the rounded
/// positions of element and terminal, 2 nm for an element nudged off a grid line (doc 14 §4.2). Two terminals QET
/// draws on one line can therefore be that far apart across it, so a segment that leaves an end may slant by as much.
const ROUNDING: i64 = 2;

/// The whole QET px of `v` when `v` is within [`ROUNDING`] of it, else `v`.
fn whole_px(v: Nm) -> Nm {
    Grid::QET_1PX
        .snap_point(v)
        .filter(|s| (i128::from(s.0) - i128::from(v.0)).abs() <= i128::from(ROUNDING))
        .unwrap_or(v)
}

/// The point `p` with each coordinate at its [`whole_px`].
fn whole(p: Pt2) -> Pt2 {
    Pt2::new(whole_px(p.x), whole_px(p.y))
}

/// The route through `points` (from `a`'s [`whole`] point to `b`'s), [`tidy`], with its first and last vertices put
/// back at the ends' exact points `a` and `b`.
fn finish(points: impl IntoIterator<Item = Pt2>, a: Pt2, b: Pt2) -> Vec<Pt2> {
    let mut route = tidy(points);
    if let Some(first) = route.first_mut() {
        *first = a;
    }
    if let Some(last) = route.last_mut() {
        *last = b;
    }
    route
}

/// The stored path of a conductor drawn from end 1 at `a` through `segments` to end 2 at `b`, both ends
/// included: `a`, the point after each segment but the last, then `b`. A corner coordinate that names the same
/// QET px as an end's coordinate takes that end's [`whole_px`] value (the tables importer's rule), any other the
/// whole QET px nearest to it, and the route is [`finish`]ed. `None` (QET draws its automatic path) without segments,
/// when the segments do not end within 1 px of `b` on both axes (QET's own coherence test,
/// `Conductor::pathFromXml`), a length does not convert or the arithmetic overflows.
fn stored_route(a: Pt2, b: Pt2, segments: &[Segment]) -> Option<Vec<Pt2>> {
    let mut points = points_of(a, segments)?;
    let last = points.pop()?;
    // 3 × |Δ| nm ≤ 793 750: within one px (1 px = 793 750 / 3 nm).
    let near = |u: Nm, v: Nm| (i128::from(u.0) - i128::from(v.0)).abs() * 3 <= 793_750;
    if !(near(last.x, b.x) && near(last.y, b.y)) {
        return None;
    }
    let (wa, wb) = (whole(a), whole(b));
    let px = |v: Nm| Grid::QET_1PX.snap(v);
    let snap = |v: Nm, ends: [Nm; 2]| {
        ends.into_iter()
            .find(|e| px(*e) == px(v))
            .or_else(|| Grid::QET_1PX.snap_point(v))
            .unwrap_or(v)
    };
    let corners = points
        .into_iter()
        .map(|p| Pt2::new(snap(p.x, [wa.x, wb.x]), snap(p.y, [wa.y, wb.y])));
    let points = std::iter::once(wa).chain(corners).chain(std::iter::once(wb));
    Some(finish(points, a, b))
}

/// `points` without a vertex equal to the one before it and without the middle vertex of three on one horizontal or
/// vertical line: a spur that runs out and back (QET draws it over itself) goes, and a straight run is one segment.
fn tidy(points: impl IntoIterator<Item = Pt2>) -> Vec<Pt2> {
    let inline = |a: Pt2, b: Pt2, c: Pt2| (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y);
    let mut out: Vec<Pt2> = Vec::new();
    for p in points {
        while let [.., a, b] = out[..] {
            if !inline(a, b, p) {
                break;
            }
            out.pop();
        }
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    out
}

/// Why `route` (from `a` to `b`, [`tidy`]) is no route `connection.route` accepts on a QET page
/// (`ew_commands::geometry::require_route`), in words, and is not drawn right-angled; `None` when it is one: every
/// segment horizontal or vertical (within [`ROUNDING`]), every coordinate on the 10 px grid or equal to that
/// coordinate of an end, and no segment running over another.
fn route_problem(route: &[Pt2], a: Pt2, b: Pt2) -> Option<&'static str> {
    let level = |u: Nm, v: Nm| (i128::from(u.0) - i128::from(v.0)).abs() <= i128::from(ROUNDING);
    if route
        .windows(2)
        .any(|s| !level(s[0].x, s[1].x) && !level(s[0].y, s[1].y))
    {
        return Some("a segment is neither horizontal nor vertical");
    }
    let g = Grid::QET_10PX;
    let fits = |v: Nm, ends: [Nm; 2]| ends.contains(&v) || g.snap_point(v) == Some(v);
    if !route
        .iter()
        .all(|p| fits(p.x, [a.x, b.x]) && fits(p.y, [a.y, b.y]))
    {
        return Some("a corner is off the 10 px grid");
    }
    // Per line (horizontal or not, its constant coordinate at the whole px), the segments' intervals in order: one
    // that starts before the furthest end so far runs over an earlier one.
    let mut lines: BTreeMap<(bool, i64), Vec<(i64, i64)>> = BTreeMap::new();
    for s in route.windows(2) {
        let (h, at, u, v) = if level(s[0].y, s[1].y) {
            (true, whole_px(s[0].y).0, s[0].x.0, s[1].x.0)
        } else {
            (false, whole_px(s[0].x).0, s[0].y.0, s[1].y.0)
        };
        lines.entry((h, at)).or_default().push((u.min(v), u.max(v)));
    }
    for spans in lines.values_mut() {
        spans.sort_unstable();
        let mut reach = i64::MIN;
        for (lo, hi) in spans.iter() {
            if *lo < reach {
                return Some("a segment runs over another");
            }
            reach = reach.max(*hi);
        }
    }
    None
}

/// A coordinate of QET's automatic path: nanometres for the route, hundredths of a QET px for QET's decisions.
#[derive(Clone, Copy, Debug)]
struct Coord {
    nm: Nm,
    cpx: i64,
}

/// `n / d` rounded half away from zero (C's `std::round` and Qt 6's `qRound`), `d > 0`.
fn div_round(n: i64, d: i64) -> i64 {
    let (q, r) = (n / d, n % d);
    if 2 * r.abs() >= d { q + n.signum() } else { q }
}

impl Coord {
    /// A page coordinate: exact in nm, in hundredths of a px rounded.
    fn of(v: Nm) -> Coord {
        let n = i128::from(v.0) * 300;
        let q = (n + n.signum() * 793_750 / 2) / 793_750;
        Coord {
            nm: v,
            // |q| ≤ |v|: always fits.
            cpx: i64::try_from(q).unwrap_or_default(),
        }
    }

    /// The point of the 10 px grid with index `k`; `None` on overflow.
    fn grid(k: i64) -> Option<Coord> {
        Some(Coord {
            nm: Grid::QET_10PX.at(k)?,
            cpx: k.checked_mul(1000)?,
        })
    }

    /// `std::round((self ± 10 px) / 10) × 10`: QET's `Conductor::extendTerminal`.
    fn extended(self, by: i64) -> Option<Coord> {
        Coord::grid(div_round(self.cpx.checked_add(by * 100)?, 1000))
    }

    /// QET's whole px of this coordinate (`qRound`).
    fn px(self) -> i64 {
        div_round(self.cpx, 100)
    }

    /// The bridge line between `self` and `other`: `qRound(self + other) / 2`, then down to a multiple of 10 px.
    fn bridge(self, other: Coord) -> Option<Coord> {
        let mid = div_round(self.cpx.checked_add(other.cpx)?, 100) / 2;
        Coord::grid(mid.div_euclid(10))
    }
}

/// QET's automatic path of a conductor from the terminal at `p1` leaving towards `o1` to the terminal at `p2`
/// leaving towards `o2` (QElectroTech, `sources/qetgraphicsitem/conductor.cpp`, `Conductor::generateConductorPath`
/// and `Conductor::extendTerminal`, GPL-2.0-or-later, rewritten here): both terminals are extended by 10 px onto the
/// 10 px grid, the path runs from the left extension to the right one by one corner or a two-corner bridge on the
/// 10 px grid (QET's cases 1 to 4, which depend on whether it runs down or up and on the two orientations), and is
/// expressed from terminal 1 to terminal 2. It is computed from the terminals' [`whole`] points and [`finish`]ed.
/// `None` for an unknown direction or on overflow.
fn auto_route(p1: Pt2, o1: &Dir4, p2: Pt2, o2: &Dir4) -> Option<Vec<Pt2>> {
    type P = (Coord, Coord);
    let extend = |(x, y): P, o: &Dir4| -> Option<P> {
        Some(match o {
            Dir4::Up => (x, y.extended(-10)?),
            Dir4::Right => (x.extended(10)?, y),
            Dir4::Down => (x, y.extended(10)?),
            Dir4::Left => (x.extended(-10)?, y),
            Dir4::Unknown(_) => return None,
        })
    };
    let (w1, w2) = (whole(p1), whole(p2));
    let (s1, s2) = (
        (Coord::of(w1.x), Coord::of(w1.y)),
        (Coord::of(w2.x), Coord::of(w2.y)),
    );
    let (n1, n2) = (extend(s1, o1)?, extend(s2, o2)?);
    // The path always runs from left to right (after the extension).
    let forward = n1.0.nm <= n2.0.nm;
    let ((start0, start, od), (end, end0, oa)) = if forward {
        ((s1, n1, o1), (n2, s2, o2))
    } else {
        ((s2, n2, o2), (n1, s1, o1))
    };
    let (n, e, s, w) = (Dir4::Up, Dir4::Right, Dir4::Down, Dir4::Left);
    let mut points: Vec<P> = vec![start0, start];
    // A vertical bridge at x between the two, unless they share a whole px of x.
    let x_bridge = |points: &mut Vec<P>| -> Option<()> {
        if start.0.px() != end.0.px() {
            let x = start.0.bridge(end.0)?;
            points.extend([(x, start.1), (x, end.1)]);
        }
        Some(())
    };
    let y_bridge = |points: &mut Vec<P>| -> Option<()> {
        if start.1.px() != end.1.px() {
            let y = start.1.bridge(end.1)?;
            points.extend([(start.0, y), (end.0, y)]);
        }
        Some(())
    };
    if start.1.nm < end.1.nm {
        // Running down.
        if (*od == n && (*oa == s || *oa == w)) || (*od == e && *oa == w) {
            x_bridge(&mut points)?;
        } else if (*od == s && (*oa == n || *oa == e)) || (*od == w && *oa == e) {
            y_bridge(&mut points)?;
        } else if (*od == n || *od == e) && (*oa == n || *oa == e) {
            points.push((end.0, start.1));
        } else {
            points.push((start.0, end.1));
        }
    } else if (*od == w && (*oa == e || *oa == s)) || (*od == n && *oa == s) {
        // Running up (or level).
        y_bridge(&mut points)?;
    } else if (*od == e && (*oa == w || *oa == n)) || (*od == s && *oa == n) {
        x_bridge(&mut points)?;
    } else if (*od == w || *od == n) && (*oa == w || *oa == n) {
        points.push((start.0, end.1));
    } else {
        points.push((end.0, start.1));
    }
    points.extend([end, end0]);
    if !forward {
        points.reverse();
    }
    Some(finish(
        points.into_iter().map(|(x, y)| Pt2::new(x.nm, y.nm)),
        p1,
        p2,
    ))
}

/// The route stored for a conductor whose ends are drawn at `a` and `b` (each with the direction a wire leaves it,
/// when known), and a report message when it is not the path QET draws: the stored path replayed from end 1
/// ([`stored_route`]), else QET's automatic path ([`auto_route`]); empty (the connection is unrouted) when neither is
/// a valid route ([`route_problem`]). Ends at one [`whole`] point (terminals that touch) get no message.
fn wire_route(
    a: Option<(Pt2, Option<Dir4>)>,
    b: Option<(Pt2, Option<Dir4>)>,
    segments: &[Segment],
) -> (Vec<Pt2>, Option<String>) {
    let unrouted = |why: &str| (Vec::new(), Some(format!("wire left unrouted: {why}")));
    let (Some((pa, da)), Some((pb, db))) = (a, b) else {
        return unrouted("an end has no position on the page");
    };
    if whole(pa) == whole(pb) {
        // Two terminals that touch: the one segment between their [`ROUNDING`] apart points, or nothing at one point
        // (EW-CORE-073 asks no route of that).
        let route = if pa == pb { Vec::new() } else { vec![pa, pb] };
        return (route, None);
    }
    let mut set_aside = None;
    if !segments.is_empty()
        && let Some(route) = stored_route(pa, pb, segments)
    {
        match route_problem(&route, pa, pb) {
            None => return (route, None),
            Some(why) => set_aside = Some(why),
        }
    }
    let (Some(da), Some(db)) = (da, db) else {
        return unrouted(set_aside.unwrap_or("the direction of a terminal is unknown"));
    };
    let Some(route) = auto_route(pa, &da, pb, &db) else {
        return unrouted(set_aside.unwrap_or("a coordinate is out of range"));
    };
    match (route_problem(&route, pa, pb), set_aside) {
        (None, None) => (route, None),
        (None, Some(why)) => (
            route,
            Some(format!("stored path not kept ({why}); QET's automatic path used")),
        ),
        (Some(why), _) => unrouted(set_aside.unwrap_or(why)),
    }
}

/// The displacement from the first to the last point of a conductor drawn through `segments`, nm; `None`
/// without segments, when a length does not convert or the sum overflows.
fn displacement(segments: &[Segment]) -> Option<(i64, i64)> {
    if segments.is_empty() {
        return None;
    }
    let (mut dx, mut dy) = (0i64, 0i64);
    for seg in segments {
        let len = seg.length.px_to_nm().ok()?.0;
        if seg.horizontal {
            dx = dx.checked_add(len)?;
        } else {
            dy = dy.checked_add(len)?;
        }
    }
    Some((dx, dy))
}

/// Resolves copy `n` of the repeated UUID `u` at `terminal`.
fn copy_at<'a>(
    u: &str,
    n: usize,
    terminal: &TerminalRef,
    lk: &Lookup<'_>,
    reg: &'a Registry,
    ix: &CopyIndex,
) -> EndResult<'a> {
    let Some(key) = ix.copies.get(u).and_then(|c| c.get(n)) else {
        return Err(unresolved("element not found", None));
    };
    match reg.get(&(lk.diagram.index, key.clone())) {
        Some(placed) => resolve_at(terminal, lk, key, placed),
        None => Err(unresolved("element not found", None)),
    }
}

/// `p` moved by `(dx, dy)` × `sign`, `None` on overflow.
fn shifted(p: Pt2, (dx, dy): (i64, i64), sign: i64) -> Option<Pt2> {
    Some(Pt2::new(
        Nm(p.x.0.checked_add(dx.checked_mul(sign)?)?),
        Nm(p.y.0.checked_add(dy.checked_mul(sign)?)?),
    ))
}

/// Picks the copy of each end that names a repeated UUID (review R1: linear overall). The copies' terminal
/// positions are indexed once per folio and UUID ([`CopyIndex`]); an end facing a decided end looks up the
/// position where the conductor's segments put it (one lookup); two ends that both name repeated UUIDs probe
/// the positions of the smaller copy set, at most [`MAX_PAIR_PROBES`]. Exactly one match decides; otherwise the
/// first resolving copy is bound and the end is flagged (the second value; EW-IMP-120). Every index insertion,
/// lookup and probe is counted in `comparisons`.
fn choose_ends<'a>(
    segments: &[Segment],
    cands: [Cand<'a>; 2],
    lk: &Lookup<'_>,
    reg: &'a Registry,
    ix: &mut CopyIndex,
    comparisons: &mut u64,
) -> ([EndResult<'a>; 2], [bool; 2]) {
    let d = displacement(segments);
    let first = |ix: &mut CopyIndex, u: &str, t: &TerminalRef, comparisons: &mut u64| -> EndResult<'a> {
        let n = ix
            .positions(u, t, lk, reg, comparisons)
            .resolving
            .first()
            .copied()
            .unwrap_or(0);
        copy_at(u, n, t, lk, reg, ix)
    };
    let [c1, c2] = cands;
    match (c1, c2) {
        (Cand::One(a), Cand::One(b)) => ([a, b], [false, false]),
        (Cand::Copies { u, terminal }, Cand::One(b)) => {
            let target = match (&b, d) {
                (Ok(rb), Some(dv)) => end_pos(rb).and_then(|p| shifted(p, dv, -1)),
                _ => None,
            };
            let pos = ix.positions(&u, &terminal, lk, reg, comparisons);
            *comparisons += 1;
            let hit = target
                .and_then(|t| pos.by_pos.get(&t))
                .filter(|v| v.len() == 1)
                .map(|v| v[0]);
            match hit {
                Some(n) => ([copy_at(&u, n, &terminal, lk, reg, ix), b], [false, false]),
                None => ([first(ix, &u, &terminal, comparisons), b], [true, false]),
            }
        }
        (Cand::One(a), Cand::Copies { u, terminal }) => {
            let target = match (&a, d) {
                (Ok(ra), Some(dv)) => end_pos(ra).and_then(|p| shifted(p, dv, 1)),
                _ => None,
            };
            let pos = ix.positions(&u, &terminal, lk, reg, comparisons);
            *comparisons += 1;
            let hit = target
                .and_then(|t| pos.by_pos.get(&t))
                .filter(|v| v.len() == 1)
                .map(|v| v[0]);
            match hit {
                Some(n) => ([a, copy_at(&u, n, &terminal, lk, reg, ix)], [false, false]),
                None => ([a, first(ix, &u, &terminal, comparisons)], [false, true]),
            }
        }
        (Cand::Copies { u: u1, terminal: t1 }, Cand::Copies { u: u2, terminal: t2 }) => {
            let mut found: Option<(usize, usize)> = None;
            let mut matches = 0usize;
            if let Some(dv) = d {
                let n1 = ix.positions(&u1, &t1, lk, reg, comparisons).resolving.len();
                let n2 = ix.positions(&u2, &t2, lk, reg, comparisons).resolving.len();
                // Probe the positions of the smaller copy set in the other's index.
                let (small, other, sign) = if n1 <= n2 {
                    ((&u1, &t1), (&u2, &t2), 1)
                } else {
                    ((&u2, &t2), (&u1, &t1), -1)
                };
                if n1.min(n2) <= MAX_PAIR_PROBES {
                    let probes: Vec<(Pt2, Vec<usize>)> = ix
                        .positions(small.0, small.1, lk, reg, comparisons)
                        .by_pos
                        .iter()
                        .map(|(p, v)| (*p, v.clone()))
                        .collect();
                    let index = ix.positions(other.0, other.1, lk, reg, comparisons);
                    for (p, ns) in probes {
                        *comparisons += 1;
                        let Some(t) = shifted(p, dv, sign) else { continue };
                        if let Some(ms) = index.by_pos.get(&t) {
                            matches += ns.len() * ms.len();
                            found = Some(if sign == 1 { (ns[0], ms[0]) } else { (ms[0], ns[0]) });
                            if matches > 1 {
                                break;
                            }
                        }
                    }
                }
            }
            match (matches, found) {
                (1, Some((n1, n2))) => (
                    [
                        copy_at(&u1, n1, &t1, lk, reg, ix),
                        copy_at(&u2, n2, &t2, lk, reg, ix),
                    ],
                    [false, false],
                ),
                _ => (
                    [first(ix, &u1, &t1, comparisons), first(ix, &u2, &t2, comparisons)],
                    [true, true],
                ),
            }
        }
    }
}

/// The source key of one conductor end: `<element key>/<terminal index>`, `/j` for a junction; the element
/// key is the project-wide one ([`Placed::ek`]).
fn end_key(r: &Resolved<'_>) -> String {
    let ek = &r.placed.ek;
    match r.placed.target {
        Target::Junction { .. } => format!("{ek}/j"),
        _ => format!("{ek}/{}", r.index),
    }
}

/// The conductor value of one text field kept as `extra` (see [`TEXT_FIELDS`]).
type FieldValue = fn(&QetConductor) -> &str;

/// The conductor properties kept as text: field name in the summary, `extra` key, value of a conductor.
const TEXT_FIELDS: [(&str, &str, FieldValue); 4] = [
    ("function", "qet.function", |c| &c.function),
    ("tension_protocol", "qet.tension_protocol", |c| {
        &c.tension_protocol
    }),
    ("conductor_color", "qet.color", |c| &c.color),
    ("conductor_section", "qet.section", |c| &c.section),
];

/// Maps every conductor of `project` against the placed elements of `reg`.
pub fn map_conductors(project: &QetProject, reg: &Registry, ids: &Ids, findings: &mut Findings) -> WireOut {
    map_conductors_counted(project, reg, ids, findings).0
}

/// [`map_conductors`], also returning the number of candidate comparisons made to bind the ends of repeated
/// UUIDs (for the structural complexity test).
fn map_conductors_counted(
    project: &QetProject,
    reg: &Registry,
    ids: &Ids,
    findings: &mut Findings,
) -> (WireOut, u64) {
    let mut comparisons = 0u64;
    let mut out = WireOut::default();
    let mut repeats: BTreeMap<String, u32> = BTreeMap::new();
    // Per text field: conductors with a value and the distinct values.
    let mut stats: [(u32, BTreeSet<String>); 4] = Default::default();

    for d in &project.diagrams {
        let keys = element_keys(d);
        let tids = terminal_ids(d, &keys);
        let lk = Lookup {
            diagram: d,
            by_key: keys.iter().enumerate().map(|(i, k)| (k.as_str(), i)).collect(),
            tids: &tids,
        };
        let mut ix = CopyIndex::new(&keys);
        let diagram_folio = reg
            .range((d.index, String::new())..)
            .next()
            .filter(|((di, _), _)| *di == d.index)
            .map(|(_, p)| p.folio.as_str());

        for (n, c) in d.conductors.iter().enumerate() {
            let item = || Some(format!("conductor {n}"));
            let cands = [
                resolve_end(&c.end1, &lk, reg, &ix),
                resolve_end(&c.end2, &lk, reg, &ix),
            ];
            let (ends, repeated) = choose_ends(&c.segments, cands, &lk, reg, &mut ix, &mut comparisons);
            let failure = ends
                .iter()
                .enumerate()
                .find_map(|(i, r)| r.as_ref().err().map(|e| (i + 1, e)));
            if let Some((which, e)) = failure {
                let folio = e.placed.map(|p| p.folio.as_str()).or(diagram_folio);
                let loc = findings.loc(folio, item(), Some(c.line));
                findings.push(
                    Code::UNRESOLVED_REFERENCE,
                    Severity::Error,
                    loc,
                    format!("conductor dropped: end {which}: {}", e.reason),
                );
                out.lost += 1;
                continue;
            }
            let [Ok(e1), Ok(e2)] = ends else {
                continue;
            };
            let folio = Some(e1.placed.folio.as_str());
            if e1.endpoint == e2.endpoint {
                let loc = findings.loc(folio, item(), Some(c.line));
                findings.push(
                    Code::UNRESOLVED_REFERENCE,
                    Severity::Error,
                    loc,
                    "conductor dropped: both ends resolve to the same connection point",
                );
                out.lost += 1;
                continue;
            }

            for (i, (rep, e)) in repeated.iter().zip([&e1, &e2]).enumerate() {
                if *rep {
                    let loc = findings.loc(Some(e.placed.folio.as_str()), item(), Some(c.line));
                    findings.push(
                        Code::imp(120),
                        Severity::Warning,
                        loc,
                        format!(
                            "conductor end {}: element UUID repeated on this folio; bound to the first copy (geometry does not decide)",
                            i + 1
                        ),
                    );
                }
            }

            let (mut k1, mut k2) = (end_key(&e1), end_key(&e2));
            if k2 < k1 {
                std::mem::swap(&mut k1, &mut k2);
            }
            let base = format!("qet:wire:{k1}|{k2}");
            let seen = repeats.entry(base.clone()).or_insert(0);
            *seen += 1;
            let key = if *seen == 1 {
                base
            } else {
                format!("{base}#{seen}")
            };

            let mut extra = BTreeMap::new();
            for (i, (_, extra_key, value)) in TEXT_FIELDS.iter().enumerate() {
                let v = value(c).trim();
                if !v.is_empty() {
                    extra.insert((*extra_key).to_owned(), Value::String(v.to_owned()));
                    stats[i].0 += 1;
                    stats[i].1.insert(v.to_owned());
                }
            }
            let number = match c.num.trim() {
                "" => NumberSpec::Unassigned,
                num => match Ident::from_input(num) {
                    Ok(ident) => NumberSpec::Frozen(ident),
                    Err(_) => {
                        let loc = findings.loc(folio, item(), Some(c.line));
                        findings.push(
                            Code::FOREIGN_VALUE,
                            Severity::Warning,
                            loc,
                            format!("wire number {num:?} kept as text"),
                        );
                        extra.insert("qet.num".to_owned(), Value::String(num.to_owned()));
                        NumberSpec::Unassigned
                    }
                },
            };
            let (route, note) = wire_route(end_at(&e1), end_at(&e2), &c.segments);
            if let Some(note) = note {
                let loc = findings.loc(folio, item(), Some(c.line));
                findings.push(Code::imp(9), Severity::Warning, loc, note);
            }

            out.connections.push(Connection {
                id: ids.id(&key),
                v: 1,
                a: e1.endpoint.clone(),
                b: e2.endpoint.clone(),
                page: Some(e1.placed.page),
                route,
                kind: ConnectionKind::default(),
                overrides: WireOverrides::default(),
                number,
                source_ref: Some(ids.source_ref(&key)),
                extra,
            });
        }
    }

    for ((name, _, _), (count, distinct)) in TEXT_FIELDS.iter().zip(&stats) {
        if *count > 0 {
            findings.file_level(
                Code::FOREIGN_VALUE,
                Severity::Warning,
                format!(
                    "conductor {name} kept as text on {count} conductors ({} distinct values); not mapped to wire properties",
                    distinct.len()
                ),
            );
        }
    }

    let mut at_junction: BTreeMap<Id<Junction>, Vec<Id<Connection>>> = BTreeMap::new();
    for c in &out.connections {
        for e in [&c.a, &c.b] {
            if let Endpoint::Junction(j) = e {
                at_junction.entry(*j).or_default().push(c.id);
            }
        }
    }
    for placed in reg.values() {
        let Target::Junction { id, key, at } = &placed.target else {
            continue;
        };
        out.junctions.push(Junction {
            id: *id,
            v: 1,
            page: placed.page,
            at: *at,
            style: JunctionStyle::Dot,
            wiring: Some(JunctionWiring::Star),
            sequence: at_junction.get(id).cloned().unwrap_or_default(),
            source_ref: Some(ids.source_ref(key)),
            extra: BTreeMap::new(),
        });
        let loc = findings.loc(Some(&placed.folio), Some(key.clone()), None);
        findings.push(
            Code::imp(118),
            Severity::Info,
            loc,
            "junction wiring assumed (star) at a QET junction element",
        );
    }
    (out, comparisons)
}

#[cfg(test)]
mod tests {
    use ew_ids::Uuid;
    use ew_interop_api::{Code, Severity};
    use ew_schema::{Function, Page};
    use ew_units::Nm;
    use serde_json::json;

    use super::*;
    use crate::decimal::Decimal;
    use crate::project::cx::PlacedTerminal;
    use crate::project::model::{InstTerminal, QetElement};

    fn uid(n: u128) -> String {
        Uuid::from_u128(n).to_string()
    }

    fn ids() -> Ids {
        Ids {
            namespace: Uuid::from_u128(1),
            lineage: Id::from_uuid(Uuid::from_u128(2)),
        }
    }

    fn function(n: u128) -> Id<Function> {
        Id::from_uuid(Uuid::from_u128(n))
    }

    fn px(n: i64) -> Nm {
        Decimal::from_int(n).px_to_nm().unwrap()
    }

    fn inst(id: &str, xy: Option<(i64, i64)>) -> InstTerminal {
        InstTerminal {
            id: Some(id.to_owned()),
            x: xy.map(|(x, _)| Decimal::from_int(x)),
            y: xy.map(|(_, y)| Decimal::from_int(y)),
        }
    }

    fn element(uuid: Option<String>, terminals: Vec<InstTerminal>) -> QetElement {
        QetElement {
            n: 0,
            type_path: "embed://t.elmt".to_owned(),
            uuid,
            x: Decimal::ZERO,
            y: Decimal::ZERO,
            orientation: 0,
            prefix: String::new(),
            terminals,
            infos: Vec::new(),
            dyn_texts: Vec::new(),
            links: Vec::new(),
            line: 1,
        }
    }

    /// A placed terminal at definition position `(x, y)` px; its page position is offset by (1000, 2000) nm.
    fn pterm(uuid: Option<String>, x: i64, y: i64) -> PlacedTerminal {
        let local = Pt2::new(px(x), px(y));
        PlacedTerminal {
            uuid,
            local,
            page_pos: Some(Pt2::new(Nm(local.x.0 + 1000), Nm(local.y.0 + 2000))),
            dir: Some(Dir4::Down),
        }
    }

    /// A placed element of folio F1 with the project-wide element key `ek`.
    fn placed(ek: &str, target: Target, terminals: Vec<PlacedTerminal>) -> Placed {
        Placed {
            diagram: 0,
            page: Id::<Page>::from_uuid(Uuid::from_u128(9)),
            folio: "F1".to_owned(),
            ek: ek.to_owned(),
            target,
            terminals,
        }
    }

    fn func(n: u128, cps: Vec<Option<u16>>) -> Target {
        Target::Function {
            function: function(n),
            cps,
        }
    }

    fn end_u(element: &str, terminal: &str) -> End {
        End {
            element: Some(element.to_owned()),
            terminal: TerminalRef::Uuid(terminal.to_owned()),
        }
    }

    fn end_id(id: &str) -> End {
        End {
            element: None,
            terminal: TerminalRef::Id(id.to_owned()),
        }
    }

    fn cond(end1: End, end2: End) -> QetConductor {
        QetConductor {
            end1,
            end2,
            num: String::new(),
            function: String::new(),
            tension_protocol: String::new(),
            color: String::new(),
            section: String::new(),
            segments: Vec::new(),
            line: 7,
        }
    }

    fn diagram(elements: Vec<QetElement>, conductors: Vec<QetConductor>) -> QetDiagram {
        QetDiagram {
            elements,
            conductors,
            ..QetDiagram::default()
        }
    }

    fn project(diagrams: Vec<QetDiagram>) -> QetProject {
        QetProject {
            diagrams,
            ..QetProject::default()
        }
    }

    fn run(p: &QetProject, reg: &Registry) -> (WireOut, Findings) {
        let mut f = Findings::new("t.qet", 100);
        let out = map_conductors(p, reg, &ids(), &mut f);
        (out, f)
    }

    fn codes(f: &Findings) -> Vec<(Code, Severity)> {
        f.entries().iter().map(|e| (e.code, e.severity)).collect()
    }

    /// Two uuid elements A (terminals ta0, ta1) and B (terminal tb0), with registry.
    fn two_elements() -> (QetDiagram, Registry) {
        let (a, b) = (uid(10), uid(11));
        let d = diagram(
            vec![
                element(
                    Some(a.clone()),
                    vec![inst("1", Some((0, 0))), inst("2", Some((10, 0)))],
                ),
                element(Some(b.clone()), vec![inst("3", Some((0, 0)))]),
            ],
            Vec::new(),
        );
        let mut reg = Registry::new();
        reg.insert(
            (0, a.clone()),
            placed(
                &a,
                func(100, vec![Some(0), Some(1)]),
                vec![pterm(Some(uid(20)), 0, 0), pterm(Some(uid(21)), 10, 0)],
            ),
        );
        reg.insert(
            (0, b.clone()),
            placed(&b, func(101, vec![Some(0)]), vec![pterm(Some(uid(22)), 0, 0)]),
        );
        (d, reg)
    }

    #[test]
    fn uuid_ends_resolve_to_cp_endpoints() {
        let (mut d, reg) = two_elements();
        d.conductors
            .push(cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0);
        assert!(f.entries().is_empty());
        assert_eq!(out.connections.len(), 1);
        let c = &out.connections[0];
        assert_eq!(c.a, Endpoint::Cp(function(100), 1));
        assert_eq!(c.b, Endpoint::Cp(function(101), 0));
        assert_eq!(c.page, Some(Id::from_uuid(Uuid::from_u128(9))));
        let key = format!("qet:wire:{}/1|{}/0", uid(10), uid(11));
        assert_eq!(c.source_ref.as_ref().unwrap().key, key);
        assert_eq!(c.id, ids().id::<Connection>(&key));
        assert_eq!(c.number, NumberSpec::Unassigned);
    }

    #[test]
    fn integer_ends_use_coordinates_then_list_position() {
        // Instance terminal 0 sits at the definition position of terminal 1 (reordered list); terminals
        // 1 and 2 carry no coordinates and fall back to the list position.
        let b = uid(11);
        let d = diagram(
            vec![
                element(
                    None,
                    vec![inst("1", Some((10, 0))), inst("2", None), inst("3", None)],
                ),
                element(Some(b.clone()), vec![inst("9", Some((0, 0)))]),
            ],
            vec![
                cond(end_id("1"), end_u(&b, &uid(22))),
                cond(end_id("3"), end_u(&b, &uid(22))),
            ],
        );
        let mut reg = Registry::new();
        reg.insert(
            (0, "#0".to_owned()),
            placed(
                "d0#0",
                func(100, vec![Some(10), Some(11), Some(12)]),
                vec![pterm(None, 0, 0), pterm(None, 10, 0), pterm(None, 20, 0)],
            ),
        );
        reg.insert(
            (0, b.clone()),
            placed(&b, func(101, vec![Some(0)]), vec![pterm(Some(uid(22)), 0, 0)]),
        );
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0, "{:?}", f.entries());
        assert_eq!(out.connections[0].a, Endpoint::Cp(function(100), 11));
        assert_eq!(out.connections[1].a, Endpoint::Cp(function(100), 12));
        // The uuid-less element's key is folio-local: prefixed with the diagram index.
        let key = format!("qet:wire:{}/0|d0#0/1", uid(11));
        assert_eq!(out.connections[0].source_ref.as_ref().unwrap().key, key);
    }

    #[test]
    fn integer_end_with_element_must_name_that_element() {
        let (mut d, reg) = two_elements();
        // Terminal id 3 belongs to element B, not A.
        d.conductors.push(cond(
            End {
                element: Some(uid(10)),
                terminal: TerminalRef::Id("3".to_owned()),
            },
            end_u(&uid(11), &uid(22)),
        ));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 1);
        assert_eq!(codes(&f), vec![(Code::UNRESOLVED_REFERENCE, Severity::Error)]);
    }

    #[test]
    fn unresolved_element_is_lost_and_reported() {
        let (mut d, reg) = two_elements();
        d.conductors
            .push(cond(end_u(&uid(10), &uid(21)), end_u(&uid(99), &uid(22))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 1);
        assert!(out.connections.is_empty());
        let e = &f.entries()[0];
        assert_eq!(
            (e.code, e.severity),
            (Code::UNRESOLVED_REFERENCE, Severity::Error)
        );
        assert_eq!(e.message, "conductor dropped: end 2: element not found");
        let loc = e.location.as_ref().unwrap();
        assert_eq!(loc.folio.as_deref(), Some("F1"));
        assert_eq!(loc.item.as_deref(), Some("conductor 0"));
        assert_eq!(loc.line, Some(7));
    }

    #[test]
    fn terminal_without_cp_and_dropped_element_are_lost() {
        let (mut d, mut reg) = two_elements();
        reg.get_mut(&(0, uid(10))).unwrap().target = func(100, vec![Some(0), None]);
        reg.get_mut(&(0, uid(11))).unwrap().target = Target::Dropped;
        d.conductors
            .push(cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22))));
        d.conductors
            .push(cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 2);
        assert_eq!(
            f.entries()[0].message,
            "conductor dropped: end 1: terminal maps to no connection point"
        );
        assert_eq!(
            f.entries()[1].message,
            "conductor dropped: end 2: element was not imported"
        );
    }

    #[test]
    fn same_cp_at_both_ends_is_lost() {
        let (mut d, reg) = two_elements();
        d.conductors
            .push(cond(end_u(&uid(10), &uid(20)), end_u(&uid(10), &uid(20))));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 1);
        assert!(out.connections.is_empty());
        assert_eq!(
            f.entries()[0].message,
            "conductor dropped: both ends resolve to the same connection point"
        );
    }

    #[test]
    fn parallel_conductors_get_distinct_ids() {
        let (mut d, reg) = two_elements();
        let c = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        // The same pair in the opposite direction is the same base key.
        let rev = cond(end_u(&uid(11), &uid(22)), end_u(&uid(10), &uid(20)));
        d.conductors.extend([c.clone(), rev, c]);
        let (out, _) = run(&project(vec![d]), &reg);
        let keys: Vec<&str> = out
            .connections
            .iter()
            .map(|c| c.source_ref.as_ref().unwrap().key.as_str())
            .collect();
        let base = format!("qet:wire:{}/0|{}/0", uid(10), uid(11));
        assert_eq!(keys, vec![base.clone(), format!("{base}#2"), format!("{base}#3")]);
        let distinct: BTreeSet<_> = out.connections.iter().map(|c| c.id).collect();
        assert_eq!(distinct.len(), 3);
    }

    /// The page point (x, y) px.
    fn at(x: i64, y: i64) -> Pt2 {
        Pt2::new(px(x), px(y))
    }

    fn seg(horizontal: bool, n: &str) -> Segment {
        Segment {
            horizontal,
            length: Decimal::parse(n).unwrap(),
        }
    }

    #[test]
    fn stored_segments_are_replayed_from_end_1() {
        // A's terminal at (100, 0) px, B's at (0, 50) px. A → B: down 20, left 100, down 30.
        let (a, b) = (uid(10), uid(11));
        let mut c = cond(end_u(&a, &uid(20)), end_u(&b, &uid(22)));
        c.segments = vec![seg(false, "20"), seg(true, "-100"), seg(false, "30")];
        // B → A: lengths that miss A by 0.4 px, within QET's tolerance; the corner takes A's x.
        let mut back = cond(end_u(&b, &uid(22)), end_u(&a, &uid(20)));
        back.segments = vec![seg(false, "-30"), seg(true, "100.4"), seg(false, "-20")];
        let d = diagram(
            vec![
                element(Some(a.clone()), Vec::new()),
                element(Some(b.clone()), Vec::new()),
            ],
            vec![c, back],
        );
        let mut reg = Registry::new();
        let ta = vec![pterm_on_page(Some(uid(20)), 100, 0)];
        reg.insert((0, a.clone()), placed(&a, func(100, vec![Some(0)]), ta));
        let tb = vec![pterm_on_page(Some(uid(22)), 0, 50)];
        reg.insert((0, b.clone()), placed(&b, func(101, vec![Some(0)]), tb));
        let (out, f) = run(&project(vec![d]), &reg);
        assert!(f.entries().is_empty(), "{:?}", f.entries());
        assert_eq!(
            out.connections[0].route,
            vec![at(100, 0), at(100, 20), at(0, 20), at(0, 50)]
        );
        assert_eq!(
            out.connections[1].route,
            vec![at(0, 50), at(0, 20), at(100, 20), at(100, 0)]
        );
    }

    #[test]
    fn stored_path_rules() {
        let (a, b) = (at(100, 0), at(0, 50));
        // Lengths that miss end 2 by more than 1 px: QET sets them aside (no stored path).
        assert_eq!(stored_route(a, b, &[seg(false, "50"), seg(true, "-102")]), None);
        assert_eq!(stored_route(a, b, &[]), None);
        let far = Pt2::new(Nm(i64::MAX), Nm(0));
        assert_eq!(stored_route(far, b, &[seg(true, "1")]), None);
        // An end 1 nm off its whole px: the corners take the whole px, the route still ends on the end; a stretch out
        // and back is tidied away.
        let a1 = Pt2::new(Nm(a.x.0 + 1), a.y);
        let lengths = [
            seg(false, "20"),
            seg(false, "-10"),
            seg(true, "-100"),
            seg(false, "40"),
        ];
        assert_eq!(
            stored_route(a1, b, &lengths),
            Some(vec![a1, Pt2::new(a.x, px(10)), Pt2::new(b.x, px(10)), b])
        );
        // An end at a fractional px: a corner within that px takes the end's exact value.
        let a2 = Pt2::new(Decimal::parse("100.5").unwrap().px_to_nm().unwrap(), a.y);
        assert_eq!(
            stored_route(a2, b, &[seg(false, "20"), seg(true, "-100.5"), seg(false, "30")]),
            Some(vec![a2, Pt2::new(a2.x, px(20)), Pt2::new(b.x, px(20)), b])
        );
    }

    #[test]
    fn route_problems_are_named() {
        let (a, b) = (at(0, 0), at(100, 50));
        assert_eq!(
            route_problem(&[a, b], a, b),
            Some("a segment is neither horizontal nor vertical")
        );
        let off = [a, at(55, 0), at(55, 50), b];
        assert_eq!(route_problem(&off, a, b), Some("a corner is off the 10 px grid"));
        // Out along y = 0, down, back along y = 50 past the start, up, and over the first segment again.
        let around = [
            at(0, 0),
            at(100, 0),
            at(100, 50),
            at(-50, 50),
            at(-50, 0),
            at(50, 0),
        ];
        assert_eq!(
            route_problem(&around, at(0, 0), at(50, 0)),
            Some("a segment runs over another")
        );
        assert_eq!(route_problem(&[a, at(100, 0), b], a, b), None);
    }

    /// QET's automatic path for the terminal-orientation cases of `Conductor::generateConductorPath`, worked out by
    /// hand: extend each terminal 10 px (rounded onto the 10 px grid), then join the extensions from left to right.
    #[test]
    fn automatic_path_follows_qets_rule() {
        let (n, e, s, w) = (Dir4::Up, Dir4::Right, Dir4::Down, Dir4::Left);
        let auto = |p1, o1: &Dir4, p2, o2: &Dir4| auto_route(p1, o1, p2, o2).unwrap();
        // Straight: facing on one line; the bridge at x 150 lies on the line, so the route is one segment.
        assert_eq!(
            auto(at(100, 100), &e, at(200, 100), &w),
            vec![at(100, 100), at(200, 100)]
        );
        // Facing, running down (case 3): extensions at x 110 and round(19.5) × 10 = 200, the bridge at
        // (110 + 200) / 2 = 155 down to the grid: 150.
        assert_eq!(
            auto(at(100, 100), &e, at(205, 160), &w),
            vec![at(100, 100), at(150, 100), at(150, 160), at(205, 160)]
        );
        // Facing, running up (case 4): the bridge at x 150.
        assert_eq!(
            auto(at(100, 160), &e, at(200, 100), &w),
            vec![at(100, 160), at(150, 160), at(150, 100), at(200, 100)]
        );
        // Same side, both up, running down (case 2): over the top at the higher extension, y 90.
        assert_eq!(
            auto(at(100, 100), &n, at(200, 140), &n),
            vec![at(100, 100), at(100, 90), at(200, 90), at(200, 140)]
        );
        // Same side, both down, running up (case 1): under the bottom at the lower extension, y 170.
        assert_eq!(
            auto(at(100, 160), &s, at(200, 100), &s),
            vec![at(100, 160), at(100, 170), at(200, 170), at(200, 100)]
        );
        // Perpendicular, running down (case 1): down from the left terminal, then right.
        assert_eq!(
            auto(at(100, 100), &s, at(200, 160), &w),
            vec![at(100, 100), at(100, 160), at(200, 160)]
        );
        // The same wire from the other end: the path is expressed from terminal 1 to terminal 2.
        assert_eq!(
            auto(at(200, 160), &w, at(100, 100), &s),
            vec![at(200, 160), at(100, 160), at(100, 100)]
        );
        // Facing away on one line: QET draws the stubs over the line; the route is the line.
        assert_eq!(
            auto(at(100, 100), &w, at(200, 100), &e),
            vec![at(100, 100), at(200, 100)]
        );
        // Facing, a terminal 1 nm off the line of the other (the rounding of its page position): QET's one
        // vertical segment, slanting by that 1 nm.
        let p1 = Pt2::new(Nm(px(440).0 + 1), px(360));
        let route = auto(p1, &s, at(440, 368), &n);
        assert_eq!(route, vec![p1, at(440, 368)]);
        assert_eq!(route_problem(&route, p1, at(440, 368)), None);
        let unknown = Dir4::Unknown(json!("x"));
        assert_eq!(auto_route(at(0, 0), &n, at(10, 0), &unknown), None);
    }

    #[test]
    fn wire_route_falls_back_to_the_automatic_path_or_leaves_the_wire_unrouted() {
        let (a, b) = (at(100, 100), at(200, 160));
        let (down, left) = (Some(Dir4::Down), Some(Dir4::Left));
        let l_shape = vec![a, at(100, 160), b];
        // Stored lengths that miss end 2: QET's automatic path, as QET draws it; no finding.
        assert_eq!(
            wire_route(
                Some((a, down.clone())),
                Some((b, left.clone())),
                &[seg(true, "5")]
            ),
            (l_shape.clone(), None)
        );
        // A stored corner off the 10 px grid: the automatic path, with a finding.
        let off = [seg(false, "25"), seg(true, "100"), seg(false, "35")];
        let (route, note) = wire_route(Some((a, down.clone())), Some((b, left.clone())), &off);
        assert_eq!(route, l_shape);
        assert_eq!(
            note.as_deref(),
            Some("stored path not kept (a corner is off the 10 px grid); QET's automatic path used")
        );
        // Neither: unrouted, with the reason.
        let (route, note) = wire_route(Some((a, None)), Some((b, left.clone())), &off);
        assert!(route.is_empty());
        assert_eq!(
            note.as_deref(),
            Some("wire left unrouted: a corner is off the 10 px grid")
        );
        let (route, note) = wire_route(Some((a, None)), Some((b, left.clone())), &[]);
        assert!(route.is_empty());
        assert_eq!(
            note.as_deref(),
            Some("wire left unrouted: the direction of a terminal is unknown")
        );
        // Terminals that touch: nothing to draw at one point, the 1 nm between them otherwise; nothing to report.
        assert_eq!(
            wire_route(Some((a, down.clone())), Some((a, left.clone())), &[]),
            (Vec::new(), None)
        );
        let a1 = Pt2::new(Nm(a.x.0 + 1), a.y);
        assert_eq!(
            wire_route(Some((a1, down.clone())), Some((a, left)), &[]),
            (vec![a1, a], None)
        );
        let (_, note) = wire_route(None, Some((b, down)), &[]);
        assert_eq!(
            note.as_deref(),
            Some("wire left unrouted: an end has no position on the page")
        );
    }

    #[test]
    fn junction_element_gets_junction_with_both_connections() {
        let (a, j) = (uid(10), uid(12));
        let d = diagram(
            vec![
                element(
                    Some(a.clone()),
                    vec![inst("1", Some((0, 0))), inst("2", Some((10, 0)))],
                ),
                element(Some(j.clone()), vec![inst("3", Some((0, 0)))]),
            ],
            vec![
                cond(end_u(&a, &uid(20)), end_u(&j, &uid(23))),
                cond(end_u(&a, &uid(21)), end_id("3")),
            ],
        );
        let jid: Id<Junction> = Id::from_uuid(Uuid::from_u128(77));
        let at = Pt2::new(Nm(5), Nm(6));
        let mut reg = Registry::new();
        reg.insert(
            (0, a.clone()),
            placed(
                &a,
                func(100, vec![Some(0), Some(1)]),
                vec![pterm(Some(uid(20)), 0, 0), pterm(Some(uid(21)), 10, 0)],
            ),
        );
        reg.insert(
            (0, j.clone()),
            placed(
                &j,
                Target::Junction {
                    id: jid,
                    key: format!("qet:junction:{j}"),
                    at,
                },
                vec![pterm(Some(uid(23)), 0, 0)],
            ),
        );
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0);
        assert_eq!(out.connections.len(), 2);
        assert_eq!(out.connections[0].b, Endpoint::Junction(jid));
        assert_eq!(out.connections[1].b, Endpoint::Junction(jid));
        assert_eq!(out.junctions.len(), 1);
        let junction = &out.junctions[0];
        assert_eq!(junction.id, jid);
        assert_eq!(junction.at, at);
        assert_eq!(junction.style, JunctionStyle::Dot);
        assert_eq!(junction.wiring, Some(JunctionWiring::Star));
        assert_eq!(
            junction.sequence,
            vec![out.connections[0].id, out.connections[1].id]
        );
        assert_eq!(
            junction.source_ref.as_ref().unwrap().key,
            format!("qet:junction:{j}")
        );
        let e = f.entries().last().unwrap();
        assert_eq!((e.code, e.severity), (Code::imp(118), Severity::Info));
        assert_eq!(
            e.message,
            "junction wiring assumed (star) at a QET junction element"
        );
        assert_eq!(
            e.location.as_ref().unwrap().item.as_deref(),
            Some(format!("qet:junction:{j}").as_str())
        );
    }

    #[test]
    fn text_properties_are_kept_in_extra_and_summarised() {
        let (mut d, reg) = two_elements();
        let mut c1 = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        c1.function = " PE ".to_owned();
        c1.color = "BK".to_owned();
        c1.section = "1.5".to_owned();
        let mut c2 = cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22)));
        c2.function = "N".to_owned();
        c2.color = "BK".to_owned();
        d.conductors.extend([c1, c2]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.connections[0].extra["qet.function"], json!("PE"));
        assert_eq!(out.connections[0].extra["qet.color"], json!("BK"));
        assert_eq!(out.connections[0].extra["qet.section"], json!("1.5"));
        assert!(!out.connections[0].extra.contains_key("qet.tension_protocol"));
        let msgs: Vec<&str> = f.entries().iter().map(|e| e.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "conductor function kept as text on 2 conductors (2 distinct values); not mapped to wire properties",
                "conductor conductor_color kept as text on 2 conductors (1 distinct values); not mapped to wire properties",
                "conductor conductor_section kept as text on 1 conductors (1 distinct values); not mapped to wire properties",
            ]
        );
        assert!(
            f.entries()
                .iter()
                .all(|e| (e.code, e.severity) == (Code::FOREIGN_VALUE, Severity::Warning))
        );
        assert!(
            f.entries()
                .iter()
                .all(|e| e.location.as_ref().unwrap().folio.is_none())
        );
    }

    #[test]
    fn wire_number_is_frozen_or_kept_as_text() {
        let (mut d, reg) = two_elements();
        let mut c1 = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        c1.num = " 101 ".to_owned();
        let mut c2 = cond(end_u(&uid(10), &uid(21)), end_u(&uid(11), &uid(22)));
        c2.num = "a\tb".to_owned();
        d.conductors.extend([c1, c2]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(
            out.connections[0].number,
            NumberSpec::Frozen(Ident::from_input("101").unwrap())
        );
        assert_eq!(out.connections[1].number, NumberSpec::Unassigned);
        assert_eq!(out.connections[1].extra["qet.num"], json!("a\tb"));
        assert_eq!(codes(&f), vec![(Code::FOREIGN_VALUE, Severity::Warning)]);
        assert_eq!(
            f.entries()[0].location.as_ref().unwrap().item.as_deref(),
            Some("conductor 1")
        );
    }

    #[test]
    fn counts_include_integer_and_uuid_ends() {
        let (mut d, _) = two_elements();
        d.conductors.extend([
            cond(end_u(&uid(10), &uid(20)), end_id("3")),
            cond(end_id("2"), end_u(&uid(11), &uid(22))),
            // An unknown element and an unknown terminal id count for nothing.
            cond(end_u(&uid(99), &uid(20)), end_id("77")),
            cond(
                End {
                    element: None,
                    terminal: TerminalRef::Missing,
                },
                end_u(&uid(10), &uid(21)),
            ),
        ]);
        let counts = conductor_counts(&project(vec![d]));
        assert_eq!(counts.len(), 2);
        assert_eq!(counts[&(0, uid(10))], 3);
        assert_eq!(counts[&(0, uid(11))], 2);
    }

    /// A placed terminal at definition position (0, 0) whose page position is `(x, y)` px.
    fn pterm_on_page(uuid: Option<String>, x: i64, y: i64) -> PlacedTerminal {
        PlacedTerminal {
            uuid,
            local: Pt2::ORIGIN,
            page_pos: Some(Pt2::new(px(x), px(y))),
            dir: Some(Dir4::Down),
        }
    }

    /// Element A (UUID 10) pasted twice on one folio, at page (0, 0) px and (100, 0) px, and element B at
    /// (100, 50) px. The copies share their terminal UUID; the second copy's element key is `a#2`.
    fn pasted_twice() -> (QetDiagram, Registry) {
        let (a, b) = (uid(10), uid(11));
        let d = diagram(
            vec![
                element(Some(a.clone()), vec![inst("1", Some((0, 0)))]),
                element(Some(a.clone()), vec![inst("2", Some((0, 0)))]),
                element(Some(b.clone()), vec![inst("3", Some((0, 0)))]),
            ],
            Vec::new(),
        );
        let mut reg = Registry::new();
        reg.insert(
            (0, a.clone()),
            placed(
                &a,
                func(100, vec![Some(0)]),
                vec![pterm_on_page(Some(uid(20)), 0, 0)],
            ),
        );
        reg.insert(
            (0, format!("{a}#2")),
            placed(
                "ek-a2",
                func(102, vec![Some(0)]),
                vec![pterm_on_page(Some(uid(20)), 100, 0)],
            ),
        );
        reg.insert(
            (0, b.clone()),
            placed(
                &b,
                func(101, vec![Some(0)]),
                vec![pterm_on_page(Some(uid(22)), 100, 50)],
            ),
        );
        (d, reg)
    }

    fn vertical(n: i64) -> Segment {
        Segment {
            horizontal: false,
            length: Decimal::from_int(n),
        }
    }

    /// k copies of A (terminal UUID 20) at (100 i, 0) px and k distinct elements B_i at (100 i, 50 + 10 i) px;
    /// conductor i runs from A (by UUID) to B_i, drawn 50 + 10 i px down, so only copy A_i fits it. Returns
    /// the output, the findings and the comparison count.
    fn copies_to_distinct_ends(k: usize) -> (WireOut, Findings, u64) {
        let a = uid(10);
        let mut reg = Registry::new();
        let mut elements = vec![];
        let mut conductors = Vec::new();
        for i in 0..k {
            let n = i + 1;
            let ka = if n == 1 { a.clone() } else { format!("{a}#{n}") };
            let b = uid(100_000 + u128::try_from(i).unwrap());
            let x = i64::try_from(100 * i).unwrap();
            let down = 50 + 10 * i64::try_from(i).unwrap();
            let fa = 1_000 + u128::try_from(i).unwrap();
            let fb = 2_000_000 + u128::try_from(i).unwrap();
            let pa = vec![pterm_on_page(Some(uid(20)), x, 0)];
            let pb = vec![pterm_on_page(Some(uid(22)), x, down)];
            reg.insert((0, ka), placed(&format!("a{n}"), func(fa, vec![Some(0)]), pa));
            reg.insert(
                (0, b.clone()),
                placed(&format!("b{n}"), func(fb, vec![Some(0)]), pb),
            );
            elements.push(element(Some(a.clone()), Vec::new()));
            elements.push(element(Some(b.clone()), Vec::new()));
            let mut c = cond(end_u(&a, &uid(20)), end_u(&b, &uid(22)));
            c.segments = vec![vertical(down)];
            conductors.push(c);
        }
        let d = diagram(elements, conductors);
        let mut f = Findings::new("t.qet", 10_000);
        let (out, comparisons) = map_conductors_counted(&project(vec![d]), &reg, &ids(), &mut f);
        (out, f, comparisons)
    }

    #[test]
    fn repeated_uuid_ends_cost_linear_comparisons_overall() {
        // The copies' terminal positions are indexed once per folio and UUID (k insertions) and each conductor
        // makes one lookup: 2k comparisons for k conductors, the same constant for k = 50 and k = 200 (a
        // per-conductor enumeration of the copies would make k²).
        for k in [50usize, 200] {
            let (out, f, comparisons) = copies_to_distinct_ends(k);
            let k64 = u64::try_from(k).unwrap();
            assert!(comparisons <= 3 * k64, "{comparisons} comparisons for k = {k}");
            assert_eq!(out.lost, 0);
            assert!(f.entries().is_empty(), "{:?}", f.entries());
            for (i, c) in out.connections.iter().enumerate() {
                let i = u128::try_from(i).unwrap();
                assert_eq!(c.a, Endpoint::Cp(function(1_000 + i), 0), "k = {k}");
                assert_eq!(c.b, Endpoint::Cp(function(2_000_000 + i), 0), "k = {k}");
            }
        }
    }

    #[test]
    fn repeated_uuid_is_bound_by_the_conductor_geometry() {
        let (mut d, reg) = pasted_twice();
        // A -> B, drawn 50 px down: only the copy at (100, 0) ends at B (100, 50).
        let mut c = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        c.segments = vec![vertical(50)];
        // B -> A, drawn 50 px up: the same copy, from the other side.
        let mut back = cond(end_u(&uid(11), &uid(22)), end_u(&uid(10), &uid(20)));
        back.segments = vec![vertical(-50)];
        d.conductors.extend([c, back]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0, "{:?}", f.entries());
        assert!(f.entries().is_empty(), "{:?}", f.entries());
        assert_eq!(out.connections[0].a, Endpoint::Cp(function(102), 0));
        assert_eq!(out.connections[0].b, Endpoint::Cp(function(101), 0));
        assert_eq!(out.connections[1].a, Endpoint::Cp(function(101), 0));
        assert_eq!(out.connections[1].b, Endpoint::Cp(function(102), 0));
        // The wire's source key names the second copy by its project-wide element key.
        let key = out.connections[0].source_ref.as_ref().unwrap().key.clone();
        assert!(key.contains("ek-a2/0"), "{key}");
    }

    #[test]
    fn repeated_uuid_without_deciding_geometry_binds_the_first_copy_and_warns() {
        let (mut d, reg) = pasted_twice();
        // No segments; and segments that fit neither copy.
        let plain = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        let mut off = cond(end_u(&uid(10), &uid(20)), end_u(&uid(11), &uid(22)));
        off.segments = vec![vertical(7)];
        d.conductors.extend([plain, off]);
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.lost, 0);
        assert_eq!(out.connections.len(), 2);
        for c in &out.connections {
            assert_eq!(c.a, Endpoint::Cp(function(100), 0));
            assert_eq!(c.b, Endpoint::Cp(function(101), 0));
        }
        assert_eq!(
            codes(&f),
            vec![
                (Code::imp(120), Severity::Warning),
                (Code::imp(120), Severity::Warning)
            ]
        );
        let e = &f.entries()[0];
        assert_eq!(
            e.message,
            "conductor end 1: element UUID repeated on this folio; bound to the first copy (geometry does not decide)"
        );
        let loc = e.location.as_ref().unwrap();
        assert_eq!(loc.folio.as_deref(), Some("F1"));
        assert_eq!(loc.item.as_deref(), Some("conductor 0"));
        assert_eq!(loc.line, Some(7));
        assert_eq!(
            f.entries()[1].location.as_ref().unwrap().item.as_deref(),
            Some("conductor 1")
        );
    }

    #[test]
    fn single_terminal_element_resolves_without_terminal_attribute() {
        let (mut d, reg) = two_elements();
        d.conductors.push(cond(
            end_u(&uid(10), &uid(20)),
            End {
                element: Some(uid(11)),
                terminal: TerminalRef::Missing,
            },
        ));
        d.conductors.push(cond(
            End {
                element: Some(uid(10)),
                terminal: TerminalRef::Missing,
            },
            end_u(&uid(11), &uid(22)),
        ));
        let (out, f) = run(&project(vec![d]), &reg);
        assert_eq!(out.connections.len(), 1);
        assert_eq!(out.lost, 1);
        assert_eq!(
            f.entries()[0].message,
            "conductor dropped: end 1: terminal not specified"
        );
    }
}
