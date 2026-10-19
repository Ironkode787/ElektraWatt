// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Synthetic S-XL fixtures (WP-S1.05): format-level records of generated projects in the four size classes of
//! NFR §2.1, for the performance gates, benchmarks and scale tests.
//!
//! # What is generated
//!
//! [`topology`] plans the project as pure indices: a Golden-B-shaped cabinet (a "replica" of 18 folios with a
//! supply, a PSU, a distribution, a PLC station with a bus cable, loads, terminals and PLC modules) is
//! replicated until the page bound of the class is reached, then densified to the device, function,
//! connection, terminal and channel bounds exactly and to the CP bound from below. Power rails (L, N, PE) run
//! through interruption-point pairs across all replicas, 24 V and 0 V rails per replica, the PLC stations form
//! a daisy chain over cable cores, and the remaining connection budget is page-local signal wires. This module
//! turns the plan into records ([`project`], [`emit`]) and ships the handles benchmarks need ([`Handles`]).
//!
//! Every page is a multi-line page (`PageType::MultiLine`), including the folios that doc 20 draws as cover,
//! topology or layout pages, so that every representation type the plan uses is valid on every page. Library
//! copies (one form, and one function definition with its symbol per plan entry) are embedded in the project.
//!
//! # Signal wires, routes and symbols
//!
//! Signal wires never join two CPs of one device, and PLC channels reach load and contact CPs through terminals
//! (chains of four CPs; the terminal function carries an internal `Short` link between its sides `a` and `b`):
//! the pairing is described in [`topology`]. A symbol has one CP per CP of its function definition, laid out in
//! rows of six grid steps from the representation's origin, so a CP is drawn at the representation's position
//! plus `(idx % 6, idx / 6)` grid steps and stays inside the six-by-six-step cell of its slot; slots sit on
//! multiples of three steps, so every CP position is a grid position.
//!
//! **Routes.** With the default [`Options`] (`routes: true`) a documented fraction of the signal wires is
//! drawn with a route: every 4th signal wire of a page in plan order (25 %, see
//! [`topology::ROUTE_EVERY`]) and every signal wire that ends at the main function of a load (so that
//! `rep.move` of a load updates routes). A route is orthogonal with two vertices (both CPs share a
//! coordinate) or three (an L, horizontal leg first), starts and ends exactly at the two CP positions, and has
//! every vertex on the connection grid. Rails, bus cores and the remaining signal wires have empty routes.
//! `Options { routes: false }` ([`project_with`], [`emit_with`]) leaves every route empty and changes nothing
//! else: the records are equal to the default ones except for the `route` fields, and the IDs are the same.
//! `rep.move` of a representation moves the route ends of its attached wires with it
//! (`geometry::follow_end`: the end vertex moves by the delta and its neighbour keeps the segment
//! orthogonal; both ends moved: the route translates) and never rejects because of a route unless a
//! coordinate would leave the drawable range.
//!
//! # Determinism
//!
//! The plan is index arithmetic that depends on the class only. The seed only feeds the identifiers: one
//! [`V7Generator`](ew_ids::V7Generator) over a seeded [`SynthSource`] (SplitMix64 bytes, a counter clock from
//! 2026-01-01T00:00:00Z; no test-only feature of `ew-ids` is needed) mints them in a fixed order, family by
//! family in plan index order. Nothing iterates a hash map and nothing reads the time, so a (class, seed)
//! pair yields identical records on every platform. [`GENERATOR_VERSION`] is bumped whenever that output
//! changes; it is part of the perf-gate baseline key.
//!
//! # Loading
//!
//! ```ignore
//! let g = ew_testkit::synth::project(SizeClass::S, 1);
//! let session = Session::from_records(g.project, g.records, g.lib, SessionOptions::default())?;
//! ```
//!
//! or [`model`] for the validated `Model` snapshot.
//!
//! # Counts per class
//!
//! `Devices` counts every device record: terminals, strips, cables and PLC modules are devices. `CPs` are the
//! instantiated connection points (the CP count of each function's definition, summed over all functions) and
//! stay below the class bound (S 3 750, M 37 500, L 175 000, XL 625 000), the other columns hit their bound
//! exactly. `Records` is `records.len() + 1`, the project row included.
//!
//! | Class | Pages | Devices | Functions | Connections | CPs | Terminals | Channels | Representations | Junctions | Records |
//! |---|---|---|---|---|---|---|---|---|---|---|
//! | S | 50 | 300 | 900 | 1 500 | 3 637 | 120 | 60 | 900 | 348 | 4 024 |
//! | M | 300 | 2 500 | 7 500 | 15 000 | 37 178 | 1 000 | 500 | 7 500 | 2 960 | 35 884 |
//! | L | 1 500 | 12 000 | 36 000 | 70 000 | 171 330 | 4 800 | 2 400 | 36 000 | 14 226 | 170 319 |
//! | XL | 5 000 | 50 000 | 150 000 | 250 000 | 603 482 | 20 000 | 10 000 | 150 000 | 59 438 | 666 389 |
//!
//! The S and M rows are asserted by `tests/synth.rs`; the L and XL rows come from the plan (their fixtures are
//! generated with the performance spike, see the ignored test there; the plan-only checks of L and XL run
//! always). The counts are the same in generator versions 1 and 2.
//!
//! Signal wires and routes (version 2; the rest of the connections are rails and bus cores). The routed share
//! is the quarter of every page plus the wires at load main functions, so it depends on how many spare load
//! CPs a class has:
//!
//! | Class | Signal wires | Routed | Routed share |
//! |---|---|---|---|
//! | S | 686 | 369 | 54 % |
//! | M | 8 364 | 7 277 | 87 % |
//! | L | 37 963 | 30 736 | 81 % |
//! | XL | 119 181 | 38 198 | 32 % |

pub mod topology;

mod emit;
mod ids;

pub use ids::SynthSource;
pub use topology::{Bounds, Counts, Plan, SizeClass};

/// Bumped whenever the output for a (class, seed) changes; part of the perf-gate baseline key (18 §4.4).
///
/// Version 2: signal wires never join one device with itself, channel-terminal-load chains, routes on a
/// quarter of the signal wires and on those at load main functions, one symbol with CPs per function
/// definition, terminals short their two sides.
pub const GENERATOR_VERSION: u32 = 2;

/// Generation options. [`Options::default`] is what [`project`] and [`emit`] use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Options {
    /// Draw the routed signal wires with a route (default); `false` leaves every route empty.
    pub routes: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { routes: true }
    }
}

/// A generated project: format-level records, ready for `Session::from_records` or `Model::from_records`.
#[derive(Clone, Debug)]
pub struct Generated {
    /// The size class.
    pub class: SizeClass,
    /// The seed the IDs were minted from.
    pub seed: u64,
    /// The project row.
    pub project: ew_schema::Project,
    /// Every record except the project row, in the order of the golden files: structure nodes, sections,
    /// pages, devices, functions, representations, junctions, connections, conductors, potential
    /// definitions.
    pub records: Vec<ew_schema::Record>,
    /// The embedded library copies: the form, the symbol and one function definition per plan entry.
    pub lib: Vec<ew_schema::LibDocument>,
    /// The counts of the plan (equal to [`Plan::counts`]).
    pub counts: Counts,
    /// Handles for benchmarks and tests.
    pub handles: Handles,
}

/// IDs and values that benchmarks and tests need to address parts of a generated project.
#[derive(Clone, Debug)]
pub struct Handles {
    /// The pages, in plan page order.
    pub pages: Vec<ew_ids::Id<ew_schema::Page>>,
    /// The main representation of every load, in `Plan::load_reps` order: targets for moves.
    pub load_reps: Vec<ew_ids::Id<ew_schema::Representation>>,
    /// Per page, the signal CPs no connection uses (`Plan::free_cps` order): targets for new wires.
    pub free_cps: Vec<Vec<(ew_ids::Id<ew_schema::Function>, u16)>>,
    /// The connection grid of the pages.
    pub grid: ew_units::Grid,
}

impl Generated {
    /// The project row, the records and the library documents, as `Session::from_records` takes them.
    pub fn into_parts(
        self,
    ) -> (
        ew_schema::Project,
        Vec<ew_schema::Record>,
        Vec<ew_schema::LibDocument>,
    ) {
        (self.project, self.records, self.lib)
    }
}

/// Builds the plan of `class` and emits it with IDs minted from `seed`.
pub fn project(class: SizeClass, seed: u64) -> Generated {
    project_with(class, seed, &Options::default())
}

/// [`project`] with explicit [`Options`].
pub fn project_with(class: SizeClass, seed: u64, options: &Options) -> Generated {
    emit_with(&Plan::build(class), seed, options)
}

/// Emits an already built plan (for callers that hold one) with the default [`Options`].
pub fn emit(plan: &Plan, seed: u64) -> Generated {
    emit_with(plan, seed, &Options::default())
}

/// [`emit`] with explicit [`Options`].
pub fn emit_with(plan: &Plan, seed: u64, options: &Options) -> Generated {
    emit::emit(plan, seed, options)
}

/// The validated model snapshot of [`project`].
///
/// # Panics
///
/// If the generated records do not form a valid model (a generator bug).
pub fn model(class: SizeClass, seed: u64) -> ew_model::Model {
    let (project, records, lib) = project(class, seed).into_parts();
    ew_model::Model::from_records(project, records, lib).expect("the generated project is a valid model")
}
