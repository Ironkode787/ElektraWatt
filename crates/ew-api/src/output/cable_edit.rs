// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The edit-cable table's gestures below the shell (WP-S2.07 part 2, §7): [`Session::swap_cores`] compiles a swap of
//! two cores of one cable into one `fields.set_many` that moves the core assignment of their connections (each
//! connection of the one core becomes a connection of the other), so the assignment moves, not the colour.

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Conductor, ConnectionKind};
use serde_json::json;

use super::navigators::Uuid;
use crate::{AnyCommand, Session};

impl Session {
    /// The one command that swaps the cores `a` and `b` (`cable.swap_cores`, §7): one `fields.set_many` setting the
    /// `kind` of every connection of `a` to `Core(b)` and of every connection of `b` to `Core(a)` (connections in id
    /// order; a core without connections contributes no entry).
    ///
    /// # Errors
    ///
    /// A reason key: `reason-swap-two-cores` when `a` and `b` are not two different conductors of one cable,
    /// `reason-swap-cores-unwired` when neither has a connection.
    pub fn swap_cores(&self, a: Uuid, b: Uuid) -> Result<AnyCommand, &'static str> {
        let q = self.query();
        let (ia, ib): (Id<Conductor>, Id<Conductor>) = (Id::from_uuid(a), Id::from_uuid(b));
        let (Some(ca), Some(cb)) = (q.conductors().get(ia), q.conductors().get(ib)) else {
            return Err("reason-swap-two-cores");
        };
        if a == b || ca.cable != cb.cable {
            return Err("reason-swap-two-cores");
        }
        let of = |core: Id<Conductor>| {
            let mut ids: Vec<_> = q
                .connections()
                .iter()
                .filter(|(_, c)| matches!(c.kind, ConnectionKind::Core(x) if x == core))
                .map(|(_, c)| c.id)
                .collect();
            ids.sort();
            ids.into_iter()
                .map(|c| c.as_uuid().to_string())
                .collect::<Vec<_>>()
        };
        let edits: Vec<_> = [(of(ia), ib), (of(ib), ia)]
            .into_iter()
            .filter(|(targets, _)| !targets.is_empty())
            .map(|(targets, to)| {
                let value = serde_json::to_value(ConnectionKind::Core(to)).unwrap_or_default();
                json!({"kind": "connection", "targets": targets, "path": "kind", "value": value})
            })
            .collect();
        if edits.is_empty() {
            return Err("reason-swap-cores-unwired");
        }
        Ok(AnyCommand {
            name: "fields.set_many".into(),
            params: json!({ "edits": edits }),
        })
    }
}
