// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The derived consequences of a transaction (doc 03 §8.6 "diff_query", doc 17 §3.5 `--preview`), v0
//! subset: the printed device tags and the cross-reference texts. A preview shows them next to its
//! [`ModelDiff`](crate::ModelDiff): a settings edit or a renumber changes one record and many derived values
//! (doc 20 §2.2: the tags of Golden B, the "106-…" cross-references).
//!
//! Values are read through two tracked-read contexts, one over the snapshot before and one over the
//! candidate, so the diff never reads rows directly (ARCH §3 rule 1). The BOM, wire and TOC rows of
//! doc 03 §8.6 follow with the full `diff_query` (WP-S1.05).

use std::collections::BTreeSet;

use ew_derive::{DeviceTag, Q, XrefTextOf};
use ew_ids::Id;
use ew_schema::{Device, Representation};

/// One derived value that differs: its key, the value before and the value after. A value that does not
/// exist on one side (a device or representation added or removed, a representation without a
/// cross-reference) is the empty string there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DerivedChange<K> {
    /// The record the value belongs to.
    pub key: K,
    /// The value before.
    pub before: String,
    /// The value after.
    pub after: String,
}

/// The derived values that a transaction changes, each list in key (ID) order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DerivedDiff {
    /// Printed device tags (`Tag::short`: "-101S1", "S51" for a frozen tag with a hidden sign), terminals
    /// included.
    pub tags: Vec<DerivedChange<Id<Device>>>,
    /// Cross-reference texts (`xref_text`), one per representation that prints one on either side.
    pub xrefs: Vec<DerivedChange<Id<Representation>>>,
}

impl DerivedDiff {
    /// Whether no derived value changed.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.xrefs.is_empty()
    }
}

fn tag_text(q: &Q, id: Id<Device>) -> String {
    q.get::<DeviceTag>(id)
        .as_ref()
        .as_ref()
        .map(ew_derive::Tag::short)
        .unwrap_or_default()
}

fn xref_text(q: &Q, id: Id<Representation>) -> String {
    if q.reps().get(id).is_none() {
        return String::new();
    }
    q.get::<XrefTextOf>(id).text.clone()
}

/// The derived values that differ between `before` and `after` (see [`DerivedDiff`]). Cost: one tag per
/// device and one cross-reference text per representation on each side (request-cached in each `Q`).
pub fn derived_diff(before: &Q, after: &Q) -> DerivedDiff {
    let devices: BTreeSet<Id<Device>> = before.devices().ids().chain(after.devices().ids()).collect();
    let tags = devices
        .into_iter()
        .filter_map(|id| {
            let (b, a) = (tag_text(before, id), tag_text(after, id));
            (b != a).then_some(DerivedChange {
                key: id,
                before: b,
                after: a,
            })
        })
        .collect();
    let reps: BTreeSet<Id<Representation>> = before.reps().ids().chain(after.reps().ids()).collect();
    let xrefs = reps
        .into_iter()
        .filter_map(|id| {
            let (b, a) = (xref_text(before, id), xref_text(after, id));
            (b != a).then_some(DerivedChange {
                key: id,
                before: b,
                after: a,
            })
        })
        .collect();
    DerivedDiff { tags, xrefs }
}
