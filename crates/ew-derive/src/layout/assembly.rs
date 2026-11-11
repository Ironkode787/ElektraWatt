// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The member order of an assembly or PLC station (FR-LAY-061): one function shared by
//! `layout.place_assembly`, `layout.auto_fill` and EW-LAY-016, so that the layout and the check cannot disagree.

use std::cmp::Ordering;

use ew_ids::Id;
use ew_mltext::natural_cmp;
use ew_schema::Device;

use super::snap::{RailItem, min_gap};
use crate::Q;
use crate::tag::DeviceTag;

/// The text a device without a designation sorts by: the fallback EW-LAY-016 names such a device with, so that its
/// order is the rule's to the byte.
const NO_DESIGNATION: &str = "a device without designation";

/// `member_order(root)` (D6): `root` first, then its direct children (`Device.parent == root`) by `Device.position`
/// (none last), then their full designation in natural order, then their ID. Grandchildren are not members (a valve's
/// accessories belong to the valve, EW-LAY-016 groups by direct parent). Pure and cold: the order is read from the
/// device records and their tags each time. `root` is listed even when it is not in the project.
pub fn member_order(q: &Q, root: Id<Device>) -> Vec<Id<Device>> {
    let devices = q.devices();
    let mut children: Vec<(u32, String, Id<Device>)> = q
        .idx()
        .device_children(root)
        .filter(|c| *c != root)
        .filter_map(|c| {
            let d = devices.get(c)?;
            let name = q
                .try_get::<DeviceTag>(c)
                .ok()
                .and_then(|t| t.as_ref().as_ref().map(|t| t.full.clone()))
                .unwrap_or_else(|| NO_DESIGNATION.to_owned());
            Some((d.position.unwrap_or(u32::MAX), name, c))
        })
        .collect();
    children.sort_by(member_cmp);
    children.dedup_by_key(|c| c.2);
    std::iter::once(root)
        .chain(children.into_iter().map(|(_, _, c)| c))
        .collect()
}

/// The order of two children: position, then natural designation, then ID.
fn member_cmp(a: &(u32, String, Id<Device>), b: &(u32, String, Id<Device>)) -> Ordering {
    a.0.cmp(&b.0)
        .then_with(|| natural_cmp(&a.1, &b.1))
        .then_with(|| a.2.cmp(&b.2))
}

/// The left edge an assembly member is pushed in at: one function shared by
/// `layout.place_assembly`'s insertion and the layout tool's default slot, never a second copy.
///
/// With a predecessor `pred` (the nearest earlier member on the rail, its rightmost item) the member's doubled centre is
/// the smallest value above the predecessor's, so that push mode starts it at the predecessor's end plus the minimum
/// gap: `⌊(pred.s0 + pred.s1 − width) / 2⌋ + 1`. Without one, before its first placed successor `succ`: `succ.s0 −
/// min_gap(item, succ) − width`. Neither: 0. The result is a left edge to resolve in push mode, not a final place.
pub fn member_slot(pred: Option<&RailItem>, succ: Option<&RailItem>, item: &RailItem) -> i128 {
    let width = item.width();
    match (pred, succ) {
        (Some(p), _) => {
            p.s0.saturating_add(p.s1)
                .saturating_sub(width)
                .div_euclid(2)
                .saturating_add(1)
        }
        (None, Some(s)) => s.s0.saturating_sub(min_gap(item, s)).saturating_sub(width),
        (None, None) => 0,
    }
}
