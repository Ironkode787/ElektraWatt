// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `strip_bridges(strip)`: the terminal bridges of one strip, grouped (doc 08 §2.3, §6.5; WP-S2.07 D6).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{BridgeKind, Connection, ConnectionKind, Device, Endpoint, PartRef};

use crate::Q;
use crate::query::{Policy, Query};

/// One bridge of a strip: the bridge connections of one group (doc 08 §2.3: one bridge object per group).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BridgeGroup {
    /// The group number on the strip (`BridgeSpec.group`).
    pub group: u16,
    /// The bridge kind: the kind of the group's first connection in [`BridgeGroup::connections`] order (the
    /// specification does not reconcile mixed kinds; the first in printed order keeps the result independent of IDs,
    /// NFR-REL-07).
    pub kind: BridgeKind,
    /// The devices the group's connections end at, each once, in (`seq`, ID) order (a device without a terminal
    /// aspect first; a bridge joins terminals, so there is none in a well-formed project).
    pub terminals: Vec<Id<Device>>,
    /// The group's connections in (lower `seq` of their terminal ends, ID) order.
    pub connections: Vec<Id<Connection>>,
    /// The carrier (D6): the first connection of [`BridgeGroup::connections`] with a part
    /// (`overrides.wire_part`), else the first connection.
    pub carrier: Id<Connection>,
    /// The carrier's `overrides.wire_part`: the bridge's part, `None` when the group has none.
    pub part: Option<PartRef>,
}

/// One bridge connection of a group: the lower seq of its terminal ends, its ID and the record.
type Member = (Option<u32>, Id<Connection>, Arc<Connection>);

/// `strip_bridges(strip)`: the bridges of a strip, in group order.
///
/// A bridge connection (`ConnectionKind::Bridge`) belongs to the strip of its first terminal end, `a` then `b`
/// (an end whose function's device has a terminal aspect). The strip's bridge connections are found from its
/// terminals' connected CPs and grouped by `BridgeSpec.group`; each group is a [`BridgeGroup`]. A strip without
/// terminals, a missing device and a device without bridges have none. The result does not depend on a hash
/// order (NFR-REL-07).
#[derive(Debug)]
pub struct StripBridges;

impl Query for StripBridges {
    type Key = Id<Device>;
    type Value = Vec<BridgeGroup>;
    const FAMILY: &'static str = "strip_bridges";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<BridgeGroup>> {
        let strip = *key;
        let devices = q.devices();
        let functions = q.functions();
        let connections = q.connections();
        let seq_of = |d: Id<Device>| {
            devices
                .get(d)
                .and_then(|x| x.aspects.terminal.as_ref())
                .map(|t| t.seq)
        };
        let terminal_end = |e: &Endpoint| -> Option<(Id<Device>, Id<Device>, u32)> {
            let Endpoint::Cp(f, _) = e else { return None };
            let d = functions.get(*f)?.device?;
            let t = devices.get(d)?.aspects.terminal.as_ref()?;
            Some((d, t.strip, t.seq))
        };
        let end_device = |e: &Endpoint| -> Option<Id<Device>> {
            match e {
                Endpoint::Cp(f, _) => functions.get(*f)?.device,
                _ => None,
            }
        };

        // The strip's bridge connections, each once.
        let mut found: BTreeSet<Id<Connection>> = BTreeSet::new();
        for t in q.idx().terminals_of_strip(strip) {
            for f in q.idx().functions_of_device(t) {
                for (_, at) in q.idx().connected_cps_of_function(f) {
                    found.extend(at);
                }
            }
        }

        let mut groups: BTreeMap<u16, Vec<Member>> = BTreeMap::new();
        for id in found {
            let Some(c) = connections.get(id) else { continue };
            let ConnectionKind::Bridge(spec) = &c.kind else {
                continue;
            };
            let ends = [terminal_end(&c.a), terminal_end(&c.b)];
            let Some(owner) = ends.iter().flatten().map(|(_, s, _)| *s).next() else {
                continue;
            };
            if owner != strip {
                continue;
            }
            let lower = ends.iter().flatten().map(|(_, _, seq)| *seq).min();
            groups
                .entry(spec.group)
                .or_default()
                .push((lower, id, Arc::clone(c)));
        }

        let out = groups
            .into_iter()
            .filter_map(|(group, mut members)| {
                members.sort_by_key(|(lower, id, _)| (*lower, *id));
                let kind = members.first().and_then(|(_, _, c)| match &c.kind {
                    ConnectionKind::Bridge(spec) => Some(spec.kind.clone()),
                    _ => None,
                })?;
                let mut terminals: Vec<Id<Device>> = Vec::new();
                for (_, _, c) in &members {
                    for d in [end_device(&c.a), end_device(&c.b)].into_iter().flatten() {
                        if !terminals.contains(&d) {
                            terminals.push(d);
                        }
                    }
                }
                terminals.sort_by_key(|d| (seq_of(*d), *d));
                let (_, first, _) = members.first()?;
                let carrier = members
                    .iter()
                    .find(|(_, _, c)| c.overrides.wire_part.is_some())
                    .map_or(*first, |(_, id, _)| *id);
                let part = members
                    .iter()
                    .find(|(_, id, _)| *id == carrier)
                    .and_then(|(_, _, c)| c.overrides.wire_part.clone());
                Some(BridgeGroup {
                    group,
                    kind,
                    terminals,
                    connections: members.iter().map(|(_, id, _)| *id).collect(),
                    carrier,
                    part,
                })
            })
            .collect();
        Arc::new(out)
    }
}
