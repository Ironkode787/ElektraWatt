// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A potential reasoned for a wire that has only a fallback name (FT-1700): the window offers it to accept or decline,
//! and never sets it without the person's accept.
//!
//! Only what is safe to reason: a connection point on the wire's potential whose own role says its type (a part's or a
//! function definition's `potential_role`: the − of a DC supply is M, an N or PE pin), or the A2 of a coil whose A1 is
//! on a potential of type L+ (A2 is then the 0 V, M) or of one phase (A2 is then the N). Every such point of the
//! potential must agree on one type. When the project holds exactly one named potential of that type (named by a
//! definition, a source interruption point or the supply, not by the fallback), its name is the offer; when it holds
//! none, the offer is a new potential of that type named by the type's printed word ([`new_name`]: `0V` for M, `N`,
//! `PE`, `L+`, …), with `-2`, `-3`… where another potential bears the word. Several named potentials of the type, or a
//! type without a printed word (signal), offer nothing.
//!
//! A potential without a type whose name is a type's own word (`N`, `PE`, `L1`, `L2`, `L3`, `L`, `L+`, `L-`, `M`,
//! `0V`; [`said_type`]) counts as of that type, for a coil's feed and for the names of the type. Such a potential is
//! itself offered its type (`Type L1?`), which gives its wires their colour and cross-section.

use std::collections::BTreeSet;

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::{ContactKind, PotentialType};
use ew_derive::{
    CpRef, EndpointKey, FunctionCps, NameSource, PotentialAttrsOf, PotentialNameOf, PotentialPartition, Q,
    cp_label,
};

use crate::Session;

/// The potential [`Session::potential_offer`] reasons for a wire.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PotentialOffer {
    /// The name offered: the project's one named potential of the reasoned type (`0VDC`).
    pub name: String,
    /// The type's stored word (`m`, `n`, `pe`).
    pub ptype: String,
    /// The connection point that gives the reason, as the page prints it (`-1K1:A2`).
    pub cp: String,
    /// For a coil's A2, the name of the potential on its A1 (`24VDC`); `None` where the point's own role gives the
    /// type.
    pub fed_from: Option<String>,
    /// Whether the name is a new potential's (the project has no named potential of the type), not an existing one's.
    pub new: bool,
    /// Whether the offer is the type its own name says, for a potential named that way without a type (`Type L1?`);
    /// `cp` is then empty.
    pub type_only: bool,
}

/// The name a new potential of `ptype` takes: the type's printed word, `0V` for M; `None` for a type without one.
fn new_name(ptype: &PotentialType) -> Option<&'static str> {
    use PotentialType::{Fe, L, L1, L2, L3, LMinus, LPlus, M, N, Pe, Pen};
    Some(match ptype {
        L1 => "L1",
        L2 => "L2",
        L3 => "L3",
        L => "L",
        N => "N",
        Pe => "PE",
        Pen => "PEN",
        Fe => "FE",
        LPlus => "L+",
        LMinus => "L-",
        M => "0V",
        _ => return None,
    })
}

/// The type whose own word `name` is (`N`, `PE`, `L1`, `L2`, `L3`, `L`, `L+`, `L-`, `M`, and `0V`, the word a new
/// potential of M takes); `None` for any other name.
fn word_type(name: &str) -> Option<PotentialType> {
    use PotentialType::{L, L1, L2, L3, LMinus, LPlus, M, N, Pe};
    let name = name.trim();
    if name.eq_ignore_ascii_case("M") {
        return Some(M);
    }
    [L1, L2, L3, L, N, Pe, LPlus, LMinus, M]
        .into_iter()
        .find(|t| new_name(t).is_some_and(|w| w.eq_ignore_ascii_case(name)))
}

/// The type of potential `id`: its own (a definition's or a member's role), else the type its name says when a
/// definition, a source point or the supply gives the name and the name is a type's own word ([`word_type`]).
pub(crate) fn said_type(q: &Q, id: ew_derive::PotentialId) -> Option<PotentialType> {
    if let Some(t) = q.get::<PotentialAttrsOf>(id).ptype.as_ref() {
        return Some(t.value.clone());
    }
    let name = q.get::<PotentialNameOf>(id).name.clone();
    if fallback(&name.source) {
        return None;
    }
    word_type(&name.text)
}

/// Whether `name` is only the fallback name of a potential.
fn fallback(source: &NameSource) -> bool {
    matches!(source, NameSource::Fallback(_))
}

/// Whether `function`'s definition is a coil.
fn is_coil(q: &Q, function: Id<ew_derive::ew_schema::Function>) -> bool {
    q.functions()
        .get(function)
        .and_then(|f| q.fndefs().get(f.def))
        .is_some_and(|d| d.item.contact_kind == ContactKind::Coil)
}

impl Session {
    /// The potential reasoned for the wire `connection` (see the module documentation); `None` when its potential is
    /// named, or nothing safe can be reasoned.
    pub fn potential_offer(&self, connection: Uuid) -> Option<PotentialOffer> {
        let q = self.query();
        let wire = q.connections().get(Id::from_uuid(connection))?;
        let partition = q.get::<PotentialPartition>(());
        let p = partition.potential_at(EndpointKey::of(&wire.a)?)?;
        let own = q.get::<PotentialNameOf>(p.id).name.clone();
        if !fallback(&own.source) {
            // A name that says a type the potential does not have yet: its type is offered.
            if q.get::<PotentialAttrsOf>(p.id).ptype.is_some() {
                return None;
            }
            let ptype = word_type(&own.text)?;
            return Some(PotentialOffer {
                name: own.text,
                ptype: serde_json::to_value(&ptype).ok()?.as_str()?.to_owned(),
                cp: String::new(),
                fed_from: None,
                new: false,
                type_only: true,
            });
        }
        // Each point of the potential that gives a reason: the point, the type it gives, the coil's feed.
        let mut reasons: Vec<(CpRef, PotentialType, Option<String>)> = Vec::new();
        for cp in p
            .cps
            .iter()
            .filter(|cp| p.ips.binary_search(&cp.function).is_err())
        {
            let cps = q.get::<FunctionCps>(cp.function);
            let Some(own) = cps.iter().find(|c| c.idx == cp.idx) else {
                continue;
            };
            if let Some(role) = &own.potential_role {
                reasons.push((*cp, role.clone(), None));
                continue;
            }
            let Some(a1) = cps
                .iter()
                .find(|c| c.designation == "A1")
                .filter(|_| own.designation == "A2" && is_coil(&q, cp.function))
            else {
                continue;
            };
            let Some(feed) = partition.potential_at(CpRef::new(cp.function, a1.idx).key()) else {
                continue;
            };
            let ret = match said_type(&q, feed.id) {
                Some(PotentialType::LPlus) => PotentialType::M,
                Some(PotentialType::L1 | PotentialType::L2 | PotentialType::L3 | PotentialType::L) => {
                    PotentialType::N
                }
                _ => continue,
            };
            reasons.push((
                *cp,
                ret,
                Some(q.get::<PotentialNameOf>(feed.id).name.text.clone()),
            ));
        }
        let (cp, ptype, fed_from) = reasons.first()?.clone();
        if reasons.iter().any(|(_, t, _)| *t != ptype) {
            return None;
        }
        let names: BTreeSet<String> = partition
            .potentials()
            .filter(|o| o.id != p.id)
            .filter(|o| said_type(&q, o.id).as_ref() == Some(&ptype))
            .map(|o| q.get::<PotentialNameOf>(o.id).name.clone())
            .filter(|n| !fallback(&n.source))
            .map(|n| n.text)
            .collect();
        let mut names = names.into_iter();
        let (name, new) = match (names.next(), names.next()) {
            (Some(name), None) => (name, false),
            (None, _) => {
                let word = new_name(&ptype)?;
                let name = std::iter::once(word.to_owned())
                    .chain((2..).map(|n| format!("{word}-{n}")))
                    .find(|n| ew_derive::potential_named(&q, n).is_none())?;
                (name, true)
            }
            _ => return None,
        };
        Some(PotentialOffer {
            name,
            new,
            ptype: serde_json::to_value(&ptype).ok()?.as_str()?.to_owned(),
            cp: cp_label(&q, cp),
            fed_from,
            type_only: false,
        })
    }
}
