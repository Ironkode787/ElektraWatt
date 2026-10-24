// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The interruption-point and black-box commands of WP-S2.04 part 3 (§5.1, §9.2, §9.4): `connection.draw_to_ip`,
//! `ip.arrange` and `box.reshape`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use std::fmt::Debug;

use common::fixture::*;
use common::*;
use ew_commands::commands::{ArrangeIps, DrawToIp, EndpointParam, IpAtEnd, ReshapeBox};
use ew_commands::geometry::point_on_grid;
use ew_commands::{CmdError, Engine, Receipt};
use ew_ids::{Id, Uuid};
use ew_model::{Entity, Model};
use ew_schema::{
    BoxCp, BoxSpec, Connection, Dir4, Endpoint, Extra, Function, FunctionKind, IpDirection, Junction, Kind,
    LibDocument, Page, Record, Representation, SymbolCp, SymbolDef,
};
use ew_units::{Nm, Orient2, Pt2};

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> T {
    (**e.model().table::<T>().get(&id).expect("record exists")).clone()
}

fn has<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> bool {
    e.model().table::<T>().get(&id).is_some()
}

fn created<T>(r: &Receipt, kind: Kind) -> Vec<Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| typed(*k))
        .collect()
}

/// A rejection whose reason contains `needle`.
#[track_caller]
fn rejected_with<T: Debug>(r: Result<T, CmdError>, needle: &str) {
    match r {
        Err(CmdError::Rejected { reason, .. }) => {
            assert!(reason.contains(needle), "reason `{reason}` lacks `{needle}`");
        }
        other => panic!("expected a rejection mentioning `{needle}`, got {other:?}"),
    }
}

#[track_caller]
fn not_found<T: Debug>(r: Result<T, CmdError>) {
    match r {
        Err(CmdError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[track_caller]
fn invalid_at<T: Debug>(r: Result<T, CmdError>, at: &str) {
    match r {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, at),
        other => panic!("expected InvalidParams at {at}, got {other:?}"),
    }
}

/// The grid position `k` steps from the origin (a step is a third of [`G`], so not always a whole nanometre).
fn grid_at(k: i64) -> Nm {
    grid().at(k).expect("on the grid")
}

fn cp_param(f: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function: f, idx }
}

fn cp_lib(idx: u16, fn_cp: u16, pos: Pt2, dir: Dir4) -> SymbolCp {
    SymbolCp {
        idx,
        fn_cp,
        pos,
        dir,
        discipline: Default::default(),
        extra: Extra::new(),
    }
}

/// Every segment of a route is horizontal or vertical.
fn orthogonal(route: &[Pt2]) -> bool {
    route.windows(2).all(|w| w[0].x == w[1].x || w[0].y == w[1].y)
}

// ---------------------------------------------------------------------------------------------------------
// The library of these tests

/// An item whose FnDef has exactly one CP ("1", index 0).
const ITEM_IP1: Uuid = Uuid::from_u128(0x9301);
/// An item whose FnDef is never embedded.
const ITEM_GONE: Uuid = Uuid::from_u128(0x9309);

/// An IP symbol with one CP at (G, 0) drawing FnDef CP 0; its FnDef resolves to `ITEM_IP1`'s copy.
const SYM_IP: Id<SymbolDef> = id(0x80);
/// The same FnDef, a symbol that draws no CP.
const SYM_IP_BARE: Id<SymbolDef> = id(0x81);
/// A symbol whose FnDef is not embedded.
const SYM_IP_GONE: Id<SymbolDef> = id(0x82);

fn ip_library() -> Vec<LibDocument> {
    let mut lib = library();
    lib.push(LibDocument::Fndef(fndef_copy_for(LIBRARY_X, ITEM_IP1, &["1"])));
    let mut ip = symbol_using(SYM_IP, LIBRARY_X, ITEM_IP1);
    ip.item.cps = ew_schema::KeyedList::from_vec(vec![cp_lib(0, 0, pt(G, 0), Dir4::Up)]).expect("one CP");
    lib.push(LibDocument::Symbol(ip));
    lib.push(LibDocument::Symbol(symbol_using(
        SYM_IP_BARE,
        LIBRARY_X,
        ITEM_IP1,
    )));
    let mut gone = symbol_using(SYM_IP_GONE, LIBRARY_X, ITEM_GONE);
    gone.item.cps = ew_schema::KeyedList::from_vec(vec![cp_lib(0, 0, pt(G, 0), Dir4::Up)]).expect("one CP");
    lib.push(LibDocument::Symbol(gone));
    lib
}

/// The golden records with `f` applied, over [`ip_library`].
fn ip_model(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    let (project, mut records, _) = mini_golden_parts();
    f(&mut records);
    Model::from_records(project, records, ip_library()).expect("valid model")
}

// ---------------------------------------------------------------------------------------------------------
// connection.draw_to_ip

/// The wire `W`: on page 102 from `cp(FN_102F2, 4)` to `cp(FN_102F2, 5)`, 4 G long.
const W: Id<Connection> = id(0x4100);

fn model_with_wire() -> Model {
    ip_model(|records| {
        let mut w = connection(W, Some(PAGE_102), cp(FN_102F2, 4), cp(FN_102F2, 5));
        w.route = vec![pt(0, 0), pt(4 * G, 0)];
        records.push(Record::Connection(w));
    })
}

const IP_NAME: &str = "24VDC-2";

fn ip_at_end(pos: Pt2) -> IpAtEnd {
    IpAtEnd {
        symbol: SYM_IP,
        fndef: None,
        pos,
        orient: Orient2::default(),
        name: ident(IP_NAME),
        direction: IpDirection::Source,
        star_source: false,
        sort: None,
    }
}

/// A wire from `a` to an IP at `pos` (its CP is at `pos + (G, 0)`) over `route`.
fn draw_to_ip(a: EndpointParam, pos: Pt2, route: Vec<Pt2>) -> DrawToIp {
    DrawToIp {
        a,
        page: PAGE_102,
        route,
        kind: None,
        ip: ip_at_end(pos),
    }
}

/// The IP functions of the model named [`IP_NAME`].
fn new_ips(e: &Engine) -> Vec<Function> {
    e.model()
        .table::<Function>()
        .iter()
        .map(|(_, f)| (**f).clone())
        .filter(|f| matches!(&f.kind, FunctionKind::Ip(s) if s.name == ident(IP_NAME)))
        .collect()
}

fn reps_of(e: &Engine, f: Id<Function>) -> Vec<Representation> {
    e.model()
        .table::<Representation>()
        .iter()
        .map(|(_, r)| (**r).clone())
        .filter(|r| r.functions == vec![f])
        .collect()
}

#[test]
fn draw_to_ip_from_a_cp_creates_function_representation_and_connection_in_one_undo_step() {
    let mut e = engine(ip_model(|_| {}));
    let before = lines(e.model());
    let cmd = draw_to_ip(cp_param(FN_102G1, 1), pt(5 * G, 0), vec![pt(0, 0), pt(6 * G, 0)]);
    let r = e.execute(&cmd).expect("connection.draw_to_ip");
    let ips = new_ips(&e);
    assert_eq!(ips.len(), 1, "one IP function");
    let f = &ips[0];
    assert_eq!(f.device, None);
    assert_eq!(
        f.def,
        ew_ids::v5::lib_copy_id(LIBRARY_X, ITEM_IP1),
        "the symbol's FnDef copy"
    );
    let FunctionKind::Ip(spec) = &f.kind else {
        panic!("an IP function");
    };
    assert_eq!(spec.name, ident(IP_NAME));
    assert_eq!(spec.direction, IpDirection::Source);
    assert_eq!(spec.external, None);
    let reps = reps_of(&e, f.id);
    assert_eq!(reps.len(), 1, "one representation");
    assert_eq!(reps[0].page, PAGE_102);
    assert_eq!(reps[0].symbol, SYM_IP);
    assert_eq!(reps[0].pos, pt(5 * G, 0));
    assert_eq!(reps[0].rep_type, ew_schema::RepType::MultiLine);
    // One connection, ending at the IP's CP; it is the receipt's primary.
    let key = r.outcome.primary.expect("primary");
    assert_eq!(key.0, Kind::Connection);
    let cons: Vec<Id<Connection>> = created(&r, Kind::Connection);
    assert_eq!(cons, vec![typed::<Connection>(key)]);
    let c = row(&e, typed::<Connection>(key));
    assert_eq!(c.a, cp(FN_102G1, 1));
    assert_eq!(c.b, cp(f.id, 0));
    assert_eq!(c.page, Some(PAGE_102));
    assert_eq!(c.route, vec![pt(0, 0), pt(6 * G, 0)]);
    assert_eq!(c.number, ew_schema::NumberSpec::default());
    assert_eq!(r.created.len(), 3, "function, representation, connection");
    // One undo removes all of it.
    e.undo().expect("undo");
    assert_eq!(lines(e.model()), before);
    assert!(!has(&e, f.id));
    assert!(!has(&e, reps[0].id));
    assert!(!has(&e, c.id));
}

#[test]
fn draw_to_ip_from_a_point_on_a_connection_splits_it_at_a_new_junction() {
    let mut e = engine(model_with_wire());
    let before = lines(e.model());
    let cmd = draw_to_ip(
        EndpointParam::OnConnection {
            connection: W,
            at: pt(2 * G, 0),
        },
        pt(G, 3 * G),
        vec![pt(2 * G, 0), pt(2 * G, 3 * G)],
    );
    let r = e.execute(&cmd).expect("connection.draw_to_ip");
    let junctions: Vec<Id<Junction>> = created(&r, Kind::Junction);
    assert_eq!(junctions.len(), 1, "one junction");
    let j = row(&e, junctions[0]);
    assert_eq!(j.page, PAGE_102);
    assert_eq!(j.at, pt(2 * G, 0));
    let new: Id<Connection> = typed(r.outcome.primary.expect("primary"));
    let created_cons: Vec<Id<Connection>> = created(&r, Kind::Connection);
    assert_eq!(created_cons.len(), 2, "the second half of W and the new wire");
    let second = *created_cons.iter().find(|c| **c != new).expect("second half");
    assert_eq!(j.sequence, vec![W, second, new]);
    assert_eq!(row(&e, W).b, Endpoint::Junction(j.id));
    assert_eq!(row(&e, W).route, vec![pt(0, 0), pt(2 * G, 0)]);
    assert_eq!(row(&e, second).a, Endpoint::Junction(j.id));
    assert_eq!(row(&e, second).route, vec![pt(2 * G, 0), pt(4 * G, 0)]);
    let ips = new_ips(&e);
    assert_eq!(ips.len(), 1);
    let c = row(&e, new);
    assert_eq!(c.a, Endpoint::Junction(j.id));
    assert_eq!(c.b, cp(ips[0].id, 0));
    assert_eq!(c.route, vec![pt(2 * G, 0), pt(2 * G, 3 * G)]);
    // One undo.
    e.undo().expect("undo");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn draw_to_ip_with_the_explicit_fndef_and_no_route_creates_the_wire() {
    let mut e = engine(ip_model(|_| {}));
    let mut cmd = draw_to_ip(cp_param(FN_102G1, 1), pt(5 * G, 0), vec![]);
    cmd.ip.fndef = Some(ew_ids::v5::lib_copy_id(LIBRARY_X, ITEM_IP1));
    let r = e.execute(&cmd).expect("connection.draw_to_ip");
    let c = row(&e, typed::<Connection>(r.outcome.primary.expect("primary")));
    assert!(c.route.is_empty());
    assert_eq!(c.kind, ew_schema::ConnectionKind::Wire);
}

#[test]
fn draw_to_ip_rejections_write_nothing() {
    let mut e = engine(model_with_wire());
    let rev = e.model().rev();
    let before = lines(e.model());
    let good = || draw_to_ip(cp_param(FN_102G1, 1), pt(5 * G, 0), vec![pt(0, 0), pt(6 * G, 0)]);
    // A missing page.
    let mut cmd = good();
    cmd.page = id::<Page>(0xdead);
    not_found(e.execute(&cmd));
    // A missing symbol copy.
    let mut cmd = good();
    cmd.ip.symbol = id(0xdeae);
    not_found(e.execute(&cmd));
    // A FnDef that is not embedded: the symbol's, and an explicit one.
    let mut cmd = good();
    cmd.ip.symbol = SYM_IP_GONE;
    not_found(e.execute(&cmd));
    let mut cmd = good();
    cmd.ip.fndef = Some(id(0xdeaf));
    not_found(e.execute(&cmd));
    // `ip.pos` off the grid.
    let mut cmd = good();
    cmd.ip.pos = pt(5 * G + 1, 0);
    rejected_with(e.execute(&cmd), "/ip/pos");
    // A FnDef with two CPs.
    let mut cmd = good();
    cmd.ip.symbol = SYM_TWO;
    rejected_with(e.execute(&cmd), "exactly one");
    // A symbol that does not draw the FnDef's CP.
    let mut cmd = good();
    cmd.ip.symbol = SYM_IP_BARE;
    rejected_with(e.execute(&cmd), "does not draw");
    // A route that does not end at the IP's CP (6 G).
    let mut cmd = good();
    cmd.route = vec![pt(0, 0), pt(5 * G, 0)];
    rejected_with(e.execute(&cmd), "/route/1");
    // A route vertex off the grid.
    let mut cmd = good();
    cmd.route = vec![pt(1, 0), pt(6 * G, 0)];
    rejected_with(e.execute(&cmd), "/route/0");
    // A missing `a` record: a function, a junction, a connection.
    let mut cmd = good();
    cmd.a = cp_param(id(0xdeb0), 1);
    not_found(e.execute(&cmd));
    let mut cmd = good();
    cmd.a = EndpointParam::Junction { junction: id(0xdeb1) };
    not_found(e.execute(&cmd));
    let mut cmd = good();
    cmd.a = EndpointParam::OnConnection {
        connection: id(0xdeb2),
        at: pt(0, 0),
    };
    not_found(e.execute(&cmd));
    // A split point that is not on the wire's route.
    let mut cmd = good();
    cmd.a = EndpointParam::OnConnection {
        connection: W,
        at: pt(2 * G, G),
    };
    assert!(matches!(e.execute(&cmd), Err(CmdError::Rejected { .. })));
    assert_eq!(e.model().rev(), rev);
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// ip.arrange

const IP_A: Id<Function> = id(0x210);
const IP_B: Id<Function> = id(0x211);
const IP_C: Id<Function> = id(0x212);
const REP_A: Id<Representation> = id(0x310);
const REP_B: Id<Representation> = id(0x311);
const REP_C: Id<Representation> = id(0x312);
const CON_A: Id<Connection> = id(0x4200);
const CON_B: Id<Connection> = id(0x4201);

fn ip_rep(rep_id: Id<Representation>, page: Id<Page>, f: Id<Function>, pos: Pt2) -> Record {
    let mut r = rep(rep_id, page, vec![f]);
    r.symbol = SYM_IP;
    r.pos = pos;
    Record::Representation(r)
}

fn ip_with_sort(f: Id<Function>, name: &str, sort: Option<i32>) -> Record {
    let mut function = ip_function(f, name, IpDirection::Source);
    let FunctionKind::Ip(spec) = &mut function.kind else {
        unreachable!("an IP function");
    };
    spec.sort = sort;
    Record::Function(function)
}

/// A at (6 G, 6 G), B at (2 G, 2 G), C at (4 G, 2 G) on page 102, with a wire from A's CP and a wire to B's CP.
fn arrange_model(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    ip_model(|records| {
        records.push(ip_with_sort(IP_A, "A-1", Some(7)));
        records.push(ip_with_sort(IP_B, "B-1", None));
        records.push(ip_with_sort(IP_C, "C-1", Some(1)));
        records.push(ip_rep(REP_A, PAGE_102, IP_A, pt(6 * G, 6 * G)));
        records.push(ip_rep(REP_B, PAGE_102, IP_B, pt(2 * G, 2 * G)));
        records.push(ip_rep(REP_C, PAGE_102, IP_C, pt(4 * G, 2 * G)));
        // A's CP (7 G, 6 G) is the first vertex; B's CP (3 G, 2 G) the last.
        let mut a = connection(CON_A, Some(PAGE_102), cp(IP_A, 0), cp(FN_T3, 2));
        a.route = vec![pt(7 * G, 6 * G), pt(7 * G, 8 * G)];
        records.push(Record::Connection(a));
        let mut b = connection(CON_B, Some(PAGE_102), cp(FN_T2, 2), cp(IP_B, 0));
        b.route = vec![pt(3 * G, 6 * G), pt(3 * G, 2 * G)];
        records.push(Record::Connection(b));
        f(records);
    })
}

fn arrange(ips: &[Id<Function>], set_sort: bool) -> ArrangeIps {
    ArrangeIps {
        ips: ips.to_vec(),
        set_sort,
    }
}

fn sort_of(e: &Engine, f: Id<Function>) -> Option<i32> {
    match row(e, f).kind {
        FunctionKind::Ip(spec) => spec.sort,
        _ => panic!("an IP function"),
    }
}

#[test]
fn arrange_ips_gives_each_its_slot_in_the_new_order_and_routes_follow() {
    let mut e = engine(arrange_model(|_| {}));
    let before = lines(e.model());
    // Slots: (2 G, 2 G), (4 G, 2 G), (6 G, 6 G), sorted by (y, x).
    let r = e
        .execute(&arrange(&[IP_A, IP_B, IP_C], false))
        .expect("ip.arrange");
    assert_eq!(r.outcome.primary, None);
    assert_eq!(row(&e, REP_A).pos, pt(2 * G, 2 * G));
    assert_eq!(row(&e, REP_B).pos, pt(4 * G, 2 * G));
    assert_eq!(row(&e, REP_C).pos, pt(6 * G, 6 * G));
    // Symbol, orientation and type are kept.
    assert_eq!(row(&e, REP_A).symbol, SYM_IP);
    assert_eq!(row(&e, REP_A).orient, Orient2::default());
    // A's CP moved from (7 G, 6 G) to (3 G, 2 G): the first vertex follows.
    let a = row(&e, CON_A).route;
    assert_eq!(a[0], pt(3 * G, 2 * G));
    assert_eq!(*a.last().expect("vertex"), pt(7 * G, 8 * G), "the far end stays");
    assert!(orthogonal(&a), "{a:?}");
    // B's CP moved from (3 G, 2 G) to (5 G, 2 G): the last vertex follows.
    let b = row(&e, CON_B).route;
    assert_eq!(*b.last().expect("vertex"), pt(5 * G, 2 * G));
    assert_eq!(b[0], pt(3 * G, 6 * G), "the far end stays");
    assert!(orthogonal(&b), "{b:?}");
    // set_sort false leaves `sort` alone.
    assert_eq!(sort_of(&e, IP_A), Some(7));
    assert_eq!(sort_of(&e, IP_B), None);
    assert_eq!(sort_of(&e, IP_C), Some(1));
    // One undo.
    e.undo().expect("undo");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn arrange_ips_with_set_sort_numbers_them_in_the_new_order() {
    let mut e = engine(arrange_model(|_| {}));
    e.execute(&arrange(&[IP_C, IP_A, IP_B], true))
        .expect("ip.arrange");
    // C takes the first slot, A the second, B the third.
    assert_eq!(row(&e, REP_C).pos, pt(2 * G, 2 * G));
    assert_eq!(row(&e, REP_A).pos, pt(4 * G, 2 * G));
    assert_eq!(row(&e, REP_B).pos, pt(6 * G, 6 * G));
    assert_eq!(sort_of(&e, IP_C), Some(1));
    assert_eq!(sort_of(&e, IP_A), Some(2));
    assert_eq!(sort_of(&e, IP_B), Some(3));
}

#[test]
fn arrange_ips_rejections_write_nothing() {
    // One IP.
    let mut e = engine(arrange_model(|_| {}));
    let rev = e.model().rev();
    let before = lines(e.model());
    invalid_at(e.execute(&arrange(&[IP_A], false)), "/ips");
    invalid_at(e.execute(&arrange(&[], false)), "/ips");
    // An IP twice.
    invalid_at(e.execute(&arrange(&[IP_A, IP_B, IP_A], false)), "/ips/2");
    invalid_at(e.execute(&arrange(&[IP_A, IP_A], false)), "/ips/1");
    // A missing function.
    not_found(e.execute(&arrange(&[IP_A, id(0xdead)], false)));
    // A function that is not an interruption point.
    rejected_with(
        e.execute(&arrange(&[IP_A, FN_102F2], false)),
        "not an interruption point",
    );
    // IPs on two pages (FN_IP_101 is on page 101).
    rejected_with(e.execute(&arrange(&[IP_A, FN_IP_101], false)), "different pages");
    assert_eq!(e.model().rev(), rev);
    assert_eq!(lines(e.model()), before);
    // An IP with two representations cannot be built: the model refuses it (EW-INV-013), so that refusal of the
    // command is a guard no valid model reaches.
}

// ---------------------------------------------------------------------------------------------------------
// box.reshape

/// The box: `REP_102F2` (function `FN_102F2`) at (G, G), a rectangle of 18 × 12 grid steps (6 G × 4 G) with the
/// top CP 2 at (2 G, 0) and the bottom CP 6 at (4 G, 4 G); CP 2 is wired by `CON_W22`, CP 6 by `CON_W29`.
const BOX_POS: i64 = G;

fn rect(w: i64, h: i64) -> Vec<Pt2> {
    let (w, h) = (grid_at(w), grid_at(h));
    vec![
        Pt2::new(Nm(0), Nm(0)),
        Pt2::new(w, Nm(0)),
        Pt2::new(w, h),
        Pt2::new(Nm(0), h),
    ]
}

fn box_cp(idx: u16, pos: Pt2, dir: Dir4) -> BoxCp {
    BoxCp {
        idx,
        pos,
        dir,
        extra: Extra::new(),
    }
}

fn box_spec(outline: Vec<Pt2>, cps: Vec<BoxCp>) -> BoxSpec {
    BoxSpec {
        outline,
        cps,
        inner: vec![],
        extra: Extra::new(),
    }
}

/// Absolute page position of a box-relative point (the box is unrotated at (G, G)).
fn at_box(p: Pt2) -> Pt2 {
    pt(BOX_POS + p.x.0, BOX_POS + p.y.0)
}

fn box_model(cps: Vec<BoxCp>, outline: Vec<Pt2>) -> Model {
    ip_model(|records| {
        for rec in records.iter_mut() {
            match rec {
                Record::Representation(r) if r.id == REP_102F2 => {
                    r.pos = pt(BOX_POS, BOX_POS);
                    r.box_ = Some(box_spec(outline.clone(), cps.clone()));
                }
                Record::Connection(c) if c.id == CON_W22 => {
                    // From J5 to CP 2 at (3 G, G).
                    c.route = vec![pt(3 * G, -G), pt(3 * G, G)];
                }
                Record::Connection(c) if c.id == CON_W29 => {
                    // From CP 6 at (5 G, 5 G) to a terminal.
                    c.route = vec![pt(5 * G, 5 * G), pt(5 * G, 7 * G), pt(9 * G, 7 * G)];
                }
                _ => {}
            }
        }
    })
}

fn standard_cps() -> Vec<BoxCp> {
    vec![
        box_cp(2, pt(2 * G, 0), Dir4::Up),
        box_cp(6, pt(4 * G, 4 * G), Dir4::Down),
    ]
}

fn standard_box() -> Model {
    box_model(standard_cps(), rect(18, 12))
}

fn reshape(outline: Vec<Pt2>, cps: Option<Vec<BoxCp>>) -> ReshapeBox {
    ReshapeBox {
        rep: REP_102F2,
        outline,
        cps,
    }
}

fn box_of(e: &Engine) -> BoxSpec {
    row(e, REP_102F2).box_.expect("a box")
}

fn cp_of(spec: &BoxSpec, idx: u16) -> BoxCp {
    spec.cps.iter().find(|c| c.idx == idx).expect("the CP").clone()
}

/// Whether the absolute point lies on the closed axis-aligned rectangle outline of `outline` (relative vertices).
fn on_rect_outline(outline: &[Pt2], p: Pt2) -> bool {
    let (l, t) = (BOX_POS + outline[0].x.0, BOX_POS + outline[0].y.0);
    let (r, b) = (BOX_POS + outline[2].x.0, BOX_POS + outline[2].y.0);
    let on_vertical = (p.x.0 == l || p.x.0 == r) && (t..=b).contains(&p.y.0);
    let on_horizontal = (p.y.0 == t || p.y.0 == b) && (l..=r).contains(&p.x.0);
    on_vertical || on_horizontal
}

#[test]
fn reshape_with_cps_replaces_the_outline_and_the_cps_and_routes_follow() {
    let mut e = engine(standard_box());
    let before = lines(e.model());
    let outline = rect(20, 12);
    let cps = vec![
        box_cp(6, pt(grid_at(14).0, grid_at(12).0), Dir4::Down),
        box_cp(2, pt(4 * G, 0), Dir4::Up),
    ];
    let r = e
        .execute(&reshape(outline.clone(), Some(cps.clone())))
        .expect("box.reshape");
    assert_eq!(r.outcome.primary, Some((Kind::Representation, REP_102F2.cast())));
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    let spec = box_of(&e);
    assert_eq!(spec.outline, outline);
    assert_eq!(spec.cps, cps, "the CPs as given, in the given order");
    // CP 2 moved from (3 G, G) to (5 G, G): the last vertex of the wire to it follows.
    let w22 = row(&e, CON_W22).route;
    assert_eq!(*w22.last().expect("vertex"), pt(5 * G, G));
    assert_eq!(w22[0], pt(3 * G, -G));
    assert!(orthogonal(&w22), "{w22:?}");
    // CP 6 moved from (5 G, 5 G) by two grid steps to the right.
    let w29 = row(&e, CON_W29).route;
    assert_eq!(w29[0], at_box(pt(grid_at(14).0, grid_at(12).0)));
    assert_eq!(*w29.last().expect("vertex"), pt(9 * G, 7 * G));
    assert!(orthogonal(&w29), "{w29:?}");
    e.undo().expect("undo");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn reshape_without_cps_widening_keeps_top_cps_and_moves_bottom_cps_with_the_edge() {
    let mut e = engine(standard_box());
    let before = lines(e.model());
    let two_steps = grid_at(2).0;
    let outline = rect(20, 12);
    let r = e.execute(&reshape(outline.clone(), None)).expect("box.reshape");
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    let spec = box_of(&e);
    assert_eq!(spec.outline, outline);
    assert_eq!(spec.cps.len(), 2);
    // The top edge runs left to right from its first vertex: CP 2 keeps its offset, so its position.
    let top = cp_of(&spec, 2);
    assert_eq!(top.pos, pt(2 * G, 0));
    assert_eq!(top.dir, Dir4::Up);
    // The bottom edge runs right to left from the (moved) right vertex: CP 6 keeps its 2 G offset from it, so it
    // moves right by the two grid steps the box grew.
    let bottom = cp_of(&spec, 6);
    assert_eq!(bottom.pos, pt(grid_at(14).0, grid_at(12).0));
    assert_eq!(bottom.pos.x.0 - 4 * G, two_steps);
    assert_eq!(bottom.dir, Dir4::Down);
    // Every CP lies on the outline and on the grid (absolute positions).
    for c in &spec.cps {
        let abs = at_box(c.pos);
        assert!(on_rect_outline(&spec.outline, abs), "CP {} at {abs:?}", c.idx);
        assert!(point_on_grid(grid(), abs), "CP {} at {abs:?}", c.idx);
    }
    // The wire to the top CP is untouched; the wire to the bottom CP follows its CP.
    assert_eq!(row(&e, CON_W22).route, vec![pt(3 * G, -G), pt(3 * G, G)]);
    let w29 = row(&e, CON_W29).route;
    assert_eq!(w29[0], at_box(bottom.pos));
    assert_eq!(*w29.last().expect("vertex"), pt(9 * G, 7 * G));
    assert!(orthogonal(&w29), "{w29:?}");
    e.undo().expect("undo");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn reshape_to_a_triangle_moves_the_cp_of_the_vanished_edge_to_the_nearest_edge_with_a_notice() {
    // CP 8 lies on the left edge (edge 3) at (0, 2 G); the triangle has three edges, 0 to 2.
    let cps = vec![
        box_cp(2, pt(2 * G, 0), Dir4::Up),
        box_cp(8, pt(0, 2 * G), Dir4::Left),
    ];
    let mut e = engine(box_model(cps, rect(18, 12)));
    let outline = vec![pt(0, 0), pt(6 * G, 0), pt(6 * G, 4 * G)];
    let r = e.execute(&reshape(outline.clone(), None)).expect("box.reshape");
    let spec = box_of(&e);
    assert_eq!(spec.outline, outline);
    assert_eq!(cp_of(&spec, 2).pos, pt(2 * G, 0));
    assert_eq!(cp_of(&spec, 2).dir, Dir4::Up);
    // The nearest remaining edge is edge 0 (the top), its nearest grid point (0, 0); the outward normal is up.
    assert_eq!(cp_of(&spec, 8).pos, pt(0, 0));
    assert_eq!(cp_of(&spec, 8).dir, Dir4::Up);
    assert_eq!(
        r.outcome.notices,
        vec!["connection point 8 moved to edge 0: its edge 3 no longer exists".to_owned()]
    );
}

#[test]
fn reshape_rejections_write_nothing() {
    let mut e = engine(standard_box());
    let rev = e.model().rev();
    let before = lines(e.model());
    let good = || rect(20, 12);
    // No such representation; a representation without a box.
    let mut cmd = reshape(good(), None);
    cmd.rep = id(0x3ff);
    not_found(e.execute(&cmd));
    let mut cmd = reshape(good(), None);
    cmd.rep = REP_102G1;
    rejected_with(e.execute(&cmd), "no box");
    // Two outline vertices.
    invalid_at(e.execute(&reshape(vec![pt(0, 0), pt(G, 0)], None)), "/outline");
    // An off-grid vertex.
    let mut outline = good();
    outline[2] = pt(outline[2].x.0, outline[2].y.0 + 1);
    rejected_with(e.execute(&reshape(outline, None)), "/outline/2");
    // Two CPs with one index.
    invalid_at(
        e.execute(&reshape(
            good(),
            Some(vec![
                box_cp(2, pt(2 * G, 0), Dir4::Up),
                box_cp(2, pt(4 * G, 0), Dir4::Up),
            ]),
        )),
        "/cps/1/idx",
    );
    // A CP off the outline (on the grid, inside the box).
    invalid_at(
        e.execute(&reshape(
            good(),
            Some(vec![
                box_cp(2, pt(2 * G, 0), Dir4::Up),
                box_cp(6, pt(2 * G, 2 * G), Dir4::Down),
            ]),
        )),
        "/cps/1/pos",
    );
    // A CP off the grid (on the outline).
    rejected_with(
        e.execute(&reshape(
            good(),
            Some(vec![
                box_cp(6, pt(4 * G, 4 * G), Dir4::Down),
                box_cp(2, pt(2 * G + 1, 0), Dir4::Up),
            ]),
        )),
        "/cps/1/pos",
    );
    // `cps` that leave out CP 6, at which a connection on the page ends.
    rejected_with(
        e.execute(&reshape(good(), Some(vec![box_cp(2, pt(2 * G, 0), Dir4::Up)]))),
        "keep it in `cps`",
    );
    assert_eq!(e.model().rev(), rev);
    assert_eq!(lines(e.model()), before);
}
