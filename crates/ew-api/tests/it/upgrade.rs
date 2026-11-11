// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The upgrade of a project of an older format through the session:
//! MG-3 (the consent, the verified backup before any project byte, the backup extracting byte-equal to the original
//! folder, no second question) and MG-4 (declining, or closing between the consent and the save, leaves every project
//! file byte-identical), with the backup's name sequence and a backup that cannot be written. The `"0.0"` fixture is a
//! small project built here whose project record is at `$v` 0 under its member's old name.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use ew_api::{AnyCommand, Recovery, SaveError, SaveStep, Session, SessionOptions};
use ew_store::ew_format::ew_schema::{FORMAT, Kind};
use ew_store::ew_format::{
    FileHash, FilePath, FormatStep, KindStep, Limits, MANIFEST, Manifest, MigrationError, Migrations, sha256,
};
use ew_store::ewz;
use ew_store::lock::Identity;
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{ManualClock, ProcessProbe, ProcessState, StoreOptions, Vfs};
use ew_testids::seeded_ids;
use serde_json::Value;

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

/// A probe that knows exactly one live process.
#[derive(Debug)]
struct Alive(u32);

impl ProcessProbe for Alive {
    fn probe(&self, pid: u32) -> ProcessState {
        if pid == self.0 {
            ProcessState::Alive { started: None }
        } else {
            ProcessState::Dead
        }
    }
}

fn project_v0(mut value: Value) -> Result<Value, MigrationError> {
    if let Some(members) = value.as_object_mut()
        && let Some(profile) = members.remove("profile")
    {
        members.insert("derivation_profile".to_owned(), profile);
    }
    Ok(value)
}

fn same(value: Value) -> Result<Value, MigrationError> {
    Ok(value)
}

/// The test chain: the project's kind step from `$v` 0, and the format step from `"0.0"`.
static CHAIN: Migrations = Migrations {
    kinds: &[KindStep {
        kind: Kind::Project,
        from: 0,
        step: project_v0,
    }],
    formats: &[FormatStep {
        from: "0.0",
        to: "0.1",
        manifest: same,
        record: same,
    }],
};

fn root() -> PathBuf {
    PathBuf::from("/p")
}

/// The backup's first name.
fn backup() -> PathBuf {
    PathBuf::from("/p.pre-0.0.ewz")
}

/// Store options on `vfs` for process `pid` with the test chain; every other process is dead.
fn options(vfs: &Arc<MemVfs>, pid: u32) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.vfs = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.clock = Arc::new(ManualClock::fixed());
    o.probe = Arc::new(Alive(pid));
    o.identity = Identity {
        session: ew_store::lock::new_session_id(),
        user: "<os user>".into(),
        host: "<host>".into(),
        pid,
        pid_started: UNIX_EPOCH + Duration::from_secs(1_792_222_918),
        app_version: "ElektraWatt test".into(),
    };
    o.state_dir = Some(PathBuf::from("/state"));
    o.migrations = &CHAIN;
    o
}

/// The project's files without the lock.
fn files(vfs: &MemVfs) -> BTreeMap<String, Vec<u8>> {
    vfs.files(&root())
        .into_iter()
        .filter(|(name, _)| !name.starts_with(".~ew.lock"))
        .collect()
}

/// The files beside the project folder that an upgrade writes: its backups and their temporary names.
fn beside(vfs: &MemVfs) -> Vec<String> {
    vfs.files(Path::new("/"))
        .into_keys()
        .filter(|name| name.starts_with("p.pre-"))
        .collect()
}

/// A small project (a section, two pages, two devices) as the test format `"0.0"` wrote it, and the bytes of its
/// `project.jsonl` in this version's format.
fn files_v0() -> (BTreeMap<String, Vec<u8>>, Vec<u8>) {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let mut s = Session::from_records(
        serde_json::from_str(PROJECT).unwrap(),
        [],
        [],
        SessionOptions {
            ids: Some(seeded_ids(48)),
            ..SessionOptions::default()
        },
    )
    .unwrap();
    for (name, params) in [
        (
            "section.create",
            serde_json::json!({"number": 1, "folio_range": [1, 99]}),
        ),
        ("page.create", serde_json::json!({"number": 1})),
        ("page.create", serde_json::json!({"number": 2})),
        ("device.create", serde_json::json!({"letter": "K", "counter": 1})),
        ("device.create", serde_json::json!({"letter": "K", "counter": 2})),
    ] {
        s.execute(&AnyCommand {
            name: name.into(),
            params,
        })
        .unwrap();
    }
    s.create(&root(), options(&vfs, 1)).unwrap();
    s.close();
    let mut files = files(&vfs);
    // The project record at `$v` 0 under its member's old name, and the manifest of format "0.0" hashed to it.
    let text = String::from_utf8(files["project.jsonl"].clone()).unwrap();
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: Value = serde_json::from_str(line).unwrap();
            if value["$k"] == "project" {
                let profile = value
                    .as_object_mut()
                    .unwrap()
                    .remove("derivation_profile")
                    .expect("a profile");
                value["profile"] = profile;
                value["$v"] = 0.into();
                return value.to_string();
            }
            line.to_owned()
        })
        .collect();
    let project = format!("{}\n", lines.join("\n")).into_bytes();
    let mut manifest = Manifest::parse(&files[MANIFEST], &Limits::default()).unwrap();
    manifest.format = "0.0".to_owned();
    manifest.min_reader = "0.0".to_owned();
    manifest.files.insert(
        FilePath::new("project.jsonl").unwrap(),
        FileHash::Sha256(sha256(&project)),
    );
    let current = files
        .insert("project.jsonl".to_owned(), project)
        .expect("a project file");
    // Compact, unlike `Manifest::to_text`'s, and read all the same: the backup keeps these bytes.
    let compact: Value = serde_json::from_str(&manifest.to_text()).unwrap();
    let compact = format!("{compact}\n");
    assert_ne!(compact, manifest.to_text());
    files.insert(MANIFEST.to_owned(), compact.into_bytes());
    assert_ne!(files["project.jsonl"], current);
    (files, current)
}

fn mem(files: &BTreeMap<String, Vec<u8>>) -> Arc<MemVfs> {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    for (name, bytes) in files {
        vfs.put(&ew_store::join(&root(), name), bytes);
    }
    vfs.put_dir(Path::new("/state"));
    vfs
}

/// Opens the project for editing as process `pid`: migrated from `"0.0"`.
fn open(vfs: &Arc<MemVfs>, pid: u32) -> Session {
    let (s, report) = Session::open(
        &root(),
        options(vfs, pid),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert_eq!(report.access, ew_api::Access::Edit);
    assert_eq!(report.migrated_from.as_deref(), Some("0.0"));
    s
}

fn consent_text() -> String {
    format!(
        "the project is in the older format 0.0, and saving converts it to format {FORMAT}, which older versions of \
         ElektraWatt cannot open; upgrade it first with `ew upgrade`, which keeps a backup"
    )
}

/// MG-3: the first save asks; after the consent the backup is written and read back before any project file is
/// written, extracts byte-equal to the original folder, and the project is then saved in this version's format; a
/// second save does not ask and writes no second backup.
#[test]
fn the_consented_save_writes_the_backup_first_and_then_the_project() {
    let (disk, current) = files_v0();
    let vfs = mem(&disk);
    let mut s = open(&vfs, 2);
    assert!(s.is_dirty(), "a migrated project differs from its files");
    let e = s.save().expect_err("asks first");
    assert!(
        matches!(&e, SaveError::UpgradeConsent { from } if from == "0.0"),
        "{e:?}"
    );
    assert_eq!(e.to_string(), consent_text());
    assert_eq!(files(&vfs), disk);
    assert_eq!(beside(&vfs), Vec::<String>::new());

    s.confirm_upgrade();
    vfs.set_trace(true);
    let saved = s.save().unwrap();
    let trace: Vec<String> = vfs.trace().iter().map(|l| l.replace('\\', "/")).collect();
    vfs.set_trace(false);
    assert_eq!(saved.generation, 2);
    assert_eq!(s.upgrade_backup(), Some(backup().as_path()));
    assert!(!s.is_dirty());
    // The backup is in place before the first call that changes anything in the project folder.
    let placed = trace
        .iter()
        .position(|l| l.ends_with(" rename_noreplace /p.pre-0.0.ewz.tmp -> /p.pre-0.0.ewz"))
        .unwrap_or_else(|| panic!("the backup is moved into place: {trace:#?}"));
    let first_write = trace
        .iter()
        .position(|l| {
            let op = l.split_whitespace().nth(1).unwrap_or_default();
            !matches!(op, "read" | "read_at" | "stat" | "list" | "canonicalize") && l.contains(" /p/")
        })
        .expect("the save writes");
    assert!(placed < first_write, "{trace:#?}");
    assert_eq!(beside(&vfs), ["p.pre-0.0.ewz"]);
    // It extracts byte-equal to the original folder.
    let packed = ewz::read(&vfs.read(&backup()).unwrap()).expect("the backup verifies");
    ewz::extract(&*vfs, &packed, Path::new("/restored")).unwrap();
    assert_eq!(vfs.files(Path::new("/restored")), disk);
    // The project is in this version's format, its project record migrated.
    let after = files(&vfs);
    let manifest = Manifest::parse(&after[MANIFEST], &Limits::default()).unwrap();
    assert_eq!((manifest.format.as_str(), manifest.generation), (FORMAT, 2));
    assert_eq!(after["project.jsonl"], current);
    for (name, bytes) in disk
        .iter()
        .filter(|(n, _)| !matches!(n.as_str(), "project.jsonl" | "manifest.json"))
    {
        assert_eq!(&after[name], bytes, "{name} is not written");
    }
    // A second save does not ask.
    let again = s.save().unwrap();
    assert!(again.written.is_empty() && !again.manifest_written, "{again:?}");
    s.close();
    assert_eq!(beside(&vfs), ["p.pre-0.0.ewz"]);
    let (s, report) = Session::open(
        &root(),
        options(&vfs, 3),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert_eq!(report.migrated_from, None);
    s.close();
}

/// MG-4: declining (no consent, the session closed) and a session closed between the consent and the save leave
/// every project file byte-identical and write no backup; the next open asks again.
#[test]
fn declining_or_closing_before_the_save_changes_nothing() {
    let (disk, _) = files_v0();
    let vfs = mem(&disk);
    let mut s = open(&vfs, 2);
    assert_eq!(s.save().expect_err("asks").to_string(), consent_text());
    s.close();
    assert_eq!(files(&vfs), disk);

    let mut s = open(&vfs, 3);
    s.confirm_upgrade();
    s.close();
    assert_eq!(files(&vfs), disk);
    assert_eq!(beside(&vfs), Vec::<String>::new());

    let mut s = open(&vfs, 4);
    assert_eq!(s.save().expect_err("asks again").to_string(), consent_text());
    s.close();
    assert_eq!(files(&vfs), disk);
}

/// Ruling 5's names: past a name that holds another file the backup takes `-2`; a name that holds this backup is
/// reused. A backup that cannot be written refuses the save at `SaveStep::Backup`, nothing written.
#[test]
fn the_backup_moves_past_a_taken_name_reuses_its_own_and_refuses_when_unwritable() {
    let (disk, _) = files_v0();
    // The bytes of the backup, from an upgrade of a copy.
    let bytes = {
        let vfs = mem(&disk);
        let mut s = open(&vfs, 2);
        s.confirm_upgrade();
        s.save().unwrap();
        s.close();
        vfs.read(&backup()).unwrap()
    };

    let vfs = mem(&disk);
    vfs.put(&backup(), b"another file");
    let mut s = open(&vfs, 2);
    s.confirm_upgrade();
    s.save().unwrap();
    s.close();
    assert_eq!(beside(&vfs), ["p.pre-0.0-2.ewz", "p.pre-0.0.ewz"]);
    assert_eq!(vfs.read(&backup()).unwrap(), b"another file");
    assert_eq!(vfs.read(Path::new("/p.pre-0.0-2.ewz")).unwrap(), bytes);

    let vfs = mem(&disk);
    vfs.put(&backup(), &bytes);
    let mut s = open(&vfs, 2);
    s.confirm_upgrade();
    s.save().unwrap();
    assert_eq!(s.upgrade_backup(), Some(backup().as_path()));
    s.close();
    assert_eq!(beside(&vfs), ["p.pre-0.0.ewz"]);

    let vfs = mem(&disk);
    let mut s = open(&vfs, 2);
    s.confirm_upgrade();
    vfs.set_read_only(true);
    let e = s.save().expect_err("the backup cannot be written");
    vfs.set_read_only(false);
    assert!(
        matches!(
            &e,
            SaveError::Failed {
                step: SaveStep::Backup,
                ..
            }
        ),
        "{e:?}"
    );
    assert_eq!(
        e.to_string(),
        "save failed at Backup (/p.pre-0.0.ewz): read-only file system; nothing changed on disk"
    );
    assert_eq!(files(&vfs), disk);
    assert_eq!(beside(&vfs), Vec::<String>::new());
    // The consent stands: the next save writes the backup and saves.
    s.save().unwrap();
    s.close();
    assert_eq!(beside(&vfs), ["p.pre-0.0.ewz"]);
}

/// Decision log ID-1230 ruling 4: an unsaved migration makes only an editing session dirty. A read-only migrated
/// session is not, so the purge's preview (a reader) lists instead of refusing as unsaved; an editing one is, and
/// `ew fmt`'s format refuses it as unsaved before the store's consent gate (its words are FT-1148's).
#[test]
fn only_an_editing_migrated_session_is_dirty() {
    let (disk, _) = files_v0();
    let vfs = mem(&disk);
    let mut o = options(&vfs, 2);
    o.mode = ew_store::OpenMode::ReadOnly;
    o.state_dir = None;
    let (s, report) = Session::open(&root(), o, SessionOptions::default(), Recovery::Keep).unwrap();
    assert_eq!(report.migrated_from.as_deref(), Some("0.0"));
    assert!(
        matches!(report.access, ew_api::Access::ReadOnly(_)),
        "{:?}",
        report.access
    );
    assert!(!s.is_dirty());
    assert!(s.purge_preview().is_ok());
    s.close();

    let mut s = open(&vfs, 3);
    assert!(s.is_dirty());
    assert_eq!(
        ew_api::fmt::format(&mut s).unwrap_err().to_string(),
        "the session has unsaved changes; a format writes only the saved project"
    );
    s.close();
    assert_eq!(files(&vfs), disk);
}
