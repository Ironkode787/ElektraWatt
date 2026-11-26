// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `edit.delete`: deletes a selection (doc 06 §3.4, §3.6). `edit.paste` lives in `paste.rs`.

use std::collections::BTreeSet;

use ew_derive::Q;
use ew_derive::layout::carrier_labels;
use ew_ids::Id;
use ew_model::{AnyKey, ModelError};
use ew_schema::{Carrier, Device, Enclosure, Kind, MountingSurface, Placement};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::common::TargetRef;
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::tx::TxBuilder;

/// Deletions that remove more than this many records (the targets and everything their delete plan
/// takes along) run as bulk commands (doc 06 §3.1).
pub const BULK_ABOVE: usize = 500;

/// `edit.delete`: removes the selected records with everything the delete policies take along
/// (FR-DEV-030), in **one** delete plan for all targets (`TxBuilder::delete_many`): the result does not
/// depend on the order of the targets, a target that another target's cascade removes is planned once,
/// and a target listed twice counts once. Deleting a representation leaves its function unplaced (an IP
/// function goes with it); deleting a device removes its functions, their representations and
/// connections. A strip takes along its terminals that no symbol of their own draws, with their wires (a cut or a
/// delete leaves them so); one with a terminal still drawn is refused in words, naming it and its page
/// ([`strip_terminals`]). A target that does not exist at the start is `NotFound`; a forbid link (a referenced
/// structure node) rejects the whole command (`ModelError::Referenced`). Cost:
/// `Bulk` when the plan removes or changes more than 500 records (the plan is computed once by
/// `cost`). A released or withdrawn revision is immutable (FR-REV-009): naming one is refused at
/// `/targets/<i>`; the open revision may be deleted. A deleted page takes the open revision's note on it along.
///
/// The layout kinds (`layout_space`, `enclosure`, `mounting_surface`, `carrier`, `placement`)
/// delete with the model's cascades as `layout.*.delete` do, after four checks on the selection: a plate
/// surface without its enclosure is refused; a plan that removes the device of an enclosure is refused; a carrier left
/// holding a placement the same delete does not remove is refused (`layout.carrier.delete` with `then` empties it),
/// the first in `carrier_labels` order; a surface left holding placements of its own likewise. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteSelection {
    /// The selected records, at least one.
    pub targets: Vec<TargetRef>,
}

impl Command for DeleteSelection {
    const NAME: &'static str = "edit.delete";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, model: &ew_model::Model) -> Cost {
        let keys: Vec<ew_model::AnyKey> = self.targets.iter().map(|t| t.key()).collect();
        // A plan that cannot be made (a missing target, a forbid link) is refused by `run`; it costs
        // nothing.
        match model.delete_plan_many(&keys) {
            Ok(plan) if plan.len() > BULK_ABOVE => Cost::Bulk,
            _ => Cost::Interactive,
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.targets.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/targets",
                text!("refusal-params-no-target"),
            ));
        }
        for (i, t) in self.targets.iter().enumerate() {
            if t.kind == crate::commands::common::KindName::Project {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/targets/{i}"),
                    text!("refusal-edit-delete-project"),
                ));
            }
            if tx.row(&t.key()).is_none() {
                return Err(CmdError::NotFound { key: t.key() });
            }
            if t.kind == crate::commands::common::KindName::Revision {
                crate::commands::revision::refuse_closed_revision(
                    tx,
                    Self::NAME,
                    &format!("/targets/{i}"),
                    t.id.cast(),
                )?;
            }
        }
        let mut keys: Vec<ew_model::AnyKey> = self.targets.iter().map(|t| t.key()).collect();
        keys.extend(strip_terminals(tx, &keys)?);
        refuse_layout_delete(tx, &keys)?;
        tx.delete_many(&keys)?;
        crate::commands::revision::drop_notes_on_removed_pages(tx)?;
        Ok(Outcome::default())
    }
}

/// The layout checks of `edit.delete`, before the delete plan is applied: "occupied" means
/// holding a placement the same delete does not remove. The removed placements and devices come from the plan of the
/// targets that are not layout containers (spaces, enclosures, surfaces, carriers); the containers' closure from the
/// records read through the transaction.
fn refuse_layout_delete(tx: &TxBuilder<'_>, keys: &[AnyKey]) -> Result<(), CmdError> {
    let container = |k: &AnyKey| {
        matches!(
            k.0,
            Kind::LayoutSpace | Kind::Enclosure | Kind::MountingSurface | Kind::Carrier
        )
    };
    let others: Vec<AnyKey> = keys.iter().copied().filter(|k| !container(k)).collect();
    let mut removed: BTreeSet<AnyKey> = BTreeSet::new();
    if !others.is_empty() {
        match tx.base_model().delete_plan_many(&others) {
            Ok(plan) => removed.extend(plan.iter().filter(|op| op.after().is_none()).map(|op| op.key())),
            Err(ModelError::Referenced { link, target, .. }) if link == "enclosure.device" => {
                return carries_enclosure(tx, target.1.cast());
            }
            // Another forbid link fired first (-UC1's plate child names it through `device.parent`): a targeted
            // enclosure device is still refused in these words; the delete itself reports any other refusal.
            Err(_) => {
                let targeted: BTreeSet<AnyKey> =
                    others.iter().copied().filter(|k| k.0 == Kind::Device).collect();
                if !targeted.is_empty() {
                    if let Some(e) = tx
                        .iter::<Enclosure>()
                        .into_iter()
                        .find(|e| targeted.contains(&(Kind::Device, e.device.cast())))
                    {
                        return carries_enclosure(tx, e.device);
                    }
                }
            }
        }
    }
    let of = |kind: Kind| -> BTreeSet<AnyKey> { keys.iter().copied().filter(|k| k.0 == kind).collect() };
    let (spaces, enclosures, surfaces, carriers) = (
        of(Kind::LayoutSpace),
        of(Kind::Enclosure),
        of(Kind::MountingSurface),
        of(Kind::Carrier),
    );
    if spaces.is_empty() && enclosures.is_empty() && surfaces.is_empty() && carriers.is_empty() {
        return Ok(());
    }
    let all_enclosures = tx.iter::<Enclosure>();
    // a removed device that carries an enclosure, whatever the rest of the selection
    if let Some(e) = all_enclosures
        .iter()
        .find(|e| removed.contains(&(Kind::Device, e.device.cast())))
    {
        return carries_enclosure(tx, e.device);
    }
    let gone_enclosures: BTreeSet<Id<Enclosure>> = all_enclosures
        .iter()
        .filter(|e| {
            enclosures.contains(&(Kind::Enclosure, e.id.cast()))
                || spaces.contains(&(Kind::LayoutSpace, e.space.cast()))
        })
        .map(|e| e.id)
        .collect();
    let all_surfaces = tx.iter::<MountingSurface>();
    for s in &all_surfaces {
        if surfaces.contains(&(Kind::MountingSurface, s.id.cast())) && !gone_enclosures.contains(&s.enclosure)
        {
            let device = tx.get::<Enclosure>(s.enclosure).map(|e| e.device);
            return tx.reject(text!(
                "refusal-edit-delete-plate",
                device.map_or_else(
                    || text!("refusal-edit-delete-the-plate").into(),
                    |d| crate::words::device(tx, d)
                )
            ));
        }
    }
    let gone_surfaces: Vec<&MountingSurface> = all_surfaces
        .iter()
        .map(AsRef::as_ref)
        .filter(|s| {
            surfaces.contains(&(Kind::MountingSurface, s.id.cast())) || gone_enclosures.contains(&s.enclosure)
        })
        .collect();
    let left = |placements: Vec<Id<Placement>>| -> usize {
        placements
            .into_iter()
            .filter(|p| !removed.contains(&(Kind::Placement, p.cast())))
            .count()
    };
    let holds = |n: usize| {
        if n == 1 {
            text!("refusal-edit-delete-holds-one")
        } else {
            text!("refusal-edit-delete-holds", n.to_string())
        }
    };
    // the carriers that go: the targeted ones and those of the surfaces that go, in label order per surface
    let base_model = tx.base_model().clone();
    let q = Q::new(&base_model);
    let mut surface_ids: BTreeSet<Id<MountingSurface>> = gone_surfaces.iter().map(|s| s.id).collect();
    for c in &carriers {
        if let Some(row) = tx.get::<Carrier>(c.1.cast()) {
            surface_ids.insert(row.surface);
        }
    }
    for surface in &surface_ids {
        let goes = gone_surfaces.iter().any(|s| s.id == *surface);
        for c in tx.carriers_of_surface(*surface) {
            let _ = tx.get::<Carrier>(c);
        }
        for (c, label) in carrier_labels(&q, *surface) {
            if !goes && !carriers.contains(&(Kind::Carrier, c.cast())) {
                continue;
            }
            let n = left(tx.placements_on_carrier(c));
            if n > 0 {
                return tx.reject(text!(
                    "refusal-edit-delete-carrier-holds",
                    ew_derive::text::carrier_label(label),
                    holds(n)
                ));
            }
        }
    }
    for s in &gone_surfaces {
        let n = left(tx.placements_on_surface(s.id));
        if n > 0 {
            let device = tx.get::<Enclosure>(s.enclosure).map(|e| e.device);
            return tx.reject(text!(
                "refusal-edit-delete-plate-holds",
                device.map_or_else(
                    || text!("refusal-edit-delete-the-enclosure").into(),
                    |d| crate::words::device(tx, d)
                ),
                holds(n)
            ));
        }
    }
    Ok(())
}

/// The terminals that go with the strips among `keys` (doc 08 §6.6): a strip whose terminals the same delete does not
/// name takes them along, with their wires and bridges, when none is drawn by a symbol of its own (terminals a cut or
/// a delete left behind: a wire at such a terminal is drawn on no page, so it could not be found to delete it first);
/// a strip with a drawn terminal is refused, naming the first in strip order and the page it is drawn on.
fn strip_terminals(tx: &TxBuilder<'_>, keys: &[AnyKey]) -> Result<Vec<AnyKey>, CmdError> {
    let mut out = Vec::new();
    for key in keys.iter().filter(|k| k.0 == Kind::Device) {
        let strip: Id<Device> = key.1.cast();
        if tx.get::<Device>(strip).is_none_or(|d| d.aspects.strip.is_none()) {
            continue;
        }
        let mut terminals: Vec<(u32, Id<Device>)> = tx
            .terminals_of_strip(strip)
            .into_iter()
            .filter(|t| !keys.contains(&(Kind::Device, t.cast())))
            .filter_map(|t| Some((tx.get::<Device>(t)?.aspects.terminal.as_ref()?.seq, t)))
            .collect();
        terminals.sort_unstable();
        for (_, t) in &terminals {
            for f in tx.functions_of_device(*t) {
                if let Some(rep) = tx.reps_of_function(f).first() {
                    let page = tx.get::<ew_schema::Representation>(*rep).map(|r| r.page);
                    return tx.reject(text!(
                        "refusal-edit-delete-strip-drawn",
                        crate::words::device(tx, strip),
                        crate::words::device(tx, *t),
                        page.map_or_else(|| text!("refusal-words-page-gone"), |p| crate::words::page(tx, p))
                    ));
                }
            }
        }
        out.extend(terminals.into_iter().map(|(_, t)| (Kind::Device, t.cast())));
    }
    Ok(out)
}

/// "-UC1 carries an enclosure; delete the enclosure first."
fn carries_enclosure(tx: &TxBuilder<'_>, device: Id<Device>) -> Result<(), CmdError> {
    tx.reject(text!(
        "refusal-edit-delete-carries-enclosure",
        crate::words::device(tx, device)
    ))
}
