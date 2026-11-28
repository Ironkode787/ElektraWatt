// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where a device stands (doc 13 §6.1): per rail what stands on it in mounting order ([`rail_assembly`]), per surface
//! the devices placed on it without a rail with the holes they sit in ([`surface_items`]), and every place of one
//! device ([`device_places`]), which the rework list names.
//!
//! Every position is the layout's own: a rail's items are `rail_occupancy`'s intervals with the end brackets the rail
//! gives a strip (`group_accessories`), a device on a surface its `placement_geometry` footprint, its holes
//! `surface_features`'. Each is measured as the hole table measures, from the surface's datum corner; a rail's
//! positions from its start, the end nearest that corner ([`RailStart`]).

use ew_ids::Id;
use ew_mltext::{NaturalKey, natural_key};
use ew_schema::{Carrier, CarrierKind, CarrierRef, Corner, Device, MountingSurface, PartRef, Placement};

use super::accessories::GroupAccessories;
use super::carrier::RailAxis;
use super::holes::{Owner, SurfaceFeatures};
use super::legend::{CarrierLabelKind, carrier_label, carrier_length};
use super::queries::{Occupied, PlacementGeometry, RailOccupancy};
use super::surfaces::{from_datum, surface_datum};
use crate::Q;
use crate::cut_list::cut_to;

/// The end of a rail its positions are measured from: the end nearest its surface's datum corner, as the surface's
/// page shows it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RailStart {
    /// The left end of a rail that runs across.
    Left,
    /// The right end of a rail that runs across.
    Right,
    /// The lower end of a rail that runs up.
    Bottom,
    /// The upper end of a rail that runs up.
    Top,
}

/// How a rail is measured: its axis, the end its positions start from, and whether that end is its path's last point.
struct Measure {
    axis: RailAxis,
    start: RailStart,
    flipped: bool,
}

impl Measure {
    /// The rail `rail` of the surface `s`; `None` for a rail without a straight axis.
    fn of(q: &Q, rail: &Carrier, s: &MountingSurface) -> Option<Measure> {
        let axis = RailAxis::of(&rail.path)?;
        let across = axis.dir.k() % 2 == 0;
        let (left, bottom) = match surface_datum(q, s) {
            Corner::BottomLeft | Corner::Unknown(_) => (true, true),
            Corner::BottomRight => (false, true),
            Corner::TopLeft => (true, false),
            Corner::TopRight => (false, false),
        };
        let start = match (across, left, bottom) {
            (true, true, _) => RailStart::Left,
            (true, false, _) => RailStart::Right,
            (false, _, true) => RailStart::Bottom,
            (false, _, false) => RailStart::Top,
        };
        // The end `path[0]` is, by the direction to `path[1]`.
        let first = match axis.dir.k() {
            0 => RailStart::Left,
            1 => RailStart::Bottom,
            2 => RailStart::Right,
            _ => RailStart::Top,
        };
        Some(Measure {
            axis,
            start,
            flipped: first != start,
        })
    }

    /// An interval `[s0, s1]` along the path from `path[0]` as from–to from the rail's start.
    fn span(&self, s0: i128, s1: i128) -> (i128, i128) {
        if self.flipped {
            (self.axis.length - s1, self.axis.length - s0)
        } else {
            (s0, s1)
        }
    }
}

/// One thing that stands on a rail ([`RailAssembly::items`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Mounted {
    /// The device: a strip stands as one item.
    pub device: Id<Device>,
    /// Its placement.
    pub placement: Id<Placement>,
    /// The part it realises; `None` for a strip and a device without parts.
    pub part: Option<PartRef>,
    /// Where it starts, in nanometres from the rail's start; a strip with its end brackets.
    pub from: i128,
    /// Where it ends.
    pub to: i128,
    /// Its footprint is unknown (its part has no width): the interval is the placeholder's, not the device's.
    pub unknown: bool,
}

/// What the assembly list prints of one rail (doc 13 §6.1): where it lies on its surface and everything on it from its
/// start.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RailAssembly {
    /// The rail.
    pub rail: Id<Carrier>,
    /// Its surface.
    pub surface: Id<MountingSurface>,
    /// Its number among its surface's rails ("Rail 1").
    pub number: u32,
    /// Its part.
    pub part: Option<PartRef>,
    /// Its cut length, its path rounded up to the project's cut increment.
    pub cut: Option<i128>,
    /// Its length along its path, in nanometres.
    pub length: i128,
    /// The end its positions start from.
    pub start: RailStart,
    /// Its centre line from the datum corner's edge it runs along: `y` of a rail that runs across, `x` of one that
    /// runs up.
    pub across: i128,
    /// Its two ends from the datum corner along it, the smaller first.
    pub ends: (i128, i128),
    /// What stands on it, by where it starts, then its placement.
    pub items: Vec<Mounted>,
}

/// The interval `o` with the end brackets the rail gives its placement: what a strip takes on its rail.
fn with_brackets(q: &Q, rail: Id<Carrier>, o: &Occupied) -> (i128, i128) {
    let brackets = q.get::<GroupAccessories>(rail);
    brackets
        .iter()
        .filter(|b| b.placement == o.placement)
        .fold((o.s0, o.s1), |(a, b), x| (a.min(x.s0), b.max(x.s1)))
}

/// The interval `[s0, s1]` along `rail`'s path as from–to from its start ([`RailStart`]); unchanged for a rail without
/// a straight axis.
pub(crate) fn from_rail_start(q: &Q, rail: Id<Carrier>, (s0, s1): (i128, i128)) -> (i128, i128) {
    let carriers = q.table::<Carrier>();
    let surfaces = q.table::<MountingSurface>();
    carriers
        .get(rail)
        .and_then(|c| Measure::of(q, c, surfaces.get(c.surface)?))
        .map_or((s0, s1), |m| m.span(s0, s1))
}

/// `rail_assembly(rail)`: where `rail` lies and what stands on it (see the module documentation); `None` for a missing
/// carrier, a duct and a rail without a straight axis.
pub fn rail_assembly(q: &Q, rail: Id<Carrier>) -> Option<RailAssembly> {
    let carrier = q.table::<Carrier>().get(rail).cloned()?;
    if carrier.kind != CarrierKind::Rail {
        return None;
    }
    let surface = q.table::<MountingSurface>().get(carrier.surface).cloned()?;
    let m = Measure::of(q, &carrier, &surface)?;
    let ends = carrier
        .path
        .iter()
        .map(|p| from_datum(q, &surface, (i128::from(p.x.0), i128::from(p.y.0))))
        .collect::<Vec<_>>();
    let across_rail = m.axis.dir.k() % 2 == 0;
    let (along, across): (Vec<i128>, i128) = if across_rail {
        (ends.iter().map(|e| e.0).collect(), ends.first()?.1)
    } else {
        (ends.iter().map(|e| e.1).collect(), ends.first()?.0)
    };
    let (lo, hi) = (*along.iter().min()?, *along.iter().max()?);
    let geometry = |p: Id<Placement>| q.get::<PlacementGeometry>(p);
    let mut items: Vec<Mounted> = q
        .get::<RailOccupancy>(rail)
        .iter()
        .map(|o| {
            let (s0, s1) = with_brackets(q, rail, o);
            let (from, to) = m.span(s0, s1);
            Mounted {
                device: o.device,
                placement: o.placement,
                part: geometry(o.placement)
                    .as_ref()
                    .as_ref()
                    .and_then(|g| g.part.clone()),
                from,
                to,
                unknown: o.unknown,
            }
        })
        .collect();
    items.sort_by_key(|i| (i.from, i.to, i.placement));
    let number = carrier_label(q, rail)
        .filter(|l| l.kind == CarrierLabelKind::Rail)
        .map_or(0, |l| l.n);
    Some(RailAssembly {
        rail,
        surface: surface.id,
        number,
        part: carrier.part.clone(),
        cut: carrier_length(q, rail).map(|l| cut_to(q, l)),
        length: m.axis.length,
        start: m.start,
        across,
        ends: (lo, hi),
        items,
    })
}

/// A device placed on a surface itself, without a rail ([`surface_items`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OnSurface {
    /// The device.
    pub device: Id<Device>,
    /// Its placement.
    pub placement: Id<Placement>,
    /// The part it realises.
    pub part: Option<PartRef>,
    /// Its footprint's centre from the datum corner, `(x, y)` in nanometres.
    pub at: (i128, i128),
    /// Its footprint's width and height; `None` when it is unknown.
    pub size: Option<(i128, i128)>,
    /// The numbers of the holes it sits in (H5 is 5), in the hole table's order.
    pub holes: Vec<u32>,
}

/// The device placed by `p` on the surface `s` with the surface's holes `features`; `None` without geometry.
fn on_surface(
    q: &Q,
    s: &MountingSurface,
    p: Id<Placement>,
    features: &[super::holes::Feature],
) -> Option<OnSurface> {
    let placement = q.table::<Placement>().get(p).cloned()?;
    let geom = q.get::<PlacementGeometry>(p);
    let g = geom.as_ref().as_ref()?;
    let (min, max) = (g.footprint.min(), g.footprint.max());
    let centre = (
        (i128::from(min.x.0) + i128::from(max.x.0)) / 2,
        (i128::from(min.y.0) + i128::from(max.y.0)) / 2,
    );
    let mut holes: Vec<u32> = features
        .iter()
        .filter(|f| {
            f.owners
                .iter()
                .any(|o| matches!(o, Owner::Placement { p: x, .. } if *x == p))
        })
        .map(|f| f.no)
        .collect();
    holes.sort_unstable();
    Some(OnSurface {
        device: placement.device,
        placement: p,
        part: g.part.clone(),
        at: from_datum(q, s, centre),
        size: (!g.unknown).then(|| {
            (
                i128::from(max.x.0) - i128::from(min.x.0),
                i128::from(max.y.0) - i128::from(min.y.0),
            )
        }),
        holes,
    })
}

/// `surface_items(surface)`: the devices placed on `surface` itself, without a rail, in natural order of their tags,
/// then by placement; empty for a missing surface.
pub fn surface_items(q: &Q, surface: Id<MountingSurface>) -> Vec<OnSurface> {
    let Some(s) = q.table::<MountingSurface>().get(surface).cloned() else {
        return Vec::new();
    };
    let features = q.get::<SurfaceFeatures>(surface);
    let mut out: Vec<(NaturalKey, OnSurface)> = q
        .idx()
        .placements_on_surface(surface)
        .filter_map(|p| on_surface(q, &s, p, &features))
        .map(|i| (natural_key(&crate::short_tag(q, i.device)), i))
        .collect();
    out.sort_by(|a, b| (&a.0, a.1.placement).cmp(&(&b.0, b.1.placement)));
    out.into_iter().map(|(_, i)| i).collect()
}

/// One place of a device ([`device_places`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DevicePlace {
    /// On a rail: the rail's surface and number, the end it is measured from and from–to from it.
    Rail {
        /// The rail's surface.
        surface: Id<MountingSurface>,
        /// The rail's number ("Rail 1").
        number: u32,
        /// The end the positions start from.
        start: RailStart,
        /// Where the device starts.
        from: i128,
        /// Where it ends.
        to: i128,
    },
    /// On a surface itself: its centre from the datum corner and the holes it sits in.
    Surface {
        /// The surface.
        surface: Id<MountingSurface>,
        /// Its centre, `(x, y)`.
        at: (i128, i128),
        /// The numbers of its holes.
        holes: Vec<u32>,
    },
}

/// `device_places(device)`: every place of `device` in the layout, by placement; empty for a device not placed. A
/// placement on a duct or on a carrier of a kind a newer format added is no place.
pub fn device_places(q: &Q, device: Id<Device>) -> Vec<DevicePlace> {
    let placements = q.table::<Placement>();
    let mut ids: Vec<Id<Placement>> = q.idx().placements_of_device(device).collect();
    ids.sort();
    ids.into_iter()
        .filter_map(|p| match placements.get(p)?.carrier {
            CarrierRef::Carrier(c) => {
                let rail = rail_assembly(q, c)?;
                let item = rail.items.iter().find(|i| i.placement == p)?;
                Some(DevicePlace::Rail {
                    surface: rail.surface,
                    number: rail.number,
                    start: rail.start,
                    from: item.from,
                    to: item.to,
                })
            }
            CarrierRef::Surface(s) => {
                let surface = q.table::<MountingSurface>().get(s).cloned()?;
                let features = q.get::<SurfaceFeatures>(s);
                let item = on_surface(q, &surface, p, &features)?;
                Some(DevicePlace::Surface {
                    surface: s,
                    at: item.at,
                    holes: item.holes,
                })
            }
            CarrierRef::Unknown(_) => None,
        })
        .collect()
}
