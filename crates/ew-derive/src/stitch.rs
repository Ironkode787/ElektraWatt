// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The stitch graph over local nets: `stitch_graph()` (ARCH §6.4, doc 08 §3.3).
//!
//! # Nodes
//!
//! One node per page-local net ([`PageLocalNets`], pages in ID order), plus a singleton node for every
//! CP that no local net holds but that a page-less connection, a potential definition, a bond or an
//! interruption point names. A CP held by local nets of two pages (one function drawn on two pages)
//! joins them by a [`EdgeKind::SharedCp`] edge.
//!
//! # Edges (doc 08 §3.3)
//!
//! | Level | Kinds |
//! |---|---|
//! | [`Level::Net`] | IP pairs, page-less `Wire`/`Core`/`Bus`/`Internal` connections, shared CPs |
//! | [`Level::Signal`] | FnDef `Short` links; the terminal level rule |
//! | [`Level::Potential`] | `Bridge` connections (drawn or page-less); rail bonds per location |
//! | [`Level::Link`] | bonds and `PassThrough` pairs: recorded, never joined |
//!
//! - **IP pairing** (doc 03 §10, FR-XRF-007, FR-XRF-010). Points pair by (name, scope key); the scope
//!   key is the node of the IP page's resolved structure in the aspect of `ip_pairing.scope` (none for
//!   `project`). An absent `ip_pairing` setting means star mode, project scope (reading R-1 of the
//!   WP-S1.07 report). Star mode: the star source (the point with `star_source`; else the lowest-ID
//!   `Source` point, noted when there are two or more; else the smallest ID) pairs with every other
//!   point. Chain mode: points ordered by (sort key, page order, anchor row, anchor column, ID) pair as
//!   (1, 2), (3, 4), …. External points never pair. A point without a partner, a point without a CP, and
//!   a point whose page lacks the node of a `function`/`location` scope (it joins the project-wide group)
//!   are noted (EW-CORE-051).
//! - **Shorts.** A FnDef link joins two instantiated CPs of the function when both indices exist. A
//!   terminal function (contact kind `terminal`) whose FnDef declares no `PassThrough` link also joins all
//!   its CPs of one level (doc 08 §6.2, "`Short` over all CPs of a level, shaft included"): a
//!   template-origin terminal takes its CP indices from the part pattern (decision log ID-67, ID-108),
//!   which the FnDef's link indices do not name. Levels stay separate. A terminal that declares a
//!   pass-through (disconnect, fuse) joins the CPs of one level *and one side*; the pass-through pair links
//!   the sides (review decision D3). CPs without a level, or without a side on such a terminal, join nothing.
//! - **Duplicated pins** (doc 03 §9.3, review decision D2): on a black box or a part-pattern-origin function
//!   (not a terminal), two CPs with the same printed designation and level are one pin, hence one signal,
//!   unless a declared pass-through pair links them; a black-box part pattern (the golden PSU -102G1 with
//!   +#1/+#2 and −#1/−#2, the MICO with 24VDC#1/#2 and GND#1/#2) declares no links of its own. Equal
//!   designations of an FnDef-origin function or of a terminal (a double-deck terminal's a/b per level) are
//!   separate points.
//! - **Rail bonding at MVP** (doc 08 §3.7, review decision D5): the PE CPs of every rail-bonded terminal
//!   of one location join one virtual location PE node, expressed as a chain of [`EdgeKind::RailBond`]
//!   edges in CP order. A PE CP is a CP in the graph whose clamp is the rail foot or whose potential role
//!   is `Pe` or `Pen`; there is no fallback to another CP. A rail-bonded terminal without a PE CP joins
//!   nothing (EW-TERM-005); one without a location joins the only own location (or the implicit main location,
//!   [`RailLocation::Main`], when there is none), and is skipped (EW-POT-008) only when the project has two or
//!   more own locations (amended by cycle 33).
//! - **Cores** join as connections (net level); there is no separate conductor edge. CI-05 makes two
//!   connections on one conductor parts of one physical wire, which their IP pair joins already.
//!
//! Only CPs already in the graph take part in shorts and rail bonds, so unconnected functions add no
//! nodes. The edge list is sorted by (level, kind, endpoints) (doc 08 §11).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{Bond, Connection, Function, IpDirection, Page, Representation, StructureNode};

use crate::Q;
use crate::index::PagelessLinksV;
use crate::index::{BondIndex, BondRec, BondVias, DefPoints, DefPointsV, Index, PageIds, PagelessLinks};
use crate::ip::{IpPairing, IpPairingQ};
use crate::links::{DeclaredLink, FunctionLinks, FunctionLinksOf, RailBonds, RailBondsQ, RailLocation};
use crate::nets::{CpRef, LocalNets, NetId, PageLocalNets};
use crate::note::DeriveNote;
use crate::query::{Policy, Query};

/// The level of a stitch edge (doc 08 §3.3), in the order the partition applies them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Level {
    /// Joins nets.
    Net,
    /// Joins nets into signals.
    Signal,
    /// Joins signals into potentials.
    Potential,
    /// Recorded, never joined (bonds, pass-through pairs).
    Link,
}

/// Where a `Short` comes from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ShortSource {
    /// A link the FnDef declares.
    Declared,
    /// The terminal level rule (doc 08 §6.2): CPs of this level.
    TerminalLevel(u8),
    /// Duplicated device pins (doc 03 §9.3, "PSU +#1/+#2"): two CPs of one function with the same printed
    /// designation that no declared pass-through pair links.
    DuplicatePin,
}

/// The kind of a stitch edge.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EdgeKind {
    /// One CP held by local nets of two pages.
    SharedCp(CpRef),
    /// An interruption-point pair.
    IpPair {
        /// The first point (the star source, or the earlier chain point).
        a: Id<Function>,
        /// The second point.
        b: Id<Function>,
    },
    /// A page-less `Wire`, `Core`, `Bus` or `Internal` connection.
    PageLess(Id<Connection>),
    /// A short inside one function.
    Short {
        /// The function.
        function: Id<Function>,
        /// First CP index.
        a: u16,
        /// Second CP index.
        b: u16,
        /// Declared or by the terminal level rule.
        source: ShortSource,
    },
    /// A `Bridge` connection.
    Bridge(Id<Connection>),
    /// A rail bond through the location PE node of `location` (a terminal without a location bonds on the implicit
    /// main location or the only own location, amended by cycle 33; never when the project has two or more).
    RailBond {
        /// The location.
        location: RailLocation,
    },
    /// A bond (non-joining, doc 08 §3.7).
    Bond(Id<Bond>),
    /// A pass-through pair (non-joining; a flow edge, doc 08 §3.6). `a` and `b` keep the FnDef's order.
    PassThrough {
        /// The function.
        function: Id<Function>,
        /// First CP index.
        a: u16,
        /// Second CP index.
        b: u16,
        /// The contact can open.
        opens: bool,
    },
}

impl EdgeKind {
    /// The level of the kind.
    pub fn level(&self) -> Level {
        match self {
            EdgeKind::SharedCp(_) | EdgeKind::IpPair { .. } | EdgeKind::PageLess(_) => Level::Net,
            EdgeKind::Short { .. } => Level::Signal,
            EdgeKind::Bridge(_) | EdgeKind::RailBond { .. } => Level::Potential,
            EdgeKind::Bond(_) | EdgeKind::PassThrough { .. } => Level::Link,
        }
    }
}

/// One edge between two stitch nodes (indices into [`StitchGraph::nodes`]).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct StitchEdge {
    /// The level.
    pub level: Level,
    /// The kind.
    pub kind: EdgeKind,
    /// The node of the first end.
    pub a: usize,
    /// The node of the second end.
    pub b: usize,
}

/// One node: a local net, or a CP that no local net holds.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StitchNode {
    /// The local net's ID, or the CP's ID.
    pub net: NetId,
    /// The page of the local net; `None` for a page-less node.
    pub page: Option<Id<Page>>,
    /// The members in node order.
    pub members: Vec<EndpointKey>,
    /// Whether a joining connection lies inside the node (a local net with connections).
    pub connected: bool,
}

/// Interruption-point pairing mode as applied.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PairMode {
    /// One source refers to all others.
    Star,
    /// Points pair in order.
    Chain,
}

/// One interruption point.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IpInfo {
    /// The IP function.
    pub function: Id<Function>,
    /// The pairing name.
    pub name: String,
    /// The signal direction.
    pub direction: IpDirection,
    /// An external connection point (never pairs).
    pub external: bool,
    /// The star-source flag.
    pub star_source: bool,
    /// The chain sort key.
    pub sort: Option<i32>,
    /// Its CP.
    pub cp: Option<CpRef>,
    /// Its representation (the first, in ID order).
    pub rep: Option<Id<Representation>>,
    /// The page of that representation.
    pub page: Option<Id<Page>>,
    /// The index of its pairing group in [`StitchGraph::ip_groups`].
    pub group: Option<usize>,
}

/// The interruption points of one (name, scope key).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IpGroup {
    /// The name.
    pub name: String,
    /// The scope key (`None` for project scope).
    pub scope: Option<Id<StructureNode>>,
    /// The mode.
    pub mode: PairMode,
    /// The members: star mode in ID order, chain mode in chain order.
    pub members: Vec<Id<Function>>,
    /// The star source (star mode).
    pub source: Option<Id<Function>>,
    /// The pairs.
    pub pairs: Vec<(Id<Function>, Id<Function>)>,
    /// Members without a partner.
    pub unpaired: Vec<Id<Function>>,
}

/// The value of [`StitchGraphQ`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StitchGraph {
    /// The nodes.
    pub nodes: Vec<StitchNode>,
    /// The edges, sorted.
    pub edges: Vec<StitchEdge>,
    /// Every interruption point by function.
    pub ips: BTreeMap<Id<Function>, IpInfo>,
    /// The pairing groups, in (name, scope) order.
    pub ip_groups: Vec<IpGroup>,
    /// The connections cut as a bond's `via` (doc 08 §3.7).
    pub cut: BTreeSet<Id<Connection>>,
    /// The notes of this derivation (also pushed to the `Q`).
    pub notes: Vec<DeriveNote>,
    node_of: BTreeMap<EndpointKey, usize>,
    /// Every interruption point's pair partners, sorted and deduplicated, built once from `ip_groups`.
    partners: BTreeMap<Id<Function>, Vec<Id<Function>>>,
}

impl StitchGraph {
    /// The node holding `at`.
    pub fn node_of(&self, at: EndpointKey) -> Option<usize> {
        self.node_of.get(&at).copied()
    }

    /// The pair partners of an interruption point, in ID order (empty when it has none).
    pub fn ip_partners(&self, ip: Id<Function>) -> &[Id<Function>] {
        self.partners.get(&ip).map_or(&[], Vec::as_slice)
    }
}

/// `stitch_graph()` (doc 08 §3.3). Initial policy Memo (ARCH §6.4).
#[derive(Debug)]
pub struct StitchGraphQ;

impl crate::heap::HeapBytes for StitchGraph {
    fn heap_bytes(&self) -> usize {
        self.nodes.heap_bytes()
            + self.edges.heap_bytes()
            + self.ips.heap_bytes()
            + self.ip_groups.heap_bytes()
            + self.cut.heap_bytes()
            + self.notes.heap_bytes()
            + self.node_of.heap_bytes()
            + self.partners.heap_bytes()
    }
}

impl Query for StitchGraphQ {
    type Key = ();
    type Value = StitchGraph;
    const FAMILY: &'static str = "stitch_graph";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<StitchGraph> {
        let built = build(q, &StitchInputs::read(q));
        for n in built.graph.notes.iter().chain(&built.cycles) {
            q.note(n.clone());
        }
        Arc::new(built.graph)
    }
}

/// What the stitch graph reads besides the per-page nets and the per-function links: the derived-index
/// families (ARCH §6.3 rule 5), each memoised. The partition keeps them with its value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct StitchInputs {
    /// Every page.
    pub(crate) pages: Arc<Index<Id<Page>, ()>>,
    /// The page-less joining connections and the bridges.
    pub(crate) pageless: Arc<PagelessLinksV>,
    /// The potential definition points.
    pub(crate) defs: Arc<DefPointsV>,
    /// The bonds.
    pub(crate) bonds: Arc<Index<Id<Bond>, BondRec>>,
    /// The bond cut.
    pub(crate) cut: Arc<BTreeSet<Id<Connection>>>,
    /// The interruption points and their pairing.
    pub(crate) ip: Arc<IpPairing>,
    /// The rail joins.
    pub(crate) rail: Arc<RailBonds>,
}

impl StitchInputs {
    /// Reads every input family.
    pub(crate) fn read(q: &Q) -> StitchInputs {
        StitchInputs {
            pages: q.get::<PageIds>(()),
            pageless: q.get::<PagelessLinks>(()),
            defs: q.get::<DefPoints>(()),
            bonds: q.get::<BondIndex>(()),
            cut: q.get::<BondVias>(()),
            ip: q.get::<IpPairingQ>(()),
            rail: q.get::<RailBondsQ>(()),
        }
    }
}

/// A stitch graph with what its build read: the inputs of the partition's cold run.
pub(crate) struct StitchBuild {
    /// The graph.
    pub(crate) graph: StitchGraph,
    /// The page-local nets of every page, in page ID order.
    pub(crate) pages: Vec<(Id<Page>, Arc<LocalNets>)>,
    /// The links of every function with a CP in the graph.
    pub(crate) links: BTreeMap<Id<Function>, Arc<FunctionLinks>>,
    /// The cycle notes of the rail-bonded terminals (pushed beside the graph's notes).
    pub(crate) cycles: Vec<DeriveNote>,
}

/// Builds the stitch graph from its inputs (doc 08 §3.3; the algorithm of the module documentation, in the
/// node order of the cycle-7 graph).
pub(crate) fn build(q: &Q, inputs: &StitchInputs) -> StitchBuild {
    let mut b = Builder {
        cut: (*inputs.cut).clone(),
        ..Builder::default()
    };
    let mut pages = Vec::with_capacity(inputs.pages.len());
    // 1. Local nets, pages in ID order.
    for page in inputs.pages.keys().copied() {
        let local = q.get::<PageLocalNets>(page);
        for net in &local.nets {
            let i = b.nodes.len();
            b.nodes.push(StitchNode {
                net: net.id,
                page: Some(page),
                members: net.members.iter().map(|m| m.at).collect(),
                connected: !net.connections.is_empty(),
            });
            for m in &net.members {
                match b.node_of.get(&m.at) {
                    Some(&first) => {
                        if let Some(cp) = CpRef::of(m.at) {
                            b.edge(EdgeKind::SharedCp(cp), first, i);
                        }
                    }
                    None => {
                        b.node_of.insert(m.at, i);
                    }
                }
            }
        }
        pages.push((page, local));
    }
    // 2. Page-less joining connections and every bridge, in connection ID order.
    for (id, link) in inputs.pageless.links.iter() {
        let kind = if link.bridge {
            EdgeKind::Bridge(*id)
        } else {
            EdgeKind::PageLess(*id)
        };
        let (na, nb) = (b.node(link.a), b.node(link.b));
        b.nodes[na].connected = true;
        b.nodes[nb].connected = true;
        b.edge(kind, na, nb);
    }
    // 3. Definition points and bonds.
    for (_, at) in inputs.defs.defs.iter() {
        b.node(*at);
    }
    for (id, bond) in inputs.bonds.iter() {
        if let (Some(a), Some(z)) = (bond.a, bond.b) {
            let (na, nz) = (b.node(a), b.node(z));
            b.edge(EdgeKind::Bond(*id), na, nz);
        }
    }
    // 4. Interruption points and their pairs.
    let ip = &inputs.ip;
    for info in ip.ips.values() {
        if let Some(cp) = info.cp {
            b.node(cp.key());
        }
    }
    b.notes.extend(ip.notes.iter().cloned());
    for g in &ip.groups {
        for &(x, y) in &g.pairs {
            let (Some(cx), Some(cy)) = (ip.ips[&x].cp, ip.ips[&y].cp) else {
                continue;
            };
            let (nx, ny) = (b.node(cx.key()), b.node(cy.key()));
            b.edge(EdgeKind::IpPair { a: x, b: y }, nx, ny);
        }
    }
    // 5. Shorts and pass-through pairs between CPs already in the graph.
    let functions: BTreeSet<Id<Function>> = b
        .node_of
        .keys()
        .filter_map(|at| CpRef::of(*at).map(|cp| cp.function))
        .collect();
    let mut links = BTreeMap::new();
    for f in functions {
        let l = q.get::<FunctionLinksOf>(f);
        function_edges(&mut b, f, &l);
        links.insert(f, l);
    }
    // 6. Rail bonds.
    b.notes.extend(inputs.rail.notes.iter().cloned());
    for (location, cps) in &inputs.rail.by_location {
        let present: Vec<CpRef> = cps
            .iter()
            .copied()
            .filter(|cp| b.node_of.contains_key(&cp.key()))
            .collect();
        for w in present.windows(2) {
            let (na, nb) = (b.node_of[&w[0].key()], b.node_of[&w[1].key()]);
            b.edge(EdgeKind::RailBond { location: *location }, na, nb);
        }
    }

    b.edges.sort();
    b.edges.dedup();
    let partners = ip
        .ips
        .keys()
        .map(|f| (*f, ip.partners(*f).to_vec()))
        .filter(|(_, p)| !p.is_empty())
        .collect();
    let graph = StitchGraph {
        nodes: b.nodes,
        edges: b.edges,
        ips: ip.ips.clone(),
        ip_groups: ip.groups.clone(),
        cut: b.cut,
        notes: b.notes,
        node_of: b.node_of,
        partners,
    };
    StitchBuild {
        graph,
        pages,
        links,
        cycles: inputs.rail.cycles.clone(),
    }
}

/// The short and pass-through edges of function `f` between CPs in the graph (doc 08 §3.3, §6.2; review
/// decisions D2 and D3).
fn function_edges(b: &mut Builder, f: Id<Function>, links: &FunctionLinks) {
    let present = |b: &Builder, i: u16| b.node_of.contains_key(&EndpointKey::Cp(f, i));
    for d in &links.declared {
        let (kind, x, y) = match *d {
            DeclaredLink::Short { a, b: z } => (
                EdgeKind::Short {
                    function: f,
                    a,
                    b: z,
                    source: ShortSource::Declared,
                },
                a,
                z,
            ),
            DeclaredLink::PassThrough { a, b: z, opens } => (
                EdgeKind::PassThrough {
                    function: f,
                    a,
                    b: z,
                    opens,
                },
                a,
                z,
            ),
        };
        if present(b, x) && present(b, y) {
            let (na, nb) = (
                b.node_of[&EndpointKey::Cp(f, x)],
                b.node_of[&EndpointKey::Cp(f, y)],
            );
            b.edge(kind, na, nb);
        }
    }
    for idxs in &links.dup_groups {
        let idxs: Vec<u16> = idxs.iter().copied().filter(|i| present(b, *i)).collect();
        b.chain(f, &idxs, &links.passes, ShortSource::DuplicatePin);
    }
    for (level, idxs) in &links.level_groups {
        let idxs: Vec<u16> = idxs.iter().copied().filter(|i| present(b, *i)).collect();
        b.chain(f, &idxs, &links.passes, ShortSource::TerminalLevel(*level));
    }
}

#[derive(Default)]
struct Builder {
    nodes: Vec<StitchNode>,
    edges: Vec<StitchEdge>,
    node_of: BTreeMap<EndpointKey, usize>,
    cut: BTreeSet<Id<Connection>>,
    notes: Vec<DeriveNote>,
}

impl Builder {
    /// The node of `at`, adding a page-less singleton when absent.
    fn node(&mut self, at: EndpointKey) -> usize {
        if let Some(&i) = self.node_of.get(&at) {
            return i;
        }
        let i = self.nodes.len();
        self.nodes.push(StitchNode {
            net: NetId::of([at]).expect("one node"),
            page: None,
            members: vec![at],
            connected: false,
        });
        self.node_of.insert(at, i);
        i
    }

    fn edge(&mut self, kind: EdgeKind, a: usize, b: usize) {
        let (a, b) = match kind {
            EdgeKind::PassThrough { .. } => (a, b),
            _ => (a.min(b), a.max(b)),
        };
        self.edges.push(StitchEdge {
            level: kind.level(),
            kind,
            a,
            b,
        });
    }

    /// Shorts the CPs `idxs` of `function` (all in the graph) in index order, skipping a pair that a
    /// pass-through pair of the FnDef links.
    fn chain(
        &mut self,
        function: Id<Function>,
        idxs: &[u16],
        passes: &BTreeSet<(u16, u16)>,
        source: ShortSource,
    ) {
        for w in idxs.windows(2) {
            if passes.contains(&(w[0].min(w[1]), w[0].max(w[1]))) {
                continue;
            }
            let (na, nb) = (
                self.node_of[&EndpointKey::Cp(function, w[0])],
                self.node_of[&EndpointKey::Cp(function, w[1])],
            );
            self.edge(
                EdgeKind::Short {
                    function,
                    a: w[0],
                    b: w[1],
                    source,
                },
                na,
                nb,
            );
        }
    }
}
