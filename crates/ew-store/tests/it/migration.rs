// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Migration of a project written before the revision kind was typed (ADR-0011 amendment 4,
//! Typing a reserved kind is additive within format 0.1, so there is no format or
//! `$v` bump). An older build kept the revisions as records of a reserved kind and wrote their members in
//! byte-wise key order; this build opens such a project for editing with typed revisions, reports the file
//! as not canonical (info), and its next save rewrites the file in the canonical member order.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use common::{fixture, mem_project, options, reopen};
use ew_schema::{Revision, RevisionState};
use ew_format::{FindingKind, hash_text, sha256};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{Access, Changes, OpenMode, Opened, Store, Vfs, join};

/// The sample's revisions with a date and a description, as an earlier build wrote them (an unknown kind: byte-wise
/// key order, which serde_json's maps keep), and as this build writes them (the typed kind's member order).
fn old_and_canonical(sample: &[u8]) -> (String, String) {
    let (mut old, mut canonical) = (String::new(), String::new());
    for line in std::str::from_utf8(sample).expect("UTF-8").lines() {
        let mut value: serde_json::Value = serde_json::from_str(line).expect("JSON");
        value["date"] = "2025-03-03".into();
        value["description"] = "Initial issue".into();
        old.push_str(&format!("{value}\n"));
        let record = ew_schema::Record::from_value(value).expect("a revision");
        canonical.push_str(&format!(
            "{}\n",
            ew_format::record_line(&record).expect("a line")
        ));
    }
    (old, canonical)
}

#[test]
fn an_old_project_opens_with_typed_revisions_and_the_next_save_rewrites_them() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let (old, canonical) = old_and_canonical(&files["revisions.jsonl"]);
    assert_ne!(canonical, old, "the typed form differs");
    // The project as the older build saved it: its revisions file and the manifest's hash of it.
    let manifest = String::from_utf8(files["manifest.json"].clone()).expect("UTF-8");
    let manifest = manifest.replace(
        &hash_text(&sha256(&files["revisions.jsonl"])),
        &hash_text(&sha256(old.as_bytes())),
    );
    vfs.put(&join(&root, "revisions.jsonl"), old.as_bytes());
    vfs.put(&join(&root, "manifest.json"), manifest.as_bytes());

    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    let kinds: Vec<FindingKind> = report.findings.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [FindingKind::NonCanonical], "{:?}", report.findings);
    let revisions: Vec<(String, RevisionState)> = model
        .table::<Revision>()
        .iter()
        .map(|(_, r)| (r.label.as_str().to_owned(), r.state.clone()))
        .collect();
    // The sample's one revision, open (its script releases none).
    assert_eq!(revisions, [("V1.0".to_owned(), RevisionState::Open)]);
    assert!(model.unknown_records().is_empty(), "no revision is kept untyped");

    // A save without an edit rewrites the non-canonical file: the canonical lines of the typed kind.
    store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("saves");
    let written = vfs
        .read(&join(&root, "revisions.jsonl"))
        .expect("the file exists");
    assert_eq!(
        String::from_utf8(written).expect("UTF-8"),
        canonical,
        "the file is in the typed canonical form"
    );
    drop(store);

    let again = reopen(&vfs, 2);
    assert!(again.report.findings.is_empty(), "{:?}", again.report.findings);
    assert!(again.model.content_eq(&model));
}

/// A library copy of the sample: its first function definition, which functions name.
fn lib_copy() -> String {
    fixture()
        .into_keys()
        .find(|name| name.starts_with("lib/fndefs/"))
        .expect("a function definition copy")
}

/// The sample's model, as it opens unchanged.
fn sample_model() -> ew_model::Model {
    let (vfs, root) = mem_project(DirSync::Explicit);
    Store::open(&root, options(&vfs, 1))
        .expect("the sample opens")
        .model
}

/// The sample with a BOM before [`lib_copy`] (an editor's). With `listed`, the manifest lists the copy
/// with the BOM (the shape the last save saw); without, it lists the canonical copy (an edit after the last save).
fn with_lib_bom(listed: bool) -> (Arc<MemVfs>, PathBuf) {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let canonical = files[&lib_copy()].clone();
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend_from_slice(&canonical);
    vfs.put(&join(&root, &lib_copy()), &bom);
    if listed {
        let manifest = String::from_utf8(files["manifest.json"].clone()).expect("UTF-8");
        let manifest = manifest.replace(&hash_text(&sha256(&canonical)), &hash_text(&sha256(&bom)));
        vfs.put(&join(&root, "manifest.json"), manifest.as_bytes());
    }
    (vfs, root)
}

/// The cargo-fuzz finding (`fuzz/corpus/migration_chain/lib-document-bom`): a BOM before a library
/// copy made the copy a JSON syntax error, so the function naming it refused the open (EW-INV-001, a dangling
/// `def`). The BOM is a shape: the project opens for editing with the sample's model, the save rewrites the copy
/// canonically and the next open has nothing to report.
#[test]
fn a_library_copy_with_a_bom_opens_and_the_next_save_rewrites_it() {
    let sample = sample_model();
    let (vfs, root) = with_lib_bom(true);
    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    let kinds: Vec<FindingKind> = report.findings.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [FindingKind::NonCanonical], "{:?}", report.findings);
    assert!(model.content_eq(&sample), "the BOM changed the model");

    let saved = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("saves");
    assert_eq!(saved.written.len(), 1, "{:?}", saved.written);
    let written = vfs.read(&join(&root, &lib_copy())).expect("the copy exists");
    assert_eq!(
        written,
        fixture()[&lib_copy()],
        "the copy is in its canonical form"
    );
    drop(store);

    let again = reopen(&vfs, 2);
    assert!(again.report.findings.is_empty(), "{:?}", again.report.findings);
    assert!(again.model.content_eq(&sample));
}

/// The same BOM as an edit after the last save, opened read-only (the mode of the fuzz input that found it): the
/// project opens with the sample's model.
#[test]
fn a_library_copy_with_a_bom_after_the_last_save_opens_read_only() {
    let sample = sample_model();
    let (vfs, root) = with_lib_bom(false);
    let mut o = options(&vfs, 1);
    o.mode = OpenMode::ReadOnly;
    let opened = Store::open(&root, o).expect("opens");
    let mut kinds: Vec<FindingKind> = opened.report.findings.iter().map(|f| f.kind).collect();
    kinds.sort_by_key(|k| format!("{k:?}"));
    assert_eq!(
        kinds,
        [FindingKind::ForeignEdit, FindingKind::NonCanonical],
        "{:?}",
        opened.report.findings
    );
    assert!(opened.model.content_eq(&sample), "the BOM changed the model");
}
