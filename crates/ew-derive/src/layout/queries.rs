// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout queries of the MVP (doc 12 §2.6): `mountable(dev)`, `unplaced(space)`,
//! `placement_geometry(p)`, `rail_occupancy(carrier)`, `layout_label(p)` (D10's text) and
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
    Carrier, CarrierKind, CarrierParams, CarrierRef, CategoryKey, Device, Enclosure, LayoutSpace, LibCopy,
    MechData, MountKind, MountingSurface, Page, Part, PartRef, Placement, StructureNode,
};
use ew_units::{Nm, Pt2, Rect};

use super::carrier::{PathError, RailAxis, check_path, path_length};
use super::fit::Profile;
use super::pose::{Quarter, clip_offset};
use crate::Q;
use crate::query::{Policy, Query};
use crate::structure::{DevicesByStructure, PageDisplayStructure, StructureOf};
use crate::tag::DeviceTag;

/// The side of the unknown footprint: a 20 × 20 mm square about the pose.
pub const UNKNOWN_SIDE: i64 = 20_000_000;

/// Whether a FnDef category is an earth point, a symbol that stands for a connection to earth: `EarthingBar / <kind>
/// / Point`, protective earth (`ew.fn.pe`), functional earth (`ew.fn.fe`) and any other kind of the family (signal
/// ground, chassis, equipotential bonding).
pub fn is_earth_point(category: &CategoryKey) -> bool {
    let (a, _, c) = category;
    a.as_str() == "EarthingBar" && c.as_str() == "Point"
}

/// Whether a device is an earth connection (D8 case 8): it has functions, every one drawn with an earth point
/// ([`is_earth_point`]), and no part use. Nothing of it is bought or mounted on its own: EW-CORE-041 asks no part of
/// it and the layout exempts it, and the Devices list says what it is. A person who gives it a part makes it a
/// device like any other.
pub fn is_earth_connection(q: &Q, device: &Device) -> bool {
    let (fns, fndefs) = (q.functions(), q.fndefs());
    let mut functions = q.idx().functions_of_device(device.id).peekable();
    device.parts.is_empty()
        && functions.peek().is_some()
        && functions.all(|f| {
            fns.get(f)
                .and_then(|f| fndefs.get(f.def))
                .is_some_and(|d| is_earth_point(&d.item.category))
        })
}

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
pub(crate) fn strip_terminals(q: &Q, strip: Id<Device>) -> Vec<Arc<Device>> {
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

/// The primary mountable part of a device (D8 cases 5 and 7), as `layout.place` writes it: the first part use whose
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
    /// Case 2: the device of an enclosure or of a mounting surface: it is the layout (-UC1, -UC1-U1).
    Layout,
    /// Case 4: a terminal of a strip, placed through its strip.
    Terminal,
    /// Case 4: a cable.
    Cable,
    /// Case 4: a device without functions (nothing is drawn or wired; a PLC configuration device is one).
    NoFunctions,
    /// Case 6: every part use mounts on its parent or not at all (the valves, the DP plug -3X1).
    WithParent,
    /// Case 8: an earth connection ([`is_earth_connection`]: no part use and every function an earth point, FnDef
    /// category `EarthingBar/<kind>/Point`): -XPE.
    PePoint,
}

/// How a mountable device is placed (D8 cases 3, 5, 7 and 9).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MountAs {
    /// Case 3: a strip with terminals, placed as one placement; its footprint is its terminals'.
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

/// `mountable(dev)`: the mountable predicate as built (doc 12 §2.7), first match wins:
///
/// 1. a device whose location has no layout space: not considered (supplied by others, waits for its
///    attribute);
/// 2. the device of an enclosure or of a surface: exempt;
/// 3. a strip with terminals: mountable, placed as a strip;
/// 4. a terminal of a strip, a cable, a device without functions (EW-CORE-041's predicates): exempt;
/// 5. a part use whose part has a mount kind other than `none` and `on_parent`: mountable, the first such use is the
///    primary part;
/// 6. part uses that are all `on_parent` or `none`: exempt;
/// 7. part uses without a mount kind: mountable, unknown footprint;
/// 8. an earth connection (no part use and every function an earth point, [`is_earth_connection`]): exempt;
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
    mountability_of_kind(q, dev, &device)
}

/// Steps 2 to 9 of [`Mountable`]: what the device is mountable as, wherever it is located.
fn mountability_of_kind(q: &Q, dev: Id<Device>, device: &Device) -> Mountability {
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
    if is_strip_with_terminals(q, device) {
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
    if is_earth_connection(q, device) {
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

/// The mountable devices that have no location and no placement, in natural order of their full tag (then ID): the
/// devices no layout space lists, which Auto-fill rail offers to give its cabinet's location.
pub fn unlocated(q: &Q) -> Vec<Id<Device>> {
    let mut out: Vec<(NaturalKey, Id<Device>)> = q
        .devices()
        .iter()
        .filter(|(d, _)| location_of(q, *d).is_none())
        .filter(|(d, device)| mountability_of_kind(q, *d, device).is_mountable())
        .map(|(d, _)| d)
        .filter(|d| q.idx().placements_of_device(*d).next().is_none())
        .map(|d| {
            let tag = q.get::<DeviceTag>(d);
            let full = tag.as_ref().as_ref().map_or("", |t| t.full.as_str());
            (natural_key(full), d)
        })
        .collect();
    out.sort();
    out.into_iter().map(|(_, d)| d).collect()
}

/// The [`unlocated`] devices drawn on a wiring page (multi-line or single-line; a strip through its terminals), each
/// with whether it stands behind terminals, in the field: a wire or core from it ends, past junctions, at a
/// terminal's field side (its strip's `field_side`), and every one does. What a layout page's Give location offers the
/// cabinet's location in one step, the field devices unticked.
pub fn unlocated_drawn(q: &Q) -> Vec<(Id<Device>, bool)> {
    let (pages, reps) = (q.pages(), q.table::<ew_schema::Representation>());
    let wiring = |f: Id<ew_schema::Function>| {
        q.idx().reps_of_function(f).any(|r| {
            reps.get(r).and_then(|r| pages.get(r.page)).is_some_and(|p| {
                matches!(
                    p.kind,
                    ew_schema::PageType::MultiLine | ew_schema::PageType::SingleLine
                )
            })
        })
    };
    unlocated(q)
        .into_iter()
        .filter(|d| {
            std::iter::once(*d)
                .chain(q.idx().terminals_of_strip(*d))
                .any(|d| q.idx().functions_of_device(d).any(wiring))
        })
        .map(|d| (d, behind_terminals(q, d)))
        .collect()
}

/// Whether every wire and core from `device` ends, past junctions and its own points, at a terminal's field side, and
/// one does at least (see [`unlocated_drawn`]).
fn behind_terminals(q: &Q, device: Id<Device>) -> bool {
    use ew_model::EndpointKey;
    let (connections, functions, devices) = (q.connections(), q.functions(), q.devices());
    let own: std::collections::BTreeSet<Id<ew_schema::Function>> =
        q.idx().functions_of_device(device).collect();
    let mut stack: Vec<EndpointKey> = own
        .iter()
        .flat_map(|f| {
            q.idx()
                .connected_cps_of_function(*f)
                .into_iter()
                .map(|(idx, _)| EndpointKey::Cp(*f, idx))
        })
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut reached = false;
    while let Some(at) = stack.pop() {
        if !seen.insert(at) {
            continue;
        }
        for c in q.idx().connections_at(at) {
            let Some(c) = connections.get(c) else { continue };
            if !matches!(
                c.kind,
                ew_schema::ConnectionKind::Wire | ew_schema::ConnectionKind::Core(_)
            ) {
                continue;
            }
            let Some((a, b)) = crate::nets::ends(c) else {
                continue;
            };
            let other = if a == at { b } else { a };
            match other {
                EndpointKey::Cp(f, idx) if !own.contains(&f) => {
                    // A terminal's point on its strip's field side (default side 2).
                    let field = functions
                        .get(f)
                        .and_then(|f| devices.get(f.device?))
                        .and_then(|t| devices.get(t.aspects.terminal.as_ref()?.strip))
                        .map(|s| {
                            s.aspects
                                .strip
                                .as_ref()
                                .map_or(ew_schema::Side::Side2, |s| s.field_side.clone())
                        })
                        .is_some_and(|side| {
                            q.get::<crate::FunctionCps>(f)
                                .iter()
                                .any(|c| c.idx == idx && c.side.as_ref() == Some(&side))
                        });
                    if !field {
                        return false;
                    }
                    reached = true;
                }
                _ => stack.push(other),
            }
        }
    }
    reached
}

/// The devices Auto-fill rail offers for a rail of the layout space `space` (doc 12 §3.3): the space's
/// [`Unplaced`] devices, then the [`unlocated`] ones, each with its first guess of whether it goes on the rail. The
/// guess reads what the model says of a device, first match wins:
///
/// 1. its primary part's mount kind: a rail profile goes on the rail, any other kind (screwed, in the door, on a
///    busbar) does not;
/// 2. it has the space's location: it goes;
/// 3. a function category of field equipment (the category table's `field` flag, `ew_rds::category_flags`: a sensor,
///    a cylinder, a motor, a valve terminal), or
///    it stands behind terminals in the field (every wire from it ends at a terminal's field side): it does not;
/// 4. otherwise it goes.
pub fn fill_candidates(q: &Q, space: Id<LayoutSpace>) -> Vec<(Id<Device>, bool)> {
    let located = q.get::<Unplaced>(space);
    let devices = q.devices();
    let guess = |d: Id<Device>, in_space: bool| {
        let mount = devices
            .get(d)
            .and_then(|dev| primary_part(q, dev))
            .and_then(|p| mount_kind_of(q, &p));
        if let Some(kind) = mount.filter(mounts) {
            return super::fit::Profile::of(&kind).is_some();
        }
        if in_space {
            return true;
        }
        let (fns, fndefs) = (q.functions(), q.fndefs());
        let field = q.idx().functions_of_device(d).any(|f| {
            fns.get(f)
                .and_then(|f| fndefs.get(f.def))
                .is_some_and(|def| ew_rds::category_flags(def.item.category.0.as_str()).field)
        });
        !field && !behind_terminals(q, d)
    };
    located
        .iter()
        .map(|d| (*d, guess(*d, true)))
        .chain(unlocated(q).into_iter().map(|d| (d, guess(d, false))))
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// placement_geometry(p)

/// The value of [`PlacementGeometry`]: a placement's footprint in the frame of its mounting surface.
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
    /// The footprint is unknown: the part has no `W` or `H`, the device has no part, or a terminal of the strip
    /// has none; the square is drawn dashed.
    pub unknown: bool,
    /// The part's `front_2d` polylines in the surface frame; empty without them (the footprint is drawn instead,
    /// FR-LAY-011).
    pub outline: Vec<Vec<Pt2>>,
    /// The part use realised: the placement's `part`, else the device's primary part (an imported `None` means the
    /// primary); `None` for a strip and a device without parts.
    pub part: Option<PartRef>,
}

/// The shape of what a placement realises, in the part frame.
struct Shape {
    /// `(W, H)`; `None`: unknown.
    size: Option<(i128, i128)>,
    /// The clip point in the part frame.
    clip: (i128, i128),
    /// The `front_2d` polylines in the part frame.
    outline: Vec<Vec<Pt2>>,
    part: Option<PartRef>,
}

/// What one item of a strip is ([`StripItem`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StripItemKind {
    /// A terminal of the strip.
    Terminal(Id<Device>),
    /// An accessory standing in the strip (`part_accessories`, per strip: an end plate after the last terminal whose
    /// part names it).
    Accessory(crate::DerivedAccessory),
    /// An accessory a person added to the strip itself: its index in `Strip.accessories`.
    Own(usize),
}

/// One item of a strip in mounting order ([`StripLayout`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StripItem {
    /// What it is.
    pub kind: StripItemKind,
    /// Its part: a terminal's primary part, the accessory's part.
    pub part: Option<PartRef>,
    /// Its width along the strip, `mech.width` in nanometres; `None` when its part has none.
    pub width: Option<i128>,
    /// Its height, `mech.height`; `None` when its part has none.
    pub height: Option<i128>,
    /// Where it starts along the strip, from the strip's start, in nanometres (an item without a width takes none).
    pub s: i128,
}

/// `strip_layout(strip)`: a strip's items in mounting order (doc 12 §2.6, §3.4): its terminals by `seq` and, after
/// each, the accessories a person added to the strip there (`Strip.accessories` by `after_seq`; one before the first
/// terminal stands first), then the accessories its terminals' parts stand there (`part_accessories`, per strip),
/// each with its part's width and its place along the strip. Empty for a missing device and a device without
/// terminals.
#[derive(Debug)]
pub struct StripLayout;

impl Query for StripLayout {
    type Key = Id<Device>;
    type Value = Vec<StripItem>;
    const FAMILY: &'static str = "strip_layout";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<StripItem>> {
        let Some(strip) = q.devices().get(*key).cloned() else {
            return Arc::new(Vec::new());
        };
        let accessories = crate::strip_accessories(q, &strip);
        let size = |part: Option<&PartRef>| {
            let m = part.and_then(|p| mech_of(q, p.copy));
            (
                m.and_then(|m| m.width).map(|w| i128::from(w.0)),
                m.and_then(|m| m.height).map(|h| i128::from(h.0)),
            )
        };
        let mut out: Vec<StripItem> = Vec::new();
        let mut s = 0i128;
        let mut push = |kind: StripItemKind, part: Option<PartRef>| {
            let (width, height) = size(part.as_ref());
            out.push(StripItem {
                kind,
                part,
                width,
                height,
                s,
            });
            s = s.saturating_add(width.unwrap_or(0));
        };
        let terminals = strip_terminals(q, *key);
        if terminals.is_empty() {
            return Arc::new(out);
        }
        // A strip's own accessories by the terminal they follow: the last whose `seq` is not above their `after_seq`.
        let seqs: Vec<u32> = terminals
            .iter()
            .map(|t| t.aspects.terminal.as_ref().map_or(0, |a| a.seq))
            .collect();
        let own: Vec<(usize, Option<usize>, PartRef)> = strip
            .aspects
            .strip
            .as_ref()
            .map(|a| {
                let mut v: Vec<(usize, Option<usize>, PartRef)> = a
                    .accessories
                    .iter()
                    .enumerate()
                    .map(|(i, x)| (i, seqs.iter().rposition(|s| *s <= x.after_seq), x.part.clone()))
                    .collect();
                v.sort_by_key(|(i, at, _)| (at.map_or(0, |k| k + 1), *i));
                v
            })
            .unwrap_or_default();
        for (i, _, part) in own.iter().filter(|o| o.1.is_none()) {
            push(StripItemKind::Own(*i), Some(part.clone()));
        }
        for (k, t) in terminals.iter().enumerate() {
            push(StripItemKind::Terminal(t.id), primary_part(q, t));
            for (i, _, part) in own.iter().filter(|o| o.1 == Some(k)) {
                push(StripItemKind::Own(*i), Some(part.clone()));
            }
            for a in accessories.iter().filter(|a| a.after == Some(t.id)) {
                push(StripItemKind::Accessory(a.clone()), Some(a.part.clone()));
            }
        }
        Arc::new(out)
    }
}

/// The shape of a placement: a strip's items side by side ([`StripLayout`]: the terminals and the accessories standing
/// in it, widths summed, the terminals' largest height, the clip point at the left end at mid-height; unknown when a
/// terminal has no width or height), else the realised part's `W × H` with its clip point (`rail_offset`, else the
/// centre).
fn shape(q: &Q, device: &Device, part: Option<&PartRef>) -> Shape {
    if part.is_none() && is_strip_with_terminals(q, device) {
        let mut width = 0i128;
        let mut height = 0i128;
        let mut known = true;
        for item in q.get::<StripLayout>(device.id).iter() {
            match (&item.kind, item.width, item.height) {
                (StripItemKind::Terminal(_), Some(w), Some(h)) => {
                    width += w;
                    height = height.max(h);
                }
                (StripItemKind::Terminal(_), _, _) => known = false,
                (StripItemKind::Accessory(_) | StripItemKind::Own(_), w, _) => width += w.unwrap_or(0),
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
/// unknown (doc 12 §2.6).
///
/// On a rail the pose is `t = (s, 0, 0)`: the part's clip point lies on the rail axis at `s` from `path[0]`, and the
/// part turns by the rail's direction followed by the pose's 0° or 180°; the part origin is the clip point minus the
/// clip offset turned by that turn. On the surface the pose `t = (u, v, 0)` is the part origin, turned by `k · 90°`
/// about it. The footprint is the part's `W × H` rectangle from its origin, turned; an unknown footprint is the
/// 20 × 20 mm square centred on the pose's point (the clip point on a rail, `(u, v)` on the surface).
///
/// `None` for a missing placement or device, a carrier that is not a rail with a valid axis (a duct carries none), a
/// missing surface, a turn that is not a quarter turn, or a footprint coordinate outside the `Nm` range (an outline
/// point outside it drops the outline only, [`PlacementGeom::outline`] empty). A rail pose with `s`
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

/// The `front_2d` polylines placed by `place`; empty (no outline) when any point falls outside the `Nm` range, so the
/// footprint and the occupancy stay (part 4, finding 4).
fn placed_outline(lines: &[Vec<Pt2>], place: impl Fn((i128, i128)) -> Option<Pt2>) -> Vec<Vec<Pt2>> {
    lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|p| place((i128::from(p.x.0), i128::from(p.y.0))))
                .collect::<Option<Vec<Pt2>>>()
        })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
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
    let outline = placed_outline(&shape.outline, place);
    Some(PlacementGeom {
        surface,
        footprint,
        turn,
        unknown: false,
        outline,
        part: shape.part,
    })
}

/// The clearance pads along a rail of a placement realising `part`: `[clearance.left,
/// clearance.right]`, swapped when `flipped`; `[0, 0]` without a part (a strip, a partless device) or a clearance; a
/// negative side counts as 0.
fn pads(q: &Q, part: Option<&PartRef>, flipped: bool) -> [i128; 2] {
    let Some(c) = part
        .and_then(|p| mech_of(q, p.copy))
        .and_then(|m| m.clearance.as_ref())
    else {
        return [0, 0];
    };
    let (left, right) = (i128::from(c.left.0).max(0), i128::from(c.right.0).max(0));
    if flipped { [right, left] } else { [left, right] }
}

/// What a placement of a device takes along a rail: the width of its footprint along the axis, where its
/// clip point lies from the footprint's left (smaller-`s`) edge, its clearance pads and whether it is a strip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RailExtent {
    /// The footprint's extent along the axis, in nanometres (the unknown square's side when unknown).
    pub width: i128,
    /// The clip point's distance from the footprint's left edge along the axis: `s0 = s_clip − clip`.
    pub clip: i128,
    /// The clearance pads `[before, after]` along the axis ([`Occupied::pad`]).
    pub pad: [i128; 2],
    /// Whether the placement places a strip with terminals as a whole (no part, D8 case 3).
    pub strip: bool,
}

/// `rail_extent`: what a placement of `device` realising `part` (`None`: the primary part, or the strip as a whole),
/// turned 0° (`flipped` false) or 180° on a rail, takes along the axis: the shape [`PlacementGeometry`] places, so a
/// placement at clip point `s` occupies `[s − clip, s − clip + width]` in [`RailOccupancy`].
pub fn rail_extent(q: &Q, device: &Device, part: Option<&PartRef>, flipped: bool) -> RailExtent {
    let strip = places_strip(q, device, part);
    let shape = shape(q, device, part);
    let pad = if strip {
        [0, 0]
    } else {
        pads(q, shape.part.as_ref(), flipped)
    };
    match shape.size {
        Some((w, _)) => RailExtent {
            width: w,
            clip: if flipped { w - shape.clip.0 } else { shape.clip.0 },
            pad,
            strip,
        },
        None => RailExtent {
            width: i128::from(UNKNOWN_SIDE),
            clip: i128::from(UNKNOWN_SIDE / 2),
            pad,
            strip,
        },
    }
}

/// Whether a placement of `device` with the stored part use `part` places a strip with terminals as a whole (D8 case
/// 3): no part, and the device is a strip with terminals.
pub fn places_strip(q: &Q, device: &Device, part: Option<&PartRef>) -> bool {
    part.is_none() && is_strip_with_terminals(q, device)
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
    /// Whether its footprint is unknown.
    pub unknown: bool,
    /// The clearance pads along the rail axis, `[before, after]` in nanometres: the realised part's
    /// `clearance.left` and `clearance.right`, swapped when the placement is flipped (turned 180° on the rail); `[0, 0]`
    /// for a strip, a device without a part and a part without a clearance; a negative clearance counts as 0. The pads
    /// space the snapping only: EW-LAY-006 compares the bare intervals `[s0, s1]`. Last, so that the derived order
    /// stays `(s0, s1, placement, …)`.
    pub pad: [i128; 2],
}

/// `rail_occupancy(carrier)`: the intervals the placements on a rail occupy along its axis, sorted by `(s0, s1,
/// placement)` (doc 12 §2.6). Strips are one interval (their terminals side by side). Intervals may overlap: the
/// overlap is reported (EW-LAY-006), never resolved. Empty for a missing carrier, a duct, a rail without a valid
/// axis; a placement without geometry is left out. The intervals are the bare footprints; each carries its clearance
/// pads ([`Occupied::pad`]), which the snapping reads and the overlap check does not.
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
                let flipped = Quarter::of(pl.pose.q) == Some(Quarter::HALF);
                Some(Occupied {
                    s0: a.min(b),
                    s1: a.max(b),
                    placement: p,
                    device: pl.device,
                    locked: pl.locked,
                    unknown: geom.unknown,
                    pad: pads(q, geom.part.as_ref(), flipped),
                })
            })
            .collect();
        out.sort();
        Arc::new(out)
    }
}

// ---------------------------------------------------------------------------------------------------------
// layout_label(p)

/// How a layout view labels its placements (FR-LAY-009; the `label_mode` of a surface view).
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
/// (full, the page is not read). A layout page without a location of its own suppresses the location of the layout
/// space its surface view shows (the cabinet the page draws), so "+CAB1-K1" prints "-K1" on that cabinet's layout. The text is derived from the device's tag each time, so renumbering a device changes
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
            LabelMode::Abbreviated => {
                let tag = q.get::<DeviceTag>(device);
                let Some(tag) = tag.as_ref() else {
                    return Arc::new(String::new());
                };
                Arc::new(tag.displayed(&q.get::<PageDisplayStructure>(page)))
            }
            LabelMode::Full => {
                let tag = q.get::<DeviceTag>(device);
                Arc::new(tag.as_ref().as_ref().map(|t| t.full.clone()).unwrap_or_default())
            }
        }
    }
}

/// The location node of the layout space whose surface the panel-layout page `page` shows.
pub(crate) fn shown_location(q: &Q, page: Id<Page>) -> Option<Id<StructureNode>> {
    let surface = super::legend::page_surface(q, page)?;
    let enclosure = q.table::<MountingSurface>().get(surface)?.enclosure;
    let space = q.table::<Enclosure>().get(enclosure)?.space;
    Some(q.table::<LayoutSpace>().get(space)?.location)
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
    /// Why the path is invalid, if it is; the length and outline are then not derived.
    pub path_error: Option<PathError>,
    /// The path length in nanometres (the BOM quantity of part 4); `None` for an invalid path or a kind this
    /// build does not know.
    pub length: Option<i128>,
    /// A rail's axis.
    pub axis: Option<RailAxis>,
    /// The outline: per path segment, the rectangle of the carrier's width centred on the segment (a duct's `width`, a
    /// rail's profile width: 35 mm for TS 35, 32 mm, 15 mm); segments end at the path's points, so a duct's corner is
    /// covered by its two segments only up to its points. Empty when the width or path is unknown.
    pub outline: Vec<Rect>,
}

/// `carrier_geometry(carrier)`: the axis check, length, rail axis and outline of a carrier (doc 12 §2.4). `None`
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
            (CarrierKind::Rail, CarrierParams::Rail(r)) => Profile::of(&r.profile).map(Profile::width),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// An outline point that lands outside the `Nm` range drops the outline (an
    /// empty one), never the placement's geometry; points in range are placed.
    #[test]
    fn an_outline_point_outside_the_nm_range_drops_only_the_outline() {
        let p = |x: i64, y: i64| Pt2::new(Nm(x), Nm(y));
        let lines = vec![vec![p(0, 0), p(5, 0)], vec![p(0, 0), p(20, 0)]];
        // Near the top of the range: the second line's (20, 0) overflows.
        let far = |(x, y): (i128, i128)| pt(i128::from(i64::MAX) - 10 + x, y);
        assert_eq!(placed_outline(&lines, far), Vec::<Vec<Pt2>>::new());
        let near = |(x, y): (i128, i128)| pt(100 + x, y);
        assert_eq!(
            placed_outline(&lines, near),
            vec![vec![p(100, 0), p(105, 0)], vec![p(100, 0), p(120, 0)]]
        );
    }
}
