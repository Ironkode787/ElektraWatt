// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The labelled entity list of a preview (WP-S2.09 part 3, §4.2): the Preview tab's Entities list and the history
//! panel's Show changes read the records a transaction changes as [`PreviewEntity`] lines, each with the label the
//! property grid prints for the record (a device's tag, a page's folio and title, a node's path, a name).
//!
//! **Order** (§9 item 4): change (`added`, `changed`, `removed`), then the kind word, then the label in natural
//! order, then the ID; no hash-map order and no time.

use std::cmp::Ordering;

use ew_commands::{ChangeKind, EntityChange, ModelDiff, Preview};
use ew_derive::Q;
use ew_derive::ew_ids::Uuid;
use ew_derive::ew_mltext::natural_cmp;

use super::properties::record_title;
use crate::Session;

/// One record a preview (or a history entry) changes, with its printed label.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PreviewEntity {
    /// `added`, `removed` or `changed`.
    pub change: &'static str,
    /// The record kind's word (`device`, `page`).
    pub kind: String,
    /// The record's ID.
    pub id: Uuid,
    /// The printed label by the property grid's label rule: the candidate's for an added record, the current
    /// model's otherwise.
    pub label: String,
    /// The changed top-level members, sorted (empty for an added or removed record).
    pub fields: Vec<String>,
}

/// The word of a change kind.
fn change_word(change: ChangeKind) -> &'static str {
    match change {
        ChangeKind::Added => "added",
        ChangeKind::Changed => "changed",
        ChangeKind::Removed => "removed",
    }
}

/// The rank of a change word in the list's order: added, changed, removed.
fn change_rank(word: &str) -> u8 {
    match word {
        "added" => 0,
        "changed" => 1,
        _ => 2,
    }
}

/// The top-level member of a changed path (`tag` of `tag.counter`, `parts` of `parts[0].qty`).
fn top_member(path: &str) -> &str {
    let end = path.find(['.', '[']).unwrap_or(path.len());
    &path[..end]
}

/// The source language of the project of `q` (for a multi-language label).
fn source_language(q: &Q) -> Option<String> {
    q.project().languages.as_ref().map(|l| l.source.to_string())
}

/// One line of `change`: its label from `current` (or `candidate` for an added record).
fn entity(change: &EntityChange, current: &Q, candidate: Option<&Q>) -> PreviewEntity {
    let (kind, id) = change.key();
    let id = id.as_uuid();
    let q = match (change.change, candidate) {
        (ChangeKind::Added, Some(c)) => c,
        _ => current,
    };
    let label = record_title(q, id, source_language(q).as_deref()).unwrap_or_default();
    let mut fields: Vec<String> = change.fields.iter().map(|f| top_member(f).to_owned()).collect();
    fields.sort();
    fields.dedup();
    PreviewEntity {
        change: change_word(change.change),
        kind: kind.name().to_owned(),
        id,
        label,
        fields,
    }
}

/// The lines of `diff` in §4.2's order: labels from `current`, an added record's from `candidate` (else from
/// `current`, where it has none and its label is empty).
pub(super) fn entities_of(diff: &ModelDiff, current: &Q, candidate: Option<&Q>) -> Vec<PreviewEntity> {
    let mut out: Vec<PreviewEntity> = diff
        .changes
        .iter()
        .map(|c| entity(c, current, candidate))
        .collect();
    out.sort_by(entity_order);
    out
}

/// §4.2's order: change, kind, label in natural order (then bytes), ID.
fn entity_order(a: &PreviewEntity, b: &PreviewEntity) -> Ordering {
    change_rank(a.change)
        .cmp(&change_rank(b.change))
        .then_with(|| a.kind.cmp(&b.kind))
        .then_with(|| natural_cmp(&a.label, &b.label))
        .then_with(|| a.label.cmp(&b.label))
        .then_with(|| a.id.cmp(&b.id))
}

impl Session {
    /// The records `preview` changes (WP-S2.09 §4.2), one per `EntityChange` of [`Preview::diff`], labelled by the
    /// property grid's rule (an added record by the candidate model's), in the order change (`added`, `changed`,
    /// `removed`), kind, label in natural order, ID. One call per card (§9 item 5).
    pub fn preview_entities(&self, preview: &Preview) -> Vec<PreviewEntity> {
        let current = self.query();
        let candidate = preview
            .diff()
            .changes
            .iter()
            .any(|c| c.change == ChangeKind::Added)
            .then(|| self.query_over(preview.candidate()));
        entities_of(preview.diff(), &current, candidate.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_top_member_of_a_path() {
        assert_eq!(top_member("tag.counter"), "tag");
        assert_eq!(top_member("parts[0].qty"), "parts");
        assert_eq!(top_member("folio"), "folio");
    }

    #[test]
    fn the_change_order_is_added_changed_removed() {
        let line = |change, label: &str| PreviewEntity {
            change,
            kind: "device".into(),
            id: Uuid::nil(),
            label: label.into(),
            fields: Vec::new(),
        };
        let mut v = [
            line("removed", "-1"),
            line("changed", "-10"),
            line("changed", "-9"),
            line("added", "-2"),
        ];
        v.sort_by(entity_order);
        let got: Vec<(&str, &str)> = v.iter().map(|e| (e.change, e.label.as_str())).collect();
        assert_eq!(
            got,
            [
                ("added", "-2"),
                ("changed", "-9"),
                ("changed", "-10"),
                ("removed", "-1")
            ]
        );
    }
}
