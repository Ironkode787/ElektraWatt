// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Migration of a project written before the revision kind was typed (FT-165; ADR-0011 amendment 4,
//! decision log ID-334: typing a reserved kind is additive within format 0.1, so there is no format or
//! `$v` bump). An older build kept the revisions as records of a reserved kind and wrote their members in
//! byte-wise key order; this build opens such a project for editing with typed revisions, reports the file
//! as not canonical (info), and its next save rewrites the file in the canonical member order.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use common::{fixture, mem_project, options, reopen};
use ew_store::ew_format::ew_schema::{Revision, RevisionState};
use ew_store::ew_format::{FindingKind, hash_text, sha256};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{Access, Changes, OpenMode, Opened, Store, Vfs, join};

/// The golden fixture's revisions as the cycle-16 build wrote them (an unknown kind: byte-wise key order).
const OLD: &str = concat!(
    r#"{"$k":"revision","date":"2025-03-03","description":"Initial issue","id":"4a41f813-2160-5abe-89f2-a3f019bf4268","label":"V1.0","source_ref":{"key":"revisions|V1.0","lineage":"763dd2f1-b9b8-558e-8d55-97d7304a4929"},"state":"released"}"#,
    "\n",
    r#"{"$k":"revision","author":"engineer","date":"2025-04-14","description":"Changes on 101 and 106","id":"31a45c26-4b99-565c-bbac-68077de2577c","label":"V2.0","page_notes":[{"page":"71260e59-ea02-5e16-8678-401d4044d9d7"},{"page":"b57668be-b65f-5802-a9dc-3ba4c8ed9866"}],"source_ref":{"key":"revisions|V2.0","lineage":"763dd2f1-b9b8-558e-8d55-97d7304a4929"},"state":"open"}"#,
    "\n",
);

#[test]
fn an_old_project_opens_with_typed_revisions_and_the_next_save_rewrites_them() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let canonical = files["revisions.jsonl"].clone();
    assert_ne!(canonical, OLD.as_bytes(), "the fixture is in the typed form");
    // The project as the older build saved it: its revisions file and the manifest's hash of it.
    let manifest = String::from_utf8(files["manifest.json"].clone()).expect("UTF-8");
    let manifest = manifest.replace(
        &hash_text(&sha256(&canonical)),
        &hash_text(&sha256(OLD.as_bytes())),
    );
    vfs.put(&join(&root, "revisions.jsonl"), OLD.as_bytes());
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
    assert_eq!(revisions.len(), 2);
    assert!(revisions.contains(&("V1.0".to_owned(), RevisionState::Released)));
    assert!(revisions.contains(&("V2.0".to_owned(), RevisionState::Open)));
    assert!(model.unknown_records().is_empty(), "no revision is kept untyped");

    // A save without an edit rewrites the non-canonical file: the canonical lines of the typed kind.
    store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("saves");
    let written = vfs
        .read(&join(&root, "revisions.jsonl"))
        .expect("the file exists");
    assert_eq!(written, canonical, "the file is in the typed canonical form");
    drop(store);

    let again = reopen(&vfs, 2);
    assert!(again.report.findings.is_empty(), "{:?}", again.report.findings);
    assert!(again.model.content_eq(&model));
}

/// A library copy of the golden project: a function definition that functions name.
const LIB_COPY: &str = "lib/fndefs/42cda040-6e1c-59c9-a7a6-051633364977.json";

/// The golden project's model, as it opens unchanged.
fn golden_model() -> ew_model::Model {
    let (vfs, root) = mem_project(DirSync::Explicit);
    Store::open(&root, options(&vfs, 1))
        .expect("the golden project opens")
        .model
}

/// The golden project with a BOM before [`LIB_COPY`] (an editor's). With `listed`, the manifest lists the copy
/// with the BOM (the shape the last save saw); without, it lists the canonical copy (an edit after the last save).
fn with_lib_bom(listed: bool) -> (Arc<MemVfs>, PathBuf) {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let canonical = files[LIB_COPY].clone();
    let mut bom = b"\xEF\xBB\xBF".to_vec();
    bom.extend_from_slice(&canonical);
    vfs.put(&join(&root, LIB_COPY), &bom);
    if listed {
        let manifest = String::from_utf8(files["manifest.json"].clone()).expect("UTF-8");
        let manifest = manifest.replace(&hash_text(&sha256(&canonical)), &hash_text(&sha256(&bom)));
        vfs.put(&join(&root, "manifest.json"), manifest.as_bytes());
    }
    (vfs, root)
}

/// The cargo-fuzz finding of cycle 22 (`fuzz/corpus/migration_chain/lib-document-bom`): a BOM before a library
/// copy made the copy a JSON syntax error, so the function naming it refused the open (EW-INV-001, a dangling
/// `def`). The BOM is a shape: the project opens for editing with the golden model, the save rewrites the copy
/// canonically and the next open has nothing to report.
#[test]
fn a_library_copy_with_a_bom_opens_and_the_next_save_rewrites_it() {
    let golden = golden_model();
    let (vfs, root) = with_lib_bom(true);
    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    let kinds: Vec<FindingKind> = report.findings.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [FindingKind::NonCanonical], "{:?}", report.findings);
    assert!(model.content_eq(&golden), "the BOM changed the model");

    let saved = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("saves");
    assert_eq!(saved.written.len(), 1, "{:?}", saved.written);
    let written = vfs.read(&join(&root, LIB_COPY)).expect("the copy exists");
    assert_eq!(written, fixture()[LIB_COPY], "the copy is in its canonical form");
    drop(store);

    let again = reopen(&vfs, 2);
    assert!(again.report.findings.is_empty(), "{:?}", again.report.findings);
    assert!(again.model.content_eq(&golden));
}

/// The same BOM as an edit after the last save, opened read-only (the mode of the fuzz input that found it): the
/// project opens with the golden model.
#[test]
fn a_library_copy_with_a_bom_after_the_last_save_opens_read_only() {
    let golden = golden_model();
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
    assert!(opened.model.content_eq(&golden), "the BOM changed the model");
}
