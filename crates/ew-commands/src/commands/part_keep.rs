// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part.keep` (`KeepPart`): Keep in my parts (doc 07 §5.8), the project's side.

use ew_ids::Id;
use ew_schema::{Kind, Part};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// `part.keep` (`KeepPart`): Keep in my parts (doc 07 §5.8). A part only in this project has been written into My parts;
/// the project embeds that copy and its closure (`copies`, as `library.embed` takes them) and every use of the old
/// copy `from` moves to the kept copy `to` with everything it chose (a device's variant, a carrier's part, a strip
/// accessory, a wire's part: [`crate::commands::part_edit::repoint_part_copy`]), in one undo step. Keeping a part
/// changes whose it is, never what uses it or how. The old copy stays, unused, until purged.
///
/// Rejected: `copies` as `library.embed` rejects them; `/to` not among them, as a part copy; `/from` not a part copy
/// the project holds. Outcome: the kept copy.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeepPart {
    /// The project's copy that is kept.
    #[schemars(with = "UuidS")]
    pub from: Id<Part>,
    /// The kept copy, of My parts.
    #[schemars(with = "UuidS")]
    pub to: Id<Part>,
    /// The library documents to embed (`lib/` JSON form): the kept copy and its closure.
    pub copies: Vec<Value>,
}

impl Command for KeepPart {
    const NAME: &'static str = "part.keep";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if tx.row(&(Kind::Part, self.from.cast())).is_none() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/from",
                text!("refusal-part-edit-copy-missing", self.from.to_string()),
            ));
        }
        crate::commands::library::EmbedLibraryCopies {
            copies: self.copies.clone(),
        }
        .run(tx)?;
        if tx.row(&(Kind::Part, self.to.cast())).is_none() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/to",
                text!("refusal-part-edit-copy-missing", self.to.to_string()),
            ));
        }
        crate::commands::part_edit::repoint_part_copy(tx, self.from, self.to)?;
        Ok(Outcome::primary((Kind::Part, self.to.cast())))
    }
}
