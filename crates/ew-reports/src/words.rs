// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The words of the lists by key (architecture §18): a column's heading, a word in a cell, the title of a box and the
//! note of a list that is not derived stand in `texts/<language>/lists.ftl` and are made with `text!`.
//!
//! A heading or a word reaches a row set as the multi-language text of its renderings in every language of those files
//! ([`in_languages`]), so that a list page prints it in the project's language, the CSV and the XLSX in English, and
//! the sorting and grouping read it in the project's source language, each by the rule it has for every other text. A
//! word that [`crate::render::render`] adds at the edge (yes and no, the text of a value to be decided, a missing value,
//! the piece) is rendered there in the output language ([`text`]).

use std::sync::OnceLock;

use ew_l10n::Text;
use ew_l10n::render::{DEFAULT_LANGUAGE, Language, OnError, language_for};
use ew_mltext::{LangTag, MlText};

/// Every language of this crate's text files with its bundle, in the order of the folder.
fn languages() -> &'static [(&'static str, Language)] {
    static LANGUAGES: OnceLock<Vec<(&'static str, Language)>> = OnceLock::new();
    LANGUAGES.get_or_init(|| {
        let mut tags: Vec<&'static str> = crate::SOURCES.iter().map(|(tag, _)| *tag).collect();
        tags.dedup();
        tags.into_iter()
            .map(|tag| {
                let sources = crate::SOURCES.iter().filter(|(l, _)| *l == tag).map(|(_, s)| *s);
                (tag, Language::new(tag, sources, &[], false))
            })
            .collect()
    })
}

/// `t` in the shipped language `tag`: its pattern there where it resolves, else en-GB's, else its English as the key
/// table holds it.
fn format(t: &Text, tag: &str) -> String {
    let lookup = |tag: &str| {
        let (_, language) = languages().iter().find(|(l, _)| *l == tag)?;
        language.text(t, OnError::Miss)
    };
    lookup(tag)
        .or_else(|| lookup(DEFAULT_LANGUAGE))
        .unwrap_or_else(|| t.to_string())
}

/// `t` in the language a list prints for `lang` ([`language_for`]: the tag, else its primary language, else en-GB;
/// en-GB without a language).
pub fn text(t: &Text, lang: Option<&LangTag>) -> String {
    let tags: Vec<&str> = languages().iter().map(|(tag, _)| *tag).collect();
    format(t, language_for(lang, &tags))
}

/// `t` in every language of this crate's text files, as a row set carries a word of the program.
pub fn in_languages(t: &Text) -> MlText {
    MlText::from_langs(
        languages()
            .iter()
            .filter_map(|(tag, _)| Some((tag.parse().ok()?, format(t, tag)))),
    )
}

#[cfg(test)]
mod tests {
    /// The entries are NFC, as a word of a row set is (`MlText` normalises it): a word rendered at the edge prints
    /// the bytes of its entry.
    #[test]
    fn every_entry_is_nfc() {
        for (tag, source) in crate::SOURCES {
            assert!(ew_mltext::is_nfc(source), "{tag}");
        }
    }
}
