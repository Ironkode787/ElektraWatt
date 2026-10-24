// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The committed command-parameter schemas under `crates/ew-commands/schemas/` (FR-EXT-015, doc 17 §2.1)
//! equal what `CommandRegistry::builtin().schema_files()` generates.
//!
//! The test regenerates every file in memory and fails when a committed file differs, is missing or is
//! stale; `EW_COMMANDS_BLESS=1 cargo test -p ew-commands --test schemas` rewrites the directory. A second
//! test checks that every schema is a usable JSON Schema document.

// Test target: bless and diagnostic environment flags (EW_*), which only choose whether a committed file is rewritten or a line is printed; the compared result never depends on them.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ew_commands::CommandRegistry;
use serde_json::Value;

fn schema_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schemas")
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
    let generated: BTreeMap<String, String> = CommandRegistry::builtin().schema_files().into_iter().collect();
    // The bless flag only chooses whether the committed schemas are rewritten.
    if std::env::var_os("EW_COMMANDS_BLESS").is_some() {
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
        "schemas/ is out of date (differing or missing: {differing:?}; stale: {stale:?}); run \
         `EW_COMMANDS_BLESS=1 cargo test -p ew-commands --test schemas` and commit the result"
    );
}

#[test]
fn every_schema_is_a_json_schema_document() {
    let registry = CommandRegistry::builtin();
    let files: BTreeMap<String, String> = registry.schema_files().into_iter().collect();
    for (name, text) in &files {
        assert!(
            text.ends_with("}\n") && !text.contains('\r'),
            "{name}: LF and a final newline"
        );
    }
    let names: Vec<&str> = registry.names().collect();
    assert_eq!(names.len(), 50, "the v0 command set has 50 commands");
    assert_eq!(
        files.len(),
        names.len() + 1,
        "one schema per command plus index.json"
    );
    for name in &names {
        let file = format!("{name}.schema.json");
        let text = files.get(&file).unwrap_or_else(|| panic!("{file} missing"));
        let schema: Value = serde_json::from_str(text).expect("valid JSON");
        assert_eq!(
            schema["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "{file}"
        );
        assert_eq!(schema["title"], *name, "{file}");
    }
    let index: Value = serde_json::from_str(&files["index.json"]).expect("valid JSON");
    assert_eq!(index["commands"].as_array().expect("commands array").len(), 50);
}
