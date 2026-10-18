// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden CSV bootstrap (`ew-import-tables`, WP-S3.05; doc 20 §3.2 path (a)) loads through
//! `Session::from_records` with the model's integrity check, and the loaded model has the counts of doc 20.
//! It lives here because importers may not depend on the model (ARCH §3 rule 4).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::{Session, SessionOptions};
use ew_import_tables::ew_interop_api::{ImportBatch, ImportInput, Severity};
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use ew_schema::{LibDocument, Record};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json_files(dir: &Path, ext: &str) -> Vec<PathBuf> {
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
        for p in json_files(&repo().join("content").join(sub), "json") {
            docs.push(LibDocument::from_json(&std::fs::read_to_string(&p).unwrap()).unwrap());
        }
    }
    BuiltinLibrary::new(docs).unwrap()
}

fn golden_batch() -> ImportBatch {
    let inputs: Vec<ImportInput> = json_files(&repo().join("golden/J-E-2416/tables"), "csv")
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

fn session(batch: &ImportBatch) -> Session {
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    Session::from_records(
        project.expect("the batch holds the project"),
        records,
        batch.lib.clone(),
        SessionOptions::default(),
    )
    .expect("the golden batch passes the model's integrity check")
}

#[test]
fn the_golden_tables_import_without_errors() {
    let batch = golden_batch();
    let errors: Vec<_> = batch
        .report
        .entries
        .iter()
        .filter(|e| e.severity != Severity::Info)
        .collect();
    assert!(errors.is_empty(), "unexpected findings: {errors:#?}");
}

#[test]
fn the_golden_batch_loads_with_the_counts_of_doc_20() {
    let batch = golden_batch();
    let s = session(&batch);
    let world = s.world();
    let mut per_kind: BTreeMap<String, usize> = BTreeMap::new();
    for ((kind, _), _) in ew_commands::rows::model_lines(&world.model).unwrap() {
        *per_kind.entry(kind.name().to_owned()).or_default() += 1;
    }
    let n = |k: &str| per_kind.get(k).copied().unwrap_or(0);
    // doc 20 §5 (doc 03 §20): 13 drawn pages; the other 23 of the 36 are generated.
    assert_eq!(n("page"), 13);
    // doc 20 §4.3: sections 1, 2, 9; =P01, +J-E-2416, +J-E-206, +FIELD; 4 external points (§15).
    assert_eq!(n("section"), 3);
    assert_eq!(n("structure_node"), 4);
    assert_eq!(n("external_ref"), 4);
    // doc 20 §6.1: 68 devices (rows 1–68), plus the 3 + 20 + 5 terminal devices of §9.
    assert_eq!(n("device"), 68 + 28);
    // doc 20 §15: 22 IP pairs and 4 external points = 48 interruption-point functions.
    let ips = batch
        .records
        .iter()
        .filter(|r| matches!(r, Record::Function(f) if matches!(f.kind, ew_schema::FunctionKind::Ip(_))))
        .count();
    assert_eq!(ips, 48);
    // doc 20 §8.2: 44 physical wires drawn as 72 connections (22 through IP pairs as two parts, 6 junction
    // arms that carry no wire ID), plus the 19 bridge connections of BR1 … BR6 (§8.3).
    assert_eq!(n("connection"), 72 + 19);
    let wire_ids = batch
        .records
        .iter()
        .filter(|r| {
            matches!(r, Record::Connection(c) if c.source_ref.as_ref().is_some_and(|s| {
                let key = s.key.strip_prefix("connections|W").unwrap_or("");
                !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit())
            }))
        })
        .count();
    assert_eq!(wire_ids, 44);
    // doc 20 §8.3: J1 … J6; §10: 13 conductors; §7.3: 19 potentials less DP (bus, §11) = 18; §7.4: B1.
    assert_eq!(n("junction"), 6);
    assert_eq!(n("conductor"), 13);
    assert_eq!(n("potential_def"), 18);
    assert_eq!(n("bond"), 1);
    // doc 20 §16 and §17.3 (doc 03 §20): 11 page-oriented report blocks and 4 embedded ones.
    assert_eq!(n("report_block"), 11 + 4);
    // doc 20 §14.1: 23 BOM rows with a part number (rows 24–27 are TBD).
    let parts = batch
        .lib
        .iter()
        .filter(|d| matches!(d, LibDocument::Part(_)))
        .count();
    assert_eq!(parts, 23);
}

#[test]
fn the_golden_import_is_deterministic() {
    let a = golden_batch();
    let b = golden_batch();
    let lines = |batch: &ImportBatch| -> Vec<String> {
        batch
            .records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .chain(batch.lib.iter().map(|d| serde_json::to_string(d).unwrap()))
            .collect()
    };
    assert_eq!(lines(&a), lines(&b));
}
