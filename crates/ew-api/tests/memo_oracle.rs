// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The memoised `Q` against a cold `Q` over the golden project, family by family and key by key, at every
//! step of an edit sequence (ADR-0002 amendment 4: the memo engine must give the values of a cold run).
//!
//! The states: open, a rep move on its page, a cross-page rep move, a page-less wire, a drawn wire, a
//! delete, a colour override, a potential rename, `page.renumber`, a tag counter change,
//! `numbering.renumber`, three undos, a redo, the topology cases of the incremental potential partition (two
//! potentials joined by a drawn wire, the potential split again by deleting that wire, a bridge added and
//! removed), and a colour- and size-profile edit applied to the memo store directly (no command edits the
//! project settings yet).
//!
//! Tests:
//! - `memo_equals_cold_over_every_state` (`MemoMode::On`): at every state every `Memo`/`MemoLru` family of
//!   `ew_derive::FAMILIES` (`potential_flow` is a cold lookup, decision log ID-221) is read for every key of
//!   the snapshot through a memoising `Q` and through `Session::query_over`; the values are equal, so are the
//!   derivation notes, and a second memoising `Q` (mostly memo hits) is equal again, with hits in the
//!   commit statistics.
//! - `oracle_finds_no_mismatch_over_every_state` (`MemoMode::Oracle`): the same states with the engine's own
//!   oracle re-running every memoised read cold; no mismatch and no cycle.
//! - `the_family_list_is_the_memo_families` (the guard that a promotion or demotion of a family breaks the
//!   test loudly).
//! - `a_cold_q_evaluates_every_query_once` (review cycle 10, finding 1): a cold `Q` evaluates each (family,
//!   key) once, detached scopes included.
//! - `creating_and_deleting_a_page_300_times_does_not_grow_the_store` (review cycle 10, finding 3): the
//!   memo store's dependencies, memos and push index stay within a small constant of the live set.

// The profile edit of the last state applies raw ops to a snapshot (`Model::apply_unchecked`, `raw-read`).
#![allow(clippy::disallowed_methods)]

mod golden_common;

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;
use std::rc::Rc;

use ew_api::{AnyCommand, CmdError, MemoMode, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_derive::ew_ids::Id;
use ew_derive::{
    FunctionCps, Oracle, PotentialId, PotentialNameOf, PotentialPartition, Q, RecordingTracker, XrefTextOf,
};
use ew_model::{AnyRow, EndpointKey, Model, Op};
use ew_schema::{Kind, Page, Record};
use serde_json::{Value, json};

// The families this test reads: the memoised families of both `FAMILIES` lists (`ew_api::verify`).
use ew_api::verify::SWEPT;

/// The oracle samples one memoised read in this many (see `oracle_finds_no_mismatch_over_every_state`).
const ORACLE_ONE_IN: u32 = 8;

/// The golden session with the colour and size profiles, a seeded ID source and the given memo mode.
fn session(batch: &ew_import_tables::ew_interop_api::ImportBatch, memo: MemoMode) -> Session {
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let mut project = project.expect("the batch holds the project");
    golden_common::with_profiles(&mut project);
    Session::from_records(
        project,
        records,
        batch.lib.clone(),
        SessionOptions {
            memo,
            ids: Some(seeded_ids(11)),
            ..SessionOptions::default()
        },
    )
    .expect("the golden batch passes the model's integrity check")
}

/// Reads every family at every key of the snapshot through `memo` (and compares with `cold`), through the
/// sweep `ew verify-derived` runs (`ew_api::verify`); a difference fails the test. The keys come from `cold`
/// when there is one, else from `memo`. Returns the keys read per family.
fn sweep(memo: &Q, cold: Option<&Q>, state: &str) -> BTreeMap<&'static str, usize> {
    let result = ew_api::verify::sweep(memo, cold);
    assert!(
        result.differences.is_empty(),
        "{state}: memoised values differ from cold: {:?}",
        result.differences
    );
    let swept: Vec<&str> = result.keys.keys().copied().collect();
    assert_eq!(swept, SWEPT, "the sweep reads exactly the listed families");
    result.keys
}

// ---------------------------------------------------------------------------------------------------------
// The states

/// The ID of a record as JSON (a string).
fn id_json<T>(id: Id<T>) -> Value {
    serde_json::to_value(id).expect("an ID serialises")
}

fn cmd(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// The first of `candidates` that the session accepts, executed (a bulk command through preview and
/// commit). A refused candidate changes nothing. Returns the executed candidate as text.
fn apply(session: &mut Session, state: &str, candidates: Vec<AnyCommand>) -> String {
    assert!(!candidates.is_empty(), "{state}: no candidate command");
    let mut refused = Vec::new();
    for c in candidates {
        let result = match session.execute(&c) {
            Err(CmdError::NeedsPreview { .. }) => {
                let preview = session.preview(&c);
                preview.and_then(|p| session.commit(p))
            }
            other => other,
        };
        match result {
            Ok(_) => return format!("{} {}", c.name, c.params),
            Err(e) => refused.push(format!("{} {}: {e:?}", c.name, c.params)),
        }
    }
    panic!("{state}: every candidate was refused:\n{}", refused.join("\n"));
}

/// Targets of the golden snapshot at open, in ID order: what the candidate commands are built from.
struct Targets {
    /// Same-page rep moves.
    move_on_page: Vec<AnyCommand>,
    /// Cross-page rep moves (reps without connected CPs).
    move_across: Vec<AnyCommand>,
    /// `connection.draw` between CPs of different pages, without a page.
    wire_pageless: Vec<AnyCommand>,
    /// `connection.draw` between CPs on one page.
    wire_drawn: Vec<AnyCommand>,
    /// `edit.delete` of a drawn connection.
    delete: Vec<AnyCommand>,
    /// `fields.set` of a connection colour override.
    colour: Vec<AnyCommand>,
    /// `fields.set` of a potential's name.
    rename: Vec<AnyCommand>,
    /// `fields.set` of a device's tag counter.
    counter: Vec<AnyCommand>,
}

/// A connection point of a function (the function's ID as JSON, the CP index) and whether it is free.
type CpEntry = (Value, u16, bool);

/// The connection points (function, idx) of the representations on `page`, free ones first.
fn cps_on(q: &Q, page: Id<Page>) -> Vec<CpEntry> {
    let mut out = Vec::new();
    for rep in q.idx().reps_by_page(page) {
        let Some(rep) = q.reps().get(rep) else { continue };
        for f in &rep.functions {
            for cp in q.get::<FunctionCps>(*f).iter().filter(|c| c.physical) {
                let free = q.idx().connections_at_cp(*f, cp.idx).next().is_none();
                out.push((id_json(*f), cp.idx, free));
            }
        }
    }
    out.sort_by_key(|(_, _, free)| !*free);
    out
}

fn cp_end(f: &Value, idx: u16) -> Value {
    json!({"cp": {"function": f, "idx": idx}})
}

impl Targets {
    fn of(q: &Q) -> Targets {
        let pages: Vec<Id<Page>> = q.pages().ids().collect();
        // Rep moves: delta of a few grid steps of the rep's page (multiples of the grid numerator are on the
        // grid).
        let mut move_on_page = Vec::new();
        let mut move_across = Vec::new();
        for rep in q.reps().ids().take(40) {
            let r = q.reps().get(rep).expect("the rep exists");
            let Some(page) = q.pages().get(r.page) else {
                continue;
            };
            let step = i64::try_from(page.grid.num()).expect("a grid step fits");
            for k in 1..=3 {
                move_on_page.push(cmd(
                    "rep.move",
                    json!({"reps": [id_json(rep)], "delta": [step * k, 0]}),
                ));
                move_on_page.push(cmd(
                    "rep.move",
                    json!({"reps": [id_json(rep)], "delta": [0, step * k]}),
                ));
            }
            let connected = r
                .functions
                .iter()
                .any(|f| !q.idx().connected_cps_of_function(*f).is_empty());
            if !connected {
                for other in pages.iter().filter(|p| **p != r.page).take(6) {
                    move_across.push(cmd(
                        "rep.move",
                        json!({"reps": [id_json(rep)], "delta": [0, 0], "page": id_json(*other)}),
                    ));
                }
            }
        }
        move_on_page.truncate(24);
        move_across.truncate(60);

        // Wires: pairs of CPs, first on one page, then across two pages.
        let per_page: Vec<(Id<Page>, Vec<CpEntry>)> = pages
            .iter()
            .map(|p| (*p, cps_on(q, *p)))
            .filter(|(_, cps)| cps.len() >= 2)
            .collect();
        let mut wire_drawn = Vec::new();
        for (page, cps) in &per_page {
            let cps: Vec<_> = cps.iter().take(6).collect();
            for (i, a) in cps.iter().enumerate() {
                for b in &cps[i + 1..] {
                    if a.0 != b.0 {
                        wire_drawn.push(cmd(
                            "connection.draw",
                            json!({"a": cp_end(&a.0, a.1), "b": cp_end(&b.0, b.1), "page": id_json(*page)}),
                        ));
                    }
                }
            }
        }
        wire_drawn.truncate(60);
        let mut wire_pageless = Vec::new();
        for (i, (_, a_cps)) in per_page.iter().enumerate() {
            for (_, b_cps) in &per_page[i + 1..] {
                for a in a_cps.iter().take(4) {
                    for b in b_cps.iter().take(4) {
                        wire_pageless.push(cmd(
                            "connection.draw",
                            json!({"a": cp_end(&a.0, a.1), "b": cp_end(&b.0, b.1)}),
                        ));
                    }
                }
            }
        }
        wire_pageless.truncate(60);

        let drawn: Vec<_> = q
            .connections()
            .iter()
            .filter(|(_, c)| c.page.is_some())
            .map(|(id, _)| id)
            .take(4)
            .collect();
        let delete = drawn
            .iter()
            .map(|id| {
                cmd(
                    "edit.delete",
                    json!({"targets": [{"kind": "connection", "id": id_json(*id)}]}),
                )
            })
            .collect();
        let colour = drawn
            .iter()
            .flat_map(|id| {
                [json!(["RD"]), json!("RD")].into_iter().map(move |value| {
                    cmd(
                        "fields.set",
                        json!({
                            "kind": "connection",
                            "targets": [id_json(*id)],
                            "path": "overrides.colour",
                            "value": value,
                        }),
                    )
                })
            })
            .collect();
        let rename = q
            .potential_defs()
            .ids()
            .take(3)
            .map(|id| {
                cmd(
                    "fields.set",
                    json!({"kind": "potential_def", "targets": [id_json(id)], "path": "name", "value": "RENAMED_POTENTIAL"}),
                )
            })
            .collect();
        let counter = q
            .devices()
            .iter()
            .filter(|(_, d)| {
                let v = serde_json::to_value(&***d).expect("a device serialises");
                v["tag"]["counter"].is_number() && v.get("terminal").is_none()
            })
            .take(3)
            .map(|(id, _)| {
                cmd(
                    "fields.set",
                    json!({"kind": "device", "targets": [id_json(id)], "path": "tag.counter", "value": 900}),
                )
            })
            .collect();
        Targets {
            move_on_page,
            move_across,
            wire_pageless,
            wire_drawn,
            delete,
            colour,
            rename,
            counter,
        }
    }
}

/// A connection point as (function, index).
type CpId = (Id<ew_schema::Function>, u16);

fn potential_of(q: &Q, (f, idx): CpId) -> Option<PotentialId> {
    q.get::<PotentialPartition>(())
        .potential_at(EndpointKey::Cp(f, idx))
        .map(|p| p.id)
}

/// Pairs of physical CPs on one page (page, a, b) whose potentials differ in `q`, in ID order.
fn cp_pairs_in_different_potentials(q: &Q) -> Vec<(Id<Page>, CpId, CpId)> {
    let mut out = Vec::new();
    for page in q.pages().ids() {
        let mut cps: Vec<CpId> = Vec::new();
        for rep in q.idx().reps_by_page(page) {
            let Some(rep) = q.reps().get(rep) else { continue };
            for f in &rep.functions {
                for cp in q.get::<FunctionCps>(*f).iter().filter(|c| c.physical) {
                    cps.push((*f, cp.idx));
                }
            }
        }
        cps.truncate(12);
        for (i, a) in cps.iter().enumerate() {
            for b in &cps[i + 1..] {
                let (pa, pb) = (potential_of(q, *a), potential_of(q, *b));
                if a.0 != b.0 && pa.is_some() && pb.is_some() && pa != pb {
                    out.push((page, *a, *b));
                }
            }
        }
    }
    out.truncate(60);
    out
}

/// The connections that have both CPs as ends.
fn connections_between(q: &Q, a: CpId, b: CpId) -> Vec<Id<ew_schema::Connection>> {
    let at_b: BTreeSet<_> = q.idx().connections_at_cp(b.0, b.1).collect();
    q.idx()
        .connections_at_cp(a.0, a.1)
        .filter(|c| at_b.contains(c))
        .collect()
}

fn same_potential(q: &Q, a: CpId, b: CpId) -> bool {
    let pa = potential_of(q, a);
    pa.is_some() && pa == potential_of(q, b)
}

/// Draws a connection (of `kind`, default a wire) between one of the CP pairs that lie in two different
/// potentials on a page; checks, through the cold `Q` after it, that the CPs share a potential. Returns the
/// pair.
fn draw_between_potentials(session: &mut Session, label: &str, kind: Option<Value>) -> (CpId, CpId) {
    let pairs = cp_pairs_in_different_potentials(&session.query_over(&session.world().model));
    assert!(
        !pairs.is_empty(),
        "{label}: no CP pair in two different potentials"
    );
    let candidates: Vec<AnyCommand> = pairs
        .iter()
        .map(|(page, a, b)| {
            let mut params = json!({
                "a": cp_end(&id_json(a.0), a.1),
                "b": cp_end(&id_json(b.0), b.1),
                "page": id_json(*page),
            });
            if let Some(kind) = &kind {
                params["kind"] = kind.clone();
            }
            cmd("connection.draw", params)
        })
        .collect();
    let what = apply(session, label, candidates.clone());
    let i = candidates
        .iter()
        .position(|c| format!("{} {}", c.name, c.params) == what)
        .expect("the executed candidate is one of the candidates");
    let (_, a, b) = pairs[i];
    let q = session.query_over(&session.world().model);
    assert!(
        same_potential(&q, a, b),
        "{label}: the CPs share a potential after the draw"
    );
    (a, b)
}

/// Deletes the one connection between the CPs; checks that the CPs are in different potentials afterwards.
fn delete_between(session: &mut Session, label: &str, (a, b): (CpId, CpId)) {
    let q = session.query_over(&session.world().model);
    let found = connections_between(&q, a, b);
    assert_eq!(found.len(), 1, "{label}: one connection joins the CPs");
    let c = cmd(
        "edit.delete",
        json!({"targets": [{"kind": "connection", "id": id_json(found[0])}]}),
    );
    apply(session, label, vec![c]);
    let q = session.query_over(&session.world().model);
    assert!(
        !same_potential(&q, a, b),
        "{label}: the CPs are in different potentials again"
    );
}

/// The profile edit of the last state: the first colour row of `ProjectSettings.extra` gets another
/// colour and the first size row another cross-section (`2.5`). The model after it, at the next revision (the Session's own snapshot is not changed).
fn with_edited_profile(model: &Model) -> (Model, Vec<ew_model::AnyKey>) {
    let before = model.project().clone();
    let mut project = (*before).clone();
    let profile = project
        .settings
        .extra
        .get_mut("colour_profile")
        .expect("the session has a colour profile");
    profile["rows"][0]["colour"] = json!(["RD"]);
    let size = project
        .settings
        .extra
        .get_mut("size_profile")
        .expect("the session has a size profile");
    size["rows"][0]["csa"] = json!("2.5");
    let key = (Kind::Project, before.id.cast());
    let after = model
        .apply_unchecked(
            &[Op::update(
                AnyRow::Project(before),
                AnyRow::Project(project.into()),
            )],
            model.rev().next(),
        )
        .expect("the project row applies");
    (after, vec![key])
}

/// Applies the states in order; after each one `visit` gets the session, the label and the snapshot that
/// the memo store and `session.query()` see.
fn run_states(session: &mut Session, mut visit: impl FnMut(&Session, &str, &Model)) {
    let targets = Targets::of(&session.query_over(&session.world().model));
    let mut model = session.world().model.clone();
    visit(session, "1 open", &model);

    let mut step = |session: &mut Session, label: &str, run: &dyn Fn(&mut Session) -> String| {
        let before = session.world().rev;
        let what = run(session);
        assert!(
            session.world().rev > before,
            "{label}: the revision did not change ({what})"
        );
        eprintln!("{label}: {what}");
        let model = session.world().model.clone();
        visit(session, label, &model);
    };
    let by = |candidates: Vec<AnyCommand>| {
        move |session: &mut Session| apply(session, "state", candidates.clone())
    };
    step(
        session,
        "2 rep.move on its page",
        &by(targets.move_on_page.clone()),
    );
    step(
        session,
        "3 rep.move to another page",
        &by(targets.move_across.clone()),
    );
    step(session, "4 page-less wire", &by(targets.wire_pageless.clone()));
    step(session, "5 drawn wire", &by(targets.wire_drawn.clone()));
    step(session, "6 delete", &by(targets.delete.clone()));
    step(session, "7 colour override", &by(targets.colour.clone()));
    step(session, "8 potential rename", &by(targets.rename.clone()));
    step(
        session,
        "9 page.renumber",
        &by(vec![cmd(
            "page.renumber",
            json!({"scope": "project", "start": 1000, "step": 3}),
        )]),
    );
    step(session, "10 tag counter", &by(targets.counter.clone()));
    step(
        session,
        "11 numbering.renumber",
        &by(vec![cmd("numbering.renumber", json!({"scope": "project"}))]),
    );
    for n in 12..=14 {
        step(session, &format!("{n} undo"), &|s| {
            s.undo().expect("undo");
            "undo".into()
        });
    }
    step(session, "15 redo", &|s| {
        s.redo().expect("redo");
        "redo".into()
    });

    // The topology cases of the incremental potential partition.
    let joined = std::cell::Cell::new(None);
    step(session, "16 join two potentials", &|s| {
        let ends = draw_between_potentials(s, "16 join two potentials", None);
        joined.set(Some(ends));
        format!("connection.draw {ends:?}")
    });
    let ends = joined.get().expect("the join state ran");
    step(session, "17 split a potential", &|s| {
        delete_between(s, "17 split a potential", ends);
        format!("edit.delete {ends:?}")
    });
    let bridged = std::cell::Cell::new(None);
    let bridge = json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 0}});
    step(session, "18 bridge added", &|s| {
        let ends = draw_between_potentials(s, "18 bridge added", Some(bridge.clone()));
        bridged.set(Some(ends));
        format!("connection.draw bridge {ends:?}")
    });
    let ends = bridged.get().expect("the bridge state ran");
    step(session, "19 bridge removed", &|s| {
        delete_between(s, "19 bridge removed", ends);
        format!("edit.delete {ends:?}")
    });

    // The profile edits go to the memo store directly: no command edits the project settings. The colour
    // and the size edit are one advance; no session command may follow.
    model = session.world().model.clone();
    let (after, keys) = with_edited_profile(&model);
    session.memo().expect("memoising session").advance(&after, &keys);
    visit(session, "20 colour and size profile edit", &after);
}

fn memo_q_notes_equal(session: &Session, model: &Model, state: &str) -> BTreeMap<&'static str, usize> {
    let mut memo = session.query();
    let mut cold = session.query_over(model);
    let counts = sweep(&memo, Some(&cold), state);
    assert_eq!(
        memo.diagnostics().notes(),
        cold.diagnostics().notes(),
        "{state}: derivation notes, first pass"
    );
    counts
}

// ---------------------------------------------------------------------------------------------------------
// Tests

/// The guard: the swept list is the memoised families of `ew_derive::FAMILIES` and `ew_reports::FAMILIES`.
fn assert_family_list() {
    use ew_derive::{FAMILIES, Policy};
    let mut memoised: Vec<&str> = FAMILIES
        .iter()
        .chain(ew_reports::FAMILIES)
        .filter(|(_, p)| matches!(p, Policy::Memo | Policy::MemoLru))
        .map(|(n, _)| *n)
        .collect();
    memoised.sort_unstable();
    assert_eq!(memoised, SWEPT, "a family changed its policy: update the test");
}

#[test]
fn the_family_list_is_the_memo_families() {
    assert_family_list();
}

#[test]
fn memo_equals_cold_over_every_state() {
    assert_family_list();
    let batch = golden_common::batch();
    let mut session = session(&batch, MemoMode::On);
    let mut counts_at_open = None;
    run_states(&mut session, |session, state, model| {
        let counts = memo_q_notes_equal(session, model, state);
        // The second pass: a new memoising `Q`, mostly memo hits.
        let mut second = session.query();
        let mut cold = session.query_over(model);
        sweep(&second, Some(&cold), state);
        assert_eq!(
            second.diagnostics().notes(),
            cold.diagnostics().notes(),
            "{state}: derivation notes, second pass"
        );
        let stats = session.memo().expect("memoising session").commit_stats();
        assert!(
            stats.total().hits > 0,
            "{state}: the second pass hit no memo: {stats:?}"
        );
        counts_at_open.get_or_insert(counts);
    });
    eprintln!(
        "keys per family at open: {:?}",
        counts_at_open.expect("a state ran")
    );
}

#[test]
fn oracle_finds_no_mismatch_over_every_state() {
    assert_family_list();
    let batch = golden_common::batch();
    let mut session = session(&batch, MemoMode::Oracle);
    // `Oracle::Every` (the Session's own setting) re-runs every memoised read cold at every depth: about
    // five minutes over the 20 states in a debug build. One read in `ORACLE_ONE_IN`, counted over all reads
    // of the store (deterministic), keeps the test under a minute.
    session
        .memo()
        .expect("memoising session")
        .set_oracle(Oracle::OneIn(
            NonZeroU32::new(ORACLE_ONE_IN).expect("n is not zero"),
        ));
    run_states(&mut session, |session, state, _model| {
        let q = session.query();
        sweep(&q, None, state);
        let mismatches = session.take_oracle_mismatches();
        assert!(
            mismatches.is_empty(),
            "{state}: oracle mismatches: {mismatches:#?}"
        );
        let total = session.memo().expect("memoising session").total_stats().total();
        assert_eq!(total.cycles, 0, "{state}: cycles detected");
        assert!(total.oracle_checks > 0, "{state}: the oracle checked nothing");
    });
}

/// Review cycle 10, finding 1: a cold `Q` (`MemoMode::Off`, `Session::query_over`, the oracle's cold twin)
/// evaluates every (family, key) at most once, detached scopes included. Before the fix every detached scope
/// had a cache of its own: one cold partition of the golden project evaluated `drawn_page_order` once per
/// interruption point.
#[test]
fn a_cold_q_evaluates_every_query_once() {
    let batch = golden_common::batch();
    let session = session(&batch, MemoMode::Off);
    let model = session.world().model;
    let recorder = Rc::new(RecordingTracker::new());
    let q = Q::with_tracker(&model, Box::new(Rc::clone(&recorder)));
    let part = q.get::<PotentialPartition>(());
    let evaluated = recorder.evaluated();
    let count = |family: &str| evaluated.iter().filter(|(f, _)| *f == family).count();
    assert!(count("ip_info") > 1, "the golden project has interruption points");
    assert_eq!(
        count("drawn_page_order"),
        1,
        "drawn_page_order once per cold partition"
    );
    // The names, flows and cross-references in the same `Q`: still each evaluation once.
    for p in part.potentials() {
        q.get::<PotentialNameOf>(p.id);
        q.get::<ew_derive::PotentialFlow>(p.id);
    }
    for rep in q.reps().ids() {
        q.get::<XrefTextOf>(rep);
    }
    let mut seen = BTreeSet::new();
    for e in recorder.evaluated() {
        assert!(seen.insert(e.clone()), "{e:?} is evaluated twice in one cold Q");
    }
    eprintln!("cold evaluations: {}", seen.len());
}

/// The memo store's sizes the growth test watches: (partition dependencies, `page_ids` dependencies,
/// `page_local_nets` memos, push-index entries).
fn growth_counts(session: &Session) -> (usize, usize, usize, usize) {
    let store = session.memo().expect("memoising session");
    let memory = store.memory();
    let of = |family: &str| {
        memory
            .iter()
            .find(|m| m.family == family)
            .copied()
            .unwrap_or_default()
    };
    (
        of("potential_partition").dep_entries,
        of("page_ids").dep_entries,
        of("page_local_nets").memos,
        store.push_index_entries(),
    )
}

/// Review cycle 10, finding 3: 300 cycles of creating and deleting an empty page, the partition read after
/// each commit, then 20 cycles with every memoised family read for every key after each commit. Before the fix
/// the incremental frames kept every dependency, part stamps were never pruned and the memos of deleted pages
/// were never collected (the review's probe: partition dependencies 110 -> 410, `page_ids` dependencies
/// 1 -> 301, `page_local_nets` memos 13 -> 313, push index 9,526 -> 10,726). Now every count stays within a
/// small constant of the live set.
#[test]
fn creating_and_deleting_a_page_300_times_does_not_grow_the_store() {
    let batch = golden_common::batch();
    let mut session = session(&batch, MemoMode::On);
    let live_pages = session.world().model.table::<Page>().len();
    let sweeping = std::cell::Cell::new(false);
    let read = |session: &Session| {
        let q = session.query();
        if sweeping.get() {
            sweep(&q, None, "growth");
        } else {
            q.get::<PotentialPartition>(());
        }
    };
    let cycle = |session: &mut Session| {
        let receipt = session
            .execute(&cmd("page.create", json!({"number": 9000})))
            .expect("page.create");
        let page = receipt
            .created
            .iter()
            .find(|(kind, _)| *kind == Kind::Page)
            .map(|(_, id)| id.cast::<Page>())
            .expect("a page is created");
        read(session);
        apply(
            session,
            "page.delete",
            vec![cmd("page.delete", json!({"pages": [id_json(page)]}))],
        );
        read(session);
    };
    read(&session);
    for _ in 0..2 {
        cycle(&mut session);
    }
    let before = growth_counts(&session);
    for _ in 0..298 {
        cycle(&mut session);
    }
    let after = growth_counts(&session);
    eprintln!(
        "(partition deps, page_ids deps, page_local_nets memos, push index): {before:?} -> {after:?} \
         ({live_pages} live pages)"
    );
    assert!(
        after.0 <= before.0 + 2,
        "partition dependencies {before:?} -> {after:?}"
    );
    assert!(after.1 <= 4, "page_ids dependencies {before:?} -> {after:?}");
    assert!(
        after.2 <= live_pages + 2,
        "page_local_nets memos {before:?} -> {after:?}"
    );
    assert!(
        after.3 <= before.3 + 32,
        "push index entries {before:?} -> {after:?}"
    );
    let collected = session
        .memo()
        .expect("memoising session")
        .total_stats()
        .total()
        .collected;
    assert!(
        collected >= 290,
        "the deleted pages' memos are collected: {collected}"
    );

    // Every family read for every key: the memos only the host reads (a deleted page's title block) go too.
    sweeping.set(true);
    let families = |session: &Session| -> BTreeMap<&'static str, (usize, usize)> {
        session
            .memo()
            .expect("memoising session")
            .memory()
            .into_iter()
            .map(|m| (m.family, (m.memos, m.dep_entries)))
            .collect()
    };
    for _ in 0..2 {
        cycle(&mut session);
    }
    let (before, push_before) = (families(&session), growth_counts(&session).3);
    for _ in 0..20 {
        cycle(&mut session);
    }
    let (after, push_after) = (families(&session), growth_counts(&session).3);
    for (family, (memos, deps)) in &after {
        let (m0, d0) = before.get(family).copied().unwrap_or_default();
        assert!(
            *memos <= m0 + 1 && *deps <= d0 + 8,
            "{family}: (memos, dependency entries) {:?} -> {:?}",
            (m0, d0),
            (memos, deps)
        );
    }
    assert!(
        push_after <= push_before + 32,
        "push index entries over full sweeps {push_before} -> {push_after}"
    );
    eprintln!("full sweeps: push index {push_before} -> {push_after}");
}
