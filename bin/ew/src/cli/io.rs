// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a run reads and writes besides its arguments and the files it names: the three streams and the environment
//! ([`Io`]), and the lines a run prints on them ([`outln!`], [`errln!`], [`Io::error`] and its kin).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;

/// The streams and the environment of one run of the command line.
pub struct Io<'a> {
    /// The standard input: the typed word of a human-only subcommand, `git setup`'s answer.
    pub input: &'a mut dyn std::io::BufRead,
    /// The standard output: the result.
    pub out: &'a mut dyn std::io::Write,
    /// The standard error: the refusals, warnings and notices.
    pub err: &'a mut dyn std::io::Write,
    /// The environment.
    pub env: &'a Env,
}

/// The environment a run reads: its variables, its current directory and whether it talks to a person.
pub struct Env(Source);

enum Source {
    /// The process's own.
    Process,
    /// Given: the variables by name, the current directory; never a terminal.
    Given {
        vars: BTreeMap<String, OsString>,
        cwd: PathBuf,
    },
}

impl Env {
    /// The process's environment: its variables, its current directory, and a terminal when standard input and
    /// output both are one.
    pub fn process() -> Env {
        Env(Source::Process)
    }

    /// An environment with no variable, the current directory `cwd` and no terminal.
    pub fn new(cwd: PathBuf) -> Env {
        Env(Source::Given {
            vars: BTreeMap::new(),
            cwd,
        })
    }

    /// This environment with the variable `name` set to `value` (no effect on the process's own).
    #[must_use]
    pub fn with(mut self, name: &str, value: impl Into<OsString>) -> Env {
        if let Source::Given { vars, .. } = &mut self.0 {
            vars.insert(name.to_owned(), value.into());
        }
        self
    }

    /// The variable `name`, `None` when it is unset or empty.
    // The program boundary: every variable a run reads is read here.
    #[allow(
        clippy::disallowed_methods,
        reason = "the environment is read at the program boundary"
    )]
    pub(crate) fn var(&self, name: &str) -> Option<OsString> {
        match &self.0 {
            Source::Process => std::env::var_os(name),
            Source::Given { vars, .. } => vars.get(name).cloned(),
        }
        .filter(|v| !v.is_empty())
    }

    /// The current directory, as [`std::env::current_dir`] gives it.
    pub(crate) fn cwd(&self) -> std::io::Result<PathBuf> {
        match &self.0 {
            Source::Process => std::env::current_dir(),
            Source::Given { cwd, .. } => Ok(cwd.clone()),
        }
    }

    /// Whether `ew` talks to a person: standard input and output are both a terminal. A human-only subcommand asks
    /// its typed word only then, and `git setup` its question.
    pub(crate) fn is_terminal(&self) -> bool {
        use std::io::IsTerminal;
        matches!(self.0, Source::Process) && std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
    }

    /// The store options of a writer or reader of version `app_version`, the state directory (the lock's and the
    /// journal's) this environment names.
    pub(crate) fn store(&self, app_version: &str) -> ew_api::StoreOptions {
        let mut store = ew_api::StoreOptions::new(app_version);
        store.state_dir = ew_api::state_dir_in(&|n| self.var(n));
        store
    }

    /// The library tiers of this user under the data and cache directories this environment names, with the system
    /// and company libraries given; `None` when it names neither directory.
    pub(crate) fn tiers(
        &self,
        system: Vec<PathBuf>,
        company: Vec<PathBuf>,
    ) -> Option<ew_api::library::LibraryTiers> {
        let var = |n: &str| self.var(n);
        Some(ew_api::library::LibraryTiers::under(
            &ew_api::library::data_dir_in(&var)?,
            &ew_api::library::cache_dir_in(&var)?,
            system,
            company,
        ))
    }
}

/// Writes `args` on `to`, the stream `name`; a stream that is gone ends the run as `println!` does.
pub(crate) fn put(to: &mut dyn std::io::Write, name: &str, args: std::fmt::Arguments<'_>) {
    if let Err(e) = to.write_fmt(args) {
        panic!("failed printing to {name}: {e}");
    }
}

/// `print!` on the run's standard output: `out!(io, "…", …)`.
macro_rules! out {
    ($io:expr, $($arg:tt)*) => {
        $crate::cli::io::put(&mut *$io.out, "stdout", format_args!($($arg)*))
    };
}

/// `println!` on the run's standard output: `outln!(io, "…", …)`, `outln!(io)` for an empty line.
macro_rules! outln {
    ($io:expr) => {
        $crate::cli::io::put(&mut *$io.out, "stdout", format_args!("\n"))
    };
    ($io:expr, $($arg:tt)*) => {
        $crate::cli::io::put(&mut *$io.out, "stdout", format_args!("{}\n", format_args!($($arg)*)))
    };
}

/// `eprint!` on the run's standard error: `err!(io, "…", …)`.
macro_rules! err {
    ($io:expr, $($arg:tt)*) => {
        $crate::cli::io::put(&mut *$io.err, "stderr", format_args!($($arg)*))
    };
}

/// `eprintln!` on the run's standard error: `errln!(io, "…", …)`, `errln!(io)` for an empty line.
macro_rules! errln {
    ($io:expr) => {
        $crate::cli::io::put(&mut *$io.err, "stderr", format_args!("\n"))
    };
    ($io:expr, $($arg:tt)*) => {
        $crate::cli::io::put(&mut *$io.err, "stderr", format_args!("{}\n", format_args!($($arg)*)))
    };
}

pub(crate) use {err, errln, out, outln};

impl Io<'_> {
    /// Prints [`super::error_line`] on the standard error.
    pub(crate) fn error(&mut self, text: impl std::fmt::Display) {
        errln!(self, "{}", super::error_line(text));
    }

    /// Prints [`super::warning_line`] on the standard error.
    pub(crate) fn warning(&mut self, text: impl std::fmt::Display) {
        errln!(self, "{}", super::warning_line(text));
    }

    /// Prints [`super::notice_line`] on the standard error.
    pub(crate) fn notice(&mut self, text: impl std::fmt::Display) {
        errln!(self, "{}", super::notice_line(text));
    }
}

/// Runs `f` on an [`Io`] of buffers, the standard input `input` and the environment `env`: its value and what it
/// wrote on the standard output and the standard error (the unit tests).
#[cfg(test)]
pub(crate) fn captured<R>(input: &str, env: &Env, f: impl FnOnce(&mut Io<'_>) -> R) -> (R, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let value = f(&mut Io {
        input: &mut input.as_bytes(),
        out: &mut out,
        err: &mut err,
        env,
    });
    (
        value,
        String::from_utf8(out).expect("UTF-8 output"),
        String::from_utf8(err).expect("UTF-8 errors"),
    )
}
