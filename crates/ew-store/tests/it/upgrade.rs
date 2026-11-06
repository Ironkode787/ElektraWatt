// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A project of an older format through the store (WP-S1.17 D10, D11 and D3, decision log ID-1221 rulings 3, 4, 5
//! and 7): the migrated open (MG-1), a migration that fails (MG-2), the consent gate of the first save, the unstable
//! marker and its read side (SG-1). The `"0.0"` fixture is built here: a project record at `$v` 0 with its member's
//! old name, a library copy no step changes, and a manifest of format `"0.0"` with a member the format step renames.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{options, project_files};
use ew_store::ew_format::ew_schema::{FORMAT, Kind, LibDocument, Project, Record, Sha256};
use ew_store::ew_format::{
    Access as FormatAccess, FileHash, FilePath, FormatStep, KindStep, Limits, MANIFEST, Manifest,
    MigrationError, Migrations, sha256,
};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{
    Access, Changes, OpenError, Opened, ReadOnlyReason, SaveError, Store, StoreOptions, Vfs, join,
};
use serde_json::Value;

const PROJECT: &str =
    r#"{"$k":"project","id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;
/// [`PROJECT`] as the test format `"0.0"` wrote it.
const PROJECT_V0: &str =
    r#"{"$k":"project","$v":0,"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","profile":"2027.1"}"#;

fn project_v0(mut value: Value) -> Result<Value, MigrationError> {
    if let Some(members) = value.as_object_mut() {
        if let Some(profile) = members.remove("profile") {
            members.insert("derivation_profile".to_owned(), profile);
        }
    }
    Ok(value)
}

fn same(value: Value) -> Result<Value, MigrationError> {
    Ok(value)
}

fn manifest_step(mut value: Value) -> Result<Value, MigrationError> {
    if let Some(members) = value.as_object_mut() {
        if let Some(old) = members.remove("x_old") {
            members.insert("x_new".to_owned(), old);
        }
    }
    Ok(value)
}

fn fails(_: Value) -> Result<Value, MigrationError> {
    Err(MigrationError::at("/profile", "the profile cannot be read"))
}

const FORMAT_STEP: FormatStep = FormatStep {
    from: "0.0",
    to: "0.1",
    manifest: manifest_step,
    record: same,
};

/// MG-1's test chain.
static CHAIN: Migrations = Migrations {
    kinds: &[KindStep {
        kind: Kind::Project,
        from: 0,
        step: project_v0,
    }],
    formats: &[FORMAT_STEP],
};

/// MG-2's: the project's step fails.
static FAILING: Migrations = Migrations {
    kinds: &[KindStep {
        kind: Kind::Project,
        from: 0,
        step: fails,
    }],
    formats: &[FORMAT_STEP],
};

/// The fndef of the repository's library with a correct copy hash.
fn fndef() -> LibDocument {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/fndefs/ew.fn.coil.json");
    let mut doc = LibDocument::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    let LibDocument::Fndef(copy) = &mut doc else {
        panic!("not a fndef");
    };
    copy.sha256 = Sha256(sha256(serde_json::to_string(&copy.item).unwrap().as_bytes()));
    doc
}

fn project() -> Project {
    match Record::from_json(PROJECT).unwrap() {
        Record::Project(project) => project,
        other => panic!("not a project: {other:?}"),
    }
}

/// The current-format project: the project record and one library copy.
fn current_files() -> BTreeMap<String, Vec<u8>> {
    ew_store::ew_format::project_files(&project(), &[], &[fndef()], 1, "test")
        .unwrap()
        .into_iter()
        .map(|(path, bytes)| (path.as_str().to_owned(), bytes))
        .collect()
}

/// [`current_files`] as the test format `"0.0"` wrote them.
fn files_v0() -> BTreeMap<String, Vec<u8>> {
    let mut files = current_files();
    let project = format!("{PROJECT_V0}\n").into_bytes();
    let mut manifest = Manifest::parse(&files[MANIFEST], &Limits::default()).unwrap();
    manifest.format = "0.0".to_owned();
    manifest.min_reader = "0.0".to_owned();
    manifest.files.insert(
        FilePath::new("project.jsonl").unwrap(),
        FileHash::Sha256(sha256(&project)),
    );
    manifest.extra.insert("x_old".to_owned(), "kept".into());
    files.insert("project.jsonl".to_owned(), project);
    files.insert(MANIFEST.to_owned(), manifest.to_text().into_bytes());
    files
}

fn mem_of(files: BTreeMap<String, Vec<u8>>) -> (Arc<MemVfs>, PathBuf) {
    common::mem_project_of(DirSync::Explicit, files)
}

fn chain_options(vfs: &Arc<MemVfs>, chain: &'static Migrations) -> StoreOptions {
    let mut o = options(vfs, 1);
    o.migrations = chain;
    o
}

fn manifest_on(vfs: &MemVfs, root: &Path) -> Manifest {
    Manifest::parse(&vfs.read(&join(root, MANIFEST)).unwrap(), &Limits::default()).unwrap()
}

/// MG-1 through the store: the `"0.0"` project opens editable and migrated, with no finding for the file a step
/// changed; that file, and only it, is in the next save; the shipped chain is empty and refuses the open.
#[test]
fn mg1_an_older_project_opens_migrated_with_its_changed_files_dirty() {
    let (vfs, root) = mem_of(files_v0());
    let Opened { store, model, report } = Store::open(&root, chain_options(&vfs, &CHAIN)).expect("opens");
    assert_eq!(report.access, Access::Edit);
    assert_eq!(report.format, FormatAccess::Migrate);
    assert_eq!(report.migrated_from.as_deref(), Some("0.0"));
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    assert_eq!(store.migrated_from(), Some("0.0"));
    assert_eq!(**model.project(), project());
    let dirty: Vec<String> = store
        .canonical_rewrite()
        .unwrap()
        .into_keys()
        .map(|p| p.as_str().to_owned())
        .collect();
    assert_eq!(dirty, ["project.jsonl"]);
    store.close();

    let (vfs, root) = mem_of(files_v0());
    let e = Store::open(&root, options(&vfs, 1)).expect_err("the shipped chain refuses");
    assert!(matches!(e, OpenError::Migration(_)), "{e:?}");
    assert_eq!(
        e.to_string(),
        "the project cannot be migrated from its older format: manifest.json at /format: this version has no \
         migration step from format 0.0 towards 0.1"
    );
}

/// MG-2: a step that fails refuses the open with its file, line and pointer; nothing on disk changes.
#[test]
fn mg2_a_failing_step_refuses_the_open_and_changes_nothing() {
    let (vfs, root) = mem_of(files_v0());
    let before = vfs.files(&root);
    let e = Store::open(&root, chain_options(&vfs, &FAILING)).expect_err("refused");
    assert_eq!(
        e.to_string(),
        "the project cannot be migrated from its older format: project.jsonl line 1 at /profile: the profile \
         cannot be read"
    );
    let OpenError::Migration(m) = e else {
        panic!("not a migration error");
    };
    assert_eq!(m.pointer, "/profile");
    assert_eq!(vfs.files(&root), before);
}

/// The consent gate (ruling 5): the first save refuses with `UpgradeConsent` and writes nothing; after
/// `confirm_upgrade` it writes this version's format, the manifest step's member and the migrated file, and nothing
/// else; the reopened project is not migrated, and a second save writes nothing.
#[test]
fn the_first_save_needs_the_consent_and_then_converts_the_project() {
    let (vfs, root) = mem_of(files_v0());
    let before = project_files(&vfs, &root);
    let Opened { mut store, model, .. } = Store::open(&root, chain_options(&vfs, &CHAIN)).expect("opens");
    let e = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect_err("asks first");
    assert!(
        matches!(&e, SaveError::UpgradeConsent { from } if from == "0.0"),
        "{e:?}"
    );
    assert_eq!(
        e.to_string(),
        format!(
            "the project is in the older format 0.0, and saving converts it to format {FORMAT}, which older versions \
             of ElektraWatt cannot open; upgrade it first with `ew upgrade`, which keeps a backup"
        )
    );
    assert_eq!(project_files(&vfs, &root), before, "nothing was written");

    store.confirm_upgrade();
    let saved = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("saves");
    assert_eq!(
        saved.written.iter().map(FilePath::as_str).collect::<Vec<_>>(),
        ["project.jsonl"]
    );
    assert_eq!(store.migrated_from(), None);
    let after = project_files(&vfs, &root);
    assert_eq!(after["project.jsonl"], format!("{PROJECT}\n").into_bytes());
    let lib = ew_store::ew_format::lib_file(&fndef()).unwrap();
    assert_eq!(
        after[lib.as_str()],
        before[lib.as_str()],
        "a file no step changed is not rewritten"
    );
    let manifest = manifest_on(&vfs, &root);
    assert_eq!(manifest.format, FORMAT);
    assert_eq!(manifest.min_reader, FORMAT);
    assert!(!manifest.unstable);
    assert_eq!(manifest.extra.get("x_new"), Some(&Value::from("kept")));
    assert_eq!(manifest.extra.get("x_old"), None);
    let again = store
        .save(&model, Changes::Keys(&BTreeSet::new()))
        .expect("saves");
    assert!(again.written.is_empty() && !again.manifest_written, "{again:?}");
    store.close();

    let Opened { store, report, .. } = Store::open(&root, chain_options(&vfs, &CHAIN)).expect("reopens");
    assert_eq!(report.format, FormatAccess::ReadWrite);
    assert_eq!(report.migrated_from, None);
    assert!(report.findings.is_empty(), "{:?}", report.findings);
    store.close();
}

/// A store migrated read-only refuses as read-only, before the consent; a store that was not migrated ignores the
/// consent.
#[test]
fn the_consent_changes_nothing_for_a_read_only_or_current_store() {
    let (vfs, root) = mem_of(files_v0());
    let mut o = chain_options(&vfs, &CHAIN);
    o.mode = ew_store::OpenMode::ReadOnly;
    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, o).expect("opens");
    assert_eq!(report.migrated_from.as_deref(), Some("0.0"));
    let e = store.save(&model, Changes::All).expect_err("read-only");
    assert!(
        matches!(e, SaveError::ReadOnly(ReadOnlyReason::Requested)),
        "{e:?}"
    );

    let (vfs, root) = mem_of(current_files());
    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, chain_options(&vfs, &CHAIN)).expect("opens");
    assert_eq!(report.migrated_from, None);
    store.confirm_upgrade();
    let saved = store.save(&model, Changes::All).expect("saves");
    assert!(saved.written.is_empty() && !saved.manifest_written, "{saved:?}");
    store.close();
}

/// FT-1489 (WP-S6.08 D19): `Store::create` reports the marker as a save does: `writes_unstable` says what the manifest
/// it wrote holds, by the released list.
#[test]
fn a_created_project_reports_its_unstable_marker() {
    static RELEASED_OLDER: [&str; 1] = ["0.0"];
    static RELEASED_THIS: [&str; 1] = [FORMAT];
    for (released, marked) in [(&RELEASED_THIS[..], false), (&RELEASED_OLDER[..], true)] {
        let (vfs, root) = mem_of(current_files());
        let Opened { store, model, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
        store.close();
        let target = PathBuf::from("/copy.ew");
        let mut o = options(&vfs, 2);
        o.released = released;
        let created = Store::create(&target, &model, o).expect("creates");
        assert_eq!(created.writes_unstable(), marked, "{released:?}");
        assert_eq!(manifest_on(&vfs, &target).unstable, marked, "{released:?}");
        created.close();
    }
}

/// SG-1's `unstable` clauses (ruling 7): nothing is marked while no format is released; with an injected released list
/// that does not hold the format written, the save marks the manifest; a reader of that unreleased format opens it
/// read-write and a released one read-only.
#[test]
fn the_unstable_marker_follows_the_released_list() {
    static RELEASED_OLDER: [&str; 1] = ["0.0"];
    static RELEASED_THIS: [&str; 1] = [FORMAT];
    for (released, marked) in [
        (&[][..], false),
        (&RELEASED_THIS[..], false),
        (&RELEASED_OLDER[..], true),
    ] {
        let (vfs, root) = mem_of(current_files());
        let mut o = options(&vfs, 1);
        o.released = released;
        let Opened { mut store, model, .. } = Store::open(&root, o).expect("opens");
        let saved = store.save(&model, Changes::All).expect("saves");
        assert_eq!(saved.manifest_written, marked, "{released:?}");
        assert_eq!(manifest_on(&vfs, &root).unstable, marked, "{released:?}");
        // FT-1154 (WP-S6.08 D3): the save's result says so, for the front ends' warning.
        assert_eq!(saved.unstable, marked, "{released:?}");
        store.close();
        if !marked {
            continue;
        }
        let Opened { store, report, .. } = Store::open(&root, options(&vfs, 2)).expect("reopens");
        assert_eq!(
            report.access,
            Access::Edit,
            "an unreleased reader of its format edits it"
        );
        store.close();
        let mut o = options(&vfs, 3);
        o.release_build = true;
        let Opened { store, report, .. } = Store::open(&root, o).expect("reopens");
        assert_eq!(
            report.access,
            Access::ReadOnly(ReadOnlyReason::Format(
                "written by a development build with an unreleased format"
            ))
        );
        store.close();
    }
}
