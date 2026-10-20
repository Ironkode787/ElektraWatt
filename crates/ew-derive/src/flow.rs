// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Potential flow: `potential_flow(potential)` (ARCH §6.4, doc 08 §3.6).
//!
//! `potential_flow_bfs()` is one BFS over the whole partition, from the potentials that hold a source output
//! or an external point, over the pass-through pairs between two potentials. It reads only the partition and
//! the own attributes of the potentials (`potential_attrs`), and it is a **producer of parts**
//! ([`PartKey::Flow`]): `potential_flow(p)` (Cold) reads the entry of `p` only, so a re-run of the BFS reaches
//! the readers of the entries that changed. It keeps its value when neither the partition nor an attribute it
//! read changed; otherwise the BFS runs whole (WP-S1.07 part 2: the flow stays global). It is note-free: the
//! pass-through conflicts are values, reported with the potentials' names by `flow_notes()`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{Function, PotentialEnv, PotentialType};
use ew_units::Dec3;

use crate::Q;
use crate::heap::HeapBytes;
use crate::links::DeclaredLink;
use crate::memo_keys::PartKey;
use crate::names::{PotentialAttrsOf, PotentialAttrsV, PotentialNameOf};
use crate::note::DeriveNote;
use crate::potentials::{Attr, AttrSource, Partition, PotentialId, PotentialPartition};
use crate::query::{Changes, Policy, Query};

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

/// A pass-through pair whose two sides carry different types of their own (doc 08 §3.6).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct FlowConflict {
    /// The function of the pair.
    pub function: Id<Function>,
    /// The potential of the pair's first CP.
    pub a: PotentialId,
    /// The potential of its second CP.
    pub b: PotentialId,
    /// The own type of `a` (`Debug` text).
    pub a_type: String,
    /// The own type of `b` (`Debug` text).
    pub b_type: String,
}

/// The value of [`FlowBfs`]: one BFS over the whole partition. Two values are equal when their entries and
/// conflicts are (the inputs kept for the incremental check are not compared).
#[derive(Clone, Eq, Debug, Default)]
pub struct Flow {
    /// One entry per potential of the partition.
    pub entries: BTreeMap<PotentialId, FlowEntry>,
    /// The pass-through conflicts, in (function, potentials) order.
    pub conflicts: Vec<FlowConflict>,
    /// The partition the BFS ran over (the incremental check).
    partition: Option<Arc<Partition>>,
    /// The own attributes it read.
    attrs: BTreeMap<PotentialId, Arc<PotentialAttrsV>>,
}

impl PartialEq for Flow {
    fn eq(&self, other: &Flow) -> bool {
        self.entries == other.entries && self.conflicts == other.conflicts
    }
}

impl HeapBytes for Flow {
    fn heap_bytes(&self) -> usize {
        self.entries
            .values()
            .map(|e| {
                std::mem::size_of::<(PotentialId, FlowEntry)>()
                    + e.ptype.heap_bytes()
                    + e.voltage.heap_bytes()
                    + e.env.heap_bytes()
                    + e.loads.capacity() * std::mem::size_of::<Load>()
            })
            .sum::<usize>()
            + self.conflicts.len() * std::mem::size_of::<FlowConflict>()
            + self.attrs.len() * 2 * std::mem::size_of::<usize>()
    }
}

/// The flow BFS over the partition (doc 08 §3.6: "one BFS over the whole partition"). Policy Memo,
/// incremental, a producer of the parts [`PartKey::Flow`].
#[derive(Debug)]
pub struct FlowBfs;

impl Query for FlowBfs {
    type Key = ();
    type Value = Flow;
    const FAMILY: &'static str = "potential_flow_bfs";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _key: &()) -> Arc<Flow> {
        Arc::new(run_flow(q))
    }

    fn run_incremental(q: &Q, _: &(), previous: Option<(&Arc<Flow>, Changes<'_>)>) -> Arc<Flow> {
        let Some((prev, changes)) = previous else {
            return Arc::new(run_flow(q));
        };
        if let Some(via) = changes.via() {
            let mut same = true;
            for dep in via {
                if q.via_key::<PotentialPartition>(dep).is_some() {
                    let now = q.get::<PotentialPartition>(());
                    same &= prev
                        .partition
                        .as_ref()
                        .is_some_and(|old| Arc::ptr_eq(old, &now) || old.same_flow_inputs(&now));
                } else if let Some(p) = q.via_key::<PotentialAttrsOf>(dep) {
                    let now = q.get::<PotentialAttrsOf>(p);
                    same &= prev.attrs.get(&p).is_some_and(|old| Arc::ptr_eq(old, &now));
                } else {
                    same = false;
                }
            }
            if same {
                return Arc::clone(prev);
            }
        }
        let next = run_flow(q);
        let keys: BTreeSet<PotentialId> = prev.entries.keys().chain(next.entries.keys()).copied().collect();
        q.changed_parts(
            keys.into_iter()
                .filter(|p| prev.entries.get(p) != next.entries.get(p))
                .map(PartKey::Flow),
        );
        Arc::new(next)
    }
}

/// One pass-through pair between two distinct potentials (indices into the potential list).
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

fn run_flow(q: &Q) -> Flow {
    let part = q.get::<PotentialPartition>(());
    let ip = part.ip_pairing();
    let potentials: Vec<PotentialId> = part.potentials().map(|p| p.id).collect();
    let index: BTreeMap<PotentialId, usize> = potentials.iter().enumerate().map(|(i, p)| (*p, i)).collect();
    let n = potentials.len();
    let attrs: Vec<Arc<PotentialAttrsV>> = potentials.iter().map(|p| q.get::<PotentialAttrsOf>(*p)).collect();
    let pot_of = |at: EndpointKey| part.potential_at(at).and_then(|p| index.get(&p.id).copied());
    let links: BTreeMap<Id<Function>, &crate::links::FunctionLinks> = part.functions().collect();

    // Pass-through pairs between two distinct potentials, in (function, a, b) order. A pair inside one
    // potential is a bypassed contact (doc 08 §12 item 2). `passing` holds every pair that touches a potential.
    let mut pairs: Vec<Pair> = Vec::new();
    let mut passing: Vec<BTreeSet<Id<Function>>> = vec![BTreeSet::new(); n];
    for (function, l) in &links {
        for d in &l.declared {
            let DeclaredLink::PassThrough { a, b, opens } = *d else {
                continue;
            };
            let (ea, eb) = (EndpointKey::Cp(*function, a), EndpointKey::Cp(*function, b));
            if !(part.is_node(ea) && part.is_node(eb)) {
                continue;
            }
            let (pa, pb) = (pot_of(ea), pot_of(eb));
            for pi in [pa, pb].into_iter().flatten() {
                passing[pi].insert(*function);
            }
            let (Some(pa), Some(pb)) = (pa, pb) else { continue };
            if pa != pb {
                pairs.push(Pair {
                    function: *function,
                    a,
                    b,
                    opens,
                    pa,
                    pb,
                });
            }
        }
    }
    pairs.sort_by_key(|p| (p.function, p.a, p.b, p.pa, p.pb));
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, p) in pairs.iter().enumerate() {
        adj[p.pa].push(i);
        adj[p.pb].push(i);
    }
    let is_ip = |f: Id<Function>| ip.is_ip(f);
    let source_output =
        |f: Id<Function>, idx: u16| links.get(&f).is_some_and(|l| l.source_outputs.contains(&idx));

    // Roots: potentials holding an external point or a source output CP.
    let mut st: Vec<State> = attrs
        .iter()
        .map(|a| State {
            ptype: a.ptype.clone(),
            voltage: a.voltage.clone(),
            env: a.env.clone(),
            ..State::default()
        })
        .collect();
    let mut frontier: Vec<usize> = Vec::new();
    for (i, id) in potentials.iter().enumerate() {
        let Some(p) = part.get(*id) else { continue };
        let root = p.cps.iter().any(|cp| {
            if let Some(info) = ip.ips.get(&cp.function) {
                return info.external;
            }
            source_output(cp.function, cp.idx)
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
            let l = links.get(&pair.function);
            let protection = l.is_some_and(|l| l.protection);
            let switches_n = l.is_some_and(|l| l.switches_n);
            let sets_switched = pair.opens && !protection && !switches_n;
            let own = &attrs[d];
            st[d] = State {
                distance: Some(level.saturating_add(1)),
                upstream: Some(Upstream {
                    function: pair.function,
                    from_idx,
                    to_idx,
                    from: potentials[u],
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
    let mut entries = BTreeMap::new();
    for (i, id) in potentials.iter().enumerate() {
        let Some(p) = part.get(*id) else { continue };
        let s = &st[i];
        let feeds: BTreeSet<Id<Function>> = p
            .cps
            .iter()
            .filter(|cp| !is_ip(cp.function) && source_output(cp.function, cp.idx))
            .map(|cp| cp.function)
            .collect();
        let mut loads: Vec<Load> = p
            .cps
            .iter()
            .map(|cp| cp.function)
            .filter(|f| !is_ip(*f) && !passing[i].contains(f) && !feeds.contains(f))
            .filter(|f| !links.get(f).is_some_and(|l| l.terminal_contact))
            .map(|function| Load {
                function,
                protected_by: s.protected_by,
            })
            .collect();
        loads.sort();
        loads.dedup();
        entries.insert(
            *id,
            FlowEntry {
                potential: *id,
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
    let mut conflicts: Vec<FlowConflict> = Vec::new();
    for pair in &pairs {
        let (a, b) = (&attrs[pair.pa], &attrs[pair.pb]);
        if let (Some(ta), Some(tb)) = (&a.ptype, &b.ptype)
            && ta.value != tb.value
        {
            conflicts.push(FlowConflict {
                function: pair.function,
                a: potentials[pair.pa],
                b: potentials[pair.pb],
                a_type: format!("{:?}", ta.value),
                b_type: format!("{:?}", tb.value),
            });
        }
    }
    conflicts.sort();
    conflicts.dedup();
    Flow {
        entries,
        conflicts,
        attrs: potentials.iter().copied().zip(attrs).collect(),
        partition: Some(part),
    }
}

/// `flow_notes()`: the EW-POT-001 notes of the pass-through conflicts of the flow, naming the two potentials
/// (doc 08 §3.6). Policy Memo; `potential_flow` reads it as a cut-off point, so its notes reach the flow's
/// readers.
#[derive(Debug)]
pub struct FlowNotes;

impl Query for FlowNotes {
    type Key = ();
    type Value = Vec<DeriveNote>;
    const FAMILY: &'static str = "flow_notes";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Vec<DeriveNote>> {
        let flow = q.get::<FlowBfs>(());
        let mut notes: Vec<DeriveNote> = flow
            .conflicts
            .iter()
            .map(|c| {
                let a = q.get::<PotentialNameOf>(c.a);
                let b = q.get::<PotentialNameOf>(c.b);
                DeriveNote::Connectivity {
                    code: "EW-POT-001",
                    subject: format!("potential {} / {}", a.name.text, b.name.text),
                    detail: format!(
                        "pass-through of function {} joins type {} and type {} without a converting function",
                        c.function, c.a_type, c.b_type
                    ),
                }
            })
            .collect();
        notes.sort();
        notes.dedup();
        for n in &notes {
            q.note(n.clone());
        }
        Arc::new(notes)
    }
}

/// `potential_flow(potential)`: the potential's entry of [`FlowBfs`]; `None` for an unknown ID. Policy Cold:
/// a lookup into the memoised `potential_flow_bfs` (ADR-0002 amendment 4, decision log ID-221, ARCH §6.4),
/// depending on the potential's entry only; it surfaces the flow's notes (`flow_notes`).
#[derive(Debug)]
pub struct PotentialFlow;

impl Query for PotentialFlow {
    type Key = PotentialId;
    type Value = Option<FlowEntry>;
    const FAMILY: &'static str = "potential_flow";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &PotentialId) -> Arc<Option<FlowEntry>> {
        let flow = q.get_part::<FlowBfs>((), PartKey::Flow(*key));
        let _ = q.get_value_part::<FlowNotes>(());
        Arc::new(flow.entries.get(key).cloned())
    }
}
