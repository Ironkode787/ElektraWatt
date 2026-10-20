// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The incremental maintenance of the potential partition (ADR-0002 amendment 4, ARCH §6.1 "incremental
//! connectivity"; decision log ID-219): the components an edit touched are recomputed from the per-page nets,
//! every other component is kept as it is.
//!
//! # The algorithm
//!
//! 1. **What changed.** The partition reads only memoised families, so the memos it was marked through
//!    (`Changes::via`) name what may have changed: the local nets of a page, the links of a function, or one of
//!    the index families (pages, page-less links, definition points, bonds, interruption-point pairing, rail
//!    joins, bond paths). A memo that re-verified without a change keeps its `Arc`, so a pointer comparison with
//!    what the previous value read tells a change from a mark. Anything else in `via` (a direct input read, an
//!    unknown family) makes the hook fall back to the cold algorithm.
//! 2. **Seeds.** Every endpoint whose incidence a change touched: the members of the page nets that differ
//!    (old and new), the ends of the page-less links, definitions and bonds that differ, the CPs of the
//!    interruption points whose pairing differs, the PE CPs of the locations whose rail joins differ, and the
//!    CPs of the functions whose links differ.
//! 3. **Region.** The components holding a seed are removed. A search from the seeds over the current inputs
//!    finds the new components through them; when it reaches an endpoint of a component that was not removed (a
//!    new edge to it: an added connection unions), that component is removed too and searched with the rest. A
//!    component no change touched keeps all its internal edges, so a search through it finds it whole.
//! 4. **Components.** Each component found is computed from the current inputs: a union-find over its members at
//!    the net and the signal level gives its nets and signals; its definitions, CPs, junctions and potential
//!    follow (doc 08 §3.3, §3.4, the rules of the cold algorithm).
//! 5. **Parts.** The potentials whose topology differs, the endpoints whose potential differs, and the bonds
//!    when they differ.
//!
//! The work is proportional to the touched components and their pages, not to the project. The oracle
//! compares the result with the cold algorithm (`crates/ew-api/tests/memo_oracle.rs`, the property test of
//! `crates/ew-derive/tests/incremental_partition.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_model::EndpointKey;
use ew_schema::{Function, Junction, Page, PotentialDef, StructureNode};

use crate::Q;
use crate::index::{BondIndex, BondVias, DefPoints, PageIds, PagelessLinks};
use crate::ip::IpPairingQ;
use crate::links::{BondPathsQ, FunctionLinks, FunctionLinksOf, RailBondsQ};
use crate::memo_keys::PartKey;
use crate::nets::{CpRef, Dsu, LocalNets, NetId, PageLocalNets};
use crate::potentials::{Component, Partition, Potential, PotentialId, Signal, bond_links};
use crate::query::Changes;
use crate::stitch::StitchInputs;

/// What the hook knows changed.
#[derive(Default)]
struct Touched {
    pages: BTreeSet<Id<Page>>,
    functions: BTreeSet<Id<Function>>,
}

/// Classifies the `via` entries; `None` for an entry the hook cannot attribute.
fn touched(q: &Q, changes: Changes<'_>) -> Option<Touched> {
    let mut t = Touched::default();
    for dep in changes.via()? {
        if let Some(p) = q.via_key::<PageLocalNets>(dep) {
            t.pages.insert(p);
        } else if let Some(f) = q.via_key::<FunctionLinksOf>(dep) {
            t.functions.insert(f);
        } else if q.via_key::<PageIds>(dep).is_some()
            || q.via_key::<PagelessLinks>(dep).is_some()
            || q.via_key::<DefPoints>(dep).is_some()
            || q.via_key::<BondIndex>(dep).is_some()
            || q.via_key::<BondVias>(dep).is_some()
            || q.via_key::<IpPairingQ>(dep).is_some()
            || q.via_key::<RailBondsQ>(dep).is_some()
            || q.via_key::<BondPathsQ>(dep).is_some()
        {
            // Re-read below with every other index family.
        } else {
            // A direct read, or a family the partition does not read: not attributable.
            return None;
        }
    }
    Some(t)
}

/// The working copy of a partition (persistent maps: every clone is O(1)).
struct Work {
    p: Partition,
    /// The previous potential of every endpoint of a removed component (`None`: not in a potential, or not a
    /// node before).
    old_pid: BTreeMap<EndpointKey, Option<PotentialId>>,
    /// The removed components of potentials, by potential ID.
    removed: BTreeMap<PotentialId, Arc<Component>>,
    /// The endpoints to search from.
    frontier: Vec<EndpointKey>,
}

impl Work {
    /// Removes the component `key`; its members join the frontier.
    fn remove(&mut self, key: EndpointKey) {
        let Some(comp) = self.p.comps.remove(&key) else {
            return;
        };
        let pid = comp.potential.as_ref().map(|p| p.id);
        for m in &comp.members {
            self.p.comp_of.remove(m);
            self.old_pid.entry(*m).or_insert(pid);
            self.frontier.push(*m);
        }
        if let Some(p) = &comp.potential {
            self.p.by_id.remove(&p.id);
            self.removed.insert(p.id, Arc::clone(&comp));
        }
        for s in &comp.signals {
            self.p.signal_of.remove(&s.id);
        }
    }

    /// A seed endpoint: its component (if any) is removed, and it is searched from.
    fn seed(&mut self, e: EndpointKey) {
        match self.p.comp_of.get(&e).copied() {
            Some(c) => self.remove(c),
            None => {
                self.old_pid.entry(e).or_insert(None);
                self.frontier.push(e);
            }
        }
    }
}

/// Adds `page` to the page list of `e` (kept ascending).
fn add_page(p: &mut Partition, e: EndpointKey, page: Id<Page>) {
    let mut list = p.pages_of.get(&e).cloned().unwrap_or_default();
    if let Err(i) = list.binary_search(&page) {
        list.insert(i, page);
        p.pages_of.insert(e, list);
    }
}

/// Removes `page` from the page list of `e`.
fn remove_page(p: &mut Partition, e: EndpointKey, page: Id<Page>) {
    let Some(mut list) = p.pages_of.get(&e).cloned() else {
        return;
    };
    if let Ok(i) = list.binary_search(&page) {
        list.remove(i);
        if list.is_empty() {
            p.pages_of.remove(&e);
        } else {
            p.pages_of.insert(e, list);
        }
    }
}

/// Updates the nets of `page` to `new` (`None`: the page is gone) and seeds the members of every net that
/// differs.
fn update_page(w: &mut Work, page: Id<Page>, new: Option<Arc<LocalNets>>) {
    let old = w.p.pages.get(&page).cloned();
    if match (&old, &new) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b) || a == b,
        (None, None) => true,
        _ => false,
    } {
        if let (Some(_), Some(b)) = (&old, &new) {
            w.p.pages.insert(page, Arc::clone(b));
        }
        return;
    }
    // Nets present, equal, in both versions keep their members and their incidence.
    let same = |net: &crate::nets::LocalNet, other: Option<&Arc<LocalNets>>| {
        other.is_some_and(|o| o.net_of(net.members[0].at) == Some(net))
    };
    let mut seeds: Vec<EndpointKey> = Vec::new();
    if let Some(old) = &old {
        for net in old.nets.iter().filter(|n| !same(n, new.as_ref())) {
            for m in &net.members {
                remove_page(&mut w.p, m.at, page);
                seeds.push(m.at);
            }
        }
    }
    if let Some(new) = &new {
        for net in new.nets.iter().filter(|n| !same(n, old.as_ref())) {
            for m in &net.members {
                add_page(&mut w.p, m.at, page);
                seeds.push(m.at);
            }
        }
    }
    match new {
        Some(n) => {
            w.p.pages.insert(page, n);
        }
        None => {
            w.p.pages.remove(&page);
        }
    }
    for e in seeds {
        w.seed(e);
    }
}

/// The current inputs of the search and the component computation.
struct Ctx<'a> {
    q: &'a Q,
    inputs: &'a StitchInputs,
    /// The bond ends (both ends known), which are stitch nodes.
    bond_ends: BTreeSet<EndpointKey>,
    /// The links of the functions met, read once.
    links: BTreeMap<Id<Function>, Arc<FunctionLinks>>,
}

impl Ctx<'_> {
    /// Whether `e` is a stitch node now: in a page-local net, or a page-less point.
    fn present(&self, p: &Partition, e: EndpointKey) -> bool {
        p.pages_of.contains_key(&e)
            || self.inputs.pageless.touches(e)
            || self.inputs.defs.at(e).next().is_some()
            || self.bond_ends.contains(&e)
            || CpRef::of(e).is_some_and(|cp| self.inputs.ip.at(cp).is_some())
    }

    fn links(&mut self, f: Id<Function>) -> Arc<FunctionLinks> {
        let q = self.q;
        Arc::clone(self.links.entry(f).or_insert_with(|| q.get::<FunctionLinksOf>(f)))
    }

    /// The short edges of `f` between its CPs that are nodes now (declared shorts, duplicated pins, terminal
    /// levels; doc 08 §3.3, §6.2).
    fn shorts(&mut self, p: &Partition, f: Id<Function>) -> Vec<(u16, u16)> {
        let links = self.links(f);
        let present = |i: u16| self.present(p, EndpointKey::Cp(f, i));
        let mut out = Vec::new();
        for d in &links.declared {
            if let crate::links::DeclaredLink::Short { a, b } = *d
                && present(a)
                && present(b)
            {
                out.push((a, b));
            }
        }
        for idxs in links
            .dup_groups
            .iter()
            .chain(links.level_groups.iter().map(|(_, v)| v))
        {
            let idxs: Vec<u16> = idxs.iter().copied().filter(|i| present(*i)).collect();
            for w in idxs.windows(2) {
                if !links.passes.contains(&(w[0].min(w[1]), w[0].max(w[1]))) {
                    out.push((w[0], w[1]));
                }
            }
        }
        out
    }

    /// The PE CPs of `cp`'s rail location that are nodes now (the rail joins them all, doc 08 §3.7).
    fn rail(&self, p: &Partition, cp: CpRef) -> Option<(Id<StructureNode>, Vec<CpRef>)> {
        let location = *self.inputs.rail.location_of.get(&cp)?;
        let cps = self.inputs.rail.by_location.get(&location)?;
        Some((
            location,
            cps.iter().copied().filter(|c| self.present(p, c.key())).collect(),
        ))
    }
}

/// The components through the frontier, over the current inputs (step 3 of the module documentation).
fn search(w: &mut Work, cx: &mut Ctx<'_>) -> Vec<Vec<EndpointKey>> {
    let mut assigned: BTreeSet<EndpointKey> = BTreeSet::new();
    let mut seen_nets: BTreeSet<(Id<Page>, EndpointKey)> = BTreeSet::new();
    let mut seen_rails: BTreeSet<Id<StructureNode>> = BTreeSet::new();
    let mut shorts: BTreeMap<Id<Function>, Vec<(u16, u16)>> = BTreeMap::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < w.frontier.len() {
        let start = w.frontier[i];
        i += 1;
        if assigned.contains(&start) || !cx.present(&w.p, start) {
            continue;
        }
        assigned.insert(start);
        let mut comp = Vec::new();
        let mut stack = vec![start];
        while let Some(e) = stack.pop() {
            comp.push(e);
            let mut next: Vec<EndpointKey> = Vec::new();
            // The page-local nets holding `e`: all their members, once per net.
            for page in w.p.pages_of.get(&e).cloned().unwrap_or_default() {
                let Some(nets) = w.p.pages.get(&page) else {
                    continue;
                };
                let Some(net) = nets.net_of(e) else { continue };
                if seen_nets.insert((page, net.members[0].at)) {
                    next.extend(net.members.iter().map(|m| m.at));
                }
            }
            // Interruption-point pairs, page-less links and bridges.
            if let Some(cp) = CpRef::of(e) {
                let ip = &cx.inputs.ip;
                if let Some(f) = ip.at(cp) {
                    for g in ip.partners(f) {
                        if let Some(pcp) = ip.ips.get(g).and_then(|i| i.cp) {
                            next.push(pcp.key());
                        }
                    }
                }
            }
            for (_, link) in cx.inputs.pageless.at(e) {
                next.push(if link.a == e { link.b } else { link.a });
            }
            if let Some(cp) = CpRef::of(e) {
                // Shorts inside the function, and the rail of its location.
                let edges = match shorts.get(&cp.function) {
                    Some(s) => s.clone(),
                    None => {
                        let s = cx.shorts(&w.p, cp.function);
                        shorts.insert(cp.function, s.clone());
                        s
                    }
                };
                for (a, b) in edges {
                    if a == cp.idx {
                        next.push(EndpointKey::Cp(cp.function, b));
                    } else if b == cp.idx {
                        next.push(EndpointKey::Cp(cp.function, a));
                    }
                }
                if let Some((location, cps)) = cx.rail(&w.p, cp)
                    && cps.contains(&cp)
                    && seen_rails.insert(location)
                {
                    next.extend(cps.iter().map(|c| c.key()));
                }
            }
            for n in next {
                if assigned.contains(&n) || !cx.present(&w.p, n) {
                    continue;
                }
                if let Some(c) = w.p.comp_of.get(&n).copied() {
                    // A component no change touched, reached by a new edge: it joins.
                    w.remove(c);
                }
                assigned.insert(n);
                stack.push(n);
            }
        }
        comp.sort();
        out.push(comp);
    }
    out
}

/// One component from its members, over the current inputs (step 4).
fn component(w: &Work, cx: &mut Ctx<'_>, members: Vec<EndpointKey>) -> Component {
    let p = &w.p;
    let inputs = cx.inputs;
    let mut dsu: Dsu<EndpointKey> = Dsu::default();
    for e in &members {
        dsu.node(*e);
    }
    let mut connected = false;
    // Net level: the page-local nets, the interruption-point pairs, the page-less joining connections.
    for e in &members {
        for page in p.pages_of.get(e).into_iter().flatten() {
            if let Some(net) = p.pages.get(page).and_then(|n| n.net_of(*e)) {
                connected |= !net.connections.is_empty();
                let (x, y) = (dsu.node(*e), dsu.node(net.members[0].at));
                dsu.union(x, y);
            }
        }
        if let Some(cp) = CpRef::of(*e)
            && let Some(f) = inputs.ip.at(cp)
        {
            for g in inputs.ip.partners(f) {
                if let Some(pcp) = inputs.ip.ips.get(g).and_then(|i| i.cp) {
                    let (x, y) = (dsu.node(*e), dsu.node(pcp.key()));
                    dsu.union(x, y);
                }
            }
        }
        for (_, link) in inputs.pageless.at(*e) {
            connected = true;
            if !link.bridge {
                let (x, y) = (dsu.node(link.a), dsu.node(link.b));
                dsu.union(x, y);
            }
        }
    }
    let net_groups = dsu.groups();
    let mut net_of: BTreeMap<EndpointKey, NetId> = BTreeMap::new();
    for g in &net_groups {
        if let Some(id) = NetId::of(g.iter().copied()) {
            for e in g {
                net_of.insert(*e, id);
            }
        }
    }
    // Signal level: the shorts inside functions.
    let functions: BTreeSet<Id<Function>> = members
        .iter()
        .filter_map(|e| CpRef::of(*e).map(|c| c.function))
        .collect();
    // A function can have CPs in several components: only its shorts inside this one count.
    let member_set: BTreeSet<EndpointKey> = members.iter().copied().collect();
    for f in functions {
        for (a, b) in cx.shorts(p, f) {
            let (ea, eb) = (EndpointKey::Cp(f, a), EndpointKey::Cp(f, b));
            if member_set.contains(&ea) && member_set.contains(&eb) {
                let (x, y) = (dsu.node(ea), dsu.node(eb));
                dsu.union(x, y);
            }
        }
    }
    let mut signals: Vec<Signal> = dsu
        .groups()
        .into_iter()
        .filter_map(|g| {
            let id = NetId::of(g.iter().copied())?;
            let mut nets: Vec<NetId> = g.iter().filter_map(|e| net_of.get(e).copied()).collect();
            nets.sort();
            nets.dedup();
            let mut ips: Vec<Id<Function>> = g
                .iter()
                .filter_map(|e| CpRef::of(*e))
                .map(|cp| cp.function)
                .filter(|f| inputs.ip.is_ip(*f))
                .collect();
            ips.sort();
            ips.dedup();
            Some(Signal { id, nets, ips })
        })
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
    let has_device_cp = cps.iter().any(|cp| !inputs.ip.is_ip(cp.function));
    let potential = if defs.is_empty() && !(connected && has_device_cp) {
        None
    } else {
        cps.iter().map(|c| c.id()).min().map(|min_cp| {
            let id = defs
                .first()
                .map_or(PotentialId::Cp(min_cp), |d| PotentialId::Def(*d));
            let mut ips: Vec<Id<Function>> = cps
                .iter()
                .map(|c| c.function)
                .filter(|f| inputs.ip.is_ip(*f))
                .collect();
            ips.dedup();
            let mut nets: Vec<NetId> = net_of.values().copied().collect();
            nets.sort();
            nets.dedup();
            Potential {
                id,
                defs,
                cps,
                junctions,
                signals: signals.iter().map(|s| s.id).collect(),
                nets,
                ips,
            }
        })
    };
    Component::new(members, signals, potential)
}

/// The changed CPs of an interruption point between two pairings: none when its CP and partners are equal.
fn ip_seeds(
    old: &crate::ip::IpPairing,
    new: &crate::ip::IpPairing,
    out: &mut Vec<EndpointKey>,
    p: &Partition,
) {
    let keys: BTreeSet<Id<Function>> = old.ips.keys().chain(new.ips.keys()).copied().collect();
    for f in keys {
        let (o, n) = (old.ips.get(&f), new.ips.get(&f));
        let same = o.map(|i| i.cp) == n.map(|i| i.cp) && old.partners(f) == new.partners(f);
        if same {
            continue;
        }
        out.extend(o.and_then(|i| i.cp).map(|c| c.key()));
        out.extend(n.and_then(|i| i.cp).map(|c| c.key()));
        if o.is_some() != n.is_some() {
            // Whether `f` is an interruption point decides whether its CPs count as device CPs.
            out.extend(p.comp_of.range(EndpointKey::cps_of(f)).map(|(e, _)| *e));
        }
    }
}

/// The incremental update (see the module documentation): the new partition and the parts it changed, or
/// `None` when the hook cannot attribute a change (the caller runs the cold algorithm).
pub(crate) fn update(q: &Q, prev: &Partition, changes: Changes<'_>) -> Option<(Partition, Vec<PartKey>)> {
    let touched = touched(q, changes)?;
    let inputs = StitchInputs::read(q);
    let bond_paths = q.get::<BondPathsQ>(());
    let old_inputs = prev.inputs.clone();
    let mut w = Work {
        p: prev.clone(),
        old_pid: BTreeMap::new(),
        removed: BTreeMap::new(),
        frontier: Vec::new(),
    };
    w.p.inputs = inputs.clone();

    // 2. Seeds.
    let mut pages = touched.pages;
    if !Arc::ptr_eq(&old_inputs.pages, &inputs.pages) {
        pages.extend(old_inputs.pages.changed_keys(&inputs.pages));
    }
    for page in pages {
        let new = inputs.pages.contains(&page).then(|| q.get::<PageLocalNets>(page));
        update_page(&mut w, page, new);
    }
    let mut seeds: Vec<EndpointKey> = Vec::new();
    if !Arc::ptr_eq(&old_inputs.pageless, &inputs.pageless) {
        for id in old_inputs.pageless.links.changed_keys(&inputs.pageless.links) {
            for l in [old_inputs.pageless.links.get(&id), inputs.pageless.links.get(&id)]
                .into_iter()
                .flatten()
            {
                seeds.extend([l.a, l.b]);
            }
        }
    }
    if !Arc::ptr_eq(&old_inputs.defs, &inputs.defs) {
        for id in old_inputs.defs.defs.changed_keys(&inputs.defs.defs) {
            seeds.extend(old_inputs.defs.defs.get(&id).copied());
            seeds.extend(inputs.defs.defs.get(&id).copied());
        }
    }
    if !Arc::ptr_eq(&old_inputs.bonds, &inputs.bonds) {
        for id in old_inputs.bonds.changed_keys(&inputs.bonds) {
            for b in [old_inputs.bonds.get(&id), inputs.bonds.get(&id)]
                .into_iter()
                .flatten()
            {
                seeds.extend(b.a);
                seeds.extend(b.b);
            }
        }
    }
    if !Arc::ptr_eq(&old_inputs.ip, &inputs.ip) {
        ip_seeds(&old_inputs.ip, &inputs.ip, &mut seeds, &w.p);
    }
    if !Arc::ptr_eq(&old_inputs.rail, &inputs.rail) {
        let (o, n) = (&old_inputs.rail.by_location, &inputs.rail.by_location);
        let locations: BTreeSet<Id<StructureNode>> = o.keys().chain(n.keys()).copied().collect();
        for l in locations {
            if o.get(&l) != n.get(&l) {
                for list in [o.get(&l), n.get(&l)].into_iter().flatten() {
                    seeds.extend(list.iter().map(|c| c.key()));
                }
            }
        }
    }
    let mut cx = Ctx {
        q,
        inputs: &inputs,
        bond_ends: inputs
            .bonds
            .iter()
            .filter_map(|(_, b)| b.a.zip(b.b))
            .flat_map(|(a, b)| [a, b])
            .collect(),
        links: BTreeMap::new(),
    };
    for f in &touched.functions {
        let now = cx.links(*f);
        if w.p.links.get(f).is_some_and(|old| Arc::ptr_eq(old, &now)) {
            continue;
        }
        seeds.extend(w.p.comp_of.range(EndpointKey::cps_of(*f)).map(|(e, _)| *e));
    }
    for e in seeds {
        w.seed(e);
    }

    // 3.–4. The region and its components.
    let found = search(&mut w, &mut cx);
    let mut new_pid: BTreeMap<EndpointKey, Option<PotentialId>> = BTreeMap::new();
    let mut added: BTreeMap<PotentialId, Arc<Component>> = BTreeMap::new();
    let mut functions: BTreeSet<Id<Function>> = w
        .old_pid
        .keys()
        .filter_map(|e| CpRef::of(*e).map(|c| c.function))
        .collect();
    for members in found {
        let comp = component(&w, &mut cx, members);
        let key = comp.key();
        let pid = comp.potential.as_ref().map(|p| p.id);
        for m in &comp.members {
            w.p.comp_of.insert(*m, key);
            new_pid.insert(*m, pid);
            if let Some(cp) = CpRef::of(*m) {
                functions.insert(cp.function);
            }
        }
        for s in &comp.signals {
            w.p.signal_of.insert(s.id, key);
        }
        let comp = Arc::new(comp);
        if let Some(p) = &comp.potential {
            w.p.by_id.insert(p.id, key);
            added.insert(p.id, Arc::clone(&comp));
        }
        w.p.comps.insert(key, comp);
    }
    // The links of the functions with a CP in the graph.
    for f in functions {
        if w.p.comp_of.range(EndpointKey::cps_of(f)).next().is_some() {
            let l = cx.links(f);
            if !w.p.links.get(&f).is_some_and(|old| Arc::ptr_eq(old, &l)) {
                w.p.links.insert(f, l);
            }
        } else if w.p.links.contains_key(&f) {
            w.p.links.remove(&f);
        }
    }
    for f in &touched.functions {
        if let Some(old) = w.p.links.get(f).cloned() {
            let now = cx.links(*f);
            if !Arc::ptr_eq(&old, &now) {
                w.p.links.insert(*f, now);
            }
        }
    }
    drop(cx);
    w.p.bond_paths = bond_paths;
    let bonds = bond_links(&w.p.inputs, &w.p.bond_paths, |e| {
        w.p.potential_at(e).map(|p| p.id)
    });

    // 5. Parts.
    let mut parts: Vec<PartKey> = Vec::new();
    let ids: BTreeSet<PotentialId> = w.removed.keys().chain(added.keys()).copied().collect();
    for id in ids {
        if w.removed.get(&id) != added.get(&id) {
            parts.push(PartKey::Potential(id));
        }
    }
    let ends: BTreeSet<EndpointKey> = w.old_pid.keys().chain(new_pid.keys()).copied().collect();
    for e in ends {
        let (a, b) = (
            w.old_pid.get(&e).copied().flatten(),
            new_pid.get(&e).copied().flatten(),
        );
        if a != b {
            parts.push(PartKey::Endpoint(e));
        }
    }
    if bonds != w.p.bonds {
        parts.push(PartKey::Bonds);
        w.p.bonds = bonds;
    }
    Some((w.p, parts))
}
