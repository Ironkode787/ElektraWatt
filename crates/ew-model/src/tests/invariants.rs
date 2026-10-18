// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One positive and at least one negative test per structural invariant of `invariants.md`.

use ew_ids::{Id, Uuid};
use ew_rds::Aspect;
use ew_schema::{
    Connection, Device, FnRole, Function, IpDirection, Junction, KeyedList, PageType, PinMapEntry, Record,
    ReportBlock, Representation, StructureNode,
};
use serde_json::json;

use super::fixture::{
    CON_J5_IN, CON_W22, CON_W27, DEV_3W1, DEV_101F1, DEV_102F2, DEV_102G1, DEV_X1031, FN_101F1, FN_102F2,
    FN_102F2_AUX, FN_102G1, FN_IP_101, FN_T3, FNDEF_CONTACT, FORM, FUNC_P01, J5, LOC, PAGE_101, PAGE_102,
    REP_IP_101, SEC1, at_junction, cable, conductor, connection, cp, device, function, ip_function,
    mini_golden, mini_golden_parts, page, rep, strip, structure_node, terminal,
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

/// A new interruption point without a device is accepted, together with its one representation.
#[test]
fn inv003_device_iff_not_ip_ok() {
    let m = mini_golden();
    let f = ip_function(nid(0x10_000), "L-9", IpDirection::Source);
    let r = rep(nid(0x10_001), PAGE_101, vec![f.id]);
    assert!(apply(&m, &[ins(f), ins(r)]).is_ok());
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

/// A new interruption-point function may be drawn with a device's function (in one batch with its representation).
#[test]
fn inv004_ip_function_excepted_ok() {
    let m = mini_golden();
    let ip = ip_function(nid(0x10_000), "L-9", IpDirection::Source);
    let r = rep(nid(0x10_001), PAGE_101, vec![FN_101F1, ip.id]);
    assert!(apply(&m, &[ins(ip), ins(r)]).is_ok());
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

fn pin_map_entries(second_symbol_cp: u16) -> Vec<PinMapEntry> {
    [(0, 0), (second_symbol_cp, 1)]
        .into_iter()
        .map(|(symbol_cp, idx)| PinMapEntry {
            symbol_cp,
            function: FN_101F1,
            idx,
            extra: Default::default(),
        })
        .collect()
}

fn pin_map_rep(second_symbol_cp: u16) -> Representation {
    let mut r = rep(nid(0x10_000), PAGE_101, vec![FN_101F1]);
    r.pin_map = KeyedList::from_vec(pin_map_entries(second_symbol_cp)).expect("distinct symbol CPs");
    r
}

/// A pin map with distinct symbol CPs is accepted.
#[test]
fn inv005_pin_map_unique_ok() {
    let m = mini_golden();
    assert!(apply(&m, &[ins(pin_map_rep(1))]).is_ok());
}

/// A pin map with a repeated symbol CP cannot be built: `KeyedList` makes the violation unrepresentable
/// (the `pin_map_unique` check stays as a backstop for rows that bypass the constructor).
#[test]
fn inv005_pin_map_unique_rejected() {
    assert!(KeyedList::from_vec(pin_map_entries(0)).is_err());
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
    rejected_by(apply(&m, &[ins(t)]), crate::EXCLUSIVE_ASPECTS);
}

/// A strip and a cable, each with its single aspect, are accepted.
#[test]
fn inv010_strip_and_cable_alone_ok() {
    let m = mini_golden();
    let ops = [
        ins(strip(nid(0x10_000), "X", 2000)),
        ins(cable(nid(0x10_001), "W", 2)),
    ];
    assert!(apply(&m, &ops).is_ok());
}

/// A terminal that also has the cable aspect is rejected.
#[test]
fn inv010_terminal_cable_rejected() {
    let m = mini_golden();
    let cable_aspect = m
        .row(&Device::key(DEV_3W1))
        .unwrap()
        .get::<Device>()
        .unwrap()
        .aspects
        .cable
        .clone();
    assert!(cable_aspect.is_some());
    let mut t = terminal(nid(0x10_000), DEV_X1031, 5, "5");
    t.aspects.cable = cable_aspect;
    rejected_by(apply(&m, &[ins(t)]), crate::EXCLUSIVE_ASPECTS);
}

/// A strip that also has the cable aspect is rejected.
#[test]
fn inv010_strip_cable_rejected() {
    let m = mini_golden();
    let mut s = strip(nid(0x10_000), "X", 2000);
    s.aspects.cable = cable(nid(0x10_001), "W", 2).aspects.cable;
    assert!(s.aspects.cable.is_some());
    rejected_by(apply(&m, &[ins(s)]), crate::EXCLUSIVE_ASPECTS);
}

/// Adding the strip aspect to the existing cable by an update is rejected.
#[test]
fn inv010_aspect_added_by_update_rejected() {
    let m = mini_golden();
    let strip_aspect = m
        .row(&Device::key(DEV_X1031))
        .unwrap()
        .get::<Device>()
        .unwrap()
        .aspects
        .strip
        .clone();
    let op = upd(&m, DEV_3W1, |d: &mut Device| d.aspects.strip = strip_aspect);
    rejected_by(apply(&m, &[op]), crate::EXCLUSIVE_ASPECTS);
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

/// A model with one child (a Function node) under the root `FUNC_P01`, which no record's structure slot names.
fn model_with_function_child() -> (Model, Id<StructureNode>) {
    let m = mini_golden();
    let child = nid(0x10_000);
    let n = structure_node(child, Aspect::Function, "P01.1", Some(FUNC_P01));
    (apply(&m, &[ins(n)]).expect("same aspect"), child)
}

/// Changing only a parent's aspect while its child keeps the old one is rejected (the children of a touched node are evaluated).
#[test]
fn inv011_parent_aspect_changed_rejected() {
    let (m, _) = model_with_function_child();
    let op = upd(&m, FUNC_P01, |n: &mut StructureNode| {
        n.aspect = ew_schema::Open::Known(Aspect::Location);
    });
    rejected_by(m.apply(&[op], Rev(2)), crate::STRUCTURE_SAME_ASPECT);
}

/// Changing a parent's aspect together with its children's, in one batch, is accepted.
#[test]
fn inv011_parent_and_children_changed_together_ok() {
    let (m, child) = model_with_function_child();
    let ops = [
        upd(&m, FUNC_P01, |n: &mut StructureNode| {
            n.aspect = ew_schema::Open::Known(Aspect::Location);
        }),
        upd(&m, child, |n: &mut StructureNode| {
            n.aspect = ew_schema::Open::Known(Aspect::Location);
        }),
    ];
    assert!(m.apply(&ops, Rev(2)).is_ok());
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
// EW-INV-013

/// A function and a representation, a new interruption point drawn once, in one batch.
fn new_ip_with_rep(function: u128, representation: u128) -> [Op; 2] {
    let f = ip_function(nid(function), "L-9", IpDirection::Source);
    let r = rep(nid(representation), PAGE_101, vec![f.id]);
    [ins(f), ins(r)]
}

/// A new interruption point with its one representation, created in one batch, is accepted.
#[test]
fn inv013_ip_with_one_rep_ok() {
    let m = mini_golden();
    assert!(apply(&m, &new_ip_with_rep(0x10_000, 0x10_001)).is_ok());
}

/// A new interruption point without any representation is rejected.
#[test]
fn inv013_ip_without_rep_rejected() {
    let m = mini_golden();
    let f = ip_function(nid(0x10_000), "L-9", IpDirection::Source);
    rejected_by(apply(&m, &[ins(f)]), crate::IP_ONE_REPRESENTATION);
}

/// A second representation of an interruption point is rejected.
#[test]
fn inv013_second_rep_rejected() {
    let m = mini_golden();
    let r = rep(nid(0x10_000), PAGE_102, vec![FN_IP_101]);
    rejected_by(apply(&m, &[ins(r)]), crate::IP_ONE_REPRESENTATION);
}

/// Removing the only representation of an interruption point, without the function, is rejected.
#[test]
fn inv013_rep_removed_without_function_rejected() {
    let m = mini_golden();
    let op = Op::delete(m.row(&Representation::key(REP_IP_101)).unwrap());
    rejected_by(apply(&m, &[op]), crate::IP_ONE_REPRESENTATION);
}

/// Repointing a representation to another interruption point leaves the old one bare and the new one drawn twice.
#[test]
fn inv013_rep_repointed_rejected() {
    let x = nid(0x10_000);
    let m = apply(&mini_golden(), &new_ip_with_rep(0x10_000, 0x10_001)).unwrap();
    let op = upd(&m, REP_IP_101, |r: &mut Representation| r.functions = vec![x]);
    rejected_by(m.apply(&[op], Rev(2)), crate::IP_ONE_REPRESENTATION);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-014

/// A report block of the built-in kind `toc`; page-oriented unless it is embedded on `embedded`.
fn report_block(n: u128, first_folio: Option<u32>, embedded: Option<Id<ew_schema::Page>>) -> ReportBlock {
    let mut v = json!({
        "id": Uuid::from_u128(n).to_string(),
        "source": {"kind": "toc"},
        "form": FORM.as_uuid().to_string(),
    });
    if let Some(folio) = first_folio {
        v["first_folio"] = json!(folio);
    }
    if let Some(page) = embedded {
        v["embedded"] = json!({"page": page.as_uuid().to_string(), "rect": [[0, 0], [1000, 1000]]});
    }
    serde_json::from_value(v).expect("a valid report block")
}

/// A page-oriented report block with a first folio is accepted.
#[test]
fn inv014_report_block_folio_ok() {
    let m = mini_golden();
    let b = report_block(0x10_000, Some(907), None);
    assert!(apply(&m, &[ins(b)]).is_ok());
}

/// An embedded report block needs no first folio.
#[test]
fn inv014_embedded_without_folio_ok() {
    let m = mini_golden();
    let b = report_block(0x10_000, None, Some(PAGE_101));
    assert!(b.embedded.is_some() && b.first_folio.is_none());
    assert!(apply(&m, &[ins(b)]).is_ok());
}

/// A page-oriented report block without a first folio is rejected.
#[test]
fn inv014_report_block_folio_rejected() {
    let m = mini_golden();
    let b = report_block(0x10_000, None, None);
    rejected_by(apply(&m, &[ins(b)]), crate::REPORT_BLOCK_FOLIO);
}

/// Making an embedded block without a folio page-oriented (`embedded` removed) is rejected.
#[test]
fn inv014_embedded_removed_rejected() {
    let m = mini_golden();
    let b = report_block(0x10_000, None, Some(PAGE_101));
    let id = b.id;
    let m = m.apply(&[ins(b)], Rev(1)).unwrap();
    let op = upd(&m, id, |b: &mut ReportBlock| b.embedded = None);
    rejected_by(m.apply(&[op], Rev(2)), crate::REPORT_BLOCK_FOLIO);
}

// ---------------------------------------------------------------------------------------------------------
// Catalogue

/// The names in backticks of the last (Tests) cell of the catalogue row of `rule`.
fn catalogue_tests(catalogue: &str, rule: &str) -> Vec<String> {
    let prefix = format!("| {rule} |");
    let row = catalogue
        .lines()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("{rule} has no row in invariants.md"));
    let cell = row
        .trim_end()
        .trim_end_matches('|')
        .rsplit('|')
        .next()
        .expect("a last cell");
    cell.split('`').skip(1).step_by(2).map(str::to_owned).collect()
}

/// Whether `source` has a test function `name`: `fn name(` whose attribute and doc-comment lines above it
/// include `#[test]`.
fn has_test(source: &str, name: &str) -> bool {
    let signature = format!("fn {name}(");
    let lines: Vec<&str> = source.lines().collect();
    let Some(at) = lines.iter().position(|l| l.trim_start().starts_with(&signature)) else {
        return false;
    };
    lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("///") || l.starts_with("#["))
        .any(|l| l == "#[test]")
}

/// Every rule of `RULES` (16 of them) has its table row in `invariants.md`; every test the row names exists
/// as a `#[test]` function, with at least one accepting (`_ok`) and one rejecting (`rejected`/`referenced`) test.
#[test]
fn every_catalogue_rule_has_a_test() {
    let catalogue = include_str!("../../invariants.md");
    let invariant_tests = include_str!("invariants.rs");
    let ops_tests = include_str!("ops.rs");
    assert_eq!(RULES.len(), 16);
    for rule in RULES {
        let names = catalogue_tests(catalogue, rule.id);
        assert!(!names.is_empty(), "{} names no tests in invariants.md", rule.id);
        for name in &names {
            let (file, source, fn_name) = match name.strip_prefix("ops::") {
                Some(n) => ("ops.rs", ops_tests, n),
                None => ("invariants.rs", invariant_tests, name.as_str()),
            };
            assert!(
                has_test(source, fn_name),
                "{}: the test {name} named in invariants.md is not a #[test] fn in src/tests/{file}",
                rule.id
            );
        }
        let base = |n: &str| n.rsplit("::").next().unwrap_or(n).to_owned();
        assert!(
            names.iter().any(|n| base(n).ends_with("_ok")),
            "{} names no accepting test (a name ending in _ok)",
            rule.id
        );
        assert!(
            names
                .iter()
                .any(|n| base(n).contains("rejected") || base(n).contains("referenced")),
            "{} names no rejecting test (a name containing rejected or referenced)",
            rule.id
        );
    }
}
