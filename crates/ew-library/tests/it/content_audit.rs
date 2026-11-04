// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The content audit of WP-S7.03 D1, D2 and D4 (NFR-LIC-06, NFR-LIC-07): every library document under `content/` and
//! every copy under `golden/J-E-2416.ew/lib/` (the sample project ships, AC-40) carries an SPDX licence, the licence
//! of its kind (doc 07 §3.8: CC0-1.0 for part facts, CC-BY-SA-4.0 for drawn content), allowed by its file's row of
//! `ci/licence-map.toml` under `content/`, and a source. Every map expression and designation pattern of a shipped
//! part parses, and `content/README.md` lists every content set of `content/` (RC-07's inventory).
//!
//! The files are read as bytes, never through a store: Golden A is only read.

use std::path::{Path, PathBuf};

use ew_derive::ew_plcaddr::MapExpr;
use ew_derive::{PatternError, expand_pattern};
use serde_json::Value;

/// The repository root.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The licence a record of this kind ships under (doc 07 §3.8; WP-S7.03 D7 for the address profiles of `content/**`).
fn kind_licence(kind: &str) -> &'static str {
    match kind {
        "part" | "manufacturer" => "CC0-1.0",
        _ => "CC-BY-SA-4.0",
    }
}

/// The `[[path]]` rows of `ci/licence-map.toml`: `(glob, allowed)`.
fn licence_rows(text: &str) -> Vec<(String, Vec<String>)> {
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    let quoted = |s: &str| -> Vec<String> { s.split('"').skip(1).step_by(2).map(str::to_owned).collect() };
    for line in text.lines() {
        let line = line.trim();
        if line == "[[path]]" {
            rows.push((String::new(), Vec::new()));
        } else if let (Some(row), Some(v)) = (rows.last_mut(), line.strip_prefix("glob = ")) {
            row.0 = quoted(v).concat();
        } else if let (Some(row), Some(v)) = (rows.last_mut(), line.strip_prefix("allowed = ")) {
            row.1 = quoted(v);
        }
    }
    rows
}

/// Whether `path` matches a REUSE-style glob (`*` any run without `/`, `**` any run).
fn glob_match(glob: &[u8], path: &[u8]) -> bool {
    match glob {
        [] => path.is_empty(),
        [b'*', b'*', rest @ ..] => (0..=path.len()).any(|i| glob_match(rest, &path[i..])),
        [b'*', rest @ ..] => (0..=path.len())
            .take_while(|&i| i == 0 || path[i - 1] != b'/')
            .any(|i| glob_match(rest, &path[i..])),
        [c, rest @ ..] => path.first() == Some(c) && glob_match(rest, &path[1..]),
    }
}

/// The allowed licences of the longest glob that matches `path`.
fn allowed_for<'a>(rows: &'a [(String, Vec<String>)], path: &str) -> &'a [String] {
    rows.iter()
        .filter(|(g, _)| glob_match(g.as_bytes(), path.as_bytes()))
        .max_by_key(|(g, _)| g.len())
        .map_or(&[], |(_, a)| a.as_slice())
}

/// Every file under `dir`, recursively, in name order.
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read.map(|e| e.expect("a directory entry").path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// `path` relative to `root`, with `/`.
fn rel(root: &Path, path: &Path) -> String {
    let r = path.strip_prefix(root).expect("a path under the root");
    r.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The library records of a file: one document per `.json` file, one per line of a `.jsonl` file; other files and
/// JSON without `$k` (a manifest, a settings value) hold none.
fn records(path: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let docs: Vec<Value> = match path.extension().and_then(|e| e.to_str()) {
        Some("json") => {
            vec![serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))]
        }
        Some("jsonl") => text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
            .collect(),
        _ => Vec::new(),
    };
    docs.into_iter().filter(|d| d.get("$k").is_some()).collect()
}

/// The key a failure names: `meta.key`, else the item's own `key` (a manufacturer), else its ID.
fn item_key(doc: &Value) -> String {
    [&doc["meta"]["key"], &doc["key"], &doc["id"]]
        .into_iter()
        .find_map(Value::as_str)
        .unwrap_or("?")
        .to_owned()
}

/// The map expressions and designation patterns of a part's PLC section that do not parse (WP-S7.03 D4), as
/// `<member> = "<expr>" is no map expression: <reason>`.
fn map_failures(doc: &Value) -> Vec<String> {
    let plc = &doc["sections"]["plc"];
    let mut out = Vec::new();
    let mut expr = |member: &str, v: &Value| {
        if let Some(s) = v.as_str()
            && MapExpr::parse(s).is_none()
        {
            let reason = PatternError::BadExpression(s.to_owned());
            out.push(format!("{member} = \"{s}\" is no map expression: {reason}"));
        }
    };
    expr("bit_of", &plc["bit_of"]);
    for m in plc["positions"]["map"].as_array().into_iter().flatten() {
        expr("positions.channel", &m["channel"]);
    }
    for (member, v) in [
        ("channels.designation", &plc["channels"]["designation"]),
        ("pin_of", &plc["pin_of"]),
    ] {
        if let Some(s) = v.as_str()
            && let Err(reason) = expand_pattern(s, 0)
        {
            out.push(format!("{member} = \"{s}\" is no map expression: {reason}"));
        }
    }
    out
}

/// The content sets of `content/`: every folder that holds a file other than a `README.md`, a `.ewlib` library as
/// one set (its record directories are its own).
fn content_sets(root: &Path) -> Vec<String> {
    let mut all = Vec::new();
    files(&root.join("content"), &mut all);
    let mut sets: Vec<String> = all
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n != "README.md"))
        .map(|p| {
            let dir = rel(root, p.parent().expect("a folder"));
            match dir.find(".ewlib") {
                Some(i) => dir[..i + ".ewlib".len()].to_owned(),
                None => dir,
            }
        })
        .collect();
    sets.sort();
    sets.dedup();
    sets
}

/// The folders `content/README.md` lists: the first cell of each table row, in backticks.
fn listed_sets(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join("content/README.md")).unwrap_or_default();
    text.lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .map(|s| s.trim_end_matches('/').to_owned())
        .collect()
}

/// The audit of `root`'s `content/` and `golden/J-E-2416.ew/lib/`: one line per failure, in path order (T4).
fn audit(root: &Path) -> Vec<String> {
    let rows = licence_rows(&std::fs::read_to_string(root.join("ci/licence-map.toml")).unwrap_or_default());
    let mut out = Vec::new();
    let mut all = Vec::new();
    files(&root.join("content"), &mut all);
    files(&root.join("golden/J-E-2416.ew/lib"), &mut all);
    for path in &all {
        let p = rel(root, path);
        for doc in records(path) {
            let key = item_key(&doc);
            let kind = doc["$k"].as_str().unwrap_or_default();
            match doc["meta"]["licence"].as_str().filter(|l| !l.is_empty()) {
                None => out.push(format!("{p}: {key}: no licence")),
                Some(l) if l != kind_licence(kind) => out.push(format!(
                    "{p}: {key}: the licence {l} is not allowed here ({})",
                    kind_licence(kind)
                )),
                Some(l) => {
                    let allowed = allowed_for(&rows, &p);
                    if p.starts_with("content/") && !allowed.iter().any(|a| a == l) {
                        out.push(format!(
                            "{p}: {key}: the licence {l} is not allowed here ({})",
                            allowed.join(", ")
                        ));
                    }
                }
            }
            let sources = doc["meta"]["sources"].as_array();
            if !sources.is_some_and(|s| s.iter().any(|s| s.as_str().is_some_and(|s| !s.is_empty()))) {
                out.push(format!("{p}: {key}: no source"));
            }
            if kind == "part" {
                out.extend(map_failures(&doc).into_iter().map(|f| format!("{p}: {key}: {f}")));
            }
        }
    }
    let listed = listed_sets(root);
    for set in content_sets(root) {
        if !listed.contains(&set) {
            out.push(format!("{set}: not listed in content/README.md"));
        }
    }
    out
}

/// A fixture root: the repository's licence map and `content/README.md`, then the given files.
fn fixture(stem: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = ew_testdir::fresh_dir(stem);
    for (path, text) in [
        (
            "ci/licence-map.toml",
            std::fs::read_to_string(repo().join("ci/licence-map.toml")).unwrap(),
        ),
        (
            "content/README.md",
            std::fs::read_to_string(repo().join("content/README.md")).unwrap(),
        ),
    ]
    .into_iter()
    .chain(files.iter().map(|(p, t)| (*p, (*t).to_owned())))
    {
        let p = root.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    root
}

/// SC-1, SC-2: the repository's shipped records pass the audit, and `content/README.md` lists every content set.
#[test]
fn the_shipped_content_passes_the_audit() {
    assert_eq!(audit(&repo()), Vec::<String>::new());
}

/// SC-1: Golden A's 29 part and 8 manufacturer copies carry `CC0-1.0` (the sample project's fixed set).
#[test]
fn golden_as_part_and_manufacturer_copies_carry_cc0() {
    for (dir, count) in [("parts", 29), ("manufacturers", 8)] {
        let mut all = Vec::new();
        files(&repo().join("golden/J-E-2416.ew/lib").join(dir), &mut all);
        assert_eq!(all.len(), count, "{dir}");
        for p in &all {
            for doc in records(p) {
                assert_eq!(doc["meta"]["licence"], "CC0-1.0", "{}", p.display());
            }
        }
    }
}

/// SC-2 (NFR-LIC-06): no shipped record names a vendor file: every source is the project's own (`ew.original`), a
/// golden table row or a repository document.
#[test]
fn no_shipped_record_names_a_vendor_file() {
    let mut all = Vec::new();
    files(&repo().join("content"), &mut all);
    files(&repo().join("golden/J-E-2416.ew/lib"), &mut all);
    for p in &all {
        for doc in records(p) {
            for s in doc["meta"]["sources"].as_array().into_iter().flatten() {
                let s = s.as_str().unwrap_or_default();
                assert!(
                    s == "ew.original" || s.starts_with("golden.table:") || s.starts_with("docs/"),
                    "{}: {}: the source {s}",
                    p.display(),
                    item_key(&doc)
                );
            }
        }
    }
}

/// SC-1, SC-2, SC-3 and T4: the failures, whole.
#[test]
fn the_audit_names_each_failure_whole() {
    let part = r#"{"$k":"part","id":"00000000-0000-5000-8000-000000000001","version":"0.1.0","meta":{"key":"FX-1","sources":["ew.original"]},"part_number":"FX-1"}"#;
    let form = r#"{"$k":"form","id":"00000000-0000-5000-8000-000000000002","version":"0.1.0","meta":{"key":"fx.form","licence":"CC0-1.0","sources":["ew.original"]}}"#;
    let unsourced = r#"{"$k":"manufacturer","id":"00000000-0000-5000-8000-000000000003","version":"0.1.0","meta":{"licence":"CC0-1.0"},"key":"Fixture"}"#;
    let mapped = r#"{"$k":"part","id":"00000000-0000-5000-8000-000000000004","version":"0.1.0","meta":{"key":"FX-2","licence":"CC0-1.0","sources":["ew.original"]},"sections":{"plc":{"role":"module","bit_of":"n-1"}}}"#;
    let root = fixture(
        "content-audit",
        &[
            ("content/libraries/ew-starter-parts.ewlib/parts/01.1.jsonl", part),
            (
                "content/libraries/ew-starter-parts.ewlib/manufacturers.1.jsonl",
                unsourced,
            ),
            (
                "content/libraries/ew-starter-parts.ewlib/parts/04.1.jsonl",
                mapped,
            ),
            ("content/forms/fx.form.json", form),
            ("content/unlisted/fx.json", "{}"),
        ],
    );
    assert_eq!(
        audit(&root),
        [
            "content/forms/fx.form.json: fx.form: the licence CC0-1.0 is not allowed here (CC-BY-SA-4.0)",
            "content/libraries/ew-starter-parts.ewlib/manufacturers.1.jsonl: Fixture: no source",
            "content/libraries/ew-starter-parts.ewlib/parts/01.1.jsonl: FX-1: no licence",
            "content/libraries/ew-starter-parts.ewlib/parts/04.1.jsonl: FX-2: bit_of = \"n-1\" is no map expression: \
             `{n-1}` is not an expression in n (`n`, `2n`, `n+1`, `2n+1`)",
            "content/unlisted: not listed in content/README.md",
        ]
    );
    // A part licensed as its kind wants, in a file whose licence-map row allows another licence.
    let root = fixture(
        "content-audit-map",
        &[(
            "content/symbols/fx.json",
            r#"{"$k":"part","id":"00000000-0000-5000-8000-000000000005","version":"0.1.0","meta":{"key":"FX-3","licence":"CC0-1.0","sources":["ew.original"]}}"#,
        )],
    );
    assert_eq!(
        audit(&root),
        ["content/symbols/fx.json: FX-3: the licence CC0-1.0 is not allowed here (CC-BY-SA-4.0)"]
    );
    // A designation pattern takes the expander's reason.
    let root = fixture(
        "content-audit-pattern",
        &[(
            "content/libraries/ew-starter-parts.ewlib/parts/06.1.jsonl",
            r#"{"$k":"part","id":"00000000-0000-5000-8000-000000000006","version":"0.1.0","meta":{"key":"FX-4","licence":"CC0-1.0","sources":["ew.original"]},"sections":{"plc":{"role":"module","pin_of":"X2:{n+1"}}}"#,
        )],
    );
    assert_eq!(
        audit(&root),
        [
            "content/libraries/ew-starter-parts.ewlib/parts/06.1.jsonl: FX-4: pin_of = \"X2:{n+1\" is no map expression: \
             a `{` is not closed"
        ]
    );
}
