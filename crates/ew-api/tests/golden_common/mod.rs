// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden model (doc 20) loaded through the CSV bootstrap (`ew-import-tables`) and
//! `Session::from_records`, for the connectivity acceptance tests (WP-S1.07). The recipe is the one of
//! `tests/golden_csv.rs`.
//!
//! The bootstrap reads `colour_profile.csv` and `size_profile.csv` but does not emit them yet (decision log
//! ID-90). [`with_profiles`] adds them to `ProjectSettings.extra` in the doc 03 §5.1 shape, as the importer
//! will once it emits them, so that the wire-property rules of doc 20 §7.1–§7.2 can be tested.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::{Session, SessionOptions};
use ew_import_tables::ew_interop_api::{ImportBatch, ImportInput};
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use ew_schema::{LibDocument, Record};

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    paths.sort();
    paths
}

fn library() -> BuiltinLibrary {
    let mut docs = Vec::new();
    for sub in ["symbols/qet10", "fndefs", "forms"] {
        for p in files(&repo().join("content").join(sub), "json") {
            docs.push(LibDocument::from_json(&std::fs::read_to_string(&p).unwrap()).unwrap());
        }
    }
    BuiltinLibrary::new(docs).unwrap()
}

/// The golden import batch.
pub fn batch() -> ImportBatch {
    let inputs: Vec<ImportInput> = files(&repo().join("golden/J-E-2416/tables"), "csv")
        .iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            ImportInput::new(&name, std::fs::read(p).unwrap()).unwrap()
        })
        .collect();
    TablesImporter::new(library())
        .import(&inputs, &bootstrap_context())
        .expect("the golden tables import")
}

/// A session over the batch; `edit` may change the project record and the record list first.
pub fn session_with(
    batch: &ImportBatch,
    edit: impl FnOnce(&mut ew_schema::Project, &mut Vec<Record>),
) -> Session {
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let mut project = project.expect("the batch holds the project");
    edit(&mut project, &mut records);
    Session::from_records(project, records, batch.lib.clone(), SessionOptions::default())
        .expect("the golden batch passes the model's integrity check")
}

/// A session over the plain batch (no profiles).
pub fn session(batch: &ImportBatch) -> Session {
    session_with(batch, |_, _| {})
}

/// A session with the golden colour and size profiles in `ProjectSettings.extra`.
pub fn session_with_profiles(batch: &ImportBatch) -> Session {
    session_with(batch, |p, _| with_profiles(p))
}

/// The rows of one committed golden table by key: column name → cell (plain split, quoted fields kept
/// with their quotes stripped).
pub fn table(name: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let text = std::fs::read_to_string(repo().join("golden/J-E-2416/tables").join(name)).unwrap();
    let mut lines = text.lines();
    let header: Vec<String> = split(lines.next().unwrap());
    lines
        .map(|l| {
            let row: BTreeMap<String, String> = header.iter().cloned().zip(split(l)).collect();
            (row["key"].clone(), row)
        })
        .collect()
}

fn split(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Adds `colour_profile` and `size_profile` in the doc 03 §5.1 shape, built from the golden tables
/// (doc 20 §7.1, §7.2; doc 08 §4.2).
pub fn with_profiles(project: &mut ew_schema::Project) {
    let (colour, size) = profiles_json();
    project.settings.extra.insert("colour_profile".into(), colour);
    project.settings.extra.insert("size_profile".into(), size);
}

/// The two profile members as JSON.
pub fn profiles_json() -> (serde_json::Value, serde_json::Value) {
    use serde_json::json;
    let mut rows = Vec::new();
    for (_, r) in table("colour_profile.csv")
        .into_iter()
        .map(|(k, r)| (k.parse::<u32>().unwrap(), r))
        .collect::<BTreeMap<_, _>>()
    {
        let class = r["voltage_class"].as_str();
        let (ac_dc, u_min, u_max) = match class {
            "24 VDC" => ("dc", None, Some("120")),
            "230 VAC" => ("ac", Some("50"), Some("250")),
            "400 VAC" => ("ac", Some("250"), Some("1000")),
            _ => ("", None, None),
        };
        let pt = r["potential_type"].as_str();
        let (ptype, analogue): (Vec<&str>, bool) = match pt {
            "L+" => (vec!["l_plus"], false),
            "M" => (vec!["m"], false),
            "L" => (vec!["l"], false),
            "N" => (vec!["n"], false),
            "PE" => (vec!["pe"], false),
            "L1" => (vec!["l1"], false),
            "L2" => (vec!["l2"], false),
            "L3" => (vec!["l3"], false),
            "signal +" | "signal −" => (vec!["signal"], true),
            _ => (vec!["signal"], false),
        };
        let mut select = serde_json::Map::new();
        select.insert("ptype".into(), json!(ptype));
        // The PE rows select by type only (doc 08 §4.2); the others by AC/DC and voltage range.
        if pt != "PE" && !analogue {
            select.insert("ac_dc".into(), json!(ac_dc));
            if let Some(u) = u_min {
                select.insert("u_min".into(), json!(u));
            }
            if let Some(u) = u_max {
                select.insert("u_max".into(), json!(u));
            }
        }
        if analogue {
            select.insert("analogue".into(), json!(true));
        }
        rows.push(json!({
            "group": {"en": class},
            "function": {"en": r["function"]},
            "select": select,
            "colour": colour_json(&r["code"]),
        }));
    }
    let colour = json!({
        "key": "company.dbu-wh-24v",
        "name": {"en": "DBU/WH 24 V"},
        "basis": "company_convention",
        "verified": true,
        "codes": [{"code": "DBU", "base": "BU", "shade": "dark", "export_as": "DBU", "names": {"en": "Dark Blue"}}],
        "rows": rows,
    });
    let size = json!({
        "key": "company.0.75-1.5",
        "name": {"en": "0.75/1.5"},
        "verified": true,
        "class_rules": [
            {"select": {"ptype": ["pe", "pen", "fe"]}, "class": "pe"},
            {"select": {"discipline": "bus"}, "class": "bus"},
            {"select": {"analogue": true}, "class": "analogue"},
            {"select": {"ac_dc": "ac", "u_min": "50"}, "class": "mains"},
            {"select": {}, "class": "control"},
        ],
        "pe_class": "highest_at_ends",
        "bond_rule": {"colour_from": "pe", "class_from": "non_pe_side"},
        "rows": [{"class": "control", "csa": "0.75"}, {"class": "mains", "csa": "1.5"}],
    });
    (colour, size)
}

/// A colour code of the tables as a `ColourSpec` (`"GNYE"` is the two codes GN and YE).
pub fn colour_json(code: &str) -> serde_json::Value {
    match code {
        "GNYE" => serde_json::json!(["GN", "YE"]),
        c => serde_json::json!([c]),
    }
}

/// The source key (`connections|W29`, `functions|-102F2`, …) of a record.
pub fn source_key(r: &Record) -> Option<&str> {
    let s = match r {
        Record::Representation(x) => x.source_ref.as_ref(),
        Record::Connection(x) => x.source_ref.as_ref(),
        Record::Device(x) => x.source_ref.as_ref(),
        Record::Function(x) => x.source_ref.as_ref(),
        Record::Junction(x) => x.source_ref.as_ref(),
        Record::PotentialDef(x) => x.source_ref.as_ref(),
        Record::Bond(x) => x.source_ref.as_ref(),
        Record::Conductor(x) => x.source_ref.as_ref(),
        _ => None,
    }?;
    Some(s.key.as_str())
}

/// Every record's ID (as text) by its source key, such as `"connections|W29"` → the connection ID.
pub fn ids_by_key(batch: &ImportBatch) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for r in &batch.records {
        let Some(k) = source_key(r) else { continue };
        let v = serde_json::to_value(r).unwrap();
        if let Some(id) = v.get("id").and_then(|i| i.as_str()) {
            out.insert(k.to_owned(), id.to_owned());
        }
    }
    out
}
