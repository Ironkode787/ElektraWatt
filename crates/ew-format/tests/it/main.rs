// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The integration tests of `ew-format`, one test binary: each module is a former test file.

use std::collections::BTreeMap;

use ew_format::ew_schema::{LibDocument, Project, Record};
use ew_format::{EntryKind, FilePath, Limits, Loaded, Manifest, load, plan, project_files};

// Every other file of tests/it is a module (automod 1.0, sorted by name).
automod::dir!("tests/it");

/// The project record the modules write their directories with.
const PROJECT: &str =
    r#"{"$k":"project","id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

fn project() -> Project {
    match Record::from_json(PROJECT).unwrap() {
        Record::Project(project) => project,
        other => panic!("not a project: {other:?}"),
    }
}

/// The directory entries of `files`, every one a regular file.
fn entries_of(files: &BTreeMap<FilePath, Vec<u8>>) -> BTreeMap<String, EntryKind> {
    files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect()
}

/// The files of `records` and `lib`, read back through the plan and the loader with their manifest.
fn write_and_read(records: &[Record], lib: &[LibDocument]) -> (BTreeMap<FilePath, Vec<u8>>, Loaded) {
    let files = project_files(&project(), records, lib, 1, "test").unwrap();
    let manifest = Manifest::parse(
        &files[&FilePath::new("manifest.json").unwrap()],
        &Limits::default(),
    )
    .unwrap();
    let reads = plan(Some(&manifest), &entries_of(&files), &Limits::default());
    assert!(reads.findings.is_empty(), "{:?}", reads.findings);
    let input: BTreeMap<FilePath, Vec<u8>> = reads
        .read
        .iter()
        .map(|path| (path.clone(), files[path].clone()))
        .collect();
    let loaded = load(Some(&manifest), &input, &Limits::default());
    (files, loaded)
}
