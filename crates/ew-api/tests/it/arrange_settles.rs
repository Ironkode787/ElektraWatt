// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Arrange and route and Route settle in one press (docs/design/06a-drafting.md §4.7 step 7, §3.10): the page a plan
//! stores is the page it was judged on, a cable mark drawn where the plan's routes cross it included, so a second press
//! offers nothing.

use crate::common;

use ew_api::routing::RouteScope;
use ew_api::{AnyCommand, Origin};
use serde_json::json;

fn cmd(name: &str, params: serde_json::Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

/// Each symbol of each drawn page of the sample moved two steps up, alone: after the plan of Arrange and route is
/// stored a second press offers nothing, and after the plan of Route a second Route neither.
#[test]
fn after_one_symbol_is_moved_a_second_press_offers_nothing() {
    let mut s = common::sample();
    let mut folios: Vec<u32> = s.query().pages().iter().map(|(_, p)| p.folio.number).collect();
    folios.sort_unstable();
    for folio in folios.iter().map(u32::to_string) {
        let Some(tables) = s.page_tables(&folio) else {
            continue;
        };
        if s.page_draft(&folio).is_none() {
            continue;
        }
        for rep in tables.reps.iter().map(|r| r.id) {
            let moved = json!({"reps": [rep.to_string()], "steps": [0, -2]});
            if s.execute(&cmd("rep.move", moved)).is_err() {
                continue;
            }
            let name = s.symbol_name(rep).unwrap_or_default();
            for arrange in [true, false] {
                let plan = |s: &ew_api::Session| {
                    if arrange {
                        s.arrange_plan(&folio, &RouteScope::Page, &[])
                    } else {
                        s.route_plan(&folio, &RouteScope::Page)
                    }
                    .expect("a drawn page")
                };
                let Some(params) = plan(&s).params else {
                    continue;
                };
                s.execute(&cmd("layout.apply", params)).expect("layout.apply");
                let again = plan(&s);
                assert!(
                    again.params.is_none(),
                    "folio {folio}, {name} two steps up: a second press of {} offers {} moves and {} routes",
                    if arrange { "Arrange and route" } else { "Route" },
                    again.moves.len(),
                    again.plan.routes.len()
                );
                s.undo(&Origin::Gui).expect("undo the plan");
            }
            s.undo(&Origin::Gui).expect("undo the move");
        }
    }
}
