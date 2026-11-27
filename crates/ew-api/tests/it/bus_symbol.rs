// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A device on a bus-topology page drawn with its bus symbol (doc 10 §2.7), built by records: the page draws the symbol
//! derived from the part's network ports in place of the `ew.gen.bus` copy, each address inside its compartment, every
//! text of the device inside its body and clear of the others, with sockets on all four edges (a symbol upright and
//! one turned a quarter), measured in the shipped fonts on the QElectroTech grid.

use ew_derive::{PageOrder, PageRef, Q};
use ew_dl::{EntityRef, ItemKind, Layer};
use ew_ids::{Id, Uuid};
use ew_model::Model;
use ew_schema::{LibDocument, Primitive, Project, Record, Representation};
use ew_units::Pt2;
use serde_json::{Value, json};

const PAGE: u128 = 0x10;
const PORT_DEF: u128 = 0x20;
const HEAD: u128 = 0x30;
const SWITCH: u128 = 0x31;
const UPRIGHT: u128 = 0x40;
const TURNED: u128 = 0x41;
const SWITCHED: u128 = 0x42;
/// 2.6458 mm: one step of the QElectroTech grid, in nm (rounded down).
const STEP: i64 = 2_645_833;

fn uid(n: u128) -> String {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 | n).to_string()
}

fn id<T>(n: u128) -> Id<T> {
    Id::from_uuid(uid(n).parse().expect("a uuid"))
}

/// The starter bus symbol's QElectroTech edition as the project embeds it.
fn bus_copy() -> (Value, Uuid) {
    let item = ew_library::starter::item("ew.gen.bus@qet10").expect("the starter bus symbol");
    let doc: Value = serde_json::from_str(item.json).expect("json");
    let copy = doc["id"].as_str().expect("an id").parse().expect("a uuid");
    (doc, copy)
}

/// A part of one's own with network ports `ports`.
fn part(n: u128, number: &str, ports: Value) -> Value {
    json!({
        "$k": "part", "id": uid(n), "library_id": uid(0x9000), "item_id": uid(n + 0x1000), "version": "1.0.0",
        "sha256": format!("sha256:{}", "0".repeat(64)),
        "manufacturer": {"library": uid(0x9000), "item": uid(0x9100), "version_req": "^1"},
        "part_number": number,
        "sections": {"bus": {"ports": ports}},
    })
}

/// A device of part `part` with its port functions `(id, port)`.
fn device(n: u128, letter: &str, counter: u32, part: u128, ports: &[(u128, Value)]) -> Vec<Value> {
    let mut out = vec![json!({
        "$k": "device", "id": uid(n), "tag": {"letter": letter, "counter": counter},
        "parts": [{"part": {"copy": uid(part)}, "qty": "1"}],
    })];
    for (k, (f, port)) in ports.iter().enumerate() {
        let role = if k == 0 { "main" } else { "aux" };
        out.push(
            json!({"$k": "function", "id": uid(*f), "device": uid(n), "def": uid(PORT_DEF), "role": role,
            "bus_port": port}),
        );
    }
    out
}

fn rep(n: u128, symbol: Uuid, functions: &[u128], at: (i64, i64), orient: &str) -> Value {
    let functions: Vec<String> = functions.iter().map(|f| uid(*f)).collect();
    json!({
        "$k": "representation", "id": uid(n), "functions": functions, "page": uid(PAGE), "symbol": symbol.to_string(),
        "pos": [at.0 * STEP, at.1 * STEP], "orient": orient, "rep_type": "topology",
    })
}

fn profinet(interface: &str, name: &str, ip: &str) -> Value {
    json!({"interface": interface, "bus": "profinet", "role": "device",
        "address": {"value": {"profinet": {"name": name, "ip": ip}}}})
}

/// A bus page with a head station of part HS-2 (X1 with two ports, X2 with one) drawn upright and once more turned a
/// quarter, and a managed switch of part MSW-8 (X1 with eight ports); `address` is the head station's X1 device name.
fn parts(address: &str) -> (Project, Vec<Record>, Vec<LibDocument>) {
    let (symbol, copy) = bus_copy();
    let mut records = vec![json!({
        "$k": "page", "id": uid(PAGE), "folio": {"number": 105}, "kind": "bus_topology", "structure": {},
        "title": {"en": "Bus"}, "grid": {"num": 7_937_500, "den": 3},
    })];
    let x1 = profinet("X1", address, "192.168.0.11");
    let x2 = profinet("X2", "rio1-svc", "192.168.1.11");
    records.extend(device(
        0x50,
        "A",
        1,
        HEAD,
        &[(0x51, x1.clone()), (0x52, x2.clone())],
    ));
    records.extend(device(0x60, "A", 2, HEAD, &[(0x61, x1), (0x62, x2)]));
    records.extend(device(
        0x70,
        "K",
        1,
        SWITCH,
        &[(0x71, profinet("X1", "sw1", "192.168.0.2"))],
    ));
    records.push(rep(UPRIGHT, copy, &[0x51, 0x52], (10, 10), "r0"));
    records.push(rep(TURNED, copy, &[0x61, 0x62], (60, 10), "r90"));
    records.push(rep(SWITCHED, copy, &[0x71], (10, 40), "r0"));
    let mut lib = vec![
        symbol,
        json!({"$k": "fndef", "id": uid(PORT_DEF), "library_id": uid(0x9000), "item_id": uid(0x9200),
            "version": "1.0.0", "sha256": format!("sha256:{}", "1".repeat(64)),
            "category": ["Bus", "Port", "Profinet"], "contact_kind": "bus_port",
            "cps": [{"idx": 0, "designation": "P1"}]}),
    ];
    lib.push(part(
        HEAD,
        "HS-2",
        json!([{"interface": "X1", "bus": "profinet", "role": "device", "ports": 2},
               {"interface": "X2", "bus": "profinet", "role": "device"}]),
    ));
    lib.push(part(
        SWITCH,
        "MSW-8",
        json!([{"interface": "X1", "bus": "profinet", "role": "device", "ports": 8}]),
    ));
    let project: Project =
        serde_json::from_value(json!({"id": uid(1), "derivation_profile": "2027.1"})).expect("project");
    let records: Vec<Record> = records
        .into_iter()
        .map(|r| serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("{r}: {e}")))
        .collect();
    let lib: Vec<LibDocument> = lib
        .into_iter()
        .map(|d| serde_json::from_value(d.clone()).unwrap_or_else(|e| panic!("{d}: {e}")))
        .collect();
    (project, records, lib)
}

fn model(address: &str) -> Model {
    let (project, records, lib) = parts(address);
    Model::from_records(project, records, lib).unwrap_or_else(|e| panic!("model: {e:?}"))
}

/// A box `(min, max)` of the page.
type Box2 = ((i64, i64), (i64, i64));

/// The body of representation `n` on the page: its drawn symbol's rectangle, placed.
fn body(q: &Q, n: u128) -> Box2 {
    let stored: Representation = (**q.reps().get(id(n)).expect("the representation")).clone();
    let (symbol, rep) = ew_derive::drawn(q, &stored).expect("drawn");
    let rect = symbol
        .graphics
        .iter()
        .find_map(|p| match p {
            Primitive::Rect(r) => Some(r.rect.0),
            _ => None,
        })
        .expect("a body");
    let corners = [rect.min(), rect.max()].map(|c| rep.pos + rep.orient.apply(c));
    let xs = corners.map(|c: Pt2| c.x.0);
    let ys = corners.map(|c: Pt2| c.y.0);
    (
        (xs[0].min(xs[1]), ys[0].min(ys[1])),
        (xs[0].max(xs[1]), ys[0].max(ys[1])),
    )
}

/// Every glyph run of representation `n` (its static texts and its slot texts) on the page: its text and its box.
fn texts(q: &Q, n: u128) -> Vec<(String, Box2)> {
    let entry = q
        .get::<PageOrder>(())
        .entries
        .iter()
        .find(|e| e.page == PageRef::Drawn(id(PAGE)))
        .cloned()
        .expect("the page in the order");
    let scene = ew_api::output::page_scene(q, &entry);
    let entity = EntityRef(id::<()>(n).as_uuid());
    scene
        .layers
        .iter()
        .filter(|(l, _)| matches!(l, Layer::Symbols | Layer::Labels))
        .flat_map(|(_, list)| list.items.clone())
        .filter(|i| i.entity == Some(entity))
        .filter_map(|i| match &i.kind {
            ItemKind::Glyphs(run) => {
                let quad = run.layout_quad();
                let xs = quad.iter().map(|p| p.x.0);
                let ys = quad.iter().map(|p| p.y.0);
                Some((
                    run.text.clone(),
                    ((xs.clone().min()?, ys.clone().min()?), (xs.max()?, ys.max()?)),
                ))
            }
            _ => None,
        })
        .collect()
}

fn inside(a: Box2, b: Box2) -> bool {
    a.0.0 >= b.0.0 && a.0.1 >= b.0.1 && a.1.0 <= b.1.0 && a.1.1 <= b.1.1
}

fn overlap(a: Box2, b: Box2) -> bool {
    a.0.0 < b.1.0 && b.0.0 < a.1.0 && a.0.1 < b.1.1 && b.0.1 < a.1.1
}

/// The page draws each device with the symbol its part's ports give, under the `ew.gen.bus` copy: two parts, two
/// drawings, the head station's addresses printed inside it, and every text of every device inside its body and clear
/// of its other texts, the sockets' labels among them, whether its sockets stand on the top and bottom edges or, turned
/// a quarter, on the left and right ones. So no text of a device can lie on a cable that ends at a socket's stub.
#[test]
fn a_device_on_a_bus_page_is_its_parts_symbol_with_every_text_inside_its_body() {
    let model = model("rio1");
    let q = Q::new(&model);
    for n in [UPRIGHT, TURNED, SWITCHED] {
        let body = body(&q, n);
        let texts = texts(&q, n);
        assert!(
            texts.len() >= 4,
            "{n:x}: the tag, the part and the interfaces print: {texts:?}"
        );
        for (text, b) in &texts {
            assert!(
                inside(*b, body),
                "{n:x}: {text:?} at {b:?} outside the body {body:?}"
            );
        }
        for (i, (a, ab)) in texts.iter().enumerate() {
            for (c, cb) in &texts[i + 1..] {
                assert!(!overlap(*ab, *cb), "{n:x}: {a:?} lies on {c:?}");
            }
        }
    }
    let upright = texts(&q, UPRIGHT);
    for printed in [
        "-105A1",
        "HS-2",
        "X1",
        "X2",
        "rio1",
        "192.168.0.11",
        "rio1-svc",
        "192.168.1.11",
        "P1",
        "P2",
    ] {
        assert!(
            upright.iter().any(|(t, _)| t == printed),
            "{printed} printed: {upright:?}"
        );
    }
    let (a, b) = (body(&q, UPRIGHT), body(&q, SWITCHED));
    assert_ne!(a.1.0 - a.0.0, b.1.0 - b.0.0, "each part has its own drawing");
    // The sockets: X1 P1 on the top edge, X1 P2 on the bottom edge, X2's one socket on the top edge right of X1's,
    // each drawn by the function that carries its interface.
    let at = |f: u128, idx: u16| ew_derive::cp_point(&q, id(f), idx, id(PAGE)).expect("a drawn socket");
    let (x1p1, x1p2, x2) = (at(0x51, 0), at(0x51, 1), at(0x52, 0));
    assert!(
        x1p1.y.0 < a.0.1 && x2.y.0 < a.0.1,
        "on the top edge, outside the body"
    );
    assert!(x1p2.y.0 > a.1.1, "on the bottom edge, outside the body");
    assert!(
        x2.x.0 > x1p1.x.0 && x2.x.0 > x1p2.x.0,
        "X2's compartment right of X1's"
    );
}

/// The symbol's size follows from the part alone: a device name too long for its compartment is cut with "…" on the
/// page, and the body and its sockets stay where they were.
#[test]
fn a_long_address_is_cut_and_moves_no_socket() {
    let (short, long) = (model("rio1"), model("remote-io-station-number-one"));
    let (qs, ql) = (Q::new(&short), Q::new(&long));
    assert_eq!(body(&qs, UPRIGHT), body(&ql, UPRIGHT));
    for f in [0x51, 0x52] {
        for idx in [0, 1] {
            assert_eq!(
                ew_derive::cp_point(&qs, id(f), idx, id(PAGE)),
                ew_derive::cp_point(&ql, id(f), idx, id(PAGE)),
                "socket {idx} of {f:x}"
            );
        }
    }
    let texts = texts(&ql, UPRIGHT);
    assert!(
        texts
            .iter()
            .any(|(t, _)| t.starts_with("remote") && t.ends_with('…')),
        "the name is cut: {texts:?}"
    );
    assert!(!texts.iter().any(|(t, _)| t == "remote-io-station-number-one"));
}

/// Properties of a bus symbol that draws two network ports shows a section for each, in the symbol's order, named by
/// its interface, with that port's own address; a symbol of one port keeps its one section.
#[test]
fn properties_of_a_bus_symbol_has_a_section_per_network_port() {
    let (project, records, lib) = parts("rio1");
    let s = ew_api::Session::from_records(project, records, lib, ew_api::SessionOptions::default())
        .expect("a session");
    let sheet = s.property_sheet(&[id::<()>(UPRIGHT).as_uuid()]);
    let ports: Vec<(Option<&str>, Vec<Uuid>)> = sheet
        .sections
        .iter()
        .filter(|x| x.kind == "function")
        .map(|x| (x.port.as_deref(), x.targets.clone()))
        .collect();
    assert_eq!(
        ports,
        [
            (Some("X1"), vec![id::<()>(0x51).as_uuid()]),
            (Some("X2"), vec![id::<()>(0x52).as_uuid()])
        ]
    );
    let one = s.property_sheet(&[id::<()>(SWITCHED).as_uuid()]);
    assert!(one.sections.iter().all(|x| x.port.is_none()));
}
