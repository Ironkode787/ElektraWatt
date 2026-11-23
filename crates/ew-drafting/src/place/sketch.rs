// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The arrangement the placer proposes (docs/design/06a-drafting.md §4.2 to §4.7): [`Sketch`] reads a page's units and
//! relations, hangs the units in trees and sets each where its lead asks. It routes nothing and rates nothing:
//! [`super::arrange`] routes what it proposes and lets the measure judge.
//!
//! # Units and relations
//!
//! A **unit** moves as one: the parts of one `unit` number, the parts inside an outline drawn round other parts, and a
//! part that stands with its pin on another part's pin (a mark set on a connection point). It is **fixed** when a part
//! of it is kept in place, outside the scope, unticked, off the whole grid, or at the end of a kept wire.
//!
//! A **relation** joins a pin of one unit to a pin of another, by strength:
//!
//! 1. **facing**: the two pins of a net of two, on opposite sides: the best drawing is one straight wire. An end
//!    symbol of one pin that is no mark (an earth symbol among them) keeps its hook: it has no line to take;
//! 2. **tap**: a pin of a net of three or more and the one pin of that net that faces the other way (a path's drop to
//!    its branches, a return's rise from them), where the second already stands beyond the first (on a fluid page
//!    wherever it stands: rule F1 tells the supply from the work). The branch nearer across than every other belongs
//!    in line, the others keep their column. A unit in a row of three mates or more takes no tap: it stays in its row;
//! 3. **corner**: the two pins of a net of two, at a right angle: one bend;
//! 4. **level**: the two pins of a net of two on the same side: out, across and back;
//! 5. **mate**: no wire. Two units of one shape (the body of their largest part) that stand level side by side or in
//!    line under one another, a step off at most, the nearest within twice [`ROW_PULL`]: a strip's terminals, cards
//!    side by side, like devices in a row;
//! 6. **overruled**: a level pair of a unit more of whose mates face that host than are wired to it out and back; and
//!    a facing pair or a tap of two units of one row that runs across the wire, where one of them would have to leave
//!    the row for it (two cards side by side, wired from the upper edge of one to the lower edge of the other).
//!
//! A mark set on a pin of another part asks for nothing where its arrow lies along that part's body (a potential's
//! mark on a controller's supply pin: it has no way out on its own side), and a pin with a mark set on it asks for no
//! corner: the potential arrives there, and neither part steps for the wire from it.
//!
//! # The trees
//!
//! The wired relations are taken strongest first (of one strength, the one nearest in line first), each joining two
//! units that no stronger relation joined yet: a spanning forest, with every fixed unit as ground. Then the mates join
//! those trees, the relation nearest the heaviest unit of the heavier tree first, and only where one of the two is the
//! heaviest of its tree or fixed (a terminal with its device joins its strip, not the row of devices; two marks of
//! two strips are not joined; a unit that hangs on its mate is the top of no tree any more, so a spare terminal
//! between the terminals of two cards does not join the cards); then the overruled pairs.
//!
//! A tree hangs from its fixed units. A tree without one hangs from its **anchor**: of its heaviest units (4 per wired
//! pin and the bodies' area in square steps over 16; those of at least three quarters of the heaviest's weight,
//! [`ANCHORS`] at most) the one from which the tree is set with room for every unit, with every step a rail demands,
//! and with the least weight moved: a part out of place moves, not the circuit round it. A unit's relation towards the
//! root is its **lead**; a relation left out of the tree is kept for the stacks, which nest by it.
//!
//! # Rails
//!
//! A net of three pins or more whose pins, but marks', all face one side up or down is a rail. The units it taps
//! belong on the row most of them stand on, where they stand within [`ROW_PULL`] of it; the step is a demand where the
//! unit stands beyond the line of the rail's junctions. A mark of the rail belongs on that line.
//!
//! # Setting the places
//!
//! The trees are set one after another, the heaviest first, each as if the units of the trees after it will step
//! aside. A unit that then kept its place where a moved unit came to stand is in the way from the start, and the
//! trees are set again ([`PASSES`] times at most).
//!
//! A card that is the root of its tree steps along its edge first, clear of its supply's way and of another card's
//! columns ([`CardSteps`]): the judge tries the arrangement with that step, with a part of it and without it.
//!
//! A tree is set from its root where it stands. Where a unit of it then has no room (it would leave the drawing area
//! or stand on a unit that is set) the root steps by whole steps, the least first and [`REACH`] at most each way, until
//! the whole tree has room; the roots of its shape that stood level with it take the same step, so that a row of cards
//! stays a row. Without such a step the root stays, and what has no room goes back to where it stood.
//!
//! From the root outward, rank by rank (the root, then the units set beside one another by one step of the method).
//! The mates that hang on a rank join it first: each takes its host's step, stands level with it, takes the pitch the
//! row now has where the row had one, and what a rail asks of it.
//!
//! - **A row** (facing and tap leads on one side of a rank). The members that stand more than [`ROW_PULL`] beyond the
//!   nearest wait until what hangs on the near ones is set: a device wired straight to a card does not take the place
//!   of the row over the terminals.
//!   - *Across*, a member wants its pin on its host pin's line. It takes the line where the straight way between the
//!     two pins is free of other parts, and where that does not carry it past a member that stays, nor past a unit
//!     of its row that has no line of its own to take or further from such a unit than it stood by more than
//!     [`ROW_PULL`]. A unit of the row with a line of its own binds nobody: the terminals of one strip that are wired
//!     to two cards each stand at their own card's pin.
//!   - *Along*, a member keeps its place when it stands at least [`CHAIN_GAP_MIN`] beyond its host pin and clear of its
//!     host (one its host left more than a row's pull behind keeps the distance it had; a keeper one step off the
//!     distance most keepers of its shape keep takes that). Else it takes the row's line: [`CHAIN_GAP`], a step more
//!     than the tracks its stepping wires need, or the line the keepers stand on within [`ROW_PULL`] of that. A member
//!     a rail taps takes the rail's row where its lead leaves it the room. Where a member that takes the row's line
//!     would stand on a unit that is set, those members step back together to the first line that is clear, no
//!     nearer than [`CHAIN_GAP_MIN`] and their tracks (sensors under a box at the head).
//!   - *Packed*: the members whose extents meet along the side stand in the order of their host pins with [`ROW_GAP`]
//!     between: by their bodies where the text placer then sets every text of the row clear and within a step of
//!     its part (a strip's terminals two steps apart), else by their footprints (bodies and texts at their first
//!     places). Of
//!     three ways to pack them (by the least total move from their wishes: neighbours that would overlap are pooled
//!     at their median; from the first member onward; from the last member back) the one is taken whose stepping wires
//!     cross the fewest of the wires that pass the row (those that leave the rank on that side and do not end at a
//!     pin of the row that faces the rank: a supply's riser beside a row of sensors), and none that sets a member
//!     outside the drawing area while another does not. Members that keep their places and distances stay.
//!   - A member that then stands on a unit already set steps further out on its line where the line is free that far
//!     (two parts wider than their pins' pitch stand staggered, both in line); else it stands where it stood across,
//!     and steps further out from there.
//! - **A stack** (corner leads on one side of a rank that turn one way): each member is set first with what hangs on
//!   it, as one block. A block heavier than the unit it hangs on keeps its place (a card with its channels does not
//!   step for the corner of one supply wire). The blocks are taken in nesting order (the drop nearest the side they stand on turns first) and
//!   each keeps its place when every one of its wires has its corner (the drop at least [`CHAIN_GAP_MIN`] long, its own
//!   pin at least [`MARK_LEAD`] before the drop), it lies beyond the blocks before it and stands clear; else it takes
//!   the nearest such place: in the column most single marks of the stack stand in, or in the column a mark's lead
//!   before the nearest drop. The pitch is [`MARK_PITCH`], or the next larger one at which the text placer finds
//!   every text of the stack a place as clear as it had. A stack that leaves the drawing area slides back towards its
//!   hosts.
//! - **Level followers.** One whose wire has no way out and back (it stands under or over its host) is set beside the
//!   host, pins level, the pin nearest that side innermost, where that side or the other has the room. One that stands
//!   neither beside its host nor near beyond its pins takes the row beyond them: two steps clear of the host and off
//!   the pin's line by its own width, so that the wire passes it, on the side where the host has the fewer pins of
//!   that edge. On a bus page two stations whose ports face one side stand level (rule B2). A level follower that
//!   keeps its place joins the next rank: what hangs on a strip of terminals is one row.
//! - **On a fluid page** (rule F1) what hangs on a valve's unit with a part of the supply steps below every valve, and
//!   what hangs there with an actuator steps above that unit's valves, with the blocks above it, so that a stack
//!   keeps its order.
//! - **A strip opens as one** (FT-2110). A strip of terminals over or under a card, two or more of them in line on the
//!   card's pins, is set anew where the arrangement opens it ([`Sketch::open`]; the judge tries it where the strip's
//!   other terminals held a channel's terminal off its column, or the rows set a row of terminals out of its order):
//!   its other terminals bind nobody while the rows are set, and none of them is set beside or round a corner from its
//!   host. Then [`Sketch::open_strip`] sets the whole strip in the order it stands in: the longest run of channels'
//!   terminals in their channels' order with room between takes its columns, every other terminal keeps its place in
//!   the order. A strip that opened goes or stays as one ([`Sketch::with_strips`]), and no row of terminals leaves its
//!   order in any plan ([`Sketch::order_kept`]).
//!
//! Last, a moved unit that stands outside the drawing area or on another unit goes back to where it stood, with what
//! hangs on it.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::consts::{
    ANCHORS, BODY_GAP, CHAIN_GAP, CHAIN_GAP_MIN, LABEL_CLEAR, MARK_LEAD, MARK_PITCH, PASSES, REACH, ROW_GAP,
    ROW_PULL, STUB, TICKS_PER_STEP, TRACK_PITCH, TRACK_PITCH_NARROW, TRUNK_GAP,
};
use crate::labels::first_places;
use crate::measure::measure;
use crate::model::{
    DraftPage, End, Junction, JunctionId, LabelKind, Owner, PageKind, PartId, PinId, Pt, Rect, Role, Side,
    Tick, Wire, WireId,
};
use crate::route::Scope;
use crate::rules::{Wiring, across, beyond, opposite};

/// How two related pins stand to each other, strongest first (module documentation).
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

/// The arrangement being made (module documentation).
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
    /// channels' terminals while the rows are set, and the strip is set anew afterwards (FT-2110).
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

/// Rounds `v` ticks up (away from zero) to a whole step.
fn whole_up(v: Tick) -> Tick {
    let s = TICKS_PER_STEP;
    if v >= 0 {
        (v + s - 1).div_euclid(s) * s
    } else {
        -((-v + s - 1).div_euclid(s) * s)
    }
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
    beyond(Pt::default(), p, s)
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
    } else if a == opposite(b) {
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
        // Units by number, the parts inside an outline drawn round other parts (but an actuator or a pressure source,
        // which is no part of a valve terminal: one set inside its outline is moved out as any other, FT-2097), and a
        // part on another's pin.
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
                    let member = !matches!(p.role, Role::Actuator | Role::Source);
                    if i != j && member && f.bounds().holds(p.bounds()) {
                        join(&mut up, i, j);
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
                            join(&mut up, i, j);
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
            let r = root(&mut up, i);
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
            unit.weight += 4 * pins + area / 16;
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
        let first = (CHAIN_GAP_MIN - beyond(h, f, s)).max(0);
        let area = self.w.page.area.grown(-TICKS_PER_STEP / 2);
        let reach = area.w().max(area.h()) / TICKS_PER_STEP;
        let host = self.bodies(l.host);
        let Some(bodies) = (0..=reach)
            .map(|k| {
                let by =
                    across_vec(s, across(h, s) - across(f, s)).plus(along_vec(s, first + k * TICKS_PER_STEP));
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

    /// The first ID of a unit's parts: what its order falls back on.
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

    /// The relations of the page (module documentation), in a fixed order.
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
                    (lo..=hi).contains(&across(at, s)) && span_along(b, s).1 >= out(at, s) + TICKS_PER_STEP
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
                let (one, others) = match (facing(first), facing(opposite(first))) {
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
                            && (anywhere || beyond(at, self.pt0(*p), s) >= CHAIN_GAP_MIN)
                    })
                    .collect();
                // The nearest across, where one is nearer than every other.
                let skew = |p: &PinId| (across(self.pt0(*p), s) - across(at, s)).abs();
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
                        (0..=2 * ROW_PULL).contains(&gap).then_some((gap, self.id(v), v))
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
        let mut row: Vec<usize> = (0..m).collect();
        for (u, v) in &mates {
            let (a, b) = (row[*u], row[*v]);
            for r in &mut row {
                if *r == a.max(b) {
                    *r = a.min(b);
                }
            }
        }
        // A wire between two units of one row that runs across it asks nothing where one of them would have to leave
        // the row for it (two cards side by side, a pin on the upper edge of one wired to a pin on the lower edge of
        // the other): the row stays, and the wire is the router's.
        let lines = self.lines(&mates);
        for r in &mut out {
            if matches!(r.kind, Kind::Facing | Kind::Tap)
                && self.side(r.pa).is_some_and(|s| {
                    let line = &lines[usize::from(!s.vertical())];
                    line[r.a] == line[r.b] && beyond(self.pt0(r.pa), self.pt0(r.pb), s) < CHAIN_GAP_MIN
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

    /// The tree (module documentation): the leads, the relations left out, the roots.
    fn tree(&mut self) {
        let m = self.units.len();
        let mut rels = self.relations();
        let skew = |sk: &Self, r: &Rel| match (r.kind, sk.side(r.pa)) {
            (Kind::Facing | Kind::Tap, Some(s)) => (across(sk.pt0(r.pa), s) - across(sk.pt0(r.pb), s)).abs(),
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
        let mut up: Vec<usize> = (0..=m).collect();
        fn root(up: &mut [usize], mut i: usize) -> usize {
            while up[i] != i {
                up[i] = up[up[i]];
                i = up[i];
            }
            i
        }
        for u in 0..m {
            if self.units[u].fixed {
                let (a, b) = (root(&mut up, u), root(&mut up, m));
                up[a.max(b)] = a.min(b);
            }
        }
        let mut edges: Vec<Vec<Rel>> = vec![Vec::new(); m];
        let (wired, rest): (Vec<Rel>, Vec<Rel>) = rels.into_iter().partition(|r| r.kind < Kind::Mate);
        let (mut mates, overruled): (Vec<Rel>, Vec<Rel>) =
            rest.into_iter().partition(|r| r.kind == Kind::Mate);
        let pairs: Vec<(usize, usize)> = mates.iter().map(|r| (r.a, r.b)).collect();
        // The trees the wires make.
        for r in wired {
            let (a, b) = (root(&mut up, r.a), root(&mut up, r.b));
            if a == b {
                self.extra.push(r);
                continue;
            }
            up[a.max(b)] = a.min(b);
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
            let c = root(&mut up, u);
            *weight.entry(c).or_default() += self.units[u].weight.max(1);
            let key = self.key(u);
            let old = heaviest.entry(c).or_insert(key);
            *old = (*old).min(key);
        }
        let mut top: Vec<bool> = (0..m)
            .map(|u| {
                self.units[u].fixed
                    || heaviest
                        .get(&root(&mut up, u))
                        .is_some_and(|k| k.0 == -self.units[u].weight)
            })
            .collect();
        // Each unit's depth in its tree, from the tree's heaviest unit.
        let mut depth: Vec<usize> = vec![usize::MAX; m];
        let mut queue: VecDeque<usize> = (0..m)
            .filter(|u| heaviest.get(&root(&mut up, *u)) == Some(&self.key(*u)))
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
            .map(|u| weight.get(&root(&mut up, u)).copied().unwrap_or(0))
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
            let (a, b) = (root(&mut up, r.a), root(&mut up, r.b));
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
                let (own, other) = if root(&mut up, u) == a { (ka, kb) } else { (kb, ka) };
                if !self.units[u].fixed && own > other {
                    top[u] = false;
                }
            }
            heaviest.insert(a.min(b), ka.min(kb).unwrap_or_else(|| self.key(r.a)));
            up[a.max(b)] = a.min(b);
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
            let mut of: Vec<usize> = (0..self.units.len()).collect();
            for (a, b) in pairs {
                if self.mates(*a, *b, down) {
                    let (x, y) = (of[*a], of[*b]);
                    for o in &mut of {
                        if *o == x.max(y) {
                            *o = x.min(y);
                        }
                    }
                }
            }
            of
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
    /// measures along the side.
    fn rails(&self) -> Vec<Rail> {
        let mark = |p: &PinId| matches!(self.w.parts[self.w.pins[p].0].role, Role::MarkIn | Role::MarkOut);
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

    /// Sets every unit's place (module documentation).
    pub fn lay(&mut self) {
        let m = self.units.len();
        self.steps = self.rail_steps();
        self.loose = self.loose();
        // A tree is set as if the units of the trees after it will step aside. One that then kept its place where a
        // moved unit came to stand was counted on in vain: it is in the way from the start, and the trees are set
        // again ([`PASSES`] times at most).
        let mut stay: BTreeSet<usize> = BTreeSet::new();
        for _ in 0..=PASSES {
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

    /// The room for the rings of the wires round a body (§3.5, Rings, FT-2085): a unit whose wires leave its pins on one
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
                    let back = opposite(s);
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
                            let under = lo <= across(qa, s) && across(qa, s) <= hi && out(qa, back) > edge;
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
                        step = Some((s, whole_up((depth + 1) * TICKS_PER_STEP + STUB - g)));
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

    /// The strips that open over or under a card (doc 06a §4.4, FT-2110), each with its units in the order they stand
    /// and its card: a strip is the units joined through mates side by side, three or more and none fixed, of which two
    /// or more hang in line on pins of one card that face up or down (the unit most of them hang on so).
    fn opening(&self) -> Vec<(Vec<usize>, usize)> {
        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for u in 0..self.units.len() {
            groups.entry(self.strips[0][u]).or_default().push(u);
        }
        let mut out = Vec::new();
        for mut members in groups.into_values() {
            if members.len() < 3 || members.iter().any(|u| self.units[*u].fixed) {
                continue;
            }
            let mut count: BTreeMap<usize, usize> = BTreeMap::new();
            for &u in &members {
                if let Some(l) = self.in_column(u) {
                    *count.entry(l.host).or_default() += 1;
                }
            }
            let Some((card, k)) = count.into_iter().max_by_key(|(h, k)| (*k, Reverse(*h))) else {
                continue;
            };
            if k < 2 {
                continue;
            }
            members.sort_by_key(|u| (self.shape(*u).map_or(0, |r| r.x0), *u));
            out.push((members, card));
        }
        out
    }

    /// The lead of a unit that hangs in line on a pin facing up or down: a channel's terminal under or over its card.
    fn in_column(&self, u: usize) -> Option<Lead> {
        self.lead[u].filter(|l| {
            l.inline
                && matches!(l.kind, Kind::Facing | Kind::Tap)
                && self.side(l.h).is_some_and(Side::vertical)
        })
    }

    /// `f` and what hangs on it in line (facing it or tapping it; with `corners` round a corner too), but the units of
    /// `stop` and what hangs on them: what moves with it. What hangs beside it (a lamp wired out and back to a terminal)
    /// stands where it is.
    fn in_line(&self, f: usize, stop: &[usize], corners: bool) -> Vec<usize> {
        let mut out = vec![f];
        let mut k = 0;
        while k < out.len() {
            let h = out[k];
            out.extend((0..self.units.len()).filter(|u| {
                !stop.contains(u)
                    && self.lead[*u].is_some_and(|l| {
                        l.host == h
                            && (matches!(l.kind, Kind::Facing | Kind::Tap | Kind::Mate)
                                || (corners && l.kind == Kind::Corner))
                    })
            }));
            k += 1;
        }
        out
    }

    /// The units an opening strip sets anew ([`Sketch::open_strips`]): its terminals that are no channel's. While the
    /// rows are set they stand in no member's way and bind nobody.
    fn loose(&self) -> BTreeSet<usize> {
        let mut out = BTreeSet::new();
        for (members, card) in self.opening() {
            out.extend(
                members
                    .iter()
                    .copied()
                    .filter(|u| self.in_column(*u).is_none_or(|l| l.host != card)),
            );
        }
        out
    }

    /// Each strip over or under a card opens as one, in its own order (doc 06a §4.4, FT-2110), by
    /// [`Sketch::open_strip`]; where that cannot hold, the strip stays as the rows set it.
    fn open_strips(&mut self) {
        self.opened.clear();
        self.held.clear();
        for (members, card) in self.opening() {
            let start = self.d.clone();
            match self.open_strip(&members, card) {
                Ok(()) => self.opened.push(members),
                Err(why) => {
                    self.d = start;
                    self.held.push((members[0], why));
                }
            }
        }
    }

    /// The strip of `members` (in the order they stand) under or over `card` set anew, in that order (doc 06a §4.4): of
    /// its channels' terminals the longest run that stands in the order of its channels' columns, with room between
    /// each two for the terminals between them (their bodies a row's gap apart), takes its columns (of runs as long,
    /// the first);
    /// every other terminal keeps its place in the order: between two of the run evenly, before the first or after the
    /// last at the strip's pitch, beyond what hangs on them in line. No terminal passes another. All level with the first terminal of the run; what hangs
    /// on a terminal in line moves with it (round a corner too on a channel's terminal). What holds it where no two channels' terminals can take their columns
    /// (their channels in another order, or too little room between their columns), or where a unit that moves would
    /// stand on another or outside the drawing area.
    fn open_strip(&mut self, members: &[usize], card: usize) -> Result<(), Held> {
        let left = |sk: &Self, u: usize| sk.shape(u).map_or(0, |r| r.x0);
        let Some(pitch) = members
            .windows(2)
            .map(|w| left(self, w[1]) - left(self, w[0]))
            .filter(|g| *g > 0)
            .min()
        else {
            return Err(Held::Room);
        };
        // Where each channel's terminal would stand: its pin on its channel's line.
        let column: Vec<Option<Tick>> = members
            .iter()
            .map(|&u| {
                let l = self.in_column(u).filter(|l| l.host == card)?;
                Some(left(self, u) + self.pt(l.h).x - self.pt0(l.f).x)
            })
            .collect();
        let chans: Vec<usize> = (0..members.len()).filter(|i| column[*i].is_some()).collect();
        let col = |i: usize| column[i].unwrap_or(0);
        let steps = |n: usize| Tick::try_from(n).unwrap_or(Tick::MAX / 2);
        // The least pitch between two terminals: their bodies a row's gap apart (two steps for a terminal), or the
        // strip's own where that is less.
        let width = members
            .iter()
            .filter_map(|u| self.shape(*u))
            .map(|r| r.w())
            .max()
            .unwrap_or(0);
        let least = pitch.min(whole_up(width + ROW_GAP));
        // The longest run in order (`room`: with room between each two), each as its last member's best length and
        // the member before it.
        let run = |room: bool| -> Vec<usize> {
            let mut best: Vec<(usize, Option<usize>)> = vec![(1, None); chans.len()];
            for j in 0..chans.len() {
                for i in 0..j {
                    let (a, b) = (chans[i], chans[j]);
                    let fits = col(b) - col(a) >= if room { steps(b - a) * least } else { 1 };
                    if fits && best[i].0 + 1 > best[j].0 {
                        best[j] = (best[i].0 + 1, Some(i));
                    }
                }
            }
            let Some(mut k) = (0..chans.len()).max_by_key(|k| (best[*k].0, Reverse(*k))) else {
                return Vec::new();
            };
            let mut out = vec![chans[k]];
            while let Some(i) = best[k].1 {
                out.push(chans[i]);
                k = i;
            }
            out.reverse();
            out
        };
        let anchors = run(true);
        if anchors.len() < 2 {
            return Err(if run(false).len() < 2 {
                Held::Order
            } else {
                Held::Room
            });
        }
        let (first, last) = (anchors[0], anchors[anchors.len() - 1]);
        // The terminals outside the run stand beyond it at the strip's pitch, and beyond what hangs on the run's outer
        // two in line (a button over the first with its tag at its left), so that their wires to the parts over the run
        // pass clear of those parts and their texts.
        let reach = |i: usize| -> Option<Rect> {
            self.in_line(members[i], members, true)
                .into_iter()
                .skip(1)
                .map(|v| self.hull(v))
                .reduce(Rect::union)
        };
        let near = [
            col(first) - reach(first).map_or(0, |r| whole_up((col(first) - r.x0).max(0))),
            col(last) + reach(last).map_or(0, |r| whole_up((r.x1 - col(last) - width).max(0))),
        ];
        let mut place: Vec<Tick> = vec![0; members.len()];
        for i in 0..members.len() {
            place[i] = if i < first {
                near[0] - pitch * steps(first - i)
            } else if i > last {
                near[1] + pitch * steps(i - last)
            } else {
                col(i)
            };
        }
        for w in anchors.windows(2) {
            let (a, b) = (w[0], w[1]);
            let gaps = steps(b - a);
            for i in a + 1..b {
                let even = (col(b) - col(a)) * steps(i - a) / gaps;
                place[i] = col(a) + even.div_euclid(TICKS_PER_STEP) * TICKS_PER_STEP;
            }
        }
        if !place.windows(2).all(|w| w[0] + least <= w[1]) {
            return Err(Held::Room);
        }
        // Level with the first terminal of the run, what hangs on each in line with it.
        let line = self.shape(members[first]).map_or(0, |r| r.y0) + self.d[members[first]].y;
        let mut moving: BTreeSet<usize> = members.iter().copied().collect();
        for (i, &u) in members.iter().enumerate() {
            let Some(r) = self.shape(u) else {
                return Err(Held::Room);
            };
            let by = Pt::new(place[i] - r.x0, line - r.y0).minus(self.d[u]);
            // (What hangs round a corner on a terminal that is no channel's, a mark at the sheet's edge, stays.)
            let hung: Vec<usize> = self
                .in_line(u, members, column[i].is_some())
                .into_iter()
                .skip(1)
                .collect();
            self.shift(&[u], by);
            self.shift(&hung, by);
            moving.extend(hung);
        }
        // In the drawing area, on no other unit.
        for u in &moving {
            if !self.inside(*u) {
                return Err(Held::Sheet);
            }
            if let Some(o) = (0..self.units.len()).find(|o| !moving.contains(o) && self.bodies_meet(*u, *o)) {
                return Err(Held::Unit(o));
            }
        }
        Ok(())
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

    /// Whether the bodies of `u` lie in the drawing area, a step inside its edge.
    fn inside(&self, u: usize) -> bool {
        let area = self.w.page.area.grown(-TICKS_PER_STEP / 2);
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

    /// Whether a card of the page would take a step along its edge ([`Sketch::card_step`]).
    pub fn cards_step(&self) -> bool {
        (0..self.units.len()).any(|u| self.card_step(u, CardSteps::Columns) != Pt::default())
    }

    /// The step a card takes along its edge as the root of its tree (§4.4), so that its channels keep clear of its
    /// own supply and of another card's channels. Its channel pins face up or down and its supply pins stand on its
    /// left side. (a) Every pin its supply is wired to that lies on the side its channels face (the controller's
    /// supply pin, a mark, the supply pin of a card at the other edge) stands left of the card, where the wire drops
    /// beside the first channel and crosses none. (b) Where the columns of its channels in use interleave with those of
    /// a card whose pins face its own and it has fewer in use, its used columns stand right of the other's, two steps
    /// clear, and a pitch of the strip more for each spare terminal of a strip between them ([`Sketch::spares_between`],
    /// [`CardSteps::Columns`] only). No step where the card would leave the drawing area.
    fn card_step(&self, u: usize, steps: CardSteps) -> Pt {
        if steps == CardSteps::None {
            return Pt::default();
        }
        let card = self.units[u]
            .parts
            .iter()
            .map(|i| self.w.parts[*i])
            .find(|p| p.role == Role::Card);
        let Some(card) = card else {
            return Pt::default();
        };
        let facing = |s: Side| card.pins.iter().filter(|p| p.side == Some(s)).count();
        let s = if facing(Side::Up) >= facing(Side::Down) {
            Side::Up
        } else {
            Side::Down
        };
        if self.units[u].fixed || facing(s) < 2 || facing(opposite(s)) > 0 {
            return Pt::default();
        }
        let body = card.bounds();
        let ahead = |p: Pt| {
            if s == Side::Up {
                p.y < body.y0
            } else {
                p.y > body.y1
            }
        };
        let mut left = body.x0;
        for pin in card.pins.iter().filter(|p| p.side == Some(Side::Left)) {
            for net in self.w.nets.iter().filter(|n| n.pins.contains(&pin.id)) {
                for q in &net.pins {
                    let (i, at, side) = self.w.pins[q];
                    if self.unit_of[i] == u || !ahead(at) {
                        continue;
                    }
                    let way = match side {
                        Some(Side::Left) => at.x - STUB,
                        Some(Side::Right) => at.x + STUB,
                        _ => at.x,
                    };
                    left = left.max(way + TICKS_PER_STEP);
                }
            }
        }
        let mut by = whole_up(left - body.x0);
        // The columns of a card's channels in use on `side`.
        let used = |part: &crate::model::Part, side: Side| -> Vec<Tick> {
            let mut xs: Vec<Tick> = part
                .pins
                .iter()
                .filter(|p| p.side == Some(side) && self.wired.contains(&p.id))
                .map(|p| part.at.x + p.at.x)
                .collect();
            xs.sort_unstable();
            xs
        };
        let own = used(card, s);
        let other = self
            .w
            .parts
            .iter()
            .filter(|p| p.role == Role::Card && p.id != card.id && ahead(p.at))
            .map(|p| (p, used(p, opposite(s))))
            .find(|(_, xs)| !xs.is_empty());
        if let (Some((part, xs)), CardSteps::Columns, Some(&lo), Some(&hi)) =
            (other, steps, own.first(), own.last())
        {
            // Where the used columns of the two interleave, the card with fewer channels in use (of two as many, the
            // one further right) steps on until its own stand right of the other's, two steps clear: the parts of the
            // two cards' channels do not stand among each other, and a strip between them keeps its order.
            let (them_lo, them_hi) = (xs[0], xs[xs.len() - 1]);
            let fewer = (own.len(), Reverse(card.at.x), Reverse(card.id))
                < (xs.len(), Reverse(part.at.x), Reverse(part.id));
            if fewer && lo + by <= them_hi && them_lo <= hi + by {
                let spares = self.spares_between((*part, them_hi), (card, lo));
                by = by.max(whole_up(them_hi + 2 * TICKS_PER_STEP + spares - lo));
            }
        }
        let room = self.w.page.area.x1 - TICKS_PER_STEP - body.x1;
        if by > room {
            return Pt::default();
        }
        Pt::new(by, 0)
    }

    /// The room the spare terminals of a strip between two cards ask for when their columns step apart
    /// ([`Sketch::card_step`], FT-2097): where the terminal wired to the pin of card `a` at column `xa` and the one
    /// wired to the pin of card `b` at column `xb` are mates of one row (a strip), each mate between them that is wired
    /// to nothing (a spare, which keeps its place in the strip) takes the row's pitch as it stood; none otherwise.
    fn spares_between(
        &self,
        (a, xa): (&crate::model::Part, Tick),
        (b, xb): (&crate::model::Part, Tick),
    ) -> Tick {
        let terminal = |card: &crate::model::Part, x: Tick| -> Option<usize> {
            let own = self.unit_of_id(card.id)?;
            let pin = card
                .pins
                .iter()
                .find(|p| card.at.x + p.at.x == x && self.wired.contains(&p.id))?;
            let net = self.w.nets.iter().find(|n| n.pins.contains(&pin.id))?;
            net.pins.iter().map(|q| self.unit_of_pin(*q)).find(|v| *v != own)
        };
        let centre = |u: usize| self.shape(u).map(|r| (r.x0 + r.x1) / 2);
        let (Some(ta), Some(tb)) = (terminal(a, xa), terminal(b, xb)) else {
            return 0;
        };
        let (Some(ca), Some(cb)) = (centre(ta), centre(tb)) else {
            return 0;
        };
        if ta == tb || !self.mates(ta, tb, true) {
            return 0;
        }
        let (lo, hi) = (ca.min(cb), ca.max(cb));
        let mut row = vec![ca, cb];
        let mut spares = 0;
        for m in (0..self.units.len()).filter(|m| *m != ta && *m != tb && self.mates(ta, *m, true)) {
            let Some(c) = centre(m).filter(|c| lo < *c && *c < hi) else {
                continue;
            };
            row.push(c);
            let wired = self.units[m]
                .parts
                .iter()
                .any(|i| self.w.parts[*i].pins.iter().any(|p| self.wired.contains(&p.id)));
            spares += Tick::from(!wired);
        }
        row.sort_unstable();
        let pitch = row
            .windows(2)
            .map(|w| w[1] - w[0])
            .filter(|g| *g > 0)
            .min()
            .unwrap_or(0);
        spares * pitch
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
        units.retain(|u| 4 * self.units[*u].weight >= 3 * most);
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
                    (whole_up(down), TICKS_PER_STEP)
                } else if rise > 0 {
                    units.extend(blocks[k + 1..].iter().flat_map(|o| self.subtree(*o)));
                    (-whole_up(rise), -TICKS_PER_STEP)
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
                let dist = |sk: &Self, m: usize| sk.lead[m].map_or(0, |l| beyond(sk.pt(l.h), sk.pt(l.f), s));
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

    /// Sets each of `members` with what hangs on it, by itself: a block that the caller then sets as one.
    fn blocks(&mut self, members: &[usize]) {
        for m in members {
            let units: BTreeSet<usize> = self.subtree(*m).into_iter().collect();
            let outer = self.within.replace(units.clone());
            self.placed[*m] = true;
            self.lay_from(*m);
            self.within = outer;
            for u in units {
                self.placed[u] = false;
            }
        }
    }

    /// The members of a row whose extents meet along side `s` packed across in their order with [`ROW_GAP`] between:
    /// by their bodies (`tight`) or their footprints. Of three ways to pack them (by the least total move from where
    /// they stand, their wishes; the first at its wish and the others beyond it; the last at its wish and the others
    /// before it) the one whose stepping wires cross the fewest of `risers` is taken: the wires that leave the hosts'
    /// rank on this side beside the row's own (a strip's supply terminal beside the terminals of a row of sensors).
    fn pack(&mut self, s: Side, items: &[(usize, Lead)], wished: &[Tick], tight: bool, risers: &[Tick]) {
        let extent = |sk: &Self, m: usize| {
            if tight {
                sk.bodies(m).into_iter().reduce(Rect::union).unwrap_or_default()
            } else {
                sk.hull(m)
            }
        };
        let n = items.len();
        let mut group: Vec<usize> = (0..n).collect();
        let spans: Vec<(Tick, Tick)> = items
            .iter()
            .map(|(m, _)| span_along(extent(self, *m), s))
            .collect();
        for i in 0..n {
            for j in i + 1..n {
                if spans[i].0 < spans[j].1 + ROW_GAP && spans[j].0 < spans[i].1 + ROW_GAP {
                    let (a, b) = (group[i], group[j]);
                    for g in &mut group {
                        if *g == a.max(b) {
                            *g = a.min(b);
                        }
                    }
                }
            }
        }
        let mut names: Vec<usize> = group.clone();
        names.sort_unstable();
        names.dedup();
        for g in names {
            let of: Vec<usize> = (0..n).filter(|k| group[*k] == g).collect();
            let edges: Vec<(Tick, Tick)> = of
                .iter()
                .map(|k| span_across(extent(self, items[*k].0), s))
                .collect();
            let mut offset: Vec<Tick> = vec![0];
            for k in 1..of.len() {
                let need = whole_up(edges[k - 1].1 + ROW_GAP - edges[k].0);
                offset.push(offset[k - 1] + need);
            }
            let want: Vec<Tick> = offset.iter().map(|o| -o).collect();
            let mut middle: Vec<Tick> = rising(&want)
                .into_iter()
                .enumerate()
                .map(|(k, v)| v + offset[k])
                .collect();
            // (Stepped back into the drawing area as one where an end of it leaves it.)
            let (first, last) = span_across(self.w.page.area.grown(-TICKS_PER_STEP / 2), s);
            let under = (0..of.len())
                .map(|k| first - (edges[k].0 + middle[k]))
                .max()
                .unwrap_or(0);
            let over = (0..of.len())
                .map(|k| edges[k].1 + middle[k] - last)
                .max()
                .unwrap_or(0);
            if under > 0 && over + whole_up(under) <= 0 {
                middle.iter_mut().for_each(|by| *by += whole_up(under));
            } else if over > 0 && under + whole_up(over) <= 0 {
                middle.iter_mut().for_each(|by| *by -= whole_up(over));
            }
            // Members that keep their places and their distances stay: none steps aside for a wire.
            if middle.iter().all(|by| *by == 0) && of.iter().all(|k| wished[*k] == 0) {
                continue;
            }
            // A member's bodies across, and whether they would stand in a riser's way (within a step of it) when the
            // member is stepped by `by`.
            let body: Vec<(Tick, Tick)> = of
                .iter()
                .map(|k| {
                    let all = self.bodies(items[*k].0).into_iter().reduce(Rect::union);
                    span_across(all.unwrap_or_default(), s)
                })
                .collect();
            let in_way = |k: usize, by: Tick| {
                risers
                    .iter()
                    .any(|r| body[k].0 + by - BODY_GAP < *r && *r < body[k].1 + by + BODY_GAP)
            };
            let reach = 2 * ROW_PULL;
            let mut forward: Vec<Tick> = vec![0; of.len()];
            for k in 0..of.len() {
                if k > 0 {
                    forward[k] = (forward[k - 1] + offset[k] - offset[k - 1]).max(0);
                }
                let free = (0..=reach / TICKS_PER_STEP)
                    .map(|n| forward[k] + n * TICKS_PER_STEP)
                    .find(|by| !in_way(k, *by));
                forward[k] = free.unwrap_or(forward[k]);
            }
            let mut backward: Vec<Tick> = vec![0; of.len()];
            for k in (0..of.len()).rev() {
                if k + 1 < of.len() {
                    backward[k] = (backward[k + 1] - (offset[k + 1] - offset[k])).min(0);
                }
                let free = (0..=reach / TICKS_PER_STEP)
                    .map(|n| backward[k] - n * TICKS_PER_STEP)
                    .find(|by| !in_way(k, *by));
                backward[k] = free.unwrap_or(backward[k]);
            }
            // The risers each way of packing would have its stepping wires cross, or its members stand in the way of.
            let crossed = |sk: &Self, by: &[Tick]| -> usize {
                of.iter()
                    .zip(by)
                    .enumerate()
                    .map(|(n, (k, b))| {
                        let l = items[*k].1;
                        let (h, f) = (across(sk.pt(l.h), s), across(sk.pt(l.f), s) + b);
                        risers.iter().filter(|r| h.min(f) < **r && **r < h.max(f)).count()
                            + usize::from(in_way(n, *b))
                    })
                    .sum()
            };
            // (A way that sets a member outside the drawing area comes last.)
            let outside = |by: &[Tick]| -> usize {
                (0..of.len())
                    .filter(|k| edges[*k].0 + by[*k] < first || edges[*k].1 + by[*k] > last)
                    .count()
            };
            let ways = [middle, forward, backward];
            let best = (0..ways.len())
                .min_by_key(|w| (outside(&ways[*w]), crossed(self, &ways[*w]), *w))
                .unwrap_or(0);
            for (k, by) in ways[best].iter().enumerate() {
                if *by != 0 {
                    let m = items[of[k]].0;
                    self.d[m] = self.d[m].plus(across_vec(s, *by));
                }
            }
        }
    }

    /// Whether the straight way between two points keeps clear of every unit but `but`: no body within a step of it.
    fn line_free(&self, a: Pt, b: Pt, but: &[usize]) -> bool {
        let line = Rect::new(a, b).grown(1);
        !(0..self.units.len()).any(|o| {
            !but.contains(&o)
                && self
                    .bodies(o)
                    .iter()
                    .any(|body| body.grown(BODY_GAP - 2).overlaps(line))
        })
    }

    /// A row of followers on side `s` of their hosts, which stand in the rank `hosts` (module documentation).
    fn row(&mut self, hosts: &[usize], s: Side, members: &[usize]) {
        let mut items: Vec<(usize, Lead)> = members
            .iter()
            .filter_map(|m| self.lead[*m].map(|l| (*m, l)))
            .collect();
        items.sort_by_key(|(m, l)| (across(self.pt(l.h), s), across(self.pt(l.f), s), self.id(*m)));
        // The wires that leave the rank on this side and do not end at a pin of the row that faces the rank (a second
        // pole of a member is the row's own; a supply that passes the row to its far pins is not), but the rank's own
        // leads.
        let mut rank: Vec<usize> = hosts
            .iter()
            .copied()
            .chain(items.iter().map(|(_, l)| l.host))
            .collect();
        rank.sort_unstable();
        rank.dedup();
        let mine: BTreeSet<usize> = items.iter().flat_map(|(m, _)| self.subtree(*m)).collect();
        let own: BTreeSet<PinId> = self
            .w
            .nets
            .iter()
            .filter(|n| {
                n.pins
                    .iter()
                    .any(|p| mine.contains(&self.unit_of_pin(*p)) && self.side(*p) == Some(opposite(s)))
            })
            .flat_map(|n| n.pins.iter().copied())
            .chain(rank.iter().filter_map(|u| self.lead[*u].map(|l| l.f)))
            .collect();
        let risers: Vec<Tick> = rank
            .iter()
            .flat_map(|u| self.units[*u].parts.iter())
            .flat_map(|i| self.w.parts[*i].pins.iter())
            .filter(|pin| pin.side == Some(s) && self.wired.contains(&pin.id) && !own.contains(&pin.id))
            // (Of the units that stand in the hosts' line.)
            .filter(|pin| {
                items
                    .iter()
                    .any(|(_, l)| beyond(self.pt(l.h), self.pt(pin.id), s).abs() <= ROW_PULL)
            })
            .map(|pin| across(self.pt(pin.id), s))
            .collect();
        // Across: in line, where the straight way between the two pins is then free (a part of another circuit on it
        // leaves the member where it stands).
        let mut wished: Vec<Tick> = Vec::with_capacity(items.len());
        for (m, l) in &items {
            let by = if l.inline {
                across(self.pt(l.h), s) - across(self.pt(l.f), s)
            } else {
                0
            };
            self.d[*m] = self.d[*m].plus(across_vec(s, by));
            wished.push(by);
        }
        // The units of the members' strips that are not of the row, with what hangs on them, and the terminals an
        // opening strip sets anew ([`Sketch::loose`]): where the strip opens (`open`, FT-2110) they are set with it
        // afterwards ([`Sketch::open_strips`]), so they stand in no member's way and bind nobody.
        let strip = &self.strips[usize::from(!s.vertical())];
        let here: BTreeSet<usize> = items.iter().map(|(m, _)| *m).collect();
        let mut others: Vec<usize> = (0..self.units.len())
            .filter(|u| !here.contains(u) && items.iter().any(|(m, _)| strip[*u] == strip[*m]))
            .flat_map(|u| self.subtree(u))
            .collect();
        others.extend(self.loose.iter().copied());
        for (k, (m, l)) in items.iter().enumerate() {
            let far = self.pt(l.f).plus(along_vec(
                s,
                (CHAIN_GAP - beyond(self.pt(l.h), self.pt(l.f), s)).max(0),
            ));
            // (The row's other members are packed clear of it afterwards.)
            let mut own = self.subtree(*m);
            own.push(l.host);
            own.extend(items.iter().flat_map(|(o, _)| self.subtree(*o)));
            if wished[k] != 0 && !self.line_free(self.pt(l.h), far, &own) {
                own.extend(&others);
                let strip_only = self.line_free(self.pt(l.h), far, &own);
                if self.open && strip_only {
                    continue;
                }
                self.bound |= strip_only;
                self.d[*m] = self.d[*m].plus(across_vec(s, -wished[k]));
                wished[k] = 0;
            }
        }
        // A member whose line would carry it past one that stays, where the two stood in the order of their pins, stays
        // too: it is not drawn up against the one that could not go. Nor does a member take a line that carries it
        // past a unit of its row that has no line of its own to take (it is fixed, or hangs beside its host), or
        // further from it than it stood by more than the row's pull: such a row is not torn. A unit of the row with a
        // line of its own (a strip's terminal that faces another card) binds nobody: each stands in its channel's
        // column (rule E4). Where the strip opens as one (`open`), none binds: the strip is set anew afterwards.
        let elsewhere = |sk: &Self, m: usize| -> Vec<usize> {
            (0..sk.units.len())
                .filter(|u| {
                    *u != m
                        && strip[*u] == strip[m]
                        && !here.contains(u)
                        && sk.lead[*u]
                            .is_none_or(|l| !matches!(l.kind, Kind::Mate | Kind::Facing | Kind::Tap))
                })
                .collect()
        };
        let place = |sk: &Self, u: usize, moved: bool| {
            let at = sk.shape(u).map_or(0, |r| span_across(r, s).0);
            if moved { at + across(sk.d[u], s) } else { at }
        };
        let binds = |sk: &Self, wished: &[Tick], k: usize| {
            let m = items[k].0;
            wished[k] != 0
                && elsewhere(sk, m).into_iter().any(|u| {
                    let (then, now) = (
                        place(sk, m, false) - place(sk, u, false),
                        place(sk, m, true) - place(sk, u, true),
                    );
                    then.signum() != now.signum() || now.abs() > then.abs() + ROW_PULL
                })
        };
        if !self.open && (0..items.len()).any(|k| binds(self, &wished, k)) {
            self.bound = true;
        }
        loop {
            let at = |sk: &Self, k: usize| across(sk.pt(items[k].1.f), s);
            let was = |sk: &Self, k: usize| across(sk.pt0(items[k].1.f), s);
            let past = (0..items.len()).find(|k| {
                wished[*k] != 0
                    && ((0..items.len()).any(|j| {
                        wished[j] == 0
                            && (*k < j) == (was(self, *k) < was(self, j))
                            && was(self, *k) != was(self, j)
                            && (*k < j) != (at(self, *k) < at(self, j))
                    }) || (!self.open && binds(self, &wished, *k)))
            });
            let Some(k) = past else { break };
            self.d[items[k].0] = self.d[items[k].0].plus(across_vec(s, -wished[k]));
            wished[k] = 0;
        }
        // Along: a keeper keeps its place; one that stood too near, or on the wrong side, takes the chain gap for now;
        // one its host left far behind keeps the distance it had.
        let dist = |sk: &Self, l: &Lead| beyond(sk.pt(l.h), sk.pt(l.f), s);
        let mut movers: Vec<bool> = Vec::with_capacity(items.len());
        for (m, l) in &items {
            let was = beyond(self.pt0(l.h), self.pt0(l.f), s);
            let now = dist(self, l);
            let (least, most) = (was.clamp(CHAIN_GAP_MIN, CHAIN_GAP), was.max(CHAIN_GAP) + ROW_PULL);
            let keeps = was >= CHAIN_GAP_MIN && now >= least && !self.meets(*m, l.host);
            if keeps && now > most {
                self.d[*m] = self.d[*m].plus(along_vec(s, was - now));
            } else if !keeps {
                self.d[*m] = self.d[*m].plus(along_vec(s, CHAIN_GAP - now));
            }
            movers.push(!keeps);
        }
        // A keeper a step off the distance that most keepers of its shape keep takes it: a row set down a step crooked
        // is levelled.
        let mut kept: BTreeMap<(Tick, Tick), BTreeMap<Tick, usize>> = BTreeMap::new();
        for (k, (m, l)) in items.iter().enumerate() {
            if let (false, Some(shape)) = (movers[k], self.shape(*m)) {
                *kept
                    .entry(shape.size())
                    .or_default()
                    .entry(dist(self, l))
                    .or_default() += 1;
            }
        }
        for (k, (m, l)) in items.iter().enumerate() {
            let Some(counts) = self.shape(*m).and_then(|x| kept.get(&x.size())) else {
                continue;
            };
            let now = dist(self, l);
            if let (false, Some((most, n))) = (movers[k], counts.iter().max_by_key(|(d, n)| (**n, **d)))
                && *n >= 2
                && (*most - now).abs() == TICKS_PER_STEP
            {
                self.d[*m] = self.d[*m].plus(along_vec(s, *most - now));
            }
        }
        // Across: packed by their bodies where the text placer then finds every text of the row a clear place (a
        // strip's terminals two steps apart with their names); else by their footprints, texts at their first places.
        let start = self.d.clone();
        self.pack(s, &items, &wished, true, &risers);
        // (Asked once for each way the row stands to its hosts.)
        let key: (Side, Vec<(usize, Pt)>) = (
            s,
            items
                .iter()
                .map(|(m, l)| (*m, self.d[*m].minus(self.d[l.host])))
                .collect(),
        );
        let tight = match self.tight.get(&key) {
            Some(t) => *t,
            None => {
                let mut all: Vec<usize> = items.iter().flat_map(|(m, l)| [*m, l.host]).collect();
                all.sort_unstable();
                all.dedup();
                let wires: Vec<(PinId, PinId)> = items.iter().map(|(_, l)| (l.h, l.f)).collect();
                // No text more on a text, and none more away from its part, than each unit has by itself (a mark set
                // on a pin has its texts on that pin's part): texts that lay on a neighbour's as the row stood do not
                // count for it, a row whose texts clash is spread.
                let now = self.text_clashes(&all, &wires);
                let own = all.iter().fold((0, 0), |(a, b), u| {
                    let (c, d) = self.text_clashes(&[*u], &[]);
                    (a + c, b + d)
                });
                let t = { now.0 <= own.0 && now.1 <= own.1 };
                self.tight.insert(key, t);
                t
            }
        };
        if !tight {
            self.d = start;
            self.pack(s, &items, &wished, false, &risers);
        }
        // Along: the row's line, with room for the tracks of its stepping wires.
        let lanes: Vec<(Tick, Tick)> = items
            .iter()
            .filter(|(_, l)| l.inline)
            .map(|(_, l)| (across(self.pt(l.h), s), across(self.pt(l.f), s)))
            .collect();
        let tracks = crate::tracks::tracks_needed(&lanes) as Tick;
        let room = tracks
            .saturating_mul(TRACK_PITCH)
            .saturating_add(TICKS_PER_STEP)
            .max(CHAIN_GAP);
        let narrow = tracks
            .saturating_mul(TRACK_PITCH_NARROW)
            .saturating_add(CHAIN_GAP_MIN);
        for (k, (_, l)) in items.iter().enumerate() {
            let steps = l.inline && across(self.pt(l.h), s) != across(self.pt(l.f), s);
            if !movers[k] && steps && dist(self, l) < narrow {
                movers[k] = true;
            }
        }
        let line = items
            .iter()
            .enumerate()
            .filter(|(k, _)| !movers[*k])
            .map(|(_, (_, l))| dist(self, l))
            .filter(|d| (room..=room + ROW_PULL).contains(d))
            .max()
            .unwrap_or(room);
        for (k, (m, l)) in items.iter().enumerate() {
            if movers[k] {
                // (A chain opens by a row's pull at most, rule A5: its tracks then take the narrow pitch.)
                let was = beyond(self.pt0(l.h), self.pt0(l.f), s);
                let most = if was >= CHAIN_GAP_MIN {
                    was.max(CHAIN_GAP) + ROW_PULL
                } else {
                    line
                };
                let by = whole_up(line.min(most) - dist(self, l));
                self.d[*m] = self.d[*m].plus(along_vec(s, by));
            }
        }
        // A member a rail taps joins the rail's row where its lead leaves it the room.
        for (m, l) in &items {
            let Some((step, must)) = self.steps.get(m).copied() else {
                continue;
            };
            if s.vertical() != (step.y != 0) {
                continue;
            }
            let by = beyond(Pt::default(), step, s) - beyond(Pt::default(), self.d[*m], s);
            if by != 0 && dist(self, l) + by >= CHAIN_GAP_MIN {
                self.d[*m] = self.d[*m].plus(along_vec(s, by));
            } else if by != 0 && must {
                self.failed = true;
            }
        }
        // A member whose place in line is taken stands where it stood across, when that is free; one that moved onto a
        // unit already set steps further out.
        let reach = self.w.page.area.w().max(self.w.page.area.h());
        // (A row packed by its bodies stands by its bodies: its texts found their places.)
        let on = |sk: &Self, m: usize| {
            if tight {
                (0..sk.units.len()).any(|o| {
                    o != m
                        && sk.placed[o]
                        && sk.within.as_ref().is_none_or(|w| w.contains(&o))
                        && sk.bodies_meet(m, o)
                })
            } else {
                sk.blocked(m, &[])
            }
        };
        // A row whose line is taken stands nearer: the members that take the row's line step back together, a step at a
        // time and no nearer than their tracks leave them, to the first line on which none of them stands on a unit
        // that is set (a row of sensors under a box at the head).
        let taking: Vec<usize> = (0..items.len()).filter(|k| movers[*k]).collect();
        if taking.iter().any(|k| on(self, items[*k].0)) {
            let start = self.d.clone();
            let nearer = loop {
                for k in &taking {
                    let m = items[*k].0;
                    self.d[m] = self.d[m].plus(along_vec(s, -TICKS_PER_STEP));
                }
                if taking.iter().any(|k| dist(self, &items[*k].1) < narrow) {
                    break false;
                }
                if !taking.iter().any(|k| on(self, items[*k].0)) {
                    break true;
                }
            };
            if !nearer {
                self.d = start;
            }
        }
        for (k, (m, l)) in items.iter().enumerate() {
            if wished[k] != 0 && on(self, *m) {
                // Further out on its line where the line is free that far (two flow controls wider than their ports'
                // pitch stand staggered); else where it stood across.
                let start = self.d[*m];
                let mut more = 0;
                while on(self, *m) && more < 2 * ROW_PULL {
                    self.d[*m] = self.d[*m].plus(along_vec(s, TICKS_PER_STEP));
                    more += TICKS_PER_STEP;
                }
                let mut own = self.subtree(*m);
                own.push(l.host);
                if on(self, *m) || !self.line_free(self.pt(l.h), self.pt(l.f), &own) {
                    self.d[*m] = start.plus(across_vec(s, -wished[k]));
                }
            }
            let mut more = 0;
            while self.d[*m] != Pt::default() && on(self, *m) && more < reach {
                self.d[*m] = self.d[*m].plus(along_vec(s, TICKS_PER_STEP));
                more += TICKS_PER_STEP;
            }
            if more >= reach {
                // No room: back to where it stood.
                self.d[*m] = Pt::default();
                self.failed = true;
            }
            self.placed[*m] = true;
        }
    }

    /// The wires of a block towards the rank it hangs on, on side `s` of the rank, its own pins facing `sf`: its lead
    /// and the relations left out of the tree, each as (host pin, own pin).
    fn entries(
        &self,
        hosts: &[usize],
        s: Side,
        sf: Side,
        units: &[usize],
        lead: Lead,
    ) -> Vec<(PinId, PinId)> {
        let mut out = vec![(lead.h, lead.f)];
        for r in &self.extra {
            if r.kind != Kind::Corner {
                continue;
            }
            for (h, f) in [(r.pa, r.pb), (r.pb, r.pa)] {
                if hosts.contains(&self.unit_of_pin(h))
                    && units.contains(&self.unit_of_pin(f))
                    && self.side(h) == Some(s)
                    && self.side(f) == Some(sf)
                {
                    out.push((h, f));
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// A stack of corner followers on side `s` of `hosts`, their own pins facing `sf` (module documentation).
    fn stack(&mut self, hosts: &[usize], s: Side, sf: Side, members: &[usize]) {
        let way = opposite(sf);
        let mut blocks: Vec<Block> = members
            .iter()
            .filter_map(|m| {
                let units = self.subtree(*m);
                let entries = self.entries(hosts, s, sf, &units, self.lead[*m]?);
                Some((units, entries))
            })
            .collect();
        // Innermost first: the drop nearest the side the blocks stand on.
        let inner = |sk: &Self, e: &[(PinId, PinId)]| e.iter().map(|(h, _)| out(sk.pt(*h), way)).max();
        blocks.sort_by_key(|(units, e)| (Reverse(inner(self, e)), self.id(units[0])));
        let Some(key) = blocks.first().map(|(units, _)| (units[0], s, sf)) else {
            return;
        };
        let start = self.d.clone();
        let pitches: Vec<Tick> = match self.pitch.get(&key) {
            Some(p) => vec![*p],
            None => vec![MARK_PITCH, MARK_PITCH + TICKS_PER_STEP, 2 * MARK_PITCH],
        };
        let mut best: Option<(usize, Tick, Vec<Pt>)> = None;
        for pitch in pitches {
            self.d.clone_from(&start);
            self.stacked(s, way, &blocks, pitch);
            if self.pitch.contains_key(&key) {
                best = Some((0, pitch, self.d.clone()));
                break;
            }
            let mut all: Vec<usize> = blocks.iter().flat_map(|(u, _)| u.iter().copied()).collect();
            all.extend(hosts.iter().copied());
            let wires: Vec<(PinId, PinId)> = blocks.iter().flat_map(|(_, e)| e.iter().copied()).collect();
            let clashes = self.text_clashes(&all, &wires).0;
            if best.as_ref().is_none_or(|(c, _, _)| clashes < *c) {
                best = Some((clashes, pitch, self.d.clone()));
            }
            // As clear as the texts stood before (a mark set on a pin has its texts on that part wherever it stands).
            let now = std::mem::replace(&mut self.d, start.clone());
            let before = self.clashes_before(&all).0;
            self.d = now;
            if clashes <= before {
                break;
            }
        }
        if let Some((_, pitch, d)) = best {
            self.d = d;
            self.pitch.insert(key, pitch);
        }
        for (units, _) in &blocks {
            for u in units {
                self.placed[*u] = true;
            }
        }
    }

    /// Whether a body of `u` stands within [`ROW_GAP`] of a body of `o`.
    fn bodies_meet(&self, u: usize, o: usize) -> bool {
        let (a, b) = (self.bodies(u), self.bodies(o));
        u != o
            && a.iter()
                .any(|x| b.iter().any(|y| x.grown(ROW_GAP - 2).overlaps(y.grown(1))))
    }

    /// One pass over the blocks of a stack at `pitch`: each keeps its place or takes the nearest good one. The blocks
    /// keep their bodies clear of each other and of what is set; their texts are the text placer's (the caller asks it).
    fn stacked(&mut self, s: Side, way: Side, blocks: &[Block], pitch: Tick) {
        let reach = self.w.page.area.w().max(self.w.page.area.h());
        // A block takes its place at `floor` or beyond; one that stands at `least` or beyond keeps its own.
        let (mut floor, mut least) = (CHAIN_GAP_MIN, TICKS_PER_STEP);
        let mut set: Vec<usize> = Vec::new();
        let short = |sk: &Self, entries: &[(PinId, PinId)]| {
            entries
                .iter()
                .map(|(h, f)| MARK_LEAD - beyond(sk.pt(*h), sk.pt(*f), way))
                .max()
                .unwrap_or(0)
        };
        // The near column: the mark's lead before the drop nearest the side the blocks stand on.
        let near = blocks
            .iter()
            .flat_map(|(_, e)| e.iter().map(|(h, _)| out(self.pt(*h), way)))
            .max()
            .map(|x| x + MARK_LEAD);
        // The column the person made: where most of the blocks of one unit stand that have their lead (the one
        // furthest out of several).
        let mut stand: BTreeMap<Tick, usize> = BTreeMap::new();
        for (units, entries) in blocks {
            if let ([_], [(_, f)]) = (units.as_slice(), entries.as_slice())
                && short(self, entries) <= 0
            {
                *stand.entry(out(self.pt(*f), way)).or_default() += 1;
            }
        }
        let made = stand
            .iter()
            .filter(|(_, k)| **k >= 2)
            .max_by_key(|(c, k)| (**k, **c))
            .map(|(c, _)| *c);
        for (units, entries) in blocks {
            let on = |sk: &Self, set: &[usize]| {
                units.iter().any(|u| {
                    (0..sk.units.len()).any(|o| {
                        if units.contains(&o) {
                            false
                        } else if set.contains(&o) {
                            // Blocks of one stack stand as near as their pitch sets them: their bodies only must
                            // not lie on each other.
                            sk.bodies(*u)
                                .iter()
                                .any(|a| sk.bodies(o).iter().any(|b| a.grown(-1).overlaps(*b)))
                        } else {
                            sk.placed[o]
                                && sk.within.as_ref().is_none_or(|w| w.contains(&o))
                                && sk.bodies_meet(*u, o)
                        }
                    })
                })
            };
            // How far the block is from a column.
            let to_column = |sk: &Self, column: Option<Tick>| {
                entries
                    .iter()
                    .map(|(_, f)| column.unwrap_or(0) - out(sk.pt(*f), way))
                    .max()
                    .unwrap_or(0)
            };
            // Across: every pin of its own at least the mark's lead before its drop. A block that has to move, and a
            // block of one unit within the row's pull of it, takes the column the person made; without one, a block
            // that has to move takes the near column where that is within the pull of its own lead.
            let lacks = short(self, entries);
            let single = units.len() == 1 && entries.len() == 1;
            let to_made = to_column(self, made);
            if made.is_some() && to_made >= lacks && (lacks > 0 || (single && to_made.abs() <= ROW_PULL)) {
                self.shift(units, along_vec(way, whole_up(to_made)));
            } else if lacks > 0 {
                let to = to_column(self, near);
                let by = if to >= lacks && to - lacks <= ROW_PULL {
                    to
                } else {
                    lacks
                };
                self.shift(units, along_vec(way, whole_up(by)));
            }
            // Along: beyond the blocks before it.
            let level = |sk: &Self| {
                entries
                    .iter()
                    .map(|(h, f)| beyond(sk.pt(*h), sk.pt(*f), s))
                    .min()
                    .unwrap_or(0)
            };
            if level(self) < least {
                let by = whole_up(floor - level(self));
                self.shift(units, along_vec(s, by));
            }
            // Its place taken: the near column, where that is free; else further out.
            if on(self, &set) {
                let to = whole_up(to_column(self, near));
                if to != 0 {
                    self.shift(units, along_vec(way, to));
                    let lead = entries
                        .iter()
                        .map(|(h, f)| beyond(self.pt(*h), self.pt(*f), way))
                        .min()
                        .unwrap_or(0);
                    if on(self, &set) || lead < MARK_LEAD {
                        self.shift(units, along_vec(way, -to));
                    }
                }
            }
            let mut more = 0;
            while on(self, &set) && more < reach {
                self.shift(units, along_vec(s, TICKS_PER_STEP));
                more += TICKS_PER_STEP;
            }
            if more >= reach {
                self.failed = true;
            }
            let far = entries
                .iter()
                .map(|(h, f)| beyond(self.pt(*h), self.pt(*f), s))
                .max()
                .unwrap_or(floor);
            // The next block's wire runs beyond this block's bodies too.
            let base = entries.first().map_or(0, |(h, _)| out(self.pt(*h), s));
            let bodies = units
                .iter()
                .flat_map(|u| self.bodies(*u))
                .map(|b| span_along(b, s).1 - base + BODY_GAP)
                .max()
                .unwrap_or(0);
            floor = (far + pitch).max(whole_up(bodies));
            least = floor;
            set.extend(units.iter().copied());
        }
        // A stack that leaves the drawing area slides back towards its hosts as far as its first block has room.
        let edge = span_along(self.w.page.area.grown(-TICKS_PER_STEP / 2), s).1;
        let over = set
            .iter()
            .flat_map(|u| self.bodies(*u))
            .map(|b| span_along(b, s).1 - edge)
            .max()
            .unwrap_or(0);
        let slack = blocks
            .first()
            .and_then(|(_, e)| e.iter().map(|(h, f)| beyond(self.pt(*h), self.pt(*f), s)).min())
            .map_or(0, |l| l - CHAIN_GAP_MIN);
        let slide = whole_up(over).min(slack - slack.rem_euclid(TICKS_PER_STEP));
        if over > 0 && slide > 0 {
            self.shift(&set, along_vec(s, -slide));
            let on = set.iter().any(|u| {
                (0..self.units.len()).any(|o| {
                    !set.contains(&o)
                        && self.placed[o]
                        && self.within.as_ref().is_none_or(|w| w.contains(&o))
                        && self.bodies_meet(*u, o)
                })
            });
            if on {
                self.shift(&set, along_vec(s, slide));
            }
        }
    }

    /// [`Sketch::text_clashes`] of `units` where they stand on the page as it is: what the texts of these parts cannot
    /// help (a mark set on a pin has its texts on that pin's part).
    fn clashes_before(&mut self, units: &[usize]) -> (usize, usize) {
        let now = self.d.clone();
        for u in units {
            self.d[*u] = Pt::default();
        }
        // No wires: where a part stands on the wrong side of its pin its wire has no plain shape.
        let clashes = self.text_clashes(units, &[]);
        self.d = now;
        clashes
    }

    /// How many texts of `units` the text placer cannot set clear with the units where they now stand, each of `wires`
    /// (a host's pin and a follower's) drawn in its plain shape (straight, with its one corner, or stepping at the
    /// middle), and how many it sets more than a step away from their part: asked on a page of those parts alone.
    fn text_clashes(&self, units: &[usize], wires: &[(PinId, PinId)]) -> (usize, usize) {
        let page = self.w.page;
        // No drawing area: where the stack stands on the sheet is settled afterwards.
        let far = Tick::MAX / 4;
        let mut mini = DraftPage {
            kind: page.kind,
            area: Rect::new(Pt::new(-far, -far), Pt::new(far, far)),
            ..DraftPage::default()
        };
        let mut ids: BTreeSet<PartId> = BTreeSet::new();
        for u in units {
            for i in &self.units[*u].parts {
                let mut part = self.w.parts[*i].clone();
                part.at = part.at.plus(self.d[*u]);
                ids.insert(part.id);
                for t in page.fixed_texts.iter().filter(|t| t.owner == part.id.0) {
                    let mut t = t.clone();
                    t.rect = t.rect.moved(self.d[*u]);
                    mini.fixed_texts.push(t);
                }
                mini.parts.push(part);
            }
        }
        mini.labels = page
            .labels
            .iter()
            .filter(|l| matches!(l.owner, Owner::Part(p) if ids.contains(&p)))
            .cloned()
            .collect();
        for (h, f) in wires {
            let (a, b) = (self.pt(*f), self.pt(*h));
            let (Some(sf), Some(sh)) = (self.side(*f), self.side(*h)) else {
                continue;
            };
            let route = if sf.vertical() != sh.vertical() {
                let corner = if sf.vertical() {
                    Pt::new(a.x, b.y)
                } else {
                    Pt::new(b.x, a.y)
                };
                vec![a, corner, b]
            } else if sf == sh {
                // Out, across and back: the router's to place.
                continue;
            } else if across(a, sf) == across(b, sf) {
                vec![a, b]
            } else if sf.vertical() {
                let y = (a.y + b.y).div_euclid(2 * TICKS_PER_STEP) * TICKS_PER_STEP;
                vec![a, Pt::new(a.x, y), Pt::new(b.x, y), b]
            } else {
                let x = (a.x + b.x).div_euclid(2 * TICKS_PER_STEP) * TICKS_PER_STEP;
                vec![a, Pt::new(x, a.y), Pt::new(x, b.y), b]
            };
            mini.wires.push(Wire {
                id: WireId(f.0),
                ends: [End::Pin(*f), End::Pin(*h)],
                route,
                computed: true,
                stale: false,
                class: crate::model::WireClass::Wire,
                supply: None,
            });
        }
        // Every other wired pin of these parts with its first steps of wire.
        let cornered: BTreeSet<PinId> = mini
            .wires
            .iter()
            .flat_map(|w| w.ends)
            .filter_map(|e| match e {
                End::Pin(p) => Some(p),
                End::Junction(_) => None,
            })
            .collect();
        let wired: BTreeSet<PinId> = self.w.nets.iter().flat_map(|n| n.pins.iter().copied()).collect();
        for part in mini.parts.clone() {
            for pin in &part.pins {
                let Some(s) = pin.side else { continue };
                if cornered.contains(&pin.id) || !wired.contains(&pin.id) {
                    continue;
                }
                let at = part.at.plus(pin.at);
                let end = at.plus(along_vec(s, 2 * TICKS_PER_STEP));
                let j = JunctionId(pin.id.0);
                mini.junctions.push(Junction { id: j, at: end });
                mini.wires.push(Wire {
                    id: WireId(pin.id.0),
                    ends: [End::Pin(pin.id), End::Junction(j)],
                    route: vec![at, end],
                    computed: true,
                    stale: false,
                    class: crate::model::WireClass::Wire,
                    supply: None,
                });
            }
        }
        let q = measure(&mini);
        // (A label within a step of what it names reads as its own, §1.3.)
        let away = q
            .labels
            .iter()
            .filter(|l| {
                mini.parts
                    .iter()
                    .find(|part| part.id.0 == l.id.owner)
                    .is_some_and(|part| crate::geom::gap(l.rect, part.bounds()) > TICKS_PER_STEP)
            })
            .count();
        (q.counts.label_clashes + q.counts.across_texts, away)
    }

    /// Whether the way out of pin `p` along its side runs into a body of unit `o`.
    fn walled(&self, p: PinId, o: usize) -> bool {
        let Some(s) = self.side(p) else { return false };
        let at = self.pt(p);
        self.bodies(o).iter().any(|b| {
            let (lo, hi) = span_across(*b, s);
            let (_, far) = span_along(*b, s);
            lo - BODY_GAP < across(at, s) && across(at, s) < hi + BODY_GAP && far > out(at, s)
        })
    }

    /// The level followers on side `s` of their hosts (module documentation). Those that keep their place: what hangs
    /// on them is then set with the rest of their rank.
    fn beside(&mut self, s: Side, members: &[usize]) -> Vec<usize> {
        let mut stay: Vec<usize> = Vec::new();
        let mut items: Vec<(usize, Lead)> = members
            .iter()
            .filter_map(|m| self.lead[*m].map(|l| (*m, l)))
            .collect();
        items.sort_by_key(|(m, l)| (across(self.pt(l.h), s), self.id(*m)));
        // The ones whose wire has no way out and back, by host.
        let mut stuck: BTreeMap<usize, Vec<(usize, Lead)>> = BTreeMap::new();
        for (m, l) in items {
            let units = self.subtree(m);
            let walled = units.iter().any(|u| self.walled(l.h, *u)) || self.walled(l.f, l.host);
            if walled {
                stuck.entry(l.host).or_default().push((m, l));
            } else {
                // On a bus page (rule B2) two stations whose ports face one side stand level.
                let by = beyond(self.pt(l.f), self.pt(l.h), s);
                if self.w.page.kind == PageKind::Bus && by != 0 {
                    self.shift(&units, along_vec(s, by));
                    if units.iter().any(|u| self.blocked(*u, &units)) {
                        self.shift(&units, along_vec(s, -by));
                        self.failed = true;
                    }
                } else if self.w.page.kind != PageKind::Bus && !self.hairpin(s, l, &units) {
                    stay.push(m);
                    continue;
                }
                for u in units {
                    self.placed[u] = true;
                }
            }
        }
        for (host, list) in stuck {
            let (lo, hi) = span_across(self.hull(host), s);
            // One side for them all: the one their pins lie nearer to.
            let (to_lo, to_hi) = list.iter().fold((0i64, 0i64), |(a, b), (_, l)| {
                let x = across(self.pt(l.h), s);
                (a + i64::from(x - lo), b + i64::from(hi - x))
            });
            let sides: [bool; 2] = if to_lo < to_hi {
                [true, false]
            } else {
                [false, true]
            };
            let start = self.d.clone();
            let mut done = false;
            for low in sides {
                self.d.clone_from(&start);
                // The pin nearest that side innermost.
                let mut order = list.clone();
                order.sort_by_key(|(m, l)| {
                    let x = across(self.pt(l.h), s);
                    (if low { x } else { -x }, self.id(*m))
                });
                let mut edge = if low { lo } else { hi };
                let mut ok = true;
                let mut set: Vec<usize> = Vec::new();
                for (m, l) in &order {
                    let units = self.subtree(*m);
                    // Pins level.
                    let by = beyond(self.pt(l.f), self.pt(l.h), s);
                    self.shift(&units, along_vec(s, by));
                    let hull = units
                        .iter()
                        .map(|u| self.hull(*u))
                        .reduce(Rect::union)
                        .unwrap_or_default();
                    let (a, b) = span_across(hull, s);
                    let by = if low {
                        -whole_up(b + TRACK_PITCH - edge)
                    } else {
                        whole_up(edge + TRACK_PITCH - a)
                    };
                    self.shift(&units, across_vec(s, by));
                    let on = units
                        .iter()
                        .any(|u| self.blocked(*u, &units) || set.iter().any(|o| self.meets(*u, *o)));
                    if on {
                        ok = false;
                        break;
                    }
                    let hull = units
                        .iter()
                        .map(|u| self.hull(*u))
                        .reduce(Rect::union)
                        .unwrap_or_default();
                    edge = if low {
                        span_across(hull, s).0
                    } else {
                        span_across(hull, s).1
                    };
                    set.extend(units);
                }
                if ok {
                    done = true;
                    break;
                }
            }
            if !done {
                // No room beside: they stay where they stand.
                self.d = start;
                stay.extend(list.iter().map(|(m, _)| *m));
                continue;
            }
            for (m, _) in &list {
                for u in self.subtree(*m) {
                    self.placed[u] = true;
                }
            }
        }
        // What hangs on one that stays was set by itself: it is set anew with its rank.
        for m in &stay {
            for u in self.subtree(*m) {
                if u != *m {
                    self.d[u] = Pt::default();
                }
            }
            self.placed[*m] = true;
        }
        stay
    }

    /// A level follower (`units`: it and what hangs on it) that stands neither beside its host nor near beyond its
    /// pins takes the row beyond them: two steps clear of the host, and off the pin's line by its own width so that
    /// the wire passes it, on the side where the host has the fewer pins of that edge (the side it stands on where
    /// they are as many). Where neither side has the room it stays. Whether it moved.
    fn hairpin(&mut self, s: Side, l: Lead, units: &[usize]) -> bool {
        let bodies = |sk: &Self, us: &[usize]| {
            us.iter()
                .flat_map(|u| sk.bodies(*u))
                .reduce(Rect::union)
                .unwrap_or_default()
        };
        let (host, own) = (bodies(self, &[l.host]), bodies(self, units));
        let ((h0, h1), (o0, o1)) = (span_along(host, s), span_along(own, s));
        let (lo, hi) = span_across(own, s);
        let (h, f) = (across(self.pt(l.h), s), across(self.pt(l.f), s));
        let clear = hi + BODY_GAP <= h || h <= lo - BODY_GAP;
        let beside = o0 < h1 && h0 < o1;
        let near = (h1 + CHAIN_GAP_MIN..=h1 + CHAIN_GAP + ROW_PULL).contains(&o0);
        if beside || (near && clear) {
            return false;
        }
        let pins = |sk: &Self, low: bool| {
            sk.units[l.host]
                .parts
                .iter()
                .flat_map(|i| sk.w.parts[*i].pins.iter())
                .filter(|p| p.side == Some(s) && p.id != l.h && (across(sk.pt(p.id), s) < h) == low)
                .count()
        };
        let low = (pins(self, true), f > h) < (pins(self, false), f <= h);
        let along = whole_up(h1 + CHAIN_GAP_MIN - o0);
        let start = self.d.clone();
        for low in [low, !low] {
            let by = if low {
                -whole_up((hi + BODY_GAP - h).max(f + TRACK_PITCH - h))
            } else {
                whole_up((h + BODY_GAP - lo).max(h + TRACK_PITCH - f))
            };
            // (Further out where the texts of the two need it.)
            for more in 0..=ROW_PULL / TICKS_PER_STEP {
                let step = along_vec(s, along + more * TICKS_PER_STEP).plus(across_vec(s, by));
                self.shift(units, step);
                let ok = units.iter().all(|u| self.inside(*u) && !self.blocked(*u, units));
                if ok {
                    return true;
                }
                self.d.clone_from(&start);
            }
        }
        false
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

    /// The strips opened as one, each by its first terminal.
    pub fn opened(&self) -> Vec<PartId> {
        self.opened
            .iter()
            .filter_map(|s| s.first().map(|u| self.largest(*u)))
            .collect()
    }

    /// `units` with every unit of a strip opened as one that one of them belongs to, and what hangs on those: a strip
    /// that opened goes or stays as one, never in part.
    pub fn with_strips(&self, units: Vec<usize>) -> Vec<usize> {
        let mut out: BTreeSet<usize> = units.into_iter().collect();
        for strip in &self.opened {
            if strip.iter().any(|u| out.contains(u)) {
                out.extend(strip.iter().flat_map(|u| self.subtree(*u)));
            }
        }
        out.into_iter().collect()
    }

    /// The rows of terminals (the units joined through mates side by side, or one under another, whose largest part
    /// carries a terminal's name), two or more, each with whether it stands side by side and its units in the order
    /// they stand.
    fn terminal_rows(&self) -> Vec<(bool, Vec<usize>)> {
        let named: BTreeSet<PartId> = self
            .w
            .page
            .labels
            .iter()
            .filter(|l| l.kind == LabelKind::TerminalName)
            .filter_map(|l| match l.owner {
                Owner::Part(p) => Some(p),
                _ => None,
            })
            .collect();
        let mut out = Vec::new();
        for (k, down) in [(0, true), (1, false)] {
            let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for u in (0..self.units.len()).filter(|u| named.contains(&self.largest(*u))) {
                groups.entry(self.strips[k][u]).or_default().push(u);
            }
            for mut row in groups.into_values().filter(|r| r.len() > 1) {
                row.sort_by_key(|u| (self.shape(*u).map_or(0, |r| if down { r.x0 } else { r.y0 }), *u));
                out.push((down, row));
            }
        }
        out
    }

    /// Whether every row of terminals keeps the order it stands in with the moves as they are (doc 06a §4.4): no
    /// terminal passes another, in any step. Two that stood at one place along the row have no order to keep.
    pub fn order_kept(&self) -> bool {
        self.terminal_rows.iter().all(|(down, row)| {
            let along = |u: usize, d: Pt| {
                self.shape(u)
                    .map_or(0, |r| if *down { r.x0 + d.x } else { r.y0 + d.y })
            };
            row.iter().enumerate().all(|(i, &a)| {
                row[i + 1..].iter().all(|&b| {
                    let (was, now) = (
                        along(b, Pt::default()) - along(a, Pt::default()),
                        along(b, self.d[b]) - along(a, self.d[a]),
                    );
                    was == 0 || now.signum() == was.signum()
                })
            })
        })
    }

    /// The moves, by part ID.
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
