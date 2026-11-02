// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew fmt`'s library half (`ew_api::fmt`, WP-S1.22 D9): it rewrites only the files not in canonical form, through the
//! save protocol, all or nothing under a process kill or a power loss at every file-system call, and it refuses a
//! rewrite that would change a line's content.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use ew_api::fmt::{self, Backup, DiscardedLine, FmtError, RepairError, Repaired, Rewrite};
use ew_api::{AnyCommand, Recovery, Session, SessionOptions};
use ew_store::ew_format::{hash_text, sha256};
use ew_store::ewz;
use ew_store::lock::Identity;
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::{Access, ManualClock, ProcessProbe, ProcessState, ReadOnlyReason, Store, StoreOptions, Vfs};
use ew_testids::seeded_ids;

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

fn root() -> PathBuf {
    PathBuf::from("/p")
}

/// Store options on `vfs` for process `pid`; every other process is dead.
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
    o
}

fn command(name: &str, params: serde_json::Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// The project's files without the lock.
fn files(vfs: &MemVfs) -> BTreeMap<String, Vec<u8>> {
    vfs.files(&root())
        .into_iter()
        .filter(|(name, _)| !name.starts_with(".~ew.lock"))
        .collect()
}

/// A small project's canonical files: sections, pages and devices.
fn canonical_files() -> BTreeMap<String, Vec<u8>> {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let project = serde_json::from_str(PROJECT).unwrap();
    let mut s = Session::from_records(
        project,
        [],
        [],
        SessionOptions {
            ids: Some(seeded_ids(41)),
            ..SessionOptions::default()
        },
    )
    .unwrap();
    s.execute(&command(
        "section.create",
        serde_json::json!({"number": 1, "folio_range": [1, 99]}),
    ))
    .unwrap();
    s.execute(&command(
        "section.create",
        serde_json::json!({"number": 2, "folio_range": [100, 199]}),
    ))
    .unwrap();
    for n in [1, 2, 101] {
        s.execute(&command("page.create", serde_json::json!({ "number": n })))
            .unwrap();
    }
    for counter in 1..=4 {
        s.execute(&command(
            "device.create",
            serde_json::json!({"letter": "K", "counter": counter}),
        ))
        .unwrap();
    }
    s.create(&root(), options(&vfs, 1)).unwrap();
    s.close();
    files(&vfs)
}

/// The canonical files, three of them out of canonical form: the first two lines of the record file with the most
/// lines swapped, CR line ends in another file and a byte order mark before a third. Returns the files on disk and
/// the canonical bytes of the three.
fn disordered() -> (BTreeMap<String, Vec<u8>>, BTreeMap<String, Vec<u8>>) {
    let canonical = canonical_files();
    let mut records: Vec<(&String, &Vec<u8>)> =
        canonical.iter().filter(|(n, _)| n.ends_with(".jsonl")).collect();
    records.sort_by_key(|(name, bytes)| {
        (
            std::cmp::Reverse(bytes.iter().filter(|b| **b == b'\n').count()),
            *name,
        )
    });
    assert!(records.len() >= 3, "{:?}", canonical.keys());
    let mut disk = canonical.clone();
    let mut changed = BTreeMap::new();
    let (swap, bytes) = records[0];
    let text = String::from_utf8(bytes.clone()).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    assert!(lines.len() >= 2, "{swap} has two lines to swap");
    lines.swap(0, 1);
    disk.insert(swap.clone(), format!("{}\n", lines.join("\n")).into_bytes());
    let (crlf, bytes) = records[1];
    disk.insert(
        crlf.clone(),
        String::from_utf8(bytes.clone())
            .unwrap()
            .replace('\n', "\r\n")
            .into_bytes(),
    );
    let (bom, bytes) = records[2];
    disk.insert(bom.clone(), [&[0xEF, 0xBB, 0xBF][..], bytes].concat());
    for name in [swap, crlf, bom] {
        changed.insert(name.clone(), canonical[name].clone());
    }
    (disk, changed)
}

/// A [`MemVfs`] holding `files` under `/p`, durable, and an empty state directory.
fn mem(files: &BTreeMap<String, Vec<u8>>) -> Arc<MemVfs> {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    for (name, bytes) in files {
        vfs.put(&ew_store::join(&root(), name), bytes);
    }
    vfs.put_dir(Path::new("/state"));
    vfs
}

fn open(vfs: &Arc<MemVfs>, pid: u32) -> Session {
    let (s, report) = Session::open(
        &root(),
        options(vfs, pid),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert_eq!(report.access, Access::Edit);
    s
}

/// Ruling 6 of cycle 41: "`ew fmt` writes only the rewrite set: save through the plan that starts from `Loaded.rewrite`,
/// never `Changes::All` ..., so a file already canonical is not rewritten."
#[test]
fn a_format_rewrites_only_the_files_not_in_canonical_form() {
    let (disk, changed) = disordered();
    let vfs = mem(&disk);
    let mut s = open(&vfs, 2);
    let plan = fmt::plan(&s).unwrap();
    let expected: Vec<Rewrite> = changed
        .keys()
        .map(|file| Rewrite {
            file: file.clone(),
            removed: false,
        })
        .collect();
    assert_eq!(plan.files(), expected);
    vfs.set_trace(true);
    let done = fmt::format(&mut s).unwrap();
    assert_eq!(done.generation, 2);
    assert_eq!(done.files, expected);
    // No other project file was touched (only reads and stats name them).
    for line in vfs.trace() {
        let line = line.replace('\\', "/");
        let op = line.split_whitespace().nth(1).unwrap_or_default();
        if matches!(op, "read" | "stat" | "list" | "canonicalize") {
            continue;
        }
        for name in disk
            .keys()
            .filter(|n| !changed.contains_key(*n) && *n != "manifest.json")
        {
            assert!(!line.contains(&format!("/p/{name}")), "{line}");
        }
    }
    s.close();
    let after = files(&vfs);
    for (name, bytes) in &disk {
        let want = changed.get(name).unwrap_or(bytes);
        if name != "manifest.json" {
            assert_eq!(&after[name], want, "{name}");
        }
    }
    // A second format has nothing to do and writes nothing.
    let mut s = open(&vfs, 3);
    assert!(fmt::plan(&s).unwrap().is_empty());
    let again = fmt::format(&mut s).unwrap();
    assert_eq!(again.generation, 2);
    assert!(again.files.is_empty());
    s.close();
    assert_eq!(files(&vfs), after);
}

/// What a project holds after a crash in a format and the next editing open: the generation and whether each changed
/// file holds its old bytes (`Some(false)`), its canonical bytes (`Some(true)`) or other bytes (`None`, torn).
fn after_crash(
    vfs: &Arc<MemVfs>,
    disk: &BTreeMap<String, Vec<u8>>,
    changed: &BTreeMap<String, Vec<u8>>,
) -> Result<(u64, bool), String> {
    let now = files(vfs);
    let mut states = Vec::new();
    for (name, canonical) in changed {
        match now.get(name) {
            Some(b) if b == canonical => states.push(true),
            Some(b) if b == &disk[name] => states.push(false),
            other => return Err(format!("{name} is torn: {:?}", other.map(Vec::len))),
        }
    }
    let opened = match Store::open(&root(), options(vfs, 3)) {
        Ok(o) => o,
        Err(e) => return Err(format!("the project does not open: {e}")),
    };
    let opened = match &opened.report.access {
        Access::Edit => opened,
        Access::ReadOnly(ReadOnlyReason::LockedBy(info)) if info.stale => {
            drop(opened);
            Store::take_over(&root(), options(vfs, 3)).map_err(|e| format!("take-over failed: {e}"))?
        }
        other => return Err(format!("opens {other:?}")),
    };
    let generation = opened.report.generation;
    opened.store.close();
    let now = files(vfs);
    let all_new = changed.iter().all(|(name, canonical)| &now[name] == canonical);
    let all_old = changed.keys().all(|name| now[name] == disk[name]);
    match generation {
        1 if all_old => Ok((1, states.iter().any(|s| *s))),
        2 if all_new => Ok((2, true)),
        g => Err(format!(
            "generation {g} with {} of {} files canonical",
            changed.keys().filter(|n| now[*n] == changed[*n]).count(),
            changed.len()
        )),
    }
}

/// Ruling 1 of cycle 41: "`ew fmt` writes all-or-nothing through the store's existing atomic write path; a crash between
/// files leaves either the old or the new bytes of each file, never a torn one (test it with the store's fault hook
/// ...)". A process kill and a power loss (keeping 0, 1 or all pending name changes) at every file-system call of the
/// format: each file holds its old or its new bytes, and after the next editing open the project is wholly generation 1
/// (old) or wholly generation 2 (canonical).
#[test]
fn a_crash_at_every_call_of_a_format_leaves_the_old_or_the_new_generation() {
    let (disk, changed) = disordered();
    let calls = {
        let vfs = mem(&disk);
        let mut s = open(&vfs, 2);
        vfs.reset_calls();
        fmt::format(&mut s).unwrap();
        let n = vfs.calls();
        s.close();
        n
    };
    assert!(calls > 10, "{calls} calls");
    let mut failures = Vec::new();
    let mut seen = [false; 3];
    let faults = (1..=calls + 1).flat_map(|at| {
        [
            Fault::Kill { at },
            Fault::PowerLoss { at, keep: 0 },
            Fault::PowerLoss { at, keep: 1 },
            Fault::PowerLoss { at, keep: usize::MAX },
        ]
    });
    for fault in faults {
        let vfs = mem(&disk);
        let mut s = open(&vfs, 2);
        vfs.reset_calls();
        vfs.set_fault(fault);
        let result = fmt::format(&mut s);
        // The process dies here: no clean close.
        let _ = vfs.stat(&root());
        drop(s);
        vfs.restart();
        match after_crash(&vfs, &disk, &changed) {
            Ok((1, mixed)) => {
                seen[usize::from(mixed)] = true;
                if result.is_ok() && !mixed {
                    failures.push(format!(
                        "{fault:?}: the format said done and generation 1 is open"
                    ));
                }
            }
            Ok(_) => seen[2] = true,
            Err(e) => failures.push(format!("{fault:?}: {e}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(seen[0] && seen[2], "the sweep reaches both generations: {seen:?}");
}

/// D9: "It never changes a record's content, only order and form; a file whose records do not survive the round trip
/// byte-for-byte in content is refused". An explicit `"$v":1` is dropped by the canonical form (version 1 is the
/// default): the member is content here, so the format is refused and writes nothing.
#[test]
fn a_format_that_would_change_content_is_refused_and_writes_nothing() {
    let canonical = canonical_files();
    let (name, bytes) = canonical
        .iter()
        .find(|(n, _)| n.starts_with("devices/"))
        .map(|(n, b)| (n.clone(), b.clone()))
        .expect("a device file");
    let text = String::from_utf8(bytes).unwrap();
    let first = text.lines().next().unwrap();
    let versioned = first.replacen("{", "{\"$v\":1,", 1);
    let mut disk = canonical.clone();
    disk.insert(name.clone(), text.replacen(first, &versioned, 1).into_bytes());
    let vfs = mem(&disk);
    let mut s = open(&vfs, 2);
    assert_eq!(
        fmt::plan(&s).unwrap().files(),
        vec![Rewrite {
            file: name.clone(),
            removed: false
        }]
    );
    match fmt::format(&mut s) {
        Err(e @ FmtError::Content { .. }) => assert_eq!(
            e.to_string(),
            format!(
                "{name} line 1: the canonical form would change the content of this line, not only its form"
            )
        ),
        other => panic!("expected the content refusal, got {other:?}"),
    }
    s.close();
    assert_eq!(files(&vfs), disk, "nothing was written");
}

// ---------------------------------------------------------------------------------------------------------------------
// `ew fmt --repair` (WP-S1.17, decision log ID-1185)

/// A line that is not one JSON value: EW-CORE-005 at line level.
const BROKEN: &[u8] = br#"{"$k":"page","id":"#;

/// A page line whose number is not a number: EW-CORE-007, a schema violation, which a repair keeps.
const SCHEMA: &[u8] =
    br#"{"$k":"page","id":"0f0e0d0c-0b0a-7908-8706-050403020100","number":{"not":"a number"}}"#;

/// The backup's path for the project `/p` at generation 1.
fn backup() -> PathBuf {
    PathBuf::from("/p.before-repair-g1.ewz")
}

/// The files beside the project folder that a repair writes: the backup and its temporary name.
fn beside(vfs: &MemVfs) -> Vec<String> {
    vfs.files(Path::new("/"))
        .into_keys()
        .filter(|name| name.starts_with("p.before-repair"))
        .collect()
}

/// The canonical files with `lines` after the first line of a page file, and the manifest's hash of that file as a save
/// of those bytes writes it (`hashed`), or the hash of the canonical file (an edit outside ElektraWatt). Returns the
/// files, the page file's name and its canonical bytes.
fn page_with(lines: &[&[u8]], hashed: bool) -> (BTreeMap<String, Vec<u8>>, String, Vec<u8>) {
    let mut disk = canonical_files();
    let page = disk
        .keys()
        .find(|n| n.starts_with("pages/"))
        .cloned()
        .expect("a page file");
    let canonical = disk[&page].clone();
    let first = canonical.iter().position(|b| *b == b'\n').expect("a first line") + 1;
    let mut bytes = canonical[..first].to_vec();
    for line in lines {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    bytes.extend_from_slice(&canonical[first..]);
    disk.insert(page.clone(), bytes);
    if hashed {
        rehash(&mut disk, &page, &canonical);
    }
    (disk, page, canonical)
}

/// The manifest's hash of `name` in `disk` changed from that of `before` to that of its bytes in `disk`, as a save of
/// those bytes writes it.
fn rehash(disk: &mut BTreeMap<String, Vec<u8>>, name: &str, before: &[u8]) {
    let (old, new) = (hash_text(&sha256(before)), hash_text(&sha256(&disk[name])));
    let manifest = String::from_utf8(disk["manifest.json"].clone()).unwrap();
    assert_eq!(
        manifest.matches(&old).count(),
        1,
        "the manifest lists {name} once"
    );
    disk.insert(
        "manifest.json".to_owned(),
        manifest.replace(&old, &new).into_bytes(),
    );
}

/// Store options for a repair open ([`StoreOptions::repair`]).
fn repair_options(vfs: &Arc<MemVfs>, pid: u32) -> StoreOptions {
    let mut o = options(vfs, pid);
    o.repair = true;
    o
}

fn open_repair(vfs: &Arc<MemVfs>, pid: u32) -> (Session, ew_api::OpenReport) {
    Session::open(
        &root(),
        repair_options(vfs, pid),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap()
}

/// EZ-5: `ew fmt --repair` on a page file with one broken line writes the backup `.ewz` (generation 1 as it was on
/// disk, the broken line included: the pack's read-back takes it), then a project without the line, which opens
/// editable. An editing open without the repair flag stays read-only (ID-1185 ruling 4).
#[test]
fn a_repair_discards_the_broken_line_after_a_backup() {
    let (disk, page, canonical) = page_with(&[BROKEN], true);
    let vfs = mem(&disk);
    let (s, report) = Session::open(
        &root(),
        options(&vfs, 2),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert_eq!(report.access, Access::ReadOnly(ReadOnlyReason::Integrity));
    s.close();

    let (mut s, report) = open_repair(&vfs, 2);
    assert_eq!(report.access, Access::Edit);
    assert_eq!(fmt::kept_after_repair(&report), []);
    let done = fmt::repair(&mut s).unwrap();
    s.close();
    assert_eq!(
        done,
        Repaired {
            generation: 2,
            discarded: vec![DiscardedLine {
                file: page.clone(),
                line: 2,
                text: BROKEN.to_vec(),
            }],
            files: vec![Rewrite {
                file: page.clone(),
                removed: false,
            }],
            backup: Some(Backup {
                path: backup(),
                generation: 1,
                reused: false,
            }),
        }
    );
    let after = files(&vfs);
    assert_eq!(after[&page], canonical, "the page file without the line");
    for (name, bytes) in disk.iter().filter(|(n, _)| **n != page && *n != "manifest.json") {
        assert_eq!(&after[name], bytes, "{name} is not written");
    }
    let packed = ewz::read(&vfs.read(&backup()).unwrap()).unwrap();
    assert_eq!(packed.generation(), 1);
    assert_eq!(
        packed.entries, disk,
        "the backup holds generation 1 as it was on disk"
    );
    assert_eq!(
        beside(&vfs),
        ["p.before-repair-g1.ewz"],
        "no temporary file is left"
    );
    let s = open(&vfs, 3);
    s.close();
}

/// ID-1185 ruling 3: without a broken line a repair writes nothing and makes no backup, though files are out of
/// canonical form (`ew fmt`'s work, not the repair's; the manifest holds their hashes, as a save of their bytes
/// writes them).
#[test]
fn without_a_broken_line_a_repair_writes_nothing() {
    let (mut disk, changed) = disordered();
    for (name, canonical) in &changed {
        rehash(&mut disk, name, canonical);
    }
    let vfs = mem(&disk);
    let (mut s, report) = open_repair(&vfs, 2);
    assert_eq!(report.access, Access::Edit);
    let done = fmt::repair(&mut s).unwrap();
    s.close();
    assert_eq!(
        done,
        Repaired {
            generation: 1,
            discarded: Vec::new(),
            files: Vec::new(),
            backup: None,
        }
    );
    assert_eq!(files(&vfs), disk);
    assert_eq!(beside(&vfs), Vec::<String>::new());
}

/// ID-1185 rulings 1 and 8: a schema violation (EW-CORE-007) beside the broken line is reported as kept and written
/// back byte for byte; only the broken line goes.
#[test]
fn a_repair_keeps_a_schema_violation_byte_for_byte() {
    let (disk, page, canonical) = page_with(&[BROKEN, SCHEMA], true);
    let vfs = mem(&disk);
    let (mut s, report) = open_repair(&vfs, 2);
    assert_eq!(report.access, Access::Edit);
    let kept: Vec<(String, Option<u32>, String)> = fmt::kept_after_repair(&report)
        .into_iter()
        .map(|k| (k.file, k.line, k.code))
        .collect();
    assert_eq!(kept, [(page.clone(), Some(3), "EW-CORE-007".to_owned())]);
    let done = fmt::repair(&mut s).unwrap();
    s.close();
    let discarded: Vec<(String, u32)> = done.discarded.iter().map(|d| (d.file.clone(), d.line)).collect();
    assert_eq!(discarded, [(page.clone(), 2)]);
    let after = files(&vfs);
    let lines: Vec<&[u8]> = after[&page].split(|b| *b == b'\n').collect();
    assert_eq!(
        lines.iter().filter(|l| **l == SCHEMA).count(),
        1,
        "the 007 line, verbatim"
    );
    assert!(!lines.contains(&BROKEN));
    let rest: Vec<u8> = after[&page]
        .split_inclusive(|b| *b == b'\n')
        .filter(|l| l.strip_suffix(b"\n") != Some(SCHEMA))
        .flatten()
        .copied()
        .collect();
    assert_eq!(rest, canonical, "every other line as the canonical file holds it");
    let (s, report) = Session::open(
        &root(),
        options(&vfs, 3),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .unwrap();
    assert_eq!(report.access, Access::Edit);
    assert!(report.findings.iter().any(|f| f.code() == Some("EW-CORE-007")));
    assert!(!report.findings.iter().any(|f| f.code() == Some("EW-CORE-005")));
    s.close();
}

/// ID-1185 ruling 3: a store that `pack_store` refuses refuses the repair before anything is written: here a page file
/// edited outside ElektraWatt, whose bytes no longer match the manifest's hash.
#[test]
fn a_repair_that_cannot_pack_the_backup_writes_nothing() {
    let (disk, page, _) = page_with(&[BROKEN], false);
    let vfs = mem(&disk);
    let (mut s, report) = open_repair(&vfs, 2);
    assert_eq!(report.access, Access::Edit);
    match fmt::repair(&mut s) {
        Err(e @ RepairError::Pack(_)) => assert_eq!(
            e.to_string(),
            format!(
                "the backup cannot be packed: {page} does not match the manifest (changed outside ElektraWatt since \
                 the last save)"
            )
        ),
        other => panic!("expected the pack's refusal, got {other:?}"),
    }
    s.close();
    assert_eq!(files(&vfs), disk);
    assert_eq!(beside(&vfs), Vec::<String>::new());
}

/// ID-1185 rulings 5 and 8: a file at the backup's name that does not hold this generation's pack refuses the repair
/// and stays; one that holds it byte for byte is reused.
#[test]
fn a_backup_name_holding_another_file_refuses_and_this_generations_pack_is_reused() {
    let (disk, page, canonical) = page_with(&[BROKEN], true);
    let vfs = mem(&disk);
    vfs.put(&backup(), b"another file");
    let (mut s, _) = open_repair(&vfs, 2);
    match fmt::repair(&mut s) {
        Err(e @ RepairError::BackupTaken(_)) => assert_eq!(
            e.to_string(),
            "/p.before-repair-g1.ewz already exists and does not hold this generation's backup"
        ),
        other => panic!("expected the taken name, got {other:?}"),
    }
    assert_eq!(files(&vfs), disk);
    assert_eq!(vfs.read(&backup()).unwrap(), b"another file");

    let packed = ewz::pack_store(s.store().unwrap()).unwrap();
    vfs.put(&backup(), &packed.bytes);
    let done = fmt::repair(&mut s).unwrap();
    s.close();
    assert_eq!(
        done.backup,
        Some(Backup {
            path: backup(),
            generation: 1,
            reused: true,
        })
    );
    assert_eq!(files(&vfs)[&page], canonical);
    assert_eq!(vfs.read(&backup()).unwrap(), packed.bytes);
}

/// ID-1185 ruling 5: a parent folder that cannot be written refuses with nothing changed; the rerun succeeds.
#[test]
fn a_folder_that_cannot_be_written_refuses_the_backup() {
    let (disk, page, canonical) = page_with(&[BROKEN], true);
    let vfs = mem(&disk);
    let (mut s, _) = open_repair(&vfs, 2);
    vfs.set_read_only(true);
    match fmt::repair(&mut s) {
        Err(e @ RepairError::Backup { .. }) => assert_eq!(
            e.to_string(),
            "the backup cannot be written (/p.before-repair-g1.ewz: read-only file system)"
        ),
        other => panic!("expected the backup's failure, got {other:?}"),
    }
    vfs.set_read_only(false);
    assert_eq!(files(&vfs), disk);
    assert_eq!(beside(&vfs), Vec::<String>::new());
    let done = fmt::repair(&mut s).unwrap();
    s.close();
    assert_eq!(done.generation, 2);
    assert_eq!(files(&vfs)[&page], canonical);
}

/// The generation a project holds after a fault in a repair, once an editing open (taking a crashed session's lock
/// over) has completed a decided save, and whether the backup is there (its bytes `backup_bytes`, any other bytes an
/// error). Every file but the page file and the manifest is as it was; the page file holds its broken bytes at
/// generation 1 and its canonical bytes at generation 2, and generation 2 never comes without the backup.
fn after_repair_fault(
    vfs: &Arc<MemVfs>,
    disk: &BTreeMap<String, Vec<u8>>,
    page: &str,
    canonical: &[u8],
    backup_bytes: &[u8],
) -> Result<(u64, bool), String> {
    let take_over =
        || Store::take_over(&root(), repair_options(vfs, 3)).map_err(|e| format!("take-over failed: {e}"));
    let opened = match Store::open(&root(), repair_options(vfs, 3)) {
        Ok(o) => o,
        Err(ew_store::OpenError::SaveInProgress) => take_over()?,
        Err(e) => return Err(format!("the project does not open: {e}")),
    };
    let opened = match &opened.report.access {
        Access::Edit => opened,
        Access::ReadOnly(ReadOnlyReason::LockedBy(info)) if info.stale => {
            drop(opened);
            take_over()?
        }
        other => return Err(format!("opens {other:?}")),
    };
    let generation = opened.report.generation;
    opened.store.close();
    let now = files(vfs);
    for (name, bytes) in disk.iter().filter(|(n, _)| *n != page && *n != "manifest.json") {
        if now.get(name) != Some(bytes) {
            return Err(format!("{name} changed"));
        }
    }
    let backed_up = match vfs.read(&backup()) {
        Ok(b) if b == backup_bytes => true,
        Ok(b) => return Err(format!("the backup holds {} other bytes", b.len())),
        Err(_) => false,
    };
    match (generation, now.get(page).map(Vec::as_slice)) {
        (1, Some(b)) if b == disk[page].as_slice() => Ok((1, backed_up)),
        (2, Some(b)) if b == canonical && backed_up => Ok((2, true)),
        (2, Some(b)) if b == canonical => Err("generation 2 without the backup".to_owned()),
        (g, b) => Err(format!(
            "generation {g} with the page file of {:?} bytes",
            b.map(<[u8]>::len)
        )),
    }
}

/// ID-1185 ruling 8: a fault at every file-system call of a repair: an error of that one call (the process lives on), a
/// process kill and a power loss keeping 0, 1 or all pending name changes. During the backup an error leaves no backup
/// and nothing else; between the backup and the save, and inside the save before its commit point, the project is
/// unchanged; after the commit point the next open completes generation 2, which never comes without the backup. A
/// failed save keeps the broken lines, so the same session repairs again; and a rerun after every fault succeeds,
/// reusing the backup it finds.
#[test]
fn a_fault_at_every_call_of_a_repair_leaves_the_old_or_the_new_generation_and_a_rerun_succeeds() {
    let (disk, page, canonical) = page_with(&[BROKEN], true);
    let (calls, backup_bytes) = {
        let vfs = mem(&disk);
        let (mut s, _) = open_repair(&vfs, 2);
        vfs.reset_calls();
        fmt::repair(&mut s).unwrap();
        let n = vfs.calls();
        s.close();
        (n, vfs.read(&backup()).unwrap())
    };
    assert!(calls > 10, "{calls} calls");
    let mut failures = Vec::new();
    let mut seen = [false; 5];
    let faults = (1..=calls + 1).flat_map(|at| {
        [
            Fault::Error {
                at,
                kind: std::io::ErrorKind::Other,
            },
            Fault::Kill { at },
            Fault::PowerLoss { at, keep: 0 },
            Fault::PowerLoss { at, keep: 1 },
            Fault::PowerLoss { at, keep: usize::MAX },
        ]
    });
    for fault in faults {
        let vfs = mem(&disk);
        let (mut s, _) = open_repair(&vfs, 2);
        vfs.reset_calls();
        vfs.set_fault(fault);
        let result = fmt::repair(&mut s);
        if matches!(fault, Fault::Error { .. }) {
            match &result {
                Err(RepairError::Backup { .. } | RepairError::Pack(_) | RepairError::Format(_)) => {
                    seen[0] |= matches!(result, Err(RepairError::Backup { .. }));
                    if files(&vfs) != disk || !beside(&vfs).is_empty() {
                        failures.push(format!("{fault:?}: {result:?} left {:?}", beside(&vfs)));
                    }
                }
                Err(RepairError::Save {
                    error: ew_api::SaveError::Incomplete { .. },
                    ..
                }) => seen[1] = true,
                Err(RepairError::Save { .. }) => {
                    seen[2] = true;
                    // Nothing was lost: the same session repairs again, reusing the backup.
                    match fmt::repair(&mut s) {
                        Ok(again) if again.backup.as_ref().is_some_and(|b| b.reused) => {}
                        other => failures.push(format!("{fault:?}: the same session's rerun gave {other:?}")),
                    }
                }
                Ok(_) => {}
                Err(e) => failures.push(format!("{fault:?}: {e:?}")),
            }
            s.close();
        } else {
            // The process dies here: no clean close.
            let _ = vfs.stat(&root());
            drop(s);
            vfs.restart();
        }
        let had_backup = match after_repair_fault(&vfs, &disk, &page, &canonical, &backup_bytes) {
            Ok((1, backed_up)) => {
                seen[3] = true;
                if result.is_ok() && matches!(fault, Fault::Error { .. }) {
                    failures.push(format!(
                        "{fault:?}: the repair said done and generation 1 is open"
                    ));
                }
                backed_up
            }
            Ok(_) => {
                seen[4] = true;
                true
            }
            Err(e) => {
                failures.push(format!("{fault:?}: {e}"));
                continue;
            }
        };
        // The rerun, in a new session: a repair of generation 1 (reusing the backup it finds), or nothing to repair.
        let (mut s, _) = open_repair(&vfs, 4);
        match fmt::repair(&mut s) {
            Ok(done) => {
                if done.generation != 2 {
                    failures.push(format!(
                        "{fault:?}: the rerun left generation {}",
                        done.generation
                    ));
                }
                if let Some(b) = &done.backup
                    && b.reused != had_backup
                {
                    failures.push(format!(
                        "{fault:?}: the rerun's backup {b:?}, one was there: {had_backup}"
                    ));
                }
            }
            Err(e) => failures.push(format!("{fault:?}: the rerun failed: {e}")),
        }
        s.close();
        if files(&vfs).get(&page).map(Vec::as_slice) != Some(canonical.as_slice())
            || vfs.read(&backup()).ok().as_deref() != Some(backup_bytes.as_slice())
            || beside(&vfs) != ["p.before-repair-g1.ewz"]
        {
            failures.push(format!("{fault:?}: after the rerun: {:?}", beside(&vfs)));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert_eq!(
        seen, [true; 5],
        "the sweep reaches a failed backup, both sides of the save's commit point and both generations"
    );
}
