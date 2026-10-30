// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The acceptance tests of the review cycle 5 findings F1-F5 and the decisions D2-D4: checked coordinates,
//! one-pass junction clean-up, collision-only renumbering, preview identity, transient delete planning,
//! `fields.set` collisions, the connection grid and the cost of `edit.delete`.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    BULK_ABOVE, Connect, CreatePage, DeleteConnections, DeleteSelection, EditJunction, EndpointParam,
    KindName, MoveReps, PlaceIp, PlaceSymbol, PlaceTarget, RouteConnection, SetFields, TargetRef,
};
use ew_commands::geometry::MAX_COORD;
use ew_commands::rows::{row_json, row_text};
use ew_commands::{CmdError, Command, Cost, DynCommand, Engine};
use ew_ids::Id;
use ew_model::{AnyRow, Entity, Model};
use ew_schema::{
    Connection, ConnectionKind, Device, Endpoint, FnRole, Function, IpDirection, Junction, JunctionStyle,
    Kind, Page, Record, Representation,
};
use ew_units::{Grid, Nm, Orient2, Pt2};
use serde_json::json;

// ---------------------------------------------------------------------------------------------------------
// Helpers

fn row<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> T {
    (**e.model().table::<T>().get(&id).expect("record exists")).clone()
}

fn has<T: Entity<Marker = T>>(e: &Engine, id: Id<T>) -> bool {
    e.model().table::<T>().get(&id).is_some()
}

fn golden_with(f: impl FnOnce(&mut Vec<Record>)) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    f(&mut records);
    Model::from_records(p, records, library()).expect("valid model")
}

/// The reason of a rejection, or a panic naming what came back instead.
fn rejection<T: std::fmt::Debug>(r: Result<T, CmdError>) -> String {
    match r {
        Err(CmdError::Rejected { reason, .. }) => reason,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

fn rejected_naming<T: std::fmt::Debug>(r: Result<T, CmdError>, param: &str) {
    let reason = rejection(r);
    assert!(
        reason.contains(param),
        "the reason should name `{param}`, got: {reason}"
    );
}

fn cp_param(f: Id<Function>, idx: u16) -> EndpointParam {
    EndpointParam::Cp { function: f, idx }
}

fn target(kind: KindName, id: ew_schema::AnyId) -> TargetRef {
    TargetRef { kind, id }
}

/// Adds a placed device (letter, counter) with a main function and one representation on `page` at the
/// origin; `n` numbers its IDs (device `0x1000 + n`, function `0x1100 + n`, representation `0x1200 + n`).
fn add_placed(records: &mut Vec<Record>, n: u128, letter: &str, counter: u32, page: Id<Page>) {
    let (dev, f, r): (Id<Device>, Id<Function>, Id<Representation>) =
        (id(0x1000 + n), id(0x1100 + n), id(0x1200 + n));
    records.push(Record::Device(device(dev, letter, counter)));
    records.push(Record::Function(function(f, dev, FNDEF_CONTACT, FnRole::Main)));
    records.push(Record::Representation(rep(r, page, vec![f])));
}

fn place_k(letter: &str, page: Id<Page>, pos: Pt2) -> PlaceSymbol {
    PlaceSymbol {
        copies: Vec::new(),
        page,
        symbol: SYM_TWO,
        pos,
        orient: Orient2::R0,
        rep_type: None,
        variant: 0,
        target: PlaceTarget::NewDevice {
            letter: ident(letter),
            fndef: None,
            kind: None,
        },
        connect: vec![],
    }
}

/// Places a new device and returns its representation, function and device.
fn place_device(
    e: &mut Engine,
    letter: &str,
    page: Id<Page>,
    pos: Pt2,
) -> (Id<Representation>, Id<Function>, Id<Device>) {
    let r = e.execute(&place_k(letter, page, pos)).expect("device.place");
    let rep_id: Id<Representation> = typed(r.outcome.primary.expect("primary"));
    let rep = row(e, rep_id);
    let f = rep.functions[0];
    let d = row(e, f).device.expect("device");
    (rep_id, f, d)
}

fn move_reps(reps: Vec<Id<Representation>>, delta: Pt2) -> MoveReps {
    MoveReps {
        items: Vec::new(),
        steps: None,
        reps,
        delta,
        page: None,
    }
}

fn set_device(device: Id<Device>, path: &str, value: serde_json::Value) -> SetFields {
    SetFields {
        kind: KindName::Device,
        targets: vec![device.cast()],
        path: path.into(),
        value,
        copies: Vec::new(),
    }
}

// ---------------------------------------------------------------------------------------------------------
// F1: checked coordinates

/// The largest coordinate on `grid` that is at most `MAX_COORD`.
fn largest_on_grid(grid: Grid) -> i64 {
    let mut k = grid.snap(Nm(MAX_COORD));
    loop {
        let x = grid.at(k).expect("on the grid").0;
        if x <= MAX_COORD {
            return x;
        }
        k -= 1;
    }
}

#[test]
fn f1_two_moves_by_max_safe_integer_reject_the_second() {
    let d = largest_on_grid(Grid::QET_10PX);
    let mut e = engine(golden_model());
    // The aux representation sits at the origin and has no connections.
    e.execute(&move_reps(vec![REP_102F2_AUX], pt(d, 0)))
        .expect("the first move stays in range");
    assert_eq!(row(&e, REP_102F2_AUX).pos, pt(d, 0));
    let before = lines(e.model());
    let reason = rejection(e.execute(&move_reps(vec![REP_102F2_AUX], pt(d, 0))));
    assert!(reason.contains("/delta"), "got: {reason}");
    assert_eq!(lines(e.model()), before, "a refused move changes nothing");
}

#[test]
fn f1_a_single_move_by_max_coord_from_a_positive_position_is_rejected() {
    let mut e = engine(golden_model());
    e.execute(&move_reps(vec![REP_102F2_AUX], pt(G, 0)))
        .expect("move to G");
    let before = lines(e.model());
    let reason = rejection(e.execute(&move_reps(vec![REP_102F2_AUX], pt(MAX_COORD, 0))));
    assert!(reason.contains("/delta"), "got: {reason}");
    assert_eq!(lines(e.model()), before);
}

#[test]
fn f1_a_record_without_json_form_is_an_error_not_null() {
    let mut r = rep(REP_102F2_AUX, PAGE_102, vec![FN_102F2_AUX]);
    r.pos = pt(MAX_COORD + 1, 0);
    let any = AnyRow::of(r);
    assert!(matches!(row_json(&any), Err(CmdError::Internal { .. })));
    assert!(matches!(row_text(&any), Err(CmdError::Internal { .. })));

    let cmd = move_reps(vec![REP_102F2_AUX], pt(MAX_COORD + 1, 0));
    assert!(matches!(
        DynCommand::to_json(&cmd),
        Err(CmdError::Internal { .. })
    ));
}

// ---------------------------------------------------------------------------------------------------------
// F2: junction clean-up converges in one pass

fn junction_on_102(j: Id<Junction>, sequence: Vec<Id<Connection>>) -> Record {
    Record::Junction(junction(j, PAGE_102, sequence))
}

fn wire(c: Id<Connection>, a: Endpoint, b: Endpoint) -> Record {
    Record::Connection(connection(c, Some(PAGE_102), a, b))
}

#[test]
fn f2_cascade_j1_j3_j4_cleans_up_in_one_pass() {
    let (j1, j3, j4): (Id<Junction>, Id<Junction>, Id<Junction>) = (id(0x4001), id(0x4003), id(0x4004));
    let [a, b, c, d, ee, f]: [Id<Connection>; 6] = std::array::from_fn(|i| id(0x5001 + i as u128));
    let model = golden_with(|r| {
        r.push(wire(a, at_junction(j1), at_junction(j3)));
        r.push(wire(b, at_junction(j1), at_junction(j3)));
        r.push(wire(c, at_junction(j1), cp(FN_102F2, 8)));
        r.push(wire(d, at_junction(j3), at_junction(j4)));
        r.push(wire(ee, at_junction(j4), cp(FN_T3, 2)));
        r.push(wire(f, at_junction(j4), cp(FN_T1, 2)));
        r.push(junction_on_102(j1, vec![a, b, c]));
        r.push(junction_on_102(j3, vec![a, b, d]));
        r.push(junction_on_102(j4, vec![d, ee, f]));
    });
    let mut e = engine(model);
    let receipt = e
        .execute(&DeleteConnections { connections: vec![c] })
        .expect("the cascade converges (no `did not converge` internal error)");
    assert_eq!(receipt.passes, 2, "one productive pass plus the confirming one");
    for j in [j1, j3, j4] {
        assert!(!has(&e, j));
    }
    for gone in [a, b, c, d] {
        assert!(!has(&e, gone));
    }
    assert!(has(&e, ee) ^ has(&e, f), "one merged connection is left");
    let merged = row(&e, if has(&e, ee) { ee } else { f });
    let ends = [merged.a.clone(), merged.b.clone()];
    assert!(ends.contains(&cp(FN_T3, 2)) && ends.contains(&cp(FN_T1, 2)));
}

#[test]
fn f2_a_50_long_chain_cleans_up_in_one_pass() {
    // 25 pairs (A_i, B_i): A_i has [feed_i, p_i, q_i], p_i and q_i both go to B_i, B_i has [p_i, q_i, d_i]
    // and d_i is the feed of A_(i+1). Removing the feed of A_0 leaves A_0 with the loop p_0/q_0, B_0 with
    // d_0 alone, A_1 with p_1/q_1, ... down the whole chain. The IDs run against the chain order.
    const PAIRS: u128 = 25;
    let aj = |i: u128| -> Id<Junction> { id(0x4800 - 2 * i) };
    let bj = |i: u128| -> Id<Junction> { id(0x4800 - 2 * i - 1) };
    let conn = |i: u128, k: u128| -> Id<Connection> { id(0x5000 + 4 * i + k) };
    let feed: Id<Connection> = id(0x5fff);
    let model = golden_with(|r| {
        r.push(wire(feed, at_junction(aj(0)), cp(FN_102F2, 8)));
        for i in 0..PAIRS {
            let (p, q, d) = (conn(i, 0), conn(i, 1), conn(i, 2));
            r.push(wire(p, at_junction(aj(i)), at_junction(bj(i))));
            r.push(wire(q, at_junction(aj(i)), at_junction(bj(i))));
            let next = if i + 1 < PAIRS {
                at_junction(aj(i + 1))
            } else {
                cp(FN_T3, 2)
            };
            r.push(wire(d, at_junction(bj(i)), next));
            let feed_in = if i == 0 { feed } else { conn(i - 1, 2) };
            r.push(junction_on_102(aj(i), vec![feed_in, p, q]));
            r.push(junction_on_102(bj(i), vec![p, q, d]));
        }
    });
    let mut e = engine(model);
    let receipt = e
        .execute(&DeleteConnections {
            connections: vec![feed],
        })
        .expect("connection.delete cascades through 50 junctions in one pass");
    assert_eq!(receipt.passes, 2);
    assert_eq!(e.model().table::<Junction>().iter().count(), 1, "only J5 is left");
    assert!(has(&e, J5));
}

// ---------------------------------------------------------------------------------------------------------
// F3: collision-only renumbering

#[test]
fn f3_editing_an_unrelated_field_of_a_colliding_device_changes_only_that_field() {
    let (first, second): (Id<Device>, Id<Device>) = (id(0x1001), id(0x1002));
    let model = golden_with(|r| {
        add_placed(r, 1, "K", 1, PAGE_102);
        add_placed(r, 2, "K", 1, PAGE_102);
    });
    let mut e = engine(model);
    let before = lines(e.model());
    let before_second = row(&e, second);
    e.execute(&set_device(second, "function_text", json!("x")))
        .expect("an unrelated edit of a legacy collision executes");
    let after_second = row(&e, second);
    assert_eq!(after_second.tag.counter, Some(1));
    let mut expected = before_second;
    expected.function_text = ew_mltext::MlText::neutral("x");
    assert_eq!(after_second, expected);
    assert_eq!(row(&e, first).tag.counter, Some(1));
    let after = lines(e.model());
    assert_eq!(before.len(), after.len());
    let changed: Vec<_> = before
        .iter()
        .zip(&after)
        .filter(|(b, a)| b != a)
        .map(|(b, _)| b.0)
        .collect();
    assert_eq!(changed, vec![(Kind::Device, second.cast())]);
}

#[test]
fn f3_moving_onto_a_colliding_page_renumbers_the_moved_device() {
    let (moved, stayed): (Id<Device>, Id<Device>) = (id(0x1001), id(0x1002));
    let model = golden_with(|r| {
        add_placed(r, 1, "K", 1, PAGE_101); // the mover: K1 on page 101
        add_placed(r, 2, "K", 1, PAGE_102); // K1 on page 102
    });
    let mut e = engine(model);
    let stayed_before = row(&e, stayed);
    e.execute(&MoveReps {
        items: Vec::new(),
        steps: None,
        reps: vec![id(0x1201)],
        delta: pt(0, 0),
        page: Some(PAGE_102),
    })
    .expect("cross-page move of an unconnected representation");
    assert_eq!(row(&e, moved).tag.counter, Some(2), "the next free counter");
    assert_eq!(
        row(&e, stayed),
        stayed_before,
        "the device that stayed is untouched"
    );
}

// ---------------------------------------------------------------------------------------------------------
// F4: preview identity

#[test]
fn f4_a_preview_from_another_engine_is_re_run() {
    let cmd = CreatePage::at(Some(SEC1), 103);
    let start = lines(&golden_model());

    let mut a = engine_seeded(golden_model(), 1);
    let mut b = engine_seeded(golden_model(), 2);
    let mut c = engine_seeded(golden_model(), 1);
    let mut d = engine_seeded(golden_model(), 2);
    let c_receipt = c.execute(&cmd).expect("reference run with A's seed");
    d.execute(&cmd).expect("reference run with B's seed");
    assert_ne!(
        lines(c.model()),
        lines(d.model()),
        "the seeds mint different IDs, so a swapped-in candidate would be visible"
    );

    let p = b.preview(&cmd).expect("preview on B");
    let receipt = a.commit(p).expect("commit on A re-runs the command");
    assert_eq!(lines(a.model()), lines(c.model()), "A's own seed minted the IDs");
    assert_eq!(receipt.created, c_receipt.created);
    assert_eq!(lines(b.model()), start, "B is untouched by the commit on A");

    a.undo(&ew_commands::Origin::Gui)
        .expect("undo on A still works after the commit");
    assert_eq!(lines(a.model()), start);
}

#[test]
fn f4_a_preview_from_another_engine_at_a_different_revision_is_re_run() {
    let unrelated = CreatePage::at(Some(SEC1), 104);
    let cmd = CreatePage::at(Some(SEC1), 103);

    let mut a = engine_seeded(golden_model(), 1);
    a.execute(&unrelated).unwrap();
    let mut b = engine_seeded(golden_model(), 2);
    let mut c = engine_seeded(golden_model(), 1);
    c.execute(&unrelated).unwrap();
    c.execute(&cmd).unwrap();

    let b_start = lines(b.model());
    let p = b.preview(&cmd).expect("preview on B");
    a.commit(p).expect("commit on A re-runs the command");
    assert_eq!(lines(a.model()), lines(c.model()));
    assert_eq!(lines(b.model()), b_start);
}

// ---------------------------------------------------------------------------------------------------------
// F5: transient delete planning

fn delete(targets: Vec<TargetRef>) -> DeleteSelection {
    DeleteSelection { targets }
}

#[test]
fn f5_delete_main_function_and_device_in_either_order() {
    let main_fn = target(KindName::Function, FN_102F2.cast());
    let dev = target(KindName::Device, DEV_102F2.cast());
    let rep_t = target(KindName::Representation, REP_102F2.cast());

    let mut e1 = engine(golden_model());
    let mut e2 = engine(golden_model());
    let mut e3 = engine(golden_model());
    e1.execute(&delete(vec![main_fn, dev]))
        .expect("[function, device]");
    e2.execute(&delete(vec![dev, main_fn]))
        .expect("[device, function]");
    e3.execute(&delete(vec![rep_t, dev]))
        .expect("[representation, device]");
    assert_eq!(lines(e1.model()), lines(e2.model()));
    for e in [&e1, &e2, &e3] {
        assert!(!has(e, DEV_102F2));
        assert!(!has(e, FN_102F2));
        assert!(!has(e, REP_102F2));
    }
}

// ---------------------------------------------------------------------------------------------------------
// D2: fields.set collisions

#[test]
fn d2_setting_a_colliding_counter_is_rejected_with_the_next_free_one() {
    let mut e = engine(golden_model());
    let (_, _, k1) = place_device(&mut e, "K", PAGE_102, pt(G, G));
    let (_, _, k2) = place_device(&mut e, "K", PAGE_102, pt(2 * G, G));
    assert_eq!(row(&e, k1).tag.counter, Some(1));
    assert_eq!(row(&e, k2).tag.counter, Some(2));

    let before = lines(e.model());
    let reason = rejection(e.execute(&set_device(k2, "tag.counter", json!(1))));
    // The edited device leaves its own counter 2: besides the other device's 1, the lowest free counter
    // is 2 (the library computes the remedy with the edited device excluded).
    assert!(
        reason.contains("use 2"),
        "the remedy is the next free counter: {reason}"
    );
    assert_eq!(lines(e.model()), before);

    e.execute(&set_device(k2, "tag.counter", json!(5)))
        .expect("a free counter");
    assert_eq!(row(&e, k2).tag.counter, Some(5), "no renumbering");

    // A letter change that lands on a taken counter is refused too.
    let (_, _, l1) = place_device(&mut e, "L", PAGE_102, pt(3 * G, G));
    assert_eq!(row(&e, l1).tag.counter, Some(1));
    let before = lines(e.model());
    rejection(e.execute(&set_device(l1, "tag.letter", json!("K"))));
    assert_eq!(lines(e.model()), before);
}

// ---------------------------------------------------------------------------------------------------------
// D3: the connection grid

fn place_ip(pos: Pt2) -> PlaceIp {
    PlaceIp {
        page: PAGE_102,
        symbol: SYM,
        fndef: Some(FNDEF_IP),
        pos,
        orient: Orient2::default(),
        name: ident("L-2"),
        direction: IpDirection::Source,
        star_source: false,
        sort: None,
        external: None,
        copies: Vec::new(),
    }
}

#[test]
fn d3_off_grid_positions_are_rejected() {
    // device.place
    let mut e = engine(golden_model());
    rejected_naming(e.execute(&place_k("K", PAGE_102, pt(1, 0))), "/pos");
    rejected_naming(e.execute(&place_k("K", PAGE_102, pt(0, MM10))), "/pos");
    e.execute(&place_k("K", PAGE_102, pt(G, G)))
        .expect("on-grid placement");

    // ip.place
    rejected_naming(e.execute(&place_ip(pt(0, 1))), "/pos");
    e.execute(&place_ip(pt(2 * G, 2 * G))).expect("on-grid IP");

    // rep.move
    rejected_naming(e.execute(&move_reps(vec![REP_102F2_AUX], pt(MM10, 0))), "/delta");
    e.execute(&move_reps(vec![REP_102F2_AUX], pt(G, 0)))
        .expect("on-grid move");

    // connection.draw and connection.route between the CPs of two placed devices
    let mut e = engine(golden_model());
    let (_, fa, _) = place_device(&mut e, "K", PAGE_102, pt(G, G));
    let (_, fb, _) = place_device(&mut e, "K", PAGE_102, pt(4 * G, G));
    let draw = |route: Vec<Pt2>| Connect {
        a: cp_param(fa, 0),
        b: cp_param(fb, 0),
        page: Some(PAGE_102),
        route,
        kind: ConnectionKind::Wire,
    };
    rejected_naming(
        e.execute(&draw(vec![pt(G, G), pt(2 * G + 1, G), pt(4 * G, G)])),
        "/route/1",
    );
    let r = e
        .execute(&draw(vec![pt(G, G), pt(2 * G, G), pt(4 * G, G)]))
        .expect("on-grid route");
    let w: Id<Connection> = typed(r.outcome.primary.expect("primary"));

    // a split point that is on the route but off the grid
    let split = |at: Pt2| Connect {
        a: EndpointParam::OnConnection { connection: w, at },
        b: cp_param(fa, 1),
        page: None,
        route: vec![],
        kind: ConnectionKind::Wire,
    };
    rejected_naming(e.execute(&split(pt(3 * G + 1, G))), "/a/on_connection/at");
    e.execute(&split(pt(3 * G, G))).expect("on-grid split point");

    // connection.route on the first part (from the CP of A to the junction at 3G)
    let c = row(&e, w);
    assert_eq!(c.a, cp(fa, 0));
    let route = |vs: Vec<Pt2>| RouteConnection {
        connection: w,
        route: vs,
    };
    let last = *c.route.last().expect("a route");
    rejected_naming(
        e.execute(&route(vec![pt(G, G), pt(2 * G, G + 1), last])),
        "/route/1",
    );
    rejected_naming(e.execute(&route(vec![pt(2 * G, G), last])), "/route/0");
    e.execute(&route(vec![pt(G, G), pt(G, 2 * G), pt(last.x.0, 2 * G), last]))
        .expect("on-grid route that starts at its CP");
}

#[test]
fn d3_junction_edit_takes_no_coordinates_and_works_off_the_grid() {
    let model = golden_with(|r| {
        for rec in r.iter_mut() {
            if let Record::Junction(j) = rec {
                if j.id == J5 {
                    j.at = pt(1, 1);
                }
            }
        }
    });
    let mut e = engine(model);
    let bevel = JunctionStyle::Bevel(Default::default());
    e.execute(&EditJunction {
        junction: J5,
        style: Some(bevel.clone()),
        wiring: None,
        sequence: None,
    })
    .expect("junction.edit has no coordinates to check");
    assert_eq!(row(&e, J5).style, bevel);
    assert_eq!(row(&e, J5).at, pt(1, 1));
}

// ---------------------------------------------------------------------------------------------------------
// D4: the cost of edit.delete counts the planned records

#[test]
fn d4_edit_delete_cost_counts_planned_records() {
    let (dev, f): (Id<Device>, Id<Function>) = (id(0x1001), id(0x1101));
    let model = golden_with(|r| {
        r.push(Record::Device(device(dev, "K", 1)));
        r.push(Record::Function(function(f, dev, FNDEF_CONTACT, FnRole::Main)));
        for i in 0..=BULK_ABOVE as u128 {
            r.push(Record::Representation(rep(id(0x3000 + i), PAGE_102, vec![f])));
        }
    });
    let cascade = delete(vec![target(KindName::Device, dev.cast())]);
    assert_eq!(Command::cost(&cascade, &model), Cost::Bulk);

    let golden = golden_model();
    let same = target(KindName::Representation, REP_102G1.cast());
    let many = delete(vec![same; 600]);
    assert_eq!(Command::cost(&many, &golden), Cost::Interactive);
}

// ---------------------------------------------------------------------------------------------------------
// FT-437: route hygiene. `connection.draw` and `connection.route` refuse a route with a repeated vertex or a
// segment that runs over another one (`geometry::require_route`, `geometry::route_defect`).

/// Two devices on page 102 with their first CPs at (G, G) and (4G, G).
fn two_devices() -> (Engine, Id<Function>, Id<Function>) {
    let mut e = engine(golden_model());
    let (_, fa, _) = place_device(&mut e, "K", PAGE_102, pt(G, G));
    let (_, fb, _) = place_device(&mut e, "K", PAGE_102, pt(4 * G, G));
    (e, fa, fb)
}

fn draw_between(fa: Id<Function>, fb: Id<Function>, route: Vec<Pt2>) -> Connect {
    Connect {
        a: cp_param(fa, 0),
        b: cp_param(fb, 0),
        page: Some(PAGE_102),
        route,
        kind: ConnectionKind::Wire,
    }
}

/// The naive oracle: whether a route has a repeated vertex, or two segments sharing a stretch of positive length
/// on one line (every pair compared, in `i128`).
fn naive_defect(route: &[Pt2]) -> bool {
    if route.windows(2).any(|w| w[0] == w[1]) {
        return true;
    }
    let v = |p: Pt2| (i128::from(p.x.0), i128::from(p.y.0));
    let segs: Vec<((i128, i128), (i128, i128))> = route.windows(2).map(|w| (v(w[0]), v(w[1]))).collect();
    for (i, &(a, b)) in segs.iter().enumerate() {
        for &(c, d) in &segs[i + 1..] {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let cross = |p: (i128, i128)| dx * (p.1 - a.1) - dy * (p.0 - a.0);
            if cross(c) != 0 || cross(d) != 0 {
                continue;
            }
            let t = |p: (i128, i128)| dx * (p.0 - a.0) + dy * (p.1 - a.1);
            let (lo, hi) = (t(c).min(t(d)), t(c).max(t(d)));
            if lo.max(0) < hi.min(dx * dx + dy * dy) {
                return true;
            }
        }
    }
    false
}

#[test]
fn ft437_a_repeated_vertex_is_refused_by_draw_and_route() {
    let (mut e, fa, fb) = two_devices();
    let dup = vec![pt(G, G), pt(2 * G, G), pt(2 * G, G), pt(4 * G, G)];
    let expected = format!(
        "`/route/2` [{}, {}] repeats the vertex before it; remove the duplicate vertex",
        2 * G,
        G
    );
    assert_eq!(rejection(e.execute(&draw_between(fa, fb, dup.clone()))), expected);
    let r = e
        .execute(&draw_between(fa, fb, vec![pt(G, G), pt(4 * G, G)]))
        .expect("the straight wire");
    let w: Id<Connection> = typed(r.outcome.primary.expect("primary"));
    let reason = rejection(e.execute(&RouteConnection {
        connection: w,
        route: dup,
    }));
    assert_eq!(reason, expected);
}

#[test]
fn ft437_a_route_that_doubles_back_is_refused() {
    let (mut e, fa, fb) = two_devices();
    // Right to 3G, back left to 2G, down, right, up into the CP: the second segment runs back over the first.
    let back = vec![
        pt(G, G),
        pt(3 * G, G),
        pt(2 * G, G),
        pt(2 * G, 2 * G),
        pt(4 * G, 2 * G),
        pt(4 * G, G),
    ];
    assert_eq!(
        rejection(e.execute(&draw_between(fa, fb, back))),
        format!(
            "`/route/1` the segment from [{x3}, {g}] to [{x2}, {g}] runs over the segment from [{g}, {g}] to \
             [{x3}, {g}] (`/route/0`); a route never overlaps itself, remove the stretch it runs back over",
            g = G,
            x2 = 2 * G,
            x3 = 3 * G
        )
    );
    // A loop whose last stretch runs over its first, three segments later.
    let lap = vec![
        pt(G, G),
        pt(3 * G, G),
        pt(3 * G, 2 * G),
        pt(2 * G, 2 * G),
        pt(2 * G, G),
        pt(4 * G, G),
    ];
    rejected_naming(
        e.execute(&draw_between(fa, fb, lap)),
        "`/route/4` the segment from",
    );
    // Running straight on through a collinear vertex is no defect.
    e.execute(&draw_between(fa, fb, vec![pt(G, G), pt(2 * G, G), pt(4 * G, G)]))
        .expect("a collinear vertex is accepted");
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]

    /// FT-437: for routes between two CPs through interior grid vertices (repeats and overlaps likely), each of
    /// `connection.draw` and `connection.route` accepts exactly the routes the naive oracle finds tidy, stores
    /// them as given (a refused `connection.route` leaves the route as it was), and `geometry::route_defect`
    /// agrees with the oracle.
    #[test]
    fn ft437_draw_and_route_accept_exactly_the_tidy_routes(
        inner in proptest::collection::vec((0i64..6, 0i64..4), 0..6),
    ) {
        let mut route = vec![pt(G, G)];
        route.extend(inner.iter().map(|&(i, j)| pt(i * G, j * G)));
        route.push(pt(4 * G, G));
        let tidy = !naive_defect(&route);
        proptest::prop_assert_eq!(ew_commands::geometry::route_defect(&route).is_none(), tidy);

        let (mut e, fa, fb) = two_devices();
        let drawn = e.execute(&draw_between(fa, fb, route.clone()));
        proptest::prop_assert_eq!(drawn.is_ok(), tidy, "{:?}: {:?}", route, drawn);
        match drawn {
            Ok(r) => {
                let w: Id<Connection> = typed(r.outcome.primary.expect("primary"));
                proptest::prop_assert_eq!(&row(&e, w).route, &route);
            }
            Err(err) => proptest::prop_assert!(matches!(err, CmdError::Rejected { .. }), "{:?}", err),
        }

        let r = e
            .execute(&draw_between(fa, fb, vec![pt(G, G), pt(4 * G, G)]))
            .expect("the straight wire");
        let w: Id<Connection> = typed(r.outcome.primary.expect("primary"));
        let routed = e.execute(&RouteConnection { connection: w, route: route.clone() });
        proptest::prop_assert_eq!(routed.is_ok(), tidy, "{:?}: {:?}", route, routed);
        let stored = row(&e, w).route;
        if tidy {
            proptest::prop_assert_eq!(stored, route);
        } else {
            proptest::prop_assert_eq!(stored, vec![pt(G, G), pt(4 * G, G)]);
        }
    }
}
