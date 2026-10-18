// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The entity and field changes of a transaction (ADR-0008, ARCH §5.4), for previews and the history
//! panel. `diff_query` over derived rows arrives with the query layer (WP-S1.05).
//!
//! A diff is a **summary**: per changed record the kind of change, the record's key (kind and ID) and the
//! dotted paths of the changed members, never the record values. Values are read through the tracked
//! query layer; this keeps the untracked row access of `ew-commands` (`raw-read`) from leaking to the
//! crates that depend on it (ARCH §3 rule 1, finding F9).

use std::collections::BTreeMap;

use ew_model::{AnyKey, AnyRow, Model, Op};
use serde_json::Value;

use crate::error::CmdError;
use crate::rows::{model_lines, row_json};

/// What happened to a record.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ChangeKind {
    /// The record is new.
    Added,
    /// The record is removed.
    Removed,
    /// Members of the record changed.
    Changed,
}

/// One changed record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EntityChange {
    /// Added, removed or changed.
    pub change: ChangeKind,
    /// The record's key: its kind and ID.
    pub key: AnyKey,
    /// For a changed record, the changed member paths in path order (`tag.counter`, `overrides.csa`;
    /// objects are compared member by member, arrays and scalars whole). Empty for an added or removed
    /// record.
    pub fields: Vec<String>,
}

impl EntityChange {
    /// The record's key.
    pub fn key(&self) -> AnyKey {
        self.key
    }
}

/// The entity and field changes of a transaction, in key order.
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
            .binary_search_by(|c| c.key.cmp(key))
            .ok()
            .map(|i| &self.changes[i])
    }

    /// The summary of a batch of ops: per key, the first op's `before` against the last op's `after` (a
    /// batch may put a key more than once, as a coalesced history step does). O(ops), independent of the
    /// model's size.
    pub fn of_ops(ops: &[Op]) -> Result<ModelDiff, CmdError> {
        let mut ends: BTreeMap<AnyKey, (Option<&AnyRow>, Option<&AnyRow>)> = BTreeMap::new();
        for op in ops {
            ends.entry(op.key())
                .and_modify(|e| e.1 = op.after())
                .or_insert((op.before(), op.after()));
        }
        let mut changes = Vec::new();
        for (key, (before, after)) in ends {
            let change = match (before, after) {
                (None, None) => continue,
                (None, Some(_)) => EntityChange {
                    change: ChangeKind::Added,
                    key,
                    fields: Vec::new(),
                },
                (Some(_), None) => EntityChange {
                    change: ChangeKind::Removed,
                    key,
                    fields: Vec::new(),
                },
                (Some(a), Some(b)) => {
                    if a.same(b) {
                        continue;
                    }
                    let mut fields = Vec::new();
                    field_paths("", &row_json(a)?, &row_json(b)?, &mut fields);
                    if fields.is_empty() {
                        continue;
                    }
                    EntityChange {
                        change: ChangeKind::Changed,
                        key,
                        fields,
                    }
                }
            };
            changes.push(change);
        }
        Ok(ModelDiff { changes })
    }
}

fn field_paths(prefix: &str, before: &Value, after: &Value, out: &mut Vec<String>) {
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
                    (Some(x), Some(y)) => field_paths(&path, x, y, out),
                    _ => out.push(path),
                }
            }
        }
        (a, b) if a != b => out.push(prefix.to_owned()),
        _ => {}
    }
}

/// The entity and field changes from `current` to `candidate` over **every** record of both snapshots
/// (O(model); the engine summarises a transaction's ops with [`ModelDiff::of_ops`] instead).
pub fn diff_model(current: &Model, candidate: &Model) -> Result<ModelDiff, CmdError> {
    let old: BTreeMap<AnyKey, String> = model_lines(current)?.into_iter().collect();
    let new: BTreeMap<AnyKey, String> = model_lines(candidate)?.into_iter().collect();
    let mut keys: Vec<&AnyKey> = old.keys().chain(new.keys()).collect();
    keys.sort();
    keys.dedup();
    let parse = |s: &String| {
        serde_json::from_str::<Value>(s)
            .map_err(|e| CmdError::internal(format!("a canonical record line does not parse ({e})")))
    };
    let mut changes = Vec::new();
    for key in keys {
        let change = match (old.get(key), new.get(key)) {
            (None, Some(_)) => ChangeKind::Added,
            (Some(_), None) => ChangeKind::Removed,
            (Some(a), Some(b)) if a != b => {
                let mut fields = Vec::new();
                field_paths("", &parse(a)?, &parse(b)?, &mut fields);
                changes.push(EntityChange {
                    change: ChangeKind::Changed,
                    key: *key,
                    fields,
                });
                continue;
            }
            _ => continue,
        };
        changes.push(EntityChange {
            change,
            key: *key,
            fields: Vec::new(),
        });
    }
    Ok(ModelDiff { changes })
}
