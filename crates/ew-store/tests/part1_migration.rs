// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 1, acceptance L1-3 through the store (ADR-0011 amendment 4, decision log ID-334: typing a
//! member is additive within format 0.1). A build before part 1 kept a part copy's `mech` and `alternatives`
//! in the part's `extra`, wrote them after the typed members in byte-wise order and took the copy hash over
//! that text. This build opens such a project for editing with the members typed, reports the copy as not
//! canonical (info) and its stored hash by EW-CORE-014, and its next save rewrites the copy canonically; the
//! stored hash stays (accepted within 0.1), so EW-CORE-014 is all the reopened project reports.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::BTreeSet;

use common::{fixture, mem_project, options, reopen};
use ew_store::ew_format::ew_schema::{LibCopy, LibDocument, MountKind, Part, Sha256};
use ew_store::ew_format::{FindingKind, Severity, hash_text, lib_document_text, sha256};
use ew_store::mem::DirSync;
use ew_store::{Access, Changes, Opened, Store, Vfs, join};

/// A part copy of the golden project (Festo 552286): no sections, no new members.
const PART_COPY: &str = "lib/parts/07a6c371-1881-5acc-9ac3-0d773f0ddc16.json";

/// The new members as an older build kept them: unknown, so JSON values in `extra`.
const MECH: &str = r#"{"width":45000000,"height":90000000,"depth":60000000,"mount_kind":"ts35x7_5","rail_offset":[22500000,45000000],"clearance":{"top":20000000,"bottom":20000000},"mass":120}"#;
const ALTERNATIVES: &str = r#"[{"part":{"library":"d76f5765-c456-5393-935c-4cec32a87b01","item":"0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13","version_req":"*"},"kind":"equivalent"}]"#;

/// The golden copy as a build before part 1 wrote it with `mech` and `alternatives`: the text and its document.
fn written_before_part_one() -> (String, LibDocument) {
    let golden = String::from_utf8(fixture()[PART_COPY].clone()).expect("UTF-8");
    let mut doc = LibDocument::from_json(&golden).expect("the golden part copy");
    let LibDocument::Part(copy) = &mut doc else {
        panic!("a part copy");
    };
    assert!(copy.item.mech.is_none() && copy.item.alternatives.is_empty());
    // serde_json's maps are byte-wise ordered (never `preserve_order`): the older build's member order.
    let value = |s: &str| serde_json::from_str::<serde_json::Value>(s).expect("JSON");
    copy.item.extra.insert("mech".into(), value(MECH));
    copy.item.extra.insert("alternatives".into(), value(ALTERNATIVES));
    copy.sha256 = Sha256(sha256(
        serde_json::to_string(&copy.item).expect("item JSON").as_bytes(),
    ));
    let text = lib_document_text(&doc).expect("a library document text");
    (text, doc)
}

#[test]
fn an_old_part_copy_opens_typed_and_the_next_save_rewrites_it_canonically() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let (old_text, old_doc) = written_before_part_one();
    // Byte-wise: `alternatives` before `mech`, and inside an alternative `kind` before `part`.
    let alternatives = &old_text[old_text.find("\"alternatives\"").expect("alternatives")..];
    assert!(alternatives.contains("\"mech\""), "byte-wise members: {old_text}");
    assert!(
        alternatives.find("\"kind\"") < alternatives.find("\"part\""),
        "byte-wise members: {old_text}"
    );
    // The project as the older build saved it: the copy and the manifest's hash of it.
    let manifest = String::from_utf8(files["manifest.json"].clone()).expect("UTF-8");
    let manifest = manifest.replace(
        &hash_text(&sha256(&files[PART_COPY])),
        &hash_text(&sha256(old_text.as_bytes())),
    );
    vfs.put(&join(&root, PART_COPY), old_text.as_bytes());
    vfs.put(&join(&root, "manifest.json"), manifest.as_bytes());

    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    let mut found: Vec<(FindingKind, Severity, Option<&str>)> = report
        .findings
        .iter()
        .map(|f| (f.kind, f.severity(), f.code()))
        .collect();
    found.sort_by_key(|f| format!("{f:?}"));
    assert_eq!(
        found,
        [
            (FindingKind::LibraryHash, Severity::Warning, Some("EW-CORE-014")),
            (FindingKind::NonCanonical, Severity::Info, None),
        ],
        "{:?}",
        report.findings
    );
    let LibDocument::Part(old) = &old_doc else {
        panic!("a part copy");
    };
    let copy: LibCopy<Part> = (**model.table::<LibCopy<Part>>().get(&old.id).expect("the copy")).clone();
    assert!(copy.item.extra.is_empty(), "{:?}", copy.item.extra);
    let mech = copy.item.mech.as_ref().expect("mech is typed");
    assert_eq!(mech.mount_kind, Some(MountKind::Ts35x7_5));
    assert_eq!(mech.mass, Some(120));
    assert_eq!(copy.item.alternatives.len(), 1);
    assert_eq!(copy.sha256, old.sha256, "the stored hash is kept");

    // A save without an edit rewrites the copy: the canonical text of the typed document.
    let saved = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("saves");
    assert_eq!(saved.written.len(), 1, "{:?}", saved.written);
    let written =
        String::from_utf8(vfs.read(&join(&root, PART_COPY)).expect("the copy exists")).expect("UTF-8");
    assert_ne!(written, old_text);
    assert_eq!(
        written,
        lib_document_text(&LibDocument::Part(copy)).expect("text")
    );
    let mech_at = written.find("\"mech\"").expect("mech written");
    let alternatives_at = written.find("\"alternatives\"").expect("alternatives written");
    assert!(mech_at < alternatives_at, "doc 07 §2.5's member order: {written}");
    drop(store);

    // Reopened: the file is canonical now; only the stored hash is reported (accepted within 0.1).
    let again = reopen(&vfs, 2);
    let kinds: Vec<FindingKind> = again.report.findings.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, [FindingKind::LibraryHash], "{:?}", again.report.findings);
    assert!(again.model.content_eq(&model));
}
