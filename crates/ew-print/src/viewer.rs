// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The viewer adapter (WP-S3.07 D9): the set written to `<dir>/<project>-print.pdf` and opened in the default PDF
//! application, which prints it.

use std::path::PathBuf;

use crate::{PrintAdapter, PrintError, PrintJob, PrintOutcome, Program, write_atomically};

/// The viewer adapter: the set written to [`Viewer::file`] and opened with [`Viewer::opener`]; a printer is asked
/// for by the viewer, so naming one is refused.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Viewer {
    /// The program that opens the file ([`Program::opener`]).
    pub opener: Program,
    /// Where the file is written (the system's temporary directory).
    pub dir: PathBuf,
    /// The project's name, the file's stem before `-print.pdf`.
    pub project: String,
}

impl Viewer {
    /// The adapter opening `<dir>/<project>-print.pdf` with the platform's opener.
    pub fn new(dir: PathBuf, project: &str) -> Viewer {
        Viewer {
            opener: Program::opener(),
            dir,
            project: project.to_owned(),
        }
    }

    /// The file the set is written to: `<dir>/<project>-print.pdf`, the name's characters other than letters,
    /// digits, `-`, `_` and `.` replaced by `_` (so the name never leaves the directory).
    pub fn file(&self) -> PathBuf {
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
        self.dir.join(format!("{stem}-print.pdf"))
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
        let file = self.file();
        write_atomically(&file, &job.pdf)?;
        self.opener.run(&[file.clone().into_os_string()], None)?;
        Ok(PrintOutcome::Opened { file })
    }
}
