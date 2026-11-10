// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The memo backend of `Q` (ADR-0002 amendment 4), with test families over a minimal model:
//!
//! - `memoised_notes_equal_the_cold_notes` (review cycle 9, finding 3): the flattened notes of a memoised
//!   global value equal a cold `Q`'s (same order, no duplicates), on the first read and on memo hits.

use crate::common;

use std::sync::Arc;

use common::Fx;
use ew_derive::{DeriveNote, MemoStore, Policy, Q, Query};
use ew_model::Model;

/// The number of global notes `Global` pushes, read from the project's `extra` member `notes`.
fn note_count(q: &Q) -> usize {
    q.project()
        .extra
        .get("notes")
        .and_then(serde_json::Value::as_u64)
        .map_or(0, |n| usize::try_from(n).unwrap_or(0))
}

/// A global value with many notes.
struct Global;
impl Query for Global {
    type Key = ();
    type Value = usize;
    const FAMILY: &'static str = "test.global";
    const POLICY: Policy = Policy::Memo;
    fn run(q: &Q, _: &()) -> Arc<usize> {
        let n = note_count(q);
        for i in 0..n {
            q.note(DeriveNote::MalformedShim {
                member: format!("settings.test_{i:04}"),
                detail: "a global note".into(),
            });
        }
        Arc::new(n)
    }
}

/// A per-key reader of `Global` without notes of its own.
struct Reader;
impl Query for Reader {
    type Key = u32;
    type Value = u32;
    const FAMILY: &'static str = "test.reader";
    const POLICY: Policy = Policy::Memo;
    fn run(q: &Q, k: &u32) -> Arc<u32> {
        let _ = q.get::<Global>(());
        Arc::new(*k)
    }
}

/// A per-key reader of `Global` with one note of its own.
struct Noting;
impl Query for Noting {
    type Key = u32;
    type Value = u32;
    const FAMILY: &'static str = "test.noting";
    const POLICY: Policy = Policy::Memo;
    fn run(q: &Q, k: &u32) -> Arc<u32> {
        let _ = q.get::<Global>(());
        q.note(DeriveNote::MalformedShim {
            member: format!("settings.reader_{k:04}"),
            detail: "a reader's note".into(),
        });
        Arc::new(*k)
    }
}

const READERS: u32 = 40;

fn model(notes: usize) -> Model {
    let mut fx = Fx::new();
    fx.project_set("notes", serde_json::json!(notes));
    fx.build()
}

fn read_all(q: &Q) {
    for k in 0..READERS {
        let _ = q.get::<Reader>(k);
        let _ = q.get::<Noting>(k);
    }
}

#[test]
fn memoised_notes_equal_the_cold_notes() {
    for n in [10, 300] {
        let model = model(n);
        let store = MemoStore::new(&model);
        let mut memo = store.query();
        read_all(&memo);
        let mut cold = Q::new(&model);
        read_all(&cold);
        let notes = memo.diagnostics().notes();
        assert_eq!(notes, cold.diagnostics().notes(), "{n} global notes");
        assert_eq!(notes.len(), n + READERS as usize, "each note once");
        assert!(notes.windows(2).all(|w| w[0] < w[1]), "canonical order");

        // A second request: memo hits replay the shared sets.
        let mut again = store.query();
        read_all(&again);
        assert_eq!(again.diagnostics().notes(), notes);
        assert!(store.commit_stats().total().hits > 0);
    }
}

/// The model with the project member `notes` set to `n`, at the next revision of `model` (a raw edit of the
/// snapshot, as a commit would make it).
#[allow(clippy::disallowed_methods)]
fn with_notes(model: &Model, n: usize) -> (Model, Vec<ew_model::AnyKey>) {
    let before = model.project().clone();
    let mut project = (*before).clone();
    project.extra.insert("notes".into(), serde_json::json!(n));
    let key = (ew_schema::Kind::Project, before.id.cast());
    let after = model
        .apply_unchecked(
            &[ew_model::Op::update(
                ew_model::AnyRow::Project(before),
                ew_model::AnyRow::Project(project.into()),
            )],
            model.rev().next(),
        )
        .expect("the project row applies");
    (after, vec![key])
}

#[test]
fn a_fork_evolves_independently() {
    let model = model(2);
    let store = MemoStore::new(&model);
    assert_eq!(*store.query().get::<Global>(()), 2);
    let fork = store.fork();
    assert_eq!(fork.rev(), store.rev());
    // The fork shares the warm memo: a hit.
    assert_eq!(*fork.query().get::<Global>(()), 2);
    assert_eq!(fork.commit_stats().family("test.global").hits, 1);
    let (after, keys) = with_notes(&model, 5);
    fork.advance(&after, &keys);
    assert_eq!(*fork.query().get::<Global>(()), 5);
    // The store it was taken from is untouched.
    assert_eq!(store.rev(), model.rev());
    assert_eq!(*store.query().get::<Global>(()), 2);
}
