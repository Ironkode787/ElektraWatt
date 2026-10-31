// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writer and reader agree (doc 05 §3.1, §3.3, NFR-REL-07): the files do not depend on the record order, and
//! reading what was written gives no findings, nothing to rewrite and the same records.

use std::collections::BTreeMap;

use ew_format::ew_schema::{Project, Record};
use ew_format::{EntryKind, FilePath, Limits, Manifest, load, plan, project_files};
use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;

const PROJECT: &str =
    r#"{"$k":"project","id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

/// A random UUID of version 7 and variant 10 in the canonical lower-case hyphenated form.
fn uuid(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn page(id: &str, number: u32) -> String {
    format!(
        r#"{{"$k":"page","id":"{id}","folio":{{"section":"0e8b3d61-a74c-5f12-b5e9-82d4c1f06a3e","number":{number}}},"kind":"multi_line","structure":{{"function":"7d2c95e0-1f4a-5b38-a6e7-0c93b5d1f284","location":"a41f6c2d-8e05-5d97-b3c1-5f2e8a7d9c60"}},"title":{{"en-GB":"Power Supply 1 - Main Current"}},"grid":{{"num":7937500,"den":3}}}}"#
    )
}

fn device(id: &str, counter: u32) -> String {
    format!(
        r#"{{"$k":"device","id":"{id}","tag":{{"letter":"G","counter":{counter}}},"kind":"psu","parts":[{{"part":{{"copy":"e5d2a8f1-0c37-5b94-8e6a-4f1d93c7b025"}},"qty":"1"}}],"function_text":{{"en-GB":"Power supply 1 AC / 24 V DC"}},"tech_data":[{{"key":"i_out","value":{{"value":"20","unit":"A"}}}}]}}"#
    )
}

fn function(id: &str, device: &str) -> String {
    format!(
        r#"{{"$k":"function","id":"{id}","device":"{device}","def":"5b9e2d70-3c41-5f8a-b6d2-19e4c7a03f58","kind":"black_box"}}"#
    )
}

/// The generated records: pages, devices and functions of devices among them, unique IDs.
fn records(
    pages: &[([u8; 16], u32)],
    devices: &[([u8; 16], u32)],
    functions: &[([u8; 16], usize)],
) -> Vec<Record> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (id, number) in pages {
        let id = uuid(*id);
        if seen.insert(("page", id.clone())) {
            out.push(Record::from_json(&page(&id, *number)).unwrap());
        }
    }
    let mut device_ids = Vec::new();
    for (id, counter) in devices {
        let id = uuid(*id);
        if seen.insert(("device", id.clone())) {
            out.push(Record::from_json(&device(&id, *counter)).unwrap());
            device_ids.push(id);
        }
    }
    if !device_ids.is_empty() {
        for (id, which) in functions {
            let id = uuid(*id);
            if seen.insert(("function", id.clone())) {
                let device = &device_ids[which % device_ids.len()];
                out.push(Record::from_json(&function(&id, device)).unwrap());
            }
        }
    }
    out
}

/// A deterministic Fisher-Yates shuffle.
fn shuffled<T>(mut items: Vec<T>, mut seed: u64) -> Vec<T> {
    for i in (1..items.len()).rev() {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let j = usize::try_from(seed >> 33).unwrap() % (i + 1);
        items.swap(i, j);
    }
    items
}

fn project() -> Project {
    match Record::from_json(PROJECT).unwrap() {
        Record::Project(project) => project,
        other => panic!("not a project: {other:?}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    #[test]
    fn written_files_read_back_unchanged(
        pages in proptest::collection::vec((any::<[u8; 16]>(), 0u32..1000), 0..7),
        devices in proptest::collection::vec((any::<[u8; 16]>(), 1u32..100), 0..7),
        functions in proptest::collection::vec((any::<[u8; 16]>(), any::<usize>()), 0..7),
        seed in any::<u64>(),
    ) {
        let records = records(&pages, &devices, &functions);
        prop_assert!(records.len() < 21);
        let project = project();
        let files = project_files(&project, &records, &[], 1, "test").unwrap();
        let again = project_files(&project, &shuffled(records.clone(), seed), &[], 1, "test").unwrap();
        prop_assert_eq!(&files, &again);

        let manifest = Manifest::parse(&files[&FilePath::new("manifest.json").unwrap()], &Limits::default())
            .unwrap();
        let present: BTreeMap<String, EntryKind> = files
            .keys()
            .map(|k| (k.as_str().to_owned(), EntryKind::File))
            .collect();
        let reads = plan(Some(&manifest), &present, &Limits::default());
        prop_assert!(reads.findings.is_empty(), "{:?}", reads.findings);
        let input: BTreeMap<FilePath, Vec<u8>> = reads
            .read
            .iter()
            .map(|path| (path.clone(), files[path].clone()))
            .collect();
        let loaded = load(Some(&manifest), &input, &Limits::default());
        prop_assert!(loaded.findings.is_empty(), "{:?}", loaded.findings);
        prop_assert!(loaded.rewrite.is_empty(), "{:?}", loaded.rewrite);
        prop_assert!(!loaded.read_only);
        prop_assert_eq!(&loaded.project, &Some(project));
        prop_assert_eq!(loaded.records.len(), records.len());
        for record in &records {
            prop_assert!(loaded.records.contains(record), "{record:?} not loaded");
        }
        prop_assert_eq!(loaded.hashes.len(), input.len());
    }
}
