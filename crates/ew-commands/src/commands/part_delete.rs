// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part.delete` (`DeletePartCopy`): Delete part… of Parts (doc 07 §5).

use ew_ids::Id;
use ew_schema::{Kind, LibCopy, Part};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// `part.delete` (`DeletePartCopy`): deletes a part of the project's own (a project-local part copy) that nothing
/// names. A part from a library is kept (it is the library's; the project drops a copy nothing uses when it is
/// purged). Who names the copy is the link table's one question (`ew_model::LINKS`, the delete policy that edit.delete
/// and the store's dangling-reference check read too): a part a device uses, another part declares as its accessory,
/// a device's choice or the workshop stock names is refused in words that name the first that does. One undo step.
/// Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeletePartCopy {
    /// The part copy to delete.
    #[schemars(with = "UuidS")]
    pub copy: Id<Part>,
}

fn bad(message: ew_l10n::Text) -> CmdError {
    CmdError::invalid_params(DeletePartCopy::NAME, "/copy", message)
}

impl Command for DeletePartCopy {
    const NAME: &'static str = "part.delete";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(held) = tx.get::<LibCopy<Part>>(self.copy) else {
            return Err(bad(text!(
                "refusal-part-edit-copy-missing",
                self.copy.to_string()
            )));
        };
        let number = held.item.part_number.as_str().to_owned();
        if held.library_id.as_uuid() != tx.project().id.as_uuid() {
            return Err(bad(text!("refusal-part-delete-library", number)));
        }
        tx.delete((Kind::Part, self.copy.cast()))?;
        Ok(Outcome::default())
    }
}
