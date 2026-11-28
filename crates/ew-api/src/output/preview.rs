// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The labelled entity list of a preview: the Preview tab's Entities list and the history
//! panel's Show changes read the records a transaction changes as [`PreviewEntity`] lines, each with the label the
//! property grid prints for the record (a device's tag, a page's folio and title, a node's path, a name).
//!
//! **Order**: change (`added`, `changed`, `removed`), then the kind word, then the label in natural
//! order, then the ID; no hash-map order and no time.

use std::cmp::Ordering;

use ew_commands::{ChangeKind, EntityChange, ModelDiff, Preview};
use ew_derive::{PageOrder, PageRef, Q};
use ew_ids::Uuid;
use ew_mltext::natural_cmp;

use super::properties::{ml_text, record_title};
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
    /// The whole line in words where the record is said by what it does rather than by its kind: a wire's way
    /// ("101.4: a way through duct 1, 595 mm", "103.2: its way is removed, the wire is unfinished"); `None` for the
    /// line of change, kind and label.
    pub words: Option<crate::Text>,
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
    let source = source_language(q);
    // A library copy is named too (a part by its number, a manufacturer by its name), and
    // a label that only repeats the kind word ("bond") is left out, so the line never reads "added bond bond".
    let label = record_title(q, id, source.as_deref())
        .or_else(|| library_name(q, kind, id, source.as_deref()))
        .filter(|l| l != kind.name())
        .unwrap_or_default();
    let mut fields: Vec<String> = change.fields.iter().map(|f| top_member(f).to_owned()).collect();
    fields.sort();
    fields.dedup();
    let words = (kind == ew_schema::Kind::CommittedRoute)
        .then(|| route_words(change.change, id, current, candidate))
        .flatten();
    PreviewEntity {
        change: change_word(change.change),
        kind: kind.name().to_owned(),
        id,
        label,
        fields,
        words,
    }
}

/// A wire's way (`committed_route` `id`) in words, by its wire: an added one "101.4: a way through duct 1, 595 mm", a
/// changed one its new way with the old length "… 610 mm (was 595 mm)", both read from `candidate`, the result; a
/// removed one "103.2: its way is removed, the wire is unfinished" where the result has no way for the wire, else "…:
/// its old way gives way to its new one".
fn route_words(change: ChangeKind, id: Uuid, current: &Q, candidate: Option<&Q>) -> Option<crate::Text> {
    use ew_schema::{CommittedRoute, WayStep};
    let route_id = ew_ids::Id::<CommittedRoute>::from_uuid(id);
    let q = match (change, candidate) {
        (ChangeKind::Removed, _) | (_, None) => current,
        (_, Some(c)) => c,
    };
    let route = q.table::<CommittedRoute>().get(route_id)?.clone();
    let wire = record_title(q, route.wire.as_uuid(), None)?;
    if change == ChangeKind::Removed {
        let kept = candidate.is_some_and(|c| {
            c.table::<CommittedRoute>()
                .iter()
                .any(|(_, r)| r.wire == route.wire)
        });
        return Some(if kept {
            text!("preview-route-replaced", wire)
        } else {
            text!("preview-route-unfinished", wire)
        });
    }
    let comma = super::layout_edit::comma(q);
    let run = ew_derive::text::mm_with_unit(i128::from(route.run.0), comma);
    // A changed way says the length it had, where that changed.
    let was = (change == ChangeKind::Changed)
        .then(|| current.table::<CommittedRoute>().get(route_id).map(|r| r.run))
        .flatten()
        .filter(|old| *old != route.run)
        .map(|old| ew_derive::text::mm_with_unit(i128::from(old.0), comma));
    let mut ducts = Vec::new();
    for step in &route.way {
        if let WayStep::Duct(s) = step
            && !ducts.contains(&s.carrier)
        {
            ducts.push(s.carrier);
        }
    }
    let names: Vec<ew_l10n::TextArg> = ducts
        .iter()
        .filter_map(|c| ew_derive::layout::carrier_label(q, *c))
        .map(|l| ew_l10n::TextArg::Text(ew_derive::text::carrier_label(l)))
        .collect();
    Some(match (names.is_empty(), was) {
        (true, None) => text!("preview-route-way-plain", wire, run),
        (true, Some(was)) => text!("preview-route-way-plain-was", wire, run, was),
        (false, was) => {
            let through = ew_l10n::TextArg::List(ew_l10n::List::new(
                names,
                ew_derive::text!("part-fields-between"),
                ew_derive::text!("part-fields-and"),
            ));
            match was {
                None => text!("preview-route-way", wire, through, run),
                Some(was) => text!("preview-route-way-was", wire, through, run, was),
            }
        }
    })
}

/// The name of a library copy in the project: a part's number, a manufacturer's name; `None` for any other kind.
pub(super) fn library_name(q: &Q, kind: ew_schema::Kind, id: Uuid, source: Option<&str>) -> Option<String> {
    match kind {
        ew_schema::Kind::Part => q
            .parts()
            .get(ew_ids::Id::from_uuid(id))
            .map(|c| c.item.part_number.to_string()),
        ew_schema::Kind::Manufacturer => q
            .manufacturers()
            .get(ew_ids::Id::from_uuid(id))
            .and_then(|c| serde_json::to_value(&c.item.name).ok())
            .map(|v| ml_text(&v, source)),
        _ => None,
    }
}

/// The lines of `diff` in the order of [`entity_order`]: labels from `current`, an added record's from `candidate` (else from
/// `current`, where it has none and its label is empty).
pub(crate) fn entities_of(diff: &ModelDiff, current: &Q, candidate: Option<&Q>) -> Vec<PreviewEntity> {
    let mut out: Vec<PreviewEntity> = diff
        .changes
        .iter()
        .map(|c| entity(c, current, candidate))
        .collect();
    out.sort_by(entity_order);
    out
}

/// The order: change, kind, label in natural order (then bytes), ID.
fn entity_order(a: &PreviewEntity, b: &PreviewEntity) -> Ordering {
    change_rank(a.change)
        .cmp(&change_rank(b.change))
        .then_with(|| a.kind.cmp(&b.kind))
        .then_with(|| natural_cmp(&a.label, &b.label))
        .then_with(|| a.label.cmp(&b.label))
        .then_with(|| a.id.cmp(&b.id))
}

impl Session {
    /// The records `preview` changes, one per `EntityChange` of [`Preview::diff`], labelled by the
    /// property grid's rule (an added record by the candidate model's), in the order change (`added`, `changed`,
    /// `removed`), kind, label in natural order, ID. One call per card. The candidate model is read where a record is
    /// added, or where a wire's way changes (its line says the new way) or goes (whether the wire keeps one).
    pub fn preview_entities(&self, preview: &Preview) -> Vec<PreviewEntity> {
        let current = self.query();
        let candidate = preview
            .diff()
            .changes
            .iter()
            .any(|c| c.change == ChangeKind::Added || c.key().0 == ew_schema::Kind::CommittedRoute)
            .then(|| self.query_over(preview.candidate()));
        entities_of(preview.diff(), &current, candidate.as_ref())
    }

    /// The folio of every drawn page in `preview`'s candidate model, by page ID, in its page order: the Preview tab
    /// names a page whose folio the card changes by both folios ("106 → 104").
    pub fn preview_folios(&self, preview: &Preview) -> Vec<(Uuid, String)> {
        let q = self.query_over(preview.candidate());
        q.get::<PageOrder>(())
            .entries
            .iter()
            .filter_map(|e| match e.page {
                PageRef::Drawn(p) => Some((p.as_uuid(), e.folio.clone())),
                _ => None,
            })
            .collect()
    }

    /// The sheet display list of the page labelled `folio` as `preview` would leave it, read from its candidate model:
    /// the page Arrange and route shows before anything changes (docs/design/06a-drafting.md §7.3). `None` for a
    /// folio the candidate does not have.
    pub fn preview_page(&self, preview: &Preview, folio: &str) -> Option<ew_export::PdfPage> {
        let options = super::ExportOptions {
            pages: super::PageSelection::Folios(vec![folio.to_owned()]),
            underlays: true,
            screen: true,
            ..super::ExportOptions::default()
        };
        let q = self.query_over(preview.candidate());
        super::export_pages_with(&q, &options, &super::ExportAssets::of(preview.candidate()))
            .ok()?
            .pages
            .into_iter()
            .next()
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
            words: None,
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
