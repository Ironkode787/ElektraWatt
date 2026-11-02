// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! OS print adapters with per-OS features (WP-S3.07 D5, D9; ADR-0015 amendment 1).
//!
//! `ew-api` builds the print set (the PDF is the print master); the binaries hand its fields to an adapter here as a
//! [`PrintJob`], and the adapter hands it to the OS through [`PrintAdapter::print`]. Part 2's adapters are processes,
//! spawned with arguments and never through a shell:
//!
//! - [`Lp`] (Linux, CUPS): `lp [-d <printer>] -t <title> <file>`, the set written to a fresh file in the temporary
//!   directory first and removed after `lp` returns, whatever it returns;
//! - [`Viewer`] (every platform): the set written to a fresh file `<dir>/<project>-print-<process>-<n>.pdf` and opened
//!   with the default PDF application ([`Program::opener`]), so the person prints from it; it chooses no printer, and
//!   the file stays for the viewer to read (the system's temporary-file clean-up removes it).
//!
//! Both write their file only as a new one (`create_new`, mode `0o600` on Unix), never through a name already taken.
//!
//! Part 3 adds the Linux portal adapter (`Portal`, D11: xdg-desktop-portal Print through `ashpd`, Linux only) and the
//! [`Chain`] the window prints through (ID-1180 ruling 3): the adapters tried in order, a failure giving way to the
//! next with one [`Fallback`] notice; on Linux the portal, `lp`, then the viewer; elsewhere the viewer.
//!
//! Each adapter takes the program it runs as a [`Program`], so a test runs a stand-in that records its arguments.

#![forbid(unsafe_code)]

mod lp;
#[cfg(target_os = "linux")]
mod portal;
mod viewer;

use std::ffi::OsString;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

pub use ew_export::PdfPage;
pub use lp::Lp;
#[cfg(target_os = "linux")]
pub use portal::Portal;
pub use viewer::Viewer;

/// What an adapter prints: a print set's fields (`ew_api::output::print::PrintSet`), built by the binaries.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PrintJob {
    /// The job's title in the spooler (the project's name).
    pub title: String,
    /// The print set's PDF.
    pub pdf: Vec<u8>,
    /// The print set's pages (each with its placement), for the adapters that render (part 4).
    pub pages: Vec<PdfPage>,
    /// The printer; `None`: the default printer, or the one the dialog or viewer asks for.
    pub printer: Option<String>,
    /// Whether the set is drawn in black and grey.
    pub monochrome: bool,
}

/// What an adapter did with a job.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PrintOutcome {
    /// Handed to the spooler.
    Spooled {
        /// The printer named, `None` for the default.
        printer: Option<String>,
        /// What the spooler said on its standard output ("request id is office-42 (1 file(s))"), trimmed.
        message: String,
    },
    /// Written to `file` and opened in the default PDF application.
    Opened {
        /// The file.
        file: PathBuf,
    },
}

/// Why a job was not printed; the texts are the CLI's refusals (WP-S3.07 D9).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PrintError {
    /// The CUPS command `lp` is not installed.
    LpMissing,
    /// A printer was named for the viewer, which asks for one itself.
    PrinterWithViewer,
    /// The program could not be started.
    Spawn {
        /// The program.
        program: String,
        /// The system's reason.
        reason: String,
    },
    /// The program ran and failed.
    Failed {
        /// The program.
        program: String,
        /// Its exit status in words ("exit code 1").
        status: String,
        /// What it wrote on its error output, trimmed and at most [`MAX_MESSAGE`] characters.
        message: String,
    },
    /// The print file could not be written.
    Io {
        /// The file.
        path: PathBuf,
        /// The system's reason.
        reason: String,
    },
    /// The person cancelled the system's print dialog (the portal); nothing was printed and no other adapter is
    /// tried.
    Cancelled,
    /// The system's print dialog (the portal) could not be used.
    Portal {
        /// The portal's reason.
        reason: String,
    },
}

/// The most characters of a program's error output a [`PrintError::Failed`] keeps.
pub const MAX_MESSAGE: usize = 2000;

impl fmt::Display for PrintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrintError::LpMissing => write!(
                f,
                "printing needs the CUPS command `lp`; install cups-client or print from the window"
            ),
            PrintError::PrinterWithViewer => write!(
                f,
                "`--printer` needs `--via lp`; with the viewer, choose the printer in its print dialog"
            ),
            PrintError::Spawn { program, reason } => write!(f, "`{program}` could not be started: {reason}"),
            PrintError::Failed {
                program,
                status,
                message,
            } if message.is_empty() => write!(f, "`{program}` failed with {status}"),
            PrintError::Failed {
                program,
                status,
                message,
            } => write!(f, "`{program}` failed with {status}: {message}"),
            PrintError::Io { path, reason } => write!(f, "{}: {reason}", path.display()),
            PrintError::Cancelled => write!(f, "printing was cancelled"),
            PrintError::Portal { reason } => write!(f, "the system print dialog could not be used: {reason}"),
        }
    }
}

impl std::error::Error for PrintError {}

/// One way to print: the OS's spooler, its PDF viewer, or (part 3 and 4) a dialog.
pub trait PrintAdapter {
    /// The adapter's name as `--via` takes it (`lp`, `viewer`).
    fn name(&self) -> &'static str;

    /// Hands `job` to the OS.
    ///
    /// # Errors
    ///
    /// [`PrintError`] when the job cannot be handed over.
    fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError>;
}

/// The adapters tried one after the other (WP-S3.07 D11, ruling 3 of ID-1180): the window's way to print. A failure
/// gives way to the next adapter, and the job that prints after a failure carries one [`Fallback`] notice; the
/// person's cancel of a print dialog ([`PrintError::Cancelled`]) ends the chain, so nothing prints. A panic in an
/// adapter is a failure like any other.
pub struct Chain {
    adapters: Vec<Box<dyn PrintAdapter + Send>>,
}

/// The one notice of a job that printed after a failure: which adapters failed and why, in order, and the one that
/// printed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Fallback {
    /// The adapters that failed before, with their errors.
    pub failed: Vec<(&'static str, PrintError)>,
    /// The adapter that printed.
    pub used: &'static str,
}

/// What a [`Chain`] did with a job.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Printed {
    /// The adapter that printed.
    pub adapter: &'static str,
    /// What it did.
    pub outcome: PrintOutcome,
    /// The notice when an adapter before it failed.
    pub fallback: Option<Fallback>,
}

impl Chain {
    /// The chain of `adapters`, tried in their order.
    pub fn new(adapters: Vec<Box<dyn PrintAdapter + Send>>) -> Chain {
        Chain { adapters }
    }

    /// The window's Print… on this system (ID-1180 ruling 3): on Linux the portal, then `lp`, then the viewer; on
    /// other systems the viewer (until the Windows adapter, part 4). Print files go to `temp_dir`; `project` names the
    /// viewer's file.
    pub fn system(temp_dir: PathBuf, project: &str) -> Chain {
        #[cfg(target_os = "linux")]
        let adapters: Vec<Box<dyn PrintAdapter + Send>> = vec![
            Box::new(Portal::new(temp_dir.clone())),
            Box::new(Lp::new(temp_dir.clone())),
            Box::new(Viewer::new(temp_dir, project)),
        ];
        #[cfg(not(target_os = "linux"))]
        let adapters: Vec<Box<dyn PrintAdapter + Send>> = vec![Box::new(Viewer::new(temp_dir, project))];
        Chain::new(adapters)
    }

    /// The window's Open in PDF viewer: the viewer alone.
    pub fn viewer(temp_dir: PathBuf, project: &str) -> Chain {
        Chain::new(vec![Box::new(Viewer::new(temp_dir, project))])
    }

    /// The adapters' names, in order.
    pub fn names(&self) -> Vec<&'static str> {
        self.adapters.iter().map(|a| a.name()).collect()
    }

    /// Tries each adapter in turn until one prints `job`.
    ///
    /// # Errors
    ///
    /// The adapters' failures in order when none printed (an empty chain gives an empty list); a cancel ends the list
    /// with [`PrintError::Cancelled`].
    pub fn print(&self, job: &PrintJob) -> Result<Printed, Vec<(&'static str, PrintError)>> {
        let mut failed: Vec<(&'static str, PrintError)> = Vec::new();
        for adapter in &self.adapters {
            let name = adapter.name();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| adapter.print(job)))
                .unwrap_or_else(|_| {
                    Err(PrintError::Spawn {
                        program: name.to_owned(),
                        reason: "it stopped unexpectedly".to_owned(),
                    })
                });
            match result {
                Ok(outcome) => {
                    let fallback = (!failed.is_empty()).then(|| Fallback {
                        failed: std::mem::take(&mut failed),
                        used: name,
                    });
                    return Ok(Printed {
                        adapter: name,
                        outcome,
                        fallback,
                    });
                }
                Err(PrintError::Cancelled) => {
                    failed.push((name, PrintError::Cancelled));
                    return Err(failed);
                }
                Err(e) => failed.push((name, e)),
            }
        }
        Err(failed)
    }
}

impl fmt::Debug for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chain").field("adapters", &self.names()).finish()
    }
}

/// A program an adapter runs: its path and the arguments that come before the adapter's own.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Program {
    /// The program, a name looked up on the `PATH` or a path.
    pub path: PathBuf,
    /// Arguments before the adapter's.
    pub leading: Vec<OsString>,
    /// Whether a failing exit status is an error: `explorer.exe` exits 1 when it has opened the file.
    pub check_status: bool,
}

impl Program {
    /// The program `name` with no leading argument, its exit status checked.
    pub fn named(name: &str) -> Program {
        Program {
            path: PathBuf::from(name),
            leading: Vec::new(),
            check_status: true,
        }
    }

    /// The platform's opener of a file in its default application: `explorer.exe` on Windows (its exit status is
    /// not an answer), `open` on macOS, `xdg-open` elsewhere.
    pub fn opener() -> Program {
        if cfg!(windows) {
            Program {
                check_status: false,
                ..Program::named("explorer.exe")
            }
        } else if cfg!(target_os = "macos") {
            Program::named("open")
        } else {
            Program::named("xdg-open")
        }
    }

    /// The program's name for a message: its path as given.
    fn label(&self) -> String {
        self.path.display().to_string()
    }

    /// Runs the program with its leading arguments and `args`, no shell, no standard input; returns its standard
    /// output, or the error in words.
    fn run(&self, args: &[OsString], missing: Option<PrintError>) -> Result<String, PrintError> {
        let out = std::process::Command::new(&self.path)
            .args(&self.leading)
            .args(args)
            .stdin(std::process::Stdio::null())
            .output();
        let out = match out {
            Ok(o) => o,
            Err(e) => {
                return Err(match missing {
                    Some(m) if e.kind() == std::io::ErrorKind::NotFound => m,
                    _ => PrintError::Spawn {
                        program: self.label(),
                        reason: e.to_string(),
                    },
                });
            }
        };
        if self.check_status && !out.status.success() {
            return Err(PrintError::Failed {
                program: self.label(),
                status: match out.status.code() {
                    Some(c) => format!("exit code {c}"),
                    None => "no exit code".to_owned(),
                },
                message: bounded(&out.stderr),
            });
        }
        Ok(bounded(&out.stdout))
    }
}

/// A program's output as text: lossy UTF-8, trimmed, at most [`MAX_MESSAGE`] characters.
fn bounded(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim()
        .chars()
        .take(MAX_MESSAGE)
        .collect()
}

/// The most attempts at a fresh print-file name before giving up.
const NAME_ATTEMPTS: u32 = 64;

/// A fresh file `<stem>-<process>-<n>.pdf` in `dir` holding `bytes` (WP-S3.07 D9): created only if nothing is at
/// that name, so no other file or link is ever written through, with mode `0o600` on Unix; removed again when the
/// write fails. A name taken already moves on to the next `n`, at most [`NAME_ATTEMPTS`] times.
fn fresh_file(dir: &Path, stem: &str, bytes: &[u8]) -> Result<PathBuf, PrintError> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let mut last = None;
    for _ in 0..NAME_ATTEMPTS {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("{stem}-{}-{n}.pdf", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match options.open(&path) {
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

#[cfg(test)]
mod tests;
