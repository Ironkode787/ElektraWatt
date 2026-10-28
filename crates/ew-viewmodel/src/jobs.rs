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
use ew_api::output::ExportJob;

use crate::strings;

/// The most entries the Jobs pane keeps (the oldest finished one goes first).
pub const MAX_JOBS: usize = 20;

/// What a job does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JobKind {
    /// The PDF export (`file.export_pdf`).
    Pdf,
}

impl JobKind {
    /// The word in the describe text.
    pub fn word(self) -> &'static str {
        match self {
            JobKind::Pdf => "pdf",
        }
    }

    /// The key of the kind's label (`PDF export`) in [`crate::strings`].
    pub fn label_key(self) -> &'static str {
        match self {
            JobKind::Pdf => "job-kind-pdf",
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
        /// The pages of the PDF.
        pages: usize,
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
    fn gate(&self) -> Option<Gate> {
        self.gate.clone()
    }

    /// Lets the gate go: it would hold a cancelled job at its checkpoint.
    fn release(&mut self) {
        self.gate = None;
    }

    /// Panics when the test asked for a fault at `step`.
    fn fault(&self, step: &'static str) {
        assert!(self.panic_at != Some(step), "a test fault at {step}");
    }
}

#[cfg(not(feature = "testkit"))]
impl TestHooks {
    /// No gate.
    fn gate(&self) -> Option<Gate> {
        None
    }

    /// Nothing to let go.
    fn release(&mut self) {}

    /// No fault.
    fn fault(&self, _step: &'static str) {}
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
        let sibling = sibling_of(&target, &name);
        let options = JobOptions {
            name: JobKind::Pdf.word().to_owned(),
            waker: self.waker.clone(),
            gate: self.hooks.gate(),
        };
        let hooks = self.hooks.clone();
        let to = target.clone();
        let handle = Job::spawn(options, move |ctl| export_body(job, ctl, &to, &sibling, &hooks))?;
        self.next += 1;
        let number = self.next;
        self.entries.push_back(JobEntry {
            number,
            kind: JobKind::Pdf,
            name,
            rev,
            status: JobStatus::Running,
            epoch,
            path: target,
            handle: Some(handle),
            unseen_failure: None,
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
        return strings::text("job-cause-in-use").to_owned();
    }
    match e.kind() {
        std::io::ErrorKind::NotFound => strings::text("job-cause-no-folder").to_owned(),
        std::io::ErrorKind::PermissionDenied => strings::text("job-cause-denied").to_owned(),
        std::io::ErrorKind::StorageFull => strings::text("job-cause-full").to_owned(),
        kind => kind.to_string(),
    }
}

/// The export job's body (§6.3 step 3): the export, then the sibling written and synced, the checkpoint `rename`, the
/// rename over the target.
fn export_body(
    job: ExportJob,
    ctl: &JobCtl,
    target: &Path,
    sibling: &Path,
    hooks: &TestHooks,
) -> Result<ExportDone, JobError> {
    let export = job.run(ctl)?;
    let mut guard = Sibling {
        path: sibling,
        armed: true,
    };
    let failed = |e: std::io::Error| JobError::Failed(io_cause(&e));
    let mut file = std::fs::File::create(sibling).map_err(failed)?;
    file.write_all(&export.bytes).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    drop(file);
    // A test fault (feature `testkit` only): a panic with the sibling on disk.
    hooks.fault("sibling");
    ctl.checkpoint("rename")?;
    std::fs::rename(sibling, target).map_err(failed)?;
    guard.armed = false;
    Ok(ExportDone {
        pages: export.pages,
        bytes: export.bytes.len(),
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
