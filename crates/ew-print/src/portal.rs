// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Linux portal adapter (WP-S3.07 D11): xdg-desktop-portal Print through `ashpd`, the sandbox-safe print path
//! that also works in Flatpak. `PreparePrint` shows the system's print dialog, then `Print` hands the print set's file
//! descriptor to the portal with the dialog's token. The set is written to a fresh file first (as [`crate::Lp`]
//! writes it) and removed once the portal has answered, whatever it answered; the portal keeps its own descriptor.
//!
//! The calls run on the calling thread through the async-io executor of the graph's `zbus` (no tokio); the window
//! calls the adapter on a worker thread, so the print dialog never blocks the window.

use std::fs::File;
use std::os::fd::AsFd as _;
use std::path::PathBuf;

use ashpd::desktop::ResponseError;
use ashpd::desktop::print::{PageSetup, PreparePrintOptions, PrintOptions, PrintProxy, Settings};

use crate::{PrintAdapter, PrintError, PrintJob, PrintOutcome, fresh_file};

/// The portal adapter: the set written to a fresh file `ew-print-<process>-<n>.pdf` in [`Portal::temp_dir`], the
/// system's print dialog shown, the file handed over, the file removed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Portal {
    /// Where the print file is written (the system's temporary directory).
    pub temp_dir: PathBuf,
}

impl Portal {
    /// The adapter writing its print file to `temp_dir`.
    pub fn new(temp_dir: PathBuf) -> Portal {
        Portal { temp_dir }
    }
}

/// An `ashpd` error as a [`PrintError`]: the person's cancel is [`PrintError::Cancelled`], anything else
/// [`PrintError::Portal`] with its text.
fn portal_error(e: ashpd::Error) -> PrintError {
    match e {
        ashpd::Error::Response(ResponseError::Cancelled) => PrintError::Cancelled,
        other => PrintError::Portal {
            reason: other.to_string(),
        },
    }
}

impl PrintAdapter for Portal {
    fn name(&self) -> &'static str {
        "portal"
    }

    fn print(&self, job: &PrintJob) -> Result<PrintOutcome, PrintError> {
        let path = fresh_file(&self.temp_dir, "ew-print", &job.pdf)?;
        let ran = File::open(&path)
            .map_err(|e| PrintError::Io {
                path: path.clone(),
                reason: e.to_string(),
            })
            .and_then(|file| {
                ashpd::zbus::block_on(async {
                    let proxy = PrintProxy::new().await.map_err(portal_error)?;
                    let prepared = proxy
                        .prepare_print(
                            None,
                            &job.title,
                            Settings::default().set_use_color(!job.monochrome),
                            PageSetup::default(),
                            PreparePrintOptions::default().set_modal(true),
                        )
                        .await
                        .map_err(portal_error)?
                        .response()
                        .map_err(portal_error)?;
                    proxy
                        .print(
                            None,
                            &job.title,
                            &file.as_fd(),
                            PrintOptions::default()
                                .set_token(prepared.token)
                                .set_modal(true),
                        )
                        .await
                        .map_err(portal_error)?
                        .response()
                        .map_err(portal_error)
                })
            });
        let _ = std::fs::remove_file(&path);
        ran?;
        Ok(PrintOutcome::Spooled {
            printer: None,
            message: String::new(),
        })
    }
}
