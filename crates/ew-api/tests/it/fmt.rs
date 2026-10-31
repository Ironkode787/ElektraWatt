// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew fmt`'s library half (`ew_api::fmt`, WP-S1.22 D9): it rewrites only the files not in canonical form, through the
//! save protocol, all or nothing under a process kill or a power loss at every file-system call, and it refuses a
//! rewrite that would change a line's content.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use ew_api::fmt::{self, FmtError, Rewrite};
use ew_api::{AnyCommand, Recovery, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_store::lock::Identity;
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::{Access, ManualClock, ProcessProbe, ProcessState, ReadOnlyReason, Store, StoreOptions, Vfs};

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
