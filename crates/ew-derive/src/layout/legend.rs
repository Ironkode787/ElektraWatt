// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout legend of a mounting surface (doc 12 §5.4), the surface a panel page shows (D9), and the
//! legend order of carriers that the BOM's carrier lines share (part 4).
//!
//! These are plain functions over the tracked reads of `q`, not memo families: the legend is read by one report kind
//! (`enclosure_legend`, whose rows are memoised per block by `ew-reports`), and the page's surface by the BOM's
//! folio column. Nothing here is stored: positions, the order and every cell are derived each time, so placing a
//! device renumbers the rows after it (doc 12 §5.4).

use std::cmp::Reverse;

use ew_ids::Id;
use ew_mltext::{NaturalKey, natural_key};
use ew_schema::{
    Carrier, CarrierKind, CarrierRef, Device, Enclosure, LibCopy, MountingSurface, Page, PageItemKind,
    PageType, Part, PartRef, Placement,
};
use ew_units::Dec3;

use super::accessories::GroupAccessories;
use super::queries::{CarrierGeometry, primary_part};
use crate::Q;
use crate::tag::DeviceTag;

/// Nanometres per millimetre: a length in nm over this is the `Dec3` thousandths of metres.
const NM_PER_MM: i128 = 1_000_000;

/// A length in nanometres as metres exact to the millimetre (`Dec3`, thousandths): rounded half up (lengths are not
/// negative; a negative input rounds half away from zero), saturating at the `Dec3` limits.
pub fn metres_of_nm(nm: i128) -> Dec3 {
    let half = NM_PER_MM / 2;
    let mm = if nm >= 0 {
        nm.saturating_add(half) / NM_PER_MM
    } else {
        nm.saturating_sub(half) / NM_PER_MM
    };
    Dec3(i64::try_from(mm).unwrap_or(if mm < 0 { i64::MIN } else { i64::MAX }))
}

// ---------------------------------------------------------------------------------------------------------
// The surface of a panel page

/// `page_surface(page)`: the mounting surface a panel-layout page shows: the surface of its first `surface_view` item by
/// ID (a page holds at most one; a file with more draws the first, as `ew-api`'s `page_surface_view` does). `None` for a
/// missing page, a page of another kind and a page without a view.
pub fn page_surface(q: &Q, page: Id<Page>) -> Option<Id<MountingSurface>> {
    let p = q.pages().get(page)?;
    if p.kind != PageType::PanelLayout {
        return None;
    }
    let items = q.page_items();
    let mut ids: Vec<_> = q.idx().page_items_by_page(page).collect();
    ids.sort();
    ids.into_iter().find_map(|id| match &items.get(id)?.kind {
        PageItemKind::SurfaceView(v) => Some(v.surface),
        _ => None,
    })
}

/// The panel-layout pages whose surface view shows `surface` ([`page_surface`]), in ID order.
pub fn surface_pages(q: &Q, surface: Id<MountingSurface>) -> Vec<Id<Page>> {
    q.pages()
        .iter()
        .filter(|(_, p)| p.kind == PageType::PanelLayout)
        .map(|(id, _)| id)
        .filter(|id| page_surface(q, *id) == Some(surface))
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// The legend order of carriers

/// The rank of a carrier kind in the legend: rails, then ducts, then a kind of a newer format.
fn kind_rank(kind: &CarrierKind) -> u8 {
    match kind {
        CarrierKind::Rail => 0,
        CarrierKind::Duct => 1,
        CarrierKind::Unknown(_) => 2,
    }
}

/// The sort key of a carrier in the legend (D11): rails, then ducts (then a kind of a newer format), each top to bottom
/// then left to right by `path[0]` (`v` descending, then `u`), a carrier without a path after those with one, the ID
/// breaking a tie.
pub type CarrierOrder = (u8, bool, Reverse<i64>, i64, Id<Carrier>);

/// The legend sort key of `carrier` (see [`CarrierOrder`]).
pub fn carrier_order(id: Id<Carrier>, carrier: &Carrier) -> CarrierOrder {
    let rank = kind_rank(&carrier.kind);
    match carrier.path.first() {
        Some(p) => (rank, false, Reverse(p.y.0), p.x.0, id),
        None => (rank, true, Reverse(0), 0, id),
    }
}

/// The carriers of a surface in legend order.
pub fn carriers_in_order(q: &Q, surface: Id<MountingSurface>) -> Vec<Id<Carrier>> {
    let carriers = q.table::<Carrier>();
    let mut out: Vec<(CarrierOrder, Id<Carrier>)> = q
        .idx()
        .carriers_of_surface(surface)
        .filter_map(|c| carriers.get(c).map(|rec| (carrier_order(c, rec), c)))
        .collect();
    out.sort();
    out.into_iter().map(|(_, c)| c).collect()
}

/// The kind word of a carrier label.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CarrierLabelKind {
    /// "Rail".
    Rail,
    /// "Duct".
    Duct,
}

/// A carrier's derived label: its kind and its number among the surface's carriers of that kind, from 1
/// in legend order ([`carrier_order`]). Never stored: a new carrier may renumber the others, like legend positions.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CarrierLabel {
    /// Rail or duct.
    pub kind: CarrierLabelKind,
    /// 1 … n per kind.
    pub n: u32,
}

impl CarrierLabel {
    /// The label as it stands alone (a legend cell, a Properties row, a ghost's chip): "Rail 1", "Duct 3".
    pub fn label(self) -> String {
        match self.kind {
            CarrierLabelKind::Rail => format!("Rail {}", self.n),
            CarrierLabelKind::Duct => format!("Duct {}", self.n),
        }
    }

    /// The label inside a sentence (doc 16a §8, sentence case): "rail 1", "duct 3".
    pub fn in_sentence(self) -> String {
        match self.kind {
            CarrierLabelKind::Rail => format!("rail {}", self.n),
            CarrierLabelKind::Duct => format!("duct {}", self.n),
        }
    }
}

impl std::fmt::Display for CarrierLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label())
    }
}

/// `carrier_labels(surface)`: the surface's rails and ducts in legend order ([`carriers_in_order`]: rails,
/// then ducts, each top to bottom then left to right by `path[0]`), each kind numbered from 1. A carrier of a kind this
/// build does not know gets no label (it carries no placements). On Golden A: R1 … R3 are Rail 1 … 3 and D1 … D5 are
/// Duct 1 … 5 (doc 20 §13).
pub fn carrier_labels(q: &Q, surface: Id<MountingSurface>) -> Vec<(Id<Carrier>, CarrierLabel)> {
    let carriers = q.table::<Carrier>();
    let (mut rails, mut ducts) = (0u32, 0u32);
    carriers_in_order(q, surface)
        .into_iter()
        .filter_map(|c| {
            let (kind, n) = match carriers.get(c)?.kind {
                CarrierKind::Rail => {
                    rails = rails.saturating_add(1);
                    (CarrierLabelKind::Rail, rails)
                }
                CarrierKind::Duct => {
                    ducts = ducts.saturating_add(1);
                    (CarrierLabelKind::Duct, ducts)
                }
                CarrierKind::Unknown(_) => return None,
            };
            Some((c, CarrierLabel { kind, n }))
        })
        .collect()
}

/// The label of one carrier ([`carrier_labels`] of its surface); `None` for a missing carrier or one of an unknown
/// kind.
pub fn carrier_label(q: &Q, carrier: Id<Carrier>) -> Option<CarrierLabel> {
    let surface = q.table::<Carrier>().get(carrier)?.surface;
    carrier_labels(q, surface)
        .into_iter()
        .find(|(c, _)| *c == carrier)
        .map(|(_, l)| l)
}

/// A carrier's path length in nanometres (`carrier_geometry`); `None` for a path that breaks D5 or a kind of a newer
/// format (EW-LAY-010 reports it).
pub fn carrier_length(q: &Q, carrier: Id<Carrier>) -> Option<i128> {
    q.get::<CarrierGeometry>(carrier).as_ref().as_ref()?.length
}

// ---------------------------------------------------------------------------------------------------------
// layout_legend(surface)

/// What a legend row lists.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LegendItem {
    /// The enclosure the surface belongs to (its device: -UC1).
    Enclosure(Id<Enclosure>),
    /// The mounting surface itself (its device: the plate -UC1-plate).
    Surface(Id<MountingSurface>),
    /// A carrier (a rail or a duct): what a shop cuts, one row each.
    Carrier(Id<Carrier>),
    /// A placement on one of the surface's carriers or on the surface itself.
    Placement(Id<Placement>),
    /// The derived end brackets of one bracket part on the surface's rails (doc 12 §5.4's layout-only
    /// accessories): one row per part (and variant), quantity the number of brackets.
    Bracket(PartRef),
}

/// One row of [`layout_legend`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LegendRow {
    /// The position, 1 … n, derived.
    pub pos: u32,
    /// What the row lists.
    pub item: LegendItem,
    /// The device whose designation the DT cell prints; `None` for a carrier (carriers have no designation member,
    /// FP-5), for a bracket row and for a surface without a device.
    pub device: Option<Id<Device>>,
    /// The part: the enclosure part of the enclosure device, the plate part of the surface, the carrier's part, the
    /// part use a placement realises (its `part`, else the device's primary part; `None` for a strip), the bracket
    /// part.
    pub part: Option<PartRef>,
    /// The quantity: a carrier's path length in metres exact to the millimetre (`None` without a derivable length),
    /// a bracket row's number of brackets, one for every other row.
    pub qty: Option<Dec3>,
}

/// The first part use of `device` whose part copy satisfies `pred`, else its first part use.
fn part_use_where(q: &Q, device: &Device, pred: impl Fn(&Part) -> bool) -> Option<PartRef> {
    let parts = q.table::<LibCopy<Part>>();
    device
        .parts
        .iter()
        .find(|u| parts.get(u.part.copy).is_some_and(|c| pred(&c.item)))
        .or_else(|| device.parts.first())
        .map(|u| u.part.clone())
}

/// `layout_legend(surface)`: the legend of one mounting surface (doc 12 §5.4, FR-LAY-012), in order:
///
/// 1. the enclosure: its device and that device's part use with an enclosure section (else its first part use);
/// 2. the surface: its device and that device's part use with a mounting-plate section (else its first part use);
///    a surface without a device lists the enclosure section's plate part when its copy is in the project;
/// 3. one row per carrier, in legend order ([`carrier_order`]: rails, then ducts, each top to bottom then left to right
///    by `path[0]`), the DT empty, the path length in metres as the quantity (none without a derivable length);
/// 4. the placements on the surface's carriers and on the surface itself, by the device's full designation in natural
///    order (then the device ID and the placement ID), each with the part use it realises;
/// 5. the layout-only accessories (doc 12 §5.4's last group): one row per end-bracket part (and variant)
///    of the brackets `group_accessories` derives on the surface's rails, the DT empty, the number of brackets as the
///    quantity, by part number in natural order (then the part copy's ID and the variant).
///
/// Positions are 1 … n. Free graphics are never read (FR-LAY-008). Empty for a missing surface.
pub fn layout_legend(q: &Q, surface: Id<MountingSurface>) -> Vec<LegendRow> {
    let Some(s) = q.table::<MountingSurface>().get(surface).cloned() else {
        return Vec::new();
    };
    let devices = q.devices();
    let parts = q.table::<LibCopy<Part>>();
    let mut rows: Vec<LegendRow> = Vec::new();
    let mut push = |item, device, part, qty| {
        rows.push(LegendRow {
            pos: 0,
            item,
            device,
            part,
            qty,
        });
    };

    // 1. The enclosure.
    let enclosure = q.table::<Enclosure>().get(s.enclosure).cloned();
    let enclosure_device = enclosure.as_ref().and_then(|e| devices.get(e.device).cloned());
    if let Some(e) = &enclosure {
        let part = enclosure_device
            .as_ref()
            .and_then(|d| part_use_where(q, d, |p| p.sections.enclosure.is_some()));
        push(LegendItem::Enclosure(e.id), Some(e.device), part, Some(Dec3::ONE));
    }

    // 2. The surface (the plate).
    let plate_part = match s.device.and_then(|d| devices.get(d).cloned()) {
        Some(d) => part_use_where(q, &d, |p| p.sections.mounting_plate.is_some()),
        None => enclosure_device
            .as_ref()
            .and_then(|d| {
                d.parts
                    .iter()
                    .find_map(|u| parts.get(u.part.copy)?.item.sections.enclosure.clone())
            })
            .and_then(|section| section.plate)
            .map(|r| ew_ids::v5::lib_copy_id::<Part>(r.library.as_uuid(), r.item.as_uuid()))
            .filter(|p| parts.contains(*p))
            .map(PartRef::new),
    };
    push(
        LegendItem::Surface(surface),
        s.device,
        plate_part,
        Some(Dec3::ONE),
    );

    // 3. The carriers.
    let carriers = q.table::<Carrier>();
    let order = carriers_in_order(q, surface);
    for c in &order {
        let part = carriers.get(*c).and_then(|rec| rec.part.clone());
        let qty = carrier_length(q, *c).map(metres_of_nm);
        push(LegendItem::Carrier(*c), None, part, qty);
    }

    // 4. The placements, by DT.
    let placements = q.table::<Placement>();
    let mut on: Vec<Id<Placement>> = Vec::new();
    for c in &order {
        on.extend(q.idx().placements_by_carrier(*c));
    }
    on.extend(
        placements
            .iter()
            .filter(|(_, p)| p.carrier == CarrierRef::Surface(surface))
            .map(|(id, _)| id),
    );
    /// A placement keyed for the legend order: the natural key of its full tag, its device and its own ID.
    type Keyed = (NaturalKey, Id<Device>, Id<Placement>, Option<PartRef>);
    let mut keyed: Vec<Keyed> = on
        .into_iter()
        .filter_map(|p| {
            let pl = placements.get(p)?;
            let device = devices.get(pl.device)?;
            let tag = q.get::<DeviceTag>(pl.device);
            let full = tag.as_ref().as_ref().map_or("", |t| t.full.as_str());
            let part = pl.part.clone().or_else(|| primary_part(q, device));
            Some((natural_key(full), pl.device, p, part))
        })
        .collect();
    keyed.sort_by(|a, b| (&a.0, a.1, a.2).cmp(&(&b.0, b.1, b.2)));
    keyed.dedup_by_key(|k| k.2);
    for (_, device, p, part) in keyed {
        push(LegendItem::Placement(p), Some(device), part, Some(Dec3::ONE));
    }

    // 5. The end brackets, one row per part.
    let mut brackets: Vec<(PartRef, usize)> = Vec::new();
    for c in &order {
        for b in q.get::<GroupAccessories>(*c).iter() {
            match brackets
                .iter_mut()
                .find(|(p, _)| p.copy == b.part.copy && p.variant == b.part.variant)
            {
                Some((_, n)) => *n = n.saturating_add(1),
                None => brackets.push((b.part.clone(), 1)),
            }
        }
    }
    let number = |p: &PartRef| natural_key(parts.get(p.copy).map_or("", |c| c.item.part_number.as_str()));
    brackets.sort_by_cached_key(|(p, _)| (number(p), p.copy, p.variant.clone()));
    for (part, n) in brackets {
        let qty = Dec3::from_int(i64::try_from(n).unwrap_or(i64::MAX)).unwrap_or(Dec3(i64::MAX));
        push(LegendItem::Bracket(part.clone()), None, Some(part), Some(qty));
    }

    for (i, row) in rows.iter_mut().enumerate() {
        row.pos = u32::try_from(i + 1).unwrap_or(u32::MAX);
    }
    rows
}
