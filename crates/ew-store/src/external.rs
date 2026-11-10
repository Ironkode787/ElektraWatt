// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! External change (doc 05 §9): a project that changes on disk while a
//! session has it open, through a pull, a checkout or a sync client.
//!
//! - **Detection** ([`crate::Store::poll`]): the SHA-256 of `manifest.json`'s raw bytes against the one the store loaded
//!   or wrote, every [`POLL_EVERY`] and on window focus. A change waits for quiescence: two reads [`QUIESCENCE`] apart
//!   with equal bytes and no git operation in progress ([`crate::git::busy`]). The store's own save writes the hash it
//!   then holds, so it is never taken for an external change. A file that changes while the manifest stays is caught
//!   by the save's pre-check (D5).
//! - **The incoming generation** ([`crate::Store::read_incoming`]): read through the store's own open path, read-only
//!   (never a roll-forward, never a lock). A generation that a finding makes read-only, or whose model does not build,
//!   is not applied (D21): [`Unreadable`].
//! - **The merge** ([`merge`]): per record, the baseline (the last loaded or saved model), ours (the session's model)
//!   and theirs (the incoming model) through the one three-way core of `ew-format` ([`ew_format::merge_record`], D2).
//!   The colliding fields ([`Collision`]) are held for a choice of mine or theirs per field ([`ExternalMerge::rows`]).
//! - **Adopting** ([`crate::Store::adopt_incoming`]): after the session committed the result, the incoming generation
//!   becomes the store's baseline and the journal follows it (D23, [`crate::journal::Journal::rebase`]).

use std::collections::BTreeSet;
use std::time::Duration;

use ew_format::ew_schema::{Conflict, ConflictBody, DELETED_SIDE};
use ew_format::merge_record;
use ew_model::{AnyKey, AnyRow, Model};
use serde_json::Value;

use crate::rows::{all_keys, row_value, value_row};

/// How often a session polls the manifest (doc 05 §9, D20; not measured, owner's rule).
pub const POLL_EVERY: Duration = Duration::from_secs(3);
/// How long the manifest's bytes stay equal before a change is read (D20).
pub const QUIESCENCE: Duration = Duration::from_millis(750);
/// How long a session on a cloud-synced path waits before it reads an unreadable generation again (D21).
pub const CLOUD_RECHECK: Duration = Duration::from_secs(5);
/// How often a session on a cloud-synced path reads an unreadable generation again before it holds its saves (D21).
pub const CLOUD_RECHECKS: u32 = 3;

/// What a poll of the manifest found ([`crate::Store::poll`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Poll {
    /// `manifest.json` holds the bytes the store loaded or wrote.
    Unchanged,
    /// It changed, and its bytes have not been quiet for [`QUIESCENCE`] yet: poll again after it.
    Settling,
    /// It changed while a git operation is in progress: poll again later.
    GitBusy,
    /// It changed and has been quiet: read the incoming generation.
    Changed,
}

/// Why an incoming generation is not applied (D21): the project keeps its model and its edits.
#[derive(Debug)]
pub enum Unreadable {
    /// It opens, but a finding makes it read-only (a missing or changed file, an unreadable line, a newer format).
    Findings(Vec<ew_format::Finding>),
    /// It does not open (its model does not build, its format refuses).
    Open(crate::OpenError),
}

/// The side a colliding field takes ([`ExternalMerge::rows`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Take {
    /// The session's value.
    Mine,
    /// The incoming generation's value.
    Theirs,
}

/// A field that the session and the incoming generation changed differently (D22).
#[derive(Clone, PartialEq, Debug)]
pub struct Collision {
    /// The record.
    pub key: AnyKey,
    /// The member's dotted path (`overrides.colour`); empty when one side deleted the record and the other changed it.
    pub path: String,
    /// Its value at the baseline (`None`: absent); for the empty path the whole record.
    pub base: Option<Value>,
    /// Its value in the session; for the empty path the whole record, `None` when the session deleted it.
    pub mine: Option<Value>,
    /// Its value in the incoming generation; for the empty path the whole record, `None` when it was deleted there.
    pub theirs: Option<Value>,
}

/// One record that the incoming generation changed and the session must take.
#[derive(Clone, PartialEq, Debug)]
struct Merged {
    key: AnyKey,
    /// The session's record (without `$k`).
    ours: Option<Value>,
    /// The merged record, mine at every colliding field.
    merged: Option<Value>,
    /// The incoming record, for the collisions of this record.
    theirs: Option<Value>,
    /// The indexes of this record's collisions in [`ExternalMerge::collisions`].
    collisions: Vec<usize>,
}

/// The three-way merge of an external change ([`merge`]).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ExternalMerge {
    /// The records the incoming generation changed against the baseline, in key order.
    pub changed: Vec<AnyKey>,
    /// The colliding fields, in key and path order.
    pub collisions: Vec<Collision>,
    merged: Vec<Merged>,
}

impl ExternalMerge {
    /// The rows the session writes to take the change: each record whose merged form differs from the session's, with
    /// its row (`None`: deleted), mine or theirs at each colliding field as `choices` says, one choice per collision in
    /// order. Without collisions, `choices` is empty.
    ///
    /// # Errors
    ///
    /// When `choices` does not hold one choice per collision, or a merged record does not read back as a record.
    pub fn rows(&self, choices: &[Take]) -> Result<Vec<(AnyKey, Option<AnyRow>)>, String> {
        if choices.len() != self.collisions.len() {
            return Err(format!(
                "{} choices for {} colliding fields",
                choices.len(),
                self.collisions.len()
            ));
        }
        let mut out = Vec::new();
        for m in &self.merged {
            let mut value = m.merged.clone();
            for &i in &m.collisions {
                if choices[i] == Take::Mine {
                    continue;
                }
                let path = &self.collisions[i].path;
                if path.is_empty() {
                    value.clone_from(&m.theirs);
                } else if let Some(v) = &mut value {
                    set_path(v, path, at_path(m.theirs.as_ref(), path));
                }
            }
            if value == m.ours {
                continue;
            }
            let row = value
                .map(|v| value_row(m.key.0, v))
                .transpose()
                .map_err(|e| format!("{}:{}: {e}", m.key.0, m.key.1))?;
            out.push((m.key, row));
        }
        Ok(out)
    }
}

/// The three-way merge of the baseline `base`, the session's model `ours` and the incoming model `theirs` (D22): per
/// record that `theirs` changed against `base`, through [`ew_format::merge_record`]. A record that only the session
/// changed stays as it is.
///
/// # Errors
///
/// A record without a JSON form.
pub fn merge(base: &Model, ours: &Model, theirs: &Model) -> Result<ExternalMerge, String> {
    let value = |m: &Model, key: &AnyKey| -> Result<Option<Value>, String> {
        m.row(key)
            .map(|r| row_value(&r).map_err(|e| e.to_string()))
            .transpose()
    };
    let keys: BTreeSet<AnyKey> = all_keys(base).into_iter().chain(all_keys(theirs)).collect();
    let mut out = ExternalMerge::default();
    for key in keys {
        let (b, t) = (value(base, &key)?, value(theirs, &key)?);
        if b == t {
            continue;
        }
        out.changed.push(key);
        let o = value(ours, &key)?;
        if o == t {
            continue;
        }
        if o == b {
            out.merged.push(Merged {
                key,
                ours: o,
                merged: t.clone(),
                theirs: t,
                collisions: Vec::new(),
            });
            continue;
        }
        let tagged = |v: &Option<Value>| {
            v.clone().map(|mut v| {
                if let Value::Object(m) = &mut v {
                    m.insert("$k".to_owned(), Value::from(key.0.name()));
                }
                v
            })
        };
        let result = merge_record(tagged(&b).as_ref(), tagged(&o).as_ref(), tagged(&t).as_ref());
        let merged = result.merged.map(|mut v| {
            if let Value::Object(m) = &mut v {
                m.remove("$k");
            }
            v
        });
        let mut collisions = Vec::new();
        if let Some(conflict) = result.conflict {
            for path in colliding_paths(conflict)? {
                collisions.push(out.collisions.len());
                out.collisions.push(Collision {
                    key,
                    base: at_path(b.as_ref(), &path),
                    mine: at_path(o.as_ref(), &path),
                    theirs: at_path(t.as_ref(), &path),
                    path,
                });
            }
        }
        // With one side deleted, the core keeps the changed side in place; mine is the session's record.
        let merged = if collisions.iter().any(|&i| out.collisions[i].path.is_empty()) {
            o.clone()
        } else {
            merged
        };
        out.merged.push(Merged {
            key,
            ours: o,
            merged,
            theirs: t,
            collisions,
        });
    }
    Ok(out)
}

/// The colliding paths of a conflict record: its member paths, or the one empty path when one side deleted the record.
fn colliding_paths(conflict: Value) -> Result<Vec<String>, String> {
    let conflict: Conflict = serde_json::from_value(conflict).map_err(|e| e.to_string())?;
    if let ConflictBody::Record { ours, theirs, .. } = &conflict.body {
        if ours.as_str() == Some(DELETED_SIDE) || theirs.as_str() == Some(DELETED_SIDE) {
            return Ok(vec![String::new()]);
        }
    }
    Ok(conflict.member_paths())
}

/// The value at the dotted `path` of `value` (`None`: absent); the whole value for the empty path.
fn at_path(value: Option<&Value>, path: &str) -> Option<Value> {
    let mut at = value?;
    for name in path.split('.').filter(|n| !n.is_empty()) {
        at = at.get(name)?;
    }
    Some(at.clone())
}

/// Sets the dotted `path` of `value` to `to`, or removes it for `None`; missing objects on the way are made.
fn set_path(value: &mut Value, path: &str, to: Option<Value>) {
    let names: Vec<&str> = path.split('.').collect();
    let Some((last, parents)) = names.split_last() else {
        return;
    };
    let mut at = value;
    for name in parents {
        let Value::Object(m) = at else { return };
        at = m
            .entry((*name).to_owned())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
    }
    let Value::Object(m) = at else { return };
    match to {
        Some(v) => {
            m.insert((*last).to_owned(), v);
        }
        None => {
            m.remove(*last);
        }
    }
}
