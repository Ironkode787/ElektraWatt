// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bill of materials: `device_part_lines(dev)` → `bom_rows(mode)` (ARCH §6.4, FR-REP-004 … FR-REP-006,
//! D-04, D-36).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText, NaturalKey, natural_key};
use ew_schema::{Device, DeviceState, LibRef, OrderUnit, Part, PartKind, PartRef, RepType, Representation};
use ew_units::Dec3;

use crate::Q;
use crate::cell::RepAnchorCell;
use crate::pages::{DrawnPageOrder, PageOrderV, PageRef};
use crate::query::{Policy, Query};
use crate::tag::{DeviceTag, MainFunction, MainRep};

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
///    devices' main representations (a terminal without one takes its strip's first segment; a missing
///    cell counts as `u32::MAX`), then by the head rank, then by
///    part number; rows without a placed device after them, by the legend rank of the part kind
///    (enclosure, mounting plate, rail, duct, other), part number and description. A row of a
///    layout-only accessory ([`PartKind::EndBracket`], [`PartKind::Accessory`]) is always in the second
///    group, whatever the placement of its devices: the accessories of a strip belong to the layout legend
///    (doc 12 §5.4, doc 20 §14.1 rows 26 and 27), not to the schematic position of the strip. Its pages
///    are still those of its devices. The key breaks every remaining tie. The *head rank* is 0 for every row that is not a configured product. Configured
///    products of one head device tie on the cell, and the head's own order of its configured parts breaks
///    that tie before the part number does (doc 20 §14.2: order code 1 is A1, order code 2 is A2): the head
///    of a configured row is the device of the row whose main representation gives the row's least
///    placement (the smallest device ID if several tie), and the rank is the index of the config of the
///    head's `assembly` aspect (`DeviceAspects.assembly`, its `configs`, doc 03 §7) whose `part` names the part
///    copy, else, after every named part, the index of the part in the head's `Device.parts`. A malformed
///    `assembly` member is ignored, so `Device.parts` order alone applies.
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
        // The drawn-page order, not `page_order`: BOM report pages feed `page_order`, and the two agree on
        // the relative order of drawn pages (WP-S3.03).
        let order = q.get::<DrawnPageOrder>(());
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
            let placements: Vec<(Id<Device>, Placement)> = devices
                .iter()
                .filter_map(|d| placement(q, &order, *d).map(|p| (*d, p)))
                .collect();
            let mut pages: Vec<PageRef> = placements.iter().map(|(_, p)| p.page).collect();
            pages.sort_by_key(|p| (order.position(*p).unwrap_or(usize::MAX), *p));
            pages.dedup();
            let mut devices: Vec<Id<Device>> = devices.into_iter().collect();
            devices.sort_by_cached_key(|d| tag_order(q, *d));
            let part_number = part.part_number.as_str().to_owned();
            let configured = part.kind == PartKind::Configured;
            let layout_only = matches!(part.kind, PartKind::EndBracket | PartKind::Accessory);
            let sort = match placements.iter().map(|(d, p)| (p.order, *d)).min() {
                Some(((pos, column, row), head)) if !layout_only => {
                    let rank = if configured { head_rank(q, head, k.part) } else { 0 };
                    RowOrder::Placed(pos, column, row, rank, natural_key(&part_number), k.clone())
                }
                _ => RowOrder::Unplaced(
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
                    configured,
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
    /// Least (page-order position, column, row) of the devices, head rank (0 unless configured), part
    /// number, key.
    Placed(usize, u32, u32, usize, NaturalKey, BomRowKey),
    /// Legend rank, part number, description, key.
    Unplaced(u8, NaturalKey, NaturalKey, BomRowKey),
}

/// Where a placed device's main representation is.
struct Placement {
    page: PageRef,
    /// (page-order position, column, row).
    order: (usize, u32, u32),
}

/// Where a device's row is placed: its main representation's drawn page and anchor cell; `None` for an
/// unplaced device.
///
/// A terminal is drawn through its strip (the strip's segments are the representations, the terminal
/// devices have none), so a device with a terminal aspect and no main representation of its own takes its
/// strip's **first** segment: the least (page-order position, column, row) over the representations of the
/// strip's main function, contact images excluded. Not the strip's `main_rep`, which is the smallest ID and
/// for a strip drawn in several segments need not be the first one (-X1031 is drawn at 103 A3 and A10): the
/// PT 2.5-TWIN rows sit on folio 103 with -X1031 before -X1032 (doc 20 §14.1, rows 13–14).
fn placement(q: &Q, order: &PageOrderV, device: Id<Device>) -> Option<Placement> {
    if let Some(rep) = *q.get::<MainRep>(device) {
        return rep_placement(q, order, rep);
    }
    let strip = q.devices().get(device)?.aspects.terminal.as_ref()?.strip;
    if strip == device {
        return None;
    }
    let f = (*q.get::<MainFunction>(strip))?;
    let reps = q.reps();
    q.idx()
        .reps_of_function(f)
        .filter(|r| reps.get(*r).is_some_and(|r| r.rep_type != RepType::ContactImage))
        .filter_map(|r| rep_placement(q, order, r))
        .min_by_key(|p| (p.order, p.page))
}

/// The drawn page and anchor cell of one representation.
fn rep_placement(q: &Q, order: &PageOrderV, rep_id: Id<Representation>) -> Option<Placement> {
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

/// The rank of the configured part `part` among the configured parts of `head` (step 4): the index of the
/// config of the head's `assembly` member whose `part` names the copy; a part no config names ranks after
/// every config, by its index in `Device.parts`; a part the head does not use ranks last.
fn head_rank(q: &Q, head: Id<Device>, part: Id<Part>) -> usize {
    let Some(dev) = q.devices().get(head) else {
        return usize::MAX;
    };
    let configs = assembly_config_parts(dev);
    if let Some(i) = configs.iter().position(|c| *c == Some(part)) {
        return i;
    }
    let by_use = dev
        .parts
        .iter()
        .position(|u| u.part.copy == part)
        .unwrap_or(usize::MAX - configs.len());
    configs.len() + by_use
}

/// The `part` copy of each entry of the untyped `assembly` aspect's `configs` (`DeviceAspects.assembly`), in
/// order; empty when the member is absent or not of the expected shape. An entry without a readable `part`
/// gives `None`.
fn assembly_config_parts(dev: &Device) -> Vec<Option<Id<Part>>> {
    let Some(configs) = dev
        .aspects
        .assembly
        .as_ref()
        .and_then(|a| a.get("configs"))
        .and_then(|c| c.as_array())
    else {
        return Vec::new();
    };
    configs
        .iter()
        .map(|c| {
            let part = c.get("part")?.clone();
            serde_json::from_value::<PartRef>(part).ok().map(|p| p.copy)
        })
        .collect()
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
