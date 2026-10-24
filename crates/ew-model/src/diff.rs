// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The first difference between two models ([`crate::Model::content_diff`]): what
//! [`crate::Model::content_eq`] calls unequal, named for a diagnosis (the save-and-reload oracle of the command
//! fuzzer, FT-468).

use std::fmt;
use std::sync::Arc;

use ew_schema::{LibDocument, Record};

use crate::entity::AnyKey;
use crate::model::Model;

/// Which of the two compared models holds a differing row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Presence {
    /// Both models hold the row, with different contents.
    Both,
    /// Only the first model (`self` of [`crate::Model::content_diff`]) holds it.
    FirstOnly,
    /// Only the second model (`other`) holds it.
    SecondOnly,
}

/// The first difference [`crate::Model::content_diff`] finds, in the order [`crate::Model::content_eq`] compares.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ContentDiff {
    /// The project rows differ.
    Project,
    /// A table row: the first differing key in kind order (the model's table order), then ID order.
    Row {
        /// The row's kind and ID.
        key: AnyKey,
        /// Which model holds it.
        presence: Presence,
    },
    /// The records of unknown or reserved kinds: the first position, in line order, where the two lists differ,
    /// with the compact JSON of each model's record there (`None` past the end of that model's list).
    UnknownRecord {
        /// The position in line order.
        position: usize,
        /// The first model's record there.
        first: Option<String>,
        /// The second model's record there.
        second: Option<String>,
    },
    /// The library documents of unknown kinds, likewise.
    UnknownLibDocument {
        /// The position in `$k`-then-ID order.
        position: usize,
        /// The first model's document there.
        first: Option<String>,
        /// The second model's document there.
        second: Option<String>,
    },
    /// A base index: the first key, in key order, whose values differ.
    Index {
        /// The index's name: its field name in `BaseIndexes`, e.g. `structure_boxes_by_page`.
        index: &'static str,
        /// The key, as `Debug` text.
        key: String,
        /// The first model's values under the key, each as `Debug` text, in order (empty when it lacks the key).
        first: Vec<String>,
        /// The second model's values, likewise.
        second: Vec<String>,
    },
}

/// The text of an optional record or document: its JSON, or `none` past the end of a list.
fn option_text(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("none")
}

impl fmt::Display for ContentDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentDiff::Project => f.write_str("the project rows differ"),
            ContentDiff::Row { key, presence } => {
                let (kind, id) = key;
                let kind = kind.name();
                match presence {
                    Presence::Both => write!(f, "the {kind} row {id} differs"),
                    Presence::FirstOnly => write!(f, "the {kind} row {id} is in the first model only"),
                    Presence::SecondOnly => write!(f, "the {kind} row {id} is in the second model only"),
                }
            }
            ContentDiff::UnknownRecord {
                position,
                first,
                second,
            } => write!(
                f,
                "the unknown records differ at position {position}: first {}, second {}",
                option_text(first),
                option_text(second)
            ),
            ContentDiff::UnknownLibDocument {
                position,
                first,
                second,
            } => write!(
                f,
                "the unknown library documents differ at position {position}: first {}, second {}",
                option_text(first),
                option_text(second)
            ),
            ContentDiff::Index {
                index,
                key,
                first,
                second,
            } => write!(
                f,
                "the index {index} differs at key {key}: first [{}], second [{}]",
                first.join(", "),
                second.join(", ")
            ),
        }
    }
}

/// The first position where two lists differ (an item unequal, or present in one list only), with the text of
/// each list's item there.
fn first_list_difference<T: PartialEq>(
    a: &[T],
    b: &[T],
    text: impl Fn(&T) -> String,
) -> Option<(usize, Option<String>, Option<String>)> {
    let position = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i))?;
    Some((position, a.get(position).map(&text), b.get(position).map(&text)))
}

impl Model {
    /// The first difference between two models, or `None` exactly when [`crate::Model::content_eq`] holds. It
    /// compares in `content_eq`'s order and stops at the first unequal part: the project row, the table rows (kind
    /// order, then ID order), the unknown records, the unknown library documents, the base indexes (in a fixed
    /// order, then key order). `rev` and `changed_at` stamps are ignored. For diagnostics (oracles and tests); it
    /// costs what `content_eq` costs.
    pub fn content_diff(&self, other: &Model) -> Option<ContentDiff> {
        if !(Arc::ptr_eq(&self.project.value, &other.project.value)
            || self.project.value == other.project.value)
        {
            return Some(ContentDiff::Project);
        }
        if let Some((key, presence)) = crate::entity::first_row_difference(self, other) {
            return Some(ContentDiff::Row { key, presence });
        }
        if let Some((position, first, second)) =
            first_list_difference(&self.unknown, &other.unknown, |r: &Record| {
                serde_json::to_string(r).unwrap_or_else(|e| format!("<{e}>"))
            })
        {
            return Some(ContentDiff::UnknownRecord {
                position,
                first,
                second,
            });
        }
        if let Some((position, first, second)) =
            first_list_difference(&self.unknown_lib, &other.unknown_lib, |d: &LibDocument| {
                serde_json::to_string(d).unwrap_or_else(|e| format!("<{e}>"))
            })
        {
            return Some(ContentDiff::UnknownLibDocument {
                position,
                first,
                second,
            });
        }
        self.idx.first_difference(&other.idx)
    }
}
