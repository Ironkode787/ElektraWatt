// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-checks`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;

mod bus;
mod confidence;
mod core;
mod core_part3;
mod core_part4;
mod messages;
mod num;
mod plc;
mod pot;
mod pot_pack;
mod rds;
mod rel;
mod term;
