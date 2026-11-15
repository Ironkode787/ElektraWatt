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
    s.query().devices().get(Id::from_uuid(id)).cloned().expect("the device")
}

/// The starter symbol `key` in the edition of the fluid page's grid.
fn edition(s: &Session, key: &str) -> String {
    let page = fluid_page(s);
    let qet = s.query().pages().get(Id::from_uuid(page)).map(|p| p.grid) == Some(Grid::QET_10PX);
    if qet { format!("{key}@qet10") } else { key.to_owned() }
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
    let copies = s.starter_embed_part(part, &key).expect("closure").unwrap_or(json!({"copies": []}));
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
fn place_into(s: &mut Session, number: &str, symbol: &str, at: (i64, i64), terminal: Uuid) -> Result<Uuid, String> {
    let part = library_part(s, number);
    let key = edition(s, symbol);
    let copies = s.starter_embed_part(part, &key).expect("closure").unwrap_or(json!({"copies": []}));
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

    let v = place_into(&mut s, "EW-GEN-VLV-3", "ew.iso.valve-5-2-sol-spring", centre(&c[2]), vt).expect("placed");
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
        let grid = s.query().pages().get(Id::from_uuid(page)).map(|p| p.grid).expect("grid");
        let k = |a: i64, b: i64| grid.snap(Nm(b)) - grid.snap(Nm(a));
        run(
            s,
            "rep.move",
            json!({"reps": [rep.to_string()], "steps": [k(from.0, to.0), k(from.1, to.1)], "into": vt.to_string()}),
        )
    };
    move_to(&mut s, r.id, centre(&c[2]), centre(&c[0])).expect("moved");
    assert_eq!(device(&s, v).position, Some(1), "dragged to cell 1");

    let w = place_into(&mut s, "EW-GEN-VLV-3", "ew.iso.valve-5-2-sol-spring", centre(&c[1]), vt).expect("placed");
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
    let refused = place_into(&mut s, "EW-GEN-VLV-3", "ew.iso.valve-5-2-sol-spring", centre(&c[0]), vt);
    let message = refused.expect_err("held");
    assert!(message.contains("already holds"), "{message}");
    assert!(!message.contains("push insert"), "{message}");

    // A stand-alone valve (a coil of its own to wire) stays a valve of its own.
    let own = place_into(&mut s, "EW-GEN-VLV-1", "ew.iso.valve-5-2-sol-spring", centre(&c[3]), vt).expect("placed");
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
    let copies = s.starter_embed_part(part, &key).expect("closure").unwrap_or(json!({"copies": []}));
    let symbol = s.starter_symbol_copy(&key).expect("the black box");
    let (pos, _) = s.slot_box(page, (60_000_000, 40_000_000), 1).expect("a point");
    let grid = s.query().pages().get(Id::from_uuid(page)).map(|p| p.grid).expect("grid");
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
    assert!(free.black_box.as_ref().is_some_and(|b| b.cells.is_empty()), "a free box shows no cells");
    // A valve of the terminal drawn below the box, with a tube from its port 2 to a cylinder.
    let v = place_into(
        &mut s,
        "EW-GEN-VLV-3",
        "ew.iso.valve-5-2-sol-spring",
        (pos.0, pos.1 + at(40)),
        vt,
    )
    .expect("placed");
    assert_eq!(device(&s, v).position, Some(1), "the first free position of a free box");
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
            .find(|c| c.label.ends_with(&format!(":{d}")) && t.reps.iter().any(|r| r.id == c.rep && r.device == Some(dev)))
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
    bulk(&mut s, "valve_terminal.draw_slots", json!({"rep": free.id.to_string()})).expect("drawn as slots");
    let c = cells(&s, vt);
    assert_eq!(c.len(), 4);
    let r = rep_of(&s, v);
    assert!(c[0].rect[0] < r.pos.0 && r.pos.0 < c[0].rect[2], "the valve stands in cell 1");
    let t = tables(&s);
    let tube = t.connections.iter().find(|x| x.kind == "fluid").expect("the tube");
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
    let again = bulk(&mut s, "valve_terminal.draw_slots", json!({"rep": free.id.to_string()}));
    assert!(again.expect_err("refused").contains("drawn as its positions already"));
}
