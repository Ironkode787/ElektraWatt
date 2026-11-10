// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-derive`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;
// The golden fixture of `golden` (it imports it as `golden`; the name is taken by the module `golden`).
#[path = "../common/golden.rs"]
mod golden_fixture;

// Every other file of tests/it is a module (automod 1.0, sorted by name).
automod::dir!("tests/it");
