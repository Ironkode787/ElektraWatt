// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The potential partition: `potential_partition()` (ARCH §6.1, §6.4; doc 08 §3.3–§3.4; ADR-0002 amendments 4
//! and 5; WP-S1.07 part 2).
//!
//! # The value: topology only, per connected component
//!
//! The stitch graph's three snapshots (doc 08 §3.3): after the net-level edges (nets), after the
//! signal-level edges (signals) and after the potential-level edges (the connected components). Bonds and
//! pass-through pairs never join. Every component is stored on its own ([`Component`]): its member endpoints,
//! its signals with their nets, and its potential when it is one. With them the value keeps the incidence the
//! incremental maintenance needs (which component holds an endpoint, the page-local nets of every page, the
//! links of every function with a CP in the graph, and the index families it was built from), all in
//! persistent maps, so a new version shares everything an edit did not touch and two versions compare by the
//! parts they do not share.
//!
//! - **Which components are potentials.** A component holding a potential definition, or a joining
//!   connection and at least one CP that is not an interruption point.
//! - **Identity** (doc 08 §3.4): [`PotentialId::Def`] with the minimum definition ID when the potential has
//!   definitions, else [`PotentialId::Cp`] with the minimum member CP ID.
//! - **Names and attributes are not here** (ADR-0002 amendment 4): `potential_name(p)` and `potential_attrs(p)`
//!   derive them per potential, so a rename, a renumber or a move never changes the topology.
//!
//! # The producer
//!
//! The partition is a producer of parts (ADR-0002 amendment 5): a reader of one potential's topology
//! ([`PartKey::Potential`]), of the potential of one endpoint ([`PartKey::Endpoint`]) or of the bonds
//! ([`PartKey::Bonds`], [`PartKey::BondsOf`]) is invalidated only when an execution declares that part changed.
//!
//! - **Cold** (`run`, the oracle's reference) is the global algorithm of cycle 7: the whole stitch graph, one
//!   union-find over its nodes, the components shaped into the value.
//! - **Incremental** (`run_incremental`, `component.rs`): the touched inputs (the pages whose local nets
//!   changed, the functions whose links changed, the index families) give seed endpoints; the components that
//!   held them are removed and the components through the seeds are found again over the current inputs,
//!   absorbing every component a new edge reaches (an added connection unions). Only those components are
//!   recomputed, from the per-page nets; the declared parts are the potentials and endpoints whose topology
//!   changed. Anything the hook cannot attribute falls back to the cold algorithm and declares the difference.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::CircuitClass;
use ew_schema::{
    Bond, BondKind, Connection, Function, Junction, Page, PotentialDef, PotentialEnv, PotentialType,
};
use ew_units::Dec3;
use imbl::OrdMap;

use crate::heap::HeapBytes;
use crate::links::{BondPaths, BondPathsQ, FunctionLinks};
use crate::memo_keys::PartKey;
use crate::nets::{CpRef, Dsu, LocalNets, NetId};
use crate::query::{Changes, Policy, Query};
use crate::stitch::{Level, StitchBuild, StitchInputs, build};
use crate::{Cp, Q};

/// The stable identity of a potential (doc 08 §3.4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PotentialId {
    /// The minimum ID of its definitions.
    Def(Id<PotentialDef>),
    /// The minimum ID of its member CPs (no definition).
    Cp(Id<Cp>),
}

/// Where a potential's name comes from (doc 08 §3.5).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NameSource {
    /// A definition.
    Def(Id<PotentialDef>),
    /// A source interruption point.
    SourceIp(Id<Function>),
    /// The supply (doc 08 §3.5 step 2b, WP-S1.16 D1): `<root>/<tag>:<designation>` of the pass-through pair of
    /// `via` that enters the potential from upstream.
    Supply {
        /// The function whose pass-through pair enters the potential.
        via: Id<Function>,
    },
    /// The fallback `<tag>:<designation>` of a member CP.
    Fallback(CpRef),
}

/// The canonical name of a potential.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PotentialName {
    /// The text.
    pub text: String,
    /// Its source.
    pub source: NameSource,
}

/// Where an attribute of a potential comes from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum AttrSource {
    /// A definition sets it.
    Def(Id<PotentialDef>),
    /// The `potential_role` of a member CP.
    Role(CpRef),
    /// Propagated over a pass-through pair of `function` from the upstream potential (flow).
    Propagated(Id<Function>),
}

/// An attribute value with its source.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Attr<T> {
    /// The value.
    pub value: T,
    /// Its source.
    pub source: AttrSource,
}

/// The own attributes of a potential (doc 03 §9.4), the value of `potential_attrs(p)`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PotentialAttrs {
    /// Type from a definition or a member role.
    pub ptype: Option<Attr<PotentialType>>,
    /// Voltage from a definition.
    pub voltage: Option<Attr<Dec3>>,
    /// AC/DC, frequency and earthing from a definition.
    pub env: Option<Attr<PotentialEnv>>,
    /// Circuit class from a definition.
    pub circuit_class: Option<Attr<CircuitClass>>,
}

/// One potential's topology.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Potential {
    /// The identity.
    pub id: PotentialId,
    /// Its definitions, in ID order.
    pub defs: Vec<Id<PotentialDef>>,
    /// The member CPs (interruption points included), in CP order.
    pub cps: Vec<CpRef>,
    /// The member junctions, in ID order.
    pub junctions: Vec<Id<Junction>>,
    /// The signals (minimum member CP ID each), ascending.
    pub signals: Vec<NetId>,
    /// The nets, ascending.
    pub nets: Vec<NetId>,
    /// The interruption points on the potential, in ID order.
    pub ips: Vec<Id<Function>>,
}

/// One signal: nets joined by shorts (doc 08 §3.3). Its name (the first interruption-point name inside, doc
/// 08 §3.5) is derived with the potential's names (`potential_name`).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Signal {
    /// The minimum member CP ID.
    pub id: NetId,
    /// Its nets, ascending.
    pub nets: Vec<NetId>,
    /// The interruption points with a CP in the signal, in ID order (its name is the first of them in page
    /// order).
    pub ips: Vec<Id<Function>>,
}

/// A bond between two potentials (doc 03 §9.6).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BondLink {
    /// The bond.
    pub bond: Id<Bond>,
    /// Its kind.
    pub kind: BondKind,
    /// The potential at `a`.
    pub a: Option<PotentialId>,
    /// The potential at `b`.
    pub b: Option<PotentialId>,
    /// The via connection.
    pub via: Option<Id<Connection>>,
    /// The interruption points on the via path, in ID order.
    pub path_ips: Vec<Id<Function>>,
    /// Their distinct names, natural order: the bond's names ("PE-0VDC" names B1).
    pub names: Vec<String>,
}

/// One connected component of the stitch graph: a potential, or the points of no potential (an unconnected
/// interruption point, a lone junction).
#[derive(Clone, Eq, Debug)]
pub struct Component {
    /// The member endpoints, in order.
    pub members: Vec<EndpointKey>,
    /// The signals, in ID order.
    pub signals: Vec<Signal>,
    /// The potential, when the component is one.
    pub potential: Option<Potential>,
    /// A 128-bit fingerprint of the three fields above (a fast inequality check).
    fp: u128,
    /// The bytes of the component as an entry of the partition (`Component::entry_bytes`), counted once when it
    /// is built: a function of its content.
    bytes: usize,
}

impl PartialEq for Component {
    fn eq(&self, other: &Component) -> bool {
        self.fp == other.fp
            && self.members == other.members
            && self.signals == other.signals
            && self.potential == other.potential
    }
}

#[cfg(test)]
thread_local! {
    /// The components whose bytes were counted on this thread (tests: an incremental commit counts the
    /// components it rebuilt, not every component).
    pub(crate) static ENTRY_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// A deterministic 128-bit FNV-1a hasher (no randomness: fingerprints only compare values of one process).
struct Fnv128(u128);

impl Hasher for Fnv128 {
    fn write(&mut self, bytes: &[u8]) {
        const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
        for b in bytes {
            self.0 ^= u128::from(*b);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    fn finish(&self) -> u64 {
        self.0 as u64
    }
}

impl Component {
    /// A component of these members, signals and potential.
    pub(crate) fn new(
        members: Vec<EndpointKey>,
        signals: Vec<Signal>,
        potential: Option<Potential>,
    ) -> Component {
        let mut h = Fnv128(0x6c62_272e_07bb_0142_62b8_2175_6295_c58d);
        members.hash(&mut h);
        signals.hash(&mut h);
        potential.hash(&mut h);
        let mut c = Component {
            members,
            signals,
            potential,
            fp: h.0,
            bytes: 0,
        };
        c.bytes = c.entry_bytes();
        c
    }

    /// The bytes of the component as an entry of the partition's component map: the entry, the component and
    /// its vectors by length (a function of the content, so that a component built incrementally counts as the
    /// same one built cold).
    fn entry_bytes(&self) -> usize {
        use std::mem::size_of;
        #[cfg(test)]
        ENTRY_BYTES.with(|c| c.set(c.get() + 1));
        let signal = |s: &Signal| {
            size_of::<Signal>() + s.nets.len() * size_of::<NetId>() + s.ips.len() * size_of::<Id<Function>>()
        };
        let potential = self.potential.as_ref().map_or(0, |p| {
            p.defs.len() * size_of::<Id<PotentialDef>>()
                + p.cps.len() * size_of::<CpRef>()
                + p.junctions.len() * size_of::<Id<Junction>>()
                + (p.signals.len() + p.nets.len()) * size_of::<NetId>()
                + p.ips.len() * size_of::<Id<Function>>()
        });
        map_entry::<EndpointKey, Arc<Component>>()
            + size_of::<Component>()
            + self.members.len() * size_of::<EndpointKey>()
            + self.signals.iter().map(signal).sum::<usize>()
            + potential
    }

    /// The key of the component in the partition: its first member.
    pub fn key(&self) -> EndpointKey {
        self.members[0]
    }
}

/// The pages whose local nets hold an endpoint, ascending.
pub(crate) type PageList = Vec<Id<Page>>;

/// The value of [`PotentialPartition`] (see the module documentation).
///
/// It keeps a running estimate of its bytes (the components' bytes, the page-list items, the bonds' bytes),
/// updated where the incremental maintenance changes a component, a page list or the bonds, so that its byte
/// count is O(1) (review cycle 10, finding 2). Two partitions are equal when their topology is.
#[derive(Clone, Debug)]
pub struct Partition {
    /// The components by key (their first member).
    pub(crate) comps: OrdMap<EndpointKey, Arc<Component>>,
    /// The component of every stitch-node endpoint.
    pub(crate) comp_of: OrdMap<EndpointKey, EndpointKey>,
    /// The pages whose local nets hold an endpoint (only endpoints of page-local nets).
    pub(crate) pages_of: OrdMap<EndpointKey, PageList>,
    /// The component of every potential.
    pub(crate) by_id: OrdMap<PotentialId, EndpointKey>,
    /// The component of every signal.
    pub(crate) signal_of: OrdMap<NetId, EndpointKey>,
    /// The page-local nets of every page.
    pub(crate) pages: OrdMap<Id<Page>, Arc<LocalNets>>,
    /// The links of every function with a CP in the graph.
    pub(crate) links: OrdMap<Id<Function>, Arc<FunctionLinks>>,
    /// The index families it was built from.
    pub(crate) inputs: StitchInputs,
    /// The bond paths it was built from.
    pub(crate) bond_paths: Arc<BondPaths>,
    /// The bonds in ID order.
    pub(crate) bonds: Vec<BondLink>,
    /// The bytes of the component entries: the sum of their `Component::entry_bytes`.
    comps_bytes: usize,
    /// The page IDs over all page lists of `pages_of`.
    pages_items: usize,
    /// The bytes of `bonds` (`bonds_bytes`).
    bonds_bytes: usize,
}

impl PartialEq for Partition {
    fn eq(&self, other: &Partition) -> bool {
        // The byte estimates are functions of the fields compared.
        (self.comps.ptr_eq(&other.comps) || self.comps == other.comps)
            && (self.comp_of.ptr_eq(&other.comp_of) || self.comp_of == other.comp_of)
            && (self.pages_of.ptr_eq(&other.pages_of) || self.pages_of == other.pages_of)
            && (self.by_id.ptr_eq(&other.by_id) || self.by_id == other.by_id)
            && (self.signal_of.ptr_eq(&other.signal_of) || self.signal_of == other.signal_of)
            && (self.pages.ptr_eq(&other.pages) || self.pages == other.pages)
            && (self.links.ptr_eq(&other.links) || self.links == other.links)
            && self.inputs == other.inputs
            && self.bond_paths == other.bond_paths
            && self.bonds == other.bonds
    }
}

impl Eq for Partition {}

/// The estimated bytes of one entry of a persistent map: key, value and two words of node overhead.
const fn map_entry<K, V>() -> usize {
    std::mem::size_of::<K>() + std::mem::size_of::<V>() + 16
}

/// The bytes of a bond list, by length (a function of the content).
fn bonds_bytes(bonds: &[BondLink]) -> usize {
    use std::mem::size_of;
    bonds
        .iter()
        .map(|b| {
            size_of::<BondLink>()
                + b.path_ips.len() * size_of::<Id<Function>>()
                + b.names
                    .iter()
                    .map(|n| size_of::<String>() + n.len())
                    .sum::<usize>()
        })
        .sum()
}

impl Partition {
    /// Inserts a component (replacing none: its key is free), keeping the byte estimate.
    pub(crate) fn insert_comp(&mut self, key: EndpointKey, comp: Arc<Component>) {
        self.comps_bytes = self.comps_bytes.saturating_add(comp.bytes);
        if let Some(old) = self.comps.insert(key, comp) {
            self.comps_bytes = self.comps_bytes.saturating_sub(old.bytes);
        }
    }

    /// Removes a component, keeping the byte estimate.
    pub(crate) fn remove_comp(&mut self, key: &EndpointKey) -> Option<Arc<Component>> {
        let comp = self.comps.remove(key)?;
        self.comps_bytes = self.comps_bytes.saturating_sub(comp.bytes);
        Some(comp)
    }

    /// Sets the page list of `e` (`None` or empty: removes it), keeping the byte estimate.
    pub(crate) fn set_pages_of(&mut self, e: EndpointKey, list: Option<PageList>) {
        let old = self.pages_of.get(&e).map_or(0, Vec::len);
        self.pages_items = self.pages_items.saturating_sub(old);
        match list.filter(|l| !l.is_empty()) {
            Some(l) => {
                self.pages_items = self.pages_items.saturating_add(l.len());
                self.pages_of.insert(e, l);
            }
            None => {
                self.pages_of.remove(&e);
            }
        }
    }

    /// Sets the bonds, keeping the byte estimate.
    pub(crate) fn set_bonds(&mut self, bonds: Vec<BondLink>) {
        self.bonds_bytes = bonds_bytes(&bonds);
        self.bonds = bonds;
    }

    /// The byte estimate from the running counts and the maps' lengths: O(1).
    fn estimated_heap(&self) -> usize {
        use std::mem::size_of;
        self.comps_bytes
            + self.comp_of.len() * map_entry::<EndpointKey, EndpointKey>()
            + self.pages_of.len() * map_entry::<EndpointKey, PageList>()
            + self.pages_items * size_of::<Id<Page>>()
            + self.by_id.len() * map_entry::<PotentialId, EndpointKey>()
            + self.signal_of.len() * map_entry::<NetId, EndpointKey>()
            // The pages and links are shared with their families' memos and counted by those.
            + self.pages.len() * map_entry::<Id<Page>, Arc<LocalNets>>()
            + self.links.len() * map_entry::<Id<Function>, Arc<FunctionLinks>>()
            + self.bonds_bytes
    }

    /// The running counts recomputed by a walk of the value (the cold build's start, and the tests' check of
    /// the incremental maintenance). The components' bytes are the ones each counted when it was built.
    fn walked_counts(&self) -> (usize, usize, usize) {
        (
            self.comps.values().map(|c| c.bytes).sum(),
            self.pages_of.values().map(Vec::len).sum(),
            bonds_bytes(&self.bonds),
        )
    }

    /// The running counts as kept, and as a walk recomputes them (tests).
    #[cfg(test)]
    pub(crate) fn counts(&self) -> ((usize, usize, usize), (usize, usize, usize)) {
        (
            (self.comps_bytes, self.pages_items, self.bonds_bytes),
            self.walked_counts(),
        )
    }
    /// The potentials in ID order.
    pub fn potentials(&self) -> impl Iterator<Item = &Potential> + '_ {
        self.by_id
            .values()
            .filter_map(|c| self.comps.get(c)?.potential.as_ref())
    }

    /// The number of potentials.
    pub fn potential_count(&self) -> usize {
        self.by_id.len()
    }

    /// The potential with this ID.
    pub fn get(&self, id: PotentialId) -> Option<&Potential> {
        let c = self.by_id.get(&id)?;
        self.comps.get(c)?.potential.as_ref()
    }

    /// The component of a potential: what part [`PartKey::Potential`] covers (the potential's topology and its
    /// signals with their interruption points).
    pub fn component_of(&self, id: PotentialId) -> Option<&Component> {
        let c = self.by_id.get(&id)?;
        self.comps.get(c).map(Arc::as_ref)
    }

    /// The potential of a stitch-node endpoint (a CP or junction in the graph); `None` for an endpoint of no
    /// potential.
    pub fn potential_at(&self, at: EndpointKey) -> Option<&Potential> {
        self.component_at(at)?.potential.as_ref()
    }

    /// The component of a stitch-node endpoint.
    pub fn component_at(&self, at: EndpointKey) -> Option<&Component> {
        let c = self.comp_of.get(&at)?;
        self.comps.get(c).map(Arc::as_ref)
    }

    /// Whether `at` is a node of the stitch graph (in a page-local net, or a page-less point).
    pub fn is_node(&self, at: EndpointKey) -> bool {
        self.comp_of.contains_key(&at)
    }

    /// The signals in ID order.
    pub fn signals(&self) -> impl Iterator<Item = &Signal> + '_ {
        self.signal_of.iter().filter_map(|(id, c)| {
            let comp = self.comps.get(c)?;
            comp.signals.iter().find(|s| s.id == *id)
        })
    }

    /// The number of signals.
    pub fn signal_count(&self) -> usize {
        self.signal_of.len()
    }

    /// The components in key order.
    pub fn components(&self) -> impl Iterator<Item = &Component> + '_ {
        self.comps.values().map(Arc::as_ref)
    }

    /// The bonds in ID order.
    pub fn bonds(&self) -> &[BondLink] {
        &self.bonds
    }

    /// The links of every function with a CP in the graph, in function ID order (the pass-through pairs and the
    /// flow data of the potential flow).
    pub fn functions(&self) -> impl Iterator<Item = (Id<Function>, &FunctionLinks)> + '_ {
        self.links.iter().map(|(f, l)| (*f, l.as_ref()))
    }

    /// Whether the flow BFS reads the same from `other`: the components, the function links and the
    /// interruption points (shared persistent maps, or equal ones).
    pub(crate) fn same_flow_inputs(&self, other: &Partition) -> bool {
        (self.comps.ptr_eq(&other.comps) || self.comps == other.comps)
            && (self.comp_of.ptr_eq(&other.comp_of) || self.comp_of == other.comp_of)
            && (self.by_id.ptr_eq(&other.by_id) || self.by_id == other.by_id)
            && (self.links.ptr_eq(&other.links) || self.links == other.links)
            && (Arc::ptr_eq(&self.inputs.ip, &other.inputs.ip) || self.inputs.ip.ips == other.inputs.ip.ips)
    }

    /// The interruption points and their pairing the partition was built from.
    pub fn ip_pairing(&self) -> &crate::ip::IpPairing {
        &self.inputs.ip
    }
}

impl HeapBytes for Potential {
    fn heap_bytes(&self) -> usize {
        self.defs.heap_bytes()
            + self.cps.heap_bytes()
            + self.junctions.heap_bytes()
            + self.signals.heap_bytes()
            + self.nets.heap_bytes()
            + self.ips.heap_bytes()
    }
}

impl HeapBytes for Signal {
    fn heap_bytes(&self) -> usize {
        self.nets.heap_bytes() + self.ips.heap_bytes()
    }
}

impl HeapBytes for Component {
    fn heap_bytes(&self) -> usize {
        self.members.heap_bytes() + self.signals.heap_bytes() + self.potential.heap_bytes()
    }
}

impl HeapBytes for Partition {
    /// The running estimate (`Partition::estimated_heap`): O(1), whatever the size of the partition.
    fn heap_bytes(&self) -> usize {
        self.estimated_heap()
    }
}

/// `potential_partition()`: see the module documentation. Policy Memo, incremental, a producer of the parts
/// [`PartKey::Potential`], [`PartKey::Endpoint`], [`PartKey::Bonds`] and [`PartKey::BondsOf`].
#[derive(Debug)]
pub struct PotentialPartition;

impl Query for PotentialPartition {
    type Key = ();
    type Value = Partition;
    const FAMILY: &'static str = "potential_partition";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;
    const PRODUCER: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Partition> {
        Arc::new(cold(q))
    }

    fn run_incremental(q: &Q, _: &(), previous: Option<(&Arc<Partition>, Changes<'_>)>) -> Arc<Partition> {
        let Some((prev, changes)) = previous else {
            return Arc::new(cold(q));
        };
        if let Some((next, parts)) = crate::component::update(q, prev, changes) {
            q.changed_parts(parts);
            return Arc::new(next);
        }
        // The cold algorithm reads everything it relies on: nothing carried stays a dependency.
        q.drop_carried();
        let next = cold(q);
        q.changed_parts(difference(prev, &next));
        Arc::new(next)
    }
}

/// The potential of an endpoint, read as part [`PartKey::Endpoint`] of the partition: the reader depends on
/// which potential `at` belongs to, not on that potential's topology.
pub(crate) fn potential_id_at(q: &Q, at: EndpointKey) -> Option<PotentialId> {
    q.get_part::<PotentialPartition>((), PartKey::Endpoint(at))
        .potential_at(at)
        .map(|p| p.id)
}

/// The partition, read as part [`PartKey::Potential`] of `id`: the caller reads the topology of `id` only
/// (`Partition::get`, `Partition::component_of`), so a topology edit elsewhere does not reach it (FT-236).
pub fn potential_part(q: &Q, id: PotentialId) -> Arc<Partition> {
    q.get_part::<PotentialPartition>((), PartKey::Potential(id))
}

/// The bonds with an end on the potential `id`, in bond ID order, read as the parts [`PartKey::Bonds`] (the bond
/// records) and [`PartKey::BondsOf`] of `id` (which bonds end on it) instead of the whole partition (FT-236): a
/// topology change re-runs only the readers of the potentials it touches.
pub fn bonds_of_potential(q: &Q, id: PotentialId) -> Vec<BondLink> {
    let _ = q.get_part::<PotentialPartition>((), PartKey::Bonds);
    q.get_part::<PotentialPartition>((), PartKey::BondsOf(id))
        .bonds()
        .iter()
        .filter(|b| b.a == Some(id) || b.b == Some(id))
        .cloned()
        .collect()
}

/// The bonds between potentials (doc 03 §9.6), in bond ID order.
pub(crate) fn bond_links(
    inputs: &StitchInputs,
    paths: &BondPaths,
    potential_at: impl Fn(EndpointKey) -> Option<PotentialId>,
) -> Vec<BondLink> {
    inputs
        .bonds
        .iter()
        .map(|(id, b)| {
            let path = paths.paths.get(id);
            BondLink {
                bond: *id,
                kind: b.kind.clone(),
                a: b.a.and_then(&potential_at),
                b: b.b.and_then(&potential_at),
                via: b.via,
                path_ips: path.map(|p| p.ips.clone()).unwrap_or_default(),
                names: path.map(|p| p.names.clone()).unwrap_or_default(),
            }
        })
        .collect()
}

/// The cold partition: the stitch graph built whole, one union-find over its nodes (the cycle-7 algorithm),
/// shaped into components.
fn cold(q: &Q) -> Partition {
    let inputs = StitchInputs::read(q);
    let bond_paths = q.get::<BondPathsQ>(());
    let StitchBuild {
        graph: g,
        pages,
        links,
        ..
    } = build(q, &inputs);
    let n = g.nodes.len();
    let mut dsu: Dsu<usize> = Dsu::default();
    for i in 0..n {
        dsu.node(i);
    }
    let snapshot = |dsu: &mut Dsu<usize>| -> Vec<usize> { (0..n).map(|i| dsu.find(i)).collect() };
    let mut snaps: Vec<Vec<usize>> = Vec::new();
    for level in [Level::Net, Level::Signal, Level::Potential] {
        for e in g.edges.iter().filter(|e| e.level == level) {
            dsu.union(e.a, e.b);
        }
        snaps.push(snapshot(&mut dsu));
    }
    let mut connected: Vec<bool> = g.nodes.iter().map(|x| x.connected).collect();
    for e in &g.edges {
        if matches!(
            e.kind,
            crate::stitch::EdgeKind::PageLess(_) | crate::stitch::EdgeKind::Bridge(_)
        ) {
            connected[e.a] = true;
            connected[e.b] = true;
        }
    }
    let group = |snap: &[usize]| -> BTreeMap<usize, Vec<usize>> {
        let mut m: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, &r) in snap.iter().enumerate() {
            m.entry(r).or_default().push(i);
        }
        m
    };
    let ids_of = |nodes: &[usize]| -> Option<NetId> {
        NetId::of(nodes.iter().flat_map(|&i| g.nodes[i].members.iter().copied()))
    };
    let mut net_id_of_node: Vec<Option<NetId>> = vec![None; n];
    for nodes in group(&snaps[0]).values() {
        let id = ids_of(nodes);
        for &i in nodes {
            net_id_of_node[i] = id;
        }
    }
    let mut signal_of_node: Vec<Option<NetId>> = vec![None; n];
    let mut signal_nets: BTreeMap<usize, Signal> = BTreeMap::new();
    for (root, nodes) in group(&snaps[1]) {
        let Some(id) = ids_of(&nodes) else { continue };
        let mut nets: Vec<NetId> = nodes.iter().filter_map(|&i| net_id_of_node[i]).collect();
        nets.sort();
        nets.dedup();
        for &i in &nodes {
            signal_of_node[i] = Some(id);
        }
        let mut ips: Vec<Id<Function>> = nodes
            .iter()
            .flat_map(|&i| g.nodes[i].members.iter())
            .filter_map(|at| CpRef::of(*at))
            .map(|cp| cp.function)
            .filter(|f| inputs.ip.is_ip(*f))
            .collect();
        ips.sort();
        ips.dedup();
        signal_nets.insert(root, Signal { id, nets, ips });
    }
    let ip = &inputs.ip;
    let mut comps: OrdMap<EndpointKey, Arc<Component>> = OrdMap::new();
    let mut comp_of: OrdMap<EndpointKey, EndpointKey> = OrdMap::new();
    let mut by_id: OrdMap<PotentialId, EndpointKey> = OrdMap::new();
    let mut signal_of: OrdMap<NetId, EndpointKey> = OrdMap::new();
    for nodes in group(&snaps[2]).values() {
        let mut members: Vec<EndpointKey> = nodes
            .iter()
            .flat_map(|&i| g.nodes[i].members.iter().copied())
            .collect();
        members.sort();
        members.dedup();
        let mut signals: Vec<Signal> = nodes
            .iter()
            .map(|&i| snaps[1][i])
            .collect::<BTreeSet<usize>>()
            .into_iter()
            .filter_map(|r| signal_nets.get(&r).cloned())
            .collect();
        signals.sort_by_key(|s| s.id);
        let mut defs: Vec<Id<PotentialDef>> = members.iter().flat_map(|e| inputs.defs.at(*e)).collect();
        defs.sort();
        let cps: Vec<CpRef> = members.iter().filter_map(|e| CpRef::of(*e)).collect();
        let junctions: Vec<Id<Junction>> = members
            .iter()
            .filter_map(|e| match e {
                EndpointKey::Junction(j) => Some(*j),
                EndpointKey::Cp(..) => None,
            })
            .collect();
        let has_conn = nodes.iter().any(|&i| connected[i]);
        let has_device_cp = cps.iter().any(|cp| !ip.is_ip(cp.function));
        let potential = if defs.is_empty() && !(has_conn && has_device_cp) {
            None
        } else {
            cps.iter().map(|c| c.id()).min().map(|min_cp| {
                let id = defs
                    .first()
                    .map_or(PotentialId::Cp(min_cp), |d| PotentialId::Def(*d));
                let mut ips: Vec<Id<Function>> =
                    cps.iter().map(|c| c.function).filter(|f| ip.is_ip(*f)).collect();
                ips.dedup();
                let mut sigs: Vec<NetId> = nodes.iter().filter_map(|&i| signal_of_node[i]).collect();
                sigs.sort();
                sigs.dedup();
                let mut nets: Vec<NetId> = nodes.iter().filter_map(|&i| net_id_of_node[i]).collect();
                nets.sort();
                nets.dedup();
                Potential {
                    id,
                    defs,
                    cps,
                    junctions,
                    signals: sigs,
                    nets,
                    ips,
                }
            })
        };
        let comp = Component::new(members, signals, potential);
        let key = comp.key();
        for m in &comp.members {
            comp_of.insert(*m, key);
        }
        if let Some(p) = &comp.potential {
            by_id.insert(p.id, key);
        }
        for s in &comp.signals {
            signal_of.insert(s.id, key);
        }
        comps.insert(key, Arc::new(comp));
    }
    let mut pages_of: OrdMap<EndpointKey, PageList> = OrdMap::new();
    let mut page_map: OrdMap<Id<Page>, Arc<LocalNets>> = OrdMap::new();
    for (page, local) in pages {
        for net in &local.nets {
            for m in &net.members {
                pages_of.entry(m.at).or_default().push(page);
            }
        }
        page_map.insert(page, local);
    }
    let mut partition = Partition {
        comps,
        comp_of,
        pages_of,
        by_id,
        signal_of,
        pages: page_map,
        links: links.into_iter().collect(),
        inputs,
        bond_paths,
        bonds: Vec::new(),
        comps_bytes: 0,
        pages_items: 0,
        bonds_bytes: 0,
    };
    let bonds = bond_links(&partition.inputs, &partition.bond_paths, |e| {
        partition.potential_at(e).map(|p| p.id)
    });
    partition.set_bonds(bonds);
    // The cold build walks everything anyway: the running counts start from a walk.
    (partition.comps_bytes, partition.pages_items, _) = partition.walked_counts();
    partition
}

/// The parts that differ between two partitions (after a cold recompute that replaced an older value): every
/// potential whose component differs, every endpoint whose potential differs, and the bonds.
fn difference(old: &Partition, new: &Partition) -> Vec<PartKey> {
    let mut parts = Vec::new();
    let ids: BTreeSet<PotentialId> = old.by_id.keys().chain(new.by_id.keys()).copied().collect();
    for id in ids {
        if old.component_of(id) != new.component_of(id) {
            parts.push(PartKey::Potential(id));
        }
    }
    let ends: BTreeSet<EndpointKey> = old.comp_of.keys().chain(new.comp_of.keys()).copied().collect();
    for e in ends {
        let (a, b) = (
            old.potential_at(e).map(|p| p.id),
            new.potential_at(e).map(|p| p.id),
        );
        if a != b {
            parts.push(PartKey::Endpoint(e));
        }
    }
    parts.extend(bond_parts(&old.bonds, &new.bonds));
    parts
}

/// The bond parts that differ between two bond lists: [`PartKey::Bonds`] when a bond record (its existence,
/// kind, via connection or path) differs, [`PartKey::BondsOf`] for the potentials before and after at every
/// bond end whose potential differs.
pub(crate) fn bond_parts(old: &[BondLink], new: &[BondLink]) -> Vec<PartKey> {
    if old == new {
        return Vec::new();
    }
    let record = |b: &BondLink| (b.bond, b.kind.clone(), b.via, b.path_ips.clone(), b.names.clone());
    let mut parts: BTreeSet<PartKey> = BTreeSet::new();
    let (o, n): (BTreeMap<_, _>, BTreeMap<_, _>) = (
        old.iter().map(|b| (b.bond, b)).collect(),
        new.iter().map(|b| (b.bond, b)).collect(),
    );
    let ids: BTreeSet<Id<Bond>> = o.keys().chain(n.keys()).copied().collect();
    for id in ids {
        let (a, b) = (o.get(&id).copied(), n.get(&id).copied());
        if a.map(record) != b.map(record) {
            parts.insert(PartKey::Bonds);
        }
        if a.map(|x| (x.a, x.b)) != b.map(|x| (x.a, x.b)) {
            for p in [
                a.and_then(|x| x.a),
                a.and_then(|x| x.b),
                b.and_then(|x| x.a),
                b.and_then(|x| x.b),
            ]
            .into_iter()
            .flatten()
            {
                parts.insert(PartKey::BondsOf(p));
            }
        }
    }
    parts.into_iter().collect()
}

/// The pages of the drawn `Bus` connections of a potential (doc 08 §3.3; FT-105): a bus line joins its nets
/// but is no physical wire, so the pages a potential is drawn on are the pages of its wires and these. A
/// connection belongs to the potential of its first endpoint (both ends are in one potential).
pub fn bus_line_pages(q: &Q, partition: &Partition, potential: PotentialId) -> BTreeSet<Id<Page>> {
    let mut out = BTreeSet::new();
    for (_, c) in q.connections().iter() {
        let (ew_schema::ConnectionKind::Bus(_), Some(page), ew_schema::Endpoint::Cp(f, idx)) =
            (&c.kind, c.page, &c.a)
        else {
            continue;
        };
        if partition
            .potential_at(CpRef::new(*f, *idx).key())
            .is_some_and(|p| p.id == potential)
        {
            out.insert(page);
        }
    }
    out
}

/// The cables of the `Bus` connections of a potential (`BusLine.cable`, drawn or page-less; FR-CBL-008): a
/// bus line takes its colour and cross-section from its cable, not from the profile (doc 20 §7.3 "(cable)").
pub fn bus_line_cables(
    q: &Q,
    partition: &Partition,
    potential: PotentialId,
) -> BTreeSet<Id<ew_schema::Device>> {
    let mut out = BTreeSet::new();
    for (_, c) in q.connections().iter() {
        let (ew_schema::ConnectionKind::Bus(line), ew_schema::Endpoint::Cp(f, idx)) = (&c.kind, &c.a) else {
            continue;
        };
        let Some(cable) = line.cable else { continue };
        if partition
            .potential_at(CpRef::new(*f, *idx).key())
            .is_some_and(|p| p.id == potential)
        {
            out.insert(cable);
        }
    }
    out
}
