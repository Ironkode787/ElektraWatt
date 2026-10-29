// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The names and own attributes of a potential, split out of the partition (ADR-0002 amendment 4: "a folio
//! renumber or a move re-derives names and not the topology"; ARCH §6.4 `potential_names`; WP-S1.07 part 2).
//!
//! - **`potential_name(p)`** (doc 08 §3.5): the name of the first definition in rank order ([`ranked_defs`]: natural
//!   order of the name, the defining point's printed text, the ID; WP-S1.16 D9) (another name is noted as EW-POT-017); else the first non-external source interruption point in page order (page position, anchor
//!   row, anchor column, ID); else the first external source interruption point in that order (review decision
//!   D6); else (doc 08 §3.5 step 2b, WP-S1.16 D1) the supply's name `<root>/<tag>:<designation>` when the
//!   potential is entered through the pass-through pair of a function f (its flow `upstream`): `<root>` is the
//!   name of the nearest upstream potential named by a definition or a source point (the walk passes potentials
//!   named by this step), `<tag>` f's displayed tag without its leading aspect sign and `<designation>` the
//!   pair's downstream CP (`L/S51:2`); else `<tag>:<designation>` of the first member CP in (page position, row,
//!   column) order, ties by that name in natural order and then by CP ID. The aliases are every other distinct IP name on the potential,
//!   external points included, in natural order; points on a bond's `via` path (doc 03
//!   §9.6) are named on the bond, not on the potential. A signal takes the first IP name inside it, in page
//!   order. The family also pushes the EW-POT-001 notes of the potential's attributes, which name it.
//! - **`potential_attrs(p)`** (doc 03 §9.4): type, voltage, environment and class from the first definition in
//!   rank order ([`ranked_defs`]) that sets each; the conflict texts name definitions and CPs by their printed text,
//!   in printed order, never by ID (FT-587); the type else from the member CPs' `potential_role`; roles that disagree are
//!   ranked by the fixed role order of `role_rank` (PE, PEN, FE, N, L1, L2, L3, L, L+, L−, M, signal,
//!   undefined), never by the CP order, which follows the random IDs of a new project.
//!   Nothing merges silently (doc 08 §3.6): every later definition whose value differs, and every member role
//!   that contradicts a defined type (FE on PE or PEN, PE or N on PEN are compatible, doc 08 §3.7), is a
//!   conflict, which `potential_name` reports as EW-POT-001 with the potential's name. The family is
//!   note-free, so a rename does not reach the readers of the attributes (the wire properties, the flow).
//!
//! Both read one part of the partition ([`PartKey::Potential`]): an edit elsewhere does not reach them.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_mltext::natural_cmp;
use ew_schema::{Endpoint, Function, IpDirection, PotentialDef, PotentialType};

use crate::flow::FlowBfs;
use crate::heap::HeapBytes;
use crate::ip::{IpFnInfo, IpInfoOf};
use crate::links::BondPathsQ;
use crate::memo_keys::PartKey;
use crate::nets::{CpRef, NetId};
use crate::note::DeriveNote;
use crate::potentials::{
    Attr, AttrSource, NameSource, Potential, PotentialAttrs, PotentialId, PotentialName, PotentialPartition,
};
use crate::query::{Policy, Query};
use crate::{DeviceTag, DrawnPageOrder, FunctionCps, PageRef, Q, RepAnchorCell};

/// The value of [`PotentialNameOf`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PotentialNames {
    /// The canonical name.
    pub name: PotentialName,
    /// The aliases: the other distinct IP names, natural order.
    pub aliases: Vec<String>,
    /// Every distinct IP name on the potential (bond paths excluded), natural order.
    pub ip_names: Vec<String>,
    /// The name of each signal of the potential (ID order): the first IP name inside it in page order.
    pub signals: Vec<(NetId, Option<String>)>,
}

impl HeapBytes for PotentialNames {
    fn heap_bytes(&self) -> usize {
        self.name.text.capacity()
            + self.aliases.heap_bytes()
            + self.ip_names.heap_bytes()
            + self
                .signals
                .iter()
                .map(|(_, n)| std::mem::size_of::<(NetId, Option<String>)>() + n.heap_bytes())
                .sum::<usize>()
    }
}

/// The name of an unknown potential.
fn unknown() -> PotentialNames {
    PotentialNames {
        name: PotentialName {
            text: "?".to_owned(),
            source: NameSource::Fallback(CpRef::new(Id::from_uuid(Default::default()), 0)),
        },
        aliases: Vec::new(),
        ip_names: Vec::new(),
        signals: Vec::new(),
    }
}

/// The page-order key of an interruption point: (page position, anchor row, anchor column, ID).
fn ip_key(info: &IpFnInfo, f: Id<Function>) -> (usize, u32, u32, Id<Function>) {
    (info.order.0, info.order.1, info.order.2, f)
}

/// `potential_name(p)`: see the module documentation. Policy Memo.
#[derive(Debug)]
pub struct PotentialNameOf;

impl Query for PotentialNameOf {
    type Key = PotentialId;
    type Value = PotentialNames;
    const FAMILY: &'static str = "potential_name";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, key: &PotentialId) -> Arc<PotentialNames> {
        let partition = q.get_part::<PotentialPartition>((), PartKey::Potential(*key));
        let Some(comp) = partition.component_of(*key) else {
            return Arc::new(unknown());
        };
        let Some(p) = &comp.potential else {
            return Arc::new(unknown());
        };
        let on_path = q.get_value_part::<BondPathsQ>(());
        let on_path = &on_path.on_path;
        let infos: Vec<(Id<Function>, Arc<IpFnInfo>)> =
            p.ips.iter().map(|f| (*f, q.get::<IpInfoOf>(*f))).collect();
        let info = |f: Id<Function>| infos.iter().find(|(g, _)| *g == f).map(|(_, i)| i);
        let mut notes: Vec<DeriveNote> = Vec::new();

        let mut ip_names: Vec<String> = infos
            .iter()
            .filter(|(f, _)| !on_path.contains(f))
            .map(|(_, i)| i.name().to_owned())
            .collect();
        ip_names.sort_by(|a, b| natural_cmp(a, b).then_with(|| a.cmp(b)));
        ip_names.dedup();

        let def_rows = ranked_defs(q, p);
        let name = if let Some((d0, first)) = def_rows.first() {
            for (_, other) in def_rows.iter().skip(1) {
                if other.name != first.name {
                    notes.push(DeriveNote::Connectivity {
                        code: "EW-POT-017",
                        subject: format!("potential {}", first.name.as_str()),
                        detail: format!(
                            "{} names it {}; the first definition in natural order ({}) wins",
                            def_text(q, other),
                            other.name.as_str(),
                            def_text(q, first)
                        ),
                    });
                }
            }
            PotentialName {
                text: first.name.as_str().to_owned(),
                source: NameSource::Def(*d0),
            }
        } else if let Some((f, i)) = [false, true].into_iter().find_map(|external| {
            // Non-external source points name first; external ones (D6) only when none does.
            infos
                .iter()
                .filter(|(f, i)| {
                    i.spec.as_ref().is_some_and(|s| {
                        s.direction == IpDirection::Source && s.external.is_some() == external
                    }) && !on_path.contains(f)
                })
                .min_by_key(|(f, i)| ip_key(i, *f))
        }) {
            PotentialName {
                text: i.name().to_owned(),
                source: NameSource::SourceIp(*f),
            }
        } else if let Some(supply) = supply_name(q, *key) {
            supply
        } else {
            fallback_name(q, p)
        };
        let aliases: Vec<String> = ip_names.iter().filter(|n| **n != name.text).cloned().collect();
        let signals: Vec<(NetId, Option<String>)> = comp
            .signals
            .iter()
            .map(|s| {
                let first = s
                    .ips
                    .iter()
                    .filter_map(|f| info(*f).map(|i| (ip_key(i, *f), i)))
                    .min_by_key(|(k, _)| *k)
                    .map(|(_, i)| i.name().to_owned());
                (s.id, first)
            })
            .collect();

        // The attribute conflicts, named.
        for detail in &q.get::<PotentialAttrsOf>(*key).conflicts {
            notes.push(DeriveNote::Connectivity {
                code: "EW-POT-001",
                subject: format!("potential {}", name.text),
                detail: detail.clone(),
            });
        }
        notes.sort();
        notes.dedup();
        for n in notes {
            q.note(n);
        }
        Arc::new(PotentialNames {
            name,
            aliases,
            ip_names,
            signals,
        })
    }
}

/// Step 2b of doc 08 §3.5 (WP-S1.16 D1): `<root>/<tag without aspect sign>:<designation>` of the pass-through
/// pair that enters `key` from upstream (its flow entry's `upstream`), the designation the pair's CP on this side.
/// `<root>` is the name of the nearest upstream potential named by a definition or a source point; potentials
/// named by this step are walked through. `None` (step 3 applies) when the potential has no upstream, when the
/// walk reaches a potential named by the fallback or a revisit (the flow is a tree; the guard ends a walk that
/// would not be one).
fn supply_name(q: &Q, key: PotentialId) -> Option<PotentialName> {
    let entry = |p: PotentialId| {
        q.get_part::<FlowBfs>((), PartKey::Flow(p))
            .entries
            .get(&p)
            .and_then(|e| e.upstream)
    };
    let up = entry(key)?;
    let mut seen: BTreeSet<PotentialId> = BTreeSet::from([key]);
    let mut at = up.from;
    let root = loop {
        if !seen.insert(at) {
            return None;
        }
        let names = q.get::<PotentialNameOf>(at);
        match names.name.source {
            NameSource::Def(_) | NameSource::SourceIp(_) => break names.name.text.clone(),
            NameSource::Supply { .. } => at = entry(at)?.from,
            NameSource::Fallback(_) => return None,
        }
    };
    let text = cp_text(q, CpRef::new(up.function, up.to_idx));
    let text = text.strip_prefix('-').unwrap_or(&text);
    Some(PotentialName {
        text: format!("{root}/{text}"),
        source: NameSource::Supply { via: up.function },
    })
}

/// A definition and its record, as [`ranked_defs`] lists them.
pub type RankedDef = (Id<PotentialDef>, Arc<PotentialDef>);

/// The definitions of a potential in their rank (doc 08 §3.6, ID-175; WP-S1.16 D9, FT-587): natural order of the
/// name, then the defining point's printed text (`<tag>:<designation>` of its CP, a junction's after every CP's), then
/// the ID. The first applies its values and names the potential; the ID decides only between two definitions that
/// print alike, so the applied values and the texts do not follow the random IDs of a new project. A potential with
/// one definition reads nothing else.
pub fn ranked_defs(q: &Q, p: &Potential) -> Vec<RankedDef> {
    let mut rows: Vec<RankedDef> = p
        .defs
        .iter()
        .filter_map(|d| q.potential_defs().get(*d).map(|r| (*d, Arc::clone(r))))
        .collect();
    if rows.len() < 2 {
        return rows;
    }
    let mut keyed: Vec<(Option<String>, RankedDef)> = rows
        .drain(..)
        .map(|(d, r)| (def_point_text(q, &r), (d, r)))
        .collect();
    keyed.sort_by(|(ta, (da, ra)), (tb, (db, rb))| {
        natural_cmp(ra.name.as_str(), rb.name.as_str())
            .then_with(|| ra.name.as_str().cmp(rb.name.as_str()))
            .then_with(|| match (ta, tb) {
                (Some(a), Some(b)) => natural_cmp(a, b).then_with(|| a.cmp(b)),
                (a, b) => a.is_none().cmp(&b.is_none()),
            })
            .then_with(|| da.cmp(db))
    });
    keyed.into_iter().map(|(_, row)| row).collect()
}

/// The printed text of a definition's point: `<tag>:<designation>` of its CP; `None` for a junction or a point of a
/// newer format.
fn def_point_text(q: &Q, def: &PotentialDef) -> Option<String> {
    match def.at {
        Endpoint::Cp(f, idx) => Some(cp_text(q, CpRef::new(f, idx))),
        _ => None,
    }
}

/// A definition as a message prints it: `definition <name> at <tag>:<designation>` (`definition <name>` at a junction),
/// never its ID (FT-587).
pub fn def_text(q: &Q, def: &PotentialDef) -> String {
    match def_point_text(q, def) {
        Some(at) => format!("definition {} at {at}", def.name.as_str()),
        None => format!("definition {}", def.name.as_str()),
    }
}

/// `<tag>:<designation>` of a CP: its function's device tag as a report prints it (`?` without one) and the CP's
/// designation (its index without one).
pub fn cp_text(q: &Q, cp: CpRef) -> String {
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
    format!("{tag}:{designation}")
}

/// The fallback name `<tag>:<designation>` of the first member CP (not an IP) in (page position, row, column) order
/// of its function's first representation; ties go to the name in natural order (then bytes) and only then to the CP
/// ID, so two CPs of one anchor cell (a terminal's two sides) name the potential alike in every fresh copy of a
/// project (WP-S6.13 §6 item 3: an ID of a new project is random).
fn fallback_name(q: &Q, p: &Potential) -> PotentialName {
    let is_ip = |f: Id<Function>| p.ips.binary_search(&f).is_ok();
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
        (pos, row, col)
    };
    let keyed: Vec<_> = p
        .cps
        .iter()
        .filter(|cp| !is_ip(cp.function))
        .map(|cp| (key(cp), *cp))
        .collect();
    // Only the CPs that tie on the first key are named, so the family reads no other member's tag.
    let first = keyed
        .iter()
        .map(|(k, _)| *k)
        .min()
        .map_or_else(
            || p.cps.first().copied().into_iter().collect(),
            |min| {
                keyed
                    .iter()
                    .filter(|(k, _)| *k == min)
                    .map(|(_, cp)| *cp)
                    .collect::<Vec<_>>()
            },
        )
        .into_iter()
        .map(|cp| (cp_text(q, cp), cp))
        .min_by(|(ta, a), (tb, b)| {
            natural_cmp(ta, tb)
                .then_with(|| ta.cmp(tb))
                .then_with(|| a.id().cmp(&b.id()))
        });
    let Some((text, cp)) = first else {
        return unknown().name;
    };
    PotentialName {
        text,
        source: NameSource::Fallback(cp),
    }
}

/// The value of [`PotentialAttrsOf`]: the own attributes and their conflicts.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PotentialAttrsV {
    /// The attributes.
    pub attrs: PotentialAttrs,
    /// The details of the EW-POT-001 conflicts, in the order found (reported by `potential_name`).
    pub conflicts: Vec<String>,
}

impl std::ops::Deref for PotentialAttrsV {
    type Target = PotentialAttrs;
    fn deref(&self) -> &PotentialAttrs {
        &self.attrs
    }
}

impl HeapBytes for PotentialAttrsV {
    fn heap_bytes(&self) -> usize {
        self.attrs.ptype.heap_bytes()
            + self.attrs.voltage.heap_bytes()
            + self.attrs.env.heap_bytes()
            + self.attrs.circuit_class.heap_bytes()
            + self.conflicts.heap_bytes()
    }
}

/// `potential_attrs(p)`: see the module documentation. Policy Memo; note-free.
#[derive(Debug)]
pub struct PotentialAttrsOf;

impl Query for PotentialAttrsOf {
    type Key = PotentialId;
    type Value = PotentialAttrsV;
    const FAMILY: &'static str = "potential_attrs";
    const POLICY: Policy = Policy::Memo;

    fn bytes(value: &Self::Value) -> usize {
        crate::heap::value_bytes(value)
    }

    fn run(q: &Q, key: &PotentialId) -> Arc<PotentialAttrsV> {
        let partition = q.get_part::<PotentialPartition>((), PartKey::Potential(*key));
        let Some(p) = partition.get(*key) else {
            return Arc::new(PotentialAttrsV::default());
        };
        Arc::new(attrs_of(q, p))
    }
}

fn attrs_of(q: &Q, p: &Potential) -> PotentialAttrsV {
    let mut conflicts: Vec<String> = Vec::new();
    let def_rows = ranked_defs(q, p);
    let ptype = def_rows.iter().find_map(|(d, r)| {
        r.ptype.clone().map(|v| Attr {
            value: v,
            source: AttrSource::Def(*d),
        })
    });
    if let Some(t) = &ptype {
        for (_, r) in &def_rows {
            if let Some(other) = &r.ptype
                && *other != t.value
            {
                conflicts.push(format!(
                    "{} sets type {other:?}; {:?} applies (first definition in natural order)",
                    def_text(q, r),
                    t.value
                ));
            }
        }
        let mut contradicting: Vec<(String, PotentialType)> = member_roles(q, p)
            .into_iter()
            .filter(|(_, role)| !role_compatible(&t.value, role))
            .map(|(cp, role)| (cp_text(q, cp), role))
            .collect();
        // The printed order, never the CP (ID) order (FT-587).
        contradicting.sort_by(|(a, ra), (b, rb)| {
            natural_cmp(a, b)
                .then_with(|| a.cmp(b))
                .then_with(|| format!("{ra:?}").cmp(&format!("{rb:?}")))
        });
        contradicting.dedup();
        for (text, role) in contradicting {
            conflicts.push(format!(
                "member CP {text} has role {role:?}; the defined type {:?} applies",
                t.value
            ));
        }
    }
    let ptype = ptype.or_else(|| role_type(q, p, &mut conflicts));
    let voltage = def_rows.iter().find_map(|(d, r)| {
        r.voltage.map(|v| Attr {
            value: v,
            source: AttrSource::Def(*d),
        })
    });
    if let Some(w) = &voltage {
        for (_, r) in &def_rows {
            if let Some(other) = r.voltage
                && other != w.value
            {
                conflicts.push(format!(
                    "{} sets voltage {other}; {} applies (first definition in natural order)",
                    def_text(q, r),
                    w.value
                ));
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
        for (_, r) in &def_rows {
            if !r.env.is_empty() && r.env != w.value {
                conflicts.push(format!(
                    "{} sets environment {:?}; {:?} applies (first definition in natural order)",
                    def_text(q, r),
                    r.env,
                    w.value
                ));
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
        for (_, r) in &def_rows {
            if let Some(other) = &r.circuit_class
                && *other != w.value
            {
                conflicts.push(format!(
                    "{} sets circuit class {other:?}; {:?} applies (first definition in natural order)",
                    def_text(q, r),
                    w.value
                ));
            }
        }
    }
    PotentialAttrsV {
        attrs: PotentialAttrs {
            ptype,
            voltage,
            env,
            circuit_class,
        },
        conflicts,
    }
}

/// The roles of the member CPs that are not interruption points and declare one, in CP order.
pub(crate) fn member_roles(q: &Q, p: &Potential) -> Vec<(CpRef, PotentialType)> {
    p.cps
        .iter()
        .filter(|cp| p.ips.binary_search(&cp.function).is_err())
        .filter_map(|cp| {
            let fcps = q.get::<FunctionCps>(cp.function);
            fcps.iter()
                .find(|c| c.idx == cp.idx)
                .and_then(|c| c.potential_role.clone())
                .map(|role| (*cp, role))
        })
        .collect()
}

/// Whether a member CP `role` is compatible with the type `defined` by a definition (doc 08 §3.7): equal
/// values, an FE role on a PE or PEN potential (a separate check, EW-POT-003), and a PE or N role on a PEN
/// potential.
fn role_compatible(defined: &PotentialType, role: &PotentialType) -> bool {
    use PotentialType::{Fe, N, Pe, Pen};
    defined == role || matches!((defined, role), (Pe | Pen, Fe) | (Pen, Pe | N))
}

/// The rank of a member CP role when the roles of a potential disagree (the lowest applies): the protective
/// roles first, then the neutral, the phases, DC, signal and undefined, a role of a newer format last. The
/// order is fixed so that the applied type and the conflict text never follow the CP order, which is the ID
/// order and so random in a new project (the blind replay `gpp-2` differed on it).
fn role_rank(role: &PotentialType) -> u8 {
    use PotentialType::{Fe, L, L1, L2, L3, LMinus, LPlus, M, N, Pe, Pen, Signal, Undefined, Unknown};
    match role {
        Pe => 0,
        Pen => 1,
        Fe => 2,
        N => 3,
        L1 => 4,
        L2 => 5,
        L3 => 6,
        L => 7,
        LPlus => 8,
        LMinus => 9,
        M => 10,
        Signal => 11,
        Undefined => 12,
        Unknown(_) => 13,
    }
}

/// The type from member CP roles: the first role in [`role_rank`] order (two newer-format roles by their
/// text); every other distinct role is a conflict. The source is the first member CP with that role.
fn role_type(q: &Q, p: &Potential, conflicts: &mut Vec<String>) -> Option<Attr<PotentialType>> {
    let roles = member_roles(q, p);
    let key = |r: &PotentialType| (role_rank(r), format!("{r:?}"));
    let (cp, applied) = roles
        .iter()
        .min_by(|(a_cp, a), (b_cp, b)| key(a).cmp(&key(b)).then_with(|| a_cp.cmp(b_cp)))
        .cloned()?;
    let others: BTreeSet<String> = roles
        .iter()
        .filter(|(_, r)| *r != applied)
        .map(|(_, r)| format!("{r:?}"))
        .collect();
    if !others.is_empty() {
        conflicts.push(format!(
            "member roles {} meet {applied:?}; the first in role order applies",
            others.into_iter().collect::<Vec<_>>().join(", "),
        ));
    }
    Some(Attr {
        value: applied,
        source: AttrSource::Role(cp),
    })
}

/// The potential named `name` (its canonical name), the first in ID order. Reads `potential_name` of every
/// potential until it finds it.
pub fn potential_named(q: &Q, name: &str) -> Option<PotentialId> {
    let partition = q.get::<PotentialPartition>(());
    partition
        .potentials()
        .map(|p| p.id)
        .find(|id| q.get::<PotentialNameOf>(*id).name.text == name)
}
