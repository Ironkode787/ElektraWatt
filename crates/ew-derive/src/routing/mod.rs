// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The schematic router's page model and its page solve: what any router design reads first. [`PageGeometry`] is a
//! page as plain values in page nm (the representations' bodies with the black boxes' outlines, the connection points,
//! the representations' text boxes, the junctions, the frame's bands and each connection's drawn polyline). `ew-api`
//! builds the geometry of a page (`Session::page_geometry`). The solve judges its routes by the one measure,
//! `ew_drafting::measure` (docs/design/06a-drafting.md §1.2), through its bridge `draft`; the solve and the bridge go
//! with step 2 of that design's build (§10).

mod draft;
mod page;
// The page solve and one route against a page as drawn.
mod plan;
#[cfg(test)]
mod plan_tests;
mod search;

pub use draft::{Bends, Crossing, Measures, Overlap, Through, Touch, measure};
pub use page::{Body, CpPoint, DrawnConnection, JunctionPoint, PageGeometry, Point, Rect, TextBox};
pub use plan::{OneEnd, RoutePlan, SUPPLY_ORDER, Unrouted, clear_one, crosses_text, route_one, route_page};
pub use search::{
    BEND_COST, CROSSING_COST, Heading, MATE_STEP_COST, MAX_NODES, RouteEnd, RouteRequest, Routed, STEP_COST,
    TEXT_COST, auto_route, clear, route, unrouted_line,
};
