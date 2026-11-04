// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start-up scan across projects (WP-S6.05 D10, D11; PV-7): every project's leftover journal with unsaved work is
//! listed unless its lock is live, and a segment older than 30 days is deleted at start.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use common::{edit_device, mem_project_of, options, root, small_fixture};
use ew_model::AnyKey;
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{ManualClock, Opened, Store};

/// A session of process 1 that commits one edit and crashes: its lock and its journal stay.
fn crashed(vfs: &Arc<MemVfs>) {
    let Opened { mut store, model, .. } = Store::open(&root(), options(vfs, 1)).expect("the project opens");
    let (edited, keys) = edit_device(&model, 0, "start-up scan");
    let keys: Vec<AnyKey> = keys.into_iter().collect();
    store
        .journal_commit(&model, &edited, &keys, "device.set", "test")
        .expect("the journal accepts the commit");
    store.crash();
}

fn segments(vfs: &MemVfs) -> Vec<String> {
    vfs.files(Path::new("/state"))
        .into_keys()
        .filter(|name| name.ends_with(".jsonl"))
        .collect()
}

#[test]
fn a_crashed_project_is_listed_unless_its_lock_is_live() {
    let (vfs, _) = mem_project_of(DirSync::Explicit, small_fixture());
    crashed(&vfs);
    // Process 2 starts: process 1 is dead, so its lock is stale and the project is offered.
    let found = options(&vfs, 2).start_up_scan();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, root());
    assert_eq!(found[0].transactions, 1);
    assert_eq!(found[0].last_change_ms, 1_792_222_930_000);
    // Another session of a live process holds the lock: the project is open there, so it is not listed.
    assert_eq!(options(&vfs, 1).start_up_scan(), Vec::new());
    assert_eq!(segments(&vfs).len(), 1, "the scan deletes no recent journal");
}

#[test]
fn a_journal_older_than_thirty_days_is_deleted_at_start() {
    let (vfs, _) = mem_project_of(DirSync::Explicit, small_fixture());
    crashed(&vfs);
    assert_eq!(segments(&vfs).len(), 1);
    let mut later = options(&vfs, 2);
    let clock = ManualClock::fixed();
    clock.advance(Duration::from_secs(31 * 24 * 3600));
    later.clock = Arc::new(clock);
    assert_eq!(later.start_up_scan(), Vec::new());
    assert_eq!(segments(&vfs), Vec::<String>::new(), "the old segment is gone");
}

#[test]
fn no_state_directory_finds_nothing() {
    let (vfs, _) = mem_project_of(DirSync::Explicit, small_fixture());
    crashed(&vfs);
    let mut o = options(&vfs, 2);
    o.state_dir = None;
    assert_eq!(o.start_up_scan(), Vec::new());
}
