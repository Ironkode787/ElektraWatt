// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid commands of WP-S1.20: the refusals of `connection.draw` at fluid ports (§5 T1 to T3, D11),
//! `valve.set_occupancy` (D9, FL-8), `fluid.port.close` (D10) and `fields.set` path `line_kind` (D18).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    ClosePort, ClosureKindParam, ClosureParam, Connect, EndpointParam, KindName, OccupancyParam, SetFields,
    SetValveOccupancy, ValveStateParam,
};
use ew_commands::{CmdError, Engine, Origin};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    ClosureKind, Connection, ConnectionKind, Device, DeviceKind, DeviceState, Discipline, FluidLine, FnDef,
    FnRole, Function, KeyedList, LibCopy, LibDocument, LineKind, Part, Record,
};
use serde_json::json;

const PNEU: Id<FnDef> = id(0x7100);
const HYD: Id<FnDef> = id(0x7101);
const DEV_Q1: Id<Device> = id(0x7200);
const DEV_Q2: Id<Device> = id(0x7201);
const DEV_Q3: Id<Device> = id(0x7202);
const FN_Q1: Id<Function> = id(0x7300);
const FN_Q2: Id<Function> = id(0x7301);
const FN_Q3: Id<Function> = id(0x7302);

/// A FnDef copy with CPs "1" and "2" of `discipline`.
fn fluid_fndef(def: Id<FnDef>, discipline: Discipline) -> LibCopy<FnDef> {
    let mut copy = fndef(def, &["1", "2"]);
    let cps = copy
        .item
        .cps
        .iter()
        .cloned()
        .map(|mut c| {
            c.discipline = discipline.clone();
            c
        })
        .collect();
    copy.item.cps = KeyedList::from_vec(cps).expect("distinct CP indexes");
    copy
}

/// The mini golden project with two pneumatic valves -Q71 and -Q72 and a hydraulic one, -Q73.
fn fluid_model() -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(LibDocument::Fndef(fluid_fndef(PNEU, Discipline::Pneumatic)));
    lib.push(LibDocument::Fndef(fluid_fndef(HYD, Discipline::Hydraulic)));
    for (dev, f, def, n) in [
        (DEV_Q1, FN_Q1, PNEU, 71),
        (DEV_Q2, FN_Q2, PNEU, 72),
        (DEV_Q3, FN_Q3, HYD, 73),
    ] {
        records.push(Record::Device(device(dev, "Q", n)));
        records.push(Record::Function(function(f, dev, def, FnRole::Main)));
    }
    Model::from_records(p, records, lib).expect("the valves")
}

fn cp_param(f: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function: f, idx }
}

fn draw(a: EndpointParam, b: EndpointParam, kind: ConnectionKind) -> Connect {
    Connect {
        a,
        b,
        page: None,
        route: vec![],
        kind,
    }
}

fn fluid() -> ConnectionKind {
    ConnectionKind::Fluid(FluidLine::default())
}

fn refusal(e: &mut Engine, cmd: &Connect) -> String {
    match e.execute(cmd) {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_fluid_line_between_two_pneumatic_ports_is_drawn() {
    let mut e = engine(fluid_model());
    e.execute(&draw(cp_param(FN_Q1, 1), cp_param(FN_Q2, 0), fluid()))
        .expect("a fluid line");
}

/// T1: a fluid line to an electrical CP.
#[test]
fn a_fluid_line_to_an_electrical_cp_is_refused_in_words() {
    let mut e = engine(fluid_model());
    let cmd = draw(cp_param(FN_Q1, 1), cp_param(FN_102G1, 0), fluid());
    assert_eq!(
        refusal(&mut e, &cmd),
        "a fluid line joins fluid ports only; -102G1:L is not a fluid port"
    );
}

/// T2: a wire to a fluid port.
#[test]
fn a_wire_to_a_fluid_port_is_refused_in_words() {
    let mut e = engine(fluid_model());
    let cmd = draw(cp_param(FN_102G1, 0), cp_param(FN_Q1, 1), ConnectionKind::Wire);
    assert_eq!(
        refusal(&mut e, &cmd),
        "-?Q71:2 is a fluid port; only a fluid line joins it"
    );
}

/// FL-3: a pasted fragment whose wire ends at a fluid port fails EW-INV-024 (the paste path has no refusal of its
/// own; the structural invariant guards it).
#[test]
fn a_pasted_wire_to_a_fluid_port_fails_the_fluid_invariant() {
    let mut e = engine(fluid_model());
    let fragment = serde_json::json!({"format": "0.1", "project": uuid(0xd200).to_string(), "grid": [7_937_500, 3],
    "page_kind": "multi_line", "anchor": [0, 0],
    "records": [
        {"$k": "device", "id": uuid(0x7400).to_string(), "tag": {"letter": "Q", "counter": 81}},
        {"$k": "function", "id": uuid(0x7401).to_string(), "device": uuid(0x7400).to_string(),
            "def": PNEU.to_string()},
        {"$k": "device", "id": uuid(0x7402).to_string(), "tag": {"letter": "Q", "counter": 82}},
        {"$k": "function", "id": uuid(0x7403).to_string(), "device": uuid(0x7402).to_string(),
            "def": FNDEF_CONTACT.to_string()},
        {"$k": "connection", "id": uuid(0x7404).to_string(), "a": {"cp": [uuid(0x7401).to_string(), 0]},
            "b": {"cp": [uuid(0x7403).to_string(), 0]}}],
    "copies": [
        serde_json::to_value(LibDocument::Fndef(fluid_fndef(PNEU, Discipline::Pneumatic))).unwrap(),
        serde_json::to_value(e.model().lib().fndefs().get(&FNDEF_CONTACT).map(|c| LibDocument::Fndef((**c).clone()))).unwrap(),
    ]});
    let cmd = ew_commands::commands::PasteFragment {
        page: PAGE_101,
        at: ew_units::Pt2::new(ew_units::Nm(0), ew_units::Nm(0)),
        fragment: serde_json::from_value(fragment).expect("a fragment"),
        adapt: Vec::new(),
    };
    match e.execute(&cmd) {
        Err(CmdError::Model(ew_model::ModelError::Invariant { id, .. })) => {
            assert_eq!(id, ew_model::FLUID_LINES)
        }
        other => panic!("expected EW-INV-024, got {other:?}"),
    }
}

/// T3: a hydraulic port with a pneumatic one.
#[test]
fn a_hydraulic_to_pneumatic_line_is_refused_in_words() {
    let mut e = engine(fluid_model());
    let cmd = draw(cp_param(FN_Q1, 0), cp_param(FN_Q3, 0), fluid());
    assert_eq!(
        refusal(&mut e, &cmd),
        "a fluid line does not join a hydraulic port to a pneumatic or vacuum port: -?Q71:1, -?Q73:1"
    );
}

// ---------------------------------------------------------------------------------------------------------
// valve.set_occupancy (D9)

const MODULE: Id<Device> = id(0x7500);
const PART_MODULE: Id<Part> = id(0x7501);
const PART_VALVE: Id<Part> = id(0x7502);
const PART_OTHER: Id<Part> = id(0x7503);

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> Option<std::sync::Arc<T>> {
    e.model().table::<T>().get(&id).cloned()
}

/// [`fluid_model`] with a module -U91 of four valve positions and two valve parts.
fn module_model() -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(LibDocument::Fndef(fluid_fndef(PNEU, Discipline::Pneumatic)));
    let mut module_part = part(PART_MODULE, "MODULE-4");
    module_part.item.sections.plc =
        Some(serde_json::from_value(json!({"role": "module", "positions": {"count": 4}})).unwrap());
    lib.push(LibDocument::Part(module_part));
    lib.push(LibDocument::Part(part(PART_VALVE, "VALVE-52")));
    lib.push(LibDocument::Part(part(PART_OTHER, "VALVE-53")));
    let mut module = device(MODULE, "U", 91);
    module.parts =
        vec![serde_json::from_value(json!({"part": {"copy": PART_MODULE.to_string()}, "qty": "1"})).unwrap()];
    records.push(Record::Device(module));
    Model::from_records(p, records, lib).expect("the module")
}

fn occupy(position: u32, occupancy: OccupancyParam) -> SetValveOccupancy {
    SetValveOccupancy {
        module: MODULE,
        position,
        occupancy,
        copies: Vec::new(),
    }
}

fn valve(state: ValveStateParam, part: Option<Id<Part>>) -> OccupancyParam {
    OccupancyParam::Valve { state, part }
}

/// The child of the module at `position`.
fn child(e: &Engine, position: u32) -> Option<std::sync::Arc<Device>> {
    e.model()
        .table::<Device>()
        .iter()
        .map(|(_, d)| d.clone())
        .find(|d| d.parent == Some(MODULE) && d.position == Some(position))
}

/// FL-8: T5 to T8 whole.
#[test]
fn occupancy_refusals_name_the_module_and_the_position() {
    let mut e = engine(module_model());
    let mut on = |cmd: SetValveOccupancy| match e.execute(&cmd) {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    };
    let mut not_module = occupy(1, OccupancyParam::Empty);
    not_module.module = DEV_102G1;
    assert_eq!(on(not_module), "-102G1 has no valve positions");
    assert_eq!(
        on(occupy(5, OccupancyParam::Empty)),
        "-?U91 has 4 valve positions; position 5 does not exist"
    );
    assert_eq!(
        on(occupy(2, valve(ValveStateParam::Used, None))),
        "position 2 of -?U91 holds no valve; name the valve part"
    );
    let mut e = engine(module_model());
    e.execute(&occupy(2, valve(ValveStateParam::Used, Some(PART_VALVE))))
        .expect("a valve");
    let name = match e.execute(&occupy(2, valve(ValveStateParam::Used, Some(PART_OTHER)))) {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert_eq!(
        name,
        "position 2 of -?U91 holds -?U91-Q2; assign the new part to -?U91-Q2 instead"
    );
}

/// FL-8: a valve on an empty position, spare on a used valve, a blank on a valve, empty on the plate; each one undo
/// step.
#[test]
fn occupancy_makes_and_changes_valves_and_plates() {
    let mut e = engine(module_model());
    e.execute(&occupy(3, valve(ValveStateParam::Used, Some(PART_VALVE))))
        .expect("a valve");
    let v = child(&e, 3).expect("a valve at 3");
    assert_eq!(v.parts[0].part.copy, PART_VALVE);
    assert_eq!(v.state, DeviceState::Used);
    assert_eq!((v.tag.letter.as_str(), v.tag.counter), ("Q", Some(3)));
    e.execute(&occupy(3, valve(ValveStateParam::Spare, None)))
        .expect("spare");
    assert_eq!(row::<Device>(&e, v.id).unwrap().state, DeviceState::Spare);
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(row::<Device>(&e, v.id).unwrap().state, DeviceState::Used);
    let r = e
        .execute(&occupy(3, OccupancyParam::Blank { part: None }))
        .expect("a blank");
    assert_eq!(r.outcome.notices, ["removed -?U91-Q3 from position 3 of -?U91"]);
    assert!(row::<Device>(&e, v.id).is_none());
    let plate = child(&e, 3).expect("a plate at 3");
    assert_eq!(plate.kind, Some(DeviceKind::BlankingPlate));
    assert!(plate.parts.is_empty());
    let r = e.execute(&occupy(3, OccupancyParam::Empty)).expect("empty");
    assert_eq!(r.outcome.notices, ["removed -?U91-Q3 from position 3 of -?U91"]);
    assert!(child(&e, 3).is_none());
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(child(&e, 3).map(|d| d.id), Some(plate.id));
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(child(&e, 3).map(|d| d.id), Some(v.id));
}

// ---------------------------------------------------------------------------------------------------------
// fluid.port.close (D10)

fn close(f: Id<Function>, idx: u16, kind: Option<ClosureKindParam>) -> ClosePort {
    ClosePort {
        function: f,
        idx,
        closure: kind.map(|kind| ClosureParam { kind, part: None }),
    }
}

/// T9 and T10 whole; a closed port reads its closure, and opening it drops the override.
#[test]
fn closing_a_fluid_port_sets_its_closure_and_opening_drops_it() {
    let mut e = engine(fluid_model());
    assert_eq!(
        refusal_of(&mut e, &close(FN_102G1, 0, Some(ClosureKindParam::Plug))),
        "-102G1:L is not a fluid port"
    );
    e.execute(&close(FN_Q1, 1, Some(ClosureKindParam::Silencer)))
        .expect("closed");
    let f = row::<Function>(&e, FN_Q1).unwrap();
    let o = f.cp_overrides.iter().find(|o| o.idx == 1).expect("an override");
    assert_eq!(
        o.closure.as_ref().map(|c| c.kind.clone()),
        Some(ClosureKind::Silencer)
    );
    e.execute(&close(FN_Q1, 1, None)).expect("opened");
    assert!(row::<Function>(&e, FN_Q1).unwrap().cp_overrides.is_empty());
    e.execute(&draw(cp_param(FN_Q1, 0), cp_param(FN_Q2, 0), fluid()))
        .expect("a line");
    assert_eq!(
        refusal_of(&mut e, &close(FN_Q1, 0, Some(ClosureKindParam::Plug))),
        "-?Q71:1 has a fluid line; delete the line before closing the port"
    );
}

fn refusal_of(e: &mut Engine, cmd: &dyn ew_commands::DynCommand) -> String {
    match e.execute(cmd) {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// fields.set line_kind (D18)

fn set_line_kind(c: Id<Connection>, kind: &str) -> SetFields {
    SetFields {
        kind: KindName::Connection,
        targets: vec![c.cast()],
        path: "line_kind".into(),
        value: json!(kind),
        copies: Vec::new(),
    }
}

/// D18: `line_kind` sets a fluid line's kind in one undo step; T11 on a wire.
#[test]
fn the_line_kind_of_a_fluid_line_is_set_and_undone() {
    let mut e = engine(fluid_model());
    let r = e
        .execute(&draw(cp_param(FN_Q1, 0), cp_param(FN_Q2, 0), fluid()))
        .expect("a line");
    let line: Id<Connection> = typed(r.outcome.primary.unwrap());
    e.execute(&set_line_kind(line, "pilot")).expect("pilot");
    let kind = |e: &Engine| match &row::<Connection>(e, line).unwrap().kind {
        ConnectionKind::Fluid(f) => f.line_kind.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(kind(&e), LineKind::Pilot);
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(kind(&e), LineKind::Working);
    let wire = e
        .execute(&draw(
            cp_param(FN_102G1, 0),
            cp_param(FN_102G1, 1),
            ConnectionKind::Wire,
        ))
        .expect("a wire");
    let wire: Id<Connection> = typed(wire.outcome.primary.unwrap());
    assert_eq!(
        refusal_of(&mut e, &set_line_kind(wire, "pilot")),
        "a wire is not a fluid line"
    );
}
