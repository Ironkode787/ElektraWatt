// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `edit.delete`: deletes a selection (doc 06 §3.4, §3.6). `edit.paste` lives in `paste.rs` (WP-S2.05 part 3).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::common::TargetRef;
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::tx::TxBuilder;

/// Deletions that remove more than this many records (the targets and everything their delete plan
/// takes along) run as bulk commands (doc 06 §3.1, D4).
pub const BULK_ABOVE: usize = 500;

/// `edit.delete`: removes the selected records with everything the delete policies take along
/// (FR-DEV-030), in **one** delete plan for all targets (`TxBuilder::delete_many`): the result does not
/// depend on the order of the targets, a target that another target's cascade removes is planned once,
/// and a target listed twice counts once. Deleting a representation leaves its function unplaced (an IP
/// function goes with it); deleting a device removes its functions, their representations and
/// connections. A target that does not exist at the start is `NotFound`; a forbid link (a strip with
/// terminals, a referenced structure node) rejects the whole command (`ModelError::Referenced`). Cost:
/// `Bulk` when the plan removes or changes more than 500 records (D4; the plan is computed once by
/// `cost`). A released or withdrawn revision is immutable (FR-REV-009): naming one is refused at
/// `/targets/<i>`; the open revision may be deleted. Outcome: none.
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
                "give at least one target",
            ));
        }
        for (i, t) in self.targets.iter().enumerate() {
            if t.kind == crate::commands::common::KindName::Project {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/targets/{i}"),
                    "the project cannot be deleted",
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
        let keys: Vec<ew_model::AnyKey> = self.targets.iter().map(|t| t.key()).collect();
        tx.delete_many(&keys)?;
        Ok(Outcome::default())
    }
}
