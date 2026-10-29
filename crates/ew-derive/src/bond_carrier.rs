// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The carrier of a 0 V–PE bond: `bond_carrier(potential)` (WP-S1.16 D3; doc 08 §3.7; FR-CON-014).
//!
//! A potential whose member CPs carry both the role `M` and the role `PE` is a 0 V joined to PE by a plain
//! connection (EW-POT-001's member-role conflict). The quick fix "Declare 0 V–PE bond (PELV)" declares the bond on
//! the connection that joins the two groups, which then keeps 0 V and PE as two potentials. That connection is
//! the **carrier**: a connection c of the potential whose removal splits the potential's connection graph into
//! two parts, one holding every `M`-role member CP and the other every `PE`-role member CP.
//!
//! - **The graph.** Its nodes are the potential's member endpoints (CPs and junctions). Its edges are the
//!   connections that join (`Wire`, `Core`, `Bus`, `Internal`, `Bridge`) with both ends among the members, minus
//!   the connections a bond already cuts, plus the joins that are not connections: the shorts inside a function
//!   (the terminal level rule included), the interruption-point pairs and the rail bonds of each location (a
//!   chain over its PE CPs in CP order). A graph that is not connected (a join this query does not model) has no
//!   carrier: no guess.
//! - **The bridges** of the graph are found by one depth-first search (Tarjan) from the first node in endpoint
//!   order, over adjacency lists in edge order (connections in ID order, then the other joins in a fixed order).
//!   Parallel connections are two edges, so neither is a bridge.
//! - **The candidates** are the bridges that are connections and split the roles as above. They order by: one
//!   end on a `PE`-role member CP first, then the displayed number of the physical wire the connection belongs to
//!   in natural order (a wire without a number after those with one), then the two ends' texts (`tag:designation`
//!   of a CP, the M-side end first; a junction's text is empty). The first is the carrier unless the second
//!   shares every key with it (a tie on printed keys: no carrier). Nothing in the order is an ID, so a copy of
//!   the drawing with fresh IDs gets the same carrier (ID-677).
//! - **The value.** `a` is the carrier's end on the `M` side, `b` its end on the `PE` side. `None` when the
//!   potential has no `M`-role or no `PE`-role member CP, when no candidate exists, or on a tie.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::natural_cmp;
use ew_model::EndpointKey;
use ew_schema::{Connection, ConnectionKind, PotentialType};

use crate::Q;
use crate::links::RailBondsQ;
use crate::memo_keys::PartKey;
use crate::names::{cp_text, member_roles};
use crate::nets::{CpRef, ends, joins_net};
use crate::numbering::WireNumbers;
use crate::potentials::{PotentialId, PotentialPartition};
use crate::query::{Policy, Query};
use crate::stitch::{EdgeKind, StitchGraphQ};
use crate::wires::PhysicalWiresOf;

/// The value of [`BondCarrierQ`]: the connection that carries a 0 V–PE bond and its two ends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Carrier {
    /// The connection (the bond's `via`).
    pub via: Id<Connection>,
    /// Its end on the side of the `M`-role member CPs (the bond's `a`).
    pub a: EndpointKey,
    /// Its end on the side of the `PE`-role member CPs (the bond's `b`).
    pub b: EndpointKey,
}

/// `bond_carrier(potential)`: see the module documentation. Policy Memo; note-free.
#[derive(Debug)]
pub struct BondCarrierQ;

impl Query for BondCarrierQ {
    type Key = PotentialId;
    type Value = Option<Carrier>;
    const FAMILY: &'static str = "bond_carrier";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &PotentialId) -> Arc<Option<Carrier>> {
        Arc::new(carrier(q, *key))
    }
}

/// An edge of the carrier graph: its two node indices and the connection it is, if it is one.
struct Edge {
    a: usize,
    b: usize,
    connection: Option<Id<Connection>>,
}

fn carrier(q: &Q, key: PotentialId) -> Option<Carrier> {
    let partition = q.get_part::<PotentialPartition>((), PartKey::Potential(key));
    let comp = partition.component_of(key)?;
    let p = comp.potential.as_ref()?;
    let roles = member_roles(q, p);
    let m_cps: BTreeSet<EndpointKey> = roles
        .iter()
        .filter(|(_, r)| *r == PotentialType::M)
        .map(|(cp, _)| cp.key())
        .collect();
    let pe_cps: BTreeSet<EndpointKey> = roles
        .iter()
        .filter(|(_, r)| *r == PotentialType::Pe)
        .map(|(cp, _)| cp.key())
        .collect();
    if m_cps.is_empty() || pe_cps.is_empty() {
        return None;
    }

    // The nodes, in endpoint order.
    let index: BTreeMap<EndpointKey, usize> = comp
        .members
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(i, e)| (e, i))
        .collect();
    let node = |e: EndpointKey| index.get(&e).copied();
    let n = index.len();
    let mut edges: Vec<Edge> = Vec::new();
    let mut push = |a: Option<usize>, b: Option<usize>, connection: Option<Id<Connection>>| {
        if let (Some(a), Some(b)) = (a, b)
            && a != b
        {
            edges.push(Edge { a, b, connection });
        }
    };

    // The connections that join, in ID order, minus the bonds' cut ones.
    let stitch = q.get::<StitchGraphQ>(());
    let connections = q.connections();
    let mut ids: BTreeSet<Id<Connection>> = BTreeSet::new();
    for e in index.keys() {
        ids.extend(q.idx().connections_at(*e));
    }
    for id in ids {
        let Some(c) = connections.get(id) else { continue };
        if !(joins_net(&c.kind) || matches!(c.kind, ConnectionKind::Bridge(_))) || stitch.cut.contains(&id) {
            continue;
        }
        let Some((a, b)) = ends(c) else { continue };
        push(node(a), node(b), Some(id));
    }
    // The joins that are not connections: shorts and interruption-point pairs, in edge order.
    let ip_cp = |f: Id<ew_schema::Function>| stitch.ips.get(&f).and_then(|i| i.cp).map(|c| c.key());
    for e in &stitch.edges {
        match e.kind {
            EdgeKind::Short { function, a, b, .. } => push(
                node(EndpointKey::Cp(function, a)),
                node(EndpointKey::Cp(function, b)),
                None,
            ),
            EdgeKind::IpPair { a, b } => push(ip_cp(a).and_then(node), ip_cp(b).and_then(node), None),
            _ => {}
        }
    }
    // The rail bonds: a chain over each location's PE CPs in the graph, in CP order (as the stitch graph chains
    // them).
    let rail = q.get::<RailBondsQ>(());
    for cps in rail.by_location.values() {
        let chain: Vec<usize> = cps.iter().filter_map(|c| node(c.key())).collect();
        for w in chain.windows(2) {
            push(Some(w[0]), Some(w[1]), None);
        }
    }

    let bridges = bridges(n, &edges)?;
    let m_nodes: Vec<usize> = m_cps.iter().filter_map(|e| node(*e)).collect();
    let pe_nodes: Vec<usize> = pe_cps.iter().filter_map(|e| node(*e)).collect();
    // (connection, its end on the M side, its end on the PE side).
    let mut found: Vec<(Id<Connection>, EndpointKey, EndpointKey)> = Vec::new();
    let keys: Vec<EndpointKey> = index.keys().copied().collect();
    for (e, inside) in bridges {
        let edge = &edges[e];
        let Some(connection) = edge.connection else {
            continue;
        };
        let all_in = |v: &[usize]| v.iter().all(|x| inside[*x]);
        let none_in = |v: &[usize]| v.iter().all(|x| !inside[*x]);
        // `inside` is the side of the child end.
        let child_m = all_in(&m_nodes) && none_in(&pe_nodes);
        let child_pe = all_in(&pe_nodes) && none_in(&m_nodes);
        if !(child_m || child_pe) {
            continue;
        }
        let (on_m, on_pe) = if inside[edge.a] == child_m {
            (edge.a, edge.b)
        } else {
            (edge.b, edge.a)
        };
        found.push((connection, keys[on_m], keys[on_pe]));
    }
    match found.len() {
        0 => return None,
        1 => {
            let (via, a, b) = found[0];
            return Some(Carrier { via, a, b });
        }
        _ => {}
    }

    // The order of D3 on printed keys.
    let wires = q.get::<PhysicalWiresOf>(key);
    let numbers = q.get::<WireNumbers>(());
    let number = |c: Id<Connection>| {
        wires
            .wires
            .iter()
            .find(|w| w.constituents.contains(&c))
            .and_then(|w| numbers.number(w.id))
            .map(|n| n.label.clone())
    };
    let text = |e: EndpointKey| CpRef::of(e).map(|cp| cp_text(q, cp)).unwrap_or_default();
    let mut keyed: Vec<(bool, Option<String>, String, String, Carrier)> = found
        .into_iter()
        .map(|(via, a, b)| {
            (
                !pe_cps.contains(&b),
                number(via),
                text(a),
                text(b),
                Carrier { via, a, b },
            )
        })
        .collect();
    let cmp_text = |x: &str, y: &str| natural_cmp(x, y).then_with(|| x.cmp(y));
    let order = |x: &(bool, Option<String>, String, String, Carrier),
                 y: &(bool, Option<String>, String, String, Carrier)| {
        x.0.cmp(&y.0)
            .then_with(|| match (&x.1, &y.1) {
                (Some(a), Some(b)) => cmp_text(a, b),
                (a, b) => a.is_none().cmp(&b.is_none()),
            })
            .then_with(|| cmp_text(&x.2, &y.2))
            .then_with(|| cmp_text(&x.3, &y.3))
    };
    keyed.sort_by(order);
    if order(&keyed[0], &keyed[1]).is_eq() {
        return None;
    }
    Some(keyed[0].4)
}

/// The bridges of a multigraph of `n` nodes (one DFS from node 0, Tarjan's low links, an explicit stack): each
/// bridge's edge index and, per node, whether it lies on the bridge's child side. `None` when the graph is not
/// connected.
fn bridges(n: usize, edges: &[Edge]) -> Option<Vec<(usize, Vec<bool>)>> {
    if n == 0 {
        return None;
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, e) in edges.iter().enumerate() {
        adj[e.a].push(i);
        adj[e.b].push(i);
    }
    let other = |e: usize, u: usize| if edges[e].a == u { edges[e].b } else { edges[e].a };
    const UNSEEN: usize = usize::MAX;
    let mut tin = vec![UNSEEN; n];
    let mut tout = vec![0usize; n];
    let mut low = vec![0usize; n];
    let mut timer = 0usize;
    // (node, the edge it was entered by, the next adjacency position).
    let mut stack: Vec<(usize, Option<usize>, usize)> = vec![(0, None, 0)];
    tin[0] = 0;
    low[0] = 0;
    timer += 1;
    let mut found: Vec<(usize, usize)> = Vec::new();
    while let Some(top) = stack.last_mut() {
        let (u, entered, i) = *top;
        if let Some(&e) = adj[u].get(i) {
            top.2 += 1;
            if Some(e) == entered {
                continue;
            }
            let w = other(e, u);
            if tin[w] == UNSEEN {
                tin[w] = timer;
                low[w] = timer;
                timer += 1;
                stack.push((w, Some(e), 0));
            } else {
                low[u] = low[u].min(tin[w]);
            }
        } else {
            stack.pop();
            tout[u] = timer;
            if let Some(e) = entered {
                let parent = other(e, u);
                low[parent] = low[parent].min(low[u]);
                if low[u] > tin[parent] {
                    found.push((e, u));
                }
            }
        }
    }
    if tin.contains(&UNSEEN) {
        return None;
    }
    found.sort_unstable();
    Some(
        found
            .into_iter()
            .map(|(e, child)| {
                let inside = (0..n)
                    .map(|v| tin[v] >= tin[child] && tin[v] < tout[child])
                    .collect();
                (e, inside)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(a: usize, b: usize) -> Edge {
        Edge {
            a,
            b,
            connection: None,
        }
    }

    #[test]
    fn a_path_is_all_bridges_and_a_parallel_pair_is_none() {
        let path = [e(0, 1), e(1, 2)];
        let got = bridges(3, &path).expect("connected");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, 0);
        assert_eq!(got[0].1, vec![false, true, true], "the child side of 0-1");
        assert_eq!(got[1].1, vec![false, false, true]);
        let parallel = [e(0, 1), e(0, 1), e(1, 2)];
        let got = bridges(3, &parallel).expect("connected");
        assert_eq!(got.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![2]);
        let ring = [e(0, 1), e(1, 2), e(2, 0)];
        assert!(bridges(3, &ring).expect("connected").is_empty());
        assert!(bridges(3, &[e(0, 1)]).is_none(), "node 2 is not reached");
    }
}
