// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `workshop.stock` (`SetWorkshopStock`): the project's workshop stock (doc 07 §2.5 "The workshop stock").

use std::collections::BTreeSet;

use ew_schema::{LibCopy, Part, PartRef};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;

/// `workshop.stock` (`SetWorkshopStock`): sets the wire, ferrule and marker parts the shop keeps, in its order of
/// preference (`ProjectSettings.workshop.stock`; the cut increment and the other members stay), embedding the copies
/// the command carries (the part and its closure, as `part.assign` embeds them). A new project's generic stock, Add
/// the generic stock and the stock dialog each send it.
///
/// Rejected, with the pointer: a document of `copies` as `library.embed` refuses it, or another version of a held copy
/// (`/copies/i`); a stock entry whose copy the project does not hold, a part that is no wire, ferrule or marker, or a
/// part listed twice (`/stock/i`). One undo step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetWorkshopStock {
    /// The stock in its order of preference; empty: no stock.
    pub stock: Vec<PartRef>,
    /// Library documents to embed (`lib/` JSON form): the stock's parts the project lacks, with their closure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

fn bad(at: impl Into<String>, message: ew_l10n::Text) -> CmdError {
    CmdError::invalid_params(SetWorkshopStock::NAME, at, message)
}

impl Command for SetWorkshopStock {
    const NAME: &'static str = "workshop.stock";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        embed_closure(Self::NAME, tx, &self.copies)?;
        let mut seen = BTreeSet::new();
        for (i, entry) in self.stock.iter().enumerate() {
            let at = format!("/stock/{i}");
            let Some(copy) = tx.get::<LibCopy<Part>>(entry.copy) else {
                return Err(bad(at, text!("refusal-stock-missing", entry.copy.to_string())));
            };
            let number = copy.item.part_number.as_str().to_owned();
            if !copy.item.kind.is_consumable() {
                return Err(bad(at, text!("refusal-stock-kind", number)));
            }
            if !seen.insert(entry.copy) {
                return Err(bad(at, text!("refusal-stock-twice", number)));
            }
        }
        let mut project = (*tx.project()).clone();
        if project.settings.workshop.stock == self.stock {
            return tx.reject(text!("refusal-stock-unchanged"));
        }
        project.settings.workshop.stock = self.stock.clone();
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}
