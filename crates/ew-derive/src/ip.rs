// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interruption-point pairing, split out of the stitch graph: `ip_info(f)` and `ip_pairing()` (doc 03 §10,
//! FR-XRF-007, FR-XRF-010; ARCH §6.4; WP-S1.07 part 2).
//!
//! - **`ip_info(f)`** is one memo per interruption-point function: its specification, its CP, its first
//!   representation and page, its pairing scope node and its page-order position. A move of its symbol or a
//!   renumber re-derives this one value.
//! - **`ip_pairing()`** groups the points by (name, scope key) and pairs them (star or chain mode, the rules of
//!   [`stitch`](crate::stitch)). It is incremental: when none of the point values it read changed (an edit on a
//!   page with interruption points re-verifies their `ip_info` memos, which cut off), it keeps its previous
//!   value; otherwise it pairs again. It is a **producer of parts** ([`PartKey::Ip`]): a reader that needs the
//!   pairing of a few points (the physical wires of a potential, a cross-reference text) depends on those
//!   points only, and a re-pairing declares the points whose pairing changed.
//!
//! Both are note-free (ARCH §6.4, `Q::detached`): the EW-CORE-051 notes are part of their values, and the stitch
//! graph pushes them, in the order the cycle-7 stitch graph did.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_rds::Aspect;
use ew_schema::{Function, FunctionKind, IpDirection, IpPairingMode, IpPairingScope, IpSpec, Page};
use ew_schema::{Representation, StructureNode};

use crate::heap::HeapBytes;
use crate::index::{Index, IpFunctions};
use crate::memo_keys::PartKey;
use crate::nets::{CpRef, ip_cp};
use crate::note::DeriveNote;
use crate::query::{Changes, Policy, Query};
use crate::stitch::{IpGroup, IpInfo, PairMode};
use crate::{DrawnPageOrder, PageRef, Q, RepAnchorCell};

/// The value of [`IpInfoOf`]: one interruption point as the pairing and the potential names read it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IpFnInfo {
    /// The specification (`None`: the function is missing or no interruption point).
    pub spec: Option<IpSpec>,
    /// Its CP (the first CP of the function).
    pub cp: Option<CpRef>,
    /// Its first representation, in ID order.
    pub rep: Option<Id<Representation>>,
    /// The page of that representation.
    pub page: Option<Id<Page>>,
    /// Whether it takes part in pairing: not external, with a CP.
    pub pairs: bool,
    /// The pairing scope key: the node of the page's structure in the aspect of `ip_pairing.scope` (`None`:
    /// project scope, or the page lacks the node, which is noted).
    pub scope: Option<Id<StructureNode>>,
    /// The page-order position of its page, and the row and column of its representation's anchor cell
    /// (`usize::MAX` / `u32::MAX` when unknown): the page order of points (doc 08 §3.5).
    pub order: (usize, u32, u32),
    /// Its EW-CORE-051 notes (no CP; no scope node on its page).
    pub notes: Vec<DeriveNote>,
}

impl IpFnInfo {
    /// The pairing name.
    pub fn name(&self) -> &str {
        self.spec.as_ref().map_or("", |s| s.name.as_str())
    }

    /// Whether the pairing reads the same from `other`: everything but the page-order position, which only
    /// chain mode reads (a symbol moved on its page in star mode pairs as before).
    fn same_pairing(&self, other: &IpFnInfo, chain: bool) -> bool {
        self.spec == other.spec
            && self.cp == other.cp
            && self.rep == other.rep
            && self.page == other.page
            && self.pairs == other.pairs
            && self.scope == other.scope
            && self.notes == other.notes
            && (!chain || self.order == other.order)
    }

    /// The chain order key (sort key, page position, anchor row, anchor column).
    fn chain(&self) -> (i64, usize, u32, u32) {
        let sort = self
            .spec
            .as_ref()
            .and_then(|s| s.sort)
            .map_or(i64::MAX, i64::from);
        (sort, self.order.0, self.order.1, self.order.2)
    }
}

impl HeapBytes for IpFnInfo {
    fn heap_bytes(&self) -> usize {
        self.spec.as_ref().map_or(0, |s| s.name.as_str().len()) + self.notes.heap_bytes()
    }
}

/// The mode and scope aspect of the project's `ip_pairing` setting (absent: star mode, project scope).
fn pairing_setting(q: &Q) -> (PairMode, Option<Aspect>) {
    match crate::settings::ip_pairing(q) {
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
    }
}

/// `ip_info(f)`: one interruption point (see the module documentation). Policy Memo.
#[derive(Debug)]
pub struct IpInfoOf;

impl Query for IpInfoOf {
    type Key = Id<Function>;
    type Value = IpFnInfo;
    const FAMILY: &'static str = "ip_info";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, fid: &Id<Function>) -> Arc<IpFnInfo> {
        let fid = *fid;
        Arc::new(q.detached(|| {
            let spec = q.functions().get(fid).and_then(|f| match &f.kind {
                FunctionKind::Ip(spec) => Some(spec.clone()),
                _ => None,
            });
            let cp = spec.as_ref().and_then(|_| ip_cp(q, fid));
            let rep = spec.as_ref().and_then(|_| q.idx().reps_of_function(fid).next());
            let page = rep.and_then(|r| q.reps().get(r)).map(|r| r.page);
            let external = spec.as_ref().is_some_and(|s| s.external.is_some());
            let name = spec.as_ref().map_or("", |s| s.name.as_str());
            let mut notes = Vec::new();
            if spec.is_some() && !external && cp.is_none() {
                notes.push(DeriveNote::Connectivity {
                    code: "EW-CORE-051",
                    subject: format!("interruption point {name} ({fid})"),
                    detail: "the interruption point has no connection point; it pairs with nothing".into(),
                });
            }
            let pairs = spec.is_some() && !external && cp.is_some();
            let mut scope = None;
            if pairs {
                let (_, scope_aspect) = pairing_setting(q);
                // The point's structure: its structure box over its page (`ip_structure`, WP-S2.04 §10.2).
                scope = scope_aspect.and_then(|a| q.get::<crate::items::IpStructure>(fid).node(a));
                if let (Some(aspect), None) = (scope_aspect, scope) {
                    notes.push(DeriveNote::Connectivity {
                        code: "EW-CORE-051",
                        subject: format!("interruption point {name} ({fid})"),
                        detail: format!(
                            "ip_pairing scope is {aspect:?} but the page has no such structure node; \
                             paired in the project-wide group"
                        ),
                    });
                }
            }
            let order = if spec.is_some() {
                let pos = page
                    .and_then(|p| q.get::<DrawnPageOrder>(()).position(PageRef::Drawn(p)))
                    .unwrap_or(usize::MAX);
                let cell = rep.and_then(|r| q.get::<RepAnchorCell>(r).as_ref().clone());
                let (row, col) = cell.map_or((u32::MAX, u32::MAX), |c| (c.row, c.column));
                (pos, row, col)
            } else {
                (usize::MAX, u32::MAX, u32::MAX)
            };
            IpFnInfo {
                spec,
                cp,
                rep,
                page,
                pairs,
                scope,
                order,
                notes,
            }
        }))
    }
}

/// The value of [`IpPairingQ`]. Two values are equal when their pairing is (the point values kept for the
/// incremental check are not compared: a re-pairing that changes nothing cuts off).
#[derive(Clone, Eq, Debug)]
pub struct IpPairing {
    /// Every interruption point by function.
    pub ips: BTreeMap<Id<Function>, IpInfo>,
    /// The pairing groups, in (name, scope) order.
    pub groups: Vec<IpGroup>,
    /// The EW-CORE-051 notes, in the stitch graph's order: per point in ID order, then per group.
    pub notes: Vec<DeriveNote>,
    /// Every point's pair partners, sorted and deduplicated.
    partners: BTreeMap<Id<Function>, Vec<Id<Function>>>,
    /// The point of each interruption-point CP.
    at: BTreeMap<CpRef, Id<Function>>,
    /// The point values read, for the incremental check.
    infos: BTreeMap<Id<Function>, Arc<IpFnInfo>>,
    /// The function index read.
    fns: Arc<Index<Id<Function>, IpSpec>>,
    /// The heap bytes, counted once when the pairing is built (a pairing is built whole; review cycle 10,
    /// finding 2).
    heap: usize,
}

impl PartialEq for IpPairing {
    fn eq(&self, other: &IpPairing) -> bool {
        self.ips == other.ips
            && self.groups == other.groups
            && self.notes == other.notes
            && self.partners == other.partners
            && self.at == other.at
    }
}

impl IpPairing {
    /// The pair partners of a point, in ID order (empty when it has none).
    pub fn partners(&self, ip: Id<Function>) -> &[Id<Function>] {
        self.partners.get(&ip).map_or(&[], Vec::as_slice)
    }

    /// The interruption point whose CP is `cp`.
    pub fn at(&self, cp: CpRef) -> Option<Id<Function>> {
        self.at.get(&cp).copied()
    }

    /// Whether `f` is an interruption-point function.
    pub fn is_ip(&self, f: Id<Function>) -> bool {
        self.ips.contains_key(&f)
    }

    /// The group of a point.
    pub fn group_of(&self, ip: Id<Function>) -> Option<&IpGroup> {
        self.ips
            .get(&ip)
            .and_then(|i| i.group)
            .and_then(|g| self.groups.get(g))
    }

    /// What part [`PartKey::Ip`] of `ip` covers: its info without the group index, its partners and the
    /// content of its group.
    fn part(&self, ip: Id<Function>) -> Option<IpPart<'_>> {
        let info = self.ips.get(&ip)?;
        let mut info = info.clone();
        info.group = None;
        Some((info, self.partners(ip), self.group_of(ip)))
    }
}

impl IpPairing {
    /// The heap bytes by a walk of the value (once, when it is built).
    fn walk_heap(&self) -> usize {
        self.ips.heap_bytes()
            + self.groups.heap_bytes()
            + self.notes.heap_bytes()
            + self.partners.heap_bytes()
            + self.at.len() * (std::mem::size_of::<CpRef>() + std::mem::size_of::<Id<Function>>())
            + self.infos.len() * (std::mem::size_of::<Id<Function>>() + std::mem::size_of::<usize>())
    }
}

impl HeapBytes for IpPairing {
    /// Counted when the pairing was built: O(1).
    fn heap_bytes(&self) -> usize {
        self.heap
    }
}

/// What part [`PartKey::Ip`] of a point covers (see `IpPairing::part`).
type IpPart<'a> = (IpInfo, &'a [Id<Function>], Option<&'a IpGroup>);

/// A pairing group key: (name, scope node).
type GroupKey = (String, Option<Id<StructureNode>>);

/// The chain order key of a point: (sort key, page position, anchor row, anchor column, ID).
type ChainKey = (i64, usize, u32, u32, Id<Function>);

/// Pairs every interruption point (the cold algorithm, unchanged from the cycle-7 stitch graph).
fn pair(q: &Q) -> IpPairing {
    let fns = q.get::<IpFunctions>(());
    let (mode, _) = pairing_setting(q);
    let mut ips: BTreeMap<Id<Function>, IpInfo> = BTreeMap::new();
    let mut infos: BTreeMap<Id<Function>, Arc<IpFnInfo>> = BTreeMap::new();
    let mut at: BTreeMap<CpRef, Id<Function>> = BTreeMap::new();
    let mut notes: Vec<DeriveNote> = Vec::new();
    let mut groups: BTreeMap<GroupKey, Vec<Id<Function>>> = BTreeMap::new();
    let mut chain_keys: BTreeMap<Id<Function>, ChainKey> = BTreeMap::new();
    for fid in fns.keys().copied() {
        let info = q.get::<IpInfoOf>(fid);
        let Some(spec) = &info.spec else {
            // The index and the function table disagree only inside one commit; nothing to pair.
            infos.insert(fid, info);
            continue;
        };
        notes.extend(info.notes.iter().cloned());
        if let Some(cp) = info.cp {
            at.insert(cp, fid);
        }
        if info.pairs {
            groups
                .entry((spec.name.as_str().to_owned(), info.scope))
                .or_default()
                .push(fid);
            let (sort, pos, row, col) = info.chain();
            chain_keys.insert(fid, (sort, pos, row, col, fid));
        }
        ips.insert(
            fid,
            IpInfo {
                function: fid,
                name: spec.name.as_str().to_owned(),
                direction: spec.direction.clone(),
                external: spec.external.is_some(),
                star_source: spec.star_source,
                sort: spec.sort,
                cp: info.cp,
                rep: info.rep,
                page: info.page,
                group: None,
            },
        );
        infos.insert(fid, info);
    }
    let mut out_groups: Vec<IpGroup> = Vec::new();
    for ((name, scope), mut members) in groups {
        let gi = out_groups.len();
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
                    members.iter().copied().filter(|f| ips[f].star_source).collect();
                let sources: Vec<Id<Function>> = members
                    .iter()
                    .copied()
                    .filter(|f| ips[f].direction == IpDirection::Source)
                    .collect();
                if flagged.len() > 1 {
                    notes.push(DeriveNote::Connectivity {
                        code: "EW-CORE-051",
                        subject: format!("interruption point {name}"),
                        detail: format!("{} star sources; the smallest ID is the source", flagged.len()),
                    });
                }
                if flagged.is_empty() && sources.len() >= 2 {
                    notes.push(DeriveNote::Connectivity {
                        code: "EW-CORE-051",
                        subject: format!("interruption point {name}"),
                        detail: format!(
                            "{} source points and no star_source; the lowest ID source is the star source",
                            sources.len()
                        ),
                    });
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
            notes.push(DeriveNote::Connectivity {
                code: "EW-CORE-051",
                subject: format!("interruption point {name} ({u})"),
                detail: "no partner; its wire stays open-ended".into(),
            });
        }
        for &m in &g.members {
            if let Some(info) = ips.get_mut(&m) {
                info.group = Some(gi);
            }
        }
        out_groups.push(g);
    }
    let mut partners: BTreeMap<Id<Function>, Vec<Id<Function>>> = BTreeMap::new();
    for g in &out_groups {
        for &(x, y) in &g.pairs {
            partners.entry(x).or_default().push(y);
            partners.entry(y).or_default().push(x);
        }
    }
    for list in partners.values_mut() {
        list.sort();
        list.dedup();
    }
    let mut pairing = IpPairing {
        ips,
        groups: out_groups,
        notes,
        partners,
        at,
        infos,
        fns,
        heap: 0,
    };
    pairing.heap = pairing.walk_heap();
    pairing
}

/// `ip_pairing()`: every interruption point and its pairing (see the module documentation). Policy Memo,
/// incremental, a producer of the parts [`PartKey::Ip`].
#[derive(Debug)]
pub struct IpPairingQ;

impl Query for IpPairingQ {
    type Key = ();
    type Value = IpPairing;
    const FAMILY: &'static str = "ip_pairing";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;
    const PRODUCER: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<IpPairing> {
        Arc::new(pair(q))
    }

    fn run_incremental(q: &Q, _: &(), previous: Option<(&Arc<IpPairing>, Changes<'_>)>) -> Arc<IpPairing> {
        let Some((prev, changes)) = previous else {
            return Arc::new(pair(q));
        };
        if let Some(via) = changes.via() {
            let chain = pairing_setting(q).0 == PairMode::Chain;
            let mut same = true;
            for dep in via {
                if let Some(f) = q.via_key::<IpInfoOf>(dep) {
                    let now = q.get::<IpInfoOf>(f);
                    same &= prev
                        .infos
                        .get(&f)
                        .is_some_and(|old| Arc::ptr_eq(old, &now) || old.same_pairing(&now, chain));
                } else if q.via_key::<IpFunctions>(dep).is_some() {
                    same &= Arc::ptr_eq(&q.get::<IpFunctions>(()), &prev.fns);
                } else {
                    same = false;
                }
            }
            if same {
                return Arc::clone(prev);
            }
        }
        // The pairing is built whole: it depends on what this build reads, not on the points it read before
        // (a deleted point's `ip_info` included).
        q.drop_carried();
        let next = pair(q);
        let keys: BTreeSet<Id<Function>> = prev.ips.keys().chain(next.ips.keys()).copied().collect();
        q.changed_parts(
            keys.into_iter()
                .filter(|f| prev.part(*f) != next.part(*f))
                .map(PartKey::Ip),
        );
        Arc::new(next)
    }
}

/// `ip_notes()`: the EW-CORE-051 notes of the pairing (`IpPairing::notes`), pushed. The pairing itself is
/// note-free; the stitch graph and the interruption points' cross-references read this family, so their
/// readers see the notes as before the split. Policy Memo.
#[derive(Debug)]
pub struct IpNotes;

impl Query for IpNotes {
    type Key = ();
    type Value = Vec<DeriveNote>;
    const FAMILY: &'static str = "ip_notes";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Vec<DeriveNote>> {
        let notes = q.get::<IpPairingQ>(()).notes.clone();
        for n in &notes {
            q.note(n.clone());
        }
        Arc::new(notes)
    }
}
