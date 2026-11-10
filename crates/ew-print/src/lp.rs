// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CUPS adapter: `lp [-d <printer>] -t <title> <file>`, spawned with arguments.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::{PrintAdapter, PrintError, PrintJob, PrintOutcome, Program, fresh_file};

/// The CUPS adapter: the set written to a fresh file `ew-print-<process>-<n>.pdf` in [`Lp::temp_dir`], `lp` run on it, the file removed after
/// `lp` returns, whatever it returns.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Lp {
    /// The program run (`lp`).
    pub program: Program,
    /// Where the print file is written (the system's temporary directory).
    pub temp_dir: PathBuf,
}

impl Lp {
    /// The adapter running `lp` from the `PATH`, its print file in `temp_dir`.
    pub fn new(temp_dir: PathBuf) -> Lp {
        Lp {
            program: Program::named("lp"),
            temp_dir,
        }
    }
}

impl PrintAdapter for Lp {
    fn name(&self) -> &'static str {
        "lp"
    }

    fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError> {
        let file = fresh_file(&self.temp_dir, "ew-print", &job.pdf)?;
        let mut args: Vec<OsString> = Vec::new();
        if let Some(p) = &job.printer {
            args.extend(["-d".into(), p.into()]);
        }
        args.extend([
            "-t".into(),
            job.title.as_str().into(),
            file.clone().into_os_string(),
        ]);
        let ran = self.program.run(&args, Some(PrintError::LpMissing));
        let _ = std::fs::remove_file(&file);
        let message = ran?;
        Ok(PrintOutcome::Spooled {
            printer: job.printer.clone(),
            message,
        })
    }
}
