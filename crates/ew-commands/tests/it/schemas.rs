// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command-parameter schemas that `CommandRegistry::builtin().schema_files()` generates: each is a usable JSON
//! Schema document. They are not committed: `cargo xtask gen` runs this test with `EW_REFERENCE_DIR` set, and the
//! test then writes them there for the user guide's reference pages.

// Test target: `EW_REFERENCE_DIR` only chooses whether the generated files are also written out.
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use ew_commands::CommandRegistry;
use serde_json::Value;

#[test]
fn the_schemas_are_written_where_the_reference_is_asked_for() {
    let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(|d| PathBuf::from(d).join("commands")) else {
        return;
    };
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create the schema directory");
    for (name, text) in CommandRegistry::builtin().schema_files() {
        std::fs::write(dir.join(name), text).expect("write a schema");
    }
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
    assert!(!names.is_empty(), "the registry has commands");
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
    assert_eq!(
        index["commands"].as_array().expect("commands array").len(),
        names.len()
    );
}
