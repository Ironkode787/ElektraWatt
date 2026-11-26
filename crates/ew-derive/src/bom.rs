// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bill of materials: `device_part_lines(dev)` → `bom_rows(mode)` (ARCH §6.4, FR-REP-004 … FR-REP-006,
//! D-04, D-36).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText, NaturalKey, natural_key};
use ew_schema::{
    BomBasis, Carrier, Device, DeviceState, Enclosure, LibRef, MountingSurface, OrderUnit, Page, PageType,
    Part, PartKind, PartRef, RepType, Representation,
};
use ew_units::Dec3;

use crate::Q;
use crate::accessories::{BracketFrom, PartAccessories, placed_brackets, strip_bracket};
use crate::cell::RepAnchorCell;
use crate::layout::{carrier_length, carriers_in_order, metres_of_nm, page_surface};
use crate::pages::{DrawnPageOrder, PageOrderV, PageRef};
use crate::query::{Policy, Query};
use crate::strip_bridges::StripBridges;
use crate::tag::{DeviceTag, MainFunction, MainRep};

/// The two BOM modes (FR-REP-005).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum BomMode {
    /// Every component part; configured products are left out.
    Component,
    /// Configured products (the order codes of the assembly aspects) as one row each, replacing the part
    /// lines of the devices they cover.
    Assembly,
}

/// Where a part line comes from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LineSource {
    /// A part use of the device (`Device.parts`).
    Device,
    /// A strip accessory (`Strip.accessories[].part`), one piece each.
    StripAccessory,
    /// A bridge group of the strip with a part (the carrier's `overrides.wire_part`), one piece
    /// each.
    Bridge,
    /// An order code of the device's assembly aspect (`Assembly.configs[].part`), one set each; assembly
    /// mode only.
    AssemblyConfig,
    /// The carriers (rails, ducts) with one part copy on the surfaces of an enclosure whose device owns the line,
    /// by their summed path length in metres (FR-LAY-004, FR-LAY-005, FR-LAY-008).
    Carrier,
    /// The end brackets of a placed strip that holds its end bracket as a part use (a project made before
    /// accessories, FR-LAY-060): the line of that part use, with the number of brackets the rails attribute to it
    /// ([`crate::placed_brackets`]) instead of the authored quantity.
    GroupAccessory,
    /// An accessory a part of the device brings with it ([`crate::PartAccessories`], doc 07 §2.5); `rail_group`: a
    /// strip's end brackets, which print the panel pages of their rails, as [`LineSource::GroupAccessory`] does.
    Accessory {
        /// The strip's end brackets.
        rail_group: bool,
    },
    /// The plug or silencer part of a closed fluid port of the device (`CpOverride.closure.part`), one piece each
    /// (FR-FLU-028).
    Closure,
    /// The tube part of a physical tube the device owns (the device of its from end, else of its to end), by its length in metres (FR-FLU-014; a
    /// tube without a length adds none).
    Tube,
    /// The fitting part at a tube end on a port of the device, one piece each, and the tee part of a fluid junction,
    /// owned by the owner of the junction's lowest tube that has one (FR-FLU-031).
    Fitting,
    /// The plug at a bus line's end on a port of the device (`BusLine.plug_a`, `plug_b`; doc 10 §2.7), one piece each.
    Plug,
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

impl PartLine {
    /// Whether the line is a part use of the device itself ([`LineSource::Device`]): the device's own part, which a
    /// list that names one part per device prints (its first such line). A strip's end plates, end brackets, own
    /// accessories and bridges, an enclosure's carriers, a device's tubes, fittings, closures and plugs are not.
    pub fn is_own(&self) -> bool {
        matches!(self.source, LineSource::Device)
    }

    /// `f` of the line's part as its variant is listed (`Part::with_variant`): every list prints a variant's numbers
    /// and description where it gives them, else the part's; `None` when the copy is missing.
    pub fn with_part<R>(&self, q: &Q, f: impl FnOnce(&Part) -> R) -> Option<R> {
        let mut part = PartRef::new(self.part);
        part.variant = self.variant.clone();
        Some(f(&crate::printed_part(q, &part)?.item))
    }

    /// The part number the line prints (its variant's, else its part's); `None` when the copy is missing.
    pub fn part_number(&self, q: &Q) -> Option<String> {
        self.with_part(q, |p| p.part_number.as_str().to_owned())
    }
}

/// `device_part_lines(dev)`: the part lines of one device, in model order.
///
/// A missing device and a device that is not fitted have no lines. Otherwise one line per part use of the
/// device (`Device.parts` order; a cable with a length entered by hand counts its first part use, when that part is
/// sold by the metre, by that length in metres exact to the millimetre; for a strip placed on a rail, the end-bracket
/// part use of a project made before accessories ([`crate::strip_bracket`]) gives a
/// [`LineSource::GroupAccessory`] line whose quantity is the number of brackets attributed to the strip; a
/// strip without a bracket on any rail keeps the authored quantity), then, for a strip device, one line of
/// quantity 1 per strip accessory (mounting order), then one line of quantity 1 per bridge group of the
/// strip that has a part ([`StripBridges`], group order), then, for the device of an enclosure, one
/// **carrier line** per part ([`LineSource::Carrier`]): the carriers with a part on the enclosure's mounting
/// surfaces that share a part copy (and variant) give one line, whose quantity is the sum of their path
/// lengths in metres exact to the millimetre (summed in nanometres, rounded once, half up). A carrier
/// without a derivable length (an invalid path, a kind of a newer format; only loaded data has one,
/// EW-LAY-010 reports it) adds nothing, and a line all of whose carriers lack a length stays with quantity
/// 0. The carrier lines come in the legend order of their first carrier (surfaces by ID, then rails before
/// ducts, each top to bottom then left to right; [`crate::layout::carrier_order`]), after the device's own
/// lines, so a reader of the first line keeps the device's own part. Then the fluid lines (doc 11 §8.2, FR-FLU-014):
/// the closure parts of its ports ([`LineSource::Closure`]), the tube parts of the tubes it owns (the device of the
/// tube's from end, else of its to end; [`LineSource::Tube`]) and the fittings at its tube ends and the tees it owns ([`LineSource::Fitting`]), in that
/// order. Then the plugs at the ends of the bus lines on its ports ([`LineSource::Plug`], by function, CP and line);
/// a plug at an end without a device (an interruption point, a port of no device yet) is its line's cable's, else the
/// device's at the other end, so that every plug is in the lists once. Last the accessories its parts bring with them
/// ([`LineSource::Accessory`], `part_accessories`: a part's, its carriers', a strip's end plates and end brackets). A
/// line whose part copy is missing is
/// dropped.
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
        // An end bracket held as a part use counts the brackets the rails attribute to the strip on its line.
        let brackets = strip_bracket(q, dev).and_then(|b| match b.from {
            BracketFrom::PartUse(i) => placed_brackets(q, *key).map(|(n, _)| (i, n)),
            _ => None,
        });
        // A cable with a length entered counts its part sold by the metre by that length (a bus cable of 12.5 m
        // was one metre in the parts list).
        let cable_metres = match dev.aspects.cable.as_ref().map(|c| &c.length) {
            Some(ew_schema::LengthSource::Manual(n)) => Some(metres_of_nm(i128::from(n.0))),
            _ => None,
        };
        for (i, u) in dev.parts.iter().enumerate() {
            let by_metre = parts
                .get(u.part.copy)
                .is_some_and(|c| c.item.unit == OrderUnit::Metre);
            match (&brackets, cable_metres) {
                (Some((at, count)), _) if *at == i => {
                    let n = i64::try_from(*count).unwrap_or(i64::MAX);
                    let qty = Dec3::from_int(n).unwrap_or(Dec3(i64::MAX));
                    push(&u.part, qty, LineSource::GroupAccessory);
                }
                (_, Some(metres)) if i == 0 && by_metre => push(&u.part, metres, LineSource::Device),
                _ => push(&u.part, u.qty, LineSource::Device),
            }
        }
        if let Some(strip) = &dev.aspects.strip {
            for a in &strip.accessories {
                push(&a.part, Dec3::ONE, LineSource::StripAccessory);
            }
            for g in q.get::<StripBridges>(*key).iter() {
                if let Some(part) = &g.part {
                    push(part, Dec3::ONE, LineSource::Bridge);
                }
            }
        }
        for g in carrier_groups(q, *key) {
            push(&g.part, metres_of_nm(g.nm), LineSource::Carrier);
        }
        let mut functions: Vec<Id<ew_schema::Function>> = q.idx().functions_of_device(*key).collect();
        functions.sort();
        for f in functions.clone() {
            if let Some(func) = q.functions().get(f) {
                for o in &func.cp_overrides {
                    if let Some(part) = o.closure.as_ref().and_then(|c| c.part.as_ref()) {
                        push(part, Dec3::ONE, LineSource::Closure);
                    }
                }
            }
        }
        let tubes = q.get::<crate::TubeRows>(());
        for r in tubes.iter() {
            if tube_owner(r) == Some(*key)
                && let Some(part) = &r.props.part
            {
                let nm = r.length.map_or(0, |n| n.0);
                push(
                    &PartRef::new(part.value),
                    metres_of_nm(i128::from(nm)),
                    LineSource::Tube,
                );
            }
        }
        for r in tubes.iter() {
            for end in [&r.from, &r.to] {
                if end.device == Some(*key)
                    && let Some(f) = end.fitting
                {
                    push(&PartRef::new(f), Dec3::ONE, LineSource::Fitting);
                }
            }
            // A tee: owned by the owner of the lowest tube at the junction that has one.
            if tube_owner(r) == Some(*key) {
                for end in [&r.from, &r.to] {
                    let ew_model::EndpointKey::Junction(j) = end.at else {
                        continue;
                    };
                    let lowest = tubes
                        .iter()
                        .filter(|t| (t.from.at == end.at || t.to.at == end.at) && tube_owner(t).is_some())
                        .map(|t| t.id)
                        .min();
                    if lowest == Some(r.id)
                        && let Some(tee) = q.junctions().get(j).and_then(|x| x.fitting.clone())
                    {
                        push(&tee, Dec3::ONE, LineSource::Fitting);
                    }
                }
            }
        }
        for f in functions {
            for (idx, mut conns) in q.idx().connected_cps_of_function(f) {
                conns.sort();
                for c in conns {
                    let Some(conn) = q.connections().get(c) else {
                        continue;
                    };
                    let ew_schema::ConnectionKind::Bus(line) = &conn.kind else {
                        continue;
                    };
                    for (end, plug) in [(&conn.a, &line.plug_a), (&conn.b, &line.plug_b)] {
                        let here = matches!(end, ew_schema::Endpoint::Cp(ef, ei) if (*ef, *ei) == (f, idx));
                        // An end without a device of its own gives its plug to the device at the other end.
                        if let Some(p) = plug
                            && (here || end_device(q, end).is_none())
                            && plug_owner(q, conn, end) == Some(*key)
                        {
                            push(&p.part, Dec3::ONE, LineSource::Plug);
                        }
                    }
                }
            }
        }
        // A cable carries the plugs of its bus lines' ends that sit on no device (an interruption point, a port of no
        // device yet).
        if dev.aspects.cable.is_some() {
            let mut carried: Vec<_> = q
                .connections()
                .iter()
                .filter(
                    |(_, c)| matches!(&c.kind, ew_schema::ConnectionKind::Bus(l) if l.cable == Some(*key)),
                )
                .map(|(_, c)| Arc::clone(c))
                .collect();
            carried.sort_by_key(|c| c.id);
            for conn in carried {
                let ew_schema::ConnectionKind::Bus(line) = &conn.kind else {
                    continue;
                };
                for (end, plug) in [(&conn.a, &line.plug_a), (&conn.b, &line.plug_b)] {
                    if let Some(p) = plug
                        && end_device(q, end).is_none()
                    {
                        push(&p.part, Dec3::ONE, LineSource::Plug);
                    }
                }
            }
        }
        for a in q.get::<PartAccessories>(*key).iter() {
            let rail_group = a.per == ew_schema::AccessoryPer::RailGroup;
            push(&a.part, a.qty, LineSource::Accessory { rail_group });
        }
        Arc::new(lines)
    }
}

/// The device of a connection's end: the device of the function whose CP it is; `None` for a function without one
/// (an interruption point, a port of no device yet) and for a junction.
fn end_device(q: &Q, end: &ew_schema::Endpoint) -> Option<Id<Device>> {
    q.functions().get(end.function()?)?.device
}

/// The device whose part lines count the plug at end `end` of the bus line `conn` (a plug at an end without a
/// device was in no list): the device at that end, else the line's cable, else the device at the other end.
fn plug_owner(q: &Q, conn: &ew_schema::Connection, end: &ew_schema::Endpoint) -> Option<Id<Device>> {
    let ew_schema::ConnectionKind::Bus(line) = &conn.kind else {
        return None;
    };
    let other = if std::ptr::eq(end, &conn.a) {
        &conn.b
    } else {
        &conn.a
    };
    end_device(q, end).or(line.cable).or_else(|| end_device(q, other))
}

/// The device that owns a tube's BOM lines: the device of its from end, else of its to end (a tube from a tee), so a
/// tube is in the BOM whichever way it was drawn; `None` for a tube between two tees.
fn tube_owner(r: &crate::TubeRow) -> Option<Id<Device>> {
    r.from.device.or(r.to.device)
}

/// Whether a line counts a strip's end brackets (its own end-bracket part use or the brackets its terminals' parts
/// declare): it prints the panel pages of the rails holding them and none of the strip's representation pages.
fn is_bracket_line(source: LineSource) -> bool {
    matches!(
        source,
        LineSource::GroupAccessory | LineSource::Accessory { rail_group: true }
    )
}

/// The carriers with a part on the surfaces of the enclosures whose device is `device`, surface by surface in legend
/// order: what the device's carrier lines and the accessories of its carriers count.
pub(crate) fn enclosure_carriers(q: &Q, device: Id<Device>) -> Vec<Id<Carrier>> {
    let carriers = q.table::<Carrier>();
    enclosure_surfaces(q, device)
        .into_iter()
        .flat_map(|s| carriers_in_order(q, s))
        .filter(|c| carriers.get(*c).is_some_and(|rec| rec.part.is_some()))
        .collect()
}

/// The carriers of one part copy (and variant) on the surfaces of a device's enclosures: one carrier line.
struct CarrierGroup {
    /// The part (the first carrier's reference).
    part: PartRef,
    /// The summed path length in nanometres (a carrier without a length adds nothing).
    nm: i128,
    /// The surfaces those carriers are on.
    surfaces: BTreeSet<Id<MountingSurface>>,
}

/// The mounting surfaces of the enclosures whose device is `device`, in ID order.
fn enclosure_surfaces(q: &Q, device: Id<Device>) -> Vec<Id<MountingSurface>> {
    let enclosures: BTreeSet<Id<Enclosure>> = q
        .table::<Enclosure>()
        .iter()
        .filter(|(_, e)| e.device == device)
        .map(|(id, _)| id)
        .collect();
    if enclosures.is_empty() {
        return Vec::new();
    }
    q.table::<MountingSurface>()
        .iter()
        .filter(|(_, s)| enclosures.contains(&s.enclosure))
        .map(|(id, _)| id)
        .collect()
}

/// The carrier lines of `device` (see [`DevicePartLines`]): the carriers with a part on its enclosures' surfaces,
/// grouped by part copy and variant, in the legend order of each group's first carrier.
fn carrier_groups(q: &Q, device: Id<Device>) -> Vec<CarrierGroup> {
    let carriers = q.table::<Carrier>();
    let mut groups: Vec<CarrierGroup> = Vec::new();
    for surface in enclosure_surfaces(q, device) {
        for c in carriers_in_order(q, surface) {
            let Some(part) = carriers.get(c).and_then(|rec| rec.part.clone()) else {
                continue;
            };
            let nm = carrier_length(q, c).unwrap_or(0);
            match groups
                .iter_mut()
                .find(|g| g.part.copy == part.copy && g.part.variant == part.variant)
            {
                Some(g) => {
                    g.nm = g.nm.saturating_add(nm);
                    g.surfaces.insert(surface);
                }
                None => groups.push(CarrierGroup {
                    part,
                    nm,
                    surfaces: BTreeSet::from([surface]),
                }),
            }
        }
    }
    groups
}

/// The surfaces a part line of a layout object shows: a carrier line's carriers' surfaces; an
/// end-bracket line, the surfaces of the rails holding its brackets; any other line of the device of an
/// enclosure, its enclosures' surfaces; a line of the device of a surface (the plate), that surface. Empty for every
/// other line.
fn line_surfaces(q: &Q, line: &PartLine) -> BTreeSet<Id<MountingSurface>> {
    if is_bracket_line(line.source) {
        return placed_brackets(q, line.device)
            .map(|(_, surfaces)| surfaces)
            .unwrap_or_default();
    }
    if line.source == LineSource::Carrier {
        return carrier_groups(q, line.device)
            .into_iter()
            .find(|g| g.part.copy == line.part && g.part.variant == line.variant)
            .map(|g| g.surfaces)
            .unwrap_or_default();
    }
    let mut out: BTreeSet<Id<MountingSurface>> = enclosure_surfaces(q, line.device).into_iter().collect();
    out.extend(
        q.table::<MountingSurface>()
            .iter()
            .filter(|(_, s)| s.device == Some(line.device))
            .map(|(id, _)| id),
    );
    out
}

/// The panel-layout pages showing each surface ([`page_surface`]), as drawn pages.
fn surface_views(q: &Q) -> BTreeMap<Id<MountingSurface>, Vec<PageRef>> {
    let panel: Vec<Id<Page>> = q
        .pages()
        .iter()
        .filter(|(_, p)| p.kind == PageType::PanelLayout)
        .map(|(id, _)| id)
        .collect();
    let mut out: BTreeMap<Id<MountingSurface>, Vec<PageRef>> = BTreeMap::new();
    for page in panel {
        if let Some(s) = page_surface(q, page) {
            out.entry(s).or_default().push(PageRef::Drawn(page));
        }
    }
    out
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
    /// The devices whose part lines a configured product replaces (`AssemblyConfig.covers`, the union over
    /// the configs of the row), in tag natural order; empty for every other row.
    pub covers: Vec<Id<Device>>,
    /// The type designation.
    pub type_designation: Option<String>,
    /// The part description (never ratings, D-35).
    pub description: MlText,
    /// The devices with this part, in tag natural order.
    pub devices: Vec<Id<Device>>,
    /// The pages of the main representations of the devices with a line other than an end-bracket line and, for a part
    /// line of a layout object (the device of an enclosure or of a surface, a carrier line, an end-bracket line), the
    /// panel-layout pages whose surface view shows its surface, in page order,
    /// without repeats.
    pub pages: Vec<PageRef>,
}

/// `bom_rows(mode)`: the bill of materials.
///
/// 1. **Lines:** the `device_part_lines` of every device, in ID order.
/// 2. **Mode filter.** Component mode drops configured lines (a part use of a part of kind `configured`).
///    Assembly mode (D-36): every device with an `assembly` aspect whose `bom_basis` is
///    `assembly` (the *head*) contributes one line of quantity 1 per config whose `part` copy exists
///    ([`LineSource::AssemblyConfig`], configured), and the devices that config `covers` lose all their
///    own lines (part uses and strip accessories). Covers are explicit, not transitive: a child the configs
///    do not name keeps its lines. A config without a part, or whose copy is missing, prints nothing and
///    covers nothing, so no part disappears silently; a `components` basis keeps the components. A cover
///    outside the head's subtree (neither the head nor a descendant by `Device.parent`, cycle-safe) is
///    ignored, so a config never drops an unrelated device's parts (reported by a check, not here). A head
///    that a config of another head covers prints no config rows and covers nothing: the outer assembly
///    replaces it. Other lines, configured part uses included, stay.
/// 3. **Aggregation** by [`BomRowKey`] and the configured flag: quantities add up (an overflowing sum
///    saturates at the `Dec3` limit); part data comes from the part copy; the devices are sorted by their
///    short tag, full tag and ID (natural order); the pages are those of the main representations of the devices
///    with a line other than an end-bracket line ([`LineSource::GroupAccessory`]) together with, for a line of a
///    layout object (the device of an enclosure or of a mounting surface, a carrier line, an end-bracket line), the
///    panel-layout pages whose surface view shows its surface, and for a plug at a bus line's end of no device, that
///    line's page (-UC1's rows print folio 902;
///    An end-bracket line prints the panel pages of its rails and none of its strip's representation
///    pages, so row 26 prints 902), in page order. That is the Folios cell only: the order below
///    reads the representations alone. The covers of a configured row are the union of its configs' covers.
/// 4. **Order:** rows with a placed device first, by the least (page-order position, column, row) of their
///    devices' main representations (a terminal without one takes its strip's first segment; a missing
///    cell counts as `u32::MAX`), then by the head rank, then by
///    part number; rows without a placed device after them, by the legend rank of the part kind
///    (enclosure, mounting plate, rail, duct, other), part number and description. A row of a
///    layout-only accessory ([`PartKind::EndBracket`], [`PartKind::Accessory`]) is always in the second
///    group, whatever the placement of its devices: the accessories of a strip belong to the layout legend
///    (doc 12 §5.4), not to the schematic position of the strip. Its pages
///    are those of step 3. The key breaks every remaining tie. The *head rank* is 0 for every row that is not a configured product. Configured
///    products of one head device tie on the cell, and the head's own order of its configured parts breaks
///    that tie before the part number does (order code 1 is A1, order code 2 is A2): the head
///    of a configured row is the device of the row whose main representation gives the row's least
///    placement (the smallest device ID if several tie), and the rank is the index of the config of the
///    head's `assembly` aspect (`DeviceAspects.assembly`, its `configs`, doc 03 §7) whose `part` names the part
///    copy, else, after every named part, the index of the part in the head's `Device.parts`.
/// 5. **Positions:** 1, 2, … over the component rows and, separately, over the configured rows, so in
///    assembly mode the component rows after the replaced ones renumber.
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
        // the relative order of drawn pages.
        let order = q.get::<DrawnPageOrder>(());
        let parts = q.parts();
        let views = surface_views(q);

        // Aggregate.
        let mut acc: BTreeMap<(bool, BomRowKey), Acc> = BTreeMap::new();
        for l in &lines {
            let k = BomRowKey {
                part: l.line.part,
                variant: l.line.variant.clone(),
            };
            let e = acc.entry((l.line.configured, k)).or_default();
            e.qty = Dec3(e.qty.0.saturating_add(l.line.qty.0));
            e.devices.insert(l.line.device);
            if !is_bracket_line(l.line.source) {
                e.drawn.insert(l.line.device);
            }
            e.covers.extend(l.covers.iter().copied());
            if !views.is_empty() {
                e.surfaces.extend(line_surfaces(q, &l.line));
            }
            if l.line.source == LineSource::Plug {
                e.line_pages.extend(plug_pages(q, l.line.device, l.line.part));
            }
        }

        let mut rows: Vec<(RowOrder, BomRow)> = Vec::new();
        for (
            (configured, k),
            Acc {
                qty,
                devices,
                drawn,
                covers,
                surfaces,
                line_pages,
            },
        ) in acc
        {
            let Some(copy) = parts.get(k.part) else {
                continue;
            };
            let part = copy.item.with_variant(k.variant.as_ref().map(Ident::as_str));
            let placements: Vec<(Id<Device>, Placement)> = devices
                .iter()
                .filter_map(|d| placement(q, &order, *d).map(|p| (*d, p)))
                .collect();
            let mut pages: Vec<PageRef> = placements
                .iter()
                .filter(|(d, _)| drawn.contains(d))
                .map(|(_, p)| p.page)
                .collect();
            pages.extend(surfaces.iter().filter_map(|s| views.get(s)).flatten().copied());
            pages.extend(line_pages.into_iter().map(PageRef::Drawn));
            pages.sort_by_key(|p| (order.position(*p).unwrap_or(usize::MAX), *p));
            pages.dedup();
            let mut devices: Vec<Id<Device>> = devices.into_iter().collect();
            devices.sort_by_cached_key(|d| tag_order(q, *d));
            let mut covers: Vec<Id<Device>> = covers.into_iter().collect();
            covers.sort_by_cached_key(|d| tag_order(q, *d));
            let part_number = part.part_number.as_str().to_owned();
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
                    covers,
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

/// The aggregate of one row (step 3).
#[derive(Default)]
struct Acc {
    qty: Dec3,
    devices: BTreeSet<Id<Device>>,
    /// The devices with a line other than an end-bracket line: their representations' pages print in the Folios cell.
    drawn: BTreeSet<Id<Device>>,
    covers: BTreeSet<Id<Device>>,
    /// The surfaces the row's layout-object lines show (their panel pages print in the Folios cell).
    surfaces: BTreeSet<Id<MountingSurface>>,
    /// The pages of the bus lines whose plugs at an end of no device the row counts: their place.
    line_pages: BTreeSet<Id<Page>>,
}

/// The pages of the bus lines with a plug of part `part` at an end of no device that `device` counts
/// ([`plug_owner`]): the place of such a plug in the Folios cell.
fn plug_pages(q: &Q, device: Id<Device>, part: Id<Part>) -> Vec<Id<Page>> {
    q.connections()
        .iter()
        .filter_map(|(_, c)| {
            let ew_schema::ConnectionKind::Bus(line) = &c.kind else {
                return None;
            };
            let here = [(&c.a, &line.plug_a), (&c.b, &line.plug_b)]
                .into_iter()
                .any(|(end, plug)| {
                    plug.as_ref().is_some_and(|p| p.part.copy == part)
                        && end_device(q, end).is_none()
                        && plug_owner(q, c, end) == Some(device)
                });
            here.then_some(c.page).flatten()
        })
        .collect()
}

/// A part line after the mode filter, with the devices it covers (a config line) or none.
struct Line {
    line: PartLine,
    covers: Vec<Id<Device>>,
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
/// PT 2.5-TWIN rows sit on folio 103 with -X1031 before -X1032.
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
    let configs: Vec<Option<Id<Part>>> = dev
        .aspects
        .assembly
        .as_ref()
        .map(|a| {
            a.configs
                .iter()
                .map(|c| c.part.as_ref().map(|p| p.copy))
                .collect()
        })
        .unwrap_or_default();
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
fn filtered_lines(q: &Q, mode: BomMode) -> Vec<Line> {
    let per_device: Vec<(Id<Device>, Arc<Vec<PartLine>>)> = q
        .devices()
        .ids()
        .map(|d| (d, q.get::<DevicePartLines>(d)))
        .collect();
    let plain = |line: &PartLine| Line {
        line: line.clone(),
        covers: Vec::new(),
    };
    match mode {
        BomMode::Component => per_device
            .iter()
            .flat_map(|(_, lines)| lines.iter())
            .filter(|l| !l.configured)
            .map(plain)
            .collect(),
        BomMode::Assembly => {
            let (configs, covered) = assembly_lines(q);
            let mut out: Vec<Line> = per_device
                .iter()
                .filter(|(d, _)| !covered.contains(d))
                .flat_map(|(_, lines)| lines.iter())
                .map(plain)
                .collect();
            out.extend(configs);
            out
        }
    }
}

/// A printable config of a head: its configured part and its covers inside the head's subtree.
type HeadConfig = (PartRef, Vec<Id<Device>>);

/// The config lines of every assembly head (step 2) and the devices they cover.
fn assembly_lines(q: &Q) -> (Vec<Line>, BTreeSet<Id<Device>>) {
    let parts = q.parts();
    // Every head's printable configs with the covers inside its subtree.
    let mut heads: Vec<(Id<Device>, Vec<HeadConfig>)> = Vec::new();
    for (head, dev) in q.devices().iter() {
        let Some(assembly) = &dev.aspects.assembly else {
            continue;
        };
        if assembly.bom_basis != BomBasis::Assembly || dev.state == DeviceState::NotFitted {
            continue;
        }
        let configs: Vec<HeadConfig> = assembly
            .configs
            .iter()
            .filter_map(|config| {
                let part = config.part.as_ref().filter(|p| parts.contains(p.copy))?;
                let covers = config
                    .covers
                    .iter()
                    .copied()
                    .filter(|d| in_subtree(q, head, *d))
                    .collect();
                Some((part.clone(), covers))
            })
            .collect();
        heads.push((head, configs));
    }
    // A head that another head's config covers is replaced by the outer assembly.
    let covered_by_other = |h: Id<Device>| {
        heads
            .iter()
            .any(|(other, configs)| *other != h && configs.iter().any(|(_, c)| c.contains(&h)))
    };
    let mut lines = Vec::new();
    let mut covered = BTreeSet::new();
    for (head, configs) in &heads {
        if covered_by_other(*head) {
            continue;
        }
        for (part, covers) in configs {
            covered.extend(covers.iter().copied());
            lines.push(Line {
                line: PartLine {
                    device: *head,
                    part: part.copy,
                    variant: part.variant.clone(),
                    qty: Dec3::ONE,
                    source: LineSource::AssemblyConfig,
                    configured: true,
                },
                covers: covers.clone(),
            });
        }
    }
    (lines, covered)
}

/// Whether `device` is `head` or one of its descendants (`Device.parent`, followed upwards, cycle-safe).
fn in_subtree(q: &Q, head: Id<Device>, device: Id<Device>) -> bool {
    let devices = q.devices();
    let mut seen = BTreeSet::new();
    let mut at = Some(device);
    while let Some(d) = at {
        if d == head {
            return true;
        }
        if !seen.insert(d) {
            return false;
        }
        at = devices.get(d).and_then(|dev| dev.parent);
    }
    false
}
