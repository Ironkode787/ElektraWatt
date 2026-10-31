// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-import-tables`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;

mod assembly;
mod bus;
mod colour_modes;
mod connect;
mod golden;
mod hostile;
mod hostile_shapes;
mod layout;
mod memory;
mod negative;
mod parts;
mod system_library;
mod tables;
