// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The member `frames` of `project.settings.set`: the frame defaults of the project settings (doc 03 §5.1
//! `FrameDefaults`, FR-PAG-018; doc 16 §9.2 step 2).

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_schema::{FormDef, FormKind, LibCopy, PageType};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, Outcome};
use crate::commands::settings::{SetProjectSettings, member_value};
use crate::error::CmdError;
use crate::tx::TxBuilder;

/// The `ProjectSettings` member that holds the frame defaults (doc 03 §5.1), kept in `ProjectSettings.extra`
/// until `ew-schema` types it; `ew_derive::FrameSettings` and the page output read it.
pub const FRAMES_MEMBER: &str = "frames";

/// One `per_page_type` entry of the frame defaults.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameForPageType {
    /// The page type the entry applies to.
    pub page_type: PageType,
    /// The frame form copy for pages of that type.
    pub form: Id<FormDef>,
}

/// The value of the member `frames` of `project.settings.set`: the frame defaults of the project (doc 03 §5.1 `FrameDefaults`,
/// FR-PAG-018): the frame form of a drawn page whose `form` is unset is the entry of its page type, else
/// `default`; a section divider takes the entry of `section_divider`; generated report pages keep the form of
/// their block.
///
/// The value is the whole new value of `default` and `per_page_type` (`null` as `{}`), stored in the settings member
/// [`FRAMES_MEMBER`]: `per_page_type` always (an empty list when there is no entry, doc 03 §5.1), in page-type
/// order, and `default` when given. The member's other contents (members of a newer format) are kept (doc 05
/// §10.3); when there is no default, no entry and nothing else, the member is removed, so the built-in frames apply
/// again. Each form must be an embedded form copy (else [`CmdError::NotFound`]) of kind `frame`, `cover` or
/// `divider` (else invalid at its pointer below `/value`). Rejected at `/value/per_page_type/i/page_type`: a page type named twice,
/// `report` (report pages take their block's form, EW-INV-012) and a type of a newer format. Rejected: a settings
/// member [`FRAMES_MEMBER`] that is not an object (a newer or damaged format; it is never overwritten).
/// The cell mapping of the title block is part of the form copies (`library.embed`); this command does not edit
/// forms. Only the project record changes: one stored value changes the frame of every page that has no form of its
/// own.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FramesValue {
    /// The frame form of drawn pages whose type has no entry; absent: none (the built-in frame applies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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

/// Checks that `form` is an embedded frame-like form copy; `path` is its parameter pointer.
fn check_form(tx: &TxBuilder<'_>, form: Id<FormDef>, path: &str) -> Result<(), CmdError> {
    let copy = tx.require::<LibCopy<FormDef>>(form)?;
    match copy.item.kind {
        FormKind::Frame | FormKind::Cover | FormKind::Divider => Ok(()),
        _ => Err(CmdError::invalid_params(
            SetProjectSettings::NAME,
            path,
            text!("refusal-frames-not-frame"),
        )),
    }
}

/// Sets the frame defaults to `value`, the value of the member `frames` of `project.settings.set` ([`FramesValue`]).
pub(crate) fn set(tx: &mut TxBuilder<'_>, value: &Value) -> Result<Outcome, CmdError> {
    let v: FramesValue = if value.is_null() {
        FramesValue::default()
    } else {
        member_value(FRAMES_MEMBER, value)?
    };
    if let Some(form) = v.default {
        check_form(tx, form, "/value/default")?;
    }
    let mut entries: Vec<(String, Id<FormDef>)> = Vec::with_capacity(v.per_page_type.len());
    let mut seen = BTreeSet::new();
    for (i, entry) in v.per_page_type.iter().enumerate() {
        let at = format!("/value/per_page_type/{i}/page_type");
        let Some(name) = page_type_name(&entry.page_type) else {
            return Err(CmdError::invalid_params(
                SetProjectSettings::NAME,
                at,
                text!("refusal-frames-unknown-page-type"),
            ));
        };
        if entry.page_type == PageType::Report {
            return Err(CmdError::invalid_params(
                SetProjectSettings::NAME,
                at,
                text!("refusal-frames-report"),
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(CmdError::invalid_params(
                SetProjectSettings::NAME,
                at,
                text!("refusal-frames-page-type-twice", name.to_string()),
            ));
        }
        check_form(tx, entry.form, &format!("/value/per_page_type/{i}/form"))?;
        entries.push((name, entry.form));
    }
    entries.sort();
    let mut project = (*tx.project()).clone();
    let mut frames = match project.settings.extra.get(FRAMES_MEMBER) {
        None => Map::new(),
        Some(Value::Object(map)) => map.clone(),
        Some(_) => {
            return tx.reject(text!("refusal-settings-member-not-object", FRAMES_MEMBER));
        }
    };
    let serialise = |e: serde_json::Error| {
        CmdError::internal(format!(
            "project.settings.set: cannot serialise the frame defaults ({e})"
        ))
    };
    match v.default {
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
    let empty = v.default.is_none() && list.is_empty();
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
