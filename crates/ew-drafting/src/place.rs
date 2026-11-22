// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The placer (docs/design/06a-drafting.md §4): [`arrange`] puts each part where the page is best for it, within the
//! rules of §1.6 ([`crate::rules`]), and routes the page as it would become.
//!
//! # The run
//!
//! 1. **Route's plan** over the same wires (every wire of the scope, whoever shaped it): the plan to beat.
//! 2. **The arrangement** (`sketch`): the units, their relations by strength, one tree per group, and each unit's
//!    place from its lead: rows, stacks, the followers beside. It rates nothing.
//! 3. **The rules** (§1.6). The parts a broken rule names go back with what hangs on them until [`rules_kept`] holds
//!    (the rules read the parts and the nets alone).
//! 4. **Routed and judged.** The arrangement is routed for real and measured. It is taken when every row of terminals
//!    keeps the order it stands in, the rules hold on the routed page, no wire or text gains a finding (where it routes more wires than Route's plan, a wire newly across a
//!    text does not count), it has no more faults and (where it routes no more wires) no more
//!    crossings than Route's plan, and it is better than Route's plan by the measure with the price of its moves added
//!    ([`MOVE_PRICE`] for each part). Where the whole arrangement is not taken, its trees are tried one after another,
//!    each kept when the page gets better with it; then the arrangement without one branch, and one branch alone.
//! 5. **The price, for real.** From the plan taken, each branch (a unit's own move with what hangs on it; the members
//!    of a row the person made, of one shape and level, as one: a row is not left half moved) is put back in turn
//!    where that breaks no rule the plan set right; the plan without it is routed, and kept when it is the better one
//!    with the price of its moves: no part moves for a tie.
//! 6. **Without a better plan** Route's plan of the scope is offered. The parts that stand against a rule and stay
//!    (they had no room on the side their host's pin faces, or the page was no better with them there) are named.
//! 7. **Again**, on the page the plan leaves, until a run changes nothing ([`ROUNDS`] at most): the plan offered is
//!    the sum, and a second press finds nothing to do.
//!
//! On a page where a card would step along its edge (`sketch::CardSteps`) steps 2 to 6 are made three times, with the
//! step, with the part of it that clears the card's supply, and without it; where a strip's other terminals held a
//! channel's terminal off its column, each of them once more with the strip opened as one (`sketch::Sketch::open`,
//! FT-2110); the best page is offered.
//!
//! Nothing here estimates what the router will draw: every plan compared has been routed. The work of one run is one
//! run of the router over the page for each plan tried: the whole arrangement, at most [`BRANCHES`] trees, at most
//! twice [`BRANCHES`] plans without or with one branch, and at most [`BRANCHES`] for the price. A run of the router
//! grows with the page's wires and, in its search, with the drawing area; setting a tree asks the text placer once for
//! each row and stack and tries at most 4 x [`crate::consts::REACH`] steps of its root for each of at most [`crate::consts::ANCHORS`] anchors.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{BRANCHES, MOVE_PRICE, ROUNDS, TICKS_PER_STEP};
use crate::marks::{draw_marks, mark_point, move_mark};
use crate::measure::measure;
use crate::model::{DraftPage, End, PartId, PinId, Pt, Side, WireId};
use crate::route::{self, Jobs, Plan, Scope};
use crate::rules::{broken, rules_kept};

mod sketch;

use sketch::{CardSteps, Held, Sketch};

/// What Arrange and route plans for a page (§4.9): the parts' moves and the routes of the page as it would become.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Arranged {
    /// The parts that move, each by its move in ticks (whole grid steps), by ID.
    pub moves: Vec<(PartId, Pt)>,
    /// The parts that stand against a rule and stay where they stand (no room on the side their host's pin faces, or
    /// the page no better with them there), by ID, with that side.
    pub left: Vec<(PartId, Side)>,
    /// The routes, junctions and marks of the page as it would become (every wire with an end on a moved part
    /// included, an empty route for one without a way), and the page's quality before and after.
    pub plan: Plan,
    /// The strips of terminals over or under a card that could not open as one (§4.4, FT-2110), each by its first
    /// terminal, with what held it.
    pub held: Vec<(PartId, StripHeld)>,
}

/// What held a strip of terminals over or under a card from opening as one to the channels' columns (§4.4).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StripHeld {
    /// Its terminals are wired to the card's channels in another order than they stand in.
    Order,
    /// The channels' columns leave too little room for the terminals that stand between them.
    Room,
    /// This part stands where a terminal, or what moves with it, would go.
    Part(PartId),
    /// A terminal, or what moves with it, would leave the drawing area.
    Sheet,
    /// It opens, but the page is no better with it so (its wires in the strip's order cross more than they do now).
    Worse,
}

impl Arranged {
    /// Whether the plan changes nothing.
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty() && self.plan.is_empty()
    }
}

/// The plan of Arrange and route over `scope` of `page` with the parts and wires named in `fixed` kept as they stand
/// (the rows unticked in the preview: a wire kept keeps its route and the parts at its ends; module documentation).
/// Deterministic: the same page in any order of its lists gives the same plan.
pub fn arrange(page: &DraftPage, scope: Scope<'_>, fixed: &[ew_ids::Uuid]) -> Arranged {
    // Planned again on the page each plan leaves, until a run changes nothing (at most `ROUNDS` runs), so that a second
    // press of the button finds nothing to do (§4.8).
    let first = arrange_once(page, scope, fixed);
    if first.is_empty() {
        return first;
    }
    let mut now = applied_all(page, &first);
    let mut moves: BTreeMap<PartId, Pt> = first.moves.iter().copied().collect();
    let mut left = first.left.clone();
    // (The wires without a way and the notes are those of the last run that changed the page.)
    let (mut unrouted, mut notes) = (first.plan.unrouted.clone(), first.plan.notes.clone());
    for _ in 1..ROUNDS {
        let next = arrange_once(&now, scope, fixed);
        if next.is_empty() {
            break;
        }
        for (id, d) in &next.moves {
            let at = moves.entry(*id).or_default();
            *at = at.plus(*d);
        }
        left = next.left.clone();
        unrouted.clone_from(&next.plan.unrouted);
        notes.clone_from(&next.plan.notes);
        now = applied_all(&now, &next);
    }
    moves.retain(|_, d| *d != Pt::default());
    let moves: Vec<(PartId, Pt)> = moves.into_iter().collect();
    // The plan from the page to where the runs left it.
    let mut plan = Plan::default();
    for w in &now.wires {
        if page
            .wires
            .iter()
            .find(|x| x.id == w.id)
            .is_some_and(|o| o.route != w.route)
        {
            plan.routes.push((w.id, w.route.clone()));
        }
    }
    for j in &now.junctions {
        if page
            .junctions
            .iter()
            .find(|x| x.id == j.id)
            .is_some_and(|o| o.at != j.at)
        {
            plan.junctions.push((j.id, j.at));
        }
    }
    for m in &now.marks {
        if let Some(o) = page.marks.iter().find(|x| x.id == m.id)
            && let (Some(a), Some(b)) = (mark_point(o), mark_point(m))
            && a != b
        {
            plan.marks.push((m.id, b.minus(a)));
        }
    }
    // (In the order of the IDs, whatever the order of the page's lists.)
    plan.routes.sort_by_key(|(w, _)| *w);
    plan.junctions.sort_by_key(|(j, _)| *j);
    plan.marks.sort_by_key(|(m, _)| *m);
    plan.unrouted = unrouted;
    plan.notes = notes;
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
    plan.before = first.plan.before.clone();
    plan.after = measure(&now);
    Arranged {
        held: first.held.clone(),
        left: left
            .into_iter()
            .filter(|(id, _)| !moves.iter().any(|(m, _)| m == id))
            .collect(),
        moves,
        plan,
    }
}

/// `page` as `a` leaves it: its parts moved (with their texts the program may not place; a wire of a moved part the
/// plan leaves without a way keeps no route), its routes, junctions and marks.
fn applied_all(page: &DraftPage, a: &Arranged) -> DraftPage {
    let (moved, _) = moved(page, &a.moves);
    applied(&moved, &a.plan)
}

/// A plan tried and taken: what it is compared by (faults, unrouted wires, the cost with the price of its moves), each
/// unit's move, the parts' moves, Route's plan of the page moved and the page's quality after it.
struct Taken {
    score: (usize, usize, i64),
    d: Vec<Pt>,
    moves: Vec<(PartId, Pt)>,
    plan: Plan,
    quality: crate::Quality,
}

/// One run of Arrange and route (module documentation). Where a card would step along its edge ([`CardSteps`]) the
/// arrangement is made three ways: with that step, with the step that clears the supply alone, and without it; where
/// a strip's other terminals held a channel's terminal off its column, each of them again with the strip opened as one
/// (`Sketch::open_strips`); the best page by the measure with the price of its moves is offered.
fn arrange_once(page: &DraftPage, scope: Scope<'_>, fixed: &[ew_ids::Uuid]) -> Arranged {
    let (stepped, cards, bound, _) = arrange_try(page, scope, fixed, CardSteps::Columns, false);
    if !cards && !bound {
        return stepped;
    }
    let score = |a: &Arranged| {
        let q = &a.plan.after;
        let price = MOVE_PRICE * i64::from(TICKS_PER_STEP) * a.moves.len() as i64;
        (q.faults, q.unrouted, q.cost + price)
    };
    let steps: &[CardSteps] = if cards {
        &[CardSteps::Columns, CardSteps::Supply, CardSteps::None]
    } else {
        &[CardSteps::Columns]
    };
    let mut best = stepped;
    let mut held: Vec<(PartId, StripHeld)> = Vec::new();
    let mut opened: BTreeSet<PartId> = BTreeSet::new();
    for (k, steps) in steps.iter().enumerate() {
        for open in [false, true] {
            if (k == 0 && !open) || (open && !bound) {
                continue;
            }
            let (other, _, _, strips) = arrange_try(page, scope, fixed, *steps, open);
            held.extend(other.held.iter().copied());
            opened.extend(strips);
            if score(&other) < score(&best) {
                best = other;
            }
        }
    }
    // A strip that opened in none of the ways is named with what held it the first time; one that opened, where no
    // plan is offered, with the judge's word: the page was no better with it open.
    let worse: Vec<(PartId, StripHeld)> = if best.moves.is_empty() {
        opened.iter().map(|first| (*first, StripHeld::Worse)).collect()
    } else {
        Vec::new()
    };
    let mut named: BTreeSet<PartId> = opened;
    held.retain(|(first, _)| named.insert(*first));
    held.extend(worse);
    best.held = held;
    best
}

/// One arrangement routed and judged, with the strip opened as one where `open`; whether a card of the page would take
/// a step along its edge, whether a strip's other terminals held a channel's terminal off its column, and the strips
/// that opened (each by its first terminal).
fn arrange_try(
    page: &DraftPage,
    scope: Scope<'_>,
    fixed: &[ew_ids::Uuid],
    cards: CardSteps,
    open: bool,
) -> (Arranged, bool, bool, Vec<PartId>) {
    let before = measure(page);
    // Route over the same wires (every wire of the scope, whoever shaped it): the plan to beat.
    let routed_only = route::route(page, &jobs(page, scope, fixed));
    let baseline = routed_only.after.clone();
    let mut sk = Sketch::new(page, scope, fixed);
    sk.card_steps = cards;
    sk.open = open;
    let stepping = sk.cards_step();
    sk.lay();
    // (Where the rows set a row of terminals out of its order, the arrangement with the strip opened is worth trying
    // too: it keeps the order.)
    let bound = sk.bound || !sk.order_kept();
    let opened = sk.opened();
    let held: Vec<(PartId, StripHeld)> = sk
        .held
        .iter()
        .map(|(u, why)| {
            let why = match why {
                Held::Order => StripHeld::Order,
                Held::Room => StripHeld::Room,
                Held::Unit(o) => StripHeld::Part(sk.largest(*o)),
                Held::Sheet => StripHeld::Sheet,
            };
            (sk.largest(*u), why)
        })
        .collect();
    // The rules read the parts and the nets alone: the parts a broken rule names go back, with what hangs on them,
    // until none breaks.
    for _ in 0..sk.len() {
        let moves = sk.moves();
        if moves.is_empty() {
            break;
        }
        let Err(breaks) = rules_kept(page, &moved(page, &moves).0) else {
            break;
        };
        let named: BTreeSet<PartId> = breaks.iter().flat_map(|b| b.parts.iter().copied()).collect();
        let units: BTreeSet<usize> = named.iter().filter_map(|id| sk.unit_of_id(*id)).collect();
        // The named units that moved go back; where none of them moved, what hangs on them does.
        let own: Vec<usize> = units
            .iter()
            .copied()
            .filter(|u| sk.d[*u] != Pt::default())
            .collect();
        let back = if own.is_empty() {
            units.iter().copied().collect()
        } else {
            own
        };
        let mut any = false;
        // (A strip opened as one goes back as one.)
        for u in sk.with_strips(back) {
            for f in sk.subtree(u) {
                if sk.d[f] != Pt::default() {
                    sk.d[f] = Pt::default();
                    any = true;
                }
            }
        }
        if !any {
            break;
        }
    }
    let full = sk.d.clone();
    let mut seen: BTreeSet<Vec<(PartId, Pt)>> = BTreeSet::new();
    // A plan routed and judged (module documentation, 4).
    let mut judged = |sk: &mut Sketch<'_>, d: &[Pt]| -> Option<Taken> {
        sk.d = d.to_vec();
        let moves = sk.moves();
        if moves.is_empty() || !seen.insert(moves.clone()) {
            return None;
        }
        // A row of terminals keeps the order it stands in: no plan sets one terminal past another (§4.4).
        if !sk.order_kept() {
            return None;
        }
        if rules_kept(page, &moved(page, &moves).0).is_err() {
            return None;
        }
        let (plan, after) = routed(page, &moves, scope, fixed);
        if rules_kept(page, &after).is_err() {
            return None;
        }
        let quality = measure(&after);
        let (mut bad, texts) = route::worse(page, &before, &quality);
        if quality.unrouted < baseline.unrouted {
            // A plan that routes a wire Route's plan leaves without a way may lay another across a text (C4), as it may
            // cross more wires: the dashed wire weighs more (§3.8). A new fault still holds it back.
            let faulty = route::wires_with_faults(&quality);
            bad.retain(|w| faulty.contains(w));
        }
        let price = MOVE_PRICE * i64::from(TICKS_PER_STEP) * moves.len() as i64;
        let score = (quality.faults, quality.unrouted, quality.cost + price);
        let crossed =
            quality.unrouted >= baseline.unrouted && quality.counts.crossings > baseline.counts.crossings;
        if !bad.is_empty() || texts || quality.faults > baseline.faults || crossed || score >= baseline.key()
        {
            return None;
        }
        Some(Taken {
            score,
            d: d.to_vec(),
            moves,
            plan,
            quality,
        })
    };
    let without = |d: &[Pt], units: &[usize]| -> Vec<Pt> {
        let mut d = d.to_vec();
        for u in units {
            d[*u] = Pt::default();
        }
        d
    };
    let only = |d: &[Pt], units: &[usize]| -> Vec<Pt> {
        (0..d.len())
            .map(|u| if units.contains(&u) { d[u] } else { Pt::default() })
            .collect()
    };
    let mut best = judged(&mut sk, &full);
    if best.is_none() {
        // Tree by tree: each kept when the page gets better with it.
        sk.d.clone_from(&full);
        let groups: Vec<Vec<usize>> = sk.groups().into_iter().take(BRANCHES).collect();
        if groups.len() > 1 {
            for g in &groups {
                let mut d = best
                    .as_ref()
                    .map_or_else(|| vec![Pt::default(); full.len()], |b| b.d.clone());
                for u in g {
                    d[*u] = full[*u];
                }
                if let Some(t) = judged(&mut sk, &d)
                    && best.as_ref().is_none_or(|b| t.score < b.score)
                {
                    best = Some(t);
                }
            }
        }
    }
    if best.is_none() {
        // Without one branch, then one branch alone.
        sk.d.clone_from(&full);
        let branches: Vec<Vec<usize>> = sk.branches().into_iter().take(BRANCHES).collect();
        for b in &branches {
            if let Some(t) = judged(&mut sk, &without(&full, b))
                && best.as_ref().is_none_or(|x| t.score < x.score)
            {
                best = Some(t);
            }
        }
        if best.is_none() {
            for b in &branches {
                if let Some(t) = judged(&mut sk, &only(&full, b))
                    && best.as_ref().is_none_or(|x| t.score < x.score)
                {
                    best = Some(t);
                }
            }
        }
    }
    // The price, for real: a branch whose move does not pay goes back.
    if let Some(taken) = &best {
        sk.d.clone_from(&taken.d);
        let branches: Vec<Vec<usize>> = sk.branches().into_iter().take(BRANCHES).collect();
        for b in &branches {
            let Some(now) = &best else { break };
            let d = without(&now.d, b);
            if d == now.d {
                continue;
            }
            // Not where the move set a rule right.
            sk.d.clone_from(&d);
            let (was, then) = (
                broken(&moved(page, &now.moves).0),
                broken(&moved(page, &sk.moves()).0),
            );
            if then.iter().any(|x| !was.contains(x)) {
                continue;
            }
            if let Some(t) = judged(&mut sk, &d)
                && t.score < now.score
            {
                best = Some(t);
            }
        }
    }
    let left = |moves: &[(PartId, Pt)]| -> Vec<(PartId, Side)> {
        // The parts that stood against a rule, stand against it still and did not move: named with the side their
        // host's pin faces.
        let (was, then) = (broken(page), broken(&moved(page, moves).0));
        let mut out: BTreeSet<(PartId, Side)> = BTreeSet::new();
        for b in then.iter().filter(|b| was.contains(b)) {
            for id in &b.parts {
                if moves.iter().any(|(m, _)| m == id) {
                    continue;
                }
                let side = sk
                    .unit_of_id(*id)
                    .and_then(|u| sk.lead[u])
                    .and_then(|l| sk.side(l.h));
                if let Some(s) = side
                    && sk.unit_of_id(*id).is_some_and(|u| !sk.fixed(u))
                {
                    out.insert((*id, s));
                }
            }
        }
        out.into_iter().collect()
    };
    let Some(taken) = best else {
        // Without a better plan: Route's plan, and the parts that stay against a rule named all the same.
        return (
            Arranged {
                plan: Plan {
                    before,
                    after: baseline,
                    ..routed_only
                },
                left: left(&[]),
                moves: Vec::new(),
                held,
            },
            stepping,
            bound,
            opened,
        );
    };
    (
        Arranged {
            left: left(&taken.moves),
            plan: Plan {
                before,
                after: taken.quality,
                ..taken.plan
            },
            moves: taken.moves,
            held,
        },
        stepping,
        bound,
        opened,
    )
}

/// The jobs of Arrange and route (§3.2): every wire of the scope, whoever shaped it (the page's, or the selection's
/// and every wire with an end on a part of it), but the wires kept (`fixed`).
fn jobs(page: &DraftPage, scope: Scope<'_>, fixed: &[ew_ids::Uuid]) -> Jobs {
    let mut jobs = match scope {
        Scope::Page => Jobs {
            wires: page.wires.iter().map(|w| w.id).collect(),
            ..Jobs::default()
        },
        Scope::Selection(_) => route::jobs(page, scope),
    };
    jobs.wires.retain(|w| !fixed.contains(&w.0));
    jobs.wires.sort();
    jobs.his.clear();
    jobs
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
        if w.ends
            .iter()
            .any(|e| matches!(e, End::Pin(p) if pins.contains(p)))
        {
            w.route.clear();
            touched.push(w.id);
        }
    }
    for (j, d) in junctions_follow(page, &by) {
        if let Some(x) = moved.junctions.iter_mut().find(|x| x.id == j) {
            x.at = x.at.plus(d);
        }
    }
    (moved, touched)
}

/// The junctions of `page` that follow the parts moved by `by`, each with its step: a junction that stood in line with
/// a pin it is wired to, on the line that pin's wire leaves along (a drop), keeps that line; one in line with such a
/// junction through a wire between them follows it. Of several lines it stood on it keeps the one most of them take,
/// and stays where they pull apart evenly.
fn junctions_follow(page: &DraftPage, by: &BTreeMap<PartId, Pt>) -> Vec<(crate::model::JunctionId, Pt)> {
    use crate::model::JunctionId;
    let mut pins: BTreeMap<PinId, (Pt, Option<Side>, Pt)> = BTreeMap::new();
    for part in &page.parts {
        let d = by.get(&part.id).copied().unwrap_or_default();
        for pin in &part.pins {
            pins.insert(pin.id, (part.at.plus(pin.at), pin.side, d));
        }
    }
    let at: BTreeMap<JunctionId, Pt> = page.junctions.iter().map(|j| (j.id, j.at)).collect();
    let mut links: BTreeMap<JunctionId, Vec<End>> = BTreeMap::new();
    for w in &page.wires {
        for (a, b) in [(w.ends[0], w.ends[1]), (w.ends[1], w.ends[0])] {
            if let End::Junction(j) = a {
                links.entry(j).or_default().push(b);
            }
        }
    }
    for ends in links.values_mut() {
        ends.sort();
        ends.dedup();
    }
    // The step most votes ask for; none where they are even.
    let most = |votes: &[i32]| -> i32 {
        let mut count: BTreeMap<i32, usize> = BTreeMap::new();
        for v in votes {
            *count.entry(*v).or_default() += 1;
        }
        let top = count.values().copied().max().unwrap_or(0);
        let mut firsts = count.iter().filter(|(_, k)| **k == top).map(|(v, _)| *v);
        match (firsts.next(), firsts.next()) {
            (Some(v), None) => v,
            _ => 0,
        }
    };
    let mut step: BTreeMap<JunctionId, Pt> = BTreeMap::new();
    // From the pins outward, a junction at a time, as far as a chain of junctions reaches.
    for _ in 0..=at.len() {
        let mut changed = false;
        for (j, p) in &at {
            let (mut xs, mut ys): (Vec<i32>, Vec<i32>) = (Vec::new(), Vec::new());
            for e in links.get(j).into_iter().flatten() {
                match e {
                    End::Pin(pin) => {
                        if let Some((q, Some(s), d)) = pins.get(pin) {
                            if s.vertical() && q.x == p.x {
                                xs.push(d.x);
                            } else if !s.vertical() && q.y == p.y {
                                ys.push(d.y);
                            }
                        }
                    }
                    End::Junction(k) => {
                        if let (Some(q), Some(d)) = (at.get(k), step.get(k)) {
                            if q.x == p.x {
                                xs.push(d.x);
                            } else if q.y == p.y {
                                ys.push(d.y);
                            }
                        }
                    }
                }
            }
            let new = Pt::new(most(&xs), most(&ys));
            if step.get(j).copied().unwrap_or_default() != new {
                step.insert(*j, new);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    step.into_iter().filter(|(_, d)| *d != Pt::default()).collect()
}

/// Route's plan of the page with every part of `moves` moved ([`moved`]) for the wires of the moved parts and the jobs
/// of `scope`; and the page as the plan leaves it.
fn routed(
    page: &DraftPage,
    moves: &[(PartId, Pt)],
    scope: Scope<'_>,
    fixed: &[ew_ids::Uuid],
) -> (Plan, DraftPage) {
    let (moved, touched) = moved(page, moves);
    let mut jobs: Jobs = jobs(&moved, scope, fixed);
    jobs.wires.extend(touched);
    jobs.wires.sort();
    jobs.wires.dedup();
    let plan = route::route(&moved, &jobs);
    let after = applied(&moved, &plan);
    (plan, after)
}

/// `page` with `plan`'s routes, junctions and marks, its marks drawn where the routes put them (§3.10).
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
            move_mark(x, *d);
        }
    }
    draw_marks(&mut out);
    out
}
