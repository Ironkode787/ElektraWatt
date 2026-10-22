// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The copy hash of `library.embed` (`ew-commands`) agrees with the format reader's (`ew-format`,
//! EW-CORE-014): both are the SHA-256 of the item's compact JSON (doc 05 §13.2). `ew-commands` may not
//! depend on `ew-format` (ARCH §3), so the two functions are kept equal by this test, which sees both.

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
