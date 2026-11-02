// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One drawn path (FT-1020, decision log ID-1104, ID-1110): the path the page scene draws a connection along is the
//! path `ew_derive::drawn_path` gives, which EW-TERM-006 reads, and for a connection without a route the path
//! `ew_commands::geometry::provisional_path_of` gives, along which normaliser 7 splits it. Over Golden A's connections
//! (the wire layer as drawn, clamp included: every end lies on its sheet), over every pair of ends on folios 101 to 103
//! (the scene's polyline before the clamp), and over random ends (a proptest of positions and leaving directions).
//! A strip segment's terminal point is placed from derived queries that a transaction does not hold, so
//! `provisional_path_of` does not locate a connection ending there: the one gap, asserted as such.
//!
//! W45.1 F1: EW-TERM-006's Messages row and its detail card (which shows the message cell) name the terminal once.

use std::cell::RefCell;
use std::path::Path;

use ew_api::geometry::Dir;
use ew_api::output::ew_dl::{ItemKind, Layer, PathEl};
use ew_api::output::{NavKind, NavRole, PageLayer, PageTables, TableCp, page_layer};
use ew_api::{AnyCommand, Session};
use ew_commands::geometry::provisional_path_of;
use ew_commands::{
    CmdError, Command, CommandMeta, CommandRegistry, Engine, EngineOptions, Outcome, TxBuilder,
};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::{Connection, Dir4, Endpoint, Function, Junction, Page};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::{PageOrder, dir_vertical, drawn_path, provisional_path};
use ew_scene::route::{RouteEnd, connection_polyline, provisional_route};
use proptest::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Golden A's fixture, read-only (no lock; commands run in memory).
fn golden() -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416.ew");
    Session::open_read_only(&dir, "ElektraWatt test")
        .expect("the golden fixture opens")
        .0
}

fn pts(path: &[Pt2]) -> Vec<(i64, i64)> {
    path.iter().map(|p| (p.x.0, p.y.0)).collect()
}

fn pt(p: (i64, i64)) -> Pt2 {
    Pt2::new(Nm(p.0), Nm(p.1))
}

fn dir4(d: Dir) -> Dir4 {
    match d {
        Dir::Up => Dir4::Up,
        Dir::Down => Dir4::Down,
        Dir::Left => Dir4::Left,
        Dir::Right => Dir4::Right,
    }
}

/// A rectangle holding every point a record can hold: the scene's polyline before its clamp.
fn everywhere() -> Rect {
    let far = 1i64 << 60;
    Rect::from_points(pt((-far, -far)), pt((far, far)))
}

/// The polyline the wire layer draws for connection `id`.
fn wire_of(wires: &PageLayer, id: Uuid) -> Vec<(i64, i64)> {
    wires
        .list
        .items
        .iter()
        .find(|i| i.entity.map(|e| e.0) == Some(id))
        .and_then(|i| match &i.kind {
            ItemKind::Path(p) => Some(
                p.path
                    .els
                    .iter()
                    .filter_map(|el| match el {
                        PathEl::MoveTo(p) | PathEl::LineTo(p) => Some((p.x.0, p.y.0)),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no wire for {id}"))
}

/// Whether end `e` is a CP that a strip segment draws on the page of `t`.
fn by_strip_segment(t: &PageTables, e: &Endpoint) -> bool {
    let Endpoint::Cp(f, idx) = e else {
        return false;
    };
    t.cps
        .iter()
        .find(|c| c.function == f.as_uuid() && c.idx == *idx)
        .and_then(|c| t.reps.iter().find(|r| r.id == c.rep))
        .is_some_and(|r| r.rep_type == "strip_segment")
}

/// The paths `provisional_path_of` gives, one per connection.
type Paths = Vec<Option<Vec<Pt2>>>;

thread_local! {
    /// The connections [`ProbePaths`] reads and the paths it gives them, in the same order.
    static PROBE: RefCell<(Vec<Connection>, Paths)> = const { RefCell::new((Vec::new(), Vec::new())) };
}

/// A connection between two ends, the scene's polyline between them and whether an end is a strip segment's point.
type Pair = (Connection, Vec<(i64, i64)>, bool);

/// A command that changes nothing and computes `provisional_path_of` for the connections of [`PROBE`] in its
/// transaction: the function normaliser 7 calls, called as it calls it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProbePaths {}

impl Command for ProbePaths {
    const NAME: &'static str = "test.probe_paths";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        PROBE.with_borrow_mut(|(connections, paths)| {
            *paths = connections.iter().map(|c| provisional_path_of(tx, c)).collect();
        });
        Ok(Outcome::default())
    }
}

/// `provisional_path_of` of each of `connections` in a transaction over `s`'s model.
fn provisional_paths(s: &Session, connections: Vec<Connection>) -> Paths {
    let mut registry = CommandRegistry::builtin().clone();
    registry.register::<ProbePaths>();
    let mut engine = Engine::new(
        s.world().model,
        EngineOptions {
            registry: Some(registry),
            ..EngineOptions::default()
        },
    );
    PROBE.with_borrow_mut(|p| *p = (connections, Vec::new()));
    engine.preview(&ProbePaths {}).expect("the probe runs");
    PROBE.with_borrow_mut(|p| std::mem::take(&mut p.1))
}

/// Golden A: every connection the wire layer draws is drawn along `drawn_path`, and every one without a route along
/// `provisional_path_of` too, but for one that ends at a strip segment's terminal point.
#[test]
fn golden_a_s_wires_are_drawn_along_the_one_path() {
    let s = golden();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let mut drawn = 0;
    let mut route_less: Vec<(Connection, Vec<Pt2>, bool)> = Vec::new();
    for entry in &order.entries {
        let Some(t) = s.page_tables(&entry.folio) else {
            continue;
        };
        let page = Id::<Page>::from_uuid(t.page);
        let wires = page_layer(&q, entry.page, Layer::Wires);
        for row in &t.connections {
            let c = q
                .connections()
                .get(Id::from_uuid(row.id))
                .map(|c| Connection::clone(c))
                .expect("the connection");
            let Some(path) = drawn_path(&q, &c, page) else {
                continue;
            };
            let scene = wire_of(&wires, row.id);
            assert_eq!(pts(&path), scene, "{} {}", entry.folio, row.id);
            assert_eq!(row.polyline, scene, "{} {}", entry.folio, row.id);
            drawn += 1;
            if c.route.is_empty() {
                let segment = by_strip_segment(&t, &c.a) || by_strip_segment(&t, &c.b);
                route_less.push((c, path, segment));
            }
        }
    }
    assert!(drawn > 0 && !route_less.is_empty());
    let connections: Vec<Connection> = route_less.iter().map(|(c, _, _)| c.clone()).collect();
    for ((c, path, segment), got) in route_less.iter().zip(provisional_paths(&s, connections)) {
        if *segment {
            assert_eq!(got, None, "{}: ends at a strip segment", c.id);
        } else {
            assert_eq!(got.as_ref(), Some(path), "{}", c.id);
        }
    }
}

/// Every pair of ends on folios 101 to 103 (their CPs and junctions): the scene's polyline before the clamp,
/// `drawn_path` and `provisional_path_of` of a connection without a route between them are one path.
#[test]
fn every_pair_of_ends_on_folios_101_to_103_has_one_path() {
    let s = golden();
    let q = s.query();
    for folio in ["101", "102", "103"] {
        let t = s.page_tables(folio).expect("a drawn folio");
        let page = Id::<Page>::from_uuid(t.page);
        let template = q
            .connections()
            .get(Id::from_uuid(t.connections[0].id))
            .map(|c| Connection::clone(c))
            .expect("a connection");
        let cp_end = |c: &TableCp| {
            (
                Endpoint::Cp(Id::<Function>::from_uuid(c.function), c.idx),
                RouteEnd {
                    pos: pt(c.at),
                    dir: c.dir.map(dir4),
                },
            )
        };
        let mut ends: Vec<(Endpoint, RouteEnd)> = t.cps.iter().map(cp_end).collect();
        ends.extend(t.junctions.iter().map(|j| {
            (
                Endpoint::Junction(Id::<Junction>::from_uuid(j.id)),
                RouteEnd {
                    pos: pt(j.at),
                    dir: None,
                },
            )
        }));
        let mut pairs: Vec<Pair> = Vec::new();
        for (i, (a, ea)) in ends.iter().enumerate() {
            for (b, eb) in &ends[i + 1..] {
                let mut c = template.clone();
                (c.a, c.b, c.route, c.page) = (a.clone(), b.clone(), Vec::new(), Some(page));
                let scene = pts(&connection_polyline(ea, eb, &[], everywhere()));
                let path = drawn_path(&q, &c, page).expect("both ends are drawn");
                assert_eq!(pts(&path), scene, "{folio}: {:?} to {:?}", c.a, c.b);
                let segment = by_strip_segment(&t, a) || by_strip_segment(&t, b);
                pairs.push((c, scene, segment));
            }
        }
        let connections: Vec<Connection> = pairs.iter().map(|(c, _, _)| c.clone()).collect();
        for ((c, scene, segment), got) in pairs.iter().zip(provisional_paths(&s, connections)) {
            let got = got.map(|p| pts(&p));
            if *segment {
                assert_eq!(got, None, "{folio}: {:?} to {:?}", c.a, c.b);
            } else {
                assert_eq!(got.as_ref(), Some(scene), "{folio}: {:?} to {:?}", c.a, c.b);
            }
        }
    }
}

const STEP: i64 = 4_000_000;

fn leaving(k: usize) -> Option<Dir4> {
    [
        None,
        Some(Dir4::Up),
        Some(Dir4::Down),
        Some(Dir4::Left),
        Some(Dir4::Right),
    ][k]
        .clone()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any two ends on a page (equal, on one line or apart; each with any leaving direction or none): the scene draws
    /// `ew_derive::provisional_path` between them.
    #[test]
    fn the_scene_draws_the_provisional_path_between_any_two_ends(
        a in (-3i64..=3, -3i64..=3),
        b in (-3i64..=3, -3i64..=3),
        da in 0usize..5,
        db in 0usize..5,
    ) {
        let (a, b) = (pt((a.0 * STEP, a.1 * STEP)), pt((b.0 * STEP, b.1 * STEP)));
        let sheet = Rect::from_points(pt((-3 * STEP, -3 * STEP)), pt((3 * STEP, 3 * STEP)));
        let scene = provisional_route(
            &RouteEnd { pos: a, dir: leaving(da) },
            &RouteEnd { pos: b, dir: leaving(db) },
            sheet,
        );
        let vertical = |d: Option<Dir4>| d.as_ref().and_then(dir_vertical);
        prop_assert_eq!(scene, provisional_path(a, vertical(leaving(da)), b, vertical(leaving(db))));
    }
}

/// Runs `name` with `params` on `s`; the functions it created.
fn run(s: &mut Session, name: &str, params: serde_json::Value) -> Vec<Uuid> {
    let before: Vec<Uuid> = s.query().functions().iter().map(|(id, _)| id.as_uuid()).collect();
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    s.query()
        .functions()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .filter(|f| !before.contains(f))
        .collect()
}

/// W45.1 F1 (decision log ID-1110): on an empty stretch of Golden A's folio 103, terminal -X1031 71 and two coils
/// above and below it, then a wire drawn from the upper coil to the lower one across the terminal (drawing a wire does
/// not split it at a symbol it crosses): EW-TERM-006, whose Messages row names the terminal once. Its message leads with
/// the subject, so the row shows the message alone; the detail card shows the message cell, which names it once too.
#[test]
fn ew_term_006_s_row_names_the_terminal_once() {
    let mut s = golden();
    let page = s.page_tables("103").expect("folio 103").page.to_string();
    for key in ["ew.iec.terminal@qet10", "ew.iec.coil@qet10"] {
        if let Some(embed) = s.starter_embed(key) {
            run(&mut s, "library.embed", embed);
        }
    }
    let symbol = |key: &str| s.starter_symbol_copy(key).expect("a starter symbol").to_string();
    let (terminal, coil) = (symbol("ew.iec.terminal@qet10"), symbol("ew.iec.coil@qet10"));
    let strip = s.resolve_reference("tag:-X1031").expect("strip -X1031");
    // Grid indexes (60, 20), (60, 30) and (60, 40) on the QET grid (7 937 500 / 3 nm, a point rounded to the nm).
    let at = |row: i64| json!([158_750_000, (row * 7_937_500 + 1) / 3]);
    run(
        &mut s,
        "device.place",
        json!({"page": page, "symbol": terminal, "pos": at(30),
            "target": {"new_terminal": {"strip": strip.to_string(), "designation": "71"}}}),
    );
    let mut coils = Vec::new();
    for row in [20, 40] {
        let f = run(
            &mut s,
            "device.place",
            json!({"page": page, "symbol": coil, "pos": at(row), "target": {"new_device": {"letter": "K"}}}),
        );
        coils.push(f[0]);
    }
    // The upper coil's lower CP and the lower coil's upper CP.
    let t = s.page_tables("103").expect("folio 103");
    let end = |f: Uuid, lower: bool| {
        let mut cps: Vec<&TableCp> = t.cps.iter().filter(|c| c.function == f).collect();
        cps.sort_by_key(|c| c.at.1);
        let c = if lower { cps[cps.len() - 1] } else { cps[0] };
        json!({"cp": {"function": f.to_string(), "idx": c.idx}})
    };
    run(
        &mut s,
        "connection.draw",
        json!({"a": end(coils[0], true), "b": end(coils[1], false), "page": page}),
    );
    let messages = s.navigator_table(NavKind::Messages);
    let rows: Vec<(&str, &str, &str)> = messages
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Finding && n.cells[1] == "EW-TERM-006")
        .map(|n| (n.label.as_str(), n.cells[2].as_str(), n.cells[3].as_str()))
        .collect();
    let message =
        "=P01+J-E-2416-X1031:71 lies on wire 103.3 without joining it; connect it to the wire or move it";
    // The row's text, the message cell (the detail card's text) and the subject cell.
    assert_eq!(rows, [(message, message, "=P01+J-E-2416-X1031:71")]);
}
