// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command line as a library: [`run`] parses the arguments, runs the subcommand on the streams and the
//! environment it is given ([`Io`], [`Env`]) and returns its exit code. The program (`main.rs`) runs it on the
//! process's own; the tests of `bin/ew` run it on buffers and a test folder's environment, and start the program as a
//! process only for what a process is: its exit, the merge driver git runs, a piped standard input.
//!
//! Not here: the argument tree and the subcommands are in `cli.rs` and one file per group under `cli/`; the table
//! of every subcommand path with its flags, exit codes and result schema in `catalogue.rs`.

#![forbid(unsafe_code)]

// The CLI table: read by its tests today, by the CLI snapshot and reference from part 5.
#[cfg_attr(not(test), allow(dead_code))]
mod catalogue;
mod cli;

pub use cli::{Env, Io};

/// Runs the command line `args` (the words after the program name) on `io` and returns the exit code. A usage error
/// prints clap's rendering on `io.err` and returns 2 (129 for `merge-driver`, which git runs); `--help` and
/// `--version` print on `io.out` and return 0.
pub fn run(args: &[std::ffi::OsString], io: &mut Io<'_>) -> i32 {
    match cli::parse(args.iter().cloned()) {
        Ok(cmd) => cli::run(cmd, io),
        Err(e) => {
            cli::tree::usage(&e, io);
            cli::error_exit_code(args, &e)
        }
    }
}
