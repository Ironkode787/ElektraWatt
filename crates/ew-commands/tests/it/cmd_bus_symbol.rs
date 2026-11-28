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
    AssignModeParam, AssignPart, BusSymbolAt, Connect, DeleteSelection, EditPartCopy, KindName,
    PlaceBusSymbol, PlaceBusSymbols, PortsTarget, TargetRef, TransformReps,
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
    let (p, records, lib) = records_of(boxes);
    Model::from_records(p, records, lib).expect("the bus model is valid")
}

/// [`model`]'s records.
fn records_of(boxes: bool) -> (ew_schema::Project, Vec<Record>, Vec<LibDocument>) {
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
    (p, records, lib)
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

/// The item of his own bus symbol of HS-2 in the fixture's library.
const OWN_ITEM: u128 = 0x7f5;

/// His own bus symbol of HS-2 (X1 of two ports, X2 of one), made from the program's with X1 P1 two steps to the right
/// of where the program puts it; its copy as the project holds it.
fn own_symbol() -> LibCopy<SymbolDef> {
    let mut item = ew_derive::bus_symbol(&head_station(2), grid(), &[]);
    item.meta.key = Some(ident("user.hs-2-bus"));
    let mut cps: Vec<ew_schema::SymbolCp> = item.cps.iter().cloned().collect();
    let p1 = cps
        .iter_mut()
        .find(|c| c.interface.as_ref().map(|i| i.as_str()) == Some("X1") && c.fn_cp == 0)
        .expect("X1 P1");
    p1.pos = Pt2::new(Nm(p1.pos.x.0 + g(2).0), p1.pos.y);
    item.cps = ew_schema::KeyedList::from_vec(cps).expect("unique");
    let (library, own) = (id::<()>(0x9000), id::<()>(OWN_ITEM));
    LibCopy {
        id: ew_ids::v5::lib_copy_id(library.as_uuid(), own.as_uuid()),
        library_id: library.cast(),
        item_id: own.cast(),
        ..lib_copy(BUS_SYMBOL, item)
    }
}

/// HS-2 naming his own bus symbol, or the program's.
fn head_with(own: bool) -> serde_json::Value {
    let mut item = head_station(2);
    item.bus_symbol = own.then(|| ew_schema::LibRef {
        library: id(0x9000),
        item: id(OWN_ITEM),
        version_req: "*".into(),
        extra: Extra::new(),
    });
    serde_json::to_value(item).expect("json")
}

/// A bus line from `(f, idx)` to the peer, leaving its socket to row `y`.
fn line_from(e: &mut Engine, (f, idx): (Id<Function>, u16), y: i64) -> Id<Connection> {
    let start = socket(e, f, idx);
    let r = e
        .execute(&Connect {
            a: cp_param(f, idx),
            b: cp_param(PEER, 0),
            page: Some(BUS_PAGE),
            route: vec![start, Pt2::new(start.x, g(y)), at(22, y)],
            kind: serde_json::from_value::<ConnectionKind>(json!({"bus": {}})).expect("a bus line"),
            computed: false,
            dot: false,
        })
        .expect("a bus line");
    created::<Connection>(&r, ew_schema::Kind::Connection)[0]
}

/// His own symbol's copy held, -A20 drawn with the program's symbol, and a bus line at X1 P1: the line and where the
/// program puts X1 P1.
fn with_line() -> (Engine, Id<Connection>, Pt2) {
    let (p, records, mut lib) = records_of(false);
    lib.push(LibDocument::Symbol(own_symbol()));
    let mut e = engine(Model::from_records(p, records, lib).expect("valid"));
    e.execute(&place(Vec::new())).expect("bus.place_ports");
    let program = socket(&e, X1, 0);
    let line = line_from(&mut e, (X1, 0), 10);
    (e, line, program)
}

/// The part's Save of `part` over the copy the device uses (`copy`): the fixture library's is forked into the project
/// (item `0x7f0`), which the device uses afterwards; the project's own is changed in place.
fn save(e: &mut Engine, copy: &mut Id<Part>, part: serde_json::Value) {
    let fork: Id<Part> = id(0x7f0);
    let p = e
        .preview(&EditPartCopy {
            copy: *copy,
            part,
            fork: (*copy == HEAD).then(|| fork.cast()),
            copies: Vec::new(),
            redraw: Vec::new(),
        })
        .expect("part.edit_copy");
    e.commit(p).expect("committed");
    *copy = ew_ids::v5::lib_copy_id(e.model().project().id.as_uuid(), fork.as_uuid());
}

/// The commands see his own symbol where the page draws it: Done + Save moves the line at X1 P1 to his socket in one
/// undo step; a later Save that changes nothing of the sockets moves no line; a line is connected at his socket where
/// it is drawn; Use the program's symbol again moves the end back.
#[test]
fn the_commands_take_his_own_bus_symbol_where_the_page_draws_it() {
    let (mut e, line, program) = with_line();
    let mut copy = HEAD;
    save(&mut e, &mut copy, head_with(true));
    let his = socket(&e, X1, 0);
    assert_eq!(
        his,
        Pt2::new(Nm(program.x.0 + g(2).0), program.y),
        "the page draws his socket"
    );
    assert_eq!(row(&e, line).route.first(), Some(&his), "the line followed it");
    e.undo(&ew_commands::Origin::Gui).expect("undo");
    assert_eq!(row(&e, line).route.first(), Some(&program), "one undo step");
    e.redo(&ew_commands::Origin::Gui).expect("redo");
    assert_eq!(row(&e, line).route.first(), Some(&his));
    // A later Save that changes the description alone: no line moves.
    let mut described = head_with(true);
    described["description"] = json!({"en": "head station"});
    save(&mut e, &mut copy, described);
    assert_eq!(row(&e, line).route.first(), Some(&his));
    // Connect at his socket: the route's end is where the commands read the socket.
    let second = line_from(&mut e, (X1, 0), 12);
    assert_eq!(row(&e, second).route.first(), Some(&his));
    // Use the program's symbol again.
    save(&mut e, &mut copy, head_with(false));
    assert_eq!(
        row(&e, line).route.first(),
        Some(&program),
        "back at the program's socket"
    );
}

/// A bus symbol turned a quarter turns about its middle, as any symbol: its body's centre stays and the bus line at its
/// socket follows.
#[test]
fn a_bus_symbol_turned_keeps_its_centre() {
    let (mut e, line, _) = with_line();
    let drawn = symbols(&e)[0].id;
    let centre = |e: &Engine| {
        let q = ew_derive::Q::new(e.model());
        let r = (**q.reps().get(drawn).expect("the symbol")).clone();
        let (s, rep) = ew_derive::drawn(&q, &r).expect("drawn");
        let (lo, hi) = s.graphic_bounds().expect("a body");
        let (a, b) = (rep.orient.apply(lo), rep.orient.apply(hi));
        (
            rep.pos.x.0 + (a.x.0 + b.x.0) / 2,
            rep.pos.y.0 + (a.y.0 + b.y.0) / 2,
        )
    };
    let before = centre(&e);
    e.execute(&TransformReps {
        reps: vec![drawn],
        rotate: 1,
        mirror: false,
        variant: None,
    })
    .expect("turned");
    let after = centre(&e);
    let step = g(1).0;
    assert!(
        (after.0 - before.0).abs() <= step && (after.1 - before.1).abs() <= step,
        "{before:?} {after:?}"
    );
    assert_eq!(
        row(&e, line).route.first(),
        Some(&socket(&e, X1, 0)),
        "its line follows"
    );
}

/// `bus.place_ports` refuses as the tool does: a position off the grid, and a body over another device.
#[test]
fn place_ports_refuses_off_the_grid_and_over_another_device() {
    let mut e = engine(model(true));
    let mut off = place(vec![BOX_X1, BOX_X2]);
    off.pos = Pt2::new(Nm(off.pos.x.0 + 1), off.pos.y);
    assert!(!reason(e.execute(&off)).is_empty(), "off the grid");
    let mut over = place(vec![BOX_X1, BOX_X2]);
    over.pos = at(16, 2);
    let why = reason(e.execute(&over));
    assert!(why.contains("another device stands where"), "{why}");
    assert!(symbols(&e).is_empty(), "nothing placed");
}

/// `bus.place_symbols` is all or nothing: two devices whose new bodies would lie on each other are refused, naming the
/// second, and neither is drawn.
#[test]
fn place_symbols_is_all_or_nothing() {
    let (p, mut records, lib) = records_of(false);
    for r in records.iter_mut() {
        if let Record::Device(d) = r
            && d.id == OTHER
        {
            d.parts = vec![PartUse {
                part: PartRef::new(HEAD),
                qty: dec3("1"),
                extra: Extra::new(),
            }];
        }
    }
    let mut e = engine(Model::from_records(p, records, lib).expect("valid"));
    let both = PlaceBusSymbols {
        page: BUS_PAGE,
        symbol: BUS_SYMBOL,
        symbols: vec![
            BusSymbolAt {
                device: DEV,
                pos: at(10, 20),
                replace: Vec::new(),
            },
            BusSymbolAt {
                device: OTHER,
                pos: at(12, 21),
                replace: Vec::new(),
            },
        ],
        copies: Vec::new(),
    };
    let why = reason(e.execute(&both));
    assert!(
        why.contains("another device stands where") && why.contains("A21"),
        "{why}"
    );
    assert!(symbols(&e).is_empty(), "neither is drawn");
}

/// `part.assign` that gives -A20 another part of his own (X1 of three ports, no X2): the line at X1 P2 follows its
/// socket, from the bottom edge to the top one, and the line at X2, whose socket the new part has not, is said in the
/// outcome's notices.
#[test]
fn assigning_another_bus_part_moves_the_lines_and_says_a_socket_gone() {
    let other_part: Id<Part> = id(0x7a5);
    let (p, records, mut lib) = records_of(false);
    let mut item = part(other_part, "HS-3").item;
    item.sections.bus = Some(
        serde_json::from_value::<BusSection>(json!({"ports": [
            {"interface": "X1", "bus": "profinet", "role": "device", "ports": 3},
        ]}))
        .expect("a bus section"),
    );
    lib.push(LibDocument::Part(LibCopy {
        item,
        ..part(other_part, "HS-3")
    }));
    let mut e = engine(Model::from_records(p, records, lib).expect("valid"));
    e.execute(&place(Vec::new())).expect("bus.place_ports");
    let at_x1 = line_from(&mut e, (X1, 1), 30);
    let at_x2 = line_from(&mut e, (X2, 0), 12);
    let before = socket(&e, X1, 1);
    let p = e
        .preview(&AssignPart {
            devices: vec![DEV],
            part: other_part,
            copies: Vec::new(),
            mode: AssignModeParam::Replace,
            from: Some(HEAD),
            qty: None,
            variant: None,
        })
        .expect("part.assign");
    let r = e.commit(p).expect("committed");
    let now = socket(&e, X1, 1);
    assert_ne!(now, before, "three ports: P2 stands elsewhere");
    assert_eq!(row(&e, at_x1).route.first(), Some(&now), "its line follows");
    assert!(!r.outcome.notices.is_empty(), "the line at X2 is said");
    assert!(get::<Connection>(&e, at_x2).is_some());
}

/// A larger small data text on the Text styles page draws the derived bus symbol in the new size: the bus line at a
/// socket on its bottom edge follows the socket in the same step.
#[test]
fn a_text_style_change_moves_the_lines_with_the_sockets() {
    let mut e = engine(model(false));
    e.execute(&place(Vec::new())).expect("bus.place_ports");
    let line = line_from(&mut e, (X1, 1), 34);
    let before = socket(&e, X1, 1);
    let p = e
        .preview(&ew_commands::commands::SetProjectSettings {
            member: "text_styles".into(),
            value: json!({"data.small": {"size": 4_762_500}}),
        })
        .expect("project.settings.set");
    e.commit(p).expect("committed");
    let after = socket(&e, X1, 1);
    assert_ne!(after, before, "the bottom edge moved down");
    assert_eq!(row(&e, line).route.first(), Some(&after), "its line follows");
}

/// `device.place` refuses the bus symbol's copy in words that name the way: a device is drawn with it by
/// `bus.place_ports` alone, from its part.
#[test]
fn device_place_refuses_the_bus_symbol_and_names_bus_place_ports() {
    let mut e = engine(model(false));
    let place: ew_commands::commands::PlaceSymbol = serde_json::from_value(json!({
        "page": BUS_PAGE.to_string(),
        "symbol": BUS_SYMBOL.to_string(),
        "pos": [g(10).0, g(20).0],
        "target": {"existing_function": {"function": X1.to_string()}},
    }))
    .expect("device.place");
    let why = reason(e.execute(&place));
    assert!(why.contains("bus.place_ports"), "{why}");
    assert!(symbols(&e).is_empty(), "nothing placed");
}

/// A bus symbol larger than the frame's drawing area (an interface of four hundred ports) is refused with
/// its size and the area's, as the window says it of a box; nothing is placed.
#[test]
fn a_bus_symbol_larger_than_the_drawing_area_is_refused_with_both_sizes() {
    let (p, records, mut lib) = records_of(false);
    for d in lib.iter_mut() {
        if let LibDocument::Part(c) = d
            && c.id == HEAD
        {
            c.item = head_station(400);
        }
        // The page's frame: a drawing area of four columns and three rows of 40 mm.
        if let LibDocument::Form(f) = d {
            f.item.grid = Some(
                serde_json::from_value(json!({
                    "columns": {"count": 4, "width": {"num": 40_000_000, "den": 1}},
                    "rows": {"count": 3, "height": {"num": 40_000_000, "den": 1}},
                }))
                .expect("a frame grid"),
            );
        }
    }
    let mut e = engine(Model::from_records(p, records, lib).expect("valid"));
    let mut wide = place(Vec::new());
    wide.pos = at(0, 2);
    let why = reason(e.execute(&wide));
    assert!(
        why.contains("does not fit in the drawing area") && why.contains("(160 × 120 mm)"),
        "{why}"
    );
    assert!(symbols(&e).is_empty(), "nothing placed");
}
