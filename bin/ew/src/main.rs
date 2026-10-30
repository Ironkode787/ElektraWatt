// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command-line interface: CLI, `ew mcp`, merge driver, textconv and migration verifier.

#![forbid(unsafe_code)]

// The CLI table (WP-S1.22 D4): read by its tests today, by the CLI snapshot and reference from part 5.
#[cfg_attr(not(test), allow(dead_code))]
mod catalogue;
mod cli;

fn main() {
    let code = match cli::parse(std::env::args().skip(1)) {
        Ok(cmd) => cli::run(cmd),
        Err(msg) => {
            eprintln!("error: {msg}\n{}", cli::USAGE);
            2
        }
    };
    std::process::exit(code);
}
