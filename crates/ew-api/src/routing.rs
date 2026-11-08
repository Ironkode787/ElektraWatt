// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The schematic router as the canvas tools, the actions and the CLI reach it (WP-S2.19 D4): the core of
//! `ew_derive::routing` re-exported ([`route_page`], [`route_one`], [`clear_one`], [`RoutePlan`]),
//! [`Session::route_plan`] for Route page, Route selection, a move's re-solve and Golden A's route tool, and
//! [`route_many_params`], the `connection.route_many` that commits a plan. The Wire tool routes its ghost with
//! [`route_one`] against the page geometry the host hands it, and passes the result as `connection.draw`'s `route`,
//! so the committed route is exactly the ghost and the router never runs on replay.

use ew_derive::ew_ids::Uuid;
pub use ew_derive::routing::{
    Heading, Measures, OneEnd, PageGeometry, RoutePlan, Unrouted, clear_one, measure, route_one, route_page,
};

use crate::Session;
use crate::geometry::Dir;

/// What a page solve routes (WP-S2.19 D13).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum RouteScope {
    /// Route page: the page's unrouted connections.
    Page,
    /// Route selection: the connections named, and every connection of the page with an end on a representation
    /// named; routed ones are routed again.
    Selection(Vec<Uuid>),
}

impl Session {
    /// The plan of a page solve on the page labelled `folio` (WP-S2.19 §5, D4): `scope`'s connections routed against
    /// the page as drawn ([`Session::page_geometry`]); `None` for an unknown folio or a page that is not drawn.
    pub fn route_plan(&self, folio: &str, scope: &RouteScope) -> Option<RoutePlan> {
        let page = self.page_geometry(folio)?;
        let jobs: Vec<Uuid> = match scope {
            RouteScope::Page => page
                .connections
                .iter()
                .filter(|c| !c.routed)
                .map(|c| c.id)
                .collect(),
            RouteScope::Selection(ids) => {
                let on_rep = |end: Uuid| page.cps.iter().any(|cp| cp.id == end && ids.contains(&cp.rep));
                page.connections
                    .iter()
                    .filter(|c| ids.contains(&c.id) || c.ends.iter().any(|e| on_rep(*e)))
                    .map(|c| c.id)
                    .collect()
            }
        };
        Some(route_page(&page, &jobs))
    }
}

/// The parameters of the `connection.route_many` that commits `plan`'s routes (WP-S2.19 D12); `None` for a plan
/// without a route.
pub fn route_many_params(plan: &RoutePlan) -> Option<serde_json::Value> {
    if plan.routes.is_empty() {
        return None;
    }
    let routes: Vec<serde_json::Value> = plan
        .routes
        .iter()
        .map(|(id, route)| {
            let points: Vec<serde_json::Value> =
                route.iter().map(|p| serde_json::json!([p.0, p.1])).collect();
            serde_json::json!({"connection": id.to_string(), "route": points})
        })
        .collect();
    Some(serde_json::json!({ "routes": routes }))
}

/// The router's heading of a tool's direction.
pub fn heading(d: Dir) -> Heading {
    match d {
        Dir::Up => Heading::Up,
        Dir::Down => Heading::Down,
        Dir::Left => Heading::Left,
        Dir::Right => Heading::Right,
    }
}
