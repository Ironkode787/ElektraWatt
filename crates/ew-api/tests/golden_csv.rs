// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden CSV bootstrap (`ew-import-tables`, WP-S3.05; doc 20 §3.2 path (a)) loads through
//! `Session::from_records` with the model's integrity check, and the loaded model has the counts of doc 20.
//! It lives here because importers may not depend on the model (ARCH §3 rule 4).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::{Session, SessionOptions};
use ew_derive::FunctionCps;
use ew_import_tables::ew_interop_api::{ImportBatch, ImportInput, Severity};
use ew_import_tables::{BuiltinLibrary, TablesImporter, bootstrap_context};
use ew_schema::{ClampKey, Endpoint, LibDocument, Record};

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
    // arms that carry no wire ID), plus the 19 bridge connections of BR1 … BR6 (§8.3) and the two `Bus`
    // connections of the DP line (§11.1: the cable -3W1 on folio 3 and the page-less plug-on mating).
    assert_eq!(n("connection"), 72 + 19 + 2);
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
    // doc 20 §8.3: J1 … J6; §10: 13 conductors; §7.3: 19 potentials (DP included); §7.4: B1.
    assert_eq!(n("junction"), 6);
    assert_eq!(n("conductor"), 13);
    assert_eq!(n("potential_def"), 19);
    assert_eq!(n("bond"), 1);
    // doc 20 §11.2, §11.3, §6.2: one configuration project, 7 modules × 8 channels, 8 driven field devices.
    assert_eq!(n("plc_config_project"), 1);
    assert_eq!(n("channel"), 56);
    assert_eq!(n("device_link"), 8);
    // doc 20 §16 and §17.3 (doc 03 §20): 11 page-oriented report blocks and 4 embedded ones.
    assert_eq!(n("report_block"), 11 + 4);
    // doc 20 §14.1: 27 BOM rows, 23 with a part number and the 4 TBD test parts of rows 24–27; §14.2: the two
    // configured products of the assembly mode.
    let parts = batch
        .lib
        .iter()
        .filter(|d| matches!(d, LibDocument::Part(_)))
        .count();
    assert_eq!(parts, 27 + 2);
}

/// The rows of one committed golden table by key: column name → cell. The golden tables quote no field
/// (checked), so a plain split suffices here.
fn table(name: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let text = std::fs::read_to_string(repo().join("golden/J-E-2416/tables").join(name)).unwrap();
    assert!(!text.contains('"'), "{name}: this reader does not unquote");
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    lines
        .map(|l| {
            let row: BTreeMap<String, String> = header
                .iter()
                .zip(l.split(','))
                .map(|(h, c)| ((*h).to_owned(), c.to_owned()))
                .collect();
            (row["key"].clone(), row)
        })
        .collect()
}

/// The `{table}|{key}` source key of a record.
fn source_key(r: &Record) -> Option<&str> {
    let s = match r {
        Record::Representation(x) => x.source_ref.as_ref(),
        Record::Connection(x) => x.source_ref.as_ref(),
        Record::Device(x) => x.source_ref.as_ref(),
        Record::Function(x) => x.source_ref.as_ref(),
        _ => None,
    }?;
    Some(s.key.as_str())
}

/// F1 (review cycle 6 C): every representation's derived anchor cell (`rep_anchor_cell`, from the symbol's
/// anchor policy, D-14) is the cell of its `representations.csv` row — for named, origin and graphic-centre
/// anchors alike.
#[test]
fn every_representation_anchors_in_its_table_cell() {
    let batch = golden_batch();
    let s = session(&batch);
    let q = ew_derive::Q::new(&s.world().model);
    let rows = table("representations.csv");
    let mut checked = 0;
    let mut wrong = Vec::new();
    for r in &batch.records {
        let Record::Representation(rep) = r else {
            continue;
        };
        let key = source_key(r)
            .and_then(|k| k.strip_prefix("representations|"))
            .unwrap();
        let want = &rows[key]["cell"];
        let got = q.get::<ew_derive::RepAnchorCell>(rep.id);
        let got = got.as_ref().as_ref().map(ToString::to_string);
        if got.as_deref() != Some(want.as_str()) {
            wrong.push(format!("{key}: table {want}, derived {got:?}"));
        }
        checked += 1;
    }
    assert_eq!(checked, rows.len());
    assert!(
        wrong.is_empty(),
        "{} of {checked} off:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// F3 (review cycle 6 C): with the numbering settings of doc 20 §4.4 and the ▲U-34 frozen tags, every device's
/// derived tag (`device_tag`, structure omitted) is its table key as doc 20 prints it: `-101F1`, `-X1031`,
/// `S51`, `-201U3-Q1`, `-X1031:1`, `-XPE`, `-UC1-plate`.
#[test]
fn every_device_tag_is_its_table_key() {
    let batch = golden_batch();
    let s = session(&batch);
    let q = ew_derive::Q::new(&s.world().model);
    let mut checked = 0;
    let mut wrong = Vec::new();
    for r in &batch.records {
        let Record::Device(dev) = r else { continue };
        let key = source_key(r).unwrap();
        let key = key
            .strip_prefix("devices|")
            .or_else(|| key.strip_prefix("terminals|"))
            .unwrap();
        let tag = q.get::<ew_derive::DeviceTag>(dev.id);
        let tag = tag.as_ref().as_ref().expect("a tag");
        if tag.short() != key {
            wrong.push(format!("{key}: derived {} (full {})", tag.short(), tag.full));
        }
        checked += 1;
    }
    assert_eq!(checked, 68 + 28);
    assert!(
        wrong.is_empty(),
        "{} of {checked} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// F2 (review cycle 6 C): every connection endpoint CP exists among the derived CPs of its function
/// (`function_cps`), and every `/b` endpoint of `connections.csv` is clamp b. Terminal functions are
/// template-origin (their device carries PT 2.5-TWIN(-PE), whose template declares their FnDef with role main),
/// so the part's CP pattern (a, b, c, s1, RF) defines their CP index space.
#[test]
fn every_connection_endpoint_is_a_derived_cp() {
    let batch = golden_batch();
    let s = session(&batch);
    let q = ew_derive::Q::new(&s.world().model);
    let rows = table("connections.csv");
    let mut wrong = Vec::new();
    for r in &batch.records {
        let Record::Connection(c) = r else { continue };
        let key = source_key(r)
            .and_then(|k| k.strip_prefix("connections|"))
            .unwrap();
        for (end, text) in [(&c.a, &rows[key]["a"]), (&c.b, &rows[key]["b"])] {
            let Endpoint::Cp(f, idx) = end else { continue };
            let cps = q.get::<FunctionCps>(*f);
            match cps.iter().find(|cp| cp.idx == *idx) {
                None => wrong.push(format!("{key} {text}: no CP {idx} in {}", list(&cps))),
                Some(cp) if text.ends_with("/b") && cp.clamp != Some(ClampKey::Clamp('b')) => {
                    wrong.push(format!("{key} {text}: CP {idx} is {:?}, not clamp b", cp.clamp));
                }
                Some(_) => {}
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "{} endpoints wrong:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

fn list(cps: &[ew_derive::Cp]) -> String {
    let items: Vec<String> = cps
        .iter()
        .map(|c| {
            format!(
                "{}={}{}",
                c.idx,
                c.designation,
                c.clamp.as_ref().map_or(String::new(), |k| format!("/{k:?}"))
            )
        })
        .collect();
    format!("[{}]", items.join(", "))
}

/// Derived counts of the golden batch: doc 20 does not state them, they follow from its tables (functions:
/// one per device function and interruption point, §6, §9, §15; representations: one per drawn symbol, §6.1,
/// §9, §15). A change is a change of the fixture, never noise.
#[test]
fn the_golden_batch_has_160_functions_and_135_representations() {
    let batch = golden_batch();
    let n = |f: fn(&Record) -> bool| batch.records.iter().filter(|r| f(r)).count();
    assert_eq!(n(|r| matches!(r, Record::Function(_))), 160);
    assert_eq!(n(|r| matches!(r, Record::Representation(_))), 135);
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
