// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Highlighting (§7.3, D14): what a highlight shows and the connections it draws.
//!
//! Not here: the media's zones it reads (`fluid.rs`).

use ew_derive::ew_ids::{Id, Uuid};

use super::fluid::medium_lines;
use crate::Session;

/// What a highlight shows (§4.2): a potential, named by a potential overview row's record, or the net of a connection.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HighlightTarget {
    /// The potential whose ID (a definition's or a CP's) is this.
    Potential(Uuid),
    /// The net that contains this connection.
    Net(Uuid),
    /// The zones of the medium whose ID this is: the fluid lines of the nets that carry it (doc 11 §7).
    Medium(Uuid),
}

impl Session {
    /// The connection IDs a highlight draws (§7.3), sorted: for a potential the connections with an end
    /// on it (by the potential partition; a bond's `via` counts for both of its potentials), for a connection the
    /// connections of its page-local net (the connection alone when it joins no net, a bridge). Empty for an unknown
    /// ID. Reads memoised queries only.
    pub fn highlight_members(&self, target: &HighlightTarget) -> Vec<Uuid> {
        use ew_derive::ew_schema::Connection;
        use ew_derive::{PageLocalNets, PotentialId, PotentialPartition};
        use ew_model::EndpointKey;
        let q = self.query();
        let mut out: Vec<Uuid> = match target {
            HighlightTarget::Potential(u) => {
                let partition = q.get::<PotentialPartition>(());
                let Some(p) = partition
                    .potentials()
                    .find(|p| match p.id {
                        PotentialId::Def(d) => d.as_uuid() == *u,
                        PotentialId::Cp(c) => c.as_uuid() == *u,
                    })
                    .map(|p| p.id)
                else {
                    return Vec::new();
                };
                q.connections()
                    .iter()
                    .filter(|(_, c)| {
                        [&c.a, &c.b].into_iter().any(|e| {
                            EndpointKey::of(e)
                                .and_then(|k| partition.potential_at(k))
                                .is_some_and(|x| x.id == p)
                        })
                    })
                    .map(|(_, c)| c.id.as_uuid())
                    .collect()
            }
            HighlightTarget::Net(u) => {
                let id: Id<Connection> = Id::from_uuid(*u);
                let Some(c) = q.connections().get(id) else {
                    return Vec::new();
                };
                let net = c.page.and_then(|page| {
                    q.get::<PageLocalNets>(page)
                        .nets
                        .iter()
                        .find(|n| n.connections.contains(&id))
                        .map(|n| n.connections.iter().map(|x| x.as_uuid()).collect::<Vec<_>>())
                });
                net.unwrap_or_else(|| vec![*u])
            }
            HighlightTarget::Medium(u) => medium_lines(&q, Id::from_uuid(*u)),
        };
        out.sort();
        out.dedup();
        out
    }
}
