// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shipped identifier sets agree with `data/identifier-sets.toml` (ADR-0026 decision 3: sets are data).
//!
//! `src/sets/data.rs` is generated from the TOML file by [`generate`]. The test fails while the checked-in
//! file differs from what the TOML file generates; `EW_RDS_BLESS=1 cargo test -p ew-rds --test
//! identifier_sets` rewrites it. A second test reads the same TOML file through the public API.

// Test target: bless and diagnostic environment flags (EW_*), which only choose whether a committed file is rewritten or a line is printed; the compared result never depends on them.
#![allow(clippy::disallowed_methods)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use ew_rds::{EN_81346_2_2009, IdentifierSet};
use toml::{Table, Value};

struct SetMeta {
    key: String,
    title: String,
    reserved: Vec<char>,
}

struct Cell {
    letter: String,
    alternatives: Vec<String>,
    verified: bool,
}

struct Category {
    key: String,
    group: Option<String>,
    description: String,
    letters: BTreeMap<String, Cell>,
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn table<'a>(v: &'a Value, what: &str) -> &'a Table {
    v.as_table().unwrap_or_else(|| panic!("{what} must be a table"))
}

fn string(t: &Table, key: &str, what: &str) -> String {
    t.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{what}: `{key}` must be a string"))
        .to_owned()
}

fn strings(t: &Table, key: &str, what: &str) -> Vec<String> {
    match t.get(key) {
        None => Vec::new(),
        Some(v) => v
            .as_array()
            .unwrap_or_else(|| panic!("{what}: `{key}` must be an array"))
            .iter()
            .map(|s| {
                s.as_str()
                    .unwrap_or_else(|| panic!("{what}: `{key}` holds strings"))
                    .to_owned()
            })
            .collect(),
    }
}

fn only_keys(t: &Table, allowed: &[&str], what: &str) {
    for k in t.keys() {
        assert!(allowed.contains(&k.as_str()), "{what}: unknown key `{k}`");
    }
}

/// Reads the data file strictly: unknown keys, missing keys and wrong types fail.
fn load() -> (Vec<SetMeta>, Vec<Category>) {
    let path = manifest_dir().join("data/identifier-sets.toml");
    let text = std::fs::read_to_string(&path).expect("read data/identifier-sets.toml");
    let root: Table = text.parse().expect("data/identifier-sets.toml is valid TOML");
    only_keys(&root, &["set", "category"], "top level");

    let sets: Vec<SetMeta> = root["set"]
        .as_array()
        .expect("[[set]] array")
        .iter()
        .map(|v| {
            let t = table(v, "[[set]]");
            only_keys(t, &["key", "title", "reserved"], "[[set]]");
            let key = string(t, "key", "[[set]]");
            let reserved = strings(t, "reserved", &key)
                .iter()
                .map(|s| {
                    let mut chars = s.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) => c,
                        _ => panic!("{key}: reserved entries are single letters"),
                    }
                })
                .collect();
            SetMeta {
                title: string(t, "title", &key),
                key,
                reserved,
            }
        })
        .collect();
    let set_keys: BTreeSet<&str> = sets.iter().map(|s| s.key.as_str()).collect();

    let categories = root["category"]
        .as_array()
        .expect("[[category]] array")
        .iter()
        .map(|v| {
            let t = table(v, "[[category]]");
            only_keys(t, &["key", "group", "description", "letters"], "[[category]]");
            let key = string(t, "key", "[[category]]");
            let group = t.get("group").map(|_| string(t, "group", &key));
            let letters = table(&t["letters"], &key)
                .iter()
                .map(|(set, cell)| {
                    assert!(set_keys.contains(set.as_str()), "{key}: unknown set `{set}`");
                    let c = table(cell, &key);
                    only_keys(c, &["letter", "alternatives", "verified"], &key);
                    let verified = c
                        .get("verified")
                        .and_then(Value::as_bool)
                        .unwrap_or_else(|| panic!("{key}/{set}: `verified` must be a boolean"));
                    let cell = Cell {
                        letter: string(c, "letter", &key),
                        alternatives: strings(c, "alternatives", &key),
                        verified,
                    };
                    (set.clone(), cell)
                })
                .collect();
            Category {
                description: string(t, "description", &key),
                key,
                group,
                letters,
            }
        })
        .collect();
    (sets, categories)
}

/// The Rust source of `src/sets/data.rs`.
fn generate(sets: &[SetMeta], categories: &[Category]) -> String {
    let mut s = String::new();
    // The header of the generated file, not of this one. REUSE-IgnoreStart
    s.push_str(
        "// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors\n\
         // SPDX-License-Identifier: MIT OR Apache-2.0\n\
         \n\
         // @generated from data/identifier-sets.toml by tests/identifier_sets.rs. Do not edit: change the\n\
         // TOML file and run `EW_RDS_BLESS=1 cargo test -p ew-rds --test identifier_sets`.\n\
         \n\
         //! The shipped identifier sets.\n\
         \n\
         use super::{IdentifierSet, Mapping};\n\
         use crate::LetterCode;\n\
         \n\
         /// A letter code of the tables; an invalid one fails compilation.\n\
         const fn l(code: &'static str) -> LetterCode {\n    LetterCode::from_static(code)\n}\n\n",
    );
    // REUSE-IgnoreEnd
    let rows_name = |key: &str| -> String {
        let ident: String = key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        format!("ROWS_{ident}")
    };
    for set in sets {
        let rows: Vec<String> = categories
            .iter()
            .filter_map(|cat| {
                let cell = cat.letters.get(&set.key)?;
                let group = cat
                    .group
                    .as_ref()
                    .map_or("None".to_owned(), |g| format!("Some({g:?})"));
                let alternatives: Vec<String> =
                    cell.alternatives.iter().map(|a| format!("l({a:?})")).collect();
                Some(format!(
                    "    Mapping::from_static({:?}, {group}, l({:?}), &[{}], {}, {:?}),\n",
                    cat.key,
                    cell.letter,
                    alternatives.join(", "),
                    cell.verified,
                    cat.description
                ))
            })
            .collect();
        let _ = writeln!(s, "static {}: [Mapping; {}] = [", rows_name(&set.key), rows.len());
        s.extend(rows);
        s.push_str("];\n\n");
    }
    let _ = writeln!(
        s,
        "pub(super) static SHIPPED: [IdentifierSet; {}] = [",
        sets.len()
    );
    for set in sets {
        let reserved: Vec<String> = set.reserved.iter().map(|c| format!("{c:?}")).collect();
        let _ = writeln!(
            s,
            "    IdentifierSet::from_static({:?}, {:?}, &[{}], &{}),",
            set.key,
            set.title,
            reserved.join(", "),
            rows_name(&set.key)
        );
    }
    s.push_str("];\n");
    s
}

#[test]
fn generated_tables_match_the_data_file() {
    let (sets, categories) = load();
    let expected = generate(&sets, &categories);
    let path = manifest_dir().join("src/sets/data.rs");
    if std::env::var_os("EW_RDS_BLESS").is_some() {
        std::fs::write(&path, &expected).expect("write src/sets/data.rs");
        return;
    }
    let actual = std::fs::read_to_string(&path)
        .expect("read src/sets/data.rs")
        .replace("\r\n", "\n");
    assert!(
        actual == expected,
        "src/sets/data.rs is out of date with data/identifier-sets.toml; run \
         `EW_RDS_BLESS=1 cargo test -p ew-rds --test identifier_sets` and commit the result"
    );
}

#[test]
fn public_api_reflects_the_data_file() {
    let (sets, categories) = load();
    let shipped: Vec<&str> = IdentifierSet::shipped().iter().map(IdentifierSet::key).collect();
    let in_file: Vec<&str> = sets.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(shipped, in_file);
    assert_eq!(sets[0].key, EN_81346_2_2009, "the default set comes first");
    for meta in &sets {
        let set = IdentifierSet::by_key(&meta.key).unwrap();
        assert_eq!(set.title(), meta.title);
        assert_eq!(set.reserved(), meta.reserved.as_slice());
        let mut rows = 0;
        for cat in &categories {
            let Some(cell) = cat.letters.get(&meta.key) else {
                if cat.group.is_none() {
                    assert!(set.mapping(&cat.key, None).is_none(), "{}: {}", meta.key, cat.key);
                }
                continue;
            };
            rows += 1;
            let m = set
                .mapping(&cat.key, cat.group.as_deref())
                .unwrap_or_else(|| panic!("{}: no row for {}", meta.key, cat.key));
            assert_eq!(m.group(), cat.group.as_deref());
            assert_eq!(m.letter().as_str(), cell.letter);
            let alternatives: Vec<&str> = m.alternatives().iter().map(|a| a.as_str()).collect();
            assert_eq!(alternatives, cell.alternatives);
            assert_eq!(m.verified(), cell.verified);
            assert_eq!(m.description(), cat.description);
        }
        assert_eq!(set.rows().len(), rows, "{}", meta.key);
    }
}

#[test]
fn every_category_is_in_the_default_set() {
    let (_, categories) = load();
    let mut seen = BTreeSet::new();
    for cat in &categories {
        assert!(
            cat.letters.contains_key(EN_81346_2_2009),
            "{} has no row in the default set",
            cat.key
        );
        assert!(
            seen.insert((cat.key.clone(), cat.group.clone())),
            "{} listed twice",
            cat.key
        );
    }
}
