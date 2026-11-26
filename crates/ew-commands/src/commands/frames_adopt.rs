// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `frames.adopt` (`AdoptFrames`): the frames of a template taken by a project made before it changed (doc 09 §3.9,
//! Use the frames of a template…).

use ew_ids::Id;
use ew_mltext::MlText;
use ew_schema::{FormDef, LibCopy, LibDocument, ReportBlock, UserPropDef, UserPropValue};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::company::SetCompany;
use crate::commands::settings::SetProjectSettings;
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::{MlTextS, UuidS};
use crate::tx::TxBuilder;

/// `frames.adopt` (`AdoptFrames`): takes a template's frames into the project in one undo step (doc 09 §3.9). Bulk
/// (previewed): every page whose frame changes, the fields added and the cross-references that print differently are
/// listed before it is applied.
///
/// - `forms`: the template's frame and title-block copies (`lib/` JSON form, kind form), embedded; a copy the project
///   holds with another content is replaced by the template's, and the forms it replaces stay until purged.
/// - `frames`: the template's frame defaults, set as `project.settings.set`'s member `frames` sets them; a page with a
///   frame of its own keeps it.
/// - `lists_form`: the frame the template's lists print on: every page block's form.
/// - `fields` and `values`: the template's fields of its own, each one the project does not define added, and its value
///   of the whole project where this project has none.
/// - `notice` and `logo`: the template's company notice and logo (base64), as `project.company.set` sets them.
///
/// Rejected, with the pointer: `/forms/<i>` a document that is not a form copy (as `library.embed` checks it);
/// `/frames` and `/lists_form` naming a form that neither the project holds nor `forms` carries; `/notice` and `/logo`
/// as `project.company.set` refuses them. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdoptFrames {
    /// The template's frame and title-block copies (`lib/` JSON form).
    pub forms: Vec<Value>,
    /// The template's frame defaults (`{default, per_page_type}`).
    pub frames: Value,
    /// The frame the template's lists print on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub lists_form: Option<Id<FormDef>>,
    /// The template's fields of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<UserPropDef>,
    /// The template's values of its fields of the whole project.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<UserPropValue>,
    /// The template's company notice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub notice: Option<MlText>,
    /// The template's company logo, an image file in base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

impl Command for AdoptFrames {
    const NAME: &'static str = "frames.adopt";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let project = tx.project().id.as_uuid();
        for (i, value) in self.forms.iter().enumerate() {
            let at = format!("/forms/{i}");
            let (doc, _) = crate::commands::library::check_copy(Self::NAME, &at, value, project)?;
            let LibDocument::Form(copy) = doc else {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    text!("refusal-report-block-not-form"),
                ));
            };
            match tx.get::<LibCopy<FormDef>>(copy.id) {
                Some(held) if held.sha256 == copy.sha256 => {}
                Some(_) => tx.put(copy),
                None => {
                    tx.insert(copy)?;
                }
            }
        }
        SetProjectSettings {
            member: crate::commands::FRAMES_MEMBER.to_owned(),
            value: self.frames.clone(),
        }
        .run(tx)?;
        if let Some(form) = self.lists_form {
            tx.require::<LibCopy<FormDef>>(form).map_err(|_| {
                CmdError::invalid_params(Self::NAME, "/lists_form", text!("refusal-frames-adopt-lists"))
            })?;
            for block in tx.iter::<ReportBlock>() {
                if block.embedded.is_none() && block.form != form {
                    tx.update(block.id, |b: &mut ReportBlock| b.form = form)?;
                }
            }
        }
        let mut record = (*tx.project()).clone();
        for def in &self.fields {
            if !record.user_prop_defs.iter().any(|d| d.key == def.key) {
                record.user_prop_defs.push(def.clone());
            }
        }
        for value in &self.values {
            if !record.user_props.iter().any(|v| v.key == value.key) {
                record.user_props.push(value.clone());
            }
        }
        if record != *tx.project() {
            tx.set_project(record)?;
        }
        if self.notice.is_some() || self.logo.is_some() {
            SetCompany {
                name: None,
                notice: self.notice.clone(),
                personal_data: None,
                logo: self.logo.clone(),
            }
            .run(tx)?;
        }
        Ok(Outcome::default())
    }
}
