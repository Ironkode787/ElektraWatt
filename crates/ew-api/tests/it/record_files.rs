// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1290 (decision log ID-1312): `merge.resolve` routes a record to its file by `ew-commands`' own copy of doc 05
//! §2.2's rules, since `ew-commands` has no `ew-format` edge. This crate sees both, and pins the copy to
//! `ew_format::record_file`, the writer's, over Golden A's records and conflict records, kind by kind.

// The model is built from records through ew-model's raw API (dev-dependency `ew-model/raw-read`).
#![allow(clippy::disallowed_methods)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ew_commands::commands::record_file_of;
use ew_model::Model;
use ew_schema::{Kind, KindStatus, Record};
use ew_store::ew_format::{FilePath, Limits, RecordAnchors, load, record_file};
use serde_json::json;

/// Golden A's files, read only, by project-relative path.
fn golden() -> BTreeMap<FilePath, Vec<u8>> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<FilePath, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .expect("below the base")
                    .to_string_lossy()
                    .replace('\\', "/");
                if let Ok(rel) = FilePath::new(rel) {
                    out.insert(rel, std::fs::read(&path).expect("a readable file"));
                }
            }
        }
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    let mut out = BTreeMap::new();
    walk(&dir, &dir, &mut out);
    out
}

fn record(v: serde_json::Value) -> Record {
    Record::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

#[test]
fn merge_resolve_routes_every_record_to_the_file_the_writer_writes_it_in() {
    let loaded = load(None, &golden(), &Limits::default());
    let project = loaded.project.clone().expect("Golden A's project");
    let mut records = loaded.records.clone();
    // Conflict records: on a connection of Golden A, on a record that does not exist, and of whole record files (the
    // merge driver writes a file-level conflict only into a record file).
    let connection = records
        .iter()
        .find_map(|r| match r {
            Record::Connection(c) => Some(c.id.to_string()),
            _ => None,
        })
        .expect("a connection");
    let page_id = records
        .iter()
        .find_map(|r| match r {
            Record::Page(p) => Some(p.id.to_string()),
            _ => None,
        })
        .expect("a page");
    let page = format!("pages/{page_id}.jsonl");
    // The kinds Golden A holds no record of.
    records.push(record(json!({
        "$k": "structure_box", "id": "5a0c1e2d-3b4f-5a61-8000-000000000005", "page": page_id,
        "outline": [[0, 0], [10, 0], [10, 10], [0, 10]], "style": "dash_dot", "assigns": {}
    })));
    records.push(record(json!({
        "$k": "macro_instance", "id": "4b7d2e91-0c58-5a36-8f1e-d9a3c6b07e24",
        "macro": "a6e3f0b8-5d12-5c79-9b4a-1e8d7c2f0a63", "variant": "A", "members": [], "page": page_id
    })));
    records.push(record(json!({
        "$k": "waiver", "id": "6c1d0a37-92e4-5f8b-a0d6-3e7b9c2f4a18", "rule": "EW-CORE-041",
        "target": {"finding": "9c2e5a0b7d14f3e8a6c1b05d2e9f41ab"}, "reason": "a reason", "author": "engineer",
        "date": "2025-04-14", "expires": "never"
    })));
    records.push(record(json!({
        "$k": "conflict", "id": connection, "kind": "connection", "base": {}, "ours": {}, "theirs": {}
    })));
    records.push(record(json!({
        "$k": "conflict", "id": "0190d8a4-6d3e-7c41-8f00-000000000001", "kind": "device", "base": {}, "ours": {},
        "theirs": {}
    })));
    for (n, path) in [page.as_str(), "waivers.jsonl"].into_iter().enumerate() {
        records.push(record(json!({
            "$k": "conflict", "id": format!("0190d8a4-6d3e-7c41-8f00-00000000001{n}"), "kind": "file", "path": path,
            "side": "theirs", "lines": []
        })));
    }
    let anchors = RecordAnchors::new(&records);
    // The macro instance names a macro the project does not hold: it stays out of the model, which its file (its
    // page's) does not read.
    let in_model = records
        .iter()
        .filter(|r| !matches!(r, Record::MacroInstance(_)))
        .cloned();
    let model = Model::from_records(project, in_model, loaded.lib.clone()).expect("the model is valid");
    let mut kinds = BTreeSet::new();
    for r in &records {
        let writer = record_file(r, &anchors).ok().map(|f| f.as_str().to_owned());
        assert_eq!(record_file_of(&model, r), writer, "{r:?}");
        kinds.extend(r.kind());
    }
    // Every kind of record Golden A writes is compared.
    let missing: Vec<Kind> = Kind::ALL
        .into_iter()
        .filter(|k| k.status() == KindStatus::Implemented && !k.is_library() && *k != Kind::Project)
        .filter(|k| !kinds.contains(k))
        .collect();
    assert_eq!(missing, [], "kinds no record of the test holds");
}
