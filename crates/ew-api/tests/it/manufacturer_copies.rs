// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The sample project's manufacturer copies: the part references carry no `key`, and every page, report and export
//! derives exactly as from a file written before manufacturer copies existed, whose references keep the name in `key`
//! and which holds no manufacturer copy (the readers' fallback).

use crate::common;

use ew_api::output::page_view;
use ew_api::{BomMode, Export, Session};
use ew_derive::PageOrder;
use ew_schema::{LibDocument, Sha256};

/// The library copies as a file written before manufacturer copies existed holds them: no manufacturer copies, the
/// name in each part reference's `key` member, the part hashes recomputed.
fn before_part_3(lib: &[LibDocument]) -> Vec<LibDocument> {
    let mut old = lib.to_vec();
    let names: Vec<(ew_ids::Uuid, String)> = old
        .iter()
        .filter_map(|d| match d {
            LibDocument::Manufacturer(m) => Some((m.item_id.as_uuid(), m.item.key.as_str().to_owned())),
            _ => None,
        })
        .collect();
    old.retain(|d| !matches!(d, LibDocument::Manufacturer(_)));
    for doc in &mut old {
        if let LibDocument::Part(p) = doc {
            let (_, name) = names
                .iter()
                .find(|(item, _)| *item == p.item.manufacturer.item.as_uuid())
                .expect("every part names an emitted manufacturer");
            p.item
                .manufacturer
                .extra
                .insert("key".into(), serde_json::Value::String(name.clone()));
        }
        let hash = ew_commands::commands::library_copy_hash(doc);
        if let (LibDocument::Part(p), Some(hash)) = (doc, hash) {
            p.sha256 = Sha256(hash);
        }
    }
    old
}

fn pages(s: &Session) -> Vec<ew_scene::page::PageView> {
    let q = s.query();
    q.get::<PageOrder>(())
        .entries
        .iter()
        .map(|entry| page_view(&q, entry))
        .collect()
}

/// The sample's parts are the starter library's generic ones: one manufacturer copy, Generic, and no part reference
/// keeps a name in `key`.
#[test]
fn the_sample_holds_its_manufacturer_copy_and_no_reference_keeps_a_key() {
    let (_, _, lib) = common::sample_records();
    let names: Vec<&str> = lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Manufacturer(m) => Some(m.item.key.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, ["Generic"]);
    let parts: Vec<_> = lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Part(p) => Some(p),
            _ => None,
        })
        .collect();
    assert!(!parts.is_empty());
    for p in parts {
        assert!(p.item.manufacturer.extra.is_empty(), "{}", p.item.part_number);
    }
}

#[test]
fn every_page_and_export_equals_the_file_written_before_part_3() {
    let (project, records, lib) = common::sample_records();
    let now = common::session_of(project.clone(), records.clone(), lib.clone());
    let old = common::session_of(project, records, before_part_3(&lib));
    let (a, b) = (pages(&now), pages(&old));
    assert_eq!(a.len(), b.len());
    assert!(!a.is_empty());
    for (i, (a, b)) in a.iter().zip(&b).enumerate() {
        assert!(a == b, "page {i} differs");
    }
    for what in [
        Export::Bom(BomMode::Component),
        Export::Bom(BomMode::Assembly),
        Export::Cables,
    ] {
        let (a, b) = (now.export_csv(what), old.export_csv(what));
        assert_eq!(a, b, "{what:?}");
        assert!(a.is_ok(), "{what:?}");
    }
    // The manufacturer's name prints, from the copy now and from the key before.
    let bom = now.export_csv(Export::Bom(BomMode::Component)).expect("the BOM");
    assert!(bom.contains("Generic"), "{bom}");
}
