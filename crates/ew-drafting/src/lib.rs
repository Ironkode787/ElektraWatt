// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Schematic drafting (docs/design/06a-drafting.md, ADR-0034): how a page is judged, where its texts stand and where
//! its wires run.
//!
//! - [`model`]: the page as the layout sees it, in ticks of 1/16 grid step ([`DraftPage`], §2.2). Its adapters
//!   (`ew-scene`'s labels layer, `ew-api`'s `Session::page_draft`) build it once from the stored page.
//! - [`consts`]: the numbers of the layout, in one place (§1.5).
//! - [`measure`]: the one measure (§1.2): the faults F2 to F7, the unrouted wires, the cost C1 to C8 and the figures
//!   a reader compares. The checks and `ew layout` read it.
//! - [`labels`]: the one text placer (§5): [`settle_labels`].
//! - [`marks`]: a mark drawn from its wires' routes (§3.10): [`marks::draw_marks`].
//! - [`guides`]: the guides and the light nudge of a part placed by hand (§6): [`guides()`].
//! - [`occupancy`] (§3.3), [`search`] (§3.6), [`nets`] (§3.4), [`tracks`] (§3.5), `rounds` (§3.5, the rings round a
//!   body) and [`route`] (§3.2, §3.7 to §3.10): the router. [`route()`] makes a page's plan, [`route_wire`] the Wire tool's route; [`unrouted_line`] is
//!   the line of a wire without a route; [`tracks_needed`] counts the tracks of a row's stepping wires for the placer.
//! - [`place`] (§4) and [`rules`] (§1.6): the placer. [`arrange`] makes Arrange and route's plan, [`rules_kept`]
//!   checks a plan's page against the rules that bind it.
//!
//! The crate reads no project record and knows no nanometre: two numbers for one grid position cannot meet in it.

pub mod consts;
pub mod geom;
pub mod guides;
pub mod labels;
pub mod marks;
pub mod measure;
pub mod model;
pub mod nets;
pub mod occupancy;
pub mod place;
mod rounds;
pub mod route;
pub mod rules;
pub mod search;
pub mod tracks;

pub use guides::{Guide, GuideKind, Guided, guides};
pub use labels::{Settled, settle_labels};
pub use measure::{Counts, Findings, Passed, Quality, TextRef, measure};
pub use model::*;
pub use place::{Arranged, StripHeld, arrange};
pub use route::{
    Jobs, Plan, Reason, Scope, WireEnd, WireRoute, jobs, route, route_wire, unrouted_line, wire_clear,
};
pub use rules::{Broken, Rule, rules_kept};
pub use tracks::tracks_needed;
