// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The layout files (doc 05 §2.2; WP-S4.01 D3, LM-1): every layout record lives in `layout/<space>.jsonl`, the space
//! reached through carrier → surface → enclosure; a broken chain and a newer carrier form still give one
//! deterministic file; a directory of layout records loads clean, a file written while the kinds were reserved is
//! rewritten canonically, and a placement in another space's file is EW-CORE-010.

use std::collections::BTreeMap;

use ew_format::ew_schema::{Project, Record};
use ew_format::{
    FilePath, FindingKind, Limits, RecordAnchors, load, project_files, record_file, record_line,
};

const SPACE_A: &str = "4f1c8a27-6d93-5b05-8e2a-c7d0b9f3a164";
const SPACE_B: &str = "b7e3d0a5-1c48-5f92-9a6e-2d0c7f4b8e13";
const LOC_A: &str = "a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60";
const LOC_B: &str = "2b6e4f81-9c03-5d7a-8e45-1f0a3c6d9b72";
const ENCL_A: &str = "a92e5d10-3b7c-5f48-9d61-0e4f8c2a7b35";
const ENCL_B: &str = "5e0d2c7b-94a1-5b36-8f0e-c3a7d1b5e924";
const SURF_A: &str = "17d4b3e8-c05a-5926-8b7f-e2a9d0c6f143";
const SURF_B: &str = "e8a1c6f3-27b0-5d94-a5c2-0b9e4d7f1a68";
const RAIL_A: &str = "c3a0f6e2-58d1-5b74-a9e0-4d7b1c8f2e96";
const PL_RAIL: &str = "d0f7a3c9-2e64-5185-8a3b-5c9e1f0d7a26";
const PL_SURF: &str = "28c5e1b7-9a0d-5f63-b4e8-0d2a7f3c9e51";
const DEV_1: &str = "c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2";
const DEV_2: &str = "0b9d4e62-8f13-5c75-a2d0-3e7f1b6c9a84";
const DEV_UC1: &str = "93e0b4a6-1c7f-5d28-a5e9-8b3d6f0c1a72";
const DEV_UC2: &str = "6a2f9c41-0d85-5e37-b1a4-e9c3d7f0b256";
const DANGLING: &str = "ffffffff-0000-5000-8000-000000000001";
const PROJECT: &str =
    r#"{"$k":"project","id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;
const IDENTITY: &str = r#"{"t":[0,0,0],"q":[1000000000,0,0,0]}"#;

type Files = BTreeMap<FilePath, Vec<u8>>;

fn record(line: &str) -> Record {
    Record::from_json(line).unwrap_or_else(|e| panic!("{e}: {line}"))
}

fn space(id: &str, loc: &str) -> String {
    format!(r#"{{"$k":"layout_space","id":"{id}","location":"{loc}"}}"#)
}

fn enclosure(id: &str, space: &str, device: &str) -> String {
    format!(r#"{{"$k":"enclosure","id":"{id}","space":"{space}","device":"{device}","pose":{IDENTITY}}}"#)
}

fn surface(id: &str, enclosure: &str) -> String {
    format!(
        r#"{{"$k":"mounting_surface","id":"{id}","enclosure":"{enclosure}","kind":"plate","frame":{IDENTITY}}}"#
    )
}

fn rail(id: &str, surface: &str) -> String {
    format!(
        r#"{{"$k":"carrier","id":"{id}","surface":"{surface}","kind":"rail","path":[[0,0],[500000000,0]],"params":{{"rail":{{"profile":"ts35x7_5"}}}}}}"#
    )
}

fn placement(id: &str, device: &str, carrier: &str) -> String {
    format!(r#"{{"$k":"placement","id":"{id}","device":"{device}","carrier":{carrier},"pose":{IDENTITY}}}"#)
}

/// Two spaces, A with an enclosure, a plate, a rail, a placement on the rail and one on the plate, B with an
/// enclosure and a plate.
fn lines() -> Vec<String> {
    vec![
        space(SPACE_A, LOC_A),
        space(SPACE_B, LOC_B),
        enclosure(ENCL_A, SPACE_A, DEV_UC1),
        enclosure(ENCL_B, SPACE_B, DEV_UC2),
        surface(SURF_A, ENCL_A),
        surface(SURF_B, ENCL_B),
        rail(RAIL_A, SURF_A),
        placement(PL_RAIL, DEV_1, &format!(r#"{{"carrier":"{RAIL_A}"}}"#)),
        placement(PL_SURF, DEV_2, &format!(r#"{{"surface":"{SURF_A}"}}"#)),
    ]
}

fn records() -> Vec<Record> {
    lines().iter().map(|l| record(l)).collect()
}

fn layout_file(space: &str) -> FilePath {
    FilePath::new(&format!("layout/{space}.jsonl")).unwrap()
}

fn files_of(records: &[Record]) -> Files {
    let Record::Project(project) = record(PROJECT) else {
        panic!("a project")
    };
    let project: Project = project;
    let mut files = project_files(&project, records, &[], 1, "test").unwrap();
    files.remove(&FilePath::new("manifest.json").unwrap());
    files
}

/// Every layout record of space A lives in A's file, B's records in B's.
#[test]
fn every_layout_record_lives_in_the_file_of_its_space() {
    let records = records();
    let anchors = RecordAnchors::new(&records);
    for r in &records {
        let file = record_file(r, &anchors).unwrap();
        let in_b = matches!(r.id().map(|id| id.to_string()), Some(id) if id == SPACE_B || id == ENCL_B || id == SURF_B);
        let expected = layout_file(if in_b { SPACE_B } else { SPACE_A });
        assert_eq!(file, expected, "{r:?}");
    }
}

/// LM-1 at the format level: moving the rail to space B's plate moves the rail and the placement on it to B's file;
/// the placement on A's plate stays in A's.
#[test]
fn moving_a_carrier_re_keys_its_placements() {
    let mut records = records();
    for r in &mut records {
        if let Record::Carrier(c) = r {
            c.surface = SURF_B.parse().unwrap();
        }
    }
    let anchors = RecordAnchors::new(&records);
    let file_of = |id: &str| {
        let r = records
            .iter()
            .find(|r| r.id().map(|u| u.to_string()).as_deref() == Some(id))
            .unwrap();
        record_file(r, &anchors).unwrap()
    };
    assert_eq!(file_of(RAIL_A), layout_file(SPACE_B));
    assert_eq!(file_of(PL_RAIL), layout_file(SPACE_B));
    assert_eq!(file_of(PL_SURF), layout_file(SPACE_A));
    let files = files_of(&records);
    let b = String::from_utf8(files[&layout_file(SPACE_B)].clone()).unwrap();
    assert!(
        b.contains(PL_RAIL) && b.contains(RAIL_A) && !b.contains(PL_SURF),
        "{b}"
    );
}

/// A broken chain gives the file of the first ID it cannot resolve; a placement on a carrier form of a newer format
/// gives the file of its own ID.
#[test]
fn a_broken_chain_and_a_newer_carrier_form_still_give_one_file() {
    let anchors = RecordAnchors::new(&records());
    let dangling = record(&placement(
        PL_RAIL,
        DEV_1,
        &format!(r#"{{"carrier":"{DANGLING}"}}"#),
    ));
    assert_eq!(record_file(&dangling, &anchors).unwrap(), layout_file(DANGLING));
    let orphan_surface = record(&surface(SURF_A, DANGLING));
    assert_eq!(
        record_file(&orphan_surface, &anchors).unwrap(),
        layout_file(DANGLING)
    );
    let newer = record(&placement(
        PL_RAIL,
        DEV_1,
        &format!(r#"{{"placement":"{PL_SURF}"}}"#),
    ));
    assert_eq!(record_file(&newer, &anchors).unwrap(), layout_file(PL_RAIL));
}

/// LM-1: a directory of layout records loads clean and gives back the same records.
#[test]
fn a_layout_directory_loads_clean() {
    let files = files_of(&records());
    assert!(files.contains_key(&layout_file(SPACE_A)) && files.contains_key(&layout_file(SPACE_B)));
    let loaded = load(None, &files, &Limits::default());
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty());
    let mut back: Vec<String> = loaded.records.iter().map(|r| record_line(r).unwrap()).collect();
    let mut expected: Vec<String> = records().iter().map(|r| record_line(r).unwrap()).collect();
    back.sort();
    expected.sort();
    assert_eq!(back, expected);
}

/// Layout records written while the kinds were reserved (kept verbatim, every object's members in byte-wise key
/// order) load typed; the file is reported as not canonical (info) and rewritten in the typed member order (the
/// revision precedent, FT-165).
#[test]
fn layout_records_written_while_reserved_are_rewritten_canonically() {
    let canonical = files_of(&records());
    let path = layout_file(SPACE_A);
    let text = String::from_utf8(canonical[&path].clone()).unwrap();
    let old: String = text
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            format!("{}\n", serde_json::to_string(&v).unwrap())
        })
        .collect();
    assert_ne!(old, text, "the byte-wise order differs from the typed one");
    let mut files = Files::new();
    files.insert(path.clone(), old.into_bytes());
    let loaded = load(None, &files, &Limits::default());
    assert_eq!(
        loaded.findings.iter().map(|f| f.kind).collect::<Vec<_>>(),
        [FindingKind::NonCanonical],
        "{:?}",
        loaded.findings
    );
    assert!(loaded.kept.is_empty(), "{:?}", loaded.kept);
    assert!(loaded.rewrite.contains(&path));
    let rewritten: String = {
        let mut lines: Vec<String> = loaded.records.iter().map(|r| record_line(r).unwrap()).collect();
        lines.sort();
        lines.join("\n")
    };
    let mut expected: Vec<&str> = text.lines().collect();
    expected.sort_unstable();
    assert_eq!(rewritten, expected.join("\n"));
}

/// A placement in another space's file is EW-CORE-010, and both files are scheduled for a rewrite.
#[test]
fn a_placement_in_another_spaces_file_is_ew_core_010() {
    let mut files = files_of(&records());
    let line = placement(PL_RAIL, DEV_1, &format!(r#"{{"carrier":"{RAIL_A}"}}"#));
    let a = String::from_utf8(files[&layout_file(SPACE_A)].clone()).unwrap();
    let without: String = a
        .lines()
        .filter(|l| *l != line)
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(without, a);
    files.insert(layout_file(SPACE_A), without.into_bytes());
    let b = String::from_utf8(files[&layout_file(SPACE_B)].clone()).unwrap();
    files.insert(layout_file(SPACE_B), format!("{b}{line}\n").into_bytes());
    let loaded = load(None, &files, &Limits::default());
    let found: Vec<_> = loaded
        .findings
        .iter()
        .filter(|f| f.kind == FindingKind::WrongFile)
        .collect();
    assert_eq!(found.len(), 1, "{:?}", loaded.findings);
    assert_eq!(found[0].code(), Some("EW-CORE-010"));
    assert_eq!(found[0].file, Some(layout_file(SPACE_B)));
    assert!(loaded.rewrite.contains(&layout_file(SPACE_A)) && loaded.rewrite.contains(&layout_file(SPACE_B)));
}
