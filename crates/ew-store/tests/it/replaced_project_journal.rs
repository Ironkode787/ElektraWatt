// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A leftover crash journal is not offered to another project made later at the same path with the same project ID
//! (an import of the same source again, a Save As onto the path): `Store::create` deletes it (doc 05 §8.1).

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::{edit_device, mem_project_of, options, reopen, root, small_fixture};
use ew_model::AnyKey;
use ew_store::Vfs;
use ew_store::mem::DirSync;
use ew_store::{Opened, Store};

#[test]
fn a_project_made_at_the_path_of_a_crashed_one_is_not_offered_its_journal() {
    let (vfs, _) = mem_project_of(DirSync::Explicit, small_fixture());
    // A session edits the project and ends without a clean close: its journal stays in the state directory.
    let Opened { mut store, model, .. } = reopen(&vfs, 1);
    let (edited, keys) = edit_device(&model, 0, "edited before the crash");
    let keys: Vec<AnyKey> = keys.into_iter().collect();
    store
        .journal_commit(&model, &edited, &keys, "test.edit", "test")
        .expect("the journal accepts the commit");
    store.crash();
    let own = reopen(&vfs, 9);
    assert!(
        own.report.recovery.is_some(),
        "the project itself is offered its journal"
    );
    own.store.close();

    // The project is deleted, and a project with the same ID is made at the same path.
    for name in vfs.files(&root()).into_keys() {
        vfs.remove_file(&root().join(&name)).expect("the file is removed");
    }
    Store::create(&root(), &model, options(&vfs, 2))
        .expect("the project is made")
        .close();

    assert_eq!(
        reopen(&vfs, 3).report.recovery,
        None,
        "nothing of the old journal is offered"
    );
}
