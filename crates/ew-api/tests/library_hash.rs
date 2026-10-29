// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The copy hash of `library.embed` (`ew-commands`) agrees with the format reader's (`ew-format`,
//! EW-CORE-014): both are the SHA-256 of the item's compact JSON (doc 05 §13.2). `ew-commands` may not
//! depend on `ew-format` (ARCH §3), so the two functions are kept equal by this test, which sees both. The kinds
//! and members typed by WP-S1.19 part 1 (manufacturer and macro copies, the new part members) hash alike too.

use std::path::Path;

use ew_commands::commands::library_copy_hash as command_hash;
use ew_schema::LibDocument;
use ew_store::ew_format::library_copy_hash as format_hash;

fn claimed(doc: &LibDocument) -> Option<[u8; 32]> {
    match doc {
        LibDocument::Part(c) => Some(c.sha256.0),
        LibDocument::Symbol(c) => Some(c.sha256.0),
        LibDocument::Fndef(c) => Some(c.sha256.0),
        LibDocument::Form(c) => Some(c.sha256.0),
        LibDocument::Macro(c) => Some(c.sha256.0),
        LibDocument::Manufacturer(c) => Some(c.sha256.0),
        LibDocument::Unknown(_) => None,
    }
}

/// Every shipped library document under `content/`.
fn shipped() -> Vec<LibDocument> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut docs = Vec::new();
    for sub in ["symbols/qet10", "fndefs", "forms"] {
        let mut paths: Vec<_> = std::fs::read_dir(root.join(sub))
            .expect("content directory")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .collect();
        paths.sort();
        for p in paths {
            let text = std::fs::read_to_string(&p).expect("read");
            docs.push(LibDocument::from_json(&text).expect("a library document"));
        }
    }
    docs
}

#[test]
fn the_command_and_the_reader_hash_a_copy_alike() {
    let docs = shipped();
    assert!(docs.len() > 20, "{} shipped documents", docs.len());
    for doc in &docs {
        let (c, f) = (command_hash(doc), format_hash(doc));
        assert_eq!(c, f, "the two hashes agree");
        assert_eq!(c, claimed(doc), "a shipped copy carries its hash");
    }
}

#[test]
fn a_changed_item_changes_both_hashes_alike() {
    let mut doc = shipped()
        .into_iter()
        .find(|d| matches!(d, LibDocument::Form(_)))
        .expect("a shipped form");
    if let LibDocument::Form(c) = &mut doc {
        c.item.extra.insert("x_test".into(), serde_json::json!("changed"));
    }
    let (c, f) = (command_hash(&doc), format_hash(&doc));
    assert_eq!(c, f);
    assert_ne!(c, claimed(&doc), "the claimed hash no longer matches");
    assert_eq!(
        command_hash(&LibDocument::Unknown(serde_json::json!({"$k": "x"}))),
        None
    );
    assert_eq!(
        format_hash(&LibDocument::Unknown(serde_json::json!({"$k": "x"}))),
        None
    );
}

/// The kinds and members of WP-S1.19 part 1: a manufacturer copy, a macro copy and a part with every new member hash
/// alike in both crates, and the hash is the SHA-256 of the item's compact JSON in declaration order.
#[test]
fn the_part_one_kinds_and_members_hash_alike() {
    use sha2::Digest;
    const ENVELOPE: &str = r#""library_id":"6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47","item_id":"0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13","version":"1.0.0""#;
    let lib_ref = r#"{"library":"6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47","item":"8b42c7d9-1e06-5a3f-b8d5-6c0e2f9a4b71","version_req":"^1"}"#;
    let items = [
        (
            "manufacturer",
            r#"{"meta":{"key":"ew.mfr.eaton"},"key":"EATON","name":"Eaton","aliases":["Eaton / Holec"],"brands":["Holec"]}"#.to_owned(),
        ),
        (
            "macro",
            r#"{"name":{"en-GB":"PSU block"},"kind":"window","variants":[{"rep_type":"multi_line","key":"A","fragment":{"records":[]}}],"value_sets":[{"name":"1","values":{"counter":1}}]}"#.to_owned(),
        ),
        (
            "part",
            format!(
                r#"{{"manufacturer":{lib_ref},"part_number":"2904602","cp_pattern":[{{"idx":0,"designation":"1","clamp_type":"push_in","csa_min":"0.14","csa_max":"4","max_conductors":2,"twin_ferrule":true}}],"contacts":[{{"kind":"no","cps":["13","14"]}}],"mech":{{"width":60000000,"mount_kind":"ts35x7_5","rail_offset":[30000000,65000000]}},"sections":{{"enclosure":{{"width":600000000,"plate":{lib_ref}}},"duct":{{"slot_pitch":12500000}},"rail":{{"profile":"ts35x15"}},"mounting_plate":{{"thickness":3000000}}}},"alternatives":[{{"part":{lib_ref},"kind":"approved"}}],"configured":{{"order_code":"K+X","components":[{{"part":{lib_ref},"qty":"2"}}]}},"macros":[{{"rep_type":"multi_line","macro":{lib_ref}}}]}}"#
            ),
        ),
    ];
    for (kind, item) in items {
        let hash: [u8; 32] = sha2::Sha256::digest(item.as_bytes()).into();
        let hex: String = hash.iter().map(|b| format!("{b:02x}")).collect();
        let text = format!(
            r#"{{"$k":"{kind}","id":"0b7e6b3c-31f4-5d0e-9a7c-2f6d1e8a4b90",{ENVELOPE},"sha256":"sha256:{hex}",{}"#,
            &item[1..]
        );
        let doc = LibDocument::from_json(&text).expect("a library document");
        assert_eq!(serde_json::to_string(&doc).unwrap(), text, "{kind}: canonical");
        assert_eq!(command_hash(&doc), Some(hash), "{kind}: the command's hash");
        assert_eq!(format_hash(&doc), Some(hash), "{kind}: the reader's hash");
        assert_eq!(claimed(&doc), Some(hash));
    }
}
