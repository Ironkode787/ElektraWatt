// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The integration tests of `ew-import-dxf`, one test binary. Every drawing is written by the tests themselves
//! ([`common`]), so no foreign file enters the repository.

#[path = "../common/mod.rs"]
mod common;

automod::dir!("tests/it");
