// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command-line program: its entry point, which parses the arguments, runs the subcommand and exits with
//! its code. Every subcommand (import, check, export, print, query, revision, the git merge driver and textconv,
//! the migration verifier and the rest) works on a project through `ew-api`'s `Session`.
//!
//! Not here: the argument tree and the subcommands are in `cli.rs` and one file per group under `cli/`; the table
//! of every subcommand path with its flags, exit codes and result schema in `catalogue.rs`.

#![forbid(unsafe_code)]

// The CLI table: read by its tests today, by the CLI snapshot and reference from part 5.
#[cfg_attr(not(test), allow(dead_code))]
mod catalogue;
mod cli;

fn main() {
    // A usage error prints clap's rendering on stderr and exits 2 (129 for `merge-driver`, which git runs); `--help`
    // and `--version` print on stdout and exit 0.
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let code = match cli::parse(args.iter().cloned()) {
        Ok(cmd) => cli::run(cmd),
        Err(e) => {
            // Printing fails only when the stream is gone; the exit code still says what happened.
            let _ = e.print();
            cli::error_exit_code(&args, &e)
        }
    };
    std::process::exit(code);
}
