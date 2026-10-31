// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The MVP rules of the EW-LAYOUT pack (doc 15 §4.5.4; WP-S4.01 part 5, D12): EW-LAY-001, -003, -004, -006 (2D), -010
//! and -016. EW-LAY-002 is the structural invariant of D4 (a placement cannot outlive its device) and has no rule run.
//!
//! # Scope
//!
//! Doc 15 gives these rules the scopes `space`, `surface` and `carrier`, which the engine does not have. As built they
//! run at the **project** scope until space and surface scope kinds exist (WP-S4.01 part 5, the orchestrator's ruling 1):
//! each `run` enumerates the layout spaces, surfaces, carriers and placements through the layout queries of
//! `ew_derive::layout` (none of them stores a value), and each finding names its own object as a target (the device,
//! placement, carrier, layout space or enclosure), shown on the panel-layout page whose surface view shows it.
//!
//! # Texts (WP-S1.16 §10, FT-779)
//!
//! Devices print by their full designation; parts by their part number; a carrier by its kind and its path's first and
//! last points in millimetres with one decimal (`the rail at (40.0, 740.0)–(610.0, 740.0) mm`: carriers have no
//! designation member, WP-S4.01 FP-5); a location by its `+` designation. IDs enter only targets and fingerprints.
//! Every finding carries its message arguments; the en-GB patterns of `messages/en-GB/rules.ftl` render the messages
//! byte for byte.
//!
//! # Geometry
//!
//! Every comparison is exact in integer nanometres (i128): carriers are axis-parallel and turns are quarter turns at
//! the MVP (D5), so footprints and carrier outlines are axis-parallel rectangles. An overlap is a positive-area
//! intersection: rectangles that only touch do not overlap.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use ew_derive::Q;
use ew_derive::StructureOf;
use ew_derive::StructurePath;
use ew_derive::ew_ids::Id;
use ew_derive::ew_mltext::natural_cmp;
use ew_derive::ew_rds::Aspect;
use ew_derive::ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Device, Enclosure, Kind, LayoutSpace, LibCopy,
    MountKind, MountingSurface, Part, PartRef, Placement, StructureNode, SurfaceKind,
};
use ew_derive::ew_units::{Pt2, Rect};
use ew_derive::layout::{
    CarrierGeometry, Fit, PlacementGeometry, PoseError, PoseOn, Profile, RailAxis, RailOccupancy, Unplaced,
    check_pose, fits_rail, fits_surface, primary_part, surface_pages,
};
use ew_derive::part_confidence;

use crate::finding::{Finding, Severity, Target};
use crate::rule::{Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind};
use crate::rules::term::full_tag;

// ---------------------------------------------------------------------------------------------------------
// Shared helpers

/// Nanometres per tenth of a millimetre.
const NM_PER_TENTH: i128 = 100_000;

/// A length in nanometres as millimetres with one decimal (FT-779): rounded half away from zero (`40.0`, `-0.5`).
fn mm1(nm: i128) -> String {
    let tenths = (nm.unsigned_abs() + (NM_PER_TENTH as u128) / 2) / (NM_PER_TENTH as u128);
    let sign = if nm < 0 && tenths > 0 { "-" } else { "" };
    format!("{sign}{}.{}", tenths / 10, tenths % 10)
}

/// A point of the surface frame as printed: `(40.0, 740.0)`.
fn point(p: Pt2) -> String {
    format!("({}, {})", mm1(i128::from(p.x.0)), mm1(i128::from(p.y.0)))
}

/// Where a carrier lies, as printed: its path's first and last points in millimetres (`(40.0, 740.0)–(610.0, 740.0)
/// mm`), one point for a one-point path, `()` for an empty one. Bounded: only two points print whatever the path's
/// length.
fn carrier_at(c: &Carrier) -> String {
    match (c.path.first(), c.path.last()) {
        (Some(a), Some(b)) if c.path.len() > 1 => format!("{}–{} mm", point(*a), point(*b)),
        (Some(a), _) => format!("{} mm", point(*a)),
        _ => "() mm".to_owned(),
    }
}

/// The selector word of a carrier's kind in the patterns: `rail`, `duct`, or `other` (a kind of a newer format).
fn carrier_word(kind: &CarrierKind) -> &'static str {
    match kind {
        CarrierKind::Rail => "rail",
        CarrierKind::Duct => "duct",
        CarrierKind::Unknown(_) => "other",
    }
}

/// A rail profile as printed (`TS 35×7.5`).
fn profile_text(kind: &MountKind) -> String {
    match Profile::of(kind) {
        Some(Profile::Ts35x7_5) => "TS 35×7.5".to_owned(),
        Some(Profile::Ts35x15) => "TS 35×15".to_owned(),
        Some(Profile::Ts32) => "TS 32".to_owned(),
        Some(Profile::Ts15) => "TS 15".to_owned(),
        None => "an unknown profile".to_owned(),
    }
}

/// A location as a designation prints it: `+` and its path's codes (`+J-E-2416`).
fn location_text(q: &Q, node: Id<StructureNode>) -> String {
    let path = q.get::<StructurePath>(node);
    if path.codes.is_empty() {
        return "a location".to_owned();
    }
    format!("+{}", path.codes.join("+"))
}

/// The device's own location node (its resolved `+` aspect); `None` without one or on a structure cycle.
fn location_of(q: &Q, device: Id<Device>) -> Option<Id<StructureNode>> {
    q.try_get::<StructureOf>(device).ok()?.node(Aspect::Location)
}

/// A device as printed: its full designation.
fn device_name(q: &Q, device: Id<Device>) -> String {
    full_tag(q, device, "a device without designation")
}

/// A part as printed: its part number.
fn part_name(q: &Q, part: &PartRef) -> String {
    q.table::<LibCopy<Part>>()
        .get(part.copy)
        .map_or_else(|| "a missing part".to_owned(), |c| c.item.part_number.to_string())
}

/// The mounting surface a placement sits on: its carrier's surface, or the surface itself.
fn placement_surface(q: &Q, pl: &Placement) -> Option<Id<MountingSurface>> {
    match &pl.carrier {
        CarrierRef::Carrier(c) => q.table::<Carrier>().get(*c).map(|c| c.surface),
        CarrierRef::Surface(s) => Some(*s),
        CarrierRef::Unknown(_) => None,
    }
}

/// The finding shown on the first panel-layout page whose surface view shows `surface`, when there is one.
fn shown(f: Finding, q: &Q, surface: Option<Id<MountingSurface>>) -> Finding {
    match surface.and_then(|s| surface_pages(q, s).into_iter().next()) {
        Some(page) => f.on_page(page),
        None => f,
    }
}

/// The targets of a finding about a placement: its device (the subject) and the placement.
fn placement_targets(pl: &Placement) -> [Target; 2] {
    [
        Target::new(Kind::Device, pl.device),
        Target::new(Kind::Placement, pl.id),
    ]
}

/// The part use a placement realises: its `part`, else the device's primary part (D6: an imported `None` means the
/// primary); `None` for a strip and a device without parts.
fn realised_part(q: &Q, pl: &Placement) -> Option<PartRef> {
    if let Some(p) = &pl.part {
        return Some(p.clone());
    }
    q.devices().get(pl.device).and_then(|d| primary_part(q, d))
}

/// The mount kind of a part use (`None`: no copy, no `mech` or no mount kind), with the copy for its confidence.
fn mount_of(q: &Q, part: Option<&PartRef>) -> (Option<MountKind>, Option<std::sync::Arc<LibCopy<Part>>>) {
    let Some(part) = part else {
        return (None, None);
    };
    let copy = q.table::<LibCopy<Part>>().get(part.copy).cloned();
    let mount = copy
        .as_ref()
        .and_then(|c| c.item.mech.as_ref())
        .and_then(|m| m.mount_kind.clone());
    (mount, copy)
}

/// Whether two closed rectangles intersect with a positive area: touching edges do not, nor does a rectangle without
/// area (a zero width or height) lying inside another.
fn overlaps(a: Rect, b: Rect) -> bool {
    a.min().x.max(b.min().x) < a.max().x.min(b.max().x) && a.min().y.max(b.min().y) < a.max().y.min(b.max().y)
}

/// `(u, v)` of a point as i128.
fn xy(p: Pt2) -> (i128, i128) {
    (i128::from(p.x.0), i128::from(p.y.0))
}

/// Whether `p` lies inside the polygon `poly` or on its boundary (exact; ray casting with the boundary checked first).
fn in_polygon(poly: &[Pt2], p: (i128, i128)) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (xy(poly[i]), xy(poly[(i + 1) % n]));
        let cross = (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
        if cross == 0
            && p.0 >= a.0.min(b.0)
            && p.0 <= a.0.max(b.0)
            && p.1 >= a.1.min(b.1)
            && p.1 <= a.1.max(b.1)
        {
            return true;
        }
        if (a.1 > p.1) != (b.1 > p.1) {
            // the edge crosses the horizontal line through p: compare p.0 with the crossing's u, exactly
            let lhs = (p.0 - a.0) * (b.1 - a.1);
            let rhs = (b.0 - a.0) * (p.1 - a.1);
            let left_of_crossing = if b.1 > a.1 { lhs < rhs } else { lhs > rhs };
            if left_of_crossing {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether the rectangle `r` lies inside the usable polygon `usable` (its boundary included): its four corners inside,
/// no vertex of the polygon strictly inside the rectangle, and no axis-parallel edge of the polygon through its open
/// interior. Exact for the orthogonal polygons of the MVP (the plate rectangle, D5); a polygon with a diagonal edge is
/// judged by the corners and vertices only. `None` for a polygon of fewer than three points (an unknown usable area:
/// nothing to judge).
fn inside_usable(usable: &[Pt2], r: Rect) -> Option<bool> {
    if usable.len() < 3 {
        return None;
    }
    let (lo, hi) = (xy(r.min()), xy(r.max()));
    let corners = [(lo.0, lo.1), (hi.0, lo.1), (hi.0, hi.1), (lo.0, hi.1)];
    if !corners.iter().all(|c| in_polygon(usable, *c)) {
        return Some(false);
    }
    let strictly_in = |p: (i128, i128)| p.0 > lo.0 && p.0 < hi.0 && p.1 > lo.1 && p.1 < hi.1;
    let n = usable.len();
    for i in 0..n {
        let (a, b) = (xy(usable[i]), xy(usable[(i + 1) % n]));
        if strictly_in(a) {
            return Some(false);
        }
        let through = if a.1 == b.1 {
            a.1 > lo.1 && a.1 < hi.1 && a.0.max(b.0) > lo.0 && a.0.min(b.0) < hi.0
        } else if a.0 == b.0 {
            a.0 > lo.0 && a.0 < hi.0 && a.1.max(b.1) > lo.1 && a.1.min(b.1) < hi.1
        } else {
            false
        };
        if through {
            return Some(false);
        }
    }
    Some(true)
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-001

/// EW-LAY-001 "Unplaced device" (doc 15 §4.5.4): completeness, o r, warning/error, space (as built: project)
/// (FR-LAY-017, FR-CHK-039; EF-05).
pub const UNPLACED_DEVICE: RuleMeta = RuleMeta {
    id: "EW-LAY-001",
    code: "130001",
    pack: "EW-LAYOUT",
    title: "Unplaced device",
    kind: RuleKind::Completeness,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: Some(Severity::Error),
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-LAY-001: a mountable device of a location that has a layout space, without a placement (`unplaced(space)` of every
/// layout space, WP-S4.01 D8, D12). Target the device, discriminator `unplaced`, no page (the device is drawn on no
/// layout page).
///
/// "Counts at error only where a layout space exists" (doc 05 §14.1, LC-7) is a filter in this run, not a scheme
/// condition: `unplaced` lists only the devices of a location with a layout space, so a project without one reports
/// nothing and the release severity error applies to what is reported (the orchestrator's ruling 2).
#[derive(Clone, Copy, Debug)]
pub struct UnplacedDevice;

impl Rule for UnplacedDevice {
    fn meta(&self) -> &'static RuleMeta {
        &UNPLACED_DEVICE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (space_id, space) in q.table::<LayoutSpace>().iter() {
            let location = location_text(q, space.location);
            for device in q.get::<Unplaced>(space_id).iter() {
                let name = device_name(q, *device);
                out.push(
                    Finding::new(
                        &UNPLACED_DEVICE,
                        [Target::new(Kind::Device, *device)],
                        "unplaced",
                        name.clone(),
                        format!(
                            "{name} is not placed in the layout of {location}; place it on a rail or on the mounting \
                             plate"
                        ),
                    )
                    .arg("device", name)
                    .arg("location", location.clone()),
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-003

/// EW-LAY-003 "Part use not on the device" (doc 15 §4.5.4): conflict, o r, error, space (as built: project)
/// (FR-LAY-017, FR-LAY-059).
pub const PART_NOT_ON_DEVICE: RuleMeta = RuleMeta {
    id: "EW-LAY-003",
    code: "130003",
    pack: "EW-LAYOUT",
    title: "Part use not on the device",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-LAY-003: a placement whose `part` is not a part use of its device (the copy is compared; D6 writes `part` at
/// placement, D7 makes a replaced part follow, so a removed part leaves the placement naming a copy the device no
/// longer uses). A placement without `part` realises the primary part and is never reported. Targets the device and
/// the placement, discriminator `part`.
#[derive(Clone, Copy, Debug)]
pub struct PartNotOnDevice;

impl Rule for PartNotOnDevice {
    fn meta(&self) -> &'static RuleMeta {
        &PART_NOT_ON_DEVICE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let devices = q.devices();
        let mut out = Vec::new();
        for (_, pl) in q.table::<Placement>().iter() {
            let Some(part) = &pl.part else {
                continue;
            };
            let Some(device) = devices.get(pl.device) else {
                continue;
            };
            if device.parts.iter().any(|u| u.part.copy == part.copy) {
                continue;
            }
            let name = device_name(q, pl.device);
            let part_text = part_name(q, part);
            let f = Finding::new(
                &PART_NOT_ON_DEVICE,
                placement_targets(pl),
                "part",
                name.clone(),
                format!(
                    "the placement of {name} realises part {part_text}, which the device no longer uses; place the \
                     device with one of its parts, or give it the part back"
                ),
            )
            .arg("device", name)
            .arg("part", part_text);
            out.push(shown(f, q, placement_surface(q, pl)));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-004

/// EW-LAY-004 "Location mismatch" (doc 15 §4.5.4): conflict, o r, warning, space (as built: project) (FR-LAY-018).
pub const LOCATION_MISMATCH: RuleMeta = RuleMeta {
    id: "EW-LAY-004",
    code: "130004",
    pack: "EW-LAYOUT",
    title: "Location mismatch",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-LAY-004: a placed device whose own location (its resolved `+` aspect) is not the location of the layout space
/// it is placed in (placement → carrier → surface → enclosure → space), a device without a location included. Targets
/// the device and the placement, discriminator `location`.
#[derive(Clone, Copy, Debug)]
pub struct LocationMismatch;

impl Rule for LocationMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &LOCATION_MISMATCH
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let (surfaces, enclosures, spaces) = (
            q.table::<MountingSurface>(),
            q.table::<Enclosure>(),
            q.table::<LayoutSpace>(),
        );
        let mut out = Vec::new();
        for (_, pl) in q.table::<Placement>().iter() {
            let surface = placement_surface(q, pl);
            let Some(space) = surface
                .and_then(|s| surfaces.get(s))
                .and_then(|s| enclosures.get(s.enclosure))
                .and_then(|e| spaces.get(e.space))
            else {
                continue;
            };
            let own = location_of(q, pl.device);
            if own == Some(space.location) {
                continue;
            }
            let name = device_name(q, pl.device);
            let space_text = location_text(q, space.location);
            let (own_text, message) = match own {
                Some(node) => {
                    let t = location_text(q, node);
                    let m = format!(
                        "{name} is in {t} but is placed in the layout of {space_text}; move it to the layout of its \
                         location, or correct its location"
                    );
                    (t, m)
                }
                None => (
                    "none".to_owned(),
                    format!(
                        "{name} has no location but is placed in the layout of {space_text}; move it to the layout \
                         of its location, or correct its location"
                    ),
                ),
            };
            let f = Finding::new(
                &LOCATION_MISMATCH,
                placement_targets(pl),
                "location",
                name.clone(),
                message,
            )
            .arg("device", name)
            .arg("location", own_text)
            .arg("space", space_text);
            out.push(shown(f, q, surface));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-006

/// EW-LAY-006 "Collision" (doc 15 §4.5.4): conflict, o r, error, surface (as built: project; the 2D half of the MVP,
/// so the catalogue's offline mode, the 3D half of 1.0, is not set) (FR-LAY-018).
pub const COLLISION: RuleMeta = RuleMeta {
    id: "EW-LAY-006",
    code: "130006",
    pack: "EW-LAYOUT",
    title: "Collision",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// One footprint of the overlap sweep.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Item {
    /// A placement's footprint.
    Placement(Id<Placement>),
    /// A segment rectangle of a rail's outline.
    Rail(Id<Carrier>),
    /// A segment rectangle of a duct's outline.
    Duct(Id<Carrier>),
}

/// Whether two items are a pair EW-LAY-006 judges (D12): two placements, a placement and a duct, a rail and a duct;
/// never a placement and a rail (a placement sits on its own rail and may overhang another's outline), never two
/// rails, never two ducts (joints are by design).
fn judged(a: Item, b: Item) -> bool {
    match (a, b) {
        (Item::Placement(x), Item::Placement(y)) => x != y,
        (Item::Placement(_), Item::Duct(_))
        | (Item::Duct(_), Item::Placement(_))
        | (Item::Rail(_), Item::Duct(_))
        | (Item::Duct(_), Item::Rail(_)) => true,
        _ => false,
    }
}

/// The most overlap findings EW-LAY-006 lists per surface; past it one summary finding names the surface (cycle 41's
/// ruling 1: "At most 64 overlap findings per surface ... When a 65th pair is found, the rule stops collecting pairs
/// for that surface and adds one summary finding on the surface").
pub const OVERLAPS_PER_SURFACE: usize = 64;

/// The active set an item of the sweep joins.
#[derive(Clone, Copy)]
enum Class {
    Placement = 0,
    Rail = 1,
    Duct = 2,
}

impl Item {
    fn class(self) -> Class {
        match self {
            Item::Placement(_) => Class::Placement,
            Item::Rail(_) => Class::Rail,
            Item::Duct(_) => Class::Duct,
        }
    }
}

/// The classes an item of `class` is compared with: only judged pairs are visited (two placements, a placement and a
/// duct, a rail and a duct), so the sweep's work follows the judged overlaps it finds, not the rail under every
/// placement.
fn partners(class: Class) -> &'static [Class] {
    match class {
        Class::Placement => &[Class::Placement, Class::Duct],
        Class::Rail => &[Class::Duct],
        Class::Duct => &[Class::Placement, Class::Rail],
    }
}

/// The active rectangles of one class during the sweep, ordered by y so that a query visits only the rectangles whose
/// y interval really overlaps (cycle 41's ruling 1, its first option: "keep the active set ordered by y so only real
/// y-overlaps are visited (O(n log n + k))"). An interval `[a0, a1)` overlaps `[b0, b1)` exactly when `a0` lies in
/// `[b0, b1)` (the range query on `by_min`) or `b0` lies in `(a0, a1)` (the stabbing query on `tree`, a segment tree
/// over the surface's y slots whose nodes hold the intervals they cover).
struct Active {
    /// `(min y, index)` of each active rectangle.
    by_min: BTreeSet<(i128, usize)>,
    /// The segment tree's nodes (root 1), each with the indices of the intervals it covers.
    tree: Vec<BTreeSet<usize>>,
}

impl Active {
    fn new(slots: usize) -> Active {
        Active {
            by_min: BTreeSet::new(),
            tree: vec![BTreeSet::new(); 4 * slots.max(1)],
        }
    }

    /// Adds (`add`) or removes `id` over the slots `lo..hi` in the subtree of `node`, which spans the slots `l..r`.
    fn cover(&mut self, node: usize, (l, r): (usize, usize), (lo, hi): (usize, usize), id: usize, add: bool) {
        if hi <= l || r <= lo {
            return;
        }
        if lo <= l && r <= hi {
            if add {
                self.tree[node].insert(id);
            } else {
                self.tree[node].remove(&id);
            }
            return;
        }
        let mid = l + (r - l) / 2;
        self.cover(2 * node, (l, mid), (lo, hi), id, add);
        self.cover(2 * node + 1, (mid, r), (lo, hi), id, add);
    }

    /// The intervals that cover `slot` (the nodes on the path from the root to its leaf; `n` slots in all).
    fn stab(&self, slot: usize, n: usize, out: &mut Vec<usize>) {
        let (mut node, mut l, mut r) = (1, 0, n);
        loop {
            out.extend(self.tree[node].iter().copied());
            if r - l <= 1 {
                return;
            }
            let mid = l + (r - l) / 2;
            if slot < mid {
                (node, r) = (2 * node, mid);
            } else {
                (node, l) = (2 * node + 1, mid);
            }
        }
    }
}

/// The overlapping judged pairs of one surface's rectangles, each once, in the order the sweep meets them, at most
/// [`OVERLAPS_PER_SURFACE`] of them; `true` when a further pair exists (the sweep stops at it). A sweep in x over the
/// rectangles sorted by their lower left corner: a rectangle leaves the active sets when the sweep reaches its right
/// edge (touching is not overlapping), and each new rectangle is compared only with the active rectangles of its
/// partner classes whose y interval overlaps its own, so the work is O(n log n) plus the overlaps visited.
fn surface_overlaps(items: Vec<(Rect, Item)>) -> (Vec<(Item, Item)>, bool) {
    let lo = |r: &Rect| xy(r.min());
    let hi = |r: &Rect| xy(r.max());
    // a rectangle without area overlaps nothing
    let mut items: Vec<(Rect, Item)> = items
        .into_iter()
        .filter(|(r, _)| lo(r).0 < hi(r).0 && lo(r).1 < hi(r).1)
        .collect();
    items.sort_by_key(|(r, item)| (lo(r).0, lo(r).1, hi(r).0, hi(r).1, *item));
    let mut ys: Vec<i128> = items.iter().flat_map(|(r, _)| [lo(r).1, hi(r).1]).collect();
    ys.sort_unstable();
    ys.dedup();
    // slot j is [ys[j], ys[j + 1])
    let slot = |y: i128| ys.partition_point(|v| *v < y);
    let n = ys.len().saturating_sub(1);
    let span = |r: &Rect| (slot(lo(r).1), slot(hi(r).1));
    let mut active = [Active::new(n), Active::new(n), Active::new(n)];
    let mut ends: BinaryHeap<Reverse<(i128, usize)>> = BinaryHeap::new();
    let mut found: Vec<(Item, Item)> = Vec::new();
    let mut seen: BTreeSet<(Item, Item)> = BTreeSet::new();
    let mut hits: Vec<usize> = Vec::new();
    for (b, (rb, ib)) in items.iter().enumerate() {
        let x = lo(rb).0;
        while let Some(&Reverse((end, a))) = ends.peek() {
            if end > x {
                break;
            }
            ends.pop();
            let (ra, ia) = &items[a];
            let set = &mut active[ia.class() as usize];
            set.by_min.remove(&(lo(ra).1, a));
            set.cover(1, (0, n), span(ra), a, false);
        }
        let (y0, y1) = (lo(rb).1, hi(rb).1);
        for class in partners(ib.class()) {
            let set = &active[*class as usize];
            hits.clear();
            hits.extend(set.by_min.range((y0, 0)..(y1, 0)).map(|(_, a)| *a));
            let from = hits.len();
            set.stab(slot(y0), n, &mut hits);
            // the intervals that start at y0 came from the range query
            let mut k = from;
            for i in from..hits.len() {
                if lo(&items[hits[i]].0).1 < y0 {
                    hits[k] = hits[i];
                    k += 1;
                }
            }
            hits.truncate(k);
            hits.sort_unstable();
            for a in &hits {
                let (ra, ia) = items[*a];
                debug_assert!(overlaps(ra, *rb), "the sweep visits real overlaps only");
                if !judged(ia, *ib) {
                    continue;
                }
                let key = if ia <= *ib { (ia, *ib) } else { (*ib, ia) };
                if seen.contains(&key) {
                    continue;
                }
                if found.len() == OVERLAPS_PER_SURFACE {
                    return (found, true);
                }
                seen.insert(key);
                found.push(key);
            }
        }
        let set = &mut active[ib.class() as usize];
        set.by_min.insert((y0, b));
        set.cover(1, (0, n), span(rb), b, true);
        ends.push(Reverse((hi(rb).0, b)));
    }
    (found, false)
}

/// The summary finding of a surface with more than [`OVERLAPS_PER_SURFACE`] overlaps (cycle 41's ruling 1): no count,
/// so the sweep stops at the first pair past the cap.
fn crowded_finding(q: &Q, surface: Id<MountingSurface>) -> Finding {
    let enclosure = q
        .table::<MountingSurface>()
        .get(surface)
        .and_then(|s| {
            q.table::<Enclosure>()
                .get(s.enclosure)
                .map(|e| device_name(q, e.device))
        })
        .unwrap_or_else(|| "a missing enclosure".to_owned());
    let f = Finding::new(
        &COLLISION,
        [Target::new(Kind::MountingSurface, surface)],
        "overlap_limit",
        enclosure.clone(),
        format!(
            "more footprints overlap on the mounting plate of {enclosure}; the first {OVERLAPS_PER_SURFACE} overlaps \
             are listed"
        ),
    )
    .arg("form", "limit")
    .arg("enclosure", enclosure)
    .arg("listed", OVERLAPS_PER_SURFACE.to_string());
    shown(f, q, Some(surface))
}

/// EW-LAY-006 (2D): a positive-area intersection, on one surface, of two placement footprints, of a footprint and a
/// duct's outline, or of a rail's outline and a duct's (D12), found by a sweep over each surface's rectangles whose
/// active sets are ordered by y (see `surface_overlaps`). One finding per pair, whatever the number of segment
/// rectangles that overlap; at most [`OVERLAPS_PER_SURFACE`] per surface, then one summary finding on the surface
/// (`limit`). Two placements of one device read as such (`same`). Targets: the two placements' devices and
/// placements, or the placement's device and placement and the duct, or the two carriers, or the surface;
/// discriminator `overlap` (`overlap_limit` for the summary).
#[derive(Clone, Copy, Debug)]
pub struct Collision;

impl Rule for Collision {
    fn meta(&self) -> &'static RuleMeta {
        &COLLISION
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let placements = q.table::<Placement>();
        let carriers = q.table::<Carrier>();
        let mut by_surface: BTreeMap<Id<MountingSurface>, Vec<(Rect, Item)>> = BTreeMap::new();
        for (id, _) in placements.iter() {
            if let Some(g) = q.get::<PlacementGeometry>(id).as_ref() {
                by_surface
                    .entry(g.surface)
                    .or_default()
                    .push((g.footprint, Item::Placement(id)));
            }
        }
        for (id, c) in carriers.iter() {
            let item = match c.kind {
                CarrierKind::Rail => Item::Rail(id),
                CarrierKind::Duct => Item::Duct(id),
                CarrierKind::Unknown(_) => continue,
            };
            if let Some(g) = q.get::<CarrierGeometry>(id).as_ref() {
                for r in &g.outline {
                    by_surface.entry(g.surface).or_default().push((*r, item));
                }
            }
        }
        let mut pairs: BTreeSet<(Item, Item)> = BTreeSet::new();
        let mut surface_of: BTreeMap<(Item, Item), Id<MountingSurface>> = BTreeMap::new();
        let mut crowded: Vec<Id<MountingSurface>> = Vec::new();
        for (surface, items) in by_surface {
            let (found, more) = surface_overlaps(items);
            for key in found {
                pairs.insert(key);
                surface_of.insert(key, surface);
            }
            if more {
                crowded.push(surface);
            }
        }
        let mut out = Vec::new();
        for surface in crowded {
            out.push(crowded_finding(q, surface));
        }
        for pair in pairs {
            let surface = surface_of.get(&pair).copied();
            let f = match pair {
                (Item::Placement(x), Item::Placement(y)) => {
                    let (Some(px), Some(py)) = (placements.get(x), placements.get(y)) else {
                        continue;
                    };
                    if px.device == py.device {
                        // two placements of one device (one per part): cycle 41's ruling 2
                        let name = device_name(q, px.device);
                        let f = Finding::new(
                            &COLLISION,
                            placement_targets(px)
                                .into_iter()
                                .chain([Target::new(Kind::Placement, py.id)]),
                            "overlap",
                            name.clone(),
                            format!("two placements of {name} overlap; move one of them"),
                        )
                        .arg("form", "same")
                        .arg("device", name);
                        out.push(shown(f, q, surface));
                        continue;
                    }
                    let (mut nx, mut ny) = (device_name(q, px.device), device_name(q, py.device));
                    if natural_cmp(&nx, &ny).then_with(|| nx.cmp(&ny)).is_gt() {
                        std::mem::swap(&mut nx, &mut ny);
                    }
                    Finding::new(
                        &COLLISION,
                        placement_targets(px).into_iter().chain(placement_targets(py)),
                        "overlap",
                        nx.clone(),
                        format!("{nx} overlaps {ny}; move one of them"),
                    )
                    .arg("form", "placements")
                    .arg("device", nx)
                    .arg("other", ny)
                }
                (Item::Placement(x), Item::Duct(d)) => {
                    let (Some(px), Some(cd)) = (placements.get(x), carriers.get(d)) else {
                        continue;
                    };
                    let name = device_name(q, px.device);
                    let at = carrier_at(cd);
                    Finding::new(
                        &COLLISION,
                        placement_targets(px)
                            .into_iter()
                            .chain([Target::new(Kind::Carrier, d)]),
                        "overlap",
                        name.clone(),
                        format!("{name} overlaps the duct at {at}; move the device or the duct"),
                    )
                    .arg("form", "duct")
                    .arg("device", name)
                    .arg("duct", at)
                }
                (Item::Rail(r), Item::Duct(d)) => {
                    let (Some(cr), Some(cd)) = (carriers.get(r), carriers.get(d)) else {
                        continue;
                    };
                    let (rail, duct) = (carrier_at(cr), carrier_at(cd));
                    let subject = format!("the rail at {rail}");
                    Finding::new(
                        &COLLISION,
                        [Target::new(Kind::Carrier, r), Target::new(Kind::Carrier, d)],
                        "overlap",
                        subject,
                        format!("the rail at {rail} overlaps the duct at {duct}; move the rail or the duct"),
                    )
                    .arg("form", "rail")
                    .arg("rail", rail)
                    .arg("duct", duct)
                }
                _ => continue,
            };
            out.push(shown(f, q, surface));
        }
        out
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-010

/// EW-LAY-010 "Carrier incompatible" (doc 15 §4.5.4): conflict, o r, error, carrier (as built: project) (FR-LAY-014).
pub const CARRIER_INCOMPATIBLE: RuleMeta = RuleMeta {
    id: "EW-LAY-010",
    code: "130010",
    pack: "EW-LAYOUT",
    title: "Carrier incompatible",
    kind: RuleKind::Conflict,
    modes: Modes::letters("or"),
    severity: Severity::Error,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// EW-LAY-010 (D12, FT-845), one finding per case, each with its discriminator (the `form` argument):
///
/// - per placement, the first of: on a duct (`on_duct`: a duct carries no placements); a rail pose beside the rail's
///   axis (`off_axis`), turned other than 0° or 180° (`rail_turn`), or with `s` outside `[0, L]` (`off_rail`); a
///   surface pose off the plate's plane (`off_surface`) or turned other than `k · 90°` (`surface_turn`); and when the
///   pose is valid, each of: a part whose mount kind does not fit the rail's profile (`profile`, doc 12 §3.3's table)
///   or does not mount on the surface itself (`surface`, a rail part on the plate); a footprint whose interval on its
///   rail leaves `[0, L]` (`overflow`: a strip grown by added terminals); a footprint outside the surface's usable
///   polygon (`outside`). A placement on a rail whose path breaks D5 is left to the rail's own finding;
/// - per carrier: a path that breaks D5 (`path`: a rail that is not two points on one horizontal or vertical line, a
///   duct that is not an orthogonal polyline; such a carrier has no length, D11), or an outline outside the usable
///   polygon (`outside`);
/// - D3's MVP limits: a second layout space of one location (`spaces`), a second enclosure in one layout space
///   (`enclosures`), a second plate surface of one enclosure (`plates`); the record with the smallest ID is the one
///   the layout uses, every other is reported.
///
/// Placement findings target the device and the placement; carrier findings the carrier; limit findings the extra
/// layout space, enclosure or surface.
#[derive(Clone, Copy, Debug)]
pub struct CarrierIncompatible;

impl Rule for CarrierIncompatible {
    fn meta(&self) -> &'static RuleMeta {
        &CARRIER_INCOMPATIBLE
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (_, pl) in q.table::<Placement>().iter() {
            placement_cases(q, pl, &mut out);
        }
        for (id, c) in q.table::<Carrier>().iter() {
            carrier_cases(q, id, c, &mut out);
        }
        limit_cases(q, &mut out);
        out
    }
}

/// A placement finding of EW-LAY-010 with its form and arguments.
fn lay010(q: &Q, pl: &Placement, form: &'static str, message: String) -> Finding {
    let name = device_name(q, pl.device);
    let f = Finding::new(
        &CARRIER_INCOMPATIBLE,
        placement_targets(pl),
        form,
        name.clone(),
        message,
    )
    .arg("form", form)
    .arg("device", name);
    shown(f, q, placement_surface(q, pl))
}

/// The placement cases of EW-LAY-010.
fn placement_cases(q: &Q, pl: &Placement, out: &mut Vec<Finding>) {
    let name = device_name(q, pl.device);
    let part = realised_part(q, pl);
    let (mount, copy) = mount_of(q, part.as_ref());
    let confidence = copy.as_ref().map(|c| part_confidence(c, "mech"));
    let part_text = part
        .as_ref()
        .map_or_else(|| "none".to_owned(), |p| part_name(q, p));
    let mark = |f: Finding| match &confidence {
        Some(c) => f.reading(c),
        None => f,
    };
    let rail = match &pl.carrier {
        CarrierRef::Carrier(c) => {
            let Some(carrier) = q.table::<Carrier>().get(*c) else {
                return;
            };
            match carrier.kind {
                CarrierKind::Duct => {
                    let at = carrier_at(carrier);
                    out.push(
                        lay010(
                            q,
                            pl,
                            "on_duct",
                            format!("{name} is placed on the duct at {at}, which carries no devices; place it on a rail"),
                        )
                        .arg("duct", at),
                    );
                    return;
                }
                CarrierKind::Unknown(_) => return,
                CarrierKind::Rail => {}
            }
            // a rail whose path breaks D5 has no axis: its own `path` finding reports it (FT-845)
            let Some(axis) = RailAxis::of(&carrier.path) else {
                return;
            };
            let at = carrier_at(carrier);
            if let Err(e) = check_pose(PoseOn::Rail(axis), &pl.pose) {
                let f = match e {
                    PoseError::OffRail(s, l) => lay010(
                        q,
                        pl,
                        "off_rail",
                        format!(
                            "{name} is placed at {} mm on the rail at {at}, outside its 0.0 to {} mm; move it onto \
                             the rail",
                            mm1(s),
                            mm1(l)
                        ),
                    )
                    .arg("rail", at)
                    .arg("s", mm1(s))
                    .arg("length", mm1(l)),
                    PoseError::RailTurn => lay010(
                        q,
                        pl,
                        "rail_turn",
                        format!("{name} is turned on the rail at {at} by an angle other than 0° or 180°; turn it back"),
                    )
                    .arg("rail", at),
                    _ => lay010(
                        q,
                        pl,
                        "off_axis",
                        format!("{name} is placed beside the axis of the rail at {at}; move it onto the rail"),
                    )
                    .arg("rail", at),
                };
                out.push(f);
                return;
            }
            let CarrierParams::Rail(params) = &carrier.params else {
                return;
            };
            if fits_rail(mount.as_ref(), &params.profile) == Fit::DoesNotFit {
                let profile = profile_text(&params.profile);
                out.push(mark(
                    lay010(
                        q,
                        pl,
                        "profile",
                        format!(
                            "{name} (part {part_text}) does not fit the rail at {at} ({profile}); place it on a rail \
                             it fits"
                        ),
                    )
                    .arg("part", part_text.clone())
                    .arg("rail", at.clone())
                    .arg("profile", profile),
                ));
            }
            Some((*c, axis, at))
        }
        CarrierRef::Surface(_) => {
            if let Err(e) = check_pose(PoseOn::Surface, &pl.pose) {
                let f = match e {
                    PoseError::SurfaceTurn => lay010(
                        q,
                        pl,
                        "surface_turn",
                        format!(
                            "{name} is turned on the mounting plate by an angle other than a multiple of 90°; turn \
                             it back"
                        ),
                    ),
                    _ => lay010(
                        q,
                        pl,
                        "off_surface",
                        format!(
                            "{name} is placed off the plane of the mounting plate; place it on the plate"
                        ),
                    ),
                };
                out.push(f);
                return;
            }
            if fits_surface(mount.as_ref()) == Fit::DoesNotFit {
                out.push(mark(
                    lay010(
                        q,
                        pl,
                        "surface",
                        format!(
                            "{name} (part {part_text}) does not mount on the plate itself; place it on a rail"
                        ),
                    )
                    .arg("part", part_text.clone()),
                ));
            }
            None
        }
        CarrierRef::Unknown(_) => return,
    };
    let geometry = q.get::<PlacementGeometry>(pl.id);
    let Some(geom) = geometry.as_ref() else {
        return;
    };
    if let Some((carrier, axis, at)) = rail
        && let Some(o) = q
            .get::<RailOccupancy>(carrier)
            .iter()
            .find(|o| o.placement == pl.id)
        && (o.s0 < 0 || o.s1 > axis.length)
    {
        out.push(
            lay010(
                q,
                pl,
                "overflow",
                format!(
                    "{name} occupies {} to {} mm on the rail at {at}, beyond its 0.0 to {} mm; shorten it or move it",
                    mm1(o.s0),
                    mm1(o.s1),
                    mm1(axis.length)
                ),
            )
            .arg("rail", at)
            .arg("from", mm1(o.s0))
            .arg("to", mm1(o.s1))
            .arg("length", mm1(axis.length)),
        );
    }
    // borrowed, not cloned per placement (cycle 41's ruling 1)
    let usable: &[Pt2] = q
        .table::<MountingSurface>()
        .get(geom.surface)
        .map_or(&[], |s| s.usable.as_slice());
    if inside_usable(usable, geom.footprint) == Some(false) {
        out.push(lay010(
            q,
            pl,
            "outside",
            format!("{name} lies outside the usable area of the mounting plate; move it inside"),
        ));
    }
}

/// The carrier cases of EW-LAY-010: a path that breaks D5, an outline outside the usable polygon.
fn carrier_cases(q: &Q, id: Id<Carrier>, c: &Carrier, out: &mut Vec<Finding>) {
    let geometry = q.get::<CarrierGeometry>(id);
    let Some(geom) = geometry.as_ref() else {
        return;
    };
    let word = carrier_word(&c.kind);
    let at = carrier_at(c);
    let noun = match c.kind {
        CarrierKind::Rail => "rail",
        CarrierKind::Duct => "duct",
        CarrierKind::Unknown(_) => "carrier",
    };
    let target = [Target::new(Kind::Carrier, id)];
    let subject = format!("the {noun} at {at}");
    if geom.path_error.is_some() {
        let what = match c.kind {
            CarrierKind::Rail => "two points on one horizontal or vertical line",
            _ => "a line of horizontal and vertical segments",
        };
        let f = Finding::new(
            &CARRIER_INCOMPATIBLE,
            target,
            "path",
            subject,
            format!("the path of the {noun} at {at} is not {what}; edit its path"),
        )
        .arg("form", "path")
        .arg("carrier", word)
        .arg("at", at);
        out.push(shown(f, q, Some(c.surface)));
        return;
    }
    // borrowed, not cloned per carrier (cycle 41's ruling 1)
    let usable: &[Pt2] = q
        .table::<MountingSurface>()
        .get(c.surface)
        .map_or(&[], |s| s.usable.as_slice());
    if geom
        .outline
        .iter()
        .any(|r| inside_usable(usable, *r) == Some(false))
    {
        let f = Finding::new(
            &CARRIER_INCOMPATIBLE,
            target,
            "outside",
            subject,
            format!("the {noun} at {at} lies outside the usable area of the mounting plate; move it inside"),
        )
        .arg("form", "carrier_outside")
        .arg("carrier", word)
        .arg("at", at);
        out.push(shown(f, q, Some(c.surface)));
    }
}

/// D3's MVP limits: one layout space per location, one enclosure per space, one plate surface per enclosure.
fn limit_cases(q: &Q, out: &mut Vec<Finding>) {
    let devices_tag = |d: Id<Device>| device_name(q, d);
    // `iter` is in ID order: the first record of each owner is the one the layout uses, every later one is extra
    let mut seen_location: BTreeSet<Id<StructureNode>> = BTreeSet::new();
    for (id, s) in q.table::<LayoutSpace>().iter() {
        if seen_location.insert(s.location) {
            continue;
        }
        let location = location_text(q, s.location);
        out.push(
            Finding::new(
                &CARRIER_INCOMPATIBLE,
                [Target::new(Kind::LayoutSpace, id)],
                "spaces",
                location.clone(),
                format!("{location} has a second layout space; a location has one layout space; remove it"),
            )
            .arg("form", "spaces")
            .arg("location", location),
        );
    }
    let spaces = q.table::<LayoutSpace>();
    let mut seen_space: BTreeSet<Id<LayoutSpace>> = BTreeSet::new();
    for (id, e) in q.table::<Enclosure>().iter() {
        if seen_space.insert(e.space) {
            continue;
        }
        let location = spaces
            .get(e.space)
            .map_or_else(|| "a location".to_owned(), |s| location_text(q, s.location));
        let enclosure = devices_tag(e.device);
        out.push(
            Finding::new(
                &CARRIER_INCOMPATIBLE,
                [Target::new(Kind::Enclosure, id)],
                "enclosures",
                enclosure.clone(),
                format!(
                    "the layout of {location} has a second enclosure, {enclosure}; a layout space has one enclosure; \
                     remove it"
                ),
            )
            .arg("form", "enclosures")
            .arg("location", location)
            .arg("enclosure", enclosure),
        );
    }
    let enclosures = q.table::<Enclosure>();
    let mut seen_enclosure: BTreeSet<Id<Enclosure>> = BTreeSet::new();
    for (id, s) in q.table::<MountingSurface>().iter() {
        if s.kind != SurfaceKind::Plate || seen_enclosure.insert(s.enclosure) {
            continue;
        }
        let enclosure = enclosures
            .get(s.enclosure)
            .map_or_else(|| "a missing enclosure".to_owned(), |e| devices_tag(e.device));
        let f = Finding::new(
            &CARRIER_INCOMPATIBLE,
            [Target::new(Kind::MountingSurface, id)],
            "plates",
            enclosure.clone(),
            format!(
                "the enclosure {enclosure} has a second mounting plate; an enclosure has one mounting plate; remove it"
            ),
        )
        .arg("form", "plates")
        .arg("enclosure", enclosure);
        out.push(shown(f, q, Some(id)));
    }
}

// ---------------------------------------------------------------------------------------------------------
// EW-LAY-016

/// EW-LAY-016 "Assembly order" (doc 15 §4.5.4): hygiene, o r, warning, carrier (as built: project) (FR-LAY-061).
pub const ASSEMBLY_ORDER: RuleMeta = RuleMeta {
    id: "EW-LAY-016",
    code: "130016",
    pack: "EW-LAYOUT",
    title: "Assembly order",
    kind: RuleKind::Hygiene,
    modes: Modes::letters("or"),
    severity: Severity::Warning,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// The sort key of an assembly member: the parent first, then `Device.position` (a PLC module's slot under its
/// station is its position), then the full designation in natural order, then the ID.
type MemberKey = (bool, u32, String, Id<Device>);

/// EW-LAY-016: on each rail, the placed members of one assembly or PLC station (the parent device and its children,
/// `Device.parent`; a station's modules are its children) out of their order along the rail (`order`: the parent
/// first, then by position), else with another device's placement between two members (`between`). Unplaced members
/// are skipped (-201U9 is EW-LAY-001's). One finding per group and rail, naming the first pair out of order or the
/// first device between members; targets the parent device and the rail.
#[derive(Clone, Copy, Debug)]
pub struct AssemblyOrder;

impl Rule for AssemblyOrder {
    fn meta(&self) -> &'static RuleMeta {
        &ASSEMBLY_ORDER
    }

    fn run(&self, q: &Q, scope: &Scope) -> Vec<Finding> {
        if *scope != Scope::Project {
            return Vec::new();
        }
        let devices = q.devices();
        let mut out = Vec::new();
        for (cid, c) in q.table::<Carrier>().iter() {
            if c.kind != CarrierKind::Rail {
                continue;
            }
            let occ = q.get::<RailOccupancy>(cid);
            // group root → (member key, index along the rail)
            let mut groups: BTreeMap<Id<Device>, Vec<(MemberKey, usize)>> = BTreeMap::new();
            for (i, o) in occ.iter().enumerate() {
                let Some(d) = devices.get(o.device) else {
                    continue;
                };
                let key = |is_child: bool| {
                    (
                        is_child,
                        d.position.unwrap_or(u32::MAX),
                        device_name(q, d.id),
                        d.id,
                    )
                };
                if let Some(parent) = d.parent {
                    groups.entry(parent).or_default().push((key(true), i));
                }
                if q.idx().device_children(d.id).next().is_some() {
                    groups.entry(d.id).or_default().push((key(false), i));
                }
            }
            let at = carrier_at(c);
            for (root, mut members) in groups {
                if members.len() < 2 {
                    continue;
                }
                members.sort_by(|a, b| {
                    (a.0.0, a.0.1)
                        .cmp(&(b.0.0, b.0.1))
                        .then_with(|| natural_cmp(&a.0.2, &b.0.2))
                        .then_with(|| a.0.3.cmp(&b.0.3))
                });
                let assembly = device_name(q, root);
                let targets = [Target::new(Kind::Device, root), Target::new(Kind::Carrier, cid)];
                // the first member (in the wanted order) placed after a later one along the rail
                let mut along: Vec<(usize, usize)> =
                    members.iter().enumerate().map(|(w, (_, i))| (*i, w)).collect();
                along.sort();
                let wrong = along.windows(2).find(|p| p[0].1 > p[1].1);
                let f = if let Some(p) = wrong {
                    let (late, early) = (&members[p[0].1].0, &members[p[1].1].0);
                    let (early, late) = (early.2.clone(), late.2.clone());
                    Finding::new(
                        &ASSEMBLY_ORDER,
                        targets,
                        "order",
                        assembly.clone(),
                        format!(
                            "the members of {assembly} on the rail at {at} are out of order: {early} belongs before \
                             {late}; place the parent first, then the members by position"
                        ),
                    )
                    .arg("form", "order")
                    .arg("assembly", assembly)
                    .arg("rail", at.clone())
                    .arg("early", early)
                    .arg("late", late)
                } else {
                    let (lo, hi) = (along[0].0, along[along.len() - 1].0);
                    let inside: BTreeSet<usize> = along.iter().map(|(i, _)| *i).collect();
                    let Some(intruder) = (lo..=hi).find(|i| !inside.contains(i)) else {
                        continue;
                    };
                    let other = device_name(q, occ[intruder].device);
                    Finding::new(
                        &ASSEMBLY_ORDER,
                        targets,
                        "between",
                        assembly.clone(),
                        format!(
                            "{other} is placed between members of {assembly} on the rail at {at}; keep the members \
                             together"
                        ),
                    )
                    .arg("form", "between")
                    .arg("assembly", assembly)
                    .arg("rail", at.clone())
                    .arg("other", other)
                };
                out.push(shown(f, q, Some(c.surface)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_derive::ew_units::Nm;

    fn p(u: i64, v: i64) -> Pt2 {
        Pt2::new(Nm(u), Nm(v))
    }

    fn r(a: (i64, i64), b: (i64, i64)) -> Rect {
        Rect::from_points(p(a.0, a.1), p(b.0, b.1))
    }

    /// The sweep against the brute-force oracle (every pair compared) on seeded random layouts on a coarse grid, so
    /// that shared and touching edges, nested rectangles and empty rectangles are common: the same set of pairs
    /// below the cap, and the cap reported exactly when the oracle has more pairs.
    #[test]
    fn the_sweep_finds_the_pairs_the_oracle_finds() {
        let id = |n: u128| Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(n));
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = |m: u64| {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % m
        };
        for round in 0..400 {
            let count = 1 + next(if round % 10 == 0 { 60 } else { 14 });
            let items: Vec<(Rect, Item)> = (0..count)
                .map(|i| {
                    let (u, v) = (next(12) as i64, next(12) as i64);
                    let (w, h) = (next(6) as i64, next(6) as i64);
                    // odd rounds repeat IDs: one carrier with several segment rectangles
                    let n = if round % 2 == 0 {
                        u128::from(i)
                    } else {
                        u128::from(next(8))
                    } + 1;
                    let item = match next(3) {
                        0 => Item::Placement(id(n)),
                        1 => Item::Rail(id(n).cast()),
                        _ => Item::Duct(id(n).cast()),
                    };
                    (r((u, v), (u + w, v + h)), item)
                })
                .collect();
            let mut oracle: BTreeSet<(Item, Item)> = BTreeSet::new();
            for (i, (a, ia)) in items.iter().enumerate() {
                for (b, ib) in &items[i + 1..] {
                    if judged(*ia, *ib) && overlaps(*a, *b) {
                        oracle.insert(if ia <= ib { (*ia, *ib) } else { (*ib, *ia) });
                    }
                }
            }
            let (found, more) = surface_overlaps(items.clone());
            let found_set: BTreeSet<(Item, Item)> = found.iter().copied().collect();
            assert_eq!(found_set.len(), found.len(), "round {round}: each pair once");
            assert_eq!(
                more,
                oracle.len() > OVERLAPS_PER_SURFACE,
                "round {round}: the cap"
            );
            if more {
                assert_eq!(found.len(), OVERLAPS_PER_SURFACE, "round {round}");
                assert!(found_set.is_subset(&oracle), "round {round}");
            } else {
                assert!(
                    found_set == oracle,
                    "round {round}: missed {:?}, extra {:?}",
                    oracle.difference(&found_set).collect::<Vec<_>>(),
                    found_set.difference(&oracle).collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn millimetres_print_with_one_decimal_rounded_half_away_from_zero() {
        assert_eq!(mm1(0), "0.0");
        assert_eq!(mm1(40_000_000), "40.0");
        assert_eq!(mm1(58_600_000), "58.6");
        assert_eq!(mm1(149_950_000), "150.0");
        assert_eq!(mm1(-500_000), "-0.5");
        assert_eq!(mm1(-40_000), "0.0", "no negative zero");
    }

    #[test]
    fn rectangles_that_touch_do_not_overlap() {
        assert!(overlaps(r((0, 0), (10, 10)), r((5, 5), (15, 15))));
        assert!(!overlaps(r((0, 0), (10, 10)), r((10, 0), (20, 10))));
        assert!(!overlaps(r((0, 0), (10, 10)), r((0, 10), (10, 20))));
        assert!(
            !overlaps(r((0, 0), (10, 10)), r((5, 2), (5, 8))),
            "no area inside"
        );
        assert!(
            !overlaps(r((0, 0), (10, 10)), r((2, 5), (8, 5))),
            "no area inside"
        );
    }

    #[test]
    fn a_rectangle_inside_an_orthogonal_polygon_and_one_across_a_notch() {
        let plate = [p(0, 0), p(100, 0), p(100, 100), p(0, 100)];
        assert_eq!(
            inside_usable(&plate, r((0, 0), (100, 100))),
            Some(true),
            "the boundary counts"
        );
        assert_eq!(inside_usable(&plate, r((90, 10), (101, 20))), Some(false));
        // an L: the notch (50..100, 50..100) is outside
        let l = [p(0, 0), p(100, 0), p(100, 50), p(50, 50), p(50, 100), p(0, 100)];
        assert_eq!(inside_usable(&l, r((10, 10), (40, 90))), Some(true));
        assert_eq!(
            inside_usable(&l, r((40, 40), (60, 60))),
            Some(false),
            "a vertex inside"
        );
        assert_eq!(
            inside_usable(&l, r((60, 60), (90, 90))),
            Some(false),
            "in the notch"
        );
        assert_eq!(
            inside_usable(&[], r((0, 0), (1, 1))),
            None,
            "no usable area: nothing to judge"
        );
    }
}
