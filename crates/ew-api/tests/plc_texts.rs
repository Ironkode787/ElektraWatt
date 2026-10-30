// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.21 part 2 in the Session (acceptance PT-1 and the restated PT-2; D7, FT-227): the derived PLC texts on the
//! page. The generic CPU and the two generic cards of the starter parts are placed from the picker's command
//! (`box.black` from the part, here with the tool's default box CPs); the cards' pins print their channel addresses
//! in the labels layer, and a `PlcAddress` slot of a starter coil prints the address of the channel bound to it. A
//! `fields.set` of the channel's address changes the print; undo restores it. No address is stored as text.

use ew_api::output::page_layer;
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_derive::{PageRef, Q};
use ew_dl::{Item, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_schema::{Channel, Device, Function, Page};
use serde_json::{Value, json};

fn fresh(seed: u64) -> Session {
    let options = SessionOptions {
        ids: Some(ew_commands::testkit::seeded_ids(seed)),
        ..SessionOptions::default()
    };
    Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options).unwrap()
}

const BLACKBOX: &str = "ew.gen.blackbox@qet10";
const COIL: &str = "ew.iec.coil@qet10";
/// Three steps of the page's connection grid (7937500/3 nm).
const STEP: i64 = 7_937_500;

fn library_part(s: &Session, number: &str) -> Uuid {
    s.library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == number)
        .unwrap_or_else(|| panic!("{number} among the library parts"))
        .id
        .as_uuid()
}

fn first_page(s: &Session) -> Uuid {
    let q = s.query();
    let mut pages: Vec<Uuid> = q.pages().iter().map(|(id, _)| id.as_uuid()).collect();
    pages.sort();
    pages.first().copied().expect("the new project has a page")
}

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

fn new_device(s: &mut Session, f: impl FnOnce(&mut Session)) -> Uuid {
    let before: Vec<Uuid> = s.query().devices().iter().map(|(id, _)| id.as_uuid()).collect();
    f(s);
    let q = s.query();
    let new: Vec<Uuid> = q
        .devices()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .filter(|id| !before.contains(id))
        .collect();
    let [id] = new.as_slice() else {
        panic!("one new device: {new:?}")
    };
    *id
}

/// Places the starter part `number` as the picker does, with eight box CPs (function CP indices 0 … 7, the first four
/// on the top edge leaving up, the others on the bottom edge leaving down) when `pins`.
fn place(s: &mut Session, number: &str, slot: i64, pins: bool) -> Uuid {
    let part = library_part(s, number);
    let copies = s
        .starter_embed_part(part, BLACKBOX)
        .expect("the closure resolves")
        .unwrap_or_else(|| json!({"copies": []}));
    let symbol = s.starter_symbol_copy(BLACKBOX).expect("a starter symbol");
    let page = first_page(s);
    let cps: Vec<Value> = if pins {
        (0..8i64)
            .map(|i| {
                let (x, y, dir) = if i < 4 {
                    (i + 1, 0, "up")
                } else {
                    (i - 3, 8 * STEP, "down")
                };
                json!({"idx": i, "pos": [x * STEP, y], "dir": dir})
            })
            .collect()
    } else {
        Vec::new()
    };
    new_device(s, |s| {
        run(
            s,
            "box.black",
            json!({
                "page": page.to_string(),
                "symbol": symbol.to_string(),
                "pos": [12 * STEP * slot, 12 * STEP],
                "box": {"outline": [[0, 0], [5 * STEP, 0], [5 * STEP, 8 * STEP], [0, 8 * STEP]], "cps": cps},
                "target": {"from_part": {"part": part.to_string(), "template": 0, "letter": "A"}},
                "copies": copies["copies"],
            }),
        );
    })
}

fn collect(items: &[Item], out: &mut Vec<String>) {
    for i in items {
        match &i.kind {
            ItemKind::Glyphs(run) => out.push(run.text.clone()),
            ItemKind::Group(g) => collect(&g.items, out),
            _ => {}
        }
    }
}

/// The texts of the labels layer of the first page.
fn labels(s: &Session) -> Vec<String> {
    let page: Id<Page> = Id::from_uuid(first_page(s));
    let q: Q = s.query();
    let list = page_layer(&q, PageRef::Drawn(page), Layer::Labels);
    let mut out = Vec::new();
    collect(&list.list.items, &mut out);
    out
}

fn count(texts: &[String], t: &str) -> usize {
    texts.iter().filter(|x| x.as_str() == t).count()
}

fn channel(module: Uuid, n: u16) -> Id<Channel> {
    ew_ids::v5::plc_channel_id(Id::<Device>::from_uuid(module), n)
}

fn set_address(s: &mut Session, c: Id<Channel>, value: &str) {
    run(
        s,
        "fields.set",
        json!({"kind": "channel", "targets": [c], "path": "address", "value": value}),
    );
}

/// PT-1: after PW-1 (the CPU, then the input card and the output card with their pins drawn), the input card's pins
/// print I0.0 … I0.7 and the output card's Q0.0 … Q0.7 in the page's labels layer; `fields.set address "I4.0"` on
/// channel 0 changes the print, undo restores it; an excluded address prints nothing.
#[test]
fn pt1_the_card_pins_print_their_channel_addresses() {
    let mut s = fresh(21);
    place(&mut s, "EW-GEN-CPU-1", 1, false);
    let di = place(&mut s, "EW-GEN-DI-1", 2, true);
    place(&mut s, "EW-GEN-DO-1", 3, true);
    let texts = labels(&s);
    for n in 0..8 {
        assert_eq!(count(&texts, &format!("I0.{n}")), 1, "I0.{n} in {texts:?}");
        assert_eq!(count(&texts, &format!("Q0.{n}")), 1, "Q0.{n} in {texts:?}");
    }
    let c0 = channel(di, 0);
    set_address(&mut s, c0, "I4.0");
    let texts = labels(&s);
    assert_eq!(count(&texts, "I4.0"), 1);
    assert_eq!(count(&texts, "I0.0"), 0);
    s.undo(&ew_api::Origin::Gui).unwrap();
    let texts = labels(&s);
    assert_eq!(count(&texts, "I4.0"), 0);
    assert_eq!(count(&texts, "I0.0"), 1);
    set_address(&mut s, c0, "excluded");
    let texts = labels(&s);
    assert_eq!(count(&texts, "I0.0"), 0);
    assert_eq!(count(&texts, "I0.1"), 1);
}

/// The pin texts sit inside the box, 1 mm from their pin: a top pin's text below it, read bottom to top.
#[test]
fn pt1_a_pin_text_is_placed_inside_the_box_beside_its_pin() {
    let mut s = fresh(22);
    place(&mut s, "EW-GEN-CPU-1", 1, false);
    place(&mut s, "EW-GEN-DI-1", 2, true);
    let page: Id<Page> = Id::from_uuid(first_page(&s));
    let q = s.query();
    let list = page_layer(&q, PageRef::Drawn(page), Layer::Labels);
    let mut found = None;
    for item in &list.list.items {
        if let ItemKind::Glyphs(run) = &item.kind
            && run.text == "I0.0"
        {
            found = Some(run.layout_quad());
        }
    }
    let quad = found.expect("the pin text of channel 0");
    let (x0, y0) = (24 * STEP + STEP, 12 * STEP);
    let xs: Vec<i64> = quad.iter().map(|p| p.x.0).collect();
    let ys: Vec<i64> = quad.iter().map(|p| p.y.0).collect();
    // below the pin, inside the outline, about 1 mm down from the pin
    assert!(ys.iter().all(|y| *y > y0 && *y < y0 + 8 * STEP), "{quad:?}");
    let top = *ys.iter().min().unwrap();
    assert!((top - (y0 + 1_000_000)).abs() <= 1_000, "{quad:?}");
    // around the pin's x (a vertical text centred on it)
    assert!(xs.iter().all(|x| (x - x0).abs() < 3_000_000), "{quad:?}");
}

/// The restated PT-2: in a new project with the CPU and the output card, a starter coil attached to the card and
/// bound to its channel 0 prints Q0.0 in its `PlcAddress` slot; `fields.set address` changes it and undo restores it.
#[test]
fn pt2_a_starter_coil_bound_to_an_output_channel_prints_its_address() {
    let mut s = fresh(23);
    place(&mut s, "EW-GEN-CPU-1", 1, false);
    let dout = place(&mut s, "EW-GEN-DO-1", 2, false);
    let copies = s.starter_embed(COIL).expect("the coil is not embedded");
    let symbol = s.starter_symbol_copy(COIL).expect("a starter symbol");
    let page = first_page(&s);
    let coil = new_device(&mut s, |s| {
        run(
            s,
            "device.place",
            json!({
                "page": page.to_string(),
                "symbol": symbol.to_string(),
                "pos": [36 * STEP, 24 * STEP],
                "target": {"new_device": {"letter": "K"}},
                "copies": copies["copies"],
            }),
        );
    });
    assert!(
        !labels(&s).iter().any(|t| t.starts_with('Q')),
        "nothing bound yet"
    );
    run(
        &mut s,
        "assembly.attach",
        json!({"parent": dout.to_string(), "children": [{"device": coil.to_string(), "position": 1}]}),
    );
    let function: Id<Function> = s
        .query()
        .functions()
        .iter()
        .find(|(_, f)| f.device.map(|d| d.as_uuid()) == Some(coil))
        .map(|(id, _)| id)
        .expect("the coil's function");
    let c0 = channel(dout, 0);
    run(
        &mut s,
        "plc.bind_channel",
        json!({"channel": c0, "endpoint": {"cp": {"function": function, "idx": 0}}}),
    );
    assert_eq!(count(&labels(&s), "Q0.0"), 1);
    set_address(&mut s, c0, "Q4.0");
    let texts = labels(&s);
    assert_eq!((count(&texts, "Q4.0"), count(&texts, "Q0.0")), (1, 0));
    s.undo(&ew_api::Origin::Gui).unwrap();
    let texts = labels(&s);
    assert_eq!((count(&texts, "Q4.0"), count(&texts, "Q0.0")), (0, 1));
}

/// PT-2 and PT-4 with FT-698 (cycle 38): Golden A's lamp, position switch and 5/2 valve have a `PlcAddress` slot
/// (0.2.0). Every valve on the valve-manifold folios 203 to 206 prints the address of the channel bound to its coil,
/// the address folio 908 lists with that valve as its target; the unbound lamp and position switch of folio 101 print
/// nothing, and none of Golden A's black boxes has a pin bound to a channel. The exported bytes are held by the golden
/// byte-equality and snapshot tests.
#[test]
fn golden_a_prints_the_valve_addresses_of_part_2() {
    let golden = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
    let (s, _) = Session::open_read_only(std::path::Path::new(golden), "test").unwrap();
    let q = s.query();
    let mut with_slot: Vec<String> = q
        .symbols()
        .iter()
        .filter(|(_, c)| {
            c.item
                .text_slots
                .iter()
                .any(|t| t.kind == ew_schema::SlotKind::PlcAddress)
        })
        .filter_map(|(_, c)| c.item.meta.key.as_ref().map(|k| k.as_str().to_owned()))
        .collect();
    with_slot.sort();
    assert_eq!(
        with_slot,
        [
            "ew.iec.lamp@qet10",
            "ew.iec.position-switch-no@qet10",
            "ew.iso.valve-5-2-sol-spring@qet10"
        ]
    );
    // What the PlcAddress slots print: (device, text), with the folio of each.
    let mut printed: Vec<(Id<Device>, String)> = Vec::new();
    let mut folios: Vec<u32> = Vec::new();
    for (page, p) in q.pages().iter() {
        let slots = q.get::<ew_api::output::PageSlotsOf>(page);
        assert!(slots.iter().all(|(_, _, pins)| pins.is_empty()), "{page}");
        for (rep, values, _) in slots.iter() {
            let rep = q.reps().get(*rep).unwrap();
            let symbol = q.symbols().get(rep.symbol).unwrap();
            let device = rep
                .functions
                .first()
                .and_then(|f| q.functions().get(*f))
                .and_then(|f| f.device);
            for v in values {
                if symbol.item.text_slots[usize::from(v.slot)].kind == ew_schema::SlotKind::PlcAddress {
                    printed.push((device.expect("a bound function has a device"), v.text.clone()));
                    folios.push(p.folio.number);
                }
            }
        }
    }
    printed.sort();
    folios.sort_unstable();
    folios.dedup();
    assert_eq!(folios, [203, 204, 205, 206]);
    // The PLC overview's rows (folio 908): each channel bound to a device, with its printed address.
    let mut listed: Vec<(Id<Device>, String)> = Vec::new();
    for (station, d) in q.devices().iter() {
        if d.aspects.plc_station.is_none() {
            continue;
        }
        for row in q.get::<ew_derive::PlcRows>(station).iter() {
            if let (Some(device), ew_derive::ChannelAddr::At { text, .. }) = (row.target_device, &row.address)
            {
                listed.push((device, text.clone()));
            }
        }
    }
    listed.sort();
    assert!(listed.iter().any(|(_, t)| t == "Q0.1") && listed.iter().any(|(_, t)| t == "Q6.7"));
    assert_eq!(printed, listed);
}
