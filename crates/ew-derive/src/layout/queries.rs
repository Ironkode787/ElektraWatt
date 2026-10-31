// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout queries of the MVP (doc 12 §2.6, WP-S4.01 part 2): `mountable(dev)` (D8), `unplaced(space)`,
//! `placement_geometry(p)` (D5), `rail_occupancy(carrier)`, `layout_label(p)` (D10's text) and
//! `carrier_geometry(carrier)`.
//!
//! Every value is integer nanometres in the frame of the mounting surface (`u` right, `v` up), exact: carriers are
//! axis-parallel and turns are quarter turns, so footprints are axis-parallel rectangles computed in i128 and rounded
//! never. Nothing here is stored: a footprint, a label, the unplaced list and an occupancy are derived from the
//! placement records and the part data each time (doc 12 §2.3, "what is never stored").

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{NaturalKey, natural_key};
use ew_rds::Aspect;
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Device, Enclosure, LayoutSpace, LibCopy, MechData,
    MountKind, MountingSurface, Page, Part, PartRef, Placement, StructureNode,
};
use ew_units::{Nm, Pt2, Rect};

use super::carrier::{PathError, RailAxis, check_path, path_length};
use super::fit::Profile;
use super::pose::{Quarter, clip_offset};
use crate::Q;
use crate::query::{Policy, Query};
use crate::structure::{DevicesByStructure, StructureOf};
use crate::tag::{DeviceTag, DisplayedTag};

/// The side of the unknown footprint (D5): a 20 × 20 mm square about the pose.
pub const UNKNOWN_SIDE: i64 = 20_000_000;

/// The FnDef category of an integral PE point (D8 case 8): `EarthingBar/ProtectiveEarth/Point` (`ew.fn.pe`).
const PE_POINT: [&str; 3] = ["EarthingBar", "ProtectiveEarth", "Point"];

// ---------------------------------------------------------------------------------------------------------
// Shared reads

/// The layout space of a location: the one whose `location` is the node; with two (an MVP limit EW-LAY-010
/// reports) the one with the smallest ID.
pub fn space_of_location(q: &Q, location: Id<StructureNode>) -> Option<Id<LayoutSpace>> {
    q.table::<LayoutSpace>()
        .iter()
        .filter(|(_, s)| s.location == location)
        .map(|(id, _)| id)
        .min()
}

/// The device's own location node (its resolved `+` aspect); `None` without one or on a structure cycle.
fn location_of(q: &Q, device: Id<Device>) -> Option<Id<StructureNode>> {
    q.try_get::<StructureOf>(device).ok()?.node(Aspect::Location)
}

/// The mechanical data of a part copy.
fn mech_of(q: &Q, part: Id<Part>) -> Option<&MechData> {
    q.table::<LibCopy<Part>>()
        .get(part)
        .and_then(|c| c.item.mech.as_ref())
}

/// The mount kind of a part use's part (`None`: the part has no `mech.mount_kind`, or the copy is missing).
fn mount_kind_of(q: &Q, part: &PartRef) -> Option<MountKind> {
    mech_of(q, part.copy).and_then(|m| m.mount_kind.clone())
}

/// Whether a mount kind mounts the part in its own right (D8 case 5): every kind but `none` and `on_parent`, a kind of a
/// newer format included.
fn mounts(kind: &MountKind) -> bool {
    !matches!(kind, MountKind::None | MountKind::OnParent)
}

/// The terminals of a strip in strip order (`seq`, then ID).
fn strip_terminals(q: &Q, strip: Id<Device>) -> Vec<Arc<Device>> {
    let devices = q.devices();
    let mut out: Vec<Arc<Device>> = q
        .idx()
        .terminals_of_strip(strip)
        .filter_map(|t| devices.get(t).cloned())
        .collect();
    out.sort_by_key(|t| (t.aspects.terminal.as_ref().map_or(0, |a| a.seq), t.id));
    out
}

/// Whether a device is a strip with terminals (D8 case 3): it is placed as one placement without a part.
fn is_strip_with_terminals(q: &Q, device: &Device) -> bool {
    device.aspects.strip.is_some() && q.idx().terminals_of_strip(device.id).next().is_some()
}

/// The primary mountable part of a device (D6, D8 cases 5 and 7), as `layout.place` writes it: the first part use whose
/// part mounts, else the first whose part has no mount kind; `None` for a strip with terminals, a device without parts
/// and one whose parts all mount on their parent or not at all.
pub fn primary_part(q: &Q, device: &Device) -> Option<PartRef> {
    if is_strip_with_terminals(q, device) {
        return None;
    }
    device
        .parts
        .iter()
        .find(|u| mount_kind_of(q, &u.part).is_some_and(|k| mounts(&k)))
        .or_else(|| device.parts.iter().find(|u| mount_kind_of(q, &u.part).is_none()))
        .map(|u| u.part.clone())
}

// ---------------------------------------------------------------------------------------------------------
// mountable(dev)

/// Why a device is exempt from the layout (D8, doc 12 §2.7).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Exempt {
    /// Case 2: the device of an enclosure or of a mounting surface: it is the layout (-UC1, -UC1-plate).
    Layout,
    /// Case 4: a terminal of a strip, placed through its strip.
    Terminal,
    /// Case 4: a cable.
    Cable,
    /// Case 4: a device without functions (nothing is drawn or wired; a PLC configuration device is one).
    NoFunctions,
    /// Case 6: every part use mounts on its parent or not at all (the valves, the DP plug -3X1).
    WithParent,
    /// Case 8: no part use and every function an integral PE point (FnDef category `EarthingBar/ProtectiveEarth/
    /// Point`): -XPE.
    PePoint,
}

/// How a mountable device is placed (D8 cases 3, 5, 7 and 9).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MountAs {
    /// Case 3: a strip with terminals, placed as one placement; its footprint is its terminals' (D5).
    Strip,
    /// Case 5: a part use with a mount kind other than `none` and `on_parent`; it is the primary part.
    Part,
    /// Case 7: part uses without a mount kind; the first is the primary part and the footprint is unknown.
    PartWithoutMountKind,
    /// Case 9: no part use; the footprint is unknown.
    NoPart,
}

/// The value of [`Mountable`]: what the layout expects of a device (D8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Mountability {
    /// Case 1: the device's location has no layout space (or it has no location, or a missing device): the layout
    /// does not consider it.
    NotConsidered,
    /// Exempt for the reason given.
    Exempt(Exempt),
    /// Mountable: it must appear in the layout of its location.
    Mountable {
        /// How it is placed.
        how: MountAs,
        /// The primary part (cases 5 and 7); `None` for a strip and a device without parts.
        primary: Option<PartRef>,
    },
}

impl Mountability {
    /// Whether the device must appear in the layout.
    pub fn is_mountable(&self) -> bool {
        matches!(self, Mountability::Mountable { .. })
    }
}

/// `mountable(dev)`: the mountable predicate as built (WP-S4.01 D8, doc 12 §2.7), first match wins:
///
/// 1. a device whose location has no layout space: not considered (supplied by others, FT-209, waits for its
///    attribute);
/// 2. the device of an enclosure or of a surface: exempt;
/// 3. a strip with terminals: mountable, placed as a strip;
/// 4. a terminal of a strip, a cable, a device without functions (EW-CORE-041's predicates): exempt;
/// 5. a part use whose part has a mount kind other than `none` and `on_parent`: mountable, the first such use is the
///    primary part;
/// 6. part uses that are all `on_parent` or `none`: exempt;
/// 7. part uses without a mount kind: mountable, unknown footprint;
/// 8. no part use and every function's FnDef category the PE point: exempt as an integral PE point;
/// 9. otherwise (no part): mountable, unknown footprint.
///
/// Assembly children are not exempt by being children: the valve terminal's members are placed or reported in their
/// own right, and "with its parent" holds only through `on_parent`.
#[derive(Debug)]
pub struct Mountable;

impl Query for Mountable {
    type Key = Id<Device>;
    type Value = Mountability;
    const FAMILY: &'static str = "mountable";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Mountability> {
        Arc::new(mountability(q, *key))
    }
}

fn mountability(q: &Q, dev: Id<Device>) -> Mountability {
    let Some(device) = q.devices().get(dev).cloned() else {
        return Mountability::NotConsidered;
    };
    // 1
    let Some(location) = location_of(q, dev) else {
        return Mountability::NotConsidered;
    };
    if space_of_location(q, location).is_none() {
        return Mountability::NotConsidered;
    }
    // 2
    let encloses = q.table::<Enclosure>().iter().any(|(_, e)| e.device == dev);
    let surface = q
        .table::<MountingSurface>()
        .iter()
        .any(|(_, s)| s.device == Some(dev));
    if encloses || surface {
        return Mountability::Exempt(Exempt::Layout);
    }
    // 3
    if is_strip_with_terminals(q, &device) {
        return Mountability::Mountable {
            how: MountAs::Strip,
            primary: None,
        };
    }
    // 4
    if device.aspects.terminal.is_some() {
        return Mountability::Exempt(Exempt::Terminal);
    }
    if device.aspects.cable.is_some() {
        return Mountability::Exempt(Exempt::Cable);
    }
    let functions: Vec<_> = q.idx().functions_of_device(dev).collect();
    if functions.is_empty() {
        return Mountability::Exempt(Exempt::NoFunctions);
    }
    if !device.parts.is_empty() {
        let kinds: Vec<Option<MountKind>> = device.parts.iter().map(|u| mount_kind_of(q, &u.part)).collect();
        // 5
        if let Some(i) = kinds.iter().position(|k| k.as_ref().is_some_and(mounts)) {
            return Mountability::Mountable {
                how: MountAs::Part,
                primary: Some(device.parts[i].part.clone()),
            };
        }
        // 6
        if kinds.iter().all(Option::is_some) {
            return Mountability::Exempt(Exempt::WithParent);
        }
        // 7
        let first = kinds.iter().position(Option::is_none).unwrap_or(0);
        return Mountability::Mountable {
            how: MountAs::PartWithoutMountKind,
            primary: Some(device.parts[first].part.clone()),
        };
    }
    // 8
    let (fns, fndefs) = (q.functions(), q.fndefs());
    let pe_point = functions.iter().all(|f| {
        fns.get(*f).and_then(|f| fndefs.get(f.def)).is_some_and(|d| {
            let (a, b, c) = &d.item.category;
            [a.as_str(), b.as_str(), c.as_str()] == PE_POINT
        })
    });
    if pe_point {
        return Mountability::Exempt(Exempt::PePoint);
    }
    // 9
    Mountability::Mountable {
        how: MountAs::NoPart,
        primary: None,
    }
}

// ---------------------------------------------------------------------------------------------------------
// unplaced(space)

/// `unplaced(space)`: the mountable devices of the space's location without a placement, in natural order of their
/// full tag (then ID) (FR-LAY-001, FR-LAY-013; doc 12 §2.6).
///
/// A device belongs to the space when its own location node is the space's location and the space is that location's
/// layout space ([`space_of_location`]); a device of another location is never listed. A device counts as placed when
/// any placement names it, wherever it sits (a placement in another location's space is EW-LAY-004's). A missing space
/// lists nothing.
#[derive(Debug)]
pub struct Unplaced;

impl Query for Unplaced {
    type Key = Id<LayoutSpace>;
    type Value = Vec<Id<Device>>;
    const FAMILY: &'static str = "unplaced";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<Id<Device>>> {
        let Some(space) = q.table::<LayoutSpace>().get(*key).cloned() else {
            return Arc::new(Vec::new());
        };
        if space_of_location(q, space.location) != Some(*key) {
            return Arc::new(Vec::new());
        }
        let by_node = q.get::<DevicesByStructure>(());
        let mut out: Vec<(NaturalKey, Id<Device>)> = by_node
            .get(&space.location)
            .into_iter()
            .flatten()
            .copied()
            .filter(|d| location_of(q, *d) == Some(space.location))
            .filter(|d| q.get::<Mountable>(*d).is_mountable())
            .filter(|d| q.idx().placements_of_device(*d).next().is_none())
            .map(|d| {
                let tag = q.get::<DeviceTag>(d);
                let full = tag.as_ref().as_ref().map_or("", |t| t.full.as_str());
                (natural_key(full), d)
            })
            .collect();
        out.sort();
        out.dedup_by_key(|(_, d)| *d);
        Arc::new(out.into_iter().map(|(_, d)| d).collect())
    }
}

// ---------------------------------------------------------------------------------------------------------
// placement_geometry(p)

/// The value of [`PlacementGeometry`]: a placement's footprint in the frame of its mounting surface (D5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlacementGeom {
    /// The mounting surface whose frame the coordinates are in.
    pub surface: Id<MountingSurface>,
    /// The footprint: the part's `W × H` rectangle at its pose, axis-parallel (a quarter turn swaps `W` and `H`); a
    /// strip's terminals side by side; the unknown 20 × 20 mm square about the pose when [`PlacementGeom::unknown`].
    pub footprint: Rect,
    /// The part's turn on the surface: a rail's direction followed by the pose's 0° or 180°, or a surface pose's
    /// `k · 90°`.
    pub turn: Quarter,
    /// The footprint is unknown (D5): the part has no `W` or `H`, the device has no part, or a terminal of the strip
    /// has none; the square is drawn dashed.
    pub unknown: bool,
    /// The part's `front_2d` polylines in the surface frame; empty without them (the footprint is drawn instead,
    /// FR-LAY-011).
    pub outline: Vec<Vec<Pt2>>,
    /// The part use realised: the placement's `part`, else the device's primary part (an imported `None` means the
    /// primary, D6); `None` for a strip and a device without parts.
    pub part: Option<PartRef>,
}

/// The shape of what a placement realises, in the part frame.
struct Shape {
    /// `(W, H)`; `None`: unknown.
    size: Option<(i128, i128)>,
    /// The clip point in the part frame (D5).
    clip: (i128, i128),
    /// The `front_2d` polylines in the part frame.
    outline: Vec<Vec<Pt2>>,
    part: Option<PartRef>,
}

/// The shape of a placement (D5): a strip's terminals side by side (widths summed in strip order, the largest height,
/// the clip point at the left end at mid-height), else the realised part's `W × H` with its clip point
/// (`rail_offset`, else the centre).
fn shape(q: &Q, device: &Device, part: Option<&PartRef>) -> Shape {
    if part.is_none() && is_strip_with_terminals(q, device) {
        let mut width = 0i128;
        let mut height = 0i128;
        let mut known = true;
        for t in strip_terminals(q, device.id) {
            let size = primary_part(q, &t)
                .and_then(|p| mech_of(q, p.copy))
                .and_then(|m| Some((m.width?, m.height?)));
            match size {
                Some((w, h)) => {
                    width += i128::from(w.0);
                    height = height.max(i128::from(h.0));
                }
                None => known = false,
            }
        }
        return Shape {
            size: known.then_some((width, height)),
            clip: if known { (0, height / 2) } else { (0, 0) },
            outline: Vec::new(),
            part: None,
        };
    }
    let part = match part {
        Some(p) => Some(p.clone()),
        None => primary_part(q, device),
    };
    let mech = part.as_ref().and_then(|p| mech_of(q, p.copy));
    let size = mech.and_then(|m| Some((i128::from(m.width?.0), i128::from(m.height?.0))));
    Shape {
        size,
        clip: if size.is_some() { clip_offset(mech) } else { (0, 0) },
        outline: if size.is_some() {
            mech.map(|m| m.front_2d.clone()).unwrap_or_default()
        } else {
            Vec::new()
        },
        part,
    }
}

/// A point of i128 coordinates as a `Pt2`, `None` outside the `Nm` range.
fn pt(u: i128, v: i128) -> Option<Pt2> {
    Some(Pt2::new(Nm(i64::try_from(u).ok()?), Nm(i64::try_from(v).ok()?)))
}

/// `placement_geometry(p)`: the footprint and outline of a placement in its surface's frame, and whether it is
/// unknown (D5, doc 12 §2.6).
///
/// On a rail the pose is `t = (s, 0, 0)`: the part's clip point lies on the rail axis at `s` from `path[0]`, and the
/// part turns by the rail's direction followed by the pose's 0° or 180°; the part origin is the clip point minus the
/// clip offset turned by that turn. On the surface the pose `t = (u, v, 0)` is the part origin, turned by `k · 90°`
/// about it. The footprint is the part's `W × H` rectangle from its origin, turned; an unknown footprint is the
/// 20 × 20 mm square centred on the pose's point (the clip point on a rail, `(u, v)` on the surface).
///
/// `None` for a missing placement or device, a carrier that is not a rail with a valid axis (a duct carries none), a
/// missing surface, a turn that is not a quarter turn, or a coordinate outside the `Nm` range. A rail pose with `s`
/// outside `[0, L]` or `t.y`, `t.z` other than 0 still gets its footprint (from `t.x`): EW-LAY-010 reports it.
#[derive(Debug)]
pub struct PlacementGeometry;

impl Query for PlacementGeometry {
    type Key = Id<Placement>;
    type Value = Option<PlacementGeom>;
    const FAMILY: &'static str = "placement_geometry";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<PlacementGeom>> {
        Arc::new(placement_geom(q, *key))
    }
}

fn placement_geom(q: &Q, key: Id<Placement>) -> Option<PlacementGeom> {
    let pl = q.table::<Placement>().get(key)?.clone();
    let device = q.devices().get(pl.device)?.clone();
    let pose_turn = Quarter::of(pl.pose.q)?;
    let (surface, turn, anchor, on_rail) = match pl.carrier {
        CarrierRef::Carrier(c) => {
            let carrier = q.table::<Carrier>().get(c)?;
            if carrier.kind != CarrierKind::Rail {
                return None;
            }
            let axis = RailAxis::of(&carrier.path)?;
            (
                carrier.surface,
                axis.dir.then(pose_turn),
                axis.point_at(i128::from(pl.pose.t[0].0)),
                true,
            )
        }
        CarrierRef::Surface(s) => {
            if !q.table::<MountingSurface>().contains(s) {
                return None;
            }
            let [u, v, _] = pl.pose.t;
            (s, pose_turn, (i128::from(u.0), i128::from(v.0)), false)
        }
        CarrierRef::Unknown(_) => return None,
    };
    let shape = shape(q, &device, pl.part.as_ref());
    let Some((w, h)) = shape.size else {
        let half = i128::from(UNKNOWN_SIDE / 2);
        let footprint = Rect::from_points(
            pt(anchor.0 - half, anchor.1 - half)?,
            pt(anchor.0 + half, anchor.1 + half)?,
        );
        return Some(PlacementGeom {
            surface,
            footprint,
            turn,
            unknown: true,
            outline: Vec::new(),
            part: shape.part,
        });
    };
    let origin = if on_rail {
        let (cu, cv) = turn.rotate(shape.clip);
        (anchor.0 - cu, anchor.1 - cv)
    } else {
        anchor
    };
    let place = |(x, y): (i128, i128)| {
        let (du, dv) = turn.rotate((x, y));
        pt(origin.0 + du, origin.1 + dv)
    };
    let footprint = Rect::from_points(place((0, 0))?, place((w, h))?);
    let outline = shape
        .outline
        .iter()
        .map(|line| {
            line.iter()
                .map(|p| place((i128::from(p.x.0), i128::from(p.y.0))))
                .collect::<Option<Vec<Pt2>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    Some(PlacementGeom {
        surface,
        footprint,
        turn,
        unknown: false,
        outline,
        part: shape.part,
    })
}

// ---------------------------------------------------------------------------------------------------------
// rail_occupancy(carrier)

/// One occupied interval of a rail: a placement's footprint projected onto the rail axis.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Occupied {
    /// Where the footprint starts along the axis, from `path[0]`, in nanometres.
    pub s0: i128,
    /// Where it ends, `s1 ≥ s0`.
    pub s1: i128,
    /// The placement.
    pub placement: Id<Placement>,
    /// Its device.
    pub device: Id<Device>,
    /// Whether the placement is locked (excluded from push, swap and auto-fill).
    pub locked: bool,
    /// Whether its footprint is unknown (D5).
    pub unknown: bool,
}

/// `rail_occupancy(carrier)`: the intervals the placements on a rail occupy along its axis, sorted by `(s0, s1,
/// placement)` (doc 12 §2.6). Strips are one interval (their terminals side by side, D5). Intervals may overlap: the
/// overlap is reported (EW-LAY-006), never resolved. Empty for a missing carrier, a duct, a rail without a valid
/// axis; a placement without geometry is left out. Clearances are not padded at the MVP (no part of Golden A has one).
#[derive(Debug)]
pub struct RailOccupancy;

impl Query for RailOccupancy {
    type Key = Id<Carrier>;
    type Value = Vec<Occupied>;
    const FAMILY: &'static str = "rail_occupancy";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<Occupied>> {
        let Some(carrier) = q.table::<Carrier>().get(*key).cloned() else {
            return Arc::new(Vec::new());
        };
        let Some(axis) = (carrier.kind == CarrierKind::Rail)
            .then(|| RailAxis::of(&carrier.path))
            .flatten()
        else {
            return Arc::new(Vec::new());
        };
        let (du, dv) = axis.dir.rotate((1, 0));
        let (su, sv) = (i128::from(axis.start.x.0), i128::from(axis.start.y.0));
        let along = |p: Pt2| (i128::from(p.x.0) - su) * du + (i128::from(p.y.0) - sv) * dv;
        let placements = q.table::<Placement>();
        let mut out: Vec<Occupied> = q
            .idx()
            .placements_by_carrier(*key)
            .filter_map(|p| {
                let geom = q.get::<PlacementGeometry>(p);
                let geom = geom.as_ref().as_ref()?;
                let pl = placements.get(p)?;
                let (a, b) = (along(geom.footprint.min()), along(geom.footprint.max()));
                Some(Occupied {
                    s0: a.min(b),
                    s1: a.max(b),
                    placement: p,
                    device: pl.device,
                    locked: pl.locked,
                    unknown: geom.unknown,
                })
            })
            .collect();
        out.sort();
        Arc::new(out)
    }
}

// ---------------------------------------------------------------------------------------------------------
// layout_label(p)

/// How a layout view labels its placements (FR-LAY-009; the `label_mode` of a surface view, D9).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum LabelMode {
    /// The displayed tag on the view's page: identifier suppression against the page's structure ("-X1031").
    #[default]
    Abbreviated,
    /// The full designation ("=P01+J-E-2416-X1031").
    Full,
}

/// `layout_label((placement, page, mode))`: the label text of a placement on a layout page (D10's text, doc 12 §5.3):
/// the device's displayed tag on the page (abbreviated; frozen tags print as frozen, "T55") or its full designation
/// (full, the page is not read). The text is derived from the device's tag each time, so renumbering a device changes
/// its label and no layout record (D-02). A placement of one function prints its device's label: functions carry no
/// designation member in format 0.1, so D10's ":" suffix has nothing to add yet. Empty for a missing placement, device
/// or page (the full label reads the page too, so that a deleted page's labels leave the memo store with it).
#[derive(Debug)]
pub struct LayoutLabel;

impl Query for LayoutLabel {
    type Key = (Id<Placement>, Id<Page>, LabelMode);
    type Value = String;
    const FAMILY: &'static str = "layout_label";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<String> {
        let (placement, page, mode) = *key;
        let Some(device) = q.table::<Placement>().get(placement).map(|p| p.device) else {
            return Arc::new(String::new());
        };
        if q.pages().get(page).is_none() {
            return Arc::new(String::new());
        }
        match mode {
            LabelMode::Abbreviated => q.get::<DisplayedTag>((device, page)),
            LabelMode::Full => {
                let tag = q.get::<DeviceTag>(device);
                Arc::new(tag.as_ref().as_ref().map(|t| t.full.clone()).unwrap_or_default())
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// carrier_geometry(carrier)

/// The value of [`CarrierGeometry`]: a carrier's geometry in its surface's frame.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CarrierGeom {
    /// The mounting surface.
    pub surface: Id<MountingSurface>,
    /// The carrier's kind.
    pub kind: CarrierKind,
    /// Why the path breaks D5, if it does; the length and outline are then not derived.
    pub path_error: Option<PathError>,
    /// The path length in nanometres (the BOM quantity of part 4); `None` for a path that breaks D5 or a kind this
    /// build does not know.
    pub length: Option<i128>,
    /// A rail's axis.
    pub axis: Option<RailAxis>,
    /// The outline: per path segment, the rectangle of the carrier's width centred on the segment (a duct's `width`, a
    /// rail's profile width: 35 mm for TS 35, 32 mm, 15 mm); segments end at the path's points, so a duct's corner is
    /// covered by its two segments only up to its points. Empty when the width or path is unknown.
    pub outline: Vec<Rect>,
}

/// The width of a rail of a profile, in nanometres.
fn profile_width(profile: Profile) -> i128 {
    match profile {
        Profile::Ts35x7_5 | Profile::Ts35x15 => 35_000_000,
        Profile::Ts32 => 32_000_000,
        Profile::Ts15 => 15_000_000,
    }
}

/// `carrier_geometry(carrier)`: the axis check, length, rail axis and outline of a carrier (D5, doc 12 §2.4). `None`
/// for a missing carrier.
#[derive(Debug)]
pub struct CarrierGeometry;

impl Query for CarrierGeometry {
    type Key = Id<Carrier>;
    type Value = Option<CarrierGeom>;
    const FAMILY: &'static str = "carrier_geometry";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<CarrierGeom>> {
        let Some(c) = q.table::<Carrier>().get(*key).cloned() else {
            return Arc::new(None);
        };
        let known = matches!(c.kind, CarrierKind::Rail | CarrierKind::Duct);
        let path_error = check_path(&c.kind, &c.path).err();
        let valid = known && path_error.is_none();
        let width = match (&c.kind, &c.params) {
            (CarrierKind::Duct, CarrierParams::Duct(d)) => Some(i128::from(d.width.0)),
            (CarrierKind::Rail, CarrierParams::Rail(r)) => Profile::of(&r.profile).map(profile_width),
            _ => None,
        };
        let outline = match width.filter(|_| valid) {
            Some(w) => c
                .path
                .windows(2)
                .map(|s| {
                    let (a, b) = (s[0], s[1]);
                    let half = w / 2;
                    let (ax, ay, bx, by) = (
                        i128::from(a.x.0),
                        i128::from(a.y.0),
                        i128::from(b.x.0),
                        i128::from(b.y.0),
                    );
                    if ay == by {
                        Some(Rect::from_points(pt(ax, ay - half)?, pt(bx, by + half)?))
                    } else {
                        Some(Rect::from_points(pt(ax - half, ay)?, pt(bx + half, by)?))
                    }
                })
                .collect::<Option<Vec<_>>>()
                .unwrap_or_default(),
            None => Vec::new(),
        };
        Arc::new(Some(CarrierGeom {
            surface: c.surface,
            kind: c.kind.clone(),
            path_error,
            length: valid.then(|| path_length(&c.path)),
            axis: (c.kind == CarrierKind::Rail)
                .then(|| RailAxis::of(&c.path))
                .flatten(),
            outline,
        }))
    }
}
