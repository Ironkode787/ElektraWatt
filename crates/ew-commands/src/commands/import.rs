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
}

impl ApplyImport {
    /// The command name.
    pub const NAME: &'static str = "import.apply";
}

/// The hash of D11: the first 128 bits of SHA-256 over a row's JSON form without its `source_ref`, as 32 lower-case
/// hex digits.
///
/// # Errors
///
/// [`CmdError::Internal`] for a row without a JSON form.
pub fn record_hash(row: &AnyRow) -> Result<String, CmdError> {
    use sha2::Digest;
    let mut value = crate::rows::row_json(row)?;
    if let Value::Object(map) = &mut value {
        map.remove("source_ref");
    }
    let digest = sha2::Sha256::digest(value.to_string().as_bytes());
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
        for (i, value) in self.lib.iter().enumerate() {
            let (doc, _) = super::library::read_copy(Self::NAME, &format!("/lib/{i}"), value)?;
            match doc {
                LibDocument::Part(c) => tx.put(c),
                LibDocument::Symbol(c) => tx.put(c),
                LibDocument::Fndef(c) => tx.put(c),
                LibDocument::Form(c) => tx.put(c),
                LibDocument::Macro(c) => tx.put(c),
                LibDocument::Manufacturer(c) => tx.put(c),
                LibDocument::Unknown(_) => {}
            }
        }
        let run_id: Id<ImportRun> = tx.mint();
        let mut run = self.run.clone();
        run.id = run_id;
        let mut lineage_written = false;
        for record in &self.records {
            if hashed(record) {
                let row = AnyRow::from_record(record.clone()).map_err(|r| {
                    CmdError::rejected(
                        Self::NAME,
                        format!("a record of an unknown kind ({:?})", r.kind()),
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
                    format!("a normaliser removed the imported {} {id}", kind.name()),
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
