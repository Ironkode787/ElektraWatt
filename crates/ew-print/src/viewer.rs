// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer adapter: the set written to a fresh file `<dir>/<project>-print-<process>-<n>.pdf` and
//! opened in the default PDF application, which prints it.

use std::path::PathBuf;

use crate::{PrintAdapter, PrintError, PrintJob, PrintOutcome, Program, fresh_file};

/// The viewer adapter: the set written to a fresh file named by [`Viewer::stem`] and opened with [`Viewer::opener`];
/// a printer is asked for by the viewer, so naming one is refused. The file stays after the opener returns, for the
/// viewer to read; the system's temporary-file clean-up removes it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Viewer {
    /// The program that opens the file ([`Program::opener`]).
    pub opener: Program,
    /// Where the file is written (the system's temporary directory).
    pub dir: PathBuf,
    /// The project's name, the file's stem before `-print-<process>-<n>.pdf`.
    pub project: String,
}

impl Viewer {
    /// The adapter writing its files to `dir` and opening them with the platform's opener.
    pub fn new(dir: PathBuf, project: &str) -> Viewer {
        Viewer {
            opener: Program::opener(),
            dir,
            project: project.to_owned(),
        }
    }

    /// The stem of the files the sets are written to, `<project>-print`, the name's characters other than letters,
    /// digits, `-`, `_` and `.` replaced by `_` (so the name never leaves the directory); each set goes to a fresh
    /// file `<dir>/<stem>-<process>-<n>.pdf`.
    pub fn stem(&self) -> String {
        let stem: String = self
            .project
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let stem = stem.trim_start_matches('.');
        let stem = if stem.is_empty() { "project" } else { stem };
        format!("{stem}-print")
    }
}

impl PrintAdapter for Viewer {
    fn name(&self) -> &'static str {
        "viewer"
    }

    fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError> {
        if job.printer.is_some() {
            return Err(PrintError::PrinterWithViewer);
        }
        let file = fresh_file(&self.dir, &self.stem(), &job.pdf)?;
        self.opener.run(&[file.clone().into_os_string()], None)?;
        Ok(PrintOutcome::Opened { file })
    }
}
