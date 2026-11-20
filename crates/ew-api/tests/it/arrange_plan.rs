// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrange and route through the session (docs/design/06a-drafting.md §2.6, §4.9, §9.3 row 4): the plan is one
//! `layout.apply` that moves the symbols and stores their wires' routes, one Undo restores every place and route, and
//! the command refuses a symbol kept in place, one moved without its wires' routes and one out of the drawing area.

use crate::golden_common;

use ew_api::routing::RouteScope;
use ew_api::{AnyCommand, Origin};
use serde_json::json;

/// The places of the symbols and the routes of the wires of folio `folio`.
fn drawing(
    s: &ew_api::Session,
    folio: &str,
) -> (Vec<(ew_api::output::Uuid, (i64, i64))>, Vec<Vec<(i64, i64)>>) {
    let t = s.page_tables(folio).expect("a drawn page");
    (
        t.reps.iter().map(|r| (r.id, r.pos)).collect(),
        t.connections.iter().map(|c| c.polyline.clone()).collect(),
    )
}

fn apply(params: serde_json::Value) -> AnyCommand {
    AnyCommand {
        name: "layout.apply".into(),
        params,
    }
}

/// Golden A's distribution page: Arrange and route moves symbols; its one `layout.apply` stores them where the plan
/// puts them, and one Undo takes all of it back.
#[test]
fn an_arranged_page_is_one_command_and_one_undo_restores_every_place_and_route() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let folio = "103";
    let before = drawing(&s, folio);
    let plan = s
        .arrange_plan(folio, &RouteScope::Page, &[])
        .expect("a drawn page");
    assert!(!plan.moves.is_empty(), "symbols move on this page");
    let params = plan.params.clone().expect("a plan that changes the page");
    assert_eq!(params["parts"].as_array().map(Vec::len), Some(plan.moves.len()));
    s.execute(&apply(params)).expect("layout.apply");
    let after = drawing(&s, folio);
    let moved: Vec<_> = after.0.iter().filter(|r| !before.0.contains(r)).collect();
    assert_eq!(
        moved.len(),
        plan.moves.len(),
        "every symbol of the plan, and only those, moved"
    );
    assert!(after.1.iter().all(|r| !r.is_empty()), "every wire has its route");
    s.undo(&Origin::Gui).expect("undo");
    assert_eq!(
        drawing(&s, folio),
        before,
        "one Undo restores every place and route"
    );
}

/// The command's guards: a symbol kept in place, a symbol moved without the routes of its wires, a symbol moved off
/// the drawing area.
#[test]
fn layout_apply_refuses_a_kept_symbol_a_symbol_without_its_routes_and_one_out_of_the_drawing_area() {
    let batch = golden_common::batch();
    let mut s = golden_common::session(&batch);
    let t = s.page_tables("103").expect("folio 103");
    let page = t.page.to_string();
    // A symbol the plan moves: its wires get new routes in the plan.
    let plan = s.arrange_plan("103", &RouteScope::Page, &[]).expect("folio 103");
    let rep = plan.moves.first().expect("a symbol the plan moves").0.0;
    let one =
        |steps: [i64; 2]| apply(json!({"page": page, "parts": [{"rep": rep.to_string(), "steps": steps}]}));
    let words = |e: ew_api::CmdError| e.to_string();
    let no_routes = words(s.execute(&one([1, 0])).expect_err("its wires get no routes"));
    assert!(
        no_routes.contains("give every routed wire of a moved symbol its route"),
        "{no_routes}"
    );
    let outside = words(s.execute(&one([-1000, 0])).expect_err("off the drawing area"));
    assert!(
        outside.contains("drawing area") || outside.contains("drawable range"),
        "{outside}"
    );
    s.execute(&AnyCommand {
        name: "fields.set".into(),
        params: json!({"kind": "representation", "targets": [rep.to_string()], "path": "locked", "value": true}),
    })
    .expect("kept in place");
    let kept = words(s.execute(&one([1, 0])).expect_err("kept in place"));
    assert!(kept.contains("kept in place"), "{kept}");
}
