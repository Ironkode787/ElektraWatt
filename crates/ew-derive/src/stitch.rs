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
//!   nothing (EW-TERM-005); one without a location is skipped (EW-POT-008).
//! - **Cores** join as connections (net level); there is no separate conductor edge. CI-05 makes two
//!   connections on one conductor parts of one physical wire, which their IP pair joins already.
//!
//! Only CPs already in the graph take part in shorts and rail bonds, so unconnected functions add no
//! nodes. The edge list is sorted by (level, kind, endpoints) (doc 08 §11).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_rds::Aspect;
use ew_schema::{
    Bond, ClampKey, Connection, ConnectionKind, ContactKind, Function, FunctionKind, InternalKind,
    IpDirection, IpPairingMode, IpPairingScope, Page, PotentialType, Representation, Side, StructureNode,
};

use crate::cps::pattern_origin;
use crate::nets::{CpRef, NetId, PageLocalNets, bond_vias, ends, ip_cp, joins_net};
use crate::note::DeriveNote;
use crate::query::{Policy, Query};
use crate::{DrawnPageOrder, FunctionCps, PageRef, PageStructure, Q, RepAnchorCell, StructureOf};

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
    /// A rail bond through the location PE node of `location` (terminals without a location never bond).
    RailBond {
        /// The location node.
        location: Id<StructureNode>,
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
        let mut b = Builder {
            cut: bond_vias(q),
            ..Builder::default()
        };
        // 1. Local nets, pages in ID order.
        for page in q.pages().ids() {
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
        }
        // 2. Page-less joining connections and every bridge.
        for (id, c) in q.connections().iter() {
            let Some((ea, eb)) = ends(c) else { continue };
            let kind = match &c.kind {
                ConnectionKind::Bridge(_) => EdgeKind::Bridge(id),
                k if joins_net(k) && c.page.is_none() && !b.cut.contains(&id) => EdgeKind::PageLess(id),
                _ => continue,
            };
            let (na, nb) = (b.node(ea), b.node(eb));
            b.nodes[na].connected = true;
            b.nodes[nb].connected = true;
            b.edge(kind, na, nb);
        }
        // 3. Definition points and bonds.
        for (_, d) in q.potential_defs().iter() {
            if let Some(at) = EndpointKey::of(&d.at) {
                b.node(at);
            }
        }
        for (id, bond) in q.bonds().iter() {
            if let (Some(a), Some(z)) = (EndpointKey::of(&bond.a), EndpointKey::of(&bond.b)) {
                let (na, nz) = (b.node(a), b.node(z));
                b.edge(EdgeKind::Bond(id), na, nz);
            }
        }
        // 4. Interruption points and their pairing.
        pair_ips(q, &mut b);
        // 5. Shorts and pass-through pairs between CPs already in the graph.
        functions_links(q, &mut b);
        // 6. Rail bonds.
        rail_bonds(q, &mut b);

        b.edges.sort();
        b.edges.dedup();
        for n in &b.notes {
            q.note(n.clone());
        }
        let mut partners: BTreeMap<Id<Function>, Vec<Id<Function>>> = BTreeMap::new();
        for g in &b.groups {
            for &(x, y) in &g.pairs {
                partners.entry(x).or_default().push(y);
                partners.entry(y).or_default().push(x);
            }
        }
        for list in partners.values_mut() {
            list.sort();
            list.dedup();
        }
        Arc::new(StitchGraph {
            nodes: b.nodes,
            edges: b.edges,
            ips: b.ips,
            ip_groups: b.groups,
            cut: b.cut,
            notes: b.notes,
            node_of: b.node_of,
            partners,
        })
    }
}

#[derive(Default)]
struct Builder {
    nodes: Vec<StitchNode>,
    edges: Vec<StitchEdge>,
    node_of: BTreeMap<EndpointKey, usize>,
    ips: BTreeMap<Id<Function>, IpInfo>,
    groups: Vec<IpGroup>,
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

    fn note(&mut self, code: &'static str, subject: String, detail: impl Into<String>) {
        self.notes.push(DeriveNote::Connectivity {
            code,
            subject,
            detail: detail.into(),
        });
    }
}

/// The chain order key of an interruption point: (sort key, page position, anchor row, anchor column, ID).
type ChainKey = (i64, usize, u32, u32, Id<Function>);

/// A pairing group key: (name, scope node).
type GroupKey = (String, Option<Id<StructureNode>>);

fn pair_ips(q: &Q, b: &mut Builder) {
    let settings = &q.project().settings;
    let (mode, scope_aspect) = match &settings.ip_pairing {
        None => (PairMode::Star, None),
        Some(p) => {
            let mode = match p.mode {
                IpPairingMode::Chain => PairMode::Chain,
                _ => PairMode::Star,
            };
            let aspect = match p.scope {
                IpPairingScope::Function => Some(Aspect::Function),
                IpPairingScope::Location => Some(Aspect::Location),
                _ => None,
            };
            (mode, aspect)
        }
    };
    let reps = q.reps();
    let order = q.get::<DrawnPageOrder>(());
    let mut groups: BTreeMap<GroupKey, Vec<Id<Function>>> = BTreeMap::new();
    let mut chain_keys: BTreeMap<Id<Function>, ChainKey> = BTreeMap::new();
    for (fid, f) in q.functions().iter() {
        let FunctionKind::Ip(spec) = &f.kind else { continue };
        let cp = ip_cp(q, fid);
        let rep = q.idx().reps_of_function(fid).next();
        let page = rep.and_then(|r| reps.get(r)).map(|r| r.page);
        if let Some(cp) = cp {
            b.node(cp.key());
        }
        let info = IpInfo {
            function: fid,
            name: spec.name.as_str().to_owned(),
            direction: spec.direction.clone(),
            external: spec.external.is_some(),
            star_source: spec.star_source,
            sort: spec.sort,
            cp,
            rep,
            page,
            group: None,
        };
        if !info.external && cp.is_none() {
            b.note(
                "EW-CORE-051",
                format!("interruption point {} ({fid})", info.name),
                "the interruption point has no connection point; it pairs with nothing",
            );
        }
        if !info.external && cp.is_some() {
            let scope = scope_aspect.and_then(|a| page.and_then(|p| q.get::<PageStructure>(p).node(a)));
            if let (Some(aspect), None) = (scope_aspect, scope) {
                b.note(
                    "EW-CORE-051",
                    format!("interruption point {} ({fid})", info.name),
                    format!(
                        "ip_pairing scope is {aspect:?} but the page has no such structure node; \
                         paired in the project-wide group"
                    ),
                );
            }
            groups.entry((info.name.clone(), scope)).or_default().push(fid);
            let pos = page
                .and_then(|p| order.position(PageRef::Drawn(p)))
                .unwrap_or(usize::MAX);
            let cell = rep.and_then(|r| q.get::<RepAnchorCell>(r).as_ref().clone());
            let (row, col) = cell.map_or((u32::MAX, u32::MAX), |c| (c.row, c.column));
            let sort = spec.sort.map_or(i64::MAX, i64::from);
            chain_keys.insert(fid, (sort, pos, row, col, fid));
        }
        b.ips.insert(fid, info);
    }
    for ((name, scope), mut members) in groups {
        let gi = b.groups.len();
        let mut g = IpGroup {
            name: name.clone(),
            scope,
            mode,
            members: Vec::new(),
            source: None,
            pairs: Vec::new(),
            unpaired: Vec::new(),
        };
        match mode {
            PairMode::Star => {
                members.sort();
                let flagged: Vec<Id<Function>> =
                    members.iter().copied().filter(|f| b.ips[f].star_source).collect();
                let sources: Vec<Id<Function>> = members
                    .iter()
                    .copied()
                    .filter(|f| b.ips[f].direction == IpDirection::Source)
                    .collect();
                if flagged.len() > 1 {
                    b.note(
                        "EW-CORE-051",
                        format!("interruption point {name}"),
                        format!("{} star sources; the smallest ID is the source", flagged.len()),
                    );
                }
                if flagged.is_empty() && sources.len() >= 2 {
                    b.note(
                        "EW-CORE-051",
                        format!("interruption point {name}"),
                        format!(
                            "{} source points and no star_source; the lowest ID source is the star source",
                            sources.len()
                        ),
                    );
                }
                let source = flagged
                    .first()
                    .copied()
                    .or_else(|| sources.first().copied())
                    .unwrap_or(members[0]);
                g.source = Some(source);
                for &m in &members {
                    if m != source {
                        g.pairs.push((source, m));
                    }
                }
                if members.len() == 1 {
                    g.unpaired.push(source);
                }
                g.members = members;
            }
            PairMode::Chain => {
                members.sort_by_key(|f| chain_keys[f]);
                for pair in members.chunks(2) {
                    match pair {
                        [x, y] => g.pairs.push((*x, *y)),
                        [x] => g.unpaired.push(*x),
                        _ => {}
                    }
                }
                g.members = members;
            }
        }
        for &u in &g.unpaired {
            b.note(
                "EW-CORE-051",
                format!("interruption point {name} ({u})"),
                "no partner; its wire stays open-ended",
            );
        }
        for &(x, y) in &g.pairs {
            let (Some(cx), Some(cy)) = (b.ips[&x].cp, b.ips[&y].cp) else {
                continue;
            };
            let (nx, ny) = (b.node(cx.key()), b.node(cy.key()));
            b.edge(EdgeKind::IpPair { a: x, b: y }, nx, ny);
        }
        for &m in &g.members {
            if let Some(info) = b.ips.get_mut(&m) {
                info.group = Some(gi);
            }
        }
        b.groups.push(g);
    }
}

fn functions_links(q: &Q, b: &mut Builder) {
    let fndefs = q.fndefs();
    for (fid, f) in q.functions().iter() {
        if matches!(f.kind, FunctionKind::Ip(_)) {
            continue;
        }
        let Some(def) = fndefs.get(f.def) else { continue };
        let cps = q.get::<FunctionCps>(fid);
        let present: BTreeSet<u16> = cps
            .iter()
            .map(|c| c.idx)
            .filter(|&i| b.node_of.contains_key(&EndpointKey::Cp(fid, i)))
            .collect();
        if present.is_empty() {
            continue;
        }
        let exists = |i: u16| cps.iter().any(|c| c.idx == i);
        let mut has_pass_through = false;
        let mut passes: BTreeSet<(u16, u16)> = BTreeSet::new();
        for link in &def.item.internal {
            let kind = match &link.kind {
                InternalKind::Short => EdgeKind::Short {
                    function: fid,
                    a: link.a,
                    b: link.b,
                    source: ShortSource::Declared,
                },
                InternalKind::PassThrough(p) => {
                    has_pass_through = true;
                    passes.insert((link.a.min(link.b), link.a.max(link.b)));
                    EdgeKind::PassThrough {
                        function: fid,
                        a: link.a,
                        b: link.b,
                        opens: p.opens,
                    }
                }
                InternalKind::Unknown(_) => continue,
            };
            if !exists(link.a) || !exists(link.b) {
                continue;
            }
            // Only CPs in the graph take part (an unconnected CP joins nothing).
            if !present.contains(&link.a) || !present.contains(&link.b) {
                continue;
            }
            let (na, nb) = (
                b.node_of[&EndpointKey::Cp(fid, link.a)],
                b.node_of[&EndpointKey::Cp(fid, link.b)],
            );
            b.edge(kind, na, nb);
        }
        let terminal = def.item.contact_kind == ContactKind::Terminal
            || f.device
                .and_then(|d| q.devices().get(d))
                .is_some_and(|d| d.aspects.terminal.is_some());
        // Duplicated device pins (review decision D2): only a black box or a part-pattern-origin function,
        // whose equal designations are the same pin, and never a terminal. Equal designations of one level
        // chain in index order, unless a pass-through pair of the FnDef links the two.
        if !terminal && pattern_origin(q, f) {
            let mut by_designation: BTreeMap<(&str, Option<u8>), Vec<u16>> = BTreeMap::new();
            for c in cps.iter().filter(|c| present.contains(&c.idx)) {
                by_designation
                    .entry((c.designation.as_str(), c.level))
                    .or_default()
                    .push(c.idx);
            }
            for idxs in by_designation.values() {
                b.chain(fid, idxs, &passes, ShortSource::DuplicatePin);
            }
        }
        // The terminal level rule (doc 08 §6.2): without a declared pass-through, the CPs of one level are one
        // signal. With one (disconnect and fuse terminals, review decision D3), the CPs of one level and one
        // side are; the pass-through pair links the sides. Levels stay separate.
        if def.item.contact_kind == ContactKind::Terminal {
            let mut groups: BTreeMap<(u8, u8), Vec<u16>> = BTreeMap::new();
            for c in cps.iter().filter(|c| present.contains(&c.idx)) {
                let Some(level) = c.level else { continue };
                let side = match (&c.side, has_pass_through) {
                    (_, false) => 0,
                    (Some(Side::Side1), true) => 1,
                    (Some(Side::Side2), true) => 2,
                    // No side, or one of a newer format: no same-side short.
                    _ => continue,
                };
                groups.entry((level, side)).or_default().push(c.idx);
            }
            for ((level, _), idxs) in &groups {
                b.chain(fid, idxs, &passes, ShortSource::TerminalLevel(*level));
            }
        }
    }
}

fn rail_bonds(q: &Q, b: &mut Builder) {
    let mut by_location: BTreeMap<Id<StructureNode>, Vec<CpRef>> = BTreeMap::new();
    for (dev, d) in q.devices().iter() {
        let Some(t) = &d.aspects.terminal else { continue };
        if !t.rail_bonded {
            continue;
        }
        // Review decision D5: only the PE CPs of the terminal join the location PE node.
        // A terminal with a PE CP that is not connected joins nothing, without a note.
        let mut pe: Vec<CpRef> = Vec::new();
        let mut has_pe = false;
        for f in q.idx().functions_of_device(dev) {
            for c in q.get::<FunctionCps>(f).iter() {
                let cp = CpRef::new(f, c.idx);
                let is_pe = c.clamp == Some(ClampKey::RailFoot)
                    || matches!(c.potential_role, Some(PotentialType::Pe | PotentialType::Pen));
                has_pe |= is_pe;
                if is_pe && b.node_of.contains_key(&cp.key()) {
                    pe.push(cp);
                }
            }
        }
        let location = match q.try_get::<StructureOf>(dev) {
            Ok(s) => s.node(Aspect::Location),
            Err(cycle) => {
                // The cycle is noted; the terminal has no resolvable location.
                q.note_cycle(cycle);
                continue;
            }
        };
        if !has_pe {
            b.note(
                "EW-TERM-005",
                dev.to_string(),
                "rail_bonded is set but the terminal has no PE CP; it joins no location PE node",
            );
        }
        let Some(location) = location else {
            b.note(
                "EW-POT-008",
                dev.to_string(),
                "the rail-bonded terminal has no location; it joins no location PE node",
            );
            continue;
        };
        by_location.entry(location).or_default().extend(pe);
    }
    for (location, mut cps) in by_location {
        cps.sort();
        for w in cps.windows(2) {
            let (na, nb) = (b.node_of[&w[0].key()], b.node_of[&w[1].key()]);
            b.edge(EdgeKind::RailBond { location }, na, nb);
        }
    }
}
