// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Page points one nanometre apart that are one point to a reader (ADR-0033 amendment 4, the review of rc.5): on the
//! QET grid (`7_937_500/3` nm) a pin's point is its symbol's place plus the pin's offset, each rounded, and can lie
//! 1 nm beside the grid's own number of its position. The commands that compare page points read them settled
//! (`geometry::settled`): a symbol dropped on a wire whose line's number lies 1 nm beside its pins' is wired in, and
//! an autoconnect hint between two pins on one grid line is a straight wire, not an unrouted one.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::fixture::*;
use common::*;
use ew_commands::Engine;
use ew_commands::commands::{Connect, ConnectHint, PlaceSymbol, PlaceTarget};
use ew_commands::geometry::on_grid;
use ew_ids::Id;
use ew_model::Model;
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, FnRole, Function, Kind, Record, Representation,
};
use ew_units::{Nm, Orient2, Pt2};

/// Two grid steps are 5_291_666.67 nm: the grid's number is 5_291_667, and a pin drawn at 5_291_666 lies 1 nm
/// beside it.
const Y_PIN: i64 = 5_291_666;
const Y_GRID: i64 = 5_291_667;

/// The wire a model holds as a record ([`model_with`]).
const WIRE: Id<Connection> = id(0x2000);

/// Two placed devices whose second pin (10 mm below the insertion point) is drawn at `y = Y_PIN`, at x = 0 and
/// x = 8G. The positions are in the records, not placed by a command (a command would refuse them).
fn model() -> (Model, Id<Function>, Id<Function>) {
    model_with(None)
}

/// [`model`] with, given its route, a wire [`WIRE`] from the first device's second pin to the second's, as a record.
fn model_with(route: Option<Vec<Pt2>>) -> (Model, Id<Function>, Id<Function>) {
    let (p, mut records, _) = mini_golden_parts();
    let mut fns = Vec::new();
    for (n, x) in [(0u128, 0i64), (1, 8 * G)] {
        let (dev, f, r): (Id<Device>, Id<Function>, Id<Representation>) =
            (id(0x1000 + n), id(0x1100 + n), id(0x1200 + n));
        records.push(Record::Device(device(dev, "K", 1 + n as u32)));
        records.push(Record::Function(function(f, dev, FNDEF_CONTACT, FnRole::Main)));
        let mut rep = rep(r, PAGE_102, vec![f]);
        rep.symbol = SYM_TWO;
        rep.pos = pt(x, Y_PIN - MM10);
        records.push(Record::Representation(rep));
        fns.push(f);
    }
    if let Some(route) = route {
        let mut wire = connection(WIRE, Some(PAGE_102), cp(fns[0], 1), cp(fns[1], 1));
        wire.route = route;
        records.push(Record::Connection(wire));
    }
    let model = Model::from_records(p, records, library()).expect("valid model");
    assert!(!on_grid(grid(), Nm(Y_PIN)) && on_grid(grid(), Nm(Y_GRID)));
    (model, fns[0], fns[1])
}

/// A two-pin symbol placed on its side at `(4G, Y_GRID)`: its pins lie on the line `y = Y_GRID`, 10 mm apart.
fn on_its_side(connect: Vec<ConnectHint>) -> PlaceSymbol {
    PlaceSymbol {
        copies: Vec::new(),
        page: PAGE_102,
        symbol: SYM_TWO,
        pos: pt(4 * G, Y_GRID),
        orient: Orient2::R90,
        rep_type: None,
        variant: 0,
        target: PlaceTarget::NewDevice {
            letter: ident("F"),
            fndef: None,
            kind: None,
        },
        connect,
        into: None,
        replace: false,
    }
}

/// The new representation's pins on the page, by the x they stand at.
fn new_pins(e: &Engine, r: &ew_commands::Receipt) -> Vec<(Id<Function>, u16, Pt2)> {
    let rep: Id<Representation> = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Representation)
        .map(|k| typed(*k))
        .expect("the representation");
    let rep = row(e, rep);
    let f = rep.functions[0];
    let mut pins: Vec<(Id<Function>, u16, Pt2)> = [0u16, 1]
        .into_iter()
        .map(|idx| {
            let at = ew_derive::cp_point(&ew_derive::Q::new(e.model()), f, idx, PAGE_102).expect("drawn");
            (f, idx, at)
        })
        .collect();
    pins.sort_by_key(|p| p.2.x);
    pins
}

#[test]
fn a_symbol_dropped_on_a_wire_one_nm_beside_its_pins_is_wired_in() {
    let (model, fa, fb) = model();
    let mut e = engine(model);
    let r = e
        .execute(&Connect {
            a: cp_param(fa, 1),
            b: cp_param(fb, 1),
            page: Some(PAGE_102),
            route: vec![pt(0, Y_PIN), pt(8 * G, Y_PIN)],
            kind: ConnectionKind::Wire,
            computed: false,
            dot: false,
        })
        .expect("the wire");
    let w: Id<Connection> = typed(r.outcome.primary.expect("primary"));
    // The symbol's pins stand on the grid's number of the wire's line, 1 nm beside the wire's own.
    let r = e.execute(&on_its_side(Vec::new())).expect("placed");
    let pins = new_pins(&e, &r);
    assert!(pins.iter().all(|p| p.2.y == Nm(Y_GRID)), "{pins:?}");
    assert!(
        Nm(0) < pins[0].2.x && pins[1].2.x < Nm(8 * G),
        "between the wire's ends: {pins:?}"
    );
    // The wire ends at the pin met first and steps the 1 nm onto it; a second wire leaves the other pin.
    let first = row(&e, w);
    assert_eq!(first.b, Endpoint::Cp(pins[0].0, pins[0].1));
    assert_eq!(
        first.route,
        vec![pt(0, Y_PIN), pt(pins[0].2.x.0, Y_PIN), pins[0].2]
    );
    let second: Id<Connection> = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Connection)
        .map(|k| typed(*k))
        .expect("the second part of the wire");
    let second = row(&e, second);
    assert_eq!(second.a, Endpoint::Cp(pins[1].0, pins[1].1));
    assert_eq!(second.b, Endpoint::Cp(fb, 1));
    assert_eq!(
        second.route,
        vec![pins[1].2, pt(pins[1].2.x.0, Y_PIN), pt(8 * G, Y_PIN)]
    );
}

#[test]
fn an_autoconnect_hint_between_pins_one_nm_apart_is_a_straight_wire() {
    let (model, fa, _) = model();
    let mut e = engine(model);
    // The new symbol's first pin faces the first device's second pin along one grid line: `Y_GRID` against `Y_PIN`.
    let hint = ConnectHint {
        route: Vec::new(),
        cp: 0,
        to: cp_param(fa, 1),
    };
    let r = e.execute(&on_its_side(vec![hint])).expect("placed with its wire");
    let wire: Id<Connection> = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Connection)
        .map(|k| typed(*k))
        .expect("the hint's wire");
    let route = row(&e, wire).route.clone();
    assert!(!route.is_empty(), "a route, not an unrouted wire");
    let pins = new_pins(&e, &r);
    let own = pins.iter().find(|p| p.1 == 0).expect("the first pin").2;
    assert_eq!((route[0], route[route.len() - 1]), (own, pt(0, Y_PIN)));
    assert!(
        route.windows(2).all(|s| s[0].x == s[1].x || s[0].y == s[1].y),
        "level and upright stretches only: {route:?}"
    );
    assert!(
        route.iter().all(|p| p.y == Nm(Y_GRID) || p.y == Nm(Y_PIN)),
        "a straight run with its step of 1 nm: {route:?}"
    );
}

/// A wire an import stored with a stretch 1 nm out of square (rc.5's private corpus: a conductor from a corner on the
/// grid's number to a pin 1 nm beside it, one straight stretch): a symbol dropped on that stretch was found on it as a
/// reader sees it, then the stretch had no point to cut it at and the whole import was refused. The stretch runs on
/// its line with a step of 1 nm at the pin, and the symbol is wired in.
#[test]
fn a_symbol_on_a_stretch_one_nm_out_of_square_is_wired_in() {
    // From the first pin up 1 nm onto the grid's line, then one stretch to the second pin, 1 nm off that line at its
    // end: a diagonal of 1 nm across eight grid steps.
    let (model, _, fb) = model_with(Some(vec![pt(0, Y_PIN), pt(0, Y_GRID), pt(8 * G, Y_PIN)]));
    let mut e = engine(model);
    let r = e
        .execute(&on_its_side(Vec::new()))
        .expect("placed, the wire split");
    let pins = new_pins(&e, &r);
    let first = row(&e, WIRE);
    assert_eq!(first.b, Endpoint::Cp(pins[0].0, pins[0].1));
    let square = |route: &[Pt2]| route.windows(2).all(|s| s[0].x == s[1].x || s[0].y == s[1].y);
    assert!(square(&first.route), "{:?}", first.route);
    let second: Id<Connection> = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Connection)
        .map(|k| typed(*k))
        .expect("the second part of the wire");
    let second = row(&e, second);
    assert_eq!(
        (second.a.clone(), second.b.clone()),
        (Endpoint::Cp(pins[1].0, pins[1].1), Endpoint::Cp(fb, 1))
    );
    assert!(square(&second.route), "{:?}", second.route);
    assert_eq!(second.route.last(), Some(&pt(8 * G, Y_PIN)));
}
