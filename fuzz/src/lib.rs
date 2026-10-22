// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cargo-fuzz crate of ElektraWatt (FT-297, FT-272, FT-304; doc 18 §3.6, §3.12; `fuzz/README.md`).
//!
//! Three targets (`fuzz_targets/`), each a thin `fuzz_target!` over a `check` function of this library, so that
//! a crash input replays in a plain test and the seed corpora are made with the same encoders:
//!
//! - [`loader`]: arbitrary bytes through the `ew-format` reader;
//! - [`open_path`]: the golden project with old-shape records, opened by `ew-store`;
//! - [`command`]: the seeded command generator of the nightly command fuzzer over the fuzzer's input.
//!
//! The crate is outside the cargo workspace and builds on the nightly toolchain of `rust-toolchain.toml`.
//! `unsafe`: none in this crate; the `unsafe` of cargo-fuzz's `fuzz_target!` macro lives in `libfuzzer-sys`.

#![forbid(unsafe_code)]

pub mod command;
pub mod golden;
pub mod loader;
pub mod open_path;

/// The modules of the command fuzzer's test (`crates/ew-testkit/tests/command_fuzz`), included as they are: the
/// generator, its arms, the oracles and the save-and-reload and import steps. The test's own `main.rs` is the
/// module root, so its `mod` declarations and the `super::` paths of the modules resolve the same way here as in
/// the test crate. Its `#[test]` functions are not compiled outside a test build.
#[allow(dead_code, clippy::all, unused_imports)]
#[path = "../../crates/ew-testkit/tests/command_fuzz/main.rs"]
pub mod command_fuzz;
