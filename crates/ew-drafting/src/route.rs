// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The router (docs/design/06a-drafting.md §3.2, §3.6 to §3.10): [`route`] makes a page's plan for the wires its
//! caller names ([`jobs`]), [`route_wire`] the Wire tool's route between two ends, on the same occupancy and search.
//!
//! # The run
//!
//! 1. **The drawing it starts from.** The jobs' stored routes are lifted; a junction whose every wire is a job may
//!    move. The labels are placed against what stays (the starting places), and the occupancy is built over the parts,
//!    the pins, the junctions, the fixed wires and the texts (a label at its place, a fixed text; not the texts of a
//!    mark or of a wire).
//! 2. **Straights.** A net of one wire whose ends face each other on one line is that segment, where it is free.
//! 3. **Trunks** (`nets`): a net whose taps form a chain, or a pin's fan, gets the trunk of least price; in the order
//!    of their supply ranks, then their first tapped pin's place (left first), then more taps first, then the ID.
//! 4. **Tracks** (`tracks`): a net of one wire still without a route, between two pins in the shape of a Z or a U,
//!    takes the track its channel assigns it, where the route is clear and pays no more for crossings and texts than
//!    the way the search would find for it; the others go on.
//! 5. **A single tap.** A junction that is a net's only tap stands where its wires cost least with three arms.
//! 6. **The search** for the rest, by the city-block distance of their ends (shortest first), then the first end's
//!    place, then the ID; then at most [`PASSES`] passes in the same order, each wire lifted, searched again and its
//!    new route kept where it costs less. A wire ending at a junction that the other wires leave in three directions
//!    is not searched (its route would be a fourth arm). Then the **rings** (`rounds`): the searched wires that go
//!    round one end of one body, two or more, laid again together as nested rings, kept where the page costs no more.
//! 7. **Texts** (§3.7): the labels placed again against all wires; a job that still runs across a text is searched
//!    once more with the texts it crosses at their full price, and the new route kept where the page's cost is
//!    lower; at most [`TEXT_ROUNDS`] rounds.
//! 8. **Weighed against the stored drawing** (§3.9): a net whose stored routes cost no less and keep free of faults
//!    among the others keeps them; and from the stored drawing, a net takes its new routes where they cost less. The
//!    cheaper of the two pages stands. Then wire by wire: a job whose stored route still meets its ends takes it back
//!    where the page costs no more and it keeps free of faults, and a job without a stored route that the routes kept
//!    leave with a fault is searched again among them. The figures §1.2 only reports (length, labels moved, wires
//!    straight, turn-backs) veto nothing.
//! 9. **Never worse** (§1.2): the plan is offered only when the page is better by [`measure`] and no wire routed
//!    before that was free of a finding (F2 to F6, C4) gets one; nor a text free of C5, where the plan routes no wire
//!    that had no route. A net that breaks it goes back to its stored drawing once; then the rule decides. A plan
//!    that routes a wire that had no route is not refused for a text across a wire routed before (C4): only for a
//!    fault on one.
//! 10. **Marks follow** (§3.10) the wires they name, and the page with them moved, the page that is stored, is judged
//!     again by 9: where a moved mark makes it worse, nothing is offered.
//!
//! A job without a way keeps its stored route, or stays unrouted, and is named with the first thing in its way.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;

use crate::consts::{
    BEND, HALF, LEFT_TO_SEARCH, PASSES, TAKE_BACK_ROUNDS, TEXT_ROUNDS, TICKS_PER_STEP, TRACK_PITCH,
};
use crate::geom;
use crate::labels::settle_labels;
use crate::measure::{Quality, TextRef, measure};
use crate::model::{
    DraftPage, End, JunctionId, MarkId, Owner, PartId, PinId, Pt, Rect, Side, Tick, WireId, grid_down,
};
use crate::nets::{self, JobNet, Line};
use crate::occupancy::{Occupancy, Text, node_of};
use crate::search::{self, Ask, Found, Relax, Tip, TipKind, heading};
use crate::{rounds, tracks};

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
    /// The router over `page` with the routes of `jobs` lifted.
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

/// A net's routes on a trunk: per wire its route, or `None` when its drop is not straight; the junction taps' places.
type TrunkRoutes = (BTreeMap<WireId, Option<Vec<Pt>>>, BTreeMap<JunctionId, Pt>);

/// The routes of a net on trunk `line`; `None` when the line breaks the spine's order.
fn trunk_routes(r: &Router, net: &JobNet, line: Line) -> Option<TrunkRoutes> {
    let movable: BTreeSet<JunctionId> = net
        .taps
        .iter()
        .filter_map(|t| match t {
            End::Junction(j) => Some(*j),
            End::Pin(_) => None,
        })
        .collect();
    // The foot of each end.
    let mut feet: BTreeMap<End, Pt> = BTreeMap::new();
    for w in &net.wires {
        for e in r.ends.get(w)? {
            let at = r.point(*e)?;
            let foot = match e {
                End::Pin(p) => {
                    let side = r.pins.get(p)?.side;
                    let on =
                        line.across(at) == line.at && side.is_some_and(|s| s.vertical() != line.horizontal);
                    if on { at } else { line.foot(at) }
                }
                End::Junction(j) if movable.contains(j) => {
                    // The pin it drops to: the one pin among its wires' other ends that faces the line.
                    let drops: Vec<Pt> = net
                        .wires
                        .iter()
                        .filter_map(|x| {
                            let [a, b] = r.ends.get(x)?;
                            let other = if a == e {
                                *b
                            } else if b == e {
                                *a
                            } else {
                                return None;
                            };
                            let End::Pin(p) = other else { return None };
                            let pin = r.pins.get(&p)?;
                            (pin.side.is_some()
                                && pin.side == line.facing(pin.at)
                                && !net.taps.contains(&other))
                            .then_some(pin.at)
                        })
                        .collect();
                    // Of several, the nearest: the junction stands over one of its drops, so that no two of its wires
                    // come to it along one stretch of the trunk.
                    match drops
                        .iter()
                        .min_by_key(|p| ((line.along(**p) - line.along(at)).abs(), line.along(**p)))
                    {
                        Some(one) => line.foot(*one),
                        None => line.foot(at),
                    }
                }
                End::Junction(_) => line.foot(at),
            };
            feet.insert(*e, foot);
        }
    }
    // The spine's order: strictly along one way; two junction taps on one point set one step apart.
    if let Some(spine) = &net.spine {
        let mut prev: Option<i32> = None;
        let dir = spine
            .first()
            .zip(spine.last())
            .map(|(a, b)| (line.along(feet[b]) - line.along(feet[a])).signum())
            .unwrap_or(1);
        if dir == 0 {
            return None;
        }
        for t in spine {
            let mut v = line.along(feet[t]);
            if let Some(p) = prev {
                if (v - p).signum() != dir {
                    match t {
                        End::Junction(j) if movable.contains(j) && v == p => {
                            v = p + dir * TICKS_PER_STEP;
                            let f = feet[t];
                            let moved = if line.horizontal {
                                Pt::new(v, f.y)
                            } else {
                                Pt::new(f.x, v)
                            };
                            feet.insert(*t, moved);
                        }
                        _ => return None,
                    }
                }
            }
            prev = Some(v);
        }
    }
    let mut routes = BTreeMap::new();
    for w in &net.wires {
        let [a, b] = *r.ends.get(w)?;
        let (pa, pb) = (r.point(a)?, r.point(b)?);
        let start = if matches!(a, End::Junction(j) if movable.contains(&j)) {
            feet[&a]
        } else {
            pa
        };
        let end = if matches!(b, End::Junction(j) if movable.contains(&j)) {
            feet[&b]
        } else {
            pb
        };
        // A pin's drop is straight only along its side towards the line.
        let drops_straight = |e: End, p: Pt| match e {
            End::Pin(pin) => {
                let side = r.pins.get(&pin).and_then(|x| x.side);
                feet[&e] == p || (side.is_some() && side == line.facing(p))
            }
            End::Junction(_) => true,
        };
        let ok = drops_straight(a, pa) && drops_straight(b, pb);
        let route = search::clean_collinear(&[start, feet[&a], feet[&b], end]);
        routes.insert(*w, ok.then_some(route));
    }
    let places = movable.iter().map(|j| (*j, feet[&End::Junction(*j)])).collect();
    Some((routes, places))
}

/// The price of `route` for its length and its bends alone, in the search's unit (ticks).
fn plain(route: &[Pt]) -> u32 {
    let pts = geom::clean(route);
    let length: i64 = geom::segments(&pts).into_iter().map(geom::length).sum();
    let bends = i64::try_from(geom::corners(&pts).len()).unwrap_or(i64::MAX);
    u32::try_from(length.saturating_add(bends.saturating_mul(BEND * i64::from(TICKS_PER_STEP))))
        .unwrap_or(u32::MAX)
}

/// Whether a node of the occupancy lies on a laid wire (a vertex of one, or between two moves along one).
fn on_a_wire(occ: &Occupancy, p: Pt) -> bool {
    let n = node_of(p);
    occ.vertices_at(n) > 0
        || (occ.along(((n.0 - 1, n.1), false)) > 0 && occ.along((n, false)) > 0)
        || (occ.along(((n.0, n.1 - 1), true)) > 0 && occ.along((n, true)) > 0)
}

impl Router {
    /// Phase 2: the nets of one wire whose ends face each other on one line, straight where that is free.
    fn straights(&mut self, nets: &[JobNet]) -> BTreeSet<WireId> {
        let mut laid = BTreeSet::new();
        for net in nets.iter().filter(|n| n.wires.len() == 1) {
            let w = net.wires[0];
            let Some([a, b]) = self.ends.get(&w).copied() else {
                continue;
            };
            let (Some(pa), Some(pb)) = (self.point(a), self.point(b)) else {
                continue;
            };
            if pa == pb || (pa.x != pb.x && pa.y != pb.y) {
                continue;
            }
            let faces = |e: End, from: Pt, to: Pt| match e {
                End::Pin(p) => self.pins.get(&p).and_then(|x| x.side) == heading(from, to),
                End::Junction(_) => true,
            };
            if !faces(a, pa, pb) || !faces(b, pb, pa) || self.fourth_arm(w) {
                continue;
            }
            let route = vec![pa, pb];
            if self.clear(w, &route) {
                self.lay(w, route);
                laid.insert(w);
            }
        }
        laid
    }

    /// Phase 3: the trunks (module documentation); the wires laid, the nets whose order ran against their places.
    fn trunks(&mut self, nets: &[JobNet]) -> (BTreeSet<WireId>, Vec<Vec<WireId>>) {
        let mut laid = BTreeSet::new();
        let mut notes = Vec::new();
        let supply = |r: &Router, net: &JobNet| {
            net.wires
                .iter()
                .filter_map(|w| r.page.wires.iter().find(|x| x.id == *w)?.supply)
                .min()
                .unwrap_or(u8::MAX)
        };
        let mut order: Vec<_> = nets
            .iter()
            .enumerate()
            .filter(|(_, n)| n.wires.len() >= 2 && (n.spine.is_some() || is_fan(n)))
            .map(|(k, n)| {
                let first = n
                    .wires
                    .iter()
                    .flat_map(|w| self.ends.get(w).into_iter().flatten())
                    .filter_map(|e| match e {
                        End::Pin(p) => self.pins.get(p).map(|x| (x.at.x, x.at.y)),
                        End::Junction(_) => None,
                    })
                    .min()
                    .unwrap_or((i32::MAX, i32::MAX));
                (supply(self, n), first, usize::MAX - n.taps.len(), n.wires[0], k)
            })
            .collect();
        order.sort();
        for (_, _, _, _, k) in order {
            let net = &nets[k];
            let pins: Vec<(Pt, Option<Side>)> = net
                .wires
                .iter()
                .flat_map(|w| self.ends.get(w).into_iter().flatten())
                .filter_map(|e| match e {
                    End::Pin(p) => self.pins.get(p).map(|x| (x.at, x.side)),
                    End::Junction(_) => None,
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let junctions: Vec<Pt> = net
                .taps
                .iter()
                .filter_map(|t| match t {
                    End::Junction(j) => self.junction_at(*j),
                    End::Pin(_) => None,
                })
                .collect();
            let area = self.page.area;
            let lines = nets::candidates(&pins, &junctions, (area.top_left(), Pt::new(area.x1, area.y1)));
            let mut best: Option<((u64, u8, u8, i32), TrunkRoutes)> = None;
            let mut against_order = false;
            for line in lines {
                let Some((routes, places)) = trunk_routes(self, net, line) else {
                    against_order |= net.spine.is_some();
                    continue;
                };
                // The junction taps at their places for the test, and back.
                let old: Vec<(JunctionId, Pt)> = places
                    .keys()
                    .filter_map(|j| Some((*j, self.junction_at(*j)?)))
                    .collect();
                let mut valid = places
                    .values()
                    .all(|p| area.contains_strictly(*p) && !on_a_wire(&self.occ, *p));
                for (j, p) in &places {
                    self.move_junction(*j, *p);
                }
                let mut cost: u64 = 0;
                let mut kept = BTreeMap::new();
                let mut clear_count = 0usize;
                // Two wires of one trunk may share only their run out of a pin both end at: a wire that would lie along
                // another one of the trunk laid before it (two wires of a junction that leave it the same way) goes to
                // the search.
                let mut tried: Vec<(WireId, Vec<Pt>)> = Vec::new();
                for (w, route) in &routes {
                    let spine_wire = self
                        .ends
                        .get(w)
                        .is_some_and(|e| e.iter().all(|x| net.taps.contains(x)));
                    let pin_of = |x: WireId| -> Vec<End> {
                        self.ends
                            .get(&x)
                            .into_iter()
                            .flatten()
                            .copied()
                            .filter(|e| matches!(e, End::Pin(_)))
                            .collect()
                    };
                    let along = route.as_ref().is_some_and(|rt| {
                        tried.iter().any(|(o, other)| {
                            !pin_of(*w).iter().any(|e| pin_of(*o).contains(e))
                                && geom::segments(rt).iter().any(|s| {
                                    geom::segments(other)
                                        .iter()
                                        .any(|t| geom::overlap(*s, *t).is_some())
                                })
                        })
                    });
                    let ok = valid && !along && route.as_ref().is_some_and(|rt| self.clear(*w, rt));
                    if ok {
                        clear_count += 1;
                        cost += u64::from(self.price(*w, route.as_deref().unwrap_or(&[])));
                        kept.insert(*w, route.clone());
                        if let Some(rt) = route {
                            tried.push((*w, rt.clone()));
                        }
                    } else {
                        if spine_wire {
                            valid = false;
                        }
                        // A wire left to the search: its ends' distance and two bends.
                        let [a, b] = self
                            .ends
                            .get(w)
                            .copied()
                            .unwrap_or([End::Pin(PinId(Uuid::nil())); 2]);
                        let (pa, pb) = (
                            self.point(a).unwrap_or_default(),
                            self.point(b).unwrap_or_default(),
                        );
                        cost += u64::try_from(geom::length((pa, pb))).unwrap_or(0)
                            + u64::try_from(LEFT_TO_SEARCH).unwrap_or(0);
                        kept.insert(*w, None);
                    }
                }
                for (j, p) in old {
                    self.move_junction(j, p);
                }
                if !valid || clear_count < 2 {
                    continue;
                }
                let key = (cost, u8::from(!line.now), u8::from(!line.horizontal), line.at);
                if best.as_ref().is_none_or(|(k, _)| key < *k) {
                    best = Some((key, (kept, places)));
                }
            }
            let Some((_, (routes, places))) = best else {
                if against_order {
                    notes.push(net.wires.clone());
                }
                continue;
            };
            for (j, p) in places {
                self.move_junction(j, p);
            }
            for (w, route) in routes {
                if let Some(rt) = route {
                    self.lay(w, rt);
                    laid.insert(w);
                }
            }
        }
        (laid, notes)
    }

    /// Phase 4: the stepping wires (`tracks`): the nets of one wire still without a route between two pins in the shape
    /// of a Z or a U (a pin's wires together are the trunks' and the search's, which share their runs), on the tracks of their channels where the route is clear and pays no more for crossings and texts than the
    /// search's way for the wire; the wires laid.
    fn tracks(&mut self, nets: &[JobNet]) -> BTreeSet<WireId> {
        let area = self.page.area;
        let mut frames: BTreeMap<tracks::Frame, Vec<(WireId, tracks::Lane)>> = BTreeMap::new();
        for net in nets.iter().filter(|n| n.wires.len() == 1) {
            let w = net.wires[0];
            if !self.route_of(w).is_empty() {
                continue;
            }
            let Some([End::Pin(a), End::Pin(b)]) = self.ends.get(&w).copied() else {
                continue;
            };
            let (Some(pa), Some(pb)) = (self.pins.get(&a), self.pins.get(&b)) else {
                continue;
            };
            let (Some(sa), Some(sb)) = (pa.side, pb.side) else {
                continue;
            };
            if let Some((frame, lane)) = tracks::stepping((pa.at, sa, a), (pb.at, sb, b), area) {
                frames.entry(frame).or_default().push((w, lane));
            }
        }
        let mut laid = BTreeSet::new();
        // Per wire, the price of the way the search would find for it, asked when its first track is tried.
        let mut searched: BTreeMap<WireId, u32> = BTreeMap::new();
        for (frame, wires) in frames {
            let lanes: Vec<tracks::Lane> = wires.iter().map(|(_, l)| l.clone()).collect();
            tracks::lay(&lanes, &mut |k, d| {
                let w = wires[k].0;
                let Some([a, b]) = self.ends.get(&w).copied() else {
                    return false;
                };
                let (Some(pa), Some(pb)) = (self.point(a), self.point(b)) else {
                    return false;
                };
                let route = search::clean_collinear(&frame.route(pa, pb, d));
                if !self.clear(w, &route) {
                    return false;
                }
                // A track whose route pays more than the search's way for crossings and texts (it crosses what that
                // way avoids) is blocked too; the length and the bends of the pitch are the track's to spend.
                let way = *searched.entry(w).or_insert_with(|| {
                    self.search(w, &[], Relax::default())
                        .map_or(u32::MAX, |f| f.cost.saturating_sub(plain(&f.route)))
                });
                if self.price(w, &route).saturating_sub(plain(&route)) > way {
                    return false;
                }
                self.lay(w, route);
                laid.insert(w);
                true
            });
        }
        laid
    }

    /// Phase 6b: the wires round a body (`rounds`). The wires the search laid one by one that run between two pins, one
    /// of them on a body the wire has to go round, each round the end whose way passes the fewest other wired pins of
    /// its edge (then the shorter); those round one end of one body, two or more, are laid again together as nested
    /// rings, the pin nearest the corner innermost (a wire whose ring is not clear keeps its route), and the rings are
    /// kept where the page costs no more.
    fn rings(&mut self, nets: &[JobNet], searched: &[WireId]) {
        let wired = self.page.wired();
        type Key = (PartId, Side, rounds::Corner);
        let mut groups: BTreeMap<Key, Vec<(Tick, WireId, rounds::Ask)>> = BTreeMap::new();
        for net in nets.iter().filter(|n| n.wires.len() == 1) {
            let w = net.wires[0];
            if !searched.contains(&w) {
                continue;
            }
            let Some([End::Pin(a), End::Pin(b)]) = self.ends.get(&w).copied() else {
                continue;
            };
            for (p, q) in [(a, b), (b, a)] {
                let (Some(pp), Some(pq)) = (self.pins.get(&p), self.pins.get(&q)) else {
                    continue;
                };
                let (Some(sp), Some(sq)) = (pp.side, pq.side) else {
                    continue;
                };
                let ask = rounds::Ask {
                    pin: (pp.at, sp),
                    body: pp.body,
                    other: (pq.at, sq),
                    wired: self
                        .pins
                        .iter()
                        .filter(|(id, x)| {
                            **id != p && x.part == pp.part && x.side == Some(sp) && wired.contains(*id)
                        })
                        .map(|(_, x)| x.at)
                        .collect(),
                };
                let best = [rounds::Corner::First, rounds::Corner::Last]
                    .into_iter()
                    .filter_map(|c| {
                        let r = ask.route(c, TICKS_PER_STEP)?;
                        Some(((ask.passes(c), plain(&r)), c))
                    })
                    .min();
                if let Some((_, c)) = best {
                    groups
                        .entry((pp.part, sp, c))
                        .or_default()
                        .push((ask.reach(c), w, ask));
                    break;
                }
            }
        }
        for ((_, _, corner), mut wires) in groups {
            if wires.len() < 2 {
                continue;
            }
            wires.sort_by_key(|(reach, w, _)| (*reach, *w));
            let before = measure(&self.page).key();
            let old: Vec<(WireId, Vec<Pt>)> = wires
                .iter()
                .map(|(_, w, _)| (*w, self.route_of(*w).to_vec()))
                .collect();
            for (w, _) in &old {
                self.lift(*w);
            }
            let mut ring: Tick = 0;
            for ((_, w, ask), (_, was)) in wires.iter().zip(&old) {
                let route = ask
                    .route(corner, (ring + 1) * TICKS_PER_STEP)
                    .map(|r| search::clean_collinear(&r))
                    .filter(|r| self.clear(*w, r));
                if let Some(route) = route {
                    self.lay(*w, route);
                    ring += 1;
                } else {
                    self.lay(*w, was.clone());
                }
            }
            if measure(&self.page).key() > before {
                for (w, was) in old {
                    self.lay(w, was);
                }
            }
        }
    }

    /// Phase 5: a net whose only tap is a junction that may move: the junction where its wires cost least.
    fn single_taps(&mut self, nets: &[JobNet]) -> BTreeSet<WireId> {
        let mut laid = BTreeSet::new();
        for net in nets {
            let [End::Junction(j)] = net.taps.as_slice() else {
                continue;
            };
            if net.wires.iter().any(|w| !self.route_of(*w).is_empty()) {
                continue;
            }
            let Some(present) = self.junction_at(*j) else {
                continue;
            };
            let ends: Vec<(Pt, Option<Side>)> = net
                .wires
                .iter()
                .filter_map(|w| {
                    let e = self.ends.get(w)?;
                    let other = if e[0] == End::Junction(*j) { e[1] } else { e[0] };
                    let side = match other {
                        End::Pin(p) => self.pins.get(&p)?.side,
                        End::Junction(_) => None,
                    };
                    Some((self.point(other)?, side))
                })
                .collect();
            let area = self.page.area;
            let mut best: Option<(u64, usize, Pt)> = None;
            for (k, c) in nets::tap_points(present, &ends).into_iter().enumerate() {
                if !area.contains_strictly(c) {
                    continue;
                }
                if c != present && (on_a_wire(&self.occ, c) || self.occ.bodies_at(node_of(c), false) > 0) {
                    continue;
                }
                if c != present && !self.occ.holders_at(node_of(c)).is_empty() {
                    continue;
                }
                self.move_junction(*j, c);
                let mut order = net.wires.clone();
                order.sort_by_key(|w| self.order_key(*w));
                let mut total: Option<u64> = Some(0);
                for w in &order {
                    match self.search(*w, &[], Relax::default()) {
                        Some(f) => {
                            total = total.map(|t| t + u64::from(f.cost));
                            self.lay(*w, f.route);
                        }
                        None => {
                            total = None;
                            break;
                        }
                    }
                }
                for w in &order {
                    self.lift(*w);
                }
                self.move_junction(*j, present);
                if let Some(t) = total
                    && best.is_none_or(|(b, i, _)| (t, k) < (b, i))
                {
                    best = Some((t, k, c));
                }
            }
            let Some((_, _, c)) = best else { continue };
            self.move_junction(*j, c);
            let mut order = net.wires.clone();
            order.sort_by_key(|w| self.order_key(*w));
            for w in order {
                if let Some(f) = self.search(w, &[], Relax::default()) {
                    self.lay(w, f.route);
                    laid.insert(w);
                }
            }
        }
        laid
    }

    /// Phase 6: the search for the jobs without a route, then the passes; the wires searched in order, and those with
    /// no way. A job without a way keeps its stored route at once, so the jobs after it keep clear of it (§3.8); each pass
    /// tries it again first.
    fn searches(&mut self) -> (Vec<WireId>, BTreeSet<WireId>) {
        let mut rest: Vec<WireId> = self
            .jobs
            .iter()
            .copied()
            .filter(|w| self.route_of(*w).is_empty())
            .collect();
        rest.sort_by_key(|w| self.order_key(*w));
        let mut searched = Vec::new();
        let mut failed = BTreeSet::new();
        for w in rest {
            if self.points_meet(w) {
                continue;
            }
            let found = if self.fourth_arm(w) {
                None
            } else {
                self.search(w, &[], Relax::default())
            };
            match found {
                Some(f) => {
                    self.lay(w, f.route);
                    searched.push(w);
                }
                None => {
                    self.keep_stored(w);
                    failed.insert(w);
                }
            }
        }
        for _ in 0..PASSES {
            let mut changed = false;
            for w in failed.clone() {
                let old = self.route_of(w).to_vec();
                self.lift(w);
                match self
                    .search(w, &[], Relax::default())
                    .filter(|_| !self.fourth_arm(w))
                {
                    Some(f) => {
                        self.lay(w, f.route);
                        failed.remove(&w);
                        searched.push(w);
                        changed = true;
                    }
                    None if old.is_empty() => {}
                    None => self.lay(w, old),
                }
            }
            for w in searched.clone() {
                let old = self.route_of(w).to_vec();
                self.lift(w);
                let was = self.price(w, &old);
                match self.search(w, &[], Relax::default()) {
                    Some(f) if f.route != old && f.cost < was => {
                        self.lay(w, f.route);
                        changed = true;
                    }
                    _ => self.lay(w, old),
                }
            }
            if !changed {
                break;
            }
        }
        (searched, failed)
    }

    /// Job `w` on its stored route, where it has one (a job without a way, §3.8).
    fn keep_stored(&mut self, w: WireId) {
        if let Some(s) = self.stored.get(&w).filter(|s| !s.is_empty()).cloned() {
            self.lay(w, s);
        }
    }

    fn points_meet(&self, w: WireId) -> bool {
        let Some([a, b]) = self.ends.get(&w).copied() else {
            return true;
        };
        match (self.point(a), self.point(b)) {
            (Some(pa), Some(pb)) => pa == pb,
            _ => true,
        }
    }

    /// Phase 7: the text rounds (module documentation).
    fn text_rounds(&mut self) {
        for _ in 0..TEXT_ROUNDS {
            let q = measure(&self.page);
            let mut crossing: BTreeMap<WireId, Vec<TextRef>> = BTreeMap::new();
            for (w, t, _) in &q.findings.across_texts {
                if self.jobs.contains(w) {
                    crossing.entry(*w).or_default().push(*t);
                }
            }
            if crossing.is_empty() {
                return;
            }
            let texts = self.texts(&q.labels);
            if self.rebuild(texts).is_none() {
                return;
            }
            let mut cost = q.cost;
            let mut changed = false;
            for (w, refs) in crossing {
                let full: Vec<u32> = refs
                    .iter()
                    .filter_map(|t| {
                        self.occ.texts.iter().position(|x| match t {
                            TextRef::Label(id) => x.label == Some(*id),
                            TextRef::Fixed(owner, rect) => {
                                x.label.is_none() && x.owner == *owner && x.rect == *rect
                            }
                        })
                    })
                    .filter_map(|k| u32::try_from(k).ok())
                    .collect();
                let old = self.route_of(w).to_vec();
                self.lift(w);
                match self.search(w, &full, Relax::default()) {
                    Some(f) if f.route != old => {
                        self.lay(w, f.route);
                        let now = measure(&self.page);
                        if now.key() < (q.faults, q.unrouted, cost) && now.faults <= q.faults {
                            cost = now.cost;
                            changed = true;
                        } else {
                            self.lay(w, old);
                        }
                    }
                    _ => self.lay(w, old),
                }
            }
            if !changed {
                return;
            }
        }
    }

    /// The page with net `net`'s wires on their stored routes and its junctions at their first places.
    fn stored_page(&self, net: &JobNet) -> DraftPage {
        self.stored_on(&self.page, net)
    }

    /// `base` with net `net`'s wires on their stored routes and its junctions at their first places.
    fn stored_on(&self, base: &DraftPage, net: &JobNet) -> DraftPage {
        net_on(
            base,
            net,
            |w| self.stored.get(&w).map(Vec::as_slice),
            |j| self.first.get(&j).copied(),
        )
    }

    /// Phase 8: the nets with a stored drawing weighed against it (module documentation). Back: from the new drawing, a
    /// net whose stored routes cost no less and keep free of faults among the others keeps them. Forward: from the
    /// stored drawing, a net takes its new routes where they cost less and keep free of faults among the others. The
    /// cheaper page stands: one net's gain is not lost to another's loss that no single net's going back undoes.
    fn weigh(&mut self, nets: &[JobNet]) {
        let weighed: Vec<&JobNet> = nets
            .iter()
            .filter(|net| {
                net.wires
                    .iter()
                    .all(|w| self.stored.get(w).is_some_and(|r| !r.is_empty()))
            })
            .collect();
        let new = self.page.clone();
        for net in &weighed {
            let alt = self.stored_page(net);
            if alt == self.page {
                continue;
            }
            let (qa, qn) = (measure(&alt), measure(&self.page));
            let faulty: BTreeSet<WireId> = wires_with_faults(&qa);
            if qa.key() <= qn.key() && net.wires.iter().all(|w| !faulty.contains(w)) {
                self.page = alt;
            }
        }
        let mut forward = new.clone();
        for net in &weighed {
            forward = self.stored_on(&forward, net);
        }
        let mut qf = measure(&forward);
        for net in &weighed {
            let taken = taken_from(&forward, net, &new);
            if taken == forward {
                continue;
            }
            let qt = measure(&taken);
            let faulty: BTreeSet<WireId> = wires_with_faults(&qt);
            if qt.key() < qf.key() && net.wires.iter().all(|w| !faulty.contains(w)) {
                forward = taken;
                qf = qt;
            }
        }
        if qf.key() < measure(&self.page).key() {
            self.page = forward;
        }
        self.weigh_wires();
    }

    /// Phase 8, wire by wire: a net's new routes are not all or nothing. A job whose stored route still meets its ends
    /// takes it back where the page costs no more and it keeps free of faults; in the order of the jobs' IDs, at most
    /// [`TAKE_BACK_ROUNDS`] rounds, until a round takes none back. Then a job without a stored route left with a fault is searched
    /// again.
    fn weigh_wires(&mut self) {
        let mut q = measure(&self.page);
        let jobs: Vec<WireId> = self.jobs.iter().copied().collect();
        for _ in 0..TAKE_BACK_ROUNDS {
            let mut taken = false;
            for w in &jobs {
                let Some(s) = self.stored.get(w).filter(|s| !s.is_empty()) else {
                    continue;
                };
                let ends = self.ends.get(w).map(|e| e.map(|e| self.point(e)));
                let meets = ends.is_some_and(|[a, b]| a == s.first().copied() && b == s.last().copied());
                if !meets || self.route_of(*w) == s.as_slice() {
                    continue;
                }
                let mut alt = self.page.clone();
                if let Some(x) = alt.wires.iter_mut().find(|x| x.id == *w) {
                    x.route.clone_from(s);
                }
                let qa = measure(&alt);
                if qa.key() <= q.key() && !wires_with_faults(&qa).contains(w) {
                    self.page = alt;
                    q = qa;
                    taken = true;
                }
            }
            if !taken {
                break;
            }
        }
        // A job without a stored route that the routes kept leave with a fault is searched again among them, and its
        // new route kept where the page costs less.
        let faulty = wires_with_faults(&q);
        let fresh: Vec<WireId> = jobs
            .into_iter()
            .filter(|w| faulty.contains(w) && self.stored.get(w).is_none_or(Vec::is_empty))
            .collect();
        if fresh.is_empty() || self.rebuild(self.texts(&q.labels)).is_none() {
            return;
        }
        for w in fresh {
            let old = self.route_of(w).to_vec();
            self.lift(w);
            match self.search(w, &[], Relax::default()) {
                Some(f) if f.route != old => {
                    self.lay(w, f.route);
                    let now = measure(&self.page);
                    if now.key() < q.key() {
                        q = now;
                    } else {
                        self.lay(w, old);
                    }
                }
                _ => self.lay(w, old),
            }
        }
    }
}

/// `page` with net `net`'s wires on the routes and its junctions at the places they have in `from`.
fn taken_from(page: &DraftPage, net: &JobNet, from: &DraftPage) -> DraftPage {
    net_on(
        page,
        net,
        |w| from.wires.iter().find(|x| x.id == w).map(|x| x.route.as_slice()),
        |j| from.junctions.iter().find(|x| x.id == j).map(|x| x.at),
    )
}

/// `base` with net `net`'s wires on the routes `route` gives and its junctions at the places `at` gives; a wire or
/// junction they give none for stays as it is.
fn net_on<'a>(
    base: &DraftPage,
    net: &JobNet,
    route: impl Fn(WireId) -> Option<&'a [Pt]>,
    at: impl Fn(JunctionId) -> Option<Pt>,
) -> DraftPage {
    let mut page = base.clone();
    for w in &mut page.wires {
        if net.wires.contains(&w.id)
            && let Some(r) = route(w.id)
        {
            w.route.clear();
            w.route.extend_from_slice(r);
        }
    }
    for j in &mut page.junctions {
        if net.taps.contains(&End::Junction(j.id))
            && let Some(p) = at(j.id)
        {
            j.at = p;
        }
    }
    page
}

fn is_fan(net: &JobNet) -> bool {
    matches!(net.taps.as_slice(), [End::Pin(_)])
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

/// The plan of a run over `page` for `jobs` (module documentation). Deterministic: the same page in any order of its
/// lists gives the same plan.
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

/// An end of the Wire tool's route.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireEnd {
    /// A pin.
    Pin(PinId),
    /// A junction.
    Junction(JunctionId),
    /// A point on a wire, where a junction will be made.
    OnWire(WireId, Pt),
    /// A free point (a fixed bend).
    Point(Pt),
}

/// The Wire tool's route, and whether it runs across a text that cannot step aside.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WireRoute {
    /// Every vertex from the first end to the second.
    pub route: Vec<Pt>,
    /// Whether it runs across a fixed text.
    pub over_text: bool,
}

/// The tip of a Wire tool's end on the page of `r`.
fn wire_tip(r: &Router, e: WireEnd) -> Option<Tip> {
    match e {
        WireEnd::Pin(p) => r.tip(WireId(Uuid::nil()), End::Pin(p)),
        WireEnd::Junction(j) => r.tip(WireId(Uuid::nil()), End::Junction(j)),
        WireEnd::OnWire(_, at) | WireEnd::Point(at) => Some(Tip::free(at)),
    }
}

/// The router over `page` for a wire not yet drawn, with the parts `without` left out.
fn tool_router(page: &DraftPage, without: &[PartId]) -> Option<Router> {
    let mut page = page.clone();
    if !without.is_empty() {
        let gone: BTreeSet<Uuid> = without.iter().map(|p| p.0).collect();
        page.parts.retain(|p| !without.contains(&p.id));
        page.labels
            .retain(|l| !matches!(l.owner, Owner::Part(p) if gone.contains(&p.0)));
        page.fixed_texts.retain(|t| !gone.contains(&t.owner));
    }
    Router::new(&page, &[])
}

/// Whether the straight line between the Wire tool's ends `a` and `b` crosses nothing on `page` (§7.4): the two ends on
/// one line, a pin end left along its side and entered against it, no turn back after a fixed bend (`arrive`); on the
/// line no pin or junction but at its ends, no wire along it, across it or meeting it but at its ends, no body it runs
/// through or along but an end's own part's (an outline drawn round other parts that holds an end included), no text
/// that cannot step aside, and none along the wire's own route so far (`before`). A pin's stub and the gap round a
/// body close nothing here as they do to the search: a person who draws a rail one step over a row of pins drops each
/// tap down a stub.
fn crosses_nothing(page: &DraftPage, a: &Tip, b: &Tip, arrive: Option<Side>, before: &[Pt]) -> bool {
    let (Some(h), Some(back)) = (heading(a.at, b.at), heading(b.at, a.at)) else {
        return false;
    };
    if a.side.is_some_and(|s| s != h) || b.side.is_some_and(|s| s != back) || arrive == Some(back) {
        return false;
    }
    let line = (a.at, b.at);
    let end = |p: Pt| p == a.at || p == b.at;
    let own = |id: Uuid| [a, b].iter().any(|t| t.part.is_some_and(|(o, _)| o == id));
    let holds_end = |r: Rect| [a.at, b.at].iter().any(|p| r.contains(*p));
    let mut points = page
        .parts
        .iter()
        .flat_map(|p| p.pins.iter().map(|q| p.at.plus(q.at)))
        .chain(page.junctions.iter().map(|j| j.at));
    if points.any(|q| !end(q) && geom::on(q, line)) {
        return false;
    }
    let in_body = page.parts.iter().any(|p| {
        let body = p.bounds();
        let solid = if p.frame_only {
            !holds_end(body)
        } else {
            !p.pins.is_empty() && !own(p.id.0)
        };
        solid && geom::crosses(line, body.grown(1))
    });
    let on_wire = page
        .wires
        .iter()
        .map(|w| w.route.as_slice())
        .chain([before])
        .any(|route| {
            geom::segments(&geom::clean(route))
                .into_iter()
                .any(|s| geom::overlap(line, s).is_some() || geom::meet(line, s).is_some_and(|p| !end(p)))
        });
    let on_text = page
        .fixed_texts
        .iter()
        .any(|t| geom::crosses(line, t.rect.grown(-1)));
    !in_body && !on_wire && !on_text
}

/// The Wire tool's route from `from` to `to` (§7.4): the straight line between them where it crosses nothing
/// ([`crosses_nothing`]), else the search of [`route`] against the page as drawn, every routed wire fixed, a wire of
/// the same pin's run shared where that is the better way. `before` is the wire's own route so far, ending at a free
/// first end (its fixed bends; empty for a wire from its start): the route goes on from the heading it arrived with and
/// runs along none of it, so that it never runs back over itself (FT-2097). `None` where no route keeps free of faults.
pub fn route_wire(page: &DraftPage, from: WireEnd, to: WireEnd, before: &[Pt]) -> Option<WireRoute> {
    let mut r = tool_router(page, &[])?;
    let arrive = before.windows(2).last().and_then(|w| heading(w[0], w[1]));
    if before.len() >= 2 {
        r.occ.lay(before, true);
    }
    let (a, b) = (wire_tip(&r, from)?, wire_tip(&r, to)?);
    if crosses_nothing(&r.page, &a, &b, arrive, before) {
        return Some(WireRoute {
            route: vec![a.at, b.at],
            over_text: false,
        });
    }
    let found = search::route(&tool_ask(&r, a, b, arrive))?;
    let over_text = across_text(&r, &found.route, true);
    Some(WireRoute {
        route: found.route,
        over_text,
    })
}

/// Whether `route` from `from` to `to` is one the Wire tool's search could take on `page` with the parts `through` left
/// out (their bodies, texts and pins), clear of every other text: the direct route through a free terminal in line.
pub fn wire_clear(
    page: &DraftPage,
    (from, to): (WireEnd, WireEnd),
    arrive: Option<Side>,
    through: &[PartId],
    route: &[Pt],
) -> bool {
    let Some(r) = tool_router(page, through) else {
        return false;
    };
    let (Some(a), Some(b)) = (wire_tip(&r, from), wire_tip(&r, to)) else {
        return false;
    };
    // The direct route keeps clear of every text: a text in its way sends the wire to the search.
    !across_text(&r, route, false) && search::clear(&tool_ask(&r, a, b, arrive), route)
}

/// The Wire tool's search from `from` to `to` on `r`, the route arriving along `arrive`.
fn tool_ask(r: &Router, from: Tip, to: Tip, arrive: Option<Side>) -> Ask<'_> {
    Ask {
        occ: &r.occ,
        from,
        to,
        arrive,
        area: r.page.area,
        full: &[],
        relax: Relax::default(),
    }
}

/// Whether `route` runs through a text of `r`'s occupancy (inside it by more than a tick): any text, or with `fixed`
/// only one that cannot step aside.
fn across_text(r: &Router, route: &[Pt], fixed: bool) -> bool {
    r.occ.texts.iter().any(|t| {
        (!fixed || t.label.is_none())
            && geom::segments(route)
                .iter()
                .any(|s| geom::stretch_inside(*s, t.rect.grown(-1)).is_some())
    })
}

/// The plain shape of a wire from a pin at `a` leaving along `sa` to one at `b` leaving along `sb`, as the placer asks
/// the text placer about it: one corner where the sides turn a quarter; straight where they face each other on one
/// line, else stepping at the middle, its middle on the whole step at or before the half way; none where both leave on
/// one side (out, across and back: the router's to place). Unlike [`unrouted_line`] it steps where pins that face each
/// other stand off one line.
pub(crate) fn plain_line((a, sa): (Pt, Side), (b, sb): (Pt, Side)) -> Option<Vec<Pt>> {
    Some(if sa.vertical() != sb.vertical() {
        let corner = if sa.vertical() {
            Pt::new(a.x, b.y)
        } else {
            Pt::new(b.x, a.y)
        };
        vec![a, corner, b]
    } else if sa == sb {
        return None;
    } else if sa.across(a) == sa.across(b) {
        vec![a, b]
    } else if sa.vertical() {
        let y = grid_down((a.y + b.y).div_euclid(2));
        vec![a, Pt::new(a.x, y), Pt::new(b.x, y), b]
    } else {
        let x = grid_down((a.x + b.x).div_euclid(2));
        vec![a, Pt::new(x, a.y), Pt::new(x, b.y), b]
    })
}

/// The line of a wire without a route between its two ends `a` and `b` (in any one unit): right-angled, never
/// diagonal. Ends on one horizontal or vertical line are joined straight; else by one corner made of the ends' own
/// coordinates: the first leg along the first end's side, else the last leg along the second end's, else the first leg
/// across. It takes no notice of what lies under it: it is the line an unrouted wire is drawn with, dashed, and no wire.
pub fn unrouted_line(
    (a, a_side): ((i64, i64), Option<Side>),
    (b, b_side): ((i64, i64), Option<Side>),
) -> Vec<(i64, i64)> {
    if a.0 == b.0 || a.1 == b.1 {
        return vec![a, b];
    }
    let vertical_first = match (a_side, b_side) {
        (Some(d), _) => d.vertical(),
        (None, Some(d)) => !d.vertical(),
        (None, None) => false,
    };
    let corner = if vertical_first { (a.0, b.1) } else { (b.0, a.1) };
    vec![a, corner, b]
}
