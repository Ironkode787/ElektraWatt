// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The phases of a run of the router after the drawing it starts from (docs/design/06a-drafting.md §3.4 to §3.9):
//! straights, trunks, tracks, single taps, the search and its passes, the rings, the text rounds and the weighing
//! against the stored drawing, as methods of `Router`.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Uuid;

use crate::consts::{BEND, LEFT_TO_SEARCH, PASSES, TAKE_BACK_ROUNDS, TEXT_ROUNDS, TICKS_PER_STEP};
use crate::geom;
use crate::measure::{TextRef, measure};
use crate::model::{DraftPage, End, JunctionId, PartId, PinId, Pt, Side, Tick, WireId};
use crate::nets::{self, JobNet, Line};
use crate::occupancy::{Occupancy, node_of};
use crate::search::{self, Relax, heading};
use crate::{rounds, tracks};

use super::{Router, wires_with_faults};

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
    pub(super) fn straights(&mut self, nets: &[JobNet]) -> BTreeSet<WireId> {
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

    /// Phase 3: the trunks (`nets`): a net whose taps form a chain, or a pin's fan, gets the trunk of least price; in the
    /// order of their supply ranks, then their first tapped pin's place (left first), then more taps first, then the
    /// ID. The wires laid, and the nets whose order ran against their places.
    pub(super) fn trunks(&mut self, nets: &[JobNet]) -> (BTreeSet<WireId>, Vec<Vec<WireId>>) {
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
    pub(super) fn tracks(&mut self, nets: &[JobNet]) -> BTreeSet<WireId> {
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
    pub(super) fn rings(&mut self, nets: &[JobNet], searched: &[WireId]) {
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
    pub(super) fn single_taps(&mut self, nets: &[JobNet]) -> BTreeSet<WireId> {
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
    /// tries it again first. The order: the city-block distance of their ends (shortest first), then the first end's
    /// place, then the ID; each pass lifts each wire in the same order, searches it again and keeps its new route where
    /// it costs less. A wire ending at a junction that the other wires leave in three directions is not searched (its
    /// route would be a fourth arm).
    pub(super) fn searches(&mut self) -> (Vec<WireId>, BTreeSet<WireId>) {
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
                let laid = self.search_again(w, &[], |r, f, _| {
                    if r.fourth_arm(w) {
                        return false;
                    }
                    r.lay(w, f.route);
                    true
                });
                if laid {
                    failed.remove(&w);
                    searched.push(w);
                    changed = true;
                }
            }
            for w in searched.clone() {
                changed |= self.search_again(w, &[], |r, f, old| {
                    if f.route == old || f.cost >= r.price(w, old) {
                        return false;
                    }
                    r.lay(w, f.route);
                    true
                });
            }
            if !changed {
                break;
            }
        }
        (searched, failed)
    }

    /// Wire `w` lifted and searched again, with the texts `full` at their full price: its new route stays where `keep`
    /// takes it (`keep` lays it, and may judge the page with it laid), else its old route is put back. Whether the new
    /// route stays.
    fn search_again(
        &mut self,
        w: WireId,
        full: &[u32],
        keep: impl FnOnce(&mut Self, search::Found, &[Pt]) -> bool,
    ) -> bool {
        let old = self.route_of(w).to_vec();
        self.lift(w);
        let kept = self
            .search(w, full, Relax::default())
            .is_some_and(|f| keep(self, f, &old));
        if !kept {
            self.lay(w, old);
        }
        kept
    }

    /// Job `w` on its stored route, where it has one (a job without a way, §3.8).
    fn keep_stored(&mut self, w: WireId) {
        if let Some(s) = self.stored.get(&w).filter(|s| !s.is_empty()).cloned() {
            self.lay(w, s);
        }
    }

    pub(super) fn points_meet(&self, w: WireId) -> bool {
        let Some([a, b]) = self.ends.get(&w).copied() else {
            return true;
        };
        match (self.point(a), self.point(b)) {
            (Some(pa), Some(pb)) => pa == pb,
            _ => true,
        }
    }

    /// Phase 7, the text rounds (§3.7): the labels placed again against all wires; a job that still runs across a text
    /// is searched once more with the texts it crosses at their full price, and the new route kept where the page's
    /// cost is lower; at most [`TEXT_ROUNDS`] rounds.
    pub(super) fn text_rounds(&mut self) {
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
                changed |= self.search_again(w, &full, |r, f, old| {
                    if f.route == old {
                        return false;
                    }
                    r.lay(w, f.route);
                    let now = measure(&r.page);
                    let better = now.key() < (q.faults, q.unrouted, cost) && now.faults <= q.faults;
                    if better {
                        cost = now.cost;
                    }
                    better
                });
            }
            if !changed {
                return;
            }
        }
    }

    /// The page with net `net`'s wires on their stored routes and its junctions at their first places.
    pub(super) fn stored_page(&self, net: &JobNet) -> DraftPage {
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

    /// Phase 8: the nets with a stored drawing weighed against it (§3.9). Back: from the new drawing, a
    /// net whose stored routes cost no less and keep free of faults among the others keeps them. Forward: from the
    /// stored drawing, a net takes its new routes where they cost less and keep free of faults among the others. The
    /// cheaper page stands: one net's gain is not lost to another's loss that no single net's going back undoes. The
    /// figures §1.2 only reports (length, labels moved, wires straight, turn-backs) veto nothing.
    pub(super) fn weigh(&mut self, nets: &[JobNet]) {
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
            self.search_again(w, &[], |r, f, old| {
                if f.route == old {
                    return false;
                }
                r.lay(w, f.route);
                let now = measure(&r.page);
                let better = now.key() < q.key();
                if better {
                    q = now;
                }
                better
            });
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
