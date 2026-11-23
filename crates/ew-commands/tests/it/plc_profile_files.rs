// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in address profiles' files are canonical (SC-6): `content/plc-profiles/*.json`, library documents of
//! kind `plc_profile` in the starter library, are pretty JSON in the schema's member order with their copy IDs and
//! the command layer's hash. Here, not in `ew-derive`, because the hash is `ew-commands`'; that the profiles render
//! and parse is `ew-derive`'s `tests/it/plc_profiles.rs`.
//!
//! `EW_BLESS=1 cargo nextest run -p ew-commands --test it -E 'test(/^plc_profile_files::/)'` rewrites the files' IDs
//! and hashes in canonical form.

use std::path::{Path, PathBuf};

use ew_ids::{Id, v5};
use ew_schema::{LibDocument, Sha256};

/// The starter library, which the built-in profiles belong to (`ew_schema::starter::LIBRARY`).
const STARTER: u128 = 0x7202fd8b_1024_4bf1_9e99_670772eaa589;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/plc-profiles")
}

// The test boundary: `EW_BLESS=1` asks the test to rewrite the shipped files.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    // The bless switch of a test: it chooses whether the test rewrites the shipped files, never a result.
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The canonical text of a profile file: the item ID `v5(starter, meta.key)`, the copy ID, the hash of the item.
fn canonical(text: &str) -> String {
    let LibDocument::PlcProfile(mut c) = LibDocument::from_json(text).expect("a library document") else {
        panic!("not a plc_profile document")
    };
    let library = ew_derive::ew_ids::Uuid::from_u128(STARTER);
    let key = c.item.meta.key.as_ref().expect("a key").as_str().to_owned();
    c.library_id = Id::from_uuid(library);
    c.item_id = v5::imported_id(library, &key);
    c.id = v5::lib_copy_id(library, c.item_id.as_uuid());
    let doc = LibDocument::PlcProfile(c.clone());
    c.sha256 = Sha256(ew_format::library_copy_hash(&doc).expect("a hash"));
    format!(
        "{}\n",
        serde_json::to_string_pretty(&LibDocument::PlcProfile(c)).expect("serialisable")
    )
}

#[test]
fn the_profile_files_are_canonical() {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    assert!(!names.is_empty());
    for path in names {
        let mut text = std::fs::read_to_string(&path).unwrap().replace("\r\n", "\n");
        let want = canonical(&text);
        if bless() {
            std::fs::write(&path, &want).unwrap();
            text.clone_from(&want);
        }
        assert_eq!(
            text,
            want,
            "{} is not canonical (EW_BLESS=1 rewrites it)",
            path.display()
        );
    }
}
