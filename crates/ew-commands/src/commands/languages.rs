// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `project.languages.set` (`SetProjectLanguages`, FT-336): the project's languages and printed formats (doc 03
//! §5 `ProjectLanguages`, `Formatting`; doc 16 §9.2 step 8).

use ew_mltext::LangTag;
use ew_schema::{DateFormat, DecimalSeparator, ProjectLanguages, Set};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::LangTagS;
use crate::text::text;
use crate::tx::TxBuilder;

/// `project.languages.set` (`SetProjectLanguages`): sets the project's languages and its printed formats as one
/// value, the four fields of the wizard's step 8 (doc 16 §9.2).
///
/// - `source` and `translations` become `Project.languages` (`ProjectLanguages`); without `source` the project
///   has no languages member (texts are read in their neutral or first language);
/// - `date` and `decimal_separator` become `ProjectSettings.formatting.date` and `.decimal_separator`; an absent
///   value removes the member, so the pinned derivation profile's default applies. The formatting's other
///   members (`tag_case`, which decides when tags collide, and members of newer formats) are kept.
///
/// Every member is the new value, so an absent one is removed, not kept. Rejected: `translations` without
/// `source` or containing it, and the same translation twice (at `/translations`); a format of a newer version
/// (at its pointer). Only the project record changes. Bulk: the date format changes the dates of every title
/// block. Undoable as one step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetProjectLanguages {
    /// The language texts are authored in; absent: the project names no languages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<LangTagS>")]
    pub source: Option<LangTag>,
    /// The translation languages (a set; 1.0 content); needs `source`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<LangTagS>")]
    pub translations: Vec<LangTag>,
    /// The date format of printed dates; absent: the profile default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<DateFormat>,
    /// The decimal separator of printed numbers; absent: the profile default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimal_separator: Option<DecimalSeparator>,
}

impl Command for SetProjectLanguages {
    const NAME: &'static str = "project.languages.set";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if matches!(self.date, Some(DateFormat::Unknown(_))) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/date",
                text!("refusal-languages-date-format"),
            ));
        }
        if matches!(self.decimal_separator, Some(DecimalSeparator::Unknown(_))) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/decimal_separator",
                text!("refusal-languages-decimal-separator"),
            ));
        }
        let languages = match &self.source {
            None if !self.translations.is_empty() => {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    "/translations",
                    text!("refusal-languages-no-source"),
                ));
            }
            None => None,
            Some(source) => {
                if self.translations.contains(source) {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        "/translations",
                        text!(
                            "refusal-languages-source-as-translation",
                            source.as_str().to_string()
                        ),
                    ));
                }
                let translations: Set<LangTag> = self.translations.iter().cloned().collect();
                if translations.len() != self.translations.len() {
                    return Err(CmdError::invalid_params(
                        Self::NAME,
                        "/translations",
                        text!("refusal-languages-twice"),
                    ));
                }
                let mut next = tx
                    .project()
                    .languages
                    .clone()
                    .unwrap_or_else(|| ProjectLanguages {
                        source: source.clone(),
                        translations: Set::default(),
                        extra: ew_schema::Extra::new(),
                    });
                next.source = source.clone();
                next.translations = translations;
                Some(next)
            }
        };
        let mut project = (*tx.project()).clone();
        project.languages = languages;
        project.settings.formatting.date = self.date.clone();
        project.settings.formatting.decimal_separator = self.decimal_separator.clone();
        tx.set_project(project)?;
        Ok(Outcome::default())
    }
}
