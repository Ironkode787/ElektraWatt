// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 3, EM-10 (D10, FT-39): Golden A's manufacturer copies. The bootstrap emits one project-local
//! manufacturer copy per manufacturer name and the part references carry no `key`; every page, report and export
//! derives exactly as from a file written before part 3, whose references keep the name in `key` and which holds no
//! manufacturer copy (the readers' fallback). The committed outputs under `golden/J-E-2416/expected/` are unchanged
//! (the page, PDF, PNG, CSV and acceptance tests compare them byte for byte).

mod golden_common;

use ew_api::output::page_view;
use ew_api::{BomMode, Export, Session};
use ew_derive::PageOrder;
use ew_import_tables::ew_interop_api::ImportBatch;
use ew_schema::{LibDocument, Sha256};

/// The batch as a file written before WP-S1.19 part 3 holds it: no manufacturer copies, the name in each part
/// reference's `key` member, the part hashes recomputed.
fn before_part_3(batch: &ImportBatch) -> ImportBatch {
    let mut old = batch.clone();
    let names: Vec<(ew_ids::Uuid, String)> = old
        .lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Manufacturer(m) => Some((
                m.item_id.as_uuid(),
                m.item.name.as_neutral().unwrap_or_default().to_owned(),
            )),
            _ => None,
        })
        .collect();
    old.lib.retain(|d| !matches!(d, LibDocument::Manufacturer(_)));
    for doc in &mut old.lib {
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

#[test]
fn golden_a_holds_eight_manufacturer_copies_and_no_reference_keeps_a_key() {
    let batch = golden_common::batch();
    let names: Vec<&str> = batch
        .lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Manufacturer(m) => m.item.name.as_neutral(),
            _ => None,
        })
        .collect();
    assert_eq!(names.len(), 8, "{names:?}");
    let parts: Vec<_> = batch
        .lib
        .iter()
        .filter_map(|d| match d {
            LibDocument::Part(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(parts.len(), 29);
    for p in parts {
        assert!(p.item.manufacturer.extra.is_empty(), "{}", p.item.part_number);
    }
}

#[test]
fn every_page_and_export_equals_the_file_written_before_part_3() {
    let batch = golden_common::batch();
    let now = golden_common::session(&batch);
    let old = golden_common::session(&before_part_3(&batch));
    let (a, b) = (pages(&now), pages(&old));
    assert_eq!(a.len(), 36);
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
    // The manufacturer names print, from the copies now and from the keys before.
    let bom = now.export_csv(Export::Bom(BomMode::Component)).expect("the BOM");
    for name in ["Phoenix Contact", "Schneider Electric", "Festo", "Siemens"] {
        assert!(bom.contains(name), "{name}");
    }
}
