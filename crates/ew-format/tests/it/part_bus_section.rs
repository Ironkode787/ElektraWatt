// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The part-level bus section through the files (WP-S1.21 §8a.1, D15; BU-7d): a part copy in `lib/` whose
//! `sections.bus` carries typed members, unknown members and newer enum values loads and saves byte-equal, and its
//! stored `sha256` (the hash of the item's compact JSON) verifies; a copy that an older 0.1 build wrote, with `bus`
//! in `PartSections.extra`, reads typed and is rewritten canonically.

use std::collections::BTreeMap;

use ew_format::ew_schema::{BusSystem, LibDocument, Project, Record, Sha256};
use ew_format::{
    EntryKind, FilePath, FindingKind, Limits, Loaded, Manifest, Severity, hash_text, lib_document_text,
    lib_file, library_copy_hash, load, plan, project_files, sha256,
};

const PROJECT: &str =
    r#"{"$k":"project","id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;
const LIB: &str = "6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47";
const COPY: &str = "3d7e1a95-2c48-5b06-9f1d-8a4c0e6b2f73";
const ITEM: &str = "c19f4b62-7e03-5a8d-b4c5-1e9a0d3f6b28";

/// The part item's compact JSON: a CPU with a PLC section and BU-7a's bus section.
fn item() -> String {
    concat!(
        r#"{"manufacturer":{"library":"6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47","item":"0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13","version_req":"^1"},"#,
        r#""part_number":"CPU-DP","sections":{"plc":{"role":"station"},"bus":{"ports":["#,
        r#"{"interface":"DP","bus":"profibus_dp","role":"master","termination":"off","address_range":[1,125],"connector":"sub_d9","signals":{"3":"B"}},"#,
        r#"{"interface":"X9","bus":"sercos","role":"observer","termination":"auto"}"#,
        r#"],"rates":[9600,12000000]}}}"#
    )
    .to_owned()
}

/// The copy's compact JSON with the hash of `item`.
fn compact(item: &str) -> String {
    let hash = hash_text(&sha256(item.as_bytes()));
    format!(
        r#"{{"$k":"part","id":"{COPY}","library_id":"{LIB}","item_id":"{ITEM}","version":"1.0.0","sha256":"{hash}",{}"#,
        &item[1..]
    )
}

fn project() -> Project {
    match Record::from_json(PROJECT).unwrap() {
        Record::Project(project) => project,
        other => panic!("not a project: {other:?}"),
    }
}

fn write_and_read(lib: &[LibDocument]) -> (BTreeMap<FilePath, Vec<u8>>, Loaded) {
    let files = project_files(&project(), &[], lib, 1, "test").unwrap();
    let manifest = Manifest::parse(
        &files[&FilePath::new("manifest.json").unwrap()],
        &Limits::default(),
    )
    .unwrap();
    let present: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect();
    let reads = plan(Some(&manifest), &present, &Limits::default());
    assert!(reads.findings.is_empty(), "{:?}", reads.findings);
    let input: BTreeMap<FilePath, Vec<u8>> = reads
        .read
        .iter()
        .map(|path| (path.clone(), files[path].clone()))
        .collect();
    let loaded = load(Some(&manifest), &input, &Limits::default());
    (files, loaded)
}

/// BU-7d: the copy is canonical, its hash verifies, and it is written and read back unchanged.
#[test]
fn a_part_copy_with_a_bus_section_round_trips_and_its_hash_verifies() {
    let item = item();
    let doc = LibDocument::from_json(&compact(&item)).unwrap();
    assert_eq!(
        serde_json::to_string(&doc).unwrap(),
        compact(&item),
        "not canonical"
    );
    assert_eq!(library_copy_hash(&doc), Some(sha256(item.as_bytes())));
    let LibDocument::Part(copy) = &doc else {
        panic!("a part");
    };
    let bus = copy.item.sections.bus.as_ref().expect("typed");
    assert_eq!(bus.ports[0].bus, BusSystem::ProfibusDp);
    assert!(matches!(bus.ports[1].bus, BusSystem::Unknown(_)));

    let (files, loaded) = write_and_read(std::slice::from_ref(&doc));
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
    assert_eq!(loaded.lib, std::slice::from_ref(&doc));
    let path = lib_file(&doc).unwrap();
    assert_eq!(files[&path], lib_document_text(&doc).unwrap().into_bytes());
    // A second save of the loaded copy writes the same bytes.
    let (again, _) = write_and_read(&loaded.lib);
    assert_eq!(again[&path], files[&path]);
}

/// §8a.1 compatibility, as built: an older 0.1 build keeps `bus` in `PartSections.extra` at the same place (after
/// the typed sections, in key order), but writes the port members in byte-wise order (`bus` before `interface`) and
/// takes its hash over that text. The file reads typed, is reported not canonical (info) and by EW-CORE-014
/// (warning, accepted within 0.1 as for the part-1 members, decision log ID-334), and the next save rewrites it in
/// §8a.1's order.
#[test]
fn a_copy_an_older_build_wrote_reads_typed_and_is_rewritten() {
    let item = item();
    let doc = LibDocument::from_json(&compact(&item)).unwrap();
    let mut old = doc.clone();
    let LibDocument::Part(copy) = &mut old else {
        panic!("a part");
    };
    let bus = copy.item.sections.bus.take().unwrap();
    // Without `preserve_order`, a `Value` holds its members byte-wise, as the older build's `extra` did.
    let byte_wise: serde_json::Value = serde_json::from_str(&serde_json::to_string(&bus).unwrap()).unwrap();
    copy.item.sections.extra.insert("bus".into(), byte_wise);
    copy.sha256 = Sha256(sha256(serde_json::to_string(&copy.item).unwrap().as_bytes()));
    let old_text = lib_document_text(&old).unwrap();
    assert!(
        old_text.find(r#""bus": "profibus_dp""#) < old_text.find(r#""interface": "DP""#),
        "byte-wise members: {old_text}"
    );
    assert!(
        old_text.find(r#""plc""#) < old_text.find(r#""bus": {"#),
        "the section after the typed ones: {old_text}"
    );
    let path = lib_file(&doc).unwrap();
    let mut files = BTreeMap::new();
    files.insert(path.clone(), old_text.clone().into_bytes());
    let loaded = load(None, &files, &Limits::default());
    let found: Vec<(FindingKind, Severity)> =
        loaded.findings.iter().map(|f| (f.kind, f.severity())).collect();
    assert_eq!(
        found,
        [
            (FindingKind::LibraryHash, Severity::Warning),
            (FindingKind::NonCanonical, Severity::Info)
        ]
    );
    assert!(loaded.rewrite.contains(&path));
    let [LibDocument::Part(read)] = loaded.lib.as_slice() else {
        panic!("one part: {:?}", loaded.lib);
    };
    let LibDocument::Part(expected) = &doc else {
        panic!("a part");
    };
    assert_eq!(read.item, expected.item, "read typed, nothing left in extra");
    assert_ne!(lib_document_text(&loaded.lib[0]).unwrap(), old_text);
}
