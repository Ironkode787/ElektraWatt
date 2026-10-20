// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Potential flow: `potential_flow(potential)` (ARCH §6.4, doc 08 §3.6).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{ContactKind, FnDef, Function, FunctionKind, LibCopy, PotentialEnv, PotentialType};
use ew_units::Dec3;

use crate::note::DeriveNote;
use crate::potentials::{Attr, AttrSource, PotentialId, PotentialPartition};
use crate::query::{Policy, Query};
use crate::stitch::{EdgeKind, Level, StitchGraphQ};
use crate::{FunctionCps, Q};

/// The pass-through pair through which a potential is reached.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Upstream {
    /// The function of the pair.
    pub function: Id<Function>,
    /// The pair's CP on the upstream side.
    pub from_idx: u16,
    /// The pair's CP on this potential's side.
    pub to_idx: u16,
    /// The upstream potential.
    pub from: PotentialId,
}

/// A load on a potential and its protective device (doc 08 §3.6, FR-REP-026).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Load {
    /// The load function.
    pub function: Id<Function>,
    /// The nearest upstream protective device of the potential.
    pub protected_by: Option<Id<Function>>,
}

/// The flow entry of one potential.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FlowEntry {
    /// The potential.
    pub potential: PotentialId,
    /// Hops from the supply over pass-through pairs; `None` when unreached (distance ∞).
    pub distance: Option<u32>,
    /// The pair it is reached through; `None` for a root or an unreached potential.
    pub upstream: Option<Upstream>,
    /// The type: the partition's own (definition, role), else propagated.
    pub ptype: Option<Attr<PotentialType>>,
    /// The voltage: the partition's own, else propagated.
    pub voltage: Option<Attr<Dec3>>,
    /// AC/DC, frequency, earthing: the partition's own, else propagated.
    pub env: Option<Attr<PotentialEnv>>,
    /// Functionally switched (doc 08 §3.6).
    pub switched: bool,
    /// The function whose opening pair made it switched (the nearest upstream one).
    pub switched_by: Option<Id<Function>>,
    /// The nearest upstream protective device.
    pub protected_by: Option<Id<Function>>,
    /// The loads on the potential, in function ID order: its consuming functions (coils, lamps, PLC inputs,
    /// a PSU input), never a function with a pass-through pair touching it, a source feeding it, an
    /// interruption point or a terminal (review decision D4).
    pub loads: Vec<Load>,
}

/// The value of [`FlowBfs`]: one BFS over the whole partition.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Flow {
    /// One entry per potential of the partition.
    pub entries: BTreeMap<PotentialId, FlowEntry>,
    /// The notes of this derivation (also pushed to the `Q`).
    pub notes: Vec<DeriveNote>,
}

/// The flow BFS over the partition (doc 08 §3.6: "one BFS over the whole partition"). Initial policy Memo.
#[derive(Debug)]
pub struct FlowBfs;

impl Query for FlowBfs {
    type Key = ();
    type Value = Flow;
    const FAMILY: &'static str = "potential_flow_bfs";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, _key: &()) -> Arc<Flow> {
        Arc::new(run_flow(q))
    }
}

/// One pass-through pair between two distinct potentials (indices into `Partition::potentials`).
struct Pair {
    function: Id<Function>,
    a: u16,
    b: u16,
    opens: bool,
    /// The potential of CP `a`.
    pa: usize,
    /// The potential of CP `b`.
    pb: usize,
}

impl Pair {
    /// The potential on the other side of `p`, with the CP indices `(from_idx, to_idx)` seen from `p`.
    fn from(&self, p: usize) -> (usize, u16, u16) {
        if self.pa == p {
            (self.pb, self.a, self.b)
        } else {
            (self.pa, self.b, self.a)
        }
    }
}

/// The flow state of one potential while the BFS runs.
#[derive(Clone, Default)]
struct State {
    distance: Option<u32>,
    upstream: Option<Upstream>,
    ptype: Option<Attr<PotentialType>>,
    voltage: Option<Attr<Dec3>>,
    env: Option<Attr<PotentialEnv>>,
    switched: bool,
    switched_by: Option<Id<Function>>,
    protected_by: Option<Id<Function>>,
}

/// The attribute of the downstream potential: its own value, else the upstream's final one, propagated.
fn inherit<T: Clone>(own: &Option<Attr<T>>, up: &Option<Attr<T>>, function: Id<Function>) -> Option<Attr<T>> {
    own.clone().or_else(|| {
        up.as_ref().map(|a| Attr {
            value: a.value.clone(),
            source: AttrSource::Propagated(function),
        })
    })
}

/// The FnDef copy of a function.
fn fndef_of(q: &Q, function: Id<Function>) -> Option<Arc<LibCopy<FnDef>>> {
    let f = q.functions().get(function)?;
    q.fndefs().get(f.def).cloned()
}

fn run_flow(q: &Q) -> Flow {
    let graph = q.get::<StitchGraphQ>(());
    let part = q.get::<PotentialPartition>(());
    let n = part.potentials.len();
    let functions = q.functions();

    // Pass-through pairs between two distinct potentials, in (function, a, b) order. A pair inside one
    // potential is a bypassed contact (doc 08 §12 item 2).
    let mut pairs: Vec<Pair> = Vec::new();
    for e in graph.edges.iter().filter(|e| e.level == Level::Link) {
        let EdgeKind::PassThrough {
            function,
            a,
            b,
            opens,
        } = e.kind
        else {
            continue;
        };
        let (Some(pa), Some(pb)) = (part.of_node(e.a), part.of_node(e.b)) else {
            continue;
        };
        if pa != pb {
            pairs.push(Pair {
                function,
                a,
                b,
                opens,
                pa,
                pb,
            });
        }
    }
    pairs.sort_by_key(|p| (p.function, p.a, p.b, p.pa, p.pb));
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, p) in pairs.iter().enumerate() {
        adj[p.pa].push(i);
        adj[p.pb].push(i);
    }

    let is_ip = |f: Id<Function>| {
        graph.ips.contains_key(&f)
            || functions
                .get(f)
                .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_)))
    };

    // Roots: potentials holding an external point or a source output CP.
    let mut st: Vec<State> = part
        .potentials
        .iter()
        .map(|p| State {
            ptype: p.ptype.clone(),
            voltage: p.voltage.clone(),
            env: p.env.clone(),
            ..State::default()
        })
        .collect();
    let mut frontier: Vec<usize> = Vec::new();
    for (i, p) in part.potentials.iter().enumerate() {
        let root = p.cps.iter().any(|cp| {
            if let Some(ip) = graph.ips.get(&cp.function) {
                return ip.external;
            }
            !is_ip(cp.function)
                && q.get::<FunctionCps>(cp.function)
                    .iter()
                    .any(|c| c.idx == cp.idx && c.source_output)
        });
        if root {
            st[i].distance = Some(0);
            frontier.push(i);
        }
    }

    // BFS, one distance level at a time; within a level the pairs go in (function, a, b) order, so the
    // result does not depend on record order.
    let mut level = 0u32;
    while !frontier.is_empty() {
        let mut cands: Vec<(usize, usize)> = Vec::new();
        for &u in &frontier {
            for &pi in &adj[u] {
                let (d, _, _) = pairs[pi].from(u);
                if st[d].distance.is_none() {
                    cands.push((pi, u));
                }
            }
        }
        cands.sort_unstable();
        let mut next: Vec<usize> = Vec::new();
        for (pi, u) in cands {
            let pair = &pairs[pi];
            let (d, from_idx, to_idx) = pair.from(u);
            if st[d].distance.is_some() {
                continue;
            }
            let up = st[u].clone();
            let def = fndef_of(q, pair.function);
            let protection = def
                .as_ref()
                .is_some_and(|def| def.item.category.0.as_str().starts_with("Protection"));
            let switches_n = def.as_ref().is_some_and(|def| def.item.flags.switches_n_with_l);
            let sets_switched = pair.opens && !protection && !switches_n;
            let own = &part.potentials[d];
            st[d] = State {
                distance: Some(level.saturating_add(1)),
                upstream: Some(Upstream {
                    function: pair.function,
                    from_idx,
                    to_idx,
                    from: part.potentials[u].id,
                }),
                ptype: inherit(&own.ptype, &up.ptype, pair.function),
                voltage: inherit(&own.voltage, &up.voltage, pair.function),
                env: inherit(&own.env, &up.env, pair.function),
                switched: up.switched || sets_switched,
                // The nearest upstream function whose opening pair switches it.
                switched_by: if sets_switched {
                    Some(pair.function)
                } else {
                    up.switched_by
                },
                protected_by: if protection {
                    Some(pair.function)
                } else {
                    up.protected_by
                },
            };
            next.push(d);
        }
        frontier = next;
        level = level.saturating_add(1);
    }

    // Loads (review decision D4): the consuming functions on a potential. Not a load: a function with a
    // pass-through pair touching the potential (the upstream feeder, an onward switch or protective
    // device, a bypassed contact), a function feeding the potential through a source-output CP, an
    // interruption point, a terminal.
    let mut passing: Vec<BTreeSet<Id<Function>>> = vec![BTreeSet::new(); n];
    for e in graph.edges.iter().filter(|e| e.level == Level::Link) {
        let EdgeKind::PassThrough { function, .. } = e.kind else {
            continue;
        };
        for node in [e.a, e.b] {
            if let Some(pi) = part.of_node(node) {
                passing[pi].insert(function);
            }
        }
    }
    let mut entries = BTreeMap::new();
    for (i, p) in part.potentials.iter().enumerate() {
        let s = &st[i];
        let feeds: BTreeSet<Id<Function>> = p
            .cps
            .iter()
            .filter(|cp| {
                !is_ip(cp.function)
                    && q.get::<FunctionCps>(cp.function)
                        .iter()
                        .any(|c| c.idx == cp.idx && c.source_output)
            })
            .map(|cp| cp.function)
            .collect();
        let mut loads: Vec<Load> = p
            .cps
            .iter()
            .map(|cp| cp.function)
            .filter(|f| !is_ip(*f) && !passing[i].contains(f) && !feeds.contains(f))
            .filter(|&f| !fndef_of(q, f).is_some_and(|d| d.item.contact_kind == ContactKind::Terminal))
            .map(|function| Load {
                function,
                protected_by: s.protected_by,
            })
            .collect();
        loads.sort();
        loads.dedup();
        entries.insert(
            p.id,
            FlowEntry {
                potential: p.id,
                distance: s.distance,
                upstream: s.upstream,
                ptype: s.ptype.clone(),
                voltage: s.voltage.clone(),
                env: s.env.clone(),
                switched: s.switched,
                switched_by: s.switched_by,
                protected_by: s.protected_by,
                loads,
            },
        );
    }

    // Conflicts: a pair whose two sides carry different types of their own (doc 08 §3.6).
    let mut notes: Vec<DeriveNote> = Vec::new();
    for pair in &pairs {
        let (a, b) = (&part.potentials[pair.pa], &part.potentials[pair.pb]);
        if let (Some(ta), Some(tb)) = (&a.ptype, &b.ptype)
            && ta.value != tb.value
        {
            notes.push(DeriveNote::Connectivity {
                code: "EW-POT-001",
                subject: format!("potential {} / {}", a.name.text, b.name.text),
                detail: format!(
                    "pass-through of function {} joins type {:?} and type {:?} without a converting function",
                    pair.function, ta.value, tb.value
                ),
            });
        }
    }
    notes.sort();
    notes.dedup();
    for note in &notes {
        q.note(note.clone());
    }
    Flow { entries, notes }
}

/// `potential_flow(potential)`: the potential's entry of [`FlowBfs`]; `None` for an unknown ID. Policy Cold:
/// a lookup into the memoised `potential_flow_bfs` (ADR-0002 amendment 4, decision log ID-221, ARCH §6.4).
#[derive(Debug)]
pub struct PotentialFlow;

impl Query for PotentialFlow {
    type Key = PotentialId;
    type Value = Option<FlowEntry>;
    const FAMILY: &'static str = "potential_flow";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &PotentialId) -> Arc<Option<FlowEntry>> {
        Arc::new(q.get::<FlowBfs>(()).entries.get(key).cloned())
    }
}
