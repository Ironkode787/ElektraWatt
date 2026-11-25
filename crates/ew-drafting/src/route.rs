// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The router (docs/design/06a-drafting.md §3.2, §3.6 to §3.10): [`route`] makes a page's plan for the wires its
//! caller names ([`jobs`]), [`route_wire`] the Wire tool's route between two ends, on the same occupancy and search.
//! The phases of a run are the methods of `Router`, in the order [`route`] calls them.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;

use crate::consts::{HALF, TICKS_PER_STEP, TRACK_PITCH};
use crate::geom;
use crate::labels::settle_labels;
use crate::measure::{Quality, measure};
use crate::model::{
    DraftPage, End, JunctionId, MarkId, Owner, PartId, PinId, Pt, Rect, Side, WireId, grid_down,
};
use crate::nets::{self};
use crate::occupancy::{Occupancy, Text};
use crate::search::{self, Ask, Found, Relax, Tip, TipKind};

mod phases;
mod wire;

pub(crate) use wire::plain_line;
pub use wire::{WireEnd, WireRoute, route_wire, unrouted_line, wire_clear};

/// What a Route runs on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scope<'a> {
    /// The page: every wire unrouted, with a fault as stored, or computed (§3.2).
    Page,
    /// The selection: the wires named and every wire with an end on a part named.
    Selection(&'a [Uuid]),
}

/// The wires a run routes, and those of them whose stored route the person shaped (flagged his, §3.2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Jobs {
    /// The wires to route.
    pub wires: Vec<WireId>,
    /// Those of them with a route the person shaped.
    pub his: Vec<WireId>,
    /// Every junction stays where it stands (a move's re-solve: its command moves no junction).
    pub keep_junctions: bool,
}

/// The jobs of a Route on `scope` (§3.2): a page-wide Route takes every wire that is unrouted, has a fault as stored
/// (a slanted segment, a vertex off the half grid, a route that misses an end, or one of F2 to F6) or is `computed`,
/// and flags the person's among
/// them; a Route of a selection takes the wires named and every wire with an end on a part named.
pub fn jobs(page: &DraftPage, scope: Scope<'_>) -> Jobs {
    let mut wires: Vec<WireId> = Vec::new();
    let mut his: Vec<WireId> = Vec::new();
    match scope {
        Scope::Page => {
            let q = measure(page);
            let f = &q.findings;
            let mut faulty = wires_with_faults(&q);
            let mut junction_wires: BTreeMap<JunctionId, Vec<WireId>> = BTreeMap::new();
            for w in &page.wires {
                for e in w.ends {
                    if let End::Junction(j) = e {
                        junction_wires.entry(j).or_default().push(w.id);
                    }
                }
            }
            for (j, _, _) in &f.four_way {
                faulty.extend(junction_wires.get(j).into_iter().flatten());
            }
            for w in &page.wires {
                let unrouted = f.unrouted.contains(&w.id);
                let bent = !w.route.is_empty() && (f1(&w.route) || w.stale);
                let broken = faulty.contains(&w.id) || bent;
                if unrouted || broken || (w.computed && !w.route.is_empty()) {
                    wires.push(w.id);
                    if broken && !w.computed {
                        his.push(w.id);
                    }
                }
            }
        }
        Scope::Selection(ids) => {
            let mut parts_pins: BTreeSet<PinId> = BTreeSet::new();
            for p in &page.parts {
                if ids.contains(&p.id.0) {
                    parts_pins.extend(p.pins.iter().map(|x| x.id));
                }
            }
            for w in &page.wires {
                let on = w
                    .ends
                    .iter()
                    .any(|e| matches!(e, End::Pin(p) if parts_pins.contains(p)));
                if ids.contains(&w.id.0) || on {
                    wires.push(w.id);
                }
            }
        }
    }
    wires.sort();
    his.sort();
    Jobs {
        wires,
        his,
        keep_junctions: false,
    }
}

/// F1 as stored: a slanted segment, or an inner vertex off the half grid.
fn f1(route: &[Pt]) -> bool {
    let pts = geom::clean(route);
    let slanted = geom::segments(&pts).iter().any(|(a, b)| a.x != b.x && a.y != b.y);
    let inner = pts.get(1..pts.len().saturating_sub(1)).unwrap_or(&[]);
    slanted
        || inner
            .iter()
            .any(|p| p.x.rem_euclid(HALF) != 0 || p.y.rem_euclid(HALF) != 0)
}

/// What is in the way of a wire without a route (§3.8).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Reason {
    /// Other wires.
    Wires,
    /// Connection points.
    Points,
    /// Parts.
    Parts,
    /// A fourth arm at a junction.
    FourArms,
}

/// A run's plan (§3.9): what changes, and the page's quality before and after it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Plan {
    /// The wires whose route changes, with the new route, by ID.
    pub routes: Vec<(WireId, Vec<Pt>)>,
    /// The junctions that move, with where they go, by ID.
    pub junctions: Vec<(JunctionId, Pt)>,
    /// The marks that move, by how much, by ID.
    pub marks: Vec<(MarkId, Pt)>,
    /// The jobs left without a new route and what is in their way, by ID.
    pub unrouted: Vec<(WireId, Reason)>,
    /// The routes the person shaped that the plan replaces (they were broken), by ID.
    pub his: Vec<WireId>,
    /// The nets whose wiring order runs against their parts' places (each by its wires): no trunk could follow it.
    pub notes: Vec<Vec<WireId>>,
    /// The page's quality before the plan.
    pub before: Quality,
    /// Its quality after.
    pub after: Quality,
}

impl Plan {
    /// Whether the plan changes nothing.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty() && self.junctions.is_empty() && self.marks.is_empty()
    }
}

/// A pin as the router reads it.
#[derive(Clone, Copy, Debug)]
struct PinAt {
    at: Pt,
    side: Option<Side>,
    part: PartId,
    body: Rect,
}

/// The router's state over one page.
struct Router {
    /// The page as the run has it: the jobs' routes as laid, the junctions where they stand.
    page: DraftPage,
    occ: Occupancy,
    jobs: BTreeSet<WireId>,
    /// The jobs' stored routes and the junctions' first places.
    stored: BTreeMap<WireId, Vec<Pt>>,
    first: BTreeMap<JunctionId, Pt>,
    pins: BTreeMap<PinId, PinAt>,
    ends: BTreeMap<WireId, [End; 2]>,
    /// The texts of the occupancy that are no obstacle: of marks and of wires.
    skip: BTreeSet<Uuid>,
}

impl Router {
    /// Phase 1, the drawing a run starts from: the router over `page` with the routes of `jobs` lifted. The labels are
    /// placed against what stays (the starting places), and the occupancy is built over the parts, the pins, the
    /// junctions, the fixed wires and the texts (a label at its place, a fixed text; not the texts of a mark or of a
    /// wire).
    fn new(page: &DraftPage, jobs: &[WireId]) -> Option<Router> {
        let mut page = page.clone();
        let jobs: BTreeSet<WireId> = jobs.iter().copied().collect();
        let mut stored = BTreeMap::new();
        for w in &mut page.wires {
            if jobs.contains(&w.id) {
                stored.insert(w.id, std::mem::take(&mut w.route));
            }
        }
        let first = page.junctions.iter().map(|j| (j.id, j.at)).collect();
        let mut pins = BTreeMap::new();
        for p in &page.parts {
            for pin in &p.pins {
                pins.insert(
                    pin.id,
                    PinAt {
                        at: p.at.plus(pin.at),
                        side: pin.side,
                        part: p.id,
                        body: p.bounds(),
                    },
                );
            }
        }
        let ends = page.wires.iter().map(|w| (w.id, w.ends)).collect();
        let skip = page
            .marks
            .iter()
            .map(|m| m.id.0)
            .chain(page.wires.iter().map(|w| w.id.0))
            .collect();
        let occ = Occupancy::new(&page, Vec::new(), &|_| false)?;
        let mut r = Router {
            page,
            occ,
            jobs,
            stored,
            first,
            pins,
            ends,
            skip,
        };
        let texts = r.texts(&settle_labels(&r.page));
        r.rebuild(texts)?;
        Some(r)
    }

    /// The texts of the page as obstacles: the labels at `settled` places and the fixed texts, but a mark's and a
    /// wire's.
    fn texts(&self, settled: &[crate::labels::Settled]) -> Vec<Text> {
        let mut out = Vec::new();
        for s in settled {
            let Some(l) = self.page.labels.iter().find(|l| l.id == s.id) else {
                continue;
            };
            if matches!(l.owner, Owner::Wire(_) | Owner::Mark(_)) {
                continue;
            }
            out.push(Text {
                rect: s.rect,
                owner: s.id.owner,
                label: Some(s.id),
            });
        }
        let mut fixed: Vec<_> = self
            .page
            .fixed_texts
            .iter()
            .filter(|t| !self.skip.contains(&t.owner))
            .map(|t| (t.owner, t.rect))
            .collect();
        fixed.sort();
        out.extend(fixed.into_iter().map(|(owner, rect)| Text {
            rect,
            owner,
            label: None,
        }));
        out
    }

    /// The occupancy built anew over the page as it stands with `texts`.
    fn rebuild(&mut self, texts: Vec<Text>) -> Option<()> {
        let wired = self.page.wired();
        let mut occ = Occupancy::new(&self.page, texts, &|p| wired.contains(&p))?;
        let mut wires: Vec<_> = self.page.wires.iter().collect();
        wires.sort_by_key(|w| w.id);
        for w in wires {
            if !w.route.is_empty() {
                occ.lay(&w.route, true);
            }
        }
        self.occ = occ;
        Some(())
    }

    fn wire_index(&self, w: WireId) -> Option<usize> {
        self.page.wires.iter().position(|x| x.id == w)
    }

    fn route_of(&self, w: WireId) -> &[Pt] {
        self.wire_index(w)
            .map_or(&[], |i| self.page.wires[i].route.as_slice())
    }

    /// Lays `route` as wire `w`'s.
    fn lay(&mut self, w: WireId, route: Vec<Pt>) {
        self.lift(w);
        self.occ.lay(&route, true);
        if let Some(i) = self.wire_index(w) {
            self.page.wires[i].route = route;
        }
    }

    /// Lifts wire `w`'s route.
    fn lift(&mut self, w: WireId) {
        if let Some(i) = self.wire_index(w) {
            let old = std::mem::take(&mut self.page.wires[i].route);
            if !old.is_empty() {
                self.occ.lay(&old, false);
            }
        }
    }

    fn junction_at(&self, j: JunctionId) -> Option<Pt> {
        self.page.junctions.iter().find(|x| x.id == j).map(|x| x.at)
    }

    fn move_junction(&mut self, j: JunctionId, to: Pt) {
        if let Some(x) = self.page.junctions.iter_mut().find(|x| x.id == j) {
            let from = x.at;
            x.at = to;
            self.occ.move_junction(j, from, to);
        }
    }

    fn point(&self, e: End) -> Option<Pt> {
        match e {
            End::Pin(p) => self.pins.get(&p).map(|x| x.at),
            End::Junction(j) => self.junction_at(j),
        }
    }

    /// The end `e` of wire `w` as the search reads it, with the runs of the other routed wires of its pin.
    fn tip(&self, w: WireId, e: End) -> Option<Tip> {
        match e {
            End::Pin(p) => {
                let pin = self.pins.get(&p)?;
                let mut shared = Vec::new();
                let mut others: Vec<_> = self
                    .page
                    .wires
                    .iter()
                    .filter(|x| x.id != w && x.ends.contains(&e) && !x.route.is_empty())
                    .collect();
                others.sort_by_key(|x| x.id);
                for x in others {
                    let mut r = geom::clean(&x.route);
                    if r.last() == Some(&pin.at) {
                        r.reverse();
                    }
                    if r.first() == Some(&pin.at) {
                        shared.push(r);
                    }
                }
                Some(Tip {
                    at: pin.at,
                    side: pin.side,
                    owner: TipKind::Pin(p),
                    part: Some((pin.part.0, pin.body)),
                    shared,
                })
            }
            End::Junction(j) => Some(Tip {
                at: self.junction_at(j)?,
                side: None,
                owner: TipKind::Junction(j),
                part: None,
                shared: Vec::new(),
            }),
        }
    }

    fn ask<'s>(&'s self, w: WireId, full: &'s [u32], relax: Relax) -> Option<Ask<'s>> {
        let [a, b] = *self.ends.get(&w)?;
        Some(Ask {
            occ: &self.occ,
            from: self.tip(w, a)?,
            to: self.tip(w, b)?,
            arrive: None,
            area: self.page.area,
            full,
            relax,
        })
    }

    /// Wire `w` searched against the page as it stands (its own route lifted first by the caller).
    fn search(&self, w: WireId, full: &[u32], relax: Relax) -> Option<Found> {
        let ask = self.ask(w, full, relax)?;
        search::route(&ask)
    }

    /// Whether `route` is one wire `w` may take against the page as it stands.
    fn clear(&self, w: WireId, route: &[Pt]) -> bool {
        self.ask(w, &[], Relax::default())
            .is_some_and(|ask| search::clear(&ask, route))
    }

    /// The price of `route` as wire `w`'s.
    fn price(&self, w: WireId, route: &[Pt]) -> u32 {
        self.ask(w, &[], Relax::default())
            .and_then(|ask| search::price_of(&ask, route))
            .unwrap_or(u32::MAX)
    }

    /// Whether wire `w` would be a fourth arm at one of its junctions.
    fn fourth_arm(&self, w: WireId) -> bool {
        let Some(ends) = self.ends.get(&w) else {
            return false;
        };
        ends.iter().any(|e| {
            let End::Junction(j) = e else { return false };
            let Some(at) = self.junction_at(*j) else {
                return false;
            };
            let others = self
                .page
                .wires
                .iter()
                .filter(|x| x.id != w && x.ends.contains(e) && !x.route.is_empty());
            geom::arms(at, others.map(|x| geom::clean(&x.route)), false) >= 3
        })
    }

    /// The city-block distance of wire `w`'s ends and its first end's place: the order of the search.
    fn order_key(&self, w: WireId) -> (i64, (i32, i32), WireId) {
        let [a, b] = self
            .ends
            .get(&w)
            .copied()
            .unwrap_or([End::Pin(PinId(Uuid::nil())); 2]);
        let (pa, pb) = (
            self.point(a).unwrap_or_default(),
            self.point(b).unwrap_or_default(),
        );
        (geom::length((pa, pb)), (pa.y, pa.x), w)
    }
}

/// The wires with a fault (F2 to F6) in `q`.
pub(crate) fn wires_with_faults(q: &Quality) -> BTreeSet<WireId> {
    let f = &q.findings;
    let mut out = BTreeSet::new();
    out.extend(f.through_points.iter().map(|x| x.0));
    out.extend(f.on_wires.iter().flat_map(|x| x.0));
    out.extend(f.through_parts.iter().map(|x| x.0));
    out.extend(f.false_joints.iter().flat_map(|x| x.0));
    out
}

/// The wires with a finding of F2 to F6 or C4 in `q`.
fn wires_with_findings(q: &Quality) -> BTreeSet<WireId> {
    let mut out = wires_with_faults(q);
    out.extend(q.findings.across_texts.iter().map(|x| x.0));
    out
}

/// The labels with a C5 finding in `q`.
fn labels_with_clashes(q: &Quality) -> BTreeSet<crate::model::LabelId> {
    let f = &q.findings;
    let mut out: BTreeSet<_> = f.texts_on_texts.iter().map(|x| x.0).collect();
    out.extend(f.texts_on_parts.iter().map(|x| x.0));
    out.extend(f.texts_outside.iter().copied());
    out
}

/// What breaks the never-worse rule (§1.2) from `before` to `after`: the wires routed before and free of a finding
/// that get one, and whether a text free of C5 gets one where the plan routes no wire that had no route. Where the
/// caller holds the plan's wires without a way fewer (`gained`, with the wires whose faults it excuses), a wire counts
/// only where it gets a fault that is not excused: Route excuses the faults it started from, the placer none.
pub(crate) fn worse(
    page: &DraftPage,
    before: &Quality,
    after: &Quality,
    gained: Option<&BTreeSet<WireId>>,
) -> (BTreeSet<WireId>, bool) {
    let (fb, fa) = (wires_with_findings(before), wires_with_findings(after));
    let routed_before: BTreeSet<WireId> = page
        .wires
        .iter()
        .filter(|w| !w.route.is_empty())
        .map(|w| w.id)
        .collect();
    let mut wires: BTreeSet<WireId> = fa
        .difference(&fb)
        .filter(|w| routed_before.contains(w))
        .copied()
        .collect();
    if let Some(excused) = gained {
        let faulty = wires_with_faults(after);
        wires.retain(|w| faulty.contains(w) && !excused.contains(w));
    }
    let (lb, la) = (labels_with_clashes(before), labels_with_clashes(after));
    let texts = after.unrouted >= before.unrouted && la.difference(&lb).next().is_some();
    (wires, texts)
}

/// A move's re-solve (§7.4): on `after`, the page with the move applied, each wire of `affected` that is unrouted, or
/// routed with a finding of F2 to F6 or C4 or with more bends than on `before`, is a job of [`route`], the other wires
/// fixed and every junction where it stands; the others keep the route they follow the move by, and so does a job the
/// router finds no way for.
pub fn resolve_move(before: &DraftPage, after: &DraftPage, affected: &[WireId]) -> Plan {
    let (was, now) = (measure(before), measure(after));
    let bends = |q: &Quality, w: WireId| {
        q.findings
            .bends
            .iter()
            .find(|(x, _)| *x == w)
            .map_or(0, |(_, n)| *n)
    };
    let broken = wires_with_findings(&now);
    let wires: Vec<WireId> = affected
        .iter()
        .copied()
        .filter(|w| {
            let Some(wire) = after.wires.iter().find(|x| x.id == *w) else {
                return false;
            };
            now.findings.unrouted.contains(w)
                || (!wire.route.is_empty() && (broken.contains(w) || bends(&now, *w) > bends(&was, *w)))
        })
        .collect();
    if wires.is_empty() {
        return Plan::default();
    }
    route(
        after,
        &Jobs {
            wires,
            keep_junctions: true,
            ..Jobs::default()
        },
    )
}

/// The plan of a run over `page` for `jobs`: a junction whose every wire is a job may move; the phases in order, then
/// never worse (§1.2): the plan is offered only when the page is better by [`measure`] and no wire routed before
/// that was free of a finding (F2 to F6, C4) gets one; nor a text free of C5, where the plan routes no wire that had
/// no route. A net that breaks it goes back to its stored drawing; then the rule decides. A plan that routes a wire
/// that had no route is not refused for a text across a wire routed before (C4): only for a fault on one. Then the
/// marks follow (§3.10) the wires they name, and the page with them moved, the page that is stored, is judged again:
/// where a moved mark makes it worse, nothing is offered. A job without a way keeps its stored route, or stays
/// unrouted, and is named with the first thing in its way. Deterministic: the same page in any order of its lists
/// gives the same plan.
pub fn route(page: &DraftPage, jobs: &Jobs) -> Plan {
    let before = measure(page);
    let mut wanted: Vec<WireId> = jobs.wires.clone();
    wanted.sort();
    wanted.dedup();
    wanted.retain(|w| page.wires.iter().any(|x| x.id == *w));
    let empty = |before: Quality| Plan {
        after: before.clone(),
        before,
        ..Plan::default()
    };
    let Some(mut r) = Router::new(page, &wanted) else {
        return empty(before);
    };
    wanted.retain(|w| !r.points_meet(*w));
    let movable = |j: JunctionId| {
        !jobs.keep_junctions
            && page
                .wires
                .iter()
                .filter(|w| w.ends.contains(&End::Junction(j)))
                .all(|w| wanted.contains(&w.id))
    };
    let nets = nets::nets(&wanted, &r.ends, &movable);
    r.straights(&nets);
    let (_, notes) = r.trunks(&nets);
    r.tracks(&nets);
    r.single_taps(&nets);
    let (searched, failed) = r.searches();
    r.rings(&nets, &searched);
    r.text_rounds();
    r.weigh(&nets);
    // Never worse: a net that gives a wire a fault it did not have, or a wire routed before a finding, goes back to its
    // stored drawing (a wire without one to none), until none does.
    let faulty_before = wires_with_faults(&before);
    let mut after = measure(&r.page);
    for _ in 0..=nets.len() {
        let (mut culprits, _) = worse(page, &before, &after, None);
        culprits.extend(wires_with_faults(&after).difference(&faulty_before));
        let mut reverted = false;
        for net in nets
            .iter()
            .filter(|n| n.wires.iter().any(|w| culprits.contains(w)))
        {
            let stored = r.stored_page(net);
            if stored != r.page {
                r.page = stored;
                reverted = true;
            }
        }
        if !reverted {
            break;
        }
        after = measure(&r.page);
    }
    let offered = |after: &Quality| {
        // A wire without a route is the worst state a wire has: a plan that routes one is not refused for a text that
        // then lies across a wire routed before (the plan's quality names it); for a fault on such a wire it is.
        let gained = (after.unrouted < before.unrouted).then_some(&faulty_before);
        let (bad, texts) = worse(page, &before, after, gained);
        after.better_than(&before) && bad.is_empty() && !texts
    };
    if !offered(&after) {
        return empty(before);
    }
    let mut plan = Plan {
        notes,
        ..Plan::default()
    };
    for w in &wanted {
        let new = r.route_of(*w);
        let old = page
            .wires
            .iter()
            .find(|x| x.id == *w)
            .map_or(&[][..], |x| x.route.as_slice());
        if new != old {
            plan.routes.push((*w, new.to_vec()));
        }
    }
    for j in &r.page.junctions {
        if r.first.get(&j.id) != Some(&j.at) {
            plan.junctions.push((j.id, j.at));
        }
    }
    plan.junctions.sort();
    let changed: BTreeSet<WireId> = plan.routes.iter().map(|(w, _)| *w).collect();
    plan.his = jobs.his.iter().filter(|w| changed.contains(w)).copied().collect();
    for w in &failed {
        if !changed.contains(w) {
            plan.unrouted.push((*w, r.reason(*w)));
        }
    }
    plan.marks = marks_follow(&r.page, &changed);
    if !plan.marks.is_empty() {
        // The page after the plan has its marks where the plan sets them, and is judged so: a mark set on its wires
        // that puts its tag on a text makes the page that is stored worse (§1.2, §3.10).
        for (m, d) in &plan.marks {
            if let Some(x) = r.page.marks.iter_mut().find(|x| x.id == *m) {
                crate::marks::move_mark(x, *d);
            }
        }
        after = measure(&r.page);
        if !offered(&after) {
            return empty(before);
        }
    }
    plan.before = before;
    plan.after = after;
    plan
}

impl Router {
    /// What is in the way of job `w` (§3.8): a fourth arm at a junction; else, searched again without the other wires,
    /// the wires; else, without the connection points of other wires too, the points; else the parts.
    fn reason(&mut self, w: WireId) -> Reason {
        if self.fourth_arm(w) {
            return Reason::FourArms;
        }
        self.lift(w);
        let tries = [
            (
                Relax {
                    wires: true,
                    ..Relax::default()
                },
                Reason::Wires,
            ),
            (
                Relax {
                    wires: true,
                    points: true,
                },
                Reason::Points,
            ),
        ];
        for (relax, why) in tries {
            if self.search(w, &[], relax).is_some() {
                return why;
            }
        }
        Reason::Parts
    }
}

/// The marks of `page` whose wires `changed` touches, each with the move that sets it on its wires again (§3.10): onto
/// the stretch nearest its place where its wires run side by side within [`TRACK_PITCH`] of each other, across them;
/// with no such stretch, onto its first wire alone. A mark that still lies across its wires so is not moved.
fn marks_follow(page: &DraftPage, changed: &BTreeSet<WireId>) -> Vec<(MarkId, Pt)> {
    let mut out = Vec::new();
    let mut marks: Vec<_> = page.marks.iter().collect();
    marks.sort_by_key(|m| m.id);
    for m in marks {
        // (A wire's own marker stands where its wire's route sets it.)
        if !m.wires.iter().any(|w| changed.contains(w)) || crate::marks::mark_point(m).is_none() {
            continue;
        }
        let c = m.line.centre();
        // The stretches of each wire across the mark's line: vertical ones under a horizontal line.
        let runs: Vec<Vec<(i32, i32, i32)>> = m
            .wires
            .iter()
            .map(|w| {
                let route = page
                    .wires
                    .iter()
                    .find(|x| x.id == *w)
                    .map_or(&[][..], |x| x.route.as_slice());
                geom::runs(&geom::clean(route))
                    .into_iter()
                    .filter_map(|(a, b)| {
                        if m.horizontal && a.x == b.x && a.y != b.y {
                            Some((a.x, a.y.min(b.y), a.y.max(b.y)))
                        } else if !m.horizontal && a.y == b.y && a.x != b.x {
                            Some((a.y, a.x.min(b.x), a.x.max(b.x)))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();
        let along_c = if m.horizontal { c.y } else { c.x };
        // The lines on the grid where every wire has a stretch strictly across, their places within the pitch.
        let fits = |v: i32, wires: &[Vec<(i32, i32, i32)>]| -> bool {
            let mut at: Vec<i32> = Vec::new();
            for runs in wires {
                let Some(&(x, _, _)) = runs.iter().find(|(_, lo, hi)| *lo < v && v < *hi) else {
                    return false;
                };
                at.push(x);
            }
            at.sort();
            at.windows(2).all(|w| w[1] - w[0] <= TRACK_PITCH)
        };
        if fits(along_c, &runs) {
            continue;
        }
        let (lo, hi) = runs
            .iter()
            .flatten()
            .fold((i32::MAX, i32::MIN), |(lo, hi), (_, a, b)| {
                (lo.min(*a), hi.max(*b))
            });
        let near = |wires: &[Vec<(i32, i32, i32)>]| -> Option<i32> {
            let step = TICKS_PER_STEP;
            let mut best: Option<(i32, i32)> = None;
            let mut v = grid_down(lo);
            while v <= hi {
                if fits(v, wires) {
                    let d = (v - along_c).abs();
                    if best.is_none_or(|(bd, bv)| (d, v) < (bd, bv)) {
                        best = Some((d, v));
                    }
                }
                v += step;
            }
            best.map(|(_, v)| v)
        };
        let Some(v) =
            near(&runs).or_else(|| runs.first().and_then(|first| near(std::slice::from_ref(first))))
        else {
            continue;
        };
        // The move: the line onto `v`; a mark's line reaches across to the wires it names wherever it stands along it.
        let d_along = v - along_c;
        let d = if m.horizontal {
            Pt::new(0, d_along)
        } else {
            Pt::new(d_along, 0)
        };
        if d != Pt::default() {
            out.push((m.id, d));
        }
    }
    out
}
