// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The edit-cable table's gestures below the shell (WP-S2.07 part 2, §7): [`Session::swap_cores`] compiles a swap of
//! two cores of one cable into one `fields.set_many` that moves the core assignment of their connections (each
//! connection of the one core becomes a connection of the other), so the assignment moves, not the colour.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{Conductor, ConnectionKind, Part};
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
            return Err(ew_commands::text_key!("reason-swap-two-cores"));
        };
        if a == b || ca.cable != cb.cable {
            return Err(ew_commands::text_key!("reason-swap-two-cores"));
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
            return Err(ew_commands::text_key!("reason-swap-cores-unwired"));
        }
        Ok(AnyCommand {
            name: "fields.set_many".into(),
            params: json!({ "edits": edits }),
        })
    }
}

/// Why [`Session::connect_cores`] compiles no command: a text key and the texts it names.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CoresRefused {
    /// The text key: `reason-cores-no-cable`, `reason-cores-no-wires` or `error-cores-too-few` (the cable's tag, its
    /// free cores and the wires).
    pub key: &'static str,
    /// The texts the key's pattern takes, in order.
    pub args: Vec<String>,
}

impl Session {
    /// The one command of New cable… (FT-1371) for the cable part `part` (its copy ID, a project copy or an installed
    /// library's part, as [`Session::layout_part_offers`] offers it for `cable`): `cable.create` with the part
    /// (quantity 1), one conductor per core of the part's cable cores, in the part's order (FR-CBL-004), and the part
    /// and its closure as `copies` when the project does not hold it, so that the embed and the cable are one
    /// transaction and one undo step (decision log ID-1376). `None` when no cable part has that ID.
    pub fn create_cable(&self, part: Uuid) -> Option<AnyCommand> {
        let offer = self
            .layout_part_offers("cable")
            .into_iter()
            .find(|o| o.part == part)?;
        let id: Id<Part> = Id::from_uuid(part);
        let held = self
            .query()
            .parts()
            .get(id)
            .map(|c| c.item.sections.cable_cores.clone());
        let cores = match held {
            Some(cores) => cores,
            None => {
                self.library_parts()
                    .into_iter()
                    .find(|c| c.id == id)?
                    .item
                    .sections
                    .cable_cores
            }
        };
        let conductors: Vec<_> = cores
            .iter()
            .map(|c| {
                let mut spec = json!({ "core": c.core.as_str(), "shield": c.shield });
                if let Some(colour) = &c.colour {
                    spec["colour"] = serde_json::to_value(colour).unwrap_or_default();
                }
                if let Some(csa) = &c.csa {
                    spec["csa"] = serde_json::to_value(csa).unwrap_or_default();
                }
                if let Some(pair) = c.pair {
                    spec["pair"] = json!(pair);
                }
                spec
            })
            .collect();
        let mut params = json!({
            "parts": [{ "part": { "copy": part.to_string() }, "qty": "1" }],
            "conductors": conductors,
        });
        if !offer.copies.is_empty() {
            params["copies"] = serde_json::Value::Array(offer.copies);
        }
        Some(AnyCommand {
            name: "cable.create".into(),
            params,
        })
    }

    /// The project's cables for Connect cores… (FT-1371): `(cable device, its tag, its words)`, by tag. The words are
    /// the tag, unless another cable prints the same tag (two frozen tags alike; decision log ID-1376, FT-1406): then
    /// the tag, `·` and the cable's part number (none for a cable without a part), and where those words are shared too, its position among them in the order the cables were created, `(1)`, `(2)`.
    pub fn cables(&self) -> Vec<(Uuid, String, String)> {
        let q = self.query();
        let ids: Vec<Uuid> = q
            .devices()
            .iter()
            .filter(|(_, d)| d.aspects.cable.is_some())
            .map(|(id, _)| id.as_uuid())
            .collect();
        let mut out: Vec<(Uuid, String, String)> = ids
            .into_iter()
            .filter_map(|id| {
                let tag = self.device_tag(id)?;
                Some((id, tag.clone(), tag))
            })
            .collect();
        out.sort_by(|a, b| ew_derive::ew_mltext::natural_cmp(&a.1, &b.1).then(a.0.cmp(&b.0)));
        let part = |id: Uuid| {
            q.devices()
                .get(Id::from_uuid(id))
                .and_then(|d| d.parts.first())
                .and_then(|p| q.parts().get(p.part.copy))
                .map(|c| c.item.part_number.as_str().to_owned())
        };
        for pass in 0..2 {
            let mut alike: BTreeMap<String, Vec<usize>> = BTreeMap::new();
            for (i, (_, _, words)) in out.iter().enumerate() {
                alike.entry(words.clone()).or_default().push(i);
            }
            // The UUIDv7 IDs sort in the order the cables were created, and `out` is by tag, then by ID.
            for at in alike.values().filter(|at| at.len() > 1) {
                for (n, &i) in at.iter().enumerate() {
                    let (id, _, words) = &mut out[i];
                    match (pass, part(*id)) {
                        (0, Some(number)) => *words = format!("{words} \u{b7} {number}"),
                        (0, None) => {}
                        _ => *words = format!("{words} ({})", n + 1),
                    }
                }
            }
        }
        out
    }

    /// The one command of Connect cores… (FT-1371; doc 08 §7.2, by order): one `fields.set_many` that makes the
    /// connections `wires`, in the order given, cores of `cable`, taking its free cores in the order of its part's
    /// cable cores (cores the part does not list after them, by core). A free core is one that no connection outside
    /// `wires` is; a wire among `wires` that is a core already is assigned again.
    ///
    /// # Errors
    ///
    /// [`CoresRefused`]: `cable` is no cable; `wires` is empty or holds a connection that is neither a wire nor a core;
    /// the cable has fewer free cores than `wires` (nothing is assigned, doc 08 §7.2).
    pub fn connect_cores(&self, cable: Uuid, wires: &[Uuid]) -> Result<AnyCommand, CoresRefused> {
        let q = self.query();
        let refused = |key: &'static str, args: Vec<String>| CoresRefused { key, args };
        let cable_id: Id<ew_derive::ew_schema::Device> = Id::from_uuid(cable);
        let Some(device) = q.devices().get(cable_id).filter(|d| d.aspects.cable.is_some()) else {
            return Err(refused(
                ew_commands::text_key!("reason-cores-no-cable"),
                Vec::new(),
            ));
        };
        let connections = q.connections();
        let usable = !wires.is_empty()
            && wires.iter().all(|w| {
                connections
                    .get(Id::from_uuid(*w))
                    .is_some_and(|c| matches!(c.kind, ConnectionKind::Wire | ConnectionKind::Core(_)))
            });
        if !usable {
            return Err(refused(
                ew_commands::text_key!("reason-cores-no-wires"),
                Vec::new(),
            ));
        }
        let order: Vec<_> = device
            .parts
            .first()
            .and_then(|u| q.parts().get(u.part.copy))
            .map(|p| {
                p.item
                    .sections
                    .cable_cores
                    .iter()
                    .map(|c| c.core.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut cores: Vec<(usize, Conductor)> = q
            .conductors()
            .iter()
            .filter(|(_, c)| c.cable == cable_id)
            .map(|(_, c)| c.as_ref().clone())
            .map(|c| (order.iter().position(|o| *o == c.core).unwrap_or(usize::MAX), c))
            .collect();
        cores.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.core.natural_cmp(&b.1.core)));
        let taken: BTreeSet<Id<Conductor>> = connections
            .iter()
            .filter(|(id, _)| !wires.contains(&id.as_uuid()))
            .filter_map(|(_, c)| match c.kind {
                ConnectionKind::Core(x) => Some(x),
                _ => None,
            })
            .collect();
        let free: Vec<Id<Conductor>> = cores
            .iter()
            .map(|(_, c)| c.id)
            .filter(|c| !taken.contains(c))
            .collect();
        if free.len() < wires.len() {
            let tag = self.device_tag(cable).unwrap_or_default();
            return Err(refused(
                ew_commands::text_key!("error-cores-too-few"),
                vec![tag, free.len().to_string(), wires.len().to_string()],
            ));
        }
        let edits: Vec<_> = wires
            .iter()
            .zip(free)
            .map(|(w, core)| {
                let value = serde_json::to_value(ConnectionKind::Core(core)).unwrap_or_default();
                json!({"kind": "connection", "targets": [w.to_string()], "path": "kind", "value": value})
            })
            .collect();
        Ok(AnyCommand {
            name: "fields.set_many".into(),
            params: json!({ "edits": edits }),
        })
    }
}
