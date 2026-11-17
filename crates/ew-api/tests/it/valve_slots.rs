// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F15 of the rc.1 test, on the starter parts: a valve terminal is drawn as its valve positions, a cell is filled where
//! it is, a valve dragged onto another cell moves there or swaps, a device that is no valve for a terminal stays its own,
//! and a terminal drawn before as a free box is redrawn as its positions with its valves and their tubes.

use ew_api::output::ew_dl::ew_units::{Grid, Nm};
use ew_api::output::{PageTables, SlotCell};
use ew_api::{AnyCommand, Session};
use ew_ids::{Id, Uuid};
use ew_schema::Device;
use serde_json::{Value, json};

use crate::common::{library_part, new_project};

/// Runs a bulk command as the window does: its preview, then the commit.
fn bulk(s: &mut Session, name: &str, params: Value) -> Result<(), String> {
    let preview = s
        .preview(&AnyCommand {
            name: name.into(),
            params,
        })
        .map_err(|e| format!("{e}"))?;
    s.commit(preview).map(|_| ()).map_err(|e| format!("{e}"))
}

fn run(s: &mut Session, name: &str, params: Value) -> Result<(), String> {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .map(|_| ())
    .map_err(|e| format!("{e}"))
}

fn device(s: &Session, id: Uuid) -> std::sync::Arc<Device> {
    s.query()
        .devices()
        .get(Id::from_uuid(id))
        .cloned()
        .expect("the device")
}

/// The starter symbol `key` in the edition of the fluid page's grid.
fn edition(s: &Session, key: &str) -> String {
    let page = fluid_page(s);
    let qet = s.query().pages().get(Id::from_uuid(page)).map(|p| p.grid) == Some(Grid::QET_10PX);
    if qet {
        format!("{key}@qet10")
    } else {
        key.to_owned()
    }
}

/// The project with a fluid page 201.
fn project(seed: u64) -> Session {
    let mut s = new_project(seed);
    run(&mut s, "page.create", json!({"kind": "fluid", "number": 201})).expect("the fluid page");
    s
}

fn fluid_page(s: &Session) -> Uuid {
    s.page_tables("201").expect("page 201").page
}

fn tables(s: &Session) -> PageTables {
    s.page_tables("201").expect("page 201")
}

/// The devices made by `f`.
fn new_devices(s: &mut Session, f: impl FnOnce(&mut Session)) -> Vec<Uuid> {
    let before: Vec<Uuid> = s.query().devices().iter().map(|(id, _)| id.as_uuid()).collect();
    f(s);
    s.query()
        .devices()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .filter(|id| !before.contains(id))
        .collect()
}

/// Places the starter terminal drawn as its positions with its top-left corner near `at`; its ID.
fn place_terminal(s: &mut Session, at: (i64, i64)) -> Uuid {
    let part = library_part(s, "EW-GEN-VTM-1");
    let key = edition(s, "ew.gen.blackbox");
    let copies = s
        .starter_embed_part(part, &key)
        .expect("closure")
        .unwrap_or(json!({"copies": []}));
    let symbol = s.starter_symbol_copy(&key).expect("the black box");
    let page = fluid_page(s);
    let count = s.part_positions(part).expect("the terminal's positions");
    let (pos, b) = s.slot_box(page, at, count).expect("a box");
    let made = new_devices(s, |s| {
        run(
            s,
            "box.black",
            json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [pos.0, pos.1], "box": b,
            "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "K"}},
            "copies": copies["copies"]}),
        )
        .expect("the terminal");
    });
    made[0]
}

/// The cells of the terminal's box on page 201.
fn cells(s: &Session, terminal: Uuid) -> Vec<SlotCell> {
    tables(s)
        .reps
        .iter()
        .find(|r| r.device == Some(terminal) && r.black_box.is_some())
        .and_then(|r| r.black_box.clone())
        .map(|b| b.cells)
        .expect("the terminal's box")
}

fn centre(c: &SlotCell) -> (i64, i64) {
    ((c.rect[0] + c.rect[2]) / 2, (c.rect[1] + c.rect[3]) / 2)
}

/// Places part `number`'s main function with its symbol `symbol` at `at`, into `terminal`; the new device.
fn place_into(
    s: &mut Session,
    number: &str,
    symbol: &str,
    at: (i64, i64),
    terminal: Uuid,
) -> Result<Uuid, String> {
    let part = library_part(s, number);
    let key = edition(s, symbol);
    let copies = s
        .starter_embed_part(part, &key)
        .expect("closure")
        .unwrap_or(json!({"copies": []}));
    let symbol = s.starter_symbol_copy(&key).expect("the symbol");
    let page = fluid_page(s);
    let snapped = s.slot_box(page, at, 1).expect("a point").0;
    let mut out = Err(String::new());
    let made = new_devices(s, |s| {
        out = run(
            s,
            "device.place",
            json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [snapped.0, snapped.1],
            "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "Q"}},
            "copies": copies["copies"], "into": terminal.to_string()}),
        );
    });
    out.map(|()| made[0])
}

/// The symbol of `device` on page 201.
fn rep_of(s: &Session, device: Uuid) -> ew_api::output::TableRep {
    tables(s)
        .reps
        .into_iter()
        .find(|r| r.device == Some(device) && r.black_box.is_none())
        .expect("the symbol")
}

/// F15: placed, the terminal is one cell per position; a valve placed in cell 3 is the valve at position 3 and stands
/// in that cell; dragged to cell 1 it moves there, dragged onto a held cell the two swap; a stand-alone valve placed in
/// the box stays a valve of its own, and Properties' seat words say so.
#[test]
fn a_terminal_is_drawn_as_its_positions_and_filled_where_its_cells_are() {
    let mut s = project(181);
    let page = fluid_page(&s);
    let vt = place_terminal(&mut s, (60_000_000, 60_000_000));
    let c = cells(&s, vt);
    assert_eq!(c.len(), 4, "the starter terminal's four positions");
    assert!(c.iter().all(|x| x.holder.is_none()), "all empty");
    assert!(c.windows(2).all(|w| w[0].rect[2] == w[1].rect[0]), "in a row");

    let v = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[2]),
        vt,
    )
    .expect("placed");
    assert_eq!(device(&s, v).parent.map(|p| p.as_uuid()), Some(vt));
    assert_eq!(device(&s, v).position, Some(3), "the cell it was placed in");
    let c = cells(&s, vt);
    assert_eq!(c[2].holder, Some(v));
    let r = rep_of(&s, v);
    assert!(
        c[2].rect[0] < r.pos.0 && r.pos.0 < c[2].rect[2],
        "its symbol stands in the cell"
    );

    // Dragged to cell 1, by grid steps as the Select tool drags.
    let move_to = |s: &mut Session, rep: Uuid, from: (i64, i64), to: (i64, i64)| {
        let grid = s
            .query()
            .pages()
            .get(Id::from_uuid(page))
            .map(|p| p.grid)
            .expect("grid");
        let k = |a: i64, b: i64| grid.snap(Nm(b)) - grid.snap(Nm(a));
        run(
            s,
            "rep.move",
            json!({"reps": [rep.to_string()], "steps": [k(from.0, to.0), k(from.1, to.1)], "into": vt.to_string()}),
        )
    };
    move_to(&mut s, r.id, centre(&c[2]), centre(&c[0])).expect("moved");
    assert_eq!(device(&s, v).position, Some(1), "dragged to cell 1");

    let w = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[1]),
        vt,
    )
    .expect("placed");
    assert_eq!(device(&s, w).position, Some(2));
    let c = cells(&s, vt);
    let r = rep_of(&s, v);
    move_to(&mut s, r.id, centre(&c[0]), centre(&c[1])).expect("swapped");
    assert_eq!(
        (device(&s, v).position, device(&s, w).position),
        (Some(2), Some(1)),
        "the two swap"
    );
    let c = cells(&s, vt);
    assert_eq!((c[0].holder, c[1].holder), (Some(w), Some(v)));

    // A new valve onto a held cell is refused in words; with replace it takes the cell.
    let refused = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[0]),
        vt,
    );
    let message = refused.expect_err("held");
    assert!(message.contains("already holds"), "{message}");
    assert!(!message.contains("push insert"), "{message}");

    // A stand-alone valve (a coil of its own to wire) stays a valve of its own.
    let own = place_into(
        &mut s,
        "EW-GEN-VLV-1",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[3]),
        vt,
    )
    .expect("placed");
    assert_eq!(device(&s, own).parent, None, "not taken as a position");
    assert!(matches!(
        s.terminal_seat(own),
        Some(ew_api::output::TerminalSeat::Own { .. })
    ));
    assert!(cells(&s, vt)[3].holder.is_none());
}

/// F15: a terminal drawn before as a free box keeps it until Draw as slots: then its box is its cells, its valve drawn
/// elsewhere on the page stands in its cell, and the valve's tube follows it.
#[test]
fn a_free_box_is_redrawn_as_its_positions_with_its_valves_and_tubes() {
    let mut s = project(182);
    let page = fluid_page(&s);
    let part = library_part(&s, "EW-GEN-VTM-1");
    let key = edition(&s, "ew.gen.blackbox");
    let copies = s
        .starter_embed_part(part, &key)
        .expect("closure")
        .unwrap_or(json!({"copies": []}));
    let symbol = s.starter_symbol_copy(&key).expect("the black box");
    let (pos, _) = s.slot_box(page, (60_000_000, 40_000_000), 1).expect("a point");
    let grid = s
        .query()
        .pages()
        .get(Id::from_uuid(page))
        .map(|p| p.grid)
        .expect("grid");
    let at = |k: i64| grid.at(k).expect("a grid point").0;
    let made = new_devices(&mut s, |s| {
        run(
            s,
            "box.black",
            json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [pos.0, pos.1],
            "box": {"outline": [[0, 0], [at(40), 0], [at(40), at(20)], [0, at(20)]]},
            "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "K"}},
            "copies": copies["copies"]}),
        )
        .expect("a free box");
    });
    let vt = made[0];
    let free = tables(&s)
        .reps
        .into_iter()
        .find(|r| r.device == Some(vt) && r.black_box.is_some())
        .expect("the free box");
    assert!(
        free.black_box.as_ref().is_some_and(|b| b.cells.is_empty()),
        "a free box shows no cells"
    );
    // A valve of the terminal drawn below the box, with a tube from its port 2 to a cylinder.
    let v = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        (pos.0, pos.1 + at(40)),
        vt,
    )
    .expect("placed");
    assert_eq!(
        device(&s, v).position,
        Some(1),
        "the first free position of a free box"
    );
    let cyl = place_into(
        &mut s,
        "EW-GEN-CYL-1",
        "ew.iso.cylinder-double-acting",
        (pos.0 + at(60), pos.1 + at(40)),
        vt,
    )
    .expect("placed");
    assert_eq!(device(&s, cyl).parent, None);
    let t = tables(&s);
    let port = |dev: Uuid, d: &str| {
        t.cps
            .iter()
            .find(|c| {
                c.label.ends_with(&format!(":{d}"))
                    && t.reps.iter().any(|r| r.id == c.rep && r.device == Some(dev))
            })
            .map(|c| (c.function, c.idx))
            .expect("the port")
    };
    let (a, b) = (port(v, "2"), port(cyl, "A"));
    run(
        &mut s,
        "connection.draw",
        json!({"page": page.to_string(), "kind": {"fluid": {}},
        "a": {"cp": {"function": a.0.to_string(), "idx": a.1}}, "b": {"cp": {"function": b.0.to_string(), "idx": b.1}}}),
    )
    .expect("the tube");
    bulk(
        &mut s,
        "valve_terminal.draw_slots",
        json!({"rep": free.id.to_string()}),
    )
    .expect("drawn as slots");
    let c = cells(&s, vt);
    assert_eq!(c.len(), 4);
    let r = rep_of(&s, v);
    assert!(
        c[0].rect[0] < r.pos.0 && r.pos.0 < c[0].rect[2],
        "the valve stands in cell 1"
    );
    let t = tables(&s);
    let tube = t
        .connections
        .iter()
        .find(|x| x.kind == "fluid")
        .expect("the tube");
    let route = &tube.polyline;
    let p2 = t
        .cps
        .iter()
        .find(|x| x.function == a.0 && x.idx == a.1)
        .map(|x| x.at)
        .expect("port 2");
    assert!(
        route.first() == Some(&p2) || route.last() == Some(&p2),
        "the tube follows its valve: {route:?} {p2:?}"
    );
    // A second time is refused in words.
    let again = bulk(
        &mut s,
        "valve_terminal.draw_slots",
        json!({"rep": free.id.to_string()}),
    );
    assert!(
        again
            .expect_err("refused")
            .contains("drawn as its positions already")
    );
}

/// Places starter terminal part `number` drawn as its positions near `at`; its ID.
fn place_terminal_of(s: &mut Session, number: &str, at: (i64, i64)) -> Uuid {
    let part = library_part(s, number);
    let key = edition(s, "ew.gen.blackbox");
    let copies = s
        .starter_embed_part(part, &key)
        .expect("closure")
        .unwrap_or(json!({"copies": []}));
    let symbol = s.starter_symbol_copy(&key).expect("the black box");
    let page = fluid_page(s);
    let count = s.part_positions(part).expect("the terminal's positions");
    let (pos, b) = s.slot_box(page, at, count).expect("a box");
    let made = new_devices(s, |s| {
        run(
            s,
            "box.black",
            json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [pos.0, pos.1], "box": b,
            "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "K"}},
            "copies": copies["copies"]}),
        )
        .expect("the terminal");
    });
    made[0]
}

/// F15: the two kinds are said apart, and a terminal valve's solenoid says where it is driven from: inside a terminal
/// with a bus node, or through a pin of a multipin terminal's plug (solenoid 14 of position 2 on pin 3), never drawn.
#[test]
fn the_two_kinds_of_terminal_drive_their_valves_solenoids_apart() {
    let mut s = project(183);
    let bus = place_terminal_of(&mut s, "EW-GEN-VTM-1", (60_000_000, 40_000_000));
    assert!(matches!(
        s.terminal_kind(bus),
        Some(ew_api::output::TerminalKind::BusNode { .. })
    ));
    let c = cells(&s, bus);
    let v = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[0]),
        bus,
    )
    .expect("placed");
    let sol = s.valve_solenoids(v);
    assert!(
        matches!(
            sol.first().and_then(|x| x.seat.clone()),
            Some(ew_api::output::SolenoidSeat::Terminal { .. })
        ),
        "{sol:?}"
    );
    let multi = place_terminal_of(&mut s, "EW-GEN-VTP-1", (60_000_000, 140_000_000));
    assert_eq!(
        s.terminal_kind(multi),
        Some(ew_api::output::TerminalKind::Multipin)
    );
    let c = cells(&s, multi);
    assert_eq!(c.len(), 8, "the multipin terminal's eight positions");
    let w = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[1]),
        multi,
    )
    .expect("placed");
    assert_eq!(
        device(&s, w).position,
        Some(2),
        "a multipin terminal's cell takes its valve too"
    );
    match s.valve_solenoids(w).first().and_then(|x| x.seat.clone()) {
        Some(ew_api::output::SolenoidSeat::Multipin { pin, plug, .. }) => {
            assert_eq!(pin, "3", "solenoid 14 of position 2 on pin 3");
            assert_eq!(plug, None, "the plug is drawn on no electrical page yet");
        }
        other => panic!("{other:?}"),
    }
}

/// Draws `function` (a black-box function: a card, a plug) on page `page` as a box with its CPs in a row on its top edge,
/// one grid step apart, its top-left corner at grid point (`kx`, `ky`); the function's CPs by designation.
fn draw_box(s: &mut Session, page: Uuid, function: Uuid, kx: i64, ky: i64) -> Vec<(String, u16)> {
    let q = s.query();
    let grid = q
        .pages()
        .get(Id::from_uuid(page))
        .map(|p| p.grid)
        .expect("the page");
    let cps: Vec<(String, u16)> = q
        .get::<ew_derive::FunctionCps>(Id::from_uuid(function))
        .iter()
        .map(|c| (c.designation.clone(), c.idx))
        .collect();
    drop(q);
    let at = |k: i64| grid.at(k).expect("a grid point").0;
    let n = i64::try_from(cps.len()).expect("a count");
    let key = edition(s, "ew.gen.blackbox");
    let symbol = s.starter_symbol_copy(&key).expect("the black box");
    let box_cps: Vec<Value> = cps
        .iter()
        .zip(1i64..)
        .map(|((_, idx), k)| json!({"idx": idx, "pos": [at(kx + k) - at(kx), 0], "dir": "up"}))
        .collect();
    let (w, h) = (at(kx + n + 1) - at(kx), at(ky + 4) - at(ky));
    let copies = s
        .starter_embed(&key)
        .map(|e| e["copies"].clone())
        .unwrap_or(json!([]));
    run(
        s,
        "box.black",
        json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [at(kx), at(ky)],
        "box": {"outline": [[0, 0], [w, 0], [w, h], [0, h]], "cps": box_cps},
        "target": {"existing_function": {"function": function.to_string()}}, "copies": copies}),
    )
    .expect("the box");
    cps
}

/// The main function of device `d`.
fn main_function(s: &Session, d: Uuid) -> Uuid {
    let q = s.query();
    q.get::<ew_derive::MainFunction>(Id::from_uuid(d))
        .map(|f| f.as_uuid())
        .expect("a main function")
}

/// A CPU with an output card drawn on page 101 and put in its rack, as the rack dialog and the window's placement make
/// them: page 101 and the card.
fn output_card_on_101(s: &mut Session) -> (Uuid, Uuid) {
    // A CPU with an output card, as the rack dialog makes it.
    let [cpu, dout] = ["EW-GEN-CPU-1", "EW-GEN-DO-1"].map(|n| library_part(s, n));
    let mut copies: Vec<Value> = Vec::new();
    for p in [cpu, dout] {
        for d in s.module_part_copies(p) {
            if !copies.contains(&d) {
                copies.push(d);
            }
        }
    }
    let part = |p: Uuid| json!({"part": {"copy": p.to_string()}, "qty": "1"});
    run(
        s,
        "plc.add_station",
        json!({"part": part(cpu), "modules": [], "copies": copies}),
    )
    .expect("the station");
    let page = s.page_tables("101").expect("page 101").page;
    let station = {
        let q = s.query();
        q.devices()
            .iter()
            .find(|(_, d)| {
                d.aspects.plc_station.is_some() && d.parts.iter().any(|u| u.part.copy.as_uuid() == cpu)
            })
            .map(|(id, _)| id.as_uuid())
            .expect("the CPU")
    };
    // The output card drawn on page 101 and put in the CPU's rack, as the window's placement and Rack… make it.
    let key = edition(s, "ew.gen.blackbox");
    let symbol = s.starter_symbol_copy(&key).expect("the black box");
    let grid = s
        .query()
        .pages()
        .get(Id::from_uuid(page))
        .map(|p| p.grid)
        .expect("grid");
    let at = |k: i64| grid.at(k).expect("a grid point").0;
    let card_box: Vec<Value> = (0u16..10)
        .zip(1i64..)
        .map(|(idx, k)| json!({"idx": idx, "pos": [at(4 + k) - at(4), 0], "dir": "up"}))
        .collect();
    let (w, h) = (at(15) - at(4), at(24) - at(20));
    let made = new_devices(s, |s| {
        run(
            s,
            "box.black",
            json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [at(4), at(20)],
            "box": {"outline": [[0, 0], [w, 0], [w, h], [0, h]], "cps": card_box},
            "target": {"from_part": {"part": dout.to_string(), "template": 0, "letter": "A"}}}),
        )
        .expect("the card");
    });
    let card = made[0];
    run(
        s,
        "assembly.attach",
        json!({"parent": station.to_string(), "children": [{"device": card.to_string(), "position": 1}]}),
    )
    .expect("the card in the rack");
    (page, card)
}

/// F15 follow-up: the multipin terminal's plug drawn on an electrical page says at each pin what it carries; a wire from
/// a PLC output card to a pin makes that output the solenoid's (the valve's symbol prints it, EW-FL-002 is quiet for
/// it), and emptying a position changes its pins' texts.
#[test]
fn a_multipin_plug_says_its_pins_and_its_wired_outputs_drive_the_solenoids() {
    let mut s = project(184);
    run(
        &mut s,
        "page.create",
        json!({"kind": "multi_line", "number": 101}),
    )
    .expect("the electrical page");
    let multi = place_terminal_of(&mut s, "EW-GEN-VTP-1", (60_000_000, 60_000_000));
    let c = cells(&s, multi);
    let v1 = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[0]),
        multi,
    )
    .expect("v1");
    let v2 = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        centre(&c[1]),
        multi,
    )
    .expect("v2");
    let (page, card) = output_card_on_101(&mut s);

    let (card_fn, plug_fn) = (main_function(&s, card), main_function(&s, multi));
    let card_pins: Vec<(String, u16)> = (1u16..=8).map(|n| (n.to_string(), n - 1)).collect();
    let plug_pins = draw_box(&mut s, page, plug_fn, 4, 60);
    // Beside each pin what it carries: the valve's tag and solenoid, an empty position's number, the common.
    let notes: Vec<String> = {
        let q = s.query();
        ew_api::output::page_plug_notes(&q, Id::from_uuid(page))
            .into_iter()
            .map(|n| n.text)
            .collect()
    };
    let tag = |s: &Session, d: Uuid| s.device_tag(d).expect("a tag");
    assert!(notes.contains(&format!("{} 14", tag(&s, v1))), "{notes:?}");
    assert!(notes.contains(&format!("{} 12", tag(&s, v2))), "{notes:?}");
    assert!(notes.contains(&"3 · 14".to_owned()), "{notes:?}");
    assert!(notes.contains(&"COM".to_owned()), "{notes:?}");
    // Card outputs 1 and 3 wired to plug pins 1 and 3: solenoid 14 of positions 1 and 2.
    let idx =
        |pins: &[(String, u16)], d: &str| pins.iter().find(|(x, _)| x == d).map(|(_, i)| *i).expect("a pin");
    for d in ["1", "3"] {
        run(
            &mut s,
            "connection.draw",
            json!({"page": page.to_string(),
            "a": {"cp": {"function": card_fn.to_string(), "idx": idx(&card_pins, d)}},
            "b": {"cp": {"function": plug_fn.to_string(), "idx": idx(&plug_pins, d)}}}),
        )
        .expect("the wire");
    }
    let output = |s: &Session, v: Uuid| {
        let q = s.query();
        let f = ew_derive::valve_functions(&q, Id::from_uuid(v))[0];
        q.get::<ew_derive::PlcAddressText>(f).text.clone()
    };
    for v in [v1, v2] {
        let text = output(&s, v).unwrap_or_default();
        assert!(
            text.starts_with('Q'),
            "the valve's symbol prints its output: {text:?}"
        );
    }
    assert_ne!(output(&s, v1), output(&s, v2), "two outputs");
    let unbound: Vec<String> = s
        .check(&ew_api::checks::PRE_DELIVERY, ew_api::checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-FL-002")
        .map(|f| f.message)
        .collect();
    assert!(
        !unbound.iter().any(|m| m.contains("solenoid 14")),
        "solenoid 14 of both valves is driven: {unbound:?}"
    );
    // Emptying position 2 changes the texts at its pins.
    run(
        &mut s,
        "valve.set_occupancy",
        json!({"module": multi.to_string(), "position": 2, "occupancy": "empty"}),
    )
    .expect("emptied");
    let notes: Vec<String> = {
        let q = s.query();
        ew_api::output::page_plug_notes(&q, Id::from_uuid(page))
            .into_iter()
            .map(|n| n.text)
            .collect()
    };
    assert!(notes.contains(&"2 · 14".to_owned()), "{notes:?}");
}

/// `copies` with the documents of `more` it does not hold yet, as the window merges them.
fn merged(mut copies: Vec<Value>, more: Vec<Value>) -> Vec<Value> {
    for c in more {
        if !copies.iter().any(|x| x["id"] == c["id"]) {
            copies.push(c);
        }
    }
    copies
}

/// The starter terminal drawn as its positions near `at` as the window draws it, with the symbols of its supply; its ID.
fn place_drawn_terminal(s: &mut Session, at: (i64, i64)) -> Uuid {
    let part = library_part(s, "EW-GEN-VTM-1");
    let key = edition(s, "ew.gen.blackbox");
    let page = fluid_page(s);
    let copies = s
        .starter_embed_part(part, &key)
        .expect("closure")
        .map(|c| c["copies"].as_array().cloned().unwrap_or_default())
        .unwrap_or_default();
    let copies = merged(copies, s.cell_copies(None, &[], Some(page)));
    let symbol = s.starter_symbol_copy(&key).expect("the black box");
    let count = s.part_positions(part).expect("the terminal's positions");
    let (pos, b) = s.slot_box(page, at, count).expect("a box");
    let made = new_devices(s, |s| {
        run(
            s,
            "box.black",
            json!({"page": page.to_string(), "symbol": symbol.to_string(), "pos": [pos.0, pos.1], "box": b,
            "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "K"}},
            "copies": copies}),
        )
        .expect("the terminal");
    });
    made[0]
}

/// Position `position` of `terminal` filled with the starter valve for a terminal, as the rack fills it
/// (`valve.set_occupancy`); with `symbols`, the symbols that draw it in its cell go with it, as the window sends them.
fn fill(s: &mut Session, terminal: Uuid, position: u32, symbols: bool) -> Uuid {
    fill_with(s, terminal, position, "EW-GEN-VLV-3", symbols)
}

/// [`fill`] with the valve part numbered `number`.
fn fill_with(s: &mut Session, terminal: Uuid, position: u32, number: &str, symbols: bool) -> Uuid {
    let part = library_part(s, number);
    let mut copies = s.module_part_copies(part);
    if symbols {
        copies = merged(copies, s.cell_copies(Some(terminal), &[part], None));
    }
    run(
        s,
        "valve.set_occupancy",
        json!({"module": terminal.to_string(), "position": position,
        "occupancy": {"valve": {"state": "used", "part": part.to_string()}}, "copies": copies}),
    )
    .expect("filled");
    cells(s, terminal)[position as usize - 1]
        .holder
        .expect("the valve")
}

/// The ports of `device`'s symbols on page 201 (no box): designation and point.
fn ports(s: &Session, device: Uuid) -> Vec<(String, (i64, i64))> {
    let t = tables(s);
    t.cps
        .iter()
        .filter(|c| {
            t.reps
                .iter()
                .any(|r| r.id == c.rep && r.device == Some(device) && r.black_box.is_none())
        })
        .map(|c| (c.label.rsplit(':').next().unwrap_or_default().to_owned(), c.at))
        .collect()
}

/// A tube from port `a` of `x` to port `b` of `y` on page 201.
fn tube(s: &mut Session, (x, a): (Uuid, &str), (y, b): (Uuid, &str)) -> Result<(), String> {
    let t = tables(s);
    let end = |dev: Uuid, d: &str| {
        let cp = t
            .cps
            .iter()
            .find(|c| {
                c.label.rsplit(':').next() == Some(d)
                    && t.reps.iter().any(|r| r.id == c.rep && r.device == Some(dev))
            })
            .unwrap_or_else(|| panic!("no port {d}"));
        json!({"cp": {"function": cp.function.to_string(), "idx": cp.idx}})
    };
    let page = fluid_page(s);
    run(
        s,
        "connection.draw",
        json!({"page": page.to_string(), "kind": {"fluid": {}}, "a": end(x, a), "b": end(y, b)}),
    )
}

/// The open findings of rule `rule` on `device`.
fn findings(s: &Session, rule: &str, device: Uuid) -> Vec<ew_api::checks::Finding> {
    s.check(s.online_scheme(), ew_api::checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule && f.is_open() && f.targets.iter().any(|t| t.id.as_uuid() == device))
        .collect()
}

/// G16 (rc.3): a position filled from the rack stood as a tag in its cell, without a port. Now each valve stands in its
/// cell as its symbol, its working ports 2 and 4 on the cell's top edge, and tubes reach them from a cylinder: all four
/// positions filled (the owner's terminal held four valves), four valves in four cells, each with its own cylinder.
/// The terminal's supply ports 1, 3 and 5 stand in its left end, where a tube from the air supply quiets EW-FL-010;
/// emptying a position takes its symbol away.
#[test]
fn rc3_a_position_filled_from_the_rack_shows_its_valve_and_its_ports_take_tubes() {
    let mut s = project(191);
    let vt = place_drawn_terminal(&mut s, (60_000_000, 80_000_000));
    let valves: Vec<Uuid> = (1..=4).map(|n| fill(&mut s, vt, n, true)).collect();
    let c = cells(&s, vt);
    assert_eq!(c.len(), 4, "the starter terminal's four positions");
    for (i, v) in valves.iter().enumerate() {
        let n = i + 1;
        assert_eq!(c[i].holder, Some(*v), "valve {n} on position {n}");
        let r = rep_of(&s, *v);
        assert!(
            c[i].rect[0] < r.pos.0 && r.pos.0 < c[i].rect[2],
            "valve {n} stands in cell {n}"
        );
        let p = ports(&s, *v);
        for working in ["2", "4"] {
            let at = p.iter().find(|(d, _)| d == working).map(|(_, at)| *at);
            assert_eq!(
                at.map(|a| a.1),
                Some(c[i].rect[1]),
                "valve {n}: port {working} on the top edge"
            );
            let x = at.map_or(0, |a| a.0);
            assert!(
                c[i].rect[0] < x && x < c[i].rect[2],
                "valve {n}: port {working} in its cell"
            );
        }
    }
    // A cylinder above each valve, its two ports tubed to the valve's 4 and 2.
    for (i, v) in valves.iter().enumerate() {
        let cyl = place_into(
            &mut s,
            "EW-GEN-CYL-1",
            "ew.iso.cylinder-double-acting",
            (centre(&c[i]).0, 30_000_000),
            vt,
        )
        .expect("the cylinder");
        tube(&mut s, (*v, "4"), (cyl, "A")).expect("a tube to port 4");
        tube(&mut s, (*v, "2"), (cyl, "B")).expect("a tube to port 2");
    }
    let tubes = tables(&s)
        .connections
        .iter()
        .filter(|c| c.kind == "fluid")
        .count();
    assert_eq!(tubes, 2 * valves.len(), "every working port takes its tube");
    let v = valves[1];
    // The terminal's own supply in its left end.
    let mut supply: Vec<String> = ports(&s, vt).into_iter().map(|(d, _)| d).collect();
    supply.sort();
    assert_eq!(supply, ["1", "3", "5"]);
    assert_eq!(findings(&s, "EW-FL-010", vt).len(), 1, "no air yet");
    let air = place_into(
        &mut s,
        "EW-GEN-AIR-1",
        "ew.iso.air-supply",
        (20_000_000, 150_000_000),
        vt,
    )
    .expect("the air supply");
    tube(&mut s, (air, "1"), (vt, "1")).expect("a tube to the supply port");
    assert!(findings(&s, "EW-FL-010", vt).is_empty(), "air reaches gallery 1");
    // Emptied, the position holds no symbol.
    run(
        &mut s,
        "valve.set_occupancy",
        json!({"module": vt.to_string(), "position": 2, "occupancy": "empty"}),
    )
    .expect("emptied");
    assert!(!tables(&s).reps.iter().any(|r| r.device == Some(v)));
}

/// rc.3: a 5/3 valve is wider than a cell. Put between two 5/2 valves whose ports 2 and 4 take tubes, its position is
/// as wide as it needs: the box keeps its corner and grows to the right, the valve and the cells to its right move,
/// each valve stands in its own cell with ports 2 and 4 on the top edge, and the tubes still end at their ports.
#[test]
fn rc3_a_5_3_valve_widens_its_position_and_the_cells_to_its_right_follow() {
    let mut s = project(193);
    let vt = place_drawn_terminal(&mut s, (60_000_000, 80_000_000));
    let box_at = |s: &Session| {
        tables(s)
            .reps
            .iter()
            .find(|r| r.device == Some(vt) && r.black_box.is_some())
            .map(|r| r.pos)
            .expect("the box")
    };
    let corner = box_at(&s);
    let pitch = {
        let c = cells(&s, vt);
        c[0].rect[2] - c[0].rect[0]
    };
    let first = fill(&mut s, vt, 1, true);
    let last = fill(&mut s, vt, 3, true);
    for (k, v) in [first, last].into_iter().enumerate() {
        let x = 30_000_000 + 60_000_000 * i64::try_from(k).unwrap_or(0);
        let cyl = place_into(
            &mut s,
            "EW-GEN-CYL-1",
            "ew.iso.cylinder-double-acting",
            (x, 30_000_000),
            vt,
        )
        .expect("a cylinder");
        tube(&mut s, (v, "4"), (cyl, "A")).expect("a tube to port 4");
        tube(&mut s, (v, "2"), (cyl, "B")).expect("a tube to port 2");
    }
    let wide = fill_with(&mut s, vt, 2, "EW-GEN-VLV-53", true);
    let c = cells(&s, vt);
    assert_eq!(box_at(&s), corner, "the box keeps its corner");
    assert!(c[1].rect[2] - c[1].rect[0] > pitch, "position 2 is wider: {c:?}");
    assert_eq!(c[0].rect[2] - c[0].rect[0], pitch, "position 1 keeps its width");
    assert_eq!(c[2].rect[2] - c[2].rect[0], pitch, "position 3 keeps its width");
    assert_eq!(c[2].rect[0], c[1].rect[2], "position 3 follows position 2");
    for (i, v) in [first, wide, last].into_iter().enumerate() {
        let p = ports(&s, v);
        for working in ["2", "4"] {
            let (x, y) = p
                .iter()
                .find(|(d, _)| d == working)
                .map(|(_, at)| *at)
                .unwrap_or_else(|| panic!("port {working} of position {}", i + 1));
            assert_eq!(
                y,
                c[i].rect[1],
                "port {working} of position {} on the top edge",
                i + 1
            );
            assert!(
                c[i].rect[0] < x && x < c[i].rect[2],
                "port {working} of position {} in its cell",
                i + 1
            );
        }
    }
    // The tubes of the valve that moved end at its ports.
    let t = tables(&s);
    let moved: Vec<(i64, i64)> = ports(&s, last).into_iter().map(|(_, at)| at).collect();
    let tubes: Vec<_> = t
        .connections
        .iter()
        .filter(|w| {
            w.polyline
                .first()
                .into_iter()
                .chain(w.polyline.last())
                .any(|p| moved.contains(p))
        })
        .collect();
    assert_eq!(tubes.len(), 2, "both tubes of position 3 end at its ports");
}

/// G16 (rc.3), a project made before: valves put on the positions without a symbol stand as tags, each reported "not
/// drawn on any page" with the fix Draw its valves in the terminal. The fix draws them in their cells and the
/// terminal's supply in its left end; a valve drawn on another page on purpose stays there, and its cell names the
/// place it is drawn at, linked to it.
#[test]
fn rc3_a_project_made_before_draws_its_valves_in_the_terminal() {
    let mut s = project(192);
    let vt = place_terminal(&mut s, (60_000_000, 80_000_000));
    let (a, b) = (fill(&mut s, vt, 1, false), fill(&mut s, vt, 2, false));
    let not_drawn = findings(&s, "EW-CORE-047", a);
    let fix = not_drawn
        .first()
        .and_then(|f| f.fix.clone())
        .expect("a quick fix");
    assert_eq!(fix.command, "valve_terminal.draw_valves");
    assert!(s.valves_to_draw(vt));
    // Valve b drawn on page 202 on purpose.
    run(&mut s, "page.create", json!({"kind": "fluid", "number": 202})).expect("page 202");
    let page202 = s.page_tables("202").expect("page 202").page;
    let key = edition(&s, "ew.iso.valve-5-2-sol-spring");
    let embed = s.starter_embed(&key).unwrap_or(json!({"copies": []}));
    let symbol = s.starter_symbol_copy(&key).expect("the valve symbol");
    let function = ew_derive::valve_functions(&s.query(), Id::from_uuid(b))[0];
    let at = s
        .slot_box(page202, (40_000_000, 40_000_000), 1)
        .expect("a point")
        .0;
    run(
        &mut s,
        "device.place",
        json!({"page": page202.to_string(), "symbol": symbol.to_string(), "pos": [at.0, at.1],
        "target": {"existing_function": {"function": function.as_uuid().to_string()}}, "copies": embed["copies"]}),
    )
    .expect("b on page 202");
    let mut params = fix.params.clone();
    params["copies"] = Value::Array(s.cell_copies(Some(vt), &[], None));
    run(&mut s, "valve_terminal.draw_valves", params).expect("drawn");
    let c = cells(&s, vt);
    let r = rep_of(&s, a);
    assert!(c[0].rect[0] < r.pos.0 && r.pos.0 < c[0].rect[2], "a in its cell");
    assert!(findings(&s, "EW-CORE-047", a).is_empty());
    assert!(
        !tables(&s).reps.iter().any(|r| r.device == Some(b)),
        "b stays on page 202"
    );
    let mut supply: Vec<String> = ports(&s, vt).into_iter().map(|(d, _)| d).collect();
    supply.sort();
    assert_eq!(supply, ["1", "3", "5"]);
    let q = s.query();
    let views = ew_api::output::page_valve_cells(&q, Id::from_uuid(fluid_page(&s)));
    match &views[0].cells[1].1 {
        ew_scene::page::CellView::Valve {
            elsewhere: Some(_),
            drawn_at: Some((place, rep)),
        } => {
            assert!(place.starts_with("202"), "{place}");
            assert_eq!(q.reps().get(*rep).map(|r| r.page.as_uuid()), Some(page202));
        }
        other => panic!("{other:?}"),
    }
    assert!(!s.valves_to_draw(vt), "nothing left to draw");
}

/// rc.3: a 5/3 valve's two addresses ("Q0.2 / Q0.3") touched its left square. A valve with two solenoids prints each
/// solenoid's address at its own solenoid: in a multipin terminal whose plug pins of the 5/3 valve's solenoids 14 and
/// 12 are wired to two outputs, the valve's place at its left solenoid prints the output of 14, the one at its right
/// solenoid the output of 12, and no place prints both.
#[test]
fn a_valve_with_two_solenoids_prints_each_address_at_its_solenoid() {
    let mut s = project(185);
    run(
        &mut s,
        "page.create",
        json!({"kind": "multi_line", "number": 101}),
    )
    .expect("the electrical page");
    let multi = place_terminal_of(&mut s, "EW-GEN-VTP-1", (60_000_000, 60_000_000));
    let c = cells(&s, multi);
    let v = place_into(
        &mut s,
        "EW-GEN-VLV-53",
        "ew.iso.valve-5-3-cc",
        centre(&c[0]),
        multi,
    )
    .expect("the 5/3 valve");
    let pin_of = |s: &Session, name: &str| match s.valve_solenoids(v).into_iter().find(|x| x.name == name) {
        Some(ew_api::output::ValveSolenoid {
            seat: Some(ew_api::output::SolenoidSeat::Multipin { pin, .. }),
            ..
        }) => pin,
        other => panic!("solenoid {name} on a plug pin: {other:?}"),
    };
    let pins = [pin_of(&s, "14"), pin_of(&s, "12")];
    let (page, card) = output_card_on_101(&mut s);
    let (card_fn, plug_fn) = (main_function(&s, card), main_function(&s, multi));
    let plug_pins = draw_box(&mut s, page, plug_fn, 4, 60);
    let idx = |d: &str| {
        plug_pins
            .iter()
            .find(|(x, _)| x == d)
            .map(|(_, i)| *i)
            .expect("a pin")
    };
    for (output, pin) in [(0u16, &pins[0]), (1, &pins[1])] {
        run(
            &mut s,
            "connection.draw",
            json!({"page": page.to_string(),
            "a": {"cp": {"function": card_fn.to_string(), "idx": output}},
            "b": {"cp": {"function": plug_fn.to_string(), "idx": idx(pin)}}}),
        )
        .expect("the wire");
    }
    let rep = rep_of(&s, v);
    let q = s.query();
    let f = ew_derive::valve_functions(&q, Id::from_uuid(v))[0];
    let texts = (*q.get::<ew_derive::PlcAddressText>(f)).clone();
    let address = |d: &str| {
        texts
            .solenoids
            .iter()
            .find(|(x, _)| x == d)
            .map(|(_, t)| t.clone())
            .expect(d)
    };
    let (a14, a12) = (address("14"), address("12"));
    assert_ne!(a14, a12, "two outputs");
    let symbol = q
        .symbols()
        .get(Id::from_uuid(rep.symbol))
        .expect("the symbol")
        .item
        .clone();
    let slots = q.get::<ew_api::output::PageSlotsOf>(Id::from_uuid(fluid_page(&s)));
    let printed: Vec<(Option<String>, i64, String)> = slots
        .iter()
        .find(|(r, _, _)| r.as_uuid() == rep.id)
        .map(|(_, values, _)| values.clone())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| {
            let slot = &symbol.text_slots[usize::from(v.slot)];
            (slot.kind == ew_schema::SlotKind::PlcAddress).then(|| {
                (
                    slot.solenoid.as_ref().map(|x| x.as_str().to_owned()),
                    slot.pos.x.0,
                    v.text,
                )
            })
        })
        .collect();
    let at = |d: &str| {
        printed
            .iter()
            .find(|(x, _, _)| x.as_deref() == Some(d))
            .cloned()
            .expect(d)
    };
    let (left, right) = (at("14"), at("12"));
    assert_eq!(
        (left.2.as_str(), right.2.as_str()),
        (a14.as_str(), a12.as_str()),
        "{printed:?}"
    );
    assert!(
        left.1 < right.1,
        "14 at the left solenoid, 12 at the right: {printed:?}"
    );
    assert_eq!(printed.len(), 2, "no place prints both: {printed:?}");
}
