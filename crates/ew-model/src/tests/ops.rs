// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of the op layer, the delete plans (FR-DEV-030, doc 03 §4.2), loading and snapshots.

use std::collections::BTreeSet;
use std::time::Instant;

use ew_ids::{Id, Uuid};
use ew_mltext::MlText;
use ew_rds::Aspect;
use ew_schema::{
    Bond, Conductor, Connection, ConnectionKind, Date, Device, Function, Junction, Kind, Page, PotentialDef,
    Project, Record, Representation, StructureNode,
};
use serde_json::json;

use super::fixture::*;
use crate::rev::RowId;
use crate::{
    AnyKey, AnyRow, ApplyOptions, BaseIndexes, EndpointKey, Entity, Model, ModelError, Op, PreventHook,
    PreventViolation, Rev, Strictness, invert_batch,
};

fn nid<T>(n: u128) -> Id<T> {
    Id::from_uuid(Uuid::from_u128(n))
}

fn dkey(id: Id<Device>) -> AnyKey {
    <Device as Entity>::key(id)
}

/// The current typed record of a device.
fn device_of(m: &Model, id: Id<Device>) -> Device {
    (**m.table::<Device>().get(&id).expect("device exists")).clone()
}

/// An op updating a device with `f` applied, against the current row of `m`.
fn device_update(m: &Model, id: Id<Device>, f: impl FnOnce(&mut Device)) -> Op {
    let before = m.row(&dkey(id)).expect("device row");
    let mut d = device_of(m, id);
    f(&mut d);
    Op::update(before, AnyRow::of(d))
}

fn new_node(n: u128, parent: Id<StructureNode>) -> StructureNode {
    structure_node(nid(n), Aspect::Location, "NEW", Some(parent))
}

fn removed<T: Entity>(id: RowId<T>) -> (AnyKey, bool) {
    (T::key(id), false)
}

fn modified<T: Entity>(id: RowId<T>) -> (AnyKey, bool) {
    (T::key(id), true)
}

/// The plan's ops as a set of `(key, after.is_some())`.
fn shape(ops: &[Op]) -> BTreeSet<(AnyKey, bool)> {
    ops.iter().map(|op| (op.key(), op.after().is_some())).collect()
}

/// Asserts the plan is exactly `expected` and lists every modification before every removal.
fn assert_plan(ops: &[Op], expected: &[(AnyKey, bool)]) {
    assert_eq!(shape(ops), expected.iter().copied().collect::<BTreeSet<_>>());
    assert_eq!(ops.len(), expected.len(), "no key is put twice");
    let first_removal = ops
        .iter()
        .position(|op| op.after().is_none())
        .unwrap_or(ops.len());
    assert!(
        ops[first_removal..].iter().all(|op| op.after().is_none()),
        "modifications come before removals"
    );
}

fn op_for(ops: &[Op], key: AnyKey) -> &Op {
    ops.iter().find(|op| op.key() == key).expect("op for key")
}

fn junction_sequence(ops: &[Op], j: Id<Junction>) -> Vec<Id<Connection>> {
    let after = op_for(ops, <Junction as Entity>::key(j))
        .after()
        .expect("modified");
    after.get::<Junction>().expect("junction").sequence.clone()
}

fn plan_applies(m: &Model, ops: &[Op]) {
    assert!(m.apply(ops, Rev(1)).is_ok());
}

// ---------------------------------------------------------------------------------------------------------
// Op layer

/// Insert, update and delete stamp rows, tables and index keys with the batch revision.
#[test]
fn insert_update_delete_stamp_changed_at() {
    let m0 = mini_golden();
    let node = new_node(0x20000, LOC);
    let m1 = m0.apply(&[Op::insert(AnyRow::of(node.clone()))], Rev(1)).unwrap();
    let id = node.id;
    assert_eq!(m1.table::<StructureNode>().row(&id).unwrap().changed_at(), Rev(1));
    assert_eq!(m1.table::<StructureNode>().changed_at(), Rev(1));
    assert_eq!(
        m1.indexes().structure_children().key_changed_at(&LOC),
        Some(Rev(1))
    );
    assert_eq!(
        m1.table::<StructureNode>().row(&LOC).unwrap().changed_at(),
        Rev::ZERO
    );

    let mut node2 = node.clone();
    node2.description = MlText::neutral("changed");
    let key = <StructureNode as Entity>::key(id);
    let m2 = m1
        .apply(&[Op::update(m1.row(&key).unwrap(), AnyRow::of(node2))], Rev(2))
        .unwrap();
    assert_eq!(m2.table::<StructureNode>().row(&id).unwrap().changed_at(), Rev(2));
    assert_eq!(m2.table::<StructureNode>().changed_at(), Rev(2));
    assert_eq!(
        m2.table::<StructureNode>().row(&LOC).unwrap().changed_at(),
        Rev::ZERO
    );
    assert_eq!(m2.table::<Device>().changed_at(), Rev::ZERO);

    let m3 = m2.apply(&[Op::delete(m2.row(&key).unwrap())], Rev(3)).unwrap();
    assert!(m3.table::<StructureNode>().get(&id).is_none());
    assert_eq!(m3.table::<StructureNode>().changed_at(), Rev(3));
    assert_eq!(m3.rev(), Rev(3));
}

/// A successful apply leaves the original snapshot untouched.
#[test]
fn apply_leaves_the_original_snapshot_unchanged() {
    let m0 = mini_golden();
    let m1 = m0
        .apply(&[Op::insert(AnyRow::of(new_node(0x20000, LOC)))], Rev(1))
        .unwrap();
    assert_eq!(
        m1.table::<StructureNode>().len(),
        m0.table::<StructureNode>().len() + 1
    );
    assert_eq!(m0.rev(), Rev::ZERO);
    assert_eq!(m0.table::<StructureNode>().len(), 2);
    assert!(m0.content_eq(&mini_golden()));
}

/// An op whose `before` is not the current row is stale, for updates, inserts and deletes alike.
#[test]
fn stale_before_is_rejected() {
    let m = mini_golden();
    let key = dkey(DEV_101F1);

    let mut other = device_of(&m, DEV_101F1);
    other.function_text = MlText::neutral("elsewhere");
    let mut after = device_of(&m, DEV_101F1);
    after.function_text = MlText::neutral("mine");
    let op = Op::update(AnyRow::of(other), AnyRow::of(after));
    assert_eq!(
        m.apply(&[op], Rev(1)).unwrap_err(),
        ModelError::StaleBefore { key }
    );

    let insert_existing = Op::insert(AnyRow::of(device_of(&m, DEV_101F1)));
    assert_eq!(
        m.apply(&[insert_existing], Rev(1)).unwrap_err(),
        ModelError::StaleBefore { key }
    );

    let missing = device(nid(0x20001), "F", 9);
    let missing_key = missing.id;
    let delete_missing = Op::delete(AnyRow::of(missing));
    assert_eq!(
        m.apply(&[delete_missing], Rev(1)).unwrap_err(),
        ModelError::StaleBefore {
            key: dkey(missing_key)
        }
    );
}

/// Two ops on one key chain within a batch: the second expects the first's `after`.
#[test]
fn ops_on_the_same_key_chain_within_a_batch() {
    let m = mini_golden();
    let first = device_update(&m, DEV_101F1, |d| d.function_text = MlText::neutral("one"));
    let mut second_after = device_of(&m, DEV_101F1);
    second_after.function_text = MlText::neutral("two");
    let second = Op::update(first.after().unwrap().clone(), AnyRow::of(second_after.clone()));
    let m1 = m.apply(&[first.clone(), second], Rev(1)).unwrap();
    assert_eq!(device_of(&m1, DEV_101F1), second_after);

    let stale_second = Op::update(m.row(&dkey(DEV_101F1)).unwrap(), AnyRow::of(second_after));
    assert_eq!(
        m.apply(&[first, stale_second], Rev(1)).unwrap_err(),
        ModelError::StaleBefore { key: dkey(DEV_101F1) }
    );
}

/// The revision must be greater than the model's.
#[test]
fn rev_must_increase() {
    let m = mini_golden();
    assert_eq!(
        m.apply(&[], Rev(0)).unwrap_err(),
        ModelError::RevNotIncreasing {
            current: Rev(0),
            given: Rev(0)
        }
    );
    let m1 = m.apply(&[], Rev(1)).unwrap();
    assert_eq!(m1.rev(), Rev(1));
    assert_eq!(
        m1.apply(&[], Rev(1)).unwrap_err(),
        ModelError::RevNotIncreasing {
            current: Rev(1),
            given: Rev(1)
        }
    );
}

/// Ops with a mismatching key, a project removal or a reserved kind are malformed.
#[test]
fn malformed_ops_are_rejected() {
    let m = mini_golden();
    let device_row = m.row(&dkey(DEV_101F1)).unwrap();

    let mismatch = Op::Put {
        key: (Kind::Page, DEV_101F1.cast()),
        before: None,
        after: Some(device_row),
    };
    assert!(matches!(
        m.apply(&[mismatch], Rev(1)).unwrap_err(),
        ModelError::Malformed { .. }
    ));

    let project_key = (Kind::Project, PROJECT.cast());
    let delete_project = Op::Put {
        key: project_key,
        before: m.row(&project_key),
        after: None,
    };
    assert!(matches!(
        m.apply(&[delete_project], Rev(1)).unwrap_err(),
        ModelError::Malformed { key, .. } if key == project_key
    ));

    let reserved = Op::Put {
        key: (Kind::Channel, nid::<()>(0x20002)),
        before: None,
        after: None,
    };
    assert!(matches!(
        m.apply(&[reserved], Rev(1)).unwrap_err(),
        ModelError::Malformed { .. }
    ));
}

/// Applying the inverse batch restores the content and stamps the restored rows with the new revision.
#[test]
fn invert_batch_restores_and_stamps_a_new_rev() {
    let m0 = mini_golden();
    let plan = m0.delete_plan(&dkey(DEV_102F2)).unwrap();
    let m1 = m0.apply(&plan, Rev(1)).unwrap();
    assert!(!m1.content_eq(&m0));
    let m2 = m1.apply(&invert_batch(&plan), Rev(2)).unwrap();
    assert!(m2.content_eq(&m0));
    assert_eq!(m2.table::<Device>().row(&DEV_102F2).unwrap().changed_at(), Rev(2));
    for f in [FN_102F2, FN_102F2_AUX] {
        assert_eq!(m2.table::<Function>().row(&f).unwrap().changed_at(), Rev(2));
    }
    for r in [REP_102F2, REP_102F2_AUX] {
        assert_eq!(m2.table::<Representation>().row(&r).unwrap().changed_at(), Rev(2));
    }
    for c in [CON_W22, CON_W29] {
        assert_eq!(m2.table::<Connection>().row(&c).unwrap().changed_at(), Rev(2));
    }
    assert_eq!(m2.table::<Junction>().row(&J5).unwrap().changed_at(), Rev(2));
    assert_eq!(
        m2.table::<Device>().row(&DEV_101F1).unwrap().changed_at(),
        Rev::ZERO
    );
}

/// The project row is updated and inverted through the op layer like any other row.
#[test]
fn project_row_updates_and_inverts() {
    let m0 = mini_golden();
    let key = (Kind::Project, PROJECT.cast());
    let mut p: Project = (**m0.project()).clone();
    p.master.description = MlText::neutral("Renamed");
    let ops = vec![Op::update(
        m0.row(&key).unwrap(),
        AnyRow::Project(p.clone().into()),
    )];
    let m1 = m0.apply(&ops, Rev(1)).unwrap();
    assert_eq!(**m1.project(), p);
    assert_eq!(m1.project_row().changed_at(), Rev(1));
    assert!(!m1.content_eq(&m0));
    let m2 = m1.apply(&invert_batch(&ops), Rev(2)).unwrap();
    assert!(m2.content_eq(&m0));
    assert_eq!(m2.project_row().changed_at(), Rev(2));
}

// ---------------------------------------------------------------------------------------------------------
// Delete plans

/// Deleting a device cascades to its functions, representations and connections, and trims the junction.
#[test]
fn delete_plan_device_cascades_functions_reps_and_connections() {
    let m = mini_golden();
    let ops = m.delete_plan(&dkey(DEV_102F2)).unwrap();
    assert_plan(
        &ops,
        &[
            removed::<Device>(DEV_102F2),
            removed::<Function>(FN_102F2),
            removed::<Function>(FN_102F2_AUX),
            removed::<Representation>(REP_102F2),
            removed::<Representation>(REP_102F2_AUX),
            removed::<Connection>(CON_W22),
            removed::<Connection>(CON_W29),
            modified::<Junction>(J5),
        ],
    );
    assert_eq!(junction_sequence(&ops, J5), vec![CON_J5_IN, CON_W27]);
    plan_applies(&m, &ops);
}

/// Deleting a function drops the definitions and bonds at its connection points.
#[test]
fn delete_plan_function_drops_definitions_and_bonds() {
    let m = mini_golden();
    let ops = m.delete_plan(&<Function as Entity>::key(FN_102G1)).unwrap();
    assert_plan(
        &ops,
        &[
            removed::<Function>(FN_102G1),
            removed::<Representation>(REP_102G1),
            removed::<Connection>(CON_W11B),
            removed::<Connection>(CON_J5_IN),
            removed::<PotentialDef>(PD_24V),
            removed::<Bond>(BOND_PE),
            modified::<Junction>(J5),
        ],
    );
    assert_eq!(junction_sequence(&ops, J5), vec![CON_W22, CON_W27]);
    // A device without functions is allowed.
    plan_applies(&m, &ops);
}

/// Deleting a conductor turns the core connection back into a plain wire.
#[test]
fn delete_plan_conductor_unlinks_core() {
    let m = mini_golden();
    let ops = m.delete_plan(&<Conductor as Entity>::key(COND_A)).unwrap();
    assert_plan(
        &ops,
        &[removed::<Conductor>(COND_A), modified::<Connection>(CON_CORE_A)],
    );
    let after = op_for(&ops, <Connection as Entity>::key(CON_CORE_A))
        .after()
        .unwrap();
    assert!(matches!(
        after.get::<Connection>().unwrap().kind,
        ConnectionKind::Wire
    ));
    plan_applies(&m, &ops);
}

/// Deleting a connection trims the junction sequence and clears the bond's `via`.
#[test]
fn delete_plan_connection_unlinks_sequence_and_bond_via() {
    let m = mini_golden();
    let ops = m.delete_plan(&<Connection as Entity>::key(CON_W27)).unwrap();
    assert_plan(
        &ops,
        &[
            removed::<Connection>(CON_W27),
            modified::<Junction>(J5),
            modified::<Bond>(BOND_PE),
        ],
    );
    assert_eq!(junction_sequence(&ops, J5), vec![CON_J5_IN, CON_W22]);
    let bond = op_for(&ops, <Bond as Entity>::key(BOND_PE)).after().unwrap();
    assert_eq!(bond.get::<Bond>().unwrap().via, None);
    plan_applies(&m, &ops);
}

/// Deleting a cable removes its conductors and unlinks the cores.
#[test]
fn delete_plan_cable_cascades_conductors_and_unlinks_cores() {
    let m = mini_golden();
    let ops = m.delete_plan(&dkey(DEV_3W1)).unwrap();
    assert_plan(
        &ops,
        &[
            removed::<Device>(DEV_3W1),
            removed::<Conductor>(COND_A),
            removed::<Conductor>(COND_B),
            removed::<Conductor>(COND_SH),
            modified::<Connection>(CON_CORE_A),
        ],
    );
    let after = op_for(&ops, <Connection as Entity>::key(CON_CORE_A))
        .after()
        .unwrap();
    assert!(matches!(
        after.get::<Connection>().unwrap().kind,
        ConnectionKind::Wire
    ));
    plan_applies(&m, &ops);
}

/// Deleting a page removes what is drawn on it; the functions stay unplaced (FR-PAG-025).
#[test]
fn delete_plan_page_cascades_page_owned_rows() {
    let m = mini_golden();
    let ops = m.delete_plan(&<Page as Entity>::key(PAGE_101)).unwrap();
    assert_plan(
        &ops,
        &[
            removed::<Page>(PAGE_101),
            removed::<Representation>(REP_101F1),
            removed::<Representation>(REP_IP_101),
            removed::<Connection>(CON_W11A),
        ],
    );
    plan_applies(&m, &ops);
}

/// A `forbid` link with a remaining referrer rejects the plan.
#[test]
fn delete_plan_forbid_rejects() {
    let m = mini_golden();
    let err = m.delete_plan(&dkey(DEV_X1031)).unwrap_err();
    match err {
        ModelError::Referenced {
            link,
            target,
            referrer,
        } => {
            assert_eq!(link, "device.terminal.strip");
            assert_eq!(target, dkey(DEV_X1031));
            assert!([DEV_T1, DEV_T2, DEV_T3].iter().any(|t| dkey(*t) == referrer));
        }
        other => panic!("expected Referenced, got {other:?}"),
    }

    let loc = <StructureNode as Entity>::key(LOC);
    assert!(matches!(
        m.delete_plan(&loc).unwrap_err(),
        ModelError::Referenced { target, .. } if target == loc
    ));

    let fndef = (Kind::Fndef, FNDEF_MCB.cast());
    assert!(matches!(
        m.delete_plan(&fndef).unwrap_err(),
        ModelError::Referenced { link, target, .. } if link == "function.def" && target == fndef
    ));
}

/// A missing key is not found; the project row cannot be removed.
#[test]
fn delete_plan_of_missing_or_project_key() {
    let m = mini_golden();
    let missing = dkey(nid(0x20003));
    assert_eq!(
        m.delete_plan(&missing).unwrap_err(),
        ModelError::NotFound { key: missing }
    );
    assert!(matches!(
        m.delete_plan(&(Kind::Project, PROJECT.cast())).unwrap_err(),
        ModelError::Malformed { .. }
    ));
}

/// `apply` executes no cascade: a lone removal is fine without referrers and an invariant error with them.
#[test]
fn delete_without_plan_is_rejected() {
    let m = mini_golden();
    let rep = m.row(&<Representation as Entity>::key(REP_102F2_AUX)).unwrap();
    assert!(m.apply(&[Op::delete(rep)], Rev(1)).is_ok());

    let dev = m.row(&dkey(DEV_102F2)).unwrap();
    assert!(matches!(
        m.apply(&[Op::delete(dev)], Rev(1)).unwrap_err(),
        ModelError::Invariant { id: "EW-INV-001", .. }
    ));
}

/// `referrers` lists the links that name a record, in link order.
#[test]
fn referrers_lists_links() {
    let m = mini_golden();
    let found = m.referrers(&<Junction as Entity>::key(J5));
    assert!(found.iter().all(|(link, (kind, _))| {
        *kind == Kind::Connection
            && link.from == Kind::Connection
            && matches!(
                link.path().as_str(),
                "connection.a.junction" | "connection.b.junction"
            )
    }));
    let ids: BTreeSet<AnyKey> = found.iter().map(|(_, k)| *k).collect();
    let expected: BTreeSet<AnyKey> = [CON_J5_IN, CON_W22, CON_W27]
        .iter()
        .map(|c| <Connection as Entity>::key(*c))
        .collect();
    assert_eq!(ids, expected);
    assert_eq!(found.len(), 3);

    let cond: Vec<(String, AnyKey)> = m
        .referrers(&<Conductor as Entity>::key(COND_A))
        .into_iter()
        .map(|(link, k)| (link.path(), k))
        .collect();
    assert_eq!(
        cond,
        vec![(
            "connection.kind.core".to_string(),
            <Connection as Entity>::key(CON_CORE_A)
        )]
    );
}

// ---------------------------------------------------------------------------------------------------------
// Loading

/// Two records with one key, or a project record among the records, are duplicates.
#[test]
fn from_records_rejects_duplicates() {
    let (project, mut records, lib) = mini_golden_parts();
    let dev = records
        .iter()
        .find_map(|r| match r {
            Record::Device(d) if d.id == DEV_101F1 => Some(d.clone()),
            _ => None,
        })
        .unwrap();
    records.push(Record::Device(dev));
    assert_eq!(
        Model::from_records(project.clone(), records, lib.clone()).unwrap_err(),
        ModelError::DuplicateRecord { key: dkey(DEV_101F1) }
    );

    let (_, mut records, _) = mini_golden_parts();
    records.push(Record::Project(project.clone()));
    assert_eq!(
        Model::from_records(project, records, lib).unwrap_err(),
        ModelError::DuplicateRecord {
            key: (Kind::Project, PROJECT.cast())
        }
    );
}

/// Records of reserved kinds are kept in canonical order, whatever the input order.
#[test]
fn from_records_keeps_unknown_records() {
    let a = Record::Unknown(json!({"$k": "channel", "id": "00000000-0000-0000-0000-00000000a001"}));
    let b = Record::Unknown(json!({"$k": "channel", "id": "00000000-0000-0000-0000-00000000b002"}));
    let load = |extra: Vec<Record>| {
        let (project, mut records, lib) = mini_golden_parts();
        records.extend(extra);
        Model::from_records(project, records, lib).unwrap()
    };
    let ab = load(vec![a.clone(), b.clone()]);
    let ba = load(vec![b, a]);
    let plain = mini_golden();
    assert_eq!(ab.unknown_records().len(), 2);
    assert_eq!(ab.unknown_records(), ba.unknown_records());
    assert!(ab.content_eq(&ba));
    assert_eq!(ab.table::<Device>().len(), plain.table::<Device>().len());
    assert_eq!(ab.table::<Function>().len(), plain.table::<Function>().len());
    assert_eq!(ab.table::<Connection>().len(), plain.table::<Connection>().len());
    assert_eq!(
        ab.table::<StructureNode>().len(),
        plain.table::<StructureNode>().len()
    );
    assert!(plain.unknown_records().is_empty());
}

/// The indexes of a loaded model equal a fresh rebuild; a few entries are spot-checked.
#[test]
fn rebuilt_indexes_equal_loaded_indexes() {
    let m = mini_golden();
    assert!(BaseIndexes::rebuild(&m).same_entries(m.indexes()));
    let idx = m.indexes();
    assert_eq!(
        idx.functions_by_device()
            .get(&DEV_102F2)
            .copied()
            .collect::<Vec<_>>(),
        vec![FN_102F2, FN_102F2_AUX]
    );
    assert_eq!(idx.terminals_by_strip().get(&DEV_X1031).count(), 3);
    assert_eq!(
        idx.connections_by_endpoint()
            .get(&EndpointKey::Junction(J5))
            .count(),
        3
    );
    assert_eq!(idx.conductors_by_cable().get(&DEV_3W1).count(), 3);
}

// ---------------------------------------------------------------------------------------------------------
// Snapshots and settings

/// Cloning a large model is cheap (smoke test, not a benchmark).
#[test]
fn clone_is_constant_time_smoke() {
    let (project, _, _) = mini_golden_parts();
    let mut records = vec![Record::StructureNode(structure_node(
        LOC,
        Aspect::Location,
        "ROOT",
        None,
    ))];
    for i in 0..10_000u128 {
        records.push(Record::StructureNode(new_node(0x20000 + i, LOC)));
    }
    let m = Model::from_records(project, records, []).unwrap();
    assert_eq!(m.table::<StructureNode>().len(), 10_001);
    let start = Instant::now();
    for _ in 0..1_000 {
        std::hint::black_box(m.clone());
    }
    assert!(
        start.elapsed().as_secs_f64() < 1.0,
        "1000 clones took {:?}",
        start.elapsed()
    );
}

/// The strictness is read from `settings.extra.checks.strictness`; anything unreadable is strict.
#[test]
fn strictness_reads_project_settings() {
    let (project, records, lib) = mini_golden_parts();
    let with = |checks: serde_json::Value| {
        let mut p = project.clone();
        p.settings.extra.insert("checks".into(), checks);
        Model::from_records(p, records.clone(), lib.clone())
            .unwrap()
            .strictness()
    };
    assert_eq!(mini_golden().strictness(), Strictness::Strict);
    assert_eq!(
        with(json!({"strictness": {"lenient": {"since": "2026-10-01"}}})),
        Strictness::Lenient {
            since: Some(Date::new(2026, 10, 1).unwrap())
        }
    );
    assert_eq!(
        with(json!({"strictness": "lenient"})),
        Strictness::Lenient { since: None }
    );
    assert_eq!(with(json!({"strictness": 42})), Strictness::Strict);
    assert_eq!(with(json!({"strictness": "bogus"})), Strictness::Strict);
    assert_eq!(with(json!("garbage")), Strictness::Strict);
}

/// Rejects any batch that touches a device.
struct NoDevices;

impl PreventHook for NoDevices {
    fn check(&self, _: &Model, touched: &[AnyKey]) -> Result<(), PreventViolation> {
        if touched.iter().any(|k| k.0 == Kind::Device) {
            return Err(PreventViolation {
                rule: "EW-TERM-002".into(),
                message: "devices are locked".into(),
                keys: touched.to_vec(),
            });
        }
        Ok(())
    }
}

/// The prevent hook runs after the structural invariants and only when options are given.
#[test]
fn prevent_hook_runs_after_structural_invariants() {
    let m = mini_golden();
    let hook = NoDevices;
    let opts = ApplyOptions { prevent: Some(&hook) };

    let update = device_update(&m, DEV_101F1, |d| d.function_text = MlText::neutral("x"));
    assert!(matches!(
        m.apply_with(std::slice::from_ref(&update), Rev(1), &opts).unwrap_err(),
        ModelError::Prevented { rule, .. } if rule == "EW-TERM-002"
    ));

    let insert = Op::insert(AnyRow::of(new_node(0x20000, LOC)));
    assert!(m.apply_with(&[insert], Rev(1), &opts).is_ok());

    // A structural violation that also touches a device reports the structural error first.
    let lone_delete = Op::delete(m.row(&dkey(DEV_102F2)).unwrap());
    assert!(matches!(
        m.apply_with(&[lone_delete], Rev(1), &opts).unwrap_err(),
        ModelError::Invariant { id: "EW-INV-001", .. }
    ));

    // Without options the hook is not consulted.
    assert!(m.apply(&[update], Rev(1)).is_ok());
}
