// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The find pane's model (WP-S2.05 part 2, spec §7.3 to §7.6): the query being typed, the last results with the
//! revision they were made at, the result cursor and the pane's note.
//!
//! Only the shell (`ShellModel`'s `find.*` actions) calls the `Session`: `find.run` makes exactly one
//! `Session::find` call and typing makes none (§9 item 4); `find.replace_all` turns the results'
//! `replace_command` into one `fields.set_many` that always goes through the inline preview card (FR-UI-020,
//! FR-DEV-032: "one previewed command"). A result list made at another revision than the current one is stale.

use std::sync::Arc;

pub use ew_api::output::{FindField, FindHit, FindQuery, FindResults};

use crate::strings;

/// The five fields in the order the pane lists them.
pub const FIELDS: [FindField; 5] = [
    FindField::FunctionText,
    FindField::Tag,
    FindField::Property,
    FindField::Text,
    FindField::PartNumber,
];

/// The word of a field in actions, the transcript and the describe text (`function_text`).
pub fn field_word(f: FindField) -> &'static str {
    match f {
        FindField::FunctionText => "function_text",
        FindField::Tag => "tag",
        FindField::Property => "property",
        FindField::Text => "text",
        FindField::PartNumber => "part_number",
    }
}

/// The field of a word.
pub fn field_of_word(word: &str) -> Option<FindField> {
    FIELDS.into_iter().find(|f| field_word(*f) == word)
}

/// The label key of a field's checkbox.
pub fn field_label_key(f: FindField) -> &'static str {
    match f {
        FindField::FunctionText => "find-field-function-text",
        FindField::Tag => "find-field-tag",
        FindField::Property => "find-field-property",
        FindField::Text => "find-field-text",
        FindField::PartNumber => "find-field-part-number",
    }
}

/// The results of the last search with what they were made for.
#[derive(Clone, Debug)]
pub(crate) struct Ran {
    /// The project session they belong to.
    pub epoch: u64,
    /// The revision they were made at.
    pub rev: u64,
    /// The query that made them.
    pub query: FindQuery,
    /// The results.
    pub results: Arc<FindResults>,
}

/// The find pane's state in the shell.
#[derive(Clone, Debug)]
pub struct FindState {
    /// Find and replace (Ctrl+H) rather than find (Ctrl+F): the replacement field is shown.
    pub(crate) replace_mode: bool,
    pub(crate) text: String,
    pub(crate) replacement: String,
    pub(crate) regex: bool,
    pub(crate) match_case: bool,
    pub(crate) whole_word: bool,
    pub(crate) in_selection: bool,
    /// The fields checked, in [`FIELDS`] order.
    pub(crate) fields: Vec<FindField>,
    /// The folio labels of the filter.
    pub(crate) folios: Vec<String>,
    pub(crate) ran: Option<Ran>,
    /// The result cursor (0-based).
    pub(crate) row: Option<usize>,
    /// The pane's note: "Replaced 20; run again to refresh", "Preview updated: 2 fewer results".
    pub(crate) note: Option<String>,
}

impl Default for FindState {
    fn default() -> FindState {
        FindState {
            replace_mode: false,
            text: String::new(),
            replacement: String::new(),
            regex: false,
            match_case: false,
            whole_word: false,
            in_selection: false,
            fields: FIELDS.to_vec(),
            folios: Vec::new(),
            ran: None,
            row: None,
            note: None,
        }
    }
}

impl FindState {
    /// Whether the replacement field is shown (find and replace).
    pub fn replace_mode(&self) -> bool {
        self.replace_mode
    }

    /// The search text as typed.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The replacement as typed.
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// The toggles: (regex, match case, whole word, in selection).
    pub fn options(&self) -> (bool, bool, bool, bool) {
        (self.regex, self.match_case, self.whole_word, self.in_selection)
    }

    /// The fields checked.
    pub fn fields(&self) -> &[FindField] {
        &self.fields
    }

    /// The folio filter.
    pub fn folios(&self) -> &[String] {
        &self.folios
    }

    /// The last results, if any.
    pub fn results(&self) -> Option<&FindResults> {
        self.ran.as_ref().map(|r| r.results.as_ref())
    }

    /// The query of the last results.
    pub fn last_query(&self) -> Option<&FindQuery> {
        self.ran.as_ref().map(|r| &r.query)
    }

    /// The result cursor (0-based) while it names a hit.
    pub fn row(&self) -> Option<usize> {
        let n = self.results().map_or(0, |r| r.hits.len());
        self.row.filter(|r| *r < n)
    }

    /// The pane's note.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Whether the results were made at another revision than `rev` (the stale badge).
    pub fn stale(&self, rev: Option<u64>) -> bool {
        match (&self.ran, rev) {
            (Some(r), Some(rev)) => r.rev != rev,
            _ => false,
        }
    }
}

/// The text of a hit's field label (`Function text`).
pub fn hit_label(hit: &FindHit) -> String {
    strings::text(&hit.label_key).to_owned()
}

/// `before` with each matched range marked as `[[…]]` (the window marks them bold and underlined; the describe text
/// and the accessible names use the brackets, never colour alone).
pub fn marked(hit: &FindHit) -> String {
    let mut out = String::new();
    let mut at = 0;
    for (s, e) in &hit.ranges {
        if *s < at || *e > hit.before.len() {
            continue;
        }
        out.push_str(&hit.before[at..*s]);
        out.push_str("[[");
        out.push_str(&hit.before[*s..*e]);
        out.push_str("]]");
        at = *e;
    }
    out.push_str(&hit.before[at..]);
    out
}

/// "Preview updated: 2 fewer results" (or more, or the same count) after a stale replace card was rebuilt.
pub fn updated_note(before: usize, after: usize) -> String {
    match after.cmp(&before) {
        std::cmp::Ordering::Less => strings::format("find-updated-fewer", &[&(before - after).to_string()]),
        std::cmp::Ordering::Greater => strings::format("find-updated-more", &[&(after - before).to_string()]),
        std::cmp::Ordering::Equal => strings::text("find-updated-same").to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_round_trip() {
        for f in FIELDS {
            assert_eq!(field_of_word(field_word(f)), Some(f));
        }
        assert_eq!(field_of_word("nope"), None);
    }

    #[test]
    fn matches_are_marked_in_brackets() {
        let hit = FindHit {
            record: Default::default(),
            kind: "device".into(),
            row_path: "function_text".into(),
            label_key: "find-field-function-text".into(),
            subject: "-1".into(),
            folio: None,
            cell: None,
            before: "Spare spare".into(),
            ranges: vec![(0, 5), (6, 11)],
            after: None,
            replaceable: Ok(()),
        };
        assert_eq!(marked(&hit), "[[Spare]] [[spare]]");
        assert_eq!(updated_note(20, 18), "Preview updated: 2 fewer results");
        assert_eq!(updated_note(2, 3), "Preview updated: 1 more results");
        assert_eq!(updated_note(2, 2), "Preview updated: the same results");
    }
}
