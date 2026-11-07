// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The schematic router's page model and the layout measures (WP-S2.19, decision log ID-1498): what any router
//! design reads first. [`PageGeometry`] is a page as plain values in page nm (the representations' bodies with the
//! black boxes' outlines, the connection points, the representations' text boxes, the junctions, the frame's bands
//! and each connection's drawn polyline); [`measure`] says how well the connections are drawn on it. Neither reads
//! the model: `ew-api` builds the geometry of a page (`Session::page_geometry`), and `ew layout` prints the measures.
//!
//! # The measures
//!
//! All geometry is exact on integer nm, with no tolerance (0 nm): every point is an integer, the page's grid points
//! rounded once, so two things on one grid line have equal coordinates. A connection's polyline is taken without its
//! consecutive equal points; its **segments** are the pieces between consecutive points (a polyline of one point is
//! one segment of no length). A point **lies on** a segment when it is on the closed segment exactly
//! ([`on_segment`](crate::on_segment)). A segment runs **through** a rectangle when it has points strictly inside it
//! (a segment along an edge, or touching a corner, does not). A connection **ends at** a connection point or a
//! junction when that point lists the connection among its connections.
//!
//! The hard rules, which no route of a router may break:
//!
//! - **(a) Touch.** A connection touches a connection point, or a junction, that it does not end at, when the
//!   point's position lies on one of its segments. One item per (connection, point). Junctions are included as well
//!   as connection points: a line through a junction's dot reads as joined to it as much as one through a pin.
//! - **(b) Overlap.** Two connections overlap when a segment of each lies on one line and the two share a stretch of
//!   positive length. One item per pair of connections, with every stretch they share. Wires of one potential or
//!   meeting at one junction count as well: a reader cannot tell two wires on one line apart.
//! - **(c) Through a body.** A segment runs through a representation's body (or a black box's outline) other than
//!   along its own ends' escapes: for a connection point the connection ends at, the ray from its point along its
//!   leaving direction is that end's escape, and the part of a segment on it does not count inside the bodies of the
//!   representation that draws that point. One item per (connection, representation), with every stretch inside.
//! - **(d) Through a text.** A segment runs through a text box of a representation (a tag, a slot or pin text, a
//!   cross-reference text; one box per line). No escape applies. One item per (connection, representation).
//! - **(g) Four-way junction.** A junction has as many arms as there are distinct directions in which the
//!   connections ending at it leave it (the direction of each polyline's first segment from its end at the
//!   junction); four or more arms break the rule (IEC 61082-1's T junctions).
//!
//! The costs, which a router keeps low:
//!
//! - **(e) Crossing.** Two connections meet at a point where they share exactly one point between two of their
//!   segments, other than a point where either of them ends (a shared end, or the end of one on the other, which (a)
//!   names) and other than a point on a stretch they share (which (b) names). A T without a junction counts as a
//!   crossing. One item per (pair, point).
//! - **(f) Bends.** A connection bends at each vertex where its polyline changes direction (a vertex it runs straight
//!   through is no bend; a reversal is one). One item per connection with at least one bend.
//!
//! The frame's bands are part of the page's geometry for a router to keep clear of; no measure reads them yet.

mod measures;
mod page;
#[cfg(test)]
mod tests;

pub use measures::{Bends, Crossing, FourWay, Measures, Overlap, Through, Touch, TouchedPoint, measure};
pub use page::{Body, CpPoint, DrawnConnection, JunctionPoint, PageGeometry, Point, Rect, TextBox};
