// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bill of materials: `device_part_lines(dev)` → `bom_rows(mode)` (ARCH §6.4, FR-REP-004 … FR-REP-006,
//! D-04, D-36).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText, NaturalKey, natural_key};
use ew_schema::{Device, DeviceState, LibRef, OrderUnit, Part, PartKind, PartRef};
use ew_units::Dec3;

use crate::Q;
use crate::cell::RepAnchorCell;
use crate::pages::{PageOrder, PageOrderV, PageRef};
use crate::query::{Policy, Query};
use crate::tag::{DeviceTag, MainRep};

/// The two BOM modes (FR-REP-005).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum BomMode {
    /// Every component part; configured products are left out.
    Component,
    /// Configured products as one row each, replacing the components they cover.
    Assembly,
}

/// Where a part line comes from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LineSource {
    /// A part use of the device (`Device.parts`).
    Device,
    /// A strip accessory (`Strip.accessories[].part`), one piece each.
    StripAccessory,
}

/// One part line of one device.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PartLine {
    /// The device that owns the line.
    pub device: Id<Device>,
    /// The part copy.
    pub part: Id<Part>,
    /// The variant key.
    pub variant: Option<Ident>,
    /// The quantity in the part's order unit.
    pub qty: Dec3,
    /// Where it comes from.
    pub source: LineSource,
    /// Whether the part is a configured product (`PartKind::Configured`).
    pub configured: bool,
}

/// `device_part_lines(dev)`: the part lines of one device, in model order.
///
/// A missing device and a device that is not fitted have no lines. Otherwise one line per part use of the
/// device (`Device.parts` order), then, for a strip device, one line of quantity 1 per strip accessory
/// (mounting order). A line whose part copy is missing is dropped.
#[derive(Debug)]
pub struct DevicePartLines;

impl Query for DevicePartLines {
    type Key = Id<Device>;
    type Value = Vec<PartLine>;
    const FAMILY: &'static str = "device_part_lines";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<PartLine>> {
        let Some(dev) = q.devices().get(*key) else {
            return Arc::new(Vec::new());
        };
        if dev.state == DeviceState::NotFitted {
            return Arc::new(Vec::new());
        }
        let parts = q.parts();
        let mut lines = Vec::new();
        let mut push = |part: &PartRef, qty: Dec3, source: LineSource| {
            if let Some(copy) = parts.get(part.copy) {
                lines.push(PartLine {
                    device: *key,
                    part: part.copy,
                    variant: part.variant.clone(),
                    qty,
                    source,
                    configured: copy.item.kind == PartKind::Configured,
                });
            }
        };
        for u in &dev.parts {
            push(&u.part, u.qty, LineSource::Device);
        }
        if let Some(strip) = &dev.aspects.strip {
            for a in &strip.accessories {
                push(&a.part, Dec3::ONE, LineSource::StripAccessory);
            }
        }
        Arc::new(lines)
    }
}

/// The stable identity of a BOM row: the part copy and its variant.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BomRowKey {
    /// The part copy.
    pub part: Id<Part>,
    /// The variant key.
    pub variant: Option<Ident>,
}

/// One BOM row: one part (and variant), aggregated over every device that uses it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BomRow {
    /// The identity.
    pub key: BomRowKey,
    /// The 1-based position within its group (component rows, or configured-product rows).
    pub pos: u32,
    /// Whether the row is a configured product (printed "A1", "A2" … by the report layer).
    pub configured: bool,
    /// The summed quantity.
    pub qty: Dec3,
    /// The order unit.
    pub unit: OrderUnit,
    /// The manufacturer reference.
    pub manufacturer: LibRef,
    /// The part number (the order code for a configured product).
    pub part_number: String,
    /// The type designation.
    pub type_designation: Option<String>,
    /// The part description (never ratings, D-35).
    pub description: MlText,
    /// The devices with this part, in tag natural order.
    pub devices: Vec<Id<Device>>,
    /// The pages of those devices' main representations, in page order, without repeats.
    pub pages: Vec<PageRef>,
}

/// `bom_rows(mode)`: the bill of materials (doc 20 §14.1, §14.2).
///
/// 1. **Lines:** the `device_part_lines` of every device, in ID order.
/// 2. **Mode filter.** Component mode drops configured lines. Assembly mode: a device with at least one
///    configured line keeps only its configured lines, and every line of its descendants
///    (`Device.parent`, transitively, cycle-safe) is dropped; a descendant is dropped even if it is a head
///    itself, so the outermost head wins.
/// 3. **Aggregation** by [`BomRowKey`]: quantities add up (an overflowing sum saturates at the `Dec3`
///    limit); part data comes from the part copy; the devices are sorted by their short tag, full tag and
///    ID (natural order); the pages are those of the devices' main representations, in page order.
/// 4. **Order:** rows with a placed device first, by the least (page-order position, column, row) of their
///    devices' main representations (a missing cell counts as `u32::MAX`), then by part number; rows
///    without a placed device after them, by the legend rank of the part kind (enclosure, mounting plate,
///    rail, duct, other), part number and description. The key breaks every remaining tie.
/// 5. **Positions:** 1, 2, … over the component rows and, separately, over the configured rows.
#[derive(Debug)]
pub struct BomRows;

impl Query for BomRows {
    type Key = BomMode;
    type Value = Vec<BomRow>;
    const FAMILY: &'static str = "bom_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &BomMode) -> Arc<Vec<BomRow>> {
        let lines = filtered_lines(q, *key);
        let order = q.get::<PageOrder>(());
        let parts = q.parts();

        // Aggregate.
        let mut acc: BTreeMap<BomRowKey, (Dec3, BTreeSet<Id<Device>>)> = BTreeMap::new();
        for l in &lines {
            let k = BomRowKey {
                part: l.part,
                variant: l.variant.clone(),
            };
            let e = acc.entry(k).or_default();
            e.0 = Dec3(e.0.0.saturating_add(l.qty.0));
            e.1.insert(l.device);
        }

        let mut rows: Vec<(RowOrder, BomRow)> = Vec::new();
        for (k, (qty, devices)) in acc {
            let Some(copy) = parts.get(k.part) else {
                continue;
            };
            let part = &copy.item;
            let placements: Vec<Placement> =
                devices.iter().filter_map(|d| placement(q, &order, *d)).collect();
            let mut pages: Vec<PageRef> = placements.iter().map(|p| p.page).collect();
            pages.sort_by_key(|p| (order.position(*p).unwrap_or(usize::MAX), *p));
            pages.dedup();
            let mut devices: Vec<Id<Device>> = devices.into_iter().collect();
            devices.sort_by_cached_key(|d| tag_order(q, *d));
            let part_number = part.part_number.as_str().to_owned();
            let sort = match placements.iter().map(|p| p.order).min() {
                Some((pos, column, row)) => {
                    RowOrder::Placed(pos, column, row, natural_key(&part_number), k.clone())
                }
                None => RowOrder::Unplaced(
                    legend_rank(&part.kind),
                    natural_key(&part_number),
                    natural_key(&description_text(&part.description)),
                    k.clone(),
                ),
            };
            rows.push((
                sort,
                BomRow {
                    pos: 0,
                    configured: part.kind == PartKind::Configured,
                    qty,
                    unit: part.unit.clone(),
                    manufacturer: part.manufacturer.clone(),
                    part_number,
                    type_designation: part.type_designation.as_ref().map(|t| t.as_str().to_owned()),
                    description: part.description.clone(),
                    devices,
                    pages,
                    key: k,
                },
            ));
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));

        let (mut next_component, mut next_configured) = (1u32, 1u32);
        let rows = rows
            .into_iter()
            .map(|(_, mut row)| {
                let next = if row.configured {
                    &mut next_configured
                } else {
                    &mut next_component
                };
                row.pos = *next;
                *next += 1;
                row
            })
            .collect();
        Arc::new(rows)
    }
}

/// The sort key of a row (step 4): placed rows first, then unplaced ones.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum RowOrder {
    /// Least (page-order position, column, row) of the devices, part number, key.
    Placed(usize, u32, u32, NaturalKey, BomRowKey),
    /// Legend rank, part number, description, key.
    Unplaced(u8, NaturalKey, NaturalKey, BomRowKey),
}

/// Where a placed device's main representation is.
struct Placement {
    page: PageRef,
    /// (page-order position, column, row).
    order: (usize, u32, u32),
}

/// The main representation of a device: its drawn page and position; `None` for an unplaced device.
fn placement(q: &Q, order: &PageOrderV, device: Id<Device>) -> Option<Placement> {
    let rep_id = (*q.get::<MainRep>(device))?;
    let rep = q.reps().get(rep_id)?;
    let page = PageRef::Drawn(rep.page);
    let (column, row) = match q.get::<RepAnchorCell>(rep_id).as_ref() {
        Some(c) => (c.column, c.row),
        None => (u32::MAX, u32::MAX),
    };
    Some(Placement {
        page,
        order: (order.position(page).unwrap_or(usize::MAX), column, row),
    })
}

/// The sort key of a device inside a row: short tag, full tag (natural order), then the ID.
fn tag_order(q: &Q, device: Id<Device>) -> (NaturalKey, NaturalKey, Id<Device>) {
    let tag = q.get::<DeviceTag>(device);
    let (short, full) = match tag.as_ref() {
        Some(t) => (t.short(), t.full.clone()),
        None => (String::new(), String::new()),
    };
    (natural_key(&short), natural_key(&full), device)
}

/// The rank of a part kind among the rows without a placed device (layout legend, doc 12 §5.4).
fn legend_rank(kind: &PartKind) -> u8 {
    match kind {
        PartKind::Enclosure => 0,
        PartKind::MountingPlate => 1,
        PartKind::Rail => 2,
        PartKind::Duct => 3,
        _ => 4,
    }
}

/// The neutral text of a description, else the text of its first language, else empty.
fn description_text(text: &MlText) -> String {
    if let Some(n) = text.as_neutral() {
        return n.to_owned();
    }
    text.languages()
        .next()
        .and_then(|l| text.get(l))
        .unwrap_or_default()
        .to_owned()
}

/// Steps 1 and 2: the part lines of every device, filtered for the mode.
fn filtered_lines(q: &Q, mode: BomMode) -> Vec<PartLine> {
    let per_device: Vec<(Id<Device>, Arc<Vec<PartLine>>)> = q
        .devices()
        .ids()
        .map(|d| (d, q.get::<DevicePartLines>(d)))
        .collect();
    match mode {
        BomMode::Component => per_device
            .iter()
            .flat_map(|(_, lines)| lines.iter())
            .filter(|l| !l.configured)
            .cloned()
            .collect(),
        BomMode::Assembly => {
            let heads: BTreeSet<Id<Device>> = per_device
                .iter()
                .filter(|(_, lines)| lines.iter().any(|l| l.configured))
                .map(|(d, _)| *d)
                .collect();
            let mut covered: BTreeSet<Id<Device>> = BTreeSet::new();
            for head in &heads {
                descendants(q, *head, &mut covered);
            }
            per_device
                .iter()
                .filter(|(d, _)| !covered.contains(d))
                .flat_map(|(d, lines)| {
                    let head = heads.contains(d);
                    lines.iter().filter(move |l| !head || l.configured)
                })
                .cloned()
                .collect()
        }
    }
}

/// Adds every descendant of `head` (`Device.parent` children, transitively) to `out`; `head` itself is
/// not added unless a cycle leads back to it from another head. Cycle-safe.
fn descendants(q: &Q, head: Id<Device>, out: &mut BTreeSet<Id<Device>>) {
    let mut seen: BTreeSet<Id<Device>> = BTreeSet::from([head]);
    let mut stack = vec![head];
    while let Some(d) = stack.pop() {
        for c in q.idx().device_children(d) {
            if seen.insert(c) {
                out.insert(c);
                stack.push(c);
            }
        }
    }
}
