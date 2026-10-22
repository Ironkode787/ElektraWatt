// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.company.set` (`SetCompany`, FT-336): the company of a basic project, the panel builder (doc 16 §9.2
//! step 1): its name as the `Party{role: panel_builder}` of the project row and the notice text that the title
//! block's notice cell prints (`project.notice`, doc 09 §2.6, decision log ID-40), and the company logo as a project
//! asset (FT-337, doc 05 §13.3).

use ew_mltext::MlText;
use ew_schema::{Extra, Party, PartyRole};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::MlTextS;
use crate::tx::TxBuilder;

/// The project-row member that holds the company data of a basic project until `Project` types it: an object
/// `{"logo": <AssetRef>, "notice": <MlText>}` (the provisional source `title_block(page)` reads, doc 09 §2.6,
/// decision log ID-40; an earlier `logo` form, a bare `"sha256:…"`, is still read). `project.company.set` writes
/// `notice` and `logo` and keeps every other member.
pub const COMPANY_MEMBER: &str = ew_schema::COMPANY_MEMBER;

/// `project.company.set` (`SetCompany`): sets the company of the project, the panel builder (doc 16 §9.2 step 1).
///
/// - `name`: the name of the first party with role `panel_builder` (FR-PRJ-003; the title-block path
///   `party.panel_builder.name`). Without such a party, a non-empty name appends one; an empty name then changes
///   nothing. An empty name on an existing party removes its name and keeps the party (its address and other
///   members stay).
/// - `notice`: the notice text of the title block (`project.notice`), stored as `notice` of the project-row
///   member [`COMPANY_MEMBER`]; an empty text removes it (and the member when nothing else is left in it).
/// - `personal_data`: whether the panel-builder party is personal data (FR-PRJ-024, NFR-PRIV-06: "omit personal
///   data" exports leave it out). A party this command appends is marked `true` unless the parameter says
///   otherwise; without a party (and without a name that creates one) the parameter is refused.
///
/// - `logo`: the company logo, the bytes of an image file in base64 (RFC 4648 §4: standard alphabet, padded, no line
///   breaks): SVG, or PNG or JPEG (which print rasterised, D-07: the outcome says so in a notice). The bytes join the
///   project's asset store (`assets/<sha256>.<ext>`, doc 05 §13.3) and `logo` of [`COMPANY_MEMBER`] names them by
///   their reference `{sha256, format, size}`; the title block's logo cell (`project.logo`) draws it. Invalid: text
///   that is not base64, and bytes that are no SVG, PNG or JPEG file or exceed the size limits (raster 16 MiB, SVG
///   8 MiB and 100,000 elements). An empty text removes the logo; the asset stays in the store while the session
///   lasts (undo brings the reference back) and a save writes only the assets that records name.
///
/// An absent member leaves its value as it is; at least one member must be given (else invalid at the root).
/// Rejected: a project row whose [`COMPANY_MEMBER`] is not an object (a newer or damaged format; the command never
/// overwrites it). Only the project record changes. Bulk: the name and the notice are printed in every title
/// block, so front-ends preview the change. Undoable as one step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetCompany {
    /// The company name (the panel-builder party's name); empty removes it; absent keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub name: Option<MlText>,
    /// The title-block notice text (`project.notice`); empty removes it; absent keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub notice: Option<MlText>,
    /// Whether the panel-builder party is personal data (FR-PRJ-024); absent keeps it (`true` for a new party).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personal_data: Option<bool>,
    /// The logo image file (SVG, PNG or JPEG) in base64; empty removes it; absent keeps it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

impl Command for SetCompany {
    const NAME: &'static str = "project.company.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.name.is_none() && self.notice.is_none() && self.personal_data.is_none() && self.logo.is_none()
        {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "",
                "give at least one of `name`, `notice`, `personal_data` and `logo`",
            ));
        }
        let mut outcome = Outcome::default();
        // The logo first: its parameter errors come before any reading of the project.
        let logo = match self.logo.as_deref() {
            None => None,
            Some("") => Some(None),
            Some(text) => {
                let bytes = ew_schema::base64_decode(text)
                    .map_err(|e| CmdError::invalid_params(Self::NAME, "/logo", e.to_string()))?;
                let reference = tx
                    .put_asset(bytes)
                    .map_err(|e| CmdError::invalid_params(Self::NAME, "/logo", e.to_string()))?;
                if reference.format.is_raster() {
                    outcome.notices.push(format!(
                        "the logo is a {} raster image: it prints rasterised (D-07); an SVG logo prints as vectors",
                        reference.format.ext().to_uppercase()
                    ));
                }
                let value = serde_json::to_value(reference).map_err(|e| {
                    CmdError::internal(format!("project.company.set: cannot serialise the logo ({e})"))
                })?;
                Some(Some(value))
            }
        };
        let mut project = (*tx.project()).clone();
        let mut index = project
            .parties
            .iter()
            .position(|p| p.role == PartyRole::PanelBuilder);
        if let Some(name) = &self.name {
            match index {
                Some(i) => project.parties[i].name = name.clone(),
                None if !name.is_empty() => {
                    project.parties.push(Party {
                        role: PartyRole::PanelBuilder,
                        name: name.clone(),
                        address: None,
                        personal_data: self.personal_data.unwrap_or(true),
                        extra: Extra::new(),
                    });
                    index = Some(project.parties.len() - 1);
                }
                None => {}
            }
        }
        if let Some(flag) = self.personal_data {
            let Some(i) = index else {
                return tx.reject(
                    "the project has no panel-builder party to mark; give the company `name` to create it",
                );
            };
            project.parties[i].personal_data = flag;
        }
        let notice = match &self.notice {
            None => None,
            Some(n) if n.is_empty() => Some(None),
            Some(n) => Some(Some(serde_json::to_value(n).map_err(|e| {
                CmdError::internal(format!("project.company.set: cannot serialise the notice ({e})"))
            })?)),
        };
        if notice.is_some() || logo.is_some() {
            let mut company = match project.extra.get(COMPANY_MEMBER) {
                None => serde_json::Map::new(),
                Some(Value::Object(map)) => map.clone(),
                Some(_) => {
                    return tx.reject(format!(
                        "the project row's `{COMPANY_MEMBER}` member is not an object (a newer or damaged \
                         format); it is kept as it is"
                    ));
                }
            };
            for (member, value) in [("notice", notice), ("logo", logo)] {
                match value {
                    None => {}
                    Some(None) => {
                        company.remove(member);
                    }
                    Some(Some(v)) => {
                        company.insert(member.to_owned(), v);
                    }
                }
            }
            if company.is_empty() {
                project.extra.remove(COMPANY_MEMBER);
            } else {
                project
                    .extra
                    .insert(COMPANY_MEMBER.to_owned(), Value::Object(company));
            }
        }
        tx.set_project(project)?;
        Ok(outcome)
    }
}
