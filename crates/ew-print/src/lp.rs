// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CUPS adapter (WP-S3.07 D9): `lp [-d <printer>] -t <title> <file>`, spawned with arguments.

use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::{PrintAdapter, PrintError, PrintJob, PrintOutcome, Program};

/// The CUPS adapter: the set written to a fresh file in [`Lp::temp_dir`], `lp` run on it, the file removed after
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

/// The most attempts at a fresh print-file name before giving up.
const NAME_ATTEMPTS: u32 = 64;

/// A fresh file `ew-print-<process>-<n>.pdf` in `dir` holding `bytes`: created only if it did not exist, so no other
/// file is ever overwritten; removed again when the write fails.
fn fresh_file(dir: &Path, bytes: &[u8]) -> Result<PathBuf, PrintError> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let mut last = None;
    for _ in 0..NAME_ATTEMPTS {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("ew-print-{}-{n}.pdf", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        match file {
            Ok(mut f) => {
                return f
                    .write_all(bytes)
                    .and_then(|()| f.sync_all())
                    .map(|()| path.clone())
                    .map_err(|e| {
                        let _ = std::fs::remove_file(&path);
                        PrintError::Io {
                            path,
                            reason: e.to_string(),
                        }
                    });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some((path, e)),
            Err(e) => {
                return Err(PrintError::Io {
                    path,
                    reason: e.to_string(),
                });
            }
        }
    }
    let (path, e) = last.unwrap_or_else(|| (dir.to_path_buf(), std::io::Error::other("no free file name")));
    Err(PrintError::Io {
        path,
        reason: e.to_string(),
    })
}

impl PrintAdapter for Lp {
    fn name(&self) -> &'static str {
        "lp"
    }

    fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError> {
        let file = fresh_file(&self.temp_dir, &job.pdf)?;
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
