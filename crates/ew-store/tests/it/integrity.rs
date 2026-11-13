// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What an open and a save do with files and lines the writer does not own: findings
//! beyond the cap still force a read-only session, quarantined lines are written back byte for byte, an
//! unlisted file at a path the save writes is moved aside instead of overwritten, and only the save
//! protocol's own temps are deleted.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use common::{
    apply, device_copy, fixture, mem_project, mem_project_of, options, project_files, reopen, small_fixture,
};
use ew_model::{AnyRow, Op};
use ew_store::ew_format::ew_schema::Device;
use ew_store::ew_format::{FilePath, FindingKind, Limits};
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::{Access, BrokenLine, Changes, Opened, ReadOnlyReason, SaveError, Store, StoreOptions, join};

#[test]
fn a_finding_beyond_the_cap_still_forces_read_only_and_quarantined_bytes_survive() {
    // 1,001 duplicate lines fill the capped findings list, so the EW-CORE-005 of
    // the line after them (not UTF-8) is dropped from it; the session must still be read-only, and the
    // writer must put the line's bytes back, not U+FFFD.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let shard = files
        .keys()
        .find(|n| n.starts_with("devices/"))
        .expect("the fixture has shards")
        .clone();
    let mut bytes = files[&shard].clone();
    let first = bytes
        .split(|b| *b == b'\n')
        .next()
        .expect("a first line")
        .to_vec();
    for _ in 0..1_001 {
        bytes.extend_from_slice(&first);
        bytes.push(b'\n');
    }
    let broken: &[u8] = b"\xff\xfe{\"not\":\"utf-8\"}";
    bytes.extend_from_slice(broken);
    bytes.push(b'\n');
    vfs.put(&join(&root, &shard), &bytes);

    let Opened { store, model, report } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert!(Limits::default().max_findings < 1_001);
    assert!(
        report.findings.iter().any(|f| f.kind == FindingKind::Truncated),
        "the loader's list is capped"
    );
    assert!(
        !report.findings.iter().any(|f| f.kind == FindingKind::JsonSyntax),
        "the syntax finding is one of those dropped"
    );
    assert_eq!(
        report.access,
        Access::ReadOnly(ReadOnlyReason::Integrity),
        "a dropped finding that forces read-only still does"
    );

    let rendered = store.render_all(&model).expect("renders");
    let written = &rendered[&FilePath::new(shard.as_str()).expect("a valid path")];
    let mut line = broken.to_vec();
    line.push(b'\n');
    assert!(
        written.windows(line.len()).any(|w| w == line.as_slice()),
        "the quarantined line is written back byte for byte"
    );
    assert!(
        !written.windows(3).any(|w| w == "\u{fffd}".as_bytes()),
        "no replacement character"
    );
}

#[test]
fn a_save_moves_an_unlisted_file_aside_instead_of_overwriting_it() {
    // A colleague's `devices/<xx>.jsonl` that the manifest does not
    // list is EW-CORE-004 at open; a save that writes our own device into that shard moves the colleague's
    // file to `devices/<xx>.unlisted-2.jsonl` first and reports it, and the colleague's line survives.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let free = (0u64..=255)
        .find(|b| !files.contains_key(&format!("devices/{b:02x}.jsonl")))
        .expect("a shard the fixture does not use");
    let shard = format!("devices/{free:02x}.jsonl");
    let colleague = format!(
        "{{\"$k\":\"device\",\"id\":\"00000000-0000-7000-8000-0000000001{free:02x}\",\"kind\":\"a colleague's\"}}\n"
    );
    vfs.put(&join(&root, &shard), colleague.as_bytes());

    let Opened {
        mut store,
        model,
        report,
    } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Orphan && f.file.as_ref().is_some_and(|p| p.as_str() == shard)),
        "{:?}",
        report.findings
    );

    let template = model
        .table::<Device>()
        .iter()
        .map(|(_, d)| Arc::clone(d))
        .next()
        .expect("the fixture has devices");
    let ours = device_copy(&template, free);
    let mut keys = BTreeSet::new();
    let edited = apply(&model, &[Op::insert(AnyRow::of(ours))], &mut keys).expect("the device inserts");
    let saved = store
        .save(&edited, Changes::Keys(&keys))
        .expect("the save succeeds");
    assert_eq!(saved.generation, 2);

    let moved = FilePath::new(shard.as_str())
        .expect("a valid path")
        .unlisted(2, 0);
    assert_eq!(moved.as_str(), format!("devices/{free:02x}.unlisted-2.jsonl"));
    assert!(
        saved.findings.iter().any(|f| f.kind == FindingKind::Orphan
            && f.code() == Some("EW-CORE-004")
            && f.file.as_ref() == Some(&moved)
            && f.message.contains(&shard)),
        "{:?}",
        saved.findings
    );
    let on_disk = project_files(&vfs, &root);
    assert_eq!(
        on_disk.get(moved.as_str()).map(Vec::as_slice),
        Some(colleague.as_bytes()),
        "the colleague's file survives, byte for byte, under the unlisted name"
    );
    let ours_text = String::from_utf8(on_disk[&shard].clone()).expect("text");
    assert!(
        ours_text.contains(&format!("00000000-0000-7000-8000-{free:012x}")),
        "{ours_text}"
    );
    assert!(!ours_text.contains("a colleague's"), "{ours_text}");

    store.close();
    let again = reopen(&vfs, 2);
    assert_eq!(again.report.access, Access::Edit);
    assert!(again.model.content_eq(&edited));
    assert!(
        again
            .report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::Orphan && f.file.as_ref() == Some(&moved)),
        "every open reports the moved file: {:?}",
        again.report.findings
    );
}

#[test]
fn an_editing_open_deletes_only_the_save_protocols_temps() {
    // `*.tmp-<n>` names that are not a temp of a writer-owned file or an intent belong
    // to someone else and stay.
    let (vfs, root) = mem_project(DirSync::Explicit);
    let shard = fixture()
        .keys()
        .find(|n| n.starts_with("devices/"))
        .expect("a shard")
        .clone();
    let ours = [format!("{shard}.tmp-9"), "manifest.json.tmp-1".to_owned()];
    let theirs = [
        "notes.txt.tmp-3",
        "rules/check.rhai.tmp-2",
        "assets/logo.svg.tmp-4",
    ];
    for name in ours.iter().map(String::as_str).chain(theirs) {
        vfs.put(&join(&root, name), b"x");
    }
    let opened = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(opened.report.access, Access::Edit);
    let removed: BTreeSet<&str> = opened.report.removed.iter().map(String::as_str).collect();
    assert_eq!(removed, ours.iter().map(String::as_str).collect::<BTreeSet<_>>());
    let left = project_files(&vfs, &root);
    for name in theirs {
        assert!(left.contains_key(name), "{name} was deleted");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// `ew fmt --repair`'s open and save

/// A line that is not one JSON value.
const NOT_JSON: &[u8] = b"{\"$k\":\"device\",";

/// A line that is not UTF-8.
const NOT_UTF8: &[u8] = b"\xff\xfe{\"not\":\"utf-8\"}";

/// The small fixture in a [`MemVfs`] with `lines` appended to its first device shard: the shard's name, its bytes
/// before and the number of its lines before.
fn shard_with(lines: &[&[u8]]) -> (Arc<MemVfs>, PathBuf, String, Vec<u8>, u32) {
    let (vfs, root) = mem_project_of(DirSync::Explicit, small_fixture());
    let shard = project_files(&vfs, &root)
        .into_keys()
        .find(|n| n.starts_with("devices/"))
        .expect("a device shard");
    let before = project_files(&vfs, &root)[&shard].clone();
    let count = u32::try_from(before.iter().filter(|b| **b == b'\n').count()).unwrap();
    let mut bytes = before.clone();
    for line in lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    vfs.put(&join(&root, &shard), &bytes);
    (vfs, root, shard, before, count)
}

fn repair_options(vfs: &Arc<MemVfs>, pid: u32) -> StoreOptions {
    let mut o = options(vfs, pid);
    o.repair = true;
    o
}

/// A repair open of a project whose only forcing findings are broken lines is editable and
/// lists them from the store's own data; one save discards them, after which none is left and the shard holds its
/// bytes before. A plain editing open of the same project stays read-only.
#[test]
fn a_repair_open_lets_only_broken_lines_through_and_one_save_discards_them() {
    let (vfs, root, shard, before, n) = shard_with(&[NOT_JSON, NOT_UTF8]);
    let plain = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(plain.report.access, Access::ReadOnly(ReadOnlyReason::Integrity));
    assert_eq!(
        plain.store.broken_lines().len(),
        2,
        "the store tells them apart in any open"
    );
    plain.store.close();

    let Opened {
        mut store, report, ..
    } = Store::open(&root, repair_options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::Edit, "{:?}", report.findings);
    let file = FilePath::new(shard.as_str()).expect("a valid path");
    assert_eq!(
        store.broken_lines(),
        [
            BrokenLine {
                file: file.clone(),
                line: n + 1,
                text: NOT_JSON.to_vec(),
            },
            BrokenLine {
                file: file.clone(),
                line: n + 2,
                text: NOT_UTF8.to_vec(),
            },
        ]
    );
    let rewrite = store.repair_rewrite().expect("renders");
    assert_eq!(rewrite.keys().collect::<Vec<_>>(), [&file]);
    let saved = store.discard_broken_lines().expect("the save succeeds");
    assert_eq!(saved.generation, report.generation + 1);
    assert_eq!(store.broken_lines(), []);
    store.close();
    assert_eq!(project_files(&vfs, &root)[&shard], before);
    let again = reopen(&vfs, 2);
    assert_eq!(again.report.access, Access::Edit);
    again.store.close();
}

/// A repair open still demotes for every other finding that forces read-only, also one beyond
/// the findings cap (1,001 broken lines fill the list before the conflict marker).
#[test]
fn a_repair_open_still_demotes_for_any_other_forcing_finding() {
    let (vfs, root, ..) = shard_with(&[NOT_JSON, b"<<<<<<< HEAD"]);
    let opened = Store::open(&root, repair_options(&vfs, 1)).expect("opens");
    assert_eq!(opened.report.access, Access::ReadOnly(ReadOnlyReason::Integrity));
    opened.store.close();

    let mut lines: Vec<&[u8]> = vec![NOT_JSON; 1_001];
    lines.push(b"<<<<<<< HEAD");
    let (vfs, root, ..) = shard_with(&lines);
    let opened = Store::open(&root, repair_options(&vfs, 1)).expect("opens");
    assert!(Limits::default().max_findings < 1_001);
    assert!(
        !opened
            .report
            .findings
            .iter()
            .any(|f| f.kind == FindingKind::ConflictMarkers),
        "the marker's finding is one of those dropped"
    );
    assert_eq!(
        opened.report.access,
        Access::ReadOnly(ReadOnlyReason::Integrity),
        "a dropped finding beyond a repair still demotes"
    );
    opened.store.close();
}

/// The broken lines leave the kept lines only when the save succeeds (in `adopt`). An error at each
/// file-system call of the save: before its commit point the save fails, the lines are still there and the next
/// discard succeeds; after it the save is decided and they are gone. A schema violation in the same shard stays.
#[test]
fn a_failed_repair_save_keeps_the_broken_lines() {
    let schema: &[u8] = br#"{"$k":"device","id":"0e9d8b7a-6c5d-7e4f-8a3b-2c1d0e9f8a71","tag":5}"#;
    let calls = {
        let (vfs, root, ..) = shard_with(&[NOT_JSON, schema]);
        let Opened { mut store, .. } = Store::open(&root, repair_options(&vfs, 1)).expect("opens");
        vfs.reset_calls();
        store.discard_broken_lines().expect("the save succeeds");
        let n = vfs.calls();
        store.close();
        n
    };
    let (mut failed, mut decided) = (false, false);
    for at in 1..=calls {
        let (vfs, root, shard, before, _) = shard_with(&[NOT_JSON, schema]);
        let Opened { mut store, .. } = Store::open(&root, repair_options(&vfs, 1)).expect("opens");
        vfs.reset_calls();
        vfs.set_fault(Fault::Error {
            at,
            kind: std::io::ErrorKind::Other,
        });
        match store.discard_broken_lines() {
            Ok(_) => assert_eq!(store.broken_lines(), [], "at {at}"),
            Err(SaveError::Incomplete { .. }) => {
                decided = true;
                assert_eq!(store.broken_lines(), [], "at {at}: decided");
            }
            Err(e) => {
                failed = true;
                assert_eq!(store.broken_lines().len(), 1, "at {at}: {e}");
                store.discard_broken_lines().expect("the next discard succeeds");
            }
        }
        store.close();
        // The next open completes a decided save.
        let next = reopen(&vfs, 2);
        assert_eq!(next.report.access, Access::Edit, "at {at}");
        next.store.close();
        let written = &project_files(&vfs, &root)[&shard];
        let lines: Vec<&[u8]> = written.split_inclusive(|b| *b == b'\n').collect();
        let mut line = schema.to_vec();
        line.push(b'\n');
        assert_eq!(
            lines.iter().filter(|l| **l == line.as_slice()).count(),
            1,
            "at {at}: the 007 line, verbatim"
        );
        let rest: Vec<u8> = lines
            .into_iter()
            .filter(|l| *l != line.as_slice())
            .flatten()
            .copied()
            .collect();
        assert_eq!(rest, before, "at {at}: the shard without the broken line");
    }
    assert!(
        failed && decided,
        "the errors reach both sides of the commit point"
    );
}

/// A clock whose wait lets a sync client finish: a sleep writes the missing file back.
#[derive(Debug)]
struct SyncedWhileWaiting {
    clock: ew_store::ManualClock,
    vfs: Arc<MemVfs>,
    file: PathBuf,
    bytes: Vec<u8>,
    slept: std::sync::Mutex<Vec<std::time::Duration>>,
}

impl ew_store::Clock for SyncedWhileWaiting {
    fn now(&self) -> std::time::SystemTime {
        self.clock.now()
    }

    fn sleep(&self, d: std::time::Duration) {
        self.slept.lock().unwrap().push(d);
        self.vfs.put(&self.file, &self.bytes);
        self.clock.advance(d);
    }
}

/// On a cloud-synced path, an open that finds a listed file missing (EW-CORE-003) waits five
/// seconds and reads again before it reports: the file the sync client wrote meanwhile makes it an editing open. On
/// another path the same folder is reported at once.
#[test]
fn a_cloud_synced_open_reads_a_missing_file_again_after_five_seconds() {
    let (vfs, root) = mem_project(DirSync::Explicit);
    let files = fixture();
    let shard = files
        .keys()
        .find(|n| n.starts_with("devices/"))
        .expect("the fixture has shards")
        .clone();
    let file = join(&root, &shard);
    ew_store::Vfs::remove_file(&*vfs, &file).expect("the shard goes");
    // Elsewhere the missing shard is reported at once: its records are gone, so the model does not build.
    match Store::open(&root, options(&vfs, 1)) {
        Err(ew_store::OpenError::Model { findings, .. }) => {
            assert!(findings.iter().any(|f| f.kind == FindingKind::MissingFile));
        }
        Ok(_) => panic!("the open should report EW-CORE-003 at once"),
        Err(e) => panic!("EW-CORE-003 at once, not: {e}"),
    }

    let synced = Arc::new(SyncedWhileWaiting {
        clock: ew_store::ManualClock::fixed(),
        vfs: Arc::clone(&vfs),
        file: file.clone(),
        bytes: files[&shard].clone(),
        slept: std::sync::Mutex::new(Vec::new()),
    });
    let mut o = options(&vfs, 1);
    o.clock = Arc::clone(&synced) as Arc<dyn ew_store::Clock>;
    o.cloud_synced = true;
    let opened = Store::open(&root, o).expect("opens");
    assert_eq!(
        *synced.slept.lock().unwrap(),
        vec![std::time::Duration::from_secs(5)]
    );
    assert!(opened.report.findings.is_empty(), "{:?}", opened.report.findings);
    assert_eq!(opened.report.access, Access::Edit);
}

/// FT-1383: one damaged library copy that a device names (a part copy cut short, so the loader drops it)
/// no longer refuses the whole project with EW-INV-001: the project opens read-only, its finding naming the copy's
/// file, and the copy is not in the model. A repair open of it stays read-only too.
#[test]
fn a_damaged_copy_a_device_names_opens_read_only_with_its_finding() {
    let files = fixture();
    let copy = files
        .iter()
        .filter(|(name, _)| name.starts_with("devices/"))
        .find_map(|(_, bytes)| {
            let text = String::from_utf8_lossy(bytes);
            let at = text.find("\"copy\":\"")? + "\"copy\":\"".len();
            Some(text[at..at + 36].to_owned())
        })
        .expect("a device names a part copy");
    let path = format!("lib/parts/{copy}.json");
    assert!(files.contains_key(&path), "{path}");
    let (vfs, root) = mem_project(DirSync::Explicit);
    vfs.put(&join(&root, &path), b"{\"$k\": \"part\",\n");

    let Opened { model, report, store } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::ReadOnly(ReadOnlyReason::Integrity));
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.file.as_ref().is_some_and(|p| p.as_str() == path)),
        "{:?}",
        report.findings
    );
    let id = ew_store::ew_format::ew_schema::AnyId::from_uuid(copy.parse().unwrap());
    let key = (ew_store::ew_format::ew_schema::Kind::Part, id);
    assert!(!model.contains(&key), "the damaged copy is not loaded");
    assert!(
        model.integrity_check().is_err(),
        "a device names the missing copy"
    );
    // Save As of it is refused before anything is written; the copy would not open.
    let target = PathBuf::from("/q");
    let mut opts = options(&vfs, 1);
    opts.state_dir = None;
    assert!(matches!(
        Store::create(&target, &model, opts),
        Err(ew_store::OpenError::Model { .. })
    ));
    assert!(project_files(&vfs, &target).is_empty(), "nothing is written");
    use ew_store::Vfs as _;
    assert!(vfs.stat(&target).is_err(), "not even the folder");
    store.close();
    let opened = Store::open(&root, repair_options(&vfs, 1)).expect("opens");
    assert_eq!(opened.report.access, Access::ReadOnly(ReadOnlyReason::Integrity));
    opened.store.close();
}

/// The damaged definition copy of Golden A's pneumatic cylinders, whose functions end the fluid
/// lines of folio 203, opens read-only with the EW-INV-001 that names the copy; the lines' ports, whose definition is
/// gone, are not judged by EW-INV-024 (FT-1569: a port without its definition is exempt, so the update of another
/// damaged copy is no misfit). The same damage beside a violation unrelated to it, a page whose location is not in the
/// project, is still refused by that violation.
#[test]
fn a_damaged_definition_copy_whose_functions_end_fluid_lines_opens_read_only() {
    use ew_model::{ModelError, REFS_RESOLVE};
    use ew_store::ew_format::ew_schema::{AnyId, Kind};
    let damaged = "lib/fndefs/7b85e9bb-116f-5c3c-a7b6-d5260aa1c219.json";
    let (vfs, root) = mem_project(DirSync::Explicit);
    vfs.put(&join(&root, damaged), b"{\"$k\": \"fndef\",\n");
    let Opened { model, report, store } = Store::open(&root, options(&vfs, 1)).expect("opens");
    assert_eq!(report.access, Access::ReadOnly(ReadOnlyReason::Integrity));
    let rules: BTreeSet<&str> = model
        .integrity_violations()
        .iter()
        .map(|v| match v {
            ModelError::Invariant { id, .. } => *id,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(rules, BTreeSet::from([REFS_RESOLVE]));
    store.close();

    let page = "pages/1d6aab0f-e47b-5da2-9d75-d3eb428e8c48.jsonl";
    let (location, elsewhere) = (
        "101a2f5d-a009-511b-a9a9-8068d1624101",
        "00000000-0000-4000-8000-000000000001",
    );
    let text = String::from_utf8(fixture()[page].clone()).unwrap();
    assert!(text.contains(location), "{page}");
    let (vfs, root) = mem_project(DirSync::Explicit);
    vfs.put(&join(&root, damaged), b"{\"$k\": \"fndef\",\n");
    vfs.put(
        &join(&root, page),
        text.replacen(location, elsewhere, 1).as_bytes(),
    );
    let id = |s: &str| s.parse::<AnyId>().expect("a UUID");
    match Store::open(&root, options(&vfs, 1)) {
        Err(ew_store::OpenError::Model {
            error: ModelError::Invariant { id: rule, keys, .. },
            ..
        }) => {
            assert_eq!(rule, REFS_RESOLVE);
            assert_eq!(
                keys,
                [
                    (Kind::Page, id("1d6aab0f-e47b-5da2-9d75-d3eb428e8c48")),
                    (Kind::StructureNode, id(elsewhere)),
                ]
            );
        }
        Err(other) => panic!("{other:?}"),
        Ok(_) => panic!("a project with an unrelated violation opens"),
    }
}

/// A fresh project of Golden A whose first part copy a device names is cut short: the project and the copy's file.
fn damaged_part_copy() -> (Arc<MemVfs>, PathBuf, String) {
    let copy = fixture()
        .iter()
        .filter(|(name, _)| name.starts_with("devices/"))
        .find_map(|(_, bytes)| {
            let text = String::from_utf8_lossy(bytes);
            let at = text.find("\"copy\":\"")? + "\"copy\":\"".len();
            Some(text[at..at + 36].to_owned())
        })
        .expect("a device names a part copy");
    let path = format!("lib/parts/{copy}.json");
    let (vfs, root) = mem_project(DirSync::Explicit);
    vfs.put(&join(&root, &path), b"{\"$k\": \"part\",\n");
    (vfs, root, path)
}

/// `model` with the part copy of `path` as Golden A holds it (a library's document of it), and its key.
fn with_copy(model: &ew_model::Model, path: &str) -> (ew_model::Model, BTreeSet<ew_model::AnyKey>) {
    use ew_store::ew_format::ew_schema::LibDocument;
    let LibDocument::Part(copy) = serde_json::from_slice(&fixture()[path]).expect("Golden A's copy reads")
    else {
        panic!("{path} holds a part");
    };
    let mut keys = BTreeSet::new();
    let next = apply(model, &[Op::insert(AnyRow::of(copy))], &mut keys).expect("the copy inserts");
    (next, keys)
}

/// FT-1499: Update from library of a damaged copy writes the library's document over the damaged file, with
/// its manifest entry, under the lock it takes and gives back; the store stays read-only, and the project then opens
/// for editing with the copy, every other file as it was.
#[test]
fn update_damaged_copies_writes_the_copy_and_the_project_opens_for_editing() {
    let (vfs, root, path) = damaged_part_copy();
    let Opened { model, store, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let mut store = store;
    assert_eq!(*store.access(), Access::ReadOnly(ReadOnlyReason::Integrity));
    let (next, keys) = with_copy(&model, &path);
    let before = project_files(&vfs, &root);
    let report = store
        .update_damaged_copies(&next, &keys)
        .expect("the copy is written");
    assert_eq!(report.written, vec![FilePath::new(&path).unwrap()]);
    assert_eq!(*store.access(), Access::ReadOnly(ReadOnlyReason::Integrity));
    let after = project_files(&vfs, &root);
    assert_eq!(
        after[&path],
        fixture()[&path],
        "the library's document is written"
    );
    let changed: Vec<&String> = after
        .iter()
        .filter(|(name, bytes)| before.get(*name) != Some(*bytes))
        .map(|(name, _)| name)
        .collect();
    assert_eq!(changed, [&path, &"manifest.json".to_owned()]);
    assert!(
        !vfs.files(&root).contains_key(".~ew.lock"),
        "the lock is given back"
    );
    store.close();
    let again = reopen(&vfs, 1);
    assert_eq!(again.report.access, Access::Edit);
    assert!(again.model.integrity_check().is_ok());
    again.store.close();
}

/// FT-1499: another file that does not hold the bytes its manifest lists (here a page file edited outside,
/// EW-CORE-002) refuses the update before anything is written, as would another session's lock: the update would hide
/// that finding.
#[test]
fn update_damaged_copies_is_refused_beside_another_finding_and_under_another_lock() {
    let (vfs, root, path) = damaged_part_copy();
    let page = "pages/1d6aab0f-e47b-5da2-9d75-d3eb428e8c48.jsonl";
    let mut edited = fixture()[page].clone();
    edited.extend_from_slice(b"\n");
    vfs.put(&join(&root, page), &edited);
    let Opened { model, store, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let mut store = store;
    let (next, keys) = with_copy(&model, &path);
    let before = project_files(&vfs, &root);
    assert!(matches!(
        store.update_damaged_copies(&next, &keys),
        Err(SaveError::ReadOnly(ReadOnlyReason::Integrity))
    ));
    assert!(project_files(&vfs, &root) == before, "nothing is written");
    store.close();

    let (vfs, root, path) = damaged_part_copy();
    let Opened { model, store, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let mut store = store;
    let (next, keys) = with_copy(&model, &path);
    // Another session takes the lock after the read-only open gave it back.
    let other = options(&vfs, 2);
    assert!(matches!(
        ew_store::lock::acquire(
            &*other.vfs,
            &root,
            &*other.clock,
            ew_store::STALE_AFTER,
            &other.identity,
            &*other.probe
        ),
        Ok(ew_store::lock::Acquire::Held)
    ));
    let before = project_files(&vfs, &root);
    assert!(matches!(
        store.update_damaged_copies(&next, &keys),
        Err(SaveError::ReadOnly(ReadOnlyReason::LockedBy(_)))
    ));
    assert!(project_files(&vfs, &root) == before, "nothing is written");
    store.close();
}

/// The copies an update from library leaves damaged, one cut short (kept as found) and one whose
/// file is gone, neither refuse the update of the copy a library holds nor lose their manifest entries: the reopen finds
/// them damaged as before, the cut-short one changed outside ElektraWatt (EW-CORE-002) and the other missing.
#[test]
fn update_damaged_copies_keeps_the_manifest_entries_of_the_copies_it_leaves_damaged() {
    use ew_store::Vfs;
    let (vfs, root, path) = damaged_part_copy();
    // Golden A's second part copy a device names, whose file is gone, and a manufacturer copy no record names, cut short.
    let gone = fixture()
        .iter()
        .filter(|(name, _)| name.starts_with("devices/"))
        .flat_map(|(_, bytes)| {
            let text = String::from_utf8_lossy(bytes).into_owned();
            text.match_indices("\"copy\":\"")
                .map(|(at, m)| text[at + m.len()..at + m.len() + 36].to_owned())
                .collect::<Vec<_>>()
        })
        .map(|copy| format!("lib/parts/{copy}.json"))
        .find(|p| *p != path && fixture().contains_key(p))
        .expect("a second part copy");
    let cut = "lib/manufacturers/221fd06e-3263-5cab-bbb9-27aee71c8d79.json";
    vfs.remove_file(&join(&root, &gone)).expect("removed");
    vfs.put(&join(&root, cut), b"{\"$k\": \"manufacturer\",\n");
    let entries = |files: &std::collections::BTreeMap<String, Vec<u8>>| {
        let manifest: serde_json::Value =
            serde_json::from_slice(&files["manifest.json"]).expect("a manifest");
        (manifest["files"][&gone].clone(), manifest["files"][cut].clone())
    };
    let Opened { model, store, .. } = Store::open(&root, options(&vfs, 1)).expect("opens");
    let mut store = store;
    assert_eq!(*store.access(), Access::ReadOnly(ReadOnlyReason::Integrity));
    let (next, keys) = with_copy(&model, &path);
    let before = project_files(&vfs, &root);
    let report = store
        .update_damaged_copies(&next, &keys)
        .expect("the copy a library holds is written");
    assert_eq!(report.written, vec![FilePath::new(&path).unwrap()]);
    let after = project_files(&vfs, &root);
    assert_eq!(after[&path], fixture()[&path]);
    assert_eq!(
        entries(&after),
        entries(&before),
        "the entries of the copies left damaged"
    );
    assert!(!entries(&before).0.is_null() && !entries(&before).1.is_null());
    store.close();
    let again = reopen(&vfs, 1);
    assert_eq!(again.report.access, Access::ReadOnly(ReadOnlyReason::Integrity));
    let found: Vec<(FindingKind, String)> = again
        .report
        .findings
        .iter()
        .filter_map(|f| Some((f.kind, f.file.as_ref()?.to_string())))
        .filter(|(_, file)| *file == gone || file == cut)
        .collect();
    assert!(
        found.contains(&(FindingKind::MissingFile, gone.clone())),
        "{found:?}"
    );
    assert!(
        found.contains(&(FindingKind::ForeignEdit, cut.to_owned())),
        "{found:?}"
    );
    again.store.close();
}
