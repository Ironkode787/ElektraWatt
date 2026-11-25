// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `merge.resolve` (`ResolveConflicts`, FR-COL-015): the conflict records a merge left (doc 05 §11.2,
//! §11.4), resolved by taking ours or theirs, in one transaction.

use std::collections::BTreeSet;

use ew_format::{Anchors, FilePath, record_file};
use ew_ids::Id;
use ew_model::{AnyKey, AnyRow};
use ew_schema::{
    AnyId, Carrier, Conflict, ConflictBody, ConflictSide, DELETED_SIDE, Enclosure, Function, Junction, Kind,
    KindStatus, MountingSurface, Record, Representation,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::CommandMeta;
use crate::proxy::UuidS;
use crate::rows::row_json;
use crate::tx::TxBuilder;

/// The side of a conflict a choice takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Take {
    /// Ours: the side the merge kept in place.
    Ours,
    /// Theirs: the other side.
    Theirs,
}

/// One conflict and the side to take.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConflictChoice {
    /// The conflict record: the ID of the record it targets, or of a file-level conflict.
    #[schemars(with = "UuidS")]
    pub conflict: Id<Conflict>,
    /// The side to take.
    pub take: Take,
}

/// `merge.resolve` (`ResolveConflicts`): resolves conflict records (doc 05 §11.2, doc 15 §6.5).
///
/// Per choice, a **record-level** conflict:
/// - **ours** removes the conflict: ours is the record in place. When ours deleted the record (a delete/modify
///   conflict, whose merge kept theirs in place), taking ours deletes the record, with its conflict;
/// - **theirs** writes the conflicting members of theirs into the record, a member theirs lacks removed, and removes
///   the conflict; when theirs deleted the record, it deletes the record with its conflict; when the record is gone,
///   it is made again from theirs' members.
///
/// A **file-level** conflict holds the lines of the side the merge could not read: taking that side replaces the
/// records of the file with the records of its lines (a record of the file the lines lack is deleted, a record they
/// hold is written), when every line reads as a record this version knows; taking the other side removes the
/// conflict, since its records are in place.
///
/// Rejected: no choice (at `/choices`); a choice naming no conflict record, or a conflict an earlier choice names (at
/// `/choices/i/conflict`); theirs of a record this version keeps unchanged, or members that do not make a valid
/// record; held lines that do not read; the project record deleted. One transaction, one undo step. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolveConflicts {
    /// The conflicts and the side to take for each.
    pub choices: Vec<ConflictChoice>,
}

impl Command for ResolveConflicts {
    const NAME: &'static str = "merge.resolve";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.choices.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/choices",
                text!("refusal-merge-none-chosen"),
            ));
        }
        let mut seen = BTreeSet::new();
        for (i, choice) in self.choices.iter().enumerate() {
            let at = format!("/choices/{i}/conflict");
            if !seen.insert(choice.conflict) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    text!("refusal-merge-chosen-twice"),
                ));
            }
            let Some(conflict) = tx.get::<Conflict>(choice.conflict) else {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    text!("refusal-merge-no-conflict-record"),
                ));
            };
            match &conflict.body {
                ConflictBody::Record {
                    kind, ours, theirs, ..
                } => resolve_record(tx, conflict.id, kind, ours, theirs, choice.take)?,
                ConflictBody::File { path, side, lines } => {
                    resolve_file(tx, conflict.id, path, side, lines, choice.take)?;
                }
            }
        }
        Ok(Outcome::default())
    }
}

/// Removes a conflict record if it is still there (a deleted target takes its conflict along).
fn remove_conflict(tx: &mut TxBuilder<'_>, id: Id<Conflict>) {
    let key = (Kind::Conflict, id.cast());
    if tx.row(&key).is_some() {
        tx.remove_row(key);
    }
}

/// A record-level conflict ([`ResolveConflicts`]).
fn resolve_record(
    tx: &mut TxBuilder<'_>,
    id: Id<Conflict>,
    kind: &str,
    ours: &Value,
    theirs: &Value,
    take: Take,
) -> Result<(), CmdError> {
    let target: Option<AnyKey> = Kind::from_name(kind)
        .filter(|k| k.status() == KindStatus::Implemented && !k.is_library() && *k != Kind::Conflict)
        .map(|k| (k, id.cast()));
    let chosen = match take {
        Take::Ours => ours,
        Take::Theirs => theirs,
    };
    let present = target.filter(|k| tx.row(k).is_some());
    if chosen.as_str() == Some(DELETED_SIDE) {
        if let Some(key) = present {
            if key.0 == Kind::Project {
                return tx.reject(text!("refusal-merge-project-record"));
            }
            tx.delete(key)?;
        }
        remove_conflict(tx, id);
        return Ok(());
    }
    if take == Take::Ours {
        remove_conflict(tx, id);
        return Ok(());
    }
    let Some(key) = target else {
        return tx.reject(text!("refusal-merge-kind-kept", kind.to_string()));
    };
    let mut value = match tx.row(&key) {
        Some(row) => row_json(&row)?,
        None => Value::Object(Map::from_iter([("id".to_owned(), Value::from(id.to_string()))])),
    };
    apply(&mut value, Some(ours), Some(theirs));
    if let Value::Object(members) = &mut value {
        members.insert("$k".to_owned(), Value::from(kind));
    }
    let record = Record::from_value(value).map_err(|e| {
        CmdError::rejected(
            ResolveConflicts::NAME,
            text!(
                "refusal-merge-invalid-record",
                crate::error::schema_error_text(&e)
            ),
        )
    })?;
    put_record(tx, record)?;
    remove_conflict(tx, id);
    Ok(())
}

/// Writes theirs' partial object into `target`: a member both partial objects hold as objects (and the target too)
/// member by member, any other member of either side set to theirs' value, or removed where theirs lacks it.
fn apply(target: &mut Value, ours: Option<&Value>, theirs: Option<&Value>) {
    let names: BTreeSet<String> = [ours, theirs]
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .flat_map(|m| m.keys().cloned())
        .collect();
    let Value::Object(members) = target else {
        return;
    };
    for name in names {
        let (o, t) = (ours.and_then(|v| v.get(&name)), theirs.and_then(|v| v.get(&name)));
        let nested = t.is_some_and(Value::is_object)
            && o.is_none_or(Value::is_object)
            && members.get(&name).is_some_and(Value::is_object);
        match (nested, t) {
            (true, _) => {
                if let Some(inner) = members.get_mut(&name) {
                    apply(inner, o, t);
                }
            }
            (false, Some(t)) => {
                members.insert(name, t.clone());
            }
            (false, None) => {
                members.remove(&name);
            }
        }
    }
}

/// Puts a record: the project record in place, any other under its key.
fn put_record(tx: &mut TxBuilder<'_>, record: Record) -> Result<(), CmdError> {
    match AnyRow::from_record(record) {
        Ok(AnyRow::Project(project)) => tx.set_project((*project).clone()),
        Ok(row) => {
            tx.put_row(row);
            Ok(())
        }
        Err(_) => tx.reject(text!("refusal-merge-record-kept")),
    }
}

/// A file-level conflict ([`ResolveConflicts`]).
fn resolve_file(
    tx: &mut TxBuilder<'_>,
    id: Id<Conflict>,
    path: &str,
    side: &ConflictSide,
    lines: &[String],
    take: Take,
) -> Result<(), CmdError> {
    let held = match side {
        ConflictSide::Ours => Take::Ours,
        ConflictSide::Theirs => Take::Theirs,
        ConflictSide::Unknown(_) => {
            return tx.reject(text!("refusal-merge-file-side"));
        }
    };
    if take != held {
        remove_conflict(tx, id);
        return Ok(());
    }
    let mut records = Vec::new();
    for (n, line) in lines.iter().enumerate().filter(|(_, l)| !l.is_empty()) {
        let record = ew_schema::parse_value(line)
            .and_then(Record::from_value)
            .ok()
            .filter(|r| !matches!(r, Record::Unknown(_)));
        match record {
            Some(r) => records.push(r),
            None => {
                return tx.reject(text!("refusal-merge-file-line", (n + 1).to_string()));
            }
        }
    }
    let keep: BTreeSet<AnyKey> = records
        .iter()
        .filter_map(|r| Some((r.kind()?, AnyId::from_uuid(r.id()?))))
        .collect();
    let conflict: AnyKey = (Kind::Conflict, id.cast());
    let mut keys: BTreeSet<AnyKey> = tx.base_model().keys().into_iter().collect();
    keys.extend(tx.changed_keys());
    let mut gone = Vec::new();
    for key in keys {
        if key == conflict || key.0 == Kind::Project || key.0.is_library() || keep.contains(&key) {
            continue;
        }
        let Some(record) = tx.row(&key).and_then(|row| row.to_record()) else {
            continue;
        };
        if file_of(tx, &record).is_some_and(|file| file.as_str() == path) {
            gone.push(key);
        }
    }
    for record in records {
        put_record(tx, record)?;
    }
    let gone: Vec<AnyKey> = gone.into_iter().filter(|k| tx.row(k).is_some()).collect();
    tx.delete_many(&gone)?;
    remove_conflict(tx, id);
    Ok(())
}

/// The project-relative file a record of the transaction lives in: `ew_format::record_file` over the
/// transaction's rows ([`TxAnchors`]), `None` for a record without one.
fn file_of(tx: &TxBuilder<'_>, record: &Record) -> Option<FilePath> {
    record_file(record, &TxAnchors(tx)).ok()
}

/// [`Anchors`] answered from a transaction: the facts of doc 05 §2.2 as the transaction reads them.
struct TxAnchors<'a, 'm>(&'a TxBuilder<'m>);

impl Anchors for TxAnchors<'_, '_> {
    fn function_device(&self, f: AnyId) -> Option<Option<AnyId>> {
        self.0
            .get::<Function>(f.cast())
            .map(|func| func.device.map(|d| d.cast()))
    }

    fn junction_page(&self, j: AnyId) -> Option<AnyId> {
        self.0.get::<Junction>(j.cast()).map(|j| j.page.cast())
    }

    fn function_rep_page(&self, f: AnyId) -> Option<AnyId> {
        let rep = self.0.reps_of_function(f.cast()).into_iter().min()?;
        self.0.get::<Representation>(rep).map(|r| r.page.cast())
    }

    fn carrier_surface(&self, c: AnyId) -> Option<AnyId> {
        self.0.get::<Carrier>(c.cast()).map(|c| c.surface.cast())
    }

    fn surface_enclosure(&self, s: AnyId) -> Option<AnyId> {
        self.0
            .get::<MountingSurface>(s.cast())
            .map(|s| s.enclosure.cast())
    }

    fn enclosure_space(&self, e: AnyId) -> Option<AnyId> {
        self.0.get::<Enclosure>(e.cast()).map(|e| e.space.cast())
    }

    fn target_file(&self, kind: Kind, id: AnyId) -> Option<FilePath> {
        let target = self.0.row(&(kind, id))?.to_record()?;
        record_file(&target, self).ok()
    }
}
