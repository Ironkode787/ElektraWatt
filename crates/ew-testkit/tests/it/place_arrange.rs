// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks of the placement and arrangement commands on a whole project, the sample: `rep.move` by grid steps,
//! `rep.exchange_symbol`, the `new_terminal` and `from_part` targets, and the DSL verbs for them; and the
//! two-function part fixtures that the `ew-api` and `ew-viewmodel` tests read.
//!
//! Every test edits a temporary COPY of `fixtures/sample/sample.ew`; the committed directory is never opened for
//! writing. The tests that follow routes give the wires of the representations they move a route of their own
//! first, with `connection.route`.

use std::path::Path;
use std::sync::Arc;

use ew_api::{AnyCommand, CmdError, Receipt, Recovery, Session, SessionOptions, StoreOptions};
use ew_derive::{FunctionCps, Q, cp_point};
use ew_format::library_copy_hash;
use ew_ids::{Id, Uuid};
use ew_mltext::Ident;
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, Extra, FnRole, Function, FunctionKind, IpDirection, IpSpec,
    Kind, LibDocument, Page, Part, PartUse, Representation, Sha256, TemplateFn,
};
use ew_testdir::{CONTENT, Tmp};
use ew_testkit::dsl::{Builder, Done, End, IpEnd};
use ew_units::{Grid, Nm, Pt2};
use serde_json::{Value, json};

/// The grid of every page of the sample: the QET grid.
fn grid() -> Grid {
    Grid::new(7_937_500, 3).expect("the QET grid")
}

fn grid_at(k: i64) -> Nm {
    grid().at(k).expect("on the grid")
}

fn gp(kx: i64, ky: i64) -> Pt2 {
    Pt2::new(grid_at(kx), grid_at(ky))
}

// ---------------------------------------------------------------------------------------------------------
// A temporary copy of the sample, and the records the checks use, found by what they are (the sample's IDs come
// from its script)

/// A session over a copy of the sample in `work`.
fn open_copy(work: &Tmp, seed: u64) -> Session {
    let dir = work.sample("sample.ew");
    let mut store = StoreOptions::new("ElektraWatt test");
    store.state_dir = Some(work.path("state"));
    let options = SessionOptions {
        ids: Some(ew_testids::seeded_ids(seed)),
        ..SessionOptions::default()
    };
    Session::open(&dir, store, options, Recovery::Keep)
        .expect("the copy of the sample opens")
        .0
}

/// The page of folio `number`.
fn page_of(s: &Session, number: u32) -> Id<Page> {
    s.query()
        .pages()
        .iter()
        .find(|(_, p)| p.folio.number == number)
        .map(|(id, _)| id)
        .expect("the folio")
}

/// The copy of the part `number` the sample holds.
fn part_copy(s: &Session, number: &str) -> Arc<ew_schema::LibCopy<Part>> {
    s.query()
        .parts()
        .iter()
        .find(|(_, c)| c.item.part_number.as_str() == number)
        .map(|(_, c)| Arc::clone(c))
        .unwrap_or_else(|| panic!("the sample holds {number}"))
}

/// The representations on `page` of the devices that use the part `number`.
fn reps_of_part(s: &Session, page: Id<Page>, number: &str) -> Vec<Id<Representation>> {
    let copy = part_copy(s, number).id;
    let q = s.query();
    q.reps()
        .iter()
        .filter(|(_, r)| r.page == page)
        .filter(|(_, r)| {
            r.functions.iter().any(|f| {
                q.functions()
                    .get(*f)
                    .and_then(|f| f.device)
                    .and_then(|d| q.devices().get(d))
                    .is_some_and(|d| d.parts.iter().any(|u| u.part.copy == copy))
            })
        })
        .map(|(id, _)| id)
        .collect()
}

/// -X1021, the sample's 24 V / 0 V strip on 102 (terminals 1 to 8).
fn strip_x1021(s: &Session) -> Id<Device> {
    Id::from_uuid(s.resolve_reference("tag:-X1021").expect("-X1021"))
}

fn run(s: &mut Session, name: &str, params: Value) -> Result<Receipt, CmdError> {
    s.execute(&AnyCommand {
        name: name.to_owned(),
        params,
    })
}

#[track_caller]
fn must(s: &mut Session, name: &str, params: Value) -> Receipt {
    run(s, name, params.clone()).unwrap_or_else(|e| panic!("{name} {params}: {e}"))
}

/// Every row of the session's model as canonical lines: equal lines mean an unchanged model.
fn lines(s: &Session) -> Vec<String> {
    ew_commands::rows::model_lines(&s.world().model)
        .expect("every record has a JSON form")
        .into_iter()
        .map(|(k, line)| format!("{}:{} {line}", k.0.name(), k.1))
        .collect()
}

fn rep(s: &Session, id: Id<Representation>) -> Arc<Representation> {
    s.query().reps().get(id).cloned().expect("the representation")
}

fn connection(s: &Session, id: Id<Connection>) -> Arc<Connection> {
    s.query().connections().get(id).cloned().expect("the connection")
}

fn device(s: &Session, id: Id<Device>) -> Arc<Device> {
    s.query().devices().get(id).cloned().expect("the device")
}

fn function(s: &Session, id: Id<Function>) -> Arc<Function> {
    s.query().functions().get(id).cloned().expect("the function")
}

fn created<T>(r: &Receipt, kind: Kind) -> Vec<Id<T>> {
    r.created
        .iter()
        .filter(|k| k.0 == kind)
        .map(|k| k.1.cast())
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Routes

/// The point of an endpoint on `page`.
fn end_pos(q: &Q, e: &Endpoint, page: Id<Page>) -> Pt2 {
    match e {
        Endpoint::Cp(f, idx) => cp_point(q, *f, *idx, page).expect("a drawn CP"),
        Endpoint::Junction(j) => q.junctions().get(*j).expect("the junction").at,
        Endpoint::Unknown(_) => panic!("an unknown endpoint"),
    }
}

/// An orthogonal route between two points with at least one bend at each end, so that a moved end follows its CP by
/// moving its neighbour (`follow_end`; a straight two-vertex route would take a dog-leg instead): one bend
/// when the points are not aligned, else a U-shaped detour three grid steps away. Every bend shares a coordinate with
/// an end or lies on the grid.
fn l_route(a: Pt2, b: Pt2) -> Vec<Pt2> {
    let g = grid();
    let beyond = |x: Nm| g.at(g.snap(x) + 3).expect("on the grid");
    if a.x != b.x && a.y != b.y {
        vec![a, Pt2::new(b.x, a.y), b]
    } else if a.y == b.y {
        let y = beyond(a.y);
        vec![a, Pt2::new(a.x, y), Pt2::new(b.x, y), b]
    } else {
        let x = beyond(a.x);
        vec![a, Pt2::new(x, a.y), Pt2::new(x, b.y), b]
    }
}

/// The connections on `page` with an end at a function of `functions`.
fn connections_at(s: &Session, page: Id<Page>, functions: &[Id<Function>]) -> Vec<Id<Connection>> {
    let q = s.query();
    q.connections()
        .iter()
        .filter(|(_, c)| c.page == Some(page))
        .filter(|(_, c)| {
            [&c.a, &c.b]
                .into_iter()
                .any(|e| matches!(e, Endpoint::Cp(f, _) if functions.contains(f)))
        })
        .map(|(id, _)| id)
        .collect()
}

/// Gives each connection a one-bend route between its ends with `connection.route`.
fn route_all(s: &mut Session, page: Id<Page>, connections: &[Id<Connection>]) {
    for c in connections {
        let route = {
            let q = s.query();
            let w = q.connections().get(*c).cloned().expect("the connection");
            l_route(end_pos(&q, &w.a, page), end_pos(&q, &w.b, page))
        };
        must(s, "connection.route", json!({"connection": c, "route": route}));
    }
}

/// Every routed connection of `page` starts at its endpoint `a`, ends at its endpoint `b` and has axis-parallel
/// segments only.
#[track_caller]
fn assert_routes_meet_their_ends(s: &Session, page: Id<Page>) -> usize {
    let q = s.query();
    let mut routed = 0;
    for (id, c) in q.connections().iter() {
        if c.page != Some(page) || c.route.is_empty() {
            continue;
        }
        routed += 1;
        assert_eq!(
            c.route.first().copied(),
            Some(end_pos(&q, &c.a, page)),
            "{id}: route start"
        );
        assert_eq!(
            c.route.last().copied(),
            Some(end_pos(&q, &c.b, page)),
            "{id}: route end"
        );
        // No segment leans.
        for w in c.route.windows(2) {
            assert!(
                w[0].x == w[1].x || w[0].y == w[1].y,
                "{id}: a diagonal segment {w:?}"
            );
        }
    }
    routed
}

/// `Grid::at(Grid::snap(x) + k)` of a point.
fn stepped(pos: Pt2, steps: [i64; 2]) -> Pt2 {
    let g = grid();
    Pt2::new(
        g.at(g.snap(pos.x) + steps[0]).expect("in range"),
        g.at(g.snap(pos.y) + steps[1]).expect("in range"),
    )
}

// ---------------------------------------------------------------------------------------------------------
// a. rep.move by grid steps

#[test]
fn rep_move_by_steps_lands_on_the_grid_where_one_nm_delta_cannot() {
    let work = Tmp::new("ew-testkit-place-arrange");
    let mut s = open_copy(&work, 1);
    let page = page_of(&s, 101);
    let g = grid();
    // Three representations on 101 whose x grid indices have three different residues modulo 3 (so any uniform nm
    // delta leaves at least one off the grid), each on the grid and with a wire. The sample's script places on the
    // cells' centres (residue 2), so three of residue 2 are taken and two of them first moved right by the nm delta of
    // one and of two steps. Each one's wires have their other ends 0 or at least 3 grid steps away in x and 0 or at
    // least 2 in y (after its first move): the test's moves (at most 2 steps right and 1 down) then never put a bend
    // on its neighbour, which `follow_end` would merge.
    let clear = |s: &Session, f: Id<Function>, shift: i64| -> bool {
        let q = s.query();
        connections_at(s, page, &[f]).iter().all(|c| {
            let w = q.connections().get(*c).expect("the connection");
            let k = |e: &Endpoint| {
                let p = end_pos(&q, e, page);
                let moved = matches!(e, Endpoint::Cp(at, _) if *at == f);
                (g.snap(p.x) + if moved { shift } else { 0 }, g.snap(p.y))
            };
            let ((ax, ay), (bx, by)) = (k(&w.a), k(&w.b));
            let (dx, dy) = ((ax - bx).abs(), (ay - by).abs());
            (dx == 0 || dx >= 3) && (dy == 0 || dy >= 2)
        })
    };
    let wired = |s: &Session, residue: i64, shift: i64| -> Vec<Id<Representation>> {
        let q = s.query();
        q.reps()
            .iter()
            .filter(|(_, r)| r.page == page && !r.functions.is_empty())
            .filter(|(_, r)| g.snap_point(r.pos.x) == Some(r.pos.x) && g.snap_point(r.pos.y) == Some(r.pos.y))
            .filter(|(_, r)| g.snap(r.pos.x).rem_euclid(3) == residue)
            .filter(|(_, r)| !connections_at(s, page, &r.functions[..1]).is_empty())
            .filter(|(_, r)| clear(s, r.functions[0], shift))
            .map(|(id, _)| id)
            .collect()
    };
    let mut reps: Vec<Id<Representation>> = Vec::new();
    for shift in [1, 2, 0] {
        let r = wired(&s, 2, shift)
            .into_iter()
            .find(|r| !reps.contains(r))
            .expect("a wired representation of residue 2 on 101");
        // One step's nm delta from residue 2 lands on residue 0, two steps' on residue 1 (both on the grid).
        if shift > 0 {
            must(
                &mut s,
                "rep.move",
                json!({"reps": [r], "delta": [grid_at(1).0 * shift, 0]}),
            );
        }
        reps.push(r);
    }
    let mods: Vec<i64> = reps
        .iter()
        .map(|r| g.snap(rep(&s, *r).pos.x).rem_euclid(3))
        .collect();
    assert_eq!(mods, vec![0, 1, 2]);
    for r in &reps {
        let pos = rep(&s, *r).pos;
        assert_eq!(g.snap_point(pos.x), Some(pos.x));
        assert_eq!(g.snap_point(pos.y), Some(pos.y));
    }
    // Give the wires at these functions a route of the test's own (the sample's routes are drawn ones).
    let functions: Vec<Id<Function>> = reps.iter().map(|r| rep(&s, *r).functions[0]).collect();
    let wires = connections_at(&s, page, &functions);
    assert!(!wires.is_empty(), "wires at the three functions");
    route_all(&mut s, page, &wires);
    let routed = assert_routes_meet_their_ends(&s, page);
    assert!(routed >= wires.len(), "{routed}");
    let ends: Vec<(Endpoint, Endpoint)> = wires
        .iter()
        .map(|w| (connection(&s, *w).a.clone(), connection(&s, *w).b.clone()))
        .collect();
    let vertices: Vec<usize> = wires.iter().map(|w| connection(&s, *w).route.len()).collect();

    // One nm delta of one step (7_937_500 / 3 = 2_645_833.33 nm, rounded) is refused: the representation of
    // residue 1 would land 1 nm off.
    let one_step = grid_at(1);
    assert_eq!(one_step.0, 2_645_833);
    let off = rep(&s, reps[1]).pos;
    let mm = |n: Nm| n.0 as f64 / 1e6;
    let before = lines(&s);
    match run(
        &mut s,
        "rep.move",
        json!({"reps": reps, "delta": [one_step.0, 0]}),
    ) {
        Err(CmdError::Rejected { reason, .. }) => {
            assert_eq!(
                reason,
                format!(
                    "the position at x {:.1}, y {:.1} mm on page 101 is not on the drawing grid; move it onto the grid",
                    mm(Nm(off.x.0 + one_step.0)),
                    mm(off.y)
                )
            );
        }
        other => panic!("the nm delta must be refused, got {other:?}"),
    }
    assert_eq!(lines(&s), before, "a refused move writes nothing");

    // The same move as one grid step along x, then along y, then both ways: each position is exactly the grid
    // point at its index plus the steps, and the routes follow.
    for steps in [[1, 0], [0, 1], [-1, -1], [2, 0]] {
        let old: Vec<Pt2> = reps.iter().map(|r| rep(&s, *r).pos).collect();
        must(&mut s, "rep.move", json!({"reps": reps, "steps": steps}));
        for (r, old) in reps.iter().zip(&old) {
            assert_eq!(rep(&s, *r).pos, stepped(*old, steps), "{r} by {steps:?}");
        }
        assert_eq!(assert_routes_meet_their_ends(&s, page), routed, "{steps:?}");
        for ((w, e), n) in wires.iter().zip(&ends).zip(&vertices) {
            let c = connection(&s, *w);
            assert_eq!(
                (c.a.clone(), c.b.clone()),
                e.clone(),
                "moves never change connectivity"
            );
            assert_eq!(c.route.len(), *n, "the routes keep their vertices");
        }
    }
    // One undo takes back one move.
    let before_last = lines(&s);
    must(&mut s, "rep.move", json!({"reps": reps, "steps": [1, 1]}));
    s.undo(&ew_api::Origin::Gui).expect("undo");
    assert_eq!(lines(&s), before_last);
}

#[test]
fn rep_move_by_steps_refuses_a_delta_or_a_page_beside_it() {
    let work = Tmp::new("ew-testkit-place-arrange");
    let mut s = open_copy(&work, 2);
    let (page_101, page_102) = (page_of(&s, 101), page_of(&s, 102));
    let any = s
        .query()
        .reps()
        .iter()
        .find(|(_, r)| r.page == page_101)
        .map(|(id, _)| id)
        .expect("a representation on 101");
    let before = lines(&s);
    for params in [
        json!({"reps": [any], "steps": [1, 0], "delta": [7_937_500, 0]}),
        json!({"reps": [any], "steps": [1, 0], "page": page_102}),
    ] {
        match run(&mut s, "rep.move", params) {
            Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/steps"),
            other => panic!("expected InvalidParams at /steps, got {other:?}"),
        }
    }
    assert_eq!(lines(&s), before);
}

// ---------------------------------------------------------------------------------------------------------
// b. rep.exchange_symbol

const LIB_TEST: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0001);
const ITEM_FN_SIGNAL: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0010);
const ITEM_SYM_SIGNAL: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0011);
const ITEM_FN_ONE: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0012);
const ITEM_SYM_ONE: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0013);
const ITEM_PART_TWO: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0020);
/// The two-function part fixture of `crates/ew-api/tests/expected/two_fndef_part.json`.
const ITEM_PART_TWO_FX: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0030);
/// The fixture whose contact FnDef no library holds (`missing_fndef_part.json`).
const ITEM_PART_MISSING_FX: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0031);
/// The FnDef `test.fn.unknown` of the missing-entry fixture; no library holds it.
const ITEM_FN_UNKNOWN: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0032);
/// The two-starter-FnDef part fixture of `crates/ew-api/tests/expected/two_starter_fndef_part.json`.
const ITEM_PART_STARTER_FX: Uuid = Uuid::from_u128(0x7e57_0000_0000_4000_8000_0000_0000_0033);

fn copy_id<T>(item: Uuid) -> Id<T> {
    ew_ids::v5::lib_copy_id(LIB_TEST, item)
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A library document of `lib/` JSON form for `library.embed`: its `sha256` is the hash of the item's members.
fn embeddable(json: Value) -> Value {
    let doc = LibDocument::from_value(json.clone()).expect("a library document");
    let hash = library_copy_hash(&doc).expect("a hashable kind");
    let mut json = json;
    json["sha256"] = Value::String(format!("sha256:{}", hex(&hash)));
    json
}

/// A fixture FnDef copy of `LIB_TEST` with these CP designations (in index order).
fn fndef_json(item: Uuid, key: &str, cps: &[&str]) -> Value {
    embeddable(json!({
        "$k": "fndef",
        "id": copy_id::<()>(item).to_string(),
        "library_id": LIB_TEST.to_string(),
        "item_id": item.to_string(),
        "version": "0.1.0",
        "sha256": format!("sha256:{}", hex(&[0; 32])),
        "meta": {"key": key, "licence": "CC-BY-SA-4.0", "sources": ["ew.original"], "status": "draft"},
        "category": ["Lighting", "Lamp", "Cabinet"],
        "contact_kind": "pole",
        "cps": cps.iter().enumerate()
            .map(|(i, d)| json!({"idx": i, "designation": d})).collect::<Vec<_>>(),
    }))
}

/// A fixture symbol copy drawing `fndef_item`, with CPs `(idx, fn_cp, pos)`.
fn symbol_json(item: Uuid, key: &str, fndef_item: Uuid, cps: &[(u16, u16, [i64; 2])]) -> Value {
    embeddable(json!({
        "$k": "symbol",
        "id": copy_id::<()>(item).to_string(),
        "library_id": LIB_TEST.to_string(),
        "item_id": item.to_string(),
        "version": "0.1.0",
        "sha256": format!("sha256:{}", hex(&[0; 32])),
        "meta": {"key": key, "licence": "CC-BY-SA-4.0", "sources": ["ew.original"], "status": "draft"},
        "name": {"en": "Signal lamp"},
        "fndef": {"library": LIB_TEST.to_string(), "item": fndef_item.to_string(), "version_req": "^0.1"},
        "grid": {"num": 7_937_500, "den": 3},
        "graphics": [],
        "cps": cps.iter()
            .map(|(idx, fn_cp, pos)| json!({"idx": idx, "fn_cp": fn_cp, "pos": pos, "dir": "up"}))
            .collect::<Vec<_>>(),
        "text_slots": [],
        "arrangements": [],
    }))
}

#[test]
fn exchange_symbol_on_a_lamp_keeps_its_connections_and_follows_the_new_cps() {
    const G: i64 = 7_937_500;
    let work = Tmp::new("ew-testkit-place-arrange");
    let mut s = open_copy(&work, 3);
    // The pilot lamp on 102, in parallel with the contactor's coil.
    let page = page_of(&s, 102);
    let [rep_id] = reps_of_part(&s, page, "EW-GEN-LMP-1")[..] else {
        panic!("one pilot lamp on 102");
    };
    let function_id = rep(&s, rep_id).functions[0];
    // The lamp has the FnDef CPs X1 (0) and X2 (1), drawn at the insertion point and 7.9375 mm below it.
    let before = rep(&s, rep_id);
    let at = before.pos;
    assert_eq!(before.functions, vec![function_id]);
    assert!(before.pin_map.as_slice().is_empty());
    let q = s.query();
    let old_x1 = cp_point(&q, function_id, 0, page).expect("X1 drawn");
    let old_x2 = cp_point(&q, function_id, 1, page).expect("X2 drawn");
    drop(q);
    assert_eq!((old_x1, old_x2), (at, Pt2::new(at.x, Nm(at.y.0 + G))));
    // Its two connections, given routes.
    let wires = connections_at(&s, page, &[function_id]);
    assert_eq!(wires.len(), 2, "the lamp has two connections");
    route_all(&mut s, page, &wires);
    // Every routed wire of the page meets its ends.
    let routed = assert_routes_meet_their_ends(&s, page);
    let ends: Vec<(Endpoint, Endpoint)> = wires
        .iter()
        .map(|w| (connection(&s, *w).a.clone(), connection(&s, *w).b.clone()))
        .collect();

    // The fixture symbol `test.signal-lamp@qet10` with its own FnDef: designations X2 (index 0) and X1 (index 1),
    // CPs at other positions (X2 at (G, 0), X1 at (0, G)). A second symbol with a FnDef that lacks X2.
    must(
        &mut s,
        "library.embed",
        json!({"copies": [
            fndef_json(ITEM_FN_SIGNAL, "test.fn.signal-lamp", &["X2", "X1"]),
            symbol_json(ITEM_SYM_SIGNAL, "test.signal-lamp@qet10", ITEM_FN_SIGNAL,
                &[(0, 0, [G, 0]), (1, 1, [0, G])]),
            fndef_json(ITEM_FN_ONE, "test.fn.lamp-x1", &["X1"]),
            symbol_json(ITEM_SYM_ONE, "test.lamp-x1@qet10", ITEM_FN_ONE, &[(0, 0, [0, 0])]),
        ]}),
    );

    // The symbol that lacks X2 is refused: X2 is connected on this page and would be left unmapped.
    let unchanged = lines(&s);
    match run(
        &mut s,
        "rep.exchange_symbol",
        json!({"rep": rep_id, "symbol": copy_id::<()>(ITEM_SYM_ONE)}),
    ) {
        Err(CmdError::Rejected { reason, .. }) => {
            assert!(
                reason.contains("function CP 1 is connected on this page"),
                "{reason}"
            );
        }
        other => panic!("the exchange must be refused, got {other:?}"),
    }
    assert_eq!(lines(&s), unchanged, "a refused exchange writes nothing");

    // The exchange to the signal lamp.
    let symbol: Id<ew_schema::SymbolDef> = copy_id(ITEM_SYM_SIGNAL);
    let r = must(
        &mut s,
        "rep.exchange_symbol",
        json!({"rep": rep_id, "symbol": symbol}),
    );
    assert_eq!(r.outcome.primary, Some((Kind::Representation, rep_id.cast())));
    assert!(r.outcome.notices.is_empty(), "{:?}", r.outcome.notices);
    let after = rep(&s, rep_id);
    assert_eq!((after.symbol, after.variant, after.pos), (symbol, 0, at));
    assert_eq!(after.functions, before.functions, "the function is kept");
    // By designation: symbol CP 0 draws "X2" (function CP 1), symbol CP 1 draws "X1" (function CP 0).
    let map: Vec<(u16, Id<Function>, u16)> = after
        .pin_map
        .iter()
        .map(|p| (p.symbol_cp, p.function, p.idx))
        .collect();
    assert_eq!(map, vec![(0, function_id, 1), (1, function_id, 0)]);
    // The CPs moved, and the connections kept their endpoints and follow them.
    let q = s.query();
    let new_x1 = cp_point(&q, function_id, 0, page).expect("X1 drawn");
    let new_x2 = cp_point(&q, function_id, 1, page).expect("X2 drawn");
    drop(q);
    assert_eq!(
        new_x1,
        Pt2::new(at.x, Nm(at.y.0 + G)),
        "X1 is drawn by symbol CP 1"
    );
    assert_eq!(
        new_x2,
        Pt2::new(Nm(at.x.0 + G), at.y),
        "X2 is drawn by symbol CP 0"
    );
    assert_ne!((new_x1, new_x2), (old_x1, old_x2));
    assert_eq!(
        connections_at(&s, page, &[function_id]),
        wires,
        "the same two connections"
    );
    for (w, e) in wires.iter().zip(&ends) {
        let c = connection(&s, *w);
        assert_eq!((c.a.clone(), c.b.clone()), e.clone(), "the same endpoints");
    }
    assert_eq!(
        assert_routes_meet_their_ends(&s, page),
        routed,
        "both routes end at the new CP positions"
    );
    // One undo takes the exchange back.
    s.undo(&ew_api::Origin::Gui).expect("undo");
    assert_eq!(lines(&s), unchanged);
}

// ---------------------------------------------------------------------------------------------------------
// c. new_terminal on strip -X1021

#[test]
fn new_terminal_adds_one_terminal_to_strip_x1021_with_the_next_sequence_number() {
    let work = Tmp::new("ew-testkit-place-arrange");
    let mut s = open_copy(&work, 4);
    let page = page_of(&s, 102);
    let strip_id = strip_x1021(&s);
    let strip = device(&s, strip_id);
    // The symbol and the definition the strip's terminals are drawn with (`ew.iec.terminal@qet10`, `ew.fn.terminal`).
    let (symbol_terminal, fndef_terminal) = {
        let q = s.query();
        let terminal = q.idx().terminals_of_strip(strip_id).next().expect("a terminal");
        let f = q
            .idx()
            .functions_of_device(terminal)
            .next()
            .expect("its function");
        let r = q.idx().reps_of_function(f).next().expect("its representation");
        (
            q.reps().get(r).expect("the representation").symbol,
            q.functions().get(f).expect("the function").def,
        )
    };
    let terminals = |s: &Session| -> Vec<(u32, String)> {
        let q = s.query();
        let mut v: Vec<(u32, String)> = q
            .devices()
            .iter()
            .filter_map(|(_, d)| d.aspects.terminal.as_ref().cloned())
            .filter(|t| t.strip == strip_id)
            .map(|t| (t.seq, t.designation.as_str().to_owned()))
            .collect();
        v.sort();
        v
    };
    let before = terminals(&s);
    // The sample's script places the terminals 1 to 8 on -X1021.
    assert_eq!(before.len(), 8);
    assert_eq!(before.last(), Some(&(8, "8".to_owned())));

    let r = must(
        &mut s,
        "device.place",
        json!({"page": page, "symbol": symbol_terminal, "pos": gp(10, 20),
            "target": {"new_terminal": {"strip": strip_id, "designation": "9"}}}),
    );
    let devices: Vec<Id<Device>> = created(&r, Kind::Device);
    let functions: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!(
        (devices.len(), functions.len()),
        (1, 1),
        "one new terminal with its main function"
    );
    let after = terminals(&s);
    assert_eq!(after.len(), 9);
    assert_eq!(after.last(), Some(&(9, "9".to_owned())), "seq = max + 1");
    let t = device(&s, devices[0]);
    assert_eq!(t.tag.letter, strip.tag.letter, "letter X");
    assert_eq!(t.tag.letter.as_str(), "X");
    assert_eq!(t.kind, Some(ew_schema::DeviceKind::Terminal));
    assert_eq!(t.structure, strip.structure, "the strip's structure");
    let aspect = t.aspects.terminal.clone().expect("the terminal aspect");
    assert_eq!(
        (aspect.strip, aspect.seq, aspect.rail_bonded),
        (strip_id, 9, false)
    );
    let f = function(&s, functions[0]);
    assert_eq!(
        (f.device, f.def, f.role.clone()),
        (Some(devices[0]), fndef_terminal, FnRole::Main),
        "the main function has the symbol's FnDef"
    );
    let placed = rep(&s, r.outcome.primary.expect("the representation").1.cast());
    assert_eq!(
        (placed.page, placed.functions.clone(), placed.pos, placed.symbol),
        (page, vec![functions[0]], gp(10, 20), symbol_terminal)
    );
    // A used designation, a used sequence number and a non-strip are refused.
    let unchanged = lines(&s);
    for target in [
        json!({"new_terminal": {"strip": strip_id, "designation": "9"}}),
        json!({"new_terminal": {"strip": strip_id, "designation": "10", "seq": 9}}),
    ] {
        let params = json!({"page": page, "symbol": symbol_terminal, "pos": gp(12, 20), "target": target});
        assert!(matches!(
            run(&mut s, "device.place", params),
            Err(CmdError::Rejected { .. })
        ));
    }
    assert_eq!(lines(&s), unchanged);
}

// ---------------------------------------------------------------------------------------------------------
// d. from_part with a two-function part

#[test]
fn from_part_gives_a_device_with_a_black_box_and_a_contact_that_is_not_drawn() {
    let work = Tmp::new("ew-testkit-place-arrange");
    let mut s = open_copy(&work, 5);
    // The copy of the sample's PLC output card with a second template entry: the black box, then a normally open
    // contact (`ew.fn.contact-no`) on the pattern CPs "5" and "6" (4 and 5). The sample holds the FnDef (its
    // contactor's and push button's contacts).
    let contact_no: Value = embeddable(
        serde_json::from_str(
            &std::fs::read_to_string(Path::new(CONTENT).join("fndefs/ew.fn.contact-no.json"))
                .expect("the starter FnDef"),
        )
        .expect("JSON"),
    );
    let contact_def = LibDocument::from_value(contact_no).expect("a FnDef");
    let LibDocument::Fndef(contact_copy) = &contact_def else {
        panic!("a FnDef");
    };
    assert!(
        s.query().fndefs().contains(contact_copy.id),
        "the sample holds ew.fn.contact-no"
    );

    let card = part_copy(&s, "EW-GEN-DO-1");
    let mut part = (*card).clone();
    let blackbox = part.item.function_template[0].clone();
    assert_eq!(blackbox.cps, (0..10).collect::<Vec<u16>>());
    part.library_id = Id::from_uuid(LIB_TEST);
    part.item_id = Id::from_uuid(ITEM_PART_TWO);
    part.id = copy_id(ITEM_PART_TWO);
    part.item.function_template = vec![
        TemplateFn {
            // The black box keeps the pattern CPs that are not the contact's.
            cps: vec![0, 1, 2, 3, 6, 7, 8, 9],
            ..blackbox
        },
        TemplateFn {
            fndef: ew_schema::LibRef {
                library: contact_copy.library_id,
                item: contact_copy.item_id,
                version_req: "^0.1".into(),
                extra: ew_schema::Extra::new(),
            },
            main: false,
            cps: vec![4, 5],
            extra: ew_schema::Extra::new(),
        },
    ];
    let part_doc = LibDocument::Part(part.clone());
    part.sha256 = Sha256(library_copy_hash(&part_doc).expect("hashable"));
    let part_json = serde_json::to_value(LibDocument::Part(part.clone())).expect("JSON");
    must(&mut s, "library.embed", json!({"copies": [part_json]}));
    // The black-box symbol the sample's cards are drawn with.
    let symbol_blackbox = {
        let q = s.query();
        let drawn = reps_of_part(&s, page_of(&s, 104), "EW-GEN-DO-1");
        q.reps().get(drawn[0]).expect("the card's representation").symbol
    };

    let page = page_of(&s, 102);
    let r = must(
        &mut s,
        "box.black",
        json!({"page": page, "symbol": symbol_blackbox, "pos": gp(30, 30),
            "target": {"from_part": {"part": part.id, "template": 0, "letter": "A"}}}),
    );
    let devices: Vec<Id<Device>> = created(&r, Kind::Device);
    let functions: Vec<Id<Function>> = created(&r, Kind::Function);
    assert_eq!(
        (devices.len(), functions.len()),
        (1, 2),
        "one device with two functions (FR-UI-017)"
    );
    let d = device(&s, devices[0]);
    assert_eq!(
        d.parts,
        vec![PartUse {
            part: ew_schema::PartRef {
                copy: part.id,
                variant: None,
                extra: ew_schema::Extra::new()
            },
            qty: "1".parse().expect("a decimal"),
            extra: ew_schema::Extra::new()
        }]
    );
    let (bb, contact) = (function(&s, functions[0]), function(&s, functions[1]));
    assert_eq!(
        (bb.role.clone(), bb.kind.clone()),
        (FnRole::Main, FunctionKind::BlackBox)
    );
    assert_eq!(
        (contact.role.clone(), contact.kind.clone()),
        (FnRole::Aux, FunctionKind::Normal)
    );
    assert_eq!(contact.def, contact_copy.id);
    // The black box is drawn; the contact has no representation.
    let placed = rep(&s, r.outcome.primary.expect("the representation").1.cast());
    assert_eq!(placed.functions, vec![functions[0]]);
    let q = s.query();
    assert_eq!(q.idx().reps_of_function(functions[0]).count(), 1);
    assert_eq!(
        q.idx().reps_of_function(functions[1]).count(),
        0,
        "the contact is not drawn"
    );
    // The CPs follow from the part's pattern (doc 03 §7). The contact's entry lists one pattern CP per FnDef CP, so
    // they pair by position and keep the FnDef's indices 0 and 1, which its symbol draws.
    assert_eq!(contact.template_entry, Some(1));
    let cps = |f: Id<Function>| -> Vec<(u16, String)> {
        q.get::<FunctionCps>(f)
            .iter()
            .map(|c| (c.idx, c.designation.clone()))
            .collect()
    };
    assert_eq!(
        cps(functions[1]),
        vec![(0, "5".to_owned()), (1, "6".to_owned())],
        "the contact's CPs are the pattern's 5 and 6"
    );
    assert_eq!(cps(functions[0]).len(), 8);
}

// ---------------------------------------------------------------------------------------------------------
// The two-function part fixtures (`crates/ew-api/tests/expected/`, read by the ew-api and ew-viewmodel tests)

/// Whether `EW_BLESS=1` asks the fixture test to rewrite its expected files in this build's form.
// Allowed: the program boundary of a test, read once; the value never reaches a result.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The fixture `name` under `crates/ew-api/tests/expected/` as a part copy of the test library holding `item`, after
/// checking that it is in this build's form byte for byte (`EW_BLESS=1` rewrites it so) and that its hash is its
/// item's.
#[track_caller]
fn fixture_part(name: &str, item: Uuid) -> ew_schema::LibCopy<Part> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ew-api/tests/expected")
        .join(name);
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let doc = LibDocument::from_json(&committed).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&doc).expect("a serialisable library document")
    );
    if bless() {
        std::fs::write(&path, &text).expect("the fixture");
    }
    assert_eq!(
        committed,
        text,
        "{} is not in this build's form byte for byte; EW_BLESS=1 rewrites it",
        path.display()
    );
    let LibDocument::Part(part) = doc else {
        panic!("{} holds a part", path.display());
    };
    assert_eq!(
        (part.library_id, part.item_id, part.id),
        (Id::from_uuid(LIB_TEST), Id::from_uuid(item), copy_id(item)),
        "{name}: a copy of the test library"
    );
    assert_eq!(
        Some(part.sha256.0),
        library_copy_hash(&LibDocument::Part(part.clone())),
        "{name}: the hash of its item"
    );
    part
}

/// The fixtures are test inputs of their own: a ten-CP black-box part (MICO 4.10, `24VDC` ×2, `GND` ×2, `13`, `14`,
/// `Out1` to `Out4`) with the template entries their users rely on. Each is in this build's form, with the hash of its
/// item ([`fixture_part`]).
#[test]
fn the_two_fndef_part_fixtures_are_current() {
    // The starter FnDefs the fixtures' template entries name, as `library.embed` takes one.
    let starter_ref = |key: &str| -> ew_schema::LibRef {
        let json = embeddable(
            serde_json::from_str(
                &std::fs::read_to_string(Path::new(CONTENT).join("fndefs").join(format!("{key}.json")))
                    .expect("the starter FnDef"),
            )
            .expect("JSON"),
        );
        let doc = LibDocument::from_value(json).expect("a FnDef");
        let LibDocument::Fndef(copy) = &doc else {
            panic!("a FnDef");
        };
        ew_schema::LibRef {
            library: copy.library_id,
            item: copy.item_id,
            version_req: "^0.1".into(),
            extra: Extra::from([("key".to_owned(), Value::from(key))]),
        }
    };
    let entries = |part: &ew_schema::LibCopy<Part>| -> Vec<(Option<String>, bool, Vec<u16>)> {
        part.item
            .function_template
            .iter()
            .map(|t| {
                let key = t
                    .fndef
                    .extra
                    .get("key")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                (key, t.main, t.cps.clone())
            })
            .collect()
    };
    let entry = |key: &str, main: bool, cps: &[u16]| (Some(key.to_owned()), main, cps.to_vec());
    let black_box = [0, 1, 2, 3, 6, 7, 8, 9];

    let two = fixture_part("two_fndef_part.json", ITEM_PART_TWO_FX);
    assert_eq!(
        entries(&two),
        [
            entry("ew.fn.blackbox", true, &black_box),
            entry("ew.fn.contact-no", false, &[4, 5])
        ]
    );
    assert_eq!(
        two.item.function_template[1].fndef,
        starter_ref("ew.fn.contact-no")
    );

    // A contact whose FnDef no library holds.
    let missing = fixture_part("missing_fndef_part.json", ITEM_PART_MISSING_FX);
    assert_eq!(
        entries(&missing),
        [
            entry("ew.fn.blackbox", true, &black_box),
            entry("test.fn.unknown", false, &[4, 5])
        ]
    );
    let unknown = &missing.item.function_template[1].fndef;
    assert_eq!(
        (unknown.library, unknown.item),
        (Id::from_uuid(LIB_TEST), Id::from_uuid(ITEM_FN_UNKNOWN))
    );

    // Two starter FnDefs and no black box: the part-first arm reaches both.
    let starter = fixture_part("two_starter_fndef_part.json", ITEM_PART_STARTER_FX);
    assert_eq!(
        entries(&starter),
        [
            entry("ew.fn.contact-no", true, &[4, 5]),
            entry("ew.fn.contact-nc", false, &[6, 7])
        ]
    );
    assert_eq!(
        [
            &starter.item.function_template[0].fndef,
            &starter.item.function_template[1].fndef
        ],
        [&starter_ref("ew.fn.contact-no"), &starter_ref("ew.fn.contact-nc")]
    );
    let designations: Vec<&str> = two
        .item
        .cp_pattern
        .iter()
        .map(|c| c.designation.as_str())
        .collect();
    assert_eq!(designations[4..6], ["13", "14"], "the contact's pattern CPs");
}

// ---------------------------------------------------------------------------------------------------------
// The DSL verbs

#[test]
fn the_new_dsl_verbs_send_the_parameters_of_their_commands() {
    let (page, symbol, rep_id) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
    let (part, strip, pin_rep) = (Uuid::from_u128(4), Uuid::from_u128(5), Uuid::from_u128(6));
    let (con_id, ip_function) = (Uuid::from_u128(7), Uuid::from_u128(8));
    let mut sink = |name: &str, _params: Value| -> Result<Done, String> {
        if name == "connection.draw_to_ip" {
            return Ok(Done {
                primary: Some((Kind::Connection, Id::<Connection>::from_uuid(con_id).cast())),
                created: vec![(Kind::Function, Id::<Function>::from_uuid(ip_function).cast())],
            });
        }
        Ok(Done {
            primary: (name == "device.place" || name == "box.black").then_some((
                Kind::Representation,
                Id::<Representation>::from_uuid(rep_id).cast(),
            )),
            ..Done::default()
        })
    };
    let mut b = Builder::new(&mut sink);
    let (page, symbol): (Id<Page>, Id<ew_schema::SymbolDef>) = (Id::from_uuid(page), Id::from_uuid(symbol));
    let part: Id<Part> = Id::from_uuid(part);
    let strip: Id<Device> = Id::from_uuid(strip);
    let pos = gp(3, 6);
    b.place_from_part(page, symbol, pos, part, 1, "K", false).unwrap();
    b.place_from_part(page, symbol, pos, part, 0, "A", true).unwrap();
    b.place_new_terminal(page, symbol, pos, strip, "9", None).unwrap();
    b.place_new_terminal(page, symbol, pos, strip, "10", Some(30))
        .unwrap();
    b.move_steps(&[Id::from_uuid(pin_rep)], [1, -2]).unwrap();
    b.exchange_symbol(Id::from_uuid(pin_rep), symbol, None).unwrap();
    b.exchange_symbol(Id::from_uuid(pin_rep), symbol, Some(&[(0, 1), (1, 0)]))
        .unwrap();
    b.drawing(Some(Some("hops"))).unwrap();
    b.drawing(Some(None)).unwrap();
    let ip = IpEnd {
        symbol,
        def: None,
        pos: gp(6, 6),
        spec: IpSpec {
            name: "24VDC-2".parse::<Ident>().expect("an identifier"),
            direction: IpDirection::Source,
            star_source: true,
            sort: Some(2),
            external: None,
            group: None,
            extra: Extra::new(),
        },
    };
    let from = End::Cp(Id::from_uuid(ip_function), 1);
    let (drawn, f) = b
        .draw_to_ip(&from, page, &[gp(3, 6), gp(6, 6)], None, &ip)
        .unwrap();
    assert_eq!((drawn, f), (Id::from_uuid(con_id), Id::from_uuid(ip_function)));
    b.draw_to_ip(
        &from,
        page,
        &[],
        Some(&ConnectionKind::Wire),
        &IpEnd {
            def: Some(Id::from_uuid(pin_rep)),
            ..ip.clone()
        },
    )
    .unwrap();
    b.arrange_ips(&[Id::from_uuid(ip_function), Id::from_uuid(pin_rep)], false)
        .unwrap();
    b.arrange_ips(&[Id::from_uuid(pin_rep), Id::from_uuid(ip_function)], true)
        .unwrap();
    let outline = [gp(0, 0), gp(6, 0), gp(6, 4), gp(0, 4)];
    b.reshape_box(Id::from_uuid(pin_rep), &outline, None).unwrap();
    b.reshape_box(Id::from_uuid(pin_rep), &outline, Some(&[(2, gp(3, 0), "up")]))
        .unwrap();
    let script = b.script();
    let at = |i: usize| (script[i].0.as_str(), &script[i].1);
    let target = |i: usize| at(i).1["target"].clone();
    assert_eq!(at(0).0, "device.place");
    assert_eq!(
        target(0),
        json!({"from_part": {"part": part, "template": 1, "letter": "K"}})
    );
    assert_eq!(at(1).0, "box.black");
    assert_eq!(
        target(1),
        json!({"from_part": {"part": part, "template": 0, "letter": "A"}})
    );
    assert_eq!(at(0).1["page"], json!(page));
    assert_eq!(at(0).1["pos"], json!(pos));
    assert_eq!(at(2).0, "device.place");
    assert_eq!(
        target(2),
        json!({"new_terminal": {"strip": strip, "designation": "9"}})
    );
    assert_eq!(
        target(3),
        json!({"new_terminal": {"strip": strip, "designation": "10", "seq": 30}})
    );
    assert_eq!(at(4).0, "rep.move");
    assert_eq!(at(4).1, &json!({"reps": [pin_rep.to_string()], "steps": [1, -2]}));
    assert_eq!(at(5).0, "rep.exchange_symbol");
    assert_eq!(at(5).1, &json!({"rep": pin_rep.to_string(), "symbol": symbol}));
    assert_eq!(
        at(6).1,
        &json!({"rep": pin_rep.to_string(), "symbol": symbol,
            "pin_map": [{"symbol_cp": 0, "function_cp": 1}, {"symbol_cp": 1, "function_cp": 0}]})
    );
    assert_eq!(
        (at(7).0, at(7).1),
        ("project.drawing.set", &json!({"crossings": "hops"}))
    );
    assert_eq!(
        (at(8).0, at(8).1),
        ("project.drawing.set", &json!({"crossings": null}))
    );
    assert_eq!(at(9).0, "connection.draw_to_ip");
    assert_eq!(
        at(9).1,
        &json!({"a": {"cp": {"function": ip_function.to_string(), "idx": 1}}, "page": page,
            "route": [gp(3, 6), gp(6, 6)],
            "ip": {"symbol": symbol, "pos": gp(6, 6), "name": "24VDC-2", "direction": "source",
                "star_source": true, "sort": 2}})
    );
    assert_eq!(at(10).0, "connection.draw_to_ip");
    assert_eq!(at(10).1["kind"], json!("wire"));
    assert_eq!(at(10).1["ip"]["fndef"], json!(pin_rep.to_string()));
    assert!(at(10).1.get("route").is_none(), "an empty route is not sent");
    assert_eq!(at(11).0, "ip.arrange");
    assert_eq!(
        at(11).1,
        &json!({"ips": [ip_function.to_string(), pin_rep.to_string()]})
    );
    assert_eq!(
        at(12).1,
        &json!({"ips": [pin_rep.to_string(), ip_function.to_string()], "set_sort": true})
    );
    assert_eq!(at(13).0, "box.reshape");
    assert_eq!(
        at(13).1,
        &json!({"rep": pin_rep.to_string(), "outline": [gp(0, 0), gp(6, 0), gp(6, 4), gp(0, 4)]})
    );
    assert_eq!(at(14).1["cps"], json!([{"idx": 2, "pos": gp(3, 0), "dir": "up"}]));
}
