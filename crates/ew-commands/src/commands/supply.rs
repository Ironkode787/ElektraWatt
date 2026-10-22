// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.supply.set` (`SetProjectSupply`, FT-270): one member of the project's supply (doc 03 §5).

use ew_schema::{ExternalRef, SupplySpec};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::fields::set_member;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::tx::TxBuilder;

/// The member paths of the supply that `project.supply.set` edits (the `SupplySpec` JSON form).
///
/// v0 (owner ruling, cycle 16): only `origin.source`, the external reference the supply comes from. The
/// electrical members (phases, earthing, nominal voltage, frequency, upstream rating, short-circuit current)
/// change derived values project-wide; they wait for an ADR-0008 amendment that makes them Bulk with a
/// preview and semantic validation (ADR-0008 amendment 3).
pub const SETTABLE_SUPPLY: &[&str] = &["origin.source"];

/// `project.supply.set` (`SetProjectSupply`): sets one member of the project's supply; in v0 only its
/// origin, the external reference the supply comes from ("J-E-206-L", doc 20 §4.2).
///
/// `path` is one of [`SETTABLE_SUPPLY`] (else invalid at `/path`); `value` is the member's JSON form, read
/// back into the supply type (a value it does not accept is invalid at `/value`); `null` removes an
/// optional member. Rejected: a project without a supply (a basic or macro project, FR-PRJ-026: the supply
/// is part of the project's creation), and an `origin.source` that names no external reference
/// ([`CmdError::NotFound`]). Only the project record changes. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProjectSupply {
    /// The dotted member path; v0: `origin.source`.
    pub path: String,
    /// The new value; `null` removes an optional member.
    pub value: Value,
}

impl Command for SetProjectSupply {
    const NAME: &'static str = "project.supply.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !SETTABLE_SUPPLY.contains(&self.path.as_str()) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/path",
                format!(
                    "`{}` is not a settable supply member; settable: {}",
                    self.path,
                    SETTABLE_SUPPLY.join(", ")
                ),
            ));
        }
        let mut project = (*tx.project()).clone();
        let Some(supply) = project.supply.as_ref() else {
            return tx
                .reject("the project has no supply (a basic or macro project); there is nothing to set");
        };
        let mut json = serde_json::to_value(supply)
            .map_err(|e| CmdError::internal(format!("project.supply.set: cannot serialise ({e})")))?;
        let segments: Vec<&str> = self.path.split('.').collect();
        if !set_member(&mut json, &segments, &self.value) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/path",
                format!("`{}` lies below a value that is not an object", self.path),
            ));
        }
        let next: SupplySpec = serde_json::from_value(json)
            .map_err(|e| CmdError::invalid_params(Self::NAME, "/value", e.to_string()))?;
        if let Some(source) = next.origin.as_ref().and_then(|o| o.source) {
            tx.require::<ExternalRef>(source)?;
        }
        project.supply = Some(next);
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}
