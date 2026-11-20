// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one measure of a page (docs/design/06a-drafting.md §1.2): [`measure`] says what is a fault, counts what a
//! reader weighs, and reports the figures `ew layout` prints. It places the labels with [`settle_labels`] first, so it
//! judges the texts where the page shows them.
//!
//! # Definitions
//!
//! Exact on ticks. A wire's route is taken without consecutive equal points; its segments are the pieces between its
//! vertices, its runs the segments with the vertices it runs straight through left out, its corners the vertices
//! where it changes direction (a reversal is one). A point lies on a segment when it is on the closed segment; a
//! segment runs through a rectangle when it has points strictly inside it. A wire ends at a pin or a junction when one
//! of its two ends names it. Only wires with a route are geometry; a wire without one is unrouted.
//!
//! **Faults**, one item each:
//!
//! - **F2** (a wire, a point): a pin or junction the wire does not end at lies on it, and the point is not where one
//!   of its own ends stands (the clamps of a terminal drawn at one point).
//! - **F3** (a pair of wires): they share a stretch of positive length on one line, but within the [`STUB`] of a pin
//!   both end at, along the pin's side (the common beginning of two wires of one pin; the shared run of §3.4 comes with
//!   the router).
//! - **F4** (a wire, a part with pins or a drawn box): a stretch of the wire runs through the part's body, but on the
//!   ray out of a pin the wire ends at, along the pin's side, through that pin's part or a part with a pin on the same
//!   point. An outline drawn round other parts is no body.
//! - **F5** (a junction): the wires ending at it leave it in four or more directions.
//! - **F6** (a pair of wires, a point): a corner of each lies on the other's corner: they meet without joining.
//!   (A corner or an end on another wire's end, a junction on a crossing and a wire's end on a wire are F2 by their
//!   geometry: the other wire runs through a pin or a junction it does not end at.)
//! - **F7** (a part, and the part it lies on or none): two parts with pins (or drawn boxes) whose bodies overlap, an
//!   outline drawn round other parts excepted; a part whose body leaves the drawing area.
//!
//! **Cost**, in sixteenths of a step (the tick): C1 the routes' length; C2 [`BEND`] per corner; C3 [`CROSSING`] per
//! point where two wires meet that is no end of either, on no stretch they share and no F6; C4 for each pair of a wire
//! and a text it runs through, [`TEXT_CROSSED`] and [`TEXT_STEP`] per tick inside; C5 [`LABEL_CLASH`] for each pair
//! of a label and a text, a label and another part's body, and each label outside the drawing area; C6
//! [`LABEL_RANK`] for each place a label stands down its list; C7 [`HALF_STEP_EXTRA`] per tick of a run on the half
//! grid (its line at an odd multiple of half a step).
//!
//! **Reported:** the wires drawn straight (routed, no corner) against all wires; the turn-backs (three runs in a
//! row of which the third runs against the first); the detour (the routes' length against the city-block distance of
//! their ends, in percent); the labels off their first place.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{
    BEND, CROSSING, HALF_STEP_EXTRA, LABEL_CLASH, LABEL_RANK, STEP, STUB, TEXT_CROSSED, TEXT_STEP,
    TICKS_PER_STEP,
};
use crate::geom::{self, Seg};
use crate::labels::{Settled, settle_labels};
use crate::model::{DraftPage, End, JunctionId, LabelId, LabelKind, PartId, PinId, Pt, Rect, Side, WireId};

/// What a wire runs through without ending there (F2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Passed {
    /// A pin.
    Pin(PinId),
    /// A junction.
    Junction(JunctionId),
}

/// A text of the page, as a finding names it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum TextRef {
    /// A label.
    Label(LabelId),
    /// A fixed text, by its owner's ID and its box.
    Fixed(ew_ids::Uuid, Rect),
}

/// The items behind the faults and costs, each list in a fixed order (wires, parts and labels by ID).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Findings {
    /// F2: a wire and the pin or junction it runs through, with the point.
    pub through_points: Vec<(WireId, Passed, Pt)>,
    /// F3: two wires and the stretches they share.
    pub on_wires: Vec<([WireId; 2], Vec<Seg>)>,
    /// F4: a wire, the part it runs through and the stretches inside.
    pub through_parts: Vec<(WireId, PartId, Vec<Seg>)>,
    /// F5: a junction, its arms and its point.
    pub four_way: Vec<(JunctionId, usize, Pt)>,
    /// F6: two wires and the point where their corners meet.
    pub false_joints: Vec<([WireId; 2], Pt)>,
    /// F7: a part and the part it lies on, or `None` where it leaves the drawing area.
    pub parts_on_parts: Vec<(PartId, Option<PartId>)>,
    /// C3: two wires and the point they cross at.
    pub crossings: Vec<([WireId; 2], Pt)>,
    /// C2: each wire with corners, and how many.
    pub bends: Vec<(WireId, usize)>,
    /// C4: a wire, a text it runs through and the stretches inside.
    pub across_texts: Vec<(WireId, TextRef, Vec<Seg>)>,
    /// C5: a label and the text it lies on.
    pub texts_on_texts: Vec<(LabelId, TextRef)>,
    /// C5: a label and the body of another part it lies on.
    pub texts_on_parts: Vec<(LabelId, PartId)>,
    /// C5: a label outside the drawing area.
    pub texts_outside: Vec<LabelId>,
    /// Reported: each wire with turn-backs, and how many.
    pub turn_backs: Vec<(WireId, usize)>,
    /// The wires without a route between two different points.
    pub unrouted: Vec<WireId>,
}

/// The counts behind the cost and the reported figures.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Counts {
    /// C1: the routes' length, in ticks.
    pub length: i64,
    /// C2: the corners.
    pub bends: usize,
    /// C3: the crossings.
    pub crossings: usize,
    /// C4: the pairs of a wire and a text it runs through.
    pub across_texts: usize,
    /// C4: the ticks of wire on texts.
    pub on_texts: i64,
    /// C5: the clashes of labels.
    pub label_clashes: usize,
    /// C6: the places down their lists, summed.
    pub label_ranks: i64,
    /// C7: the ticks of runs on the half grid.
    pub half_grid: i64,
    /// The wires drawn straight.
    pub straight: usize,
    /// All wires (routed or not, between two different points).
    pub wires: usize,
    /// The turn-backs.
    pub turn_backs: usize,
    /// The routes' length against the city-block distance of their ends, in percent (100 without routes).
    pub detour: i64,
    /// The labels off their first place.
    pub labels_moved: usize,
}

/// How good a page is: compared by its faults, then its unrouted wires, then its cost (§1.2).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Quality {
    /// The faults F2 to F7.
    pub faults: usize,
    /// The wires without a route.
    pub unrouted: usize,
    /// The cost C1 to C7, in sixteenths of a step.
    pub cost: i64,
    /// The counts behind it.
    pub counts: Counts,
    /// The items behind it.
    pub findings: Findings,
    /// The labels where the text placer set them.
    pub labels: Vec<Settled>,
}

impl Quality {
    /// The triple it is compared by.
    pub fn key(&self) -> (usize, usize, i64) {
        (self.faults, self.unrouted, self.cost)
    }

    /// Whether it is better than `other`.
    pub fn better_than(&self, other: &Quality) -> bool {
        self.key() < other.key()
    }
}

/// A wire as the measure reads it.
struct Line {
    id: WireId,
    ends: [End; 2],
    pts: Vec<Pt>,
    segs: Vec<Seg>,
}

/// The measure of `page` (module documentation).
pub fn measure(page: &DraftPage) -> Quality {
    let labels = settle_labels(page);
    let mut parts: Vec<_> = page.parts.iter().collect();
    parts.sort_by_key(|p| p.id);
    // Every pin: its point, its side, its part.
    let mut pins: BTreeMap<PinId, (Pt, Option<Side>, PartId)> = BTreeMap::new();
    for p in &parts {
        for pin in &p.pins {
            pins.insert(pin.id, (p.at.plus(pin.at), pin.side, p.id));
        }
    }
    let mut junctions: Vec<_> = page.junctions.iter().collect();
    junctions.sort_by_key(|j| j.id);
    let point_of = |e: End| match e {
        End::Pin(p) => pins.get(&p).map(|v| v.0),
        End::Junction(j) => junctions.iter().find(|x| x.id == j).map(|x| x.at),
    };
    let mut wires: Vec<_> = page.wires.iter().collect();
    wires.sort_by_key(|w| w.id);
    let mut f = Findings::default();
    let mut c = Counts::default();
    let mut lines: Vec<Line> = Vec::new();
    let (mut drawn, mut distance) = (0_i64, 0_i64);
    for w in &wires {
        let pts = geom::clean(&w.route);
        if pts.is_empty() {
            let (a, b) = (point_of(w.ends[0]), point_of(w.ends[1]));
            if a != b || a.is_none() {
                f.unrouted.push(w.id);
                c.wires += 1;
            }
            continue;
        }
        c.wires += 1;
        let segs = geom::segments(&pts);
        let corners = geom::corners(&pts).len();
        if corners == 0 {
            c.straight += 1;
        } else {
            f.bends.push((w.id, corners));
            c.bends += corners;
        }
        let len: i64 = segs.iter().map(|s| geom::length(*s)).sum();
        c.length += len;
        drawn += len;
        if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
            distance += geom::length((*a, *b));
        }
        for (a, b) in geom::runs(&pts) {
            let half = |v: i32| v.rem_euclid(TICKS_PER_STEP) == TICKS_PER_STEP / 2;
            if (a.x == b.x && half(a.x)) || (a.y == b.y && half(a.y)) {
                c.half_grid += geom::length((a, b));
            }
        }
        let runs = geom::runs(&pts);
        let backs = runs
            .windows(3)
            .filter(|r| {
                let (d0, d2) = (geom::direction(r[0]), geom::direction(r[2]));
                d0 == (-d2.0, -d2.1)
            })
            .count();
        if backs > 0 {
            f.turn_backs.push((w.id, backs));
            c.turn_backs += backs;
        }
        lines.push(Line {
            id: w.id,
            ends: w.ends,
            pts,
            segs,
        });
    }
    c.detour = if distance > 0 {
        (drawn * 100 + distance / 2) / distance
    } else {
        100
    };
    let ends_here = |l: &Line, p: Pt| l.ends.iter().any(|e| point_of(*e) == Some(p));
    // F2.
    for l in &lines {
        let hit = |p: Pt| l.segs.iter().any(|s| geom::on(p, *s));
        for (id, (q, _, _)) in &pins {
            if !l.ends.contains(&End::Pin(*id)) && !ends_here(l, *q) && hit(*q) {
                f.through_points.push((l.id, Passed::Pin(*id), *q));
            }
        }
        for j in &junctions {
            if !l.ends.contains(&End::Junction(j.id)) && hit(j.at) {
                f.through_points.push((l.id, Passed::Junction(j.id), j.at));
            }
        }
    }
    // F3, F6 and C3, pair by pair.
    for i in 0..lines.len() {
        for k in i + 1..lines.len() {
            pair(&lines[i], &lines[k], &pins, &point_of, &mut f);
        }
    }
    // F4.
    for l in &lines {
        let rays: Vec<(PartId, Pt, Pt)> = l
            .ends
            .iter()
            .filter_map(|e| match e {
                End::Pin(p) => pins
                    .get(p)
                    .and_then(|(q, s, part)| Some((*part, *q, s.as_ref()?.unit()))),
                End::Junction(_) => None,
            })
            .collect();
        for p in parts.iter().filter(|p| !p.frame_only && !p.pins.is_empty()) {
            let body = p.bounds();
            let mut inside: Vec<Seg> = Vec::new();
            for s in &l.segs {
                if s.0 == s.1 {
                    continue;
                }
                let mut span = geom::inside(*s, body);
                for &(own, q, v) in &rays {
                    let shares = p.pins.iter().any(|pin| p.at.plus(pin.at) == q);
                    if own == p.id || shares {
                        span = span.and_then(|sp| geom::minus_ray(*s, sp, q, v));
                    }
                }
                if let Some((lo, hi)) = span {
                    let st = (geom::at(*s, lo), geom::at(*s, hi));
                    if !inside.contains(&st) {
                        inside.push(st);
                    }
                }
            }
            if !inside.is_empty() {
                f.through_parts.push((l.id, p.id, inside));
            }
        }
    }
    // F5.
    for j in &junctions {
        let mut dirs: BTreeSet<(i64, i64)> = BTreeSet::new();
        for l in lines
            .iter()
            .filter(|l| l.ends.contains(&End::Junction(j.id)) && l.pts.len() >= 2)
        {
            let n = l.pts.len();
            let next = if l.pts[0] == j.at {
                l.pts[1]
            } else if l.pts[n - 1] == j.at {
                l.pts[n - 2]
            } else {
                continue;
            };
            dirs.insert(geom::direction((j.at, next)));
        }
        if dirs.len() >= 4 {
            f.four_way.push((j.id, dirs.len(), j.at));
        }
    }
    // F7.
    let solid: Vec<_> = parts
        .iter()
        .filter(|p| !p.frame_only && !p.pins.is_empty())
        .collect();
    // Two parts with a pin on one point are drawn one on the other's pin (an interruption point on a pin).
    let pin_on_pin = |p: &crate::model::Part, o: &crate::model::Part| {
        p.pins
            .iter()
            .any(|a| o.pins.iter().any(|b| p.at.plus(a.at) == o.at.plus(b.at)))
    };
    for (i, p) in solid.iter().enumerate() {
        if !page.area.holds(p.bounds()) {
            f.parts_on_parts.push((p.id, None));
        }
        for o in &solid[i + 1..] {
            if geom::texts_meet(p.bounds(), o.bounds()) && !pin_on_pin(p, o) {
                f.parts_on_parts.push((p.id, Some(o.id)));
            }
        }
    }
    // C4 and C5 over the texts where they stand.
    let mut texts: Vec<(TextRef, Rect)> = labels.iter().map(|s| (TextRef::Label(s.id), s.rect)).collect();
    let mut fixed: Vec<_> = page
        .fixed_texts
        .iter()
        .map(|t| (TextRef::Fixed(t.owner, t.rect), t.rect))
        .collect();
    fixed.sort();
    texts.extend(fixed);
    // A mark's texts (a marker's, a cable's core names) stand on their wires by the marker's rule: no wire counts
    // across them.
    let marks: BTreeSet<ew_ids::Uuid> = page
        .marks
        .iter()
        .map(|m| m.id.0)
        .chain(page.wires.iter().map(|w| w.id.0))
        .collect();
    let crossable = |t: &TextRef| !matches!(t, TextRef::Fixed(o, _) if marks.contains(o));
    for l in &lines {
        for (t, r) in texts.iter().filter(|(t, _)| crossable(t)) {
            let inside: Vec<Seg> = l
                .segs
                .iter()
                .filter_map(|s| geom::stretch_inside(*s, r.grown(-1)))
                .collect();
            if !inside.is_empty() {
                c.on_texts += inside.iter().map(|s| geom::length(*s)).sum::<i64>();
                f.across_texts.push((l.id, *t, inside));
            }
        }
    }
    let owner_part = |id: LabelId| {
        page.labels
            .iter()
            .find(|l| l.id == id)
            .and_then(|l| match l.owner {
                crate::model::Owner::Part(p) => Some(p),
                _ => None,
            })
    };
    let rank: BTreeMap<LabelId, u32> = labels.iter().map(|s| (s.id, s.rank)).collect();
    let settled = |id: LabelId| {
        let kind = page.labels.iter().find(|l| l.id == id)?.kind;
        Some((kind, labels.iter().find(|s| s.id == id)?.rect))
    };
    // Two texts of one owner both at their first places stand as the symbol's author set them.
    let authors = |a: LabelId, t: &TextRef| match t {
        TextRef::Label(b) => {
            a.owner == b.owner
                && rank.get(&a) == Some(&0)
                && rank.get(b) == Some(&0)
                && settled(a)
                    .zip(settled(*b))
                    .is_some_and(|(x, y)| crate::labels::authors_layout(x, y))
        }
        TextRef::Fixed(..) => false,
    };
    for (i, s) in labels.iter().enumerate() {
        for (t, r) in texts.iter().skip(i + 1) {
            if !authors(s.id, t) && geom::texts_meet(s.rect, *r) {
                f.texts_on_texts.push((s.id, *t));
            }
        }
        let own = owner_part(s.id);
        // A pin's number where its symbol's author put it belongs to its pin, whatever part stands there (F7 names
        // the parts).
        let pin_at_home = s.rank == 0
            && page
                .labels
                .iter()
                .any(|l| l.id == s.id && matches!(l.kind, LabelKind::PinNumber(_)));
        for p in parts
            .iter()
            .filter(|p| !p.frame_only && Some(p.id) != own && !pin_at_home)
        {
            if geom::texts_meet(s.rect, p.bounds()) {
                f.texts_on_parts.push((s.id, p.id));
            }
        }
        if !page.area.holds(s.rect) {
            f.texts_outside.push(s.id);
        }
        c.label_ranks += i64::from(s.rank);
        if s.rank > 0 {
            c.labels_moved += 1;
        }
    }
    c.crossings = f.crossings.len();
    c.across_texts = f.across_texts.len();
    c.label_clashes = f.texts_on_texts.len() + f.texts_on_parts.len() + f.texts_outside.len();
    let step = i64::from(TICKS_PER_STEP);
    let cost = STEP * c.length
        + BEND * step * c.bends as i64
        + CROSSING * step * c.crossings as i64
        + TEXT_CROSSED * step * c.across_texts as i64
        + TEXT_STEP * c.on_texts
        + LABEL_CLASH * step * c.label_clashes as i64
        + LABEL_RANK * step * c.label_ranks
        + HALF_STEP_EXTRA * c.half_grid;
    let faults = f.through_points.len()
        + f.on_wires.len()
        + f.through_parts.len()
        + f.four_way.len()
        + f.false_joints.len()
        + f.parts_on_parts.len();
    Quality {
        faults,
        unrouted: f.unrouted.len(),
        cost,
        counts: c,
        findings: f,
        labels,
    }
}

/// F3, F6 and C3 for one pair of wires.
fn pair(
    a: &Line,
    b: &Line,
    pins: &BTreeMap<PinId, (Pt, Option<Side>, PartId)>,
    point_of: &dyn Fn(End) -> Option<Pt>,
    f: &mut Findings,
) {
    let mut shared: Vec<Seg> = Vec::new();
    let mut points: Vec<Pt> = Vec::new();
    for &s in &a.segs {
        for &t in &b.segs {
            if let Some(o) = geom::overlap(s, t) {
                if !shared.contains(&o) {
                    shared.push(o);
                }
            } else if let Some(p) = geom::meet(s, t)
                && !points.contains(&p)
            {
                points.push(p);
            }
        }
    }
    let ends = |p: Pt| {
        [a, b]
            .iter()
            .any(|l| l.ends.iter().any(|e| point_of(*e) == Some(p)))
    };
    let (ca, cb): (BTreeSet<Pt>, BTreeSet<Pt>) = (
        geom::corners(&a.pts).into_iter().collect(),
        geom::corners(&b.pts).into_iter().collect(),
    );
    for p in points {
        if ends(p) || shared.iter().any(|s| geom::on(p, *s)) {
            continue;
        }
        if ca.contains(&p) && cb.contains(&p) {
            f.false_joints.push(([a.id, b.id], p));
        } else {
            f.crossings.push(([a.id, b.id], p));
        }
    }
    // The common beginning of two wires of one pin, within its stub.
    shared.retain(|s| {
        !pins.iter().any(|(id, (q, side, _))| {
            let Some(side) = side else { return false };
            let both = a.ends.contains(&End::Pin(*id)) && b.ends.contains(&End::Pin(*id));
            let u = side.unit();
            let stub = (*q, q.plus(Pt::new(u.x * STUB, u.y * STUB)));
            both && geom::on(s.0, stub) && geom::on(s.1, stub)
        })
    });
    if !shared.is_empty() {
        f.on_wires.push(([a.id, b.id], shared));
    }
}
