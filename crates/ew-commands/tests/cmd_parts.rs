// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `parts.import` (FT-410): project-local part copies are created once; a second import skips every key; the
//! command is Bulk and checks every copy's lineage, library and key.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::*;
use ew_commands::commands::{ImportParts, import_counts};
use ew_commands::{CmdError, Cost, Engine, Receipt};
use ew_schema::{Kind, LibCopy, LibDocument, Part};
use serde_json::{Value, json};

/// The golden project's part copies (`golden/J-E-2416.ew/lib/parts`), re-homed as project-local copies of the test
/// project: `library_id` = the project ID, the copy ID its identity; the item (and so its hash) unchanged.
fn local_copies(n: usize) -> Vec<Value> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew/lib/parts");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    paths.sort();
    let project = project().id.as_uuid();
    paths
        .iter()
        .take(n)
        .map(|p| {
            let LibDocument::Part(mut c) =
                LibDocument::from_json(&std::fs::read_to_string(p).unwrap()).unwrap()
            else {
                panic!("a part copy")
            };
            c.library_id = ew_ids::Id::from_uuid(project);
            c.id = ew_ids::v5::lib_copy_id(project, c.item_id.as_uuid());
            serde_json::to_value(LibDocument::Part(c)).unwrap()
        })
        .collect()
}

fn import(e: &mut Engine, copies: Vec<Value>) -> Result<Receipt, CmdError> {
    let cmd = ImportParts { copies };
    let preview = e.preview(&cmd)?;
    e.commit(preview)
}

fn parts(e: &Engine) -> usize {
    e.model().table::<LibCopy<Part>>().len()
}

fn invalid_path(r: Result<Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::InvalidParams { command, path, .. }) => {
            assert_eq!(command, "parts.import");
            path
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn parts_import_is_bulk_and_refused_by_execute() {
    let mut e = engine(empty_model());
    let cmd = ImportParts {
        copies: local_copies(1),
    };
    assert_eq!(e.registry().get("parts.import").unwrap().meta().cost, Cost::Bulk);
    assert!(matches!(e.execute(&cmd), Err(CmdError::NeedsPreview { .. })));
}

#[test]
fn a_second_import_skips_every_key_and_undo_removes_the_first() {
    let mut e = engine(empty_model());
    let before = parts(&e);
    let copies = local_copies(5);
    let r = import(&mut e, copies.clone()).expect("parts.import");
    assert_eq!(parts(&e), before + 5);
    assert_eq!(r.outcome.note.as_deref().and_then(import_counts), Some((5, 0)));
    assert_eq!(r.outcome.primary.map(|k| k.0), Some(Kind::Part));
    let again = import(&mut e, copies).expect("a re-import commits");
    assert_eq!(
        again.outcome.note.as_deref().and_then(import_counts),
        Some((0, 5))
    );
    assert_eq!(parts(&e), before + 5);
    // One more key: created 1, skipped 5.
    let mut six = local_copies(6);
    let r = import(&mut e, std::mem::take(&mut six)).unwrap();
    assert_eq!(r.outcome.note.as_deref().and_then(import_counts), Some((1, 5)));
    // The all-skipped import changed nothing and is no undo step.
    e.undo().unwrap();
    assert_eq!(parts(&e), before + 5);
    e.undo().unwrap();
    assert_eq!(parts(&e), before);
}

#[test]
fn the_copies_are_checked() {
    let mut e = engine(empty_model());
    assert_eq!(invalid_path(import(&mut e, vec![])), "/copies");
    let good = local_copies(2);
    // Another library: library.embed's.
    let mut foreign = good[0].clone();
    let lib = uuid(0xabc);
    let item: ew_ids::Uuid = foreign["item_id"].as_str().unwrap().parse().unwrap();
    foreign["library_id"] = json!(lib.to_string());
    foreign["id"] = json!(ew_ids::v5::lib_copy_id::<()>(lib, item).as_uuid().to_string());
    assert_eq!(
        invalid_path(import(&mut e, vec![foreign])),
        "/copies/0/library_id"
    );
    // A wrong copy identity.
    let mut wrong = good[0].clone();
    wrong["id"] = good[1]["id"].clone();
    assert_eq!(invalid_path(import(&mut e, vec![wrong])), "/copies/0/id");
    // A hash that is not the item's.
    let mut hash = good[0].clone();
    hash["part_number"] = json!("CHANGED");
    assert_eq!(invalid_path(import(&mut e, vec![hash])), "/copies/0/sha256");
    // Twice.
    assert_eq!(
        invalid_path(import(&mut e, vec![good[0].clone(), good[0].clone()])),
        "/copies/1"
    );
    // Not a part.
    let form = json!({"$k": "form"});
    assert!(invalid_path(import(&mut e, vec![form])).starts_with("/copies/0"));
    assert_eq!(
        parts(&e),
        engine(empty_model()).model().table::<LibCopy<Part>>().len()
    );
}

#[test]
fn the_note_parses_back() {
    assert_eq!(import_counts("created 29, skipped 0"), Some((29, 0)));
    assert_eq!(import_counts("something else"), None);
}
