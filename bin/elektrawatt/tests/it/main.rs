// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `elektrawatt`, one test binary: each module is a former test file.

#[path = "../support/mod.rs"]
mod support;

// Every other file of tests/it is a module (automod 1.0, sorted by name).
automod::dir!("tests/it");
