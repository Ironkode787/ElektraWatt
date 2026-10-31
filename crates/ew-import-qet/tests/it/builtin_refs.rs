// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The built-in FnDef keys the importer can emit must exist in the starter content (`content/fndefs/`) with the
//! item id, library and version the importer's `LibRef` expects. Guards against the key drifting apart from the
//! content (a symbol and a FnDef must not share a key such as `ew.gen.ip`).

use std::path::{Path, PathBuf};

use ew_ids::Uuid;
use ew_import_qet::{BUILTIN_FNDEF_KEYS, BUILTIN_VERSION_REQ, IP_FNDEF_KEY};
use ew_interop_api::SYSTEM_LIBRARY;
use ew_schema::LibDocument;

fn content_dir(sub: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../content")
        .join(sub)
}

/// Every `*.json` of `content/<sub>` (not recursive) as a library document.
fn load(sub: &str) -> Vec<(String, LibDocument)> {
    let dir = content_dir(sub);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("reading content/{sub}: {e}")) {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        let doc = LibDocument::from_json(&text).unwrap_or_else(|e| panic!("content/{sub}/{name}: {e}"));
        out.push((name, doc));
    }
    assert!(!out.is_empty(), "content/{sub} has no documents");
    out
}

/// Whether `version` satisfies the caret requirement `req` (`^MAJOR.MINOR` or `^MAJOR`, the only forms the
/// importer emits): same major, and for major 0 the same minor, and at least the required minor otherwise.
fn satisfies(version: &str, req: &str) -> bool {
    let req = req.strip_prefix('^').expect("a caret requirement");
    let parse = |s: &str| -> Vec<u64> { s.split('.').map(|p| p.parse().expect("a number")).collect() };
    let (v, r) = (parse(version), parse(req));
    assert_eq!(v.len(), 3, "version {version} is not MAJOR.MINOR.PATCH");
    assert!((1..=2).contains(&r.len()), "unsupported requirement {req}");
    if v[0] != r[0] {
        return false;
    }
    match r.get(1) {
        None => true,
        Some(&min) if r[0] == 0 => v[1] == min,
        Some(&min) => v[1] >= min,
    }
}

#[test]
fn the_ip_key_is_the_fndef_not_the_symbol() {
    assert_eq!(IP_FNDEF_KEY, "ew.fn.ip");
    assert!(BUILTIN_FNDEF_KEYS.contains(&IP_FNDEF_KEY));
}

#[test]
fn every_builtin_fndef_key_exists_in_the_content_with_the_expected_reference() {
    let fndefs = load("fndefs");
    let symbols = load("symbols");
    for key in BUILTIN_FNDEF_KEYS {
        let found: Vec<_> = fndefs
            .iter()
            .filter_map(|(name, d)| match d {
                LibDocument::Fndef(c) if c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => {
                    Some((name, c))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            found.len(),
            1,
            "exactly one FnDef with key {key} in content/fndefs"
        );
        let (name, copy) = found[0];
        assert_eq!(
            copy.item_id.as_uuid(),
            Uuid::new_v5(&SYSTEM_LIBRARY, key.as_bytes()),
            "{name}: item_id is not v5(SYSTEM_LIBRARY, {key})"
        );
        assert_eq!(copy.library_id.as_uuid(), SYSTEM_LIBRARY, "{name}: library_id");
        assert!(
            satisfies(&copy.version, BUILTIN_VERSION_REQ),
            "{name}: version {} does not satisfy {BUILTIN_VERSION_REQ}",
            copy.version
        );
        // A symbol with the same key would share the FnDef's item id.
        for (sname, d) in &symbols {
            if let LibDocument::Symbol(s) = d {
                assert!(
                    s.item.meta.key.as_ref().is_none_or(|k| k.as_str() != key),
                    "{sname}: a symbol must not carry the built-in FnDef key {key}"
                );
            }
        }
    }
}

#[test]
fn version_requirement_check_is_sane() {
    assert!(satisfies("0.1.0", "^0.1"));
    assert!(satisfies("0.1.7", "^0.1"));
    assert!(!satisfies("0.2.0", "^0.1"));
    assert!(!satisfies("1.0.0", "^0.1"));
    assert!(satisfies("1.0.0", "^1"));
    assert!(!satisfies("0.1.0", "^1"));
}
