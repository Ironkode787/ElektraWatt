// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The surfaces of a cabinet (doc 12 §2.8): the mounting plate and the six faces, each seen from the side it is
//! worked from, with its sheet, its frame in the enclosure, a door's hinge, the edge margin and the datum its holes
//! are measured from. A face the enclosure part offers is a surface before it has a record: `surfaces` lists it with
//! its size from the part, and `layout.surface.add` makes its record from [`surface_frame`].
//!
//! | Face | Kind | Sheet | `u`, `v` in the enclosure | Frame |
//! |---|---|---|---|---|
//! | Mounting plate | `plate` | the plate part's | `x`, `y` | [`plate_frame`](super::plate_frame) |
//! | Door i | `door` | its width × the enclosure's height | `x` from the door's left edge, `y` | at the front, no turn |
//! | Left side | `left` | depth × height | `z`, `y` | −90° about `y` |
//! | Right side | `right` | depth × height | `depth − z`, `y` | +90° about `y`, at the right front edge |
//! | Top | `top` | width × depth | `x`, `depth − z` | −90° about `x`, at the top front edge |
//! | Bottom | `bottom` | width × depth | `x`, `z` | +90° about `x` |

use ew_ids::Id;
use ew_schema::{
    Corner, Enclosure, EnclosureSection, Hinge, LibCopy, MountingSurface, Page, Part, SurfaceKind,
};
use ew_units::{Nm, Pose3, Pt2, Quat9};

use super::legend::surface_pages;
use crate::{DrawnPageOrder, PageRef, Q};

/// `½√2` in 10⁻⁹, a quarter turn's quaternion component.
const HALF_SQRT2: i64 = 707_106_781;

/// One face of a cabinet as `surfaces` lists it: what it is, its size, where it lies, and its record and page once
/// they are made.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Face {
    /// The kind.
    pub kind: SurfaceKind,
    /// A door's place among the enclosure's doors, from the left (0 …); `None` for any other face.
    pub door: Option<usize>,
    /// How many doors the enclosure has: what a door's words depend on ("Door", "Left door", "Door 3").
    pub doors: usize,
    /// The sheet's width and height (`u` × `v`): the record's when made, else the part's; `None` when the part does
    /// not give it.
    pub size: Option<(Nm, Nm)>,
    /// A door's hinge.
    pub hinge: Option<Hinge>,
    /// The edge margin of a door or a wall.
    pub margin: Option<Nm>,
    /// The record, once made.
    pub record: Option<Id<MountingSurface>>,
    /// The first page, in page order, that shows it.
    pub page: Option<Id<Page>>,
}

/// The enclosure section of an enclosure's device: its first part use whose part carries one.
pub fn enclosure_section(q: &Q, enclosure: Id<Enclosure>) -> Option<EnclosureSection> {
    let device = q.table::<Enclosure>().get(enclosure)?.device;
    let copies = q.table::<LibCopy<Part>>();
    q.devices()
        .get(device)?
        .parts
        .iter()
        .find_map(|u| copies.get(u.part.copy)?.item.sections.enclosure.clone())
}

/// How many doors an enclosure section offers: its `doors`, one when it lists none.
pub fn door_count(section: &EnclosureSection) -> usize {
    section.doors.len().max(1)
}

/// Door `i`'s left edge and width in the enclosure (`x`): the widths the part gives, the enclosure's width shared
/// equally by the doors that give none; `None` without the enclosure's width or for a door the part does not list.
fn door_span(section: &EnclosureSection, i: usize) -> Option<(Nm, Nm)> {
    let n = door_count(section);
    if i >= n {
        return None;
    }
    let total = section.width?;
    let given: i64 = section.doors.iter().filter_map(|d| d.width).map(|w| w.0).sum();
    let open = if section.doors.is_empty() {
        1
    } else {
        section.doors.iter().filter(|d| d.width.is_none()).count()
    };
    let share = Nm(total.0.saturating_sub(given) / i64::try_from(open.max(1)).unwrap_or(1));
    let width = |k: usize| section.doors.get(k).and_then(|d| d.width).unwrap_or(share);
    let left = Nm((0..i).map(|k| width(k).0).sum());
    Some((left, width(i)))
}

/// A door's hinge as the part gives it: the door's own, else one door on the left; of two, the left on the left and the
/// right on the right.
fn door_hinge(section: &EnclosureSection, i: usize) -> Hinge {
    match section.doors.get(i) {
        Some(d) => d.hinge.clone(),
        None if i > 0 && i + 1 == door_count(section) => Hinge::Right,
        None => Hinge::Left,
    }
}

/// The sheet's size of a face as the part gives it (doc 12 §2.8); `None` where the part lacks a length it needs (an
/// enclosure part without a depth gives no side, top or bottom), and for the plate (its size is the plate part's).
pub fn face_size(section: &EnclosureSection, kind: &SurfaceKind, door: Option<usize>) -> Option<(Nm, Nm)> {
    match kind {
        SurfaceKind::Door => Some((door_span(section, door.unwrap_or(0))?.1, section.height?)),
        SurfaceKind::Left | SurfaceKind::Right => Some((section.depth?, section.height?)),
        SurfaceKind::Top | SurfaceKind::Bottom => Some((section.width?, section.depth?)),
        SurfaceKind::Plate | SurfaceKind::Unknown(_) => None,
    }
}

/// The frame and the sheet (`(0, 0)` to `size`) of a face other than the plate, made from the enclosure part: the
/// origin and the turn of the table in the module documentation; `size` the sheet's, the part's when `None`. `None`
/// for the plate (`plate_frame`), a kind of a newer format, a door the part does not list, or a size neither given nor
/// in the part. An unknown enclosure length places the frame as if it were zero.
pub fn surface_frame(
    section: &EnclosureSection,
    kind: &SurfaceKind,
    door: Option<usize>,
    size: Option<(Nm, Nm)>,
) -> Option<(Pose3, Vec<Pt2>)> {
    let (w, h) = size.or_else(|| face_size(section, kind, door))?;
    let (width, height, depth) = (
        section.width.unwrap_or(Nm::ZERO),
        section.height.unwrap_or(Nm::ZERO),
        section.depth.unwrap_or(Nm::ZERO),
    );
    let quat = |w, x, y| Quat9::new(w, x, y, 0).ok();
    let (t, q) = match kind {
        SurfaceKind::Door => {
            let left = door_span(section, door.unwrap_or(0)).map_or(Nm::ZERO, |(l, _)| l);
            ([left, Nm::ZERO, depth], Quat9::IDENTITY)
        }
        SurfaceKind::Left => ([Nm::ZERO; 3], quat(HALF_SQRT2, 0, -HALF_SQRT2)?),
        SurfaceKind::Right => ([width, Nm::ZERO, depth], quat(HALF_SQRT2, 0, HALF_SQRT2)?),
        SurfaceKind::Top => ([Nm::ZERO, height, depth], quat(HALF_SQRT2, -HALF_SQRT2, 0)?),
        SurfaceKind::Bottom => ([Nm::ZERO; 3], quat(HALF_SQRT2, HALF_SQRT2, 0)?),
        SurfaceKind::Plate | SurfaceKind::Unknown(_) => return None,
    };
    let sheet = vec![
        Pt2::new(Nm::ZERO, Nm::ZERO),
        Pt2::new(w, Nm::ZERO),
        Pt2::new(w, h),
        Pt2::new(Nm::ZERO, h),
    ];
    Some((Pose3 { t, q }, sheet))
}

/// The bounds `(w, h)` of a surface's sheet: its usable polygon's extent from the origin.
pub fn sheet_size(s: &MountingSurface) -> Option<(Nm, Nm)> {
    let w = s.usable.iter().map(|p| p.x).max()?;
    let h = s.usable.iter().map(|p| p.y).max()?;
    Some((w, h))
}

/// The door records of an enclosure, left to right (by the frame's `x`, then ID): the i-th is door i.
fn door_records(q: &Q, enclosure: Id<Enclosure>) -> Vec<Id<MountingSurface>> {
    let mut doors: Vec<(Nm, Id<MountingSurface>)> = q
        .table::<MountingSurface>()
        .iter()
        .filter(|(_, s)| s.enclosure == enclosure && s.kind == SurfaceKind::Door)
        .map(|(id, s)| (s.frame.t[0], id))
        .collect();
    doors.sort();
    doors.into_iter().map(|(_, id)| id).collect()
}

/// A door record's place among its enclosure's doors; `None` for any other surface.
pub fn door_index(q: &Q, surface: Id<MountingSurface>) -> Option<usize> {
    let s = q.table::<MountingSurface>().get(surface)?;
    (s.kind == SurfaceKind::Door).then_some(())?;
    door_records(q, s.enclosure).iter().position(|d| *d == surface)
}

/// The first page, in drawn page order, that shows `surface`.
pub fn surface_page(q: &Q, surface: Id<MountingSurface>) -> Option<Id<Page>> {
    let order = q.get::<DrawnPageOrder>(());
    surface_pages(q, surface)
        .into_iter()
        .min_by_key(|p| (order.position(PageRef::Drawn(*p)).unwrap_or(usize::MAX), *p))
}

/// The face of a surface record, as `surfaces` lists it.
pub fn face_of(q: &Q, surface: Id<MountingSurface>) -> Option<Face> {
    let s = q.table::<MountingSurface>().get(surface)?;
    let doors = enclosure_section(q, s.enclosure)
        .map_or(1, |sec| door_count(&sec))
        .max(door_records(q, s.enclosure).len());
    Some(Face {
        kind: s.kind.clone(),
        door: door_index(q, surface),
        doors,
        size: sheet_size(s),
        hinge: s.hinge.clone(),
        margin: s.edge_margin,
        record: Some(surface),
        page: surface_page(q, surface),
    })
}

/// `surfaces(enclosure)`: the plate, each door left to right, the left and the right side, the top and the bottom
/// (doc 12 §2.8). A face with a record is the record's (its size, hinge and margin as stored); one without is the
/// part's, offered to be made. A surface record of a kind of a newer format is not listed.
pub fn surfaces(q: &Q, enclosure: Id<Enclosure>) -> Vec<Face> {
    let section = enclosure_section(q, enclosure).unwrap_or_default();
    let records: Vec<(Id<MountingSurface>, SurfaceKind)> = {
        let mut r: Vec<_> = q
            .table::<MountingSurface>()
            .iter()
            .filter(|(_, s)| s.enclosure == enclosure)
            .map(|(id, s)| (id, s.kind.clone()))
            .collect();
        r.sort_by_key(|(id, _)| *id);
        r
    };
    let doors = door_records(q, enclosure);
    let n = door_count(&section).max(doors.len());
    let made = |kind: &SurfaceKind| records.iter().find(|(_, k)| k == kind).map(|(id, _)| *id);
    let offered = |kind: SurfaceKind, door: Option<usize>| Face {
        size: face_size(&section, &kind, door),
        hinge: door.map(|i| door_hinge(&section, i)),
        margin: (kind != SurfaceKind::Plate)
            .then_some(section.edge_margin)
            .flatten(),
        kind,
        door,
        doors: n,
        record: None,
        page: None,
    };
    let face = |kind: SurfaceKind, record: Option<Id<MountingSurface>>, door: Option<usize>| match record {
        Some(id) => face_of(q, id).map(|f| Face { doors: n, ..f }),
        None => Some(offered(kind, door)),
    };
    let mut out = Vec::new();
    out.extend(made(&SurfaceKind::Plate).and_then(|id| face(SurfaceKind::Plate, Some(id), None)));
    for i in 0..n {
        out.extend(face(SurfaceKind::Door, doors.get(i).copied(), Some(i)));
    }
    for kind in [
        SurfaceKind::Left,
        SurfaceKind::Right,
        SurfaceKind::Top,
        SurfaceKind::Bottom,
    ] {
        let record = made(&kind);
        out.extend(face(kind, record, None));
    }
    out
}

/// The corner a surface's holes are measured from (doc 12 §2.1): its own, else the project's
/// (`WorkshopSettings.datum`), else the bottom-left one.
pub fn surface_datum(q: &Q, surface: &MountingSurface) -> Corner {
    surface
        .datum
        .clone()
        .or_else(|| q.project().settings.workshop.datum.clone())
        .unwrap_or_default()
}

/// `to_enclosure(surface, pt)`: a point of the plate or of a door in the enclosure's front view (`x`, `y`), so that a
/// door's point lies in front of the plate's point of the same `x`, `y` (doc 12 §2.8: seen from outside, nothing is
/// mirrored); `None` for any other face, a kind of a newer format or a missing surface.
pub fn to_enclosure(q: &Q, surface: Id<MountingSurface>, pt: Pt2) -> Option<Pt2> {
    let s = q.table::<MountingSurface>().get(surface)?;
    match s.kind {
        SurfaceKind::Plate | SurfaceKind::Door => {
            let [x, y, _] = s.frame.t;
            Some(Pt2::new(pt.x.checked_add(x)?, pt.y.checked_add(y)?))
        }
        _ => None,
    }
}
