// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 4b in the engine: normaliser N5 `bus_ports` (§8a.2, D16; acceptance BU-7g to BU-7j). A part's bus
//! ports go once to the device's port-less functions whose FnDef has contact kind `bus_port`, in list and ID order;
//! a port is never rewritten or removed; a disagreement and a port of a newer format give the notices N5-b and N5-c.
//! The window path with the starter parts (BU-7f) is `ew-api`'s `bus_part4b.rs`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AssignModeParam, AssignPart, JoinDevice, KindName, PlaceSymbol, PlaceTarget, SetFields,
};
use ew_commands::normalise::ORDER;
use ew_commands::{CmdError, Engine, Receipt};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    AddrValue, BusPort, BusSection, BusSystem, ContactKind, Device, FnDef, FnRole, Function, Kind, LibCopy,
    LibDocument, NetAddress, OnOff, Part, PartRef, PartUse, PortRole, Record,
};
use ew_units::Orient2;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------------------------------------
// Fixture

/// A bus-port FnDef (`ew.fn.dp-port`'s contact kind).
const FNDEF_DP: Id<FnDef> = id(0x6a0);
/// A black-box FnDef.
const FNDEF_BOX: Id<FnDef> = id(0x6a1);
/// A bus connector FnDef (`ew.fn.dp-connector`'s contact kind).
const FNDEF_PLUG: Id<FnDef> = id(0x6a2);

/// A CPU part with one DP master port.
const CPU: Id<Part> = id(0x6b0);
/// A CPU part whose DP port is a PROFINET controller.
const CPU_PN: Id<Part> = id(0x6b1);
/// A part without a bus section.
const PLAIN: Id<Part> = id(0x6b2);
/// A CPU part whose DP port is of a newer format.
const CPU_SX: Id<Part> = id(0x6b3);
/// A bus connector part: a passive port with its termination switch on.
const PLUG: Id<Part> = id(0x6b4);
/// A part with the ports DP, DP again (ignored) and X1 P1R.
const CPU_2P: Id<Part> = id(0x6b5);

/// A black box with two port-less DP-port functions and no part.
const DEV_BOX: Id<Device> = id(0x6c0);
/// A black box without a bus-port function.
const DEV_NOBUS: Id<Device> = id(0x6c1);
/// A connector device with a port-less connector function.
const DEV_PLUG: Id<Device> = id(0x6c2);
/// A device of the part CPU with a black box and no bus-port function.
const DEV_CPU: Id<Device> = id(0x6c3);
/// A black box with three port-less DP-port functions.
const DEV_3P: Id<Device> = id(0x6c4);
/// A device with one port-less DP-port function and no part (it joins another device).
const DEV_LOOSE: Id<Device> = id(0x6c5);

const FN_BOX: Id<Function> = id(0x6d0);
const FN_P1: Id<Function> = id(0x6d1);
const FN_P2: Id<Function> = id(0x6d2);
const FN_NOBUS: Id<Function> = id(0x6d3);
const FN_PLUG: Id<Function> = id(0x6d4);
const FN_CPU: Id<Function> = id(0x6d5);
const FN_3A: Id<Function> = id(0x6d6);
const FN_3B: Id<Function> = id(0x6d7);
const FN_3C: Id<Function> = id(0x6d8);
const FN_3BOX: Id<Function> = id(0x6d9);
const FN_LOOSE: Id<Function> = id(0x6da);
const REP_LOOSE: Id<ew_schema::Representation> = id(0x6e0);

fn fndef_of(def: Id<FnDef>, kind: ContactKind) -> LibCopy<FnDef> {
    let mut copy = fndef(def, &["1", "2"]);
    copy.item.contact_kind = kind;
    copy
}

fn bus_part(part_id: Id<Part>, number: &str, ports: Value) -> LibCopy<Part> {
    let mut copy = part(part_id, number);
    copy.item.sections.bus =
        Some(serde_json::from_value::<BusSection>(json!({ "ports": ports })).expect("a bus section"));
    copy
}

fn uses(part: Id<Part>) -> Vec<PartUse> {
    vec![PartUse {
        part: PartRef::new(part),
        qty: dec3("1"),
        extra: ew_schema::Extra::new(),
    }]
}

fn model() -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(LibDocument::Fndef(fndef_of(FNDEF_DP, ContactKind::BusPort)));
    lib.push(LibDocument::Fndef(fndef_of(FNDEF_BOX, ContactKind::BlackBox)));
    lib.push(LibDocument::Fndef(fndef_of(FNDEF_PLUG, ContactKind::BusPort)));
    lib.push(LibDocument::Part(bus_part(
        CPU,
        "CPU-DP",
        json!([{"interface": "DP", "bus": "profibus_dp", "role": "master"}]),
    )));
    lib.push(LibDocument::Part(bus_part(
        CPU_PN,
        "CPU-PN",
        json!([{"interface": "DP", "bus": "profinet", "role": "controller"}]),
    )));
    lib.push(LibDocument::Part(part(PLAIN, "PLAIN-1")));
    lib.push(LibDocument::Part(bus_part(
        CPU_SX,
        "CPU-SX",
        json!([{"interface": "DP", "bus": "sercos", "role": "master"}]),
    )));
    lib.push(LibDocument::Part(bus_part(
        PLUG,
        "PLUG-1",
        json!([{"interface": "DP", "bus": "profibus_dp", "role": "passive", "termination": "on"}]),
    )));
    lib.push(LibDocument::Part(bus_part(
        CPU_2P,
        "CPU-2P",
        json!([
            {"interface": "DP", "bus": "profibus_dp", "role": "master"},
            {"interface": "DP", "bus": "profinet", "role": "controller"},
            {"interface": "X1", "port_label": "P1R", "bus": "profinet", "role": "controller"}
        ]),
    )));
    let mut cpu = device(DEV_CPU, "A", 13);
    cpu.parts = uses(CPU);
    for d in [
        device(DEV_BOX, "A", 10),
        device(DEV_NOBUS, "A", 11),
        device(DEV_PLUG, "X", 12),
        cpu,
        device(DEV_3P, "A", 14),
        device(DEV_LOOSE, "A", 15),
    ] {
        records.push(Record::Device(d));
    }
    for f in [
        function(FN_BOX, DEV_BOX, FNDEF_BOX, FnRole::Main),
        function(FN_P1, DEV_BOX, FNDEF_DP, FnRole::Aux),
        function(FN_P2, DEV_BOX, FNDEF_DP, FnRole::Aux),
        function(FN_NOBUS, DEV_NOBUS, FNDEF_BOX, FnRole::Main),
        function(FN_PLUG, DEV_PLUG, FNDEF_PLUG, FnRole::Main),
        function(FN_CPU, DEV_CPU, FNDEF_BOX, FnRole::Main),
        function(FN_3BOX, DEV_3P, FNDEF_BOX, FnRole::Main),
        function(FN_3A, DEV_3P, FNDEF_DP, FnRole::Aux),
        function(FN_3B, DEV_3P, FNDEF_DP, FnRole::Aux),
        function(FN_3C, DEV_3P, FNDEF_DP, FnRole::Aux),
        function(FN_LOOSE, DEV_LOOSE, FNDEF_DP, FnRole::Main),
    ] {
        records.push(Record::Function(f));
    }
    records.push(Record::Representation(rep(REP_LOOSE, PAGE_102, vec![FN_LOOSE])));
    Model::from_records(p, records, lib).expect("the bus model is valid")
}

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn port(e: &Engine, f: Id<Function>) -> Option<BusPort> {
    row(e, f).bus_port.clone()
}

fn assign(
    devices: Vec<Id<Device>>,
    part: Id<Part>,
    mode: AssignModeParam,
    from: Option<Id<Part>>,
) -> AssignPart {
    AssignPart {
        devices,
        part,
        copies: vec![],
        mode,
        from,
        qty: None,
    }
}

fn set(kind: KindName, target: ew_schema::AnyId, path: &str, value: Value) -> SetFields {
    SetFields {
        kind,
        targets: vec![target],
        path: path.into(),
        value,
        copies: Vec::new(),
    }
}

/// `device.place` of a DP-port function joining `device` as an auxiliary function; the new function.
fn place_port(e: &mut Engine, device: Id<Device>) -> (Id<Function>, Receipt) {
    let r = e
        .execute(&PlaceSymbol {
            copies: Vec::new(),
            page: PAGE_102,
            symbol: SYM_TWO,
            pos: pt(0, 0),
            orient: Orient2::R0,
            rep_type: None,
            variant: 0,
            target: PlaceTarget::JoinDevice {
                device,
                fndef: Some(FNDEF_DP),
                role: FnRole::Aux,
            },
            connect: vec![],
        })
        .expect("device.place join_device");
    let f = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Function)
        .map(|k| typed::<Function>(*k))
        .expect("a new function");
    (f, r)
}

/// The notices of a receipt that N5 gives (both texts name a bus port).
fn bus_notices(r: &Receipt) -> Vec<String> {
    r.outcome
        .notices
        .iter()
        .filter(|n| n.contains("bus port"))
        .cloned()
        .collect()
}

/// The port D16 seeds from a part port: no nets, the address `tbd` (`not_applicable` when passive).
fn seeded(
    interface: &str,
    label: Option<&str>,
    bus: BusSystem,
    role: PortRole,
    termination: Option<OnOff>,
) -> BusPort {
    BusPort {
        interface: ident(interface),
        port_label: label.map(ident),
        address: if role == PortRole::Passive {
            AddrValue::NotApplicable
        } else {
            AddrValue::Tbd
        },
        bus,
        role,
        termination,
        logical_net: None,
        physical_net: None,
        extra: ew_schema::Extra::new(),
    }
}

fn dp_master() -> BusPort {
    seeded("DP", None, BusSystem::ProfibusDp, PortRole::Master, None)
}

// ---------------------------------------------------------------------------------------------------------
// BU-7j: the registry row

#[test]
fn bu7j_bus_ports_runs_after_module_pin_binding_and_before_revision_ensure_open() {
    let at = |n: &str| {
        ORDER
            .iter()
            .position(|o| *o == n)
            .unwrap_or_else(|| panic!("{n} in {ORDER:?}"))
    };
    assert_eq!(at("bus_ports"), at("module_pin_binding") + 1, "{ORDER:?}");
    assert_eq!(at("revision_ensure_open"), at("bus_ports") + 1, "{ORDER:?}");
}

// ---------------------------------------------------------------------------------------------------------
// BU-7g: part.assign and the joins

/// BU-7g: assigning the CPU to a black box with two DP-port functions gives the lower-ID function the port and leaves
/// the other port-less, without a notice; the black box itself (no bus-port FnDef) gets nothing.
#[test]
fn bu7g_assign_gives_the_lower_id_function_the_port() {
    let mut e = engine(model());
    let r = e
        .execute(&assign(vec![DEV_BOX], CPU, AssignModeParam::Append, None))
        .expect("part.assign");
    assert_eq!(port(&e, FN_P1), Some(dp_master()));
    assert_eq!(port(&e, FN_P2), None, "a surplus function stays port-less");
    assert_eq!(port(&e, FN_BOX), None, "the black box is no bus-port function");
    assert_eq!(bus_notices(&r), Vec::<String>::new());
}

/// BU-7g: a device without a bus-port function gets no port and no notice.
#[test]
fn bu7g_assign_onto_a_device_without_a_bus_port_function_gives_nothing() {
    let mut e = engine(model());
    let r = e
        .execute(&assign(vec![DEV_NOBUS], CPU, AssignModeParam::Append, None))
        .expect("part.assign");
    assert_eq!(port(&e, FN_NOBUS), None);
    assert_eq!(bus_notices(&r), Vec::<String>::new());
}

/// D16 trigger (ii): a DP-port function new to a device with a bus part (`device.place` with `join_device`) gets the
/// uncarried port in the placement's transaction; a second one finds the port carried and stays port-less.
#[test]
fn a_port_function_joining_a_cpu_gets_its_uncarried_port() {
    let mut e = engine(model());
    let (first, r) = place_port(&mut e, DEV_CPU);
    assert_eq!(port(&e, first), Some(dp_master()));
    assert_eq!(bus_notices(&r), Vec::<String>::new());
    let (second, r) = place_port(&mut e, DEV_CPU);
    assert_eq!(port(&e, second), None, "the port is carried");
    assert_eq!(bus_notices(&r), Vec::<String>::new());
    assert_eq!(port(&e, first), Some(dp_master()), "unchanged");
}

/// BU-7g's `rep.join_device` clause, as the code stands: `rep.join_device` moves a function only as an auxiliary
/// contact (WP-S2.18 D2), so a DP-port function (no contact) cannot join a CPU that has no unplaced slot of its
/// FnDef; the join is refused and N5 is not reached. Reported to the orchestrator (a contradiction between §8a.5
/// BU-7g and WP-S2.18 D2).
#[test]
fn rep_join_device_refuses_a_port_function_onto_a_cpu_without_a_slot() {
    let mut e = engine(model());
    let err = e
        .execute(&JoinDevice {
            rep: REP_LOOSE,
            device: DEV_CPU,
        })
        .expect_err("D2: only a contact joins as an auxiliary function");
    assert!(
        matches!(&err, CmdError::InvalidParams { message, .. } if message.contains("only a contact joins")),
        "{err:?}"
    );
    assert_eq!(port(&e, FN_LOOSE), None);
}

/// The first port of each (`interface`, `port_label`) pair, in list order, to the port-less functions in ID order;
/// the third function stays port-less.
#[test]
fn ports_pair_in_list_order_with_functions_in_id_order() {
    let mut e = engine(model());
    let r = e
        .execute(&assign(vec![DEV_3P], CPU_2P, AssignModeParam::Append, None))
        .expect("part.assign");
    assert_eq!(port(&e, FN_3A), Some(dp_master()));
    assert_eq!(
        port(&e, FN_3B),
        Some(seeded(
            "X1",
            Some("P1R"),
            BusSystem::Profinet,
            PortRole::Controller,
            None
        ))
    );
    assert_eq!(port(&e, FN_3C), None);
    assert_eq!(
        bus_notices(&r),
        Vec::<String>::new(),
        "the duplicate DP is ignored silently"
    );
}

// ---------------------------------------------------------------------------------------------------------
// BU-7h: never rewritten

/// BU-7h: a carried port with a set address survives `part.assign replace` to a part that declares another bus and
/// role (notice N5-b), `replace_all` to a part without a bus section and `fields.set parts []` (no notice); after
/// `fields.set bus_port null` an unrelated edit leaves the function port-less, and a new bus part seeds it again.
#[test]
fn bu7h_a_port_is_never_rewritten() {
    let mut e = engine(model());
    let (f, _) = place_port(&mut e, DEV_CPU);
    e.execute(&set(
        KindName::Function,
        f.cast(),
        "bus_port.address",
        json!({"value": {"profibus": 2}}),
    ))
    .expect("fields.set bus_port.address");
    let mut set_port = dp_master();
    set_port.address = AddrValue::Value(NetAddress::Profibus(2));
    assert_eq!(port(&e, f), Some(set_port.clone()));

    let r = e
        .execute(&assign(
            vec![DEV_CPU],
            CPU_PN,
            AssignModeParam::Replace,
            Some(CPU),
        ))
        .expect("part.assign replace");
    assert_eq!(row(&e, DEV_CPU).parts, uses(CPU_PN));
    assert_eq!(port(&e, f), Some(set_port.clone()), "kept");
    assert_eq!(
        bus_notices(&r),
        ["-102A13:DP keeps its bus port; part CPU-PN declares profinet controller"]
    );

    let r = e
        .execute(&assign(vec![DEV_CPU], PLAIN, AssignModeParam::ReplaceAll, None))
        .expect("part.assign replace_all");
    assert_eq!(port(&e, f), Some(set_port.clone()));
    assert_eq!(bus_notices(&r), Vec::<String>::new());

    e.execute(&assign(vec![DEV_CPU], CPU, AssignModeParam::ReplaceAll, None))
        .expect("back to the CPU part");
    let r = e
        .execute(&set(KindName::Device, DEV_CPU.cast(), "parts", json!([])))
        .expect("fields.set parts []");
    assert_eq!(
        port(&e, f),
        Some(set_port.clone()),
        "removing the bus part removes no port"
    );
    assert_eq!(bus_notices(&r), Vec::<String>::new());

    e.execute(&assign(vec![DEV_CPU], CPU, AssignModeParam::Append, None))
        .expect("the CPU part again");
    e.execute(&set(KindName::Function, f.cast(), "bus_port", Value::Null))
        .expect("fields.set bus_port null");
    assert_eq!(port(&e, f), None);
    e.execute(&set(
        KindName::Device,
        DEV_CPU.cast(),
        "function_text",
        json!("Station"),
    ))
    .expect("an unrelated edit");
    assert_eq!(port(&e, f), None, "an unrelated edit seeds nothing");
    // A new bus part is a trigger again: the port-less function is a candidate.
    e.execute(&assign(vec![DEV_CPU], CPU_2P, AssignModeParam::ReplaceAll, None))
        .expect("another bus part");
    assert_eq!(port(&e, f), Some(dp_master()));
}

/// D16 trigger (ii) alone: a port a drafter removed from an older function stays removed when a new function joins;
/// the new function takes the port.
#[test]
fn a_joining_function_takes_a_removed_port_and_the_older_function_stays_port_less() {
    let mut e = engine(model());
    let (older, _) = place_port(&mut e, DEV_CPU);
    e.execute(&set(KindName::Function, older.cast(), "bus_port", Value::Null))
        .expect("fields.set bus_port null");
    let (newer, _) = place_port(&mut e, DEV_CPU);
    assert_eq!(port(&e, older), None);
    assert_eq!(port(&e, newer), Some(dp_master()));
}

// ---------------------------------------------------------------------------------------------------------
// BU-7i: a newer format and a passive port

/// BU-7i: a port of a newer format seeds nothing, with notice N5-c.
#[test]
fn bu7i_a_port_of_a_newer_format_is_not_applied() {
    let mut e = engine(model());
    let r = e
        .execute(&assign(vec![DEV_BOX], CPU_SX, AssignModeParam::Append, None))
        .expect("part.assign");
    assert_eq!(port(&e, FN_P1), None);
    assert_eq!(port(&e, FN_P2), None);
    assert_eq!(
        bus_notices(&r),
        ["bus port DP of part CPU-SX is of a newer format and was not applied"]
    );
}

/// BU-7i: a connector part's passive port with its termination on seeds `not_applicable` and `on`.
#[test]
fn bu7i_a_passive_connector_port_has_no_address_and_keeps_its_termination() {
    let mut e = engine(model());
    let r = e
        .execute(&assign(vec![DEV_PLUG], PLUG, AssignModeParam::Append, None))
        .expect("part.assign");
    let p = port(&e, FN_PLUG).expect("a port");
    assert_eq!(p.address, AddrValue::NotApplicable);
    assert_eq!(p.termination, Some(OnOff::On));
    assert_eq!(
        p,
        seeded(
            "DP",
            None,
            BusSystem::ProfibusDp,
            PortRole::Passive,
            Some(OnOff::On)
        )
    );
    assert_eq!(bus_notices(&r), Vec::<String>::new());
}

// ---------------------------------------------------------------------------------------------------------
// Undo and determinism

/// N5's writes are the command's: one undo step removes the port with the assignment, redo restores it, and two runs
/// of the same commands write the same model.
#[test]
fn the_seeded_port_undoes_with_its_command_and_replays_identically() {
    let run = || {
        let mut e = engine(model());
        e.execute(&assign(vec![DEV_BOX], CPU, AssignModeParam::Append, None))
            .expect("part.assign");
        e
    };
    let mut e = run();
    assert_eq!(lines(e.model()), lines(run().model()), "deterministic");
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(port(&e, FN_P1), None);
    assert!(row(&e, DEV_BOX).parts.is_empty());
    e.redo(&ew_commands::Origin::Gui).expect("redo");
    assert_eq!(port(&e, FN_P1), Some(dp_master()));
}
