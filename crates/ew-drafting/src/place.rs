// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The placer (docs/design/06a-drafting.md §4): [`arrange`] puts each part where the page is best for it, within the
//! rules of §1.6 ([`crate::rules`]), and routes the page as it would become.
//!
//! # The run
//!
//! 1. **Units** (§4.2). Parts with one `unit` number move as one, and so do the parts inside an outline drawn round
//!    other parts (a valve terminal with its valves). A unit is **fixed** when a part of it is kept in place, outside
//!    the scope, unticked in the preview, off the whole grid, or has a pin off the grid.
//! 2. **Relations.** Two units are related through a net with exactly two pins (a pair); its kind from the pins'
//!    sides: **facing** (opposite), **corner** (at a right angle), **level** (the same). A net with more pins is a
//!    trunk net: it relates its parts to its trunk only.
//! 3. **Hosts.** Weight: 4 per wired pin and the bodies' area in square steps over 16. Anchors: the fixed units, and
//!    in each group of related units without one the heaviest (then the upper, the left, the ID). From the anchors
//!    outward, breadth first, a unit's host is a related unit one step nearer an anchor: facing before corner before
//!    level, then the heavier, then its pin's order. That relation is the unit's lead.
//! 4. **Followers** (§4.3, §4.4), from the anchors down: a follower moves with its host, then the followers of one
//!    side of a host are set together. A **row** (facing and level leads): across, a facing follower wants its pin on
//!    its host pin's line, under the order of the host's pins with [`ROW_GAP`] between footprints (pooled adjacent
//!    violators, the block at its members' median); along, a follower keeps its distance when it stands at least
//!    [`CHAIN_GAP_MIN`] beyond and clear of its host, else it joins the row's line: the farthest distance a member
//!    keeps within [`ROW_PULL`] of the row's room, at least the room: [`CHAIN_GAP`], and a step more than the tracks
//!    its stepping wires need ([`crate::tracks::tracks_needed`], §4.5); a level follower stands level. A **stack**
//!    (corner leads that turn one way): across, each pin [`MARK_LEAD`] before its host pin unless it is further; along,
//!    the ones that must move go beyond the others in nesting order (the drop furthest against the way the wires go
//!    turns outermost), [`MARK_PITCH`] apart or a footprint more.
//! 5. **Trunks** (§4.6). A mark on a net whose junctions lie on one horizontal line takes that line. **Bands** (§4.7,
//!    rules E2): a group with a part of a circuit beyond the rail that feeds it, or above the highest feeding rail, is
//!    moved along the flow by the least distance.
//! 6. **The price** (§4.7). A follower's own move that sets no rule right stays only where the estimate gains at
//!    least [`MOVE_PRICE`] for each part it moves. The estimate: the city-block length of each pair's plain shape
//!    (straight, L, step, hairpin) and of each trunk net's line and drops, 4 for each bend they need and 6 for each
//!    crossing among them. Then two neighbouring groups change places where that gains more than its price, at most
//!    [`PASSES`] times.
//! 7. **Fitting** (§4.7). A group that leaves the drawing area is moved in by the least distance; of two groups on
//!    each other the later (fixed, then heavier, then upper left) takes the next free place across; a unit that still
//!    has no room goes back to where it stood, and one whose move set a rule right is named.
//! 8. **The plan** (§4.9). The parts a broken rule names go back with what hangs on them until [`rules_kept`] holds
//!    (the rules read the parts and the nets alone). The wires with an end on a moved part are routed with the jobs of
//!    Route for the scope. The plan is offered only when the rules still hold, the page is better by the measure, no
//!    wire or text gains a finding and, where it routes no dashed wire, no figure a reader compares (faults,
//!    crossings, bends) is worse; else Route's plan of the same scope is offered.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{
    BODY_GAP, CHAIN_GAP, CHAIN_GAP_MIN, MARK_LEAD, MOVE_PRICE, PASSES, ROW_GAP, ROW_PULL, TICKS_PER_STEP,
    TRACK_PITCH, TRUNK_GAP,
};
use crate::geom;
use crate::labels::settle_labels;
use crate::measure::measure;
use crate::model::{DraftPage, End, Owner, PartId, PinId, Pt, Rect, Role, Side, Tick, WireId};
use crate::route::{self, Jobs, Plan, Scope};
use crate::rules::{Wiring, across, beyond, opposite, rails, rules_kept};

/// The most branches of the moves whose plans without them, and alone, are tried (§4.9).
const BRANCHES: usize = 8;

/// What Arrange and route plans for a page (§4.9): the parts' moves and the routes of the page as it would become.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Arranged {
    /// The parts that move, each by its move in ticks (whole grid steps), by ID.
    pub moves: Vec<(PartId, Pt)>,
    /// The parts that stand against a rule and have no room on the side their host's pin faces, by ID, with that side.
    pub left: Vec<(PartId, Side)>,
    /// The routes, junctions and marks of the page as it would become (every wire with an end on a moved part
    /// included, an empty route for one without a way), and the page's quality before and after.
    pub plan: Plan,
}

impl Arranged {
    /// Whether the plan changes nothing.
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty() && self.plan.is_empty()
    }

    /// The parts that move further than a third of the drawing area `area` across or down (§4.8), by ID.
    pub fn far(&self, area: Rect) -> Vec<PartId> {
        self.moves
            .iter()
            .filter(|(_, d)| 3 * d.x.abs() > area.w() || 3 * d.y.abs() > area.h())
            .map(|(p, _)| *p)
            .collect()
    }
}

/// The plan of Arrange and route over `scope` of `page` with the parts and wires named in `fixed` kept as they stand
/// (the rows unticked in the preview: a wire kept keeps its route and the parts at its ends; module documentation).
/// Deterministic: the same page in any order of its lists gives the same plan.
pub fn arrange(page: &DraftPage, scope: Scope<'_>, fixed: &[ew_ids::Uuid]) -> Arranged {
    let before = measure(page);
    // Without a plan the measure takes: Route's plan of the scope, and the parts that have no room named all the same.
    let fallback = |left: &[(PartId, Side)]| {
        let mut jobs = route::jobs(page, scope);
        jobs.wires.retain(|w| !fixed.contains(&w.0));
        let plan = route::route(page, &jobs);
        Arranged {
            plan,
            left: left.to_vec(),
            ..Arranged::default()
        }
    };
    let mut p = Placer::new(page, scope, fixed);
    p.place();
    // The rules read the parts and the nets alone: the parts a broken rule names go back, with what hangs on them,
    // until none breaks; then the page is routed once.
    for _ in 0..p.units.len() {
        let moves = p.moves();
        if moves.is_empty() {
            break;
        }
        let Err(broken) = rules_kept(page, &moved(page, &moves).0) else {
            break;
        };
        if std::env::var("EW_PLACE_DEBUG").is_ok() { eprintln!("broken {:?}", broken.iter().map(|b| (b.rule, b.parts.iter().map(|p| p.0.to_string()[32..].to_owned()).collect::<Vec<_>>())).collect::<Vec<_>>()); }
        let named: BTreeSet<PartId> = broken.iter().flat_map(|b| b.parts.iter().copied()).collect();
        let units: BTreeSet<usize> = named.iter().filter_map(|id| p.unit_of_id(*id)).collect();
        for u in units {
            for f in p.subtree(u) {
                p.d[f] = Pt::default();
            }
        }
    }
    // The plan of all the moves; where the measure does not take it, the plans without one branch of them and then of
    // one branch alone, at most `BRANCHES` each (a branch: a unit's own move with what hangs on it). The best plan the
    // measure takes is offered.
    let full = p.d.clone();
    let mut tries: Vec<Vec<Pt>> = vec![full.clone()];
    let branches: Vec<usize> = p
        .order
        .iter()
        .copied()
        .filter(|u| {
            let own = match p.lead[*u] {
                Some(l) => full[*u].minus(full[l.host]),
                None => full[*u],
            };
            own != Pt::default() && !p.units[*u].fixed
        })
        .take(BRANCHES)
        .collect();
    for b in &branches {
        let mut d = full.clone();
        for f in p.subtree(*b) {
            d[f] = Pt::default();
        }
        tries.push(d);
    }
    for b in &branches {
        let sub = p.subtree(*b);
        let d: Vec<Pt> = (0..full.len())
            .map(|u| if sub.contains(&u) { full[u] } else { Pt::default() })
            .collect();
        tries.push(d);
    }
    let mut best: Option<(i64, Vec<(PartId, Pt)>, DraftPage, Plan, crate::Quality)> = None;
    let mut seen: BTreeSet<Vec<(PartId, Pt)>> = BTreeSet::new();
    for (k, d) in tries.into_iter().enumerate() {
        // The plans without a branch are tried only when the full one is not taken, and so on.
        if k == 1 + branches.len() && best.is_some() {
            break;
        }
        if k == 1 && best.is_some() {
            break;
        }
        p.d = d;
        let moves = p.moves();
        if moves.is_empty() || !seen.insert(moves.clone()) {
            continue;
        }
        if rules_kept(page, &moved(page, &moves).0).is_err() {
            continue;
        }
        let (moved, plan, after) = routed(page, &moves, scope, fixed);
        if rules_kept(page, &after).is_err() {
            continue;
        }
        let quality = measure(&after);
        let (bad, texts) = route::worse(page, &before, &quality);
        // No figure a reader compares worse (crossings, bends, faults), where the plan routes no wire that had no route
        // (routing a dashed wire is the larger gain).
        let figures = quality.faults > before.faults
            || quality.unrouted >= before.unrouted
                && (quality.counts.crossings > before.counts.crossings
                    || quality.counts.bends > before.counts.bends);
        if std::env::var("EW_PLACE_DEBUG").is_ok() { eprintln!("judged {k} {:?} {:?} bad {} texts {} figures {} x {}->{} b {}->{}", quality.key(), before.key(), bad.len(), texts, figures, before.counts.crossings, quality.counts.crossings, before.counts.bends, quality.counts.bends); }
        // A move pays its price (§4.7) and shows: some figure a reader compares is better.
        let (b, q) = (&before.counts, &quality.counts);
        let shows = quality.faults < before.faults
            || quality.unrouted < before.unrouted
            || q.crossings < b.crossings
            || q.bends < b.bends
            || q.turn_backs < b.turn_backs
            || q.across_texts < b.across_texts
            || q.label_clashes < b.label_clashes;
        let price = MOVE_PRICE * i64::from(TICKS_PER_STEP) * moves.len() as i64;
        let priced = (quality.faults, quality.unrouted, quality.cost + price) < before.key();
        if !priced || !shows || !bad.is_empty() || texts || figures {
            continue;
        }
        let cost = quality.cost;
        if best.as_ref().is_none_or(|b| (quality.key(), cost) < (b.4.key(), b.0)) {
            best = Some((cost, moves, moved, plan, quality));
        }
    }
    let Some((_, moves, moved, plan, quality)) = best else {
        return fallback(&p.left);
    };
    Arranged {
        plan: finished(page, &moved, plan, before, quality),
        left: p
            .left
            .iter()
            .filter(|(id, _)| !moves.iter().any(|(m, _)| m == id))
            .copied()
            .collect(),
        moves,
    }
}

/// The page with every part of `moves` moved, with its texts the program may not place, and the routes of the wires
/// with an end on a moved part lifted; and those wires.
fn moved(page: &DraftPage, moves: &[(PartId, Pt)]) -> (DraftPage, Vec<WireId>) {
    let mut moved = page.clone();
    let by: BTreeMap<PartId, Pt> = moves.iter().copied().collect();
    let mut pins: BTreeSet<PinId> = BTreeSet::new();
    for part in &mut moved.parts {
        if let Some(d) = by.get(&part.id) {
            part.at = part.at.plus(*d);
            pins.extend(part.pins.iter().map(|x| x.id));
            for t in &mut moved.fixed_texts {
                if t.owner == part.id.0 {
                    t.rect = t.rect.moved(*d);
                }
            }
        }
    }
    let mut touched: Vec<WireId> = Vec::new();
    for w in &mut moved.wires {
        if w.ends.iter().any(|e| matches!(e, End::Pin(p) if pins.contains(p))) {
            w.route.clear();
            touched.push(w.id);
        }
    }
    (moved, touched)
}

/// The page with every part of `moves` moved ([`moved`]); Route's plan of it for the wires of the moved parts and the
/// jobs of `scope`; and the page as the plan leaves it.
fn routed(
    page: &DraftPage,
    moves: &[(PartId, Pt)],
    scope: Scope<'_>,
    fixed: &[ew_ids::Uuid],
) -> (DraftPage, Plan, DraftPage) {
    let (moved, touched) = moved(page, moves);
    let mut jobs: Jobs = route::jobs(&moved, scope);
    jobs.wires.retain(|w| !fixed.contains(&w.0));
    jobs.wires.extend(touched);
    jobs.wires.sort();
    jobs.wires.dedup();
    let plan = route::route(&moved, &jobs);
    let after = applied(&moved, &plan);
    (moved, plan, after)
}

/// `page` with `plan`'s routes, junctions and marks.
fn applied(page: &DraftPage, plan: &Plan) -> DraftPage {
    let mut out = page.clone();
    for (w, r) in &plan.routes {
        if let Some(x) = out.wires.iter_mut().find(|x| x.id == *w) {
            x.route.clone_from(r);
            x.computed = !r.is_empty();
        }
    }
    for (j, at) in &plan.junctions {
        if let Some(x) = out.junctions.iter_mut().find(|x| x.id == *j) {
            x.at = *at;
        }
    }
    for (m, d) in &plan.marks {
        if let Some(x) = out.marks.iter_mut().find(|x| x.id == *m) {
            x.line = x.line.moved(*d);
        }
    }
    out
}

/// The plan of the arranged page: Route's plan on the moved page, with every wire whose route differs from the
/// page's (a wire of a moved part without a way: an empty route), the person's among them flagged.
fn finished(
    page: &DraftPage,
    moved: &DraftPage,
    mut plan: Plan,
    before: crate::Quality,
    after: crate::Quality,
) -> Plan {
    let mut routes: BTreeMap<WireId, Vec<Pt>> = plan.routes.drain(..).collect();
    for w in &moved.wires {
        let old = page.wires.iter().find(|x| x.id == w.id);
        if !routes.contains_key(&w.id) && old.is_some_and(|o| o.route != w.route) {
            routes.insert(w.id, w.route.clone());
        }
    }
    plan.routes = routes.into_iter().collect();
    plan.his = plan
        .routes
        .iter()
        .filter(|(w, _)| {
            page.wires
                .iter()
                .any(|x| x.id == *w && !x.computed && !x.route.is_empty())
        })
        .map(|(w, _)| *w)
        .collect();
    plan.before = before;
    plan.after = after;
    plan
}

/// How two related pins face each other (§4.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Facing,
    Corner,
    Level,
}

fn kind(a: Option<Side>, b: Option<Side>) -> Option<Kind> {
    let (a, b) = (a?, b?);
    Some(if a == b {
        Kind::Level
    } else if a == opposite(b) {
        Kind::Facing
    } else {
        Kind::Corner
    })
}

/// A unit's lead: its host's pin and its own.
#[derive(Clone, Copy, Debug)]
struct Lead {
    host: usize,
    h: PinId,
    f: PinId,
    kind: Kind,
}

struct Unit {
    parts: Vec<usize>,
    fixed: bool,
    weight: i64,
}

struct Placer<'a> {
    w: Wiring<'a>,
    /// Each part's texts, relative to its origin.
    texts: Vec<Vec<Rect>>,
    unit_of: Vec<usize>,
    units: Vec<Unit>,
    /// Each unit's move so far.
    d: Vec<Pt>,
    /// Each unit's lead, if it has one.
    lead: Vec<Option<Lead>>,
    /// The units whose places step 4 has set: the anchors, then each host's followers as they are set.
    settled: Vec<usize>,
    /// The units in the order the hosts were found (anchors first).
    order: Vec<usize>,
    /// The units whose own move sets a rule right.
    needed: BTreeSet<usize>,
    /// The parts named for having no room, with the side they would have gone to.
    left: Vec<(PartId, Side)>,
}

/// Rounds `v` ticks to the nearest whole step, halves away from zero.
fn whole(v: Tick) -> Tick {
    let s = TICKS_PER_STEP;
    let r = v.rem_euclid(s);
    if 2 * r >= s { v - r + s } else { v - r }
}

/// Rounds `v` ticks up (away from zero) to a whole step.
fn whole_up(v: Tick) -> Tick {
    let s = TICKS_PER_STEP;
    if v >= 0 {
        (v + s - 1).div_euclid(s) * s
    } else {
        -((-v + s - 1).div_euclid(s) * s)
    }
}

fn along_of(s: Side, t: Tick) -> Pt {
    let u = s.unit();
    Pt::new(u.x * t, u.y * t)
}

fn across_vec(s: Side, t: Tick) -> Pt {
    if s.vertical() { Pt::new(t, 0) } else { Pt::new(0, t) }
}

impl<'a> Placer<'a> {
    fn new(page: &'a DraftPage, scope: Scope<'_>, fixed: &[ew_ids::Uuid]) -> Placer<'a> {
        let w = Wiring::new(page);
        // The pins at the ends of a kept wire hold their parts.
        let held: BTreeSet<PinId> = page
            .wires
            .iter()
            .filter(|x| fixed.contains(&x.id.0))
            .flat_map(|x| x.ends)
            .filter_map(|e| match e {
                End::Pin(p) => Some(p),
                End::Junction(_) => None,
            })
            .collect();
        let n = w.parts.len();
        // Units by number, and the parts inside an outline drawn round other parts.
        let mut up: Vec<usize> = (0..n).collect();
        fn root(up: &mut [usize], mut i: usize) -> usize {
            while up[i] != i {
                up[i] = up[up[i]];
                i = up[i];
            }
            i
        }
        let join = |up: &mut Vec<usize>, a: usize, b: usize| {
            let (ra, rb) = (root(up, a), root(up, b));
            if ra != rb {
                up[ra.max(rb)] = ra.min(rb);
            }
        };
        let mut by_number: BTreeMap<u32, usize> = BTreeMap::new();
        for (i, p) in w.parts.iter().enumerate() {
            match by_number.get(&p.unit) {
                Some(&j) => join(&mut up, i, j),
                None => {
                    by_number.insert(p.unit, i);
                }
            }
        }
        for (i, f) in w.parts.iter().enumerate() {
            if f.frame_only {
                for (j, p) in w.parts.iter().enumerate() {
                    if i != j && f.bounds().holds(p.bounds()) {
                        join(&mut up, i, j);
                    }
                }
            }
        }
        let selected: Option<BTreeSet<ew_ids::Uuid>> = match scope {
            Scope::Page => None,
            Scope::Selection(ids) => Some(ids.iter().copied().collect()),
        };
        let wired: BTreeSet<PinId> = w.nets.iter().flat_map(|n| n.pins.iter().copied()).collect();
        let mut roots: BTreeMap<usize, usize> = BTreeMap::new();
        let mut units: Vec<Unit> = Vec::new();
        let mut unit_of = vec![0; n];
        for i in 0..n {
            let r = root(&mut up, i);
            let u = *roots.entry(r).or_insert_with(|| {
                units.push(Unit {
                    parts: Vec::new(),
                    fixed: false,
                    weight: 0,
                });
                units.len() - 1
            });
            unit_of[i] = u;
            let p = w.parts[i];
            let off = |v: Tick| v.rem_euclid(TICKS_PER_STEP) != 0;
            let fixed_part = p.locked
                || p.off_grid
                || off(p.at.x)
                || off(p.at.y)
                || p.pins.iter().any(|x| x.off_grid)
                || fixed.contains(&p.id.0)
                || p.pins.iter().any(|x| held.contains(&x.id))
                || selected.as_ref().is_some_and(|s| !s.contains(&p.id.0));
            let b = p.body;
            let area = i64::from(b.w() / TICKS_PER_STEP) * i64::from(b.h() / TICKS_PER_STEP);
            let pins = p.pins.iter().filter(|x| wired.contains(&x.id)).count() as i64;
            let unit = &mut units[u];
            unit.parts.push(i);
            unit.fixed |= fixed_part;
            unit.weight += 4 * pins + area / 16;
        }
        let m = units.len();
        // Each part's texts that stand at their first places now, relative to the part: they move with it.
        let mut texts: Vec<Vec<Rect>> = vec![Vec::new(); n];
        for s in settle_labels(page).into_iter().filter(|s| s.rank == 0) {
            let Some(l) = page.labels.iter().find(|l| l.id == s.id) else { continue };
            if let Owner::Part(p) = l.owner
                && let Some(i) = w.parts.iter().position(|x| x.id == p)
            {
                texts[i].push(s.rect.moved(Pt::new(-w.parts[i].at.x, -w.parts[i].at.y)));
            }
        }
        Placer {
            w,
            texts,
            unit_of,
            units,
            d: vec![Pt::default(); m],
            lead: vec![None; m],
            order: Vec::new(),
            settled: Vec::new(),
            needed: BTreeSet::new(),
            left: Vec::new(),
        }
    }

    fn unit_of_id(&self, id: PartId) -> Option<usize> {
        self.w.parts.iter().position(|p| p.id == id).map(|i| self.unit_of[i])
    }

    fn unit_of_pin(&self, p: PinId) -> usize {
        self.unit_of[self.w.pins[&p].0]
    }

    /// A pin's point with the moves so far.
    fn pt(&self, p: PinId) -> Pt {
        let (i, at, _) = self.w.pins[&p];
        at.plus(self.d[self.unit_of[i]])
    }

    fn side(&self, p: PinId) -> Option<Side> {
        self.w.pins[&p].2
    }

    /// A unit's footprint: its bodies, one step along each wired pin and its texts.
    fn footprint(&self, u: usize) -> Rect {
        let mut r: Option<Rect> = None;
        for i in &self.units[u].parts {
            let p = self.w.parts[*i];
            let mut b = p.bounds();
            for t in &self.texts[*i] {
                b = b.union(t.moved(p.at));
            }
            for pin in &p.pins {
                if let Some(s) = pin.side {
                    let at = p.at.plus(pin.at);
                    b = b.union(Rect::new(at, at.plus(along_of(s, TICKS_PER_STEP))));
                }
            }
            r = Some(r.map_or(b, |x| x.union(b)));
        }
        r.unwrap_or_default().moved(self.d[u])
    }

    fn key(&self, u: usize) -> (i64, Tick, Tick, PartId) {
        let b = self.footprint(u);
        let id = self.units[u].parts.iter().map(|i| self.w.parts[*i].id).min();
        (-self.units[u].weight, b.y0, b.x0, id.unwrap_or(PartId(ew_ids::Uuid::nil())))
    }

    /// The pairs between two units, each as (pin of the first, pin of the second, kind).
    fn relations(&self) -> Vec<Vec<(usize, PinId, PinId, Option<Kind>)>> {
        let mut out = vec![Vec::new(); self.units.len()];
        for (a, b) in self.w.pairs() {
            let (ua, ub) = (self.unit_of_pin(a), self.unit_of_pin(b));
            if ua == ub {
                continue;
            }
            let k = kind(self.side(a), self.side(b));
            out[ua].push((ub, a, b, k));
            out[ub].push((ua, b, a, k));
        }
        out
    }

    /// The units that hang on `u` (its followers and theirs), `u` included.
    fn subtree(&self, u: usize) -> Vec<usize> {
        let mut out = vec![u];
        let mut k = 0;
        while k < out.len() {
            let h = out[k];
            for (f, l) in self.lead.iter().enumerate() {
                if l.is_some_and(|l| l.host == h) {
                    out.push(f);
                }
            }
            k += 1;
        }
        out
    }

    fn shift(&mut self, units: &[usize], by: Pt) {
        for u in units {
            if !self.units[*u].fixed {
                self.d[*u] = self.d[*u].plus(by);
            }
        }
    }

    /// The whole run of the placer (module documentation, steps 3 to 7).
    fn place(&mut self) {
        self.hosts();
        self.dbg("hosts");
        self.followers();
        self.dbg("followers");
        self.trunk_marks();
        self.dbg("trunk_marks");
        self.bands();
        self.dbg("bands");
        self.priced();
        self.dbg("priced");
        self.swaps();
        self.dbg("swaps");
        self.fit();
        self.dbg("fit");
    }

    fn dbg(&self, stage: &str) {
        if std::env::var("EW_PLACE_DEBUG").is_err() { return; }
        let mut line = format!("[{stage}]");
        for (u, unit) in self.units.iter().enumerate() {
            if self.d[u] != Pt::default() {
                let id = self.w.parts[unit.parts[0]].id.0.to_string()[32..].to_owned();
                line += &format!(" {}:({},{}){}", id, self.d[u].x / 16, self.d[u].y / 16, if self.needed.contains(&u) {"!"} else {""});
            }
        }
        eprintln!("{line}");
    }

    /// The units a rail taps.
    fn railed(&self) -> BTreeSet<usize> {
        rails(&self.w)
            .into_iter()
            .flat_map(|r| r.3)
            .map(|i| self.unit_of[i])
            .collect()
    }

    /// Step 3: anchors and hosts, breadth first.
    fn hosts(&mut self) {
        let rel = self.relations();
        let m = self.units.len();
        // The groups of related units.
        let mut group = vec![usize::MAX; m];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for s in 0..m {
            if group[s] != usize::MAX {
                continue;
            }
            let g = groups.len();
            let mut todo = vec![s];
            group[s] = g;
            let mut members = Vec::new();
            while let Some(u) = todo.pop() {
                members.push(u);
                for (v, ..) in &rel[u] {
                    if group[*v] == usize::MAX {
                        group[*v] = g;
                        todo.push(*v);
                    }
                }
            }
            groups.push(members);
        }
        let railed = self.railed();
        let mut reached = vec![false; m];
        for members in &groups {
            let fixed: Vec<usize> = members.iter().copied().filter(|u| self.units[*u].fixed).collect();
            let roots = if fixed.is_empty() {
                // The anchor that moves the least weight to set the others on the side their host's pin faces: a part
                // out of place moves, not the circuit round it, and a light part before a heavy one. Then a part a rail taps (a rail stands where it is drawn), then the key.
                let Some(a) = members.iter().copied().min_by_key(|u| {
                    let (order, leads) = self.tree(&rel, &[*u]);
                    (self.wrong(&order, &leads), !railed.contains(u), self.key(*u))
                }) else {
                    continue;
                };
                vec![a]
            } else {
                fixed
            };
            let (order, leads) = self.tree(&rel, &roots);
            for u in &order {
                reached[*u] = true;
            }
            self.order.extend(order);
            for (f, l) in leads {
                self.lead[f] = Some(l);
            }
        }
        // Units no anchor reaches through a pair with sides: anchors of their own.
        for (u, r) in reached.iter().enumerate() {
            if !r {
                self.order.push(u);
            }
        }
    }

    /// The tree of hosts from `roots`, breadth first (§4.2): the units in the order found, and each follower's lead.
    fn tree(&self, rel: &[Vec<(usize, PinId, PinId, Option<Kind>)>], roots: &[usize]) -> (Vec<usize>, BTreeMap<usize, Lead>) {
        let mut seen: BTreeSet<usize> = roots.iter().copied().collect();
        let mut frontier: Vec<usize> = roots.to_vec();
        frontier.sort_by_key(|u| self.key(*u));
        let mut order = Vec::new();
        let mut leads: BTreeMap<usize, Lead> = BTreeMap::new();
        let rank = |l: &Lead| {
            let pin = self.w.parts[self.w.pins[&l.h].0]
                .pins
                .iter()
                .position(|x| x.id == l.h)
                .unwrap_or(0);
            (l.kind, -self.units[l.host].weight, pin, l.host, l.h, l.f)
        };
        while !frontier.is_empty() {
            order.extend(frontier.iter().copied());
            let mut next: BTreeMap<usize, Lead> = BTreeMap::new();
            for &h in &frontier {
                for &(f, hp, fp, k) in &rel[h] {
                    if seen.contains(&f) || self.units[f].fixed {
                        continue;
                    }
                    let Some(k) = k else { continue };
                    let cand = Lead {
                        host: h,
                        h: hp,
                        f: fp,
                        kind: k,
                    };
                    match next.get(&f) {
                        Some(old) if rank(old) <= rank(&cand) => {}
                        _ => {
                            next.insert(f, cand);
                        }
                    }
                }
            }
            let mut found: Vec<usize> = next.keys().copied().collect();
            found.sort_by_key(|u| self.key(*u));
            seen.extend(found.iter().copied());
            leads.extend(next);
            frontier = found;
        }
        (order, leads)
    }

    /// The weight of the units a tree of `leads` over the units in `order` would move along to set each follower on the side its
    /// host's pin faces, as the followers are set (step 4) without their moves across.
    fn wrong(&self, order: &[usize], leads: &BTreeMap<usize, Lead>) -> usize {
        let mut d: BTreeMap<usize, Pt> = BTreeMap::new();
        for u in order {
            let Some(l) = leads.get(u) else { continue };
            let (Some(s), Some(t)) = (self.side(l.h), self.side(l.f)) else {
                continue;
            };
            let dh = d.get(&l.host).copied().unwrap_or_default();
            let h = self.pt(l.h).plus(dh);
            let p = self.pt(l.f);
            let fluid = self.units[*u].parts.iter().any(|i| {
                matches!(self.w.parts[*i].role, Role::Source | Role::Actuator | Role::Valve)
            });
            let behind = |p: Pt| match l.kind {
                Kind::Facing => beyond(h, p, s) < CHAIN_GAP_MIN,
                Kind::Corner => beyond(h, p, s) <= 0 || beyond(p, h, t) <= 0,
                Kind::Level => false,
            };
            let mut du = if !fluid && !behind(p) { Pt::default() } else { dh };
            if behind(p.plus(du)) {
                du = du.plus(along_of(s, CHAIN_GAP - beyond(h, p.plus(du), s)));
            }
            d.insert(*u, du);
        }
        d.iter()
            .filter(|(_, x)| **x != Pt::default())
            .map(|(u, _)| self.units[*u].weight.max(1) as usize)
            .sum()
    }

    /// Step 4: the followers of each host, from the anchors down.
    fn followers(&mut self) {
        let order = self.order.clone();
        self.settled = order.iter().copied().filter(|u| self.lead[*u].is_none()).collect();
        for h in order {
            let mut sides: BTreeMap<(Side, bool, Option<Side>), Vec<usize>> = BTreeMap::new();
            for (f, l) in self.lead.iter().enumerate() {
                let Some(l) = l else { continue };
                if l.host != h {
                    continue;
                }
                let Some(s) = self.side(l.h) else { continue };
                let stack = l.kind == Kind::Corner;
                sides
                    .entry((s, stack, if stack { self.side(l.f) } else { None }))
                    .or_default()
                    .push(f);
            }
            for ((s, stack, _), members) in sides {
                // A follower moves with its host, unless it stands well beyond the host's new place where it is (the
                // host moved into the gap before it) and is no part of the supply or the work of a fluid page; its own
                // followers take its move when their turn comes.
                for f in &members {
                    let Some(l) = self.lead[*f] else { continue };
                    let fluid = self.units[*f].parts.iter().any(|i| {
                        matches!(self.w.parts[*i].role, Role::Source | Role::Actuator | Role::Valve)
                    });
                    let stays = !fluid
                        && beyond(self.pt(l.h), self.pt(l.f), s) >= CHAIN_GAP_MIN
                        && !self.meets(*f, &[h]);
                    if !stays {
                        self.d[*f] = self.d[h];
                        if self.needed.contains(&h) && self.d[h] != Pt::default() {
                            self.needed.insert(*f);
                        }
                    }
                }
                if stack {
                    self.stack(h, s, &members);
                } else {
                    self.row(h, s, &members);
                }
                self.settled.extend(members.iter().copied());
            }
        }
    }

    /// Whether `u`'s footprint meets a footprint of `others`.
    fn meets(&self, u: usize, others: &[usize]) -> bool {
        let a = self.footprint(u);
        others.iter().any(|o| *o != u && geom::texts_meet(a, self.footprint(*o)))
    }

    /// Moves unit `u` alone by `by` (its followers take it when their turn comes).
    fn nudge(&mut self, u: usize, by: Pt) {
        if !self.units[u].fixed {
            self.d[u] = self.d[u].plus(by);
        }
    }

    /// A row of followers on side `s` of host `h` (§4.4).
    fn row(&mut self, h: usize, s: Side, members: &[usize]) {
        // Across: each facing follower wants its pin on its host pin's line, in the order of the host's pins.
        let mut items: Vec<(Tick, usize, Lead)> = members
            .iter()
            .filter_map(|f| self.lead[*f].map(|l| (across(self.pt(l.h), s), *f, l)))
            .collect();
        items.sort_by_key(|(a, f, _)| (*a, *f));
        let mut want: Vec<Tick> = Vec::new();
        let mut size: Vec<Tick> = Vec::new();
        let mut low: Vec<Tick> = Vec::new();
        for (_, f, l) in &items {
            let fp = self.footprint(*f);
            let (lo, hi) = if s.vertical() { (fp.x0, fp.x1) } else { (fp.y0, fp.y1) };
            let shift = match l.kind {
                Kind::Facing => across(self.pt(l.h), s) - across(self.pt(l.f), s),
                _ => 0,
            };
            low.push(lo);
            size.push(hi - lo);
            want.push(lo + shift);
        }
        let placed = pooled(&want, &size, ROW_GAP);
        for (k, (_, f, _)) in items.iter().enumerate() {
            let by = whole(placed[k] - low[k]);
            if by != 0 {
                self.nudge(*f, across_vec(s, by));
            }
        }
        // Along: keepers keep their distance; the others join the row's line.
        let mut keep: Vec<Tick> = Vec::new();
        let mut movers: Vec<(usize, Tick)> = Vec::new();
        for (_, f, l) in &items {
            let dist = beyond(self.pt(l.h), self.pt(l.f), s);
            match l.kind {
                Kind::Level => {
                    if dist != 0 {
                        self.nudge(*f, along_of(s, -dist));
                        self.needed.insert(*f);
                    }
                }
                _ if dist >= CHAIN_GAP_MIN && !self.meets(*f, &[h]) => keep.push(dist),
                _ => movers.push((*f, dist)),
            }
        }
        // The room of §4.5: the chain gap, and a step more than the tracks of the row's stepping wires.
        let stepping: Vec<(Tick, Tick)> = items
            .iter()
            .filter(|(_, _, l)| l.kind == Kind::Facing)
            .map(|(_, _, l)| (across(self.pt(l.h), s), across(self.pt(l.f), s)))
            .collect();
        let room = (crate::tracks::tracks_needed(&stepping) as Tick)
            .saturating_mul(TRACK_PITCH)
            .saturating_add(TICKS_PER_STEP)
            .max(CHAIN_GAP);
        let line = keep
            .iter()
            .copied()
            .filter(|d| (room..=room + ROW_PULL).contains(d))
            .max()
            .unwrap_or(room);
        for (f, dist) in movers {
            self.nudge(f, along_of(s, whole_up(line - dist)));
            // Further while its footprint meets its host's.
            let mut more = 0;
            while (self.meets(f, &[h]) || self.meets(f, &self.settled)) && more < 4 * CHAIN_GAP {
                self.nudge(f, along_of(s, TICKS_PER_STEP));
                more += TICKS_PER_STEP;
            }
            self.needed.insert(f);
        }
    }

    /// A stack of corner followers on side `s` of host `h` (§4.4).
    fn stack(&mut self, h: usize, s: Side, members: &[usize]) {
        let mut items: Vec<(usize, Lead)> = members.iter().filter_map(|f| self.lead[*f].map(|l| (*f, l))).collect();
        let Some(sf) = items.first().and_then(|(_, l)| self.side(l.f)) else {
            return;
        };
        // The way the wires go from the drops: against the followers' pins' side. The drop furthest against it turns
        // outermost.
        let way = opposite(sf);
        items.sort_by_key(|(f, l)| (beyond(Pt::default(), self.pt(l.h), way), *f));
        // Across: each pin MARK_LEAD before its drop, unless it is further.
        for (f, l) in &items {
            let lead = beyond(self.pt(l.h), self.pt(l.f), way);
            if lead < MARK_LEAD {
                self.nudge(*f, along_of(way, whole_up(MARK_LEAD - lead)));
                self.needed.insert(*f);
            }
        }
        // Along: a follower beyond its drop by at least two steps and clear of its host stays; the others, innermost
        // first, each take the nearest place from the chain gap outward where its footprint meets no other's.
        let host_line = items.iter().map(|(_, l)| beyond(Pt::default(), self.pt(l.h), s)).max().unwrap_or(0);
        let mut movers = Vec::new();
        let mut placed: Vec<usize> = self.settled.clone();
        placed.push(h);
        for (f, l) in &items {
            let dist = beyond(self.pt(l.h), self.pt(l.f), s);
            if dist >= CHAIN_GAP_MIN && !self.meets(*f, &[h]) {
                placed.push(*f);
            } else {
                movers.push((*f, *l));
            }
        }
        for (f, l) in movers.into_iter().rev() {
            let at = beyond(Pt::default(), self.pt(l.f), s);
            self.nudge(f, along_of(s, whole_up(host_line + CHAIN_GAP - at)));
            // The free places from there outward, the first few, and of them the one of least estimate.
            let reach = self.w.page.area.w().max(self.w.page.area.h());
            let mut best: Option<(i64, Tick)> = None;
            let (mut more, mut found) = (0, 0);
            while more < reach && found < 8 {
                if !self.meets(f, &placed) {
                    let e = self.estimate();
                    if best.is_none_or(|(b, _)| e < b) {
                        best = Some((e, more));
                    }
                    found += 1;
                }
                self.nudge(f, along_of(s, TICKS_PER_STEP));
                more += TICKS_PER_STEP;
            }
            let back = more - best.map_or(more, |(_, m)| m);
            self.nudge(f, along_of(s, -back));
            placed.push(f);
            self.needed.insert(f);
        }
    }

    /// Step 5a: a mark of a net with a rail's line (two junctions or more on one horizontal line) takes that line; a group whose part drops
    /// straight from a junction it faces takes that junction's line across (the price decides, step 6).
    fn trunk_marks(&mut self) {
        for n in &self.w.nets {
            if n.pins.len() < 3 {
                continue;
            }
            let Some(line) = self.w.line(n) else {
                continue;
            };
            for p in &n.pins {
                let (i, _, side) = self.w.pins[p];
                let u = self.unit_of[i];
                let mark = matches!(self.w.parts[i].role, Role::MarkIn | Role::MarkOut)
                    || self.units[u].parts.len() == 1 && self.w.parts[i].pins.len() == 1;
                if !mark || self.lead[u].is_some() || side.is_none_or(Side::vertical) {
                    continue;
                }
                let dy = line - self.pt(*p).y;
                if dy != 0 && !self.units[u].fixed {
                    self.d[u] = self.d[u].plus(Pt::new(0, dy));
                    self.needed.insert(u);
                }
            }
        }
        for g in self.groups() {
            let root = g[0];
            let mut shifts: BTreeMap<Tick, usize> = BTreeMap::new();
            for (p, j) in self.drops() {
                let (i, _, side) = self.w.pins[&p];
                let Some(s) = side.filter(|s| s.vertical()) else { continue };
                let at = self.pt(p);
                if self.unit_of[i] == root && beyond(at, j, s) > 0 && j.x != at.x && (j.x - at.x).abs() <= CHAIN_GAP_MIN
                {
                    *shifts.entry(whole(j.x - at.x)).or_default() += 1;
                }
            }
            if let Some((dx, _)) = shifts.into_iter().max_by_key(|(dx, k)| (*k, -dx.abs(), -*dx))
                && dx != 0
            {
                self.shift(&g, Pt::new(dx, 0));
            }
        }
    }

    /// Each pin wired straight to a junction by one wire, with the junction's point.
    fn drops(&self) -> Vec<(PinId, Pt)> {
        let mut out = Vec::new();
        for w in &self.w.page.wires {
            for (a, b) in [(w.ends[0], w.ends[1]), (w.ends[1], w.ends[0])] {
                if let (End::Pin(p), End::Junction(j)) = (a, b)
                    && self.w.pins.contains_key(&p)
                    && let Some(x) = self.w.page.junctions.iter().find(|x| x.id == j)
                {
                    out.push((p, x.at));
                }
            }
        }
        out.sort();
        out
    }

    /// The groups: each movable root with what hangs on it.
    fn groups(&self) -> Vec<Vec<usize>> {
        self.order
            .iter()
            .filter(|u| self.lead[**u].is_none() && !self.units[**u].fixed)
            .map(|u| self.subtree(*u))
            .collect()
    }

    /// Step 5b: bands (rule E2).
    fn bands(&mut self) {
        if self.w.page.kind == crate::model::PageKind::Fluid {
            self.fluid_bands();
            return;
        }
        if self.w.page.kind != crate::model::PageKind::Electrical {
            return;
        }
        let rails = rails(&self.w);
        let head = rails.iter().filter(|r| r.1).map(|r| r.0).min();
        for g in self.groups() {
            let mut down: Tick = 0;
            let mut up: Tick = 0;
            for u in &g {
                for i in &self.units[*u].parts {
                    let part = self.w.parts[*i];
                    if matches!(part.role, Role::MarkIn | Role::MarkOut) || part.frame_only || part.pins.is_empty() {
                        continue;
                    }
                    let b = part.bounds().moved(self.d[*u]);
                    for (line, feeding, circuit, _) in &rails {
                        if circuit.contains(i) {
                            if *feeding && b.y0 < *line + TRUNK_GAP {
                                down = down.max(*line + TRUNK_GAP - b.y0);
                            } else if !*feeding && b.y1 > *line - TRUNK_GAP {
                                up = up.max(b.y1 - (*line - TRUNK_GAP));
                            }
                        }
                    }
                    if let Some(y) = head
                        && b.y0 < y
                        && !rails.iter().any(|r| r.2.contains(i))
                    {
                        down = down.max(y + TRUNK_GAP - b.y0);
                    }
                }
            }
            let by = if down > 0 { whole_up(down) } else { -whole_up(up) };
            if by != 0 {
                self.shift(&g, Pt::new(0, by));
                self.needed.extend(g.iter().copied());
            }
        }
    }

    /// Step 5b on a fluid page (rule F1): a group without a valve whose supply stands not lower than every valve, or
    /// whose actuator not higher, is moved along the flow by the least distance, the chain gap clear of the valves.
    fn fluid_bands(&mut self) {
        let roles: Vec<Role> = self.w.parts.iter().map(|p| p.role).collect();
        let role = |i: usize| roles[i];
        let valves: Vec<Rect> = (0..self.w.parts.len())
            .filter(|i| role(*i) == Role::Valve)
            .map(|i| self.w.parts[i].bounds().moved(self.d[self.unit_of[i]]))
            .collect();
        let (Some(top), Some(bottom)) = (valves.iter().map(|r| r.y0).min(), valves.iter().map(|r| r.y1).max()) else {
            return;
        };
        // The groups without a valve, and in a group with one each branch of the supply that hangs on a valve.
        let mut branches: Vec<Vec<usize>> = Vec::new();
        for g in self.groups() {
            let has_valve = g.iter().any(|u| self.units[*u].parts.iter().any(|i| role(*i) == Role::Valve));
            if !has_valve {
                branches.push(g);
                continue;
            }
            for u in &g {
                let source = self.units[*u].parts.iter().any(|i| role(*i) == Role::Source);
                let host_source = self.lead[*u].is_some_and(|l| {
                    self.units[l.host].parts.iter().any(|i| role(*i) == Role::Source || role(*i) == Role::Valve)
                        && !self.units[l.host].parts.iter().any(|i| role(*i) == Role::Valve)
                });
                if source && !host_source {
                    branches.push(self.subtree(*u));
                }
            }
        }
        for g in branches {
            let parts: Vec<(usize, usize)> = g.iter().flat_map(|u| self.units[*u].parts.iter().map(move |i| (*u, *i))).collect();
            if parts.iter().any(|(_, i)| role(*i) == Role::Valve) {
                continue;
            }
            let mut by: Tick = 0;
            for (u, i) in &parts {
                let b = self.w.parts[*i].bounds().moved(self.d[*u]);
                match role(*i) {
                    Role::Source if b.y0 < bottom => {
                        by = by.max(bottom + CHAIN_GAP - b.y0);
                        // And a trunk step under the junctions of its nets below the valves.
                        for p in &self.w.parts[*i].pins {
                            let at = self.pt(p.id);
                            for n in self.w.nets.iter().filter(|n| n.pins.contains(&p.id)) {
                                for j in n.junctions.iter().filter_map(|j| self.w.page.junctions.iter().find(|x| x.id == *j)) {
                                    if j.at.y >= bottom && p.side == Some(Side::Up) {
                                        by = by.max(j.at.y + TRUNK_GAP - at.y);
                                    }
                                }
                            }
                        }
                    }
                    Role::Actuator if b.y1 > top => by = by.min(top - CHAIN_GAP - b.y1),
                    _ => {}
                }
            }
            let by = if by > 0 { whole_up(by) } else { -whole_up(-by) };
            if by != 0 {
                self.shift(&g, Pt::new(0, by));
                self.needed.extend(g.iter().copied());
            }
        }
    }

    /// The moves so far, by part (whole steps).
    fn moves(&self) -> Vec<(PartId, Pt)> {
        let mut out: Vec<(PartId, Pt)> = Vec::new();
        for (u, unit) in self.units.iter().enumerate() {
            let d = Pt::new(whole(self.d[u].x), whole(self.d[u].y));
            if d == Pt::default() || unit.fixed {
                continue;
            }
            for i in &unit.parts {
                out.push((self.w.parts[*i].id, d));
            }
        }
        out.sort();
        out
    }

    /// Step 6a: a follower's own move that sets no rule right stays only where it pays its price.
    fn priced(&mut self) {
        let order: Vec<usize> = self.order.iter().rev().copied().collect();
        for u in order {
            if self.needed.contains(&u) || self.units[u].fixed {
                continue;
            }
            let own = match self.lead[u] {
                Some(l) => self.d[u].minus(self.d[l.host]),
                None => self.d[u],
            };
            if own == Pt::default() {
                continue;
            }
            let sub = self.subtree(u);
            let with = self.estimate();
            self.shift(&sub, Pt::new(-own.x, -own.y));
            let without = self.estimate();
            let parts: i64 = sub.iter().map(|x| self.units[*x].parts.len() as i64).sum();
            if without - with >= MOVE_PRICE * i64::from(TICKS_PER_STEP) * parts {
                self.shift(&sub, own);
            }
        }
    }

    /// Step 6b: two neighbouring groups change places where that gains more than its price.
    fn swaps(&mut self) {
        for _ in 0..PASSES {
            let mut groups = self.groups();
            groups.sort_by_key(|g| (self.span(g).x0, g[0]));
            let mut changed = false;
            for k in 1..groups.len() {
                let (a, b) = (&groups[k - 1], &groups[k]);
                let (ra, rb) = (self.span(a), self.span(b));
                // Side by side: their heights overlap.
                if ra.y1 <= rb.y0 || rb.y1 <= ra.y0 {
                    continue;
                }
                let with = self.estimate();
                let (da, db) = (whole(rb.x1 - ra.x1), whole(ra.x0 - rb.x0));
                self.shift(a, Pt::new(da, 0));
                self.shift(b, Pt::new(db, 0));
                let without = self.estimate();
                let parts: i64 = a.iter().chain(b).map(|x| self.units[*x].parts.len() as i64).sum();
                let clear = !a.iter().any(|u| self.meets(*u, b));
                if clear && with - without > MOVE_PRICE * i64::from(TICKS_PER_STEP) * parts {
                    changed = true;
                    groups.swap(k - 1, k);
                } else {
                    self.shift(a, Pt::new(-da, 0));
                    self.shift(b, Pt::new(-db, 0));
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// The footprints of `units` together.
    fn span(&self, units: &[usize]) -> Rect {
        units
            .iter()
            .map(|u| self.footprint(*u))
            .reduce(Rect::union)
            .unwrap_or_default()
    }

    /// Whether unit `u` stands inside the drawing area and on no other unit.
    fn free(&self, u: usize) -> bool {
        !self.clash(&[u], None)
    }

    /// Whether a unit of `g` leaves the drawing area, or meets a unit outside `g` (of `against` only, where given): a
    /// body on a body (but two parts drawn on one pin's point), or the first step of a wired pin's way into a body.
    fn clash(&self, g: &[usize], against: Option<&BTreeSet<usize>>) -> bool {
        let area = self.w.page.area;
        let solid = |i: &usize| {
            let p = self.w.parts[*i];
            !p.frame_only && !p.pins.is_empty()
        };
        let wired: BTreeSet<PinId> = self.w.nets.iter().flat_map(|n| n.pins.iter().copied()).collect();
        // (unit, part, body, stubs, texts)
        let mut items: Vec<(usize, usize, Rect, Vec<Rect>, Vec<Rect>)> = Vec::new();
        for (v, unit) in self.units.iter().enumerate() {
            for j in unit.parts.iter().filter(|j| solid(j)) {
                let p = self.w.parts[*j];
                let stubs = p
                    .pins
                    .iter()
                    .filter(|x| wired.contains(&x.id))
                    .filter_map(|x| {
                        let at = p.at.plus(x.at).plus(self.d[v]);
                        let s = x.side?;
                        let end = at.plus(along_of(s, TICKS_PER_STEP));
                        let r = Rect::new(at, end);
                        Some(if s.vertical() { Rect { x0: r.x0 - 1, x1: r.x1 + 1, ..r } } else { Rect { y0: r.y0 - 1, y1: r.y1 + 1, ..r } })
                    })
                    .collect();
                let texts = self.texts[*j].iter().map(|t| t.moved(p.at).moved(self.d[v])).collect();
                items.push((v, *j, p.bounds().moved(self.d[v]), stubs, texts));
            }
        }
        for (u, i, b, stubs, texts) in items.iter().filter(|x| g.contains(&x.0)) {
            if !area.holds(*b) {
                return true;
            }
            for (v, j, c, others, their) in items
                .iter()
                .filter(|x| !g.contains(&x.0) && against.is_none_or(|a| a.contains(&x.0)))
            {
                // Two parts drawn on one pin's point (an interruption point on a pin) stay so; no plan makes a pair.
                let zero = Pt::default();
                if self.pin_on_pin(*i, zero, *j, zero) && self.d[*u] == self.d[*v] {
                    continue;
                }
                if geom::texts_meet(*b, *c)
                    || stubs.iter().any(|s| s.overlaps(*c))
                    || others.iter().any(|s| s.overlaps(*b))
                {
                    return true;
                }
                // A moved part's texts on another part or its texts, and the other way round.
                let moved = self.d[*u] != self.d[*v];
                if moved
                    && (texts.iter().any(|t| geom::texts_meet(*t, *c) || their.iter().any(|o| geom::texts_meet(*t, *o)))
                        || their.iter().any(|o| geom::texts_meet(*o, *b)))
                {
                    return true;
                }
            }
        }
        false
    }

    fn pin_on_pin(&self, i: usize, di: Pt, j: usize, dj: Pt) -> bool {
        let (p, o) = (self.w.parts[i], self.w.parts[j]);
        p.pins
            .iter()
            .any(|a| o.pins.iter().any(|b| p.at.plus(a.at).plus(di) == o.at.plus(b.at).plus(dj)))
    }

    /// Step 7: fitting.
    fn fit(&mut self) {
        let area = self.w.page.area;
        let mut groups = self.groups();
        groups.sort_by_key(|g| self.key(g[0]));
        // Into the drawing area by the least distance: the branch that leaves it, the deepest first, or the group.
        let bodies = |p: &Self, units: &[usize]| {
            units
                .iter()
                .flat_map(|u| p.units[*u].parts.iter().map(move |i| (*u, *i)))
                .map(|(u, i)| p.w.parts[i].bounds().moved(p.d[u]))
                .reduce(Rect::union)
                .unwrap_or_default()
        };
        for u in self.order.clone().into_iter().rev() {
            if self.lead[u].is_none() || self.units[u].fixed {
                continue;
            }
            let own = bodies(self, &[u]);
            if area.holds(own) {
                continue;
            }
            let sub = self.subtree(u);
            let r = bodies(self, &sub);
            let dx = if r.x0 < area.x0 {
                whole_up(area.x0 - r.x0)
            } else if r.x1 > area.x1 {
                -whole_up(r.x1 - area.x1)
            } else {
                0
            };
            let dy = if r.y0 < area.y0 {
                whole_up(area.y0 - r.y0)
            } else if r.y1 > area.y1 {
                -whole_up(r.y1 - area.y1)
            } else {
                0
            };
            if r.w() <= area.w() && r.h() <= area.h() {
                self.shift(&sub, Pt::new(dx, dy));
            }
        }
        for g in &groups {
            let r = bodies(self, g).grown(-1);
            let dx = if r.x0 < area.x0 {
                whole_up(area.x0 - r.x0)
            } else if r.x1 > area.x1 {
                -whole_up(r.x1 - area.x1)
            } else {
                0
            };
            let dy = if r.y0 < area.y0 {
                whole_up(area.y0 - r.y0)
            } else if r.y1 > area.y1 {
                -whole_up(r.y1 - area.y1)
            } else {
                0
            };
            if (dx, dy) != (0, 0) && r.w() <= area.w() && r.h() <= area.h() {
                self.shift(g, Pt::new(dx, dy));
            }
        }
        // A follower with none of its own that meets another part steps along its host pin's side to the nearest free
        // place, at most a chain gap either way (a mark in a stack).
        for u in self.order.clone().into_iter().rev() {
            let Some(s) = self.lead[u].and_then(|l| self.side(l.h)) else {
                continue;
            };
            if self.units[u].fixed || self.subtree(u).len() > 1 || !self.clash(&[u], None) {
                continue;
            }
            'step: for k in 1..=CHAIN_GAP / TICKS_PER_STEP {
                for dir in [1, -1] {
                    let by = along_of(s, dir * k * TICKS_PER_STEP);
                    self.nudge(u, by);
                    if !self.clash(&[u], None) {
                        break 'step;
                    }
                    self.nudge(u, Pt::new(-by.x, -by.y));
                }
            }
        }
        // Of two groups on each other the later takes the next free place across, the nearest first, to the right
        // before the left: the fixed units stand first, then the groups in their order.
        let mut placed: BTreeSet<usize> = (0..self.units.len())
            .filter(|u| !groups.iter().any(|g| g.contains(u)))
            .collect();
        for g in &groups {
            if self.clash(g, Some(&placed)) {
                let reach = area.w() / TICKS_PER_STEP;
                'search: for k in 1..=reach {
                    for dir in [1, -1] {
                        let by = Pt::new(dir * k * TICKS_PER_STEP, 0);
                        self.shift(g, by);
                        if !self.clash(g, Some(&placed)) {
                            break 'search;
                        }
                        self.shift(g, Pt::new(-by.x, 0));
                    }
                }
            }
            placed.extend(g.iter().copied());
        }
        // A unit that still has no room goes back where it stood; named when its move set a rule right.
        for _ in 0..self.units.len() {
            let Some(u) = (0..self.units.len())
                .filter(|u| self.d[*u] != Pt::default() && !self.units[*u].fixed && !self.free(*u))
                .min_by_key(|u| std::cmp::Reverse(self.key(*u)))
            else {
                break;
            };
            if self.needed.contains(&u)
                && let Some(l) = self.lead[u]
                && let Some(s) = self.side(l.h)
            {
                for i in &self.units[u].parts {
                    self.left.push((self.w.parts[*i].id, s));
                }
            }
            for f in self.subtree(u) {
                self.d[f] = Pt::default();
            }
        }
        // Whatever still meets a moved unit at its old place: the moved one goes back too.
        for _ in 0..self.units.len() {
            let Some(u) = (0..self.units.len()).find(|u| self.d[*u] != Pt::default() && !self.free(*u)) else {
                break;
            };
            self.d[u] = Pt::default();
        }
        self.left.sort();
        self.left.dedup();
    }

    /// The estimate of §4.7, in ticks: the plain shapes' length, [`BEND`] per bend, [`CROSSING`] per crossing.
    fn estimate(&self) -> i64 {
        let drops = self.drops();
        let mut segs: Vec<(usize, geom::Seg)> = Vec::new();
        let mut cost: i64 = 0;
        let bend = crate::consts::BEND * i64::from(TICKS_PER_STEP);
        let cross = crate::consts::CROSSING * i64::from(TICKS_PER_STEP);
        for (k, n) in self.w.nets.iter().enumerate() {
            match n.pins.as_slice() {
                [a, b] => {
                    let (shape, bends) = plain(self.pt(*a), self.side(*a), self.pt(*b), self.side(*b));
                    cost += bend * bends;
                    for s in geom::segments(&shape) {
                        cost += geom::length(s);
                        segs.push((k, s));
                    }
                }
                pins if pins.len() >= 3 => {
                    let pts: Vec<(Pt, Option<Side>)> = pins.iter().map(|p| (self.pt(*p), self.side(*p))).collect();
                    let vertical: Vec<&(Pt, Option<Side>)> = pts.iter().filter(|(_, s)| s.is_some_and(Side::vertical)).collect();
                    let line = match self.w.line(n) {
                        Some(y) => Some(y),
                        _ if vertical.iter().all(|(_, s)| *s == Some(Side::Up)) && !vertical.is_empty() => {
                            vertical.iter().map(|(p, _)| p.y).min().map(|y| y - TRUNK_GAP)
                        }
                        _ if vertical.iter().all(|(_, s)| *s == Some(Side::Down)) && !vertical.is_empty() => {
                            vertical.iter().map(|(p, _)| p.y).max().map(|y| y + TRUNK_GAP)
                        }
                        _ => None,
                    };
                    match line {
                        Some(y) => {
                            let (x0, x1) = pts.iter().fold((Tick::MAX, Tick::MIN), |(a, b), (p, _)| (a.min(p.x), b.max(p.x)));
                            let trunk = (Pt::new(x0, y), Pt::new(x1, y));
                            cost += geom::length(trunk);
                            segs.push((k, trunk));
                            for (pin, (p, s)) in pins.iter().zip(&pts) {
                                if p.y == y {
                                    continue;
                                }
                                let drop = (*p, Pt::new(p.x, y));
                                cost += geom::length(drop);
                                segs.push((k, drop));
                                let toward = if y < p.y { Side::Up } else { Side::Down };
                                if *s != Some(toward) {
                                    cost += 2 * bend;
                                }
                                // A drop wired to a junction on the line beside it: a bend and the step across.
                                if let Some((_, j)) = drops.iter().find(|(q, j)| q == pin && j.y == y && j.x != p.x) {
                                    cost += bend + i64::from((j.x - p.x).abs());
                                }
                            }
                        }
                        None => {
                            let mut sorted = pts.clone();
                            sorted.sort_by_key(|(p, _)| (p.x, p.y));
                            for pair in sorted.windows(2) {
                                let (shape, bends) = plain(pair[0].0, pair[0].1, pair[1].0, pair[1].1);
                                cost += bend * bends;
                                for s in geom::segments(&shape) {
                                    cost += geom::length(s);
                                    segs.push((k, s));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for (i, (ka, a)) in segs.iter().enumerate() {
            for (kb, b) in &segs[i + 1..] {
                if ka != kb && crosses(*a, *b) {
                    cost += cross;
                }
            }
        }
        // A plain shape through another part's body, or nearer to it than the router keeps: the way round, two bends.
        let mut bodies: Vec<(usize, Rect)> = Vec::new();
        for (u, unit) in self.units.iter().enumerate() {
            for i in &unit.parts {
                let p = self.w.parts[*i];
                if !p.frame_only {
                    bodies.push((*i, p.bounds().moved(self.d[u]).grown(BODY_GAP - 1)));
                }
            }
        }
        for (k, seg) in &segs {
            let own = &self.w.nets[*k].pins;
            for (i, b) in &bodies {
                if !own.iter().any(|p| self.w.pins[p].0 == *i) && geom::crosses(*seg, *b) {
                    cost += 2 * bend;
                }
            }
        }
        cost
    }
}

/// Whether two segments cross at a right angle inside both.
fn crosses((a0, a1): geom::Seg, (b0, b1): geom::Seg) -> bool {
    let h = |p: Pt, q: Pt| p.y == q.y && p.x != q.x;
    let v = |p: Pt, q: Pt| p.x == q.x && p.y != q.y;
    let inside = |x: Tick, a: Tick, b: Tick| a.min(b) < x && x < a.max(b);
    if h(a0, a1) && v(b0, b1) {
        inside(b0.x, a0.x, a1.x) && inside(a0.y, b0.y, b1.y)
    } else if v(a0, a1) && h(b0, b1) {
        inside(a0.x, b0.x, b1.x) && inside(b0.y, a0.y, a1.y)
    } else {
        false
    }
}

/// The plain shape of a wire between `a` (leaving along `sa`) and `b`, and the bends it needs (§4.7): none for a
/// facing pair in line, one for a corner, two for a step or a hairpin, four for a pair on the wrong side of each other.
fn plain(a: Pt, sa: Option<Side>, b: Pt, sb: Option<Side>) -> (Vec<Pt>, i64) {
    let (Some(s), Some(t)) = (sa, sb) else {
        return (vec![a, Pt::new(b.x, a.y), b], i64::from(a.x != b.x && a.y != b.y));
    };
    let out = |p: Pt, s: Side| p.plus(along_of(s, TICKS_PER_STEP));
    if t == opposite(s) {
        let d = beyond(a, b, s);
        if d > 0 && across(a, s) == across(b, s) {
            return (vec![a, b], 0);
        }
        if d > 0 {
            let m = if s.vertical() { Pt::new(a.x, (a.y + b.y) / 2) } else { Pt::new((a.x + b.x) / 2, a.y) };
            let n = if s.vertical() { Pt::new(b.x, m.y) } else { Pt::new(m.x, b.y) };
            return (vec![a, m, n, b], 2);
        }
        let (p, q) = (out(a, s), out(b, t));
        let m = if s.vertical() { Pt::new((a.x + b.x) / 2, p.y) } else { Pt::new(p.x, (a.y + b.y) / 2) };
        let n = if s.vertical() { Pt::new(m.x, q.y) } else { Pt::new(q.x, m.y) };
        return (vec![a, p, m, n, q, b], 4);
    }
    if s == t {
        let reach = beyond(a, b, s).max(0);
        let p = out(a, s).plus(along_of(s, reach));
        let q = if s.vertical() { Pt::new(b.x, p.y) } else { Pt::new(p.x, b.y) };
        return (vec![a, p, q, b], 2);
    }
    let c = if s.vertical() { Pt::new(a.x, b.y) } else { Pt::new(b.x, a.y) };
    if beyond(a, b, s) > 0 && beyond(b, a, t) > 0 {
        return (vec![a, c, b], 1);
    }
    let (p, q) = (out(a, s), out(b, t));
    let m = if s.vertical() { Pt::new(q.x, p.y) } else { Pt::new(p.x, q.y) };
    (vec![a, p, m, q, b], 3)
}

/// The places of blocks of sizes `size` that want their low edges at `want`, in this order with `gap` between, of
/// least total move (pool adjacent violators, each block at its members' lower median).
fn pooled(want: &[Tick], size: &[Tick], gap: Tick) -> Vec<Tick> {
    // In offset coordinates the constraint is a plain order.
    let mut offset = Vec::with_capacity(want.len());
    let mut acc: Tick = 0;
    for s in size {
        offset.push(acc);
        acc += s + gap;
    }
    let mut blocks: Vec<(Vec<Tick>, Tick, usize)> = Vec::new();
    for (k, w) in want.iter().enumerate() {
        blocks.push((vec![w - offset[k]], w - offset[k], 1));
        while blocks.len() >= 2 && blocks[blocks.len() - 2].1 > blocks[blocks.len() - 1].1 {
            let Some(last) = blocks.pop() else { break };
            let Some(prev) = blocks.last_mut() else { break };
            prev.0.extend(last.0);
            prev.0.sort_unstable();
            prev.1 = prev.0[(prev.0.len() - 1) / 2];
            prev.2 += last.2;
        }
    }
    let mut out = Vec::with_capacity(want.len());
    for (_, v, n) in blocks {
        for _ in 0..n {
            out.push(v + offset[out.len()]);
        }
    }
    out
}
