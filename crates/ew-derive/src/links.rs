// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-function and per-terminal inputs of the stitch graph, as memoised families (WP-S1.07 part 2):
//!
//! - **`function_links(f)`**: what a function contributes to the stitch graph whatever is drawn: its declared
//!   FnDef links, the duplicated-pin and terminal-level groups of its CPs, and the flow data of the potential
//!   flow (protective device, N switched with L, terminal contact, source-output CPs). The stitch graph keeps
//!   the links between CPs that are in the graph (doc 08 §3.3), so presence is applied by the reader.
//! - **`rail_terminal(d)`** and **`rail_bonds()`**: the PE CPs and location of every rail-bonded terminal, and
//!   per location the PE CPs that the rail joins (doc 08 §3.7, review decision D5).
//! - **`bond_paths()`**: the interruption points on the `via` path of every bond and their names (doc 03 §9.6).
//!
//! All four are note-free (`Q::detached`): `rail_terminal` keeps its EW-TERM-005, EW-POT-008 and cycle notes in
//! its value, and the stitch graph pushes them.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::natural_cmp;
use ew_model::EndpointKey;
use ew_rds::Aspect;
use ew_schema::{
    Bond, ClampKey, Connection, ContactKind, Device, Function, FunctionKind, InternalKind, PotentialType,
    Side, StructureNode,
};

use crate::cps::pattern_origin;
use crate::heap::HeapBytes;
use crate::index::{BondIndex, Index, RailTerminals};
use crate::ip::IpPairingQ;
use crate::nets::{CpRef, ends};
use crate::note::DeriveNote;
use crate::query::{Changes, Policy, Query};
use crate::{FunctionCps, Q, StructureOf};

// ---------------------------------------------------------------------------------------------------------
// function_links

/// A link a function definition declares between two existing CPs.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DeclaredLink {
    /// A `Short` (signal level).
    Short {
        /// First CP index.
        a: u16,
        /// Second CP index.
        b: u16,
    },
    /// A `PassThrough` pair (a flow edge, never joined); `a` and `b` keep the FnDef's order.
    PassThrough {
        /// First CP index.
        a: u16,
        /// Second CP index.
        b: u16,
        /// The contact can open.
        opens: bool,
    },
}

/// The value of [`FunctionLinksOf`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FunctionLinks {
    /// The declared links whose two CPs exist, in FnDef order.
    pub declared: Vec<DeclaredLink>,
    /// Every declared pass-through pair as (smaller, larger) index, existing CPs or not: a chain skips them.
    pub passes: BTreeSet<(u16, u16)>,
    /// Duplicated pins (doc 03 §9.3, review decision D2; a black box or a part-pattern-origin function that is no
    /// terminal): the CP indices of one (designation, level), in CP order, per group in key order.
    pub dup_groups: Vec<Vec<u16>>,
    /// The terminal level rule (doc 08 §6.2, review decision D3; FnDef contact kind terminal): the CP indices of
    /// one level (and one side when the FnDef declares a pass-through), with the level, in key order.
    pub level_groups: Vec<(u8, Vec<u16>)>,
    /// Flow (doc 08 §3.6): the FnDef's category is a protective device.
    pub protection: bool,
    /// Flow: the FnDef switches N with L.
    pub switches_n: bool,
    /// Flow: the FnDef's contact kind is terminal (never a load).
    pub terminal_contact: bool,
    /// Flow: the indices of its source-output CPs.
    pub source_outputs: Vec<u16>,
}

impl FunctionLinks {
    /// Whether the function contributes no link at all.
    pub fn is_empty(&self) -> bool {
        self.declared.is_empty() && self.dup_groups.is_empty() && self.level_groups.is_empty()
    }
}

impl HeapBytes for FunctionLinks {
    fn heap_bytes(&self) -> usize {
        self.declared.capacity() * std::mem::size_of::<DeclaredLink>()
            + self.passes.len() * std::mem::size_of::<(u16, u16)>()
            + self.dup_groups.heap_bytes()
            + self
                .level_groups
                .iter()
                .map(|(_, v)| std::mem::size_of::<(u8, Vec<u16>)>() + v.capacity() * 2)
                .sum::<usize>()
            + self.source_outputs.capacity() * 2
    }
}

/// `function_links(f)`: see the module documentation. Empty for an interruption point, a missing function or
/// a missing function definition. Policy Memo.
#[derive(Debug)]
pub struct FunctionLinksOf;

impl Query for FunctionLinksOf {
    type Key = Id<Function>;
    type Value = FunctionLinks;
    const FAMILY: &'static str = "function_links";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, fid: &Id<Function>) -> Arc<FunctionLinks> {
        let fid = *fid;
        Arc::new(q.detached(|| function_links(q, fid)))
    }
}

fn function_links(q: &Q, fid: Id<Function>) -> FunctionLinks {
    let mut out = FunctionLinks::default();
    let Some(f) = q.functions().get(fid) else {
        return out;
    };
    if matches!(f.kind, FunctionKind::Ip(_)) {
        return out;
    }
    let Some(def) = q.fndefs().get(f.def) else {
        return out;
    };
    let cps = q.get::<FunctionCps>(fid);
    out.protection = def.item.category.0.as_str().starts_with("Protection");
    out.switches_n = def.item.flags.switches_n_with_l;
    out.terminal_contact = def.item.contact_kind == ContactKind::Terminal;
    out.source_outputs = cps.iter().filter(|c| c.source_output).map(|c| c.idx).collect();
    let exists = |i: u16| cps.iter().any(|c| c.idx == i);
    let mut has_pass_through = false;
    for link in &def.item.internal {
        let declared = match &link.kind {
            InternalKind::Short => DeclaredLink::Short { a: link.a, b: link.b },
            InternalKind::PassThrough(p) => {
                has_pass_through = true;
                out.passes.insert((link.a.min(link.b), link.a.max(link.b)));
                DeclaredLink::PassThrough {
                    a: link.a,
                    b: link.b,
                    opens: p.opens,
                }
            }
            InternalKind::Unknown(_) => continue,
        };
        if exists(link.a) && exists(link.b) {
            out.declared.push(declared);
        }
    }
    let terminal = def.item.contact_kind == ContactKind::Terminal
        || f.device
            .and_then(|d| q.devices().get(d))
            .is_some_and(|d| d.aspects.terminal.is_some());
    if !terminal && pattern_origin(q, f) {
        let mut by_designation: BTreeMap<(&str, Option<u8>), Vec<u16>> = BTreeMap::new();
        for c in cps.iter() {
            by_designation
                .entry((c.designation.as_str(), c.level))
                .or_default()
                .push(c.idx);
        }
        out.dup_groups = by_designation.into_values().filter(|v| v.len() > 1).collect();
    }
    if def.item.contact_kind == ContactKind::Terminal {
        let mut groups: BTreeMap<(u8, u8), Vec<u16>> = BTreeMap::new();
        for c in cps.iter() {
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
        out.level_groups = groups
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|((level, _), v)| (level, v))
            .collect();
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Rail bonding

/// The value of [`RailTerminalOf`]: one rail-bonded terminal.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RailTerminal {
    /// Its PE CPs (rail-foot clamp, or potential role PE or PEN), in function and CP order; the rail joins
    /// those that are in the stitch graph.
    pub pe: Vec<CpRef>,
    /// Its location; `None` without one (EW-POT-008). A structure cycle leaves it out entirely (`skipped`).
    pub location: Option<Id<StructureNode>>,
    /// Its location could not be resolved (a structure cycle, noted as `cycle`): it joins nothing, unnoted.
    pub skipped: bool,
    /// Its EW-TERM-005 and EW-POT-008 notes, in the stitch graph's order.
    pub notes: Vec<DeriveNote>,
    /// The cycle note, pushed by the stitch graph outside its notes (as the cycle-7 stitch graph did).
    pub cycle: Option<DeriveNote>,
}

impl HeapBytes for RailTerminal {
    fn heap_bytes(&self) -> usize {
        self.pe.capacity() * std::mem::size_of::<CpRef>() + self.notes.heap_bytes()
    }
}

/// `rail_terminal(d)`: see [`RailTerminal`]. Policy Memo.
#[derive(Debug)]
pub struct RailTerminalOf;

impl Query for RailTerminalOf {
    type Key = Id<Device>;
    type Value = RailTerminal;
    const FAMILY: &'static str = "rail_terminal";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, dev: &Id<Device>) -> Arc<RailTerminal> {
        let dev = *dev;
        Arc::new(q.detached(|| {
            let mut out = RailTerminal::default();
            let mut has_pe = false;
            for f in q.idx().functions_of_device(dev) {
                for c in q.get::<FunctionCps>(f).iter() {
                    let is_pe = c.clamp == Some(ClampKey::RailFoot)
                        || matches!(c.potential_role, Some(PotentialType::Pe | PotentialType::Pen));
                    has_pe |= is_pe;
                    if is_pe {
                        out.pe.push(CpRef::new(f, c.idx));
                    }
                }
            }
            let location = match q.try_get::<StructureOf>(dev) {
                Ok(s) => s.node(Aspect::Location),
                Err(cycle) => {
                    out.skipped = true;
                    out.cycle = Some(DeriveNote::QueryCycle {
                        family: cycle.family,
                        key: cycle.key,
                    });
                    return out;
                }
            };
            if !has_pe {
                out.notes.push(DeriveNote::Connectivity {
                    code: "EW-TERM-005",
                    subject: dev.to_string(),
                    detail: "rail_bonded is set but the terminal has no PE CP; it joins no location PE node"
                        .into(),
                });
            }
            if location.is_none() {
                out.notes.push(DeriveNote::Connectivity {
                    code: "EW-POT-008",
                    subject: dev.to_string(),
                    detail: "the rail-bonded terminal has no location; it joins no location PE node".into(),
                });
            }
            out.location = location;
            out
        }))
    }
}

/// The value of [`RailBondsQ`]. Two values are equal when their joins and notes are (the terminal values kept
/// for the incremental check are not compared).
#[derive(Clone, Eq, Debug, Default)]
pub struct RailBonds {
    /// Per location, the PE CPs of its rail-bonded terminals, sorted.
    pub by_location: BTreeMap<Id<StructureNode>, Vec<CpRef>>,
    /// The location of each of those PE CPs.
    pub location_of: BTreeMap<CpRef, Id<StructureNode>>,
    /// The terminals' notes, in device order.
    pub notes: Vec<DeriveNote>,
    /// The terminals' cycle notes, in device order.
    pub cycles: Vec<DeriveNote>,
    /// The terminal values read, for the incremental check.
    terminals: BTreeMap<Id<Device>, Arc<RailTerminal>>,
    /// The terminal index read.
    devices: Arc<Index<Id<Device>, ()>>,
}

impl PartialEq for RailBonds {
    fn eq(&self, other: &RailBonds) -> bool {
        self.by_location == other.by_location
            && self.location_of == other.location_of
            && self.notes == other.notes
            && self.cycles == other.cycles
    }
}

impl HeapBytes for RailBonds {
    fn heap_bytes(&self) -> usize {
        self.by_location.heap_bytes()
            + self.location_of.len()
                * (std::mem::size_of::<CpRef>() + std::mem::size_of::<Id<StructureNode>>())
            + self.notes.heap_bytes()
            + self.cycles.heap_bytes()
            + self.terminals.len() * 2 * std::mem::size_of::<usize>()
    }
}

fn rail_bonds(q: &Q) -> RailBonds {
    let devices = q.get::<RailTerminals>(());
    let mut out = RailBonds::default();
    for dev in devices.keys().copied() {
        let t = q.get::<RailTerminalOf>(dev);
        out.cycles.extend(t.cycle.iter().cloned());
        out.notes.extend(t.notes.iter().cloned());
        if let Some(location) = t.location {
            out.by_location
                .entry(location)
                .or_default()
                .extend(t.pe.iter().copied());
            for cp in &t.pe {
                out.location_of.insert(*cp, location);
            }
        }
        out.terminals.insert(dev, t);
    }
    for cps in out.by_location.values_mut() {
        cps.sort();
    }
    out.devices = devices;
    out
}

/// `rail_bonds()`: the rail joins of every location (see the module documentation). Incremental: it keeps its
/// previous value when none of the terminal values it read changed. Policy Memo.
#[derive(Debug)]
pub struct RailBondsQ;

impl Query for RailBondsQ {
    type Key = ();
    type Value = RailBonds;
    const FAMILY: &'static str = "rail_bonds";
    const POLICY: Policy = Policy::Memo;
    const INCREMENTAL: bool = true;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<RailBonds> {
        Arc::new(rail_bonds(q))
    }

    fn run_incremental(q: &Q, _: &(), previous: Option<(&Arc<RailBonds>, Changes<'_>)>) -> Arc<RailBonds> {
        if let Some((prev, changes)) = previous
            && let Some(via) = changes.via()
        {
            let mut same = true;
            for dep in via {
                if let Some(d) = q.via_key::<RailTerminalOf>(dep) {
                    let now = q.get::<RailTerminalOf>(d);
                    same &= prev.terminals.get(&d).is_some_and(|old| Arc::ptr_eq(old, &now));
                } else if q.via_key::<RailTerminals>(dep).is_some() {
                    same &= Arc::ptr_eq(&q.get::<RailTerminals>(()), &prev.devices);
                } else {
                    same = false;
                }
            }
            if same {
                return Arc::clone(prev);
            }
        }
        // Built whole: it depends on what this build reads only.
        q.drop_carried();
        Arc::new(rail_bonds(q))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Bond paths

/// The interruption points on one bond's `via` path.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BondPath {
    /// The points, in ID order.
    pub ips: Vec<Id<Function>>,
    /// Their distinct names, natural order: the bond's names ("PE-0VDC" names B1).
    pub names: Vec<String>,
}

/// The value of [`BondPathsQ`].
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BondPaths {
    /// Per bond, its path.
    pub paths: BTreeMap<Id<Bond>, BondPath>,
    /// Every point on some bond's path (named on the bond, not on a potential).
    pub on_path: BTreeSet<Id<Function>>,
}

impl HeapBytes for BondPaths {
    fn heap_bytes(&self) -> usize {
        self.paths
            .values()
            .map(|p| {
                std::mem::size_of::<(Id<Bond>, BondPath)>()
                    + p.ips.capacity() * std::mem::size_of::<Id<Function>>()
                    + p.names.heap_bytes()
            })
            .sum::<usize>()
            + self.on_path.len() * std::mem::size_of::<Id<Function>>()
    }
}

/// `bond_paths()`: see [`BondPaths`]. Policy Memo (bonds are few).
#[derive(Debug)]
pub struct BondPathsQ;

impl Query for BondPathsQ {
    type Key = ();
    type Value = BondPaths;
    const FAMILY: &'static str = "bond_paths";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<BondPaths> {
        let bonds = q.get::<BondIndex>(());
        let ip = q.get::<IpPairingQ>(());
        let mut out = BondPaths::default();
        for (id, b) in bonds.iter() {
            let ips = b.via.map(|v| via_path_ips(q, &ip, v)).unwrap_or_default();
            out.on_path.extend(ips.iter().copied());
            let mut names: Vec<String> = ips
                .iter()
                .filter_map(|f| ip.ips.get(f))
                .map(|i| i.name.clone())
                .collect();
            names.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
            names.dedup();
            out.paths.insert(*id, BondPath { ips, names });
        }
        Arc::new(out)
    }
}

/// The interruption points on the `via` path of a bond (doc 03 §9.6): from the via connection's ends,
/// through IP pairs and the single drawn connection of each paired point, up to the first node that is not
/// an interruption point.
fn via_path_ips(q: &Q, ip: &crate::ip::IpPairing, via: Id<Connection>) -> Vec<Id<Function>> {
    let connections = q.connections();
    let Some(c) = connections.get(via) else {
        return Vec::new();
    };
    let Some((a, b)) = ends(c) else {
        return Vec::new();
    };
    let mut seen: BTreeSet<Id<Function>> = BTreeSet::new();
    let mut seen_conn: BTreeSet<Id<Connection>> = BTreeSet::from([via]);
    let mut stack: Vec<EndpointKey> = vec![a, b];
    while let Some(at) = stack.pop() {
        let Some(cp) = CpRef::of(at) else { continue };
        if !ip.ips.contains_key(&cp.function) || !seen.insert(cp.function) {
            continue;
        }
        for &partner in ip.partners(cp.function) {
            if !seen.insert(partner) {
                continue;
            }
            let Some(pcp) = ip.ips[&partner].cp else { continue };
            let drawn: Vec<Id<Connection>> = q
                .idx()
                .connections_at(pcp.key())
                .filter(|id| !seen_conn.contains(id))
                .collect();
            if let [only] = drawn[..] {
                seen_conn.insert(only);
                if let Some((x, y)) = connections.get(only).and_then(|c| ends(c)) {
                    let next = if x == pcp.key() { y } else { x };
                    // Continue only through further interruption points.
                    if let Some(n) = CpRef::of(next)
                        && ip.ips.contains_key(&n.function)
                    {
                        seen.remove(&n.function);
                        stack.push(next);
                    }
                }
            }
        }
    }
    seen.into_iter().collect()
}
