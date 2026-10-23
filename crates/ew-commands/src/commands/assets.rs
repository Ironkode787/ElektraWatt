// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.assets.purge` (`PurgeAssets`, FT-380): the explicit, previewed clean-up of asset files that nothing
//! names (doc 05 §13.3, decision log ID-574). A save never removes an asset file; this command is the only way one
//! goes.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Confirmation};
use crate::tx::TxBuilder;

/// `project.assets.purge` (`PurgeAssets`): deletes the project's asset files (`assets/<sha256>.<ext>`) that nothing
/// names (doc 05 §13.3, FT-380).
///
/// An asset file goes only when its hash appears in **no project file** (records, kept unknown kinds and members,
/// quarantined and other kept lines, stray and foreign files, the manifest's members other than its file list) and
/// in **no undo or redo entry** of the session (64 hex digits anywhere, either case: `ew_schema::sha256_mentions`).
/// The deletions go through the save protocol's removal list (doc 05 §5.2 step 8): the files leave the manifest of a
/// new generation and are deleted after its commit point, so a crash leaves the old or the new generation.
///
/// The file side runs in the session, which owns the project directory (`ew_api::Session::purge_assets`): the model
/// does not change, so the command commits no ops; it is a **non-undoable marker line** in the history (ADR-0008
/// amendment 1: a transparent marker, `undoable: false`), `Cost::Bulk` (front-ends show the preview, the list of
/// files that would go, `Session::purge_preview`) and `Confirmation::Destructive` (deleted files do not come back by
/// undo). The session refuses it in a read-only session, while the project has quarantined lines, while a journal
/// holds unsaved steps (unsaved edits, a recovered or a crashed session's journal: purge only a saved, journal-free
/// session) and while a save is incomplete; a session without a project directory has no asset files (nothing to
/// purge, the marker is recorded).
///
/// - `assets`: the hashes (`sha256:<64 hex>`) the preview listed. When given, exactly those files go, and the
///   command is refused (nothing deleted) when one of them is no longer purgeable (named again, already gone): the
///   commit then never deletes a file the preview did not show. Absent: every purgeable file at the commit.
///
/// The outcome's notices name the listed hashes. Exposure `Edit`.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PurgeAssets {
    /// The asset hashes to delete, as the preview listed them (`sha256:<64 hex>`); absent: every purgeable asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<String>>,
}

impl PurgeAssets {
    /// The listed hashes, parsed and sorted (`None`: every purgeable asset).
    ///
    /// # Errors
    ///
    /// [`CmdError::InvalidParams`] at `/assets/<i>` for a text that is not `sha256:<64 lower-case hex>` or that is
    /// listed twice.
    pub fn listed(&self) -> Result<Option<std::collections::BTreeSet<ew_schema::Sha256>>, CmdError> {
        let Some(list) = &self.assets else {
            return Ok(None);
        };
        let mut out = std::collections::BTreeSet::new();
        for (i, text) in list.iter().enumerate() {
            let hash: ew_schema::Sha256 = text.parse().map_err(|_| {
                CmdError::invalid_params(
                    Self::NAME,
                    format!("/assets/{i}"),
                    "an asset hash is `sha256:` and 64 lower-case hex digits",
                )
            })?;
            if !out.insert(hash) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    format!("/assets/{i}"),
                    "the asset is listed twice",
                ));
            }
        }
        Ok(Some(out))
    }
}

impl Command for PurgeAssets {
    const NAME: &'static str = "project.assets.purge";

    fn meta() -> CommandMeta {
        CommandMeta {
            undoable: false,
            confirmation: Confirmation::Destructive,
            ..CommandMeta::edit(Self::NAME).bulk()
        }
    }

    fn run(&self, _tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let mut outcome = Outcome {
            note: Some(
                "asset files that nothing names are deleted by the session's save protocol".to_owned(),
            ),
            ..Outcome::default()
        };
        if let Some(listed) = self.listed()? {
            outcome
                .notices
                .extend(listed.iter().map(|h| format!("removes the asset file of {h}")));
        }
        Ok(outcome)
    }
}
