// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One number for a grid position (docs/design/06a-drafting.md §2.1, §9.3 row 8): on the QET grid (`7_937_500/3` nm) a
//! symbol's place plus a pin's offset, each rounded, can add up to 1 nm beside the grid's own number of the pin's
//! position, and the pin's point is that number (`Grid::at`). So the commands compare equal numbers: a wire between
//! two such pins on one grid line is straight, a symbol dropped on it is wired in, and an autoconnect hint between two
//! pins on one grid line is a straight wire, not an unrouted one. A drawing of an older build, its wire stored on the
//! pins' place plus offset, is read with the same numbers, and a command that writes the wire writes them.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::Engine;
use ew_commands::commands::{Connect, ConnectHint, EndpointParam, PlaceSymbol, PlaceTarget};
use ew_commands::geometry::on_grid;
use ew_ids::Id;
use ew_model::Model;
use ew_model::fixture::*;
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, FnRole, Function, Kind, Record, Representation,
};
use ew_units::{Nm, Orient2, Pt2};

/// Two grid steps are 5_291_666.67 nm: the grid's number is 5_291_667, and a pin whose place and offset add up to
/// 5_291_666 lies 1 nm beside it.
const Y_PIN: i64 = 5_291_666;
const Y_GRID: i64 = 5_291_667;

/// Two placed devices whose second pin (10 mm below the insertion point) adds up to `y = Y_PIN`, at x = 0 and
/// x = 8G. The positions are in the records, not placed by a command (a command would refuse them).
fn model() -> (Model, Id<Function>, Id<Function>) {
    model_with(None)
}

/// The wire an older build stored between the two devices' second pins ([`older`]).
const OLD: Id<Connection> = id(0x2000);

/// [`model`] as an older build stored it, with the wire [`OLD`] between the two second pins on their place plus
/// offset, `y = Y_PIN`: 1 nm beside the grid's number its pins have now.
fn older() -> (Model, Id<Function>, Id<Function>) {
    model_with(Some(vec![pt(0, Y_PIN), pt(8 * G, Y_PIN)]))
}

/// [`model`] with, given its route, the wire [`OLD`] as a record.
fn model_with(route: Option<Vec<Pt2>>) -> (Model, Id<Function>, Id<Function>) {
    let (p, mut records, _) = mini_project_parts();
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
        let mut wire = connection(OLD, Some(PAGE_102), cp(fns[0], 1), cp(fns[1], 1));
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

/// The pins' points: the grid's own number of their position, as ew-derive and the commands give them.
#[test]
fn a_pin_s_point_is_the_grid_s_own_number_of_its_position() {
    let (model, fa, fb) = model();
    let q = ew_derive::Q::new(&model);
    for f in [fa, fb] {
        let at = ew_derive::cp_point(&q, f, 1, PAGE_102).expect("drawn");
        assert_eq!(at.y, grid().at(2).expect("in range"), "{at:?}");
        assert_eq!(at.y, Nm(Y_GRID));
    }
    // The commands give the same point: a straight wire on the grid's line ends on both pins.
    let mut e = engine(model);
    e.execute(&Connect {
        a: cp_param(fa, 1),
        b: cp_param(fb, 1),
        page: Some(PAGE_102),
        route: vec![pt(0, Y_GRID), pt(8 * G, Y_GRID)],
        kind: ConnectionKind::Wire,
        computed: false,
        dot: false,
    })
    .expect("the wire ends on the pins' points");
}

#[test]
fn a_symbol_dropped_on_a_wire_between_such_pins_is_wired_in() {
    let (model, fa, fb) = model();
    let mut e = engine(model);
    let r = e
        .execute(&Connect {
            a: cp_param(fa, 1),
            b: cp_param(fb, 1),
            page: Some(PAGE_102),
            route: vec![pt(0, Y_GRID), pt(8 * G, Y_GRID)],
            kind: ConnectionKind::Wire,
            computed: false,
            dot: false,
        })
        .expect("the wire");
    let w: Id<Connection> = typed(r.outcome.primary.expect("primary"));
    let r = e.execute(&on_its_side(Vec::new())).expect("placed");
    let pins = new_pins(&e, &r);
    assert!(pins.iter().all(|p| p.2.y == Nm(Y_GRID)), "{pins:?}");
    assert!(
        Nm(0) < pins[0].2.x && pins[1].2.x < Nm(8 * G),
        "between the wire's ends: {pins:?}"
    );
    // The wire ends at the pin met first; a second wire leaves the other pin.
    let first = row(&e, w);
    assert_eq!(first.b, Endpoint::Cp(pins[0].0, pins[0].1));
    assert_eq!(first.route, vec![pt(0, Y_GRID), pins[0].2]);
    let second: Id<Connection> = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Connection)
        .map(|k| typed(*k))
        .expect("the second part of the wire");
    let second = row(&e, second);
    assert_eq!(second.a, Endpoint::Cp(pins[1].0, pins[1].1));
    assert_eq!(second.b, Endpoint::Cp(fb, 1));
    assert_eq!(second.route, vec![pins[1].2, pt(8 * G, Y_GRID)]);
}

#[test]
fn an_autoconnect_hint_between_such_pins_is_a_straight_wire() {
    let (model, fa, _) = model();
    let mut e = engine(model);
    // The new symbol's first pin faces the first device's second pin along one grid line.
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
    assert_eq!(route, vec![own, pt(0, Y_GRID)], "one straight stretch");
}

/// A symbol dropped on the wire of an older drawing is wired in: the commands read the wire on the grid's number of
/// its line, and the two parts they write carry it.
#[test]
fn a_symbol_dropped_on_an_older_drawing_s_wire_is_wired_in() {
    let (model, _, fb) = older();
    let mut e = engine(model);
    let r = e.execute(&on_its_side(Vec::new())).expect("placed");
    let pins = new_pins(&e, &r);
    let first = row(&e, OLD);
    assert_eq!(first.b, Endpoint::Cp(pins[0].0, pins[0].1));
    assert_eq!(first.route, vec![pt(0, Y_GRID), pins[0].2]);
    let second: Id<Connection> = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Connection)
        .map(|k| typed(*k))
        .expect("the second part of the wire");
    let second = row(&e, second);
    assert_eq!(second.b, Endpoint::Cp(fb, 1));
    assert_eq!(second.route, vec![pins[1].2, pt(8 * G, Y_GRID)]);
}

/// A wire drawn to end on the wire of an older drawing joins it with a junction on the grid's number of its line.
#[test]
fn a_wire_ended_on_an_older_drawing_s_wire_joins_it() {
    let (model, fa, _) = older();
    let mut e = engine(model);
    let at = pt(4 * G, Y_GRID);
    let r = e
        .execute(&Connect {
            a: EndpointParam::OnConnection { connection: OLD, at },
            b: cp_param(fa, 0),
            page: None,
            route: Vec::new(),
            kind: ConnectionKind::Wire,
            computed: false,
            dot: false,
        })
        .expect("the wire joins the stored one");
    let junction: Id<ew_schema::Junction> = r
        .created
        .iter()
        .find(|k| k.0 == Kind::Junction)
        .map(|k| typed(*k))
        .expect("a junction");
    assert_eq!(row(&e, junction).at, at);
    assert_eq!(row(&e, OLD).route, vec![pt(0, Y_GRID), at]);
}

/// A command that does not write the wire of an older drawing leaves its numbers as stored.
#[test]
fn a_command_that_does_not_touch_an_older_wire_keeps_its_numbers() {
    let (model, _, _) = older();
    let mut e = engine(model);
    let mut away = on_its_side(Vec::new());
    away.pos = pt(4 * G, Y_GRID + 6 * G);
    e.execute(&away).expect("placed away from the wire");
    assert_eq!(row(&e, OLD).route, vec![pt(0, Y_PIN), pt(8 * G, Y_PIN)]);
}
