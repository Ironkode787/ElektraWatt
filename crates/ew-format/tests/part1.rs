// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The typed part, manufacturer and macro data of WP-S1.19 part 1 through the files (ADR-0011 amendment 4,
//! decision log ID-334): the fixture copies of L1-2 are written and read back unchanged, each with the hash of its
//! compact JSON; a `macro_instance` line lives in its page's file at rank 46; a file written before part 1, with
//! the members in `extra`, reads typed and is rewritten canonically (L1-3).

use std::collections::BTreeMap;

use ew_format::ew_schema::{LibDocument, Project, Record, Sha256};
use ew_format::{
    EntryKind, FilePath, FindingKind, Limits, Loaded, Manifest, Severity, hash_text, lib_document_text,
    lib_file, library_copy_hash, load, plan, project_files, record_file, record_line, sha256,
};

const PROJECT: &str =
    r#"{"$k":"project","id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;
const PAGE_ID: &str = "8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a";
const PAGE: &str = r#"{"$k":"page","id":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","folio":{"section":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","number":101},"kind":"multi_line","structure":{"function":"7d2c95e0-1f4a-5b38-a6e7-0c93b5d1f284","location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"},"title":{"en-GB":"Power Supply 1 - Main Current"},"grid":{"num":7937500,"den":3}}"#;
const STRUCTURE_BOX: &str = r#"{"$k":"structure_box","id":"5a0c1e2d-3b4f-5a61-8000-000000000005","page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","outline":[[20000000,40000000],[80000000,40000000],[80000000,64000000],[20000000,64000000]],"style":"dash_dot","caption":"External Connection","assigns":{"location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"}}"#;
const REPORT_BLOCK: &str = r#"{"$k":"report_block","id":"5a0c1e2d-3b4f-5a61-8000-000000000006","source":{"kind":"part_data"},"form":"2b8d4f60-1e3a-5c97-8d05-f4a6c2e9b713","first_folio":201,"embedded":{"page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a","rect":[[10000000,180000000],[120000000,270000000]]}}"#;
/// A macro instance placed on the page with value set 1: two members, local → created.
const MACRO_INSTANCE: &str = r#"{"$k":"macro_instance","id":"4b7d2e91-0c58-5a36-8f1e-d9a3c6b07e24","macro":"a6e3f0b8-5d12-5c79-9b4a-1e8d7c2f0a63","variant":"A","value_set":"1","values":{"counter":1},"members":[["00000000-0000-5000-8000-000000000001","c7a90e35-42d8-5f61-8b03-9d2e6f1a47c2"],["00000000-0000-5000-8000-000000000002","1d6f3a92-7c05-5b8e-94a1-e3c2087f5d19"]],"page":"8e41c0d2-1b7f-5a39-a6c4-0f5e2d913b7a"}"#;

const LIB: &str = "6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47";

type Files = BTreeMap<FilePath, Vec<u8>>;

fn p(s: &str) -> FilePath {
    FilePath::new(s).unwrap()
}

fn record(line: &str) -> Record {
    Record::from_json(line).unwrap()
}

fn project() -> Project {
    match record(PROJECT) {
        Record::Project(project) => project,
        other => panic!("not a project: {other:?}"),
    }
}

fn lib_ref(item: &str) -> String {
    format!(r#"{{"library":"{LIB}","item":"{item}","version_req":"^1"}}"#)
}

/// One L1-2 fixture copy: its kind, copy ID, item ID and the compact JSON of its item, members in declaration
/// order (doc 07 §2.5).
struct Fixture {
    kind: &'static str,
    copy: &'static str,
    item_id: &'static str,
    item: String,
}

impl Fixture {
    /// The copy's compact JSON: the copy envelope, the `sha256` of the item text, then the item's members.
    fn compact(&self) -> String {
        let hash = hash_text(&sha256(self.item.as_bytes()));
        format!(
            r#"{{"$k":"{}","id":"{}","library_id":"{LIB}","item_id":"{}","version":"1.0.0","sha256":"{hash}",{}"#,
            self.kind,
            self.copy,
            self.item_id,
            &self.item[1..]
        )
    }

    fn doc(&self) -> LibDocument {
        LibDocument::from_json(&self.compact()).unwrap()
    }
}

/// The fixtures of L1-2: 2904602 with `mech` (W×H×D, `ts35x7_5`, rail offset), an alternative and a macro link;
/// PT 2.5-TWIN with the clamp members; a contactor with two contacts; an enclosure, a duct, a rail and a mounting
/// plate section; a configured product with components; a manufacturer and a macro.
fn fixtures() -> Vec<Fixture> {
    let mfr = lib_ref("0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13");
    let f = |kind, copy, item_id, item: String| Fixture {
        kind,
        copy,
        item_id,
        item,
    };
    vec![
        f(
            "part",
            "e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025",
            "8b42c7d9-1e06-5a3f-b8d5-6c0e2f9a4b71",
            format!(
                r#"{{"meta":{{"key":"pxc.2904602","licence":"CC0-1.0"}},"manufacturer":{mfr},"part_number":"2904602","description":{{"en-GB":"Power supply 1 AC / 24 V DC, 20 A"}},"cp_pattern":[{{"idx":0,"designation":"L"}},{{"idx":1,"designation":"N"}}],"mech":{{"width":60000000,"height":130000000,"depth":125000000,"mount_kind":"ts35x7_5","rail_offset":[30000000,65000000],"clearance":{{"top":50000000,"bottom":50000000}},"front_2d":[[[0,0],[60000000,0],[60000000,130000000],[0,130000000]]],"mass":1100}},"alternatives":[{{"part":{},"kind":"successor"}}],"macros":[{{"rep_type":"multi_line","macro":{}}}]}}"#,
                lib_ref("3c1e8a52-7d94-5b06-a2f8-9e0b4c6d1f37"),
                lib_ref("d2c8a5e1-7b40-5f93-8e6d-0a3b9f1c4e57")
            ),
        ),
        f(
            "part",
            "a07c4e19-d25b-5f83-9c16-4e8b0d7a2f35",
            "61b3e9d0-4a27-5c8f-9e15-b2d7a0c4f683",
            format!(
                r#"{{"kind":"terminal","manufacturer":{mfr},"part_number":"3209549","type_designation":"PT 2.5-TWIN","cp_pattern":[{{"idx":0,"designation":"1","clamp_type":"push_in","csa_min":"0.14","csa_max":"4","max_conductors":2,"twin_ferrule":true}},{{"idx":1,"designation":"1","clamp_type":"push_in","csa_min":"0.14","csa_max":"4"}}]}}"#
            ),
        ),
        f(
            "part",
            "2f9d4b61-8e03-5a7c-b5d1-0c6e3a9f2b48",
            "74e0c2a9-1b6d-5f38-8a4e-d9c3f5b7021e",
            format!(
                r#"{{"manufacturer":{mfr},"part_number":"K-2C-24VDC","cp_pattern":[{{"idx":0,"designation":"A1"}},{{"idx":1,"designation":"A2"}},{{"idx":2,"designation":"13"}},{{"idx":3,"designation":"14"}},{{"idx":4,"designation":"21"}},{{"idx":5,"designation":"22"}}],"contacts":[{{"kind":"no","cps":["13","14"],"seq":1}},{{"kind":"nc","cps":["21","22"],"seq":2}}]}}"#
            ),
        ),
        f(
            "part",
            "9a4c7e02-3d18-5b6f-a0e9-5f2b8d1c7e64",
            "c3f81d5a-6e29-5047-b8c2-1a9e4d7f0b35",
            format!(
                r#"{{"kind":"enclosure","manufacturer":{mfr},"part_number":"ENCL-600x600x250","sections":{{"enclosure":{{"width":600000000,"height":600000000,"depth":250000000,"wall":1500000,"plate":{}}}}}}}"#,
                lib_ref("e8b2a4f6-0c73-5d19-9f5e-3b7c1d0a6e82")
            ),
        ),
        f(
            "part",
            "5e1b9d73-a246-5c08-8f3d-7c0a2e6b4f91",
            "0d6a3f8c-2b95-5e71-a4c0-8f1e7b3d5a26",
            format!(
                r#"{{"kind":"duct","manufacturer":{mfr},"part_number":"DUCT-60x80","unit":"m","sections":{{"duct":{{"inner_width":56000000,"inner_height":76000000,"slot_pitch":12500000}}}}}}"#
            ),
        ),
        f(
            "part",
            "b7d03e5a-91c4-5f26-8e0b-4a6c2f9d1e73",
            "f4a9c1e7-5d30-5b82-9c6f-0e2d8b4a7c15",
            format!(
                r#"{{"kind":"rail","manufacturer":{mfr},"part_number":"NS 35/7,5 PERF 2000MM","sections":{{"rail":{{"profile":"ts35x7_5","stock_length":2000000000}}}}}}"#
            ),
        ),
        f(
            "part",
            "1c8e5f2b-7a09-5d43-b6e1-9d3f0c7a2b58",
            "e8b2a4f6-0c73-5d19-9f5e-3b7c1d0a6e82",
            format!(
                r#"{{"kind":"mounting_plate","manufacturer":{mfr},"part_number":"PLATE-535x575","sections":{{"mounting_plate":{{"width":535000000,"height":575000000,"thickness":3000000}}}}}}"#
            ),
        ),
        f(
            "part",
            "6f2a8c4e-0d57-5b91-a3e8-c1b7d5f9e026",
            "a85d2c09-3f7e-5146-9b2a-6e0d4c8f1b73",
            format!(
                r#"{{"kind":"configured","manufacturer":{mfr},"part_number":"CFG-K2C-X2","configured":{{"order_code":"K-2C-24VDC+2xPT","components":[{{"part":{},"qty":"1"}},{{"part":{},"qty":"2"}}]}}}}"#,
                lib_ref("74e0c2a9-1b6d-5f38-8a4e-d9c3f5b7021e"),
                lib_ref("61b3e9d0-4a27-5c8f-9e15-b2d7a0c4f683")
            ),
        ),
        f(
            "manufacturer",
            "1e9b4c07-3a62-5d8f-b0e5-7c2d8a4f9b16",
            "0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13",
            r#"{"meta":{"key":"ew.mfr.eaton","licence":"CC0-1.0"},"key":"EATON","name":"Eaton","aliases":["Eaton / Holec"],"brands":["Holec"]}"#
                .to_owned(),
        ),
        f(
            "macro",
            "a6e3f0b8-5d12-5c79-9b4a-1e8d7c2f0a63",
            "d2c8a5e1-7b40-5f93-8e6d-0a3b9f1c4e57",
            r#"{"meta":{"key":"ew.macro.psu-block","licence":"CC0-1.0"},"name":{"en-GB":"PSU block"},"kind":"window","variants":[{"rep_type":"multi_line","key":"A","fragment":{"format":"ew-fragment v1","records":[]}}],"variables":[{"name":"counter","ty":"tag_counter","bindings":[{"record":"00000000-0000-5000-8000-000000000001","field":"device.tag.counter"}],"default":1}],"groups":[{"name":"fuses","members":["00000000-0000-5000-8000-000000000002"],"include":"with_fuses"}],"value_sets":[{"name":"1","values":{"counter":1}}],"thumbnail":"sha256:0000000000000000000000000000000000000000000000000000000000000000"}"#
                .to_owned(),
        ),
    ]
}

/// The files of `records` and `lib`, read back through the plan and the loader with their manifest.
fn write_and_read(records: &[Record], lib: &[LibDocument]) -> (Files, Loaded) {
    let files = project_files(&project(), records, lib, 1, "test").unwrap();
    let manifest = Manifest::parse(&files[&p("manifest.json")], &Limits::default()).unwrap();
    let present: BTreeMap<String, EntryKind> = files
        .keys()
        .map(|k| (k.as_str().to_owned(), EntryKind::File))
        .collect();
    let reads = plan(Some(&manifest), &present, &Limits::default());
    assert!(reads.findings.is_empty(), "{:?}", reads.findings);
    let input: Files = reads
        .read
        .iter()
        .map(|path| (path.clone(), files[path].clone()))
        .collect();
    let loaded = load(Some(&manifest), &input, &Limits::default());
    (files, loaded)
}

/// L1-2: every fixture copy is canonical, its `sha256` is the hash of its item's compact JSON, and it is written
/// to `lib/<kind dir>/<copy id>.json` and read back unchanged, with no finding and nothing to rewrite.
#[test]
fn every_fixture_copy_round_trips_with_the_hash_of_its_compact_json() {
    let fixtures = fixtures();
    let docs: Vec<LibDocument> = fixtures.iter().map(Fixture::doc).collect();
    for (fixture, doc) in fixtures.iter().zip(&docs) {
        assert_eq!(
            serde_json::to_string(doc).unwrap(),
            fixture.compact(),
            "not canonical"
        );
        assert_eq!(
            library_copy_hash(doc),
            Some(sha256(fixture.item.as_bytes())),
            "{} {}",
            fixture.kind,
            fixture.copy
        );
        let dir = match fixture.kind {
            "part" => "parts",
            "manufacturer" => "manufacturers",
            _ => "macros",
        };
        assert_eq!(
            lib_file(doc).unwrap().as_str(),
            format!("lib/{dir}/{}.json", fixture.copy)
        );
    }
    let (files, loaded) = write_and_read(&[record(PAGE)], &docs);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
    assert!(loaded.opaque.is_empty() && loaded.kept_docs.is_empty());
    assert_eq!(loaded.lib.len(), docs.len());
    for doc in &docs {
        assert!(loaded.lib.contains(doc), "{:?} not loaded", doc.kind());
        let path = lib_file(doc).unwrap();
        assert_eq!(files[&path], lib_document_text(doc).unwrap().into_bytes());
    }
}

/// L1-2: an unknown member in each new object of the fixtures (written last) survives a read and a write; the copy
/// hash covers it, as the hash of the item's compact JSON.
#[test]
fn unknown_members_of_the_new_objects_survive_the_files() {
    let with_unknown: Vec<Fixture> = fixtures()
        .into_iter()
        .map(|mut f| {
            for object in [
                r#""mech":{"width":60000000"#,
                r#""clearance":{"top":50000000"#,
                r#""contacts":[{"kind":"no""#,
                r#""enclosure":{"width":600000000"#,
                r#""duct":{"inner_width":56000000"#,
                r#""rail":{"profile":"ts35x7_5""#,
                r#""configured":{"order_code":"K-2C-24VDC+2xPT""#,
                r#"{"part":{"library""#,
                r#""macros":[{"rep_type":"multi_line""#,
                r#""variants":[{"rep_type":"multi_line""#,
                r#""bindings":[{"record""#,
            ] {
                if let Some(at) = f.item.find(object) {
                    // Insert the member before the object's closing brace: find the brace that closes it.
                    let start = at + f.item[at..].find('{').unwrap();
                    let mut depth = 0usize;
                    let mut close = start;
                    for (i, c) in f.item[start..].char_indices() {
                        match c {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    close = start + i;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    f.item.insert_str(close, r#","zz_newer":{"a":2}"#);
                }
            }
            if !f.item.contains("zz_newer") {
                // The manufacturer and part items without a nested new object: the item itself.
                let end = f.item.len() - 1;
                f.item.insert_str(end, r#","zz_newer":true"#);
            }
            f
        })
        .collect();
    let docs: Vec<LibDocument> = with_unknown.iter().map(Fixture::doc).collect();
    assert_eq!(
        with_unknown
            .iter()
            .map(|f| f.item.matches("zz_newer").count())
            .sum::<usize>(),
        15,
        "every new object of the fixtures got the member"
    );
    for (fixture, doc) in with_unknown.iter().zip(&docs) {
        assert_eq!(serde_json::to_string(doc).unwrap(), fixture.compact());
        assert_eq!(library_copy_hash(doc), Some(sha256(fixture.item.as_bytes())));
    }
    let (_, loaded) = write_and_read(&[record(PAGE)], &docs);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
    for doc in &docs {
        assert!(loaded.lib.contains(doc));
    }
}

/// L1-4, the file half: a `macro_instance` line lives in its page's file and is written at rank 46, after the
/// structure box (44) and the embedded report block (45), and reads back unchanged.
#[test]
fn a_macro_instance_line_round_trips_in_its_page_file_at_rank_46() {
    let records = [
        record(MACRO_INSTANCE),
        record(REPORT_BLOCK),
        record(STRUCTURE_BOX),
        record(PAGE),
    ];
    let page_file = format!("pages/{PAGE_ID}.jsonl");
    let anchors = ew_format::RecordAnchors::new(&records);
    assert_eq!(record_file(&records[0], &anchors).unwrap().as_str(), page_file);
    let (files, loaded) = write_and_read(&records, &[]);
    let text = String::from_utf8(files[&p(&page_file)].clone()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines, [PAGE, STRUCTURE_BOX, REPORT_BLOCK, MACRO_INSTANCE]);
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty());
    assert!(loaded.records.contains(&record(MACRO_INSTANCE)));
    assert_eq!(record_line(&record(MACRO_INSTANCE)).unwrap(), MACRO_INSTANCE);
}

/// The 2904602 fixture as a build before part 1 wrote it: `mech`, `alternatives` and `macros` were unknown members,
/// kept in the part's `extra` and written after the typed members in byte-wise order, the members of each in
/// byte-wise order too; the copy hash was taken over that text.
fn written_before_part_one() -> (LibDocument, String) {
    let fixture = &fixtures()[0];
    let mut doc = fixture.doc();
    let LibDocument::Part(copy) = &mut doc else {
        panic!("a part");
    };
    let byte_wise = |v: serde_json::Value| serde_json::from_str::<serde_json::Value>(&v.to_string()).unwrap();
    let mech = byte_wise(serde_json::to_value(copy.item.mech.take().unwrap()).unwrap());
    let alternatives = byte_wise(serde_json::to_value(std::mem::take(&mut copy.item.alternatives)).unwrap());
    let macros = byte_wise(serde_json::to_value(std::mem::take(&mut copy.item.macros)).unwrap());
    copy.item.extra.insert("alternatives".into(), alternatives);
    copy.item.extra.insert("macros".into(), macros);
    copy.item.extra.insert("mech".into(), mech);
    copy.sha256 = Sha256(sha256(serde_json::to_string(&copy.item).unwrap().as_bytes()));
    let text = lib_document_text(&doc).unwrap();
    (doc, text)
}

/// L1-3: the old file opens with the members typed, is reported not canonical (info) and rewritten at the next
/// save; its stored `sha256`, taken over the byte-wise order, is reported by EW-CORE-014 (accepted within 0.1,
/// decision log ID-334).
#[test]
fn a_part_written_before_part_one_reads_typed_and_is_rewritten_canonically() {
    let (old_doc, old_text) = written_before_part_one();
    let fixture = &fixtures()[0];
    let path = lib_file(&old_doc).unwrap();
    assert!(
        old_text.contains(r#""clearance": {"bottom": 50000000, "top": 50000000}"#),
        "{old_text}"
    );
    let mut files = Files::new();
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
    assert_eq!(loaded.findings[0].code(), Some("EW-CORE-014"));
    assert!(loaded.rewrite.contains(&path));
    assert!(!loaded.read_only);
    let [LibDocument::Part(copy)] = loaded.lib.as_slice() else {
        panic!("one part: {:?}", loaded.lib);
    };
    assert!(copy.item.extra.is_empty(), "{:?}", copy.item.extra);
    assert!(copy.item.mech.is_some());
    assert_eq!(copy.item.alternatives.len(), 1);
    assert_eq!(copy.item.macros.len(), 1);
    // The typed item is the fixture's; only the stored hash is the old one.
    let LibDocument::Part(expected) = fixture.doc() else {
        panic!("a part");
    };
    assert_eq!(copy.item, expected.item);
    let LibDocument::Part(old) = &old_doc else {
        panic!("a part");
    };
    assert_eq!(copy.sha256, old.sha256);
    // The rewrite writes the canonical text of the typed document.
    let rewritten = lib_document_text(&loaded.lib[0]).unwrap();
    assert_ne!(rewritten, old_text);
    let again = LibDocument::from_json(&rewritten).unwrap();
    assert_eq!(again, loaded.lib[0]);
}

/// L1-3: when the byte-wise order of an old file already is the canonical one (a single new member after the last
/// typed one, itself with one member), the file reads clean and its hash still matches: nothing to rewrite.
#[test]
fn an_old_part_whose_order_agrees_reads_clean() {
    let item = r#"{"manufacturer":{"library":"6a1d0f3e-8b25-5c74-9e1a-d3f60b8c2e47","item":"0f7b2c94-5d3a-5e81-a6f9-2b8c0e4d7a13","version_req":"^1"},"part_number":"2904602","mech":{"mass":1100}}"#;
    let fixture = Fixture {
        kind: "part",
        copy: "e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025",
        item_id: "8b42c7d9-1e06-5a3f-b8d5-6c0e2f9a4b71",
        item: item.to_owned(),
    };
    let doc = fixture.doc();
    let path = lib_file(&doc).unwrap();
    let mut files = Files::new();
    files.insert(path, lib_document_text(&doc).unwrap().into_bytes());
    let loaded = load(None, &files, &Limits::default());
    assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
    assert!(loaded.rewrite.is_empty());
}
