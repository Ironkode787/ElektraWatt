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
const COUPLER: u128 = 0x32;
const UPRIGHT: u128 = 0x40;
const TURNED: u128 = 0x41;
const SWITCHED: u128 = 0x42;
const COUPLED: u128 = 0x43;
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
/// quarter, a managed switch of part MSW-8 (X1 with eight ports) and a coupler of part CPL-PN-DP on two buses (X1 on
/// PROFINET with two ports, DP on PROFIBUS); `address` is the head station's X1 device name.
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
    let dp = json!({"interface": "DP", "bus": "profibus_dp", "role": "slave", "address": {"value": {"profibus": 5}}});
    records.extend(device(
        0x80,
        "A",
        3,
        COUPLER,
        &[(0x81, profinet("X1", "cpl1", "192.168.0.20")), (0x82, dp)],
    ));
    records.push(rep(COUPLED, copy, &[0x81, 0x82], (60, 45), "r0"));
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
    lib.push(part(
        COUPLER,
        "CPL-PN-DP",
        json!([{"interface": "X1", "bus": "profinet", "role": "device", "ports": 2},
               {"interface": "DP", "bus": "profibus_dp", "role": "slave"}]),
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
    for n in [UPRIGHT, TURNED, SWITCHED, COUPLED] {
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

/// Properties of a bus symbol says with what it is drawn, in its Bus symbol rows: the program's symbol from the part's
/// network ports, with the note that it follows the part and Make my own from it…, which opens Parts at the part's bus
/// symbol. Each network port's section says its sockets: X1's two by their ports, X2's one by its name.
#[test]
fn properties_say_with_what_a_bus_symbol_is_drawn_and_each_port_s_sockets() {
    use ew_api::output::{Category, RowValue};
    let (project, records, lib) = parts("rio1");
    let s = ew_api::Session::from_records(project, records, lib, ew_api::SessionOptions::default())
        .expect("a session");
    let sheet = s.property_sheet(&[id::<()>(UPRIGHT).as_uuid()]);
    let row = |key: &str| {
        sheet
            .sections
            .iter()
            .flat_map(|x| x.rows.iter())
            .find(|r| r.key == key)
            .unwrap_or_else(|| panic!("{key}"))
    };
    let drawn = row("representation/derived.bus_symbol");
    assert_eq!(drawn.category, Category::BusSymbol);
    let RowValue::One { json, .. } = &drawn.value else {
        panic!("one value")
    };
    assert_eq!(
        *json,
        json!({"part": "HS-2", "own": null}),
        "the program's, from HS-2's ports"
    );
    assert!(drawn.note.is_some(), "it follows the part");
    let link = drawn.link.as_ref().expect("Make my own from it…");
    assert_eq!(
        link.actions,
        [(
            "parts.open",
            json!({"part": format!("id:{}", uid(HEAD)), "field": "bus_symbol"})
        )]
    );
    let sockets = |key: &str| match &row(key).value {
        RowValue::One { text, .. } => text.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(sockets("function@X1/derived.sockets"), "P1 · P2");
    assert_eq!(sockets("function@X2/derived.sockets"), "X2");
}

/// A starter station of two interfaces (EW-GEN-CPU-2: DP, X1) placed on a bus page by `bus.place_ports` as the tool
/// sends it: one symbol, and a port function per interface, each typed for its bus and made of its bus's port function
/// definition (an X1 address takes a PROFINET name).
#[test]
fn a_station_placed_on_a_bus_page_gets_a_port_function_per_interface() {
    use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(5)),
        ..SessionOptions::default()
    };
    let mut s = Session::new_project(ProjectTemplate::Builtin, NewProjectKind::Schematic, options)
        .expect("a project");
    let first = {
        let q = s.query();
        let mut pages: Vec<Uuid> = q.pages().iter().map(|(id, _)| id.as_uuid()).collect();
        pages.sort();
        pages[0]
    };
    let page = s
        .execute(&AnyCommand {
            name: "page.create".into(),
            params: json!({"kind": "bus_topology", "after": first.to_string()}),
        })
        .expect("a bus page")
        .outcome
        .primary
        .expect("the page")
        .1
        .as_uuid();
    let part = s
        .library_parts()
        .iter()
        .find(|c| c.item.part_number.as_str() == "EW-GEN-CPU-2")
        .expect("the starter CPU")
        .clone();
    let key = "ew.gen.bus@qet10";
    let mut copies: Vec<Value> = s
        .starter_embed_part(part.id.as_uuid(), key)
        .expect("the closure")
        .map(|c| c["copies"].as_array().cloned().unwrap_or_default())
        .unwrap_or_default();
    let fndefs: Vec<Uuid> = ew_api::output::part_interfaces(&part.item)
        .iter()
        .map(|i| i.fndef)
        .collect();
    copies.extend(s.port_fndef_copies(&fndefs));
    let symbol = s.starter_symbol_copy(key).expect("the bus symbol");
    // A point of the page's grid, as the tool puts it.
    let on = s
        .query()
        .pages()
        .get(Id::from_uuid(page))
        .expect("the page")
        .grid
        .at(20)
        .expect("a point")
        .0;
    s.execute(&AnyCommand {
        name: "bus.place_ports".into(),
        params: json!({
            "page": page.to_string(), "symbol": symbol.to_string(), "pos": [on, on],
            "target": {"from_part": {"part": part.id.as_uuid().to_string(), "letter": "A"}}, "copies": copies,
        }),
    })
    .expect("bus.place_ports");
    let q = s.query();
    let mut ports: Vec<(String, String)> = q
        .functions()
        .iter()
        .filter_map(|(_, f)| {
            let p = f.bus_port.as_ref()?;
            let def = q.fndefs().get(f.def)?.item.meta.key.as_ref()?.as_str().to_owned();
            Some((p.interface.as_str().to_owned(), def))
        })
        .collect();
    ports.sort();
    assert_eq!(
        ports,
        [
            ("DP".to_owned(), "ew.fn.dp-port".to_owned()),
            ("X1".to_owned(), "ew.fn.pn-port".to_owned())
        ]
    );
}

/// The coupler's part CPL-PN-DP names a bus symbol of one's own, made from the program's with its DP socket moved to the
/// bottom edge: the page draws the part's device with it, as it is. While it draws every socket of the part's network
/// ports EW-BUS-011 says nothing; once X1 counts three ports it names X1 P3 and offers the way to the drawing.
#[test]
fn a_part_s_own_bus_symbol_draws_its_devices_and_ew_bus_011_names_what_it_lacks() {
    let own_item = 0x9500;
    let copy = ew_ids::v5::lib_copy_id::<()>(
        uid(0x9000).parse().expect("a uuid"),
        uid(own_item).parse().expect("a uuid"),
    )
    .as_uuid();
    let with_own = |x1_ports: u16| {
        let (project, records, mut lib) = parts("rio1");
        let coupler = |ports: u16| {
            let mut p = part(
                COUPLER,
                "CPL-PN-DP",
                json!([{"interface": "X1", "bus": "profinet", "role": "device", "ports": ports},
                       {"interface": "DP", "bus": "profibus_dp", "role": "slave"}]),
            );
            p["bus_symbol"] = json!({"library": uid(0x9000), "item": uid(own_item), "version_req": "^1"});
            p
        };
        let made: ew_schema::Part = serde_json::from_value(coupler(2)).expect("a part");
        let mut own = ew_derive::bus_symbol(&made, ew_units::Grid::QET_10PX, &[]);
        let bottom = own.cps.iter().map(|c| c.pos.y.0).max().expect("a socket");
        let mut cps: Vec<ew_schema::SymbolCp> = own.cps.iter().cloned().collect();
        for c in cps
            .iter_mut()
            .filter(|c| c.interface.as_ref().map(|i| i.as_str()) == Some("DP"))
        {
            c.pos = Pt2::new(c.pos.x, ew_units::Nm(bottom));
            c.dir = ew_schema::Dir4::Down;
        }
        own.cps = ew_schema::KeyedList::from_vec(cps).expect("unique");
        let mut doc = serde_json::to_value(&own).expect("json");
        for (k, v) in [
            ("$k", json!("symbol")),
            ("id", json!(copy.to_string())),
            ("library_id", json!(uid(0x9000))),
            ("item_id", json!(uid(own_item))),
            ("version", json!("1.0.0")),
            ("sha256", json!(format!("sha256:{}", "2".repeat(64)))),
        ] {
            doc[k] = v;
        }
        let swap =
            |d: &LibDocument| matches!(d, LibDocument::Part(p) if p.item.part_number.as_str() == "CPL-PN-DP");
        lib.retain(|d| !swap(d));
        lib.push(serde_json::from_value(coupler(x1_ports)).expect("the coupler"));
        lib.push(serde_json::from_value(doc).expect("the own symbol"));
        (project, records, lib, own)
    };
    let model =
        |(project, records, lib, _): (Project, Vec<Record>, Vec<LibDocument>, ew_schema::SymbolDef)| {
            Model::from_records(project, records, lib).unwrap_or_else(|e| panic!("model: {e:?}"))
        };
    let rule = ew_checks::rule("EW-BUS-011").expect("EW-BUS-011");
    let own = with_own(2).3;
    let made = model(with_own(2));
    let q = Q::new(&made);
    let stored: Representation = (**q.reps().get(id(COUPLED)).expect("the coupler")).clone();
    let (drawn, rep) = ew_derive::drawn(&q, &stored).expect("drawn");
    assert_eq!(
        drawn.cps, own.cps,
        "drawn with the part's own symbol, its DP socket on the bottom edge"
    );
    assert_eq!(rep.pin_map.len(), 3, "each socket draws its port function");
    assert!(
        rule.run(&q, &ew_checks::Scope::Project).is_empty(),
        "every socket drawn"
    );
    let made = model(with_own(3));
    let q = Q::new(&made);
    let found = rule.run(&q, &ew_checks::Scope::Project);
    assert_eq!(found.len(), 1, "{found:?}");
    let text = ew_checks::render(&found[0], "en-GB");
    assert!(text.contains("X1 P3") && text.contains("CPL-PN-DP"), "{text}");
    let fix = found[0].fix.as_ref().expect("Open the bus symbol");
    assert_eq!(fix.command, "parts.open");
    // Properties of the coupler says it is drawn with the part's own symbol, and its link opens that drawing.
    let (project, records, lib, _) = with_own(2);
    let s = ew_api::Session::from_records(project, records, lib, ew_api::SessionOptions::default())
        .expect("a session");
    let sheet = s.property_sheet(&[id::<()>(COUPLED).as_uuid()]);
    let drawn = sheet
        .sections
        .iter()
        .flat_map(|x| x.rows.iter())
        .find(|r| r.key == "representation/derived.bus_symbol")
        .expect("Drawn with");
    let ew_api::output::RowValue::One { json, .. } = &drawn.value else {
        panic!("one value")
    };
    assert!(json["own"].is_string(), "his own, by its name: {json}");
    let link = drawn.link.as_ref().expect("the way to the drawing");
    assert_eq!(
        link.actions,
        [(
            "parts.open",
            json!({"symbol": format!("library:{}:{}", uid(0x9000), uid(own_item))})
        )]
    );
}

/// The part's card "On a bus page" pictures its bus symbol as a page prints it: with its texts, shaped by the page's
/// means (the part in the header, each compartment's interface, the sockets' labels, an address by its placeholder).
#[test]
fn the_picture_on_a_bus_page_prints_the_symbol_s_texts() {
    let (project, records, lib) = parts("rio1");
    let head: ew_schema::Part = lib
        .iter()
        .find_map(|d| match d {
            LibDocument::Part(p) if p.item.part_number.as_str() == "HS-2" => Some(p.item.clone()),
            _ => None,
        })
        .expect("HS-2");
    let s = ew_api::Session::from_records(project, records, lib, ew_api::SessionOptions::default())
        .expect("a session");
    let picture = s.part_bus_symbol(&head).expect("network ports");
    fn runs(items: &[ew_dl::Item], out: &mut Vec<String>) {
        for i in items {
            match &i.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(g) => runs(&g.items, out),
                _ => {}
            }
        }
    }
    let mut texts = Vec::new();
    runs(&picture.list.items, &mut texts);
    for printed in ["HS-2", "X1", "X2", "P1", "P2", "TBD"] {
        assert!(
            texts.iter().any(|t| t.contains(printed)),
            "{printed} in {texts:?}"
        );
    }
}

/// A project whose small data text is twice its size (its Text styles page): a device with a two-line address and a
/// socket on its bottom edge still prints every text inside its body, so none can reach a socket's stub: the bands are
/// sized from the project's own text styles, as the page prints.
#[test]
fn a_project_with_larger_texts_keeps_every_text_of_a_bus_symbol_inside_its_body() {
    let (mut project, records, lib) = parts("rio1");
    let doubled = ew_schema::qet_px_em(18);
    project.settings.text_styles.insert(
        "data.small".parse().expect("a key"),
        ew_schema::TextStyleSpec::of_size(doubled),
    );
    let model = Model::from_records(project, records, lib).unwrap_or_else(|e| panic!("model: {e:?}"));
    let q = Q::new(&model);
    for n in [UPRIGHT, COUPLED] {
        let body = body(&q, n);
        let texts = texts(&q, n);
        assert!(texts.iter().any(|(t, _)| t.contains("192.168.")), "{texts:?}");
        for (i, (t, b)) in texts.iter().enumerate() {
            assert!(inside(*b, body), "{t:?} {b:?} leaves the body {body:?}");
            for (u, c) in &texts[i + 1..] {
                assert!(!overlap(*b, *c), "{t:?} lies on {u:?}");
            }
        }
    }
}

/// Draw as one symbol replaces the boxes an earlier version drew on a bus page, and nothing else: a box of the same
/// switch's port function on a schematic page (the device browser's unplaced function drawn there) is no bus box.
#[test]
fn only_a_bus_page_s_boxes_are_what_draw_as_one_symbol_replaces() {
    let (project, mut records, mut lib) = parts("rio1");
    let frame = ew_library::starter::item("ew.gen.blackbox@qet10").expect("the black-box frame");
    let frame: Value = serde_json::from_str(frame.json).expect("json");
    let frame_copy = frame["id"].as_str().expect("an id").to_owned();
    lib.push(serde_json::from_value(frame).expect("the frame's copy"));
    let more = [
        json!({"$k": "page", "id": uid(0x11), "folio": {"number": 101}, "kind": "multi_line", "structure": {},
            "title": {"en": "Supply"}, "grid": {"num": 7_937_500, "den": 3}}),
        json!({"$k": "device", "id": uid(0x90), "tag": {"letter": "K", "counter": 2},
            "parts": [{"part": {"copy": uid(SWITCH)}, "qty": "1"}]}),
        json!({"$k": "function", "id": uid(0x91), "device": uid(0x90), "def": uid(PORT_DEF), "role": "main",
            "bus_port": profinet("X1", "sw2", "192.168.0.3")}),
        json!({"$k": "device", "id": uid(0x94), "tag": {"letter": "K", "counter": 3},
            "parts": [{"part": {"copy": uid(SWITCH)}, "qty": "1"}]}),
        json!({"$k": "function", "id": uid(0x95), "device": uid(0x94), "def": uid(PORT_DEF), "role": "main",
            "bus_port": profinet("X1", "sw3", "192.168.0.4")}),
    ];
    let boxed = |n: u128, function: u128, page: u128, rep_type: &str| {
        json!({"$k": "representation", "id": uid(n), "functions": [uid(function)], "page": uid(page),
            "symbol": frame_copy, "pos": [10 * STEP, 70 * STEP], "rep_type": rep_type,
            "box": {"outline": [[0, 0], [6 * STEP, 0], [6 * STEP, 3 * STEP], [0, 3 * STEP]],
                    "cps": [{"idx": 0, "pos": [3 * STEP, 0], "dir": "up"}]}})
    };
    for r in more.into_iter().chain([
        boxed(0x46, 0x91, 0x11, "multi_line"),
        boxed(0x47, 0x95, PAGE, "topology"),
    ]) {
        records.push(serde_json::from_value(r.clone()).unwrap_or_else(|e| panic!("{r}: {e}")));
    }
    let s = ew_api::Session::from_records(project, records, lib, ew_api::SessionOptions::default())
        .expect("a session");
    assert_eq!(
        s.bus_boxes("105"),
        [(id::<()>(0x47).as_uuid(), id::<()>(0x94).as_uuid())],
        "the bus page's box"
    );
    assert!(
        s.bus_boxes("101").is_empty(),
        "a schematic page's box stays a box"
    );
}
