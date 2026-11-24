// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The lock files of the program (doc 05 §7, ADR-0010 amendment 1, FR-COL-006, FR-COL-026): the project's
//! `.~ew.lock` ([`PROJECT_LOCK`]) and a library's `library.lock` (`ew-library`), one record and one protocol over the
//! [`Vfs`] ([`Lock`]). The rules in which the two differ are named in [`LockFile`].
//!
//! - **Acquisition:** `create_new` of the lock with the record, then `sync`; no byte-range locks.
//! - **Held:** the holder's session ID is in the file. Another opener is read-only and names the holder.
//! - **Heartbeat** (the project's lock): the holder sets the lock's modification time every [`HEARTBEAT`] and
//!   re-reads the session ID; a different or missing ID means the lock is lost (the session turns read-only).
//! - **Stale:** older than `stale_after` (default [`STALE_AFTER`]), or on the same host with a dead PID or a
//!   PID whose start time differs; a lock of this process as [`LockFile::own_process`] says.
//! - **Takeover** (confirmed by the caller): rename the stale lock **without replacement** to
//!   `<stale prefix><stale session>`, create our own lock, re-read it after the settle time ([`TAKEOVER_SETTLE`]).

use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use serde_json::Value;

use crate::clock::{Clock, rfc3339};
use crate::vfs::{Vfs, join};
use ew_format::{LOCK, STALE_LOCK_PREFIX};

/// The heartbeat interval (doc 05 §7.2).
pub const HEARTBEAT: Duration = Duration::from_secs(5 * 60);
/// The default stale age, `lock.stale_after` (doc 05 §7.4).
pub const STALE_AFTER: Duration = Duration::from_secs(60 * 60);
/// The wait before a takeover re-reads its new lock (doc 05 §7.4 step 2).
pub const TAKEOVER_SETTLE: Duration = Duration::from_secs(2);

/// One lock file of the program and the rules in which the project's lock and a library's differ.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LockFile {
    /// The lock file's name in its directory.
    pub name: &'static str,
    /// The name of a stale lock a takeover renamed aside, before the stale session.
    pub stale_prefix: &'static str,
    /// How the stale rule judges a lock of this process.
    pub own_process: OwnProcess,
    /// How the stale session becomes part of the renamed lock's name.
    pub stale_name: StaleName,
    /// Whether a read, a create or a rename that Windows refuses with access denied is tried again for a short while:
    /// Windows reports a file whose deletion is pending (another session's takeover just unlinked it while a handle
    /// was open) that way, which is contention, not a read-only directory. Elsewhere every call runs once.
    pub retry_pending_delete: bool,
    /// What a takeover makes of a lock file that another contender moved between two of its steps.
    pub moved_away: MovedAway,
}

/// What a takeover makes of a lock file that another contender renamed or removed between two of its steps: the
/// stale lock gone before the plain rename of doc 05 §7.4 step 3 (on Windows the rename without replacement is also
/// refused while the other contender's unlink of the stale lock is pending), and its own new lock gone before it is
/// synced.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MovedAway {
    /// An I/O error of the takeover; the plain rename replaces whatever is at the target (the project's lock).
    IsAnError,
    /// The takeover is lost; the plain rename is lost too when the target exists (a library's lock).
    IsLost,
}

/// How the stale rule judges a lock that names this process's PID on this host.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OwnProcess {
    /// A lock of another session of this process is live; this session's own lock is judged by the process probe
    /// (the project's lock).
    OtherSessionsLive,
    /// Every lock of this process is live (a library's lock).
    AllLive,
}

/// How a takeover turns the stale lock's session into the renamed lock's name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StaleName {
    /// Its hexadecimal digits and hyphens, lower-cased, at most 36; `unknown` when none is left (the project's lock).
    Filtered,
    /// The session unchanged when it holds only hexadecimal digits and hyphens, else `unknown` (a library's lock).
    WholeOrUnknown,
}

/// The project's lock `.~ew.lock` (doc 05 §7).
pub const PROJECT_LOCK: LockFile = LockFile {
    name: LOCK,
    stale_prefix: STALE_LOCK_PREFIX,
    own_process: OwnProcess::OtherSessionsLive,
    stale_name: StaleName::Filtered,
    retry_pending_delete: false,
    moved_away: MovedAway::IsAnError,
};

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

/// Who holds the lock now, as seen by the session that asks.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Holder {
    /// Our session.
    Us,
    /// Another session (`None`: a lock file that is not a readable record).
    Other(Option<LockRecord>),
    /// No lock file.
    Nobody,
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

/// A lock of one directory as one session sees it: the lock file and its rules, the directory, the seams it is read
/// and written through, who asks and the stale age (`lock.stale_after`).
#[derive(Clone, Copy, Debug)]
pub struct Lock<'a> {
    /// The lock file and its rules.
    pub file: LockFile,
    /// The file system.
    pub vfs: &'a dyn Vfs,
    /// The directory that holds the lock.
    pub dir: &'a Path,
    /// The clock of the record's times, the stale age and the settle wait.
    pub clock: &'a dyn Clock,
    /// Process liveness for the same-host stale rule.
    pub probe: &'a dyn ProcessProbe,
    /// Who asks.
    pub me: &'a Identity,
    /// A lock not refreshed for longer is stale.
    pub stale_after: Duration,
}

impl Lock<'_> {
    /// The lock file's path.
    fn path(&self) -> PathBuf {
        join(self.dir, self.file.name)
    }

    /// Runs `op`, retried for a short while on a Windows access-denied when [`LockFile::retry_pending_delete`] is set;
    /// else once.
    fn attempt<T>(&self, mut op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
        let mut tries = 0;
        loop {
            match op() {
                Err(e)
                    if self.file.retry_pending_delete && cfg!(windows) && access_denied(&e) && tries < 20 =>
                {
                    tries += 1;
                    self.clock.sleep(Duration::from_millis(5));
                }
                r => return r,
            }
        }
    }

    /// The stale rule of doc 05 §7.4 at `now`, a lock of this process judged as [`LockFile::own_process`] says.
    pub fn is_stale(
        &self,
        record: Option<&LockRecord>,
        modified: Option<SystemTime>,
        now: SystemTime,
    ) -> bool {
        let old = modified.is_some_and(|m| now.duration_since(m).is_ok_and(|age| age > self.stale_after));
        if old {
            return true;
        }
        let Some(record) = record else {
            return false;
        };
        let own_live = match self.file.own_process {
            OwnProcess::OtherSessionsLive => record.pid == self.me.pid && record.session != self.me.session,
            OwnProcess::AllLive => record.pid == self.me.pid,
        };
        if record.host != self.me.host || own_live {
            return false;
        }
        match self.probe.probe(record.pid) {
            ProcessState::Dead => true,
            ProcessState::Alive { started: Some(t) } => rfc3339(t) != record.pid_started,
            ProcessState::Alive { started: None } | ProcessState::Unknown => false,
        }
    }

    /// Reads the lock, judged stale or not at `now`; `Ok(None)` when there is none.
    pub fn read(&self, now: SystemTime) -> io::Result<Option<LockInfo>> {
        let path = self.path();
        let bytes = match self.attempt(|| self.vfs.read(&path)) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let modified = self.vfs.stat(&path).ok().and_then(|s| s.modified);
        let record = LockRecord::parse(&bytes);
        let stale = self.is_stale(record.as_ref(), modified, now);
        Ok(Some(LockInfo {
            record,
            modified,
            stale,
        }))
    }

    /// Creates the lock with our record and syncs it (doc 05 §7.1).
    pub fn acquire(&self) -> io::Result<Acquire> {
        let path = self.path();
        let now = self.clock.now();
        let record = self.me.record(now).to_text();
        match self.attempt(|| self.vfs.create_new(&path, record.as_bytes())) {
            Ok(()) => {
                self.vfs.sync(&path)?;
                self.vfs.sync_dir(self.dir)?;
                Ok(Acquire::Held)
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => match self.read(now)? {
                Some(info) if info.record.as_ref().is_some_and(|r| r.session == self.me.session) => {
                    Ok(Acquire::Held)
                }
                Some(info) => Ok(Acquire::HeldBy(info)),
                // Released between our create and our read: try once more.
                None => match self.attempt(|| self.vfs.create_new(&path, record.as_bytes())) {
                    Ok(()) => {
                        self.vfs.sync(&path)?;
                        Ok(Acquire::Held)
                    }
                    Err(e) => Err(e),
                },
            },
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

    /// Re-reads the lock's session ID (doc 05 §7.2).
    pub fn holder(&self) -> io::Result<Holder> {
        match self.attempt(|| self.vfs.read(&self.path())) {
            Ok(bytes) => {
                let record = LockRecord::parse(&bytes);
                if record.as_ref().is_some_and(|r| r.session == self.me.session) {
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
    pub fn release(&self) -> io::Result<()> {
        if self.holder()? == Holder::Us {
            self.vfs.remove_file(&self.path())?;
        }
        Ok(())
    }

    /// Takes over a stale lock (doc 05 §7.4): reads it and, when it is stale, [`Lock::displace`]s it.
    pub fn take_over(&self, settle: Duration) -> io::Result<Result<LockInfo, TakeoverError>> {
        let Some(info) = self.read(self.clock.now())? else {
            return Ok(Err(TakeoverError::NoLock));
        };
        if !info.stale {
            return Ok(Err(TakeoverError::NotStale(info)));
        }
        self.displace(info, settle)
    }

    /// Steps 1 to 3 of the takeover (doc 05 §7.4) of the stale lock `info` as read: rename it without replacement to
    /// `<stale prefix><stale session>`, create our own lock, wait `settle` and re-read it.
    pub fn displace(&self, info: LockInfo, settle: Duration) -> io::Result<Result<LockInfo, TakeoverError>> {
        let stale_session = info
            .record
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |r| self.file.stale_name.of(&r.session));
        let from = self.path();
        let to = join(self.dir, &format!("{}{stale_session}", self.file.stale_prefix));
        match self.rename_away(&from, &to)? {
            Ok(()) => {}
            Err(TakeoverError::Lost) if self.left_by_a_crashed_takeover(&from, &to, &info) => {
                // A takeover that crashed between the hard link and the unlink of `rename_noreplace` left the
                // stale copy beside the lock, both with the stale record: every later takeover would lose to
                // it. The copy is redundant (the lock still holds the record): prune it
                // and try once more; a racing instance that wins instead makes this one lose as usual.
                match self.vfs.remove_file(&to) {
                    Ok(()) => {}
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
                if let Err(lost) = self.rename_away(&from, &to)? {
                    return Ok(Err(lost));
                }
            }
            Err(lost) => return Ok(Err(lost)),
        }
        // The renamed record keeps the holder's last heartbeat as its modification time; restamp it so that
        // "older than `stale_after`" (doc 05 §7.4, last paragraph) counts from the takeover, and the takeover's
        // own open does not prune the evidence at once.
        let _ = self.vfs.set_modified(&to, self.clock.now());
        let record = self.me.record(self.clock.now()).to_text();
        match self.attempt(|| self.vfs.create_new(&from, record.as_bytes())) {
            Ok(()) => match self.vfs.sync(&from) {
                Ok(()) => {}
                Err(e)
                    if e.kind() == io::ErrorKind::NotFound && self.file.moved_away == MovedAway::IsLost =>
                {
                    return Ok(Err(TakeoverError::Lost));
                }
                Err(e) => return Err(e),
            },
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(Err(TakeoverError::Lost)),
            Err(e) => return Err(e),
        }
        self.vfs.sync_dir(self.dir)?;
        self.clock.sleep(settle);
        if self.holder()? != Holder::Us {
            return Ok(Err(TakeoverError::Lost));
        }
        Ok(Ok(info))
    }

    /// Step 1 of the takeover (doc 05 §7.4): renames the stale lock `from` to `to` without replacement;
    /// `Lost` when `to` exists or `from` is gone (another instance was faster).
    fn rename_away(&self, from: &Path, to: &Path) -> io::Result<Result<(), TakeoverError>> {
        match self.attempt(|| self.vfs.rename_noreplace(from, to)) {
            Ok(()) => Ok(Ok(())),
            Err(e) if matches!(e.kind(), io::ErrorKind::AlreadyExists | io::ErrorKind::NotFound) => {
                Ok(Err(TakeoverError::Lost))
            }
            // Doc 05 §7.4 step 3: no rename without replacement on this file system; the plain rename and the
            // re-read after the settle time (plus the heartbeat's session check) detect the loser.
            Err(e) if e.kind() == io::ErrorKind::Unsupported => match self.file.moved_away {
                MovedAway::IsAnError => self.vfs.rename(from, to).map(Ok),
                MovedAway::IsLost => {
                    if self.vfs.stat(to).is_ok() {
                        return Ok(Err(TakeoverError::Lost));
                    }
                    match self.vfs.rename(from, to) {
                        Ok(()) => Ok(Ok(())),
                        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Err(TakeoverError::Lost)),
                        Err(e) => Err(e),
                    }
                }
            },
            Err(e) => Err(e),
        }
    }

    /// Whether the existing stale copy `to` is the leftover of a takeover that crashed after linking the lock
    /// to it and before unlinking the lock: the copy holds the same bytes as the lock `from`, which still holds
    /// the stale record of `info`. Pruning such a copy loses nothing.
    fn left_by_a_crashed_takeover(&self, from: &Path, to: &Path, info: &LockInfo) -> bool {
        let (Ok(lock), Ok(copy)) = (self.vfs.read(from), self.vfs.read(to)) else {
            return false;
        };
        lock == copy && LockRecord::parse(&lock) == info.record
    }

    /// Deletes the stale locks a takeover renamed aside that are older than `stale_after` at `now` (doc 05 §7.4,
    /// last paragraph).
    pub fn prune_stale(&self, now: SystemTime) {
        let Ok(entries) = self.vfs.list(self.dir) else {
            return;
        };
        for (name, _) in entries {
            if !name.starts_with(self.file.stale_prefix) {
                continue;
            }
            let path = join(self.dir, &name);
            let old = self
                .vfs
                .stat(&path)
                .ok()
                .and_then(|s| s.modified)
                .is_some_and(|m| now.duration_since(m).is_ok_and(|age| age > self.stale_after));
            if old {
                let _ = self.vfs.remove_file(&path);
            }
        }
    }
}

/// Whether `e` is an access-denied, also where [`Vfs::rename_noreplace`] reports one as `Unsupported` (the hard link
/// refused).
fn access_denied(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied
        || (e.kind() == io::ErrorKind::Unsupported
            && e.get_ref()
                .and_then(|inner| inner.downcast_ref::<io::Error>())
                .is_some_and(|inner| inner.kind() == io::ErrorKind::PermissionDenied))
}

impl StaleName {
    /// The name part of the stale session `session`.
    fn of(self, session: &str) -> String {
        match self {
            StaleName::Filtered => {
                let s: String = session
                    .chars()
                    .filter(|c| c.is_ascii_hexdigit() || *c == '-')
                    .map(|c| c.to_ascii_lowercase())
                    .take(36)
                    .collect();
                if s.is_empty() { "unknown".to_owned() } else { s }
            }
            StaleName::WholeOrUnknown => {
                if !session.is_empty() && session.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
                    session.to_owned()
                } else {
                    "unknown".to_owned()
                }
            }
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
        assert_eq!(StaleName::Filtered.of(&a), a);
        assert_eq!(StaleName::Filtered.of("../x"), "unknown");
    }
}
