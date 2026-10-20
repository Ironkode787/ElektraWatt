// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The names and own attributes of a potential, split out of the partition (ADR-0002 amendment 4: "a folio
//! renumber or a move re-derives names and not the topology"; ARCH §6.4 `potential_names`; WP-S1.07 part 2).
//!
//! - **`potential_name(p)`** (doc 08 §3.5): the first definition's name in ID order (another name is noted as
//!   EW-POT-017); else the first non-external source interruption point in page order (page position, anchor
//!   row, anchor column, ID); else the first external source interruption point in that order (review decision
//!   D6); else `<tag>:<designation>` of the first member CP in that order. The aliases are every other distinct
//!   IP name on the potential, external points included, in natural order; points on a bond's `via` path (doc 03
//!   §9.6) are named on the bond, not on the potential. A signal takes the first IP name inside it, in page
//!   order. The family also pushes the EW-POT-001 notes of the potential's attributes, which name it.
//! - **`potential_attrs(p)`** (doc 03 §9.4): type, voltage, environment and class from the first definition in
//!   ID order that sets each; the type else from the member CPs' `potential_role` (the first in CP order).
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
use ew_schema::{Function, IpDirection, PotentialDef, PotentialType};

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

        let def_rows: Vec<(Id<PotentialDef>, Arc<PotentialDef>)> = p
            .defs
            .iter()
            .filter_map(|d| q.potential_defs().get(*d).map(|r| (*d, Arc::clone(r))))
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

/// The fallback name `<tag>:<designation>` of the first member CP (not an IP) in (page position, row,
/// column, CP ID) order of its function's first representation.
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
        (pos, row, col, cp.id())
    };
    let first = p
        .cps
        .iter()
        .filter(|cp| !is_ip(cp.function))
        .min_by_key(|cp| key(cp))
        .or_else(|| p.cps.first())
        .copied();
    let Some(cp) = first else {
        return unknown().name;
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
    let def_rows: Vec<(Id<PotentialDef>, Arc<PotentialDef>)> = p
        .defs
        .iter()
        .filter_map(|d| q.potential_defs().get(*d).map(|r| (*d, Arc::clone(r))))
        .collect();
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
                conflicts.push(format!(
                    "definition {d} sets type {other:?}; {:?} applies",
                    t.value
                ));
            }
        }
        for (cp, role) in member_roles(q, p) {
            if !role_compatible(&t.value, &role) {
                conflicts.push(format!(
                    "member CP {}:{} has role {role:?}; the defined type {:?} applies",
                    cp.function, cp.idx, t.value
                ));
            }
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
        for (d, r) in &def_rows {
            if let Some(other) = r.voltage
                && other != w.value
            {
                conflicts.push(format!(
                    "definition {d} sets voltage {other}; {} applies (first definition in ID order)",
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
        for (d, r) in &def_rows {
            if !r.env.is_empty() && r.env != w.value {
                conflicts.push(format!(
                    "definition {d} sets environment {:?}; {:?} applies (first definition in ID order)",
                    r.env, w.value
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
        for (d, r) in &def_rows {
            if let Some(other) = &r.circuit_class
                && *other != w.value
            {
                conflicts.push(format!(
                    "definition {d} sets circuit class {other:?}; {:?} applies (first definition in ID order)",
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
fn member_roles(q: &Q, p: &Potential) -> Vec<(CpRef, PotentialType)> {
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

/// The type from member CP roles: the first role in CP order; another distinct role is a conflict.
fn role_type(q: &Q, p: &Potential, conflicts: &mut Vec<String>) -> Option<Attr<PotentialType>> {
    let mut found: Option<Attr<PotentialType>> = None;
    let mut others: BTreeSet<String> = BTreeSet::new();
    for (cp, role) in member_roles(q, p) {
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
        conflicts.push(format!(
            "member roles {} meet {:?}; the first in CP order applies",
            others.into_iter().collect::<Vec<_>>().join(", "),
            f.value
        ));
    }
    found
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
