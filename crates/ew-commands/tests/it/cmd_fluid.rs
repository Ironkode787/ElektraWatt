// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid commands: the refusals of `connection.draw` at fluid ports,
//! `valve.set_occupancy`, `fluid.port.close`, `fields.set` path `line_kind`, normaliser N-FL-4
//! `drives_link` and `device.link`'s refusal.
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
    fluid_model_with(Vec::new())
}

/// [`fluid_model`] with the records `more`.
fn fluid_model_with(more: Vec<Record>) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    records.extend(more);
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

/// T1: a fluid line to an electrical CP; T2: a wire to a fluid port; T3: a hydraulic port with a pneumatic one.
#[test]
fn the_fluid_refusals_t1_to_t3_are_in_words() {
    let mut e = engine(fluid_model());
    let cases = [
        (
            "T1",
            draw(cp_param(FN_Q1, 1), cp_param(FN_102G1, 0), fluid()),
            "a fluid line joins fluid ports only; -102G1:L is not a fluid port",
        ),
        (
            "T2",
            draw(cp_param(FN_102G1, 0), cp_param(FN_Q1, 1), ConnectionKind::Wire),
            "-?Q71:2 is a fluid port; only a fluid line joins it",
        ),
        (
            "T3",
            draw(cp_param(FN_Q1, 0), cp_param(FN_Q3, 0), fluid()),
            "a fluid line does not join a hydraulic port to a pneumatic or vacuum port: -?Q71:1, -?Q73:1",
        ),
    ];
    for (case, cmd, words) in cases {
        assert_eq!(refusal(&mut e, &cmd), words, "{case}");
    }
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
        Err(CmdError::Model(ew_model::ModelError::Invariant { id, .. }, _)) => {
            assert_eq!(id, ew_model::FLUID_LINES)
        }
        other => panic!("expected EW-INV-024, got {other:?}"),
    }
}

const JF: Id<ew_schema::Junction> = id(0x7700);
const LINE_1: Id<Connection> = id(0x7701);
const LINE_2: Id<Connection> = id(0x7702);

/// [`fluid_model`] with -Q71:1 and -Q72:1 joined by two fluid lines on page 102 through the junction `JF`.
fn fluid_net_model() -> Model {
    let line = |id, a, b| {
        let mut c = connection(id, Some(PAGE_102), a, b);
        c.kind = fluid();
        Record::Connection(c)
    };
    fluid_model_with(vec![
        line(LINE_1, ew_schema::Endpoint::Cp(FN_Q1, 0), at_junction(JF)),
        line(LINE_2, at_junction(JF), ew_schema::Endpoint::Cp(FN_Q2, 0)),
        Record::Junction(junction(JF, PAGE_102, vec![LINE_1, LINE_2])),
    ])
}

/// T1 and T2 in words at a junction end and at a point on a connection, and T3 across a junction; no record
/// ID reaches the user.
#[test]
fn the_fluid_refusals_name_junction_and_connection_ends_in_words() {
    let mut e = engine(fluid_net_model());
    let on = |connection| EndpointParam::OnConnection {
        connection,
        at: ew_units::Pt2::new(ew_units::Nm(0), ew_units::Nm(0)),
    };
    let wires = EndpointParam::Junction { junction: J5 };
    let pipes = EndpointParam::Junction { junction: JF };
    let texts = [
        refusal(&mut e, &draw(cp_param(FN_Q3, 1), wires, fluid())),
        refusal(&mut e, &draw(cp_param(FN_Q3, 1), on(CON_W22), fluid())),
        refusal(
            &mut e,
            &draw(cp_param(FN_102G1, 0), pipes.clone(), ConnectionKind::Wire),
        ),
        refusal(
            &mut e,
            &draw(cp_param(FN_102G1, 0), on(LINE_1), ConnectionKind::Wire),
        ),
    ];
    assert_eq!(
        texts,
        [
            "a fluid line joins fluid ports only; the junction at x 0, y 0 mm on page 102 joins wires",
            "a fluid line joins fluid ports only; wire 102.1 is no fluid line",
            "the junction at x 0, y 0 mm on page 102 joins fluid lines; only a fluid line joins it",
            "only a fluid line branches from the fluid line on page 102"
        ]
    );
    assert_eq!(
        refusal(&mut e, &draw(cp_param(FN_Q3, 1), pipes, fluid())),
        "a fluid line does not join a hydraulic port to a pneumatic or vacuum port: -?Q73:2, -?Q71:1"
    );
}

/// T3's walk over a ring of three junctions ends: a hydraulic port drawn to the ring, which a pneumatic
/// port feeds, is refused in words; a pneumatic one is drawn.
#[test]
fn t3_walks_a_ring_of_three_junctions_to_its_end() {
    let j: Vec<Id<ew_schema::Junction>> = (0..3).map(|n| id(0x7710 + n)).collect();
    let l: Vec<Id<Connection>> = (0..4).map(|n| id(0x7720 + n)).collect();
    let line = |id, a, b| {
        let mut c = connection(id, Some(PAGE_102), a, b);
        c.kind = fluid();
        Record::Connection(c)
    };
    let mut e = engine(fluid_model_with(vec![
        line(l[0], ew_schema::Endpoint::Cp(FN_Q1, 0), at_junction(j[0])),
        line(l[1], at_junction(j[0]), at_junction(j[1])),
        line(l[2], at_junction(j[1]), at_junction(j[2])),
        line(l[3], at_junction(j[2]), at_junction(j[0])),
        Record::Junction(junction(j[0], PAGE_102, vec![l[0], l[1], l[3]])),
        Record::Junction(junction(j[1], PAGE_102, vec![l[1], l[2]])),
        Record::Junction(junction(j[2], PAGE_102, vec![l[2], l[3]])),
    ]));
    let ring = EndpointParam::Junction { junction: j[1] };
    assert_eq!(
        refusal(&mut e, &draw(cp_param(FN_Q3, 1), ring.clone(), fluid())),
        "a fluid line does not join a hydraulic port to a pneumatic or vacuum port: -?Q73:2, -?Q71:1"
    );
    e.execute(&draw(cp_param(FN_Q2, 1), ring, fluid()))
        .expect("a pneumatic line to the ring");
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
    module_model_with(Vec::new())
}

/// [`module_model`] with the library documents `extra`.
fn module_model_with(extra: Vec<LibDocument>) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut lib = library();
    lib.extend(extra);
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

/// "blank" with another part on a plate swaps its part and its template functions, the old part's going and
/// the new part's coming, in one undo step; the same part again changes nothing.
#[test]
fn a_plate_given_another_part_takes_that_parts_template_functions() {
    const PLATE_A: Id<Part> = id(0x7504);
    const PLATE_B: Id<Part> = id(0x7505);
    let plate_part = |part_id: Id<Part>, number: &str, items: &[ew_ids::Uuid]| {
        let mut copy = part(part_id, number);
        copy.item.function_template = items
            .iter()
            .enumerate()
            .map(|(i, item)| ew_schema::TemplateFn {
                fndef: lib_ref_to(LIBRARY_VALVE, *item),
                main: i == 0,
                cps: vec![],
                extra: ew_schema::Extra::new(),
            })
            .collect();
        LibDocument::Part(copy)
    };
    let mut e = engine(module_model_with(vec![
        LibDocument::Fndef(valve_52_fndef()),
        LibDocument::Fndef(solenoid_fndef()),
        plate_part(PLATE_A, "PLATE-A", &[ITEM_VALVE_52]),
        plate_part(PLATE_B, "PLATE-B", &[ITEM_SOLENOID, ITEM_VALVE_52]),
    ]));
    let functions = |e: &Engine, plate: Id<Device>| {
        let mut out: Vec<(Id<Function>, Id<FnDef>)> = e
            .model()
            .table::<Function>()
            .iter()
            .filter(|(_, f)| f.device == Some(plate))
            .map(|(id, f)| (*id, f.def))
            .collect();
        out.sort();
        out
    };
    let body: Id<FnDef> = ew_ids::v5::lib_copy_id(LIBRARY_VALVE, ITEM_VALVE_52);
    let coil: Id<FnDef> = ew_ids::v5::lib_copy_id(LIBRARY_VALVE, ITEM_SOLENOID);
    e.execute(&occupy(2, OccupancyParam::Blank { part: Some(PLATE_A) }))
        .expect("a plate");
    let plate = child(&e, 2).expect("a plate at 2").id;
    let fid = |i| ew_ids::v5::template_function_id(plate, i);
    let a = vec![(fid(0), body)];
    assert_eq!(functions(&e, plate), a);
    e.execute(&occupy(2, OccupancyParam::Blank { part: Some(PLATE_B) }))
        .expect("another part");
    assert_eq!(row::<Device>(&e, plate).unwrap().parts[0].part.copy, PLATE_B);
    let mut b = vec![(fid(0), coil), (fid(1), body)];
    b.sort();
    assert_eq!(functions(&e, plate), b);
    e.undo(&Origin::Gui).expect("undo the part change");
    assert_eq!(row::<Device>(&e, plate).unwrap().parts[0].part.copy, PLATE_A);
    assert_eq!(functions(&e, plate), a);
    e.execute(&occupy(2, OccupancyParam::Blank { part: Some(PLATE_A) }))
        .expect("the same part");
    assert_eq!(functions(&e, plate), a);
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
    // Each is one undo step.
    let closure = |e: &Engine| {
        row::<Function>(e, FN_Q1)
            .unwrap()
            .cp_overrides
            .iter()
            .find(|o| o.idx == 1)
            .and_then(|o| o.closure.as_ref().map(|c| c.kind.clone()))
    };
    e.undo(&Origin::Gui).expect("undo the opening");
    assert_eq!(closure(&e), Some(ClosureKind::Silencer));
    e.undo(&Origin::Gui).expect("undo the closing");
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
// N-FL-4 drives_link (D15) and device.link (T4)

const VALVE_DEF: Id<FnDef> = id(0x7600);
const CYL_DEF: Id<FnDef> = id(0x7601);
const TERMINAL: Id<Device> = id(0x7610);
const VALVE: Id<Device> = id(0x7611);
const CYL_A: Id<Device> = id(0x7612);
const CYL_B: Id<Device> = id(0x7613);
const FN_VALVE: Id<Function> = id(0x7621);
const FN_CYL_A: Id<Function> = id(0x7622);
const FN_CYL_B: Id<Function> = id(0x7623);

/// A pneumatic FnDef copy of `contact` kind whose CPs carry `roles`.
fn role_fndef(def: Id<FnDef>, contact: ew_schema::ContactKind, roles: &[&str]) -> LibCopy<FnDef> {
    let designations: Vec<String> = (1..=roles.len()).map(|n| n.to_string()).collect();
    let names: Vec<&str> = designations.iter().map(String::as_str).collect();
    let mut copy = fluid_fndef_n(def, &names);
    copy.item.contact_kind = contact;
    let cps = copy
        .item
        .cps
        .iter()
        .cloned()
        .zip(roles)
        .map(|(mut c, r)| {
            c.port_role = serde_json::from_value(json!(r)).ok();
            c
        })
        .collect();
    copy.item.cps = KeyedList::from_vec(cps).expect("distinct CP indexes");
    copy
}

/// A pneumatic FnDef copy with the given designations.
fn fluid_fndef_n(def: Id<FnDef>, designations: &[&str]) -> LibCopy<FnDef> {
    let mut copy = fndef(def, designations);
    let cps = copy
        .item
        .cps
        .iter()
        .cloned()
        .map(|mut c| {
            c.discipline = Discipline::Pneumatic;
            c
        })
        .collect();
    copy.item.cps = KeyedList::from_vec(cps).expect("distinct CP indexes");
    copy
}

/// A valve -Q61 (supply 1, working 2 and 4) on a terminal -U61 and two cylinders -Q62 and -Q63 (consumer A, B).
fn actuator_model() -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(LibDocument::Fndef(role_fndef(
        VALVE_DEF,
        ew_schema::ContactKind::Valve,
        &["supply1", "working2", "exhaust3", "working4", "exhaust5"],
    )));
    lib.push(LibDocument::Fndef(role_fndef(
        CYL_DEF,
        ew_schema::ContactKind::BlackBox,
        &["consumer", "consumer"],
    )));
    records.push(Record::Device(device(TERMINAL, "U", 61)));
    let mut valve = device(VALVE, "Q", 61);
    valve.parent = Some(TERMINAL);
    records.push(Record::Device(valve));
    records.push(Record::Function(function(
        FN_VALVE,
        VALVE,
        VALVE_DEF,
        FnRole::Main,
    )));
    for (d, f, n) in [(CYL_A, FN_CYL_A, 62), (CYL_B, FN_CYL_B, 63)] {
        records.push(Record::Device(device(d, "Q", n)));
        records.push(Record::Function(function(f, d, CYL_DEF, FnRole::Main)));
    }
    Model::from_records(p, records, lib).expect("the actuators")
}

fn drives_links(e: &Engine) -> Vec<(Id<Device>, Id<Device>)> {
    e.model()
        .table::<ew_schema::DeviceLink>()
        .iter()
        .map(|(_, l)| (l.from, l.to))
        .collect()
}

/// FL-10: the first line from a working port to a cylinder adds the `drives` link (T22); a second line adds none.
#[test]
fn a_working_line_to_one_cylinder_links_the_valve_once() {
    let mut e = engine(actuator_model());
    let r = e
        .execute(&draw(cp_param(FN_VALVE, 1), cp_param(FN_CYL_A, 0), fluid()))
        .expect("F1");
    assert_eq!(r.outcome.notices, ["-?Q61 now drives -?Q62"]);
    assert_eq!(drives_links(&e), [(VALVE, CYL_A)]);
    let r = e
        .execute(&draw(cp_param(FN_VALVE, 3), cp_param(FN_CYL_A, 1), fluid()))
        .expect("F2");
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    assert_eq!(drives_links(&e), [(VALVE, CYL_A)]);
    // A line from the supply port reaches no consumer through a working port: nothing.
    let mut e = engine(actuator_model());
    e.execute(&draw(cp_param(FN_VALVE, 0), cp_param(FN_CYL_B, 0), fluid()))
        .expect("a supply line");
    assert!(drives_links(&e).is_empty());
}

/// FL-10: a working line that reaches two cylinders adds no link and says so (T23).
#[test]
fn a_working_line_to_two_cylinders_links_nothing() {
    let mut e = engine(actuator_model());
    e.execute(&draw(cp_param(FN_CYL_A, 0), cp_param(FN_CYL_B, 0), fluid()))
        .expect("a line between the cylinders");
    let r = e
        .execute(&draw(cp_param(FN_VALVE, 1), cp_param(FN_CYL_A, 0), fluid()))
        .expect("a line");
    assert!(drives_links(&e).is_empty());
    assert_eq!(
        r.outcome.notices,
        ["the working-port lines of -?Q61 reach -?Q62 and -?Q63; link the driven device by hand"]
    );
}

/// D15: N-FL-4 runs after `bus_ports` (doc 06 §3.2 row 16).
#[test]
fn drives_link_runs_after_bus_ports() {
    let order = ew_commands::normalise::ORDER;
    let at = |n: &str| {
        order
            .iter()
            .position(|o| *o == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    assert_eq!(at("drives_link"), at("bus_ports") + 1, "{order:?}");
}

/// T4: a valve does not drive the terminal that holds it.
#[test]
fn a_valve_cannot_drive_its_terminal() {
    let mut e = engine(actuator_model());
    let cmd = ew_commands::commands::LinkDevices {
        from: VALVE,
        to: TERMINAL,
        kind: ew_schema::DeviceLinkKind::Drives,
    };
    assert_eq!(
        refusal_of(&mut e, &cmd),
        "-?Q61 cannot drive -?U61, which holds it"
    );
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
