// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command-line interface: CLI, `ew mcp`, merge driver, textconv and migration verifier.

#![forbid(unsafe_code)]

fn main() {
    println!("ew {}", env!("CARGO_PKG_VERSION"));
}
