// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.frames.set` (`SetFrameDefaults`, FT-336): the frame defaults of the project settings (doc 03 §5.1
//! `FrameDefaults`, FR-PAG-018; doc 16 §9.2 step 2).

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{FormDef, FormKind, LibCopy, PageType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::tx::TxBuilder;

/// The `ProjectSettings` member that holds the frame defaults (doc 03 §5.1), kept in `ProjectSettings.extra`
/// until `ew-schema` types it; `ew_derive::FrameSettings` and the page output read it.
pub const FRAMES_MEMBER: &str = "frames";

/// One `per_page_type` entry of the frame defaults.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameForPageType {
    /// The page type the entry applies to.
    pub page_type: PageType,
    /// The frame form copy for pages of that type.
    #[schemars(with = "UuidS")]
    pub form: Id<FormDef>,
}

/// `project.frames.set` (`SetFrameDefaults`): sets the frame defaults of the project (doc 03 §5.1 `FrameDefaults`,
/// FR-PAG-018): the frame form of a drawn page whose `form` is unset is the entry of its page type, else
/// `default`; a section divider takes the entry of `section_divider`; generated report pages keep the form of
/// their block.
///
/// The parameters are the whole new value of `default` and `per_page_type`, stored in the settings member
/// [`FRAMES_MEMBER`]: `per_page_type` always (an empty list when there is no entry, doc 03 §5.1), in page-type
/// order, and `default` when given. The member's other contents (members of a newer format) are kept (doc 05
/// §10.3); when there is no default, no entry and nothing else, the member is removed, so the built-in frames apply
/// again. Each form must be an embedded form copy (else [`CmdError::NotFound`]) of kind `frame`, `cover` or
/// `divider` (else invalid at its pointer). Rejected at `/per_page_type/i/page_type`: a page type named twice,
/// `report` (report pages take their block's form, EW-INV-012) and a type of a newer format. Rejected: a settings
/// member [`FRAMES_MEMBER`] that is not an object (a newer or damaged format; it is never overwritten).
/// The cell mapping of the title block is part of the form copies (`library.embed`); this command does not edit
/// forms. Only the project record changes. Bulk: one stored value changes the frame of every page that has no
/// form of its own. Undoable as one step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetFrameDefaults {
    /// The frame form of drawn pages whose type has no entry; absent: none (the built-in frame applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub default: Option<Id<FormDef>>,
    /// The frame form per page type, at most one entry per type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub per_page_type: Vec<FrameForPageType>,
}

/// One stored `per_page_type` entry (doc 03 §5.1: `{form, page_type}`).
#[derive(Serialize)]
struct StoredEntry {
    form: Id<FormDef>,
    page_type: String,
}

/// The snake_case spelling of a page type, `None` for a type of a newer format.
fn page_type_name(page_type: &PageType) -> Option<String> {
    if matches!(page_type, PageType::Unknown(_)) {
        return None;
    }
    serde_json::to_value(page_type)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
}

impl SetFrameDefaults {
    /// Checks that `form` is an embedded frame-like form copy; `path` is its parameter pointer.
    fn check_form(tx: &TxBuilder<'_>, form: Id<FormDef>, path: &str) -> Result<(), CmdError> {
        let copy = tx.require::<LibCopy<FormDef>>(form)?;
        match copy.item.kind {
            FormKind::Frame | FormKind::Cover | FormKind::Divider => Ok(()),
            _ => Err(CmdError::invalid_params(
                Self::NAME,
                path,
                "the form is not a frame (its kind must be `frame`, `cover` or `divider`)",
            )),
        }
    }
}

impl Command for SetFrameDefaults {
    const NAME: &'static str = "project.frames.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if let Some(form) = self.default {
            Self::check_form(tx, form, "/default")?;
        }
        let mut entries: Vec<(String, Id<FormDef>)> = Vec::with_capacity(self.per_page_type.len());
        let mut seen = BTreeSet::new();
        for (i, entry) in self.per_page_type.iter().enumerate() {
            let at = format!("/per_page_type/{i}/page_type");
            let Some(name) = page_type_name(&entry.page_type) else {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    "not a page type of this version",
                ));
            };
            if entry.page_type == PageType::Report {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    "report pages take the form of their report block (EW-INV-012); give no entry for `report`",
                ));
            }
            if !seen.insert(name.clone()) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    format!("the page type `{name}` has an entry already; give one entry per page type"),
                ));
            }
            Self::check_form(tx, entry.form, &format!("/per_page_type/{i}/form"))?;
            entries.push((name, entry.form));
        }
        entries.sort();
        let mut project = (*tx.project()).clone();
        let mut frames = match project.settings.extra.get(FRAMES_MEMBER) {
            None => Map::new(),
            Some(Value::Object(map)) => map.clone(),
            Some(_) => {
                return tx.reject(format!(
                    "the settings member `{FRAMES_MEMBER}` is not an object (a newer or damaged format); it is \
                     kept as it is"
                ));
            }
        };
        let serialise = |e: serde_json::Error| {
            CmdError::internal(format!(
                "project.frames.set: cannot serialise the frame defaults ({e})"
            ))
        };
        match self.default {
            Some(form) => {
                frames.insert(
                    "default".to_owned(),
                    serde_json::to_value(form).map_err(serialise)?,
                );
            }
            None => {
                frames.remove("default");
            }
        }
        let list: Vec<StoredEntry> = entries
            .into_iter()
            .map(|(page_type, form)| StoredEntry { form, page_type })
            .collect();
        let empty = self.default.is_none() && list.is_empty();
        frames.insert(
            "per_page_type".to_owned(),
            serde_json::to_value(&list).map_err(serialise)?,
        );
        if empty && frames.len() == 1 {
            project.settings.extra.remove(FRAMES_MEMBER);
        } else {
            project
                .settings
                .extra
                .insert(FRAMES_MEMBER.to_owned(), Value::Object(frames));
        }
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}
