// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PDF export as a job (WP-S2.09 part 1, D4, §6.2; FT-251): a snapshot that crosses to the job's thread and runs
//! the export's three phases there with a cold `Q`, progress and checkpoints.

use std::sync::Arc;

use ew_derive::{Q, ReportPagesProvider};
use ew_jobs::{JobCtl, JobError};

use super::{ExportAssets, ExportOptions, Halt, OutputError, PdfExport, export_pages_hooked, font_bytes};
use crate::{Session, World};

/// An export of the session's current snapshot that runs on another thread ([`Session::export_job`]): the `World`,
/// the report engine and the asset bytes, nothing of the session itself. It is `Send`; the `Session` is not.
pub struct ExportJob {
    world: World,
    reports: Arc<dyn ReportPagesProvider>,
    assets: ExportAssets,
    options: ExportOptions,
}

impl ExportJob {
    /// Runs the export on the calling thread (the job's): builds a cold `Q` over the snapshot with the report engine,
    /// then the phases of [`super::export_pages_with`] with `ctl`'s reports and checkpoints, then the PDF writer
    /// (phase `writing`, indeterminate) and the checkpoint `written`. The bytes equal [`Session::export_pdf`]'s on the
    /// revision the job was made on, whatever happened to the session since. Writes no file.
    ///
    /// # Errors
    ///
    /// [`JobError::Cancelled`] at a checkpoint after [`ew_jobs::Job::cancel`]; [`JobError::Failed`] with the
    /// [`OutputError`]'s text (no path) for a selection the page order does not have or pages the writer refuses.
    pub fn run(self, ctl: &JobCtl) -> Result<PdfExport, JobError> {
        let q = Q::new(&self.world.model).with_report_pages(self.reports);
        let pages = export_pages_hooked(&q, &self.options, &self.assets, ctl).map_err(|h| match h {
            Halt::Cancelled => JobError::Cancelled,
            Halt::Output(e) => JobError::Failed(e.to_string()),
        })?;
        ctl.report(ew_jobs::Progress {
            done: 0,
            total: 0,
            phase: "writing",
        });
        let out = ew_export::pdf(&pages.pages, &pages.options, font_bytes(), None)
            .map_err(|e| JobError::Failed(OutputError::Pdf(e).to_string()))?;
        ctl.checkpoint("written")?;
        Ok(PdfExport {
            bytes: out.bytes,
            report: out.report,
            pages: pages.pages.len(),
            findings: pages.findings,
        })
    }
}

impl std::fmt::Debug for ExportJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportJob")
            .field("rev", &self.world.rev)
            .field("options", &self.options)
            .field("assets", &self.assets)
            .finish_non_exhaustive()
    }
}

/// The export job crosses to its thread (§4.2): checked at compile time.
const _: () = {
    const fn send<T: Send>() {}
    send::<ExportJob>();
};

impl Session {
    /// The export of the current snapshot as a job (WP-S2.09 part 1, D4): O(1) in the model (the snapshot and the
    /// report engine are shared), plus the asset map. Run it with [`ExportJob::run`] on a job's thread.
    pub fn export_job(&self, options: &ExportOptions) -> ExportJob {
        ExportJob {
            world: self.world(),
            reports: Arc::clone(&self.reports),
            assets: self.export_assets(),
            options: options.clone(),
        }
    }
}
