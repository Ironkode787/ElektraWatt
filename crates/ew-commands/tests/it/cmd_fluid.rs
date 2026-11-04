// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fluid commands of WP-S1.20: the refusals of `connection.draw` at fluid ports (§5 T1 to T3, D11).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{Connect, EndpointParam};
use ew_commands::{CmdError, Engine};
use ew_ids::Id;
use ew_model::Model;
use ew_schema::{
    ConnectionKind, Device, Discipline, FnDef, FnRole, FluidLine, Function, KeyedList, LibCopy, LibDocument,
    Record,
};

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
    for (dev, f, def, n) in [(DEV_Q1, FN_Q1, PNEU, 71), (DEV_Q2, FN_Q2, PNEU, 72), (DEV_Q3, FN_Q3, HYD, 73)] {
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
        Err(CmdError::Model(ew_model::ModelError::Invariant { id, .. })) => assert_eq!(id, ew_model::FLUID_LINES),
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
