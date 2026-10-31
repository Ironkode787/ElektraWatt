// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Recovery with more than one crashed session's journal (doc 05 §8.2, WP-S1.22 part 3, ruling 2 of cycle 41):
//! recovery acts on one journal at a time, the newest, and never deletes a journal it did not replay or discard.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use common::{edit_device, mem_project_of, options, root, small_fixture};
use ew_model::{AnyKey, Model};
use ew_store::journal::ReplayError;
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{
    Access, Changes, ManualClock, Opened, PurgeError, ReadOnlyReason, RecoverError, Store, StoreOptions,
};

/// Store options for process `pid` whose clock reads `secs` seconds after the fixed test time, so that each
/// crashed session's last change has its own time (the newest journal is the one offered).
fn options_at(vfs: &Arc<MemVfs>, pid: u32, secs: u64) -> StoreOptions {
    let mut o = options(vfs, pid);
    o.clock = Arc::new(ManualClock::new(
        UNIX_EPOCH + Duration::from_secs(1_792_222_930 + secs),
    ));
    o
}

/// Opens for editing as process `pid` at `secs`; the stale lock of a crashed (dead) process is taken over.
fn open_at(vfs: &Arc<MemVfs>, pid: u32, secs: u64) -> Opened {
    let opened = Store::open(&root(), options_at(vfs, pid, secs)).expect("the project opens");
    if let Access::ReadOnly(ReadOnlyReason::LockedBy(info)) = &opened.report.access {
        assert!(info.stale, "only crashed processes hold the lock here");
        drop(opened);
        let taken =
            Store::take_over(&root(), options_at(vfs, pid, secs)).expect("the stale lock is taken over");
        assert_eq!(taken.report.access, Access::Edit);
        return taken;
    }
    assert_eq!(opened.report.access, Access::Edit);
    opened
}

/// The journal segments in the state directory, by file name.
fn journals(vfs: &MemVfs) -> BTreeMap<String, Vec<u8>> {
    vfs.files(Path::new("/state"))
        .into_iter()
        .filter(|(name, _)| name.ends_with(".jsonl"))
        .collect()
}

/// A session that commits one edit of the `nth` device (function text `text`) under the command name `cmd`, and
/// crashes; returns the edited model and its journal segment (name and bytes).
fn crashed_session(
    vfs: &Arc<MemVfs>,
    pid: u32,
    secs: u64,
    nth: usize,
    text: &str,
    cmd: &str,
) -> (Model, String) {
    let before = journals(vfs);
    let Opened { mut store, model, .. } = open_at(vfs, pid, secs);
    let (edited, keys) = edit_device(&model, nth, text);
    let keys: Vec<AnyKey> = keys.into_iter().collect();
    store
        .journal_commit(&model, &edited, &keys, cmd, "test")
        .expect("the journal accepts the commit");
    store.crash();
    let after = journals(vfs);
    let new: Vec<&String> = after.keys().filter(|name| !before.contains_key(*name)).collect();
    assert_eq!(new.len(), 1, "one new segment: {:?}", after.keys());
    (edited, new[0].clone())
}

/// Two crashed sessions with distinct changes: session A (older) edits the first device, session B (newer) the
/// second. B opened after A's crash and kept A's journal (it neither recovered nor discarded it).
struct TwoCrashes {
    vfs: Arc<MemVfs>,
    disk: Model,
    a: (Model, String),
    b: (Model, String),
}

fn two_crashes() -> TwoCrashes {
    let (vfs, _) = mem_project_of(DirSync::Explicit, small_fixture());
    let Opened {
        store, model: disk, ..
    } = Store::open(&root(), options_at(&vfs, 9, 0)).expect("opens");
    store.close();
    let a = crashed_session(&vfs, 1, 10, 0, "edited by A", "test.edit_a");
    let b = crashed_session(&vfs, 2, 20, 1, "edited by B", "test.edit_b");
    assert!(!a.0.content_eq(&b.0) && !a.0.content_eq(&disk) && !b.0.content_eq(&disk));
    TwoCrashes { vfs, disk, a, b }
}

/// Ruling 2 (cycle 41): "`Store::recover` replays the summarised segment (the newest, as now) and removes **only that
/// segment** ... Every other non-empty leftover stays on disk, is offered at the next open (newest first, one at a
/// time)". And: "Replaying a segment whose base is not the open generation behaves exactly as the existing code does
/// for that case today (refused or applied); do not invent a merge." Here the second journal's base (generation 1)
/// is not the generation on disk after the first recovery's save (2): it replays with the collision check, and with
/// distinct changes it applies.
#[test]
fn recovery_replays_the_newest_journal_and_keeps_the_other_unchanged() {
    let TwoCrashes { vfs, disk, a, b } = two_crashes();
    let a_bytes = journals(&vfs)[&a.1].clone();

    // The next open offers B's journal (the newest) and says that one more remains.
    let mut c = open_at(&vfs, 3, 30);
    let info = c.report.recovery.clone().expect("a crashed session's journal");
    assert_eq!(info.labels, vec!["test.edit_b".to_owned()]);
    assert_eq!(info.transactions, 1);
    assert_eq!(info.further, 1);
    assert!(info.base_matches);
    assert!(c.model.content_eq(&disk), "the disk state opens");
    let rec = c.store.recover(&c.model).expect("B's journal replays");
    assert_eq!(rec.transactions, 1);
    assert!(rec.model.content_eq(&b.0), "B's change and only B's");
    assert!(c.store.recovery().is_none());
    // A's journal is still there, byte for byte; B's is gone.
    let left = journals(&vfs);
    assert_eq!(left.get(&a.1), Some(&a_bytes), "A's journal is unchanged");
    assert!(!left.contains_key(&b.1), "B's journal is removed");
    // While A's journal waits for the next open, a purge is refused (it may name an asset file).
    assert!(matches!(
        c.store.purge_plan(&BTreeSet::new()),
        Err(PurgeError::Recovery)
    ));
    c.store
        .save(&rec.model, Changes::Keys(&rec.keys))
        .expect("the recovered model saves");
    let saved = rec.model;
    c.store.close();
    assert_eq!(journals(&vfs).get(&a.1), Some(&a_bytes));

    // The open after that offers A's journal, alone now; its base (1) is not the generation on disk (2).
    let mut d = open_at(&vfs, 4, 40);
    assert_eq!(d.report.generation, 2);
    let info = d.report.recovery.clone().expect("A's journal is offered");
    assert_eq!(info.labels, vec!["test.edit_a".to_owned()]);
    assert_eq!(info.further, 0);
    assert!(!info.base_matches);
    let rec = d
        .store
        .recover(&d.model)
        .expect("distinct changes replay with the collision check");
    let (expected, _) = edit_device(&saved, 0, "edited by A");
    assert!(rec.model.content_eq(&expected), "both crashed sessions' changes");
    d.store.save(&rec.model, Changes::Keys(&rec.keys)).expect("saves");
    d.store.close();
    assert!(
        journals(&vfs).is_empty(),
        "no journal left: {:?}",
        journals(&vfs).keys()
    );

    let e = open_at(&vfs, 5, 50);
    assert!(e.report.recovery.is_none());
    assert!(e.model.content_eq(&expected));
    e.store.close();
}

/// Ruling 2 (cycle 41): "`discard_recovery` removes **only the summarised segment**."
#[test]
fn a_discard_removes_only_the_summarised_journal() {
    let TwoCrashes { vfs, disk, a, b } = two_crashes();
    let a_bytes = journals(&vfs)[&a.1].clone();

    let mut c = open_at(&vfs, 3, 30);
    assert_eq!(
        c.report.recovery.as_ref().map(|i| i.labels.clone()),
        Some(vec!["test.edit_b".to_owned()])
    );
    c.store.discard_recovery();
    assert!(c.store.recovery().is_none());
    let left = journals(&vfs);
    assert!(!left.contains_key(&b.1), "B's journal is discarded");
    assert_eq!(left.get(&a.1), Some(&a_bytes), "A's journal is unchanged");
    // A second discard in the same session has no summary to act on: A's journal stays for the next open.
    c.store.discard_recovery();
    assert_eq!(journals(&vfs).get(&a.1), Some(&a_bytes));
    assert!(matches!(
        c.store.purge_plan(&BTreeSet::new()),
        Err(PurgeError::Recovery)
    ));
    c.store.close();

    let mut d = open_at(&vfs, 4, 40);
    let info = d.report.recovery.clone().expect("A's journal is offered");
    assert_eq!(info.labels, vec!["test.edit_a".to_owned()]);
    assert_eq!(info.further, 0);
    assert!(info.base_matches, "nothing was saved since A's base");
    let rec = d.store.recover(&d.model).expect("replays");
    assert!(rec.model.content_eq(&a.0));
    assert!(!rec.model.content_eq(&disk));
    d.store.close();
}

/// Ruling 2 (cycle 41), the refused case of "behaves exactly as the existing code does for that case today (refused
/// or applied)": a journal whose base moved and whose field changed on disk too is refused with its collisions, and
/// it stays on disk, unchanged, still offered.
#[test]
fn a_journal_whose_base_moved_and_collides_is_refused_and_kept() {
    let (vfs, _) = mem_project_of(DirSync::Explicit, small_fixture());
    let a = crashed_session(&vfs, 1, 10, 0, "edited by A", "test.edit_a");
    let a_bytes = journals(&vfs)[&a.1].clone();
    // Session B keeps A's journal, edits the same field and saves: the base of A's journal moved.
    let mut b = open_at(&vfs, 2, 20);
    let (edited, keys) = edit_device(&b.model, 0, "edited by B");
    b.store.save(&edited, Changes::Keys(&keys)).expect("B saves");
    b.store.close();
    assert_eq!(journals(&vfs).get(&a.1), Some(&a_bytes));

    let mut c = open_at(&vfs, 3, 30);
    let info = c.report.recovery.clone().expect("A's journal is offered");
    assert!(!info.base_matches);
    match c.store.recover(&c.model) {
        Err(RecoverError::Replay(ReplayError::Collisions(collisions))) => assert!(!collisions.is_empty()),
        other => panic!("expected the collisions, got {:?}", other.map(|r| r.transactions)),
    }
    assert!(c.store.recovery().is_some(), "still offered");
    assert_eq!(journals(&vfs).get(&a.1), Some(&a_bytes), "kept, unchanged");
    c.store.close();
    let d = open_at(&vfs, 4, 40);
    assert!(d.report.recovery.is_some());
    assert!(d.model.content_eq(&edited));
    d.store.close();
}
