// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `merge.resolve` (`ResolveConflicts`, WP-S1.17 D19; FR-COL-015): the conflict records a merge left (doc 05 §11.2,
//! §11.4), resolved by taking ours or theirs, in one transaction.

use std::collections::BTreeSet;

use ew_ids::Id;
use ew_model::{AnyKey, AnyRow};
use ew_schema::{
    AnchorRef, AnyId, Carrier, CarrierRef, Conflict, ConflictBody, ConflictSide, DELETED_SIDE, Enclosure,
    Function, Junction, Kind, KindStatus, MountingSurface, Record, Representation,
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

/// `merge.resolve` (`ResolveConflicts`): resolves conflict records (WP-S1.17 D19; doc 05 §11.2, doc 15 §6.5).
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
                "no conflict chosen; name at least one",
            ));
        }
        let mut seen = BTreeSet::new();
        for (i, choice) in self.choices.iter().enumerate() {
            let at = format!("/choices/{i}/conflict");
            if !seen.insert(choice.conflict) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    "an earlier choice names this conflict already",
                ));
            }
            let Some(conflict) = tx.get::<Conflict>(choice.conflict) else {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    "names no conflict record of the project",
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
                return tx.reject("the project record cannot be deleted; take the other side");
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
        return tx.reject(format!(
            "theirs cannot be written: this version keeps records of the kind \"{kind}\" unchanged; take ours"
        ));
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
            format!("theirs does not make a valid record ({e}); take ours, or edit the record"),
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
        Err(_) => tx.reject("the record is of a kind this version keeps unchanged"),
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
            return tx.reject(
                "the conflict of a file holds a side this version does not know; open the project in a newer version",
            );
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
                return tx.reject(format!(
                    "line {} of the file the merge could not read is not a record this version reads; take the \
                     other side, or fix the file",
                    n + 1
                ));
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
        let Some(record) = tx.row(&key).map(|row| record_of(&row)).transpose()? else {
            continue;
        };
        if file_of(tx, &record, 0).as_deref() == Some(path) {
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

/// The record of a row of a JSON Lines kind.
fn record_of(row: &AnyRow) -> Result<Record, CmdError> {
    let mut value = row_json(row)?;
    if let Value::Object(members) = &mut value {
        members.insert("$k".to_owned(), Value::from(row.kind().name()));
    }
    Record::from_value(value).map_err(|e| CmdError::internal(format!("a record does not read back: {e}")))
}

/// The file `merge.resolve` routes `record` to in `model` (its `file_of`), for the test in `ew-api` that pins it to
/// `ew_format::record_file` record kind by record kind (FT-1290): the crate that sees both.
#[cfg(feature = "testkit")]
pub fn record_file_of(model: &ew_model::Model, record: &Record) -> Option<String> {
    let mut ids = ew_ids::SystemSource;
    file_of(&TxBuilder::new(model, &mut ids), record, 0)
}

/// The project-relative file a record of the transaction lives in: doc 05 §2.2's rules as `ew_format::record_file`
/// applies them over the model (`ew-commands` has no `ew-format` edge), `None` for a record of a kind without a rule
/// here. A reference that does not resolve falls back to the shard of the referenced ID, as there. A test in `ew-api`
/// pins the two together (`record_file_of`).
fn file_of(tx: &TxBuilder<'_>, record: &Record, depth: u8) -> Option<String> {
    let page = |id: AnyId| format!("pages/{id}.jsonl");
    let shard = |id: AnyId| format!("devices/{}.jsonl", id.shard_name());
    let layout = |id: AnyId| format!("layout/{id}.jsonl");
    let rep_page = |f: AnyId| -> Option<AnyId> {
        let rep = tx.reps_of_function(f.cast()).into_iter().min()?;
        tx.get::<Representation>(rep).map(|r| r.page.cast())
    };
    let ip_file = |f: AnyId| rep_page(f).map_or_else(|| shard(f), page);
    let function_file = |f: AnyId| match tx.get::<Function>(f.cast()) {
        Some(func) => func.device.map_or_else(|| ip_file(f), |d| shard(d.cast())),
        None => shard(f),
    };
    let junction_file = |j: AnyId| {
        tx.get::<Junction>(j.cast())
            .map_or_else(|| shard(j), |j| page(j.page.cast()))
    };
    let space_of_enclosure = |e: AnyId| tx.get::<Enclosure>(e.cast()).map_or(e, |e| e.space.cast());
    let space_of_surface = |s: AnyId| {
        tx.get::<MountingSurface>(s.cast())
            .map_or(s, |s| space_of_enclosure(s.enclosure.cast()))
    };
    let space_of_carrier = |c: AnyId| {
        tx.get::<Carrier>(c.cast())
            .map_or(c, |c| space_of_surface(c.surface.cast()))
    };
    let own = || record.id().map(AnyId::from_uuid);
    let endpoint = |anchor: AnchorRef| match anchor {
        AnchorRef::DeviceOfFunction(f) => Some(function_file(f.cast())),
        AnchorRef::PageOfJunction(j) => Some(junction_file(j.cast())),
        _ => None,
    };
    Some(match record {
        Record::Project(_)
        | Record::PlcConfigProject(_)
        | Record::LogicalNet(_)
        | Record::PhysicalNet(_)
        | Record::ImportLineage(_) => "project.jsonl".to_owned(),
        Record::StructureNode(_) | Record::Section(_) | Record::ExternalRef(_) => {
            "structure.jsonl".to_owned()
        }
        Record::Page(r) => page(r.id.cast()),
        Record::Device(r) => shard(r.id.cast()),
        Record::Function(r) => match r.device {
            Some(d) => shard(d.cast()),
            None => ip_file(r.id.cast()),
        },
        Record::Representation(r) => page(r.page.cast()),
        Record::Junction(r) => page(r.page.cast()),
        Record::PageItem(r) => page(r.page.cast()),
        Record::StructureBox(r) => page(r.page.cast()),
        Record::MacroInstance(r) => page(r.page.cast()),
        Record::Conductor(r) => shard(r.cable.cast()),
        Record::Channel(r) => shard(r.module.cast()),
        Record::DeviceLink(r) => shard(r.from.cast()),
        Record::ReportBlock(r) => match &r.embedded {
            Some(e) => page(e.page.cast()),
            None => "report-blocks.jsonl".to_owned(),
        },
        Record::Revision(_) => "revisions.jsonl".to_owned(),
        Record::Waiver(_) => "waivers.jsonl".to_owned(),
        Record::Connection(r) => match r.page {
            Some(p) => page(p.cast()),
            None => endpoint(record.anchor_ref()).or_else(|| own().map(shard))?,
        },
        Record::PotentialDef(_) | Record::Bond(_) => {
            endpoint(record.anchor_ref()).or_else(|| own().map(shard))?
        }
        Record::LayoutSpace(r) => layout(r.id.cast()),
        Record::Enclosure(r) => layout(r.space.cast()),
        Record::MountingSurface(r) => layout(space_of_enclosure(r.enclosure.cast())),
        Record::Carrier(r) => layout(space_of_surface(r.surface.cast())),
        Record::Placement(r) => layout(match &r.carrier {
            CarrierRef::Carrier(c) => space_of_carrier(c.cast()),
            CarrierRef::Surface(s) => space_of_surface(s.cast()),
            CarrierRef::Unknown(_) => r.id.cast(),
        }),
        Record::Conflict(c) => match &c.body {
            ConflictBody::File { path, .. } => path.clone(),
            ConflictBody::Record { kind, .. } => {
                let target = Kind::from_name(kind)
                    .filter(|k| *k != Kind::Conflict && depth == 0)
                    .and_then(|k| tx.row(&(k, c.id.cast())))
                    .map(|row| record_of(&row))
                    .transpose()
                    .ok()
                    .flatten();
                match target {
                    Some(t) => file_of(tx, &t, depth + 1)?,
                    None => shard(c.id.cast()),
                }
            }
        },
        Record::Unknown(_) => return None,
    })
}
