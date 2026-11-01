// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-api`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;
#[path = "../golden_common/mod.rs"]
mod golden_common;
#[path = "../qet_synth/mod.rs"]
mod qet_synth;
#[path = "../page_output/view.rs"]
#[allow(dead_code)]
mod view;

// Every other file of tests/it is a module (automod 1.0, sorted by name).
automod::dir!("tests/it");
