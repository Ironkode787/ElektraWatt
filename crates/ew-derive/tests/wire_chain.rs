// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

// Applies raw ops to a snapshot (`Model::apply_unchecked`, `raw-read`): a reader of ew-model's raw API
// (ADR-0001 amendment 2), the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

//! The wire-property chain reads the profile through queries only (WP-S1.07 part 2, ADR-0002 amendment 4
//! "Query shapes", ARCH §6.4 rows `settings_member`, `profile_row_for`, `wire_props_of`), on a small wired
//! project: four potentials (L 230 V AC, L+ 24 V DC, M, PE) and a strip of three terminals that the PE
//! potential reaches by three wires.
//!
//! - `profile_edit_reexecutes_only_affected_potentials`: a colour row that one potential selects is
//!   edited; after the advance only that potential's `wire_props_of` re-executes, the other `profile_row_for`
//!   memos cut off, and every value equals a cold `Q`.
//! - `unrelated_project_edit_reexecutes_no_wire_props`: an edit of the project row that touches no profile
//!   member (a settings member the chain does not read, a project member) re-executes no `wire_props_of`.
//! - `pe_rule_reads_each_strip_once`: three PE wires to one strip make two `end_class` memos (the strip, and
//!   the device of the PE source), not one scan per wire end; the PE wires take the highest class at their
//!   ends.
//! - `wire_props_reads_no_project_row`: through a recording tracker, `wire_props_of` reads no project row;
//!   only `settings_member` does.
//! - `a_profile_default_and_notes_are_read_as_parts`: the level-1 default and the profile notes reach the
//!   wires through their own families: a default edit and an overlap note each re-execute every
//!   `wire_props_of`, and the profile rows cut off.

mod common;

use std::collections::BTreeMap;
use std::rc::Rc;

use common::{Fx, id, uid};
use ew_derive::{
    DepKey, EndClass, EndScope, MemoStore, PotentialId, PotentialNameOf, PotentialPartition, PropSource, Q,
    RecordingTracker, SizeRule, WirePropsMap, WirePropsOf,
};
use ew_model::{AnyRow, Model, Op};
use ew_schema::{CircuitClass, Csa, Device, Kind, Project};
use ew_units::Dec3;
use serde_json::{Value, json};

const FNDEF: u128 = 0x500;
const SYM: u128 = 0x510;
const P1: u128 = 0x100;
const STRIP: u128 = 0x5000;
/// The PE terminals of the strip; each has the function `T + 0x1000`.
const TERMINALS: [u128; 3] = [0x4001, 0x4002, 0x4003];
/// The terminals of the strip that carry L and L+ on their second CP (no PE wire ends on them).
const OTHERS: [u128; 2] = [0x4004, 0x4005];
/// The PE source function.
const PE_SRC: u128 = 0x1309;

/// A plain function `n` on its own device, drawn on page `P1`.
fn func(fx: &mut Fx, n: u128) {
    fx.device(n + 0x20_000, "K", Some(n as u32));
    fx.function(n, Some(n + 0x20_000), FNDEF, true);
    fx.rep(n + 0x10_000, P1, SYM, &[n], 0, n as i64 * 1_000_000);
}

/// A wire `n` from CP `a` to CP `b`, each `(function, idx)`.
fn wire(fx: &mut Fx, n: u128, a: (u128, u16), b: (u128, u16)) {
    fx.push(json!({
        "$k": "connection", "id": uid(n), "page": uid(P1),
        "a": {"cp": [uid(a.0), a.1]}, "b": {"cp": [uid(b.0), b.1]},
    }));
}

/// A potential definition `n` at CP `at` named `name`, with the members of `extra`.
fn def(fx: &mut Fx, n: u128, at: (u128, u16), name: &str, extra: Value) {
    let mut r = json!({"$k": "potential_def", "id": uid(n), "at": {"cp": [uid(at.0), at.1]}, "name": name});
    for (k, v) in extra.as_object().expect("an object") {
        r[k] = v.clone();
    }
    fx.push(r);
}

/// The colour profile: one row per potential type.
fn colour_profile() -> Value {
    json!({"key": "co.test", "rows": [
        {"select": {"ptype": ["l"]}, "colour": ["BK"]},
        {"select": {"ptype": ["l_plus"]}, "colour": ["BU"]},
        {"select": {"ptype": ["m"]}, "colour": ["WH"]},
        {"select": {"ptype": ["pe"]}, "colour": ["GN", "YE"]},
    ]})
}

/// The size profile: class rules and a row for Control and Mains.
fn size_profile() -> Value {
    json!({"key": "co.test", "class_rules": [
        {"select": {"ptype": ["pe", "pen", "fe"]}, "class": "pe"},
        {"select": {"ac_dc": "ac", "u_min": "50"}, "class": "mains"},
    ], "rows": [
        {"class": "control", "csa": "0.75"},
        {"class": "mains", "csa": "1.5"},
    ]})
}

/// The project:
///
/// - L (230 V AC, Mains): 0x1001.0 - 0x1002.0 (wire 0x2001) and 0x1001.0 - terminal 4, CP 1 (wire 0x2004).
/// - L+ (24 V DC, Control): 0x1003.0 - 0x1004.0 (wire 0x2002) and 0x1003.0 - terminal 5, CP 1 (wire 0x2005).
/// - M: 0x1005.0 - 0x1006.0 (wire 0x2003).
/// - PE: the source function's CP 0 to CP 0 of each of the terminals 1 to 3 (wires 0x2101..0x2103). The
///   function of a PE terminal touches no other potential, so a PE wire reads the others through the strip.
fn fixture() -> Fx {
    let mut fx = Fx::new();
    fx.project_set(
        "settings",
        json!({"colour_profile": colour_profile(), "size_profile": size_profile()}),
    );
    fx.fndef(FNDEF, ["Misc", "Misc", "Plain"], &[(0, "1"), (1, "2")]);
    fx.symbol(SYM, FNDEF, json!("origin"), &[(0, 0, 0, 0), (1, 1, 0, 1_000_000)]);
    fx.page(P1, 1, None, "multi_line", &[], "one");
    for f in [0x1001, 0x1002, 0x1003, 0x1004, 0x1005, 0x1006, PE_SRC] {
        func(&mut fx, f);
    }
    fx.device(STRIP, "X", Some(1)).with("strip", json!({}));
    for (seq, t) in (1..).zip(TERMINALS.into_iter().chain(OTHERS)) {
        fx.device(t, "X", None).with(
            "terminal",
            json!({"strip": uid(STRIP), "seq": seq, "designation": seq.to_string()}),
        );
        fx.function(t + 0x1000, Some(t), FNDEF, true);
        fx.rep(
            t + 0x10_000,
            P1,
            SYM,
            &[t + 0x1000],
            1_000_000,
            t as i64 * 1_000_000,
        );
    }
    wire(&mut fx, 0x2001, (0x1001, 0), (0x1002, 0));
    wire(&mut fx, 0x2002, (0x1003, 0), (0x1004, 0));
    wire(&mut fx, 0x2003, (0x1005, 0), (0x1006, 0));
    wire(&mut fx, 0x2004, (0x1001, 0), (OTHERS[0] + 0x1000, 1));
    wire(&mut fx, 0x2005, (0x1003, 0), (OTHERS[1] + 0x1000, 1));
    for (i, t) in TERMINALS.iter().enumerate() {
        wire(&mut fx, 0x2101 + i as u128, (PE_SRC, 0), (t + 0x1000, 0));
    }
    def(
        &mut fx,
        0x5001,
        (0x1001, 0),
        "L",
        json!({"ptype": "l", "voltage": "230", "env": {"ac_dc": "ac"}}),
    );
    def(
        &mut fx,
        0x5002,
        (0x1003, 0),
        "L+",
        json!({"ptype": "l_plus", "voltage": "24", "env": {"ac_dc": "dc"}}),
    );
    def(&mut fx, 0x5003, (0x1005, 0), "M", json!({"ptype": "m"}));
    def(&mut fx, 0x5004, (PE_SRC, 0), "PE", json!({"ptype": "pe"}));
    fx
}

/// The potentials of a model by name.
fn potentials(q: &Q) -> BTreeMap<String, PotentialId> {
    q.get::<PotentialPartition>(())
        .potentials()
        .map(|p| (q.get::<PotentialNameOf>(p.id).name.text.clone(), p.id))
        .collect()
}

/// `model` with the project row edited by `edit`, at the next revision, and the touched keys.
fn edited(model: &Model, edit: impl FnOnce(&mut Project)) -> (Model, Vec<ew_model::AnyKey>) {
    let before = model.project().clone();
    let mut project = (*before).clone();
    edit(&mut project);
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

/// Reads `wire_props_of` of every potential through `q`, in name order.
fn read_all(q: &Q, ps: &BTreeMap<String, PotentialId>) -> Vec<WirePropsMap> {
    ps.values().map(|p| (*q.get::<WirePropsOf>(*p)).clone()).collect()
}

/// The potentials of `before` whose wires take a colour from colour row `row`.
fn using_colour_row(before: &[WirePropsMap], ps: &BTreeMap<String, PotentialId>, row: usize) -> usize {
    ps.keys()
        .zip(before)
        .filter(|(_, map)| {
            map.wires.values().any(|w| {
                matches!(
                    w.colour.as_ref().map(|c| &c.source),
                    Some(PropSource::ColourRow { row: r, .. }) if *r == row
                )
            })
        })
        .count()
}

#[test]
fn profile_edit_reexecutes_only_affected_potentials() {
    let model = fixture().build();
    let store = MemoStore::new(&model);
    let ps = potentials(&store.query());
    assert_eq!(ps.len(), 4, "{ps:?}");
    let before = read_all(&store.query(), &ps);
    // The L+ potential alone selects colour row 1.
    let affected = using_colour_row(&before, &ps, 1);
    assert_eq!(affected, 1);

    let (after, keys) = edited(&model, |p| {
        p.settings
            .extra
            .get_mut("colour_profile")
            .expect("a colour profile")["rows"][1]["colour"] = json!(["RD"]);
    });
    store.advance(&after, &keys);
    let memo = store.query();
    let cold = Q::new(&after);
    let got = read_all(&memo, &ps);
    assert_eq!(got, read_all(&cold, &ps), "memoised equals cold");
    assert_ne!(got, before, "the edit changed a value");

    let stats = store.commit_stats();
    let wire_props = stats.family("wire_props_of");
    assert_eq!(wire_props.re_executions, affected as u64, "{wire_props:?}");
    assert!(
        wire_props.re_executions < ps.len() as u64,
        "not every potential re-executes"
    );
    let rows = stats.family("profile_row_for");
    assert!(rows.cut_offs >= 1, "the unchanged rows cut off: {rows:?}");
    // The PE wires read the changed L+ row through the strip scan, which gave the same class again.
    let scan = stats.family("end_class");
    assert_eq!((scan.re_executions, scan.cut_offs), (1, 1), "{scan:?}");
    assert!(
        rows.re_executions > rows.cut_offs,
        "the changed row did not cut off: {rows:?}"
    );
}

#[test]
fn unrelated_project_edit_reexecutes_no_wire_props() {
    let model = fixture().build();
    let store = MemoStore::new(&model);
    let ps = potentials(&store.query());
    let before = read_all(&store.query(), &ps);

    let (after, keys) = edited(&model, |p| {
        p.settings
            .extra
            .insert("numbering".into(), json!({"default": "none"}));
        p.extra
            .insert("note".into(), json!("an unrelated project member"));
    });
    store.advance(&after, &keys);
    let memo = store.query();
    let got = read_all(&memo, &ps);
    assert_eq!(got, read_all(&Q::new(&after), &ps), "memoised equals cold");
    assert_eq!(got, before, "the edit changed no wire value");

    let stats = store.commit_stats();
    for family in [
        "wire_props_of",
        "wire_profiles",
        "profile_row_for",
        "end_class",
        "wire_default_props",
    ] {
        assert_eq!(stats.family(family).re_executions, 0, "{family}: {stats:?}");
    }
    // The projection memos of the members re-ran and cut off.
    let members = stats.family("settings_member");
    assert!(members.re_executions >= 1);
    assert!(members.cut_offs >= 1);
}

#[test]
fn pe_rule_reads_each_strip_once() {
    let model = fixture().build();
    let store = MemoStore::new(&model);
    let q = store.query();
    let ps = potentials(&q);
    let maps = read_all(&q, &ps);
    let pe = &maps[ps.keys().position(|n| n == "PE").expect("the PE potential")];
    assert_eq!(pe.wires.len(), 3, "three PE wires to one strip");

    // The PE wires take the highest class at their ends: the strip holds Mains (L) and Control (L+).
    for (w, e) in &pe.wires {
        assert_eq!(e.class, Some(CircuitClass::Mains), "{w:?}");
        let csa = e.csa.as_ref().expect("a cross-section");
        assert_eq!(csa.value, Csa::Mm2(Dec3(1_500)));
        assert!(matches!(
            &csa.source,
            PropSource::SizeRow {
                class: CircuitClass::Mains,
                rule: SizeRule::PeHighestAtEnds,
                ..
            }
        ));
        let colour = e.colour.as_ref().expect("a colour");
        assert!(matches!(&colour.source, PropSource::ColourRow { row: 3, .. }));
    }
    // The other potentials took their own class.
    let l = &maps[ps.keys().position(|n| n == "L").expect("L")];
    assert!(l.wires.values().all(|e| e.class == Some(CircuitClass::Mains)));
    let lp = &maps[ps.keys().position(|n| n == "L+").expect("L+")];
    assert!(lp.wires.values().all(|e| e.class == Some(CircuitClass::Control)));

    // Six wire ends of device kind, two scopes: the strip and the PE source's device.
    let memory = store.memory();
    let scopes = memory
        .iter()
        .find(|m| m.family == "end_class")
        .map_or(0, |m| m.memos);
    assert_eq!(
        scopes, 2,
        "one end_class memo per strip or device, not per wire end"
    );
    assert_eq!(
        *q.get::<EndClass>(EndScope::Strip(id::<Device>(STRIP))),
        Some(CircuitClass::Mains)
    );
    // The PE source's device holds only the PE potential: no class ranks.
    assert_eq!(
        *q.get::<EndClass>(EndScope::Device(id::<Device>(PE_SRC + 0x20_000))),
        None
    );
    // The strip device itself holds no function: a device scope of it is empty.
    assert_eq!(*q.get::<EndClass>(EndScope::Device(id::<Device>(STRIP))), None);
}

#[test]
fn wire_props_reads_no_project_row() {
    let model = fixture().build();
    let rec = Rc::new(RecordingTracker::new());
    let q = Q::with_tracker(&model, Box::new(Rc::clone(&rec)));
    let ps = potentials(&q);
    let maps = read_all(&q, &ps);
    assert!(maps.iter().all(|m| !m.wires.is_empty()));
    let reads = rec.reads();
    assert!(
        !reads
            .iter()
            .any(|r| r.query == Some("wire_props_of") && r.dep == DepKey::Project),
        "wire_props_of read the project row"
    );
    // The project row is read by the settings members, and by nothing else of the profile chain.
    assert!(
        reads
            .iter()
            .any(|r| r.query == Some("settings_member") && r.dep == DepKey::Project)
    );
    for family in [
        "wire_profiles",
        "profile_row_for",
        "wire_default_props",
        "wire_profile_notes",
        "end_class",
    ] {
        assert!(
            !reads
                .iter()
                .any(|r| r.query == Some(family) && r.dep == DepKey::Project),
            "{family} read the project row"
        );
    }
}

#[test]
fn a_profile_default_and_notes_are_read_as_parts() {
    let model = fixture().build();
    let store = MemoStore::new(&model);
    let ps = potentials(&store.query());
    let before = read_all(&store.query(), &ps);

    // A project default that sets a member no level above it sets: every wire changes, so every potential
    // re-executes.
    let (after, keys) = edited(&model, |p| {
        p.settings
            .extra
            .insert("wire_default".into(), json!({"insulation": "H07V-K"}));
    });
    store.advance(&after, &keys);
    let got = read_all(&store.query(), &ps);
    assert_eq!(got, read_all(&Q::new(&after), &ps), "memoised equals cold");
    assert_ne!(got, before);
    let stats = store.commit_stats();
    assert_eq!(stats.family("wire_props_of").re_executions, ps.len() as u64);
    // The profile rows do not depend on the default.
    assert_eq!(
        stats.family("profile_row_for").re_executions,
        stats.family("profile_row_for").cut_offs
    );

    // An overlap note appears with a second colour row of the same selector and another colour: the notes
    // value changed, so every potential re-executes to push them.
    let (after2, keys2) = edited(&after, |p| {
        let rows = p
            .settings
            .extra
            .get_mut("colour_profile")
            .expect("a colour profile")["rows"]
            .as_array_mut()
            .expect("rows");
        rows.push(json!({"select": {"ptype": ["m"]}, "colour": ["RD"]}));
    });
    store.advance(&after2, &keys2);
    let memo = store.query();
    let cold = Q::new(&after2);
    assert_eq!(read_all(&memo, &ps), read_all(&cold, &ps));
    assert_eq!(
        store.commit_stats().family("wire_props_of").re_executions,
        ps.len() as u64
    );
    // The overlapping row changes no row of any potential (the first of two equal rows wins): the profile rows
    // cut off, only the notes moved.
    let rows = store.commit_stats().family("profile_row_for");
    assert_eq!(rows.re_executions, rows.cut_offs, "{rows:?}");
}
