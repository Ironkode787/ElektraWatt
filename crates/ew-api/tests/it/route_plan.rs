// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route through the `Session` (docs/design/06a-drafting.md §2.6, §3, §7.6): the plan is stored by one
//! `layout.apply`, which a replay stores again without routing; a drawing made before keeps every junction's form, its
//! sequence and the wires it produces.

use ew_api::AnyCommand;
use ew_api::routing::RouteScope;

/// The routes of the page connections of folio `folio`, by ID.
fn routes(s: &ew_api::Session, folio: &str) -> Vec<(ew_api::output::Uuid, Vec<(i64, i64)>)> {
    let t = s.page_tables(folio).expect("a drawn page");
    let mut out: Vec<_> = t
        .connections
        .iter()
        .map(|c| {
            (
                c.id,
                if c.stored_route {
                    c.polyline.clone()
                } else {
                    Vec::new()
                },
            )
        })
        .collect();
    out.sort();
    out
}

/// Every junction of the project: its form, its sequence and the wires it produces.
fn junctions(
    s: &ew_api::Session,
    folios: &[String],
) -> Vec<(String, String, Vec<ew_api::output::Uuid>, Vec<String>)> {
    let mut out = Vec::new();
    for f in folios {
        let Some(t) = s.page_tables(f) else { continue };
        for j in &t.junctions {
            let wires = s
                .junction_wires(j.id)
                .into_iter()
                .map(|w| format!("{} {} {}", w.label, w.from, w.to))
                .collect();
            out.push((
                j.id.to_string(),
                format!("{} {}", j.style, j.wiring),
                j.sequence.clone(),
                wires,
            ));
        }
    }
    out.sort();
    out
}

/// The sample's folio 101 with every wire routed by Route: the plan is one `layout.apply`; the same command run again on
/// the project as it was stores the same routes (a replay stores, it does not route); and no junction of the project
/// changes its form, its sequence or the wires it produces.
#[test]
fn a_plan_is_stored_by_one_command_that_a_replay_repeats_and_no_junction_changes() {
    let mut s = crate::common::sample();
    let folios = s.schematic_folios();
    let before = junctions(&s, &folios);
    let ids: Vec<ew_api::output::Uuid> = s
        .page_tables("101")
        .expect("folio 101")
        .connections
        .iter()
        .map(|c| c.id)
        .collect();
    // Three wires lose their routes, so Route has something to do.
    let unroute: Vec<AnyCommand> = ids
        .iter()
        .take(3)
        .map(|id| AnyCommand {
            name: "connection.route".into(),
            params: serde_json::json!({"connection": id.to_string(), "route": []}),
        })
        .collect();
    for c in &unroute {
        s.execute(c).expect("connection.route");
    }
    let plan = s
        .route_plan("101", &RouteScope::Selection(ids))
        .expect("folio 101");
    let params = plan.params.clone().expect("a plan that routes the three wires");
    assert!(plan.plan.unrouted.is_empty(), "{:?}", plan.plan.unrouted);
    let cmd = AnyCommand {
        name: "layout.apply".into(),
        params,
    };
    s.execute(&cmd).expect("layout.apply");
    let routed = routes(&s, "101");
    assert!(routed.iter().all(|(_, r)| !r.is_empty()), "every wire routed");
    assert_eq!(junctions(&s, &folios), before, "no junction changes");
    // The same commands on the project as it was store the same routes: a replay stores, it does not route.
    let mut again = crate::common::sample();
    for c in unroute.iter().chain([&cmd]) {
        again.execute(c).expect("replayed");
    }
    assert_eq!(routes(&again, "101"), routed);
}
