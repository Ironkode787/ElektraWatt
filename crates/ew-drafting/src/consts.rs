// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The numbers of the layout, in one place (docs/design/06a-drafting.md §1.5): nothing else in the program repeats
//! one. Distances are in ticks ([`TICKS_PER_STEP`] to a grid step), but for a drawn mark's sizes and the commands'
//! drawing distances, which are in nm (at the end); the weights of the cost are in grid steps of wire, which the
//! measure multiplies by [`TICKS_PER_STEP`] where it adds them to a length.

use ew_units::Nm;

use crate::model::Tick;

/// The unit of the model: a grid step is 16 ticks, a half step 8 (§2.1).
pub const TICKS_PER_STEP: Tick = 16;
/// Half a grid step: the spacing of the router's nodes, the half grid (§2.1, §3.3).
pub const HALF: Tick = TICKS_PER_STEP / 2;

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
/// C8: a mark's wire that bends on its way to the trunk it taps, so the trunk runs off the mark's line.
pub const MARK_JOG: i64 = 32;

/// The router's least distance of a wire from a body it does not end at.
pub const BODY_GAP: Tick = TICKS_PER_STEP;
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
/// The placer: what a move has to earn on the routed page, per part, in steps.
pub const MOVE_PRICE: i64 = 2;
/// The placer: how many steps a tree's root may step, each way, to find its tree room.
pub const REACH: i32 = 24;
/// The placer: the most units of a tree that are tried as its anchor.
pub const ANCHORS: usize = 6;
/// The placer: a unit's weight for each of its pins that has a wire (§4.2).
pub const PIN_WEIGHT: i64 = 4;
/// The placer: a unit's weight grows by one for each this many square steps of its bodies' area (§4.2).
pub const AREA_PER_WEIGHT: i64 = 16;
/// The placer: a tree's anchors are its units of at least this share of its heaviest unit's weight, as numerator and
/// denominator (§4.2).
pub const ANCHOR_SHARE: (i64, i64) = (3, 4);
/// The placer: two units of one shape are mates where the nearer stands within this of the other (§4.2).
pub const MATE_REACH: Tick = 2 * ROW_PULL;
/// The placer: the pitches a stack of marks tries, the least first (§4.4).
pub const MARK_PITCHES: [Tick; 3] = [MARK_PITCH, MARK_PITCH + TICKS_PER_STEP, 2 * MARK_PITCH];
/// The placer: the trees are set again at most this many times after their first setting (§4.4).
pub const RESETS: u32 = 2;
/// The placer: on the page of a few parts the text placer is asked about, the wire of each of their other wired pins,
/// straight out of the pin (§4.5).
pub const TRIAL_STUB: Tick = 2 * TICKS_PER_STEP;
/// The placer: the most trees, and the most branches, whose plans are routed one by one.
pub const BRANCHES: usize = 8;
/// The placer: the most runs one press makes (it stops at the first that changes nothing).
pub const ROUNDS: usize = 4;
/// The placer: an earth symbol from the pin it earths.
pub const EARTH_REACH: Tick = 8 * TICKS_PER_STEP;
/// The text placer: a label and anything it does not belong to stand at least this far apart (1/8 step).
pub const LABEL_CLEAR: Tick = 2;
/// The search areas round the two ends of a wire, before the drawing area.
pub const WINDOWS: [Tick; 2] = [4 * TICKS_PER_STEP, 16 * TICKS_PER_STEP];
/// The router: its improvement passes (§3.6).
pub const PASSES: u32 = 2;
/// The router: its text rounds (§3.7).
pub const TEXT_ROUNDS: u32 = 2;
/// The router: the rounds in which a job takes its stored route back (§3.9).
pub const TAKE_BACK_ROUNDS: u32 = 2;
/// The router: the candidate trunk lines beyond the pins a trunk taps, each way (§3.4).
pub const TRUNK_LINES: Tick = 8;
/// The router: what a wire left to the search adds to its ends' distance where trunks are compared, two bends' worth
/// of steps (§3.4). It is written apart from [`BEND`]: the two do not follow each other.
pub const LEFT_TO_SEARCH: Tick = 8 * TICKS_PER_STEP;
/// The router: the search takes the half grid where the grid's route is longer than this share of the city-block
/// distance of the ends, or of the half grid's route, as numerator and denominator (§3.6).
pub const DETOUR_MOST: (i64, i64) = (8, 5);
/// The text placer: a stretch of a wire takes its number where it is the number's length and this more (§5.2).
pub const NUMBER_ROOM: Tick = 2 * TICKS_PER_STEP;
/// The text placer: a wire number stands level with the number of a parallel wire within this (§5.2).
pub const NUMBER_LEVEL: Tick = 2 * TRACK_PITCH;
/// The text placer: the places along a stretch a wire number tries after the level one, as shares of the stretch's
/// length, numerator and denominator (§5.2).
pub const NUMBER_AT: [(i64, i64); 3] = [(1, 2), (1, 4), (3, 4)];
/// The text placer: the lines of a block of texts stand at most this far apart (§5.2).
pub const LINE_GAP: Tick = TICKS_PER_STEP / 4;
/// The search's bound on the nodes of its area.
pub const MAX_NODES: usize = 262_144;
/// Guides: a pin within this across of the line it would face shows the guide.
pub const GUIDE_SHOW: Tick = 2 * TICKS_PER_STEP;
/// Guides: the pointer within this of a guide's line takes it (3/4 step).
pub const GUIDE_CATCH: Tick = 3 * TICKS_PER_STEP / 4;
/// Guides: the catch never reaches more than this many screen points beyond the ordinary half step.
pub const GUIDE_CATCH_PT: u32 = 8;
/// Cards drawn from the station view: their channel pins this far apart (a field device with its tag and address
/// fits over each pin, and a card of eight channels beside the controller at the head of an A3 sheet).
pub const CARD_PIN_PITCH: Tick = 8 * TICKS_PER_STEP;
/// Cards drawn from the station view: their height.
pub const CARD_HEIGHT: Tick = 6 * TICKS_PER_STEP;
/// Cards drawn from the station view: the most channel pins of one box of a card wider than the drawing area.
pub const CARD_PINS_MOST: usize = 8;

// The sizes of a drawn mark and the commands' drawing distances: in nm, not ticks, for a page's grid step has no fixed
// length and these stand the same on every grid. The layout reads a mark's sizes from the scene's drawn pieces
// (`marks`), never from here.

/// How far a wire marker's line reaches past its outer wires (FT-1706).
pub const MARKER_REACH: Nm = Nm(1_500_000);
/// The gap between a wire marker's line and its text (FT-1706).
pub const MARKER_GAP: Nm = Nm(1_000_000);
/// Half the run of a cable mark's slanted stroke on a core, along and across the line.
pub const CABLE_TICK: Nm = Nm(1_000_000);
/// The gap between a cable mark's stroke or line and the texts beside them.
pub const CABLE_TEXT_GAP: Nm = Nm(500_000);
/// The commands: the shortest stretch a cable's mark placed with its cores prefers, so that its strokes and core
/// names have room: 6 mm. A preference of the placing that [`CABLE_TICK`] and [`CABLE_TEXT_GAP`] do not give.
pub const CABLE_MARK_ROOM: Nm = Nm(6_000_000);
/// The commands: how far above or below a supply point another point of its group may stand: 20 mm, the spacing of
/// a column of supply points drawn one under the other (the window places them about 16 mm apart, an imported
/// drawing 4 mm). The window spaces such a column by grid steps (`ew_viewmodel::tools::supply_place::SUPPLY_STEPS`),
/// which no length in nm equals on every grid.
pub const SUPPLY_GROUP_REACH: Nm = Nm(20_000_000);
