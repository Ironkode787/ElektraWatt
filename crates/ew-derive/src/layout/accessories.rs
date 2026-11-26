// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The items of a rail and its derived end brackets (FR-LAY-060, doc 12 §3.4): [`rail_items`] turns
//! `rail_occupancy` into the rail model's items (the strips with their bracket widths, [`crate::strip_bracket`]), and
//! the memo query [`GroupAccessories`] (`group_accessories(carrier)`) lists the end brackets the strips need. Nothing is
//! stored: a bracket is derived from the strips' positions each time, so moving a strip away from its run adds two.
//! [`crate::placed_brackets`] counts the brackets attributed to one strip over its rails, which `part_accessories` and
//! the BOM read.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Carrier, Device, PartRef, Placement};

use super::queries::{RailOccupancy, places_strip};
use super::snap::{BracketRole, RailItem, brackets};
use crate::Q;
use crate::query::{Policy, Query};

/// The items of a rail, in `rail_occupancy`'s order: each placement's bare interval, its pads, its lock and, for
/// a strip placed as a whole, the width of its end bracket ([`crate::strip_bracket`]). Empty for a missing carrier, a duct and a
/// rail without a valid axis. The tool's ghost and the commands read this one function.
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
                device.and_then(|d| crate::strip_bracket(q, d)).map(|b| b.width)
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

/// One derived end bracket of a rail.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bracket {
    /// Where it starts along the rail axis, in nanometres from `path[0]`.
    pub s0: i128,
    /// Where it ends (`s0 + w`).
    pub s1: i128,
    /// The strip's end-bracket part ([`crate::strip_bracket`]).
    pub part: PartRef,
    /// The strip it is attributed to (the BOM counts it with that strip, part 5).
    pub strip: Id<Device>,
    /// The strip's placement.
    pub placement: Id<Placement>,
    /// Before its run or after its strip, as [`brackets`] made it.
    pub role: BracketRole,
}

/// The end brackets of one rail, the value of [`GroupAccessories`]: the brackets in rail order (it dereferences to
/// them) and how many each strip holds, counted once when the family runs.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RailBrackets {
    /// The brackets, sorted as [`GroupAccessories`] says.
    pub list: Vec<Bracket>,
    /// The number of brackets attributed to each strip.
    pub per_strip: BTreeMap<Id<Device>, usize>,
}

impl RailBrackets {
    /// The brackets in rail order, with each strip's count.
    pub fn new(list: Vec<Bracket>) -> Self {
        let mut per_strip: BTreeMap<Id<Device>, usize> = BTreeMap::new();
        for b in &list {
            let n = per_strip.entry(b.strip).or_default();
            *n = n.saturating_add(1);
        }
        Self { list, per_strip }
    }

    /// How many of the rail's brackets are attributed to `strip` (0 when none).
    pub fn count_of(&self, strip: Id<Device>) -> usize {
        self.per_strip.get(&strip).copied().unwrap_or(0)
    }
}

impl std::ops::Deref for RailBrackets {
    type Target = [Bracket];

    fn deref(&self) -> &[Bracket] {
        &self.list
    }
}

/// `group_accessories(carrier)`: the end brackets of a rail (FR-LAY-060), sorted by `s0` (then `s1`, then the
/// strip's place in the occupancy): per run of strips, one at the run's start attributed to its first strip and one
/// after each strip attributed to it, from [`rail_items`] and each strip's end bracket ([`crate::strip_bracket`]). Empty for a duct, a rail
/// without a valid axis and a rail without strips. The
/// value also counts the brackets per strip ([`RailBrackets::count_of`]).
#[derive(Debug)]
pub struct GroupAccessories;

impl Query for GroupAccessories {
    type Key = Id<Carrier>;
    type Value = RailBrackets;
    const FAMILY: &'static str = "group_accessories";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<RailBrackets> {
        let items = rail_items(q, *key);
        let placements = q.table::<Placement>();
        let devices = q.devices();
        let out = brackets(&items)
            .into_iter()
            .filter_map(|b| {
                let placement = items.get(b.item)?.placement?;
                let strip = placements.get(placement)?.device;
                let part = crate::strip_bracket(q, devices.get(strip)?)?.part;
                Some(Bracket {
                    s0: b.s0,
                    s1: b.s1,
                    part,
                    strip,
                    placement,
                    role: b.role,
                })
            })
            .collect();
        Arc::new(RailBrackets::new(out))
    }
}
