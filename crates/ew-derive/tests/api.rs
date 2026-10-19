// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tracked-read context itself (ADR-0002 decisions 1, 2, 4; ARCH §3 rule 1): tracking, the
//! request-scoped cache, cycle detection, family ownership, the profile pin, and the API guarantee that a
//! query cannot obtain the model.
//!
//! **How the guarantee is kept.** A query is `fn run(q: &Q, key) -> Arc<V>`; `Q`'s fields are private and
//! none of its methods, nor those of `TableView` and `IdxView`, returns a `Model`, a raw `Table`, a raw
//! index or an `Op`. The `compile_fail` example on `Q` (run with `cargo test --doc -p ew-derive`) proves
//! that the model field cannot be named; `query_bodies_see_only_tracked_accessors` below compiles a query
//! that uses every kind of accessor, each of which is observed by the tracker.

mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::{Fx, id};
use ew_derive::{
    DepKey, DeriveNote, DeviceTag, EW_CORE_018, IxKey, NoteSeverity, Policy, ProfileStatus, Q, Query,
    RecordingTracker, StructureOf,
};
use ew_model::{AnyRow, Entity, IndexName, Op, Rev};
use ew_schema::{Device, Kind, Page};
use serde_json::json;

fn small() -> Fx {
    let mut fx = Fx::new();
    fx.node(0x10, "location", "J-E-2416", None)
        .page(0x101, 101, None, "multi_line", &[("location", 0x10)], "Power")
        .fndef(
            0x500,
            ["ProtectionOvercurrent", "MCB", "1P"],
            &[(0, "1"), (1, "2")],
        )
        .symbol(0x510, 0x500, json!("origin"), &[(0, 0, 0, 0)])
        .device(0x1000, "F", Some(1))
        .function(0x1001, Some(0x1000), 0x500, true)
        .rep(0x1002, 0x101, 0x510, &[0x1001], 40_000_000, 40_000_000);
    fx
}

#[test]
fn reads_are_reported_with_their_stamps() {
    let m = small().build();
    let rec = Rc::new(RecordingTracker::new());
    let q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
    let tag = q.get::<DeviceTag>(id(0x1000));
    assert_eq!(tag.as_ref().as_ref().unwrap().full, "+J-E-2416-101F1");
    let reads = rec.reads();
    let dev_key = (Kind::Device, id::<Device>(0x1000).cast());
    assert!(
        reads
            .iter()
            .any(|r| r.dep == DepKey::Row(dev_key) && r.query == Some("device_tag"))
    );
    assert!(reads.iter().any(|r| r.dep
        == DepKey::IndexKey(
            IndexName::FunctionsByDevice,
            IxKey::Id(id::<Device>(0x1000).cast())
        )
        && r.query == Some("main_function")));
    assert!(reads.iter().any(|r| r.dep == DepKey::Project));
    // Every read happened inside a query (none at top level), at revision 0 of a freshly built model.
    assert!(reads.iter().all(|r| r.query.is_some() && r.observed.0 == 0));
}

#[test]
fn absent_rows_are_reported_against_the_table() {
    let m = small().build();
    let rec = Rc::new(RecordingTracker::new());
    let q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
    assert!(q.pages().get(id::<Page>(0x999)).is_none());
    let r = rec.reads();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].dep, DepKey::Row((Kind::Page, id::<Page>(0x999).cast())));
    assert_eq!(r[0].query, None);
}

#[test]
fn each_key_is_evaluated_once_per_q() {
    let m = small().build();
    let rec = Rc::new(RecordingTracker::new());
    let mut q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
    let a = q.get::<DeviceTag>(id(0x1000));
    let b = q.get::<DeviceTag>(id(0x1000));
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(q.diagnostics().cached("device_tag"), 1);
    let evaluated = rec.evaluated();
    assert_eq!(evaluated.iter().filter(|(f, _)| *f == "device_tag").count(), 1);
    assert_eq!(evaluated.iter().filter(|(f, _)| *f == "structure_of").count(), 1);
    // A fresh Q starts cold.
    let mut q2 = Q::new(&m);
    assert_eq!(q2.diagnostics().cached("device_tag"), 0);
    assert_eq!(*q2.get::<DeviceTag>(id(0x1000)), *a);
}

#[derive(Debug)]
struct SelfCycle;

impl Query for SelfCycle {
    type Key = u8;
    type Value = u8;
    const FAMILY: &'static str = "test.self_cycle";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &u8) -> Arc<u8> {
        q.get::<SelfCycle>(*key)
    }
}

#[test]
#[should_panic(expected = "query cycle")]
fn a_cycle_panics() {
    let m = small().build();
    let q = Q::new(&m);
    let _ = q.get::<SelfCycle>(1);
}

#[derive(Debug)]
struct Impostor;

impl Query for Impostor {
    type Key = Id;
    type Value = ();
    const FAMILY: &'static str = "structure_of";
    const POLICY: Policy = Policy::Cold;

    fn run(_q: &Q, _key: &Id) -> Arc<()> {
        Arc::new(())
    }
}

type Id = ew_ids::Id<Device>;

#[test]
#[should_panic(expected = "share the family name")]
fn two_query_types_cannot_share_a_family() {
    let m = small().build();
    let q = Q::new(&m);
    let _ = q.get::<StructureOf>(id(0x1000));
    let _ = q.get::<Impostor>(id(0x1000));
}

#[test]
fn the_pinned_profile_is_resolved() {
    let m = small().build();
    let q = Q::new(&m);
    assert_eq!(q.profile().version, "2027.1");
    assert_eq!(*q.profile_status(), ProfileStatus::Pinned);

    let mut fx = small();
    fx.project_set("derivation_profile", json!("2099.1"));
    let m = fx.build();
    let q = Q::new(&m);
    assert_eq!(
        *q.profile_status(),
        ProfileStatus::Unknown {
            requested: "2099.1".into()
        }
    );
    assert_eq!(q.profile().version, "2027.1");
}

/// ADR-0003: an unknown pinned profile is never silent. Every `Q` over the snapshot carries an
/// error-severity note with the integrity code EW-CORE-018, before any query ran.
#[test]
fn an_unknown_profile_is_noted_as_an_error() {
    let mut fx = small();
    fx.project_set("derivation_profile", json!("2099.1"));
    let m = fx.build();
    let mut q = Q::new(&m);
    let notes = q.diagnostics().notes();
    assert_eq!(
        notes,
        [DeriveNote::UnknownProfile {
            requested: "2099.1".into(),
            used: "2027.1".into()
        }]
    );
    assert_eq!(notes[0].severity(), NoteSeverity::Error);
    assert_eq!(notes[0].code(), Some(EW_CORE_018));
    assert!(Q::new(&small().build()).diagnostics().notes().is_empty());
}

/// `Q` exposes no untracked state: its `Debug` text carries no revision and no cache state (F4), so two
/// contexts over different revisions print alike.
#[test]
fn debug_shows_no_untracked_state() {
    let m = small().build();
    let m2 = bump_counter(&m, 2, Rev(5));
    let (q, q2) = (Q::new(&m), Q::new(&m2));
    let _ = q.get::<DeviceTag>(id(0x1000));
    assert_eq!(format!("{q:?}"), format!("{q2:?}"));
    assert!(!format!("{q:?}").contains('5'));
}

/// `m` with device 0x1000's counter set to `counter`, committed as revision `rev`.
fn bump_counter(m: &ew_model::Model, counter: u32, rev: Rev) -> ew_model::Model {
    let before = m.row(&Device::key(id(0x1000))).unwrap();
    let mut after = (**before.get::<Device>().unwrap()).clone();
    after.tag.counter = Some(counter);
    m.apply(&[Op::update(before, AnyRow::of(after))], rev).unwrap()
}

/// Reads carry the stamps they observed: after a commit at revision 3 the changed device row reports 3,
/// rows and buckets the commit did not touch keep 0, and the whole device table reports 3 (F11).
#[test]
fn reads_report_non_zero_stamps_after_a_commit() {
    let m = bump_counter(&small().build(), 7, Rev(3));
    let rec = Rc::new(RecordingTracker::new());
    let q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
    assert_eq!(
        q.get::<DeviceTag>(id(0x1000)).as_ref().as_ref().unwrap().full,
        "+J-E-2416-101F7"
    );
    let _ = q.devices().len();
    let reads = rec.reads();
    let observed = |dep: DepKey| {
        reads
            .iter()
            .find(|r| r.dep == dep)
            .map(|r| r.observed)
            .unwrap_or_else(|| panic!("no read of {dep:?}"))
    };
    let dev_key = (Kind::Device, id::<Device>(0x1000).cast());
    assert_eq!(observed(DepKey::Row(dev_key)), Rev(3));
    assert_eq!(observed(DepKey::Table(Kind::Device)), Rev(3));
    let page_key = (Kind::Page, id::<Page>(0x101).cast());
    assert_eq!(observed(DepKey::Row(page_key)), Rev(0));
    assert_eq!(
        observed(DepKey::IndexKey(
            IndexName::FunctionsByDevice,
            IxKey::Id(id::<Device>(0x1000).cast())
        )),
        Rev(0)
    );
    // An absent row reports the table's stamp, an absent index key the index's.
    assert!(q.devices().get(id(0x9999)).is_none());
    let absent = rec.reads().last().cloned().unwrap();
    assert_eq!(
        absent.dep,
        DepKey::Row((Kind::Device, id::<Device>(0x9999).cast()))
    );
    assert_eq!(absent.observed, Rev(3));
}

/// The public surface hands out no `Model`, `Table`, `RevIndex` or `Op` (ARCH §3 rule 1, ADR-0002
/// amendment 2): every `pub fn` signature and `pub` field of the crate's sources is scanned; the only
/// `Model` is the input of `Q::new` and `Q::with_tracker`. The `compile_fail` doctests on `Q` and
/// `Q::diagnostics` prove that the private field and the untracked handle cannot be reached from `&Q`.
#[test]
fn the_public_api_exposes_no_raw_model_access() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<_> = std::fs::read_dir(&src)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    files.sort();
    assert!(files.len() >= 10, "found {} source files", files.len());
    let banned = ["Model", "Table", "RevIndex", "Op", "BaseIndexes"];
    let mut checked = 0;
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        // Drop the unit-test modules: they are not part of the API.
        let text = text.split("#[cfg(test)]").next().unwrap();
        let mut rest = text;
        while let Some(at) = rest.find("pub fn ") {
            let sig_end = rest[at..].find(['{', ';']).map_or(rest.len(), |e| at + e);
            let sig = &rest[at..sig_end];
            checked += 1;
            for word in banned {
                let hit = sig
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                    .any(|w| w == word);
                let allowed = word == "Model"
                    && (sig.starts_with("pub fn new(model: &Model)")
                        || sig.starts_with("pub fn with_tracker(model: &Model,"));
                assert!(!hit || allowed, "{}: `{sig}` exposes {word}", file.display());
            }
            rest = &rest[sig_end..];
        }
        for line in text.lines() {
            let line = line.trim_start();
            if let Some(field) = line.strip_prefix("pub ")
                && let Some((name, ty)) = field.split_once(": ")
                && name.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                for word in banned {
                    let hit = ty
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .any(|w| w == word);
                    assert!(!hit, "{}: field `{line}` exposes {word}", file.display());
                }
            }
        }
    }
    assert!(checked > 30, "only {checked} signatures scanned");
}

/// A query body that uses every kind of accessor `Q` offers. It compiles against `&Q` alone; there is
/// nothing else to reach the model through.
#[derive(Debug)]
struct EveryAccessor;

impl Query for EveryAccessor {
    type Key = ();
    type Value = usize;
    const FAMILY: &'static str = "test.every_accessor";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, _key: &()) -> Arc<usize> {
        let mut n = q.devices().len() + q.pages().ids().count() + q.functions().iter().count();
        n += q.idx().reps_by_page(id(0x101)).count();
        n += q.idx().functions_of_device(id(0x1000)).count();
        n += q.idx().connected_cps_of_function(id(0x1001)).len();
        n += q.symbols().len() + q.fndefs().len();
        n += usize::from(q.project().derivation_profile.as_str() == "2027.1");
        n += q.reserved(Kind::Revision).count();
        n += usize::from(q.get::<DeviceTag>(id(0x1000)).is_some());
        Arc::new(n)
    }
}

#[test]
fn query_bodies_see_only_tracked_accessors() {
    let m = small().build();
    let rec = Rc::new(RecordingTracker::new());
    let q = Q::with_tracker(&m, Box::new(Rc::clone(&rec)));
    // 1 device, 1 page, 1 function, 1 rep on the page, 1 function of the device, no connected CP,
    // 1 symbol and 1 fndef, the profile pin, no revision, the tag.
    assert_eq!(*q.get::<EveryAccessor>(()), 9);
    let reads: Vec<DepKey> = rec
        .reads()
        .into_iter()
        .filter(|r| r.query == Some("test.every_accessor"))
        .map(|r| r.dep)
        .collect();
    for expected in [
        DepKey::Table(Kind::Device),
        DepKey::Table(Kind::Page),
        DepKey::Table(Kind::Function),
        DepKey::Table(Kind::Symbol),
        DepKey::Table(Kind::Fndef),
        DepKey::Index(IndexName::ConnectionsByEndpoint),
        DepKey::Project,
        DepKey::Reserved,
    ] {
        assert!(reads.contains(&expected), "missing read {expected:?}");
    }
}
