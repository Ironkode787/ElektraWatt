// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `numbering.renumber` (`RenumberDevices`, FR-NUM-008, FR-NUM-009), page-based scheme only. The scheme
//! catalogue, structure scopes and tombstones are WP-S1.11.

use std::collections::{BTreeMap, BTreeSet};

use ew_ids::Id;
use ew_mltext::Ident;
use ew_schema::{Device, Page};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::common::yes;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::numbering::{self, CounterScope};
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// A device of a numbering group with the position (x, y in nm) of its main representation.
type Member = (i64, i64, Id<Device>);

/// Which devices `numbering.renumber` renumbers.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum DeviceScope {
    /// The listed devices.
    Selection(#[schemars(with = "Vec<UuidS>")] Vec<Id<Device>>),
    /// The devices whose main representation is on one of the pages.
    Pages(#[schemars(with = "Vec<UuidS>")] Vec<Id<Page>>),
    /// Every device.
    Project,
}

/// The numbering scheme.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NumberingScheme {
    /// `{page}{letter}{counter}`, counters unique per page of the main representation and letter.
    #[default]
    PageBased,
}

/// Options of `numbering.renumber` (FR-NUM-009).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenumberOptions {
    /// Leave devices with a frozen tag alone (always true in v0: a frozen tag is never renumbered).
    #[serde(default = "yes")]
    pub skip_frozen: bool,
    /// Renumber only devices without a counter or whose counter collides in their scope.
    #[serde(default)]
    pub only_nonconforming: bool,
    /// Close counter gaps: counters become 1, 2, 3 … in order; otherwise a conforming counter is kept.
    #[serde(default = "yes")]
    pub close_gaps: bool,
}

impl Default for RenumberOptions {
    fn default() -> RenumberOptions {
        RenumberOptions {
            skip_frozen: true,
            only_nonconforming: false,
            close_gaps: true,
        }
    }
}

/// `numbering.renumber` (`RenumberDevices`): renumbers device counters under the page-based scheme.
///
/// The devices in scope that take part in numbering (`numbering::numbered`: no terminal aspect, no frozen
/// tag, default scheme) and are placed are grouped by (page of the main representation, letter). In each
/// group the devices are ordered by their main representation's position, column first (x, then y), then
/// by ID. Counters held by devices of the group outside the scope are reserved. Then, in order:
///
/// - `only_nonconforming`: a device keeps its counter unless it has none or shares it with an earlier
///   device of the group (or a reserved one); those get the lowest free counter, one that no device of
///   the group keeps and no earlier device was given;
/// - else with `close_gaps` (the default): each device gets the lowest counter that is neither reserved
///   nor given to an earlier device, so the group reads 1, 2, 3 … in position order;
/// - else: as `only_nonconforming` (a device keeps a counter that is not reserved and not taken by an
///   earlier device; the others get the lowest free one).
///
/// Only counters change: identities and every reference by ID stay, so the topology is identical and no
/// reference is orphaned (Golden B, FR-NUM-008). Running it twice changes nothing the second time. Bulk.
/// Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenumberDevices {
    /// The devices to renumber.
    pub scope: DeviceScope,
    /// The scheme.
    #[serde(default)]
    pub scheme: NumberingScheme,
    /// The options.
    #[serde(default)]
    pub options: RenumberOptions,
}

impl Command for RenumberDevices {
    const NAME: &'static str = "numbering.renumber";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let in_scope: BTreeSet<Id<Device>> = match &self.scope {
            DeviceScope::Selection(devices) => {
                for d in devices {
                    tx.require::<Device>(*d)?;
                }
                devices.iter().copied().collect()
            }
            DeviceScope::Pages(pages) => {
                for p in pages {
                    tx.require::<Page>(*p)?;
                }
                tx.iter::<Device>()
                    .into_iter()
                    .map(|d| d.id)
                    .filter(|d| numbering::main_page(tx, *d).is_some_and(|p| pages.contains(&p)))
                    .collect()
            }
            DeviceScope::Project => tx.iter::<Device>().into_iter().map(|d| d.id).collect(),
        };
        // Numbered, placed devices by (page, letter), with the position of the main representation.
        let mut groups: BTreeMap<(Id<Page>, Ident), Vec<Member>> = BTreeMap::new();
        for id in &in_scope {
            let Some(device) = tx.get::<Device>(*id) else {
                continue;
            };
            if !numbering::numbered(&device) {
                continue;
            }
            let Some(rep) = numbering::main_rep(tx, *id) else {
                continue;
            };
            groups
                .entry((rep.page, device.tag.letter.clone()))
                .or_default()
                .push((rep.pos.x.0, rep.pos.y.0, *id));
        }
        let mut assignments: Vec<(Id<Device>, u32)> = Vec::new();
        for ((page, letter), mut members) in groups {
            members.sort();
            let reserved: BTreeSet<u32> = numbering::devices_in_scope(tx, &letter, CounterScope::Page(page))
                .into_iter()
                .filter(|(d, _)| !in_scope.contains(d))
                .filter_map(|(_, c)| c)
                .collect();
            let counters: Vec<(Id<Device>, Option<u32>)> = members
                .iter()
                .map(|(_, _, d)| (*d, tx.get::<Device>(*d).and_then(|dev| dev.tag.counter)))
                .collect();
            assignments.extend(assign_counters(&counters, &reserved, &self.options));
        }
        for (id, counter) in assignments {
            tx.update::<Device>(id, |d| d.tag.counter = Some(counter))?;
        }
        Ok(Outcome::default())
    }
}

/// The counters of one group, `members` in position order with their current counters: see
/// [`RenumberDevices`].
fn assign_counters(
    members: &[(Id<Device>, Option<u32>)],
    reserved: &BTreeSet<u32>,
    options: &RenumberOptions,
) -> Vec<(Id<Device>, u32)> {
    let lowest_free = |avoid: &dyn Fn(u32) -> bool| (1..).find(|c| !avoid(*c)).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(members.len());
    if options.close_gaps && !options.only_nonconforming {
        let mut given: BTreeSet<u32> = BTreeSet::new();
        for (id, _) in members {
            let c = lowest_free(&|c| reserved.contains(&c) || given.contains(&c));
            given.insert(c);
            out.push((*id, c));
        }
        return out;
    }
    // Keepers: the first holder of a counter that no reserved device holds.
    let mut kept: BTreeSet<u32> = BTreeSet::new();
    let mut keeps: Vec<bool> = Vec::with_capacity(members.len());
    for (_, counter) in members {
        let keep = counter.is_some_and(|c| !reserved.contains(&c) && kept.insert(c));
        keeps.push(keep);
    }
    let mut given: BTreeSet<u32> = BTreeSet::new();
    for ((id, counter), keep) in members.iter().zip(keeps) {
        let c = match counter {
            Some(c) if keep => *c,
            _ => lowest_free(&|c| reserved.contains(&c) || kept.contains(&c) || given.contains(&c)),
        };
        given.insert(c);
        out.push((*id, c));
    }
    out
}
