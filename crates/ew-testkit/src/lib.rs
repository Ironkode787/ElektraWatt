// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The test kit, a crate only tests depend on: the builder DSL that speaks the commands' JSON parameters ([`dsl`]),
//! the builders of the reference project over it ([`golden`]: the CSV bootstrap and its translation into commands)
//! and a synthetic parts library ([`library`]). Its own tests hold the command fuzzer and the
//! hostile corpus (`hostile/`).
//!
//! Not here: the seeded ID source of the tests is `ew-testids`, the temporary directory of a test `ew-testdir`.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod dsl;
pub mod golden;
pub mod library;
