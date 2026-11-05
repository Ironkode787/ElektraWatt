// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `import.apply` (`ApplyImport`, WP-S3.09 D7, D11): an import batch as one transaction.
//!
//! The command is internal to the import front door: it is not in the command registry, so `ew apply`, command logs
//! and the command schemas never see it; the host builds it from a batch the front door checked and previews it from
//! the origin `import`. On the fork it embeds the batch's library documents, checked as `library.embed` checks a copy, puts the
//! records, writes the lineage (`last_source_sha256`, `last_run`) and the [`ImportRun`] with its findings and the
//! `source` hash of every imported record; the normalisers run, and then the `ew` hash of every record they changed is
//! added (D11). The prevent hook does not judge the transaction, so a prevent rule's violation stays in the model for
//! the lenient findings (doc 14 §3.3); the structural invariants hold, and a normaliser that removes an imported
//! record refuses the import. Into an empty project it is a non-undoable checkpoint (ADR-0021 decision 3); into a
//! lineage of an existing project one undoable step.

use std::collections::BTreeMap;

use ew_ids::Id;
use ew_model::{AnyKey, AnyRow};
use ew_schema::{AnyId, ImportLineage, ImportRun, Kind, LibDocument, Record, RecordHash, Sha256};
use serde_json::Value;

use crate::command::{DynCommand, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost, Exposure, Stability};
use crate::text::{TextArg, text};
use crate::tx::TxBuilder;

/// `import.apply` (`ApplyImport`): see the module documentation.
#[derive(Clone, Debug)]
pub struct ApplyImport {
    /// The batch's records. Its project record becomes the project's only into an empty project (`checkpoint`).
    pub records: Vec<Record>,
    /// The batch's library documents, in their `lib/` JSON form.
    pub lib: Vec<Value>,
    /// The run as the host made it from the report; its `id` is minted here and its `record_hashes` written here.
    pub run: ImportRun,
    /// Whether the import makes an empty project's first state: a non-undoable checkpoint.
    pub checkpoint: bool,
    /// The hashes a re-import carries from its base for the records it keeps ([`ReimportPlan::carried`], D12); empty
    /// for an import into a new project.
    pub carried: BTreeMap<AnyId, RecordHash>,
}

impl ApplyImport {
    /// The command name.
    pub const NAME: &'static str = "import.apply";
}

// -------------------------------------------------------------------------------------------------------------------
// Re-import (WP-S3.09 D10 to D12)

/// The class of one record of a re-import (D12), in the order the card lists them.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ReimportClass {
    /// Not in the base: applied.
    Added,
    /// Changed in the source only: the source's record is applied.
    SourceChanged,
    /// Changed on both sides: this project's record is kept (EW-IMP-011).
    Conflict,
    /// In the base, not in the source: kept (EW-IMP-010).
    Removed,
    /// Changed in this project only, or deleted here: kept.
    EwChanged,
    /// Equal on both sides.
    Unchanged,
}

/// One member of a record changed on both sides: its path (`diff_model`'s dotted path), this project's value and the
/// source's (`None` where the member is absent).
#[derive(Clone, PartialEq, Debug)]
pub struct MemberChange {
    /// The member's dotted path.
    pub path: String,
    /// This project's value.
    pub ours: Option<Value>,
    /// The source's value.
    pub theirs: Option<Value>,
}

/// One record of a re-import.
#[derive(Clone, PartialEq, Debug)]
pub struct ReimportRow {
    /// Its class.
    pub class: ReimportClass,
    /// Its key.
    pub key: AnyKey,
    /// The record in words (its tag, folio or label, as `words` names it), keyed for the window's language.
    pub record: TextArg,
    /// For a conflict, the members that differ between this project and the source.
    pub members: Vec<MemberChange>,
}

/// What a re-import of a batch into its lineage does (D12): every record of the batch and of the base classed, the
/// records to apply, the base hashes of the records kept and the run entries of the conflicts and removals.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ReimportPlan {
    /// Every record of the batch and of the base, in class order, then key order.
    pub rows: Vec<ReimportRow>,
    /// The records applied: added and changed in the source.
    pub apply: Vec<Record>,
    /// The hashes the new run carries for the records kept (unchanged, kept with this project's changes, conflicts);
    /// a record removed in the source carries none (ID-1392).
    pub carried: BTreeMap<AnyId, RecordHash>,
    /// The EW-IMP-011 entries of the conflicts and the EW-IMP-010 entries of the removals, in that order.
    pub entries: Vec<ew_schema::RunEntry>,
}

impl ReimportPlan {
    /// The number of records of `class`.
    pub fn count(&self, class: ReimportClass) -> usize {
        self.rows.iter().filter(|r| r.class == class).count()
    }

    /// Whether Apply writes anything: a record to apply, a conflict or a removal to record. A re-import of the same
    /// source writes nothing (IC-12).
    pub fn writes(&self) -> bool {
        self.rows
            .iter()
            .any(|r| !matches!(r.class, ReimportClass::Unchanged | ReimportClass::EwChanged))
    }
}

/// The source keys of the records of `model` whose `source_ref` names the lineage `lineage` (D10's share of the
/// batch's source keys).
///
/// # Errors
///
/// [`CmdError::Internal`] for a record without a JSON form.
pub fn lineage_keys(
    model: &ew_model::Model,
    lineage: Id<ImportLineage>,
) -> Result<std::collections::BTreeSet<String>, CmdError> {
    let id = lineage.to_string();
    let mut keys = std::collections::BTreeSet::new();
    for key in model.keys() {
        let Some(row) = model.row(&key) else {
            continue;
        };
        let value = crate::rows::row_json(&row)?;
        let Some(source) = value.get("source_ref") else {
            continue;
        };
        if source.get("lineage").and_then(Value::as_str) == Some(id.as_str()) {
            if let Some(k) = source.get("key").and_then(Value::as_str) {
                keys.insert(k.to_owned());
            }
        }
    }
    Ok(keys)
}

/// Names the added records of `rows` as `model` holds them: the re-import's candidate after the normalisers, which
/// number an added terminal (`-X1:3`). A changed record keeps the name this project gives it (ID-1389).
pub fn name_rows(model: &ew_model::Model, rows: &mut [ReimportRow]) {
    let mut ids = ew_ids::SystemSource;
    let tx = TxBuilder::new(model, &mut ids);
    for row in rows.iter_mut().filter(|r| r.class == ReimportClass::Added) {
        row.record = crate::words::record(&tx, row.key);
    }
}

/// A member value of a re-import row as text: a text as it is, a text in several languages as its texts (`Pumps /
/// Pompen`), any other value as JSON, an absent one as `-`.
pub fn member_text(value: Option<&Value>) -> String {
    match value {
        None => "-".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(m)) if !m.is_empty() && m.values().all(Value::is_string) => m
            .values()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" / "),
        Some(other) => other.to_string(),
    }
}

/// The value at a dotted path of a JSON object.
fn at_path<'v>(value: &'v Value, path: &str) -> Option<&'v Value> {
    path.split('.').try_fold(value, |v, k| v.get(k))
}

/// A run entry of a re-import about the record `row`: its item the record in words, so that the finding's subject is
/// the record.
fn reimport_entry(code: &str, row: &ReimportRow, message: String, file: &str) -> ew_schema::RunEntry {
    let key = row.key;
    ew_schema::RunEntry {
        code: code.to_owned(),
        severity: ew_schema::RunSeverity::Warning,
        defect: None,
        message,
        location: Some(ew_schema::RunLocation {
            file: file.to_owned(),
            item: Some(row.record.to_string()),
            ..Default::default()
        }),
        targets: vec![ew_schema::RunTarget {
            kind: key.0.name().to_owned(),
            id: key.1,
            extra: Default::default(),
        }],
        fix: None,
        extra: Default::default(),
    }
}

/// The re-import of `records` (a batch imported into a lineage of `model`) against `base`, the lineage's last run
/// (D11, D12). Per record, with "current" the hash of the record in `model` now and the base's `ew` hash (else its
/// `source` hash) the record as committed:
/// - **unchanged**: the source hash is the base's and current is the committed one;
/// - **changed in the source**: the source hash differs, current is the committed one: applied;
/// - **kept with this project's changes**: the source hash is the base's, current differs, or the record was deleted
///   here (it stays deleted);
/// - **conflict**: both differ: kept, with an EW-IMP-011 entry (T10);
/// - **added**: not in the base: applied; a record of that ID already in `model` is kept as unchanged when equal,
///   else as a conflict;
/// - **removed**: in the base and in `model`, not in the batch: kept, with an EW-IMP-010 entry (T9).
///
/// The project record and the lineage of the batch are not classed. `file` names the source in the entries.
///
/// # Errors
///
/// [`CmdError::Internal`] for a record without a JSON form.
pub fn plan_reimport(
    model: &ew_model::Model,
    records: &[Record],
    base: Option<&ImportRun>,
    file: &str,
) -> Result<ReimportPlan, CmdError> {
    let mut ids = ew_ids::SystemSource;
    let mut tx = TxBuilder::new(model, &mut ids);
    let mut plan = ReimportPlan::default();
    let mut batch_ids = std::collections::BTreeSet::new();
    let without_ref = |row: &AnyRow| -> Result<Value, CmdError> {
        let mut v = crate::rows::row_json(row)?;
        if let Value::Object(map) = &mut v {
            map.remove("source_ref");
        }
        Ok(v)
    };
    let mut rows = Vec::new();
    for record in records.iter().filter(|r| hashed(r)) {
        let row = AnyRow::from_record(record.clone())
            .map_err(|r| CmdError::internal(format!("a record of an unknown kind ({:?})", r.kind())))?;
        let key = row.key();
        batch_ids.insert(key.1);
        let source = record_hash(&row)?;
        let current = tx.row(&key);
        let now = current.as_ref().map(record_hash).transpose()?;
        let based = base.and_then(|b| b.record_hashes.get(&key.1));
        let class = match (based, &now) {
            (None, None) => ReimportClass::Added,
            (None, Some(now)) if *now == source => ReimportClass::Unchanged,
            (None, Some(_)) => ReimportClass::Conflict,
            (Some(_), None) => ReimportClass::EwChanged,
            (Some(b), Some(now)) => {
                let committed = b.ew.as_ref().unwrap_or(&b.source);
                match (b.source == source, now == committed) {
                    (true, true) => ReimportClass::Unchanged,
                    (false, true) => ReimportClass::SourceChanged,
                    (true, false) => ReimportClass::EwChanged,
                    (false, false) => ReimportClass::Conflict,
                }
            }
        };
        let mut members = Vec::new();
        match class {
            ReimportClass::Added | ReimportClass::SourceChanged => plan.apply.push(record.clone()),
            ReimportClass::Conflict => {
                // The base's committed hash stays the base of the next re-import, so this project's version stays
                // "changed here" (D11): the new source becomes the source hash.
                let committed =
                    based.map_or_else(|| source.clone(), |b| b.ew.clone().unwrap_or(b.source.clone()));
                plan.carried.insert(
                    key.1,
                    RecordHash {
                        ew: (committed != source).then_some(committed),
                        source,
                        extra: Default::default(),
                    },
                );
                if let Some(ours) = &current {
                    let (ours, theirs) = (without_ref(ours)?, without_ref(&row)?);
                    let mut paths = Vec::new();
                    crate::diff::field_paths("", &ours, &theirs, &mut paths);
                    members = paths
                        .into_iter()
                        .map(|path| MemberChange {
                            ours: at_path(&ours, &path).cloned(),
                            theirs: at_path(&theirs, &path).cloned(),
                            path,
                        })
                        .collect();
                }
            }
            ReimportClass::EwChanged | ReimportClass::Unchanged | ReimportClass::Removed => {
                if let Some(b) = based {
                    plan.carried.insert(key.1, b.clone());
                } else {
                    plan.carried.insert(
                        key.1,
                        RecordHash {
                            source,
                            ew: None,
                            extra: Default::default(),
                        },
                    );
                }
            }
        }
        rows.push(ReimportRow {
            class,
            key,
            record: TextArg::Plain(String::new()),
            members,
        });
    }
    // ID-1389 (W51 finding C): a record this project holds is named as it is here (a device by its tag, not the
    // source's new label); an added record with the records applied pending, and [`name_rows`] names it as committed.
    for row in rows.iter_mut().filter(|r| r.class != ReimportClass::Added) {
        row.record = crate::words::record(&tx, row.key);
    }
    for record in &plan.apply {
        if let Ok(row) = AnyRow::from_record(record.clone()) {
            tx.put_row(row);
        }
    }
    for row in rows.iter_mut().filter(|r| r.class == ReimportClass::Added) {
        row.record = crate::words::record(&tx, row.key);
    }
    if let Some(base) = base {
        let removed: std::collections::BTreeSet<AnyId> = base
            .record_hashes
            .keys()
            .filter(|id| !batch_ids.contains(id))
            .copied()
            .collect();
        if !removed.is_empty() {
            // ID-1392 (FT-1384): a removed record carries no base hash, so the next run's base has the source's
            // state of it (absent) and the same file again finds nothing to do.
            for key in model.keys().into_iter().filter(|k| removed.contains(&k.1)) {
                rows.push(ReimportRow {
                    class: ReimportClass::Removed,
                    key,
                    record: crate::words::record(&tx, key),
                    members: Vec::new(),
                });
            }
        }
    }
    rows.sort_by_key(|a| (a.class, a.key));
    for row in &rows {
        let entry = match row.class {
            ReimportClass::Conflict => reimport_entry(
                "EW-IMP-011",
                row,
                format!(
                    "{} was changed both in {file} and here; this project's version is kept.",
                    row.record
                ),
                file,
            ),
            ReimportClass::Removed => reimport_entry(
                "EW-IMP-010",
                row,
                format!(
                    "{} was removed in {file}; it is kept here. Delete it if it is no longer needed.",
                    row.record
                ),
                file,
            ),
            _ => continue,
        };
        plan.entries.push(entry);
    }
    plan.rows = rows;
    Ok(plan)
}

/// The hash of D11: the first 128 bits of SHA-256 over the record's canonical line without its `source_ref` member
/// (doc 05 §3.1: `$k` first, then the members in declaration order; ID-1374), as 32 lower-case hex digits.
///
/// # Errors
///
/// [`CmdError::Internal`] for a row without a JSON form.
pub fn record_hash(row: &AnyRow) -> Result<String, CmdError> {
    use sha2::Digest;
    let (kind, id) = row.key();
    let mut value = crate::rows::row_json(row)?;
    if let Value::Object(map) = &mut value {
        map.remove("source_ref");
        map.insert("$k".to_owned(), Value::from(kind.name()));
    }
    let line = Record::from_value(value)
        .ok()
        .and_then(|r| serde_json::to_string(&r).ok())
        .ok_or_else(|| CmdError::internal(format!("the record {kind}:{id} has no canonical line")))?;
    let digest = sha2::Sha256::digest(line.as_bytes());
    Ok(digest[..16].iter().map(|b| format!("{b:02x}")).collect())
}

/// The imported records whose hashes a run keeps: every record of the batch but the lineage.
fn hashed(record: &Record) -> bool {
    !matches!(record, Record::Project(_) | Record::ImportLineage(_))
}

impl DynCommand for ApplyImport {
    fn name(&self) -> &'static str {
        Self::NAME
    }

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            cost: Cost::Bulk,
            undoable: !self.checkpoint,
            exposure: Exposure::Import,
            stability: Stability::Internal,
            ..CommandMeta::edit(Self::NAME)
        }
    }

    fn cost(&self, _model: &ew_model::Model) -> Cost {
        Cost::Bulk
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        // The batch's library documents are checked as `library.embed` checks a copy (its identity and its hash),
        // without the library rule: the import's own library is project-local (doc 05 §13.2).
        // A re-import keeps the copies the project holds (D12: this project's version is kept).
        fn put_copy<T: ew_model::Entity>(tx: &mut TxBuilder<'_>, copy: T, keep_held: bool) {
            if !keep_held || !tx.exists::<T>(copy.id()) {
                tx.put(copy);
            }
        }
        let keep = !self.checkpoint;
        for (i, value) in self.lib.iter().enumerate() {
            let (doc, _) = super::library::read_copy(Self::NAME, &format!("/lib/{i}"), value)?;
            match doc {
                LibDocument::Part(c) => put_copy(tx, c, keep),
                LibDocument::Symbol(c) => put_copy(tx, c, keep),
                LibDocument::Fndef(c) => put_copy(tx, c, keep),
                LibDocument::Form(c) => put_copy(tx, c, keep),
                LibDocument::Macro(c) => put_copy(tx, c, keep),
                LibDocument::Manufacturer(c) => put_copy(tx, c, keep),
                LibDocument::PlcProfile(c) => put_copy(tx, c, keep),
                LibDocument::Unknown(_) => {}
            }
        }
        let run_id: Id<ImportRun> = tx.mint();
        let mut run = self.run.clone();
        run.id = run_id;
        run.record_hashes.extend(self.carried.clone());
        let mut lineage_written = false;
        for record in &self.records {
            if hashed(record) {
                let row = AnyRow::from_record(record.clone()).map_err(|r| {
                    CmdError::rejected(
                        Self::NAME,
                        text!("refusal-import-unknown-kind", format!("{:?}", r.kind())),
                    )
                })?;
                let source = record_hash(&row)?;
                run.record_hashes.insert(
                    row.key().1,
                    RecordHash {
                        source,
                        ew: None,
                        extra: Default::default(),
                    },
                );
                tx.put_row(row);
            } else if let Record::Project(project) = record {
                if self.checkpoint {
                    tx.set_project(project.clone())?;
                }
            } else if let Record::ImportLineage(lineage) = record {
                let mut lineage = lineage.clone();
                lineage.last_source_sha256 = Some(run.source.sha256);
                lineage.last_run = Some(run_id);
                lineage_written = true;
                tx.put(lineage);
            }
        }
        if !lineage_written {
            let id: Id<ImportLineage> = run.lineage;
            let sha: Sha256 = run.source.sha256;
            tx.update::<ImportLineage>(id, |l| {
                l.last_source_sha256 = Some(sha);
                l.last_run = Some(run_id);
            })?;
        }
        tx.put(run);
        Ok(Outcome::default())
    }

    fn after_normalise(&self, tx: &mut TxBuilder<'_>) -> Result<(), CmdError> {
        let Some(run) = tx
            .iter::<ImportRun>()
            .into_iter()
            .find(|r| tx.is_new(&(Kind::ImportRun, r.id.cast::<()>())))
        else {
            return Ok(());
        };
        let mut run = (*run).clone();
        let mut changed: BTreeMap<AnyId, String> = BTreeMap::new();
        for record in self.records.iter().filter(|r| hashed(r)) {
            let (Some(kind), Some(id)) = (record.kind(), record.id()) else {
                continue;
            };
            let key: AnyKey = (kind, Id::from_uuid(id));
            let Some(row) = tx.row(&key) else {
                return Err(CmdError::rejected(
                    Self::NAME,
                    text!("refusal-import-removed", kind.name(), id.to_string()),
                ));
            };
            let now = record_hash(&row)?;
            if run.record_hashes.get(&key.1).is_some_and(|h| h.source != now) {
                changed.insert(key.1, now);
            }
        }
        for (id, ew) in changed {
            if let Some(h) = run.record_hashes.get_mut(&id) {
                h.ew = Some(ew);
            }
        }
        tx.put(run);
        Ok(())
    }

    fn prevent(&self) -> bool {
        false
    }

    fn to_json(&self) -> Result<Value, CmdError> {
        Ok(serde_json::json!({
            "lineage": self.run.lineage.to_string(),
            "source": self.run.source.name,
            "records": self.records.len(),
            "checkpoint": self.checkpoint,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ID-1374 (C51-p2#4): D11's hash is taken over the record's canonical line without its `source_ref` member: `$k`
    /// first, the members in declaration order.
    #[test]
    fn the_record_hash_is_over_the_canonical_line_without_source_ref() {
        use sha2::Digest;
        let line = r#"{"$k":"device","id":"c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2","tag":{"letter":"G","counter":1},"kind":"psu","function_text":{"en-GB":"Power supply"}}"#;
        let with_ref = line.replace(
            r#""kind":"psu","#,
            r#""kind":"psu","source_ref":{"lineage":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","key":"k"},"#,
        );
        let record = Record::from_json(&with_ref).expect("the device reads");
        let row = AnyRow::from_record(record).expect("a known kind");
        let digest = sha2::Sha256::digest(line.as_bytes());
        let expected: String = digest[..16].iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(record_hash(&row).unwrap(), expected);
    }
}
