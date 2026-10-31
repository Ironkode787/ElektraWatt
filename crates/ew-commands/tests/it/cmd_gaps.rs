// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `box.black`, `bond.create` and `external_ref.create`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    CreateBond, CreateExternalRef, EndpointParam, KindName, PlaceBlackBox, PlaceTarget, SetFields,
};
use ew_commands::{CmdError, Engine, Receipt};
use ew_ids::Id;
use ew_model::Entity;
use ew_schema::{
    Bond, BondKind, BoxCp, BoxSpec, Dir4, ExternalRef, Extra, FnRole, Function, FunctionKind, Kind,
    Representation,
};
use ew_units::{Orient2, Pt2};

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> std::sync::Arc<T> {
    e.model().table::<T>().get(&id).expect("row exists").clone()
}

fn created<T>(r: &Receipt, kind: Kind) -> Vec<Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| typed(*k))
        .collect()
}

fn primary<T>(r: &Receipt) -> Id<T> {
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

fn invalid_path<T: std::fmt::Debug>(r: Result<T, CmdError>, command: &str) -> String {
    match r {
        Err(CmdError::InvalidParams { command: c, path, .. }) => {
            assert_eq!(c, command);
            path
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------
// box.black

fn box_cp(idx: u16, pos: Pt2) -> BoxCp {
    BoxCp {
        idx,
        pos,
        dir: Dir4::Down,
        extra: Extra::new(),
    }
}

fn spec() -> BoxSpec {
    BoxSpec {
        outline: vec![pt(0, 0), pt(4 * G, 0), pt(4 * G, 3 * G), pt(0, 3 * G)],
        cps: vec![box_cp(0, pt(G, 3 * G)), box_cp(1, pt(2 * G, 3 * G))],
        inner: vec![],
        extra: Extra::new(),
    }
}

fn new_device(letter: &str) -> PlaceTarget {
    PlaceTarget::NewDevice {
        letter: ident(letter),
        fndef: None,
        kind: None,
    }
}

fn black(pos: Pt2, box_: Option<BoxSpec>, target: PlaceTarget) -> PlaceBlackBox {
    PlaceBlackBox {
        copies: Vec::new(),
        page: PAGE_102,
        symbol: SYM_TWO,
        pos,
        orient: Orient2::R0,
        rep_type: None,
        box_,
        target,
    }
}

#[test]
fn black_box_new_device_creates_function_and_representation() {
    let mut e = engine(golden_model());
    let r = e
        .execute(&black(pt(3 * G, 4 * G), Some(spec()), new_device("A")))
        .expect("box.black");
    let devs: Vec<Id<ew_schema::Device>> = created(&r, Kind::Device);
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    let reps: Vec<Id<Representation>> = created(&r, Kind::Representation);
    assert_eq!((devs.len(), fns.len(), reps.len()), (1, 1, 1));
    assert_eq!(primary::<Representation>(&r), reps[0]);
    let f = row(&e, fns[0]);
    assert_eq!(f.kind, FunctionKind::BlackBox);
    assert_eq!(f.role, FnRole::Main);
    assert_eq!(f.device, Some(devs[0]));
    let rep = row(&e, reps[0]);
    assert_eq!(rep.box_, Some(spec()));
    assert_eq!(rep.functions, vec![fns[0]]);
    assert_eq!(
        (rep.page, rep.symbol, rep.pos),
        (PAGE_102, SYM_TWO, pt(3 * G, 4 * G))
    );
}

#[test]
fn black_box_join_device_creates_aux_black_box_function() {
    let mut e = engine(golden_model());
    let target = PlaceTarget::JoinDevice {
        device: DEV_102F2,
        fndef: None,
        role: FnRole::Aux,
    };
    let r = e
        .execute(&black(pt(3 * G, 4 * G), Some(spec()), target))
        .expect("box.black join");
    let fns: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!(fns.len(), 1);
    assert!(created::<ew_schema::Device>(&r, Kind::Device).is_empty());
    let f = row(&e, fns[0]);
    assert_eq!(f.kind, FunctionKind::BlackBox);
    assert_eq!(f.role, FnRole::Aux);
    assert_eq!(f.device, Some(DEV_102F2));
}

#[test]
fn black_box_existing_function_must_be_a_black_box() {
    let mut e = engine(golden_model());
    let target = PlaceTarget::ExistingFunction { function: FN_102F2 };
    rejected(e.execute(&black(pt(3 * G, 4 * G), Some(spec()), target)));

    let r = e
        .execute(&black(pt(3 * G, 4 * G), Some(spec()), new_device("A")))
        .unwrap();
    let f: Id<Function> = created::<Function>(&r, Kind::Function)[0];
    let target = PlaceTarget::ExistingFunction { function: f };
    let r2 = e
        .execute(&black(pt(6 * G, 8 * G), Some(spec()), target))
        .expect("second representation");
    assert!(created::<Function>(&r2, Kind::Function).is_empty());
    let rep2 = row(&e, primary::<Representation>(&r2));
    assert_eq!(rep2.functions, vec![f]);
    assert_eq!(rep2.pos, pt(6 * G, 8 * G));
}

#[test]
fn black_box_outline_needs_three_points() {
    let mut e = engine(golden_model());
    let mut b = spec();
    b.outline = vec![pt(0, 0), pt(G, G)];
    let path = invalid_path(e.execute(&black(pt(0, 0), Some(b), new_device("A"))), "box.black");
    assert_eq!(path, "/box/outline");
}

#[test]
fn black_box_duplicate_cp_index_is_invalid() {
    let mut e = engine(golden_model());
    let mut b = spec();
    b.cps = vec![box_cp(0, pt(G, 3 * G)), box_cp(0, pt(2 * G, 3 * G))];
    let path = invalid_path(e.execute(&black(pt(0, 0), Some(b), new_device("A"))), "box.black");
    assert_eq!(path, "/box/cps/1/idx");
}

#[test]
fn black_box_cp_off_grid_is_refused() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let mut b = spec();
    b.cps = vec![box_cp(0, pt(G, 3 * G)), box_cp(1, pt(2 * G + 1000, 3 * G))];
    rejected(e.execute(&black(pt(0, 0), Some(b), new_device("A"))));
    assert_eq!(lines(e.model()), before);
}

#[test]
fn black_box_off_grid_pos_and_disallowed_rep_type_are_refused() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    rejected(e.execute(&black(pt(G + 1000, 0), Some(spec()), new_device("A"))));
    let mut cmd = black(pt(3 * G, 4 * G), Some(spec()), new_device("A"));
    cmd.rep_type = Some(ew_schema::RepType::Overview);
    rejected(e.execute(&cmd));
    assert_eq!(lines(e.model()), before);
}

#[test]
fn black_box_second_main_function_is_refused_by_ew_inv_002() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let target = PlaceTarget::JoinDevice {
        device: DEV_102F2,
        fndef: None,
        role: FnRole::Main,
    };
    let err = e
        .execute(&black(pt(3 * G, 4 * G), Some(spec()), target))
        .unwrap_err();
    assert!(matches!(err, CmdError::Model(_)), "{err:?}");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn black_box_undo_and_redo() {
    let mut e = engine(golden_model());
    let before = golden_model();
    e.execute(&black(pt(3 * G, 4 * G), Some(spec()), new_device("A")))
        .unwrap();
    let after = lines(e.model());
    assert_ne!(after, lines(&before));
    e.undo(&ew_commands::Origin::Gui).unwrap();
    assert!(e.model().content_eq(&before));
    e.redo(&ew_commands::Origin::Gui).unwrap();
    assert_eq!(lines(e.model()), after);
}

// ---------------------------------------------------------------------------------------------------------
// bond.create

fn bond(a: EndpointParam, b: EndpointParam, via: Option<Id<ew_schema::Connection>>) -> CreateBond {
    CreateBond {
        a,
        b,
        kind: BondKind::Equipotential,
        via,
    }
}

fn cp_param(function: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function, idx }
}

#[test]
fn bond_create_inserts_the_bond() {
    let mut e = engine(golden_model());
    let cmd = bond(cp_param(FN_102F2, 1), cp_param(FN_102G1, 1), Some(CON_W27));
    let r = e.execute(&cmd).expect("bond.create");
    let id: Id<Bond> = primary(&r);
    let b = row(&e, id);
    assert_eq!(b.a, cp(FN_102F2, 1));
    assert_eq!(b.b, cp(FN_102G1, 1));
    assert_eq!(b.kind, BondKind::Equipotential);
    assert_eq!(b.via, Some(CON_W27));
}

#[test]
fn bond_create_rejects_on_connection_end_and_equal_ends() {
    let mut e = engine(golden_model());
    let on_conn = EndpointParam::OnConnection {
        connection: CON_W27,
        at: pt(0, 0),
    };
    rejected(e.execute(&bond(on_conn, cp_param(FN_102G1, 1), None)));
    rejected(e.execute(&bond(cp_param(FN_102G1, 1), cp_param(FN_102G1, 1), None)));
}

#[test]
fn bond_create_rejects_an_unknown_kind_and_a_missing_function() {
    let mut e = engine(golden_model());
    let mut cmd = bond(cp_param(FN_102F2, 1), cp_param(FN_102G1, 1), None);
    cmd.kind = BondKind::Unknown(serde_json::json!("future_kind"));
    assert_eq!(invalid_path(e.execute(&cmd), "bond.create"), "/kind");
    not_found(e.execute(&bond(cp_param(id(0xbeef), 0), cp_param(FN_102G1, 1), None)));
    assert_eq!(e.model().rev().0, 0);
}

#[test]
fn bond_create_missing_via_is_not_found() {
    let mut e = engine(golden_model());
    let cmd = bond(cp_param(FN_102F2, 1), cp_param(FN_102G1, 1), Some(id(0xdead)));
    not_found(e.execute(&cmd));
}

// ---------------------------------------------------------------------------------------------------------
// external_ref.create

fn xref(location: Id<ew_schema::StructureNode>) -> CreateExternalRef {
    CreateExternalRef {
        location,
        device: ident("JE-N1000-03"),
        connection: Some(ident("L")),
        project: Some(ident("OTHER")),
    }
}

#[test]
fn external_ref_create_inserts_the_reference() {
    let mut e = engine(golden_model());
    let r = e.execute(&xref(LOC)).expect("external_ref.create");
    let x = row(&e, primary::<ExternalRef>(&r));
    assert_eq!(x.location, LOC);
    assert_eq!(x.device, ident("JE-N1000-03"));
    assert_eq!(x.connection, Some(ident("L")));
    assert_eq!(x.project, Some(ident("OTHER")));
}

#[test]
fn external_ref_create_needs_an_existing_location_node() {
    let mut e = engine(golden_model());
    rejected(e.execute(&xref(FUNC_P01)));
    not_found(e.execute(&xref(id(0xbeef))));
}

#[test]
fn external_ref_device_is_editable_with_fields_set() {
    let mut e = engine(golden_model());
    let r = e.execute(&xref(LOC)).unwrap();
    let x: Id<ExternalRef> = primary(&r);
    e.execute(&SetFields {
        kind: KindName::ExternalRef,
        targets: vec![x.cast()],
        path: "device".into(),
        value: serde_json::json!("JE-N2000-01"),
        copies: Vec::new(),
    })
    .expect("fields.set device");
    assert_eq!(row(&e, x).device, ident("JE-N2000-01"));
}
