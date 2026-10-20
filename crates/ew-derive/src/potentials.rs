// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The potential partition: `potential_partition()` (ARCH §6.4, doc 08 §3.3–§3.5, doc 03 §9.4).
//!
//! One union-find over the stitch nodes takes three snapshots (doc 08 §3.3): after the net-level edges
//! (nets), after the signal-level edges (signals) and after the potential-level edges (potentials).
//! Bonds and pass-through pairs never join.
//!
//! - **Which components are potentials.** A component holding a potential definition, or a joining
//!   connection and at least one CP that is not an interruption point. Components of unconnected points
//!   are not potentials (the stitch graph notes the unpaired ones).
//! - **Identity** (doc 08 §3.4): [`PotentialId::Def`] with the minimum definition ID when the potential has
//!   definitions, else [`PotentialId::Cp`] with the minimum member CP ID.
//! - **Name** (doc 08 §3.5): the first definition's name in ID order (another name is noted as
//!   EW-POT-017); else the first non-external source interruption point in page order (page position,
//!   anchor row, anchor column, ID); else the first external source interruption point in that order
//!   (review decision D6); else `<tag>:<designation>` of the first member CP in that order.
//! - **Aliases**: every other distinct IP name on the potential, external points included, in natural
//!   order. Points on a bond's `via` path (doc 03 §9.6) are named on the bond, not on the potential.
//! - **Own attributes** (doc 03 §9.4): type, voltage, environment and class from the first definition in
//!   ID order that sets each; the type else from the member CPs' `potential_role` (the first in CP order;
//!   a second distinct role is noted as EW-POT-001). Nothing merges silently (doc 08 §3.6): every later
//!   definition whose voltage, circuit class or (non-empty, compared as a whole) environment differs from
//!   the winning one, and every member CP role that contradicts a defined type (FE on PE or PEN, PE or N on
//!   PEN are compatible, doc 08 §3.7), is noted as EW-POT-001; the first definition's value applies.
//!   Propagation over pass-through pairs and `switched` belong to [`potential_flow`](crate::PotentialFlow).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::natural_cmp;
use ew_model::EndpointKey;
use ew_schema::{
    Bond, BondKind, CircuitClass, Connection, Function, FunctionKind, IpDirection, Junction, PotentialDef,
    PotentialEnv, PotentialType,
};
use ew_units::Dec3;

use crate::nets::{CpRef, Dsu, NetId, ends};
use crate::note::DeriveNote;
use crate::query::{Policy, Query};
use crate::stitch::{EdgeKind, Level, StitchGraph, StitchGraphQ};
use crate::{Cp, DeviceTag, DrawnPageOrder, FunctionCps, PageRef, Q, RepAnchorCell};

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

/// One potential.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Potential {
    /// The identity.
    pub id: PotentialId,
    /// The canonical name.
    pub name: PotentialName,
    /// The aliases: the other distinct IP names, natural order.
    pub aliases: Vec<String>,
    /// Every distinct IP name on the potential (bond paths excluded), natural order.
    pub ip_names: Vec<String>,
    /// Its definitions, in ID order.
    pub defs: Vec<Id<PotentialDef>>,
    /// The member CPs (interruption points included), in CP order.
    pub cps: Vec<CpRef>,
    /// The member junctions.
    pub junctions: Vec<Id<Junction>>,
    /// The stitch nodes, ascending.
    pub nodes: Vec<usize>,
    /// The signals (minimum member CP ID each), ascending.
    pub signals: Vec<NetId>,
    /// The nets, ascending.
    pub nets: Vec<NetId>,
    /// The interruption points on the potential, in ID order.
    pub ips: Vec<Id<Function>>,
    /// Type from a definition or a member role.
    pub ptype: Option<Attr<PotentialType>>,
    /// Voltage from a definition.
    pub voltage: Option<Attr<Dec3>>,
    /// AC/DC, frequency and earthing from a definition.
    pub env: Option<Attr<PotentialEnv>>,
    /// Circuit class from a definition.
    pub circuit_class: Option<Attr<CircuitClass>>,
}

/// One signal: nets joined by shorts (doc 08 §3.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Signal {
    /// The minimum member CP ID.
    pub id: NetId,
    /// The first IP name inside, in page order.
    pub name: Option<String>,
    /// Its nets, ascending.
    pub nets: Vec<NetId>,
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

/// The value of [`PotentialPartition`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Partition {
    /// The potentials in ID order.
    pub potentials: Vec<Potential>,
    /// The signals in ID order.
    pub signals: Vec<Signal>,
    /// The bonds in ID order.
    pub bonds: Vec<BondLink>,
    /// The notes of this derivation (also pushed to the `Q`).
    pub notes: Vec<DeriveNote>,
    of_node: Vec<Option<usize>>,
    by_id: BTreeMap<PotentialId, usize>,
}

impl Partition {
    /// The potential with this ID.
    pub fn get(&self, id: PotentialId) -> Option<&Potential> {
        self.by_id.get(&id).map(|&i| &self.potentials[i])
    }

    /// The first potential named `name` (canonical name).
    pub fn by_name(&self, name: &str) -> Option<&Potential> {
        self.potentials.iter().find(|p| p.name.text == name)
    }

    /// The index into `potentials` of a stitch node's potential.
    pub fn of_node(&self, node: usize) -> Option<usize> {
        self.of_node.get(node).copied().flatten()
    }

    /// The potential of a node (CP or junction) of `graph`.
    pub fn of(&self, graph: &StitchGraph, at: EndpointKey) -> Option<&Potential> {
        graph
            .node_of(at)
            .and_then(|n| self.of_node(n))
            .map(|i| &self.potentials[i])
    }
}

/// `potential_partition()` (doc 08 §3.3). Initial policy Memo (ARCH §6.4).
#[derive(Debug)]
pub struct PotentialPartition;

impl crate::heap::HeapBytes for Partition {
    fn heap_bytes(&self) -> usize {
        self.potentials.heap_bytes()
            + self.signals.heap_bytes()
            + self.bonds.heap_bytes()
            + self.notes.heap_bytes()
            + self.of_node.heap_bytes()
            + self.by_id.heap_bytes()
    }
}

impl Query for PotentialPartition {
    type Key = ();
    type Value = Partition;
    const FAMILY: &'static str = "potential_partition";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, _: &()) -> Arc<Partition> {
        let g = q.get::<StitchGraphQ>(());
        let n = g.nodes.len();
        let mut dsu: Dsu<usize> = Dsu::default();
        for i in 0..n {
            dsu.node(i);
        }
        let mut connected: Vec<bool> = g.nodes.iter().map(|x| x.connected).collect();
        let snapshot = |dsu: &mut Dsu<usize>| -> Vec<usize> { (0..n).map(|i| dsu.find(i)).collect() };
        let mut snaps: Vec<Vec<usize>> = Vec::new();
        for level in [Level::Net, Level::Signal, Level::Potential] {
            for e in g.edges.iter().filter(|e| e.level == level) {
                dsu.union(e.a, e.b);
            }
            snaps.push(snapshot(&mut dsu));
        }
        for e in &g.edges {
            if matches!(e.kind, EdgeKind::PageLess(_) | EdgeKind::Bridge(_)) {
                connected[e.a] = true;
                connected[e.b] = true;
            }
        }
        let mut notes = Vec::new();
        let functions = q.functions();
        let is_ip = |f: Id<Function>| {
            functions
                .get(f)
                .is_some_and(|f| matches!(f.kind, FunctionKind::Ip(_)))
        };

        // Definitions by node.
        let mut defs_of_node: BTreeMap<usize, Vec<Id<PotentialDef>>> = BTreeMap::new();
        for (id, d) in q.potential_defs().iter() {
            if let Some(node) = EndpointKey::of(&d.at).and_then(|at| g.node_of(at)) {
                defs_of_node.entry(node).or_default().push(id);
            }
        }

        // Group nodes by snapshot root.
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
        let net_id_of_node: Vec<Option<NetId>> = {
            let groups = group(&snaps[0]);
            let mut v = vec![None; n];
            for nodes in groups.values() {
                let id = ids_of(nodes);
                for &i in nodes {
                    v[i] = id;
                }
            }
            v
        };
        let mut signal_of_node: Vec<Option<NetId>> = vec![None; n];
        let mut signals = Vec::new();
        let ip_order = IpOrder::new(q, &g);
        for nodes in group(&snaps[1]).values() {
            let Some(id) = ids_of(nodes) else { continue };
            let mut nets: Vec<NetId> = nodes.iter().filter_map(|&i| net_id_of_node[i]).collect();
            nets.sort();
            nets.dedup();
            let name = nodes
                .iter()
                .flat_map(|&i| g.nodes[i].members.iter())
                .filter_map(|&at| CpRef::of(at))
                .filter(|cp| g.ips.contains_key(&cp.function))
                .min_by_key(|cp| ip_order.key(cp.function))
                .map(|cp| g.ips[&cp.function].name.clone());
            for &i in nodes {
                signal_of_node[i] = Some(id);
            }
            signals.push(Signal { id, name, nets });
        }
        signals.sort_by_key(|s| s.id);

        // Bond paths (doc 03 §9.6).
        let mut bonds = Vec::new();
        let mut on_bond_path: BTreeSet<Id<Function>> = BTreeSet::new();
        let bond_rows: Vec<(Id<Bond>, ew_schema::Bond)> =
            q.bonds().iter().map(|(id, b)| (id, (**b).clone())).collect();
        for (id, b) in &bond_rows {
            let path = b.via.map(|v| via_path_ips(q, &g, v)).unwrap_or_default();
            on_bond_path.extend(path.iter().copied());
            let mut names: Vec<String> = path
                .iter()
                .filter_map(|f| g.ips.get(f))
                .map(|i| i.name.clone())
                .collect();
            names.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
            names.dedup();
            bonds.push((*id, b.clone(), path, names));
        }

        // Potentials.
        let mut potentials = Vec::new();
        let mut of_node: Vec<Option<usize>> = vec![None; n];
        let defs_table = q.potential_defs();
        for nodes in group(&snaps[2]).values() {
            let mut defs: Vec<Id<PotentialDef>> = nodes
                .iter()
                .filter_map(|i| defs_of_node.get(i))
                .flatten()
                .copied()
                .collect();
            defs.sort();
            let mut cps: Vec<CpRef> = Vec::new();
            let mut junctions: Vec<Id<Junction>> = Vec::new();
            for &i in nodes {
                for &at in &g.nodes[i].members {
                    match at {
                        EndpointKey::Cp(f, idx) => cps.push(CpRef::new(f, idx)),
                        EndpointKey::Junction(j) => junctions.push(j),
                    }
                }
            }
            cps.sort();
            cps.dedup();
            junctions.sort();
            junctions.dedup();
            let has_conn = nodes.iter().any(|&i| connected[i]);
            let has_device_cp = cps.iter().any(|cp| !is_ip(cp.function));
            if defs.is_empty() && !(has_conn && has_device_cp) {
                continue;
            }
            let Some(min_cp) = cps.iter().map(|c| c.id()).min() else {
                continue;
            };
            let id = defs
                .first()
                .map_or(PotentialId::Cp(min_cp), |d| PotentialId::Def(*d));
            let ips: Vec<Id<Function>> = {
                let mut v: Vec<Id<Function>> = cps
                    .iter()
                    .map(|c| c.function)
                    .filter(|f| g.ips.contains_key(f))
                    .collect();
                v.dedup();
                v
            };
            let mut ip_names: Vec<String> = ips
                .iter()
                .filter(|f| !on_bond_path.contains(f))
                .map(|f| g.ips[f].name.clone())
                .collect();
            ip_names.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
            ip_names.dedup();

            // Name.
            let def_rows: Vec<(Id<PotentialDef>, &PotentialDef)> = defs
                .iter()
                .filter_map(|d| defs_table.get(*d).map(|r| (*d, r.as_ref())))
                .collect();
            let name = if let Some((d0, first)) = def_rows.first() {
                for (d, other) in def_rows.iter().skip(1) {
                    if other.name != first.name {
                        notes.push(DeriveNote::Connectivity {
                            code: "EW-POT-017",
                            subject: format!("potential {}", first.name.as_str()),
                            detail: format!(
                                "definition {d} names it {}; the first definition in ID order ({d0}) wins",
                                other.name.as_str()
                            ),
                        });
                    }
                }
                PotentialName {
                    text: first.name.as_str().to_owned(),
                    source: NameSource::Def(*d0),
                }
            } else if let Some(ip) = [false, true].into_iter().find_map(|external| {
                // Non-external source points name first; external ones (D6) only when none does.
                ips.iter()
                    .filter(|f| {
                        let i = &g.ips[*f];
                        i.direction == IpDirection::Source
                            && i.external == external
                            && !on_bond_path.contains(f)
                    })
                    .min_by_key(|f| ip_order.key(**f))
            }) {
                PotentialName {
                    text: g.ips[ip].name.clone(),
                    source: NameSource::SourceIp(*ip),
                }
            } else {
                fallback_name(q, &ip_order, &cps, &is_ip)
            };
            let aliases: Vec<String> = ip_names.iter().filter(|n| **n != name.text).cloned().collect();

            // Own attributes.
            let ptype = def_rows.iter().find_map(|(d, r)| {
                r.ptype.clone().map(|v| Attr {
                    value: v,
                    source: AttrSource::Def(*d),
                })
            });
            if let Some(t) = &ptype {
                for (d, r) in &def_rows {
                    if let Some(other) = &r.ptype
                        && *other != t.value
                    {
                        notes.push(DeriveNote::Connectivity {
                            code: "EW-POT-001",
                            subject: format!("potential {}", name.text),
                            detail: format!("definition {d} sets type {other:?}; {:?} applies", t.value),
                        });
                    }
                }
            }
            if let Some(t) = &ptype {
                role_conflicts(q, &cps, &is_ip, &name.text, &t.value, &mut notes);
            }
            let ptype = ptype.or_else(|| role_type(q, &cps, &is_ip, &name.text, &mut notes));
            let voltage = def_rows.iter().find_map(|(d, r)| {
                r.voltage.map(|v| Attr {
                    value: v,
                    source: AttrSource::Def(*d),
                })
            });
            if let Some(w) = &voltage {
                for (d, r) in &def_rows {
                    if let Some(other) = r.voltage
                        && other != w.value
                    {
                        notes.push(DeriveNote::Connectivity {
                            code: "EW-POT-001",
                            subject: format!("potential {}", name.text),
                            detail: format!(
                                "definition {d} sets voltage {other}; {} applies (first definition in ID order)",
                                w.value
                            ),
                        });
                    }
                }
            }
            let env = def_rows.iter().find_map(|(d, r)| {
                (!r.env.is_empty()).then(|| Attr {
                    value: r.env.clone(),
                    source: AttrSource::Def(*d),
                })
            });
            if let Some(w) = &env {
                for (d, r) in &def_rows {
                    if !r.env.is_empty() && r.env != w.value {
                        notes.push(DeriveNote::Connectivity {
                            code: "EW-POT-001",
                            subject: format!("potential {}", name.text),
                            detail: format!(
                                "definition {d} sets environment {:?}; {:?} applies (first definition in ID order)",
                                r.env, w.value
                            ),
                        });
                    }
                }
            }
            let circuit_class = def_rows.iter().find_map(|(d, r)| {
                r.circuit_class.clone().map(|v| Attr {
                    value: v,
                    source: AttrSource::Def(*d),
                })
            });
            if let Some(w) = &circuit_class {
                for (d, r) in &def_rows {
                    if let Some(other) = &r.circuit_class
                        && *other != w.value
                    {
                        notes.push(DeriveNote::Connectivity {
                            code: "EW-POT-001",
                            subject: format!("potential {}", name.text),
                            detail: format!(
                                "definition {d} sets circuit class {other:?}; {:?} applies (first definition in ID order)",
                                w.value
                            ),
                        });
                    }
                }
            }
            let mut sigs: Vec<NetId> = nodes.iter().filter_map(|&i| signal_of_node[i]).collect();
            sigs.sort();
            sigs.dedup();
            let mut nets: Vec<NetId> = nodes.iter().filter_map(|&i| net_id_of_node[i]).collect();
            nets.sort();
            nets.dedup();
            potentials.push(Potential {
                id,
                name,
                aliases,
                ip_names,
                defs,
                cps,
                junctions,
                nodes: nodes.clone(),
                signals: sigs,
                nets,
                ips,
                ptype,
                voltage,
                env,
                circuit_class,
            });
        }
        potentials.sort_by_key(|p| p.id);
        let mut by_id = BTreeMap::new();
        for (pi, p) in potentials.iter().enumerate() {
            by_id.insert(p.id, pi);
            for &i in &p.nodes {
                of_node[i] = Some(pi);
            }
        }
        let pot_at = |e: &ew_schema::Endpoint| {
            EndpointKey::of(e)
                .and_then(|at| g.node_of(at))
                .and_then(|i| of_node[i])
                .map(|pi| potentials[pi].id)
        };
        let bonds: Vec<BondLink> = bonds
            .into_iter()
            .map(|(id, b, path_ips, names)| BondLink {
                bond: id,
                kind: b.kind.clone(),
                a: pot_at(&b.a),
                b: pot_at(&b.b),
                via: b.via,
                path_ips,
                names,
            })
            .collect();
        notes.sort();
        notes.dedup();
        for note in &notes {
            q.note(note.clone());
        }
        Arc::new(Partition {
            potentials,
            signals,
            bonds,
            notes,
            of_node,
            by_id,
        })
    }
}

/// Page order of interruption points and member CPs: (page position, anchor row, anchor column, ID).
pub(crate) struct IpOrder {
    keys: BTreeMap<Id<Function>, (usize, u32, u32)>,
}

impl IpOrder {
    fn new(q: &Q, g: &StitchGraph) -> IpOrder {
        let order = q.get::<DrawnPageOrder>(());
        let mut keys = BTreeMap::new();
        for (f, info) in &g.ips {
            let pos = info
                .page
                .and_then(|p| order.position(PageRef::Drawn(p)))
                .unwrap_or(usize::MAX);
            let cell = info.rep.and_then(|r| q.get::<RepAnchorCell>(r).as_ref().clone());
            let (row, col) = cell.map_or((u32::MAX, u32::MAX), |c| (c.row, c.column));
            keys.insert(*f, (pos, row, col));
        }
        IpOrder { keys }
    }

    fn key(&self, f: Id<Function>) -> (usize, u32, u32, Id<Function>) {
        let (p, r, c) = self
            .keys
            .get(&f)
            .copied()
            .unwrap_or((usize::MAX, u32::MAX, u32::MAX));
        (p, r, c, f)
    }
}

/// The fallback name `<tag>:<designation>` of the first member CP (not an IP) in (page position, row,
/// column, CP ID) order of its function's first representation.
fn fallback_name(q: &Q, _ip: &IpOrder, cps: &[CpRef], is_ip: &dyn Fn(Id<Function>) -> bool) -> PotentialName {
    let order = q.get::<DrawnPageOrder>(());
    let reps = q.reps();
    let key = |cp: &CpRef| {
        let rep = q.idx().reps_of_function(cp.function).next();
        let pos = rep
            .and_then(|r| reps.get(r))
            .and_then(|r| order.position(PageRef::Drawn(r.page)))
            .unwrap_or(usize::MAX);
        let cell = rep.and_then(|r| q.get::<RepAnchorCell>(r).as_ref().clone());
        let (row, col) = cell.map_or((u32::MAX, u32::MAX), |c| (c.row, c.column));
        (pos, row, col, cp.id())
    };
    let first = cps
        .iter()
        .filter(|cp| !is_ip(cp.function))
        .min_by_key(|cp| key(cp))
        .or_else(|| cps.first())
        .copied();
    let Some(cp) = first else {
        return PotentialName {
            text: "?".to_owned(),
            source: NameSource::Fallback(CpRef::new(Id::from_uuid(Default::default()), 0)),
        };
    };
    let tag = q
        .functions()
        .get(cp.function)
        .and_then(|f| f.device)
        .and_then(|d| q.get::<DeviceTag>(d).as_ref().as_ref().map(|t| t.short()))
        .unwrap_or_else(|| "?".to_owned());
    let designation = q
        .get::<FunctionCps>(cp.function)
        .iter()
        .find(|c| c.idx == cp.idx)
        .map_or_else(|| cp.idx.to_string(), |c| c.designation.clone());
    PotentialName {
        text: format!("{tag}:{designation}"),
        source: NameSource::Fallback(cp),
    }
}

/// The roles of the member CPs that are not interruption points and declare one, in CP order.
fn member_roles<'a>(
    q: &'a Q,
    cps: &'a [CpRef],
    is_ip: &'a dyn Fn(Id<Function>) -> bool,
) -> impl Iterator<Item = (CpRef, PotentialType)> + 'a {
    cps.iter()
        .filter(move |cp| !is_ip(cp.function))
        .filter_map(move |cp| {
            let fcps = q.get::<FunctionCps>(cp.function);
            fcps.iter()
                .find(|c| c.idx == cp.idx)
                .and_then(|c| c.potential_role.clone())
                .map(|role| (*cp, role))
        })
}

/// Whether a member CP `role` is compatible with the type `defined` by a definition (doc 08 §3.7): equal
/// values, an FE role on a PE or PEN potential (a separate check, EW-POT-003), and a PE or N role on a PEN
/// potential.
fn role_compatible(defined: &PotentialType, role: &PotentialType) -> bool {
    use PotentialType::{Fe, N, Pe, Pen};
    defined == role || matches!((defined, role), (Pe | Pen, Fe) | (Pen, Pe | N))
}

/// Notes every member CP whose role contradicts the type `defined` by a definition (doc 08 §3.6).
fn role_conflicts(
    q: &Q,
    cps: &[CpRef],
    is_ip: &dyn Fn(Id<Function>) -> bool,
    name: &str,
    defined: &PotentialType,
    notes: &mut Vec<DeriveNote>,
) {
    for (cp, role) in member_roles(q, cps, is_ip) {
        if !role_compatible(defined, &role) {
            notes.push(DeriveNote::Connectivity {
                code: "EW-POT-001",
                subject: format!("potential {name}"),
                detail: format!(
                    "member CP {}:{} has role {role:?}; the defined type {defined:?} applies",
                    cp.function, cp.idx
                ),
            });
        }
    }
}

/// The type from member CP roles: the first role in CP order; another distinct role is noted.
fn role_type(
    q: &Q,
    cps: &[CpRef],
    is_ip: &dyn Fn(Id<Function>) -> bool,
    name: &str,
    notes: &mut Vec<DeriveNote>,
) -> Option<Attr<PotentialType>> {
    let mut found: Option<Attr<PotentialType>> = None;
    let mut others: BTreeSet<String> = BTreeSet::new();
    for (cp, role) in member_roles(q, cps, is_ip) {
        match &found {
            None => {
                found = Some(Attr {
                    value: role,
                    source: AttrSource::Role(cp),
                });
            }
            Some(f) if f.value != role => {
                others.insert(format!("{role:?}"));
            }
            Some(_) => {}
        }
    }
    if let Some(f) = &found
        && !others.is_empty()
    {
        notes.push(DeriveNote::Connectivity {
            code: "EW-POT-001",
            subject: format!("potential {name}"),
            detail: format!(
                "member roles {} meet {:?}; the first in CP order applies",
                others.into_iter().collect::<Vec<_>>().join(", "),
                f.value
            ),
        });
    }
    found
}

/// The interruption points on the `via` path of a bond (doc 03 §9.6): from the via connection's ends,
/// through IP pairs and the single drawn connection of each paired point, up to the first node that is
/// not an interruption point.
fn via_path_ips(q: &Q, g: &StitchGraph, via: Id<Connection>) -> Vec<Id<Function>> {
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
        if !g.ips.contains_key(&cp.function) || !seen.insert(cp.function) {
            continue;
        }
        for &partner in g.ip_partners(cp.function) {
            if !seen.insert(partner) {
                continue;
            }
            let Some(pcp) = g.ips[&partner].cp else { continue };
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
                        && g.ips.contains_key(&n.function)
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
