// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The memoised `Q` against a cold `Q` over the golden project, family by family and key by key, at every
//! step of an edit sequence (ADR-0002 amendment 4: the memo engine must give the values of a cold run).
//!
//! The states: open, a rep move on its page, a cross-page rep move, a page-less wire, a drawn wire, a
//! delete, a colour override, a potential rename, `page.renumber`, a tag counter change,
//! `numbering.renumber`, three undos, a redo, and a colour-profile edit applied to the memo store directly
//! (no command edits the project settings yet).
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

// The profile edit of the last state applies raw ops to a snapshot (`Model::apply_unchecked`, `raw-read`).
#![allow(clippy::disallowed_methods)]

mod golden_common;

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use ew_api::{AnyCommand, CmdError, MemoMode, Session, SessionOptions};
use ew_commands::testkit::seeded_ids;
use ew_derive::ew_ids::Id;
use ew_derive::{
    BomMode, BomRows, DevicePartLines, DeviceTag, DevicesByStructure, DrawnPageOrder, FlowBfs, FunctionCps,
    Member, Oracle, PageLocalNets, PageOrder, PageRef, PhysicalWiresOf, PotentialId, PotentialPartition, Q,
    Query, SettingsMember, StitchGraphQ, TitleBlockValues, TocRows, WirePropsOf, XrefTextOf,
};
use ew_derive::{
    BondIndex, BondPathsQ, BondVias, DefPoints, FunctionLinksOf, IpFunctions, IpInfoOf, IpPairingQ, PageIds,
    PagelessLinks, RailBondsQ, RailTerminalOf, RailTerminals,
};
use ew_model::{AnyRow, Model, Op};
use ew_schema::{Device, Kind, Page, Record};
use serde_json::{Value, json};

/// The families this test reads: the `Memo`/`MemoLru` families of `ew_derive::FAMILIES`, sorted.
const SWEPT: &[&str] = &[
    "bom_rows",
    "bond_index",
    "bond_paths",
    "bond_vias",
    "def_points",
    "device_part_lines",
    "device_tag",
    "devices_by_structure",
    "drawn_page_order",
    "function_links",
    "ip_functions",
    "ip_info",
    "ip_pairing",
    "page_ids",
    "page_local_nets",
    "page_order",
    "pageless_links",
    "physical_wires_of",
    "potential_flow_bfs",
    "potential_partition",
    "rail_bonds",
    "rail_terminal",
    "rail_terminals",
    "settings_member",
    "stitch_graph",
    "title_block",
    "toc_rows",
    "wire_props_of",
    "xref_text",
];

/// Every settings member (the keys of `settings_member`).
const ALL_MEMBERS: [Member; 8] = [
    Member::IpPairing,
    Member::ColourProfile,
    Member::SizeProfile,
    Member::WireDefault,
    Member::Numbering,
    Member::Frames,
    Member::XrefFormat,
    Member::DerivationProfile,
];

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

/// The keys of the snapshot a `Q` sees, for the families with a non-trivial key.
struct Keys {
    devices: Vec<Id<Device>>,
    functions: Vec<Id<ew_schema::Function>>,
    pages: Vec<Id<Page>>,
    reps: Vec<Id<ew_schema::Representation>>,
    page_refs: Vec<PageRef>,
    potentials: Vec<PotentialId>,
}

impl Keys {
    fn of(q: &Q) -> Keys {
        Keys {
            devices: q.devices().ids().collect(),
            functions: q.functions().ids().collect(),
            pages: q.pages().ids().collect(),
            reps: q.reps().ids().collect(),
            page_refs: q.get::<PageOrder>(()).entries.iter().map(|e| e.page).collect(),
            potentials: q
                .get::<PotentialPartition>(())
                .potentials
                .iter()
                .map(|p| p.id)
                .collect(),
        }
    }
}

/// Reads `F` at every key through `memo` and, when given, `cold`; the values must be equal. Returns the
/// number of keys.
fn check<F: Query>(memo: &Q, cold: Option<&Q>, keys: impl IntoIterator<Item = F::Key>, state: &str) -> usize {
    let mut n = 0;
    for key in keys {
        let m = memo.get::<F>(key.clone());
        if let Some(cold) = cold {
            let c = cold.get::<F>(key.clone());
            assert_eq!(*m, *c, "{state}: {}({key:?})", F::FAMILY);
        }
        n += 1;
    }
    n
}

/// Reads every family at every key of the snapshot through `memo` (and compares with `cold`). The keys come
/// from `cold` when there is one, else from `memo`. Returns the keys read per family.
fn sweep(memo: &Q, cold: Option<&Q>, state: &str) -> BTreeMap<&'static str, usize> {
    let keys = Keys::of(cold.unwrap_or(memo));
    let mut n = BTreeMap::new();
    let mut put = |family: &'static str, count: usize| {
        n.insert(family, count);
    };
    put(
        BomRows::FAMILY,
        check::<BomRows>(memo, cold, [BomMode::Component, BomMode::Assembly], state),
    );
    for (family, count) in [
        (BondIndex::FAMILY, check::<BondIndex>(memo, cold, [()], state)),
        (BondPathsQ::FAMILY, check::<BondPathsQ>(memo, cold, [()], state)),
        (BondVias::FAMILY, check::<BondVias>(memo, cold, [()], state)),
        (DefPoints::FAMILY, check::<DefPoints>(memo, cold, [()], state)),
        (
            FunctionLinksOf::FAMILY,
            check::<FunctionLinksOf>(memo, cold, keys.functions.iter().copied(), state),
        ),
        (IpFunctions::FAMILY, check::<IpFunctions>(memo, cold, [()], state)),
        (
            IpInfoOf::FAMILY,
            check::<IpInfoOf>(memo, cold, keys.functions.iter().copied(), state),
        ),
        (IpPairingQ::FAMILY, check::<IpPairingQ>(memo, cold, [()], state)),
        (PageIds::FAMILY, check::<PageIds>(memo, cold, [()], state)),
        (
            PagelessLinks::FAMILY,
            check::<PagelessLinks>(memo, cold, [()], state),
        ),
        (RailBondsQ::FAMILY, check::<RailBondsQ>(memo, cold, [()], state)),
        (
            RailTerminalOf::FAMILY,
            check::<RailTerminalOf>(memo, cold, keys.devices.iter().copied(), state),
        ),
        (
            RailTerminals::FAMILY,
            check::<RailTerminals>(memo, cold, [()], state),
        ),
    ] {
        put(family, count);
    }
    put(
        DevicePartLines::FAMILY,
        check::<DevicePartLines>(memo, cold, keys.devices.iter().copied(), state),
    );
    put(
        DeviceTag::FAMILY,
        check::<DeviceTag>(memo, cold, keys.devices.iter().copied(), state),
    );
    put(
        DevicesByStructure::FAMILY,
        check::<DevicesByStructure>(memo, cold, [()], state),
    );
    put(
        DrawnPageOrder::FAMILY,
        check::<DrawnPageOrder>(memo, cold, [()], state),
    );
    put(
        PageLocalNets::FAMILY,
        check::<PageLocalNets>(memo, cold, keys.pages.iter().copied(), state),
    );
    put(PageOrder::FAMILY, check::<PageOrder>(memo, cold, [()], state));
    put(
        PhysicalWiresOf::FAMILY,
        check::<PhysicalWiresOf>(memo, cold, keys.potentials.iter().copied(), state),
    );
    put(FlowBfs::FAMILY, check::<FlowBfs>(memo, cold, [()], state));
    put(
        PotentialPartition::FAMILY,
        check::<PotentialPartition>(memo, cold, [()], state),
    );
    put(
        SettingsMember::FAMILY,
        check::<SettingsMember>(memo, cold, ALL_MEMBERS, state),
    );
    put(
        StitchGraphQ::FAMILY,
        check::<StitchGraphQ>(memo, cold, [()], state),
    );
    put(
        TitleBlockValues::FAMILY,
        check::<TitleBlockValues>(memo, cold, keys.page_refs.iter().copied(), state),
    );
    put(TocRows::FAMILY, check::<TocRows>(memo, cold, [()], state));
    put(
        WirePropsOf::FAMILY,
        check::<WirePropsOf>(memo, cold, keys.potentials.iter().copied(), state),
    );
    put(
        XrefTextOf::FAMILY,
        check::<XrefTextOf>(memo, cold, keys.reps.iter().copied(), state),
    );
    let swept: Vec<&str> = n.keys().copied().collect();
    assert_eq!(swept, SWEPT, "the sweep reads exactly the listed families");
    n
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

/// The colour-profile edit of the last state: the first colour row of `ProjectSettings.extra` gets another
/// colour. The model after it, at the next revision (the Session's own snapshot is not changed).
fn with_edited_profile(model: &Model) -> (Model, Vec<ew_model::AnyKey>) {
    let before = model.project().clone();
    let mut project = (*before).clone();
    let profile = project
        .settings
        .extra
        .get_mut("colour_profile")
        .expect("the session has a colour profile");
    profile["rows"][0]["colour"] = json!(["RD"]);
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

    // The profile edit goes to the memo store directly: no command edits the project settings.
    model = session.world().model.clone();
    let (after, keys) = with_edited_profile(&model);
    session.memo().expect("memoising session").advance(&after, &keys);
    visit(session, "16 colour profile edit", &after);
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

/// The guard: the swept list is the memoised families of `ew_derive::FAMILIES`.
fn assert_family_list() {
    use ew_derive::{FAMILIES, Policy};
    let mut memoised: Vec<&str> = FAMILIES
        .iter()
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
    // five minutes over the 16 states in a debug build. One read in `ORACLE_ONE_IN`, counted over all reads
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
