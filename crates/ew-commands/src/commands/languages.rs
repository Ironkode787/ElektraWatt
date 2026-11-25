// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The member `languages` of `project.settings.set`: the project's languages and printed formats (doc 03 §5
//! `ProjectLanguages`, `Formatting`; doc 16 §9.2 step 8).

use ew_mltext::LangTag;
use ew_schema::{DateFormat, DecimalSeparator, ProjectLanguages, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::settings::{SetProjectSettings, member_value};
use crate::error::CmdError;
use crate::tx::TxBuilder;

/// The name of the member of `project.settings.set` that sets the languages and printed formats.
pub(crate) const LANGUAGES_MEMBER: &str = "languages";

/// The value of the member `languages` of `project.settings.set`: the project's languages and its printed formats as
/// one value, the four fields of the wizard's step 8 (doc 16 §9.2); `null` as `{}`.
///
/// - `source` and `translations` become `Project.languages` (`ProjectLanguages`); without `source` the project
///   has no languages member (texts are read in their neutral or first language);
/// - `date` and `decimal_separator` become `ProjectSettings.formatting.date` and `.decimal_separator`; an absent
///   value removes the member, so the pinned derivation profile's default applies. The formatting's other
///   members (`tag_case`, which decides when tags collide, and members of newer formats) are kept.
///
/// Every member is the new value, so an absent one is removed, not kept. Rejected: `translations` without
/// `source` or containing it, and the same translation twice (at `/value/translations`); a format of a newer version
/// (at its pointer below `/value`). Only the project record changes; the date format changes the dates of every
/// title block.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguagesValue {
    /// The language texts are authored in; absent: the project names no languages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<LangTag>,
    /// The translation languages (a set; 1.0 content); needs `source`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    translations: Vec<LangTag>,
    /// The date format of printed dates; absent: the profile default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    date: Option<DateFormat>,
    /// The decimal separator of printed numbers; absent: the profile default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    decimal_separator: Option<DecimalSeparator>,
}

/// Sets the languages and formats to `value`, the value of the member `languages` of `project.settings.set`
/// ([`LanguagesValue`]).
pub(crate) fn set(tx: &mut TxBuilder<'_>, value: &Value) -> Result<Outcome, CmdError> {
    let v: LanguagesValue = if value.is_null() {
        LanguagesValue::default()
    } else {
        member_value(LANGUAGES_MEMBER, value)?
    };
    let name = SetProjectSettings::NAME;
    if matches!(v.date, Some(DateFormat::Unknown(_))) {
        return Err(CmdError::invalid_params(
            name,
            "/value/date",
            text!("refusal-languages-date-format"),
        ));
    }
    if matches!(v.decimal_separator, Some(DecimalSeparator::Unknown(_))) {
        return Err(CmdError::invalid_params(
            name,
            "/value/decimal_separator",
            text!("refusal-languages-decimal-separator"),
        ));
    }
    let languages = match &v.source {
        None if !v.translations.is_empty() => {
            return Err(CmdError::invalid_params(
                name,
                "/value/translations",
                text!("refusal-languages-no-source"),
            ));
        }
        None => None,
        Some(source) => {
            if v.translations.contains(source) {
                return Err(CmdError::invalid_params(
                    name,
                    "/value/translations",
                    text!("refusal-languages-source-as-translation", source.as_str()),
                ));
            }
            let translations: Set<LangTag> = v.translations.iter().cloned().collect();
            if translations.len() != v.translations.len() {
                return Err(CmdError::invalid_params(
                    name,
                    "/value/translations",
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
    project.settings.formatting.date = v.date.clone();
    project.settings.formatting.decimal_separator = v.decimal_separator.clone();
    tx.set_project(project)?;
    Ok(Outcome::default())
}
