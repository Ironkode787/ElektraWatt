// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `diff_model(current, candidate)` (ADR-0008, ARCH §5.4): the entity and field changes between two
//! snapshots, for previews. `diff_query` over derived rows arrives with the query layer (WP-S1.05).

use std::collections::BTreeMap;

use ew_model::{AnyKey, Model};
use serde_json::Value;

use crate::rows::model_lines;

/// One changed field: a dotted path of JSON members (`tag.counter`, `overrides.csa`). Objects are
/// compared member by member; arrays and scalars are compared whole.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FieldChange {
    /// The member path.
    pub path: String,
    /// The old value (`None`: the member was absent, i.e. at its default).
    pub before: Option<Value>,
    /// The new value (`None`: the member is absent now).
    pub after: Option<Value>,
}

/// One changed record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EntityChange {
    /// The record is new.
    Added {
        /// Its key.
        key: AnyKey,
        /// Its JSON form.
        after: Value,
    },
    /// The record is removed.
    Removed {
        /// Its key.
        key: AnyKey,
        /// Its old JSON form.
        before: Value,
    },
    /// Fields of the record changed.
    Changed {
        /// Its key.
        key: AnyKey,
        /// The changed fields, in path order.
        fields: Vec<FieldChange>,
    },
}

impl EntityChange {
    /// The record's key.
    pub fn key(&self) -> AnyKey {
        match self {
            EntityChange::Added { key, .. }
            | EntityChange::Removed { key, .. }
            | EntityChange::Changed { key, .. } => *key,
        }
    }
}

/// The entity and field changes between two snapshots, in key order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ModelDiff {
    /// The changes.
    pub changes: Vec<EntityChange>,
}

impl ModelDiff {
    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The number of changed records.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// The change of one record, if any.
    pub fn get(&self, key: &AnyKey) -> Option<&EntityChange> {
        self.changes
            .binary_search_by(|c| c.key().cmp(key))
            .ok()
            .map(|i| &self.changes[i])
    }
}

fn field_changes(prefix: &str, before: &Value, after: &Value, out: &mut Vec<FieldChange>) {
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => field_changes(&path, x, y, out),
                    (x, y) => out.push(FieldChange {
                        path,
                        before: x.cloned(),
                        after: y.cloned(),
                    }),
                }
            }
        }
        (a, b) if a != b => out.push(FieldChange {
            path: prefix.to_owned(),
            before: Some(a.clone()),
            after: Some(b.clone()),
        }),
        _ => {}
    }
}

/// The entity and field changes from `current` to `candidate`: every record added, removed or changed,
/// in key order, with the changed field paths of each changed record.
pub fn diff_model(current: &Model, candidate: &Model) -> ModelDiff {
    let old: BTreeMap<AnyKey, String> = model_lines(current).into_iter().collect();
    let new: BTreeMap<AnyKey, String> = model_lines(candidate).into_iter().collect();
    let mut keys: Vec<&AnyKey> = old.keys().chain(new.keys()).collect();
    keys.sort();
    keys.dedup();
    let parse = |s: &String| serde_json::from_str::<Value>(s).unwrap_or(Value::Null);
    let mut changes = Vec::new();
    for key in keys {
        match (old.get(key), new.get(key)) {
            (None, Some(b)) => changes.push(EntityChange::Added {
                key: *key,
                after: parse(b),
            }),
            (Some(a), None) => changes.push(EntityChange::Removed {
                key: *key,
                before: parse(a),
            }),
            (Some(a), Some(b)) if a != b => {
                let mut fields = Vec::new();
                field_changes("", &parse(a), &parse(b), &mut fields);
                changes.push(EntityChange::Changed { key: *key, fields });
            }
            _ => {}
        }
    }
    ModelDiff { changes }
}
