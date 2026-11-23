// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `parts.import`: project-local part copies are created once; a second import skips every key; the
//! command is Bulk and checks every copy's lineage, library and key.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::commands::{ImportParts, import_counts};
use ew_commands::{CmdError, Cost, Engine, Origin, Receipt};
use ew_format::library_copy_hash;
use ew_model::fixture::{ident, lib_ref_to, manufacturer_copy, part};
use ew_schema::{Kind, LibCopy, LibDocument, Manufacturer, Part, Sha256};
use serde_json::{Value, json};

/// The library of the manufacturers the part copies name before a test re-homes them.
const MAKER_LIBRARY: u128 = 0x9a00;
/// The manufacturer items of [`local_manufacturers`]; the part copies name them in turn.
const MAKERS: [u128; 2] = [0x9a01, 0x9a02];

/// `n` part copies `P1` … `Pn` (key and part number), project-local to the test project: `library_id` = the
/// project ID, the copy ID its identity, `sha256` its item's hash. Each names a manufacturer of [`MAKERS`] in
/// turn, in [`MAKER_LIBRARY`].
fn local_copies(n: usize) -> Vec<Value> {
    let project = project().id.as_uuid();
    (0..n)
        .map(|i| {
            let item = uuid(0x9b01 + i as u128);
            let key = format!("P{}", i + 1);
            let mut c = part(ew_ids::v5::lib_copy_id(project, item), &key);
            c.library_id = ew_ids::Id::from_uuid(project);
            c.item_id = ew_ids::Id::from_uuid(item);
            c.item.meta.key = Some(ident(&key));
            c.item.manufacturer = lib_ref_to(uuid(MAKER_LIBRARY), uuid(MAKERS[i % MAKERS.len()]));
            let mut doc = LibDocument::Part(c);
            let hash = library_copy_hash(&doc).expect("a part copy has a hash");
            if let LibDocument::Part(c) = &mut doc {
                c.sha256 = Sha256(hash);
            }
            serde_json::to_value(doc).unwrap()
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
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(parts(&e), before + 5);
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(parts(&e), before);
}

/// The manufacturer copies of [`MAKERS`], project-local like [`local_copies`].
fn local_manufacturers() -> Vec<LibCopy<Manufacturer>> {
    let project = project().id.as_uuid();
    MAKERS
        .iter()
        .map(|&item| {
            let mut c = manufacturer_copy(ew_ids::v5::lib_copy_id(project, uuid(item)));
            c.library_id = ew_ids::Id::from_uuid(project);
            c.item_id = ew_ids::Id::from_uuid(uuid(item));
            c.sha256 = Sha256(
                library_copy_hash(&LibDocument::Manufacturer(c.clone()))
                    .expect("a manufacturer copy has a hash"),
            );
            c
        })
        .collect()
}

fn makers(e: &Engine) -> usize {
    e.model().table::<LibCopy<Manufacturer>>().len()
}

/// Only the manufacturer copies an inserted part copy names are inserted; an import whose part copies
/// are all skipped inserts none and is an empty transaction (no undo step).
#[test]
fn only_the_manufacturers_of_inserted_parts_are_inserted() {
    let project = project().id.as_uuid();
    let all = local_manufacturers();
    // The first part names a project-local copy of its manufacturer; its hash follows the changed reference.
    let mut part = local_copies(1).remove(0);
    let maker_item = part["manufacturer"]["item"].as_str().unwrap().to_owned();
    part["manufacturer"]["library"] = json!(project.to_string());
    let doc = LibDocument::from_value(part).unwrap();
    let hash = ew_format::library_copy_hash(&doc).unwrap();
    let mut part = serde_json::to_value(&doc).unwrap();
    part["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    let named = all
        .iter()
        .find(|m| m.item_id.as_uuid().to_string() == maker_item)
        .expect("the part's manufacturer")
        .clone();
    let other = all
        .iter()
        .find(|m| m.item_id != named.item_id)
        .expect("another manufacturer")
        .clone();
    let value =
        |m: &LibCopy<Manufacturer>| serde_json::to_value(LibDocument::Manufacturer(m.clone())).unwrap();
    let mut e = engine(empty_model());
    let before = makers(&e);
    let r = import(&mut e, vec![part.clone(), value(&named), value(&other)]).unwrap();
    assert_eq!(r.outcome.note.as_deref().and_then(import_counts), Some((1, 0)));
    assert_eq!(makers(&e), before + 1, "only the named manufacturer");
    let held = e.model().table::<LibCopy<Manufacturer>>();
    assert!(held.iter().any(|(id, _)| *id == named.id));
    assert!(held.iter().all(|(id, _)| *id != other.id));
    // Every part skipped: no manufacturer inserted, no undo step.
    let r = import(&mut e, vec![part, value(&other)]).unwrap();
    assert_eq!(r.outcome.note.as_deref().and_then(import_counts), Some((0, 1)));
    assert_eq!(makers(&e), before + 1);
    e.undo(&Origin::Gui).unwrap();
    assert_eq!(makers(&e), before, "the undo step was the first import");
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
