// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden CSV bootstrap as a session, shared by the report tests (the same loading as
//! `tests/golden_csv.rs`).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use ew_api::{Session, SessionOptions};
use ew_import_tables::ew_interop_api::{ImportBatch, ImportInput};
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use ew_schema::{LibDocument, Record};

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    paths.sort();
    paths
}

fn library() -> BuiltinLibrary {
    let mut docs = Vec::new();
    for sub in ["symbols/qet10", "fndefs", "forms"] {
        for p in json_files(&repo().join("content").join(sub), "json") {
            docs.push(LibDocument::from_json(&std::fs::read_to_string(&p).unwrap()).unwrap());
        }
    }
    BuiltinLibrary::new(docs).unwrap()
}

/// The imported golden batch (doc 20 §3.2 path (a)).
pub fn golden_batch() -> ImportBatch {
    let inputs: Vec<ImportInput> = json_files(&repo().join("golden/J-E-2416/tables"), "csv")
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            ImportInput::new(&name, std::fs::read(p).unwrap()).unwrap()
        })
        .collect();
    TablesImporter::new(library())
        .import(&inputs, &bootstrap_context())
        .expect("the golden tables import")
}

/// The project and the other records of a batch.
pub fn split(batch: &ImportBatch) -> (ew_schema::Project, Vec<Record>) {
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    (project.expect("the batch holds the project"), records)
}

/// A session over records (in the given order).
pub fn session_of(project: ew_schema::Project, records: Vec<Record>, lib: Vec<LibDocument>) -> Session {
    Session::from_records(project, records, lib, SessionOptions::default())
        .expect("the golden batch passes the model's integrity check")
}

/// A session over the golden batch.
pub fn golden_session() -> Session {
    let batch = golden_batch();
    let (project, records) = split(&batch);
    session_of(project, records, batch.lib.clone())
}

/// The `{table}|{key}` source key (`report_blocks|901`, `devices|-UC1`) of an imported record.
pub fn source_key(r: &Record) -> Option<&str> {
    let s = match r {
        Record::Representation(x) => x.source_ref.as_ref(),
        Record::Connection(x) => x.source_ref.as_ref(),
        Record::Device(x) => x.source_ref.as_ref(),
        Record::Function(x) => x.source_ref.as_ref(),
        Record::ReportBlock(x) => x.source_ref.as_ref(),
        Record::Page(x) => x.source_ref.as_ref(),
        _ => None,
    }?;
    Some(s.key.as_str())
}
