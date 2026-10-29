// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One positive and at least one negative test per structural invariant of `invariants.md`.

use ew_ids::{Id, Uuid};
use ew_rds::Aspect;
use ew_schema::{
    Connection, Device, FnDef, FnRole, Function, IpDirection, Junction, KeyedList, LibCopy, LibDocument,
    PageNote, PageType, Part, PinMapEntry, Record, ReportBlock, Representation, Revision, RevisionState,
    StructureNode, SymbolDef,
};
use serde_json::json;

use super::fixture::{
    CON_J5_IN, CON_W22, CON_W27, DEV_3W1, DEV_101F1, DEV_102F2, DEV_102G1, DEV_X1031, FN_101F1, FN_102F2,
    FN_102F2_AUX, FN_102G1, FN_IP_101, FN_T3, FNDEF_CONTACT, FORM, FUNC_P01, ITEM_X, ITEM_Y, J5, LIBRARY_X,
    LOC, PAGE_101, PAGE_102, REP_IP_101, SEC1, at_junction, cable, conductor, connection, cp, device,
    fndef_copy_for, function, ip_function, mini_golden, mini_golden_parts, page, part_using, rep, strip,
    structure_node, symbol_using, terminal,
};
use crate::{AnyKey, AnyRow, ApplyOptions, Entity, Model, ModelError, Op, RULES, Rev, RowId, invert_batch};

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
    m.apply_history(ops, Rev(1))
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

/// A `LibRef` referrer that the removal of its copy leaves behind is no violation of EW-INV-001: the history
/// path (`apply`, which undo and redo use) accepts the removal, and the result passes the integrity check,
/// because the symbol's `LibRef` then names nothing (NFR-REL-11). That an edit may not remove it is EW-INV-021.
#[test]
fn inv001_lib_ref_referrer_left_by_removal_ok() {
    let (m, fkey, _) = lib_ref_model(true);
    let op = Op::delete(m.row(&fkey).unwrap());
    let m1 = apply(&m, &[op]).expect("the history path removes the copy");
    assert!(m1.row(&fkey).is_none());
    assert!(m1.integrity_check().is_ok());
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-021 (delete-time): the `LibRef` keep-alive

/// The mini golden project with an embedded symbol copy whose `fndef` names `ITEM_X` of `LIBRARY_X`, with or
/// without the FnDef copy that the `LibRef` resolves to; the keys of that FnDef copy and of the symbol.
fn lib_ref_model(embedded: bool) -> (Model, AnyKey, AnyKey) {
    let copy = fndef_copy_for(LIBRARY_X, ITEM_X, &["1"]);
    let sym = symbol_using(nid(0x21_000), LIBRARY_X, ITEM_X);
    let fkey = <LibCopy<FnDef> as Entity>::key(copy.id);
    let skey = <LibCopy<SymbolDef> as Entity>::key(sym.id);
    let (project, records, mut lib) = mini_golden_parts();
    lib.push(LibDocument::Symbol(sym));
    if embedded {
        lib.push(LibDocument::Fndef(copy));
    }
    let m = Model::from_records(project, records, lib).expect("a valid model");
    (m, fkey, skey)
}

/// A batch on the edit path (`apply_with`, as every command commits), with no prevent hook.
fn edit(m: &Model, ops: &[Op]) -> Result<Model, ModelError> {
    m.apply_with(ops, m.rev().next(), &ApplyOptions::default())
}

/// An edit may remove an embedded FnDef copy that no record names, and a copy together with the last record
/// that names it, in one batch.
#[test]
fn inv021_unnamed_copy_removed_by_an_edit_ok() {
    let (m, fkey, skey) = lib_ref_model(true);
    let both = [
        Op::delete(m.row(&skey).unwrap()),
        Op::delete(m.row(&fkey).unwrap()),
    ];
    assert!(edit(&m, &both).is_ok(), "the copy with its referrer");
    let m1 = edit(&m, &both[..1]).expect("the symbol alone");
    let op = Op::delete(m1.row(&fkey).unwrap());
    assert!(edit(&m1, &[op]).is_ok(), "the copy nothing names any more");
}

/// An edit that removes an FnDef copy which a record still names through a `LibRef` is `Referenced`, for a
/// symbol's `fndef` and for a part's function template, also when the removal op was made by hand (not by
/// the delete plan, which refuses it first): the rule is the model's, not a command's.
#[test]
fn inv021_named_copy_removed_by_an_edit_referenced() {
    let (m, fkey, skey) = lib_ref_model(true);
    let op = Op::delete(m.row(&fkey).unwrap());
    match edit(&m, std::slice::from_ref(&op)).unwrap_err() {
        ModelError::Referenced {
            link,
            target,
            referrer,
        } => assert_eq!(
            (link.as_str(), target, referrer),
            ("symbol.fndef.item", fkey, skey)
        ),
        other => panic!("expected Referenced, got {other:?}"),
    }
    // the same removal with an unrelated change in the batch: still refused
    let node = structure_node(nid(0x21_001), Aspect::Location, "J2", Some(LOC));
    assert!(matches!(
        edit(&m, &[ins(node), op]),
        Err(ModelError::Referenced { link, .. }) if link == "symbol.fndef.item"
    ));

    let copy = fndef_copy_for(LIBRARY_X, ITEM_Y, &["1"]);
    let part = part_using(nid(0x21_002), "TPL-21", LIBRARY_X, ITEM_Y);
    let pkey = <LibCopy<Part> as Entity>::key(part.id);
    let fkey = <LibCopy<FnDef> as Entity>::key(copy.id);
    let m = edit(&m, &[ins(copy), ins(part)]).expect("embedding a part with its FnDef");
    match edit(&m, &[Op::delete(m.row(&fkey).unwrap())]).unwrap_err() {
        ModelError::Referenced {
            link,
            target,
            referrer,
        } => assert_eq!(
            (link.as_str(), target, referrer),
            ("part.function_template[].fndef.item", fkey, pkey)
        ),
        other => panic!("expected Referenced, got {other:?}"),
    }
}

/// FT-316: the history path restores the state before an embedding. A symbol names an FnDef item the project
/// does not embed (valid); embedding the copy makes the `LibRef` a link. The edit path refuses to remove the
/// copy again (a user's delete of a named copy), the history path (the undo of the embedding) restores the
/// state before it, which passes the integrity check, and redo embeds it again.
#[test]
fn inv021_undo_of_an_embedding_restores_the_state_before_ok() {
    let (m0, fkey, skey) = lib_ref_model(false);
    assert!(
        m0.integrity_check().is_ok(),
        "a LibRef to a copy that is not embedded"
    );
    let embed = [ins(fndef_copy_for(LIBRARY_X, ITEM_X, &["1"]))];
    let m1 = edit(&m0, &embed).expect("embedding the FnDef the symbol names");
    let named: Vec<AnyKey> = m1.referrers(&fkey).into_iter().map(|(_, k)| k).collect();
    assert_eq!(named, vec![skey], "the LibRef is a link now");
    let undo = invert_batch(&embed);
    assert!(
        matches!(edit(&m1, &undo), Err(ModelError::Referenced { .. })),
        "an edit may not remove the named copy"
    );
    let m2 = m1
        .apply_history(&undo, m1.rev().next())
        .expect("the undo of the embedding");
    assert!(m2.content_eq(&m0), "undo restores the state before the embedding");
    assert!(m2.integrity_check().is_ok());
    let m3 = m2
        .apply_history(&embed, m2.rev().next())
        .expect("the redo of the embedding");
    assert!(m3.content_eq(&m1));
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
    let m = m.apply_history(&[ins(r)], Rev(1)).unwrap();
    let op = upd(&m, FN_102F2_AUX, |f: &mut Function| {
        f.device = Some(DEV_101F1);
        f.role = FnRole::Aux;
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::REP_ONE_DEVICE);
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
// EW-INV-015

/// A terminal whose parent is its own strip, and a child of a terminal's strip, are accepted: both edges
/// of each point the same way.
#[test]
fn inv015_inheritance_acyclic_ok() {
    let m = mini_golden();
    let mut t = terminal(nid(0x10_000), DEV_X1031, 5, "5");
    t.parent = Some(DEV_X1031);
    let mut child = device(nid(0x10_001), "Q", 9);
    child.parent = Some(DEV_X1031);
    let m = apply(&m, &[ins(t), ins(child)]).expect("accepted");
    assert!(m.integrity_check().is_ok());
}

/// Review probe 1: terminal B of strip S, and S's parent is B. No parent chain is cyclic (EW-INV-009
/// holds), but S inherits from B and B from S.
#[test]
fn inv015_strip_whose_parent_is_its_terminal_rejected() {
    let m = mini_golden();
    let b = terminal(nid(0x10_000), DEV_X1031, 5, "5");
    let m = apply(&m, &[ins(b)]).expect("the terminal alone is valid");
    let op = upd(&m, DEV_X1031, |d: &mut Device| {
        d.parent = Some(nid(0x10_000));
        d.tag.scheme = ew_schema::TagScheme::Child;
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::INHERITANCE_ACYCLIC);
}

/// Review probe 2: A's parent is terminal B, B's strip is S, S's parent is A.
#[test]
fn inv015_three_device_cycle_rejected() {
    let m = mini_golden();
    let a = nid::<Device>(0x10_001);
    let mut dev_a = device(a, "K", 9);
    dev_a.parent = Some(nid(0x10_000));
    let b = terminal(nid(0x10_000), DEV_X1031, 5, "5");
    let m = apply(&m, &[ins(b), ins(dev_a)]).expect("valid without the closing edge");
    let op = upd(&m, DEV_X1031, |d: &mut Device| d.parent = Some(a));
    rejected_by(m.apply_history(&[op], Rev(2)), crate::INHERITANCE_ACYCLIC);
}

/// The integrity check of a loaded project rejects an inheritance cycle (it runs every invariant on every
/// record, `Model::from_records`).
#[test]
fn inv015_cycle_rejected_on_load() {
    let (p, mut records, lib) = mini_golden_parts();
    records.push(Record::Device(terminal(nid(0x10_000), DEV_X1031, 5, "5")));
    for r in &mut records {
        if let Record::Device(d) = r {
            if d.id == DEV_X1031 {
                d.parent = Some(nid(0x10_000));
            }
        }
    }
    rejected_by(Model::from_records(p, records, lib), crate::INHERITANCE_ACYCLIC);
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
    rejected_by(m.apply_history(&[op], Rev(2)), crate::STRUCTURE_SAME_ASPECT);
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
    assert!(m.apply_history(&ops, Rev(2)).is_ok());
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
    rejected_by(m.apply_history(&[op], Rev(2)), crate::IP_ONE_REPRESENTATION);
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
    let m = m.apply_history(&[ins(b)], Rev(1)).unwrap();
    let op = upd(&m, id, |b: &mut ReportBlock| b.embedded = None);
    rejected_by(m.apply_history(&[op], Rev(2)), crate::REPORT_BLOCK_FOLIO);
}

// ---------------------------------------------------------------------------------------------------------
// EW-INV-016 … EW-INV-019: revisions

fn revision(n: u128, label: &str, state: RevisionState) -> Revision {
    Revision {
        id: nid(n),
        v: 1,
        label: ew_mltext::Ident::from_input(label).expect("a label"),
        date: None,
        author: None,
        checker: None,
        description: ew_mltext::MlText::default(),
        state,
        page_notes: Vec::new(),
        withdraws: None,
        source_ref: None,
        extra: Default::default(),
    }
}

fn note(page: Id<ew_schema::Page>) -> PageNote {
    PageNote {
        page,
        description: ew_mltext::MlText::default(),
        extra: Default::default(),
    }
}

/// Golden A's history: V1.0 released (historic), V2.0 open.
fn with_history() -> Model {
    let m = mini_golden();
    let v1 = revision(0x60_001, "V1.0", RevisionState::Released);
    let v2 = revision(0x60_002, "V2.0", RevisionState::Open);
    m.apply_history(&[ins(v1), ins(v2)], Rev(1))
        .expect("the history is valid")
}

/// One open revision among released and withdrawn ones is accepted.
#[test]
fn inv016_one_open_revision_ok() {
    let m = with_history();
    let w = revision(0x60_003, "V1.0", RevisionState::Withdrawn);
    assert!(m.apply_history(&[ins(w)], Rev(2)).is_ok());
}

/// A second open revision is rejected.
#[test]
fn inv016_second_open_revision_rejected() {
    let m = with_history();
    let v3 = revision(0x60_003, "V3.0", RevisionState::Open);
    rejected_by(m.apply_history(&[ins(v3)], Rev(2)), crate::ONE_OPEN_REVISION);
}

/// Reopening a released revision while another is open is rejected.
#[test]
fn inv016_reopened_revision_rejected() {
    let m = with_history();
    let op = upd(&m, nid(0x60_001), |r: &mut Revision| {
        r.state = RevisionState::Open
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::ONE_OPEN_REVISION);
}

/// Distinct labels are accepted, and a withdrawal record may repeat the label of the revision it withdraws.
#[test]
fn inv017_revision_label_unique_ok() {
    let m = with_history();
    let w = revision(0x60_003, "V2.0", RevisionState::Withdrawn);
    assert!(m.apply_history(&[ins(w)], Rev(2)).is_ok());
}

/// A second revision with an existing label is rejected.
#[test]
fn inv017_duplicate_label_rejected() {
    let m = with_history();
    let twin = revision(0x60_003, "V1.0", RevisionState::Released);
    rejected_by(
        m.apply_history(&[ins(twin)], Rev(2)),
        crate::REVISION_LABEL_UNIQUE,
    );
}

/// Relabelling the open revision to a released revision's label is rejected.
#[test]
fn inv017_relabel_rejected() {
    let m = with_history();
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| {
        r.label = ew_mltext::Ident::from_input("V1.0").unwrap();
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::REVISION_LABEL_UNIQUE);
}

/// Author and checker of different roles, or either unset, are accepted.
#[test]
fn inv018_revision_four_eyes_ok() {
    let m = with_history();
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| {
        r.author = Some(ew_schema::PartyRole::Engineer);
        r.checker = Some(ew_schema::PartyRole::Checker);
    });
    let m = m.apply_history(&[op], Rev(2)).expect("author and checker differ");
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| r.author = None);
    assert!(m.apply_history(&[op], Rev(3)).is_ok());
}

/// The same party as author and checker is rejected (D-34, FR-REV-005).
#[test]
fn inv018_author_is_checker_rejected() {
    let m = with_history();
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| {
        r.author = Some(ew_schema::PartyRole::Engineer);
        r.checker = Some(ew_schema::PartyRole::Engineer);
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::REVISION_FOUR_EYES);
}

/// Notes on two pages are accepted, also for a page that does not exist (a page note is not a link).
#[test]
fn inv019_page_notes_unique_ok() {
    let m = with_history();
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| {
        r.page_notes = vec![note(PAGE_101), note(PAGE_102), note(nid(0x60_0ff))];
    });
    assert!(m.apply_history(&[op], Rev(2)).is_ok());
}

/// Two notes on one page are rejected.
#[test]
fn inv019_two_notes_on_one_page_rejected() {
    let m = with_history();
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| {
        r.page_notes = vec![note(PAGE_101), note(PAGE_102), note(PAGE_101)];
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::REVISION_PAGE_NOTE_UNIQUE);
}

/// A released revision may be removed (the undo of its creation needs a delete), and the open revision may
/// become Released (the release command, later).
#[test]
fn inv020_released_revision_delete_ok() {
    let m = with_history();
    let op = Op::delete(m.row(&Revision::key(nid(0x60_001))).expect("V1.0"));
    let m = m
        .apply_history(&[op], Rev(2))
        .expect("a released revision is removed");
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| {
        r.state = RevisionState::Released;
    });
    assert!(m.apply_history(&[op], Rev(3)).is_ok());
}

/// An update of a released revision is rejected: a member, or its state (a withdrawal is a new record), and
/// so is an update of a withdrawal record.
#[test]
fn inv020_released_revision_update_rejected() {
    let m = with_history();
    let op = upd(&m, nid(0x60_001), |r: &mut Revision| {
        r.description = ew_mltext::MlText::neutral("rewritten");
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::REVISION_IMMUTABLE);
    let op = upd(&m, nid(0x60_001), |r: &mut Revision| {
        r.state = RevisionState::Withdrawn;
    });
    rejected_by(m.apply_history(&[op], Rev(2)), crate::REVISION_IMMUTABLE);
    let w = revision(0x60_003, "V1.0", RevisionState::Withdrawn);
    let m = m.apply_history(&[ins(w)], Rev(2)).expect("a withdrawal record");
    let op = upd(&m, nid(0x60_003), |r: &mut Revision| {
        r.label = ew_mltext::Ident::from_input("V0.1").unwrap();
    });
    rejected_by(m.apply_history(&[op], Rev(3)), crate::REVISION_IMMUTABLE);
}

/// Deleting a page that a revision notes leaves the note (history), and the integrity check of a loaded
/// project with two open revisions rejects it.
#[test]
fn inv016_two_open_revisions_rejected_on_load() {
    let m = with_history();
    let op = upd(&m, nid(0x60_002), |r: &mut Revision| {
        r.page_notes = vec![note(PAGE_101)]
    });
    let m = m.apply_history(&[op], Rev(2)).unwrap();
    let plan = m.delete_plan(&ew_schema::Page::key(PAGE_101)).expect("a plan");
    let after = m
        .apply_history(&plan, Rev(3))
        .expect("the page goes, the note stays");
    let kept = after.revisions.get(&nid(0x60_002)).unwrap();
    assert_eq!(kept.page_notes.len(), 1);

    let (project, mut records, lib) = mini_golden_parts();
    records.push(Record::Revision(revision(0x60_001, "V1.0", RevisionState::Open)));
    records.push(Record::Revision(revision(0x60_002, "V2.0", RevisionState::Open)));
    rejected_by(
        Model::from_records(project, records, lib),
        crate::ONE_OPEN_REVISION,
    );
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

/// Every rule of `RULES` (23 of them) has its table row in `invariants.md`; every test the row names exists
/// as a `#[test]` function, with at least one accepting (`_ok`) and one rejecting (`rejected`/`referenced`) test.
#[test]
fn every_catalogue_rule_has_a_test() {
    let catalogue = include_str!("../../invariants.md");
    let invariant_tests = include_str!("invariants.rs");
    let ops_tests = include_str!("ops.rs");
    let rows = catalogue
        .lines()
        .skip_while(|l| !l.starts_with("| ID | Class |"))
        .skip(2)
        .take_while(|l| l.starts_with("| "))
        .count();
    assert_eq!(RULES.len(), rows, "one catalogue row per rule");
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

// ---------------------------------------------------------------------------------------------------------
// EW-INV-022 and EW-INV-023 (WP-S2.04 part 4a, ADR-0009 amendment 8)

/// A graphic and a structure box on an existing page are accepted.
#[test]
fn inv022_items_and_boxes_on_their_page_ok() {
    let m = mini_golden();
    let ops = [
        ins(super::fixture::graphic_line(nid(0x43_000), PAGE_101)),
        ins(super::fixture::structure_box(
            nid(0x44_000),
            PAGE_101,
            Some(LOC),
            None,
        )),
    ];
    assert!(edit(&m, &ops).is_ok());
}

/// A page item or a structure box on a page that does not exist is rejected, through the page link of EW-INV-001.
#[test]
fn inv022_item_on_missing_page_rejected() {
    let m = mini_golden();
    rejected_by(
        edit(&m, &[ins(super::fixture::graphic_line(nid(0x43_000), nid(0x39)))]),
        crate::REFS_RESOLVE,
    );
    rejected_by(
        edit(
            &m,
            &[ins(super::fixture::structure_box(
                nid(0x44_000),
                nid(0x39),
                None,
                None,
            ))],
        ),
        crate::REFS_RESOLVE,
    );
}

/// A wire marker listing connections of its own page is accepted.
#[test]
fn inv023_marker_on_its_page_ok() {
    let m = mini_golden();
    let marker = super::fixture::wire_marker(nid(0x43_000), PAGE_102, vec![CON_W22, CON_W27]);
    assert!(edit(&m, &[ins(marker)]).is_ok());
}

/// A wire marker listing a connection that does not exist is rejected.
#[test]
fn inv023_missing_connection_rejected() {
    let m = mini_golden();
    let marker = super::fixture::wire_marker(nid(0x43_000), PAGE_102, vec![CON_W22, nid(0x4ff)]);
    rejected_by(edit(&m, &[ins(marker)]), crate::MARKER_CONNECTIONS);
}

/// A wire marker listing a connection of another page is rejected.
#[test]
fn inv023_connection_on_another_page_rejected() {
    let m = mini_golden();
    let marker = super::fixture::wire_marker(nid(0x43_000), PAGE_101, vec![CON_W22]);
    rejected_by(edit(&m, &[ins(marker)]), crate::MARKER_CONNECTIONS);
}

/// A connection moved to another page while a marker lists it is rejected.
#[test]
fn inv023_connection_moved_off_its_page_rejected() {
    let m = mini_golden();
    let marker = super::fixture::wire_marker(nid(0x43_000), PAGE_102, vec![CON_W22]);
    let m = edit(&m, &[ins(marker)]).expect("a marker");
    let op = upd(&m, CON_W22, |c: &mut Connection| c.page = Some(PAGE_101));
    rejected_by(edit(&m, &[op]), crate::MARKER_CONNECTIONS);
}

/// A project whose wire marker lists a deleted connection and one of another page (an older build kept the record
/// verbatim) loads: the integrity check does not judge the list (a load finding reports it); an edit that keeps the
/// stale entries and one that drops them both commit, and so do their undo and redo.
#[test]
fn inv023_stale_list_loads_and_edits_keep_it_ok() {
    let mut records: Vec<Record> = Vec::new();
    let base = mini_golden();
    let marker = super::fixture::wire_marker(nid(0x43_000), PAGE_102, vec![CON_W22, nid(0x4ff), CON_W11A_ID]);
    records.push(Record::PageItem(marker.clone()));
    let m = reload_with(&base, records);
    let moved = upd(&m, marker.id, |i: &mut ew_schema::PageItem| {
        if let ew_schema::PageItemKind::WireMarker(w) = &mut i.kind {
            w.at = ew_units::Pt2::new(ew_units::Nm(0), ew_units::Nm(0));
        }
    });
    let m1 = edit(&m, std::slice::from_ref(&moved)).expect("an edit that keeps the stale entries");
    let undone = m1
        .apply_history(&invert_batch(&[moved]), m1.rev().next())
        .expect("undo");
    assert!(undone.content_eq(&m));
    let cleaned = upd(&m, marker.id, |i: &mut ew_schema::PageItem| {
        if let ew_schema::PageItemKind::WireMarker(w) = &mut i.kind {
            w.connections = vec![CON_W22];
        }
    });
    let m2 = edit(&m, std::slice::from_ref(&cleaned)).expect("an edit that writes a clean list");
    assert!(
        m2.apply_history(&invert_batch(&[cleaned]), m2.rev().next())
            .is_ok(),
        "undo restores the stale list"
    );
}

/// An edit that adds a stale entry to a list is rejected, also when the list held stale entries already.
#[test]
fn inv023_added_stale_entry_rejected() {
    let base = mini_golden();
    let marker = super::fixture::wire_marker(nid(0x43_000), PAGE_102, vec![CON_W22, nid(0x4ff)]);
    let m = reload_with(&base, vec![Record::PageItem(marker.clone())]);
    let op = upd(&m, marker.id, |i: &mut ew_schema::PageItem| {
        if let ew_schema::PageItemKind::WireMarker(w) = &mut i.kind {
            w.connections.push(nid(0x4fe));
        }
    });
    rejected_by(edit(&m, &[op]), crate::MARKER_CONNECTIONS);
}

/// The ID of `CON_W11A`, a connection of page 101.
const CON_W11A_ID: Id<Connection> = super::fixture::CON_W11A;

/// The mini golden project plus `extra` records, loaded through `Model::from_records` (the integrity check of an
/// open).
fn reload_with(_base: &Model, extra: Vec<Record>) -> Model {
    let (project, mut records, lib) = super::fixture::mini_golden_parts();
    records.extend(extra);
    Model::from_records(project, records, lib).expect("a stale list is not an integrity violation")
}
