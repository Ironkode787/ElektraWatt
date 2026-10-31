// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-derive`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;
// The golden fixture of `golden` and `value_bytes` (they import it as `golden`; the name is taken by the module
// `golden`).
#[path = "../common/golden.rs"]
mod golden_fixture;

mod agreement;
mod api;
mod bom;
mod bus;
mod cells;
mod connectivity;
mod connectivity_notes;
mod cps;
mod cycles;
mod golden;
mod items;
mod layout;
mod memo;
mod part_text;
mod plc;
mod plc_address_text;
mod plc_positions;
mod potential_conflicts;
mod structure;
mod supply_names;
mod tags;
mod title_block;
mod unwind;
mod value_bytes;
mod wire_chain;
mod wire_numbers;
mod xref;
