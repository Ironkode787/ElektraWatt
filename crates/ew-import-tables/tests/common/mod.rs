// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the integration tests: the built-in library from `content/`, table inputs from memory
//! or from `golden/J-E-2416/tables/`, and a default import.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use ew_ids::Id;
use ew_import_tables::{BuiltinLibrary, TablesImporter};
use ew_interop_api::{ImportBatch, ImportContext, ImportError, ImportInput};
use ew_schema::LibDocument;

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed golden tables directory.
pub fn tables_dir() -> PathBuf {
    repo().join("golden/J-E-2416/tables")
}

/// Every `*.json` library document of `content/<sub>` (not recursive).
pub fn content_docs(sub: &str) -> Vec<LibDocument> {
    let dir = repo().join("content").join(sub);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading content/{sub}: {e}"))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap();
            LibDocument::from_json(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        })
        .collect()
}

/// The built-in library of the golden project: the `@qet10` symbols, the FnDefs and the forms. Read from
/// `content/` once per test binary; every call returns a copy.
pub fn library() -> BuiltinLibrary {
    static LIBRARY: OnceLock<BuiltinLibrary> = OnceLock::new();
    LIBRARY
        .get_or_init(|| {
            let mut docs = content_docs("symbols/qet10");
            docs.extend(content_docs("fndefs"));
            docs.extend(content_docs("forms"));
            BuiltinLibrary::new(docs).expect("the starter content is a valid built-in library")
        })
        .clone()
}

/// Inputs from `(file name, text)` pairs.
pub fn inputs(files: &[(&str, &str)]) -> Vec<ImportInput> {
    files
        .iter()
        .map(|(n, t)| ImportInput::new(n, t.as_bytes().to_vec()).unwrap())
        .collect()
}

/// The committed golden tables as inputs, in file-name order.
pub fn golden_inputs() -> Vec<ImportInput> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(tables_dir())
        .expect("golden/J-E-2416/tables exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "csv"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            ImportInput::new(&name, std::fs::read(p).unwrap()).unwrap()
        })
        .collect()
}

/// A default context (the project ID is not used by the bootstrap).
pub fn context() -> ImportContext {
    ImportContext::new(Id::from_uuid(ew_ids::Uuid::nil()))
}

/// Imports `inputs` with the golden library and the default context.
pub fn import(inputs: &[ImportInput]) -> Result<ImportBatch, ImportError> {
    TablesImporter::new(library()).import(inputs, &context())
}

/// The minimal valid project table.
pub const PROJECT_CSV: &str = "key,derivation_profile\nJ-E-2416,2027.1\n";
