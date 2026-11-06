// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S3.09 step 4: a QET project becomes a project through `import.apply` (D7, D13). IC-6: the synthetic folios 101 to
//! 103 written as a new project hold the batch's records, an import run under `imports/` and the lineage's `last_run`,
//! and two imports with the same ID seed give the same bytes; IC-8: the run reads back unchanged after a save and a
//! reopen; the import is one non-undoable checkpoint.

use crate::golden_common::repo;
use crate::qet_synth::build;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::import::{StarterLibrary, create_project_from, import_qet_with};
use ew_api::{Origin, Recovery, Session, SessionOptions, StoreOptions};

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let name = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(name, std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn imported_session(seed: u64) -> Session {
    let library = StarterLibrary::from_dir(&repo().join("content")).expect("the starter content");
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(seed)),
        ..SessionOptions::default()
    };
    let imported = import_qet_with("synthetic.qet", build().text.into_bytes(), &library, options)
        .expect("the import reads the file");
    imported.session.expect("import.apply commits the batch")
}

fn store() -> StoreOptions {
    StoreOptions::new("0.1.0-test")
}

#[test]
fn an_import_is_written_with_its_run_and_reproducibly() {
    let root = Dir(ew_testdir::fresh_dir("ew-api-import-apply"));
    std::fs::create_dir_all(&root.0).unwrap();
    let mut s = imported_session(7);
    // The checkpoint: nothing before it can be undone.
    assert!(s.undo(&Origin::Gui).is_err());
    let (a, b) = (root.0.join("a"), root.0.join("b"));
    create_project_from(s, &a, store()).unwrap();
    create_project_from(imported_session(7), &b, store()).unwrap();
    let (fa, fb) = (files(&a), files(&b));
    let runs: Vec<&String> = fa.keys().filter(|k| k.starts_with("imports/")).collect();
    assert_eq!(runs.len(), 1, "{:?}", fa.keys().collect::<Vec<_>>());
    let without_state = |m: &BTreeMap<String, Vec<u8>>| -> BTreeMap<String, Vec<u8>> {
        m.iter()
            .filter(|(k, _)| !k.starts_with(".ew"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    assert_eq!(
        without_state(&fa),
        without_state(&fb),
        "two imports with one seed give equal bytes"
    );
    let run_line = String::from_utf8(fa[runs[0]].clone()).unwrap();
    let run_id = runs[0].trim_start_matches("imports/").trim_end_matches(".jsonl");
    let project_jsonl = String::from_utf8(fa["project.jsonl"].clone()).unwrap();
    assert!(
        project_jsonl
            .lines()
            .any(|l| l.contains("\"$k\":\"import_lineage\"")
                && l.contains(&format!("\"last_run\":\"{run_id}\""))),
        "{project_jsonl}"
    );
    let run: serde_json::Value = serde_json::from_str(run_line.trim_end()).unwrap();
    assert_eq!(run["mode"], "lenient");
    assert_eq!(run["source"]["name"], "synthetic.qet");
    assert!(run["record_hashes"].as_object().is_some_and(|h| !h.is_empty()));

    // IC-9, IC-10 (WP-S3.09 D9): the migration scheme lists every entry of the run once, as an EW-IMP finding with the
    // entry's severity and message, and lists the same after the project is reopened.
    let import_findings = |s: &Session| -> Vec<(String, String, String)> {
        let report = s.check(&ew_api::checks::MIGRATION, ew_api::checks::CheckMode::Release);
        let mut v: Vec<(String, String, String)> = report
            .findings
            .iter()
            .filter(|f| f.rule.starts_with("EW-IMP-"))
            .map(|f| (f.rule.to_owned(), f.severity.name().to_owned(), f.message.clone()))
            .collect();
        v.sort();
        v
    };
    let entries = run["entries"].as_array().map_or(0, Vec::len);
    let (reopened_once, _) = Session::open(&b, store(), SessionOptions::default(), Recovery::Keep).unwrap();
    let listed = import_findings(&reopened_once);
    reopened_once.close();
    assert!(entries > 0, "the synthetic import reports something");
    assert_eq!(listed.len(), entries, "{listed:?}");
    let pre = ew_api::checks::PRE_DELIVERY;
    let (plain, _) = Session::open(&b, store(), SessionOptions::default(), Recovery::Keep).unwrap();
    let release = plain.check(&pre, ew_api::checks::CheckMode::Release);
    assert!(
        !release.findings.iter().any(|f| f.rule.starts_with("EW-IMP-")),
        "pre-delivery lists no import finding"
    );
    plain.close();

    // IC-8: reopened and saved again, the run's file is unchanged.
    let (mut reopened, _) = Session::open(&a, store(), SessionOptions::default(), Recovery::Keep).unwrap();
    reopened.save().unwrap();
    reopened.close();
    assert_eq!(std::fs::read(a.join(runs[0])).unwrap(), fa[runs[0]]);
}

/// FT-1489 (WP-S6.08 D19): the project `ew import qet --out` writes reports whether its manifest is marked `unstable`:
/// with a released list that lacks this build's format it is, with the format released it is not.
#[test]
fn an_imported_project_reports_an_unreleased_format() {
    static OLDER: [&str; 1] = ["0.0"];
    static THIS: [&str; 1] = [ew_api::FORMAT];
    let root = Dir(ew_testdir::fresh_dir("ew-api-import-unreleased"));
    std::fs::create_dir_all(&root.0).unwrap();
    for (n, (released, marked)) in [(&OLDER[..], true), (&THIS[..], false)].into_iter().enumerate() {
        let out = root.0.join(format!("p{n}.ew"));
        let mut options = store();
        options.released = released;
        assert_eq!(
            create_project_from(imported_session(7), &out, options).unwrap(),
            marked
        );
        let manifest = std::fs::read_to_string(out.join("manifest.json")).unwrap();
        assert_eq!(manifest.contains("\"unstable\": true"), marked, "{manifest}");
    }
}
