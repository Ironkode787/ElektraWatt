// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The integration tests of `ew-commands`, one test binary: each module is a former test file.

#[path = "../common/mod.rs"]
mod common;
// The JSON fixture builder of `ew-derive`'s tests, which `agreement` builds its models with.
#[path = "../../../ew-derive/tests/common/mod.rs"]
mod derive_fixture;

// Every other file of tests/it is a module (automod 1.0, sorted by name).
automod::dir!("tests/it");
