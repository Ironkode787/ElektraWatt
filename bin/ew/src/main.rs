// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `ew` command-line program: its entry point, which runs the command line ([`ew::run`]) on the process's
//! arguments, streams and environment and exits with its code. Every subcommand (import, check, export, print,
//! query, revision, the git merge driver and textconv, the migration verifier and the rest) works on a project
//! through `ew-api`'s `Session`.

#![forbid(unsafe_code)]

fn main() {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let env = ew::Env::process();
    let code = ew::run(
        &args,
        &mut ew::Io {
            input: &mut std::io::stdin().lock(),
            out: &mut std::io::stdout().lock(),
            err: &mut std::io::stderr().lock(),
            env: &env,
        },
    );
    std::process::exit(code);
}
