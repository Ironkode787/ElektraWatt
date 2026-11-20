// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The test kit, a crate only tests depend on: the builders of the reference project ([`golden`]: the CSV bootstrap
//! and the command DSL) and a synthetic parts library ([`library`]). Its own tests hold the command fuzzer and the
//! hostile corpus (`hostile/`).
//!
//! Not here: the seeded ID source of the tests is `ew-testids`, the temporary directory of a test `ew-testdir`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod golden;
pub mod library;
