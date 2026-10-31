// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew import csv`'s library half (WP-S1.22 FM-6, FT-27): the golden tables read from their directory, imported with
//! the starter content and written as a new project, which opens with the model and the check findings of the golden
//! CSV bootstrap (`tests/golden_csv.rs`), all or nothing at its target.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::import::{
    CreateError, ImportMode, StarterLibrary, TablesError, create_project, import_tables, read_tables,
    session_from_batch,
};
use ew_api::{Session, SessionOptions, StoreOptions};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tables() -> PathBuf {
    repo().join("golden/J-E-2416/tables")
}

fn library() -> StarterLibrary {
    StarterLibrary::from_dir(&repo().join("content")).expect("the starter content reads")
}

/// Every file of a directory tree with its bytes, by relative path with `/`.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn go(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                go(&p, base, out);
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    go(dir, dir, &mut out);
    out
}

fn store() -> StoreOptions {
    let mut o = StoreOptions::new(env!("CARGO_PKG_VERSION"));
    o.state_dir = None;
    o
}

/// FM-6: the tables import to a project that opens, holds the rows of the bootstrapped model (so the record counts of
/// `golden_csv.rs`) and checks with the same findings. Whether the written project equals `golden/J-E-2416.ew` byte
/// for byte is reported, not required (it is printed).
#[test]
fn the_golden_tables_import_to_a_project_with_the_bootstrapped_model() {
    let inputs = read_tables(&tables()).unwrap();
    assert!(inputs.iter().any(|i| i.name() == "devices.csv"));
    let batch = import_tables(&inputs, &library(), ImportMode::Strict).expect("the golden tables import");
    assert!(
        batch
            .report
            .entries
            .iter()
            .all(|e| e.severity == ew_api::import::Severity::Info),
        "{:#?}",
        batch.report.entries
    );
    let bootstrapped = session_from_batch(&batch, SessionOptions::default()).unwrap();

    let dir = ew_testdir::fresh_dir("ew-api-import-csv");
    let out = dir.join("new.ew");
    create_project(&batch, &out, store()).expect("the project is written");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["new.ew"], "no temporary directory is left");
    assert!(!out.join(".~ew.lock").exists(), "the lock is released");

    let (opened, report) = Session::open_reader(&out, env!("CARGO_PKG_VERSION"), "check").expect("it opens");
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert_eq!(report.generation, 1);
    let lines = |s: &Session| ew_commands::rows::model_lines(&s.world().model).unwrap();
    assert_eq!(
        lines(&opened),
        lines(&bootstrapped),
        "the rows of the bootstrapped model"
    );
    let check = |s: &Session| s.check(&PRE_DELIVERY, CheckMode::Release).findings;
    assert_eq!(check(&opened), check(&bootstrapped), "the same check findings");
    opened.close();

    // Reported, not required: the written files against the checked-in Golden A.
    let written = tree(&out);
    let golden = tree(&repo().join("golden/J-E-2416.ew"));
    let differ: Vec<&String> = golden
        .keys()
        .chain(written.keys())
        .filter(|k| golden.get(*k) != written.get(*k))
        .collect();
    println!(
        "FM-6: {} of {} files of golden/J-E-2416.ew equal byte for byte; differing: {differ:?}",
        golden
            .keys()
            .filter(|k| golden.get(*k) == written.get(*k))
            .count(),
        golden.len()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An unreadable table refuses the import with the reader's line (FM-6), before anything is written; a non-empty
/// target refuses the write and keeps what is there; an empty one is replaced by the project.
#[test]
fn an_unreadable_table_and_a_non_empty_target_are_refused() {
    let dir = ew_testdir::fresh_dir("ew-api-import-csv-refused");
    let copy = dir.join("tables");
    std::fs::create_dir_all(&copy).unwrap();
    for (name, bytes) in tree(&tables()) {
        std::fs::write(copy.join(name), bytes).unwrap();
    }
    let devices = std::fs::read_to_string(copy.join("devices.csv")).unwrap();
    std::fs::write(copy.join("devices.csv"), format!("{devices}a,b\n")).unwrap();
    let lines = devices.lines().count() + 1;
    let fields = devices.lines().next().unwrap().split(',').count();
    match import_tables(&read_tables(&copy).unwrap(), &library(), ImportMode::Strict) {
        Err(e @ TablesError::Unreadable { .. }) => assert_eq!(
            e.to_string(),
            format!("devices.csv: line {lines}: expected {fields} fields, found 2; nothing was imported")
        ),
        other => panic!(
            "expected the unreadable table, got {:?}",
            other.map(|b| b.records.len())
        ),
    }

    let batch = import_tables(&read_tables(&tables()).unwrap(), &library(), ImportMode::Strict).unwrap();
    let full = dir.join("full");
    std::fs::create_dir_all(&full).unwrap();
    std::fs::write(full.join("notes.txt"), "keep").unwrap();
    match create_project(&batch, &full, store()) {
        Err(e @ CreateError::NotEmpty(_)) => assert_eq!(
            e.to_string(),
            format!(
                "{}: not an empty directory; the new project needs a new or empty directory",
                full.display()
            )
        ),
        other => panic!("expected the refusal, got {other:?}"),
    }
    assert_eq!(tree(&full).into_keys().collect::<Vec<_>>(), ["notes.txt"]);
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    create_project(&batch, &empty, store()).unwrap();
    assert!(empty.join("manifest.json").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
