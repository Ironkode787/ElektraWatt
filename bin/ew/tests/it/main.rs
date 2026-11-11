// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew`, one test binary: each module is a former test file.

#[path = "../support/mod.rs"]
mod support;

/// The property grid pilot's command log, recorded by `bin/elektrawatt/tests/it/headless/grid_properties.rs`.
#[path = "../../../elektrawatt/tests/grid/pilot_log.rs"]
mod pilot_log;

/// The find-and-replace script's command log, recorded by `bin/elektrawatt/tests/it/headless/grid_find.rs`.
#[path = "../../../elektrawatt/tests/grid/find_log.rs"]
mod find_log;

// Every other file of tests/it is a module (automod 1.0, sorted by name).
automod::dir!("tests/it");
