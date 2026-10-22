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

use common::{fixture, mem_project, options, reopen};
use ew_store::ew_format::ew_schema::{Revision, RevisionState};
use ew_store::ew_format::{FindingKind, hash_text, sha256};
use ew_store::mem::DirSync;
use ew_store::{Access, Changes, Opened, Store, Vfs, join};

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
