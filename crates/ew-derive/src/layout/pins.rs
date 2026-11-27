// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `pin_spots(p)`: where the pins of a placed device stand on its surface, the way out of each and its height
//! (doc 12 §8.1). The ways (`super::ways`), their status and the overlays of the window read the pins from here.
//!
//! A pin stands on the top or the bottom edge of its device's footprint in the part's frame, turned with the part:
//! the edge its part's pin names (`CpData.face`), else the edge the drawing shows (a wire leaving it upwards on the
//! first page in page order that draws it: the top edge, any other: the bottom edge). The pins of one edge share it
//! evenly in the order of the part's pins (of the function CPs where the part names none): the k-th of n at
//! (k + ½)/n. A terminal's side pins stand on its own width within its strip. A pin's height above the surface is the
//! rail's height plus half its device's depth (the rail's alone where the depth is not known). A pin in a door stands
//! at its device's centre, half what the device reaches behind the door (`behind_door`) away from it, with no way out
//! of an edge.

use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{
    Carrier, CarrierParams, CarrierRef, Device, Dir4, Function, MountingSurface, PartRef, PinFace, Placement,
    SurfaceKind,
};
use ew_units::{Nm, Pt2, Rect};

use super::Profile;
use super::queries::{PlacementGeometry, StripItemKind, StripLayout, places_strip};
use crate::query::{Policy, Query};
use crate::{Cp, DrawnPageOrder, FunctionCps, PageRef, Q, cp_end};

/// One pin of a placed device ([`PinSpots`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinSpot {
    /// The connection point, as a connection endpoint names it.
    pub cp: (Id<Function>, u16),
    /// Where it stands on its footprint's edge, in the surface's frame.
    pub at: Pt2,
    /// The way out of its edge in the surface's frame: one of `(0, 1)`, `(0, −1)`, `(1, 0)`, `(−1, 0)`; `(0, 0)` in a
    /// door.
    pub out: (i8, i8),
    /// Its height above the surface.
    pub h: Nm,
    /// The edge in the part's frame.
    pub face: PinFace,
    /// Whether the edge is the drawing's (the part names none).
    pub drawn: bool,
}

/// The value of [`PinSpots`]: the surface, the footprint and the pins of one placement.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PinSpotsV {
    /// The surface the placement stands on.
    pub surface: Id<MountingSurface>,
    /// Its footprint in the surface's frame.
    pub footprint: Rect,
    /// Its pins, in the order of the part's pins.
    pub spots: Vec<PinSpot>,
}

impl PinSpotsV {
    /// The pin of a connection point.
    pub fn spot(&self, cp: (Id<Function>, u16)) -> Option<&PinSpot> {
        self.spots.iter().find(|s| s.cp == cp)
    }
}

/// `pin_spots(p)`: see the module documentation. `None` for a placement without geometry.
#[derive(Debug)]
pub struct PinSpots;

impl Query for PinSpots {
    type Key = Id<Placement>;
    type Value = Option<PinSpotsV>;
    const FAMILY: &'static str = "pin_spots";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<PinSpotsV>> {
        Arc::new(spots(q, *key))
    }
}

/// The edge a wire leaves the connection point `cp` at, in its part's frame, and whether the drawing decides it (the
/// part's pin names no edge): the drawing's edge is the top where a wire leaves the CP upwards on the first page in
/// page order that draws it, else the bottom.
pub fn pin_face(q: &Q, function: Id<Function>, cp: &Cp) -> (PinFace, bool) {
    match cp.pin.as_ref().and_then(|(_, f)| f.clone()) {
        Some(face @ (PinFace::Top | PinFace::Bottom)) => (face, false),
        _ => (drawn_face(q, function, cp.idx), true),
    }
}

/// The edge the drawing gives the CP `(function, idx)` (see [`pin_face`]); the bottom where no page draws it.
pub fn drawn_face(q: &Q, function: Id<Function>, idx: u16) -> PinFace {
    let order = q.get::<DrawnPageOrder>(());
    let mut functions = vec![function];
    // A terminal's sides are drawn by its strip's segments.
    if let Some(strip) = q
        .functions()
        .get(function)
        .and_then(|f| f.device)
        .and_then(|d| q.devices().get(d))
        .and_then(|d| d.aspects.terminal.as_ref().map(|t| t.strip))
    {
        functions.extend(q.idx().functions_of_device(strip));
    }
    let mut pages: Vec<(usize, Id<ew_schema::Page>)> = functions
        .iter()
        .flat_map(|f| q.idx().reps_of_function(*f).collect::<Vec<_>>())
        .filter_map(|r| q.reps().get(r).map(|r| r.page))
        .map(|p| (order.position(PageRef::Drawn(p)).unwrap_or(usize::MAX), p))
        .collect();
    pages.sort();
    pages.dedup();
    let dir = pages.iter().find_map(|(_, page)| cp_end(q, function, idx, *page));
    match dir {
        Some((_, Some(Dir4::Up))) => PinFace::Top,
        _ => PinFace::Bottom,
    }
}

/// The depth of a part use's part.
fn depth_of(q: &Q, part: Option<&PartRef>) -> Option<Nm> {
    part.and_then(|p| q.parts().get(p.copy))
        .and_then(|c| c.item.mech.as_ref())
        .and_then(|m| m.depth)
}

/// The height of the carrier a placement sits on: a rail's by its profile, else nothing.
fn rail_height(q: &Q, placement: &Placement) -> Nm {
    let CarrierRef::Carrier(c) = placement.carrier else {
        return Nm::ZERO;
    };
    match q.table::<Carrier>().get(c).map(|c| &c.params) {
        Some(CarrierParams::Rail(r)) => Profile::of(&r.profile).map_or(Nm::ZERO, Profile::height),
        _ => Nm::ZERO,
    }
}

/// A pin's height: the rail's plus half the depth (the rail's alone without a depth).
fn height(rail: Nm, depth: Option<Nm>) -> Nm {
    Nm(rail.0.saturating_add(depth.map_or(0, |d| d.0.max(0) / 2)))
}

/// One pin before it is placed on its edge: its CP, its edge, whether the drawing decided it, and its order key.
#[derive(Clone)]
struct Pin {
    cp: (Id<Function>, u16),
    face: PinFace,
    drawn: bool,
    order: (u16, Id<Function>, u16),
}

/// The wireable pins of the functions `functions`: every physical CP, or on a terminal (`sides`) those with a side.
fn pins_of(q: &Q, functions: impl IntoIterator<Item = Id<Function>>, sides: bool) -> Vec<Pin> {
    let mut out: Vec<Pin> = Vec::new();
    for f in functions {
        for cp in q.get::<FunctionCps>(f).iter() {
            if !cp.physical || (sides && cp.side.is_none()) {
                continue;
            }
            let (face, drawn) = pin_face(q, f, cp);
            out.push(Pin {
                cp: (f, cp.idx),
                face,
                drawn,
                order: (cp.pin.as_ref().map_or(u16::MAX, |(i, _)| *i), f, cp.idx),
            });
        }
    }
    out.sort_by_key(|p| p.order);
    out
}

/// The pins of a placement in the part's frame: per pin its point `(x, y)` on the top (`y = h`) or bottom (`y = 0`)
/// edge of the part's `w × h`, spread over the stretch `[x0, x0 + width)` it shares with its edge's other pins.
fn spread(pins: Vec<Pin>, x0: i128, width: i128, h: i128, out: &mut Vec<(Pin, (i128, i128))>) {
    for face in [PinFace::Top, PinFace::Bottom] {
        let on: Vec<Pin> = pins.iter().filter(|p| p.face == face).cloned().collect();
        let n = i128::try_from(on.len()).unwrap_or(i128::MAX);
        let y = if face == PinFace::Top { h } else { 0 };
        for (k, pin) in on.into_iter().enumerate() {
            let k = i128::try_from(k).unwrap_or(0);
            out.push((pin, (x0 + width * (2 * k + 1) / (2 * n), y)));
        }
    }
}

fn spots(q: &Q, key: Id<Placement>) -> Option<PinSpotsV> {
    let pl = q.table::<Placement>().get(key)?.clone();
    let geom = q.get::<PlacementGeometry>(key);
    let geom = geom.as_ref().as_ref()?;
    let device: Arc<Device> = q.devices().get(pl.device)?.clone();
    let fp = geom.footprint;
    let (fw, fh) = (i128::from(fp.width().0), i128::from(fp.height().0));
    let turn = geom.turn;
    let (w, h) = if turn.k() % 2 == 1 { (fh, fw) } else { (fw, fh) };
    let rail = rail_height(q, &pl);
    // A pin in a door stands at its device's centre, half what the device reaches behind the door (doc 12 §8.1).
    if q.table::<MountingSurface>()
        .get(geom.surface)
        .is_some_and(|s| s.kind == SurfaceKind::Door)
    {
        let functions: Vec<Id<Function>> = match pl.function {
            Some(f) => vec![f],
            None => q.idx().functions_of_device(device.id).collect(),
        };
        let centre = Pt2::new(
            Nm(fp.min().x.0 + fp.width().0 / 2),
            Nm(fp.min().y.0 + fp.height().0 / 2),
        );
        let h = Nm(super::behind_door(q, key).map_or(0, |d| d.0.max(0) / 2));
        let spots = pins_of(q, functions, false)
            .into_iter()
            .map(|pin| PinSpot {
                cp: pin.cp,
                at: centre,
                out: (0, 0),
                h,
                face: pin.face,
                drawn: pin.drawn,
            })
            .collect();
        return Some(PinSpotsV {
            surface: geom.surface,
            footprint: fp,
            spots,
        });
    }
    let mut placed: Vec<(Pin, (i128, i128))> = Vec::new();
    let mut depth_by_fn: Vec<(Id<Function>, Option<Nm>)> = Vec::new();
    if places_strip(q, &device, pl.part.as_ref()) {
        for item in q.get::<StripLayout>(device.id).iter() {
            let StripItemKind::Terminal(t) = item.kind else {
                continue;
            };
            let functions: Vec<Id<Function>> = q.idx().functions_of_device(t).collect();
            let depth = depth_of(q, item.part.as_ref());
            depth_by_fn.extend(functions.iter().map(|f| (*f, depth)));
            spread(
                pins_of(q, functions, true),
                item.s,
                item.width.unwrap_or(0),
                h,
                &mut placed,
            );
        }
    } else {
        let functions: Vec<Id<Function>> = match pl.function {
            Some(f) => vec![f],
            None => q.idx().functions_of_device(device.id).collect(),
        };
        let depth = depth_of(q, geom.part.as_ref());
        depth_by_fn.extend(functions.iter().map(|f| (*f, depth)));
        spread(pins_of(q, functions, false), 0, w, h, &mut placed);
    }
    // The part's frame on the surface: its origin is the footprint's corner the turned `(w, h)` points away from.
    let (a, b) = turn.rotate((w, h));
    let (min, max) = (fp.min(), fp.max());
    let ox = i128::from(if a >= 0 { min.x.0 } else { max.x.0 });
    let oy = i128::from(if b >= 0 { min.y.0 } else { max.y.0 });
    let spots = placed
        .into_iter()
        .filter_map(|(pin, (x, y))| {
            let (du, dv) = turn.rotate((x, y));
            let at = Pt2::new(Nm(i64::try_from(ox + du).ok()?), Nm(i64::try_from(oy + dv).ok()?));
            let up = if pin.face == PinFace::Top { 1 } else { -1 };
            let (ou, ov) = turn.rotate((0, up));
            let depth = depth_by_fn
                .iter()
                .find(|(f, _)| *f == pin.cp.0)
                .and_then(|(_, d)| *d);
            Some(PinSpot {
                cp: pin.cp,
                at,
                out: (i8::try_from(ou).ok()?, i8::try_from(ov).ok()?),
                h: height(rail, depth),
                face: pin.face,
                drawn: pin.drawn,
            })
        })
        .collect();
    Some(PinSpotsV {
        surface: geom.surface,
        footprint: fp,
        spots,
    })
}

/// The placement that stands for the device of the function `function` on the layout: a placement of that function
/// on its own, else one of its whole device (the smallest ID), else, for a terminal, its strip's.
pub fn placement_of_function(q: &Q, function: Id<Function>) -> Option<Id<Placement>> {
    let device = q.functions().get(function)?.device?;
    let placements = q.table::<Placement>();
    let of = |d: Id<Device>| {
        let mut own: Vec<Id<Placement>> = q.idx().placements_of_device(d).collect();
        own.sort();
        own.iter()
            .copied()
            .find(|p| placements.get(*p).is_some_and(|p| p.function == Some(function)))
            .or_else(|| {
                own.into_iter()
                    .find(|p| placements.get(*p).is_some_and(|p| p.function.is_none()))
            })
    };
    of(device).or_else(|| {
        let strip = q.devices().get(device)?.aspects.terminal.as_ref()?.strip;
        of(strip)
    })
}

/// What a placement takes out of its surface (doc 12 §7): on a rail the rail's height plus its part's depth plus the
/// clearance it keeps in front, on the surface itself its depth and its front clearance; for a strip its terminals'
/// deepest. `None` where the depth is not known.
pub fn reach(q: &Q, placement: Id<Placement>) -> Option<Nm> {
    let pl = q.table::<Placement>().get(placement)?.clone();
    let device = q.devices().get(pl.device)?.clone();
    let geom = q.get::<PlacementGeometry>(placement);
    let geom = geom.as_ref().as_ref()?;
    let front = |part: Option<&PartRef>| {
        part.and_then(|p| q.parts().get(p.copy))
            .and_then(|c| {
                c.item
                    .mech
                    .as_ref()
                    .and_then(|m| m.clearance.as_ref())
                    .map(|c| c.front.0.max(0))
            })
            .unwrap_or(0)
    };
    let take = if places_strip(q, &device, pl.part.as_ref()) {
        q.get::<StripLayout>(device.id)
            .iter()
            .filter(|i| matches!(i.kind, StripItemKind::Terminal(_)))
            .map(|i| Some(depth_of(q, i.part.as_ref())?.0.max(0) + front(i.part.as_ref())))
            .collect::<Option<Vec<i64>>>()?
            .into_iter()
            .max()?
    } else {
        depth_of(q, geom.part.as_ref())?.0.max(0) + front(geom.part.as_ref())
    };
    Some(Nm(rail_height(q, &pl).0.saturating_add(take)))
}
