// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command-line interface: CLI, `ew mcp`, merge driver, textconv and migration verifier.

#![forbid(unsafe_code)]

// The CLI table (WP-S1.22 D4): read by its tests today, by the CLI snapshot and reference from part 5.
#[cfg_attr(not(test), allow(dead_code))]
mod catalogue;
mod cli;

fn main() {
    // A usage error prints clap's rendering on stderr and exits 2; `--help` and `--version` print on stdout and exit 0.
    let code = match cli::parse(std::env::args_os().skip(1)) {
        Ok(cmd) => cli::run(cmd),
        Err(e) => {
            // Printing fails only when the stream is gone; the exit code still says what happened.
            let _ = e.print();
            e.exit_code()
        }
    };
    std::process::exit(code);
}
