// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! One drawn path (FT-1020, decision log ID-1104, ID-1110; WP-S2.19 D1 to D3): the path the page scene draws a wire
//! along is the path `ew_derive::drawn_path` gives, which EW-TERM-006 and normaliser 7 read: the stored route. Over
//! Golden A's connections (the wire layer as drawn: every page connection has the router's route, so none is
//! unrouted), and over random ends (a proptest): a connection without a route is unrouted, has no drawn path and is
//! drawn as the straight line between its ends.
//!
//! W45.1 F1: EW-TERM-006's Messages row and its detail card (which shows the message cell) name the terminal once.

use std::path::Path;

use ew_api::output::ew_dl::{ItemKind, Layer, PathEl};
use ew_api::output::{NavKind, NavRole, PageLayer, TableCp, page_layer};
use ew_api::{AnyCommand, Session};
use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::{Connection, Dir4, Page};
use ew_derive::ew_units::{Nm, Pt2, Rect};
use ew_derive::{PageOrder, drawn_path};
use ew_scene::route::{RouteEnd, connection_polyline};
use proptest::prelude::*;
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

/// Golden A: every connection the wire layer draws is drawn along `drawn_path`, its stored route; no page connection
/// is unrouted (WP-S2.19 RG1).
#[test]
fn golden_a_s_wires_are_drawn_along_the_one_path() {
    let s = golden();
    let q = s.query();
    let order = q.get::<PageOrder>(());
    let mut drawn = 0;
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
            assert!(!c.route.is_empty(), "{} {}: unrouted", entry.folio, row.id);
            let path = drawn_path(&q, &c, page).expect("a routed wire is drawn");
            let scene = wire_of(&wires, row.id);
            assert_eq!(pts(&path), scene, "{} {}", entry.folio, row.id);
            assert_eq!(row.polyline, scene, "{} {}", entry.folio, row.id);
            drawn += 1;
        }
    }
    assert!(drawn > 0);
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

    /// Any two ends on a page (equal, on one line or apart; each with any leaving direction or none): without a stored
    /// route the scene draws the straight line between them (WP-S2.19 D3), nothing where they meet.
    #[test]
    fn the_scene_draws_an_unrouted_connection_straight_between_any_two_ends(
        a in (-3i64..=3, -3i64..=3),
        b in (-3i64..=3, -3i64..=3),
        da in 0usize..5,
        db in 0usize..5,
    ) {
        let (a, b) = (pt((a.0 * STEP, a.1 * STEP)), pt((b.0 * STEP, b.1 * STEP)));
        let sheet = Rect::from_points(pt((-3 * STEP, -3 * STEP)), pt((3 * STEP, 3 * STEP)));
        let scene = connection_polyline(
            &RouteEnd { pos: a, dir: leaving(da) },
            &RouteEnd { pos: b, dir: leaving(db) },
            &[],
            sheet,
        );
        let expected = if a == b { vec![a] } else { vec![a, b] };
        prop_assert_eq!(scene, expected);
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
        (json!({"cp": {"function": f.to_string(), "idx": c.idx}}), c.at)
    };
    let ((a, pa), (b, pb)) = (end(coils[0], true), end(coils[1], false));
    // A straight stored route (an unrouted wire has no wire geometry, WP-S2.19 D3).
    run(
        &mut s,
        "connection.draw",
        json!({"a": a, "b": b, "page": page, "route": [[pa.0, pa.1], [pb.0, pb.1]]}),
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
