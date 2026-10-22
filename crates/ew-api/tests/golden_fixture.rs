// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The checked-in project fixture `golden/J-E-2416.ew/` (doc 20 §3.2 path (c)): it is the golden CSV
//! bootstrap (path (a)) written by the format crate, and these tests prove that regenerating it from the
//! tables is byte-identical to the committed directory (the manifest's `generator` member excepted,
//! doc 05 §17.1) and that the store loads it cleanly.
//!
//! Bless the one comparing test only, so that no other test of this file reads the fixture while bless
//! writes it (nextest runs tests in parallel):
//! `EW_BLESS_GOLDEN_FIXTURE=1 cargo nextest run -p ew-api --test golden_fixture --locked -E 'test(the_fixture_is_byte_identical_to_the_bootstrap)'`.

// The tests count rows through the untracked readers (dev-dependency `ew-model/raw-read`; tests are exempt
// from `cargo xtask check-raw-read`). The same allow covers the bless flag and the clock-named scratch directory
// (neither reaches a compared byte).
#![allow(clippy::disallowed_methods)]

mod golden_common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ew_schema::{
    Connection, Device, ExternalRef, Function, LibDocument, Page, Record, Representation, Section,
    StructureNode,
};
use ew_store::ew_format::{FilePath, Limits, MANIFEST, Manifest, project_files};
use ew_store::{Access, Changes, OpenMode, Store, StoreOptions};

const GENERATOR: &str = concat!("ElektraWatt ", env!("CARGO_PKG_VERSION"));

fn fixture_dir() -> PathBuf {
    golden_common::repo().join("golden/J-E-2416.ew")
}

fn generate() -> BTreeMap<String, Vec<u8>> {
    let batch = golden_common::batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let project = project.expect("the batch holds the project");
    let files = project_files(&project, &records, &batch.lib, 1, GENERATOR).expect("the batch renders");
    files
        .into_iter()
        .map(|(p, b)| (p.as_str().to_owned(), b))
        .collect()
}

fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = e.unwrap().path();
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read(&path).unwrap());
        }
    }
}

fn committed() -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    walk(&fixture_dir(), &fixture_dir(), &mut out);
    out
}

fn manifest_without_generator(bytes: &[u8]) -> Manifest {
    let mut m = Manifest::parse(bytes, &Limits::default()).expect("the manifest parses");
    m.generator = String::new();
    m
}

fn unique_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("ew-golden-fixture-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).unwrap();
        }
    }
}

#[test]
fn the_fixture_is_byte_identical_to_the_bootstrap() {
    let generated = generate();
    if std::env::var("EW_BLESS_GOLDEN_FIXTURE").is_ok_and(|v| v == "1") {
        let dir = fixture_dir();
        // Bless mode only writes; it never deletes. A file the tables no longer produce is listed for the
        // author to remove by hand (and the comparison below fails until it is gone).
        if dir.exists() {
            let stale: Vec<String> = committed()
                .keys()
                .filter(|k| !generated.contains_key(*k))
                .cloned()
                .collect();
            assert!(
                stale.is_empty(),
                "remove these files from golden/J-E-2416.ew by hand, then bless again: {stale:#?}"
            );
        }
        for (rel, bytes) in &generated {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
        }
        return;
    }
    let on_disk = committed();
    let mut differing = Vec::new();
    for rel in generated.keys().chain(on_disk.keys()).collect::<BTreeSet<_>>() {
        match (generated.get(rel), on_disk.get(rel)) {
            (Some(g), Some(d)) if rel == MANIFEST => {
                if manifest_without_generator(g) != manifest_without_generator(d) {
                    differing.push(format!("{rel} (differs)"));
                }
            }
            (Some(g), Some(d)) => {
                if g != d {
                    differing.push(format!("{rel} (differs)"));
                }
            }
            (Some(_), None) => differing.push(format!("{rel} (missing from the committed fixture)")),
            (None, Some(_)) => differing.push(format!("{rel} (not generated)")),
            (None, None) => unreachable!(),
        }
    }
    assert!(
        differing.is_empty(),
        "golden/J-E-2416.ew is out of date with golden/J-E-2416/tables; run \
         `EW_BLESS_GOLDEN_FIXTURE=1 cargo nextest run -p ew-api --test golden_fixture --locked \
         -E 'test(the_fixture_is_byte_identical_to_the_bootstrap)'` (this test only: the others read the \
         fixture in parallel) and commit the result. Differing paths:\n{}",
        differing.join("\n")
    );
}

#[test]
fn the_fixture_opens_without_findings() {
    let batch = golden_common::batch();
    let functions = batch
        .records
        .iter()
        .filter(|r| matches!(r, Record::Function(_)))
        .count();
    let representations = batch
        .records
        .iter()
        .filter(|r| matches!(r, Record::Representation(_)))
        .count();
    let parts = batch
        .lib
        .iter()
        .filter(|d| matches!(d, LibDocument::Part(_)))
        .count();

    let copy = unique_dir("copy");
    let state = unique_dir("state");
    copy_dir(&fixture_dir(), &copy);

    let mut opts = StoreOptions::new("test");
    opts.state_dir = Some(state.clone());
    let opened = Store::open(&copy, opts).expect("the fixture opens");
    let (mut store, model, report) = (opened.store, opened.model, opened.report);
    assert!(report.findings.is_empty(), "findings: {:#?}", report.findings);
    assert_eq!(report.access, Access::Edit);
    assert_eq!(report.generation, 1);

    assert_eq!(model.table::<Page>().len(), 13);
    assert_eq!(model.table::<Section>().len(), 3);
    assert_eq!(model.table::<StructureNode>().len(), 4);
    assert_eq!(model.table::<ExternalRef>().len(), 4);
    assert_eq!(model.table::<Device>().len(), 68 + 28);
    assert_eq!(model.table::<Connection>().len(), 72 + 19 + 2);
    assert_eq!(model.table::<Function>().len(), functions);
    assert_eq!(model.table::<Representation>().len(), representations);
    assert_eq!(model.lib().parts().len(), parts);

    let none = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("an empty save");
    assert!(none.written.is_empty() && none.removed.is_empty() && !none.manifest_written);
    let all = store.save(&model, Changes::All).expect("a full save");
    assert!(
        all.written.is_empty() && all.removed.is_empty() && !all.manifest_written,
        "a full save of an unchanged project wrote {:?}, removed {:?}",
        all.written,
        all.removed
    );
    store.close();
    assert!(!copy.join(".~ew.lock").exists(), "the lock is released");

    let _ = std::fs::remove_dir_all(&copy);
    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn the_committed_fixture_opens_read_only_without_a_lock() {
    let mut opts = StoreOptions::new("test");
    opts.mode = OpenMode::ReadOnly;
    opts.state_dir = None;
    let opened = Store::open(&fixture_dir(), opts).expect("the fixture opens read-only");
    assert!(
        opened.report.findings.is_empty(),
        "findings: {:#?}",
        opened.report.findings
    );
    assert!(
        matches!(opened.report.access, Access::ReadOnly(_)),
        "{:?}",
        opened.report.access
    );
    opened.store.close();
    assert!(!fixture_dir().join(".~ew.lock").exists());
}

#[test]
fn the_fixture_paths_are_portable() {
    const FIXED: [&str; 12] = [
        "project.jsonl",
        "structure.jsonl",
        "report-blocks.jsonl",
        "revisions.jsonl",
        "manifest.json",
        "pages",
        "devices",
        "lib",
        "parts",
        "symbols",
        "fndefs",
        "forms",
    ];
    let paths = committed();
    assert!(!paths.is_empty());
    for rel in paths.keys() {
        assert!(rel.len() <= 120, "{rel} is longer than 120 characters");
        assert!(
            rel.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "./-".contains(c)),
            "{rel} has a character outside [a-z0-9./-]"
        );
        assert!(
            FilePath::new(rel.clone()).is_ok_and(|p| p.is_portable()),
            "{rel} is not portable"
        );
        for seg in rel.split('/') {
            if FIXED.contains(&seg) {
                continue;
            }
            let stem = seg
                .strip_suffix(".jsonl")
                .or_else(|| seg.strip_suffix(".json"))
                .unwrap_or(seg);
            assert!(
                stem.chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c) || c == '-'),
                "{rel}: segment {seg} is not a UUID or shard name"
            );
        }
    }
}
