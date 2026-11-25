// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The members `supply.<path>` of `project.settings.set`: one member of the project's supply (doc 03 §5).

use ew_schema::{ExternalRef, ProjectKind, SupplySpec};
use ew_units::Dec3;
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::fields::set_member;
use crate::commands::settings::SetProjectSettings;
use crate::error::CmdError;
use crate::tx::TxBuilder;

/// The prefix of a member of `project.settings.set` that names a member of the supply (`supply.u_nom`).
pub(crate) const SUPPLY_PREFIX: &str = "supply.";

/// The member paths of the supply that `project.settings.set` edits as `supply.<path>` (the `SupplySpec` JSON form):
/// its electrical members and its origin (ADR-0008 amendment 6).
pub const SETTABLE_SUPPLY: &[&str] = &[
    "phases",
    "neutral",
    "pe",
    "earthing",
    "u_nom",
    "frequency",
    "origin.upstream_rating",
    "origin.ik_max",
    "origin.source",
];

/// Sets the member `path` of the project's supply to `value`, the member `supply.<path>` of `project.settings.set`
/// (doc 06 §3.1, ADR-0008 amendment 6): its phases, neutral, protective earth, earthing system, nominal voltage,
/// frequency, upstream rating, short-circuit current, or the external reference it comes from ("J-E-206-L").
///
/// `path` is one of [`SETTABLE_SUPPLY`]; `value` is the member's JSON form, read back into the supply type (a value it
/// does not accept is invalid at `/value`); `null` removes an optional member. Validated in words at `/value`: one to three phases, a nominal voltage above 0, a frequency of 0
/// (DC) or above, an upstream rating and a short-circuit current above 0. A schematic project without a supply (a new
/// one's) takes the supply of one phase with N and PE, 230 V, 50 Hz, its earthing unset, with the first member set.
/// Rejected: a basic or macro project, which has no supply (FR-PRJ-026), and an `origin.source` that names no
/// external reference ([`CmdError::NotFound`]). The cover page and the title blocks print the supply. Only the
/// project record changes.
pub(crate) fn set(tx: &mut TxBuilder<'_>, path: &str, value: &Value) -> Result<Outcome, CmdError> {
    let name = SetProjectSettings::NAME;
    let mut project = (*tx.project()).clone();
    let supply = match (&project.supply, &project.kind) {
        (Some(s), _) => s.clone(),
        (None, ProjectKind::Schematic) => SupplySpec {
            phases: 1,
            neutral: true,
            pe: true,
            earthing: None,
            u_nom: Dec3(230_000),
            frequency: Dec3(50_000),
            origin: None,
            extra: Default::default(),
        },
        (None, _) => return tx.reject(text!("refusal-supply-none")),
    };
    let mut json = serde_json::to_value(&supply)
        .map_err(|e| CmdError::internal(format!("project.settings.set: cannot serialise ({e})")))?;
    let segments: Vec<&str> = path.split('.').collect();
    if !set_member(&mut json, &segments, value) {
        return Err(CmdError::invalid_params(
            name,
            "/member",
            text!("refusal-fields-below-non-object", path.to_string()),
        ));
    }
    let next: SupplySpec = serde_json::from_value(json).map_err(|e| {
        CmdError::invalid_params(
            name,
            "/value",
            text!("refusal-params-detail", crate::error::serde_error_text(&e)),
        )
    })?;
    let origin = next.origin.as_ref();
    let wrong = if !(1..=3).contains(&next.phases) {
        Some(text!("refusal-supply-phases"))
    } else if next.u_nom <= Dec3::ZERO {
        Some(text!("refusal-supply-voltage"))
    } else if next.frequency < Dec3::ZERO {
        Some(text!("refusal-supply-frequency"))
    } else if [
        origin.and_then(|o| o.upstream_rating),
        origin.and_then(|o| o.ik_max),
    ]
    .into_iter()
    .flatten()
    .any(|v| v <= Dec3::ZERO)
    {
        Some(text!("refusal-supply-current"))
    } else {
        None
    };
    if let Some(why) = wrong {
        return Err(CmdError::invalid_params(name, "/value", why));
    }
    if let Some(source) = origin.and_then(|o| o.source) {
        tx.require::<ExternalRef>(source)?;
    }
    project.supply = Some(next);
    tx.set_project(project)?;
    Ok(Outcome::default())
}
