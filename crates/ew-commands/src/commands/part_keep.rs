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

/// `part.keep` (`KeepPart`): Keep in my parts (doc 07 §5.8). A part only in this project has been written into My parts,
/// with the parts only in this project that it names (`also`); the project embeds those copies and their closure
/// (`copies`, as `library.embed` takes them) and every use of each old copy moves to its kept copy with everything it
/// chose (a device's variant, a carrier's part, a strip accessory, a wire's part:
/// [`crate::commands::part_edit::repoint_part_copy`]), in one undo step. Keeping a part changes whose it is, never what
/// uses it or how. The old copies stay, unused, until purged.
///
/// Rejected: `copies` as `library.embed` rejects them; a kept copy (`/to`, `/also/<i>/to`) not among them, as a part
/// copy; a copy that is kept (`/from`, `/also/<i>/from`) not a part copy the project holds. Outcome: the kept copy `to`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeepPart {
    /// The project's copy that is kept.
    #[schemars(with = "UuidS")]
    pub from: Id<Part>,
    /// The kept copy, of My parts.
    #[schemars(with = "UuidS")]
    pub to: Id<Part>,
    /// The library documents to embed (`lib/` JSON form): the kept copies and their closure.
    pub copies: Vec<Value>,
    /// The other copies kept in the same step: the parts only in this project that the kept part names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<KeptCopy>,
}

/// A copy of the project's that is kept in My parts and the kept copy that takes its uses.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeptCopy {
    /// The project's copy that is kept.
    #[schemars(with = "UuidS")]
    pub from: Id<Part>,
    /// The kept copy, of My parts.
    #[schemars(with = "UuidS")]
    pub to: Id<Part>,
}

impl Command for KeepPart {
    const NAME: &'static str = "part.keep";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let pairs: Vec<(String, Id<Part>, Id<Part>)> = std::iter::once((String::new(), self.from, self.to))
            .chain((self.also.iter().enumerate()).map(|(i, k)| (format!("/also/{i}"), k.from, k.to)))
            .collect();
        let missing = |at: String, copy: Id<Part>| {
            CmdError::invalid_params(
                Self::NAME,
                &at,
                text!("refusal-part-edit-copy-missing", copy.to_string()),
            )
        };
        for (at, from, _) in &pairs {
            if tx.row(&(Kind::Part, from.cast())).is_none() {
                return Err(missing(format!("{at}/from"), *from));
            }
        }
        // A project that holds every kept copy already (Keep again after an Undo) embeds nothing.
        if !self.copies.is_empty() {
            crate::commands::library::EmbedLibraryCopies {
                copies: self.copies.clone(),
            }
            .run(tx)?;
        }
        for (at, _, to) in &pairs {
            if tx.row(&(Kind::Part, to.cast())).is_none() {
                return Err(missing(format!("{at}/to"), *to));
            }
        }
        for (_, from, to) in pairs {
            crate::commands::part_edit::repoint_part_copy(tx, from, to)?;
        }
        Ok(Outcome::primary((Kind::Part, self.to.cast())))
    }
}
