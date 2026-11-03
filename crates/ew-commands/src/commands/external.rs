// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `external.apply` (`ApplyExternal`, WP-S1.17 D22, decision log ID-1284 ruling 1): an external change, merged by the
//! session, committed as one undoable transaction.

use ew_model::{AnyKey, AnyRow, Asset};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Exposure, Stability};
use crate::tx::TxBuilder;

/// `external.apply` (`ApplyExternal`): the records of a project that changed on disk (a pull, a checkout, a sync
/// client), as the session merged them three-way with its own edits (doc 05 §9; WP-S1.17 D22).
///
/// Exposure `Internal`: only the session's external-change apply submits it, from the origin `external`; scripts, the
/// palette, MCP, plugins and the CLI cannot. It runs no normaliser and no prevent rule (the state another build saved
/// is taken as it is); the structural invariants hold. Its rows and assets are not parameters: a submission from JSON
/// writes nothing. One transaction, one undo step "External change". Outcome: none.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyExternal {
    /// The generation on disk that the change brings.
    pub generation: u64,
    /// The short hash of the git commit checked out, when the project is in a git worktree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The rows written (`None`: removed), each where the merged record differs from the session's.
    #[serde(skip)]
    pub rows: Vec<(AnyKey, Option<AnyRow>)>,
    /// The assets the incoming generation holds that the session's model does not.
    #[serde(skip)]
    pub assets: Vec<Asset>,
}

impl Command for ApplyExternal {
    const NAME: &'static str = "external.apply";

    fn meta() -> CommandMeta {
        CommandMeta {
            exposure: Exposure::Internal,
            stability: Stability::Internal,
            ..CommandMeta::edit(Self::NAME)
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        for asset in &self.assets {
            tx.put_asset(asset.bytes().to_vec()).map_err(|e| {
                CmdError::rejected(Self::NAME, format!("an image of the change does not read ({e})"))
            })?;
        }
        // Removals first: a removal's own bookkeeping (a macro member's instance) never outlives a row written after it.
        for (key, row) in &self.rows {
            if row.is_none() && tx.row(key).is_some() {
                tx.remove_row(*key);
            }
        }
        for (_, row) in &self.rows {
            match row {
                Some(AnyRow::Project(project)) => tx.set_project((**project).clone())?,
                Some(row) => tx.put_row(row.clone()),
                None => {}
            }
        }
        Ok(Outcome::default())
    }
}
