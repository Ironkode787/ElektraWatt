// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bill of materials: `device_part_lines(dev)` → `bom_rows(mode)` (ARCH §6.4, FR-REP-004 … FR-REP-006,
//! D-04, D-36).

use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{Device, LibRef, OrderUnit, Part};
use ew_units::Dec3;

use crate::Q;
use crate::pages::PageRef;
use crate::query::{Policy, Query};

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
#[derive(Debug)]
pub struct DevicePartLines;

impl Query for DevicePartLines {
    type Key = Id<Device>;
    type Value = Vec<PartLine>;
    const FAMILY: &'static str = "device_part_lines";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<PartLine>> {
        let _ = (q, key);
        Arc::new(Vec::new())
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

/// `bom_rows(mode)`.
#[derive(Debug)]
pub struct BomRows;

impl Query for BomRows {
    type Key = BomMode;
    type Value = Vec<BomRow>;
    const FAMILY: &'static str = "bom_rows";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &BomMode) -> Arc<Vec<BomRow>> {
        let _ = (q, key);
        Arc::new(Vec::new())
    }
}
