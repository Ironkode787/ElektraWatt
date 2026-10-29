// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `library.lock`, the writer's lock of a `.ewlib` directory (WP-S1.19 §7.2), with the project lock's record and
//! stale rule (doc 05 §7.1, §7.4). Readers never take it.
//!
//! - **Acquire:** `create_new` of `library.lock` with the record, then sync. A live lock of another session is
//!   [`LibraryError::Locked`]; a stale one is [`LibraryError::Stale`], and nothing is taken over without the user's
//!   confirmation.
//! - **Stale** (as built for the project lock, decision log ID-216): older than `stale_after`, or, on Linux only, on
//!   this host with a process that no longer runs.
//! - **Take over** ([`take_over`], called only after the user confirmed): rename the stale lock **without
//!   replacement** to `library.lock.stale-<stale session>` (a hard link, then the unlink; a link target that exists
//!   with the lock's bytes is a crashed takeover's leftover, pruned before one more try), restamp the renamed file's
//!   modification time, `create_new` our own lock and re-read it after the settle time. Of two concurrent takeovers
//!   exactly one ends holding the lock: `create_new` admits one.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::LibraryError;

/// The lock file's name.
pub const LOCK: &str = "library.lock";
/// The prefix of a stale lock renamed aside by a takeover.
pub const STALE_PREFIX: &str = "library.lock.stale-";
/// The default `lock.stale_after` (doc 05 §7.4): 60 minutes.
pub const STALE_AFTER: Duration = Duration::from_secs(60 * 60);
/// The wait before a takeover re-reads its new lock (doc 05 §7.4 step 2).
pub const TAKEOVER_SETTLE: Duration = Duration::from_secs(2);

/// The lock timing: `lock.stale_after` and the takeover's settle wait.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LockPolicy {
    /// A lock not refreshed for longer is stale.
    pub stale_after: Duration,
    /// The wait before a takeover re-reads its new lock.
    pub settle: Duration,
}

impl Default for LockPolicy {
    fn default() -> LockPolicy {
        LockPolicy {
            stale_after: STALE_AFTER,
            settle: TAKEOVER_SETTLE,
        }
    }
}

/// The wall clock of the lock record, which is not library data (doc 05 §7.1: the lock may carry times).
// The lock record's times and the stale age; no library byte depends on them.
#[allow(clippy::disallowed_methods)]
pub(crate) fn now() -> SystemTime {
    SystemTime::now()
}

/// `YYYY-MM-DDThh:mm:ssZ`.
pub(crate) fn rfc3339(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

/// The lock record of doc 05 §7.1.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LockRecord {
    /// The session ID, the ownership key.
    pub session: String,
    /// The OS user.
    pub user: String,
    /// The host name.
    pub host: String,
    /// The process ID.
    pub pid: u32,
    /// The process's start time, RFC 3339.
    pub pid_started: String,
    /// The program and version.
    pub app_version: String,
    /// When the session took the lock, RFC 3339.
    pub started_at: String,
}

impl LockRecord {
    /// The compact text, members in the order of doc 05 §7.1, with a final LF.
    pub fn to_text(&self) -> String {
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
        let body: Vec<String> = members
            .iter()
            .map(|(k, v)| format!("{}:{v}", Value::from(*k)))
            .collect();
        format!("{{{}}}\n", body.join(","))
    }

    /// Reads a record; `None` for a file that is not one.
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

/// Who this registry is, for the lock record.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Identity {
    /// A fresh session ID per registry.
    pub session: String,
    /// The OS user.
    pub user: String,
    /// The host name.
    pub host: String,
    /// The process ID.
    pub pid: u32,
    /// The process's start time (approximated by the first call in this process).
    pub pid_started: SystemTime,
    /// The program and version.
    pub app_version: String,
}

/// The first non-empty of the environment variables `names`, trimmed.
// The user and host names of the lock record, which is not library data.
#[allow(clippy::disallowed_methods)]
fn env_text(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|n| std::env::var(n).ok().filter(|v| !v.trim().is_empty()))
        .map(|v| v.trim().to_owned())
}

impl Identity {
    /// This process with a fresh session ID (a UUIDv7 from `ew-ids`).
    pub fn current(app_version: &str) -> Identity {
        static STARTED: OnceLock<SystemTime> = OnceLock::new();
        let host = env_text(&["COMPUTERNAME", "HOSTNAME"])
            .or_else(|| {
                std::fs::read_to_string("/etc/hostname")
                    .ok()
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| "localhost".to_owned());
        Identity {
            session: ew_ids::Id::<()>::new_v7().to_string(),
            user: env_text(&["USERNAME", "USER", "LOGNAME"]).unwrap_or_else(|| "unknown".to_owned()),
            host,
            pid: std::process::id(),
            pid_started: *STARTED.get_or_init(now),
            app_version: app_version.to_owned(),
        }
    }

    /// The record of this identity taken at `at`.
    pub fn record(&self, at: SystemTime) -> LockRecord {
        LockRecord {
            session: self.session.clone(),
            user: self.user.clone(),
            host: self.host.clone(),
            pid: self.pid,
            pid_started: rfc3339(self.pid_started),
            app_version: self.app_version.clone(),
            started_at: rfc3339(at),
        }
    }
}

/// The stale rule of doc 05 §7.4 as built (ID-216): age, or on Linux a dead process on this host.
fn is_stale(
    record: Option<&LockRecord>,
    modified: Option<SystemTime>,
    policy: &LockPolicy,
    me: &Identity,
) -> bool {
    let old = modified.is_some_and(|m| now().duration_since(m).is_ok_and(|age| age > policy.stale_after));
    if old {
        return true;
    }
    let Some(r) = record else {
        return false;
    };
    if r.host != me.host || r.pid == me.pid || !cfg!(target_os = "linux") {
        return false;
    }
    !Path::new(&format!("/proc/{}", r.pid)).exists()
}

/// The lock as found.
struct Found {
    bytes: Vec<u8>,
    record: Option<LockRecord>,
    modified: Option<SystemTime>,
}

fn read(path: &Path) -> io::Result<Option<Found>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
    Ok(Some(Found {
        record: LockRecord::parse(&bytes),
        bytes,
        modified,
    }))
}

/// The error for a lock held by someone else.
fn held_error(found: &Found, policy: &LockPolicy, me: &Identity) -> LibraryError {
    let r = found.record.clone().unwrap_or(LockRecord {
        session: String::new(),
        user: String::new(),
        host: String::new(),
        pid: 0,
        pid_started: String::new(),
        app_version: String::new(),
        started_at: String::new(),
    });
    if is_stale(found.record.as_ref(), found.modified, policy, me) {
        LibraryError::Stale {
            holder: r.user,
            host: r.host,
            last_seen: found.modified,
        }
    } else {
        LibraryError::Locked {
            holder: r.user,
            host: r.host,
            since: r.started_at,
        }
    }
}

fn create(path: &Path, me: &Identity) -> io::Result<()> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(me.record(now()).to_text().as_bytes())?;
    f.sync_all()
}

fn io_error(e: io::Error) -> LibraryError {
    if matches!(
        e.kind(),
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem
    ) {
        LibraryError::ReadOnly
    } else {
        LibraryError::Io(e.to_string())
    }
}

/// Takes the lock of library directory `dir` for `me`: `Ok` when created or already ours, [`LibraryError::Locked`]
/// for a live lock of another session, [`LibraryError::Stale`] for a stale one, [`LibraryError::ReadOnly`] when the
/// directory refuses the file.
pub fn acquire(dir: &Path, me: &Identity, policy: &LockPolicy) -> Result<(), LibraryError> {
    let path = dir.join(LOCK);
    for _ in 0..2 {
        match create(&path, me) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => match read(&path).map_err(io_error)? {
                Some(f) if f.record.as_ref().is_some_and(|r| r.session == me.session) => return Ok(()),
                Some(f) => return Err(held_error(&f, policy, me)),
                // Released between our create and our read: try once more.
                None => continue,
            },
            Err(e) => return Err(io_error(e)),
        }
    }
    Err(LibraryError::Io(
        "the lock changed twice while it was taken".to_owned(),
    ))
}

/// Removes the lock of `dir` if it is ours.
pub fn release(dir: &Path, me: &Identity) -> Result<(), LibraryError> {
    let path = dir.join(LOCK);
    if let Some(f) = read(&path).map_err(io_error)?
        && f.record.is_some_and(|r| r.session == me.session)
    {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error(e)),
        }
    }
    Ok(())
}

/// Renames `from` to `to` without replacing an existing `to`: a hard link, then the unlink of `from`. Where the file
/// system has no hard links, the plain rename (doc 05 §7.4 step 3; the re-read detects the loser).
fn rename_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    match std::fs::hard_link(from, to) {
        Ok(()) => match std::fs::remove_file(from) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        },
        Err(e) if matches!(e.kind(), io::ErrorKind::AlreadyExists | io::ErrorKind::NotFound) => Err(e),
        Err(_) => {
            if to.exists() {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
            std::fs::rename(from, to)
        }
    }
}

/// Takes over a stale lock of `dir` (doc 05 §7.4), only after the user confirmed: a live lock is
/// [`LibraryError::Locked`]; with no lock this is [`acquire`].
pub fn take_over(dir: &Path, me: &Identity, policy: &LockPolicy) -> Result<(), LibraryError> {
    let path = dir.join(LOCK);
    let Some(found) = read(&path).map_err(io_error)? else {
        return acquire(dir, me, policy);
    };
    if found.record.as_ref().is_some_and(|r| r.session == me.session) {
        return Ok(());
    }
    if !is_stale(found.record.as_ref(), found.modified, policy, me) {
        return Err(held_error(&found, policy, me));
    }
    let stale_session = found
        .record
        .as_ref()
        .map(|r| r.session.clone())
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'))
        .unwrap_or_else(|| "unknown".to_owned());
    let target: PathBuf = dir.join(format!("{STALE_PREFIX}{stale_session}"));
    let mut renamed = false;
    for _ in 0..2 {
        match rename_no_replace(&path, &target) {
            Ok(()) => {
                renamed = true;
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                // A crashed takeover left the same bytes under the target: redundant, pruned, one more try.
                if std::fs::read(&target).is_ok_and(|b| b == found.bytes) {
                    let _ = std::fs::remove_file(&target);
                    continue;
                }
                break;
            }
            Err(e) => return Err(io_error(e)),
        }
    }
    if renamed {
        // Restamp: the age rule of the renamed file counts from the takeover (ID-210).
        if let Ok(f) = File::options().write(true).open(&target) {
            let _ = f.set_modified(now());
        }
    }
    match create(&path, me) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return match read(&path).map_err(io_error)? {
                Some(f) => Err(held_error(&f, policy, me)),
                None => Err(LibraryError::Io(
                    "the lock changed during the takeover".to_owned(),
                )),
            };
        }
        Err(e) => return Err(io_error(e)),
    }
    std::thread::sleep(policy.settle);
    match read(&path).map_err(io_error)? {
        Some(f) if f.record.as_ref().is_some_and(|r| r.session == me.session) => {
            prune_stale(dir, policy);
            Ok(())
        }
        Some(f) => Err(held_error(&f, policy, me)),
        None => Err(LibraryError::Io(
            "the lock disappeared during the takeover".to_owned(),
        )),
    }
}

/// Deletes `library.lock.stale-*` files older than `stale_after` (counted from their takeover).
fn prune_stale(dir: &Path, policy: &LockPolicy) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        if !name.to_string_lossy().starts_with(STALE_PREFIX) {
            continue;
        }
        let old = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .is_some_and(|m| now().duration_since(m).is_ok_and(|age| age > policy.stale_after));
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::*;

    fn other(host: &str) -> Identity {
        Identity {
            session: "0192f3a4-5b6c-7d8e-8f01-23456789abd0".into(),
            user: "someone".into(),
            host: host.into(),
            pid: 4_000_000,
            pid_started: UNIX_EPOCH,
            app_version: "ElektraWatt 0.0.0".into(),
        }
    }

    fn quick() -> LockPolicy {
        LockPolicy {
            stale_after: Duration::from_secs(60),
            settle: Duration::from_millis(10),
        }
    }

    /// Writes `who`'s lock into `dir` with its modification time `age` ago.
    fn plant(dir: &Path, who: &Identity, age: Duration) {
        std::fs::write(dir.join(LOCK), who.record(now()).to_text()).unwrap();
        let f = File::options().write(true).open(dir.join(LOCK)).unwrap();
        f.set_modified(now() - age).unwrap();
    }

    #[test]
    fn records_round_trip() {
        let r = other("h").record(UNIX_EPOCH);
        assert_eq!(LockRecord::parse(r.to_text().as_bytes()), Some(r.clone()));
        assert!(r.to_text().starts_with("{\"lock\":1,\"session\":"));
        assert_eq!(LockRecord::parse(b"{}"), None);
        assert_eq!(
            rfc3339(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
    }

    #[test]
    fn a_live_lock_is_locked_and_a_stale_one_is_stale() {
        let dir = ew_testdir::fresh_dir("ew-library-lock");
        let me = Identity::current("test");
        plant(&dir, &other("elsewhere"), Duration::from_secs(1));
        match acquire(&dir, &me, &quick()) {
            Err(LibraryError::Locked { holder, host, .. }) => {
                assert_eq!((holder.as_str(), host.as_str()), ("someone", "elsewhere"));
            }
            r => panic!("{r:?}"),
        }
        // A live lock is not taken over.
        assert!(matches!(
            take_over(&dir, &me, &quick()),
            Err(LibraryError::Locked { .. })
        ));
        plant(&dir, &other("elsewhere"), Duration::from_secs(3600));
        match acquire(&dir, &me, &quick()) {
            Err(LibraryError::Stale {
                holder, last_seen, ..
            }) => {
                assert_eq!(holder, "someone");
                assert!(last_seen.is_some());
            }
            r => panic!("{r:?}"),
        }
        // The confirmed takeover: renamed aside, our lock created.
        take_over(&dir, &me, &quick()).unwrap();
        let ours = LockRecord::parse(&std::fs::read(dir.join(LOCK)).unwrap()).unwrap();
        assert_eq!(ours.session, me.session);
        let aside = dir.join(format!("{STALE_PREFIX}{}", other("x").session));
        assert!(aside.is_file());
        // Restamped to the takeover time.
        let age = now()
            .duration_since(std::fs::metadata(&aside).unwrap().modified().unwrap())
            .unwrap_or_default();
        assert!(age < Duration::from_secs(600), "{age:?}");
        // Ours: acquire again is fine; release removes it.
        acquire(&dir, &me, &quick()).unwrap();
        release(&dir, &me).unwrap();
        assert!(!dir.join(LOCK).exists());
        // No lock: a takeover is an ordinary acquire.
        take_over(&dir, &me, &quick()).unwrap();
        release(&dir, &me).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_crashed_takeovers_leftover_is_pruned() {
        let dir = ew_testdir::fresh_dir("ew-library-lock-leftover");
        let me = Identity::current("test");
        plant(&dir, &other("elsewhere"), Duration::from_secs(3600));
        let bytes = std::fs::read(dir.join(LOCK)).unwrap();
        let target = dir.join(format!("{STALE_PREFIX}{}", other("x").session));
        std::fs::write(&target, &bytes).unwrap();
        take_over(&dir, &me, &quick()).unwrap();
        assert_eq!(
            LockRecord::parse(&std::fs::read(dir.join(LOCK)).unwrap())
                .unwrap()
                .session,
            me.session
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn of_two_concurrent_takeovers_exactly_one_wins() {
        for round in 0..5 {
            let dir = ew_testdir::fresh_dir("ew-library-lock-race");
            plant(&dir, &other("elsewhere"), Duration::from_secs(3600));
            let barrier = Arc::new(Barrier::new(2));
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let (dir, barrier) = (dir.clone(), Arc::clone(&barrier));
                    std::thread::spawn(move || {
                        let me = Identity::current("test");
                        barrier.wait();
                        (take_over(&dir, &me, &quick()), me.session)
                    })
                })
                .collect();
            let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            let winners: Vec<&String> = results
                .iter()
                .filter(|(r, _)| r.is_ok())
                .map(|(_, s)| s)
                .collect();
            assert_eq!(winners.len(), 1, "round {round}: {results:?}");
            let holder = LockRecord::parse(&std::fs::read(dir.join(LOCK)).unwrap()).unwrap();
            assert_eq!(&holder.session, winners[0]);
            for (r, _) in &results {
                if let Err(e) = r {
                    assert!(matches!(e, LibraryError::Locked { .. }), "{e:?}");
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
