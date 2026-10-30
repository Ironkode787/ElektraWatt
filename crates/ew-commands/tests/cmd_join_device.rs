// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `rep.join_device` (WP-S2.18 D3, FT-628): a placed representation joins another device, its wires kept, in one
//! transaction and one undo step; each refusal with its pointer.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use std::collections::BTreeSet;
use std::fmt::Debug;

use common::fixture::*;
use common::*;
use ew_commands::commands::{JoinDevice, KindName, LinkDevices, SetFields};
use ew_commands::{CmdError, Engine, Origin};
use ew_ids::{Id, Uuid};
use ew_model::{Entity, Model};
use ew_schema::{
    Connection, ContactKind, CpData, Device, DeviceLinkKind, Endpoint, Extra, FnDef, FnRole, Function,
    LibDocument, Part, PartRef, PartUse, Record, Representation, TemplateFn,
};

// ---------------------------------------------------------------------------------------------------------
// The fixture

const ITEM_COIL: Uuid = Uuid::from_u128(0x9601);
const ITEM_NO: Uuid = Uuid::from_u128(0x9602);
const ITEM_LAMP: Uuid = Uuid::from_u128(0x9603);

const PART_RLY: Id<Part> = id(0x5601);
/// A relay whose contact entry draws one pattern CP (index 5) for a two-CP FnDef: its CP is 5, not 0 and 1.
const PART_ODD: Id<Part> = id(0x5602);

/// The coil relay -K1: its main function is a coil.
const DEV_K1: Id<Device> = id(0x1601);
/// The source -K2: one NO contact, no part, one representation, wired at both CPs.
const DEV_K2: Id<Device> = id(0x1602);
/// -K3 with the relay part: a placed coil and two unplaced NO contact slots (template entries 1 and 2).
const DEV_K3: Id<Device> = id(0x1603);
/// -K4: two functions (not D3-eligible).
const DEV_K4: Id<Device> = id(0x1604);
/// -K5 with the odd relay part: a placed coil, no slot.
const DEV_K5: Id<Device> = id(0x1605);
/// The lamp -H1: one non-contact function, no part (a source that is no contact, a target that takes nothing).
const DEV_H1: Id<Device> = id(0x1606);

const FN_K1: Id<Function> = id(0x2601);
const FN_K2: Id<Function> = id(0x2602);
const FN_K3_COIL: Id<Function> = id(0x2603);
const FN_K3_NO1: Id<Function> = id(0x2604);
const FN_K3_NO2: Id<Function> = id(0x2605);
const FN_K4_A: Id<Function> = id(0x2606);
const FN_K4_B: Id<Function> = id(0x2607);
const FN_K5: Id<Function> = id(0x2608);
const FN_H1: Id<Function> = id(0x2609);

const REP_K1: Id<Representation> = id(0x3601);
const REP_K2: Id<Representation> = id(0x3602);
const REP_K3: Id<Representation> = id(0x3603);
const REP_K4: Id<Representation> = id(0x3604);
const REP_K5: Id<Representation> = id(0x3605);
const REP_H1: Id<Representation> = id(0x3606);

const CON_A: Id<Connection> = id(0x4601);
const CON_B: Id<Connection> = id(0x4602);

fn def(item: Uuid) -> Id<FnDef> {
    ew_ids::v5::lib_copy_id(LIBRARY_X, item)
}

fn fndef_of(item: Uuid, cps: &[&str], kind: ContactKind) -> LibDocument {
    let mut copy = fndef_copy_for(LIBRARY_X, item, cps);
    copy.item.contact_kind = kind;
    LibDocument::Fndef(copy)
}

fn entry(item: Uuid, main: bool, cps: Vec<u16>) -> TemplateFn {
    TemplateFn {
        fndef: lib_ref_to(LIBRARY_X, item),
        main,
        cps,
        extra: Extra::new(),
    }
}

fn join_library() -> Vec<LibDocument> {
    let mut lib = library();
    lib.push(fndef_of(ITEM_COIL, &["A1", "A2"], ContactKind::Coil));
    lib.push(fndef_of(ITEM_NO, &["13", "14"], ContactKind::No));
    lib.push(fndef_of(ITEM_LAMP, &["X1", "X2"], ContactKind::Terminal));
    let mut rly = part(PART_RLY, "RLY");
    rly.item.function_template = vec![
        entry(ITEM_COIL, true, vec![]),
        entry(ITEM_NO, false, vec![]),
        entry(ITEM_NO, false, vec![]),
    ];
    lib.push(LibDocument::Part(rly));
    let mut odd = part(PART_ODD, "ODD");
    odd.item.cp_pattern = ew_schema::KeyedList::from_vec(vec![
        serde_json::from_value::<CpData>(serde_json::json!({"idx": 5, "designation": "13"})).expect("a CP"),
    ])
    .expect("one CP");
    odd.item.function_template = vec![entry(ITEM_COIL, true, vec![]), entry(ITEM_NO, false, vec![5])];
    lib.push(LibDocument::Part(odd));
    lib
}

fn uses(part: Id<Part>) -> Vec<PartUse> {
    vec![PartUse {
        part: PartRef {
            copy: part,
            variant: None,
            extra: Extra::new(),
        },
        qty: dec3("1"),
        extra: Extra::new(),
    }]
}

fn at(r: Id<Representation>, f: Id<Function>, x: i64) -> Record {
    let mut rep = rep(r, PAGE_102, vec![f]);
    rep.pos = pt(x * 4 * G, 8 * G);
    Record::Representation(rep)
}

fn slot(f: Id<Function>, device: Id<Device>, item: Uuid, role: FnRole, entry: u16) -> Record {
    let mut x = function(f, device, def(item), role);
    x.template_entry = Some(entry);
    Record::Function(x)
}

fn join_model() -> Model {
    let (project, mut records, _) = mini_golden_parts();
    let mut k3 = device(DEV_K3, "K", 3);
    k3.parts = uses(PART_RLY);
    let mut k5 = device(DEV_K5, "K", 5);
    k5.parts = uses(PART_ODD);
    records.extend([
        Record::Device(device(DEV_K1, "K", 1)),
        Record::Device(device(DEV_K2, "K", 2)),
        Record::Device(k3),
        Record::Device(device(DEV_K4, "K", 4)),
        Record::Device(k5),
        Record::Device(device(DEV_H1, "H", 1)),
        Record::Function(function(FN_K1, DEV_K1, def(ITEM_COIL), FnRole::Main)),
        Record::Function(function(FN_K2, DEV_K2, def(ITEM_NO), FnRole::Main)),
        slot(FN_K3_COIL, DEV_K3, ITEM_COIL, FnRole::Main, 0),
        slot(FN_K3_NO1, DEV_K3, ITEM_NO, FnRole::Aux, 1),
        slot(FN_K3_NO2, DEV_K3, ITEM_NO, FnRole::Aux, 2),
        Record::Function(function(FN_K4_A, DEV_K4, def(ITEM_NO), FnRole::Main)),
        Record::Function(function(FN_K4_B, DEV_K4, def(ITEM_NO), FnRole::Aux)),
        slot(FN_K5, DEV_K5, ITEM_COIL, FnRole::Main, 0),
        Record::Function(function(FN_H1, DEV_H1, def(ITEM_LAMP), FnRole::Main)),
        at(REP_K1, FN_K1, 1),
        at(REP_K2, FN_K2, 2),
        at(REP_K3, FN_K3_COIL, 3),
        at(REP_K4, FN_K4_A, 4),
        at(REP_K5, FN_K5, 5),
        at(REP_H1, FN_H1, 6),
        Record::Connection(connection(CON_A, Some(PAGE_102), cp(FN_K2, 0), cp(FN_T1, 2))),
        Record::Connection(connection(CON_B, Some(PAGE_102), cp(FN_T3, 2), cp(FN_K2, 1))),
    ]);
    Model::from_records(project, records, join_library()).expect("valid model")
}

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn join(rep: Id<Representation>, device: Id<Device>) -> JoinDevice {
    JoinDevice { rep, device }
}

fn get<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> Option<T> {
    e.model().table::<T>().get(&id).map(|r| (**r).clone())
}

/// Every connection as `(id, a, b)`, the CP endpoints of `from` named as those of `to`.
fn wires(e: &Engine, from: Id<Function>, to: Id<Function>) -> BTreeSet<(Id<Connection>, String, String)> {
    let name = |p: &Endpoint| match p {
        Endpoint::Cp(f, i) if *f == from => format!("{to}:{i}"),
        Endpoint::Cp(f, i) => format!("{f}:{i}"),
        other => format!("{other:?}"),
    };
    e.model()
        .table::<Connection>()
        .iter()
        .map(|(id, c)| (*id, name(&c.a), name(&c.b)))
        .collect()
}

#[track_caller]
fn invalid_at<T: Debug>(r: Result<T, CmdError>, at: &str, needle: &str) {
    match r {
        Err(CmdError::InvalidParams { path, message, .. }) => {
            assert_eq!(path, at, "{message}");
            assert!(message.contains(needle), "`{message}` lacks `{needle}`");
        }
        other => panic!("expected InvalidParams at {at}, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// The two ways (D1)

/// A device without a slot of the FnDef whose main function is a coil takes the contact as a new auxiliary function:
/// the function record moves (same ID, so every wire is untouched), the emptied source device goes, the
/// representation stays where it was, and one undo restores the source.
#[test]
fn a_contact_joins_a_coil_relay_as_an_auxiliary_function_and_keeps_every_wire() {
    let mut e = engine(join_model());
    let before = lines(e.model());
    let wires_before = wires(&e, FN_K2, FN_K2);
    let rep_before = get(&e, REP_K2).expect("the representation");
    let r = e.execute(&join(REP_K2, DEV_K1)).expect("rep.join_device");
    assert_eq!(r.outcome.primary.map(typed::<Representation>), Some(REP_K2));
    let f = get(&e, FN_K2).expect("the moved function");
    assert_eq!(f.device, Some(DEV_K1));
    assert_eq!(f.role, FnRole::Aux);
    assert_eq!(f.template_entry, None);
    assert!(get(&e, DEV_K2).is_none(), "the emptied source device is removed");
    let rep_after = get(&e, REP_K2).expect("the representation");
    assert_eq!(rep_after.functions, vec![FN_K2]);
    assert_eq!(
        (rep_after.page, rep_after.pos, rep_after.orient),
        (rep_before.page, rep_before.pos, rep_before.orient),
        "same page, position and orientation"
    );
    assert_eq!(wires(&e, FN_K2, FN_K2), wires_before, "every wire kept");
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(lines(e.model()), before, "one undo step restores the source");
}

/// A device with an unplaced function of the FnDef: the representation fills the slot of the lowest template entry,
/// which keeps its role and entry; the wires move to it CP for CP; the old function and its device go.
#[test]
fn a_contact_fills_the_first_slot_and_its_wires_follow_cp_for_cp() {
    let mut e = engine(join_model());
    let before = lines(e.model());
    let wires_before = wires(&e, FN_K2, FN_K3_NO1);
    e.execute(&join(REP_K2, DEV_K3)).expect("rep.join_device");
    let rep = get(&e, REP_K2).expect("the representation");
    assert_eq!(rep.functions, vec![FN_K3_NO1], "the slot of template entry 1");
    let slot = get(&e, FN_K3_NO1).expect("the slot");
    assert_eq!(
        (slot.device, slot.role, slot.template_entry),
        (Some(DEV_K3), FnRole::Aux, Some(1))
    );
    assert!(get(&e, FN_K2).is_none(), "the old function is removed");
    assert!(get(&e, DEV_K2).is_none(), "its device is removed");
    assert_eq!(
        wires(&e, FN_K2, FN_K3_NO1),
        wires_before,
        "every wire kept, now at the slot's CPs"
    );
    assert_eq!(
        get(&e, CON_A).map(|c| c.a),
        Some(cp(FN_K3_NO1, 0)),
        "CP 0 stays CP 0"
    );
    assert_eq!(get(&e, CON_B).map(|c| c.b), Some(cp(FN_K3_NO1, 1)));
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(lines(e.model()), before, "one undo step restores the source");
}

// ---------------------------------------------------------------------------------------------------------
// Refusals

#[test]
fn each_refusal_names_its_pointer_and_writes_nothing() {
    let mut e = engine(join_model());
    let rev = e.model().rev();
    // `/rep`: the device has another function; the device has a part (and other functions); an interruption point;
    // a terminal.
    invalid_at(e.execute(&join(REP_K4, DEV_K1)), "/rep", "another function");
    invalid_at(e.execute(&join(REP_K3, DEV_K1)), "/rep", "exactly one function");
    invalid_at(e.execute(&join(REP_IP_101, DEV_K1)), "/rep", "interruption point");
    invalid_at(e.execute(&join(REP_T1, DEV_K1)), "/rep", "terminal");
    // `/device`: the representation's own device; a device whose family takes no auxiliary contacts; a FnDef that is
    // no contact; a part whose template draws other CPs than the connected ones.
    invalid_at(e.execute(&join(REP_K2, DEV_K2)), "/device", "name another device");
    invalid_at(
        e.execute(&join(REP_K2, DEV_H1)),
        "/device",
        "takes no auxiliary contacts",
    );
    invalid_at(e.execute(&join(REP_H1, DEV_K1)), "/device", "only a contact");
    invalid_at(
        e.execute(&join(REP_K2, DEV_K5)),
        "/device",
        "connection point 0, 1",
    );
    // A representation or device that does not exist.
    assert!(matches!(
        e.execute(&join(id(0x3fff), DEV_K1)),
        Err(CmdError::NotFound { .. })
    ));
    assert!(matches!(
        e.execute(&join(REP_K2, id(0x1fff))),
        Err(CmdError::NotFound { .. })
    ));
    assert_eq!(e.model().rev(), rev, "no refused command wrote anything");
}

/// The odd relay takes an unwired contact: its CPs only matter where something is connected.
#[test]
fn an_unwired_contact_joins_a_device_whose_template_draws_other_cps() {
    let mut e = engine(join_model());
    e.execute(&ew_commands::commands::DeleteConnections {
        connections: vec![CON_A, CON_B],
    })
    .expect("connection.delete");
    e.execute(&join(REP_K2, DEV_K5)).expect("rep.join_device");
    assert_eq!(get(&e, FN_K2).and_then(|f| f.device), Some(DEV_K5));
}

// ---------------------------------------------------------------------------------------------------------
// What the removed source carries (FX37a)

fn set_device(device: Id<Device>, path: &str, value: serde_json::Value) -> SetFields {
    SetFields {
        kind: KindName::Device,
        targets: vec![device.cast()],
        path: path.into(),
        value,
    }
}

/// A source device in a device link is refused at `/rep`: the link would go with the device (it cascades).
#[test]
fn a_source_device_in_a_device_link_is_refused_and_nothing_is_written() {
    let mut e = engine(join_model());
    e.execute(&LinkDevices {
        from: DEV_H1,
        to: DEV_K2,
        kind: DeviceLinkKind::Drives,
    })
    .expect("device.link");
    let rev = e.model().rev();
    invalid_at(e.execute(&join(REP_K2, DEV_K1)), "/rep", "remove the link first");
    invalid_at(e.execute(&join(REP_K2, DEV_K3)), "/rep", "device link");
    assert_eq!(e.model().rev(), rev, "the refused join wrote nothing");
    assert!(get(&e, DEV_K2).is_some(), "the source device stays");
}

/// The join goes ahead and its notice names each member of the source device that goes with it.
#[test]
fn a_join_that_drops_the_function_text_says_so() {
    let mut e = engine(join_model());
    e.execute(&set_device(DEV_K2, "function_text", serde_json::json!("Pump on")))
        .expect("fields.set function_text");
    e.execute(&set_device(DEV_K2, "process_tag", serde_json::json!("P101")))
        .expect("fields.set process_tag");
    let r = e.execute(&join(REP_K2, DEV_K1)).expect("rep.join_device");
    assert_eq!(
        r.outcome.notices,
        vec!["the joined device's function text and process tag were dropped with it".to_owned()]
    );
}

/// A join from a source with nothing but its tag has no notice, in either way.
#[test]
fn a_join_that_drops_nothing_has_no_notice() {
    let mut e = engine(join_model());
    let r = e
        .execute(&join(REP_K2, DEV_K1))
        .expect("rep.join_device (auxiliary)");
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    let mut e = engine(join_model());
    let r = e.execute(&join(REP_K2, DEV_K3)).expect("rep.join_device (slot)");
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
}
