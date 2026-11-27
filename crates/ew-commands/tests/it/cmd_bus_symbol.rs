// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `bus.place_ports` (doc 10 §2.7): a device of a part of one's own with two network interfaces (X1 of two ports, X2 of
//! one) drawn on a bus-topology page as one bus symbol; the boxes an earlier version drew replaced by it, the bus line
//! at a replaced socket keeping its end; a part's Save that adds a socket moving the line at a socket that moved; and
//! deleting one interface's function keeping the symbol for the other.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::*;
use ew_commands::Engine;
use ew_commands::commands::{
    Connect, DeleteSelection, EditPartCopy, KindName, PlaceBusSymbol, PortsTarget, TargetRef,
};
use ew_ids::Id;
use ew_model::Model;
use ew_model::fixture::*;
use ew_schema::{
    BoxCp, BoxSpec, BusSection, Connection, ConnectionKind, ContactKind, Device, Dir4, Extra, FnDef, FnRole,
    Function, LibCopy, LibDocument, Page, PageType, Part, PartRef, PartUse, Record, RepType, Representation,
    SymbolDef,
};
use ew_units::{Nm, Pt2};
use serde_json::json;

const BUS_PAGE: Id<Page> = id(0x7a0);
const PORT_DEF: Id<FnDef> = id(0x7a1);
const BUS_SYMBOL: Id<SymbolDef> = id(0x7a2);
const BOX_SYMBOL: Id<SymbolDef> = id(0x7a3);
const HEAD: Id<Part> = id(0x7a4);
const DEV: Id<Device> = id(0x7b0);
const OTHER: Id<Device> = id(0x7b1);
const X1: Id<Function> = id(0x7c0);
const X2: Id<Function> = id(0x7c1);
const PEER: Id<Function> = id(0x7c2);
const BOX_X1: Id<Representation> = id(0x7d0);
const BOX_X2: Id<Representation> = id(0x7d1);
const BOX_PEER: Id<Representation> = id(0x7d2);
const LINE_X2: Id<Connection> = id(0x7e0);

/// `k` steps of the page's grid, in nm.
fn g(k: i64) -> Nm {
    grid().at(k).expect("on the range")
}

fn at(x: i64, y: i64) -> Pt2 {
    Pt2::new(g(x), g(y))
}

fn head_station(ports_x1: u16) -> ew_schema::Part {
    let mut item = part(HEAD, "HS-2").item;
    item.sections.bus = Some(
        serde_json::from_value::<BusSection>(json!({"ports": [
            {"interface": "X1", "bus": "profinet", "role": "device", "ports": ports_x1},
            {"interface": "X2", "bus": "profinet", "role": "device"},
        ]}))
        .expect("a bus section"),
    );
    item
}

/// A box of `function` at `pos`, `w` × `h` steps, with its CPs `(idx, x, on top)`.
fn boxed(
    id: Id<Representation>,
    function: Id<Function>,
    pos: Pt2,
    w: i64,
    h: i64,
    cps: &[(u16, i64, bool)],
) -> Representation {
    let mut r = rep(id, BUS_PAGE, vec![function]);
    r.symbol = BOX_SYMBOL;
    r.rep_type = RepType::Topology;
    r.pos = pos;
    r.box_ = Some(BoxSpec {
        outline: vec![at(0, 0), at(w, 0), at(w, h), at(0, h)],
        cps: cps
            .iter()
            .map(|(idx, x, top)| BoxCp {
                idx: *idx,
                pos: if *top { at(*x, 0) } else { at(*x, h) },
                dir: if *top { Dir4::Up } else { Dir4::Down },
                name: None,
                extra: Extra::new(),
            })
            .collect(),
        inner: Vec::new(),
        slots: None,
        extra: Extra::new(),
    });
    r
}

fn bus_line(
    id: Id<Connection>,
    a: (Id<Function>, u16),
    b: (Id<Function>, u16),
    route: Vec<Pt2>,
) -> Connection {
    let mut c = connection(id, Some(BUS_PAGE), cp(a.0, a.1), cp(b.0, b.1));
    c.kind = serde_json::from_value::<ConnectionKind>(json!({"bus": {}})).expect("a bus line");
    c.route = route;
    c
}

/// The head station -A20 with its X1 and X2 port functions, a peer -A21, and, with `boxes`, the page as an earlier
/// version drew it: a box per interface, X2's socket joined to the peer's by a bus line.
fn model(boxes: bool) -> Model {
    let (p, mut records, _) = mini_project_parts();
    let mut page = page(BUS_PAGE, None, 105, "Bus");
    page.kind = PageType::BusTopology;
    records.push(Record::Page(page));
    let mut lib = library();
    let mut port_def = fndef(PORT_DEF, &["P1", "P2"]);
    port_def.item.contact_kind = ContactKind::BusPort;
    lib.push(LibDocument::Fndef(port_def));
    let mut bus = symbol(BUS_SYMBOL);
    bus.item.meta.key = Some(ident("ew.gen.bus"));
    bus.item.rep_type = RepType::Topology;
    lib.push(LibDocument::Symbol(bus));
    lib.push(LibDocument::Symbol(symbol(BOX_SYMBOL)));
    lib.push(LibDocument::Part(LibCopy {
        item: head_station(2),
        ..part(HEAD, "HS-2")
    }));
    let mut dev = device(DEV, "A", 20);
    dev.parts = vec![PartUse {
        part: PartRef::new(HEAD),
        qty: dec3("1"),
        extra: Extra::new(),
    }];
    records.push(Record::Device(dev));
    records.push(Record::Device(device(OTHER, "A", 21)));
    for (f, interface, role, d) in [
        (X1, Some("X1"), FnRole::Main, DEV),
        (X2, Some("X2"), FnRole::Aux, DEV),
        (PEER, None, FnRole::Main, OTHER),
    ] {
        let mut func = function(f, d, PORT_DEF, role);
        func.bus_port = interface.map(|i| {
            serde_json::from_value(
                json!({"interface": i, "bus": "profinet", "role": "device", "address": "tbd"}),
            )
            .expect("a port")
        });
        records.push(Record::Function(func));
    }
    if boxes {
        records.push(Record::Representation(boxed(
            BOX_X1,
            X1,
            at(10, 20),
            8,
            6,
            &[(0, 3, true), (1, 3, false)],
        )));
        records.push(Record::Representation(boxed(
            BOX_X2,
            X2,
            at(20, 20),
            8,
            6,
            &[(0, 2, true)],
        )));
        records.push(Record::Representation(boxed(
            BOX_PEER,
            PEER,
            at(20, 4),
            8,
            6,
            &[(0, 2, false)],
        )));
        records.push(Record::Connection(bus_line(
            LINE_X2,
            (X2, 0),
            (PEER, 0),
            vec![at(22, 20), at(22, 10)],
        )));
    }
    Model::from_records(p, records, lib).expect("the bus model is valid")
}

fn place(replace: Vec<Id<Representation>>) -> PlaceBusSymbol {
    PlaceBusSymbol {
        page: BUS_PAGE,
        symbol: BUS_SYMBOL,
        target: PortsTarget::Device { device: DEV },
        pos: at(10, 20),
        replace,
        copies: Vec::new(),
    }
}

/// Where the page draws function CP `(f, idx)`.
fn socket(e: &Engine, f: Id<Function>, idx: u16) -> Pt2 {
    let q = ew_derive::Q::new(e.model());
    ew_derive::cp_point(&q, f, idx, BUS_PAGE).expect("a drawn socket")
}

fn symbols(e: &Engine) -> Vec<Representation> {
    rows::<Representation>(e)
        .into_iter()
        .filter(|r| r.symbol == BUS_SYMBOL)
        .collect()
}

/// One representation of the bus symbol draws the device's port functions in its part's interface order, without a
/// box or a pin map: its sockets are derived (X1's two, X2's one).
#[test]
fn a_device_is_drawn_as_one_bus_symbol_of_its_port_functions() {
    let mut e = engine(model(false));
    let r = e.execute(&place(Vec::new())).expect("bus.place_ports");
    let drawn: Id<Representation> = primary(&r);
    let rep = row(&e, drawn);
    assert_eq!(rep.functions, vec![X1, X2]);
    assert_eq!((rep.box_.is_none(), rep.pin_map.is_empty()), (true, true));
    assert_eq!(rep.rep_type, RepType::Topology);
    assert_ne!(socket(&e, X1, 0), socket(&e, X1, 1));
    assert!(socket(&e, X2, 0).x > socket(&e, X1, 0).x);
    // Drawn once: a second symbol of the device on the page is refused in words.
    assert!(reason(e.execute(&place(Vec::new()))).contains("drawn on this page already"));
}

/// Draw as one symbol: the boxes an earlier version drew go in the same step, and the bus line at X2's socket keeps
/// its end there, its route starting at the new socket.
#[test]
fn the_symbol_replaces_the_boxes_and_a_bus_line_follows_its_socket() {
    let mut e = engine(model(true));
    e.execute(&place(vec![BOX_X1, BOX_X2]))
        .expect("bus.place_ports with replace");
    assert!(get::<Representation>(&e, BOX_X1).is_none() && get::<Representation>(&e, BOX_X2).is_none());
    assert_eq!(symbols(&e).len(), 1);
    let line = row(&e, LINE_X2);
    assert_eq!(
        (line.a.clone(), line.b.clone()),
        (cp(X2, 0), cp(PEER, 0)),
        "the line keeps its ends"
    );
    assert_eq!(
        line.route.first(),
        Some(&socket(&e, X2, 0)),
        "its route starts at the new socket"
    );
    assert_eq!(line.route.last(), Some(&at(22, 10)), "its other end stays");
    // A representation of another device is not the symbol's to replace.
    let mut e = engine(model(true));
    assert!(reason(e.execute(&place(vec![BOX_X1, BOX_PEER]))).contains("no drawing of"));
}

/// The part's Save that gives X1 a third port: its symbol is drawn anew from the part, P2 moves from the bottom edge
/// to the top one, and the bus line at P2 follows it.
#[test]
fn a_part_s_save_moves_the_line_at_a_socket_that_moved() {
    let mut e = engine(model(true));
    e.execute(&place(vec![BOX_X1, BOX_X2])).expect("bus.place_ports");
    let before = socket(&e, X1, 1);
    let r = e
        .execute(&Connect {
            a: cp_param(X1, 1),
            b: cp_param(PEER, 0),
            page: Some(BUS_PAGE),
            route: vec![before, Pt2::new(before.x, g(10)), at(22, 10)],
            kind: serde_json::from_value::<ConnectionKind>(json!({"bus": {}})).expect("a bus line"),
            computed: false,
            dot: false,
        })
        .expect("the line at P2");
    let line: Id<Connection> = created(&r, ew_schema::Kind::Connection)[0];
    // Bulk: previewed, then committed.
    let save = e
        .preview(&EditPartCopy {
            copy: HEAD,
            part: serde_json::to_value(head_station(3)).expect("json"),
            fork: Some(id(0x7f0)),
            copies: Vec::new(),
            redraw: Vec::new(),
        })
        .expect("part.edit_copy");
    e.commit(save).expect("committed");
    let after = socket(&e, X1, 1);
    assert_ne!(after, before, "P2 moved");
    assert_eq!(row(&e, line).route.first(), Some(&after));
}

/// Deleting X2's function takes it out of the symbol's functions: the symbol stays, drawing X1.
#[test]
fn deleting_one_interface_s_function_keeps_the_symbol_for_the_other() {
    let mut e = engine(model(false));
    let r = e.execute(&place(Vec::new())).expect("bus.place_ports");
    let drawn: Id<Representation> = primary(&r);
    e.execute(&DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Function,
            id: X2.cast(),
        }],
    })
    .expect("edit.delete");
    assert_eq!(row(&e, drawn).functions, vec![X1]);
}
