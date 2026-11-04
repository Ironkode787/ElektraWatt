// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cargo-fuzz crate of ElektraWatt (FT-297, FT-272, FT-304; doc 18 §3.6, §3.12; `fuzz/README.md`).
//!
//! The targets (`fuzz_targets/`), each a thin `fuzz_target!` over a `check` function of this library, so that
//! a crash input replays in a plain test and the seed corpora are made with the same encoders:
//!
//! - [`loader`]: arbitrary bytes through the `ew-format` reader;
//! - [`open_path`]: the golden project with old-shape records, opened by `ew-store`;
//! - [`command`]: the seeded command generator of the nightly command fuzzer over the fuzzer's input;
//! - [`png_pdf`]: a PNG through `ew-render-cpu`'s `decode_image` and, when it accepts it, the PDF writer of `ew-export`;
//! - [`ewz_read`]: arbitrary bytes through `ew-store`'s strict `.ewz` reader (WP-S1.17 part 2);
//! - [`merge_file`]: three sides and a path through `ew-format`'s three-way merge (WP-S1.17 part 4b);
//! - [`sql_sandbox`]: SQL text through `ew-sqlview`'s sandbox over the golden project's projection (WP-S6.04, FT-223);
//! - [`svg_asset`]: bytes through the asset sniff and, as an SVG, `ew-scene`'s drawing path (WP-S6.04);
//! - [`image_decode`]: a PNG or a JPEG through `ew-render-cpu`'s `decode_image` (WP-S6.04);
//! - [`textconv`]: one project file through `ew_api::output::textconv`, with or without a session (WP-S6.04);
//! - [`table_round_trip`]: a round-trip file through `Session::table_import` (WP-S6.04);
//! - [`command_log`]: a `.ewcmd` through `Session::run_script` on the golden project (WP-S6.04).
//! - [`importers`]: the QET project and element importers, the golden tables and the parts table, through the import
//!   front door (WP-S3.09 D15).
//!
//! The crate is outside the cargo workspace and builds on the nightly toolchain of `rust-toolchain.toml`.
//! `unsafe`: none in this crate; the `unsafe` of cargo-fuzz's `fuzz_target!` macro lives in `libfuzzer-sys`.

#![forbid(unsafe_code)]

pub mod command;
pub mod command_log;
pub mod ewz_read;
pub mod golden;
pub mod image_decode;
pub mod importers;
pub mod loader;
pub mod merge_file;
pub mod open_path;
pub mod png_pdf;
pub mod sql_sandbox;
pub mod svg_asset;
pub mod table_round_trip;
pub mod textconv;

/// The modules of the command fuzzer's test (`crates/ew-testkit/tests/command_fuzz`), included as they are: the
/// generator, its arms, the oracles and the save-and-reload and import steps. The test's own `main.rs` is the
/// module root, so its `mod` declarations and the `super::` paths of the modules resolve the same way here as in
/// the test crate. Its `#[test]` functions are not compiled outside a test build.
#[allow(dead_code, clippy::all, unused_imports)]
#[path = "../../crates/ew-testkit/tests/command_fuzz/main.rs"]
pub mod command_fuzz;
