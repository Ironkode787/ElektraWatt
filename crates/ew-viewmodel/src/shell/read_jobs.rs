// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Imports whose reading runs as a job (FT-1763, doc 16 §8.3): an EDZ or a parts XML read for File ▸ Import…, a PLC
//! exchange file read and merged for its preview, a report spreadsheet made into a project. What needs no session (the
//! file read, parsed and, where the session's part of it was taken first, merged) runs on the job's thread and hands
//! back the rest of the import ([`Finish`]), which runs on the window's thread when the job ends: the dialog, the card
//! or the new project. Meanwhile the Jobs pane shows the read with Cancel, which takes effect when the parsing ends
//! (the job's checkpoint). A job of a project that closed meanwhile ends in nothing; a script and a headless run wait
//! for the read, so their outcome is the finish's.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ew_api::ew_jobs::{Job, JobCtl, JobEnd, JobError, JobOptions};

use super::{Outcome, ShellModel, failed};
use crate::jobs::JobKind;
use crate::key;
use crate::strings;

/// The rest of an import, run on the window's thread when its read ends.
pub(crate) type Finish = Box<dyn FnOnce(&mut ShellModel) -> Outcome + Send>;

/// A read running as a job, and what its end needs.
pub(crate) struct ReadJob {
    job: Job<Finish>,
    after: AfterRead,
}

/// What a read's end needs: its entry in the Jobs pane, the project session it was started in (`None`: none was
/// open) and the file's name.
struct AfterRead {
    number: u64,
    epoch: Option<u64>,
    name: String,
}

impl std::fmt::Debug for ReadJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadJob")
            .field("number", &self.after.number)
            .finish_non_exhaustive()
    }
}

impl ShellModel {
    /// Runs `read` (the file `name` read and parsed off the window's thread) as a job of the Jobs pane; its end runs
    /// the [`Finish`] it hands back (see the module documentation). A script and a headless run wait for it.
    pub(crate) fn start_read(
        &mut self,
        name: &str,
        read: impl FnOnce(&JobCtl) -> Result<Finish, JobError> + Send + 'static,
    ) -> Outcome {
        let options = JobOptions {
            name: JobKind::Read.word().to_owned(),
            waker: self.jobs.waker.clone(),
            gate: self.jobs.hooks.gate(),
        };
        let hooks = self.jobs.hooks.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let body = {
            // The entry's Cancel, read at each checkpoint (the Jobs pane sets it; the job's own flag is a script's).
            let cancel = Arc::clone(&cancel);
            move |ctl: &JobCtl| {
                let stop = |at: &'static str| -> Result<(), JobError> {
                    ctl.checkpoint(at)?;
                    if cancel.load(Ordering::Relaxed) {
                        return Err(JobError::Cancelled);
                    }
                    Ok(())
                };
                // A test fault (feature `testkit` only): the body panics, as a reader's panic would end it.
                hooks.fault("read");
                stop("read")?;
                let finish = read(ctl)?;
                stop("parsed")?;
                Ok(finish)
            }
        };
        let Ok(job) = Job::spawn(options, body) else {
            return failed(key!("error-import-read"), &[name]);
        };
        let (rev, epoch) = self
            .project
            .as_ref()
            .map_or((0, None), |p| (p.session.world().rev.0, Some(p.epoch)));
        let number = self.jobs.start_flagged(
            JobKind::Read,
            name.to_owned(),
            rev,
            epoch.unwrap_or_default(),
            cancel,
        );
        let after = AfterRead {
            number,
            epoch,
            name: name.to_owned(),
        };
        if self.mode == super::Mode::Headless || !self.is_person() {
            return self.finish_read(job.wait(), after);
        }
        self.import.reads.push(ReadJob { job, after });
        Outcome::Done
    }

    /// Takes the ends of the reads that have finished (the window every frame, through `poll_jobs`).
    pub(super) fn poll_reads(&mut self) {
        let mut i = 0;
        while i < self.import.reads.len() {
            match self.import.reads[i].job.try_end() {
                Some(end) => {
                    let pending = self.import.reads.remove(i);
                    let _ = self.finish_read(end, pending.after);
                }
                None => i += 1,
            }
        }
    }

    /// Waits for the running reads and takes their ends (the end of a headless run).
    pub(super) fn wait_reads(&mut self) {
        for ReadJob { job, after } in std::mem::take(&mut self.import.reads) {
            let _ = self.finish_read(job.wait(), after);
        }
    }

    /// The end of a read: its entry leaves the Jobs pane; a cancel or a panic is said in words; else the rest of the
    /// import runs, unless the project changed meanwhile.
    fn finish_read(&mut self, end: JobEnd<Finish>, pending: AfterRead) -> Outcome {
        self.jobs.entries.retain(|e| e.number != pending.number);
        if self.project.as_ref().map(|p| p.epoch) != pending.epoch {
            return Outcome::Done;
        }
        match end {
            JobEnd::Done(finish) => finish(self),
            JobEnd::Cancelled => {
                let message = strings::format(key!("error-import-cancelled"), &[&pending.name]);
                self.notify(message.clone(), false);
                Outcome::Failed { message }
            }
            JobEnd::Panicked | JobEnd::Failed(_) => failed(key!("error-import-panicked"), &[&pending.name]),
        }
    }
}
