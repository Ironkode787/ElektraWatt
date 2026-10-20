// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lock file `.~ew.lock` of doc 05 §7 (ADR-0010 amendment 1; FR-COL-006, FR-COL-026), end to end on
//! the real file system with a copy of the golden fixture: acquisition, read-only fallback, heartbeat,
//! lock loss, stale detection, takeover, release and the pruning of `.~ew.lock.stale-*` files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ew_store::lock::{TakeoverError, new_session_id};
use ew_store::{
    Access, Changes, Clock, Identity, LockRecord, LockState, ManualClock, OpenError, OpenMode, Opened,
    ProcessProbe, ProcessState, ReadOnlyReason, SaveError, Store, StoreOptions,
};

const LOCK: &str = ".~ew.lock";
const STALE_PREFIX: &str = ".~ew.lock.stale-";
const MIN: Duration = Duration::from_secs(60);

/// A probe that reports the listed PIDs dead and every other PID alive without a start time.
#[derive(Debug)]
struct Probe(BTreeSet<u32>);

impl ProcessProbe for Probe {
    fn probe(&self, pid: u32) -> ProcessState {
        if self.0.contains(&pid) {
            ProcessState::Dead
        } else {
            ProcessState::Alive { started: None }
        }
    }
}

/// A scratch directory tree removed at the end of the test.
struct Scratch {
    base: PathBuf,
    project: PathBuf,
    state: PathBuf,
    clock: Arc<ManualClock>,
    start: SystemTime,
}

impl Scratch {
    fn new() -> Scratch {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let base = std::env::temp_dir().join(format!(
            "ew-store-lock-{}-{}-{nanos}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let project = base.join("J-E-2416.ew");
        let state = base.join("state");
        std::fs::create_dir_all(&state).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
        copy_dir(&fixture, &project);
        // Whole seconds: the lock record and file times are compared at second precision.
        let whole = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let start = UNIX_EPOCH + Duration::from_secs(whole);
        Scratch {
            base,
            project,
            state,
            clock: Arc::new(ManualClock::new(start)),
            start,
        }
    }

    fn lock_path(&self) -> PathBuf {
        self.project.join(LOCK)
    }

    fn lock_bytes(&self) -> Vec<u8> {
        std::fs::read(self.lock_path()).unwrap()
    }

    fn lock_record(&self) -> LockRecord {
        LockRecord::parse(&self.lock_bytes()).expect("the lock file holds a record")
    }

    fn set_lock_mtime(&self, t: SystemTime) {
        set_mtime(&self.lock_path(), t);
    }

    fn opts(&self, pid: u32, host: &str, dead: &[u32]) -> StoreOptions {
        let mut o = StoreOptions::new("ElektraWatt test");
        o.state_dir = Some(self.state.clone());
        o.clock = Arc::clone(&self.clock) as Arc<dyn Clock>;
        o.identity = Identity {
            session: new_session_id(),
            user: "<os user>".into(),
            host: host.into(),
            pid,
            pid_started: UNIX_EPOCH,
            app_version: "ElektraWatt test".into(),
        };
        o.probe = Arc::new(Probe(dead.iter().copied().collect()));
        o
    }

    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        snap(&self.project, &self.project, &mut out);
        out
    }

    fn stale_files(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&self.project)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.starts_with(STALE_PREFIX))
            .collect();
        v.sort();
        v
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn set_mtime(path: &Path, t: SystemTime) {
    let f = std::fs::File::options().write(true).open(path).unwrap();
    f.set_modified(t).unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let dest = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            std::fs::copy(entry.path(), dest).unwrap();
        }
    }
}

fn snap(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            snap(root, &entry.path(), out);
        } else {
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read(entry.path()).unwrap());
        }
    }
}

fn locked_by(opened: &Opened) -> ew_store::LockInfo {
    match &opened.report.access {
        Access::ReadOnly(ReadOnlyReason::LockedBy(info)) => (**info).clone(),
        other => panic!("expected a read-only session locked by another, got {other:?}"),
    }
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn expect_takeover_error(result: Result<Opened, OpenError>) -> TakeoverError {
    match result {
        Err(OpenError::Takeover(e)) => *e,
        Err(e) => panic!("expected a takeover error, got {e}"),
        Ok(o) => panic!("expected a takeover error, got access {:?}", o.report.access),
    }
}

#[test]
fn first_open_takes_the_lock() {
    let s = Scratch::new();
    let o = s.opts(1001, "<host>", &[]);
    let session = o.identity.session.clone();
    let opened = Store::open(&s.project, o).unwrap();
    assert_eq!(opened.report.access, Access::Edit);
    assert!(s.lock_path().exists());
    let bytes = s.lock_bytes();
    let rec = LockRecord::parse(&bytes).unwrap();
    assert_eq!(rec.session, session);
    assert_eq!(rec.user, "<os user>");
    assert_eq!(rec.host, "<host>");
    assert_eq!(rec.pid, 1001);
    assert_eq!(rec.app_version, "ElektraWatt test");
    // RFC 3339, whole seconds, UTC: YYYY-MM-DDTHH:MM:SSZ.
    let b = rec.started_at.as_bytes();
    assert_eq!(b.len(), 20, "{}", rec.started_at);
    for (i, c) in b.iter().enumerate() {
        match i {
            4 | 7 => assert_eq!(*c, b'-'),
            10 => assert_eq!(*c, b'T'),
            13 | 16 => assert_eq!(*c, b':'),
            19 => assert_eq!(*c, b'Z'),
            _ => assert!(c.is_ascii_digit(), "{}", rec.started_at),
        }
    }
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with("{\"lock\":1,\"session\":"), "{text}");
    assert!(text.ends_with("}\n"), "{text}");
    assert_eq!(text.matches('\n').count(), 1, "one line");
    assert!(
        !text.contains(": ") && !text.contains(", "),
        "compact JSON: {text}"
    );
}

#[test]
fn second_instance_opens_read_only_naming_the_holder() {
    let s = Scratch::new();
    let a = Store::open(&s.project, s.opts(1001, "<host>", &[])).unwrap();
    assert_eq!(a.report.access, Access::Edit);
    let a_record = s.lock_record();
    let a_bytes = s.lock_bytes();

    let mut b = Store::open(&s.project, s.opts(1002, "<host>", &[])).unwrap();
    let info = locked_by(&b);
    assert_eq!(info.record.as_ref(), Some(&a_record));
    assert!(!info.stale);
    assert_eq!(
        format!("{}", info.record.as_ref().unwrap()),
        format!("<os user>@<host> since {}", a_record.started_at)
    );

    let before = s.snapshot();
    match b.store.save(&b.model, Changes::All) {
        Err(SaveError::ReadOnly(_)) => {}
        other => panic!("expected SaveError::ReadOnly, got {other:?}"),
    }
    assert_eq!(s.snapshot(), before, "a read-only save writes nothing");
    assert_eq!(s.lock_bytes(), a_bytes, "B never touched the lock");
    drop(b);
    assert_eq!(s.lock_bytes(), a_bytes, "B's drop does not release A's lock");
    drop(a);
}

#[test]
fn read_only_mode_never_takes_the_lock() {
    let s = Scratch::new();
    let mut o = s.opts(1001, "<host>", &[]);
    o.mode = OpenMode::ReadOnly;
    let opened = Store::open(&s.project, o).unwrap();
    assert_eq!(opened.report.access, Access::ReadOnly(ReadOnlyReason::Requested));
    assert!(!s.lock_path().exists());
}

#[test]
fn heartbeat_refreshes_and_detects_loss() {
    let s = Scratch::new();
    let mut a = Store::open(&s.project, s.opts(1001, "<host>", &[])).unwrap();
    s.clock.advance(5 * MIN);
    assert_eq!(a.store.heartbeat().unwrap(), LockState::Held);
    let mtime = std::fs::metadata(s.lock_path()).unwrap().modified().unwrap();
    assert_eq!(secs(mtime), secs(s.clock.now()));

    // C takes the lock over behind A's back.
    let c = s.opts(1003, "<other host>", &[]).identity.record(s.clock.now());
    std::fs::write(s.lock_path(), c.to_text()).unwrap();
    assert_eq!(a.store.heartbeat().unwrap(), LockState::Lost(Some(c.clone())));
    match a.store.access() {
        Access::ReadOnly(ReadOnlyReason::LockLost(rec)) => assert_eq!(rec.as_deref(), Some(&c)),
        other => panic!("expected LockLost, got {other:?}"),
    }
    let before = s.snapshot();
    assert!(a.store.save(&a.model, Changes::All).is_err());
    assert_eq!(s.snapshot(), before);
    drop(a);
    assert_eq!(
        s.lock_record(),
        c,
        "a session that lost the lock leaves the new holder's lock alone"
    );

    // Variant: the lock file is deleted.
    let s = Scratch::new();
    let mut a = Store::open(&s.project, s.opts(1001, "<host>", &[])).unwrap();
    std::fs::remove_file(s.lock_path()).unwrap();
    assert_eq!(a.store.heartbeat().unwrap(), LockState::Lost(None));
    assert!(matches!(
        a.store.access(),
        Access::ReadOnly(ReadOnlyReason::LockLost(None))
    ));
}

#[test]
fn save_after_lock_loss_writes_nothing() {
    let s = Scratch::new();
    let mut a = Store::open(&s.project, s.opts(1001, "<host>", &[])).unwrap();
    let c = s.opts(1003, "<other host>", &[]).identity.record(s.clock.now());
    std::fs::write(s.lock_path(), c.to_text()).unwrap();
    let before = s.snapshot();
    match a.store.save(&a.model, Changes::All) {
        Err(SaveError::LockLost(rec)) => assert_eq!(rec.as_deref(), Some(&c)),
        other => panic!("expected SaveError::LockLost, got {other:?}"),
    }
    assert_eq!(s.snapshot(), before);
}

/// A crashes (its lock stays, last heartbeat at the clock's start) and the clock moves `after`.
fn crashed_holder(s: &Scratch, host: &str, pid: u32, after: Duration) -> String {
    let o = s.opts(pid, host, &[]);
    let session = o.identity.session.clone();
    let a = Store::open(&s.project, o).unwrap();
    assert_eq!(a.report.access, Access::Edit);
    a.store.crash();
    s.set_lock_mtime(s.start);
    s.clock.advance(after);
    session
}

#[test]
fn stale_by_age() {
    let s = Scratch::new();
    crashed_holder(&s, "<host>", 1001, 61 * MIN);
    let b = s.opts(1002, "<other host>", &[]);
    let b_session = b.identity.session.clone();
    let opened = Store::open(&s.project, b.clone()).unwrap();
    assert!(locked_by(&opened).stale);
    drop(opened);
    let taken = Store::take_over(&s.project, b).unwrap();
    assert_eq!(taken.report.access, Access::Edit);
    assert_eq!(s.lock_record().session, b_session);
    drop(taken);

    // 59 minutes: not stale, the takeover is refused.
    let s = Scratch::new();
    crashed_holder(&s, "<host>", 1001, 59 * MIN);
    let b = s.opts(1002, "<other host>", &[]);
    let opened = Store::open(&s.project, b.clone()).unwrap();
    assert!(!locked_by(&opened).stale);
    drop(opened);
    let e = expect_takeover_error(Store::take_over(&s.project, b));
    assert!(matches!(e, TakeoverError::NotStale(_)), "{e:?}");
    assert!(s.stale_files().is_empty());
}

#[test]
fn takeover_by_age_keeps_the_stale_file() {
    let s = Scratch::new();
    let a_session = crashed_holder(&s, "<host>", 1001, 61 * MIN);
    let taken = Store::take_over(&s.project, s.opts(1002, "<other host>", &[])).unwrap();
    assert_eq!(taken.report.access, Access::Edit);
    assert!(s.project.join(format!("{STALE_PREFIX}{a_session}")).exists());
}

#[test]
fn stale_by_dead_pid_on_the_same_host() {
    let s = Scratch::new();
    let a_session = crashed_holder(&s, "<host>", 1001, Duration::ZERO);
    let b = s.opts(1002, "<host>", &[1001]);
    let opened = Store::open(&s.project, b.clone()).unwrap();
    assert!(
        locked_by(&opened).stale,
        "a dead PID on the same host is stale at once"
    );
    drop(opened);
    let taken = Store::take_over(&s.project, b).unwrap();
    assert_eq!(taken.report.access, Access::Edit);
    assert!(s.project.join(format!("{STALE_PREFIX}{a_session}")).exists());
    drop(taken);

    // Same host, PID alive: not stale.
    let s = Scratch::new();
    crashed_holder(&s, "<host>", 1001, Duration::ZERO);
    let b = s.opts(1002, "<host>", &[]);
    let opened = Store::open(&s.project, b.clone()).unwrap();
    assert!(!locked_by(&opened).stale);
    drop(opened);
    let e = expect_takeover_error(Store::take_over(&s.project, b));
    assert!(matches!(e, TakeoverError::NotStale(_)), "{e:?}");
}

#[test]
fn two_takeovers_one_winner() {
    let s = Scratch::new();
    crashed_holder(&s, "<host>", 1001, 61 * MIN);
    let mut b = Store::take_over(&s.project, s.opts(1002, "<other host>", &[])).unwrap();
    assert_eq!(b.report.access, Access::Edit);
    // B's lock is fresh: its heartbeat sets the modification time to the clock's now.
    assert_eq!(b.store.heartbeat().unwrap(), LockState::Held);
    let b_record = s.lock_record();

    // C, starting from the same stale state, meets B's fresh lock.
    let e = expect_takeover_error(Store::take_over(&s.project, s.opts(1003, "<host>", &[])));
    assert!(
        matches!(e, TakeoverError::NotStale(_) | TakeoverError::Lost),
        "{e:?}"
    );
    assert_eq!(s.lock_record(), b_record);
    drop(b);

    // A racing winner already renamed: the stale target name exists while A's stale lock is in place.
    let s = Scratch::new();
    let a_session = crashed_holder(&s, "<host>", 1001, 61 * MIN);
    let a_record = s.lock_record();
    let target = s.project.join(format!("{STALE_PREFIX}{a_session}"));
    std::fs::write(&target, b"winner").unwrap();
    let e = expect_takeover_error(Store::take_over(&s.project, s.opts(1003, "<host>", &[])));
    assert!(matches!(e, TakeoverError::Lost), "{e:?}");
    assert_eq!(s.lock_record(), a_record, "C did not take the lock");
    assert_eq!(std::fs::read(&target).unwrap(), b"winner");
}

#[test]
fn a_takeover_that_crashed_after_its_hard_link_does_not_block_the_next_one() {
    // Review cycle 8, 5(e): the rename without replacement is a hard link plus an unlink; a crash between
    // the two leaves `.~ew.lock.stale-<session>` beside the lock, both the stale record. Every later
    // takeover used to lose to that copy.
    let s = Scratch::new();
    let a_session = crashed_holder(&s, "<host>", 1001, 61 * MIN);
    let a_record = s.lock_record();
    let copy = s.project.join(format!("{STALE_PREFIX}{a_session}"));
    std::fs::hard_link(s.lock_path(), &copy).unwrap();
    let b = s.opts(1002, "<other host>", &[]);
    let b_session = b.identity.session.clone();
    let taken = Store::take_over(&s.project, b).unwrap();
    assert_eq!(taken.report.access, Access::Edit);
    assert_eq!(s.lock_record().session, b_session);
    assert_eq!(
        LockRecord::parse(&std::fs::read(&copy).unwrap()),
        Some(a_record),
        "the stale record is kept as evidence"
    );
    assert_eq!(s.stale_files(), [format!("{STALE_PREFIX}{a_session}")]);
}

#[test]
fn one_project_through_two_paths_has_one_journal() {
    // Review cycle 8, 5(c): the journal directory is keyed by the canonical project path.
    let s = Scratch::new();
    let a = Store::open(&s.project, s.opts(1001, "<host>", &[])).unwrap();
    let dir = a
        .store
        .journal_dir()
        .expect("a journal with a state dir")
        .to_path_buf();
    a.store.close();
    let other = s.base.join(".").join("J-E-2416.ew");
    assert_ne!(other.as_os_str(), s.project.as_os_str(), "a different text");
    let b = Store::open(&other, s.opts(1002, "<host>", &[])).unwrap();
    assert_eq!(b.store.journal_dir(), Some(dir.as_path()));
    b.store.close();
}

#[test]
fn close_releases_and_drop_releases() {
    let s = Scratch::new();
    let o = s.opts(1001, "<host>", &[]);
    let session = o.identity.session.clone();
    let opened = Store::open(&s.project, o).unwrap();
    let jdir = opened
        .store
        .journal_dir()
        .expect("a journal with a state dir")
        .to_path_buf();
    let has_segment = |session: &str| {
        std::fs::read_dir(&jdir)
            .map(|d| {
                d.filter_map(Result::ok)
                    .any(|e| e.file_name().to_string_lossy().starts_with(session))
            })
            .unwrap_or(false)
    };
    assert!(has_segment(&session), "the edit session journals");
    assert!(s.lock_path().exists());
    opened.store.close();
    assert!(!s.lock_path().exists());
    assert!(!has_segment(&session));

    // Drop without close: the lock goes, the journal segment stays.
    let o = s.opts(1002, "<host>", &[]);
    let session = o.identity.session.clone();
    let opened = Store::open(&s.project, o).unwrap();
    assert_eq!(opened.report.access, Access::Edit);
    assert!(s.lock_path().exists());
    assert!(has_segment(&session));
    drop(opened);
    assert!(!s.lock_path().exists());
    assert!(has_segment(&session), "drop keeps the journal segment");
}

#[test]
fn stale_lock_files_are_pruned() {
    let s = Scratch::new();
    let old = s.project.join(format!("{STALE_PREFIX}x"));
    let fresh = s.project.join(format!("{STALE_PREFIX}y"));
    std::fs::write(&old, b"old").unwrap();
    std::fs::write(&fresh, b"fresh").unwrap();
    set_mtime(&old, s.start - 61 * MIN);
    set_mtime(&fresh, s.start - 10 * MIN);
    let opened = Store::open(&s.project, s.opts(1001, "<host>", &[])).unwrap();
    assert_eq!(opened.report.access, Access::Edit);
    assert!(
        !old.exists(),
        "a stale-lock file older than 60 minutes is deleted"
    );
    assert!(fresh.exists(), "a fresh one is kept");
}

#[test]
fn lock_record_round_trip() {
    let s = Scratch::new();
    let id = s.opts(4242, "<host>", &[]).identity;
    let now = UNIX_EPOCH + Duration::from_secs(1_792_222_930);
    let rec = id.record(now);
    assert_eq!(rec.started_at, "2026-10-17T07:42:10Z");
    let text = rec.to_text();
    assert_eq!(LockRecord::parse(text.as_bytes()), Some(rec.clone()));
    assert_eq!(
        LockRecord::parse(text.replace("\"lock\":1", "\"lock\":2").as_bytes()),
        None
    );
    let no_session =
        r#"{"lock":1,"user":"u","host":"h","pid":1,"pid_started":"x","app_version":"v","started_at":"t"}"#;
    assert_eq!(LockRecord::parse(no_session.as_bytes()), None);
    assert_eq!(LockRecord::parse(b"not json at all\n"), None);
}
