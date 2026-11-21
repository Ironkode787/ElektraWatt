// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrange and route through the session (docs/design/06a-drafting.md §2.6, §4.9, §9.3 row 4): the plan is one
//! `layout.apply` that moves the symbols and stores their wires' routes, one Undo restores every place and route, and
//! the command refuses a symbol kept in place, one moved without its wires' routes and one out of the drawing area.

use crate::common;

use ew_api::routing::RouteScope;
use ew_api::{AnyCommand, Origin};
use serde_json::json;

/// The places of the symbols and the routes of the wires of a page.
type Drawing = (Vec<(ew_api::output::Uuid, (i64, i64))>, Vec<Vec<(i64, i64)>>);

/// The places of the symbols and the routes of the wires of folio `folio`.
fn drawing(s: &ew_api::Session, folio: &str) -> Drawing {
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

/// The sample with its motor breaker -103F1 moved three grid steps (7.9375 mm) right on folio 103, out of line with
/// the contactor's main contacts under it: the sample's pages are drawn in line, so Arrange and route has nothing to
/// move on them as they are.
fn displaced() -> ew_api::Session {
    let mut s = common::sample();
    let breaker = s.resolve_reference("-103F1").expect("the motor breaker");
    let rep = s
        .page_tables("103")
        .expect("folio 103")
        .reps
        .iter()
        .find(|r| r.device == Some(breaker))
        .expect("-103F1 on 103")
        .id;
    s.execute(&AnyCommand {
        name: "rep.move".into(),
        params: json!({"reps": [rep.to_string()], "delta": [7_937_500, 0]}),
    })
    .expect("rep.move");
    s
}

/// The motor page with a breaker out of line: Arrange and route moves symbols; its one `layout.apply` stores them
/// where the plan puts them, and one Undo takes all of it back.
#[test]
fn an_arranged_page_is_one_command_and_one_undo_restores_every_place_and_route() {
    let mut s = displaced();
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
    let mut s = displaced();
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

/// What of a page's layout model a move by whole steps leaves as it is: each text's size and places (relative to its
/// owner) and each symbol's body (relative to its origin), in ticks.
type Shapes = (
    Vec<(ew_drafting::LabelId, (i32, i32), ew_drafting::Places)>,
    Vec<(ew_drafting::PartId, ew_drafting::Rect)>,
);

fn shapes(s: &ew_api::Session, folio: &str) -> Shapes {
    let d = s.page_draft(folio).expect("a drawn page");
    (
        d.labels
            .iter()
            .map(|l| (l.id, l.size, l.places.clone()))
            .collect(),
        d.parts.iter().map(|p| (p.id, p.body)).collect(),
    )
}

/// Every plan Arrange and route offers on a page of the sample is stored by its `layout.apply`: on its grid, whose step
/// is no whole number of nm, the route of a moved symbol's wire ends on the number the moved connection point then
/// has (docs/design/06a-drafting.md §2.1). The page as stored is the page the plan was judged on: every text keeps its
/// box in ticks and every symbol its body where it was moved to. And on the page as stored a second press offers
/// nothing.
#[test]
fn every_plan_arrange_offers_is_stored_by_its_command() {
    let mut s = displaced();
    let mut folios: Vec<u32> = s.query().pages().iter().map(|(_, p)| p.folio.number).collect();
    folios.sort_unstable();
    let mut stored = false;
    for folio in folios {
        let folio = folio.to_string();
        let Some(plan) = s.arrange_plan(&folio, &RouteScope::Page, &[]) else {
            continue;
        };
        let Some(params) = plan.params.clone() else {
            continue;
        };
        let before = shapes(&s, &folio);
        if let Err(e) = s.execute(&apply(params)) {
            panic!("folio {folio}: {e}");
        }
        stored |= !plan.moves.is_empty();
        assert_eq!(
            shapes(&s, &folio),
            before,
            "folio {folio}: the stored page's texts and bodies"
        );
        // The stored page is the plan's own fixed point: a second press offers nothing.
        let again = s.arrange_plan(&folio, &RouteScope::Page, &[]);
        assert!(
            again.is_none_or(|p| p.params.is_none()),
            "folio {folio}: a second press offers a plan"
        );
        s.undo(&Origin::Gui).expect("undo");
    }
    assert!(stored, "a plan with moved symbols was stored");
}
