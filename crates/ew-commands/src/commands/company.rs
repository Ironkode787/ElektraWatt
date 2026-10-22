// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.company.set` (`SetCompany`, FT-336): the company of a basic project, the panel builder (doc 16 §9.2
//! step 1): its name as the `Party{role: panel_builder}` of the project row and the notice text that the title
//! block's notice cell prints (`project.notice`, doc 09 §2.6, decision log ID-40).

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
/// `{"logo": "sha256:…", "notice": <MlText>}` (the provisional source `title_block(page)` reads, doc 09 §2.6,
/// decision log ID-40). `project.company.set` writes `notice` and keeps every other member (the logo, FT-337).
pub const COMPANY_MEMBER: &str = "ew.company";

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
}

impl Command for SetCompany {
    const NAME: &'static str = "project.company.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.name.is_none() && self.notice.is_none() && self.personal_data.is_none() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "",
                "give at least one of `name`, `notice` and `personal_data`",
            ));
        }
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
        if let Some(notice) = &self.notice {
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
            if notice.is_empty() {
                company.remove("notice");
            } else {
                let value = serde_json::to_value(notice).map_err(|e| {
                    CmdError::internal(format!("project.company.set: cannot serialise the notice ({e})"))
                })?;
                company.insert("notice".to_owned(), value);
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
        Ok(Outcome::default())
    }
}
