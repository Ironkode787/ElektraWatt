// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The supply groups of a part's PLC section through the files (FR-PLC-024; PX-2): a part copy in
//! `lib/` whose `sections.plc.supply_groups` carries typed members, unknown members and newer enum values loads and
//! saves byte-equal, and its stored `sha256` (the hash of the item's compact JSON) verifies.

use ew_format::ew_schema::{LibDocument, SupplyFeeds, SupplyKind};
use ew_format::{hash_text, lib_document_text, lib_file, library_copy_hash, sha256};

use crate::write_and_read;

const LIB: &str = "6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47";
const COPY: &str = "5e2b8c41-9d07-5f36-a1c8-4b7e0d2f9a65";
const ITEM: &str = "a83c1f70-4b29-5e6d-8c05-7f1a3e9b0d42";

/// The part item's compact JSON: an interlinking block with two supply groups (doc 10 §2.6's 550208, as ruling 1
/// shapes it), an unknown group member and a separate earth pin.
fn item() -> String {
    concat!(
        r#"{"manufacturer":{"library":"6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47","item":"0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13","version_req":"^1"},"#,
        r#""part_number":"550208","sections":{"plc":{"role":"station","supply_groups":["#,
        r#"{"kind":"electronics","cps":{"l_plus":"4","m":"2"}},"#,
        r#"{"kind":"load","cps":{"l_plus":"5","m":"1"},"feeds":"outputs","fuse":"2 A"},"#,
        r#"{"kind":"sensor","cps":{"l_plus":"7","m":"6"}}"#,
        r#"],"earth":{"pin":"3"}}}}"#
    )
    .to_owned()
}

/// The copy's compact JSON with the hash of `item`.
fn compact(item: &str) -> String {
    let hash = hash_text(&sha256(item.as_bytes()));
    format!(
        r#"{{"$k":"part","id":"{COPY}","library_id":"{LIB}","item_id":"{ITEM}","version":"1.0.0","sha256":"{hash}",{}"#,
        &item[1..]
    )
}

/// PX-2: the copy is canonical, its hash verifies, and it is written and read back unchanged.
#[test]
fn a_part_copy_with_supply_groups_round_trips_and_its_hash_verifies() {
    let item = item();
    let doc = LibDocument::from_json(&compact(&item)).unwrap();
    assert_eq!(
        serde_json::to_string(&doc).unwrap(),
        compact(&item),
        "not canonical"
    );
    assert_eq!(library_copy_hash(&doc), Some(sha256(item.as_bytes())));
    let LibDocument::Part(copy) = &doc else {
        panic!("a part");
    };
    let plc = copy.item.sections.plc.as_ref().expect("a PLC section");
    let kinds: Vec<&SupplyKind> = plc.supply_groups.iter().map(|g| &g.kind).collect();
    assert_eq!(
        kinds,
        [
            &SupplyKind::Electronics,
            &SupplyKind::Load,
            &SupplyKind::Unknown(serde_json::json!("sensor"))
        ]
    );
    assert_eq!(plc.supply_groups[1].feeds(), SupplyFeeds::Outputs);

    let (files, loaded) = write_and_read(&[], std::slice::from_ref(&doc));
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
    assert_eq!(loaded.lib, std::slice::from_ref(&doc));
    let path = lib_file(&doc).unwrap();
    assert_eq!(files[&path], lib_document_text(&doc).unwrap().into_bytes());
    // A second save of the loaded copy writes the same bytes.
    let (again, _) = write_and_read(&[], &loaded.lib);
    assert_eq!(again[&path], files[&path]);
}
