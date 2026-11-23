// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `library.lock`, the writer's lock of a `.ewlib` directory: the project lock's record and protocol
//! (`ew_store::lock`, doc 05 §7.1, §7.4) under the library's file names and rules ([`LIBRARY_LOCK`]), its outcomes
//! as [`LibraryError`]s. Readers never take it.
//!
//! - **Acquire:** a live lock of another session is [`LibraryError::Locked`]; a stale one is [`LibraryError::Stale`],
//!   and nothing is taken over without the user's confirmation.
//! - **Stale** (the project lock's rule): older than `stale_after`, or, on Linux only, on this host with a process
//!   that no longer runs; every lock of this process is live.
//! - **Take over** ([`take_over`], called only after the user confirmed): the stale lock is renamed aside to
//!   `library.lock.stale-<stale session>` and our own lock created; when another session's takeover moved the stale
//!   lock first, ours is an ordinary [`acquire`], so of two concurrent takeovers exactly one ends holding the lock.
//!   A takeover that took the lock deletes the renamed locks older than `stale_after`.

use std::path::Path;
use std::time::Duration;

use ew_store::lock::{
    Acquire, Identity, Lock, LockFile, LockInfo, OwnProcess, STALE_AFTER, StaleName, TAKEOVER_SETTLE,
    TakeoverError,
};
use ew_store::{Clock as _, OsProbe, OsVfs, SystemClock};

use crate::LibraryError;

/// The lock file's name.
pub const LOCK: &str = "library.lock";
/// The prefix of a stale lock renamed aside by a takeover.
pub const STALE_PREFIX: &str = "library.lock.stale-";

/// A library's lock file and its rules.
pub const LIBRARY_LOCK: LockFile = LockFile {
    name: LOCK,
    stale_prefix: STALE_PREFIX,
    own_process: OwnProcess::AllLive,
    stale_name: StaleName::WholeOrUnknown,
    retry_pending_delete: true,
};

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

/// This process as a registry's lock names it: a fresh session ID per registry, a UUIDv7 from `ew-ids`.
pub fn identity(app_version: &str) -> Identity {
    Identity {
        session: ew_ids::Id::<()>::new_v7().to_string(),
        ..Identity::current(app_version)
    }
}

/// The lock of library directory `dir` as `me` sees it under `policy`.
fn lock<'a>(dir: &'a Path, me: &'a Identity, policy: &LockPolicy) -> Lock<'a> {
    Lock {
        file: LIBRARY_LOCK,
        vfs: &OsVfs,
        dir,
        clock: &SystemClock,
        probe: &OsProbe,
        me,
        stale_after: policy.stale_after,
    }
}

/// The error for a lock held by someone else.
fn held_error(info: &LockInfo) -> LibraryError {
    let (holder, host, since) = info.record.as_ref().map_or_else(Default::default, |r| {
        (r.user.clone(), r.host.clone(), r.started_at.clone())
    });
    if info.stale {
        LibraryError::Stale {
            holder,
            host,
            last_seen: info.modified,
        }
    } else {
        LibraryError::Locked { holder, host, since }
    }
}

fn io_error(e: std::io::Error) -> LibraryError {
    if matches!(
        e.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::ReadOnlyFilesystem
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
    match lock(dir, me, policy).acquire().map_err(io_error)? {
        Acquire::Held => Ok(()),
        Acquire::HeldBy(info) => Err(held_error(&info)),
        Acquire::NotWritable(_) => Err(LibraryError::ReadOnly),
    }
}

/// Removes the lock of `dir` if it is ours.
pub fn release(dir: &Path, me: &Identity) -> Result<(), LibraryError> {
    match lock(dir, me, &LockPolicy::default()).release() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        r => r.map_err(io_error),
    }
}

/// Takes over a stale lock of `dir` (doc 05 §7.4), only after the user confirmed: a live lock is
/// [`LibraryError::Locked`]; with no lock this is [`acquire`].
pub fn take_over(dir: &Path, me: &Identity, policy: &LockPolicy) -> Result<(), LibraryError> {
    let lock = lock(dir, me, policy);
    let taken = lock.take_over(policy.settle).map_err(io_error)?;
    settled(&lock, taken, policy)
}

/// What a takeover's outcome `taken` means for the library: taken (the old renamed locks are then pruned), ours
/// already, held by another session, or, when there was no lock or another takeover moved the stale lock first, an
/// ordinary [`acquire`], which takes a lock nobody holds and names the winner of a race.
fn settled(
    lock: &Lock<'_>,
    taken: Result<LockInfo, TakeoverError>,
    policy: &LockPolicy,
) -> Result<(), LibraryError> {
    match taken {
        Ok(_) => {
            lock.prune_stale(SystemClock.now());
            Ok(())
        }
        Err(TakeoverError::NoLock | TakeoverError::Lost) => acquire(lock.dir, lock.me, policy),
        Err(TakeoverError::NotStale(info))
            if info.record.as_ref().is_some_and(|r| r.session == lock.me.session) =>
        {
            Ok(())
        }
        Err(TakeoverError::NotStale(info)) => Err(held_error(&info)),
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ew_store::lock::LockRecord;

    use super::*;

    fn now() -> SystemTime {
        SystemClock.now()
    }

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
    fn a_live_lock_is_locked_and_a_stale_one_is_stale() {
        let dir = ew_testdir::Tmp::new("ew-library-lock");
        let me = identity("test");
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
    }

    #[test]
    fn a_crashed_takeovers_leftover_is_pruned() {
        let dir = ew_testdir::Tmp::new("ew-library-lock-leftover");
        let me = identity("test");
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
    }

    /// B read the stale lock S, then A moved S aside to `stale-<S>` and created its live lock. B's
    /// rename meets the copy of S, but the lock no longer holds S: B must not prune the copy and rename A's live lock
    /// aside; it loses with `Locked` and A's lock stays as it is.
    #[test]
    fn a_takeover_that_lost_the_race_leaves_the_winners_lock_alone() {
        let dir = ew_testdir::Tmp::new("ew-library-lock-lost");
        plant(&dir, &other("elsewhere"), Duration::from_secs(3600));
        let path = dir.join(LOCK);
        let b = identity("test");
        let found = lock(&dir, &b, &quick()).read(now()).unwrap().unwrap();
        let found_bytes = std::fs::read(&path).unwrap();
        // A wins: S moved aside, A's lock created.
        let target = dir.join(format!("{STALE_PREFIX}{}", other("x").session));
        std::fs::rename(&path, &target).unwrap();
        let a = identity("test");
        acquire(&dir, &a, &quick()).unwrap();
        let a_bytes = std::fs::read(&path).unwrap();
        // B continues from its read of S.
        let lock = lock(&dir, &b, &quick());
        let taken = lock.displace(found, quick().settle).unwrap();
        match settled(&lock, taken, &quick()) {
            Err(LibraryError::Locked { .. }) => {}
            r => panic!("{r:?}"),
        }
        assert_eq!(std::fs::read(&path).unwrap(), a_bytes, "A's lock is untouched");
        assert_eq!(
            std::fs::read(&target).unwrap(),
            found_bytes,
            "the stale copy stays"
        );
    }

    /// The protocol assumes a contender's steps between its read of the stale lock and its create finish within the
    /// settle time (2 s in production). The race runs with 250 ms rather than [`quick`]'s 10 ms, which a loaded test
    /// runner can exceed between two system calls.
    #[test]
    fn of_two_concurrent_takeovers_exactly_one_wins() {
        let policy = LockPolicy {
            settle: Duration::from_millis(250),
            ..quick()
        };
        for round in 0..5 {
            let dir = ew_testdir::Tmp::new("ew-library-lock-race");
            plant(&dir, &other("elsewhere"), Duration::from_secs(3600));
            let barrier = Arc::new(Barrier::new(2));
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let (dir, barrier) = (dir.to_path_buf(), Arc::clone(&barrier));
                    std::thread::spawn(move || {
                        let me = identity("test");
                        barrier.wait();
                        (take_over(&dir, &me, &policy), me.session)
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
        }
    }
}
