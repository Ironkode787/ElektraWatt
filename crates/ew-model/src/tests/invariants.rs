// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One positive and at least one negative test per structural invariant of `invariants.md`.

use ew_ids::{Id, Uuid};
use ew_rds::Aspect;
use ew_schema::{
    Connection, Device, FnRole, Function, IpDirection, Junction, PageType, PinMapEntry, Record,
    Representation, StructureNode,
};

use super::fixture::{
    CON_J5_IN, CON_W22, CON_W27, DEV_3W1, DEV_101F1, DEV_102F2, DEV_102G1, DEV_X1031, FN_101F1, FN_102F2,
    FN_102F2_AUX, FN_102G1, FN_IP_101, FN_T3, FNDEF_CONTACT, J5, LOC, PAGE_101, PAGE_102, SEC1, at_junction,
    conductor, connection, cp, device, function, ip_function, mini_golden, mini_golden_parts, page, rep,
    structure_node, terminal,
};
use crate::{AnyRow, Entity, Model, ModelError, Op, RULES, Rev, RowId};

fn nid<T>(n: u128) -> Id<T> {
    Id::from_uuid(Uuid::from_u128(n))
}

/// Panics with the actual result unless it is the rejection `rule`.
fn rejected_by(r: Result<Model, ModelError>, rule: &str) {
    match &r {
        Err(ModelError::Invariant { id, .. }) if *id == rule => {}
        Err(e) => panic!("expected {rule}, got error: {e:?}"),
        Ok(_) => panic!("expected {rule}, but the batch was accepted"),
    }
}

fn ins<T: Entity>(record: T) -> Op {
    Op::insert(AnyRow::of(record))
}

/// An update op of the row `id`: the current row with `f` applied.
fn upd<T: Entity + Clone>(m: &Model, id: RowId<T>, f: impl FnOnce(&mut T)) -> Op {
    let before = m.row(&T::key(id)).expect("row exists");
    let mut record = (**before.get::<T>().expect("kind matches")).clone();
    f(&mut record);
    Op::update(before, AnyRow::of(record))
}

fn apply(m: &Model, ops: &[Op]) -> Result<Model, ModelError> {
    m.apply(ops, Rev(1))
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-001

/// A new function whose device and def exist is accepted.
#[test]
fn inv001_refs_resolve_ok() {
    let m = mini_golden();
    let f = function(nid(0x10_000), DEV_101F1, FNDEF_CONTACT, FnRole::Aux);
    assert!(apply(&m, &[ins(f)]).is_ok());
}

/// A function naming a missing FnDef is rejected.
#[test]
fn inv001_refs_resolve_rejected() {
    let m = mini_golden();
    let f = function(nid(0x10_000), DEV_101F1, nid(0x10_001), FnRole::Aux);
    rejected_by(apply(&m, &[ins(f)]), crate::REFS_RESOLVE);
}

/// Removing a device while its conductors (cascade referrers) remain is rejected.
#[test]
fn inv001_removed_target_with_cascade_referrer_rejected() {
    let m = mini_golden();
    let op = Op::delete(m.row(&Device::key(DEV_3W1)).unwrap());
    rejected_by(apply(&m, &[op]), crate::REFS_RESOLVE);
}

/// Removing a strip that terminals name is a forbid-link `Referenced` error.
#[test]
fn inv001_forbid_referenced() {
    let m = mini_golden();
    let op = Op::delete(m.row(&Device::key(DEV_X1031)).unwrap());
    let err = apply(&m, &[op]).expect_err("must be rejected");
    assert!(
        matches!(err, ModelError::Referenced { ref link, .. } if link == "device.terminal.strip"),
        "{err:?}"
    );
}

/// The integrity check of a load rejects a dangling reference.
#[test]
fn integrity_check_on_load_rejected() {
    let (p, mut records, lib) = mini_golden_parts();
    records.push(Record::Representation(rep(
        nid(0x10_000),
        PAGE_101,
        vec![nid(0x10_001)],
    )));
    let r = Model::from_records(p, records, lib);
    rejected_by(r, crate::REFS_RESOLVE);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-002

/// The fixture is valid and an extra Aux function next to the Main is accepted.
#[test]
fn inv002_one_main_function_ok() {
    let m = mini_golden();
    assert!(m.integrity_check().is_ok());
    let f = function(nid(0x10_000), DEV_101F1, FNDEF_CONTACT, FnRole::Aux);
    assert!(apply(&m, &[ins(f)]).is_ok());
}

/// A device without functions is allowed.
#[test]
fn inv002_zero_functions_ok() {
    let m = mini_golden();
    assert!(apply(&m, &[ins(device(nid(0x10_000), "Q", 9))]).is_ok());
}

/// A second Main function on a device is rejected.
#[test]
fn inv002_one_main_function_rejected() {
    let m = mini_golden();
    let f = function(nid(0x10_000), DEV_101F1, FNDEF_CONTACT, FnRole::Main);
    rejected_by(apply(&m, &[ins(f)]), crate::ONE_MAIN_FUNCTION);
}

/// Moving the Main away leaves the old device (a neighbour) without one.
#[test]
fn inv002_main_moved_away_rejected() {
    let m = mini_golden();
    let new = nid(0x10_000);
    let ops = [
        ins(device(new, "Q", 9)),
        upd(&m, FN_102F2, |f: &mut Function| f.device = Some(new)),
    ];
    rejected_by(apply(&m, &ops), crate::ONE_MAIN_FUNCTION);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-003

/// A new interruption point without a device is accepted.
#[test]
fn inv003_device_iff_not_ip_ok() {
    let m = mini_golden();
    let f = ip_function(nid(0x10_000), "L-9", IpDirection::Source);
    assert!(apply(&m, &[ins(f)]).is_ok());
}

/// A Normal function without a device is rejected.
#[test]
fn inv003_device_iff_not_ip_rejected() {
    let m = mini_golden();
    let mut f = function(nid(0x10_000), DEV_101F1, FNDEF_CONTACT, FnRole::Aux);
    f.device = None;
    rejected_by(apply(&m, &[ins(f)]), crate::DEVICE_IFF_NOT_IP);
}

/// An interruption point with a device is rejected (role Aux isolates the rule from EW-INV-002).
#[test]
fn inv003_ip_with_device_rejected() {
    let m = mini_golden();
    let mut f = ip_function(nid(0x10_000), "L-9", IpDirection::Source);
    f.device = Some(DEV_101F1);
    f.role = FnRole::Aux;
    rejected_by(apply(&m, &[ins(f)]), crate::DEVICE_IFF_NOT_IP);
}

// ---------------------------------------------------------------------------------------------------------
// CI-01

/// A connection between two different CPs of one function is accepted.
#[test]
fn ci01_endpoints_differ_ok() {
    let m = mini_golden();
    let c = connection(nid(0x10_000), Some(PAGE_101), cp(FN_101F1, 0), cp(FN_101F1, 2));
    assert!(apply(&m, &[ins(c)]).is_ok());
}

/// A connection with identical endpoints is rejected.
#[test]
fn ci01_endpoints_differ_rejected() {
    let m = mini_golden();
    let c = connection(nid(0x10_000), Some(PAGE_101), cp(FN_101F1, 0), cp(FN_101F1, 0));
    rejected_by(apply(&m, &[ins(c)]), crate::ENDPOINTS_DIFFER);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-004

/// A representation drawing two functions of one device is accepted.
#[test]
fn inv004_rep_one_device_ok() {
    let m = mini_golden();
    let r = rep(nid(0x10_000), PAGE_102, vec![FN_102F2, FN_102F2_AUX]);
    assert!(apply(&m, &[ins(r)]).is_ok());
}

/// An interruption-point function may be drawn with a device's function.
#[test]
fn inv004_ip_function_excepted_ok() {
    let m = mini_golden();
    let r = rep(nid(0x10_000), PAGE_101, vec![FN_101F1, FN_IP_101]);
    assert!(apply(&m, &[ins(r)]).is_ok());
}

/// A representation drawing functions of two devices is rejected.
#[test]
fn inv004_rep_one_device_rejected() {
    let m = mini_golden();
    let r = rep(nid(0x10_000), PAGE_101, vec![FN_101F1, FN_102G1]);
    rejected_by(apply(&m, &[ins(r)]), crate::REP_ONE_DEVICE);
}

/// Moving a drawn function to another device breaks the representation (a neighbour).
#[test]
fn inv004_function_moved_rejected() {
    let m = mini_golden();
    let r = rep(nid(0x10_000), PAGE_102, vec![FN_102F2, FN_102F2_AUX]);
    let m = m.apply(&[ins(r)], Rev(1)).unwrap();
    let op = upd(&m, FN_102F2_AUX, |f: &mut Function| {
        f.device = Some(DEV_101F1);
        f.role = FnRole::Aux;
    });
    rejected_by(m.apply(&[op], Rev(2)), crate::REP_ONE_DEVICE);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-005

fn pin_map_rep(second_symbol_cp: u16) -> Representation {
    let mut r = rep(nid(0x10_000), PAGE_101, vec![FN_101F1]);
    r.pin_map = [(0, 0), (second_symbol_cp, 1)]
        .into_iter()
        .map(|(symbol_cp, idx)| PinMapEntry {
            symbol_cp,
            function: FN_101F1,
            idx,
            extra: Default::default(),
        })
        .collect();
    r
}

/// A pin map with distinct symbol CPs is accepted.
#[test]
fn inv005_pin_map_unique_ok() {
    let m = mini_golden();
    assert!(apply(&m, &[ins(pin_map_rep(1))]).is_ok());
}

/// A pin map with a repeated symbol CP is rejected.
#[test]
fn inv005_pin_map_unique_rejected() {
    let m = mini_golden();
    rejected_by(apply(&m, &[ins(pin_map_rep(0))]), crate::PIN_MAP_UNIQUE);
}

// ---------------------------------------------------------------------------------------------------------
// CI-04

/// The fixture satisfies CI-04 and a connection added together with its junction entry is accepted.
#[test]
fn ci04_junction_sequence_ok() {
    let m = mini_golden();
    assert!(m.integrity_check().is_ok());
    let c = nid(0x10_000);
    let ops = [
        ins(connection(c, Some(PAGE_102), at_junction(J5), cp(FN_T3, 0))),
        upd(&m, J5, |j: &mut Junction| j.sequence.push(c)),
    ];
    assert!(apply(&m, &ops).is_ok());
}

/// A connection ending at a junction that does not list it is rejected.
#[test]
fn ci04_junction_sequence_rejected() {
    let m = mini_golden();
    let c = connection(nid(0x10_000), Some(PAGE_102), at_junction(J5), cp(FN_T3, 0));
    rejected_by(apply(&m, &[ins(c)]), crate::JUNCTION_SEQUENCE);
}

/// Rewiring a connection away from a junction without updating it is rejected (old junction is a neighbour).
#[test]
fn ci04_rewired_connection_rejected() {
    let m = mini_golden();
    let op = upd(&m, CON_W22, |c: &mut Connection| c.a = cp(FN_102G1, 6));
    rejected_by(apply(&m, &[op]), crate::JUNCTION_SEQUENCE);
}

/// A sequence listing a connection twice is rejected.
#[test]
fn ci04_duplicate_entry_rejected() {
    let m = mini_golden();
    let op = upd(&m, J5, |j: &mut Junction| {
        j.sequence = vec![CON_J5_IN, CON_W22, CON_W27, CON_W22];
    });
    rejected_by(apply(&m, &[op]), crate::JUNCTION_SEQUENCE);
}

/// A CI-04 violation that is repaired later in the same batch is accepted (ADR-0009 decision 1).
#[test]
fn transient_violation_inside_batch_ok() {
    let m = mini_golden();
    let c = nid(0x10_000);
    let ops = [
        ins(connection(c, Some(PAGE_102), at_junction(J5), cp(FN_T3, 0))),
        upd(&m, J5, |j: &mut Junction| j.sequence.push(c)),
    ];
    let m1 = apply(&m, &ops).expect("transient violation is legal");
    assert_eq!(m1.table::<Junction>().row(&J5).unwrap().value().sequence.len(), 4);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-006

/// A conductor of a cable device is accepted.
#[test]
fn inv006_conductor_cable_ok() {
    let m = mini_golden();
    assert!(apply(&m, &[ins(conductor(nid(0x10_000), DEV_3W1, "D"))]).is_ok());
}

/// A conductor of a device without the cable aspect is rejected.
#[test]
fn inv006_conductor_cable_rejected() {
    let m = mini_golden();
    let c = conductor(nid(0x10_000), DEV_101F1, "D");
    rejected_by(apply(&m, &[ins(c)]), crate::CONDUCTOR_CABLE);
}

/// Removing the cable aspect from a cable that has conductors is rejected.
#[test]
fn inv006_cable_aspect_removed_rejected() {
    let m = mini_golden();
    let op = upd(&m, DEV_3W1, |d: &mut Device| d.aspects.cable = None);
    rejected_by(apply(&m, &[op]), crate::CONDUCTOR_CABLE);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-007

/// A terminal on a strip is accepted.
#[test]
fn inv007_terminal_strip_ok() {
    let m = mini_golden();
    assert!(apply(&m, &[ins(terminal(nid(0x10_000), DEV_X1031, 4, "4"))]).is_ok());
}

/// A terminal whose strip lacks the strip aspect is rejected.
#[test]
fn inv007_terminal_strip_rejected() {
    let m = mini_golden();
    let t = terminal(nid(0x10_000), DEV_101F1, 1, "1");
    rejected_by(apply(&m, &[ins(t)]), crate::TERMINAL_STRIP);
}

/// Removing the strip aspect from a strip that has terminals is rejected.
#[test]
fn inv007_strip_aspect_removed_rejected() {
    let m = mini_golden();
    let op = upd(&m, DEV_X1031, |d: &mut Device| d.aspects.strip = None);
    rejected_by(apply(&m, &[op]), crate::TERMINAL_STRIP);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-008

/// A chain of location nodes is accepted.
#[test]
fn inv008_structure_acyclic_ok() {
    let m = mini_golden();
    let (a, b) = (nid(0x10_000), nid(0x10_001));
    let ops = [
        ins(structure_node(a, Aspect::Location, "A", Some(LOC))),
        ins(structure_node(b, Aspect::Location, "B", Some(a))),
    ];
    assert!(apply(&m, &ops).is_ok());
}

/// Two nodes that are each other's parent are rejected.
#[test]
fn inv008_structure_acyclic_rejected() {
    let m = mini_golden();
    let (a, b) = (nid(0x10_000), nid(0x10_001));
    let ops = [
        ins(structure_node(a, Aspect::Location, "A", Some(b))),
        ins(structure_node(b, Aspect::Location, "B", Some(a))),
    ];
    rejected_by(apply(&m, &ops), crate::STRUCTURE_ACYCLIC);
}

/// A node that is its own parent is rejected.
#[test]
fn inv008_self_parent_rejected() {
    let m = mini_golden();
    let op = upd(&m, LOC, |n: &mut StructureNode| n.parent = Some(LOC));
    rejected_by(apply(&m, &[op]), crate::STRUCTURE_ACYCLIC);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-009

/// A device nested under another device is accepted.
#[test]
fn inv009_device_acyclic_ok() {
    let m = mini_golden();
    let mut d = device(nid(0x10_000), "Q", 9);
    d.parent = Some(DEV_102F2);
    assert!(apply(&m, &[ins(d)]).is_ok());
}

/// Two devices that are each other's parent are rejected.
#[test]
fn inv009_device_acyclic_rejected() {
    let m = mini_golden();
    let ops = [
        upd(&m, DEV_101F1, |d: &mut Device| d.parent = Some(DEV_102G1)),
        upd(&m, DEV_102G1, |d: &mut Device| d.parent = Some(DEV_101F1)),
    ];
    rejected_by(apply(&m, &ops), crate::DEVICE_ACYCLIC);
}

/// A device that is its own parent is rejected.
#[test]
fn inv009_self_parent_rejected() {
    let m = mini_golden();
    let op = upd(&m, DEV_101F1, |d: &mut Device| d.parent = Some(DEV_101F1));
    rejected_by(apply(&m, &[op]), crate::DEVICE_ACYCLIC);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-010

/// A plain terminal device is accepted.
#[test]
fn inv010_terminal_aspects_ok() {
    let m = mini_golden();
    assert!(apply(&m, &[ins(terminal(nid(0x10_000), DEV_X1031, 5, "5"))]).is_ok());
}

/// A terminal that also has the strip aspect is rejected.
#[test]
fn inv010_terminal_aspects_rejected() {
    let m = mini_golden();
    let strip_aspect = m
        .row(&Device::key(DEV_X1031))
        .unwrap()
        .get::<Device>()
        .unwrap()
        .aspects
        .strip
        .clone();
    let mut t = terminal(nid(0x10_000), DEV_X1031, 5, "5");
    t.aspects.strip = strip_aspect;
    rejected_by(apply(&m, &[ins(t)]), crate::TERMINAL_ASPECTS);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-011

/// A child with the parent's aspect is accepted.
#[test]
fn inv011_structure_same_aspect_ok() {
    let m = mini_golden();
    let n = structure_node(nid(0x10_000), Aspect::Location, "L2", Some(LOC));
    assert!(apply(&m, &[ins(n)]).is_ok());
}

/// A Function-aspect node under a Location node is rejected.
#[test]
fn inv011_structure_same_aspect_rejected() {
    let m = mini_golden();
    let n = structure_node(nid(0x10_000), Aspect::Function, "F2", Some(LOC));
    rejected_by(apply(&m, &[ins(n)]), crate::STRUCTURE_SAME_ASPECT);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-012

/// A stored MultiLine page is accepted.
#[test]
fn inv012_no_stored_report_page_ok() {
    let m = mini_golden();
    let p = page(nid(0x10_000), Some(SEC1), 103, "New page");
    assert!(apply(&m, &[ins(p)]).is_ok());
}

/// A stored Report page is rejected.
#[test]
fn inv012_no_stored_report_page_rejected() {
    let m = mini_golden();
    let mut p = page(nid(0x10_000), Some(SEC1), 103, "Report");
    p.kind = PageType::Report;
    rejected_by(apply(&m, &[ins(p)]), crate::NO_STORED_REPORT_PAGE);
}

// ---------------------------------------------------------------------------------------------------------
// Catalogue

/// Every rule of `RULES` (14 of them) has its table row in `invariants.md`.
#[test]
fn every_catalogue_rule_has_a_test() {
    let catalogue = include_str!("../../invariants.md");
    assert_eq!(RULES.len(), 14);
    for rule in RULES {
        let row = format!("| {} |", rule.id);
        assert!(
            catalogue.lines().any(|l| l.starts_with(&row)),
            "{} has no row in invariants.md",
            rule.id
        );
    }
}
