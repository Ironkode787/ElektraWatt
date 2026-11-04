// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid side (doc 11 §5.1, WP-S1.20 D12): `fluid_nets()`, the partition of the fluid CPs joined by `fluid`
//! connections and their junctions.
//!
//! A fluid net is a connected set of nodes (function CPs and junctions) joined by connections of kind `Fluid`, on
//! every page and across pages; galleries, fluid interruption points and `Short` links are 1.0's. Its ID is its
//! minimum member CP ID, as an electrical net's ([`NetId`]). A fluid line is no wire: it joins no electrical net
//! ([`crate::joins_net`] leaves it out). A CP that no fluid line reaches is in no fluid net.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{Connection, ConnectionKind};

use crate::Q;
use crate::nets::{Dsu, NetId, ends};
use crate::query::{Policy, Query};

/// One fluid net.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FluidNet {
    /// The net ID (minimum member CP ID).
    pub id: NetId,
    /// The members in node order.
    pub members: Vec<EndpointKey>,
    /// The fluid connections of the net, in ID order.
    pub connections: Vec<Id<Connection>>,
}

/// The value of [`FluidNets`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FluidPartition {
    /// The nets in ID order.
    pub nets: Vec<FluidNet>,
    net_of: BTreeMap<EndpointKey, usize>,
}

impl FluidPartition {
    /// The fluid net of a node; `None` when no fluid line reaches it.
    pub fn net_of(&self, at: EndpointKey) -> Option<&FluidNet> {
        self.net_of.get(&at).map(|&i| &self.nets[i])
    }
}

/// `fluid_nets()`: the fluid partition of the project (doc 11 §5.1, WP-S1.20 D12). Policy Memo.
#[derive(Debug)]
pub struct FluidNets;

impl Query for FluidNets {
    type Key = ();
    type Value = FluidPartition;
    const FAMILY: &'static str = "fluid_nets";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _: &()) -> Arc<FluidPartition> {
        let mut dsu = Dsu::default();
        let mut via: BTreeMap<EndpointKey, BTreeSet<Id<Connection>>> = BTreeMap::new();
        for (id, c) in q.connections().iter() {
            if !matches!(c.kind, ConnectionKind::Fluid(_)) {
                continue;
            }
            let Some((a, b)) = ends(c) else { continue };
            let (ia, ib) = (dsu.node(a), dsu.node(b));
            dsu.union(ia, ib);
            via.entry(a).or_default().insert(id);
            via.entry(b).or_default().insert(id);
        }
        let mut nets: Vec<FluidNet> = dsu
            .groups()
            .into_iter()
            .filter_map(|members| {
                let id = NetId::of(members.iter().copied())?;
                let connections: BTreeSet<Id<Connection>> = members
                    .iter()
                    .flat_map(|m| via.get(m).into_iter().flatten().copied())
                    .collect();
                Some(FluidNet {
                    id,
                    members,
                    connections: connections.into_iter().collect(),
                })
            })
            .collect();
        nets.sort_by(|x, y| x.id.cmp(&y.id).then_with(|| x.members[0].cmp(&y.members[0])));
        let mut net_of = BTreeMap::new();
        for (i, n) in nets.iter().enumerate() {
            for m in &n.members {
                net_of.insert(*m, i);
            }
        }
        Arc::new(FluidPartition { nets, net_of })
    }
}
