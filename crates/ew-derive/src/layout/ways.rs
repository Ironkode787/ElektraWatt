// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The ways of a cabinet (doc 12 §8, §9): the ducts of a surface as a graph ([`DuctGraph`]), the one pass that lays
//! the cabinet's wires ([`plan_routes`]), the run of a stored way measured again ([`measure`]) and whether a stored
//! way still holds ([`route_status`]).
//!
//! Every length is a sum of coordinate differences in integer nanometres: carriers are axis-parallel and turns are
//! quarter turns. A way is the shortest path from one pin's entries into the ducts to the other's, the direct way over
//! the fronts included; ties go to the lower duct ID, then the lower position along it (the graph's node order), so
//! the same cabinet gives the same ways in every order of its records. The search shares nothing with the router of
//! schematic pages (`ew-drafting`), whose lattice and prices are a page's.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::sync::Arc;

use ew_ids::{Id, v5};
use ew_rds::Aspect;
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CommittedRoute, Connection, Device, DuctStretch, Enclosure, Extra,
    Function, LayoutSpace, MountingSurface, Placement, RouteEnd, SurfaceKind, WayStep,
};
use ew_units::{Nm, Pt2, Rect};

use super::check_path;
use super::fill::{inner_area, wire_area};
use super::pins::{PinSpot, PinSpots, placement_of_function};
use super::queries::{CarrierGeometry, RailOccupancy};
use crate::query::{Policy, Query};
use crate::structure::StructureOf;
use crate::wire_record::{WireRecord, WireRecordSet};
use crate::wires::{EndKind, PhysicalWire};
use crate::{Q, cut_to};

// ---------------------------------------------------------------------------------------------------------
// The shop's numbers (doc 12 §8.6, §9): one reader each

/// The reserve a fitter adds at each end of a routed wire when the project sets none: 50 mm (PM §6.8: a fixed reserve
/// of 50 to 150 mm per wire; unverified).
pub const RESERVE: Nm = Nm(50_000_000);

/// The duct fill limit when the project sets none: 60 % of a duct's inner area (PM §5.3; unverified).
pub const FILL_LIMIT: u8 = 60;

/// The loop a wire into a door keeps across the hinge when the project sets none: 300 mm (PM §6.7; unverified).
pub const DOOR_LOOP: Nm = Nm(300_000_000);

/// The reserve per end in force: `workshop.reserve`, else [`RESERVE`].
pub fn reserve(q: &Q) -> Nm {
    q.project()
        .settings
        .workshop
        .reserve
        .filter(|n| n.0 >= 0)
        .unwrap_or(RESERVE)
}

/// The duct fill limit in force, in percent: `workshop.fill_limit` (1 … 100), else [`FILL_LIMIT`].
pub fn fill_limit(q: &Q) -> u8 {
    q.project()
        .settings
        .workshop
        .fill_limit
        .filter(|n| (1..=100).contains(n))
        .unwrap_or(FILL_LIMIT)
}

/// The door loop in force: `workshop.door_loop`, else [`DOOR_LOOP`].
pub fn door_loop(q: &Q) -> Nm {
    q.project()
        .settings
        .workshop
        .door_loop
        .filter(|n| n.0 >= 0)
        .unwrap_or(DOOR_LOOP)
}

/// The length of a wire laid along a way of `run` (doc 12 §9): the run, the reserve at both ends and the door loop
/// for a door way, rounded up once to the cut increment.
pub fn routed_length(q: &Q, run: Nm, door: bool) -> Nm {
    let loop_ = if door { i128::from(door_loop(q).0) } else { 0 };
    let raw = i128::from(run.0) + 2 * i128::from(reserve(q).0) + loop_;
    Nm(i64::try_from(cut_to(q, raw)).unwrap_or(i64::MAX))
}

// ---------------------------------------------------------------------------------------------------------
// The ducts of a surface as a graph

/// One duct's centreline (doc 12 §8.1): its path in the surface's frame, length, width and height.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DuctLine {
    /// The duct.
    pub carrier: Id<Carrier>,
    /// Its path's points.
    pub path: Vec<(i128, i128)>,
    /// Its length along the path.
    pub len: i128,
    /// Its nominal width across.
    pub width: i128,
    /// Its height out of the surface: wires lie in it at half of it.
    pub height: i128,
    /// Its outline, per segment.
    pub outline: Vec<Rect>,
}

impl DuctLine {
    /// The duct's centreline, `None` for a carrier that is no duct, a duct of a newer form or one with an invalid path.
    pub fn of(q: &Q, carrier: Id<Carrier>) -> Option<DuctLine> {
        let c = q.table::<Carrier>().get(carrier)?;
        let CarrierParams::Duct(d) = &c.params else {
            return None;
        };
        if c.kind != CarrierKind::Duct || check_path(&c.kind, &c.path).is_err() {
            return None;
        }
        let path: Vec<(i128, i128)> = c
            .path
            .iter()
            .map(|p| (i128::from(p.x.0), i128::from(p.y.0)))
            .collect();
        let len = path.windows(2).map(|s| l1(s[0], s[1])).sum();
        let outline = q
            .get::<CarrierGeometry>(carrier)
            .as_ref()
            .as_ref()
            .map(|g| g.outline.clone())
            .unwrap_or_default();
        Some(DuctLine {
            carrier,
            path,
            len,
            width: i128::from(d.width.0),
            height: i128::from(d.height.0),
            outline,
        })
    }

    /// Where a segment starts along the path.
    fn start_of(&self, seg: usize) -> i128 {
        self.path.windows(2).take(seg).map(|s| l1(s[0], s[1])).sum()
    }

    /// The segment that holds `pos` (the first of two at a vertex) and the point there; `None` outside `[0, len]`.
    pub fn at(&self, pos: i128) -> Option<(usize, (i128, i128))> {
        if !(0..=self.len).contains(&pos) {
            return None;
        }
        let mut s = 0;
        for (i, w) in self.path.windows(2).enumerate() {
            let l = l1(w[0], w[1]);
            if pos <= s + l {
                let t = pos - s;
                let (a, b) = (w[0], w[1]);
                return Some((
                    i,
                    (a.0 + (b.0 - a.0).signum() * t, a.1 + (b.1 - a.1).signum() * t),
                ));
            }
            s += l;
        }
        None
    }

    /// The position along the path of a point on segment `seg`.
    fn pos_on(&self, seg: usize, p: (i128, i128)) -> i128 {
        self.start_of(seg) + l1(self.path[seg], p)
    }

    /// The points of the centreline from position `from` to `to`, its vertices between them included; empty when
    /// either lies outside the path.
    pub fn stretch(&self, from: i128, to: i128) -> Vec<(i128, i128)> {
        let (Some((_, a)), Some((_, b))) = (self.at(from), self.at(to)) else {
            return Vec::new();
        };
        let mut s = 0;
        let mut inner: Vec<(i128, (i128, i128))> = Vec::new();
        for w in self.path.windows(2) {
            s += l1(w[0], w[1]);
            if s > from.min(to) && s < from.max(to) {
                inner.push((s, w[1]));
            }
        }
        if from > to {
            inner.reverse();
        }
        std::iter::once(a)
            .chain(inner.into_iter().map(|(_, p)| p))
            .chain(std::iter::once(b))
            .collect()
    }
}

/// The points of a stored or planned way in its surface's frame, for the window's marks (doc 12 §12.5): from the
/// first pin into its duct, along the ducts and their joints, out to the other pin; a direct way across, then along.
pub fn way_points(q: &Q, ends: &[RouteEnd; 2], way: &[WayStep]) -> Vec<Pt2> {
    let pt = |(x, y): (i128, i128)| {
        Pt2::new(
            Nm(i64::try_from(x).unwrap_or(0)),
            Nm(i64::try_from(y).unwrap_or(0)),
        )
    };
    let mut out = vec![ends[0].at];
    let stretches: Vec<&DuctStretch> = way
        .iter()
        .filter_map(|s| match s {
            WayStep::Duct(d) => Some(d),
            _ => None,
        })
        .collect();
    if stretches.is_empty() {
        out.push(Pt2::new(ends[1].at.x, ends[0].at.y));
    }
    for s in stretches {
        if let Some(line) = DuctLine::of(q, s.carrier) {
            out.extend(
                line.stretch(i128::from(s.from.0), i128::from(s.to.0))
                    .into_iter()
                    .map(pt),
            );
        }
    }
    out.push(ends[1].at);
    out.dedup();
    out
}

/// A joint of two ducts (doc 12 §8.1): where their outlines touch or overlap, at the point of the one's centreline
/// nearest the other's; the stretch between the two points belongs to the joint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Joint {
    /// The first duct (its index in [`DuctGraphV::ducts`]) and the position along it.
    pub a: (usize, i128),
    /// The second duct and the position along it.
    pub b: (usize, i128),
    /// The length of the stretch between the two points.
    pub len: i128,
}

/// The value of [`DuctGraph`]: a surface's ducts in ID order and their joints.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DuctGraphV {
    /// The ducts, by ID.
    pub ducts: Vec<DuctLine>,
    /// Their joints, by the pair of ducts.
    pub joints: Vec<Joint>,
}

impl DuctGraphV {
    /// The index of a duct.
    pub fn index(&self, carrier: Id<Carrier>) -> Option<usize> {
        self.ducts.iter().position(|d| d.carrier == carrier)
    }
}

/// `duct_graph(surface)`: the ducts of a mounting surface as a graph of their centrelines (doc 12 §8.1).
#[derive(Debug)]
pub struct DuctGraph;

impl Query for DuctGraph {
    type Key = Id<MountingSurface>;
    type Value = DuctGraphV;
    const FAMILY: &'static str = "duct_graph";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<DuctGraphV> {
        let mut ids: Vec<Id<Carrier>> = q.idx().carriers_of_surface(*key).collect();
        ids.sort();
        let ducts: Vec<DuctLine> = ids.into_iter().filter_map(|c| DuctLine::of(q, c)).collect();
        let mut joints = Vec::new();
        for i in 0..ducts.len() {
            for j in i + 1..ducts.len() {
                if let Some(joint) = joint(&ducts[i], &ducts[j], i, j) {
                    joints.push(joint);
                }
            }
        }
        Arc::new(DuctGraphV { ducts, joints })
    }
}

fn l1(a: (i128, i128), b: (i128, i128)) -> i128 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// The closest values of two intervals: the lowest common value where they overlap, else their nearest ends.
fn closest_1d(a: (i128, i128), b: (i128, i128)) -> (i128, i128) {
    let (a0, a1) = (a.0.min(a.1), a.0.max(a.1));
    let (b0, b1) = (b.0.min(b.1), b.0.max(b.1));
    if a1 < b0 {
        (a1, b0)
    } else if b1 < a0 {
        (a0, b1)
    } else {
        let v = a0.max(b0);
        (v, v)
    }
}

/// The joint of ducts `a` and `b` (indices `i`, `j`): `None` when their outlines neither touch nor overlap.
fn joint(a: &DuctLine, b: &DuctLine, i: usize, j: usize) -> Option<Joint> {
    let touch = a
        .outline
        .iter()
        .any(|ra| b.outline.iter().any(|rb| ra.intersects(*rb)));
    if !touch {
        return None;
    }
    let mut best: Option<(i128, i128, i128)> = None;
    for (sa, wa) in a.path.windows(2).enumerate() {
        for (sb, wb) in b.path.windows(2).enumerate() {
            let (xa, xb) = closest_1d((wa[0].0, wa[1].0), (wb[0].0, wb[1].0));
            let (ya, yb) = closest_1d((wa[0].1, wa[1].1), (wb[0].1, wb[1].1));
            let (pa, pb) = ((xa, ya), (xb, yb));
            let cand = (l1(pa, pb), a.pos_on(sa, pa), b.pos_on(sb, pb));
            if best.is_none_or(|b| cand < b) {
                best = Some(cand);
            }
        }
    }
    let (len, pa, pb) = best?;
    Some(Joint {
        a: (i, pa),
        b: (j, pb),
        len,
    })
}

// ---------------------------------------------------------------------------------------------------------
// Into a duct

/// One way from a pin into a duct (doc 12 §8.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry {
    /// The duct's index in the graph.
    pub duct: usize,
    /// Where the line meets its centreline, along its path.
    pub pos: i128,
    /// The distance from the pin to the centreline.
    pub stub: i128,
    /// The difference between the pin's height and the duct's half height.
    pub climb: i128,
}

/// The entries of a pin: straight out of its edge to the first duct that line meets, and back across its own
/// device's front to the first duct on the other side; a line that meets another footprint first is not taken.
/// `own` is the pin's footprint, `others` the other footprints of its surface.
pub fn entries(graph: &DuctGraphV, spot: &PinSpot, own: Rect, others: &[Rect]) -> Vec<Entry> {
    let p = (i128::from(spot.at.x.0), i128::from(spot.at.y.0));
    let out = (i128::from(spot.out.0), i128::from(spot.out.1));
    let mut found = Vec::new();
    for dir in [out, (-out.0, -out.1)] {
        if dir == (0, 0) {
            continue;
        }
        // The nearest centreline the line crosses: (distance, duct, position).
        let mut best: Option<(i128, usize, i128)> = None;
        for (i, d) in graph.ducts.iter().enumerate() {
            for (s, w) in d.path.windows(2).enumerate() {
                let (a, b) = (w[0], w[1]);
                // A segment across the line: horizontal for a vertical line, vertical for a horizontal one.
                let (across, along_lo, along_hi, c, pc, pa) = if dir.0 == 0 {
                    (a.1 == b.1, a.0.min(b.0), a.0.max(b.0), a.1, p.1, p.0)
                } else {
                    (a.0 == b.0, a.1.min(b.1), a.1.max(b.1), a.0, p.0, p.1)
                };
                if !across || pa < along_lo || pa > along_hi {
                    continue;
                }
                let t = (c - pc) * (dir.0 + dir.1);
                if t <= 0 {
                    continue;
                }
                let foot = if dir.0 == 0 { (p.0, c) } else { (c, p.1) };
                let cand = (t, i, d.pos_on(s, foot));
                if best.is_none_or(|b| cand < b) {
                    best = Some(cand);
                }
            }
        }
        let Some((t, i, pos)) = best else { continue };
        let reach = (t - graph.ducts[i].width / 2).max(0);
        let end = (p.0 + dir.0 * reach, p.1 + dir.1 * reach);
        let blocked = others.iter().filter(|r| **r != own).any(|r| crosses(*r, p, end));
        if !blocked {
            found.push(Entry {
                duct: i,
                pos,
                stub: t,
                climb: (i128::from(spot.h.0) - graph.ducts[i].height / 2).abs(),
            });
        }
    }
    found
}

/// Whether the axis-parallel line from `a` to `b` runs through the inside of `r`.
fn crosses(r: Rect, a: (i128, i128), b: (i128, i128)) -> bool {
    let (x0, x1) = (i128::from(r.min().x.0), i128::from(r.max().x.0));
    let (y0, y1) = (i128::from(r.min().y.0), i128::from(r.max().y.0));
    let (lx, hx) = (a.0.min(b.0), a.0.max(b.0));
    let (ly, hy) = (a.1.min(b.1), a.1.max(b.1));
    if a.0 == b.0 {
        x0 < a.0 && a.0 < x1 && ly < y1 && y0 < hy
    } else {
        y0 < a.1 && a.1 < y1 && lx < x1 && x0 < hx
    }
}

// ---------------------------------------------------------------------------------------------------------
// The run of a way

/// The perpendicular distance from `at` to the centreline of the segment of `duct` that holds `pos`.
fn hop(duct: &DuctLine, pos: i128, at: Pt2) -> Option<i128> {
    let (seg, _) = duct.at(pos)?;
    let (a, b) = (duct.path[seg], duct.path[seg + 1]);
    let (x, y) = (i128::from(at.x.0), i128::from(at.y.0));
    Some(if a.1 == b.1 {
        (y - a.1).abs()
    } else {
        (x - a.0).abs()
    })
}

/// The run of a way along today's ducts (doc 12 §8.5): from each end the distance to its duct's centreline and the climb
/// to its half height, the stretches along the ducts, the joints between them; for a direct way (no steps) across
/// plus along plus the difference of the two heights. `None` when a duct of the way is gone or no longer holds a
/// position of it ([`StaleWhy::DuctGone`], [`StaleWhy::DuctChanged`]).
pub fn measure(q: &Q, ends: &[RouteEnd; 2], way: &[WayStep]) -> Result<Nm, StaleWhy> {
    let stretches: Vec<&DuctStretch> = way
        .iter()
        .filter_map(|s| match s {
            WayStep::Duct(d) => Some(d),
            _ => None,
        })
        .collect();
    let (e0, e1) = (&ends[0], &ends[1]);
    let climb = |e: &RouteEnd, d: &DuctLine| (i128::from(e.h.0) - d.height / 2).abs();
    let run: i128 = if stretches.is_empty() {
        l1(
            (i128::from(e0.at.x.0), i128::from(e0.at.y.0)),
            (i128::from(e1.at.x.0), i128::from(e1.at.y.0)),
        ) + (i128::from(e0.h.0) - i128::from(e1.h.0)).abs()
    } else {
        let mut lines: Vec<(DuctLine, &DuctStretch)> = Vec::new();
        for s in &stretches {
            if !q.table::<Carrier>().contains(s.carrier) {
                return Err(StaleWhy::DuctGone(s.carrier));
            }
            let line = DuctLine::of(q, s.carrier).ok_or(StaleWhy::DuctChanged(s.carrier))?;
            lines.push((line, s));
        }
        let changed = |c: Id<Carrier>| StaleWhy::DuctChanged(c);
        let (first, f) = &lines[0];
        let (last, l) = &lines[lines.len() - 1];
        let mut run = hop(first, i128::from(f.from.0), e0.at).ok_or(changed(f.carrier))? + climb(e0, first);
        run += hop(last, i128::from(l.to.0), e1.at).ok_or(changed(l.carrier))? + climb(e1, last);
        for (k, (line, s)) in lines.iter().enumerate() {
            let (from, to) = (i128::from(s.from.0), i128::from(s.to.0));
            let (_, pf) = line.at(from).ok_or(changed(s.carrier))?;
            line.at(to).ok_or(changed(s.carrier))?;
            run += (to - from).abs();
            if k > 0 {
                let (prev, ps) = &lines[k - 1];
                let (_, pt) = prev.at(i128::from(ps.to.0)).ok_or(changed(ps.carrier))?;
                run += l1(pt, pf);
            }
        }
        run
    };
    Ok(Nm(i64::try_from(run).unwrap_or(i64::MAX)))
}

// ---------------------------------------------------------------------------------------------------------
// Whether a stored way holds

/// Why a stored way no longer holds (doc 12 §8.5); the words are [`crate::text::stale_why`]'s.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StaleWhy {
    /// An end's pin is not where it was: its device moved, turned, went to another surface or rail, or took another
    /// part.
    Moved(Id<Device>),
    /// The wire now lands on another pin at an end.
    EndChanged {
        /// The pin it was laid from.
        from: (Id<Function>, u16),
        /// The pin it lands on now.
        to: (Id<Function>, u16),
    },
    /// An end's device is no longer placed.
    Unplaced(Id<Device>),
    /// A duct of its way moved or changed its size: its run measured again is another.
    DuctChanged(Id<Carrier>),
    /// A duct of its way is gone.
    DuctGone(Id<Carrier>),
}

/// The status of a wire's stored way ([`route_status`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RouteStatus {
    /// No way is stored for the wire.
    None,
    /// The stored way holds: its run, and whether it crosses into a door.
    Holds {
        /// The stored run.
        run: Nm,
        /// Whether the way crosses a door's hinge.
        door: bool,
    },
    /// The stored way no longer holds.
    Stale {
        /// The stored run.
        run: Nm,
        /// Whether the way crosses a door's hinge.
        door: bool,
        /// Why.
        why: StaleWhy,
    },
}

/// The stored way of a wire, when one is read: the record `v5(wire, "route")` whose wire is that wire and whose
/// space exists.
pub fn stored_way(q: &Q, wire: Id<Connection>) -> Option<Arc<CommittedRoute>> {
    let r = q.table::<CommittedRoute>().get(v5::route_id(wire))?;
    (r.wire == wire && q.table::<LayoutSpace>().contains(r.space)).then(|| r.clone())
}

/// `route_status(wire)`: whether the wire's stored way holds (doc 12 §8.5): its ends are the pins as they are now, and
/// its run, measured again along its steps over today's ducts, is the run it stored. The first cause found is named:
/// an end that moved to another pin, a device no longer placed, a pin that moved, a duct gone, a duct changed.
pub fn route_status(q: &Q, wire: &PhysicalWire) -> RouteStatus {
    let Some(r) = stored_way(q, wire.id) else {
        return RouteStatus::None;
    };
    let door = r.way.iter().any(|s| matches!(s, WayStep::Door(_)));
    let stale = |why| RouteStatus::Stale {
        run: r.run,
        door,
        why,
    };
    let now: Vec<(Id<Function>, u16)> = wire.ends.iter().map(|e| (e.cp.function, e.cp.idx)).collect();
    let was: Vec<(Id<Function>, u16)> = r.ends.iter().map(|e| e.cp).collect();
    if let (Some(from), Some(to)) = (
        was.iter().find(|c| !now.contains(c)),
        now.iter().find(|c| !was.contains(c)),
    ) {
        return stale(StaleWhy::EndChanged { from: *from, to: *to });
    }
    for end in &r.ends {
        // The device the layout places: a terminal's strip, else the function's device.
        let device = q.functions().get(end.cp.0).and_then(|f| f.device).map(|d| {
            q.devices()
                .get(d)
                .and_then(|x| x.aspects.terminal.as_ref().map(|t| t.strip))
                .unwrap_or(d)
        });
        let spots = placement_of_function(q, end.cp.0).and_then(|p| q.get::<PinSpots>(p).as_ref().clone());
        let Some(spots) = spots else {
            return match device {
                Some(d) => stale(StaleWhy::Unplaced(d)),
                None => stale(StaleWhy::EndChanged {
                    from: end.cp,
                    to: end.cp,
                }),
            };
        };
        let same = spots.surface == end.surface
            && spots.spot(end.cp).is_some_and(|s| s.at == end.at && s.h == end.h);
        if !same {
            if let Some(d) = device {
                return stale(StaleWhy::Moved(d));
            }
        }
    }
    match measure(q, &r.ends, &r.way) {
        Ok(run) if run == r.run => RouteStatus::Holds { run: r.run, door },
        Ok(_) => {
            let duct = r.way.iter().find_map(|s| match s {
                WayStep::Duct(d) => Some(StaleWhy::DuctChanged(d.carrier)),
                _ => None,
            });
            // A direct way's run is its ends': one that differs was stored by another rule; its first end names it.
            let device = || {
                q.functions()
                    .get(r.ends[0].cp.0)
                    .and_then(|f| f.device)
                    .map(StaleWhy::Moved)
            };
            match duct.or_else(device) {
                Some(why) => stale(why),
                None => RouteStatus::None,
            }
        }
        Err(why) => stale(why),
    }
}

// ---------------------------------------------------------------------------------------------------------
// The cabinet's wires

/// What a wire of a cabinet is to Route (doc 12 §8.1, §8.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CabinetWire {
    /// Both ends on devices of the cabinet: Route lays it.
    Routed,
    /// An end outside the cabinet: made on site, no way.
    Site,
    /// A cable's core: cut with its cable, no way.
    Core,
}

/// The cabinet of a layout space: the location node it lays out.
fn location_of_space(q: &Q, space: Id<LayoutSpace>) -> Option<Id<ew_schema::StructureNode>> {
    q.table::<LayoutSpace>().get(space).map(|s| s.location)
}

/// Whether an end of a wire is on a device of the location `loc`.
fn end_in(q: &Q, kind: EndKind, function: Id<Function>, loc: Id<ew_schema::StructureNode>) -> bool {
    kind == EndKind::Device
        && q.functions()
            .get(function)
            .and_then(|f| f.device)
            .and_then(|d| q.try_get::<StructureOf>(d).ok())
            .and_then(|s| s.node(Aspect::Location))
            == Some(loc)
}

/// `cabinet_wires(space)`: the wires of the cabinet a layout space lays out, in the order of the wire list, each with
/// what it is to Route (doc 12 §8.2): a wire with both ends on devices of the space's location is routed, one with an
/// end outside is made on site, a cable's core is cut with its cable. The plan, EW-MFG-001 and the card's counts read
/// it.
pub fn cabinet_wires(q: &Q, space: Id<LayoutSpace>) -> Vec<(WireRecord, CabinetWire)> {
    let Some(loc) = location_of_space(q, space) else {
        return Vec::new();
    };
    q.get::<WireRecordSet>(())
        .iter()
        .filter_map(|r| Some((r.clone(), classify(q, r, loc)?)))
        .collect()
}

/// What a wire is to the cabinet of the location `loc`; `None` for a wire with no end in it.
fn classify(q: &Q, r: &WireRecord, loc: Id<ew_schema::StructureNode>) -> Option<CabinetWire> {
    let inside = r.wire.ends.map(|e| end_in(q, e.kind, e.cp.function, loc));
    Some(match inside {
        [false, false] => return None,
        _ if r.wire.core.is_some() => CabinetWire::Core,
        [true, true] => CabinetWire::Routed,
        _ => CabinetWire::Site,
    })
}

/// The cabinet a wire belongs to and what it is to Route there ([`cabinet_wires`]); `None` for a wire of no cabinet.
pub fn space_of_wire(q: &Q, r: &WireRecord) -> Option<(Id<LayoutSpace>, CabinetWire)> {
    q.table::<LayoutSpace>()
        .iter()
        .find_map(|(space, s)| Some((space, classify(q, r, s.location)?)))
}

/// The first panel page in page order that shows a space's plate: where Route runs for its cabinet.
pub fn cabinet_page(q: &Q, space: Id<LayoutSpace>) -> Option<Id<ew_schema::Page>> {
    let plate = plate_of_space(q, space)?;
    let order = q.get::<crate::DrawnPageOrder>(());
    super::surface_pages(q, plate).into_iter().min_by_key(|p| {
        (
            order.position(crate::PageRef::Drawn(*p)).unwrap_or(usize::MAX),
            *p,
        )
    })
}

// ---------------------------------------------------------------------------------------------------------
// The plan

/// Which wires Route plans (doc 12 §8.5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RouteScope {
    /// The wires without a way and those whose way no longer holds.
    Due,
    /// These wires, whatever their state.
    Wires(BTreeSet<Id<Connection>>),
    /// Every wire of the cabinet.
    All,
}

/// Why a wire of the cabinet gets no way (doc 12 §8.2); the words are [`crate::text::no_way`]'s.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoWay {
    /// An end's device is not placed.
    Unplaced(Id<Device>),
    /// An end's pin meets no duct and the wire has no direct way; the rail its device stands on.
    NoDuct {
        /// The pin.
        cp: (Id<Function>, u16),
        /// The rail of its device, if it stands on one.
        rail: Option<Id<Carrier>>,
    },
    /// An end's device stands on a surface Route does not lay wires on (a side wall).
    OffPlate(Id<Device>),
}

/// A way laid for a wire: its ends, its steps and its run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Way {
    /// The two ends, From first.
    pub ends: [RouteEnd; 2],
    /// The steps; empty for a direct way.
    pub steps: Vec<WayStep>,
    /// The run.
    pub run: Nm,
}

impl Way {
    /// Whether the way crosses a door's hinge.
    pub fn door(&self) -> bool {
        self.steps.iter().any(|s| matches!(s, WayStep::Door(_)))
    }

    /// The record that stores it for `wire` in `space`.
    pub fn record(&self, space: Id<LayoutSpace>, wire: Id<Connection>) -> CommittedRoute {
        CommittedRoute {
            id: v5::route_id(wire),
            v: 1,
            space,
            wire,
            ends: self.ends.clone(),
            way: self.steps.clone(),
            run: self.run,
            extra: Extra::new(),
        }
    }
}

/// What the plan says of one wire of the cabinet.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Planned {
    /// A way laid now.
    Way(Way),
    /// Not planned: its stored way stays.
    Kept,
    /// No way, and why.
    NoWay(NoWay),
    /// Made on site.
    Site,
    /// A cable's core.
    Core,
}

/// One wire of the plan.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlannedWire {
    /// The wire.
    pub wire: WireRecord,
    /// What the plan says of it.
    pub planned: Planned,
}

/// The value of [`plan_routes`]: the cabinet's wires in the order of the wire list, each with what the plan says.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RoutePlan {
    /// The wires.
    pub wires: Vec<PlannedWire>,
    /// The stored ways of the space whose wire is no longer a wire of the cabinet: the next store removes them.
    pub gone: Vec<Id<CommittedRoute>>,
}

/// The plate of a layout space: the plate surface of its enclosure (the smallest ID).
pub fn plate_of_space(q: &Q, space: Id<LayoutSpace>) -> Option<Id<MountingSurface>> {
    let enclosures: BTreeSet<Id<Enclosure>> = q
        .table::<Enclosure>()
        .iter()
        .filter(|(_, e)| e.space == space)
        .map(|(id, _)| id)
        .collect();
    q.table::<MountingSurface>()
        .iter()
        .filter(|(_, s)| enclosures.contains(&s.enclosure) && s.kind == SurfaceKind::Plate)
        .map(|(id, _)| id)
        .min()
}

/// One end of a wire resolved on the layout.
struct End {
    cp: (Id<Function>, u16),
    placement: Id<Placement>,
    spot: PinSpot,
    surface: Id<MountingSurface>,
    footprint: Rect,
}

impl End {
    fn record(&self) -> RouteEnd {
        RouteEnd {
            cp: self.cp,
            surface: self.surface,
            at: self.spot.at,
            h: self.spot.h,
            extra: Extra::new(),
        }
    }
}

/// The room each duct holds: per duct index the stretches `(lo, hi, area)` of the ways laid in it.
type Room = BTreeMap<usize, Vec<(i128, i128, i128)>>;

/// The stretches of a way through the ducts: per stretch the duct's index and the positions it runs from and to.
type Stretches = Vec<(usize, i128, i128)>;

/// Whether the stretch `[a, b]` of duct `i` has room for a wire of `area` under the limit.
fn has_room(room: &Room, i: usize, a: i128, b: i128, area: i128, cap: i128) -> bool {
    let (lo, hi) = (a.min(b), a.max(b));
    let held = room.get(&i).map(Vec::as_slice).unwrap_or(&[]);
    let mut cuts: Vec<i128> = vec![lo, hi];
    cuts.extend(
        held.iter()
            .flat_map(|s| [s.0, s.1])
            .filter(|x| lo < *x && *x < hi),
    );
    cuts.sort_unstable();
    cuts.dedup();
    let pieces: Vec<(i128, i128)> = if cuts.len() < 2 {
        vec![(lo, hi)]
    } else {
        cuts.windows(2).map(|w| (w[0], w[1])).collect()
    };
    pieces.iter().all(|(p0, p1)| {
        let sum: i128 = held
            .iter()
            .filter(|s| s.0 <= *p0 && *p1 <= s.1)
            .map(|s| s.2)
            .sum();
        (sum + area) * 100 <= cap
    })
}

/// The shortest way through the ducts between the entries of two pins, with room under the limit when `cap` is given:
/// its stretches `(duct, from, to)` in order and its cost, or `None`.
fn shortest(
    graph: &DuctGraphV,
    from: &[Entry],
    to: &[Entry],
    room: &Room,
    area: i128,
    caps: Option<&[i128]>,
) -> Option<(Stretches, i128)> {
    // The nodes: per duct its joint and entry positions, in (duct, position) order; then the two ends.
    let mut pos: Vec<Vec<i128>> = vec![Vec::new(); graph.ducts.len()];
    for j in &graph.joints {
        pos[j.a.0].push(j.a.1);
        pos[j.b.0].push(j.b.1);
    }
    for e in from.iter().chain(to) {
        pos[e.duct].push(e.pos);
    }
    for p in &mut pos {
        p.sort_unstable();
        p.dedup();
    }
    let mut base = Vec::with_capacity(pos.len());
    let mut n = 0;
    for p in &pos {
        base.push(n);
        n += p.len();
    }
    let node = |d: usize, x: i128| base[d] + pos[d].binary_search(&x).unwrap_or(0);
    let (s, t) = (n, n + 1);
    let mut adj: Vec<Vec<(usize, i128)>> = vec![Vec::new(); n + 2];
    for (d, p) in pos.iter().enumerate() {
        for k in 1..p.len() {
            let ok = caps.is_none_or(|c| has_room(room, d, p[k - 1], p[k], area, c[d]));
            if ok {
                let (a, b, w) = (base[d] + k - 1, base[d] + k, p[k] - p[k - 1]);
                adj[a].push((b, w));
                adj[b].push((a, w));
            }
        }
    }
    for j in &graph.joints {
        let (a, b) = (node(j.a.0, j.a.1), node(j.b.0, j.b.1));
        adj[a].push((b, j.len));
        adj[b].push((a, j.len));
    }
    for e in from {
        adj[s].push((node(e.duct, e.pos), e.stub + e.climb));
    }
    for e in to {
        adj[node(e.duct, e.pos)].push((t, e.stub + e.climb));
    }
    let mut dist: Vec<Option<i128>> = vec![None; n + 2];
    let mut prev: Vec<Option<usize>> = vec![None; n + 2];
    let mut heap = BinaryHeap::new();
    dist[s] = Some(0);
    heap.push(Reverse((0i128, s)));
    while let Some(Reverse((c, u))) = heap.pop() {
        if dist[u].is_some_and(|d| c > d) {
            continue;
        }
        if u == t {
            break;
        }
        for &(v, w) in &adj[u] {
            let nc = c + w;
            if dist[v].is_none_or(|d| nc < d) {
                dist[v] = Some(nc);
                prev[v] = Some(u);
                heap.push(Reverse((nc, v)));
            }
        }
    }
    let cost = dist[t]?;
    let mut path = Vec::new();
    let mut at = prev[t];
    while let Some(u) = at {
        if u == s {
            break;
        }
        path.push(u);
        at = prev[u];
    }
    path.reverse();
    let owner: Vec<usize> = pos
        .iter()
        .enumerate()
        .flat_map(|(d, p)| std::iter::repeat_n(d, p.len()))
        .collect();
    let mut stretches: Stretches = Vec::new();
    for u in path {
        let d = owner[u];
        let x = pos[d][u - base[d]];
        match stretches.last_mut() {
            Some(last) if last.0 == d => last.2 = x,
            _ => stretches.push((d, x, x)),
        }
    }
    Some((stretches, cost))
}

/// The surfaces Route lays wires on in a space: its plate.
fn route_surfaces(q: &Q, space: Id<LayoutSpace>) -> Vec<Id<MountingSurface>> {
    plate_of_space(q, space).into_iter().collect()
}

/// The footprints of the placements on `surface`.
fn footprints_on(q: &Q, surface: Id<MountingSurface>) -> Vec<(Id<Placement>, Rect)> {
    q.table::<Placement>()
        .iter()
        .filter_map(|(id, _)| {
            let s = q.get::<PinSpots>(id);
            let s = s.as_ref().as_ref()?;
            (s.surface == surface).then_some((id, s.footprint))
        })
        .collect()
}

/// The rail a placement stands on.
fn rail_of(q: &Q, placement: Id<Placement>) -> Option<Id<Carrier>> {
    match q.table::<Placement>().get(placement)?.carrier {
        ew_schema::CarrierRef::Carrier(c) => Some(c),
        _ => None,
    }
}

/// Whether two placements stand side by side on one rail, no footprint between them (or are one).
fn neighbours(q: &Q, a: Id<Placement>, b: Id<Placement>) -> bool {
    if a == b {
        return true;
    }
    let (Some(ra), Some(rb)) = (rail_of(q, a), rail_of(q, b)) else {
        return false;
    };
    if ra != rb {
        return false;
    }
    let occ = q.get::<RailOccupancy>(ra);
    let at = |p| occ.iter().position(|o| o.placement == p);
    matches!((at(a), at(b)), (Some(x), Some(y)) if x.abs_diff(y) == 1)
}

/// `plan_routes(space, scope)`: the one pass of doc 12 §8.4 over the cabinet of `space`. The planned wires are laid one
/// after another in the order of the wire list; each takes the shortest way along which no piece of duct goes over the
/// fill limit with it, or the direct way where that is shorter; where no way has room it takes the shortest all the
/// same. The ways not planned hold their room. Run by `layout.route` and by Route's preview.
pub fn plan_routes(q: &Q, space: Id<LayoutSpace>, scope: &RouteScope) -> RoutePlan {
    let wires = cabinet_wires(q, space);
    let surfaces = route_surfaces(q, space);
    let Some(&plate) = surfaces.first() else {
        return RoutePlan {
            wires: wires
                .into_iter()
                .map(|(wire, kind)| PlannedWire {
                    planned: match kind {
                        CabinetWire::Site => Planned::Site,
                        CabinetWire::Core => Planned::Core,
                        CabinetWire::Routed => Planned::Kept,
                    },
                    wire,
                })
                .collect(),
            gone: Vec::new(),
        };
    };
    let graph = q.get::<DuctGraph>(plate);
    let limit = i128::from(fill_limit(q));
    let caps: Vec<i128> = graph
        .ducts
        .iter()
        .map(|d| limit * inner_area(q, d.carrier).0)
        .collect();
    let footprints: Vec<Rect> = footprints_on(q, plate).into_iter().map(|(_, r)| r).collect();
    let planned = |r: &WireRecord| match scope {
        RouteScope::All => true,
        RouteScope::Wires(set) => set.contains(&r.wire.id),
        RouteScope::Due => !matches!(route_status(q, &r.wire), RouteStatus::Holds { .. }),
    };
    // The room the ways not planned hold.
    let mut room: Room = BTreeMap::new();
    for (r, kind) in &wires {
        if *kind != CabinetWire::Routed || planned(r) {
            continue;
        }
        let (Some(way), Some((area, _))) = (stored_way(q, r.wire.id), wire_area(q, r)) else {
            continue;
        };
        hold(&mut room, &graph, &way.way, area);
    }
    let mut out = Vec::with_capacity(wires.len());
    for (r, kind) in wires {
        let p = match kind {
            CabinetWire::Site => Planned::Site,
            CabinetWire::Core => Planned::Core,
            CabinetWire::Routed if !planned(&r) => Planned::Kept,
            CabinetWire::Routed => {
                let area = wire_area(q, &r).map_or(0, |a| a.0);
                match lay(q, &r, plate, &graph, &footprints, &room, area, &caps) {
                    Ok(way) => {
                        hold(&mut room, &graph, &way.steps, area);
                        Planned::Way(way)
                    }
                    Err(why) => Planned::NoWay(why),
                }
            }
        };
        out.push(PlannedWire { wire: r, planned: p });
    }
    let current: BTreeSet<Id<Connection>> = out.iter().map(|w| w.wire.wire.id).collect();
    let gone = q
        .table::<CommittedRoute>()
        .iter()
        .filter(|(id, c)| c.space == space && (!current.contains(&c.wire) || *id != v5::route_id(c.wire)))
        .map(|(id, _)| id)
        .collect();
    RoutePlan { wires: out, gone }
}

/// Adds the stretches of a way to the room its ducts hold.
fn hold(room: &mut Room, graph: &DuctGraphV, way: &[WayStep], area: i128) {
    for s in way {
        if let WayStep::Duct(d) = s
            && let Some(i) = graph.index(d.carrier)
        {
            let (a, b) = (i128::from(d.from.0), i128::from(d.to.0));
            room.entry(i).or_default().push((a.min(b), a.max(b), area));
        }
    }
}

/// Resolves an end of a wire on the layout.
fn resolve(q: &Q, cp: (Id<Function>, u16), plate: Id<MountingSurface>) -> Result<End, NoWay> {
    let device = q.functions().get(cp.0).and_then(|f| f.device);
    let unplaced = || NoWay::Unplaced(device.unwrap_or(Id::from_uuid(ew_ids::Uuid::nil())));
    let placement = placement_of_function(q, cp.0).ok_or_else(unplaced)?;
    let spots = q.get::<PinSpots>(placement);
    let spots = spots.as_ref().as_ref().ok_or_else(unplaced)?;
    if spots.surface != plate {
        return Err(NoWay::OffPlate(
            q.table::<Placement>().get(placement).map_or_else(
                || device.unwrap_or(Id::from_uuid(ew_ids::Uuid::nil())),
                |p| p.device,
            ),
        ));
    }
    let spot = spots.spot(cp).cloned().ok_or(NoWay::NoDuct {
        cp,
        rail: rail_of(q, placement),
    })?;
    Ok(End {
        cp,
        placement,
        spot,
        surface: spots.surface,
        footprint: spots.footprint,
    })
}

/// Lays one wire: the shortest way with room, or the direct way where it is shorter, else the shortest way.
#[allow(clippy::too_many_arguments)]
fn lay(
    q: &Q,
    r: &WireRecord,
    plate: Id<MountingSurface>,
    graph: &DuctGraphV,
    footprints: &[Rect],
    room: &Room,
    area: i128,
    caps: &[i128],
) -> Result<Way, NoWay> {
    let [a, b] = r.wire.ends.map(|e| (e.cp.function, e.cp.idx));
    let (ea, eb) = (resolve(q, a, plate)?, resolve(q, b, plate)?);
    let ends = [ea.record(), eb.record()];
    let direct = neighbours(q, ea.placement, eb.placement)
        .then(|| measure(q, &ends, &[]).ok())
        .flatten();
    let na = entries(graph, &ea.spot, ea.footprint, footprints);
    let nb = entries(graph, &eb.spot, eb.footprint, footprints);
    let steps = |stretches: Stretches| -> Vec<WayStep> {
        stretches
            .into_iter()
            .map(|(d, from, to)| {
                WayStep::Duct(DuctStretch {
                    carrier: graph.ducts[d].carrier,
                    from: Nm(i64::try_from(from).unwrap_or(0)),
                    to: Nm(i64::try_from(to).unwrap_or(0)),
                    extra: Extra::new(),
                })
            })
            .collect()
    };
    let with_room = shortest(graph, &na, &nb, room, area, Some(caps));
    let chosen: Option<Vec<WayStep>> = match (with_room, direct) {
        (Some((s, c)), Some(d)) if i128::from(d.0) >= c => Some(steps(s)),
        (Some((s, _)), None) => Some(steps(s)),
        (_, Some(_)) => Some(Vec::new()),
        (None, None) => shortest(graph, &na, &nb, room, area, None).map(|(s, _)| steps(s)),
    };
    let Some(steps) = chosen else {
        let (end, spot) = if na.is_empty() {
            (&ea, &ea.spot)
        } else {
            (&eb, &eb.spot)
        };
        return Err(NoWay::NoDuct {
            cp: spot.cp,
            rail: rail_of(q, end.placement),
        });
    };
    let run = measure(q, &ends, &steps).unwrap_or(Nm::ZERO);
    Ok(Way { ends, steps, run })
}
