// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Save protocol, `.~ew.lock`, journal, recovery, watcher, external-change diff, `.ewz` transport, as-built
//! archive, git commits and tags.
//!
//! `ew-store` v0 (WP-S1.09) implements the I/O half of the project format of
//! `docs/design/05-project-format-and-persistence.md` (doc 05; ADR-0010 with amendment 1) on top of
//! `ew-format`:
//!
//! - **Open** ([`Store::open`]): take the lock (or open read-only), roll an interrupted save forward or
//!   delete its temps, read the manifest and the listed files, verify, parse and check them, build the
//!   [`Model`](ew_model::Model), find a leftover journal (doc 05 §6.1).
//! - **Save** ([`Store::save`]): the protocol of doc 05 §5.2 with the commit point of §5.3: temps with
//!   `sync_all`, the durable intent manifest, directory syncs, `save-begin` in the journal, renames, the
//!   manifest rename, removals, `save-done`. Only the files of the dirty set are serialised, files equal to
//!   their hash are dropped, and an unchanged project writes nothing (NFR-REL-04, NFR-REL-16).
//! - **Lock** ([`lock`]): `.~ew.lock` with heartbeat, stale detection, read-only sessions and the takeover
//!   rename without replacement (doc 05 §7).
//! - **Journal** ([`journal`]): field-level diffs of every commit in the OS state directory, replayed onto
//!   the base generation after a crash (doc 05 §8).
//! - **I/O seam** ([`Vfs`]): every file operation goes through it; [`mem::MemVfs`] numbers, fails and
//!   "powers off" calls for the fault-injection harness (doc 05 §5.6).
//!
//! Not in v0: the watcher and the external-change merge (§9), `.ewz` (§12), the as-built archive (§14),
//! git integration (§11), journal snapshot slots (§8.1) and migrations (§10.2), which are WP-S1.17 and
//! later work packages.
//!
//! The save is synchronous and takes an immutable snapshot (`&Model`), so it can move to a writer thread
//! unchanged (NFR-PERF-12; budgets are measured at milestones).

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

/// The format layer, re-exported so that the crates above (`ew-api`) name its types without an edge of
/// their own (ARCH §3); it re-exports `ew-schema` in turn.
pub use ew_format;

pub mod clock;
pub mod journal;
pub mod lock;
pub mod mem;
mod rows;
mod store;
mod vfs;

pub use clock::{Clock, ManualClock, SystemClock};
pub use lock::{HEARTBEAT, Identity, LockInfo, LockRecord, OsProbe, ProcessProbe, ProcessState, STALE_AFTER};
pub use store::{
    Access, BASE_MEMBER, Changes, LockState, OpenError, OpenMode, OpenReport, Opened, READ_ONLY_POLL,
    READ_ONLY_WAIT, RENAME_BACKOFF, ReadOnlyReason, RecoverError, Recovered, SaveError, SaveReport, SaveStep,
    Store, StoreOptions, default_state_dir,
};
pub use vfs::{EntryKind, OsVfs, Stat, Vfs, is_transient, join};
