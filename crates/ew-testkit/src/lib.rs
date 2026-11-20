// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The test kit: the golden builders of Golden A ([`golden`]: the CSV bootstrap and the command DSL) and a synthetic
//! parts library ([`library`]). A dev-only crate. Its tests hold the command fuzzer and the hostile corpus; the
//! seeded ID source of the tests is `ew_testids`, the fresh temporary directory `ew_testdir`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod golden;
pub mod library;
