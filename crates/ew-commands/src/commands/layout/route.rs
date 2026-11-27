// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `layout.route`: Route on a panel page, which stores the ways of a cabinet's wires (doc 12 §8.5).

#[allow(clippy::wildcard_imports)]
use super::*;

use ew_derive::layout::{CabinetWire, Planned, RouteScope, cabinet_name, cabinet_wires, plan_routes};
use ew_schema::{CommittedRoute, Connection};

use crate::command::Counts;

/// `layout.route` (`LayoutRoute`, doc 12 §8.5): plans the cabinet of a layout space and stores the plan, one
/// `committed_route` per wire with a way (inserted or replaced, ID `v5(wire, "route")`); the space's stored ways whose
/// wire is no longer a wire of the cabinet go. Nothing given plans the wires without a way and those whose way no longer
/// holds; `wires` plans those wires, whatever their state; `all` every wire of the cabinet. The plan is
/// `ew_derive::layout::plan_routes`', the preview's too.
///
/// Rejected: a space that does not exist (`/space`); a wire that is not a wire the cabinet's Route lays
/// (`/wires/<i>`). Bulk: it reads the whole cabinet. Outcome: the ways stored (created) and the wires without a way
/// (skipped).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutRoute {
    /// The layout space whose cabinet is routed.
    #[schemars(with = "UuidS")]
    pub space: Id<LayoutSpace>,
    /// The wires to route, by their carrier connections; empty: the due ones (or every one with `all`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<UuidS>")]
    pub wires: Vec<Id<Connection>>,
    /// Route every wire of the cabinet again.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub all: bool,
}

impl Command for LayoutRoute {
    const NAME: &'static str = "layout.route";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<LayoutSpace>(self.space) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/space",
                text!("refusal-params-space-gone"),
            ));
        }
        // The plan reads the whole project through the wire records (the wiring, the potentials, the pages in their
        // order, the parts, the shop's settings) and the cabinet's layout: every table, not a list kept in step by hand.
        let _ = tx.project();
        tx.read_every_table();
        let q = Q::new(tx.base_model());
        let scope = if self.all {
            RouteScope::All
        } else if self.wires.is_empty() {
            RouteScope::Due
        } else {
            let routed: BTreeSet<Id<Connection>> = cabinet_wires(&q, self.space)
                .into_iter()
                .filter(|(_, kind)| *kind == CabinetWire::Routed)
                .map(|(r, _)| r.wire.id)
                .collect();
            if let Some(i) = self.wires.iter().position(|w| !routed.contains(w)) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/wires/{i}"),
                    text!("refusal-route-not-a-wire", cabinet_name(&q, self.space)),
                ));
            }
            RouteScope::Wires(self.wires.iter().copied().collect())
        };
        let plan = plan_routes(&q, self.space, &scope);
        let mut counts = Counts::default();
        for w in &plan.wires {
            match &w.planned {
                Planned::Way(way) => {
                    let record = way.record(self.space, w.wire.wire.id);
                    if tx.get::<CommittedRoute>(record.id).is_none_or(|r| *r != record) {
                        tx.put(record);
                    }
                    counts.created += 1;
                }
                Planned::NoWay(_) => counts.skipped += 1,
                Planned::Kept | Planned::Site | Planned::Core => {}
            }
        }
        for gone in &plan.gone {
            tx.delete_if_exists((Kind::CommittedRoute, gone.cast()))?;
        }
        Ok(Outcome {
            primary: None,
            counts: Some(counts),
            notices: Vec::new(),
        })
    }
}
