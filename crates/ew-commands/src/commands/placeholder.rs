// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part.placeholder` (`AssignPlaceholder`, doc 07 §3.3): a part that belongs to a device and whose type is
//! not known yet.

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_schema::{
    Device, LibCopy, LibItem, LibRef, PLACEHOLDER_PART_NUMBER, Part, PartKind, PartSections, Sha256,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::assign::{ASSIGN_BULK_FROM, AssignModeParam, AssignPart, is_append};
use super::part_edit::hash;
use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// `part.placeholder` (`AssignPlaceholder`): gives devices a placeholder part, a part that belongs to them and whose
/// type is not known yet (doc 07 §3.3), in one undo step.
///
/// The placeholder is a project-local part copy numbered [`PLACEHOLDER_PART_NUMBER`] (`TBD`) with `description` as its
/// neutral description, no manufacturer (its reference names the project library and no item) and the kind `terminal`
/// when every device is a terminal, else `device`. A project-local placeholder of the same kind and description (white
/// space around it trimmed) is reused, so devices given one in several steps share one copy and one row of the parts
/// list; else a new copy is made (item minted, version 0.1.0, its hash taken over its item). Then the devices take it
/// as `part.assign` with `mode` and `from` gives a held part.
///
/// Rejected: an empty description (`/description`); every refusal of `part.assign` (`/devices`, `/from`). Cost and
/// outcome are `part.assign`'s.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssignPlaceholder {
    /// The devices, at least one.
    #[schemars(with = "Vec<UuidS>")]
    pub devices: Vec<Id<Device>>,
    /// What the part is, in the person's words ("Grey terminal"): the placeholder's description.
    pub description: String,
    /// How the part uses change, as `part.assign`'s; default `append`.
    #[serde(default, skip_serializing_if = "is_append")]
    pub mode: AssignModeParam,
    /// The copy ID of the part to replace; with mode `replace` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub from: Option<Id<Part>>,
}

impl AssignPlaceholder {
    /// The `part.assign` that gives the devices the part copy `part`.
    fn assign(&self, part: Id<Part>) -> AssignPart {
        AssignPart {
            devices: self.devices.clone(),
            part,
            copies: Vec::new(),
            mode: self.mode,
            from: self.from,
            qty: None,
            variant: None,
        }
    }
}

impl Command for AssignPlaceholder {
    const NAME: &'static str = "part.placeholder";

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
        let words = self.description.trim();
        if words.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/description",
                text!("refusal-placeholder-no-description"),
            ));
        }
        let description = MlText::neutral(words);
        let terminals = !self.devices.is_empty()
            && self
                .devices
                .iter()
                .all(|d| tx.get::<Device>(*d).is_some_and(|d| d.aspects.terminal.is_some()));
        let kind = if terminals {
            PartKind::Terminal
        } else {
            PartKind::Device
        };
        let project = tx.project().id.as_uuid();
        let held = tx
            .iter::<LibCopy<Part>>()
            .into_iter()
            .filter(|c| {
                c.library_id.as_uuid() == project
                    && c.item.is_placeholder()
                    && c.item.kind == kind
                    && c.item.description == description
            })
            .map(|c| c.id)
            .min();
        let part = match held {
            Some(id) => id,
            None => {
                let item: Id<LibItem> = tx.mint();
                let mut copy = LibCopy {
                    id: ew_ids::v5::lib_copy_id(project, item.as_uuid()),
                    v: 1,
                    library_id: Id::from_uuid(project),
                    item_id: item,
                    version: "0.1.0".to_owned(),
                    sha256: Sha256([0; 32]),
                    item: placeholder_part(project, kind, description),
                };
                copy.sha256 = hash(&copy)?;
                tx.insert(copy)?
            }
        };
        self.assign(part).run(tx)
    }
}

/// The item of a placeholder part: numbered `TBD`, described, of kind `kind`, without a manufacturer.
fn placeholder_part(project: ew_ids::Uuid, kind: PartKind, description: MlText) -> Part {
    Part {
        meta: Default::default(),
        kind,
        manufacturer: LibRef {
            library: Id::from_uuid(project),
            item: Id::from_uuid(ew_ids::Uuid::nil()),
            version_req: "*".to_owned(),
            extra: Default::default(),
        },
        part_number: Ident::from_input(PLACEHOLDER_PART_NUMBER).expect("an identifier"),
        type_designation: None,
        order_number: None,
        gtin: None,
        description,
        category: None,
        tech_data: Default::default(),
        free_props: Default::default(),
        unit: Default::default(),
        cp_pattern: Default::default(),
        function_template: Vec::new(),
        contacts: Vec::new(),
        mech: None,
        sections: PartSections::default(),
        accessories: Vec::new(),
        alternatives: Vec::new(),
        configured: None,
        variants: Default::default(),
        macros: Vec::new(),
        box_: None,
        extra: Default::default(),
    }
}
