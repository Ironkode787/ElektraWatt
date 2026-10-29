// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! L1-5 (FT-24): every starter document under `content/` carries [`SYSTEM_LIBRARY`] as its `library_id`, with
//! `item_id = v5(SYSTEM_LIBRARY, meta.key)`. Moved here from `ew-interop-api`, a permissive crate whose own tests do
//! not read files outside the crate.

use std::path::{Path, PathBuf};

use ew_interop_api::SYSTEM_LIBRARY;

/// The JSON documents of one starter content directory, sorted by file name.
fn documents(dir: &str) -> Vec<(PathBuf, serde_json::Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../content")
        .join(dir);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let doc = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (p, doc)
        })
        .collect()
}

#[test]
fn system_library_is_the_library_id_of_every_starter_document() {
    let mut seen = 0;
    for dir in ["symbols", "symbols/qet10", "fndefs", "forms"] {
        let docs = documents(dir);
        assert!(!docs.is_empty(), "content/{dir} holds no starter documents");
        for (path, doc) in docs {
            let name = path.display();
            assert_eq!(
                doc["library_id"].as_str(),
                Some(SYSTEM_LIBRARY.to_string().as_str()),
                "{name}: library_id"
            );
            let key = doc["meta"]["key"]
                .as_str()
                .unwrap_or_else(|| panic!("{name}: no meta.key"));
            let item = ew_ids::Uuid::new_v5(&SYSTEM_LIBRARY, key.as_bytes());
            assert_eq!(
                doc["item_id"].as_str(),
                Some(item.to_string().as_str()),
                "{name}: item_id"
            );
            seen += 1;
        }
    }
    assert!(seen > 70, "only {seen} starter documents found");
}
