// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device, representation, terminal and cable commands: `device.create`, `device.place`, `rep.move`,
//! `rep.transform`, `terminal.add`, `cable.create`, `conductor.add`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    AddConductors, AddTerminals, ConductorSpec, ConnectHint, CreateCable, CreateDevice, EndpointParam,
    MoveReps, PlaceSymbol, PlaceTarget, SetAnchorPolicy, TerminalSpec, TransformReps,
};
use ew_commands::{AnyCommand, CmdError, Engine, Receipt};
use ew_model::{Entity, Model};
use ew_schema::{
    AnchorPolicy, Cable, Conductor, Connection, Device, DeviceAspects, DeviceKind, Function, Kind, LibCopy,
    NumberSpec, PartRef, PartUse, Record, Representation, SymbolDef,
};
use ew_units::{Orient2, Pt2};

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn golden_with(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    f(&mut records);
    Model::from_records(p, records, library()).expect("valid model")
}

fn row<T: Entity<Marker = T>>(e: &Engine, id: ew_ids::Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn maybe_row<T: Entity<Marker = T>>(e: &Engine, id: ew_ids::Id<T>) -> Option<std::sync::Arc<T>> {
    e.model().table::<T>().get(&id).cloned()
}

fn created<T>(r: &Receipt, kind: Kind) -> Vec<ew_ids::Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| typed(*k))
        .collect()
}

fn primary<T>(r: &Receipt) -> ew_ids::Id<T> {
    typed(r.outcome.primary.expect("primary outcome"))
}

fn rejected<T: std::fmt::Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::Rejected { .. }) => {}
        other => panic!("expected a rejection, got {other:?}"),
    }
}

fn not_found<T: std::fmt::Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

fn create(letter: &str) -> CreateDevice {
    CreateDevice {
        letter: ident(letter),
        counter: None,
        kind: None,
        parts: vec![],
        function_text: Default::default(),
        structure: Default::default(),
        strip: None,
        main_fndef: None,
    }
}

fn new_device(letter: &str) -> PlaceTarget {
    PlaceTarget::NewDevice {
        letter: ident(letter),
        fndef: None,
        kind: None,
    }
}

fn place(page: ew_ids::Id<ew_schema::Page>, pos: Pt2, target: PlaceTarget) -> PlaceSymbol {
    PlaceSymbol {
        copies: Vec::new(),
        page,
        symbol: SYM_TWO,
        pos,
        orient: Orient2::R0,
        rep_type: None,
        variant: 0,
        target,
        connect: vec![],
    }
}

/// A device row as `device.create` writes it: no structure assignment.
fn expected_device(id: ew_ids::Id<Device>, letter: &str, counter: Option<u32>) -> Device {
    let mut d = device(id, letter, 0);
    d.tag.counter = counter;
    d.structure = Default::default();
    d
}

fn conductor_spec(core: &str) -> ConductorSpec {
    ConductorSpec {
        core: ident(core),
        colour: None,
        csa: None,
        shield: false,
        pair: None,
    }
}

fn terminal_spec(designation: &str, seq: Option<u32>) -> TerminalSpec {
    TerminalSpec {
        designation: ident(designation),
        seq,
        function_text: Default::default(),
        levels: vec![],
        rail_bonded: false,
    }
}

fn kinds(model: &Model, kind: Kind) -> Vec<(ew_model::AnyKey, String)> {
    lines(model).into_iter().filter(|(k, _)| k.0 == kind).collect()
}

fn add_terminals(strip: ew_ids::Id<Device>, terminals: Vec<TerminalSpec>) -> AddTerminals {
    AddTerminals {
        strip,
        fndef: FNDEF_TERMINAL,
        terminals,
    }
}

// ---------------------------------------------------------------------------------------------------------
// device.create

#[test]
fn create_device_without_function_stays_unnumbered() {
    let mut e = engine(golden_model());
    let mut cmd = create("F");
    cmd.kind = Some(DeviceKind::Psu);
    cmd.function_text = ew_mltext::MlText::neutral("Protection");
    cmd.parts = vec![PartUse {
        part: PartRef::new(PART_MCB),
        qty: dec3("1"),
        extra: Default::default(),
    }];
    let r = e.execute(&cmd).expect("device.create");
    let id: ew_ids::Id<Device> = primary(&r);
    assert_eq!(r.created.len(), 1);
    let mut expected = expected_device(id, "F", None);
    expected.kind = Some(DeviceKind::Psu);
    expected.function_text = ew_mltext::MlText::neutral("Protection");
    expected.parts = cmd.parts.clone();
    assert_eq!(*row(&e, id), expected);
    assert!(
        e.model()
            .table::<Function>()
            .iter()
            .all(|(_, f)| f.device != Some(id))
    );
}

#[test]
fn create_device_with_main_function_and_explicit_counter() {
    let mut e = engine(golden_model());
    let mut cmd = create("K");
    cmd.counter = Some(9);
    cmd.main_fndef = Some(FNDEF_MCB);
    let r = e.execute(&cmd).unwrap();
    let id: ew_ids::Id<Device> = primary(&r);
    let fns: Vec<ew_ids::Id<Function>> = created(&r, Kind::Function);
    assert_eq!(fns.len(), 1);
    assert_eq!(*row(&e, id), expected_device(id, "K", Some(9)));
    assert_eq!(
        *row(&e, fns[0]),
        function(fns[0], id, FNDEF_MCB, ew_schema::FnRole::Main)
    );
}

#[test]
fn create_strip_device_carries_the_strip_aspect() {
    let mut e = engine(golden_model());
    let mut cmd = create("X");
    cmd.counter = Some(2000);
    cmd.strip = strip(DEV_X1031, "X", 1).aspects.strip;
    cmd.main_fndef = Some(FNDEF_STRIP);
    let r = e.execute(&cmd).unwrap();
    let id: ew_ids::Id<Device> = primary(&r);
    let mut expected = expected_device(id, "X", Some(2000));
    expected.aspects = DeviceAspects {
        strip: cmd.strip.clone(),
        ..Default::default()
    };
    assert_eq!(*row(&e, id), expected);
}

#[test]
fn create_device_needs_existing_parts_and_fndef() {
    let mut e = engine(golden_model());
    let mut cmd = create("F");
    cmd.main_fndef = Some(id(0xdead));
    not_found(e.execute(&cmd));
    let mut cmd = create("F");
    cmd.parts = vec![PartUse {
        part: PartRef::new(id(0xbeef)),
        qty: dec3("1"),
        extra: Default::default(),
    }];
    not_found(e.execute(&cmd));
    assert_eq!(e.model().rev().0, 0);
}

// ---------------------------------------------------------------------------------------------------------
// device.place and online numbering

#[test]
fn place_new_device_creates_device_function_and_representation() {
    let mut e = engine(golden_model());
    let target = PlaceTarget::NewDevice {
        letter: ident("F"),
        fndef: None,
        kind: Some(DeviceKind::Psu),
    };
    let mut cmd = place(PAGE_102, pt(3 * G, 4 * G), target);
    cmd.orient = Orient2::R90;
    cmd.variant = 2;
    let r = e.execute(&cmd).expect("device.place");
    let rep_id: ew_ids::Id<Representation> = primary(&r);
    let devs: Vec<ew_ids::Id<Device>> = created(&r, Kind::Device);
    let fns: Vec<ew_ids::Id<Function>> = created(&r, Kind::Function);
    assert_eq!((devs.len(), fns.len(), r.created.len()), (1, 1, 3));
    let mut expected_dev = expected_device(devs[0], "F", Some(1));
    expected_dev.kind = Some(DeviceKind::Psu);
    assert_eq!(*row(&e, devs[0]), expected_dev);
    assert_eq!(
        *row(&e, fns[0]),
        function(fns[0], devs[0], fndef_two(), ew_schema::FnRole::Main)
    );
    let mut expected_rep = rep(rep_id, PAGE_102, vec![fns[0]]);
    expected_rep.symbol = SYM_TWO;
    expected_rep.pos = pt(3 * G, 4 * G);
    expected_rep.orient = Orient2::R90;
    expected_rep.variant = 2;
    assert_eq!(*row(&e, rep_id), expected_rep);
}

#[test]
fn online_numbering_fills_the_page_scope() {
    let mut e = engine(golden_model());
    let mut counters = Vec::new();
    for _ in 0..2 {
        let r = e
            .execute(&place(PAGE_102, pt(0, 0), new_device("F")))
            .expect("device.place");
        let d: ew_ids::Id<Device> = created(&r, Kind::Device)[0];
        counters.push(row(&e, d).tag.counter);
    }
    // F2 exists on the page: the first free counter is 1, then 3.
    assert_eq!(counters, vec![Some(1), Some(3)]);
    // Another letter has its own counters.
    let r = e.execute(&place(PAGE_102, pt(0, 0), new_device("K"))).unwrap();
    let d: ew_ids::Id<Device> = created(&r, Kind::Device)[0];
    assert_eq!(row(&e, d).tag.counter, Some(1));
}

#[test]
fn unplaced_device_gets_its_counter_when_its_main_function_is_placed() {
    let mut e = engine(golden_model());
    let mut cmd = create("F");
    cmd.main_fndef = Some(fndef_two());
    let r = e.execute(&cmd).unwrap();
    let dev: ew_ids::Id<Device> = primary(&r);
    let f: ew_ids::Id<Function> = created(&r, Kind::Function)[0];
    assert_eq!(row(&e, dev).tag.counter, None, "unplaced");
    e.execute(&place(
        PAGE_102,
        pt(0, 0),
        PlaceTarget::ExistingFunction { function: f },
    ))
    .unwrap();
    assert_eq!(row(&e, dev).tag.counter, Some(1));
}

#[test]
fn explicit_counter_is_kept_when_placed() {
    let mut e = engine(golden_model());
    let mut cmd = create("F");
    cmd.counter = Some(7);
    cmd.main_fndef = Some(fndef_two());
    let r = e.execute(&cmd).unwrap();
    let dev: ew_ids::Id<Device> = primary(&r);
    let f: ew_ids::Id<Function> = created(&r, Kind::Function)[0];
    e.execute(&place(
        PAGE_102,
        pt(0, 0),
        PlaceTarget::ExistingFunction { function: f },
    ))
    .unwrap();
    assert_eq!(row(&e, dev).tag.counter, Some(7));
}

#[test]
fn place_existing_function_adds_a_representation_and_keeps_the_device() {
    let mut e = engine(golden_model());
    let before = row(&e, DEV_102F2);
    let r = e
        .execute(&place(
            PAGE_101,
            pt(G, 0),
            PlaceTarget::ExistingFunction { function: FN_102F2 },
        ))
        .unwrap();
    let rep_id: ew_ids::Id<Representation> = primary(&r);
    assert_eq!(row(&e, rep_id).functions, vec![FN_102F2]);
    assert_eq!(row(&e, rep_id).page, PAGE_101);
    assert_eq!(r.created, vec![(Kind::Representation, rep_id.cast())]);
    assert_eq!(
        row(&e, DEV_102F2),
        before,
        "the main representation stays on page 102"
    );
}

#[test]
fn place_rejects_an_interruption_point_and_bad_references() {
    let mut e = engine(golden_model());
    rejected(e.execute(&place(
        PAGE_101,
        pt(0, 0),
        PlaceTarget::ExistingFunction { function: FN_IP_101 },
    )));
    not_found(e.execute(&place(id(0xaaa), pt(0, 0), new_device("F"))));
    let mut cmd = place(PAGE_101, pt(0, 0), new_device("F"));
    cmd.symbol = id(0xbbb);
    not_found(e.execute(&cmd));
    // The symbol SYM names a FnDef that is not embedded: pass `fndef`.
    let mut cmd = place(PAGE_101, pt(0, 0), new_device("F"));
    cmd.symbol = SYM;
    rejected(e.execute(&cmd));
    cmd.target = PlaceTarget::NewDevice {
        letter: ident("F"),
        fndef: Some(FNDEF_MCB),
        kind: None,
    };
    e.execute(&cmd).expect("an explicit fndef places it");
}

#[test]
fn place_join_device_adds_an_auxiliary_function() {
    let mut e = engine(golden_model());
    let r = e
        .execute(&place(
            PAGE_102,
            pt(0, 0),
            PlaceTarget::JoinDevice {
                device: DEV_102F2,
                fndef: None,
                role: ew_schema::FnRole::Aux,
            },
        ))
        .unwrap();
    let f: ew_ids::Id<Function> = created(&r, Kind::Function)[0];
    assert_eq!(
        *row(&e, f),
        function(f, DEV_102F2, fndef_two(), ew_schema::FnRole::Aux)
    );
    assert!(created::<Device>(&r, Kind::Device).is_empty());
    // A second main function is refused by EW-INV-002.
    let err = e
        .execute(&place(
            PAGE_102,
            pt(0, 0),
            PlaceTarget::JoinDevice {
                device: DEV_102F2,
                fndef: None,
                role: ew_schema::FnRole::Main,
            },
        ))
        .unwrap_err();
    assert!(matches!(err, CmdError::Model(_)), "{err:?}");
}

#[test]
fn place_hints_create_wires_and_extend_junctions() {
    let mut e = engine(golden_model());
    let mut cmd = place(PAGE_102, pt(0, 0), new_device("F"));
    cmd.connect = vec![
        ConnectHint {
            cp: 1,
            to: EndpointParam::Cp {
                function: FN_102G1,
                idx: 1,
            },
        },
        ConnectHint {
            cp: 0,
            to: EndpointParam::Junction { junction: J5 },
        },
    ];
    let r = e.execute(&cmd).unwrap();
    let f: ew_ids::Id<Function> = created(&r, Kind::Function)[0];
    let wires: Vec<ew_ids::Id<Connection>> = created(&r, Kind::Connection);
    assert_eq!(wires.len(), 2);
    assert_eq!(
        *row(&e, wires[0]),
        connection(wires[0], Some(PAGE_102), cp(f, 1), cp(FN_102G1, 1))
    );
    assert_eq!(
        *row(&e, wires[1]),
        connection(wires[1], Some(PAGE_102), cp(f, 0), at_junction(J5))
    );
    assert_eq!(
        row(&e, J5).sequence,
        vec![CON_J5_IN, CON_W22, CON_W27, wires[1]],
        "the wire is appended to the junction sequence"
    );
}

#[test]
fn place_hints_reject_bad_cps_and_connections_and_drop_missing_targets() {
    let mut e = engine(golden_model());
    let mut cmd = place(PAGE_102, pt(0, 0), new_device("F"));
    cmd.connect = vec![ConnectHint {
        cp: 5,
        to: EndpointParam::Cp {
            function: FN_102G1,
            idx: 1,
        },
    }];
    rejected(e.execute(&cmd));
    // a page-less connection cannot take a junction; the split point must be on the grid
    for (connection, at) in [(CON_BR1, pt(0, 0)), (CON_W11B, pt(1, 0))] {
        cmd.connect = vec![ConnectHint {
            cp: 0,
            to: EndpointParam::OnConnection { connection, at },
        }];
        rejected(e.execute(&cmd));
    }
    cmd.connect = vec![ConnectHint {
        cp: 0,
        to: EndpointParam::Junction { junction: id(0xccc) },
    }];
    // a hint whose target no longer exists is dropped with a notice; the symbol is still placed
    let r = e.execute(&cmd).unwrap();
    // FT-779: the notice names the gone record in words, never by its ID.
    assert!(
        r.outcome
            .notices
            .iter()
            .any(|n| n == "autoconnect hint 1 dropped: the junction it connects to no longer exists"),
        "{:?}",
        r.outcome.notices
    );
    assert!(created::<Connection>(&r, Kind::Connection).is_empty());
}

#[test]
fn place_hint_on_a_connection_splits_it_at_a_junction() {
    let mut e = engine(golden_model());
    let mut cmd = place(PAGE_102, pt(0, 0), new_device("F"));
    cmd.connect = vec![ConnectHint {
        cp: 0,
        to: EndpointParam::OnConnection {
            connection: CON_W11B,
            at: pt(G, 0),
        },
    }];
    let r = e.execute(&cmd).unwrap();
    let junctions: Vec<ew_ids::Id<ew_schema::Junction>> = created(&r, Kind::Junction);
    assert_eq!(junctions.len(), 1);
    assert_eq!(row(&e, junctions[0]).at, pt(G, 0));
    assert_eq!(row(&e, junctions[0]).sequence.len(), 3);
}

#[test]
fn place_on_a_wire_splits_it() {
    let wire: ew_ids::Id<Connection> = id(0x4100);
    let mut original = connection(wire, Some(PAGE_102), cp(FN_102G1, 0), cp(FN_T1, 1));
    original.route = vec![pt(0, -MM10), pt(0, 2 * MM10)];
    original.number = NumberSpec::Assigned(ew_schema::AssignedNumber::new(7));
    original.overrides.csa = Some(ew_schema::Csa::Mm2(ew_units::Dec3(2_500)));
    original.overrides.colour = Some(ew_schema::ColourSpec::one(ew_schema::ColourCode::Iec(
        ew_schema::IecColour::Bu,
    )));
    let model = golden_with(|r| r.push(Record::Connection(original.clone())));
    let mut e = engine(model);
    let r = e.execute(&place(PAGE_102, pt(0, 0), new_device("F"))).unwrap();
    assert!(
        r.passes <= ew_commands::normalise::MAX_PASSES + 1,
        "passes: {}",
        r.passes
    );
    let f: ew_ids::Id<Function> = created(&r, Kind::Function)[0];
    let first = row(&e, wire);
    assert_eq!(first.a, cp(FN_102G1, 0));
    assert_eq!(first.b, cp(f, 0));
    assert_eq!(first.route, vec![pt(0, -MM10), pt(0, 0)]);
    assert_eq!(first.number, original.number);
    assert_eq!(first.overrides, original.overrides);
    assert_eq!(first.kind, original.kind);
    let wires: Vec<ew_ids::Id<Connection>> = created(&r, Kind::Connection);
    assert_eq!(wires.len(), 1);
    let second = row(&e, wires[0]);
    assert_eq!(second.a, cp(f, 1));
    assert_eq!(second.b, cp(FN_T1, 1));
    assert_eq!(second.route, vec![pt(0, MM10), pt(0, 2 * MM10)]);
    assert_eq!(second.number, NumberSpec::Unassigned);
    assert_eq!(second.overrides, original.overrides);
    assert_ne!(second.overrides, Default::default());
    assert_eq!(second.kind, original.kind);
    assert_eq!(second.page, Some(PAGE_102));
}

// ---------------------------------------------------------------------------------------------------------
// rep.move

fn mv(reps: Vec<ew_ids::Id<Representation>>, delta: Pt2) -> MoveReps {
    MoveReps {
        items: Vec::new(),
        steps: None,
        reps,
        delta,
        page: None,
    }
}

/// The golden model with a routed wire between the representations of `FN_102F2` and `FN_T1`.
fn routed_golden() -> Model {
    golden_with(|records| {
        for rec in records {
            if let Record::Connection(c) = rec {
                if c.id == CON_W29 {
                    c.route = vec![pt(0, 0), pt(G, 0), pt(G, 3 * G), pt(2 * G, 3 * G)];
                }
            }
        }
    })
}

#[test]
fn move_translates_the_whole_route_when_both_ends_move() {
    let mut e = engine(routed_golden());
    let delta = pt(G, 2 * G);
    let r = e.execute(&mv(vec![REP_102F2, REP_T1], delta)).unwrap();
    assert_eq!(r.outcome.primary, None);
    assert_eq!(row(&e, REP_102F2).pos, delta);
    assert_eq!(row(&e, REP_T1).pos, delta);
    assert_eq!(row(&e, REP_102G1).pos, pt0(), "other representations stay");
    assert_eq!(
        row(&e, CON_W29).route,
        vec![pt(G, 2 * G), pt(2 * G, 2 * G), pt(2 * G, 5 * G), pt(3 * G, 5 * G)]
    );
}

#[test]
fn move_follows_the_end_when_one_end_moves() {
    let mut e = engine(routed_golden());
    e.execute(&mv(vec![REP_102F2], pt(0, G))).unwrap();
    assert_eq!(
        row(&e, CON_W29).route,
        vec![pt(0, G), pt(G, G), pt(G, 3 * G), pt(2 * G, 3 * G)]
    );
    // The end at B follows too, keeping its neighbour orthogonal.
    let mut e = engine(routed_golden());
    e.execute(&mv(vec![REP_T1], pt(G, 0))).unwrap();
    assert_eq!(
        row(&e, CON_W29).route,
        vec![pt(0, 0), pt(G, 0), pt(G, 3 * G), pt(3 * G, 3 * G)]
    );
}

#[test]
fn move_across_pages_moves_wires_with_both_ends_and_renumbers() {
    let mut e = engine(golden_model());
    let placed = e.execute(&place(PAGE_102, pt(0, 0), new_device("F"))).unwrap();
    let new_dev: ew_ids::Id<Device> = created(&placed, Kind::Device)[0];
    assert_eq!(row(&e, new_dev).tag.counter, Some(1));
    e.execute(&MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![REP_101F1, REP_IP_101],
        delta: pt0(),
        page: Some(PAGE_102),
    })
    .unwrap();
    assert_eq!(row(&e, REP_101F1).page, PAGE_102);
    assert_eq!(row(&e, REP_IP_101).page, PAGE_102);
    assert_eq!(row(&e, CON_W11A).page, Some(PAGE_102));
    assert_eq!(
        row(&e, DEV_101F1).tag.counter,
        Some(3),
        "FR-NUM-029: the moved device renumbers"
    );
    assert_eq!(
        row(&e, new_dev).tag.counter,
        Some(1),
        "the device that stayed keeps its counter"
    );
}

#[test]
fn move_across_pages_rejects_a_wire_with_one_end_left_behind() {
    let mut e = engine(golden_model());
    let cmd = MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![REP_101F1],
        delta: pt0(),
        page: Some(PAGE_102),
    };
    rejected(e.execute(&cmd));
    // A wire to a junction stays behind as well.
    let cmd = MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![REP_102G1],
        delta: pt0(),
        page: Some(PAGE_101),
    };
    rejected(e.execute(&cmd));
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn move_rejects_empty_lists_and_unknown_references() {
    let mut e = engine(golden_model());
    rejected(e.execute(&mv(vec![], pt(G, 0))));
    not_found(e.execute(&mv(vec![id(0x999)], pt(G, 0))));
    not_found(e.execute(&MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![REP_102F2],
        delta: pt0(),
        page: Some(id(0x998)),
    }));
}

#[test]
fn move_changes_only_the_position_without_wires() {
    let mut e = engine(golden_model());
    let before = row(&e, REP_102F2_AUX);
    e.execute(&mv(vec![REP_102F2_AUX], pt(5 * G, -G))).unwrap();
    let mut expected = (*before).clone();
    expected.pos = pt(5 * G, -G);
    assert_eq!(*row(&e, REP_102F2_AUX), expected);
}

// ---------------------------------------------------------------------------------------------------------
// rep.transform

fn transform(reps: Vec<ew_ids::Id<Representation>>, rotate: u8, mirror: bool) -> TransformReps {
    TransformReps {
        reps,
        rotate,
        mirror,
        variant: None,
    }
}

/// The golden model with `REP_102F2` drawn by the two-CP symbol at the origin and a vertical wire at its
/// CP 1.
fn two_cp_golden() -> Model {
    golden_with(|records| {
        for rec in records.iter_mut() {
            if let Record::Representation(r) = rec {
                if r.id == REP_102F2 {
                    r.symbol = SYM_TWO;
                }
            }
        }
        let mut wire = connection(id(0x4101), Some(PAGE_102), cp(FN_102F2, 1), cp(FN_T1, 2));
        wire.route = vec![pt(0, MM10), pt(0, 3 * MM10)];
        records.push(Record::Connection(wire));
    })
}

#[test]
fn transform_composes_orientation_and_sets_the_variant() {
    let mut e = engine(golden_model());
    let mut cmd = transform(vec![REP_102G1], 1, true);
    cmd.variant = Some(3);
    let r = e.execute(&cmd).unwrap();
    assert_eq!(r.outcome.primary, None);
    let rep = row(&e, REP_102G1);
    assert_eq!(rep.orient, Orient2::R0.then(Orient2::R90).then(Orient2::M0));
    assert_eq!(rep.orient, Orient2::M270);
    assert_eq!(rep.variant, 3);
    // A second turn composes with the stored orientation.
    e.execute(&transform(vec![REP_102G1], 0, false)).unwrap();
    assert_eq!(row(&e, REP_102G1).orient, Orient2::M270);
    e.execute(&transform(vec![REP_102G1], 1, false)).unwrap();
    assert_eq!(row(&e, REP_102G1).orient, Orient2::M270.then(Orient2::R90));
}

#[test]
fn transform_makes_the_route_follow_the_moved_cp() {
    let mut e = engine(two_cp_golden());
    e.execute(&transform(vec![REP_102F2], 1, false)).unwrap();
    assert_eq!(row(&e, REP_102F2).orient, Orient2::R90);
    // CP 1 at (0, 10 mm) turns to (-10 mm, 0): a displacement of (-10 mm, -10 mm). The far end stays (FT-423), so the
    // vertical segment keeps its axis with a dog-leg at the grid y nearest the middle.
    let ym = grid().snap_point(ew_units::Nm(3 * MM10 / 2)).expect("in range").0;
    assert_eq!(
        row(&e, id::<Connection>(0x4101)).route,
        vec![pt(-MM10, 0), pt(-MM10, ym), pt(0, ym), pt(0, 3 * MM10)]
    );
}

#[test]
fn transform_without_cp_geometry_leaves_routes_alone() {
    let mut e = engine(routed_golden());
    let before = kinds(e.model(), Kind::Connection);
    e.execute(&transform(vec![REP_102F2, REP_T1], 2, true)).unwrap();
    assert_eq!(kinds(e.model(), Kind::Connection), before);
}

#[test]
fn transform_rejects_empty_lists_and_bad_rotations() {
    let mut e = engine(golden_model());
    rejected(e.execute(&transform(vec![], 1, false)));
    rejected(e.execute(&transform(vec![REP_102G1], 4, false)));
    not_found(e.execute(&transform(vec![id(0x997)], 1, false)));
    assert_eq!(e.model().rev().0, 0);
}

// ---------------------------------------------------------------------------------------------------------
// terminal.add

#[test]
fn add_terminals_defaults_the_sequence_numbers() {
    let mut e = engine(golden_model());
    let mut with_text = terminal_spec("4", None);
    with_text.function_text = ew_mltext::MlText::neutral("2+");
    with_text.rail_bonded = true;
    let r = e
        .execute(&add_terminals(
            DEV_X1031,
            vec![with_text.clone(), terminal_spec("5", None)],
        ))
        .unwrap();
    let devs: Vec<ew_ids::Id<Device>> = created(&r, Kind::Device);
    let fns: Vec<ew_ids::Id<Function>> = created(&r, Kind::Function);
    assert_eq!((devs.len(), fns.len()), (2, 2));
    assert_eq!(primary::<Device>(&r), devs[0]);
    let mut t4 = terminal(devs[0], DEV_X1031, 4, "4");
    t4.tag.counter = Some(4);
    t4.function_text = ew_mltext::MlText::neutral("2+");
    t4.aspects.terminal.as_mut().unwrap().rail_bonded = true;
    assert_eq!(*row(&e, devs[0]), t4);
    let mut t5 = terminal(devs[1], DEV_X1031, 5, "5");
    t5.tag.counter = Some(5);
    assert_eq!(*row(&e, devs[1]), t5);
    assert_eq!(
        *row(&e, fns[0]),
        function(fns[0], devs[0], FNDEF_TERMINAL, ew_schema::FnRole::Main)
    );
}

#[test]
fn add_terminals_starts_at_one_in_an_empty_strip_and_keeps_explicit_sequences() {
    let mut e = engine(golden_model());
    let mut cmd = create("X");
    cmd.counter = Some(2000);
    cmd.strip = strip(DEV_X1031, "X", 1).aspects.strip;
    let strip_id: ew_ids::Id<Device> = primary(&e.execute(&cmd).unwrap());
    let r = e
        .execute(&add_terminals(
            strip_id,
            vec![terminal_spec("A", Some(10)), terminal_spec("B", None)],
        ))
        .unwrap();
    let devs: Vec<ew_ids::Id<Device>> = created(&r, Kind::Device);
    let seq = |d: ew_ids::Id<Device>| row(&e, d).aspects.terminal.clone().unwrap().seq;
    assert_eq!((seq(devs[0]), seq(devs[1])), (10, 1));
    // The tag carries the strip's letter and the sequence number.
    assert_eq!(row(&e, devs[0]).tag.letter, ident("X"));
    assert_eq!(row(&e, devs[0]).tag.counter, Some(10));
}

#[test]
fn add_terminals_rejections() {
    let mut e = engine(golden_model());
    rejected(e.execute(&add_terminals(DEV_102F2, vec![terminal_spec("9", None)])));
    rejected(e.execute(&add_terminals(DEV_X1031, vec![])));
    // Designation of an existing terminal.
    rejected(e.execute(&add_terminals(DEV_X1031, vec![terminal_spec("2", None)])));
    // Designation twice in the list.
    rejected(e.execute(&add_terminals(
        DEV_X1031,
        vec![terminal_spec("7", None), terminal_spec("7", None)],
    )));
    // Sequence number in use.
    rejected(e.execute(&add_terminals(DEV_X1031, vec![terminal_spec("8", Some(2))])));
    not_found(e.execute(&add_terminals(id(0x996), vec![terminal_spec("8", None)])));
    assert_eq!(e.model().rev().0, 0);
}

// ---------------------------------------------------------------------------------------------------------
// cable.create and conductor.add

fn cable_cmd(cores: &[&str]) -> CreateCable {
    CreateCable {
        letter: ident("W"),
        counter: None,
        parts: vec![],
        function_text: Default::default(),
        structure: Default::default(),
        cable: Cable::default(),
        conductors: cores.iter().map(|c| conductor_spec(c)).collect(),
    }
}

#[test]
fn create_cable_creates_conductors_in_order() {
    let mut e = engine(golden_model());
    let mut cmd = cable_cmd(&["1", "2", "SH"]);
    cmd.counter = Some(5);
    cmd.conductors[2].shield = true;
    cmd.conductors[1].pair = Some(1);
    let r = e.execute(&cmd).unwrap();
    let id: ew_ids::Id<Device> = primary(&r);
    let mut expected = expected_device(id, "W", Some(5));
    expected.aspects.cable = Some(Cable::default());
    assert_eq!(*row(&e, id), expected);
    let conductors: Vec<ew_ids::Id<Conductor>> = created(&r, Kind::Conductor);
    assert_eq!(conductors.len(), 3);
    assert_eq!(*row(&e, conductors[0]), conductor(conductors[0], id, "1"));
    let mut c2 = conductor(conductors[1], id, "2");
    c2.pair = Some(1);
    assert_eq!(*row(&e, conductors[1]), c2);
    let mut c3 = conductor(conductors[2], id, "SH");
    c3.shield = true;
    assert_eq!(*row(&e, conductors[2]), c3);
    assert!(
        e.model()
            .table::<Function>()
            .iter()
            .all(|(_, f)| f.device != Some(id))
    );
}

#[test]
fn create_cable_letter_defaults_to_w_and_rejects_duplicate_cores() {
    let cmd: CreateCable = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(cmd.letter, ident("W"));
    let mut e = engine(golden_model());
    rejected(e.execute(&cable_cmd(&["1", "2", "1"])));
    assert_eq!(e.model().rev().0, 0);
    // No conductors is fine.
    let r = e.execute(&cable_cmd(&[])).unwrap();
    assert_eq!(r.created.len(), 1);
}

#[test]
fn add_conductors_extends_a_cable() {
    let mut e = engine(golden_model());
    let mut shield = conductor_spec("SH2");
    shield.shield = true;
    let r = e
        .execute(&AddConductors {
            cable: DEV_3W1,
            conductors: vec![conductor_spec("C"), shield],
        })
        .unwrap();
    let ids: Vec<ew_ids::Id<Conductor>> = created(&r, Kind::Conductor);
    assert_eq!(ids.len(), 2);
    assert_eq!(primary::<Conductor>(&r), ids[0]);
    assert_eq!(*row(&e, ids[0]), conductor(ids[0], DEV_3W1, "C"));
    let mut sh = conductor(ids[1], DEV_3W1, "SH2");
    sh.shield = true;
    assert_eq!(*row(&e, ids[1]), sh);
    assert!(maybe_row(&e, COND_A).is_some());
}

#[test]
fn add_conductors_rejections() {
    let mut e = engine(golden_model());
    let add = |cable, cores: &[&str]| AddConductors {
        cable,
        conductors: cores.iter().map(|c| conductor_spec(c)).collect(),
    };
    rejected(e.execute(&add(DEV_102F2, &["C"])));
    rejected(e.execute(&add(DEV_3W1, &["A"])));
    rejected(e.execute(&add(DEV_3W1, &["C", "C"])));
    rejected(e.execute(&add(DEV_3W1, &[])));
    not_found(e.execute(&add(id(0x995), &["C"])));
    assert_eq!(e.model().rev().0, 0);
}

// ---------------------------------------------------------------------------------------------------------
// rep.set_anchor_policy

/// The anchor override of a representation.
fn anchor_of(e: &Engine, rep: ew_ids::Id<Representation>) -> Option<AnchorPolicy> {
    e.model()
        .table::<Representation>()
        .get(&rep)
        .expect("representation")
        .anchor
        .clone()
}

/// The anchor policy of the shared symbol copy.
fn symbol_anchor(e: &Engine) -> AnchorPolicy {
    e.model()
        .table::<LibCopy<SymbolDef>>()
        .get(&SYM)
        .expect("symbol copy")
        .item
        .anchor
        .clone()
}

fn set_anchor(reps: Vec<ew_ids::Id<Representation>>, anchor: Option<AnchorPolicy>) -> SetAnchorPolicy {
    SetAnchorPolicy { reps, anchor }
}

fn invalid_at<T: std::fmt::Debug>(r: Result<T, CmdError>, at: &str) {
    match r {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, at),
        other => panic!("expected InvalidParams at {at}, got {other:?}"),
    }
}

#[test]
fn set_anchor_policy_overrides_one_representation_and_undoes() {
    let mut e = engine(golden_model());
    assert_eq!(anchor_of(&e, REP_102F2), None);
    let r = e
        .execute(&set_anchor(vec![REP_102F2], Some(AnchorPolicy::GraphicCentre)))
        .unwrap();
    assert!(r.outcome.notices.is_empty());
    assert_eq!(anchor_of(&e, REP_102F2), Some(AnchorPolicy::GraphicCentre));
    // The symbol copy and the other representations of it keep their policy.
    assert_eq!(symbol_anchor(&e), AnchorPolicy::Origin);
    assert_eq!(anchor_of(&e, REP_101F1), None);
    e.execute(&set_anchor(
        vec![REP_102F2, REP_101F1],
        Some(AnchorPolicy::Origin),
    ))
    .unwrap();
    assert_eq!(anchor_of(&e, REP_102F2), Some(AnchorPolicy::Origin));
    assert_eq!(anchor_of(&e, REP_101F1), Some(AnchorPolicy::Origin));
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(anchor_of(&e, REP_102F2), Some(AnchorPolicy::GraphicCentre));
    assert_eq!(anchor_of(&e, REP_101F1), None);
}

#[test]
fn set_anchor_policy_null_resets_to_the_symbol_copy() {
    let mut e = engine(golden_model());
    e.execute(&set_anchor(vec![REP_102F2], Some(AnchorPolicy::GraphicCentre)))
        .unwrap();
    e.execute(&AnyCommand {
        name: "rep.set_anchor_policy".into(),
        params: serde_json::json!({ "reps": [REP_102F2.to_string()], "anchor": null }),
    })
    .unwrap();
    assert_eq!(anchor_of(&e, REP_102F2), None);
}

#[test]
fn set_anchor_policy_rejections() {
    let mut e = engine(golden_model());
    invalid_at(
        e.execute(&set_anchor(vec![], Some(AnchorPolicy::Origin))),
        "/reps",
    );
    invalid_at(
        e.execute(&set_anchor(
            vec![REP_102F2, REP_102F2],
            Some(AnchorPolicy::Origin),
        )),
        "/reps/1",
    );
    invalid_at(
        e.execute(&set_anchor(
            vec![REP_102F2],
            Some(AnchorPolicy::Unknown(serde_json::json!("future"))),
        )),
        "/anchor",
    );
    not_found(e.execute(&set_anchor(vec![id(0x999)], Some(AnchorPolicy::GraphicCentre))));
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn set_anchor_policy_with_the_same_value_changes_nothing() {
    let mut e = engine(golden_model());
    let r = e.execute(&set_anchor(vec![REP_102F2], None));
    assert!(r.map_or(true, |r| r.outcome.notices.is_empty()));
    assert_eq!(anchor_of(&e, REP_102F2), None);
}

#[test]
fn set_anchor_policy_parses_from_json() {
    let mut e = engine(golden_model());
    let cmd = AnyCommand {
        name: "rep.set_anchor_policy".into(),
        params: serde_json::json!({ "reps": [REP_102F2.to_string()], "anchor": "graphic_centre" }),
    };
    e.execute(&cmd).unwrap();
    assert_eq!(anchor_of(&e, REP_102F2), Some(AnchorPolicy::GraphicCentre));
    let cmd = AnyCommand {
        name: "rep.set_anchor_policy".into(),
        params: serde_json::json!({ "reps": [REP_102F2.to_string()], "anchor": { "slot": "tag" } }),
    };
    e.execute(&cmd).unwrap();
    assert!(matches!(anchor_of(&e, REP_102F2), Some(AnchorPolicy::Slot(_))));
}
