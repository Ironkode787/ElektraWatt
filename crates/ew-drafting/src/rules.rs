// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rules that bind the placer (docs/design/06a-drafting.md §1.6): [`rules_kept`] checks a plan's page against
//! the page it starts from. Each rule is a small function that names the parts that break it.
//!
//! # How a rule reads a page
//!
//! A **pair** is a net (wires joined through junctions or ending at one pin) with exactly two pins, on two parts; a
//! net with more pins is a **trunk** net. A pin's side is the side its wire leaves on.
//!
//! - **A1.** A part with pins (or a drawn box) outside the drawing area or on another part (F7 of the measure).
//! - **A2.** A part kept in place (`locked`: kept, outside the scope, inside a structure box) that moved.
//! - **A3.** A part turned, mirrored or reshaped (its body or a pin moved against its origin), or gone.
//! - **A4.** The partners that face the pins of one line (the pins of one part on one side, or of parts of one size
//!   whose pins stand on one line: the terminals of a strip) out of the order of those pins across that side. Facing
//!   pairs only: wires that turn a corner nest, and wires that turn back lie inside one another, the other way round.
//! - **A5.** A pair of facing pins, the second at least two steps beyond the first, closed up (nearer than it stood
//!   and nearer than the chain gap) or spread (further than it stood, or than the chain gap, by more than
//!   [`ROW_PULL`]: what a row's common line may take); not a pair with a part that stood against another rule (it had
//!   to move).
//! - **E1.** A pair from a pin that faces down to a pin that faces up whose first pin is not above the second.
//! - **E2.** A part of a circuit above the rail that feeds it (or below the one it returns to): a trunk net with two
//!   junctions or more on one horizontal line and whose pins all face that line from one side, its circuits the parts
//!   reached from its pins through pairs; and any part but a mark above the highest feeding rail or below the lowest
//!   returning one.
//! - **E3.** An incoming mark not at the left end of its net (a pin or junction of the net further left), an outgoing
//!   one not at the right end.
//! - **E4.** A card's partner not beyond the card's pin along its side, or two partners of one edge of a card out of
//!   the order of their channels. Not a partner whose own pin faces the same way as the card's: its wire turns back,
//!   and it has no place in the channel's column.
//! - **E5.** An earth symbol further than [`EARTH_REACH`] (city-block) from the nearest pin of its net.
//! - **F1.** A pressure source or service unit not lower than every valve; an actuator not higher than a valve it is
//!   reached from through tubes and parts that are neither valves nor sources.
//! - **F2.** Two actuators whose order from left to right changed.
//! - **B1.** Two stations of a bus line out of the line's order from left to right (the line read from its
//!   controller, else from its leftmost station).
//! - **B2.** Two stations of a pair whose ports face the same side not level.
//!
//! [`rules_kept`] holds when no rule is broken in the plan's page but where it was broken before by the same parts
//! and none of them moved: a part that stood against a rule and has no room is left where it stands, and named.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{CHAIN_GAP, CHAIN_GAP_MIN, EARTH_REACH, ROW_PULL};
use crate::measure::parts_on_parts;
use crate::model::{DraftPage, End, JunctionId, PageKind, Part, PartId, PinId, Pt, Role, Side, Tick, WireId};

/// A rule of §1.6.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Rule {
    /// Every part inside the drawing area; no part on a part.
    A1,
    /// A part kept in place, outside the scope or inside a structure box does not move.
    A2,
    /// No part is turned, mirrored or reshaped, and none leaves its page.
    A3,
    /// The parts of a row stand in the order of the pins they are wired to.
    A4,
    /// The spacing along a circuit is the person's.
    A5,
    /// Current runs down a circuit.
    E1,
    /// Potentials at the head and the foot.
    E2,
    /// A potential enters at the left and leaves at the right.
    E3,
    /// A card's channels in their order, each channel's parts on the side its pin faces.
    E4,
    /// An earth symbol beside the pin it earths.
    E5,
    /// The supply below the valves, each actuator above its valve.
    F1,
    /// The actuators keep their order.
    F2,
    /// The stations of a line in the order of the line.
    B1,
    /// Stations with their ports on the same side stand level.
    B2,
}

/// A rule broken, and the parts that break it, by ID.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Broken {
    /// The rule.
    pub rule: Rule,
    /// The parts.
    pub parts: Vec<PartId>,
}

/// A net of the page: its pins, junctions and wires, each by ID.
#[derive(Clone, Default, Debug)]
pub(crate) struct Net {
    pub pins: Vec<PinId>,
    pub junctions: Vec<JunctionId>,
    pub wires: Vec<WireId>,
}

/// The page's parts, pins and nets as the rules and the placer read them.
pub(crate) struct Wiring<'a> {
    pub page: &'a DraftPage,
    /// The parts, by ID.
    pub parts: Vec<&'a Part>,
    /// Each pin: its part's index, its point on the page, its side.
    pub pins: BTreeMap<PinId, (usize, Pt, Option<Side>)>,
    /// The nets, each with at least one wire, in the order of their first wire's ID.
    pub nets: Vec<Net>,
}

impl<'a> Wiring<'a> {
    pub fn new(page: &'a DraftPage) -> Wiring<'a> {
        let mut parts: Vec<&Part> = page.parts.iter().collect();
        parts.sort_by_key(|p| p.id);
        let mut pins = BTreeMap::new();
        for (i, p) in parts.iter().enumerate() {
            for pin in &p.pins {
                pins.insert(pin.id, (i, p.at.plus(pin.at), pin.side));
            }
        }
        // Union-find over the wires' ends.
        let mut wires: Vec<_> = page.wires.iter().collect();
        wires.sort_by_key(|w| w.id);
        let mut keys: BTreeMap<End, usize> = BTreeMap::new();
        for w in &wires {
            for e in w.ends {
                let n = keys.len();
                keys.entry(e).or_insert(n);
            }
        }
        let mut up: Vec<usize> = (0..keys.len()).collect();
        fn root(up: &mut [usize], mut i: usize) -> usize {
            while up[i] != i {
                up[i] = up[up[i]];
                i = up[i];
            }
            i
        }
        for w in &wires {
            let (a, b) = (keys[&w.ends[0]], keys[&w.ends[1]]);
            let (ra, rb) = (root(&mut up, a), root(&mut up, b));
            if ra != rb {
                up[ra.max(rb)] = ra.min(rb);
            }
        }
        let mut by_root: BTreeMap<usize, Net> = BTreeMap::new();
        for w in &wires {
            let r = root(&mut up, keys[&w.ends[0]]);
            by_root.entry(r).or_default().wires.push(w.id);
        }
        for (e, k) in &keys {
            let r = root(&mut up, *k);
            let net = by_root.entry(r).or_default();
            match e {
                End::Pin(p) if pins.contains_key(p) => net.pins.push(*p),
                End::Junction(j) => net.junctions.push(*j),
                End::Pin(_) => {}
            }
        }
        let mut nets: Vec<Net> = by_root.into_values().collect();
        nets.sort_by_key(|n| n.wires.first().copied());
        Wiring {
            page,
            parts,
            pins,
            nets,
        }
    }

    /// The nets of exactly two pins on two parts: (pin, pin) in ID order.
    pub fn pairs(&self) -> Vec<(PinId, PinId)> {
        self.nets
            .iter()
            .filter_map(|n| match n.pins.as_slice() {
                [a, b] if self.pins[a].0 != self.pins[b].0 => Some((*a, *b)),
                _ => None,
            })
            .collect()
    }

    pub fn part_id(&self, i: usize) -> PartId {
        self.parts[i].id
    }

    /// The horizontal line most of a net's junctions lie on, two at least (the upper of two such): its rail's line.
    pub fn line(&self, n: &Net) -> Option<Tick> {
        let mut ys: BTreeMap<Tick, usize> = BTreeMap::new();
        for p in n.junctions.iter().filter_map(|j| self.junction(*j)) {
            *ys.entry(p.y).or_default() += 1;
        }
        let (line, count) = ys.into_iter().max_by_key(|(y, k)| (*k, -*y))?;
        (count >= 2).then_some(line)
    }

    fn junction(&self, j: JunctionId) -> Option<Pt> {
        self.page.junctions.iter().find(|x| x.id == j).map(|x| x.at)
    }
}

/// A partner of a part's pin on one side: the pin's coordinate across that side, the partner pin's, the partner.
type Partner = (Tick, Tick, usize);

/// The opposite side.
pub(crate) fn opposite(s: Side) -> Side {
    match s {
        Side::Up => Side::Down,
        Side::Down => Side::Up,
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

/// How far `p` lies beyond `from` along `s`.
pub(crate) fn beyond(from: Pt, p: Pt, s: Side) -> Tick {
    let u = s.unit();
    (p.x - from.x) * u.x + (p.y - from.y) * u.y
}

/// The coordinate of `p` across `s` (x for a vertical side, y for a horizontal one).
pub(crate) fn across(p: Pt, s: Side) -> Tick {
    if s.vertical() { p.x } else { p.y }
}

/// Whether the plan's page `after` keeps the rules of §1.6 against the page `before` it starts from (module
/// documentation); else the rules it breaks, each with the parts that break it, in rule order.
pub fn rules_kept(before: &DraftPage, after: &DraftPage) -> Result<(), Vec<Broken>> {
    let moved: BTreeSet<PartId> = after
        .parts
        .iter()
        .filter(|p| {
            before
                .parts
                .iter()
                .find(|b| b.id == p.id)
                .is_none_or(|b| b.at != p.at)
        })
        .map(|p| p.id)
        .collect();
    let (wb, wa) = (Wiring::new(before), Wiring::new(after));
    let (sb, sa) = (states(&wb), states(&wa));
    // A part that stood against a rule had to move: the spacing of its pairs is not the person's to keep (A5).
    let misplaced: BTreeSet<PartId> = sb.iter().flat_map(|b| b.parts.iter().copied()).collect();
    let mut out: BTreeSet<Broken> = sa
        .into_iter()
        .filter(|b| !sb.contains(b) || b.parts.iter().any(|p| moved.contains(p)))
        .collect();
    out.extend(changes(&wb, &wa, &moved, &misplaced));
    if out.is_empty() {
        Ok(())
    } else {
        Err(out.into_iter().collect())
    }
}

/// Every rule broken on `page` that reads one page: A1, E1 to E5, F1, B2 (as the page's kind asks).
pub fn broken(page: &DraftPage) -> Vec<Broken> {
    states(&Wiring::new(page)).into_iter().collect()
}

fn broken_by(rule: Rule, mut parts: Vec<PartId>) -> Broken {
    parts.sort();
    parts.dedup();
    Broken { rule, parts }
}

fn states(w: &Wiring<'_>) -> BTreeSet<Broken> {
    let mut out = BTreeSet::new();
    for (p, o) in parts_on_parts(w.page) {
        out.insert(broken_by(Rule::A1, o.into_iter().chain([p]).collect()));
    }
    match w.page.kind {
        PageKind::Electrical => {
            out.extend(e1(w));
            out.extend(e2(w));
            out.extend(e3(w));
            out.extend(e4(w));
            out.extend(e5(w));
        }
        PageKind::Fluid => out.extend(f1(w)),
        PageKind::Bus => out.extend(b2(w)),
    }
    out
}

fn changes(
    b: &Wiring<'_>,
    a: &Wiring<'_>,
    moved: &BTreeSet<PartId>,
    misplaced: &BTreeSet<PartId>,
) -> Vec<Broken> {
    let mut out = Vec::new();
    for p in &b.parts {
        let Some(q) = a.parts.iter().find(|q| q.id == p.id) else {
            out.push(broken_by(Rule::A3, vec![p.id]));
            continue;
        };
        if p.locked && moved.contains(&p.id) {
            out.push(broken_by(Rule::A2, vec![p.id]));
        }
        if p.body != q.body || p.pins != q.pins || p.frame_only != q.frame_only {
            out.push(broken_by(Rule::A3, vec![p.id]));
        }
    }
    if a.parts.len() != b.parts.len() {
        for q in &a.parts {
            if !b.parts.iter().any(|p| p.id == q.id) {
                out.push(broken_by(Rule::A3, vec![q.id]));
            }
        }
    }
    out.extend(
        a4(a)
            .into_iter()
            .filter(|x| x.parts.iter().any(|p| moved.contains(p))),
    );
    out.extend(a5(b, a, moved, misplaced));
    match a.page.kind {
        PageKind::Fluid => out.extend(f2(b, a, moved)),
        PageKind::Bus => out.extend(
            b1(a)
                .into_iter()
                .filter(|x| x.parts.iter().any(|p| moved.contains(p))),
        ),
        PageKind::Electrical => {}
    }
    out
}

/// A4: per line of pins that face one side, of one part or of parts of one size (a card's edge, the terminals of a
/// strip), their partners' pins across that side in the order of the pins.
fn a4(w: &Wiring<'_>) -> Vec<Broken> {
    let mut rows: BTreeMap<((Tick, Tick), Side, Tick), Vec<Partner>> = BTreeMap::new();
    for (a, b) in w.pairs() {
        for (h, f) in [(a, b), (b, a)] {
            let ((hp, ha, hs), (fp, fa, fs)) = (w.pins[&h], w.pins[&f]);
            // A row is of parts that face their pins: a wire that turns a corner nests with its neighbours, and one
            // that turns back lies inside or outside them (the order of such parts is the other way round).
            if let Some(s) = hs
                && fs == Some(opposite(s))
            {
                let body = w.parts[hp].body;
                let line = if s.vertical() { ha.y } else { ha.x };
                rows.entry(((body.w(), body.h()), s, line)).or_default().push((
                    across(ha, s),
                    across(fa, s),
                    fp,
                ));
            }
        }
    }
    let mut out = Vec::new();
    for (_, mut row) in rows {
        row.sort();
        for i in 0..row.len() {
            for j in i + 1..row.len() {
                let (x, y) = (row[i], row[j]);
                if x.0 < y.0 && x.1 > y.1 && x.2 != y.2 {
                    out.push(broken_by(Rule::A4, vec![w.part_id(x.2), w.part_id(y.2)]));
                }
            }
        }
    }
    out
}

/// A5: the facing pairs closed up or spread, but a pair with a part that stood against a rule.
fn a5(b: &Wiring<'_>, a: &Wiring<'_>, moved: &BTreeSet<PartId>, misplaced: &BTreeSet<PartId>) -> Vec<Broken> {
    let mut out = Vec::new();
    for (p, q) in b.pairs() {
        let ((pi, pb, ps), (qi, qb, qs)) = (b.pins[&p], b.pins[&q]);
        let (Some(s), Some(t)) = (ps, qs) else { continue };
        if t != opposite(s) {
            continue;
        }
        let (Some(pa), Some(qa)) = (a.pins.get(&p), a.pins.get(&q)) else {
            continue;
        };
        let (before, after) = (beyond(pb, qb, s), beyond(pa.1, qa.1, s));
        let ids = [b.part_id(pi), b.part_id(qi)];
        // Closed up: nearer than the chain gap, or than it stood where it stood nearer. Spread: further than a row's
        // line takes a part beyond the chain gap, or beyond where it stood.
        let closed = after < before.min(CHAIN_GAP);
        let spread = after > before.max(CHAIN_GAP) + ROW_PULL;
        if before >= CHAIN_GAP_MIN
            && (closed || spread)
            && ids.iter().any(|x| moved.contains(x))
            && !ids.iter().any(|x| misplaced.contains(x))
        {
            out.push(broken_by(Rule::A5, ids.to_vec()));
        }
    }
    out
}

/// E1: of a pair from a pin facing down to one facing up, the first above the second.
fn e1(w: &Wiring<'_>) -> Vec<Broken> {
    let mut out = Vec::new();
    for (a, b) in w.pairs() {
        for (d, u) in [(a, b), (b, a)] {
            let ((di, dp, ds), (ui, up, us)) = (w.pins[&d], w.pins[&u]);
            if ds == Some(Side::Down) && us == Some(Side::Up) && dp.y >= up.y {
                out.push(broken_by(Rule::E1, vec![w.part_id(di), w.part_id(ui)]));
            }
        }
    }
    out
}

/// The rails of the page: (line, feeding, the parts of its circuits, the parts it taps): a trunk net with two junctions
/// or more on one horizontal line (the line most of them lie on) and whose pins (but marks') all face it from one side.
pub(crate) fn rails(w: &Wiring<'_>) -> Vec<(Tick, bool, BTreeSet<usize>, Vec<usize>)> {
    let pairs = w.pairs();
    let mut out = Vec::new();
    for n in &w.nets {
        let Some(line) = w.line(n) else {
            continue;
        };
        let taps: Vec<usize> = n
            .pins
            .iter()
            .filter(|p| !matches!(w.parts[w.pins[*p].0].role, Role::MarkIn | Role::MarkOut))
            .map(|p| w.pins[p].0)
            .collect();
        let sides: BTreeSet<Option<Side>> = n
            .pins
            .iter()
            .filter(|p| !matches!(w.parts[w.pins[*p].0].role, Role::MarkIn | Role::MarkOut))
            .map(|p| w.pins[p].2)
            .collect();
        let feeding = match sides.into_iter().collect::<Vec<_>>()[..] {
            [Some(Side::Up)] => true,
            [Some(Side::Down)] => false,
            _ => continue,
        };
        if taps.len() < 2 {
            continue;
        }
        // The circuits: the parts reached from the tapped parts through pairs.
        let mut circuit: BTreeSet<usize> = taps.iter().copied().collect();
        let mut todo: Vec<usize> = taps.clone();
        while let Some(i) = todo.pop() {
            for (a, b) in &pairs {
                let (pa, pb) = (w.pins[a].0, w.pins[b].0);
                for (x, y) in [(pa, pb), (pb, pa)] {
                    if x == i && !matches!(w.parts[y].role, Role::MarkIn | Role::MarkOut) && circuit.insert(y)
                    {
                        todo.push(y);
                    }
                }
            }
        }
        out.push((line, feeding, circuit, taps));
    }
    out
}

/// E2: no part of a circuit beyond its rail; no part but a mark above the highest feeding rail or below the lowest
/// returning one.
fn e2(w: &Wiring<'_>) -> Vec<Broken> {
    let mut out = Vec::new();
    let rails = rails(w);
    let mark = |i: usize| matches!(w.parts[i].role, Role::MarkIn | Role::MarkOut);
    for (line, feeding, circuit, _) in &rails {
        for i in circuit {
            let b = w.parts[*i].bounds();
            if (*feeding && b.y0 < *line) || (!*feeding && b.y1 > *line) {
                out.push(broken_by(Rule::E2, vec![w.part_id(*i)]));
            }
        }
    }
    let head = rails.iter().filter(|r| r.1).map(|r| r.0).min();
    let foot = rails.iter().filter(|r| !r.1).map(|r| r.0).max();
    for (i, p) in w.parts.iter().enumerate() {
        if mark(i) || p.frame_only || p.pins.is_empty() {
            continue;
        }
        let b = p.bounds();
        if head.is_some_and(|y| b.y0 < y) || foot.is_some_and(|y| b.y1 > y) {
            out.push(broken_by(Rule::E2, vec![p.id]));
        }
    }
    out
}

/// E3: an incoming mark at the left end of its net, an outgoing one at the right end.
fn e3(w: &Wiring<'_>) -> Vec<Broken> {
    let mut out = Vec::new();
    for n in &w.nets {
        let xs: Vec<(Tick, PinId)> = n.pins.iter().map(|p| (w.pins[p].1.x, *p)).collect();
        let js: Vec<Tick> = n
            .junctions
            .iter()
            .filter_map(|j| w.junction(*j))
            .map(|p| p.x)
            .collect();
        for (x, p) in &xs {
            let part = w.pins[p].0;
            let others = xs
                .iter()
                .filter(|(_, q)| q != p)
                .map(|(x, _)| *x)
                .chain(js.iter().copied());
            let bad = match w.parts[part].role {
                Role::MarkIn => others.into_iter().any(|o| o < *x),
                Role::MarkOut => others.into_iter().any(|o| o > *x),
                _ => false,
            };
            if bad {
                out.push(broken_by(Rule::E3, vec![w.part_id(part)]));
            }
        }
    }
    out
}

/// E4: a card's partners beyond its pins, in the order of its channels.
fn e4(w: &Wiring<'_>) -> Vec<Broken> {
    let mut out = Vec::new();
    let mut edges: BTreeMap<(usize, Side), Vec<Partner>> = BTreeMap::new();
    for (a, b) in w.pairs() {
        for (c, f) in [(a, b), (b, a)] {
            let ((ci, cp, cs), (fi, fp, fs)) = (w.pins[&c], w.pins[&f]);
            let (Some(s), Role::Card) = (cs, w.parts[ci].role) else {
                continue;
            };
            // A partner whose pin faces the way the card's does is wired out, across and back: it has no side of the
            // card to stand on, and its place in a row follows its neighbours.
            if fs == Some(s) {
                continue;
            }
            if beyond(cp, fp, s) <= 0 {
                out.push(broken_by(Rule::E4, vec![w.part_id(fi)]));
            }
            edges
                .entry((ci, s))
                .or_default()
                .push((across(cp, s), across(fp, s), fi));
        }
    }
    for (_, mut row) in edges {
        row.sort();
        for i in 0..row.len() {
            for j in i + 1..row.len() {
                if row[i].0 < row[j].0 && row[i].1 > row[j].1 {
                    out.push(broken_by(
                        Rule::E4,
                        vec![w.part_id(row[i].2), w.part_id(row[j].2)],
                    ));
                }
            }
        }
    }
    out
}

/// E5: an earth symbol within reach of the nearest other pin of its net.
fn e5(w: &Wiring<'_>) -> Vec<Broken> {
    let mut out = Vec::new();
    for n in &w.nets {
        for p in &n.pins {
            let (i, at, _) = w.pins[p];
            if w.parts[i].role != Role::Earth {
                continue;
            }
            let near = n
                .pins
                .iter()
                .filter(|q| w.pins[*q].0 != i)
                .map(|q| {
                    let o = w.pins[q].1;
                    (o.x - at.x).abs() + (o.y - at.y).abs()
                })
                .min();
            if near.is_some_and(|d| d > EARTH_REACH) {
                out.push(broken_by(Rule::E5, vec![w.part_id(i)]));
            }
        }
    }
    out
}

/// The parts of each role.
fn of_role(w: &Wiring<'_>, roles: &[Role]) -> Vec<usize> {
    (0..w.parts.len())
        .filter(|i| roles.contains(&w.parts[*i].role))
        .collect()
}

/// The actuators reached from valve `v` through tubes and parts that are neither valves nor sources.
fn driven(w: &Wiring<'_>, v: usize) -> BTreeSet<usize> {
    let mut seen: BTreeSet<usize> = BTreeSet::from([v]);
    let mut todo = vec![v];
    let mut out = BTreeSet::new();
    while let Some(i) = todo.pop() {
        for n in &w.nets {
            if !n.pins.iter().any(|p| w.pins[p].0 == i) {
                continue;
            }
            for p in &n.pins {
                let j = w.pins[p].0;
                if !seen.insert(j) {
                    continue;
                }
                match w.parts[j].role {
                    Role::Actuator => {
                        out.insert(j);
                    }
                    Role::Valve | Role::Source => {}
                    _ => todo.push(j),
                }
            }
        }
    }
    out
}

/// F1: the supply below every valve, every actuator above the valves it is reached from.
fn f1(w: &Wiring<'_>) -> Vec<Broken> {
    let mut out = Vec::new();
    let valves = of_role(w, &[Role::Valve]);
    for s in of_role(w, &[Role::Source]) {
        let b = w.parts[s].bounds();
        if valves.iter().any(|v| b.y0 < w.parts[*v].bounds().y1) {
            out.push(broken_by(Rule::F1, vec![w.part_id(s)]));
        }
    }
    for v in &valves {
        let top = w.parts[*v].bounds().y0;
        for a in driven(w, *v) {
            if w.parts[a].bounds().y1 > top {
                out.push(broken_by(Rule::F1, vec![w.part_id(a)]));
            }
        }
    }
    out
}

/// F2: the actuators' order from left to right kept.
fn f2(b: &Wiring<'_>, a: &Wiring<'_>, moved: &BTreeSet<PartId>) -> Vec<Broken> {
    let order = |w: &Wiring<'_>| -> Vec<PartId> {
        let mut xs: Vec<(Tick, PartId)> = of_role(w, &[Role::Actuator])
            .into_iter()
            .map(|i| (w.parts[i].bounds().centre().x, w.part_id(i)))
            .collect();
        xs.sort();
        xs.into_iter().map(|x| x.1).collect()
    };
    let (ob, oa) = (order(b), order(a));
    let mut out = Vec::new();
    for (i, x) in ob.iter().enumerate() {
        for y in &ob[i + 1..] {
            let (px, py) = (oa.iter().position(|p| p == x), oa.iter().position(|p| p == y));
            if px > py && (moved.contains(x) || moved.contains(y)) {
                out.push(broken_by(Rule::F2, vec![*x, *y]));
            }
        }
    }
    out
}

/// B1: the stations of each bus line in its order from left to right.
fn b1(w: &Wiring<'_>) -> Vec<Broken> {
    let station = |i: usize| matches!(w.parts[i].role, Role::Station | Role::Controller);
    // The line as a graph of stations through pairs.
    let mut next: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (a, b) in w.pairs() {
        let (i, j) = (w.pins[&a].0, w.pins[&b].0);
        if station(i) && station(j) {
            next.entry(i).or_default().insert(j);
            next.entry(j).or_default().insert(i);
        }
    }
    let mut out = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut starts: Vec<usize> = next.keys().copied().collect();
    starts.sort_by_key(|i| (w.parts[*i].role != Role::Controller, w.parts[*i].bounds().x0, *i));
    for s in starts {
        if seen.contains(&s) || next[&s].len() > 1 && w.parts[s].role != Role::Controller {
            continue;
        }
        let mut chain = vec![s];
        seen.insert(s);
        while let Some(n) = next[chain.last().unwrap_or(&s)]
            .iter()
            .find(|n| !seen.contains(n))
        {
            seen.insert(*n);
            chain.push(*n);
        }
        for pair in chain.windows(2) {
            if w.parts[pair[0]].bounds().x0 > w.parts[pair[1]].bounds().x0 {
                out.push(broken_by(Rule::B1, vec![w.part_id(pair[0]), w.part_id(pair[1])]));
            }
        }
    }
    out
}

/// B2: two stations of a pair whose ports face one side stand level.
fn b2(w: &Wiring<'_>) -> Vec<Broken> {
    let mut out = Vec::new();
    for (a, b) in w.pairs() {
        let ((i, pa, sa), (j, pb, sb)) = (w.pins[&a], w.pins[&b]);
        let stations = [i, j]
            .iter()
            .all(|x| matches!(w.parts[*x].role, Role::Station | Role::Controller));
        if let (true, Some(s), Some(t)) = (stations, sa, sb)
            && s == t
            && beyond(pa, pb, s) != 0
        {
            out.push(broken_by(Rule::B2, vec![w.part_id(i), w.part_id(j)]));
        }
    }
    out
}
