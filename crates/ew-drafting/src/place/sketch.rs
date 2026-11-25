// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The arrangement the placer proposes (docs/design/06a-drafting.md §4.2 to §4.7): [`Sketch`] reads a page's units and
//! relations, hangs the units in trees and sets each where its lead asks. It routes nothing and rates nothing:
//! [`super::arrange`] routes what it proposes and lets the measure judge. Each step of the method is on the function
//! that makes it.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::consts::{
    ANCHOR_SHARE, ANCHORS, AREA_PER_WEIGHT, CHAIN_GAP_MIN, HALF, LABEL_CLEAR, MATE_REACH, PIN_WEIGHT, REACH,
    RESETS, ROW_GAP, ROW_PULL, STUB, TICKS_PER_STEP, TRUNK_GAP,
};
use crate::labels::first_places;
use crate::model::{
    DraftPage, End, Groups, Owner, PageKind, PartId, PinId, Pt, Rect, Role, Side, Tick, grid_away,
};
use crate::route::Scope;
use crate::rules::Wiring;

mod followers;
mod strips;

/// How two related pins stand to each other, strongest first (§4.2).
///
/// A **relation** joins a pin of one unit to a pin of another, by strength:
///
/// 1. **facing**: the two pins of a net of two, on opposite sides: the best drawing is one straight wire. An end
///    symbol of one pin that is no mark (an earth symbol among them) keeps its hook: it has no line to take;
/// 2. **tap**: a pin of a net of three or more and the one pin of that net that faces the other way (a path's drop to
///    its branches, a return's rise from them), where the second already stands beyond the first (on a fluid page
///    wherever it stands: rule F1 tells the supply from the work). The branch nearer across than every other belongs
///    in line, the others keep their column. A unit in a row of three mates or more takes no tap: it stays in its row;
/// 3. **corner**: the two pins of a net of two, at a right angle: one bend;
/// 4. **level**: the two pins of a net of two on the same side: out, across and back;
/// 5. **mate**: no wire. Two units of one shape (the body of their largest part) that stand level side by side or in
///    line under one another, a step off at most, the nearest within twice [`ROW_PULL`]: a strip's terminals, cards
///    side by side, like devices in a row;
/// 6. **overruled**: a level pair of a unit more of whose mates face that host than are wired to it out and back; and
///    a facing pair or a tap of two units of one row that runs across the wire, where one of them would have to leave
///    the row for it (two cards side by side, wired from the upper edge of one to the lower edge of the other).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) enum Kind {
    Facing,
    Tap,
    Corner,
    Level,
    Mate,
    /// A level pair that most of the unit's row-mates overrule: they face the same host.
    Overruled,
}

/// How a card that is its tree's root steps along its edge ([`Sketch::card_step`]): clear of its supply's way and of
/// another card's columns, clear of its supply's way alone, or not at all. Which of them is best the judge finds.
///
/// A card that is the root of its tree steps along its edge first, clear of its supply's way and of another card's
/// columns ([`CardSteps`]): the judge tries the arrangement with that step, with a part of it and without it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CardSteps {
    Columns,
    Supply,
    None,
}

/// A relation between pin `pa` of unit `a` and pin `pb` of unit `b`.
#[derive(Clone, Copy, Debug)]
struct Rel {
    a: usize,
    pa: PinId,
    b: usize,
    pb: PinId,
    kind: Kind,
    /// The second belongs in line with the first (every facing pair; of a tap's branches the nearest).
    inline: bool,
}

/// A unit's lead: its host, the host's pin and its own.
#[derive(Clone, Copy, Debug)]
pub(super) struct Lead {
    pub host: usize,
    pub h: PinId,
    pub f: PinId,
    pub kind: Kind,
    pub inline: bool,
}

struct Unit {
    parts: Vec<usize>,
    fixed: bool,
    weight: i64,
}

/// What a unit stands on: its bodies with one step of wire out of each pin, and its texts.
struct Foot {
    bodies: Vec<Rect>,
    texts: Vec<Rect>,
}

impl Foot {
    fn meets(&self, o: &Foot) -> bool {
        let near = |a: &[Rect], b: &[Rect], gap: Tick| {
            a.iter()
                .any(|x| b.iter().any(|y| x.grown(gap).overlaps(y.grown(1))))
        };
        near(&self.bodies, &o.bodies, ROW_GAP - 2)
            || near(&self.bodies, &o.texts, LABEL_CLEAR)
            || near(&self.texts, &o.bodies, LABEL_CLEAR)
            || near(&self.texts, &o.texts, LABEL_CLEAR)
    }

    fn hull(&self) -> Option<Rect> {
        self.bodies.iter().chain(&self.texts).copied().reduce(Rect::union)
    }
}

/// A rail: the side its tapped pins face, those pins, its marks' pins and the line its junctions lie on (two at
/// least), as [`out`] measures along the side.
type Rail = (Side, Vec<PinId>, Vec<PinId>, Option<Tick>);

/// A block of a stack: its units, and its wires towards the rank it hangs on, each as (host pin, own pin).
type Block = (Vec<usize>, Vec<(PinId, PinId)>);

/// The arrangement being made: the page's units, their relations, the trees and each unit's place (§4.2 to §4.7).
pub(super) struct Sketch<'a> {
    pub w: Wiring<'a>,
    /// Each part's texts at their first places, relative to its origin.
    texts: Vec<Vec<Rect>>,
    unit_of: Vec<usize>,
    units: Vec<Unit>,
    /// Each unit's move.
    pub d: Vec<Pt>,
    pub lead: Vec<Option<Lead>>,
    /// The relations left out of the tree.
    extra: Vec<Rel>,
    /// The units whose place is set.
    placed: Vec<bool>,
    /// While a block is set by itself: the units that count as in its way.
    within: Option<BTreeSet<usize>>,
    /// The roots, heaviest first.
    roots: Vec<usize>,
    /// The units of the trees that are set for good.
    settled: BTreeSet<usize>,
    /// A unit found no room while the tree in hand was set.
    failed: bool,
    /// The pitch each stack was given (by its first block and its sides): asked of the text placer once.
    pitch: BTreeMap<(usize, Side, Side), Tick>,
    /// Whether each row (by its side and its members' places against their hosts, packed by their bodies) may stand
    /// so: asked of the text placer once.
    tight: BTreeMap<(Side, Vec<(usize, Pt)>), bool>,
    /// The pins a wire ends at.
    wired: BTreeSet<PinId>,
    /// The relations of the tree, by unit.
    edges: Vec<Vec<Rel>>,
    /// What the rails ask of their taps and marks ([`Sketch::rail_steps`]).
    steps: BTreeMap<usize, (Pt, bool)>,
    /// The rows the person made, by unit: the units joined through mates that stand side by side have one number in
    /// the first list, those that stand one under another one number in the second.
    strips: [Vec<usize>; 2],
    /// How a card that is its tree's root steps along its edge ([`Sketch::card_step`]).
    pub card_steps: CardSteps,
    /// A strip over or under a card opens as one ([`Sketch::open_strips`]): its other terminals bind none of the
    /// channels' terminals while the rows are set, and the strip is set anew afterwards.
    pub open: bool,
    /// Set by a run without `open` where a strip's other terminals held a channel's terminal off its column: the
    /// arrangement with the strip opened is worth trying.
    pub bound: bool,
    /// The strips opened as one, each by its units: a strip goes or stays as one ([`Sketch::with_strips`]).
    opened: Vec<Vec<usize>>,
    /// What the opening strips set anew ([`Sketch::loose`]).
    loose: BTreeSet<usize>,
    /// The strips that could not open, each by its first unit, with what held it ([`Sketch::open_strip`]).
    pub held: Vec<(usize, Held)>,
    /// The rows of terminals the person made, each in the order they stand ([`Sketch::order_kept`]).
    terminal_rows: Vec<(bool, Vec<usize>)>,
}

/// What holds a strip from opening as one ([`Sketch::open_strip`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Held {
    /// Its terminals are wired to the card's channels in another order than they stand in.
    Order,
    /// The channels' columns leave too little room for the terminals between them.
    Room,
    /// A unit stands where a terminal, or what moves with it, would go.
    Unit(usize),
    /// A terminal, or what moves with it, would leave the drawing area.
    Sheet,
}

/// `t` ticks along `s`.
fn along_vec(s: Side, t: Tick) -> Pt {
    let u = s.unit();
    Pt::new(u.x * t, u.y * t)
}

/// `t` ticks across `s`.
fn across_vec(s: Side, t: Tick) -> Pt {
    if s.vertical() {
        Pt::new(t, 0)
    } else {
        Pt::new(0, t)
    }
}

/// How far `p` lies out along `s` from the page's origin.
fn out(p: Pt, s: Side) -> Tick {
    s.beyond(Pt::default(), p)
}

/// The extent of `r` across `s`.
fn span_across(r: Rect, s: Side) -> (Tick, Tick) {
    if s.vertical() { (r.x0, r.x1) } else { (r.y0, r.y1) }
}

/// The extent of `r` along `s`, nearest first, as [`out`] measures.
fn span_along(r: Rect, s: Side) -> (Tick, Tick) {
    match s {
        Side::Up => (-r.y1, -r.y0),
        Side::Down => (r.y0, r.y1),
        Side::Left => (-r.x1, -r.x0),
        Side::Right => (r.x0, r.x1),
    }
}

fn kind_of(a: Option<Side>, b: Option<Side>) -> Option<Kind> {
    let (a, b) = (a?, b?);
    Some(if a == b {
        Kind::Level
    } else if a == b.opposite() {
        Kind::Facing
    } else {
        Kind::Corner
    })
}

/// The values of least total distance from `want` that never fall: neighbours out of order are pooled at their lower
/// median (isotonic regression in the sum of distances).
fn rising(want: &[Tick]) -> Vec<Tick> {
    let mut blocks: Vec<(Vec<Tick>, Tick)> = Vec::new();
    for w in want {
        blocks.push((vec![*w], *w));
        while blocks.len() >= 2 && blocks[blocks.len() - 2].1 > blocks[blocks.len() - 1].1 {
            let Some(last) = blocks.pop() else { break };
            let Some(prev) = blocks.last_mut() else { break };
            prev.0.extend(last.0);
            prev.0.sort_unstable();
            prev.1 = prev.0[(prev.0.len() - 1) / 2];
        }
    }
    blocks
        .into_iter()
        .flat_map(|(members, v)| std::iter::repeat_n(v, members.len()))
        .collect()
}

impl<'a> Sketch<'a> {
    /// The units and relations of `page` for a run over `scope` with `fixed` kept (§4.2).
    ///
    /// A **unit** moves as one: the parts of one `unit` number, the parts inside an outline drawn round other parts, and a
    /// part that stands with its pin on another part's pin (a mark set on a connection point). It is **fixed** when a part
    /// of it is kept in place, outside the scope, unticked, off the whole grid, or at the end of a kept wire.
    pub fn new(page: &'a DraftPage, scope: Scope<'_>, fixed: &[ew_ids::Uuid]) -> Sketch<'a> {
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
        let mut groups = Groups::new(n);
        // Units by number, the parts inside an outline drawn round other parts (but an actuator or a pressure source,
        // which is no part of a valve terminal: one set inside its outline is moved out as any other), and a
        // part on another's pin.
        let mut by_number: BTreeMap<u32, usize> = BTreeMap::new();
        for (i, p) in w.parts.iter().enumerate() {
            match by_number.get(&p.unit) {
                Some(&j) => {
                    groups.join(i, j);
                }
                None => {
                    by_number.insert(p.unit, i);
                }
            }
        }
        for (i, f) in w.parts.iter().enumerate() {
            if f.frame_only {
                for (j, p) in w.parts.iter().enumerate() {
                    let member = !matches!(p.role, Role::Actuator | Role::Source);
                    if i != j && member && f.bounds().holds(p.bounds()) {
                        groups.join(i, j);
                    }
                }
            }
        }
        let mut at_point: BTreeMap<Pt, Vec<usize>> = BTreeMap::new();
        for (i, at, _) in w.pins.values() {
            let here = at_point.entry(*at).or_default();
            if !here.contains(i) {
                here.push(*i);
            }
        }
        for here in at_point.values() {
            // A part of one pin set on another part's pin rides on it.
            for &i in here {
                if w.parts[i].pins.len() == 1 {
                    for &j in here {
                        if j != i {
                            groups.join(i, j);
                        }
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
        for (i, slot) in unit_of.iter_mut().enumerate() {
            let r = groups.root(i);
            let u = *roots.entry(r).or_insert_with(|| {
                units.push(Unit {
                    parts: Vec::new(),
                    fixed: false,
                    weight: 0,
                });
                units.len() - 1
            });
            *slot = u;
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
            unit.weight += PIN_WEIGHT * pins + area / AREA_PER_WEIGHT;
        }
        let m = units.len();
        let mut texts: Vec<Vec<Rect>> = vec![Vec::new(); n];
        for (id, rect) in first_places(page) {
            let Some(l) = page.labels.iter().find(|l| l.id == id) else {
                continue;
            };
            if let Owner::Part(p) = l.owner
                && let Some(i) = w.parts.iter().position(|x| x.id == p)
            {
                texts[i].push(rect.moved(Pt::new(-w.parts[i].at.x, -w.parts[i].at.y)));
            }
        }
        for t in &page.fixed_texts {
            if let Some(i) = w.parts.iter().position(|x| x.id.0 == t.owner) {
                texts[i].push(t.rect.moved(Pt::new(-w.parts[i].at.x, -w.parts[i].at.y)));
            }
        }
        let mut sketch = Sketch {
            w,
            texts,
            unit_of,
            units,
            d: vec![Pt::default(); m],
            lead: vec![None; m],
            extra: Vec::new(),
            placed: vec![false; m],
            within: None,
            roots: Vec::new(),
            settled: BTreeSet::new(),
            failed: false,
            pitch: BTreeMap::new(),
            tight: BTreeMap::new(),
            wired,
            edges: Vec::new(),
            steps: BTreeMap::new(),
            strips: [Vec::new(), Vec::new()],
            card_steps: CardSteps::Columns,
            open: false,
            bound: false,
            opened: Vec::new(),
            loose: BTreeSet::new(),
            held: Vec::new(),
            terminal_rows: Vec::new(),
        };
        sketch.tree();
        sketch.terminal_rows = sketch.terminal_rows();
        sketch
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn fixed(&self, u: usize) -> bool {
        self.units[u].fixed
    }

    /// What stands in the place unit `u`, left against a rule, would take on the side its host's pin faces (§4.7): in
    /// line with that pin, with what hangs on it, [`CHAIN_GAP_MIN`] beyond the pin at least and clear of the host.
    /// `Some(Some(o))`: it lies on unit `o`; `Some(None)`: it leaves the drawing area; `None`: it is clear.
    pub fn in_place(&self, u: usize) -> Option<Option<usize>> {
        let l = self.lead[u]?;
        let s = self.side(l.h)?;
        let (h, f) = (self.pt(l.h), self.pt(l.f));
        let tree = self.subtree(u);
        let meets = |a: &[Rect], b: &[Rect]| a.iter().any(|x| b.iter().any(|y| x.grown(1).overlaps(*y)));
        let first = (CHAIN_GAP_MIN - s.beyond(h, f)).max(0);
        let area = self.w.page.area.grown(-HALF);
        let reach = area.w().max(area.h()) / TICKS_PER_STEP;
        let host = self.bodies(l.host);
        let Some(bodies) = (0..=reach)
            .map(|k| {
                let by =
                    across_vec(s, s.across(h) - s.across(f)).plus(along_vec(s, first + k * TICKS_PER_STEP));
                tree.iter()
                    .flat_map(|t| self.bodies(*t))
                    .map(|b| b.moved(by))
                    .collect::<Vec<Rect>>()
            })
            .find(|b| !meets(b, &host))
        else {
            return Some(None);
        };
        let on = (0..self.units.len())
            .filter(|o| !tree.contains(o))
            .find(|o| meets(&bodies, &self.bodies(*o)));
        match on {
            Some(o) => Some(Some(o)),
            None if !bodies.iter().all(|b| area.holds(*b)) => Some(None),
            None => None,
        }
    }

    pub fn unit_of_id(&self, id: PartId) -> Option<usize> {
        self.w
            .parts
            .iter()
            .position(|p| p.id == id)
            .map(|i| self.unit_of[i])
    }

    fn unit_of_pin(&self, p: PinId) -> usize {
        self.unit_of[self.w.pins[&p].0]
    }

    /// A pin's point with the moves so far.
    fn pt(&self, p: PinId) -> Pt {
        let (i, at, _) = self.w.pins[&p];
        at.plus(self.d[self.unit_of[i]])
    }

    /// A pin's point on the page as it is.
    fn pt0(&self, p: PinId) -> Pt {
        self.w.pins[&p].1
    }

    pub fn side(&self, p: PinId) -> Option<Side> {
        self.w.pins[&p].2
    }

    /// The part of unit `u` that names it: its largest (a terminal, not the mark set on its pin).
    pub fn largest(&self, u: usize) -> PartId {
        self.units[u]
            .parts
            .iter()
            .map(|i| self.w.parts[*i])
            .max_by_key(|p| {
                let b = p.bounds();
                (i64::from(b.w()) * i64::from(b.h()), Reverse(p.id))
            })
            .map_or(PartId(ew_ids::Uuid::nil()), |p| p.id)
    }

    /// The first ID of a unit's parts: what its order falls back on.
    fn id(&self, u: usize) -> PartId {
        self.units[u]
            .parts
            .iter()
            .map(|i| self.w.parts[*i].id)
            .min()
            .unwrap_or(PartId(ew_ids::Uuid::nil()))
    }

    /// The order of anchors: heavier, then upper, then left, then the ID.
    fn key(&self, u: usize) -> (i64, Tick, Tick, PartId) {
        let b = self.foot(u).hull().unwrap_or_default();
        (-self.units[u].weight, b.y0, b.x0, self.id(u))
    }

    /// A unit's footprint where it now stands.
    fn foot(&self, u: usize) -> Foot {
        let mut foot = Foot {
            bodies: Vec::new(),
            texts: Vec::new(),
        };
        let d = self.d[u];
        for i in &self.units[u].parts {
            let p = self.w.parts[*i];
            foot.bodies.push(p.bounds().moved(d));
            for pin in &p.pins {
                if let Some(s) = pin.side {
                    let at = p.at.plus(pin.at).plus(d);
                    foot.bodies
                        .push(Rect::new(at, at.plus(along_vec(s, TICKS_PER_STEP))));
                }
            }
            foot.texts
                .extend(self.texts[*i].iter().map(|t| t.moved(p.at).moved(d)));
        }
        foot
    }

    /// The bodies of a unit where it now stands (an outline drawn round other parts is one to every other unit).
    fn bodies(&self, u: usize) -> Vec<Rect> {
        self.units[u]
            .parts
            .iter()
            .map(|i| self.w.parts[*i].bounds().moved(self.d[u]))
            .collect()
    }

    /// The bodies of the parts of role `role` in a unit, where it now stands.
    fn bodies_of(&self, u: usize, role: Role) -> Vec<Rect> {
        self.units[u]
            .parts
            .iter()
            .filter(|i| self.w.parts[**i].role == role)
            .map(|i| self.w.parts[*i].bounds().moved(self.d[u]))
            .collect()
    }

    fn hull(&self, u: usize) -> Rect {
        self.foot(u).hull().unwrap_or_default()
    }

    fn meets(&self, a: usize, b: usize) -> bool {
        a != b && self.foot(a).meets(&self.foot(b))
    }

    /// Whether `u` stands on a unit whose place is set (while a block is set by itself, on a unit of that block).
    fn blocked(&self, u: usize, but: &[usize]) -> bool {
        let foot = self.foot(u);
        (0..self.units.len()).any(|o| {
            o != u
                && self.placed[o]
                && !but.contains(&o)
                && self.within.as_ref().is_none_or(|w| w.contains(&o))
                && foot.meets(&self.foot(o))
        })
    }

    /// The units that hang on `u` (its followers and theirs), `u` first.
    pub fn subtree(&self, u: usize) -> Vec<usize> {
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

    /// The relations of the page ([`Kind`]), in a fixed order.
    ///
    /// A mark set on a pin of another part asks for nothing where its arrow lies along that part's body (a potential's
    /// mark on a controller's supply pin: it has no way out on its own side), and a pin with a mark set on it asks for no
    /// corner: the potential arrives there, and neither part steps for the wire from it.
    fn relations(&self) -> Vec<Rel> {
        // A mark set on a pin of another part: the parts of its unit that have a pin on its own.
        let under = |p: &PinId| -> Vec<&crate::model::Part> {
            let (i, at, _) = self.w.pins[p];
            self.units[self.unit_of[i]]
                .parts
                .iter()
                .filter(|j| **j != i && self.w.parts[i].pins.len() == 1)
                .map(|j| self.w.parts[*j])
                .filter(|part| !part.frame_only && part.pins.iter().any(|q| part.at.plus(q.at) == at))
                .collect()
        };
        // Such a mark whose pin faces along the part's body (the body reaches a step or more beyond it on that side)
        // has no way out there: it asks for no row and no line.
        let shut = |p: &PinId| {
            self.side(*p).is_some_and(|s| {
                let at = self.pt0(*p);
                under(p).iter().any(|part| {
                    let b = part.bounds();
                    let (lo, hi) = span_across(b, s);
                    (lo..=hi).contains(&s.across(at)) && span_along(b, s).1 >= out(at, s) + TICKS_PER_STEP
                })
            })
        };
        // A pin with a mark set on it is where its potential arrives: neither part steps for a corner of the wire
        // from there (the controller's box and the card beside it keep their places).
        let ridden = |p: &PinId| {
            let (i, at, _) = self.w.pins[p];
            self.units[self.unit_of[i]].parts.iter().any(|j| {
                let part = self.w.parts[*j];
                *j != i && part.pins.len() == 1 && part.at.plus(part.pins[0].at) == at
            })
        };
        let mut out: Vec<Rel> = Vec::new();
        for net in &self.w.nets {
            let mut pins: Vec<PinId> = net.pins.clone();
            pins.sort();
            pins.dedup();
            if let [a, b] = pins[..] {
                let (ua, ub) = (self.unit_of_pin(a), self.unit_of_pin(b));
                if ua != ub
                    && !shut(&a)
                    && !shut(&b)
                    && let Some(kind) = kind_of(self.side(a), self.side(b))
                    && !(kind == Kind::Corner && (ridden(&a) || ridden(&b)))
                {
                    // An earth symbol stands beside the pin it earths, on a short hook (§1.3): it has no line to take.
                    // So does every other end symbol of one pin that is no mark (the adapter tells an earth symbol
                    // by its potential, and not every one has that).
                    let earth = self.w.page.kind == PageKind::Electrical
                        && [a, b].iter().any(|p| {
                            let part = self.w.parts[self.w.pins[p].0];
                            part.pins.len() == 1 && matches!(part.role, Role::Earth | Role::Other)
                        });
                    out.push(Rel {
                        a: ua,
                        pa: a,
                        b: ub,
                        pb: b,
                        kind,
                        inline: kind == Kind::Facing && !earth,
                    });
                }
                continue;
            }
            // A net of three or more: the one pin that faces against the others on an axis, and each of those that
            // already stands beyond it.
            for first in [Side::Down, Side::Right] {
                let facing = |s: Side| -> Vec<PinId> {
                    pins.iter()
                        .copied()
                        .filter(|p| self.side(*p) == Some(s))
                        .collect()
                };
                let (one, others) = match (facing(first), facing(first.opposite())) {
                    (a, b) if a.len() == 1 && !b.is_empty() => (a[0], b),
                    (a, b) if b.len() == 1 && a.len() >= 2 => (b[0], a),
                    _ => continue,
                };
                let Some(s) = self.side(one) else { continue };
                if shut(&one) {
                    continue;
                }
                let hub = self.unit_of_pin(one);
                let at = self.pt0(one);
                // On a fluid page the supply and the work are told by their roles (rule F1): the relation holds
                // wherever the parts stand.
                let anywhere = self.w.page.kind == PageKind::Fluid;
                let branches: Vec<PinId> = others
                    .into_iter()
                    .filter(|p| {
                        self.unit_of_pin(*p) != hub
                            && (anywhere || s.beyond(at, self.pt0(*p)) >= CHAIN_GAP_MIN)
                    })
                    .collect();
                // The nearest across, where one is nearer than every other.
                let skew = |p: &PinId| (s.across(self.pt0(*p)) - s.across(at)).abs();
                let nearest = branches
                    .iter()
                    .copied()
                    .min_by_key(|p| (skew(p), *p))
                    .filter(|n| branches.iter().filter(|p| skew(p) == skew(n)).count() == 1);
                for p in branches {
                    out.push(Rel {
                        a: hub,
                        pa: one,
                        b: self.unit_of_pin(p),
                        pb: p,
                        kind: Kind::Tap,
                        inline: Some(p) == nearest,
                    });
                }
            }
        }
        // Mates: each unit and the nearest unit of its shape that stands level beside it, or in line under it, within
        // twice the row's pull (a strip's terminals, cards side by side, like devices in a row).
        let m = self.units.len();
        let body = |u: usize| self.shape(u);
        let pin = |u: usize| {
            self.units[u]
                .parts
                .iter()
                .flat_map(|i| self.w.parts[*i].pins.iter().map(|p| p.id))
                .min()
        };
        let mut mates: Vec<(usize, usize)> = Vec::new();
        for u in 0..m {
            let Some(x) = body(u) else { continue };
            for down in [true, false] {
                let next = (0..m)
                    .filter(|v| *v != u && self.mates(u, *v, down))
                    .filter_map(|v| {
                        let y = body(v)?;
                        let gap = if down { y.x0 - x.x1 } else { y.y0 - x.y1 };
                        (0..=MATE_REACH).contains(&gap).then_some((gap, self.id(v), v))
                    })
                    .min();
                if let (Some((_, _, v)), Some(pa), Some(pb)) = (next, pin(u), next.and_then(|n| pin(n.2))) {
                    mates.push((u, v));
                    out.push(Rel {
                        a: u,
                        pa,
                        b: v,
                        pb,
                        kind: Kind::Mate,
                        inline: false,
                    });
                }
            }
        }
        // A level pair is overruled where more of the unit's mates face the same host than are wired to it out and
        // back: the unit stays in its row.
        let mut rows = Groups::new(m);
        for (u, v) in &mates {
            rows.join(*u, *v);
        }
        let row = rows.roots();
        // A wire between two units of one row that runs across it asks nothing where one of them would have to leave
        // the row for it (two cards side by side, a pin on the upper edge of one wired to a pin on the lower edge of
        // the other): the row stays, and the wire is the router's.
        let lines = self.lines(&mates);
        for r in &mut out {
            if matches!(r.kind, Kind::Facing | Kind::Tap)
                && self.side(r.pa).is_some_and(|s| {
                    let line = &lines[usize::from(!s.vertical())];
                    line[r.a] == line[r.b] && s.beyond(self.pt0(r.pa), self.pt0(r.pb)) < CHAIN_GAP_MIN
                })
            {
                r.kind = Kind::Overruled;
                r.inline = false;
            }
        }
        let wired = |out: &[Rel], group: usize, host: usize, kind: Kind| {
            out.iter()
                .filter(|r| r.kind == kind)
                .flat_map(|r| [(r.a, r.b), (r.b, r.a)])
                .filter(|(u, h)| row[*u] == group && *h == host)
                .map(|(u, _)| u)
                .collect::<BTreeSet<usize>>()
                .len()
        };
        let overruled: Vec<usize> = (0..out.len())
            .filter(|k| {
                let r = &out[*k];
                r.kind == Kind::Level
                    && [(r.a, r.b), (r.b, r.a)].iter().any(|(u, h)| {
                        wired(&out, row[*u], *h, Kind::Facing) > wired(&out, row[*u], *h, Kind::Level)
                    })
            })
            .collect();
        for k in overruled {
            out[k].kind = Kind::Overruled;
        }
        // A tap does not take a unit out of its row: one of three mates or more keeps its place in it.
        let rowed = |u: usize| row.iter().filter(|r| **r == row[u]).count() >= 3;
        out.retain(|r| r.kind != Kind::Tap || !(rowed(r.a) || rowed(r.b)));
        out
    }

    /// The tree (§4.2): the leads, the relations left out, the roots.
    ///
    /// The wired relations are taken strongest first (of one strength, the one nearest in line first), each joining two
    /// units that no stronger relation joined yet: a spanning forest, with every fixed unit as ground. Then the mates join
    /// those trees, the relation nearest the heaviest unit of the heavier tree first, and only where one of the two is the
    /// heaviest of its tree or fixed (a terminal with its device joins its strip, not the row of devices; two marks of
    /// two strips are not joined; a unit that hangs on its mate is the top of no tree any more, so a spare terminal
    /// between the terminals of two cards does not join the cards); then the overruled pairs.
    ///
    /// A tree hangs from its fixed units. A tree without one hangs from its **anchor**: of its heaviest units (4 per wired
    /// pin and the bodies' area in square steps over 16; those of at least three quarters of the heaviest's weight,
    /// [`ANCHORS`] at most) the one from which the tree is set with room for every unit, with every step a rail demands,
    /// and with the least weight moved: a part out of place moves, not the circuit round it. A unit's relation towards the
    /// root is its **lead**; a relation left out of the tree is kept for the stacks, which nest by it.
    fn tree(&mut self) {
        let m = self.units.len();
        let mut rels = self.relations();
        let skew = |sk: &Self, r: &Rel| match (r.kind, sk.side(r.pa)) {
            (Kind::Facing | Kind::Tap, Some(s)) => (s.across(sk.pt0(r.pa)) - s.across(sk.pt0(r.pb))).abs(),
            _ => 0,
        };
        rels.sort_by_key(|r| {
            (
                r.kind,
                skew(self, r),
                Reverse(self.units[r.a].weight + self.units[r.b].weight),
                r.pa.min(r.pb),
                r.pa.max(r.pb),
            )
        });
        // Every fixed unit is ground: index `m`.
        let mut groups = Groups::new(m + 1);
        for u in 0..m {
            if self.units[u].fixed {
                groups.join(u, m);
            }
        }
        let mut edges: Vec<Vec<Rel>> = vec![Vec::new(); m];
        let (wired, rest): (Vec<Rel>, Vec<Rel>) = rels.into_iter().partition(|r| r.kind < Kind::Mate);
        let (mut mates, overruled): (Vec<Rel>, Vec<Rel>) =
            rest.into_iter().partition(|r| r.kind == Kind::Mate);
        let pairs: Vec<(usize, usize)> = mates.iter().map(|r| (r.a, r.b)).collect();
        // The trees the wires make.
        for r in wired {
            let (a, b) = (groups.root(r.a), groups.root(r.b));
            if a == b {
                self.extra.push(r);
                continue;
            }
            groups.join(a, b);
            edges[r.a].push(r);
            edges[r.b].push(r);
        }
        // Mates join those trees. A mate relation holds where one of the two is the heaviest of its tree (its top: it
        // hangs on its mate with what hangs on it) or is fixed: two units that both hang on heavier ones (two marks
        // of two strips) are not joined. The relation nearest the top of the heavier tree first: a terminal with its
        // device joins its strip, not the row of devices.
        let mut heaviest: BTreeMap<usize, (i64, Tick, Tick, PartId)> = BTreeMap::new();
        let mut weight: BTreeMap<usize, i64> = BTreeMap::new();
        for u in 0..m {
            let c = groups.root(u);
            *weight.entry(c).or_default() += self.units[u].weight.max(1);
            let key = self.key(u);
            let old = heaviest.entry(c).or_insert(key);
            *old = (*old).min(key);
        }
        let mut top: Vec<bool> = (0..m)
            .map(|u| {
                self.units[u].fixed
                    || heaviest
                        .get(&groups.root(u))
                        .is_some_and(|k| k.0 == -self.units[u].weight)
            })
            .collect();
        // Each unit's depth in its tree, from the tree's heaviest unit.
        let mut depth: Vec<usize> = vec![usize::MAX; m];
        let mut queue: VecDeque<usize> = (0..m)
            .filter(|u| heaviest.get(&groups.root(*u)) == Some(&self.key(*u)))
            .collect();
        for u in &queue {
            depth[*u] = 0;
        }
        while let Some(u) = queue.pop_front() {
            for r in &edges[u] {
                let v = if r.a == u { r.b } else { r.a };
                if depth[v] == usize::MAX {
                    depth[v] = depth[u] + 1;
                    queue.push_back(v);
                }
            }
        }
        let tree_weight: Vec<i64> = (0..m)
            .map(|u| weight.get(&groups.root(u)).copied().unwrap_or(0))
            .collect();
        mates.sort_by_key(|r| {
            let (near, far) = if (tree_weight[r.a], Reverse(r.a)) >= (tree_weight[r.b], Reverse(r.b)) {
                (r.a, r.b)
            } else {
                (r.b, r.a)
            };
            (depth[near], depth[far], r.pa.min(r.pb), r.pa.max(r.pb))
        });
        for r in mates.into_iter().chain(overruled) {
            let (a, b) = (groups.root(r.a), groups.root(r.b));
            if a == b {
                self.extra.push(r);
                continue;
            }
            if r.kind == Kind::Mate && !(top[r.a] || top[r.b]) {
                continue;
            }
            // (A unit that hangs on its mate is the top of no tree any more: a lone terminal between two strips of
            // two cards does not join the cards.)
            let (ka, kb) = (heaviest.get(&a).copied(), heaviest.get(&b).copied());
            for u in [r.a, r.b] {
                let (own, other) = if groups.root(u) == a { (ka, kb) } else { (kb, ka) };
                if !self.units[u].fixed && own > other {
                    top[u] = false;
                }
            }
            heaviest.insert(a.min(b), ka.min(kb).unwrap_or_else(|| self.key(r.a)));
            groups.join(a, b);
            edges[r.a].push(r);
            edges[r.b].push(r);
        }
        self.edges = edges;
        self.strips = self.lines(&pairs);
        // From the fixed units, then from the heaviest unit of each tree left (the anchor is chosen when the tree is
        // set: `anchor`).
        let mut starts: Vec<usize> = (0..m).collect();
        starts.sort_by_key(|u| (!self.units[*u].fixed, self.key(*u)));
        let fixed: Vec<usize> = starts.iter().copied().filter(|u| self.units[*u].fixed).collect();
        let mut seen = self.hang(&fixed);
        self.roots = fixed;
        for u in starts {
            if !seen[u] {
                for (k, s) in self.hang(&[u]).into_iter().enumerate() {
                    seen[k] |= s;
                }
                self.roots.push(u);
            }
        }
    }

    /// The rows the person made, from the pairs of mates: the units joined through mates that stand side by side have
    /// one number in the first list, those that stand one under another one number in the second.
    fn lines(&self, pairs: &[(usize, usize)]) -> [Vec<usize>; 2] {
        [true, false].map(|down| {
            let mut of = Groups::new(self.units.len());
            for (a, b) in pairs {
                if self.mates(*a, *b, down) {
                    of.join(*a, *b);
                }
            }
            of.roots()
        })
    }

    /// Hangs the trees of `roots` from them, breadth first: each unit reached takes its relation towards the root as
    /// its lead. The units reached.
    fn hang(&mut self, roots: &[usize]) -> Vec<bool> {
        let mut seen = vec![false; self.units.len()];
        let mut queue: VecDeque<usize> = VecDeque::new();
        for u in roots {
            seen[*u] = true;
            self.lead[*u] = None;
            queue.push_back(*u);
        }
        while let Some(h) = queue.pop_front() {
            let mut found: Vec<(usize, Lead)> = self.edges[h]
                .iter()
                .filter_map(|r| {
                    let (f, lead) = if r.a == h {
                        (r.b, (r.pa, r.pb))
                    } else {
                        (r.a, (r.pb, r.pa))
                    };
                    (!seen[f]).then_some((
                        f,
                        Lead {
                            host: h,
                            h: lead.0,
                            f: lead.1,
                            kind: r.kind,
                            inline: r.inline,
                        },
                    ))
                })
                .collect();
            found.sort_by_key(|(f, l)| (self.key(*f), l.h, l.f));
            for (f, l) in found {
                if !seen[f] {
                    seen[f] = true;
                    self.lead[f] = Some(l);
                    queue.push_back(f);
                }
            }
        }
        seen
    }

    /// The rails of the page: each net of three pins or more whose pins, but marks', all face one side up or down, as
    /// that side, its tapped pins, its marks' pins and the line its junctions lie on (two at least), as [`out`]
    /// measures along the side (§4.6).
    ///
    /// A net of three pins or more whose pins, but marks', all face one side up or down is a rail. The units it taps
    /// belong on the row most of them stand on, where they stand within [`ROW_PULL`] of it; the step is a demand where the
    /// unit stands beyond the line of the rail's junctions. A mark of the rail belongs on that line.
    fn rails(&self) -> Vec<Rail> {
        let mark = |p: &PinId| self.w.parts[self.w.pins[p].0].role.is_mark();
        let mut rails = Vec::new();
        for net in &self.w.nets {
            let mut pins: Vec<PinId> = net.pins.clone();
            pins.sort();
            pins.dedup();
            let (marks, taps): (Vec<PinId>, Vec<PinId>) = pins.iter().partition(|p| mark(p));
            let sides: BTreeSet<Option<Side>> = taps.iter().map(|p| self.side(*p)).collect();
            let side = match sides.into_iter().collect::<Vec<_>>()[..] {
                [Some(s)] if s.vertical() => s,
                _ => continue,
            };
            if pins.len() < 3 || taps.len() < 2 {
                continue;
            }
            let line = self.w.line(net).map(|y| out(Pt::new(0, y), side));
            rails.push((side, taps, marks, line));
        }
        rails
    }

    /// What the rails ask (§4.6): each tapped unit that does not stand on its rail's row, with the step that sets it
    /// there and whether it has to take it (it stands beyond its rail's line); each mark of a rail with a line, with
    /// the step onto the line. The row is the level most of a rail's taps stand on, short of the line by the trunk gap
    /// at least; a tap further from it than [`ROW_PULL`] keeps its own.
    fn rail_steps(&self) -> BTreeMap<usize, (Pt, bool)> {
        let mut steps: BTreeMap<usize, (Pt, bool)> = BTreeMap::new();
        for (s, taps, marks, line) in self.rails() {
            let level = |p: &PinId| out(self.pt0(*p), s);
            let fits = |l: Tick| line.is_none_or(|r| l <= r - TRUNK_GAP);
            let mut count: BTreeMap<Tick, usize> = BTreeMap::new();
            for p in taps.iter().filter(|p| fits(level(p))) {
                *count.entry(level(p)).or_default() += 1;
            }
            let row = count
                .iter()
                .max_by_key(|(l, k)| (**k, **l))
                .map(|(l, _)| *l)
                .or(line.map(|r| r - TRUNK_GAP));
            let Some(row) = row else { continue };
            for p in &taps {
                let u = self.unit_of_pin(*p);
                let by = row - level(p);
                let must = !fits(level(p));
                if by != 0 && !self.units[u].fixed && (must || by.abs() <= ROW_PULL) {
                    let step = (along_vec(s, by), must);
                    steps
                        .entry(u)
                        .and_modify(|old| {
                            if must && !old.1 {
                                *old = step;
                            }
                        })
                        .or_insert(step);
                }
            }
            if let Some(r) = line {
                for p in &marks {
                    let u = self.unit_of_pin(*p);
                    let by = r - level(p);
                    if by != 0 && !self.units[u].fixed && self.units[u].parts.len() == 1 {
                        steps.entry(u).or_insert((along_vec(s, by), false));
                    }
                }
            }
        }
        steps
    }

    /// Sets every unit's place (§4.4).
    ///
    /// The trees are set one after another, the heaviest first, each as if the units of the trees after it will step
    /// aside. A unit that then kept its place where a moved unit came to stand is in the way from the start, and the
    /// trees are set again ([`RESETS`] times at most).
    pub fn lay(&mut self) {
        let m = self.units.len();
        self.steps = self.rail_steps();
        self.loose = self.loose();
        // A tree is set as if the units of the trees after it will step aside. One that then kept its place where a
        // moved unit came to stand was counted on in vain: it is in the way from the start, and the trees are set
        // again ([`RESETS`] times at most).
        let mut stay: BTreeSet<usize> = BTreeSet::new();
        for _ in 0..=RESETS {
            self.d.fill(Pt::default());
            self.settled.clear();
            // What never moves is in the way from the start: the fixed units and the units without a relation.
            for u in 0..m {
                self.placed[u] = self.units[u].fixed || self.edges[u].is_empty() || stay.contains(&u);
            }
            self.lay_trees();
            let hit: Vec<usize> = (0..m)
                .filter(|o| {
                    self.d[*o] == Pt::default()
                        && !stay.contains(o)
                        && (0..m).any(|u| self.d[u] != Pt::default() && self.bodies_meet(u, *o))
                })
                .collect();
            if hit.is_empty() {
                break;
            }
            stay.extend(hit);
        }
        if self.open {
            self.open_strips();
        }
        self.ring_room();
        self.legal();
    }

    /// The room for the rings of the wires round a body (§3.5, Rings): a unit whose wires leave its pins on one
    /// side and go round its body to parts that stay behind it (beyond its far edge, under the body and facing it:
    /// terminals kept under a card whose pins face up) steps away from them along that side, with what hangs on it,
    /// until the rings round each end of the body, a step each, and a step clear of those parts' stubs fit between;
    /// where it then stands inside the drawing area and on no unit, else it stays.
    fn ring_room(&mut self) {
        use crate::rounds::{Ask, Corner};
        for u in 0..self.units.len() {
            if self.units[u].fixed {
                continue;
            }
            let hangs = self.subtree(u);
            let mut step: Option<(Side, Tick)> = None;
            for &i in &self.units[u].parts {
                let part = self.w.parts[i];
                let body = part.bounds().moved(self.d[u]);
                let pins: Vec<(PinId, Pt, Side)> = part
                    .pins
                    .iter()
                    .filter(|p| self.wired.contains(&p.id))
                    .filter_map(|p| Some((p.id, self.pt(p.id), p.side?)))
                    .collect();
                for s in [Side::Up, Side::Down, Side::Left, Side::Right] {
                    let back = s.opposite();
                    let (edge, (lo, hi)) = (span_along(body, back).1, span_across(body, s));
                    // The wires round each end, and the nearest of the parts they lead to.
                    let (mut ends, mut gap) = ([0; 2], None::<Tick>);
                    for &(p, at, _) in pins.iter().filter(|x| x.2 == s) {
                        for net in self
                            .w
                            .nets
                            .iter()
                            .filter(|n| n.pins.len() == 2 && n.pins.contains(&p))
                        {
                            let Some(&q) = net.pins.iter().find(|q| **q != p) else {
                                continue;
                            };
                            let (o, qa) = (self.unit_of_pin(q), self.pt(q));
                            let under = lo <= s.across(qa) && s.across(qa) <= hi && out(qa, back) > edge;
                            if hangs.contains(&o)
                                || self.d[o] != Pt::default()
                                || self.side(q) != Some(s)
                                || !under
                            {
                                continue;
                            }
                            let ask = Ask {
                                pin: (at, s),
                                body,
                                other: (qa, s),
                                wired: pins
                                    .iter()
                                    .filter(|x| x.0 != p && x.2 == s)
                                    .map(|x| x.1)
                                    .collect(),
                            };
                            let end = [Corner::First, Corner::Last]
                                .into_iter()
                                .min_by_key(|c| (ask.passes(*c), ask.reach(*c)));
                            ends[usize::from(end == Some(Corner::Last))] += 1;
                            let g = out(qa, back) - edge;
                            gap = Some(gap.map_or(g, |x| x.min(g)));
                        }
                    }
                    let depth: Tick = ends[0].max(ends[1]);
                    if let Some(g) = gap
                        && depth > 0
                        && g < (depth + 1) * TICKS_PER_STEP + STUB
                    {
                        step = Some((s, grid_away((depth + 1) * TICKS_PER_STEP + STUB - g)));
                    }
                }
            }
            let Some((s, by)) = step else { continue };
            let movers: Vec<usize> = hangs.into_iter().filter(|v| !self.units[*v].fixed).collect();
            self.shift(&movers, along_vec(s, by));
            if movers
                .iter()
                .any(|v| self.blocked(*v, &movers) || !self.inside(*v))
            {
                self.shift(&movers, along_vec(s, -by));
            }
        }
    }

    /// Sets the trees one after another, the heaviest first.
    fn lay_trees(&mut self) {
        let mut wish: BTreeMap<usize, Pt> = BTreeMap::new();
        for k in 0..self.roots.len() {
            let mut root = self.roots[k];
            let by = if self.edges[root].is_empty() {
                let first = self.first_step(root, &wish);
                self.step_alone(root, first)
            } else {
                if !self.units[root].fixed {
                    root = self.anchor(root, &wish);
                    self.roots[k] = root;
                }
                let first = self.first_step(root, &wish);
                self.settle(root, first)
            };
            // A row the person made stays a row: the roots of its shape that stood level with it step with it.
            if by == Pt::default() {
                continue;
            }
            for mate in self.roots.clone() {
                if mate == root || self.units[mate].fixed || wish.contains_key(&mate) {
                    continue;
                }
                let with = Pt::new(
                    if self.mates(root, mate, false) { by.x } else { 0 },
                    if self.mates(root, mate, true) { by.y } else { 0 },
                );
                if with != Pt::default() && !self.settled.contains(&mate) {
                    wish.insert(mate, with);
                }
            }
        }
    }

    /// A root nothing hangs on steps by `by` with its row, where it then stands clear inside the drawing area; the step
    /// it took.
    fn step_alone(&mut self, root: usize, by: Pt) -> Pt {
        self.settled.insert(root);
        if by == Pt::default() || self.units[root].fixed {
            return Pt::default();
        }
        self.d[root] = by;
        let clear = !(0..self.units.len()).any(|o| o != root && self.placed[o] && self.bodies_meet(root, o));
        if clear && self.inside(root) {
            by
        } else {
            self.d[root] = Pt::default();
            Pt::default()
        }
    }

    /// Whether the bodies of `u` lie in the drawing area, half a step inside its edge.
    fn inside(&self, u: usize) -> bool {
        let area = self.w.page.area.grown(-HALF);
        self.bodies(u).iter().all(|b| area.holds(*b))
    }

    /// The step a root tries first: what its rail asks, and its row's step along the other axis.
    fn first_step(&self, root: usize, wish: &BTreeMap<usize, Pt>) -> Pt {
        let rail = self.steps.get(&root).map_or(Pt::default(), |s| s.0);
        let row = wish.get(&root).copied().unwrap_or_default();
        let card = self.card_step(root, self.card_steps);
        Pt::new(
            if rail.x != 0 {
                rail.x
            } else if row.x != 0 {
                row.x
            } else {
                card.x
            },
            if rail.y != 0 { rail.y } else { row.y },
        )
    }

    /// The anchor of the tree that hangs from `root`: of its heaviest units, the one from which the tree is set with
    /// every step a rail demands taken and the least weight moved (a part out of place moves, not the circuit round
    /// it); then one a rail taps, then the heaviest. The tree is left hanging from it.
    fn anchor(&mut self, root: usize, wish: &BTreeMap<usize, Pt>) -> usize {
        let units = self.anchors(root);
        if self.subtree(root).len() < 2 {
            return root;
        }
        let mut best: Option<((bool, usize, i64, bool), usize)> = None;
        for c in units {
            self.hang(&[c]);
            let tree = self.subtree(c);
            let first = self.first_step(c, wish);
            let found = self.room(c, &tree, first);
            let unmet = tree
                .iter()
                .filter(|u| {
                    self.steps
                        .get(u)
                        .is_some_and(|(by, must)| *must && (self.d[**u].x, self.d[**u].y) != (by.x, by.y))
                })
                .count();
            let moved: i64 = tree
                .iter()
                .filter(|u| self.d[**u] != Pt::default())
                .map(|u| self.units[*u].weight.max(1))
                .sum();
            let cost = (found.is_none(), unmet, moved, !self.steps.contains_key(&c));
            if best.as_ref().is_none_or(|(b, _)| cost < *b) {
                best = Some((cost, c));
            }
            for u in &tree {
                self.d[*u] = Pt::default();
            }
        }
        let anchor = best.map_or(root, |(_, c)| c);
        self.hang(&[anchor]);
        anchor
    }

    /// The units of the tree that hangs from `root` that may be its anchor: about as heavy as the heaviest (a strip
    /// does not hang on one of its marks), the heaviest first, [`ANCHORS`] at most.
    fn anchors(&self, root: usize) -> Vec<usize> {
        let mut units = self.subtree(root);
        units.sort_by_key(|u| self.key(*u));
        let most = units.iter().map(|u| self.units[*u].weight).max().unwrap_or(0);
        units.retain(|u| ANCHOR_SHARE.1 * self.units[*u].weight >= ANCHOR_SHARE.0 * most);
        units.truncate(ANCHORS);
        units
    }

    /// The tree of `root` set for good ([`Sketch::room`]); the root's step.
    fn settle(&mut self, root: usize, first: Pt) -> Pt {
        let tree = self.subtree(root);
        let taken = self.room(root, &tree, first);
        for u in &tree {
            self.placed[*u] = true;
            self.settled.insert(*u);
        }
        taken.unwrap_or_default()
    }

    /// The tree of `root` set from the root where it stands, or stepped by the least distance at which the whole tree
    /// has room (`first` tried first); the root's step. Where no step gives it room, the root stays and what has no
    /// room goes back to where it stood.
    ///
    /// A tree is set from its root where it stands. Where a unit of it then has no room (it would leave the drawing area
    /// or stand on a unit that is set) the root steps by whole steps, the least first and [`REACH`] at most each way, until
    /// the whole tree has room; the roots of its shape that stood level with it take the same step, so that a row of cards
    /// stays a row. Without such a step the root stays, and what has no room goes back to where it stood.
    fn room(&mut self, root: usize, tree: &[usize], first: Pt) -> Option<Pt> {
        let mut steps: Vec<Pt> = vec![first, Pt::default()];
        if !self.units[root].fixed {
            let s = TICKS_PER_STEP;
            for k in 1..=REACH {
                steps.extend([
                    Pt::new(0, k * s),
                    Pt::new(0, -k * s),
                    Pt::new(k * s, 0),
                    Pt::new(-k * s, 0),
                ]);
            }
        } else {
            steps = vec![Pt::default()];
        }
        let mut taken: Option<Pt> = None;
        for by in steps {
            if self.set_tree(root, tree, by) {
                taken = Some(by);
                break;
            }
        }
        if taken.is_none() {
            self.set_tree(root, tree, Pt::default());
            // What stands outside or on a unit goes back, with what hangs on it.
            for _ in 0..tree.len() {
                let bad = tree.iter().copied().find(|u| {
                    self.d[*u] != Pt::default()
                        && (!self.inside(*u)
                            || (0..self.units.len()).any(|o| {
                                o != *u && (self.placed[o] || tree.contains(&o)) && self.bodies_meet(*u, o)
                            }))
                });
                let Some(u) = bad else { break };
                for f in self.subtree(u) {
                    self.d[f] = Pt::default();
                }
            }
        }
        taken
    }

    /// Sets the tree of `root` with the root stepped by `by`; whether every unit of it then has its place: inside the
    /// drawing area, on no unit that is set, none left without room.
    fn set_tree(&mut self, root: usize, tree: &[usize], by: Pt) -> bool {
        for u in tree {
            self.d[*u] = Pt::default();
            self.placed[*u] = false;
        }
        self.d[root] = by;
        self.placed[root] = true;
        self.failed = false;
        self.lay_from(root);
        self.fluid(tree);
        // A unit that keeps its place has it, however near the person set it to another.
        let ok = !self.failed
            && tree.iter().all(|u| {
                self.d[*u] == Pt::default()
                    || (self.inside(*u)
                        && !(0..self.units.len()).any(|o| {
                            o != *u && self.placed[o] && !tree.contains(&o) && self.bodies_meet(*u, o)
                        }))
            });
        for u in tree {
            self.placed[*u] = false;
        }
        ok
    }

    /// On a fluid page (rule F1): what hangs on a valve's unit with a part of the supply that does not stand lower than
    /// every valve steps down by the least distance, and what hangs there with an actuator that does not stand higher
    /// than that unit's valves steps up; each by whole blocks, until it stands clear.
    ///
    /// **On a fluid page** (rule F1) what hangs on a valve's unit with a part of the supply steps below every valve, and
    /// what hangs there with an actuator steps above that unit's valves, with the blocks above it, so that a stack
    /// keeps its order.
    fn fluid(&mut self, tree: &[usize]) {
        if self.w.page.kind != PageKind::Fluid {
            return;
        }
        let reach = self.w.page.area.h();
        let low = (0..self.units.len())
            .flat_map(|u| self.bodies_of(u, Role::Valve))
            .map(|b| b.y1)
            .max();
        let Some(low) = low else { return };
        let hosts: Vec<usize> = tree
            .iter()
            .copied()
            .filter(|u| !self.bodies_of(*u, Role::Valve).is_empty())
            .collect();
        for host in hosts {
            let top = self
                .bodies_of(host, Role::Valve)
                .iter()
                .map(|b| b.y0)
                .min()
                .unwrap_or(low);
            let mut blocks: Vec<usize> = (0..self.units.len())
                .filter(|f| self.lead[*f].is_some_and(|l| l.host == host) && !self.units[*f].fixed)
                .filter(|f| !self.subtree(*f).iter().any(|u| self.units[*u].fixed))
                .collect();
            // The lowest first: a block that steps up takes the blocks above it along, so that a stack keeps its order.
            let foot = |sk: &Self, b: usize| {
                sk.subtree(b)
                    .iter()
                    .flat_map(|u| sk.bodies(*u))
                    .map(|r| r.y1)
                    .max()
            };
            blocks.sort_by_key(|b| (Reverse(foot(self, *b)), self.id(*b)));
            for (k, b) in blocks.iter().enumerate() {
                let mut units = self.subtree(*b);
                let of = |sk: &Self, role: Role| -> Vec<Rect> {
                    units.iter().flat_map(|u| sk.bodies_of(*u, role)).collect()
                };
                let down = of(self, Role::Source)
                    .iter()
                    .map(|r| low + CHAIN_GAP_MIN - r.y0)
                    .max()
                    .unwrap_or(0);
                let rise = of(self, Role::Actuator)
                    .iter()
                    .map(|r| r.y1 - (top - CHAIN_GAP_MIN))
                    .max()
                    .unwrap_or(0);
                let (by, step) = if down > 0 {
                    (grid_away(down), TICKS_PER_STEP)
                } else if rise > 0 {
                    units.extend(blocks[k + 1..].iter().flat_map(|o| self.subtree(*o)));
                    (-grid_away(rise), -TICKS_PER_STEP)
                } else {
                    continue;
                };
                self.shift(&units, Pt::new(0, by));
                let mut more = 0;
                while more < reach
                    && units.iter().any(|u| {
                        (0..self.units.len()).any(|o| {
                            !units.contains(&o)
                                && (self.placed[o] || tree.contains(&o))
                                && self.bodies_meet(*u, o)
                        })
                    })
                {
                    self.shift(&units, Pt::new(0, step));
                    more += TICKS_PER_STEP;
                }
            }
        }
    }

    /// Sets what hangs on `root`, rank by rank.
    ///
    /// From the root outward, rank by rank (the root, then the units set beside one another by one step of the method).
    /// The mates that hang on a rank join it first: each takes its host's step, stands level with it, takes the pitch the
    /// row now has where the row had one, and what a rail asks of it.
    fn lay_from(&mut self, root: usize) {
        let mut ranks: VecDeque<Vec<usize>> = VecDeque::from([vec![root]]);
        let mut late: VecDeque<(Side, Vec<usize>)> = VecDeque::new();
        while let Some(mut hosts) = ranks.pop_front() {
            self.join_mates(&mut hosts);
            let mut rows: BTreeMap<Side, Vec<usize>> = BTreeMap::new();
            let mut stacks: BTreeMap<(Side, Side), Vec<usize>> = BTreeMap::new();
            let mut besides: BTreeMap<Side, Vec<usize>> = BTreeMap::new();
            // The next rank: the rows and the level followers that stay, so that what hangs on units that stand in
            // one line is set as one row.
            let mut next: Vec<usize> = Vec::new();
            for (f, l) in self.lead.iter().enumerate() {
                let Some(l) = l else { continue };
                if !hosts.contains(&l.host) || self.units[f].fixed {
                    continue;
                }
                let (Some(s), Some(sf)) = (self.side(l.h), self.side(l.f)) else {
                    continue;
                };
                // A terminal of a strip over a card that is no channel's stays in its strip: it is set with the strip
                // (`open_strips`), not beside or round a corner from its host.
                if self.loose.contains(&f) && !matches!(l.kind, Kind::Facing | Kind::Tap | Kind::Mate) {
                    continue;
                }
                match l.kind {
                    Kind::Facing | Kind::Tap => rows.entry(s).or_default().push(f),
                    Kind::Corner => stacks.entry((s, sf)).or_default().push(f),
                    Kind::Level | Kind::Overruled => besides.entry(s).or_default().push(f),
                    Kind::Mate => {}
                }
            }
            for (s, members) in rows {
                // The members that stand far beyond the nearest of the row wait until what hangs on the near ones is
                // set: a device wired straight to a card does not take the place of the row over the terminals.
                let dist = |sk: &Self, m: usize| sk.lead[m].map_or(0, |l| s.beyond(sk.pt(l.h), sk.pt(l.f)));
                let nearest = members
                    .iter()
                    .map(|m| dist(self, *m))
                    .filter(|d| *d >= CHAIN_GAP_MIN)
                    .min();
                let (near, far): (Vec<usize>, Vec<usize>) = members
                    .iter()
                    .partition(|m| nearest.is_none_or(|n| dist(self, **m) <= n + ROW_PULL));
                self.row(&hosts, s, &near);
                next.extend(near);
                if !far.is_empty() {
                    late.push_back((s, far));
                }
            }
            for ((s, sf), members) in stacks {
                self.blocks(&members);
                // A block heavier than the unit it hangs on keeps its place: a card with its channels does not step
                // for the corner of one supply wire.
                let block =
                    |sk: &Self, m: usize| sk.subtree(m).iter().map(|u| sk.units[*u].weight).sum::<i64>();
                let (heavy, light): (Vec<usize>, Vec<usize>) = members.iter().partition(|m| {
                    self.lead[**m].is_some_and(|l| block(self, **m) > self.units[l.host].weight)
                });
                self.stack(&hosts, s, sf, &light);
                let kept: Vec<usize> = heavy.into_iter().flat_map(|m| self.subtree(m)).collect();
                for u in kept {
                    self.placed[u] = true;
                }
            }
            for (s, members) in besides {
                self.blocks(&members);
                next.extend(self.beside(s, &members));
            }
            if !next.is_empty() {
                ranks.push_back(next);
            }
            if ranks.is_empty()
                && let Some((s, members)) = late.pop_front()
            {
                self.row(&[], s, &members);
                ranks.push_back(members);
            }
        }
    }

    /// A moved unit that still stands outside the drawing area or on a unit goes back to where it stood, with what
    /// hangs on it.
    fn legal(&mut self) {
        for _ in 0..self.units.len() {
            let bad = (0..self.units.len()).find(|u| {
                self.d[*u] != Pt::default()
                    && (!self.inside(*u)
                        || (0..self.units.len()).any(|o| {
                            o != *u
                                && self
                                    .bodies(*u)
                                    .iter()
                                    .any(|a| self.bodies(o).iter().any(|b| a.grown(1).overlaps(*b)))
                        }))
            });
            let Some(u) = bad else { break };
            for f in self.subtree(u) {
                self.d[f] = Pt::default();
            }
        }
    }

    /// Whether units `a` and `b` are of one shape and stood level on the page as it is (`down`: side by side, their
    /// upper edges on one line; else one over the other, their left edges on one line).
    fn mates(&self, a: usize, b: usize, down: bool) -> bool {
        let (Some(x), Some(y)) = (self.shape(a), self.shape(b)) else {
            return false;
        };
        // Level, or a step off it: a row set down a step crooked is a row.
        let off = if down { x.y0 - y.y0 } else { x.x0 - y.x0 };
        // (One size up to the rounding of a body that does not lie on the ticks.)
        let like = (x.w() - y.w()).abs() <= LABEL_CLEAR && (x.h() - y.h()).abs() <= LABEL_CLEAR;
        like && off.abs() <= TICKS_PER_STEP
    }

    /// The body of a unit's largest part on the page as it is: its shape.
    fn shape(&self, u: usize) -> Option<Rect> {
        self.units[u]
            .parts
            .iter()
            .map(|i| self.w.parts[*i].bounds())
            .max_by_key(|b| (i64::from(b.w()) * i64::from(b.h()), Reverse(*b)))
    }

    /// The mates that hang on the units of `rank` join it, each set beside its host: with it by the host's own step,
    /// and at the pitch the host keeps from its mate on the other side where that is nearer (a strip's terminal
    /// without a lead of its own stays in its strip, at the strip's new pitch).
    fn join_mates(&mut self, rank: &mut Vec<usize>) {
        let mut k = 0;
        while k < rank.len() {
            let host = rank[k];
            k += 1;
            let followers: Vec<usize> = (0..self.units.len())
                .filter(|f| {
                    self.lead[*f].is_some_and(|l| l.host == host && l.kind == Kind::Mate)
                        && !self.units[*f].fixed
                })
                .collect();
            for f in followers {
                self.d[f] = self.d[host];
                let (Some(a), Some(b)) = (self.shape(host), self.shape(f)) else {
                    continue;
                };
                let down = (a.x0 - b.x0).abs() > (a.y0 - b.y0).abs();
                let at = |r: Rect| if down { r.x0 } else { r.y0 };
                let was = at(b) - at(a);
                // Level with its host.
                self.d[f] = self.d[f].plus(if down {
                    Pt::new(0, a.y0 - b.y0)
                } else {
                    Pt::new(a.x0 - b.x0, 0)
                });
                // The host's mate on the other side that stood as far from it as this one: the row's pitch as it was
                // and as it now is.
                let other = rank
                    .iter()
                    .copied()
                    .filter(|o| *o != host && *o != f && self.mates(host, *o, down))
                    .filter_map(|o| {
                        let c = self.shape(o)?;
                        let then = at(c) - at(a);
                        let now = then
                            + if down {
                                self.d[o].x - self.d[host].x
                            } else {
                                self.d[o].y - self.d[host].y
                            };
                        (then == -was && now.signum() == then.signum()).then_some(now.abs())
                    })
                    .min();
                if let Some(pitch) = other
                    && pitch != was.abs()
                    && pitch >= (if down { a.w() } else { a.h() }) + ROW_GAP
                {
                    let by = was.signum() * pitch - was;
                    let step = if down { Pt::new(by, 0) } else { Pt::new(0, by) };
                    self.d[f] = self.d[f].plus(step);
                    let on = (0..self.units.len()).any(|o| {
                        o != f
                            && self.placed[o]
                            && self.within.as_ref().is_none_or(|w| w.contains(&o))
                            && self.bodies_meet(f, o)
                    });
                    if on {
                        self.d[f] = self.d[f].minus(step);
                    }
                }
                // What a rail asks of it comes first.
                if let Some((step, _)) = self.steps.get(&f).copied() {
                    if step.x != 0 {
                        self.d[f].x = step.x;
                    }
                    if step.y != 0 {
                        self.d[f].y = step.y;
                    }
                }
                // On a unit that is set (two of a row set down on each other), it steps away from its host.
                let away = if was < 0 { -TICKS_PER_STEP } else { TICKS_PER_STEP };
                let away = if down { Pt::new(away, 0) } else { Pt::new(0, away) };
                let mut more = 0;
                while more < 2 * ROW_PULL
                    && (0..self.units.len()).any(|o| {
                        o != f
                            && self.placed[o]
                            && self.within.as_ref().is_none_or(|w| w.contains(&o))
                            && self.bodies_meet(f, o)
                    })
                {
                    self.d[f] = self.d[f].plus(away);
                    more += TICKS_PER_STEP;
                }
                self.placed[f] = true;
                rank.push(f);
            }
        }
    }

    /// The moves, by part ID.
    pub fn moves(&self) -> Vec<(PartId, Pt)> {
        let mut out: Vec<(PartId, Pt)> = self
            .w
            .parts
            .iter()
            .enumerate()
            .filter(|(i, _)| self.d[self.unit_of[*i]] != Pt::default())
            .map(|(i, p)| (p.id, self.d[self.unit_of[i]]))
            .collect();
        out.sort();
        out
    }

    /// The branches that moved: each unit whose move is its own (not its host's), with what hangs on it; the roots'
    /// first.
    pub fn branches(&self) -> Vec<Vec<usize>> {
        let mut order: Vec<usize> = Vec::new();
        let mut queue: VecDeque<usize> = self.roots.iter().copied().collect();
        while let Some(u) = queue.pop_front() {
            order.push(u);
            for (f, l) in self.lead.iter().enumerate() {
                if l.is_some_and(|l| l.host == u) {
                    queue.push_back(f);
                }
            }
        }
        let heads = order.into_iter().filter(|u| {
            let own = match self.lead[*u] {
                Some(l) => self.d[*u].minus(self.d[l.host]),
                None => self.d[*u],
            };
            own != Pt::default() && !self.units[*u].fixed
        });
        // The members of a row the person made (of one shape, level, on one host or on hosts that are a row
        // themselves) go or stay as one: a row is not left half moved.
        let level = |a: usize, b: usize| self.mates(a, b, true) || self.mates(a, b, false);
        let one_row = |a: usize, b: usize| match (self.lead[a], self.lead[b]) {
            (Some(x), Some(y)) => {
                level(a, b)
                    && x.kind == y.kind
                    && self.side(x.h) == self.side(y.h)
                    && (x.host == y.host || level(x.host, y.host))
            }
            _ => false,
        };
        let mut rows: Vec<Vec<usize>> = Vec::new();
        for h in heads {
            match rows.iter_mut().find(|r| r.iter().any(|o| one_row(*o, h))) {
                Some(r) => r.push(h),
                None => rows.push(vec![h]),
            }
        }
        let mut out: Vec<Vec<usize>> = Vec::new();
        for r in rows {
            let units = self.with_strips(r.into_iter().flat_map(|h| self.subtree(h)).collect());
            match out.iter_mut().find(|o| o.iter().any(|u| units.contains(u))) {
                Some(o) => {
                    o.extend(units);
                    o.sort_unstable();
                    o.dedup();
                }
                None => out.push(units),
            }
        }
        out
    }

    /// The trees with a move, each as its units.
    pub fn groups(&self) -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = Vec::new();
        for r in &self.roots {
            let units = self.with_strips(self.subtree(*r));
            if !units.iter().any(|u| self.d[*u] != Pt::default()) {
                continue;
            }
            match out.iter_mut().find(|o| o.iter().any(|u| units.contains(u))) {
                Some(o) => {
                    o.extend(units);
                    o.sort_unstable();
                    o.dedup();
                }
                None => out.push(units),
            }
        }
        out
    }
}
