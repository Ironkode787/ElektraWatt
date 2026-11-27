// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `workshop.stock` (`SetWorkshopStock`): the project's workshop stock (doc 07 §2.5 "The workshop stock").

use std::collections::BTreeSet;

use ew_schema::{LibCopy, Part, PartRef};
use ew_units::Nm;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::NmS;
use crate::tx::TxBuilder;

/// `workshop.stock` (`SetWorkshopStock`): sets the wire, ferrule and marker parts the shop keeps, in its order of
/// preference (`ProjectSettings.workshop.stock`), and, when given, what the cut list cuts to
/// (`ProjectSettings.workshop.cut_increment`: a length sets it, `null` clears it, absent keeps it), embedding the copies
/// the command carries (the part and its closure, as `part.assign` embeds them). A new project's generic stock, Add
/// the generic stock and the Workshop stock of Parts each send it.
///
/// Rejected, with the pointer: a document of `copies` as `library.embed` refuses it, or another version of a held copy
/// (`/copies/i`); a stock entry whose copy the project does not hold, a part that is no wire, ferrule or marker, or a
/// part listed twice (`/stock/i`); a cut increment of zero or less (`/cut_increment`). One undo step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetWorkshopStock {
    /// The stock in its order of preference; empty: no stock.
    pub stock: Vec<PartRef>,
    /// Library documents to embed (`lib/` JSON form): the stock's parts the project lacks, with their closure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
    /// What the cut list rounds a piece up to: a length sets it, `null` clears it (5 mm), absent keeps it.
    #[serde(
        default,
        deserialize_with = "crate::commands::layout::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<Option<NmS>>")]
    pub cut_increment: Option<Option<Nm>>,
}

fn bad(at: impl Into<String>, message: ew_l10n::Text) -> CmdError {
    CmdError::invalid_params(SetWorkshopStock::NAME, at, message)
}

/// A refusal of `project.settings.set` at `at`.
fn settings_bad(at: impl Into<String>, message: ew_l10n::Text) -> CmdError {
    CmdError::invalid_params(crate::commands::settings::SetProjectSettings::NAME, at, message)
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
            if !copy.item.kind.in_stock() {
                return Err(bad(at, text!("refusal-stock-kind", number)));
            }
            if !seen.insert(entry.copy) {
                return Err(bad(at, text!("refusal-stock-twice", number)));
            }
        }
        let mut project = (*tx.project()).clone();
        let cut = match &self.cut_increment {
            Some(Some(n)) if n.0 <= 0 => {
                return Err(bad(
                    "/cut_increment",
                    text!("refusal-stock-cut", ew_derive::mm_exact(i128::from(n.0))),
                ));
            }
            Some(n) => *n,
            None => project.settings.workshop.cut_increment,
        };
        if project.settings.workshop.stock == self.stock && project.settings.workshop.cut_increment == cut {
            return tx.reject(text!("refusal-stock-unchanged"));
        }
        project.settings.workshop.stock = self.stock.clone();
        project.settings.workshop.cut_increment = cut;
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}

/// The members of the workshop settings that `project.settings.set` sets one by one, as `workshop.<member>` (doc 13
/// §2.3): which fine-stranded ends take a ferrule, what two wires of one size on one clamp take, what a marker says,
/// whether cores and jumpers carry one, where a device's label goes and the columns of a label file; the layout's
/// numbers (doc 12 §8.6, §9): the reserve per end of a routed wire, the duct fill limit and the door loop; and what the
/// cut list cuts to (doc 07 §2.5, Project properties ▸ Workshop).
pub const SETTABLE_WORKSHOP: &[&str] = &[
    "ferrules",
    "two_wires",
    "marking",
    "mark_cores",
    "mark_jumpers",
    "device_labels",
    "label_profile",
    // How the holes are made and measured (doc 12-3d §10.3, doc 12 §2.8; `hole_member`).
    "fixing",
    "rail_fixing.screw",
    "rail_fixing.pitch",
    "rail_fixing.from_end",
    "duct_fixing.screw",
    "duct_fixing.pitch",
    "duct_fixing.from_end",
    "min_web",
    "datum",
    "reserve",
    "fill_limit",
    "door_loop",
    "cut_increment",
];

/// The prefix of a member of [`SETTABLE_WORKSHOP`] in `project.settings.set`.
pub const WORKSHOP_PREFIX: &str = "workshop.";

/// Whether the profile `p`'s values of a newer format (its delimiter, its file form) are each the one the project holds
/// in `held`, and it has at least one: a value set as it was read.
fn newer_kept(p: &ew_schema::ExportProfile, held: Option<&ew_schema::ExportProfile>) -> bool {
    let delimiter = matches!(p.delimiter, ew_schema::Delimiter::Unknown(_));
    let file = matches!(p.file, Some(ew_schema::FileForm::Unknown(_)));
    (delimiter || file)
        && held.is_some_and(|h| (!delimiter || h.delimiter == p.delimiter) && (!file || h.file == p.file))
}

/// Sets the member `path` of [`SETTABLE_WORKSHOP`] of the project's workshop settings to `value`, the member's JSON form
/// (`"not_into_spring_clamps"`); a value it does not take is refused at `/value` with the values it takes, and `null`
/// gives the member back its default. Only the project record changes; the wire list's end cells follow.
pub(crate) fn set_member(tx: &mut TxBuilder<'_>, path: &str, value: &Value) -> Result<Outcome, CmdError> {
    use crate::commands::settings::member_value;
    let member = format!("{WORKSHOP_PREFIX}{path}");
    let mut project = (*tx.project()).clone();
    let workshop = &mut project.settings.workshop;
    match (path, value.is_null()) {
        ("ferrules", true) => workshop.ferrules = Default::default(),
        ("ferrules", false) => workshop.ferrules = member_value(&member, value)?,
        ("two_wires", true) => workshop.two_wires = Default::default(),
        ("two_wires", false) => workshop.two_wires = member_value(&member, value)?,
        ("marking", true) => workshop.marking = Default::default(),
        ("marking", false) => workshop.marking = member_value(&member, value)?,
        ("mark_cores", true) => workshop.mark_cores = false,
        ("mark_cores", false) => workshop.mark_cores = member_value(&member, value)?,
        ("mark_jumpers", true) => workshop.mark_jumpers = false,
        ("mark_jumpers", false) => workshop.mark_jumpers = member_value(&member, value)?,
        ("device_labels", true) => workshop.device_labels = Default::default(),
        ("device_labels", false) => workshop.device_labels = member_value(&member, value)?,
        ("label_profile", true) => workshop.label_profile = None,
        ("label_profile", false) => {
            // A delimiter or a file form of a newer format is taken where the project holds it already: the person left
            // it as it is. Any other value this build does not know is refused with the values it takes.
            let profile: ew_schema::ExportProfile = match member_value(&member, value) {
                Ok(p) => p,
                Err(refused) => match serde_json::from_value::<ew_schema::ExportProfile>(value.clone()) {
                    Ok(p) if newer_kept(&p, workshop.label_profile.as_ref()) => p,
                    _ => return Err(refused),
                },
            };
            if profile.columns.is_empty() {
                return Err(settings_bad(
                    "/value/columns",
                    text!("refusal-label-profile-empty"),
                ));
            }
            if let Some((i, c)) = profile
                .columns
                .iter()
                .enumerate()
                .find(|(_, c)| ew_derive::LabelColumn::parse(&c.key).is_none())
            {
                let keys: Vec<&str> = ew_derive::LabelColumn::ALL.iter().map(|c| c.key()).collect();
                return Err(settings_bad(
                    format!("/value/columns/{i}/key"),
                    text!("refusal-label-profile-column", c.key.clone(), keys.join(", ")),
                ));
            }
            workshop.label_profile = Some(profile);
        }
        ("reserve", true) => workshop.reserve = None,
        ("door_loop", true) => workshop.door_loop = None,
        ("reserve" | "door_loop", false) => {
            let n: ew_units::Nm = member_value(&member, value)?;
            if n.0 < 0 {
                return Err(settings_bad("/value", text!("refusal-workshop-length-negative")));
            }
            if path == "reserve" {
                workshop.reserve = Some(n);
            } else {
                workshop.door_loop = Some(n);
            }
        }
        ("cut_increment", true) => workshop.cut_increment = None,
        ("cut_increment", false) => {
            let n: ew_units::Nm = member_value(&member, value)?;
            if n.0 <= 0 {
                return Err(settings_bad(
                    "/value",
                    text!("refusal-stock-cut", ew_derive::mm_exact(i128::from(n.0))),
                ));
            }
            workshop.cut_increment = Some(n);
        }
        ("fill_limit", true) => workshop.fill_limit = None,
        ("fill_limit", false) => {
            let n: i64 = member_value(&member, value)?;
            let Some(limit) = u8::try_from(n).ok().filter(|n| (1..=100).contains(n)) else {
                return Err(settings_bad("/value", text!("refusal-workshop-fill-limit")));
            };
            workshop.fill_limit = Some(limit);
        }
        (p, _) if matches!(p, "fixing" | "min_web" | "datum") || p.contains("_fixing.") => {
            crate::commands::layout::hole_member(tx, workshop, &member, path, value)?;
        }
        _ => {
            return Err(CmdError::internal(format!(
                "project.settings.set: {member} is not settable"
            )));
        }
    }
    tx.set_project(project)?;
    Ok(Outcome::default())
}
