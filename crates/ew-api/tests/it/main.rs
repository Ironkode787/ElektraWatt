// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-api`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;
#[path = "../golden_common/mod.rs"]
mod golden_common;
#[path = "../qet_synth/mod.rs"]
mod qet_synth;
// The bless flag only chooses whether a committed file is rewritten; the compared result never depends on it.
#[allow(clippy::disallowed_methods)]
#[path = "../page_output/golden_files.rs"]
mod golden_files;
#[path = "../page_output/pdf_read.rs"]
mod pdf_read;
#[path = "../page_output/pdf_support.rs"]
mod pdf_support;
#[path = "../page_output/view.rs"]
mod view;

// Every other file of tests/it is a module (automod 1.0, sorted by name).
automod::dir!("tests/it");
