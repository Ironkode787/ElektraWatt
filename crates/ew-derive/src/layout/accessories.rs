// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The items of a rail and its derived end brackets (WP-S4.02 D3, FR-LAY-060, doc 12 §3.4): [`rail_items`] turns
//! `rail_occupancy` into the rail model's items (the strips with their declared bracket widths), and the memo query
//! [`GroupAccessories`] (`group_accessories(carrier)`) lists the end brackets the strips need. Nothing is stored: a
//! bracket is derived from the strips' positions each time, so moving a strip away from its run adds two.
//! [`strip_brackets`] counts the brackets attributed to one strip over its rails: the BOM's line of the strip's
//! declaring part use reads it (part 5, D13).

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Carrier, CarrierRef, Device, LibCopy, MountingSurface, Part, PartKind, PartRef, Placement};

use super::queries::{RailOccupancy, places_strip};
use super::snap::{RailItem, brackets};
use crate::Q;
use crate::query::{Policy, Query};

/// The end-bracket part a strip declares (D3): its first part use whose part copy has kind `end_bracket`, with that
/// part's `mech.width` in nanometres (absent or negative: 0). `None` when no part use is an end bracket.
pub fn bracket_part(q: &Q, device: &Device) -> Option<(PartRef, i128)> {
    let parts = q.table::<LibCopy<Part>>();
    device.parts.iter().find_map(|u| {
        let copy = parts.get(u.part.copy)?;
        (copy.item.kind == PartKind::EndBracket).then(|| {
            let w = copy
                .item
                .mech
                .as_ref()
                .and_then(|m| m.width)
                .map_or(0, |w| i128::from(w.0).max(0));
            (u.part.clone(), w)
        })
    })
}

/// The index in `Device.parts` of the part use a strip declares its end brackets by (D3): the first whose part copy
/// has kind `end_bracket`, as [`bracket_part`] finds it.
pub fn bracket_use(q: &Q, device: &Device) -> Option<usize> {
    let parts = q.table::<LibCopy<Part>>();
    device.parts.iter().position(|u| {
        parts
            .get(u.part.copy)
            .is_some_and(|c| c.item.kind == PartKind::EndBracket)
    })
}

/// The end brackets attributed to one placed strip (WP-S4.02 D13, part 5).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StripBrackets {
    /// The index in `Device.parts` of the declaring part use ([`bracket_use`]).
    pub part_use: usize,
    /// How many brackets [`GroupAccessories`] attributes to the strip, over every rail it is placed on.
    pub count: usize,
    /// The mounting surfaces of the rails holding them.
    pub surfaces: BTreeSet<Id<MountingSurface>>,
}

/// The end brackets attributed to `device` over the rails its placements are on (D13): the BOM's line of the declaring
/// part use takes its quantity and its panel folios from them. `None` for a missing device, a device that declares no
/// bracket part and a strip none of whose placements holds a bracket (not placed, or placed on a surface or a duct):
/// such a strip keeps its authored quantity and folios (doc 12 §3.4).
pub fn strip_brackets(q: &Q, device: Id<Device>) -> Option<StripBrackets> {
    let dev = q.devices().get(device)?;
    let part_use = bracket_use(q, dev)?;
    let placements = q.table::<Placement>();
    let carriers: BTreeSet<Id<Carrier>> = q
        .idx()
        .placements_of_device(device)
        .filter_map(|p| match placements.get(p)?.carrier {
            CarrierRef::Carrier(c) => Some(c),
            _ => None,
        })
        .collect();
    let table = q.table::<Carrier>();
    let mut count = 0usize;
    let mut surfaces = BTreeSet::new();
    for c in carriers {
        let n = q
            .get::<GroupAccessories>(c)
            .iter()
            .filter(|b| b.strip == device)
            .count();
        if n > 0 {
            count = count.saturating_add(n);
            if let Some(rec) = table.get(c) {
                surfaces.insert(rec.surface);
            }
        }
    }
    (count > 0).then_some(StripBrackets {
        part_use,
        count,
        surfaces,
    })
}

/// The items of a rail (D3), in `rail_occupancy`'s order: each placement's bare interval, its pads, its lock and, for
/// a strip placed as a whole, the width of the bracket part it declares. Empty for a missing carrier, a duct and a
/// rail without a valid axis. The tool's ghost and the commands read this one function (D4).
pub fn rail_items(q: &Q, carrier: Id<Carrier>) -> Vec<RailItem> {
    let occupancy = q.get::<RailOccupancy>(carrier);
    let placements = q.table::<Placement>();
    let devices = q.devices();
    occupancy
        .iter()
        .map(|o| {
            let device = devices.get(o.device);
            let part = placements.get(o.placement).and_then(|p| p.part.as_ref());
            let strip = device.is_some_and(|d| places_strip(q, d, part));
            let w = if strip {
                device.and_then(|d| bracket_part(q, d)).map(|(_, w)| w)
            } else {
                None
            };
            RailItem {
                s0: o.s0,
                s1: o.s1,
                strip,
                w,
                pad: o.pad,
                locked: o.locked,
                placement: Some(o.placement),
            }
        })
        .collect()
}

/// One derived end bracket of a rail (D3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bracket {
    /// Where it starts along the rail axis, in nanometres from `path[0]`.
    pub s0: i128,
    /// Where it ends (`s0 + w`).
    pub s1: i128,
    /// The end-bracket part use of the strip that declares it.
    pub part: PartRef,
    /// The strip it is attributed to (the BOM counts it with that strip, part 5).
    pub strip: Id<Device>,
    /// The strip's placement.
    pub placement: Id<Placement>,
}

/// `group_accessories(carrier)`: the end brackets of a rail (D3, FR-LAY-060), sorted by `s0` (then `s1`, then the
/// strip's place in the occupancy): per run of strips, one at the run's start attributed to its first strip and one
/// after each strip attributed to it, from [`rail_items`] and each strip's declaring part use. Empty for a duct, a rail
/// without a valid axis and a rail without strips. On Golden A's R2: four, attributed 1X1, 1X1, -X1031, -X1032.
#[derive(Debug)]
pub struct GroupAccessories;

impl Query for GroupAccessories {
    type Key = Id<Carrier>;
    type Value = Vec<Bracket>;
    const FAMILY: &'static str = "group_accessories";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<Bracket>> {
        let items = rail_items(q, *key);
        let placements = q.table::<Placement>();
        let devices = q.devices();
        let out = brackets(&items)
            .into_iter()
            .filter_map(|b| {
                let placement = items.get(b.item)?.placement?;
                let strip = placements.get(placement)?.device;
                let (part, _) = bracket_part(q, devices.get(strip)?)?;
                Some(Bracket {
                    s0: b.s0,
                    s1: b.s1,
                    part,
                    strip,
                    placement,
                })
            })
            .collect();
        Arc::new(out)
    }
}
