// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Nets, taps and trunks (docs/design/06a-drafting.md §3.4).
//!
//! **A net job** is a set of job wires joined through junctions that may move (every wire at one is a job) or through
//! a pin two or more of them end at. Its **taps** are those junctions and those pins; its other ends (a pin with one
//! of its wires, a junction that stays) are its **leaves**.
//!
//! **The trunk.** When the taps form a chain (each joined to at most two other taps by the net's wires), the chain is
//! the spine, and its taps must lie along the trunk in the chain's order; a net with one tap that is a pin (a fan) has
//! a spine whose order is free. The trunk is one straight line; every end of the net has its **foot** on it: a pin the
//! point across from it (a pin whose side runs along the trunk and which lies on it is its own foot), a junction tap the
//! point across from the pin it drops to (else from where it stands). Each wire then runs from its end to its foot,
//! along the trunk to the other end's foot, and to its other end: a pin's wires share the drop from their pin, and
//! part on the trunk. A junction tap stands at its foot.
//!
//! **Candidates**, for each axis (horizontal first): the line the junction taps lie on now, when they lie on one; the
//! line of each leaf pin whose side runs along the axis; [`TRUNK_GAP`] beyond the outermost pin on each side the pins
//! face, and from there outward by [`TRUNK_PITCH`]; the middle between two rows of pins that face each other. A
//! candidate is **valid** when the spine's taps keep their order along it (two taps on one point are set one step
//! apart), its taps fall on no wire, and the trunk's runs and every straight drop are free in the occupancy; a drop
//! that is not straight, or not free, leaves its wire to the search with the tap where the trunk put it. The valid
//! candidate of least price (the routes' prices in the search's weights) is taken; ties go to the line the taps lie on
//! now, then the horizontal, then the smaller coordinate.
//!
//! A net with one tap that is a junction has no trunk: the junction is tried at its place and at each crossing of a
//! line through one end along its side with a line through another's, and stands where its three wires, searched,
//! cost least with three arms.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{TICKS_PER_STEP, TRUNK_GAP, TRUNK_PITCH};
use crate::model::{End, JunctionId, PinId, Pt, Side, Tick, WireId};

/// A net job.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Net {
    /// Its wires, by ID.
    pub wires: Vec<WireId>,
    /// Its taps: the junctions that may move and the pins two or more of its wires end at.
    pub taps: Vec<End>,
    /// The order of its taps along the trunk, when they form a chain (a fan's order is free: `None` with one pin tap).
    pub spine: Option<Vec<End>>,
}

/// The net jobs of `jobs` (module documentation), each wire in exactly one, in the order of their first wire's ID.
/// `ends`: each wire's two ends; `movable`: whether a junction may move.
pub fn nets(jobs: &[WireId], ends: &BTreeMap<WireId, [End; 2]>, movable: &dyn Fn(JunctionId) -> bool) -> Vec<Net> {
    let mut jobs: Vec<WireId> = jobs.to_vec();
    jobs.sort();
    jobs.dedup();
    // The ends that join wires of the net: a movable junction, or a pin two or more jobs end at.
    let mut at: BTreeMap<End, Vec<WireId>> = BTreeMap::new();
    for w in &jobs {
        if let Some(e) = ends.get(w) {
            for x in e {
                at.entry(*x).or_default().push(*w);
            }
        }
    }
    let joins = |e: &End, n: usize| match e {
        End::Junction(j) => movable(*j),
        End::Pin(_) => n >= 2,
    };
    // Union-find by the smallest member.
    let mut root: BTreeMap<WireId, WireId> = jobs.iter().map(|w| (*w, *w)).collect();
    fn find(root: &BTreeMap<WireId, WireId>, mut x: WireId) -> WireId {
        while let Some(&r) = root.get(&x) {
            if r == x {
                break;
            }
            x = r;
        }
        x
    }
    for (e, ws) in &at {
        if !joins(e, ws.len()) {
            continue;
        }
        for w in &ws[1..] {
            let (a, b) = (find(&root, ws[0]), find(&root, *w));
            let (lo, hi) = (a.min(b), a.max(b));
            root.insert(hi, lo);
        }
    }
    let mut groups: BTreeMap<WireId, Vec<WireId>> = BTreeMap::new();
    for w in &jobs {
        groups.entry(find(&root, *w)).or_default().push(*w);
    }
    groups
        .into_values()
        .map(|wires| {
            let mut taps: Vec<End> = at
                .iter()
                .filter(|(e, ws)| {
                    let mine: Vec<_> = ws.iter().filter(|w| wires.contains(w)).collect();
                    !mine.is_empty() && joins(e, mine.len())
                })
                .map(|(e, _)| *e)
                .collect();
            taps.sort();
            let spine = chain(&wires, &taps, ends);
            Net { wires, taps, spine }
        })
        .collect()
}

/// The taps of a net in chain order, when they form one (each joined to at most two others, all joined, no cycle);
/// its first tap the smaller of its two ends.
fn chain(wires: &[WireId], taps: &[End], ends: &BTreeMap<WireId, [End; 2]>) -> Option<Vec<End>> {
    if taps.len() < 2 {
        return None;
    }
    let mut next: BTreeMap<End, BTreeSet<End>> = taps.iter().map(|t| (*t, BTreeSet::new())).collect();
    let mut edges = 0usize;
    for w in wires {
        let [a, b] = *ends.get(w)?;
        if next.contains_key(&a) && next.contains_key(&b) && a != b {
            if next.get(&a).is_some_and(|s| s.contains(&b)) {
                return None;
            }
            next.entry(a).or_default().insert(b);
            next.entry(b).or_default().insert(a);
            edges += 1;
        }
    }
    if edges + 1 != taps.len() || next.values().any(|s| s.len() > 2) {
        return None;
    }
    let first = *next.iter().find(|(_, s)| s.len() <= 1)?.0;
    let mut order = vec![first];
    let mut prev: Option<End> = None;
    let mut here = first;
    while let Some(&n) = next.get(&here)?.iter().find(|n| Some(**n) != prev) {
        order.push(n);
        prev = Some(here);
        here = n;
        if order.len() > taps.len() {
            return None;
        }
    }
    (order.len() == taps.len()).then_some(order)
}

/// One candidate line of a trunk: horizontal (`y`) or vertical (`x`) at `at`, and whether the taps lie on it now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Line {
    /// Whether it runs across (its points share `y`).
    pub horizontal: bool,
    /// Its coordinate across.
    pub at: Tick,
    /// Whether the junction taps lie on it now.
    pub now: bool,
}

impl Line {
    /// The point of the line across from `p`.
    pub fn foot(self, p: Pt) -> Pt {
        if self.horizontal {
            Pt::new(p.x, self.at)
        } else {
            Pt::new(self.at, p.y)
        }
    }

    /// The coordinate of `p` along the line.
    pub fn along(self, p: Pt) -> Tick {
        if self.horizontal { p.x } else { p.y }
    }

    /// The coordinate of `p` across the line.
    pub fn across(self, p: Pt) -> Tick {
        if self.horizontal { p.y } else { p.x }
    }

    /// The side a pin at `p` must face to drop straight onto the line.
    pub fn facing(self, p: Pt) -> Option<Side> {
        let c = self.across(p);
        match (self.horizontal, self.at.cmp(&c)) {
            (_, std::cmp::Ordering::Equal) => None,
            (true, std::cmp::Ordering::Less) => Some(Side::Up),
            (true, std::cmp::Ordering::Greater) => Some(Side::Down),
            (false, std::cmp::Ordering::Less) => Some(Side::Left),
            (false, std::cmp::Ordering::Greater) => Some(Side::Right),
        }
    }
}

/// The candidate lines of a trunk (module documentation) for the pins `pins` of a net (point and side), its junction
/// taps standing at `junctions`, within `[lo, hi]` across on each axis: horizontal first, each axis in the order of the
/// rules, each line once.
pub fn candidates(pins: &[(Pt, Option<Side>)], junctions: &[Pt], bounds: (Pt, Pt)) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let grid = |v: Tick| v.div_euclid(TICKS_PER_STEP) * TICKS_PER_STEP;
    for horizontal in [true, false] {
        let (lo, hi) = if horizontal {
            (bounds.0.y, bounds.1.y)
        } else {
            (bounds.0.x, bounds.1.x)
        };
        let mut push = |at: Tick, now: bool| {
            if (lo..=hi).contains(&at) && !out.iter().any(|l| l.horizontal == horizontal && l.at == at) {
                out.push(Line { horizontal, at, now });
            }
        };
        let across = |p: Pt| if horizontal { p.y } else { p.x };
        // The line the junction taps lie on now.
        if let Some(first) = junctions.first()
            && junctions.iter().all(|j| across(*j) == across(*first))
        {
            push(across(*first), true);
        }
        // The line of each pin whose side runs along the axis.
        let along = |s: Side| s.vertical() != horizontal;
        for (p, s) in pins {
            if s.is_some_and(along) {
                push(across(*p), false);
            }
        }
        // Beyond the outermost pin on each side the pins face, then outward.
        let (before, after) = if horizontal {
            (Side::Up, Side::Down)
        } else {
            (Side::Left, Side::Right)
        };
        let facing = |want: Side| pins.iter().filter(move |(_, s)| *s == Some(want)).map(|(p, _)| across(*p));
        if let Some(min) = facing(before).min() {
            for k in 0..8 {
                push(grid(min - TRUNK_GAP - k * TRUNK_PITCH), false);
            }
        }
        if let Some(max) = facing(after).max() {
            for k in 0..8 {
                push(grid(max + TRUNK_GAP + k * TRUNK_PITCH + TICKS_PER_STEP - 1), false);
            }
        }
        // The middle between two rows that face each other.
        let rows_after: BTreeSet<Tick> = facing(after).collect();
        let rows_before: BTreeSet<Tick> = facing(before).collect();
        for a in &rows_after {
            for b in &rows_before {
                if a < b {
                    push(grid(a + (b - a) / 2), false);
                }
            }
        }
    }
    out
}

/// The points of a single tap's candidates: its present place, and each crossing of a line through one end along its
/// side with a line through another end along its side, on the grid, each once, the present place first.
pub fn tap_points(present: Pt, ends: &[(Pt, Option<Side>)]) -> Vec<Pt> {
    let mut out = vec![present];
    let grid = |v: Tick| v.div_euclid(TICKS_PER_STEP) * TICKS_PER_STEP;
    for (i, (a, sa)) in ends.iter().enumerate() {
        for (k, (b, sb)) in ends.iter().enumerate() {
            if i == k {
                continue;
            }
            // A vertical line through a (its side up or down, or none) and a horizontal one through b.
            let va = sa.is_none_or(Side::vertical);
            let hb = sb.is_none_or(|s| !s.vertical());
            if va && hb {
                let p = Pt::new(grid(a.x), grid(b.y));
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

/// The pin ends of a net that are taps: a pin two or more of its wires end at.
pub fn pin_taps(net: &Net) -> Vec<PinId> {
    net.taps
        .iter()
        .filter_map(|t| match t {
            End::Pin(p) => Some(*p),
            End::Junction(_) => None,
        })
        .collect()
}
