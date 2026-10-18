// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The committed JSON Schemas under `schemas/<FORMAT>/` (doc 05 §10.4) equal what `ew-schema` generates.
//!
//! The test regenerates every file in memory and fails when a committed file differs, is missing or is
//! stale; `EW_SCHEMA_BLESS=1 cargo test -p ew-schema --test schemas` rewrites the directory. A second test
//! checks that every schema is a usable JSON Schema document.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ew_schema::{FORMAT, Kind, KindStatus, schema_files};
use serde_json::Value;

fn schema_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas")
        .join(FORMAT)
}

fn committed() -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(schema_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(entry.path()).expect("read schema file");
            files.insert(name, text);
        }
    }
    files
}

#[test]
fn committed_schemas_match_the_generated_ones() {
    let generated: BTreeMap<String, String> = schema_files().into_iter().collect();
    if std::env::var_os("EW_SCHEMA_BLESS").is_some() {
        let dir = schema_dir();
        std::fs::create_dir_all(&dir).expect("create schema directory");
        for name in committed().keys() {
            if !generated.contains_key(name) {
                std::fs::remove_file(dir.join(name)).expect("remove stale schema");
            }
        }
        for (name, text) in &generated {
            std::fs::write(dir.join(name), text).expect("write schema");
        }
        return;
    }
    let committed = committed();
    let stale: Vec<&String> = committed.keys().filter(|n| !generated.contains_key(*n)).collect();
    let differing: Vec<&String> = generated
        .iter()
        .filter(|(name, text)| committed.get(*name) != Some(text))
        .map(|(name, _)| name)
        .collect();
    assert!(
        stale.is_empty() && differing.is_empty(),
        "schemas/{FORMAT}/ is out of date (differing or missing: {differing:?}; stale: {stale:?}); run \
         `EW_SCHEMA_BLESS=1 cargo test -p ew-schema --test schemas` and commit the result"
    );
}

#[test]
fn every_schema_is_a_json_schema_document() {
    let files: BTreeMap<String, String> = schema_files().into_iter().collect();
    for kind in Kind::ALL {
        let name = format!("{}.schema.json", kind.name());
        match kind.status() {
            KindStatus::Implemented => {
                let text = files.get(&name).unwrap_or_else(|| panic!("{name} missing"));
                assert!(
                    text.ends_with("}\n") && !text.contains('\r'),
                    "{name}: LF and a final newline"
                );
                let schema: Value = serde_json::from_str(text).expect("valid JSON");
                assert_eq!(
                    schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
                    "{name}"
                );
                assert_eq!(schema["title"], kind.name(), "{name}");
                assert_eq!(schema["properties"]["$k"]["const"], kind.name(), "{name}");
                assert!(schema["properties"]["id"].is_object(), "{name}: id");
                assert_eq!(schema["required"][0], "$k", "{name}");
                assert!(
                    schema["required"]
                        .as_array()
                        .unwrap()
                        .contains(&Value::from("id")),
                    "{name}: id is required"
                );
            }
            KindStatus::Reserved => assert!(!files.contains_key(&name), "{name} is reserved"),
        }
    }
    let index: Value = serde_json::from_str(&files["index.json"]).unwrap();
    assert_eq!(index["format"], FORMAT);
    assert_eq!(index["kinds"].as_array().unwrap().len(), Kind::ALL.len());
    let record: Value = serde_json::from_str(&files["record.schema.json"]).unwrap();
    assert!(record["oneOf"].as_array().unwrap().len() >= 15);
}

/// Library documents carry the copy metadata members of doc 05 §13.2.
#[test]
fn library_schemas_carry_the_copy_metadata() {
    let files: BTreeMap<String, String> = schema_files().into_iter().collect();
    for kind in [Kind::Part, Kind::Symbol, Kind::Fndef, Kind::Form] {
        let schema: Value = serde_json::from_str(&files[&format!("{}.schema.json", kind.name())]).unwrap();
        for member in ["library_id", "item_id", "version", "sha256"] {
            assert!(schema["properties"][member].is_object(), "{kind}: {member}");
        }
    }
}
