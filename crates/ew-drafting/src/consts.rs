// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The numbers of the layout, in one place (docs/design/06a-drafting.md §1.5): nothing else in the program repeats
//! one. Distances are in ticks ([`TICKS_PER_STEP`] to a grid step); the weights of the cost are in grid steps of
//! wire, which the measure multiplies by [`TICKS_PER_STEP`] where it adds them to a length.

use crate::model::Tick;

/// The unit of the model: a grid step is 16 ticks, a half step 8 (§2.1).
pub const TICKS_PER_STEP: Tick = 16;

/// C1: a step of wire.
pub const STEP: i64 = 1;
/// C2: a bend.
pub const BEND: i64 = 4;
/// C3: a crossing of two wires.
pub const CROSSING: i64 = 6;
/// C4: a wire across a text.
pub const TEXT_CROSSED: i64 = 12;
/// C4: each step of a wire on a text.
pub const TEXT_STEP: i64 = 1;
/// C5: a label on another text, on another part's body, or outside the drawing area.
pub const LABEL_CLASH: i64 = 24;
/// C6: each place a label stands down its list.
pub const LABEL_RANK: i64 = 1;
/// C7: each step on the half grid, more.
pub const HALF_STEP_EXTRA: i64 = 1;

/// The router's least distance of a wire from a body it does not end at.
pub const BODY_GAP: Tick = TICKS_PER_STEP;
/// The router's least distance between two wires.
pub const WIRE_GAP: Tick = TICKS_PER_STEP;
/// The straight run of a wire out of its pin: the stretch two wires of one pin may share.
pub const STUB: Tick = TICKS_PER_STEP;
/// Parallel wires and tracks side by side.
pub const TRACK_PITCH: Tick = 2 * TICKS_PER_STEP;
/// Tracks where a channel is too narrow for [`TRACK_PITCH`].
pub const TRACK_PITCH_NARROW: Tick = TICKS_PER_STEP;
/// The placer: between the facing pins of two parts in line.
pub const CHAIN_GAP: Tick = 5 * TICKS_PER_STEP;
/// The placer: never less than this between the facing pins of two parts in line.
pub const CHAIN_GAP_MIN: Tick = 2 * TICKS_PER_STEP;
/// The router: a trunk from the pins it taps.
pub const TRUNK_GAP: Tick = 2 * TICKS_PER_STEP;
/// The router: trunks side by side.
pub const TRUNK_PITCH: Tick = 2 * TICKS_PER_STEP;
/// The placer: marks in a stack.
pub const MARK_PITCH: Tick = 2 * TICKS_PER_STEP;
/// The placer: a mark before its drop.
pub const MARK_LEAD: Tick = 2 * TICKS_PER_STEP;
/// The placer: between two footprints in a row.
pub const ROW_GAP: Tick = TICKS_PER_STEP;
/// The placer: a part joins a common line when within this.
pub const ROW_PULL: Tick = 6 * TICKS_PER_STEP;
/// The placer: between two groups set side by side.
pub const COLUMN_GAP: Tick = 6 * TICKS_PER_STEP;
/// The placer: what a move has to earn in the estimate, per part, in steps.
pub const MOVE_PRICE: i64 = 2;
/// The placer: the head and the foot of the sheet kept for trunks and their marks, and [`TRUNK_PITCH`] more per trunk.
pub const BAND: Tick = 3 * TICKS_PER_STEP;
/// The placer: an earth symbol from the pin it earths.
pub const EARTH_REACH: Tick = 8 * TICKS_PER_STEP;
/// The text placer: a label and anything it does not belong to stand at least this far apart (1/8 step).
pub const LABEL_CLEAR: Tick = 2;
/// The search areas round the two ends of a wire, before the drawing area.
pub const WINDOWS: [Tick; 2] = [4 * TICKS_PER_STEP, 16 * TICKS_PER_STEP];
/// Improvement passes of the router; text rounds.
pub const PASSES: u32 = 2;
/// The search's bound on the nodes of its area.
pub const MAX_NODES: usize = 262_144;
/// Guides: a pin within this across of the line it would face shows the guide.
pub const GUIDE_SHOW: Tick = 2 * TICKS_PER_STEP;
/// Guides: the pointer within this of a guide's line takes it (3/4 step).
pub const GUIDE_CATCH: Tick = 3 * TICKS_PER_STEP / 4;
/// Guides: the catch never reaches more than this many screen points beyond the ordinary half step.
pub const GUIDE_CATCH_PT: u32 = 8;
/// Cards drawn from the station view: their channel pins this far apart.
pub const CARD_PIN_PITCH: Tick = 5 * TICKS_PER_STEP;
/// Cards drawn from the station view: their height.
pub const CARD_HEIGHT: Tick = 6 * TICKS_PER_STEP;
/// Cards drawn from the station view: the most channel pins of one box of a card wider than the drawing area.
pub const CARD_PINS_MOST: usize = 16;
