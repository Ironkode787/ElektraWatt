// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A stored route that misses its end (docs/design/06a-drafting.md §3.2): no command stores one, but a file
//! written outside the program can, and the page joins the end to it by one straight stretch, a hook. A page-wide
//! Route takes it as broken, flags it as the person's and draws it to its end.

use crate::common;

use ew_api::AnyCommand;
use ew_api::routing::RouteScope;
use ew_schema::Record;

/// The sample with the first stretch of a routed wire moved three grid steps across, off the connection point it
/// starts at; and that wire's ID.
fn hooked() -> (ew_api::Session, ew_ids::Uuid) {
    let (project, mut records, lib) = common::sample_records();
    let mut hooked = None;
    for r in &mut records {
        let Record::Connection(c) = r else { continue };
        if hooked.is_some() || c.route.len() < 3 {
            continue;
        }
        let (a, b) = (c.route[0], c.route[1]);
        // Three QET steps are a whole number of nanometres: 7.9375 mm.
        let across = ew_units::Nm(7_937_500);
        if a.x == b.x && a.y != b.y {
            c.route[0].x = a.x + across;
            c.route[1].x = b.x + across;
        } else if a.y == b.y && a.x != b.x {
            c.route[0].y = a.y + across;
            c.route[1].y = b.y + across;
        } else {
            continue;
        }
        hooked = Some(c.id.as_uuid());
    }
    let id = hooked.expect("a routed wire");
    (common::session_of(project, records, lib), id)
}

#[test]
fn a_page_wide_route_draws_a_route_that_misses_its_end_to_it() {
    let (mut s, id) = hooked();
    let mut folios: Vec<u32> = s.query().pages().iter().map(|(_, p)| p.folio.number).collect();
    folios.sort_unstable();
    let folio = folios
        .iter()
        .map(u32::to_string)
        .find(|f| {
            s.page_tables(f)
                .is_some_and(|t| t.connections.iter().any(|c| c.id == id))
        })
        .expect("the wire's page");
    let plan = s.route_plan(&folio, &RouteScope::Page).expect("a drawn page");
    assert!(
        plan.plan.his.iter().any(|w| w.0 == id),
        "flagged as the person's: {:?}",
        plan.plan.his
    );
    let params = plan.params.expect("a plan");
    s.execute(&AnyCommand {
        name: "layout.apply".into(),
        params,
    })
    .expect("layout.apply");
    let t = s.page_tables(&folio).expect("a drawn page");
    let c = t.connections.iter().find(|c| c.id == id).expect("the wire");
    let draft = s.page_draft(&folio).expect("a drawn page");
    let wire = draft.wires.iter().find(|w| w.id.0 == id).expect("the wire");
    assert!(
        c.stored_route && !wire.stale,
        "its route meets its ends: {:?}",
        c.polyline
    );
}
