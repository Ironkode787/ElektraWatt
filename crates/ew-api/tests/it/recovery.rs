// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The app's recovery path (`Session::open` with `Recovery::Apply`, doc 05 §8.2) with more than one crashed session's
//! journal, a refused replay, the confirmed discard, and a reader that starts while a save is in progress (WP-S1.22
//! part 3, rulings 2, 4 and 5 of cycle 41).

// The tests count pages through the untracked readers (dev-dependency `ew-model/raw-read`).
#![allow(clippy::disallowed_methods)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ew_api::{AnyCommand, RecoverError, Recovery, ReplayError, SaveError, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_store::lock::Identity;
use ew_store::mem::{DirSync, Fault, MemVfs};
use ew_store::{Clock, ManualClock, OpenMode, ProcessProbe, ProcessState, StoreOptions, Vfs};

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;
/// The fixed test time ([`ManualClock::fixed`]).
const T0: u64 = 1_792_222_930;

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

/// Store options on `vfs` for process `pid` with a clock `secs` after the fixed test time; every other process is dead.
fn options_at(vfs: &Arc<MemVfs>, pid: u32, secs: u64) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.vfs = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.clock = Arc::new(ManualClock::new(UNIX_EPOCH + Duration::from_secs(T0 + secs)));
    o.probe = Arc::new(Alive(pid));
    o.identity = Identity {
        session: ew_store::lock::new_session_id(),
        user: "<os user>".into(),
        host: "<host>".into(),
        pid,
        pid_started: UNIX_EPOCH + Duration::from_secs(T0 - 12),
        app_version: "ElektraWatt test".into(),
    };
    o.state_dir = Some(PathBuf::from("/state"));
    o
}

fn session_options(seed: u64) -> SessionOptions {
    SessionOptions {
        ids: Some(seeded_ids(seed)),
        ..SessionOptions::default()
    }
}

fn page(number: u32) -> AnyCommand {
    AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({ "number": number }),
    }
}

/// The folio numbers of the session's pages, in order.
fn folios(s: &Session) -> Vec<u32> {
    let mut out: Vec<u32> = s
        .world()
        .model
        .table::<ew_schema::Page>()
        .iter()
        .map(|(_, p)| p.folio.number)
        .collect();
    out.sort_unstable();
    out
}

/// The journal segments in the state directory, by file name.
fn journals(vfs: &MemVfs) -> BTreeMap<String, Vec<u8>> {
    vfs.files(Path::new("/state"))
        .into_iter()
        .filter(|(name, _)| name.ends_with(".jsonl"))
        .collect()
}

/// A project with pages 1 and 2 in a fresh MemVfs.
fn project() -> Arc<MemVfs> {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let mut s =
        Session::from_records(serde_json::from_str(PROJECT).unwrap(), [], [], session_options(1)).unwrap();
    s.execute(&page(1)).unwrap();
    s.execute(&page(2)).unwrap();
    s.create(&root(), options_at(&vfs, 1, 0)).unwrap();
    s.close();
    vfs
}

/// A session of process `pid` at `secs` that runs `cmd` and crashes (no save, no clean close); returns its journal
/// segment's name.
fn crashed(vfs: &Arc<MemVfs>, pid: u32, secs: u64, cmd: &AnyCommand) -> String {
    let before = journals(vfs);
    let (mut s, _) = Session::open(
        &root(),
        options_at(vfs, pid, secs),
        session_options(u64::from(pid) * 100),
        Recovery::Keep,
    )
    .unwrap();
    s.execute(cmd).unwrap();
    drop(s);
    let new: Vec<String> = journals(vfs)
        .into_keys()
        .filter(|n| !before.contains_key(n))
        .collect();
    assert_eq!(new.len(), 1, "{new:?}");
    new[0].clone()
}

/// Ruling 2 of cycle 41: "Recovery acts on one journal at a time and never deletes an unreplayed one ... the app's path
/// (`ew-api` `Recovery::Apply`) shows the same." Two crashed sessions add page 5 (older) and page 6 (newer); each
/// open with `Recovery::Apply` replays the newest journal left and keeps the other one, byte for byte.
#[test]
fn the_app_path_recovers_one_journal_at_a_time() {
    let vfs = project();
    let a = crashed(&vfs, 2, 10, &page(5));
    let b = crashed(&vfs, 3, 20, &page(6));
    let a_bytes = journals(&vfs)[&a].clone();

    let (mut s, report) = Session::open(
        &root(),
        options_at(&vfs, 4, 30),
        session_options(400),
        Recovery::Apply,
    )
    .unwrap();
    let info = report.recovery.clone().expect("the newest journal");
    assert_eq!(info.labels, vec!["page.create".to_owned()]);
    assert_eq!(info.transactions, 1);
    assert_eq!(info.further, 1);
    assert!(s.recovery_refusal().is_none());
    assert_eq!(folios(&s), vec![1, 2, 6], "B's page, not A's");
    assert!(s.is_dirty());
    let left = journals(&vfs);
    assert_eq!(left.get(&a), Some(&a_bytes), "A's journal is unchanged");
    assert!(!left.contains_key(&b), "B's journal is replayed and removed");
    s.save().unwrap();
    s.close();

    let (mut s, report) = Session::open(
        &root(),
        options_at(&vfs, 5, 40),
        session_options(500),
        Recovery::Apply,
    )
    .unwrap();
    let info = report.recovery.clone().expect("A's journal now");
    assert_eq!(info.further, 0);
    assert!(!info.base_matches, "A's base is generation 1, the disk holds 2");
    assert!(s.recovery_refusal().is_none());
    assert_eq!(
        folios(&s),
        vec![1, 2, 5, 6],
        "A's page replays onto the saved generation"
    );
    s.save().unwrap();
    s.close();
    assert!(journals(&vfs).is_empty());

    let (s, report) = Session::open(
        &root(),
        options_at(&vfs, 6, 50),
        session_options(600),
        Recovery::Keep,
    )
    .unwrap();
    assert!(report.recovery.is_none());
    assert_eq!(folios(&s), vec![1, 2, 5, 6]);
    s.close();
}

/// Ruling 4 of cycle 41: "`ew-api` reports a refused replay instead of dropping the `RecoverError`". Session A sets page
/// 2's title and crashes; session B sets the same title to another text and saves; the replay of A's journal collides:
/// it is refused and reported, the disk state opens, the journal stays. Ruling 2: "`discard_recovery` removes **only
/// the summarised segment**" (here the only one).
#[test]
fn a_refused_replay_is_reported_and_the_journal_kept() {
    let vfs = project();
    let (probe, _) = Session::open(
        &root(),
        options_at(&vfs, 9, 1),
        session_options(900),
        Recovery::Keep,
    )
    .unwrap();
    let page2 = probe
        .world()
        .model
        .table::<ew_schema::Page>()
        .iter()
        .find(|(_, p)| p.folio.number == 2)
        .map(|(id, _)| serde_json::to_value(id).unwrap())
        .expect("page 2");
    probe.close();
    let title = |text: &str| AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({"kind": "page", "targets": [page2], "path": "title", "value": {"en": text}}),
    };
    let a = crashed(&vfs, 2, 10, &title("from A"));
    let a_bytes = journals(&vfs)[&a].clone();
    let (mut b, _) = Session::open(
        &root(),
        options_at(&vfs, 3, 20),
        session_options(300),
        Recovery::Keep,
    )
    .unwrap();
    b.execute(&title("from B")).unwrap();
    b.save().unwrap();
    let saved = b.world().model.clone();
    b.close();

    let (mut s, report) = Session::open(
        &root(),
        options_at(&vfs, 4, 30),
        session_options(400),
        Recovery::Apply,
    )
    .unwrap();
    assert!(report.recovery.is_some());
    match s.recovery_refusal() {
        Some(RecoverError::Replay(ReplayError::Collisions(c))) => assert!(!c.is_empty()),
        other => panic!("expected the collisions, got {other:?}"),
    }
    assert!(s.world().model.content_eq(&saved), "the disk state");
    assert!(!s.is_dirty());
    assert_eq!(journals(&vfs).get(&a), Some(&a_bytes), "kept, unchanged");
    assert!(s.discard_recovery(), "the confirmed discard");
    assert!(s.recovery_refusal().is_none());
    assert!(!journals(&vfs).contains_key(&a));
    assert!(!s.discard_recovery(), "nothing left to discard");
    s.close();
    assert!(journals(&vfs).is_empty());
}

/// A clock whose first sleep tells the test that the reader waits and blocks until the test lets it go on; the time
/// stands still.
#[derive(Debug)]
struct GateClock {
    now: SystemTime,
    waiting: Mutex<Option<mpsc::Sender<()>>>,
    go: Mutex<mpsc::Receiver<()>>,
    sleeps: AtomicU32,
}

impl Clock for GateClock {
    fn now(&self) -> SystemTime {
        self.now
    }

    fn sleep(&self, _d: Duration) {
        self.sleeps.fetch_add(1, Ordering::SeqCst);
        let first = self.waiting.lock().unwrap().take();
        if let Some(tell) = first {
            tell.send(()).unwrap();
            self.go
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(60))
                .expect("the test lets the reader go on");
        }
    }
}

/// Ruling 5 of cycle 41 (FM-7 as built): "a reader started while a save is in progress (`ew-store` `mem.rs`
/// `Fault`/`hold`, in-process) waits and succeeds once the save completes during the wait". The writer's save of
/// generation 2 is held at its manifest rename (an antivirus holds `manifest.json`); a reader opens behind the live
/// writer and waits; the hold ends, the writer completes the save, and the reader reads generation 2.
#[test]
fn a_reader_waits_for_a_save_in_progress_and_reads_the_saved_generation() {
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let manifest = root().join("manifest.json");
    let mut writer =
        Session::from_records(serde_json::from_str(PROJECT).unwrap(), [], [], session_options(1)).unwrap();
    writer.execute(&page(1)).unwrap();
    writer.create(&root(), options_at(&vfs, 1, 0)).unwrap();
    writer.execute(&page(2)).unwrap();
    vfs.hold(&manifest, 1000);
    match writer.save() {
        Err(SaveError::Incomplete { generation: 2, .. }) => {}
        other => panic!("expected the save of generation 2 held at its manifest rename, got {other:?}"),
    }

    let (tell, waiting) = mpsc::channel();
    let (go, gate) = mpsc::channel();
    let clock = Arc::new(GateClock {
        now: UNIX_EPOCH + Duration::from_secs(T0),
        waiting: Mutex::new(Some(tell)),
        go: Mutex::new(gate),
        sleeps: AtomicU32::new(0),
    });
    let mut reader = options_at(&vfs, 7, 0);
    reader.mode = OpenMode::ReadOnly;
    reader.state_dir = None;
    reader.clock = Arc::clone(&clock) as Arc<dyn Clock>;
    // The writer (process 1) is alive for the reader.
    reader.probe = Arc::new(Alive(1));
    let (generation, folios_read) = std::thread::scope(|scope| {
        let read = scope.spawn(move || {
            let (s, report) = Session::open(&root(), reader, SessionOptions::default(), Recovery::Keep)
                .expect("the reader opens");
            let found = (report.generation, folios(&s));
            s.close();
            found
        });
        waiting
            .recv_timeout(Duration::from_secs(60))
            .expect("the reader waits behind the save in progress");
        vfs.hold(&manifest, 0);
        assert_eq!(writer.retry_save().expect("the save completes"), Some(2));
        go.send(()).unwrap();
        read.join().unwrap()
    });
    assert_eq!(generation, 2);
    assert_eq!(folios_read, vec![1, 2]);
    assert!(clock.sleeps.load(Ordering::SeqCst) >= 1);
    writer.close();
}

/// `ew recover --replay`'s save (`Session::save_and_close`): when the save of a replayed journal fails before its
/// commit point, the session is dropped without its clean close, so the replayed transactions stay in its journal and
/// the next open offers them again (ruling 2 of cycle 41: "never deletes an unreplayed one"); a save that succeeds
/// closes cleanly and leaves no journal.
#[test]
fn a_failed_save_of_a_replay_keeps_the_replayed_transactions_in_a_journal() {
    let vfs = project();
    let a = crashed(&vfs, 2, 10, &page(5));
    let (s, report) = Session::open(
        &root(),
        options_at(&vfs, 3, 20),
        session_options(300),
        Recovery::Apply,
    )
    .unwrap();
    assert_eq!(
        report.recovery.map(|r| r.labels),
        Some(vec!["page.create".to_owned()])
    );
    assert_eq!(folios(&s), vec![1, 2, 5]);
    assert!(
        !journals(&vfs).contains_key(&a),
        "A's journal is replayed into this session's journal"
    );
    vfs.reset_calls();
    vfs.set_fault(Fault::Error {
        at: 1,
        kind: std::io::ErrorKind::PermissionDenied,
    });
    match s.save_and_close() {
        Err(SaveError::Incomplete { .. }) | Ok(_) => panic!("the save fails before its commit point"),
        Err(_) => {}
    }
    vfs.set_fault(Fault::None);
    assert_eq!(journals(&vfs).len(), 1, "this session's journal stays");

    let (s, report) = Session::open(
        &root(),
        options_at(&vfs, 4, 30),
        session_options(400),
        Recovery::Apply,
    )
    .unwrap();
    let info = report
        .recovery
        .expect("the replayed transactions are offered again");
    assert_eq!((info.transactions, info.labels), (1, vec!["recover".to_owned()]));
    assert_eq!(folios(&s), vec![1, 2, 5]);
    s.save_and_close().expect("the save");
    assert!(journals(&vfs).is_empty());
    let (s, report) = Session::open(
        &root(),
        options_at(&vfs, 5, 40),
        session_options(500),
        Recovery::Keep,
    )
    .unwrap();
    assert!(report.recovery.is_none());
    assert_eq!(folios(&s), vec![1, 2, 5]);
    s.close();
}
