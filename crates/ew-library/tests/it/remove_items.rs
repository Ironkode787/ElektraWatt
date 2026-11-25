// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Registry::remove` (a part removed from My parts): an item taken out of a writable library is gone from
//! its documents, its search and the next open, the rest of its shard stays; taking out an item the library does not
//! hold changes nothing.

use ew_ids::Uuid;
use ew_library::{LibraryTiers, PartQuery, Registry, starter_parts_library};
use ew_schema::{Kind, LibDocument};
use serde_json::json;

const ZERO: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// A part of library `lib`, numbered `number`, of manufacturer `maker`.
fn part(lib: Uuid, item: Uuid, number: &str, maker: Uuid) -> LibDocument {
    LibDocument::from_value(json!({
        "$k": "part", "id": item.to_string(), "library_id": lib.to_string(),
        "item_id": item.to_string(), "version": "1.0.0", "sha256": ZERO,
        "meta": {"licence": "CC0-1.0"}, "kind": "device",
        "manufacturer": {"library": lib.to_string(), "item": maker.to_string(), "version_req": "^1"},
        "part_number": number, "description": "A card of one's own",
    }))
    .unwrap()
}

/// The part numbers library `lib` holds.
fn numbers(reg: &Registry, lib: Uuid) -> Vec<String> {
    let mut out: Vec<String> = reg
        .documents(lib, Kind::Part)
        .into_iter()
        .filter_map(|d| match d {
            LibDocument::Part(c) => Some(c.item.part_number.as_str().to_owned()),
            _ => None,
        })
        .collect();
    out.sort();
    out
}

/// The part numbers of library `lib` that a search for `text` finds.
fn found(reg: &Registry, lib: Uuid, text: &str) -> Vec<String> {
    reg.search(&PartQuery {
        text: text.into(),
        ..PartQuery::default()
    })
    .into_iter()
    .filter(|h| h.library == lib && h.library != starter_parts_library())
    .map(|h| h.part_number)
    .collect()
}

#[test]
fn an_item_taken_out_of_a_library_is_gone_and_the_rest_stays() {
    let root = ew_testdir::Tmp::new("library-remove");
    let tiers = LibraryTiers::under(&root.join("data"), &root.join("cache"), Vec::new(), Vec::new());
    let mut reg = Registry::open(&tiers).unwrap();
    let lib = reg
        .create(&tiers.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    let (maker, a, b) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let mfr = LibDocument::from_value(json!({
        "$k": "manufacturer", "id": maker.to_string(), "library_id": lib.to_string(),
        "item_id": maker.to_string(), "version": "1.0.0", "sha256": ZERO, "key": "EX", "name": "Example",
    }))
    .unwrap();
    reg.write(
        lib,
        &[mfr, part(lib, a, "DI-4", maker), part(lib, b, "DO-4", maker)],
    )
    .unwrap();
    assert_eq!(numbers(&reg, lib), ["DI-4", "DO-4"]);
    assert_eq!(found(&reg, lib, "DI-4"), ["DI-4"]);
    let report = reg.remove(lib, Kind::Part, &[a]).unwrap();
    assert_eq!(report.deleted, [a]);
    assert_eq!(numbers(&reg, lib), ["DO-4"], "the rest of its shard stays");
    assert!(
        found(&reg, lib, "DI-4").is_empty(),
        "the search no longer finds it"
    );
    // Taking it out again changes nothing.
    let again = reg.remove(lib, Kind::Part, &[a]).unwrap();
    assert!(again.deleted.is_empty() && again.written.is_empty());
    assert_eq!(again.generation, report.generation);
    // The next open reads the library without it.
    drop(reg);
    let reg = Registry::open(&tiers).unwrap();
    assert_eq!(numbers(&reg, lib), ["DO-4"]);
}
