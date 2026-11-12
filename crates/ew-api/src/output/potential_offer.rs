// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A potential reasoned for a wire that has only a fallback name (FT-1700): the window offers it to accept or decline,
//! and never sets it without the person's accept.
//!
//! Only what is safe to reason: a connection point on the wire's potential whose own role says its type (a part's or a
//! function definition's `potential_role`: the − of a DC supply is M, an N or PE pin), or the A2 of a coil whose A1 is
//! on a potential of type L+ (A2 is then the 0 V, M) or of one phase (A2 is then the N). Every such point of the
//! potential must agree on one type, and the project must hold exactly one named potential of that type (named by a
//! definition, a source interruption point or the supply, not by the fallback); its name is the offer. Anything else
//! offers nothing.

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
        if !fallback(&q.get::<PotentialNameOf>(p.id).name.source) {
            return None;
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
            let ret = match q
                .get::<PotentialAttrsOf>(feed.id)
                .ptype
                .as_ref()
                .map(|a| &a.value)
            {
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
            .filter(|o| {
                q.get::<PotentialAttrsOf>(o.id)
                    .ptype
                    .as_ref()
                    .is_some_and(|a| a.value == ptype)
            })
            .map(|o| q.get::<PotentialNameOf>(o.id).name.clone())
            .filter(|n| !fallback(&n.source))
            .map(|n| n.text)
            .collect();
        let mut names = names.into_iter();
        let (Some(name), None) = (names.next(), names.next()) else {
            return None;
        };
        Some(PotentialOffer {
            name,
            ptype: serde_json::to_value(&ptype).ok()?.as_str()?.to_owned(),
            cp: cp_label(&q, cp),
            fed_from,
        })
    }
}
