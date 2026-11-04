// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell's jobs (WP-S2.09 part 1, D3 to D5, §6.3, §6.4): the PDF export as a job of `ew_jobs`, its entries in the
//! Jobs pane, and the file rule.
//!
//! **The file rule.** The job's body runs the export ([`ew_api::output::ExportJob::run`]) and then writes the bytes to
//! the hidden sibling `.<file name>.ew-part` in the target's folder, syncs it and renames it over the target. A guard
//! removes the sibling on every other way out of the body, a cancel at the checkpoint before the rename, a failure,
//! a panic (the guard drops while the panic unwinds), so no partial file remains; after the rename the export is done.
//! The cause of a failure is a fixed English text by the I/O error's kind, never a path.
//!
//! **Determinism.** Nothing here prints a progress number, an elapsed time or a path; the window keeps its own clock
//! for the elapsed time and the 200 ms rule. The entries are numbered from 1 per shell; the last [`MAX_JOBS`] are kept.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use ew_api::ew_jobs::{Gate, Job, JobCtl, JobEnd, JobError, JobOptions, Progress, Waker};
use ew_api::output::{CsvOptions, ExportJob, Table, WorkbookMeta};

use crate::key;
use crate::strings;
use crate::strings::Key;

/// The most entries the Jobs pane keeps (the oldest finished one goes first).
pub const MAX_JOBS: usize = 20;

/// What a job does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JobKind {
    /// The PDF export (`file.export_pdf`).
    Pdf,
    /// The print set of the print pre-dialog (`file.print`, WP-S3.07 ruling 2 of ID-1180): it writes no file, and
    /// the dialog shows its progress in the preview's place rather than the Jobs pane.
    Print,
    /// A navigator's table as XLSX or CSV (`navigator.export_table`, WP-S3.08 ruling 4 of ID-1224).
    Table,
}

impl JobKind {
    /// The word in the describe text.
    pub fn word(self) -> &'static str {
        match self {
            JobKind::Pdf => "pdf",
            JobKind::Print => "print",
            JobKind::Table => "table",
        }
    }

    /// The key of the kind's label (`PDF export`) in [`crate::strings`].
    pub fn label_key(self) -> Key<'static> {
        match self {
            JobKind::Pdf => key!("job-kind-pdf"),
            JobKind::Print => key!("job-kind-print"),
            JobKind::Table => key!("job-kind-table"),
        }
    }
}

/// Where a job stands.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum JobStatus {
    /// Its thread runs.
    Running,
    /// The file is written.
    Done {
        /// The pages of the PDF; `None` for a table, which has none.
        pages: Option<usize>,
        /// Its size in KiB, rounded up.
        kib: usize,
        /// The model's revision when the end arrived, when it had moved from the job's (`now <m>`).
        now: Option<u64>,
    },
    /// It met its cancel flag; no file was written.
    Cancelled,
    /// It failed, with the cause (no path); a panic reads "an internal error".
    Failed(String),
}

/// What the export job's body hands back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExportDone {
    /// The pages of the PDF.
    pub pages: usize,
    /// The bytes written.
    pub bytes: usize,
}

/// One entry of the Jobs pane.
#[derive(Debug)]
pub struct JobEntry {
    /// Counted from 1 per shell.
    pub number: u64,
    /// What it does.
    pub kind: JobKind,
    /// The file's base name.
    pub name: String,
    /// The revision it runs on (`as of rev <r>`).
    pub rev: u64,
    /// Where it stands.
    pub status: JobStatus,
    /// The project session it was made in.
    pub(crate) epoch: u64,
    /// The target (never printed).
    pub(crate) path: PathBuf,
    /// The running job; `None` once its end has been taken.
    pub(crate) handle: Option<Job<ExportDone>>,
    /// The notification text of a failed, cancelled or panicked end that no step waited for (a `wait: false` export
    /// whose end was taken by polling or at the end of a headless run); cleared when a waiting step takes the end.
    /// The headless runner reports these on stderr (FT-513).
    pub(crate) unseen_failure: Option<String>,
    /// Whether a step waited for the job (a waiting `file.export_pdf` or `jobs.wait`), set for every outcome and
    /// never by a poll: `jobs.wait {}` picks the newest job no step waited for, so its choice does not depend on
    /// which poll took an end (R2-1 of cycle 31).
    pub(crate) waited: bool,
}

impl JobEntry {
    /// The last progress of a running job.
    pub fn progress(&self) -> Option<Progress> {
        self.handle.as_ref().map(Job::progress)
    }

    /// Whether it runs.
    pub fn is_running(&self) -> bool {
        self.status == JobStatus::Running
    }
}

/// The jobs of the shell.
#[derive(Default)]
pub struct JobsState {
    pub(crate) entries: VecDeque<JobEntry>,
    pub(crate) next: u64,
    /// The window's repaint request; `None` headless.
    pub(crate) waker: Option<Waker>,
    /// The test gate and fault of the jobs started from now on (empty without the feature `testkit`).
    pub(crate) hooks: TestHooks,
    /// The `file://` URL the window must hand to the system opener (Open PDF), taken once.
    pub(crate) open_request: Option<String>,
}

/// The test hooks of the export jobs (feature `testkit`): a gate and a fault. Without the feature the struct is empty,
/// no job gets a gate and the body checks no fault, so neither is compiled into the product.
#[derive(Clone, Default)]
pub(crate) struct TestHooks {
    /// The test gate (`ShellModel::set_job_gate_for_tests`).
    #[cfg(feature = "testkit")]
    pub(crate) gate: Option<Gate>,
    /// A test fault (`ShellModel::set_job_panic_for_tests`): the body panics at this step.
    #[cfg(feature = "testkit")]
    pub(crate) panic_at: Option<&'static str>,
}

#[cfg(feature = "testkit")]
impl TestHooks {
    /// The gate a new job gets.
    pub(crate) fn gate(&self) -> Option<Gate> {
        self.gate.clone()
    }

    /// Lets the gate go: it would hold a cancelled job at its checkpoint.
    fn release(&mut self) {
        self.gate = None;
    }

    /// Panics when the test asked for a fault at `step`.
    pub(crate) fn fault(&self, step: &'static str) {
        assert!(self.panic_at != Some(step), "a test fault at {step}");
    }
}

#[cfg(not(feature = "testkit"))]
impl TestHooks {
    /// No gate.
    pub(crate) fn gate(&self) -> Option<Gate> {
        None
    }

    /// Nothing to let go.
    fn release(&mut self) {}

    /// No fault.
    pub(crate) fn fault(&self, _step: &'static str) {}
}

impl std::fmt::Debug for JobsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobsState")
            .field("entries", &self.entries)
            .field("next", &self.next)
            .finish_non_exhaustive()
    }
}

impl JobsState {
    /// The entries, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &JobEntry> {
        self.entries.iter()
    }

    /// The entry numbered `number`.
    pub fn get(&self, number: u64) -> Option<&JobEntry> {
        self.entries.iter().find(|e| e.number == number)
    }

    pub(crate) fn get_mut(&mut self, number: u64) -> Option<&mut JobEntry> {
        self.entries.iter_mut().find(|e| e.number == number)
    }

    /// The number of running jobs.
    pub fn running(&self) -> usize {
        self.entries.iter().filter(|e| e.is_running()).count()
    }

    /// The newest running job.
    pub fn newest_running(&self) -> Option<&JobEntry> {
        self.entries.iter().rev().find(|e| e.is_running())
    }

    /// The numbers the pane holds, oldest first (the valid numbers of a refusal).
    pub(crate) fn numbers(&self) -> Vec<u64> {
        self.entries.iter().map(|e| e.number).collect()
    }

    /// Starts the export job `job` writing `target` (its base name `name`, revision `rev`, project session `epoch`);
    /// returns its number.
    pub(crate) fn start_export(
        &mut self,
        job: ExportJob,
        target: PathBuf,
        name: String,
        rev: u64,
        epoch: u64,
    ) -> std::io::Result<u64> {
        let produce = move |ctl: &JobCtl| job.run(ctl).map(|e| (e.bytes, e.pages));
        self.start(JobKind::Pdf, produce, target, name, rev, epoch)
    }

    /// Starts the table export job (WP-S3.08 ruling 4 of ID-1224): `file` encoded on the job's thread and written to
    /// `target` by the same file rule; returns its number.
    pub(crate) fn start_table(
        &mut self,
        file: TableFile,
        target: PathBuf,
        name: String,
        rev: u64,
        epoch: u64,
    ) -> std::io::Result<u64> {
        let produce = move |_: &JobCtl| file.encode().map(|bytes| (bytes, 0));
        self.start(JobKind::Table, produce, target, name, rev, epoch)
    }

    /// Starts a job of `kind` whose body produces the bytes (and pages) with `produce`, then writes them to `target`.
    fn start(
        &mut self,
        kind: JobKind,
        produce: impl FnOnce(&JobCtl) -> Result<(Vec<u8>, usize), JobError> + Send + 'static,
        target: PathBuf,
        name: String,
        rev: u64,
        epoch: u64,
    ) -> std::io::Result<u64> {
        let sibling = sibling_of(&target, &name);
        let options = JobOptions {
            name: kind.word().to_owned(),
            waker: self.waker.clone(),
            gate: self.hooks.gate(),
        };
        let hooks = self.hooks.clone();
        let to = target.clone();
        let handle = Job::spawn(options, move |ctl| {
            export_body(produce, ctl, &to, &sibling, &hooks)
        })?;
        self.next += 1;
        let number = self.next;
        self.entries.push_back(JobEntry {
            number,
            kind,
            name,
            rev,
            status: JobStatus::Running,
            epoch,
            path: target,
            handle: Some(handle),
            unseen_failure: None,
            waited: false,
        });
        // The oldest finished entries go; a running one stays.
        while self.entries.len() > MAX_JOBS {
            match self.entries.iter().position(|e| !e.is_running()) {
                Some(i) => {
                    self.entries.remove(i);
                }
                None => break,
            }
        }
        Ok(number)
    }

    /// Cancels every running job and waits for each to end (the shell's end: no job outlives its window or run, so
    /// no sibling is left by a killed thread). Returns each job's real end, for the shell to record: a job that had
    /// already written its file is done, not cancelled (ID-719).
    pub(crate) fn stop_all(&mut self) -> Vec<(u64, JobEnd<ExportDone>)> {
        // A test gate would hold a cancelled job at its checkpoint: it is let go.
        self.hooks.release();
        let handles: Vec<(u64, Job<ExportDone>)> = self
            .entries
            .iter_mut()
            .filter_map(|e| Some((e.number, e.handle.take()?)))
            .collect();
        for (_, h) in &handles {
            h.cancel();
        }
        handles
            .into_iter()
            .map(|(number, h)| (number, h.wait()))
            .collect()
    }
}

/// What a table export job encodes (WP-S3.08 ruling 4 of ID-1224): the restricted table as a workbook with its
/// metadata, or as CSV in a preset.
pub(crate) enum TableFile {
    /// A workbook (`ew_api::output::xlsx`).
    Xlsx {
        /// The table.
        table: Table,
        /// The workbook's metadata, with the base digest of the table.
        meta: WorkbookMeta,
    },
    /// A CSV (`ew_api::output::table_csv`).
    Csv {
        /// The table.
        table: Table,
        /// The preset and its options.
        options: CsvOptions,
    },
}

impl TableFile {
    /// The file's bytes; a workbook the writer refuses fails with the writer's sentence.
    fn encode(self) -> Result<Vec<u8>, JobError> {
        match self {
            TableFile::Xlsx { table, meta } => {
                let rev = meta.rev;
                ew_api::output::export_tables(
                    &ew_api::output::XlsxExporter { meta },
                    rev,
                    std::slice::from_ref(&table),
                )
                .map(|(bytes, _)| bytes)
                .map_err(|e| JobError::Failed(e.to_string()))
            }
            TableFile::Csv { table, options } => ew_api::output::export_tables(
                &ew_api::output::CsvExporter { options },
                0,
                std::slice::from_ref(&table),
            )
            .map(|(bytes, _)| bytes)
            .map_err(|e| JobError::Failed(e.to_string())),
        }
    }
}

/// The hidden sibling of `target` (whose base name is `name`): `.<name>.ew-part` in the same folder.
pub fn sibling_of(target: &Path, name: &str) -> PathBuf {
    target.with_file_name(format!(".{name}.ew-part"))
}

/// Removes the sibling on drop unless the rename has taken it (a cancel, a failure, a panic's unwinding).
struct Sibling<'a> {
    path: &'a Path,
    armed: bool,
}

impl Drop for Sibling<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(self.path);
        }
    }
}

/// The cause text of an I/O error: a fixed English text by its kind (a file in use on Windows has its own), never a
/// path.
pub(crate) fn io_cause(e: &std::io::Error) -> String {
    // ERROR_SHARING_VIOLATION: another program holds the file open (a PDF viewer).
    if cfg!(windows) && e.raw_os_error() == Some(32) {
        return strings::text(key!("job-cause-in-use")).to_owned();
    }
    match e.kind() {
        std::io::ErrorKind::NotFound => strings::text(key!("job-cause-no-folder")).to_owned(),
        std::io::ErrorKind::PermissionDenied => strings::text(key!("job-cause-denied")).to_owned(),
        std::io::ErrorKind::StorageFull => strings::text(key!("job-cause-full")).to_owned(),
        kind => kind.to_string(),
    }
}

/// The export job's body (§6.3 step 3): the export (`produce`: the bytes and the pages), then the sibling written and
/// synced, the checkpoint `rename`, the rename over the target.
fn export_body(
    produce: impl FnOnce(&JobCtl) -> Result<(Vec<u8>, usize), JobError>,
    ctl: &JobCtl,
    target: &Path,
    sibling: &Path,
    hooks: &TestHooks,
) -> Result<ExportDone, JobError> {
    let (bytes, pages) = produce(ctl)?;
    let mut guard = Sibling {
        path: sibling,
        armed: true,
    };
    let failed = |e: std::io::Error| JobError::Failed(io_cause(&e));
    let mut file = std::fs::File::create(sibling).map_err(failed)?;
    file.write_all(&bytes).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    drop(file);
    // A test fault (feature `testkit` only): a panic with the sibling on disk.
    hooks.fault("sibling");
    ctl.checkpoint("rename")?;
    std::fs::rename(sibling, target).map_err(failed)?;
    guard.armed = false;
    Ok(ExportDone {
        pages,
        bytes: bytes.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sibling_is_hidden_beside_the_target() {
        assert_eq!(
            sibling_of(Path::new("out/golden.pdf"), "golden.pdf"),
            Path::new("out/.golden.pdf.ew-part")
        );
        assert_eq!(
            sibling_of(Path::new("x.pdf"), "x.pdf"),
            Path::new(".x.pdf.ew-part")
        );
    }

    #[test]
    fn io_causes_name_no_path() {
        let e = std::io::Error::new(std::io::ErrorKind::NotFound, "C:\\secret\\x.pdf");
        assert_eq!(io_cause(&e), "the folder does not exist");
        let e = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "/home/x");
        assert_eq!(io_cause(&e), "access to the folder or the file was denied");
        let e = std::io::Error::other("/tmp/y");
        assert!(!io_cause(&e).contains('/'));
    }
}
