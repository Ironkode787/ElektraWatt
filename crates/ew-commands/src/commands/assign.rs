// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part.assign` (`AssignPart`, WP-S1.19 D8, FT-168, FR-PRT-023): the one part assignment command.

use std::collections::BTreeSet;

use ew_ids::Id;

use ew_schema::{Device, Kind, LibCopy, LibDocument, Part, PartRef, PartUse};
use ew_units::Dec3;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::library::check_copy;
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::proxy::{Dec3S, UuidS};
use crate::tx::TxBuilder;

/// From this many devices on, `part.assign` is `Bulk` (WP-S1.19 §8: interactive below 50 devices).
pub const ASSIGN_BULK_FROM: usize = 50;

/// How `part.assign` changes a device's part uses (FR-PRT-023).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssignModeParam {
    /// Adds a part use after the device's existing ones (the default).
    #[default]
    Append,
    /// Replaces every part use of the device by the one part use.
    ReplaceAll,
    /// Replaces each part use of the copy `from` by the part (a device that does not use `from` is refused).
    Replace,
}

/// `part.assign` (`AssignPart`, WP-S1.19 D8, FT-168): assigns a part to devices in one undo step, embedding the
/// library copies it needs (D7: the command carries the documents, never reads a library).
///
/// Each document of `copies` (the `lib/` JSON form, the part's closure as the caller resolved it) is checked as
/// `library.embed` checks it, at `/copies/i`; a copy the project holds with the same hash is reused (not inserted
/// again), one it holds with another hash (another version of the item) is refused: "update from library first"
/// (`library.update`, FR-PRT-014). The others are inserted. Then each device's part uses change by `mode`:
/// - `append` (the default): a part use `{part, qty}` is added after the existing ones;
/// - `replace_all`: the part uses become the one `{part, qty}`;
/// - `replace`: every part use of the copy `from` becomes `{part, qty}` (without `qty`: the replaced use's
///   quantity); the variant is dropped. A device without a part use of `from` is refused.
///
/// `qty` defaults to 1. Template functions are not matched onto existing functions (D8: part-first placement
/// does that). Rejected, with the pointer: an empty device list (`/devices`), a device listed twice or that does
/// not exist (`/devices/i`), `part` neither held nor in `copies` (`/part`), a copy listed twice or held at another
/// version (`/copies/i`), `replace` without `from` and `from` with another mode (`/from`), a device that does not
/// use `from` (`/devices/i`), `qty` ≤ 0 (`/qty`). Cost: `Interactive` below 50 devices, `Bulk` from 50
/// ([`ASSIGN_BULK_FROM`]). Outcome: the first device.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssignPart {
    /// The devices, at least one.
    #[schemars(with = "Vec<UuidS>")]
    pub devices: Vec<Id<Device>>,
    /// The part's copy ID, `v5(NS_LIBCOPY, library ‖ item)`: held by the project or in `copies`.
    #[schemars(with = "UuidS")]
    pub part: Id<Part>,
    /// The library documents to embed (`lib/` JSON form): the part and its closure; may be empty when the
    /// project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
    /// How the part uses change; default `append`.
    #[serde(default, skip_serializing_if = "is_append")]
    pub mode: AssignModeParam,
    /// The copy ID of the part to replace; with mode `replace` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub from: Option<Id<Part>>,
    /// The quantity in the part's order unit; default 1 (`replace`: the replaced use's quantity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub qty: Option<Dec3>,
}

fn is_append(m: &AssignModeParam) -> bool {
    *m == AssignModeParam::Append
}

/// Embeds the documents of a command's `copies` parameter (`/copies/i`): each is checked as `library.embed` checks
/// it; one the project holds with the same hash is skipped, one it holds with another hash refused ("update from
/// library first"). Returns the copy keys the transaction holds afterwards (inserted or reused).
pub(crate) fn embed_closure(
    command: &'static str,
    tx: &mut TxBuilder<'_>,
    copies: &[Value],
) -> Result<BTreeSet<ew_model::AnyKey>, CmdError> {
    let project = tx.project().id.as_uuid();
    let mut seen = BTreeSet::new();
    let mut new = Vec::new();
    for (i, value) in copies.iter().enumerate() {
        let at = format!("/copies/{i}");
        let (doc, l) = check_copy(command, &at, value, project)?;
        let kind = doc.kind().expect("an implemented kind");
        let key = (kind, l.id);
        if !seen.insert(key) {
            return Err(CmdError::invalid_params(command, at, "the copy is listed twice"));
        }
        if let Some(held) = tx.row(&key) {
            let held_hash = held_sha(&held);
            if held_hash != Some(l.claimed) {
                let version = held_version(&held).unwrap_or_default();
                return Err(CmdError::invalid_params(
                    command,
                    at,
                    format!(
                        "the project holds item {} at version {version} with another hash; update from library \
                         first (library.update)",
                        l.item
                    ),
                ));
            }
            continue;
        }
        new.push(doc);
    }
    for doc in new {
        match doc {
            LibDocument::Part(c) => {
                tx.insert(c)?;
            }
            LibDocument::Symbol(c) => {
                tx.insert(c)?;
            }
            LibDocument::Fndef(c) => {
                tx.insert(c)?;
            }
            LibDocument::Form(c) => {
                tx.insert(c)?;
            }
            LibDocument::Macro(c) => {
                tx.insert(c)?;
            }
            LibDocument::Manufacturer(c) => {
                tx.insert(c)?;
            }
            LibDocument::Unknown(_) => unreachable!("check_copy refuses an unknown kind"),
        }
    }
    Ok(seen)
}

/// The hash a held library copy row claims.
pub(crate) fn held_sha(row: &ew_model::AnyRow) -> Option<ew_schema::Sha256> {
    use ew_model::AnyRow;
    Some(match row {
        AnyRow::Part(c) => c.sha256,
        AnyRow::Symbol(c) => c.sha256,
        AnyRow::Fndef(c) => c.sha256,
        AnyRow::Form(c) => c.sha256,
        AnyRow::Macro(c) => c.sha256,
        AnyRow::Manufacturer(c) => c.sha256,
        _ => return None,
    })
}

/// The version a held library copy row carries.
pub(crate) fn held_version(row: &ew_model::AnyRow) -> Option<String> {
    use ew_model::AnyRow;
    Some(match row {
        AnyRow::Part(c) => c.version.clone(),
        AnyRow::Symbol(c) => c.version.clone(),
        AnyRow::Fndef(c) => c.version.clone(),
        AnyRow::Form(c) => c.version.clone(),
        AnyRow::Macro(c) => c.version.clone(),
        AnyRow::Manufacturer(c) => c.version.clone(),
        _ => return None,
    })
}

impl Command for AssignPart {
    const NAME: &'static str = "part.assign";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, _model: &ew_model::Model) -> Cost {
        if self.devices.len() >= ASSIGN_BULK_FROM {
            Cost::Bulk
        } else {
            Cost::Interactive
        }
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        if self.devices.is_empty() {
            return Err(CmdError::invalid_params(
                name,
                "/devices",
                "give at least one device",
            ));
        }
        let mut listed = BTreeSet::new();
        for (i, d) in self.devices.iter().enumerate() {
            if !listed.insert(*d) {
                return Err(CmdError::invalid_params(
                    name,
                    format!("/devices/{i}"),
                    "the device is listed twice",
                ));
            }
            if !tx.exists::<Device>(*d) {
                return Err(CmdError::invalid_params(
                    name,
                    format!("/devices/{i}"),
                    format!("no device {d}"),
                ));
            }
        }
        if let Some(qty) = self.qty {
            if qty <= Dec3::ZERO {
                return Err(CmdError::invalid_params(
                    name,
                    "/qty",
                    "the quantity must be greater than 0",
                ));
            }
        }
        match (self.mode, self.from) {
            (AssignModeParam::Replace, None) => {
                return Err(CmdError::invalid_params(
                    name,
                    "/from",
                    "mode `replace` needs `from`, the part to replace",
                ));
            }
            (AssignModeParam::Append | AssignModeParam::ReplaceAll, Some(_)) => {
                return Err(CmdError::invalid_params(
                    name,
                    "/from",
                    "`from` goes with mode `replace` only",
                ));
            }
            _ => {}
        }
        if let Some(from) = self.from {
            for (i, d) in self.devices.iter().enumerate() {
                let dev = tx.require::<Device>(*d)?;
                if !dev.parts.iter().any(|u| u.part.copy == from) {
                    return Err(CmdError::invalid_params(
                        name,
                        format!("/devices/{i}"),
                        format!("device {d} does not use part {from}"),
                    ));
                }
            }
        }
        embed_closure(name, tx, &self.copies)?;
        if !tx.exists::<LibCopy<Part>>(self.part) {
            return Err(CmdError::invalid_params(
                name,
                "/part",
                format!(
                    "part copy {} is neither held by the project nor in `copies`",
                    self.part
                ),
            ));
        }
        let qty = self.qty.unwrap_or(Dec3::ONE);
        let new_use = |qty: Dec3| PartUse {
            part: PartRef::new(self.part),
            qty,
            extra: ew_schema::Extra::new(),
        };
        for d in &self.devices {
            match self.mode {
                AssignModeParam::Append => tx.update::<Device>(*d, |dev| dev.parts.push(new_use(qty)))?,
                AssignModeParam::ReplaceAll => {
                    tx.update::<Device>(*d, |dev| dev.parts = vec![new_use(qty)])?
                }
                AssignModeParam::Replace => {
                    let from = self.from.expect("checked above");
                    tx.update::<Device>(*d, |dev| {
                        for u in &mut dev.parts {
                            if u.part.copy == from {
                                *u = new_use(self.qty.unwrap_or(u.qty));
                            }
                        }
                    })?;
                }
            }
        }
        Ok(Outcome::primary((Kind::Device, self.devices[0].cast())))
    }
}
