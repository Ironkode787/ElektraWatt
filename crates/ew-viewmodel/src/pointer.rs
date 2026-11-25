// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pointer's tolerances (architecture §3.4): how far around a thing the pointer still takes it, in screen points
//! at every zoom. The canvas, the tools and the window read them here.

/// How far around an entity's rectangle the pointer still hovers it, in points.
pub const HOVER_SLOP_PT: f64 = 4.0;

/// How far a press may travel and still be a click, in screen points.
pub const CLICK_SLOP_PT: f64 = 3.0;

/// How near a corner of the selected legend a press grabs it, in screen points.
pub const GRIP_PT: f64 = 4.0;

/// How near a grip of a selected box a press takes it, in screen points (a drag that missed the small handle
/// moved the whole box): the round handle the wire's grips have, and a margin.
pub const GRIP_SLOP_PT: f64 = 7.0;

/// How far a connection point attracts the pointer, in screen points, at every zoom: what Select rings and what the
/// Wire tool's click takes (at eight points a click nine points off a port placed a bend). A little
/// under ten: at 100 % a step of the QET grid is ten points, and the grid point beside a pin stays one for a bend.
pub const CP_SNAP_PT: f64 = 9.5;

/// How near a rail or a snap target the pointer must come, in screen points.
pub const LAYOUT_SNAP_PT: f64 = 15.0;
