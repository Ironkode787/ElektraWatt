// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The starter content reads in Dutch. A Dutch project's parts list prints a part's description in the
//! project's language first, and the symbol browser names a symbol in the window's language, so every starter part
//! carries a Dutch description and every starter symbol a Dutch name beside the English one.

use std::path::Path;

use ew_library::starter::{self, StarterKind};
use serde_json::Value;

/// The text of `value` (a localised text) in `lang`, or `""`.
fn text<'a>(value: &'a Value, lang: &str) -> &'a str {
    value.get(lang).and_then(Value::as_str).unwrap_or("")
}

/// Every shipped starter part has an English and a Dutch description.
#[test]
fn every_starter_part_has_a_dutch_description() {
    let parts =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/libraries/ew-starter-parts.ewlib/parts");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&parts).unwrap().flatten() {
        for line in std::fs::read_to_string(entry.path()).unwrap().lines() {
            let rec: Value = serde_json::from_str(line).unwrap();
            let description = &rec["description"];
            assert!(!text(description, "en").is_empty(), "{}", rec["part_number"]);
            if text(description, "nl").trim().is_empty() {
                missing.push(rec["part_number"].to_string());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "parts without a Dutch description: {missing:?}"
    );
}

/// Every starter symbol, in both grid editions, has an English and a Dutch name. The junction dot and bevel
/// stay as they are: every project's output draws them from a compiled-in copy whose seal its derivation profile
/// pins (`ew-api`'s `output::view`), so their names change only with a new profile.
#[test]
fn every_starter_symbol_has_a_dutch_name() {
    let missing: Vec<&str> = starter::items()
        .iter()
        .filter(|i| i.kind == StarterKind::Symbol && !i.key.starts_with("ew.gen.junction-"))
        .filter(|i| {
            let doc: Value = serde_json::from_str(i.json).unwrap();
            text(&doc["name"], "en").is_empty() || text(&doc["name"], "nl").trim().is_empty()
        })
        .map(|i| i.key)
        .collect();
    assert!(missing.is_empty(), "symbols without a Dutch name: {missing:?}");
}
