// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lock file `.~ew.lock` (doc 05 §7, ADR-0010 amendment 1, FR-COL-006, FR-COL-026).
//!
//! - **Acquisition:** `create_new` of `.~ew.lock` with the record, then `sync`; no byte-range locks.
//! - **Held:** the holder's session ID is in the file. Another opener is read-only and names the holder.
//! - **Heartbeat:** the holder sets the lock's modification time every [`HEARTBEAT`] and re-reads the
//!   session ID; a different or missing ID means the lock is lost (the session turns read-only).
//! - **Stale:** older than `stale_after` (default [`STALE_AFTER`]), or on the same host with a dead PID or a
//!   PID whose start time differs.
//! - **Takeover** (confirmed by the caller): rename the stale lock **without replacement** to
//!   `.~ew.lock.stale-<stale session>`, create our own lock, re-read it after [`TAKEOVER_SETTLE`].

use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde_json::Value;

use crate::clock::{Clock, rfc3339};
use crate::vfs::{Vfs, join};
use ew_format::{LOCK, STALE_LOCK_PREFIX};

/// The heartbeat interval (doc 05 §7.2).
pub const HEARTBEAT: Duration = Duration::from_secs(5 * 60);
/// The default stale age, `lock.stale_after` (doc 05 §7.4, open question 10).
pub const STALE_AFTER: Duration = Duration::from_secs(60 * 60);
/// The wait before a takeover re-reads its new lock (doc 05 §7.4 step 2).
pub const TAKEOVER_SETTLE: Duration = Duration::from_secs(2);

/// Who holds (or wants) the lock: the record of doc 05 §7.1.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LockRecord {
    /// The session ID (a UUID): the ownership key.
    pub session: String,
    /// The OS user.
    pub user: String,
    /// The host name.
    pub host: String,
    /// The process ID.
    pub pid: u32,
    /// The process's start time, RFC 3339 (PID reuse detection).
    pub pid_started: String,
    /// The program and version.
    pub app_version: String,
    /// When the session took the lock, RFC 3339.
    pub started_at: String,
}

impl LockRecord {
    /// The compact JSON text of the record, members in the order of doc 05 §7.1, with a final LF.
    pub fn to_text(&self) -> String {
        // serde_json's map is ordered by key; the record is written member by member instead.
        let members: [(&str, Value); 8] = [
            ("lock", Value::from(1)),
            ("session", Value::from(self.session.as_str())),
            ("user", Value::from(self.user.as_str())),
            ("host", Value::from(self.host.as_str())),
            ("pid", Value::from(self.pid)),
            ("pid_started", Value::from(self.pid_started.as_str())),
            ("app_version", Value::from(self.app_version.as_str())),
            ("started_at", Value::from(self.started_at.as_str())),
        ];
        let mut out = String::from("{");
        for (i, (k, v)) in members.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&Value::from(*k).to_string());
            out.push(':');
            out.push_str(&v.to_string());
        }
        out.push_str("}\n");
        out
    }

    /// Reads a lock record; `None` for a file that is not one (a torn create, a foreign file).
    pub fn parse(bytes: &[u8]) -> Option<LockRecord> {
        let value: Value = serde_json::from_slice(bytes).ok()?;
        let s = |k: &str| value.get(k)?.as_str().map(str::to_owned);
        if value.get("lock")?.as_u64()? != 1 {
            return None;
        }
        Some(LockRecord {
            session: s("session")?,
            user: s("user").unwrap_or_default(),
            host: s("host").unwrap_or_default(),
            pid: u32::try_from(value.get("pid")?.as_u64()?).ok()?,
            pid_started: s("pid_started").unwrap_or_default(),
            app_version: s("app_version").unwrap_or_default(),
            started_at: s("started_at").unwrap_or_default(),
        })
    }
}

impl fmt::Display for LockRecord {
    /// `<user>@<host> since <time>` (the banner of doc 05 §7.3).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{} since {}", self.user, self.host, self.started_at)
    }
}

/// Who this process is, for the lock record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Identity {
    /// The session ID: a fresh UUID per session.
    pub session: String,
    /// The OS user.
    pub user: String,
    /// The host name.
    pub host: String,
    /// The process ID.
    pub pid: u32,
    /// The process's start time.
    pub pid_started: SystemTime,
    /// The program and version (`"ElektraWatt 0.0.0"`).
    pub app_version: String,
}

fn env(names: &[&str]) -> Option<String> {
    names
        .iter()
        // The user and host names of the lock record, which is not project data.
        .find_map(|n| std::env::var(n).ok().filter(|v| !v.trim().is_empty()))
        .map(|v| v.trim().to_owned())
}

/// The process's start time: the first time this function is called (a process-wide approximation; the
/// OS start time needs platform APIs the store does not use yet).
fn process_started() -> SystemTime {
    static STARTED: OnceLock<SystemTime> = OnceLock::new();
    // The lock record's `pid_started`: not project data (the project files never carry a time, NFR-REL-07).
    *STARTED.get_or_init(SystemTime::now)
}

/// A fresh random UUID (version 4) as text, from the standard library's randomly keyed hasher: unique per
/// session, which is all the lock and the journal need (it is not a record ID; those come from `ew-ids`).
pub fn new_session_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let word = |salt: u64| {
        // Randomness and the clock only make the session ID unique (it names the lock and the journal, not a record).
        #[allow(clippy::disallowed_types)]
        // The randomly keyed hasher is the source of that uniqueness.
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(salt);
        h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
        h.write_u32(std::process::id());
        // Part of the uniqueness of the session ID (see above).
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        h.write_u128(nanos);
        h.finish()
    };
    let (a, b) = (word(0x5eed), word(0xface));
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_be_bytes());
    bytes[8..].copy_from_slice(&b.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

impl Identity {
    /// This process, with a fresh session ID and the program name `app_version`.
    pub fn current(app_version: &str) -> Identity {
        let host = env(&["COMPUTERNAME", "HOSTNAME"])
            .or_else(|| {
                std::fs::read_to_string("/etc/hostname")
                    .ok()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| "localhost".to_owned());
        Identity {
            session: new_session_id(),
            user: env(&["USERNAME", "USER", "LOGNAME"]).unwrap_or_else(|| "unknown".to_owned()),
            host,
            pid: std::process::id(),
            pid_started: process_started(),
            app_version: app_version.to_owned(),
        }
    }

    /// The lock record of this identity taken at `now`.
    pub fn record(&self, now: SystemTime) -> LockRecord {
        LockRecord {
            session: self.session.clone(),
            user: self.user.clone(),
            host: self.host.clone(),
            pid: self.pid,
            pid_started: rfc3339(self.pid_started),
            app_version: self.app_version.clone(),
            started_at: rfc3339(now),
        }
    }
}

/// What the OS says about a process on this host.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessState {
    /// Running; with its start time where the platform reports one.
    Alive {
        /// The start time, if known.
        started: Option<SystemTime>,
    },
    /// Not running.
    Dead,
    /// The platform gives no answer: only the age rule applies.
    Unknown,
}

/// Process liveness for the same-host stale rule (doc 05 §7.4).
pub trait ProcessProbe: Send + Sync + fmt::Debug {
    /// The state of process `pid` on this host.
    fn probe(&self, pid: u32) -> ProcessState;
}

/// The OS probe: on Linux, `/proc/<pid>` tells whether the process runs (its start time is not read: the
/// PID-reuse rule then needs a platform API, a known gap); elsewhere [`ProcessState::Unknown`], so only the
/// age rule makes a lock stale.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsProbe;

impl ProcessProbe for OsProbe {
    fn probe(&self, pid: u32) -> ProcessState {
        if cfg!(target_os = "linux") {
            if Path::new(&format!("/proc/{pid}")).exists() {
                ProcessState::Alive { started: None }
            } else {
                ProcessState::Dead
            }
        } else {
            ProcessState::Unknown
        }
    }
}

/// The lock as found on disk.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LockInfo {
    /// The record, `None` when the file is not a readable lock record.
    pub record: Option<LockRecord>,
    /// The lock file's modification time (the last heartbeat).
    pub modified: Option<SystemTime>,
    /// Whether it is stale (doc 05 §7.4).
    pub stale: bool,
}

/// The stale rule of doc 05 §7.4.
pub fn is_stale(
    record: Option<&LockRecord>,
    modified: Option<SystemTime>,
    now: SystemTime,
    stale_after: Duration,
    me: &Identity,
    probe: &dyn ProcessProbe,
) -> bool {
    let old = modified.is_some_and(|m| now.duration_since(m).is_ok_and(|age| age > stale_after));
    if old {
        return true;
    }
    let Some(record) = record else {
        return false;
    };
    if record.host != me.host || (record.pid == me.pid && record.session != me.session) {
        return false;
    }
    match probe.probe(record.pid) {
        ProcessState::Dead => true,
        ProcessState::Alive { started: Some(t) } => rfc3339(t) != record.pid_started,
        ProcessState::Alive { started: None } | ProcessState::Unknown => false,
    }
}

/// Reads `.~ew.lock` of `root`; `Ok(None)` when there is none.
pub fn read_lock(
    vfs: &dyn Vfs,
    root: &Path,
    now: SystemTime,
    stale_after: Duration,
    me: &Identity,
    probe: &dyn ProcessProbe,
) -> io::Result<Option<LockInfo>> {
    let path = join(root, LOCK);
    let bytes = match vfs.read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let modified = vfs.stat(&path).ok().and_then(|s| s.modified);
    let record = LockRecord::parse(&bytes);
    let stale = is_stale(record.as_ref(), modified, now, stale_after, me, probe);
    Ok(Some(LockInfo {
        record,
        modified,
        stale,
    }))
}

/// The outcome of trying to take the lock.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Acquire {
    /// We hold it.
    Held,
    /// Another session holds it.
    HeldBy(LockInfo),
    /// The directory is not writable (EACCES, EROFS): read-only with this reason (doc 05 §7.1).
    NotWritable(String),
}

/// Creates `.~ew.lock` with our record and syncs it (doc 05 §7.1).
pub fn acquire(
    vfs: &dyn Vfs,
    root: &Path,
    clock: &dyn Clock,
    stale_after: Duration,
    me: &Identity,
    probe: &dyn ProcessProbe,
) -> io::Result<Acquire> {
    let path = join(root, LOCK);
    let now = clock.now();
    match vfs.create_new(&path, me.record(now).to_text().as_bytes()) {
        Ok(()) => {
            vfs.sync(&path)?;
            vfs.sync_dir(root)?;
            Ok(Acquire::Held)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            match read_lock(vfs, root, now, stale_after, me, probe)? {
                Some(info) if info.record.as_ref().is_some_and(|r| r.session == me.session) => {
                    Ok(Acquire::Held)
                }
                Some(info) => Ok(Acquire::HeldBy(info)),
                // Released between our create and our read: try once more.
                None => match vfs.create_new(&path, me.record(now).to_text().as_bytes()) {
                    Ok(()) => {
                        vfs.sync(&path)?;
                        Ok(Acquire::Held)
                    }
                    Err(e) => Err(e),
                },
            }
        }
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
            ) =>
        {
            Ok(Acquire::NotWritable(e.to_string()))
        }
        Err(e) => Err(e),
    }
}

/// Who holds the lock now, as seen by session `me`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Holder {
    /// Our session.
    Us,
    /// Another session (`None`: a lock file that is not a readable record).
    Other(Option<LockRecord>),
    /// No lock file.
    Nobody,
}

/// Re-reads the lock's session ID (doc 05 §7.2).
pub fn holder(vfs: &dyn Vfs, root: &Path, me: &Identity) -> io::Result<Holder> {
    match vfs.read(&join(root, LOCK)) {
        Ok(bytes) => {
            let record = LockRecord::parse(&bytes);
            if record.as_ref().is_some_and(|r| r.session == me.session) {
                Ok(Holder::Us)
            } else {
                Ok(Holder::Other(record))
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Holder::Nobody),
        Err(e) => Err(e),
    }
}

/// Removes our lock if it is still ours.
pub fn release(vfs: &dyn Vfs, root: &Path, me: &Identity) -> io::Result<()> {
    if holder(vfs, root, me)? == Holder::Us {
        vfs.remove_file(&join(root, LOCK))?;
    }
    Ok(())
}

/// Why a takeover did not happen.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TakeoverError {
    /// The lock is not stale: a takeover needs a stale lock.
    NotStale(LockInfo),
    /// Another instance took the lock first.
    Lost,
    /// There is no lock to take over (open normally).
    NoLock,
}

/// Takes over a stale lock (doc 05 §7.4): rename it without replacement to
/// `.~ew.lock.stale-<stale session>`, create our own lock, wait [`TAKEOVER_SETTLE`] and re-read it.
pub fn take_over(
    vfs: &dyn Vfs,
    root: &Path,
    clock: &dyn Clock,
    stale_after: Duration,
    me: &Identity,
    probe: &dyn ProcessProbe,
) -> io::Result<Result<LockInfo, TakeoverError>> {
    let Some(info) = read_lock(vfs, root, clock.now(), stale_after, me, probe)? else {
        return Ok(Err(TakeoverError::NoLock));
    };
    if !info.stale {
        return Ok(Err(TakeoverError::NotStale(info)));
    }
    let stale_session = info
        .record
        .as_ref()
        .map_or_else(|| "unknown".to_owned(), |r| sanitize(&r.session));
    let from = join(root, LOCK);
    let to = join(root, &format!("{STALE_LOCK_PREFIX}{stale_session}"));
    match rename_away(vfs, &from, &to)? {
        Ok(()) => {}
        Err(TakeoverError::Lost) if left_by_a_crashed_takeover(vfs, &from, &to, &info) => {
            // A takeover that crashed between the hard link and the unlink of `rename_noreplace` left the
            // stale copy beside the lock, both with the stale record: every later takeover would lose to
            // it (review cycle 8, 5(e)). The copy is redundant (the lock still holds the record): prune it
            // and try once more; a racing instance that wins instead makes this one lose as usual.
            match vfs.remove_file(&to) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            if let Err(lost) = rename_away(vfs, &from, &to)? {
                return Ok(Err(lost));
            }
        }
        Err(lost) => return Ok(Err(lost)),
    }
    // The renamed record keeps the holder's last heartbeat as its modification time; restamp it so that
    // "older than `stale_after`" (doc 05 §7.4, last paragraph) counts from the takeover, and the takeover's
    // own open does not prune the evidence at once.
    let _ = vfs.set_modified(&to, clock.now());
    match vfs.create_new(&from, me.record(clock.now()).to_text().as_bytes()) {
        Ok(()) => vfs.sync(&from)?,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(Err(TakeoverError::Lost)),
        Err(e) => return Err(e),
    }
    vfs.sync_dir(root)?;
    clock.sleep(TAKEOVER_SETTLE);
    if holder(vfs, root, me)? != Holder::Us {
        return Ok(Err(TakeoverError::Lost));
    }
    Ok(Ok(info))
}

/// Step 1 of the takeover (doc 05 §7.4): renames the stale lock `from` to `to` without replacement;
/// `Lost` when `to` exists or `from` is gone (another instance was faster).
fn rename_away(vfs: &dyn Vfs, from: &Path, to: &Path) -> io::Result<Result<(), TakeoverError>> {
    match vfs.rename_noreplace(from, to) {
        Ok(()) => Ok(Ok(())),
        Err(e) if matches!(e.kind(), io::ErrorKind::AlreadyExists | io::ErrorKind::NotFound) => {
            Ok(Err(TakeoverError::Lost))
        }
        // Doc 05 §7.4 step 3: no rename without replacement on this file system; the plain rename and the
        // re-read after the settle time (plus the heartbeat's session check) detect the loser.
        Err(e) if e.kind() == io::ErrorKind::Unsupported => vfs.rename(from, to).map(Ok),
        Err(e) => Err(e),
    }
}

/// Whether the existing stale copy `to` is the leftover of a takeover that crashed after linking the lock
/// to it and before unlinking the lock: the copy holds the same bytes as the lock `from`, which still holds
/// the stale record of `info`. Pruning such a copy loses nothing.
fn left_by_a_crashed_takeover(vfs: &dyn Vfs, from: &Path, to: &Path, info: &LockInfo) -> bool {
    let (Ok(lock), Ok(copy)) = (vfs.read(from), vfs.read(to)) else {
        return false;
    };
    lock == copy && LockRecord::parse(&lock) == info.record
}

/// A session ID usable in a file name: lower-case hex digits and hyphens only.
fn sanitize(session: &str) -> String {
    let s: String = session
        .chars()
        .filter(|c| c.is_ascii_hexdigit() || *c == '-')
        .map(|c| c.to_ascii_lowercase())
        .take(36)
        .collect();
    if s.is_empty() { "unknown".to_owned() } else { s }
}

/// Deletes `.~ew.lock.stale-*` files older than `stale_after` (doc 05 §7.4, last paragraph).
pub fn prune_stale_locks(vfs: &dyn Vfs, root: &Path, now: SystemTime, stale_after: Duration) {
    let Ok(entries) = vfs.list(root) else {
        return;
    };
    for (name, _) in entries {
        if !name.starts_with(STALE_LOCK_PREFIX) {
            continue;
        }
        let path = join(root, &name);
        let old = vfs
            .stat(&path)
            .ok()
            .and_then(|s| s.modified)
            .is_some_and(|m| now.duration_since(m).is_ok_and(|age| age > stale_after));
        if old {
            let _ = vfs.remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_round_trip_and_order() {
        let me = Identity {
            session: "01a14728-8400-7c3e-9b21-6f0d9a4e2b10".into(),
            user: "<os user>".into(),
            host: "<host>".into(),
            pid: 48211,
            pid_started: SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_222_918),
            app_version: "ElektraWatt 1.0.0".into(),
        };
        let rec = me.record(SystemTime::UNIX_EPOCH + Duration::from_secs(1_792_222_930));
        let text = rec.to_text();
        assert_eq!(
            text,
            "{\"lock\":1,\"session\":\"01a14728-8400-7c3e-9b21-6f0d9a4e2b10\",\"user\":\"<os user>\",\
             \"host\":\"<host>\",\"pid\":48211,\"pid_started\":\"2026-10-17T07:41:58Z\",\
             \"app_version\":\"ElektraWatt 1.0.0\",\"started_at\":\"2026-10-17T07:42:10Z\"}\n"
        );
        assert_eq!(LockRecord::parse(text.as_bytes()), Some(rec));
        assert_eq!(LockRecord::parse(b""), None);
        assert_eq!(
            LockRecord::parse(b"{\"lock\":2,\"session\":\"x\",\"pid\":1}"),
            None
        );
    }

    #[test]
    fn session_ids_are_v4_and_differ() {
        let a = new_session_id();
        let b = new_session_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(&a[14..15], "4");
        assert_eq!(sanitize(&a), a);
        assert_eq!(sanitize("../x"), "unknown");
    }
}
