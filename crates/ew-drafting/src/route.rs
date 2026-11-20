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
//! 4. **A single tap.** A junction that is a net's only tap stands where its wires cost least with three arms.
//! 5. **The search** for the rest, by the city-block distance of their ends (shortest first), then the first end's
//!    place, then the ID; then at most [`PASSES`] passes in the same order, each wire lifted, searched again and its
//!    new route kept where it costs less. A wire ending at a junction that the other wires leave in three directions
//!    is not searched (its route would be a fourth arm).
//! 6. **Texts** (§3.7): the labels placed again against all wires; a job that still runs across a text is searched
//!    once more with the texts it crosses at their full price, and the new route kept where the page's cost is
//!    lower; at most [`PASSES`] rounds.
//! 7. **Weighed against the stored drawing** (§3.9): a net whose stored routes cost no less and keep free of faults
//!    among the others keeps them.
//! 8. **Never worse** (§1.2): the plan is offered only when the page is better by [`measure`] and no wire routed
//!    before that was free of a finding (F2 to F6, C4) gets one; nor a text free of C5, where the plan routes no wire
//!    that had no route. A net that breaks it goes back to its stored drawing once; then the rule decides.
//! 9. **Marks follow** (§3.10) the wires they name.
//!
//! A job without a way keeps its stored route, or stays unrouted, and is named with the first thing in its way.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;

use crate::consts::{PASSES, TICKS_PER_STEP, TRACK_PITCH};
use crate::geom;
use crate::labels::settle_labels;
use crate::measure::{Quality, TextRef, measure};
use crate::model::{DraftPage, End, JunctionId, MarkId, Owner as LabelOwner, PartId, PinId, Pt, Rect, Side, WireId};
use crate::nets::{self, Line, Net};
use crate::occupancy::{HALF, Occupancy, Text, node_of};
use crate::search::{self, Ask, Found, Owner, Relax, Tip, heading};

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
}

/// The jobs of a Route on `scope` (§3.2): a page-wide Route takes every wire that is unrouted, has a fault as stored
/// (a slanted segment or a vertex off the half grid, or one of F2 to F6) or is `computed`, and flags the person's among
/// them; a Route of a selection takes the wires named and every wire with an end on a part named.
pub fn jobs(page: &DraftPage, scope: Scope<'_>) -> Jobs {
    let mut wires: Vec<WireId> = Vec::new();
    let mut his: Vec<WireId> = Vec::new();
    match scope {
        Scope::Page => {
            let q = measure(page);
            let f = &q.findings;
            let mut faulty: BTreeSet<WireId> = BTreeSet::new();
            faulty.extend(f.through_points.iter().map(|x| x.0));
            faulty.extend(f.on_wires.iter().flat_map(|x| x.0));
            faulty.extend(f.through_parts.iter().map(|x| x.0));
            faulty.extend(f.false_joints.iter().flat_map(|x| x.0));
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
                let bent = !w.route.is_empty() && f1(&w.route);
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
                let on = w.ends.iter().any(|e| matches!(e, End::Pin(p) if parts_pins.contains(p)));
                if ids.contains(&w.id.0) || on {
                    wires.push(w.id);
                }
            }
        }
    }
    wires.sort();
    his.sort();
    Jobs { wires, his }
}

/// F1 as stored: a slanted segment, or an inner vertex off the half grid.
fn f1(route: &[Pt]) -> bool {
    let pts = geom::clean(route);
    let slanted = geom::segments(&pts).iter().any(|(a, b)| a.x != b.x && a.y != b.y);
    let inner = pts.get(1..pts.len().saturating_sub(1)).unwrap_or(&[]);
    slanted || inner.iter().any(|p| p.x.rem_euclid(HALF) != 0 || p.y.rem_euclid(HALF) != 0)
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
            if matches!(l.owner, LabelOwner::Wire(_) | LabelOwner::Mark(_)) {
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
        let wired: BTreeSet<PinId> = self
            .page
            .wires
            .iter()
            .flat_map(|w| w.ends)
            .filter_map(|e| match e {
                End::Pin(p) => Some(p),
                End::Junction(_) => None,
            })
            .collect();
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
        self.wire_index(w).map_or(&[], |i| self.page.wires[i].route.as_slice())
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
                    owner: Owner::Pin(p),
                    part: Some((pin.part.0, pin.body)),
                    shared,
                })
            }
            End::Junction(j) => Some(Tip {
                at: self.junction_at(j)?,
                side: None,
                owner: Owner::Junction(j),
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
            let mut arms: BTreeSet<Side> = BTreeSet::new();
            for x in &self.page.wires {
                if x.id == w || !x.ends.contains(e) || x.route.is_empty() {
                    continue;
                }
                let r = geom::clean(&x.route);
                let next = if r.first() == Some(&at) {
                    r.get(1)
                } else if r.last() == Some(&at) {
                    r.len().checked_sub(2).and_then(|k| r.get(k))
                } else {
                    None
                };
                if let Some(h) = next.and_then(|n| heading(at, *n)) {
                    arms.insert(h);
                }
            }
            arms.len() >= 3
        })
    }

    /// The city-block distance of wire `w`'s ends and its first end's place: the order of the search.
    fn order_key(&self, w: WireId) -> (i64, (i32, i32), WireId) {
        let [a, b] = self.ends.get(&w).copied().unwrap_or([End::Pin(PinId(Uuid::nil())); 2]);
        let (pa, pb) = (self.point(a).unwrap_or_default(), self.point(b).unwrap_or_default());
        (geom::length((pa, pb)), (pa.y, pa.x), w)
    }
}

/// The routes of a net on trunk `line`: per wire its route, or `None` when its drop is not straight; the junction taps'
/// places; `None` when the line breaks the spine's order.
fn trunk_routes(r: &Router, net: &Net, line: Line) -> Option<(BTreeMap<WireId, Option<Vec<Pt>>>, BTreeMap<JunctionId, Pt>)> {
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
                    let on = line.across(at) == line.at && side.is_some_and(|s| s.vertical() != line.horizontal);
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
                            (pin.side.is_some() && pin.side == line.facing(pin.at) && !net.taps.contains(&other))
                                .then_some(pin.at)
                        })
                        .collect();
                    match drops.as_slice() {
                        [one] => line.foot(*one),
                        _ => line.foot(at),
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
        let start = if matches!(a, End::Junction(j) if movable.contains(&j)) { feet[&a] } else { pa };
        let end = if matches!(b, End::Junction(j) if movable.contains(&j)) { feet[&b] } else { pb };
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

/// Whether a node of the occupancy lies on a laid wire (a vertex of one, or between two moves along one).
fn on_a_wire(occ: &Occupancy, p: Pt) -> bool {
    let n = node_of(p);
    occ.vertices_at(n) > 0
        || (occ.along(((n.0 - 1, n.1), false)) > 0 && occ.along((n, false)) > 0)
        || (occ.along(((n.0, n.1 - 1), true)) > 0 && occ.along((n, true)) > 0)
}

impl Router {
    /// Phase 2: the nets of one wire whose ends face each other on one line, straight where that is free.
    fn straights(&mut self, nets: &[Net]) -> BTreeSet<WireId> {
        let mut laid = BTreeSet::new();
        for net in nets.iter().filter(|n| n.wires.len() == 1) {
            let w = net.wires[0];
            let Some([a, b]) = self.ends.get(&w).copied() else { continue };
            let (Some(pa), Some(pb)) = (self.point(a), self.point(b)) else { continue };
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
    fn trunks(&mut self, nets: &[Net]) -> (BTreeSet<WireId>, Vec<Vec<WireId>>) {
        let mut laid = BTreeSet::new();
        let mut notes = Vec::new();
        let supply = |r: &Router, net: &Net| {
            net.wires
                .iter()
                .filter_map(|w| r.page.wires.iter().find(|x| x.id == *w)?.supply)
                .min()
                .unwrap_or(u8::MAX)
        };
        let mut order: Vec<(u8, (i32, i32), usize, WireId, usize)> = nets
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
            let mut best: Option<((u64, u8, u8, i32), BTreeMap<WireId, Option<Vec<Pt>>>, BTreeMap<JunctionId, Pt>)> =
                None;
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
                let mut valid = places.values().all(|p| {
                    area.x0 < p.x && p.x < area.x1 && area.y0 < p.y && p.y < area.y1 && !on_a_wire(&self.occ, *p)
                });
                for (j, p) in &places {
                    self.move_junction(*j, *p);
                }
                let mut cost: u64 = 0;
                let mut kept = BTreeMap::new();
                let mut clear_count = 0usize;
                for (w, route) in &routes {
                    let spine_wire = self
                        .ends
                        .get(w)
                        .is_some_and(|e| e.iter().all(|x| net.taps.contains(x)));
                    let ok = valid && route.as_ref().is_some_and(|rt| self.clear(*w, rt));
                    if ok {
                        clear_count += 1;
                        cost += u64::from(self.price(*w, route.as_deref().unwrap_or(&[])));
                        kept.insert(*w, route.clone());
                    } else {
                        if spine_wire {
                            valid = false;
                        }
                        // A wire left to the search: its ends' distance and two bends.
                        let [a, b] = self.ends.get(w).copied().unwrap_or([End::Pin(PinId(Uuid::nil())); 2]);
                        let (pa, pb) = (self.point(a).unwrap_or_default(), self.point(b).unwrap_or_default());
                        cost += u64::try_from(geom::length((pa, pb))).unwrap_or(0) + 8 * u64::try_from(TICKS_PER_STEP).unwrap_or(16);
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
                if best.as_ref().is_none_or(|(k, _, _)| key < *k) {
                    best = Some((key, kept, places));
                }
            }
            let Some((_, routes, places)) = best else {
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

    /// Phase 4: a net whose only tap is a junction that may move: the junction where its wires cost least.
    fn single_taps(&mut self, nets: &[Net]) -> BTreeSet<WireId> {
        let mut laid = BTreeSet::new();
        for net in nets {
            let [End::Junction(j)] = net.taps.as_slice() else { continue };
            if net.wires.iter().any(|w| !self.route_of(*w).is_empty()) {
                continue;
            }
            let Some(present) = self.junction_at(*j) else { continue };
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
                if !(area.x0 < c.x && c.x < area.x1 && area.y0 < c.y && c.y < area.y1) {
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

    /// Phase 5: the search for the jobs without a route, then the passes; the wires searched in order, and those with
    /// no way.
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
            if self.fourth_arm(w) {
                failed.insert(w);
                continue;
            }
            match self.search(w, &[], Relax::default()) {
                Some(f) => {
                    self.lay(w, f.route);
                    searched.push(w);
                }
                None => {
                    failed.insert(w);
                }
            }
        }
        for _ in 0..PASSES {
            let mut changed = false;
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

    fn points_meet(&self, w: WireId) -> bool {
        let Some([a, b]) = self.ends.get(&w).copied() else {
            return true;
        };
        match (self.point(a), self.point(b)) {
            (Some(pa), Some(pb)) => pa == pb,
            _ => true,
        }
    }

    /// Phase 6: the text rounds (module documentation).
    fn text_rounds(&mut self) {
        for _ in 0..PASSES {
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
                            TextRef::Fixed(owner, rect) => x.label.is_none() && x.owner == *owner && x.rect == *rect,
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
    fn stored_page(&self, net: &Net) -> DraftPage {
        let mut page = self.page.clone();
        for w in &mut page.wires {
            if net.wires.contains(&w.id)
                && let Some(s) = self.stored.get(&w.id)
            {
                w.route.clone_from(s);
            }
        }
        for j in &mut page.junctions {
            if net.taps.contains(&End::Junction(j.id))
                && let Some(p) = self.first.get(&j.id)
            {
                j.at = *p;
            }
        }
        page
    }

    /// Phase 7: each net weighed against its stored drawing (module documentation).
    fn weigh(&mut self, nets: &[Net]) {
        for net in nets {
            if net
                .wires
                .iter()
                .any(|w| self.stored.get(w).is_none_or(Vec::is_empty))
            {
                continue;
            }
            let same = net.wires.iter().all(|w| self.stored.get(w).map(Vec::as_slice) == Some(self.route_of(*w)));
            if same {
                continue;
            }
            let alt = self.stored_page(net);
            let (qa, qn) = (measure(&alt), measure(&self.page));
            let faulty: BTreeSet<WireId> = wires_with_faults(&qa);
            if qa.key() <= qn.key() && net.wires.iter().all(|w| !faulty.contains(w)) {
                self.page = alt;
            }
        }
    }
}

fn is_fan(net: &Net) -> bool {
    matches!(net.taps.as_slice(), [End::Pin(_)])
}

/// The wires with a fault (F2 to F6) in `q`.
fn wires_with_faults(q: &Quality) -> BTreeSet<WireId> {
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
/// that get one, and whether a text free of C5 gets one where the plan routes no wire that had no route.
fn worse(page: &DraftPage, before: &Quality, after: &Quality) -> (BTreeSet<WireId>, bool) {
    let (fb, fa) = (wires_with_findings(before), wires_with_findings(after));
    let routed_before: BTreeSet<WireId> = page
        .wires
        .iter()
        .filter(|w| !w.route.is_empty())
        .map(|w| w.id)
        .collect();
    let wires = fa
        .difference(&fb)
        .filter(|w| routed_before.contains(w))
        .copied()
        .collect();
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
        page.wires
            .iter()
            .filter(|w| w.ends.contains(&End::Junction(j)))
            .all(|w| wanted.contains(&w.id))
    };
    let nets = nets::nets(&wanted, &r.ends, &movable);
    r.straights(&nets);
    let (_, notes) = r.trunks(&nets);
    r.single_taps(&nets);
    let (_, failed) = r.searches();
    r.text_rounds();
    r.weigh(&nets);
    // A job without a way keeps its stored route.
    for w in &failed {
        if let Some(s) = r.stored.get(w).filter(|s| !s.is_empty())
            && let Some(i) = r.wire_index(*w)
        {
            r.page.wires[i].route.clone_from(s);
        }
    }
    let mut after = measure(&r.page);
    // Never worse: a net that gives a wire a finding goes back to its stored drawing once.
    let (bad, _) = worse(page, &before, &after);
    if !bad.is_empty() {
        for net in nets.iter().filter(|n| n.wires.iter().any(|w| bad.contains(w))) {
            r.page = r.stored_page(net);
        }
        after = measure(&r.page);
    }
    let (bad, texts) = worse(page, &before, &after);
    if !after.better_than(&before) || !bad.is_empty() || texts {
        return empty(before);
    }
    let mut plan = Plan {
        notes,
        ..Plan::default()
    };
    for w in &wanted {
        let new = r.route_of(*w);
        let old = page.wires.iter().find(|x| x.id == *w).map_or(&[][..], |x| x.route.as_slice());
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
    plan.before = before;
    plan.after = after;
    plan
}

impl Router {
    /// What is in the way of job `w` (§3.8): searched again without the other wires, then without the connection points
    /// of other wires, then without the bodies.
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
                    bodies: false,
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
        if !m.wires.iter().any(|w| changed.contains(w)) {
            continue;
        }
        let c = m.line.centre();
        // The stretches of each wire across the mark's line: vertical ones under a horizontal line.
        let runs: Vec<Vec<(i32, i32, i32)>> = m
            .wires
            .iter()
            .map(|w| {
                let route = page.wires.iter().find(|x| x.id == *w).map_or(&[][..], |x| x.route.as_slice());
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
        let fits = |v: i32, wires: &[Vec<(i32, i32, i32)>]| -> Option<i32> {
            let mut at: Vec<i32> = Vec::new();
            for runs in wires {
                let x = runs.iter().find(|(_, lo, hi)| *lo < v && v < *hi)?.0;
                at.push(x);
            }
            at.sort();
            at.windows(2)
                .all(|w| w[1] - w[0] <= TRACK_PITCH)
                .then(|| (at.first().copied().unwrap_or(0) + at.last().copied().unwrap_or(0)) / 2)
        };
        let here = |wires: &[Vec<(i32, i32, i32)>]| fits(along_c, wires);
        if here(&runs).is_some() {
            continue;
        }
        let (lo, hi) = runs
            .iter()
            .flatten()
            .fold((i32::MAX, i32::MIN), |(lo, hi), (_, a, b)| (lo.min(*a), hi.max(*b)));
        let near = |wires: &[Vec<(i32, i32, i32)>]| -> Option<(i32, i32)> {
            let step = TICKS_PER_STEP;
            let mut best: Option<(i32, i32, i32)> = None;
            let mut v = lo.div_euclid(step) * step;
            while v <= hi {
                if let Some(x) = fits(v, wires) {
                    let d = (v - along_c).abs();
                    if best.is_none_or(|(bd, bv, _)| (d, v) < (bd, bv)) {
                        best = Some((d, v, x));
                    }
                }
                v += step;
            }
            best.map(|(_, v, x)| (v, x))
        };
        let Some((v, _)) = near(&runs).or_else(|| runs.first().and_then(|first| near(std::slice::from_ref(first))))
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
        page.labels.retain(|l| !matches!(l.owner, LabelOwner::Part(p) if gone.contains(&p.0)));
        page.fixed_texts.retain(|t| !gone.contains(&t.owner));
    }
    Router::new(&page, &[])
}

/// The Wire tool's route from `from` to `to` (§7.4): the search of [`route`] against the page as drawn, every routed
/// wire fixed, a wire of the same pin's run shared where that is the better way; `arrive` the heading a free first end
/// was arrived at with. `None` where no route keeps free of faults.
pub fn route_wire(page: &DraftPage, from: WireEnd, to: WireEnd, arrive: Option<Side>) -> Option<WireRoute> {
    let r = tool_router(page, &[])?;
    let (a, b) = (wire_tip(&r, from)?, wire_tip(&r, to)?);
    let ask = Ask {
        occ: &r.occ,
        from: a,
        to: b,
        arrive,
        area: r.page.area,
        full: &[],
        relax: Relax::default(),
    };
    let found = search::route(&ask)?;
    let over_text = r.occ.texts.iter().any(|t| {
        t.label.is_none()
            && geom::segments(&found.route)
                .iter()
                .any(|s| geom::stretch_inside(*s, t.rect.grown(-1)).is_some())
    });
    Some(WireRoute {
        route: found.route,
        over_text,
    })
}

/// Whether `route` from `from` to `to` is one the Wire tool's search could take on `page` with the parts `through` left
/// out (their bodies, texts and pins): the direct route through a free terminal in line.
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
    let ask = Ask {
        occ: &r.occ,
        from: a,
        to: b,
        arrive,
        area: r.page.area,
        full: &[],
        relax: Relax::default(),
    };
    search::clear(&ask, route)
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
