// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The generic commands: `fields.set`, `edit.delete` and `numbering.renumber`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    BULK_ABOVE, DeleteSelection, DeviceScope, KindName, RenumberDevices, RenumberOptions, SetFields,
    TargetRef,
};
use ew_commands::{CmdError, Command, Cost, Engine};
use ew_ids::Id;
use ew_mltext::MlText;
use ew_model::{AnyKey, Model};
use ew_schema::{
    AnyId, Connection, Csa, Device, FrozenTag, Function, Kind, LabelMode, Page, Record, Representation,
    TagSign,
};
use serde_json::{Value, json};

fn set(kind: KindName, targets: &[AnyId], path: &str, value: Value) -> SetFields {
    SetFields {
        kind,
        targets: targets.to_vec(),
        path: path.into(),
        value,
    }
}

fn set_device(device: Id<Device>, path: &str, value: Value) -> SetFields {
    set(KindName::Device, &[device.cast()], path, value)
}

fn device(e: &Engine, id: Id<Device>) -> std::sync::Arc<Device> {
    e.model().table::<Device>().get(&id).expect("device").clone()
}

fn target(kind: KindName, id: AnyId) -> TargetRef {
    TargetRef { kind, id }
}

fn invalid_path(err: CmdError) -> String {
    match err {
        CmdError::InvalidParams { command, path, .. } => {
            assert_eq!(command, "fields.set");
            path
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// fields.set

#[test]
fn set_device_function_text() {
    let mut e = engine(golden_model());
    e.execute(&set_device(DEV_102F2, "function_text", json!("Feed")))
        .unwrap();
    assert_eq!(device(&e, DEV_102F2).function_text, MlText::neutral("Feed"));
    assert_eq!(e.model().rev().0, 1);
}

#[test]
fn set_tag_counter() {
    let mut e = engine(golden_model());
    e.execute(&set_device(DEV_102F2, "tag.counter", json!(9)))
        .unwrap();
    assert_eq!(device(&e, DEV_102F2).tag.counter, Some(9));
    // A reset removes the counter; online numbering gives the placed device the lowest free one again.
    e.execute(&set_device(DEV_102F2, "tag.counter", Value::Null))
        .unwrap();
    assert_eq!(device(&e, DEV_102F2).tag.counter, Some(1));
}

#[test]
fn set_strip_aspect_member() {
    let mut e = engine(golden_model());
    e.execute(&set_device(DEV_X1031, "strip.label_mode", json!("designation")))
        .unwrap();
    let strip = device(&e, DEV_X1031);
    assert_eq!(
        strip.aspects.strip.as_ref().map(|s| s.label_mode.clone()),
        Some(LabelMode::Designation)
    );
}

#[test]
fn set_connection_override_creates_the_parent_and_null_resets_it() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let cmd = |v: Value| set(KindName::Connection, &[CON_W22.cast()], "overrides.csa", v);
    e.execute(&cmd(json!({"awg": 12}))).unwrap();
    let con = e.model().table::<Connection>().get(&CON_W22).unwrap().clone();
    assert_eq!(con.overrides.csa, Some(Csa::Awg(12)));
    e.execute(&cmd(Value::Null)).unwrap();
    assert_eq!(
        lines(e.model()),
        before,
        "a reset restores the exact record bytes"
    );
}

#[test]
fn set_page_title() {
    let mut e = engine(golden_model());
    let cmd = set(KindName::Page, &[PAGE_101.cast()], "title", json!("Main"));
    e.execute(&cmd).unwrap();
    let page = e.model().table::<Page>().get(&PAGE_101).unwrap().clone();
    assert_eq!(page.title, MlText::neutral("Main"));
}

#[test]
fn set_rejects_an_unknown_path() {
    let mut e = engine(golden_model());
    for path in ["nope", "id", "tag", "tag.letter.x", "strip.groups.0", ""] {
        let err = e.execute(&set_device(DEV_102F2, path, json!(1))).unwrap_err();
        assert_eq!(invalid_path(err), "/path", "{path}");
    }
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn set_rejects_a_kind_without_settable_fields() {
    let mut e = engine(golden_model());
    let err = e
        .execute(&set(KindName::Junction, &[J5.cast()], "style", json!("dot")))
        .unwrap_err();
    assert_eq!(invalid_path(err), "/path");
}

#[test]
fn set_rejects_an_aspect_path_on_a_device_without_the_aspect() {
    let mut e = engine(golden_model());
    for path in ["strip.label_mode", "terminal.seq", "cable.pre_assembled"] {
        let err = e.execute(&set_device(DEV_102F2, path, json!(1))).unwrap_err();
        assert_eq!(invalid_path(err), "/path", "{path}");
    }
    // The aspect of another kind is no help: the strip has no terminal aspect.
    let err = e
        .execute(&set_device(DEV_X1031, "terminal.rail_bonded", json!(true)))
        .unwrap_err();
    assert_eq!(invalid_path(err), "/path");
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn set_rejects_a_bad_value() {
    let mut e = engine(golden_model());
    let err = e
        .execute(&set_device(DEV_102F2, "tag.counter", json!("many")))
        .unwrap_err();
    assert_eq!(invalid_path(err), "/value");
    let err = e
        .execute(&set_device(DEV_X1031, "strip.label_mode", json!(7)))
        .unwrap_err();
    assert_eq!(invalid_path(err), "/value");
    // `null` on a member without a default leaves the record unreadable.
    let err = e
        .execute(&set_device(DEV_102F2, "tag.letter", Value::Null))
        .unwrap_err();
    assert_eq!(invalid_path(err), "/value");
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn set_on_a_missing_target_is_not_found() {
    let mut e = engine(golden_model());
    let missing: Id<Device> = id(0xdead);
    let err = e
        .execute(&set_device(missing, "tag.counter", json!(3)))
        .unwrap_err();
    assert_eq!(
        err,
        CmdError::NotFound {
            key: (Kind::Device, missing.cast())
        }
    );
}

#[test]
fn set_needs_distinct_targets_and_at_least_one() {
    let mut e = engine(golden_model());
    let err = e
        .execute(&set(KindName::Device, &[], "tag.counter", json!(1)))
        .unwrap_err();
    assert_eq!(invalid_path(err), "/targets");
    let dup = set(
        KindName::Device,
        &[DEV_102F2.cast(), DEV_101F1.cast(), DEV_102F2.cast()],
        "tag.counter",
        json!(1),
    );
    assert_eq!(invalid_path(e.execute(&dup).unwrap_err()), "/targets/2");
}

#[test]
fn set_over_several_targets_is_one_undo_step() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let cmd = set(
        KindName::Device,
        &[DEV_101F1.cast(), DEV_102F2.cast(), DEV_102G1.cast()],
        "function_text",
        json!("Protection"),
    );
    e.execute(&cmd).unwrap();
    for d in [DEV_101F1, DEV_102F2, DEV_102G1] {
        assert_eq!(device(&e, d).function_text, MlText::neutral("Protection"));
    }
    assert_eq!(e.history().entries().len(), 1);
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn set_null_on_an_absent_member_commits_nothing() {
    // A reset of a member that is absent changes nothing and commits no op.
    let mut e = engine(golden_model());
    let before = lines(e.model());
    e.execute(&set_device(DEV_102F2, "function_text", Value::Null))
        .unwrap();
    e.execute(&set_device(DEV_102F2, "process_tag", Value::Null))
        .unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// edit.delete

fn delete(targets: Vec<TargetRef>) -> DeleteSelection {
    DeleteSelection { targets }
}

fn exists(e: &Engine, key: AnyKey) -> bool {
    e.model().row(&key).is_some()
}

#[test]
fn delete_a_device_takes_its_functions_representations_and_connections() {
    let mut e = engine(golden_model());
    e.execute(&delete(vec![target(KindName::Device, DEV_102F2.cast())]))
        .unwrap();
    assert!(!exists(&e, (Kind::Device, DEV_102F2.cast())));
    for f in [FN_102F2, FN_102F2_AUX] {
        assert!(!exists(&e, (Kind::Function, f.cast())), "{f}");
    }
    for r in [REP_102F2, REP_102F2_AUX] {
        assert!(!exists(&e, (Kind::Representation, r.cast())), "{r}");
    }
    for c in [CON_W22, CON_W29] {
        assert!(!exists(&e, (Kind::Connection, c.cast())), "{c}");
    }
    // The junction no longer lists the removed wire (or is gone with its clean-up).
    if let Some(j) = e.model().table::<ew_schema::Junction>().get(&J5) {
        assert!(!j.sequence.contains(&CON_W22));
    }
    // Unrelated records stay.
    assert!(exists(&e, (Kind::Device, DEV_102G1.cast())));
    assert!(exists(&e, (Kind::Connection, CON_W11B.cast())));
}

#[test]
fn delete_a_representation_leaves_the_function_unplaced() {
    let mut e = engine(golden_model());
    e.execute(&delete(vec![target(
        KindName::Representation,
        REP_102F2_AUX.cast(),
    )]))
    .unwrap();
    assert!(!exists(&e, (Kind::Representation, REP_102F2_AUX.cast())));
    assert!(exists(&e, (Kind::Function, FN_102F2_AUX.cast())));
    assert!(exists(&e, (Kind::Device, DEV_102F2.cast())));
    let f = e.model().table::<Function>().get(&FN_102F2_AUX).unwrap().clone();
    assert_eq!(f.device, Some(DEV_102F2));
}

#[test]
fn delete_a_strip_with_terminals_is_forbidden() {
    let mut e = engine(golden_model());
    let err = e
        .execute(&delete(vec![target(KindName::Device, DEV_X1031.cast())]))
        .unwrap_err();
    assert!(matches!(err, CmdError::Model(_)), "{err:?}");
    assert_eq!(e.model().rev().0, 0);
    assert!(exists(&e, (Kind::Device, DEV_X1031.cast())));
}

#[test]
fn delete_a_missing_target_is_not_found_and_changes_nothing() {
    let mut e = engine(golden_model());
    let missing: AnyId = id::<()>(0xbeef);
    let err = e
        .execute(&delete(vec![
            target(KindName::Device, DEV_102F2.cast()),
            target(KindName::Page, missing),
        ]))
        .unwrap_err();
    assert_eq!(
        err,
        CmdError::NotFound {
            key: (Kind::Page, missing)
        }
    );
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn delete_skips_a_target_that_an_earlier_cascade_removed() {
    let mut e = engine(golden_model());
    let r = e.execute(&delete(vec![
        target(KindName::Device, DEV_102F2.cast()),
        target(KindName::Function, FN_102F2.cast()),
        target(KindName::Representation, REP_102F2.cast()),
        target(KindName::Device, DEV_102F2.cast()),
    ]));
    assert!(r.is_ok(), "{r:?}");
    assert!(!exists(&e, (Kind::Function, FN_102F2.cast())));
}

#[test]
fn delete_needs_at_least_one_target() {
    let mut e = engine(golden_model());
    let err = e.execute(&delete(vec![])).unwrap_err();
    assert!(matches!(err, CmdError::InvalidParams { .. }), "{err:?}");
}

/// The golden model plus `n` extra representations of `FN_102F2` on page 102.
fn model_with_extra_reps(n: usize) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    for i in 0..n {
        records.push(Record::Representation(rep(
            id(0x20_000 + i as u128),
            PAGE_102,
            vec![FN_102F2],
        )));
    }
    Model::from_records(p, records, library()).expect("golden plus representations")
}

#[test]
fn delete_is_bulk_above_500_records() {
    // (a) one device whose removal cascades over more than 500 records
    let model = model_with_extra_reps(BULK_ABOVE + 100);
    let cmd = delete(vec![target(KindName::Device, DEV_102F2.cast())]);
    assert_eq!(Command::cost(&cmd, &model), Cost::Bulk);
    let mut e = engine(model);
    let err = e.execute(&cmd).unwrap_err();
    assert!(matches!(err, CmdError::NeedsPreview { .. }), "{err:?}");
    // the golden model's own cascade is small
    assert_eq!(Command::cost(&cmd, &golden_model()), Cost::Interactive);
}

#[test]
fn many_targets_with_a_small_plan_stay_interactive() {
    // (b) 501 targets that are all the same representation: one record plus its cascade
    let mut targets = vec![target(KindName::Representation, REP_102F2_AUX.cast())];
    targets.resize(BULK_ABOVE + 1, targets[0]);
    let cmd = delete(targets);
    assert_eq!(Command::cost(&cmd, &golden_model()), Cost::Interactive);
    let mut e = engine(golden_model());
    e.execute(&cmd).unwrap();
    assert!(!exists(&e, (Kind::Representation, REP_102F2_AUX.cast())));
}

#[test]
fn missing_targets_cost_interactive_and_fail_not_found() {
    // (c) a plan that cannot be made costs `Interactive`; running it fails
    let cmd = delete(
        (0..BULK_ABOVE + 1)
            .map(|i| target(KindName::Device, id::<()>(0x10_000 + i as u128)))
            .collect(),
    );
    assert_eq!(Command::cost(&cmd, &golden_model()), Cost::Interactive);
    let mut e = engine(golden_model());
    let err = e.execute(&cmd).unwrap_err();
    assert!(matches!(err, CmdError::NotFound { .. }), "{err:?}");
}

#[test]
fn bulk_delete_runs_through_preview_and_commit() {
    let model = model_with_extra_reps(BULK_ABOVE + 100);
    let cmd = delete(vec![target(KindName::Device, DEV_102F2.cast())]);
    assert_eq!(Command::cost(&cmd, &model), Cost::Bulk);
    let before = lines(&model);
    let mut e = engine(model);
    let p = e.preview(&cmd).unwrap();
    e.commit(p).unwrap();
    assert!(!exists(&e, (Kind::Device, DEV_102F2.cast())));
    assert!(!exists(&e, (Kind::Representation, id::<()>(0x20_000))));
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// numbering.renumber

fn renumber(scope: DeviceScope, options: RenumberOptions) -> RenumberDevices {
    RenumberDevices {
        scope,
        scheme: Default::default(),
        include_frozen: false,
        options,
    }
}

fn project_scope() -> RenumberDevices {
    renumber(DeviceScope::Project, RenumberOptions::default())
}

/// The golden model with `DEV_102G1` turned into an `F` device, to the right of `DEV_102F2` on page 102.
fn two_f_model(edit: impl FnOnce(&mut Device, &mut Device)) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut devices = records.iter_mut().filter_map(|r| match r {
        Record::Device(d) if d.id == DEV_102F2 || d.id == DEV_102G1 => Some(d),
        _ => None,
    });
    let (mut a, mut b) = (devices.next().expect("device"), devices.next().expect("device"));
    if a.id != DEV_102F2 {
        std::mem::swap(&mut a, &mut b);
    }
    b.tag.letter = ident("F");
    edit(a, b);
    for r in &mut records {
        if let Record::Representation(rep) = r
            && rep.id == REP_102G1
        {
            rep.pos = pt(50 * MM10, 0);
        }
    }
    Model::from_records(p, records, library()).expect("valid model")
}

fn counter(e: &Engine, d: Id<Device>) -> Option<u32> {
    device(e, d).tag.counter
}

fn topology(model: &Model) -> Vec<(AnyKey, String)> {
    lines(model)
        .into_iter()
        .filter(|((kind, _), _)| {
            matches!(
                kind,
                Kind::Function | Kind::Representation | Kind::Connection | Kind::Junction
            )
        })
        .collect()
}

#[test]
fn renumber_assigns_counters_in_position_order() {
    let mut e = engine(two_f_model(|_, _| {}));
    e.execute(&set_device(DEV_102F2, "tag.counter", json!(7)))
        .unwrap();
    assert_eq!(counter(&e, DEV_102F2), Some(7));
    let p = e.preview(&project_scope()).unwrap();
    e.commit(p).unwrap();
    // Page 102, letter F: DEV_102F2 is left of DEV_102G1 although its ID is larger.
    assert_eq!(counter(&e, DEV_102F2), Some(1));
    assert_eq!(counter(&e, DEV_102G1), Some(2));
    // Page 101 is a group of its own.
    assert_eq!(counter(&e, DEV_101F1), Some(1));
}

#[test]
fn renumber_a_lone_device_becomes_one() {
    let mut e = engine(golden_model());
    e.execute(&set_device(DEV_102F2, "tag.counter", json!(7)))
        .unwrap();
    let p = e.preview(&project_scope()).unwrap();
    e.commit(p).unwrap();
    assert_eq!(counter(&e, DEV_102F2), Some(1));
    assert_eq!(counter(&e, DEV_101F1), Some(1));
    assert_eq!(counter(&e, DEV_102G1), Some(1));
    // Terminals are named by strip and designation: not renumbered.
    assert_eq!(counter(&e, DEV_T1), Some(1));
    assert_eq!(counter(&e, DEV_T3), Some(3));
    // The cable has no placed function: it keeps its counter.
    assert_eq!(counter(&e, DEV_3W1), Some(1));
}

#[test]
fn renumber_changes_no_identity_and_no_reference() {
    let mut e = engine(two_f_model(|_, _| {}));
    e.execute(&set_device(DEV_102F2, "tag.counter", json!(7)))
        .unwrap();
    let before = topology(e.model());
    let p = e.preview(&project_scope()).unwrap();
    e.commit(p).unwrap();
    assert_eq!(
        topology(e.model()),
        before,
        "functions, reps, connections, junctions"
    );
    let ids_before: Vec<_> = before.iter().map(|(k, _)| *k).collect();
    let ids_after: Vec<_> = topology(e.model()).iter().map(|(k, _)| *k).collect();
    assert_eq!(ids_before, ids_after);
}

#[test]
fn renumber_touches_only_counters() {
    let mut e = engine(two_f_model(|_, _| {}));
    e.execute(&set_device(DEV_102F2, "tag.counter", json!(7)))
        .unwrap();
    let before = device(&e, DEV_102F2);
    let p = e.preview(&project_scope()).unwrap();
    e.commit(p).unwrap();
    let after = device(&e, DEV_102F2);
    let mut expected = (*before).clone();
    expected.tag.counter = Some(1);
    assert_eq!(*after, expected);
}

#[test]
fn renumber_twice_changes_nothing_the_second_time() {
    let mut e = engine(two_f_model(|_, _| {}));
    e.execute(&set_device(DEV_102F2, "tag.counter", json!(7)))
        .unwrap();
    let p = e.preview(&project_scope()).unwrap();
    assert!(!p.ops().is_empty());
    e.commit(p).unwrap();
    let again = e.preview(&project_scope()).unwrap();
    assert!(again.ops().is_empty(), "{:?}", again.ops());
}

#[test]
fn renumber_only_nonconforming_keeps_conforming_counters() {
    // F2 = 7 and G1 (now F) = 1 do not collide: nothing changes.
    let mut e = engine(two_f_model(|f2, _| f2.tag.counter = Some(7)));
    let only = RenumberOptions {
        only_nonconforming: true,
        ..RenumberOptions::default()
    };
    let p = e.preview(&renumber(DeviceScope::Project, only)).unwrap();
    assert!(p.ops().is_empty(), "{:?}", p.ops());
}

#[test]
fn renumber_only_nonconforming_fixes_a_collision_and_a_missing_counter() {
    let mut e = engine(two_f_model(|f2, g1| {
        f2.tag.counter = Some(2);
        g1.tag.counter = Some(2);
    }));
    let only = RenumberOptions {
        only_nonconforming: true,
        ..RenumberOptions::default()
    };
    let p = e.preview(&renumber(DeviceScope::Project, only)).unwrap();
    e.commit(p).unwrap();
    // The first in position order keeps 2; the second takes the lowest free counter.
    assert_eq!(counter(&e, DEV_102F2), Some(2));
    assert_eq!(counter(&e, DEV_102G1), Some(1));

    let mut e = engine(two_f_model(|f2, g1| {
        f2.tag.counter = None;
        g1.tag.counter = Some(1);
    }));
    let p = e.preview(&renumber(DeviceScope::Project, only)).unwrap();
    e.commit(p).unwrap();
    assert_eq!(counter(&e, DEV_102G1), Some(1), "the conforming counter stays");
    assert_eq!(
        counter(&e, DEV_102F2),
        Some(2),
        "the free counter is not the kept one"
    );
}

#[test]
fn renumber_without_closing_gaps_keeps_free_counters() {
    let mut e = engine(two_f_model(|f2, g1| {
        f2.tag.counter = Some(7);
        g1.tag.counter = Some(7);
    }));
    let keep = RenumberOptions {
        close_gaps: false,
        ..RenumberOptions::default()
    };
    let p = e.preview(&renumber(DeviceScope::Project, keep)).unwrap();
    e.commit(p).unwrap();
    assert_eq!(counter(&e, DEV_102F2), Some(7));
    assert_eq!(counter(&e, DEV_102G1), Some(1));
}

#[test]
fn renumber_leaves_frozen_tags_alone() {
    let mut e = engine(two_f_model(|f2, g1| {
        f2.tag.counter = Some(7);
        g1.tag.frozen = Some(FrozenTag {
            text: ident("S51"),
            sign: TagSign::default(),
            extra: Default::default(),
        });
        g1.tag.counter = Some(5);
    }));
    let p = e.preview(&project_scope()).unwrap();
    e.commit(p).unwrap();
    assert_eq!(counter(&e, DEV_102G1), Some(5), "frozen: untouched");
    assert_eq!(counter(&e, DEV_102F2), Some(1));
}

fn frozen(text: &str) -> Option<FrozenTag> {
    Some(FrozenTag {
        text: ident(text),
        sign: TagSign::default(),
        extra: Default::default(),
    })
}

fn include_frozen(options: RenumberOptions) -> RenumberDevices {
    RenumberDevices {
        include_frozen: true,
        ..renumber(DeviceScope::Project, options)
    }
}

#[test]
fn renumber_include_frozen_unfreezes_and_numbers() {
    let mut e = engine(two_f_model(|f2, g1| {
        f2.tag.counter = Some(7);
        g1.tag.frozen = frozen("S51");
        g1.tag.counter = Some(5);
    }));
    let before = lines(e.model());
    let p = e.preview(&include_frozen(RenumberOptions::default())).unwrap();
    e.commit(p).unwrap();
    assert_eq!(counter(&e, DEV_102F2), Some(1));
    assert_eq!(counter(&e, DEV_102G1), Some(2));
    assert!(device(&e, DEV_102G1).tag.frozen.is_none());
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), before);
}

#[test]
fn renumber_include_frozen_with_only_nonconforming_keeps_conforming_counters() {
    let mut e = engine(two_f_model(|f2, g1| {
        f2.tag.counter = Some(1);
        g1.tag.frozen = frozen("S51");
        g1.tag.counter = None;
    }));
    let only = RenumberOptions {
        only_nonconforming: true,
        ..RenumberOptions::default()
    };
    let p = e.preview(&include_frozen(only)).unwrap();
    e.commit(p).unwrap();
    assert_eq!(counter(&e, DEV_102F2), Some(1));
    assert_eq!(counter(&e, DEV_102G1), Some(2));
    assert!(device(&e, DEV_102G1).tag.frozen.is_none());
    let p = e.preview(&include_frozen(only)).unwrap();
    assert!(p.ops().is_empty(), "{:?}", p.ops());
}

#[test]
fn renumber_include_frozen_keeps_an_unplaced_frozen_tag() {
    let mut e = engine(golden_model());
    e.execute(&set_device(DEV_3W1, "tag.frozen", json!({"text": "W99"})))
        .unwrap();
    let counter_before = counter(&e, DEV_3W1);
    let p = e.preview(&include_frozen(RenumberOptions::default())).unwrap();
    e.commit(p).unwrap();
    let w = device(&e, DEV_3W1);
    assert_eq!(w.tag.frozen, frozen("W99"));
    assert_eq!(w.tag.counter, counter_before);
}

#[test]
fn renumber_scopes_selection_and_pages() {
    let mut e = engine(two_f_model(|f2, g1| {
        f2.tag.counter = Some(7);
        g1.tag.counter = Some(9);
    }));
    let p = e
        .preview(&renumber(
            DeviceScope::Selection(vec![DEV_102G1]),
            RenumberOptions::default(),
        ))
        .unwrap();
    e.commit(p).unwrap();
    // The device outside the selection keeps its counter (reserved): G1 takes the lowest free one.
    assert_eq!(counter(&e, DEV_102F2), Some(7));
    assert_eq!(counter(&e, DEV_102G1), Some(1));

    let p = e
        .preview(&renumber(
            DeviceScope::Pages(vec![PAGE_101]),
            RenumberOptions::default(),
        ))
        .unwrap();
    assert!(
        p.ops().is_empty(),
        "page 101 is already numbered 1: {:?}",
        p.ops()
    );
    let p = e
        .preview(&renumber(
            DeviceScope::Pages(vec![PAGE_102]),
            RenumberOptions::default(),
        ))
        .unwrap();
    e.commit(p).unwrap();
    assert_eq!(counter(&e, DEV_102F2), Some(1));
    assert_eq!(counter(&e, DEV_102G1), Some(2));
}

#[test]
fn renumber_rejects_unknown_scope_members() {
    let mut e = engine(golden_model());
    let missing: Id<Device> = id(0xdead);
    let r = e.preview(&renumber(
        DeviceScope::Selection(vec![missing]),
        RenumberOptions::default(),
    ));
    assert!(matches!(r, Err(CmdError::NotFound { .. })));
    let r = e.preview(&renumber(
        DeviceScope::Pages(vec![id::<Page>(0xdead)]),
        RenumberOptions::default(),
    ));
    assert!(matches!(r, Err(CmdError::NotFound { .. })));
}

#[test]
fn renumber_is_refused_by_execute() {
    let mut e = engine(golden_model());
    let err = e.execute(&project_scope()).unwrap_err();
    assert!(matches!(err, CmdError::NeedsPreview { .. }), "{err:?}");
}

#[test]
fn representation_positions_are_read_from_the_main_representation() {
    // Moving the representation of DEV_102G1 to the left of DEV_102F2 swaps their numbers.
    let (p, mut records, _) = mini_golden_parts();
    for r in &mut records {
        match r {
            Record::Device(d) if d.id == DEV_102G1 => d.tag.letter = ident("F"),
            Record::Representation(rep) if rep.id == REP_102F2 => rep.pos = pt(10 * MM10, 0),
            Record::Representation(rep) if rep.id == REP_102G1 => rep.pos = pt(0, 0),
            _ => {}
        }
    }
    let model = Model::from_records(p, records, library()).unwrap();
    let mut e = engine(model);
    let pv = e.preview(&project_scope()).unwrap();
    e.commit(pv).unwrap();
    assert_eq!(counter(&e, DEV_102G1), Some(1));
    assert_eq!(counter(&e, DEV_102F2), Some(2));
    let _: Option<std::sync::Arc<Representation>> = None;
}
