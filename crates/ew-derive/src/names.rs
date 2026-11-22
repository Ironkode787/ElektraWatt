// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The names and own attributes of a potential, split out of the partition (ADR-0002 amendment 4: "a folio
//! renumber or a move re-derives names and not the topology"; ARCH §6.4 `potential_names`).
//!
//! - **`potential_name(p)`** (doc 08 §3.5): the name of the first definition in rank order ([`ranked_defs`]: natural
//!   order of the name, the defining point's printed text, the ID) (another name is noted as EW-POT-017); else the first non-external source interruption point in page order (page position, anchor
//!   row, anchor column, ID); else the first external source interruption point in that order (review decision
//!   D6); else (doc 08 §3.5 step 2b) the supply's name `<root>/<tag>:<designation>` when the
//!   potential is entered through the pass-through pair of a function f (its flow `upstream`): `<root>` is the
//!   name of the nearest upstream potential named by a definition or a source point (the walk passes potentials
//!   named by this step), `<tag>` f's displayed tag without its leading aspect sign and `<designation>` the
//!   pair's downstream CP (`L/S51:2`); else (step 2c) the first interruption point of any other direction in page
//!   order, non-external ones first (a target no source feeds, a supply coming in, still names the potential it
//!   carries); else `<tag>:<designation>` of the first member CP in (page position, row,
//!   column) order, ties by that name in natural order and then by CP ID. The aliases are every other distinct IP name on the potential,
//!   external points included, in natural order; points on a bond's `via` path (doc 03
//!   §9.6) are named on the bond, not on the potential. A signal takes the first IP name inside it, in page
//!   order. The family also pushes the EW-POT-001 notes of the potential's attributes, which name it.
//! - **`potential_attrs(p)`** (doc 03 §9.4): type, voltage, environment and class from the first definition in
//!   rank order ([`ranked_defs`]) that sets each; the conflict texts name definitions and CPs by their printed text,
//!   in printed order, never by ID; the type else from the member CPs' `potential_role`; roles that disagree are
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
        } else if let Some((f, i)) = [false, true].into_iter().find_map(|external| {
            // Step 2c: a point no source feeds (a target placed first, a supply coming in from elsewhere) still names
            // the potential it carries; non-external points first, as in step 2.
            infos
                .iter()
                .filter(|(f, i)| {
                    i.spec.as_ref().is_some_and(|s| s.external.is_some() == external) && !on_path.contains(f)
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
        for conflict in &q.get::<PotentialAttrsOf>(*key).conflicts {
            notes.push(DeriveNote::Connectivity {
                code: "EW-POT-001",
                subject: format!("potential {}", name.text),
                detail: conflict.detail(q),
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

/// Step 2b of doc 08 §3.5: `<root>/<tag without aspect sign>:<designation>` of the pass-through
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

/// The definitions of a potential in their rank (doc 08 §3.6): natural order of the
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
/// never its ID.
pub fn def_text(q: &Q, def: &PotentialDef) -> String {
    match def_point_text(q, def) {
        Some(at) => format!("definition {} at {at}", def.name.as_str()),
        None => format!("definition {}", def.name.as_str()),
    }
}

/// `<tag>:<designation>` of a CP: its function's device tag as a report prints it (`?` without one) and the CP's
/// designation (its index without one); the designation alone for a device that has no tag (an earth connection).
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
    if tag.is_empty() {
        return designation;
    }
    format!("{tag}:{designation}")
}

/// The fallback name `<tag>:<designation>` of the first member CP (not an IP) in (page position, row, column) order
/// of its function's first representation; ties go to the name in natural order (then bytes) and only then to the CP
/// ID, so two CPs of one anchor cell (a terminal's two sides) name the potential alike in every fresh copy of a
/// project (an ID of a new project is random).
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
    // A protective earth that only an earth symbol (a device without a tag) names is PE, not that symbol's pin (`1`,
    // an imported earth point's) (FT-1933).
    let tagless = q
        .functions()
        .get(cp.function)
        .and_then(|f| f.device)
        .and_then(|d| q.get::<DeviceTag>(d).as_ref().as_ref().map(|t| t.short()))
        .is_some_and(|t| t.is_empty());
    let earth = tagless
        && q.get::<PotentialAttrsOf>(p.id)
            .attrs
            .ptype
            .as_ref()
            .is_some_and(|t| t.value == PotentialType::Pe);
    PotentialName {
        text: if earth { "PE".to_owned() } else { text },
        source: NameSource::Fallback(cp),
    }
}

/// The attribute two definitions of one potential disagree on.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DefAttr {
    /// The type.
    Type,
    /// The voltage.
    Voltage,
    /// The environment (AC or DC, frequency, earthing).
    Environment,
    /// The circuit class.
    Class,
}

/// One EW-POT-001 conflict of a potential's own attributes (doc 08 §3.6), by the records it is about, never by
/// text: the check words it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AttrConflict {
    /// Two definitions set `attr` differently: `def`'s value loses to `applied`'s, the first in rank order that
    /// sets it.
    Defs {
        /// The attribute.
        attr: DefAttr,
        /// The definition whose value does not apply.
        def: Id<PotentialDef>,
        /// The definition whose value applies.
        applied: Id<PotentialDef>,
    },
    /// A member CP whose role contradicts the type the definition `def` gives.
    Role {
        /// The CP.
        cp: CpRef,
        /// Its role.
        role: PotentialType,
        /// The definition that gives the type.
        def: Id<PotentialDef>,
    },
    /// Member CPs of two roles meet on a potential that no definition types: `cp` (the first in printed order of the
    /// role `role`) against `first` (the first of the role that applies, `applied`).
    Roles {
        /// The first member CP of the role that applies.
        first: CpRef,
        /// The role that applies (the type).
        applied: PotentialType,
        /// The first member CP of the other role.
        cp: CpRef,
        /// The other role.
        role: PotentialType,
    },
}

impl AttrConflict {
    /// The conflict in English, as the EW-POT-001 notes of `potential_name` carry it (the check words it for a
    /// person).
    pub fn detail(&self, q: &Q) -> String {
        let defs = q.potential_defs();
        match self {
            AttrConflict::Defs { attr, def, applied } => {
                let (Some(d), Some(a)) = (defs.get(*def), defs.get(*applied)) else {
                    return "two definitions disagree".to_owned();
                };
                let (what, value, applies) = match attr {
                    DefAttr::Type => ("type", format!("{:?}", d.ptype), format!("{:?}", a.ptype)),
                    DefAttr::Voltage => {
                        let v = |x: &PotentialDef| x.voltage.map(|v| format!("{v} V")).unwrap_or_default();
                        ("voltage", v(d), v(a))
                    }
                    DefAttr::Environment => ("environment", format!("{:?}", d.env), format!("{:?}", a.env)),
                    DefAttr::Class => (
                        "circuit class",
                        format!("{:?}", d.circuit_class),
                        format!("{:?}", a.circuit_class),
                    ),
                };
                format!(
                    "{} sets {what} {value}; {applies} applies (first definition in natural order)",
                    def_text(q, d)
                )
            }
            AttrConflict::Role { cp, role, def } => format!(
                "member CP {} has role {role:?}; the defined type {:?} applies",
                cp_text(q, *cp),
                defs.get(*def).and_then(|d| d.ptype.clone())
            ),
            AttrConflict::Roles { applied, role, .. } => {
                format!("member roles {role:?} meet {applied:?}; the first in role order applies")
            }
        }
    }
}

/// The value of [`PotentialAttrsOf`]: the own attributes and their conflicts.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PotentialAttrsV {
    /// The attributes.
    pub attrs: PotentialAttrs,
    /// The EW-POT-001 conflicts, in the order found: the definitions' type, the member CPs' roles in printed order,
    /// then the definitions' voltage, environment and class, each in rank order.
    pub conflicts: Vec<AttrConflict>,
}

impl std::ops::Deref for PotentialAttrsV {
    type Target = PotentialAttrs;
    fn deref(&self) -> &PotentialAttrs {
        &self.attrs
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

    fn run(q: &Q, key: &PotentialId) -> Arc<PotentialAttrsV> {
        let partition = q.get_part::<PotentialPartition>((), PartKey::Potential(*key));
        let Some(p) = partition.get(*key) else {
            return Arc::new(PotentialAttrsV::default());
        };
        Arc::new(attrs_of(q, p))
    }
}

/// The first definition in rank order that sets an attribute (`get`), as an [`Attr`]; every later definition whose
/// value differs is a conflict of `attr`.
fn def_attr<T: Clone + PartialEq>(
    def_rows: &[RankedDef],
    attr: DefAttr,
    conflicts: &mut Vec<AttrConflict>,
    get: impl Fn(&PotentialDef) -> Option<T>,
) -> Option<Attr<T>> {
    let (applied, value) = def_rows.iter().find_map(|(d, r)| get(r).map(|v| (*d, v)))?;
    for (d, r) in def_rows {
        if get(r).is_some_and(|other| other != value) {
            conflicts.push(AttrConflict::Defs {
                attr,
                def: *d,
                applied,
            });
        }
    }
    Some(Attr {
        value,
        source: AttrSource::Def(applied),
    })
}

fn attrs_of(q: &Q, p: &Potential) -> PotentialAttrsV {
    let mut conflicts: Vec<AttrConflict> = Vec::new();
    let def_rows = ranked_defs(q, p);
    let ptype = def_attr(&def_rows, DefAttr::Type, &mut conflicts, |r| r.ptype.clone());
    if let Some(t) = &ptype
        && let AttrSource::Def(def) = t.source
    {
        let mut contradicting: Vec<(String, CpRef, PotentialType)> = member_roles(q, p)
            .into_iter()
            .filter(|(_, role)| !role_compatible(&t.value, role))
            .map(|(cp, role)| (cp_text(q, cp), cp, role))
            .collect();
        // The printed order, never the CP (ID) order; two CPs that print alike with one role are one conflict.
        contradicting.sort_by(|(a, ca, ra), (b, cb, rb)| {
            natural_cmp(a, b)
                .then_with(|| a.cmp(b))
                .then_with(|| format!("{ra:?}").cmp(&format!("{rb:?}")))
                .then_with(|| ca.cmp(cb))
        });
        contradicting.dedup_by(|(a, _, ra), (b, _, rb)| a == b && ra == rb);
        for (_, cp, role) in contradicting {
            conflicts.push(AttrConflict::Role { cp, role, def });
        }
    }
    let ptype = ptype.or_else(|| role_type(q, p, &mut conflicts));
    let voltage = def_attr(&def_rows, DefAttr::Voltage, &mut conflicts, |r| r.voltage);
    let env = def_attr(&def_rows, DefAttr::Environment, &mut conflicts, |r| {
        (!r.env.is_empty()).then(|| r.env.clone())
    });
    let circuit_class = def_attr(&def_rows, DefAttr::Class, &mut conflicts, |r| {
        r.circuit_class.clone()
    });
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

/// Whether a member CP `role` may meet the type `defined` (a definition's, or the member role of a lower
/// [`role_rank`]) on one potential (doc 08 §3.6, the pairs of EW-POT-001). Either order of the two:
///
/// - **Nothing to report:** equal values; an FE role on a PE or PEN potential (a separate check, EW-POT-003); PE or N
///   on PEN; L on one phase, L1, L2 or L3 (a single-phase input fed from one phase, FT-1701); M with L− (two names
///   of one 0 V); a signal on any conductor but an earth (a signal pin wired straight to a supply is a fixed level);
///   an undefined role, or one of a newer format, with anything (it says nothing).
/// - **A fault:** every other pair: a live conductor (L1, L2, L3, L, L+) on PE, PEN, FE, N, 0 V or another live
///   conductor; N on PE or FE (N and PE meet only in a PEN conductor); AC with DC; a signal on an earth.
/// - **A bond:** 0 V (M or L−) on PE. A wire drawn straight from a 0 V point to a protective-earth point is the
///   bond itself and never joins the two (`EarthWire`, the drawn bond); 0 V and PE that still meet here are joined
///   some other way, and the bond is declared on the wire that joins them (Draw ▸ Connect ▸ Bond).
fn role_compatible(defined: &PotentialType, role: &PotentialType) -> bool {
    use PotentialType::{Fe, L, L1, L2, L3, LMinus, M, N, Pe, Pen, Signal, Undefined, Unknown};
    let pair = |a: &PotentialType, b: &PotentialType| {
        matches!(
            (a, b),
            (Pe | Pen, Fe) | (Pen, Pe | N) | (L1 | L2 | L3, L) | (M, LMinus) | (Undefined | Unknown(_), _)
        ) || (*a == Signal && !matches!(b, Pe | Pen | Fe))
    };
    defined == role || pair(defined, role) || pair(role, defined)
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
/// text); every other distinct role that is not [`role_compatible`] with it is a conflict, named by its first member
/// CP in printed order against the first of the applied role. The source is the first member CP with that role.
fn role_type(q: &Q, p: &Potential, conflicts: &mut Vec<AttrConflict>) -> Option<Attr<PotentialType>> {
    let roles = member_roles(q, p);
    let key = |r: &PotentialType| (role_rank(r), format!("{r:?}"));
    let (cp, applied) = roles
        .iter()
        .min_by(|(a_cp, a), (b_cp, b)| key(a).cmp(&key(b)).then_with(|| a_cp.cmp(b_cp)))
        .cloned()?;
    // The first member CP of a role in printed order, never the CP (ID) order.
    let first_of = |role: &PotentialType| {
        roles
            .iter()
            .filter(|(_, r)| r == role)
            .map(|(c, _)| (cp_text(q, *c), *c))
            .min_by(|(a, ca), (b, cb)| natural_cmp(a, b).then_with(|| a.cmp(b)).then_with(|| ca.cmp(cb)))
            .map(|(_, c)| c)
    };
    let mut others: Vec<&PotentialType> = roles
        .iter()
        .map(|(_, r)| r)
        .filter(|r| !role_compatible(&applied, r))
        .collect();
    others.sort_by_key(|r| key(r));
    others.dedup();
    let first = first_of(&applied).unwrap_or(cp);
    for role in others {
        if let Some(c) = first_of(role) {
            conflicts.push(AttrConflict::Roles {
                first,
                applied: applied.clone(),
                cp: c,
                role: role.clone(),
            });
        }
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
