// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Session::new_project` (FT-307, FR-PRJ-009): the built-in basic project, a company basic project directory,
//! the kind, and the new identities of a copy (doc 07 §2.8).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ew_api::{NewProjectKind, ProjectTemplate, Recovery, Session, SessionOptions, StoreOptions};
use ew_commands::testkit::seeded_ids;

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

fn temp(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ew-api-newproject-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn store(state: &Path) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.state_dir = Some(state.to_path_buf());
    o
}

fn options(seed: u64) -> SessionOptions {
    SessionOptions {
        ids: Some(seeded_ids(seed)),
        ..SessionOptions::default()
    }
}

fn project_text(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("project.jsonl")).unwrap()
}

/// Every UUID that appears in the lines of a session's model.
fn ids_of(session: &Session) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (_, line) in ew_commands::rows::model_lines(&session.world().model).unwrap() {
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if let Some(id) = v.get("id").and_then(serde_json::Value::as_str) {
            out.insert(id.to_owned());
        }
    }
    out
}

#[test]
fn the_built_in_basic_project_has_one_empty_page_and_round_trips() {
    let work = temp("builtin");
    let mut session =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options(1)).unwrap();
    assert_eq!(session.page_labels(), ["1"]);
    assert!(!session.is_dirty());
    let dir = work.join("p.ew");
    session.create(&dir, store(&work.join("state"))).unwrap();
    assert!(!session.is_dirty(), "a created project is saved");
    session.close();
    let text = project_text(&dir);
    assert!(text.contains("\"derivation_profile\":\"2027.1\""), "{text}");
    assert!(text.contains("EN81346-2:2009"), "{text}");
    assert!(
        !text.contains("\"kind\""),
        "a schematic project writes no kind: {text}"
    );

    let (reopened, report) = Session::open(
        &dir,
        store(&work.join("state")),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert!(matches!(report.access, ew_api::Access::Edit));
    assert_eq!(reopened.page_labels(), ["1"]);
    // The page derives and exports on the built-in frame.
    let pdf = reopened
        .export_pdf(&ew_api::output::ExportOptions::default())
        .unwrap();
    assert!(pdf.bytes.starts_with(b"%PDF"));
    reopened.close();
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn the_kind_basic_is_written_and_a_basic_project_is_a_template() {
    let work = temp("basic");
    let mut basic =
        Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Basic, options(2)).unwrap();
    let basic_dir = work.join("company.ew");
    basic.create(&basic_dir, store(&work.join("state"))).unwrap();
    basic.close();
    assert!(project_text(&basic_dir).contains("\"kind\":\"basic\""));

    let mut copy = Session::new_project(
        ProjectTemplate::Directory(&basic_dir),
        NewProjectKind::Schematic,
        options(3),
    )
    .unwrap();
    let copy_dir = work.join("new.ew");
    copy.create(&copy_dir, store(&work.join("state"))).unwrap();
    copy.close();
    let text = project_text(&copy_dir);
    assert!(
        !text.contains("\"kind\""),
        "the copy is a schematic project: {text}"
    );
    assert_ne!(text, project_text(&basic_dir), "a new project ID");
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn a_directory_template_copies_the_records_under_new_ids() {
    let work = temp("golden");
    let (golden, _) = Session::open_read_only(Path::new(GOLDEN_A), "ElektraWatt test").unwrap();
    let mut copy = Session::new_project(
        ProjectTemplate::Directory(Path::new(GOLDEN_A)),
        NewProjectKind::Schematic,
        options(4),
    )
    .unwrap();
    assert_eq!(copy.page_labels(), golden.page_labels(), "the same pages");
    let (old, new) = (ids_of(&golden), ids_of(&copy));
    assert!(old.len() > 100, "{} records", old.len());
    assert_eq!(old.len(), new.len(), "the same records");
    // Instantiated records are re-identified; library copies keep their IDs (their IDs derive from library and item).
    let shared = old.intersection(&new).count();
    assert!(
        shared < old.len() / 2,
        "{shared} of {} IDs survived; only library copies and the like may",
        old.len()
    );
    // The result is a sound project: it saves, reopens without integrity findings and derives the same page texts.
    let dir = work.join("copy.ew");
    copy.create(&dir, store(&work.join("state"))).unwrap();
    copy.close();
    let (reopened, report) = Session::open(
        &dir,
        store(&work.join("state")),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert!(matches!(report.access, ew_api::Access::Edit));
    assert!(
        ew_api::integrity_items(&report)
            .iter()
            .all(|i| i.severity == ew_api::checks::Severity::Info),
        "{:?}",
        ew_api::integrity_items(&report)
    );
    assert_eq!(reopened.page_labels(), golden.page_labels());
    reopened.close();
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn a_directory_that_is_no_project_is_an_error() {
    let work = temp("missing");
    let err = Session::new_project(
        ProjectTemplate::Directory(&work.join("nope.ew")),
        NewProjectKind::Schematic,
        options(5),
    )
    .unwrap_err();
    assert!(matches!(err, ew_api::OpenError::NotADirectory(_)), "{err}");
    let err = Session::new_project(
        ProjectTemplate::Directory(&work),
        NewProjectKind::Schematic,
        options(5),
    )
    .unwrap_err();
    assert!(matches!(err, ew_api::OpenError::NoProject(_)), "{err}");
    let _ = std::fs::remove_dir_all(&work);
}
