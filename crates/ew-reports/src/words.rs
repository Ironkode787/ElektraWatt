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

use ew_l10n::Text;
use ew_mltext::{LangTag, MlText};

/// `t` in the language a list prints for `lang` (`Catalogue::text_for`: the tag, else its primary language, else
/// en-GB; en-GB without a language).
pub fn text(t: &Text, lang: Option<&LangTag>) -> String {
    crate::catalogue().text_for(t, lang)
}

/// A finding's severity in words ("error", Dutch "fout"): the open-items list's Severity cell and the lines of the
/// package's Contents.txt (render it with [`text`]).
pub fn severity(s: ew_checks::Severity) -> Text {
    match s {
        ew_checks::Severity::Info => text!("cell-severity-info"),
        ew_checks::Severity::Warning => text!("cell-severity-warning"),
        ew_checks::Severity::Error => text!("cell-severity-error"),
    }
}

/// The name of the list kind `kind` (its word, `terminal_diagram`) by its key `report-kind-<kind>`, and with `-words`
/// the words it is also searched for by ("cable list" for the cable overview); `None` for a kind without them.
fn kind_text(kind: &str, words: bool) -> Option<Text> {
    let key = format!(
        "report-kind-{}{}",
        kind.replace('_', "-"),
        if words { "-words" } else { "" }
    );
    let (key, english) = crate::TEXTS.iter().find(|(k, _)| *k == key)?;
    Some(Text::checked_by_macro(key, english, Vec::new()))
}

/// The name of the list kind `kind` (its word) in the shipped language `tag` (`nl-NL`; en-GB where it has none), as
/// the window offers it; `None` for a kind without a name.
pub fn kind_name(kind: &str, tag: &str) -> Option<String> {
    kind_text(kind, false).map(|t| crate::catalogue().text_or_english(&t, tag))
}

/// The words the list kind `kind` is also searched for by, comma-separated, in the shipped language `tag`; `None` for
/// a kind without them.
pub fn kind_search_words(kind: &str, tag: &str) -> Option<String> {
    kind_text(kind, true).map(|t| crate::catalogue().text_or_english(&t, tag))
}

/// The name of the list kind `kind` in every language of this crate's text files, by tag (en-GB first); empty for a
/// kind without a name.
pub fn kind_names(kind: &str) -> Vec<(&'static str, String)> {
    kind_text(kind, false).map_or_else(Vec::new, |t| {
        let catalogue = crate::catalogue();
        catalogue
            .tags()
            .into_iter()
            .map(|tag| (tag, catalogue.text_or_english(&t, tag)))
            .collect()
    })
}

/// The text of the key `key` whose pattern selects on its arguments (a message of several lines, which `text!` does
/// not build), its arguments to be given by name ([`Text::with`]).
pub(crate) fn selecting(key: &'static str) -> Text {
    Text::checked_by_macro(key, "?", Vec::new())
}

/// `t` in every language of this crate's text files, each decimal point between two digits written as the list prints a
/// quantity in that language ([`crate::render::writes_comma`]: the project's separator in its own language, else the
/// language's): the words of a row that hold a measured value ("0.75 mm² BK", "from 0 to 48.2 mm").
pub(crate) fn in_languages_measured(q: &ew_derive::Q, t: &Text) -> MlText {
    let catalogue = crate::catalogue();
    MlText::from_langs(catalogue.tags().into_iter().filter_map(|tag| {
        let lang: LangTag = tag.parse().ok()?;
        let text = catalogue.text_or_english(t, tag);
        let text = if crate::render::writes_comma(q, Some(&lang)) {
            ew_l10n::render::decimal_with_comma(&text)
        } else {
            text
        };
        Some((lang, text))
    }))
}

/// `t` in every language of this crate's text files, as a row set carries a word of the program.
pub fn in_languages(t: &Text) -> MlText {
    crate::catalogue().in_languages(t)
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
