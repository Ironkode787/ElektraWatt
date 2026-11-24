// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The grid fit (docs/design/06-schematic-editor.md §8 item 3) parts what the nearest grid points would push together:
//! a page's grid changed (`page.set_grid`) and a paste onto a page of another grid (`edit.paste`) leave no symbol on
//! another that stood apart, keep the order of the symbols in a row, and leave every wire a way.

use std::collections::BTreeSet;

use ew_api::routing::RouteScope;
use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_testids::seeded_ids;
use serde_json::{Value, json};

/// The point of the QET grid at column `c` and row `r` of grid steps (7 937 500 / 3 nm, rounded to the nm).
fn qet(c: i64, r: i64) -> Value {
    json!([(c * 7_937_500 + 1) / 3, (r * 7_937_500 + 1) / 3])
}

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e}"));
}

/// The QET columns of the coils of a row: twelve, three steps apart, with a step between their bodies. On the 4 mm grid
/// each coil's nearest grid point puts the last two one step (4 mm) apart, closer than their bodies are wide (two QET
/// steps), whether the page's grid changes under them or the whole row is pasted onto a page of 4 mm.
const COLUMNS: [i64; 12] = [66, 69, 72, 75, 78, 81, 84, 87, 90, 93, 96, 99];

/// A new project's page `1` (the QET grid) with two rows of coils at [`COLUMNS`], the upper row's A1 on row 30 and the
/// lower row's on row 45, each upper coil's A2 wired straight down to the A1 of the coil below it.
fn two_rows() -> Session {
    let mut s = Session::new_project(
        ProjectTemplate::Builtin,
        NewProjectKind::Schematic,
        SessionOptions {
            ids: Some(seeded_ids(1817)),
            ..SessionOptions::default()
        },
    )
    .expect("a new project");
    let page = s.page_tables("1").expect("page 1").page.to_string();
    if let Some(embed) = s.starter_embed("ew.iec.coil@qet10") {
        run(&mut s, "library.embed", embed);
    }
    let coil = s
        .starter_symbol_copy("ew.iec.coil@qet10")
        .expect("the starter coil")
        .to_string();
    let place = |s: &mut Session, c: i64, r: i64| {
        let before: BTreeSet<_> = s.query().functions().ids().collect();
        run(
            s,
            "device.place",
            json!({"page": page, "symbol": coil, "pos": qet(c, r), "target": {"new_device": {"letter": "K"}}}),
        );
        s.query()
            .functions()
            .ids()
            .find(|f| !before.contains(f))
            .expect("the coil's function")
            .to_string()
    };
    for c in COLUMNS {
        let (upper, lower) = (place(&mut s, c, 30), place(&mut s, c, 45));
        // A coil's A2 stands four steps below its A1.
        run(
            &mut s,
            "connection.draw",
            json!({
                "a": {"cp": {"function": upper, "idx": 1}},
                "b": {"cp": {"function": lower, "idx": 0}},
                "page": page,
                "route": [qet(c, 34), qet(c, 45)],
            }),
        );
    }
    s
}

/// The EW-CORE-077 findings ("Symbol on a symbol") on the page labelled `folio`, by their English messages.
fn on_one_another(s: &Session, folio: &str) -> Vec<String> {
    let page = s.page_tables(folio).map(|t| t.page);
    s.check(&ew_api::checks::DRAFTING, ew_api::checks::CheckMode::Online)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-077" && f.page.map(|p| p.as_uuid()) == page)
        .map(|f| f.message().clone())
        .collect()
}

/// The x of each upper coil's A1 on the page labelled `folio`, from left to right as the coils were placed.
fn upper_pins(s: &Session, folio: &str) -> Vec<i64> {
    let t = s.page_tables(folio).expect("a drawn page");
    let top = t.cps.iter().map(|c| c.at.1).min().expect("pins");
    let mut xs: Vec<i64> = t.cps.iter().filter(|c| c.at.1 == top).map(|c| c.at.0).collect();
    xs.sort_unstable();
    xs
}

/// Routes every wire of the page labelled `folio` (as the window does after a fit) and asserts that each found a way.
fn route_all(s: &mut Session, folio: &str) {
    let ids = s
        .page_tables(folio)
        .expect("a drawn page")
        .connections
        .iter()
        .map(|c| c.id)
        .collect();
    let plan = s
        .route_plan(folio, &RouteScope::Selection(ids))
        .expect("a drawn page");
    assert_eq!(plan.plan.unrouted, [], "every wire finds a way");
    run(s, "layout.apply", plan.params.expect("the routes"));
    let t = s.page_tables(folio).expect("a drawn page");
    assert!(t.connections.iter().all(|c| c.stored_route), "every wire routed");
}

/// Changed to the 4 mm grid, the page's rows of coils keep apart, in their order, two steps from pin to pin (they stood
/// three QET steps apart, a step between their bodies), and each wire finds its way again.
#[test]
fn a_grid_change_parts_the_coils_it_would_push_together() {
    let mut s = two_rows();
    assert_eq!(on_one_another(&s, "1"), Vec::<String>::new(), "apart before");
    let page = s.page_tables("1").expect("page 1").page.to_string();
    run(
        &mut s,
        "page.set_grid",
        json!({"page": page, "grid": {"num": 4_000_000, "den": 1}}),
    );
    assert_eq!(on_one_another(&s, "1"), Vec::<String>::new());
    let xs = upper_pins(&s, "1");
    assert_eq!(xs.len(), COLUMNS.len());
    assert!(xs.windows(2).all(|w| w[1] - w[0] >= 8_000_000), "{xs:?}");
    route_all(&mut s, "1");
}

/// The two rows pasted onto a page of the 4 mm grid are fitted to it together: no coil on another, the order kept, every
/// wire a way.
#[test]
fn a_paste_onto_another_grid_parts_the_coils_it_would_push_together() {
    let mut s = two_rows();
    let reps: Vec<_> = s
        .page_tables("1")
        .expect("page 1")
        .reps
        .iter()
        .map(|r| r.id)
        .collect();
    let fragment = ew_api::window::parse_fragment(&s.copy_fragment(&reps).expect("copied").text)
        .expect("the copy reads")
        .params;
    run(
        &mut s,
        "page.create",
        json!({"number": 2, "title": "Fit", "grid": {"num": 4_000_000, "den": 1}}),
    );
    let page = s.page_tables("2").expect("page 2").page.to_string();
    run(
        &mut s,
        "edit.paste",
        json!({"page": page, "at": [40_000_000, 40_000_000], "fragment": fragment}),
    );
    assert_eq!(on_one_another(&s, "2"), Vec::<String>::new());
    let xs = upper_pins(&s, "2");
    assert_eq!(xs.len(), COLUMNS.len());
    assert!(xs.windows(2).all(|w| w[1] - w[0] >= 8_000_000), "{xs:?}");
    route_all(&mut s, "2");
}
