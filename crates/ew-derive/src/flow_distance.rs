// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The per-CP flow distance: `cp_flow_distance(potential)` (doc 08 §3.8 "Orientation (From/To)" step 2, FT-101).
//!
//! # Rule
//!
//! The flow distance of a CP is the number of hops from the CPs where its potential **enters**, over the
//! potential's physical wires, its bridges and the shorts inside a function (the terminal level rule and the
//! declared `Short` links). The entry CPs are:
//!
//! - for a root potential of the flow (`potential_flow` distance 0, doc 08 §3.6): its member CPs that are an
//!   external connection point or a source output (PSU +/−, a transformer secondary);
//! - for a potential reached through a pass-through pair: the pair's CP on this potential's side (the
//!   downstream side of its upstream pair, `Upstream::to_idx`);
//! - for an unreached potential: none, so no CP has a distance and orientation falls back to its step 3.
//!
//! Duplicated pins of a black box (`-102G1:+#1`, `+#2`, doc 03 §9.3) are one pin: they take the same distance
//! (no hop). Rail bonds are not hops (the rule names wires, bridges and terminal shorts only). A CP the walk
//! does not reach has no distance.
//!
//! The walk is a breadth-first search over CPs in `CpRef` order with the entry CPs at distance 0, so the
//! result does not depend on record order. Policy Memo, keyed by potential: it re-runs only when the
//! potential's topology, its flow entry, its wires, the links of its functions or the bridges change.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::Function;

use crate::Q;
use crate::flow::PotentialFlow;
use crate::index::PagelessLinks;
use crate::ip::IpPairingQ;
use crate::links::{DeclaredLink, FunctionLinksOf};
use crate::memo_keys::PartKey;
use crate::nets::CpRef;
use crate::potentials::{PotentialId, potential_part};
use crate::query::{Policy, Query};
use crate::wires::PhysicalWiresOf;

/// The value of [`CpFlowDistanceOf`]: the flow distance of every reached CP of one potential.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CpFlowDistances {
    /// The distance per CP (hops from an entry CP); a CP that is not reached is absent.
    pub distance: BTreeMap<CpRef, u32>,
    /// The entry CPs (distance 0), in `CpRef` order.
    pub entries: Vec<CpRef>,
}

impl CpFlowDistances {
    /// The distance of a CP; `None` when the walk does not reach it.
    pub fn get(&self, cp: CpRef) -> Option<u32> {
        self.distance.get(&cp).copied()
    }
}

/// `cp_flow_distance(potential)`: see the module documentation. Policy Memo.
#[derive(Debug)]
pub struct CpFlowDistanceOf;

impl Query for CpFlowDistanceOf {
    type Key = PotentialId;
    type Value = CpFlowDistances;
    const FAMILY: &'static str = "cp_flow_distance";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        std::mem::size_of::<Self::Value>()
            + value.distance.len() * (std::mem::size_of::<(CpRef, u32)>() + 2 * std::mem::size_of::<usize>())
            + value.entries.capacity() * std::mem::size_of::<CpRef>()
    }

    fn run(q: &Q, key: &PotentialId) -> Arc<CpFlowDistances> {
        Arc::new(distances(q, *key))
    }
}

fn distances(q: &Q, p: PotentialId) -> CpFlowDistances {
    let partition = potential_part(q, p);
    let Some(potential) = partition.get(p) else {
        return CpFlowDistances::default();
    };
    let flow = q.get::<PotentialFlow>(p);
    let Some(entry) = flow.as_ref().as_ref() else {
        return CpFlowDistances::default();
    };
    let members: BTreeSet<CpRef> = potential.cps.iter().copied().collect();
    let functions: BTreeSet<Id<Function>> = members.iter().map(|c| c.function).collect();
    let links: BTreeMap<Id<Function>, _> = functions
        .iter()
        .map(|f| (*f, q.get::<FunctionLinksOf>(*f)))
        .collect();

    // The entry CPs.
    let mut entries: BTreeSet<CpRef> = BTreeSet::new();
    match (entry.distance, entry.upstream) {
        (Some(_), Some(up)) => {
            entries.insert(CpRef::new(up.function, up.to_idx));
        }
        (Some(0), None) => {
            let ip = partition.ip_pairing();
            for f in &potential.ips {
                let _ = q.get_part::<IpPairingQ>((), PartKey::Ip(*f));
            }
            for cp in &members {
                let external = ip
                    .ips
                    .get(&cp.function)
                    .is_some_and(|info| info.external && info.cp == Some(*cp));
                let source = links
                    .get(&cp.function)
                    .is_some_and(|l| l.source_outputs.contains(&cp.idx));
                if external || source {
                    entries.insert(*cp);
                }
            }
        }
        _ => {}
    }
    entries.retain(|cp| members.contains(cp));
    if entries.is_empty() {
        return CpFlowDistances::default();
    }

    // The hop edges (1) and the pin groups (0) between member CPs.
    let mut hops: BTreeMap<CpRef, BTreeSet<CpRef>> = BTreeMap::new();
    let mut add = |a: CpRef, b: CpRef| {
        if a != b && members.contains(&a) && members.contains(&b) {
            hops.entry(a).or_default().insert(b);
            hops.entry(b).or_default().insert(a);
        }
    };
    for w in &q.get::<PhysicalWiresOf>(p).wires {
        add(w.ends[0].cp, w.ends[1].cp);
    }
    let pageless = q.get::<PagelessLinks>(());
    for cp in &members {
        for (_, link) in pageless.at(cp.key()).filter(|(_, l)| l.bridge) {
            let other = if link.a == cp.key() { link.b } else { link.a };
            if let Some(o) = CpRef::of(other) {
                add(*cp, o);
            }
        }
    }
    let mut pins: BTreeMap<CpRef, BTreeSet<CpRef>> = BTreeMap::new();
    for (f, l) in &links {
        for d in &l.declared {
            if let DeclaredLink::Short { a, b } = *d {
                add(CpRef::new(*f, a), CpRef::new(*f, b));
            }
        }
        for (_, group) in &l.level_groups {
            for (i, a) in group.iter().enumerate() {
                for b in &group[i + 1..] {
                    add(CpRef::new(*f, *a), CpRef::new(*f, *b));
                }
            }
        }
        for group in &l.dup_groups {
            let cps: Vec<CpRef> = group
                .iter()
                .map(|i| CpRef::new(*f, *i))
                .filter(|c| members.contains(c))
                .collect();
            for a in &cps {
                pins.entry(*a)
                    .or_default()
                    .extend(cps.iter().copied().filter(|b| b != a));
            }
        }
    }

    // A 0-1 breadth-first search: pin mates at the same distance, hop neighbours one further.
    let mut distance: BTreeMap<CpRef, u32> = BTreeMap::new();
    let mut queue: VecDeque<(CpRef, u32)> = VecDeque::new();
    for cp in &entries {
        queue.push_back((*cp, 0));
    }
    while let Some((cp, d)) = queue.pop_front() {
        if distance.get(&cp).is_some_and(|known| *known <= d) {
            continue;
        }
        distance.insert(cp, d);
        if let Some(mates) = pins.get(&cp) {
            for m in mates.iter().rev() {
                if distance.get(m).is_none_or(|known| *known > d) {
                    queue.push_front((*m, d));
                }
            }
        }
        if let Some(next) = hops.get(&cp) {
            for n in next {
                if distance.get(n).is_none_or(|known| *known > d + 1) {
                    queue.push_back((*n, d + 1));
                }
            }
        }
    }
    CpFlowDistances {
        distance,
        entries: entries.into_iter().collect(),
    }
}
