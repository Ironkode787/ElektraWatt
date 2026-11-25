// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The memo backend of `Q` (ADR-0002 amendment 4), with a test family over a minimal model.

use crate::common;

use std::sync::Arc;

use common::Fx;
use ew_derive::{MemoStore, Policy, Q, Query};
use ew_model::Model;

/// The project's `extra` member `notes`, as a number.
fn note_count(q: &Q) -> usize {
    q.project()
        .extra
        .get("notes")
        .and_then(serde_json::Value::as_u64)
        .map_or(0, |n| usize::try_from(n).unwrap_or(0))
}

/// A global value: the project's member `notes`.
struct Global;
impl Query for Global {
    type Key = ();
    type Value = usize;
    const FAMILY: &'static str = "test.global";
    const POLICY: Policy = Policy::Memo;
    fn run(q: &Q, _: &()) -> Arc<usize> {
        Arc::new(note_count(q))
    }
}

fn model(notes: usize) -> Model {
    let mut fx = Fx::new();
    fx.project_set("notes", serde_json::json!(notes));
    fx.build()
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
