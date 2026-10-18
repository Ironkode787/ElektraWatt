// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `edit.delete`: deletes a selection (doc 06 §3.4, §3.6). `edit.paste` is not in v0 (fragments and
//! paste options arrive with WP-S1.13).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::commands::common::TargetRef;
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::tx::TxBuilder;

/// Selections above this many records run as bulk commands (doc 06 §3.1).
pub const BULK_ABOVE: usize = 500;

/// `edit.delete`: removes the selected records with everything the delete policies take along
/// (FR-DEV-030): each target in order through `TxBuilder::delete`; a target already removed by an earlier
/// target's cascade is skipped. Deleting a representation leaves its function unplaced (an IP function goes
/// with it); deleting a device removes its functions, their representations and connections. A target that
/// does not exist at the start is `NotFound`; a forbid link (a strip with terminals, a referenced structure
/// node) rejects the whole command (`ModelError::Referenced`). Cost: `Bulk` above 500 targets. Outcome:
/// none.
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

    fn cost(&self) -> Cost {
        if self.targets.len() > BULK_ABOVE {
            Cost::Bulk
        } else {
            Cost::Interactive
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
        for t in &self.targets {
            if tx.row(&t.key()).is_none() {
                return Err(CmdError::NotFound { key: t.key() });
            }
        }
        for t in &self.targets {
            tx.delete_if_exists(t.key())?;
        }
        Ok(Outcome::default())
    }
}
