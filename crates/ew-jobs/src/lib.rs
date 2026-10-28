// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The job runner (WP-S2.09 part 1, D3; decision log ID-692): one named `std` thread per job, a progress cell, a
//! cancel flag checked at checkpoints, a waker for the window, `catch_unwind` and a test gate.
//!
//! **What it is.** [`Job::spawn`] starts a thread named `ew-job-<name>` and returns at once; the body gets a
//! [`JobCtl`]. [`JobCtl::report`] stores the progress and calls the waker; [`JobCtl::checkpoint`] first waits while a
//! test [`Gate`] holds that checkpoint, then returns [`Cancelled`] when the job was cancelled. [`Job::cancel`] only
//! sets the flag: a body that never reaches a checkpoint is not interrupted (cooperative cancel, ARCH §2). When the
//! body returns, its end is stored, [`Job::is_finished`] turns true and the waker is called once; [`Job::try_end`]
//! hands the end over once, [`Job::wait`] blocks for it.
//!
//! **Panics.** The body runs under `catch_unwind`: a panic ends the job as [`JobEnd::Panicked`] and the spawning
//! thread goes on. The panic's message is not kept (it may hold project text, doc 18 §12.3). A body holds a snapshot
//! only, never the session, so a panic leaves the session untouched.
//!
//! **Not here** (FT-497): a pool, a scheduler, staleness adoption ("adopt if unchanged"); they wait for a second kind
//! of job and a measurement. The crate has no dependency.
//!
//! # Example
//!
//! ```
//! use ew_jobs::{Job, JobEnd, JobOptions, Progress};
//!
//! let options = JobOptions { name: "sum".into(), ..JobOptions::default() };
//! let job = Job::spawn(options, |ctl| {
//!     let mut sum = 0u32;
//!     for i in 1..=4u32 {
//!         ctl.checkpoint("step")?;
//!         sum += i;
//!         ctl.report(Progress { done: i, total: 4, phase: "adding" });
//!     }
//!     Ok(sum)
//! })
//! .unwrap();
//! assert!(matches!(job.wait(), JobEnd::Done(10)));
//! ```

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

/// The last reported progress of a job; `total` 0 means indeterminate (the phase text is all there is).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Progress {
    /// The units done.
    pub done: u32,
    /// The units in all; 0 when not known.
    pub total: u32,
    /// What the job is doing (`building`, `drawing`, `writing`): a fixed English word, never project text.
    pub phase: &'static str,
}

/// A job body met its cancel flag at a checkpoint.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cancelled;

/// What a job body returns when it does not complete.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum JobError {
    /// It was cancelled ([`JobCtl::checkpoint`] returned [`Cancelled`]).
    Cancelled,
    /// It failed; the text is the cause in English, with no file system path.
    Failed(String),
}

impl From<Cancelled> for JobError {
    fn from(_: Cancelled) -> JobError {
        JobError::Cancelled
    }
}

/// Called after each progress report and once at the end: the window's repaint request.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// How a job ended.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum JobEnd<T> {
    /// The body returned its result.
    Done(T),
    /// The body met the cancel flag.
    Cancelled,
    /// The body failed with this cause.
    Failed(String),
    /// The body panicked; the message is not kept.
    Panicked,
}

/// The options of [`Job::spawn`].
#[derive(Clone, Default)]
pub struct JobOptions {
    /// The job's name: its thread is `ew-job-<name>`.
    pub name: String,
    /// The waker; `None` wakes nobody.
    pub waker: Option<Waker>,
    /// The test gate; product code passes none.
    pub gate: Option<Gate>,
}

impl std::fmt::Debug for JobOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobOptions")
            .field("name", &self.name)
            .field("waker", &self.waker.is_some())
            .field("gate", &self.gate.is_some())
            .finish()
    }
}

/// Locks `m`, whatever a panicking holder left: every lock of this crate guards plain values that stay consistent.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The state of a [`Gate`].
#[derive(Default)]
struct GateState {
    /// The checkpoints that are held.
    held: Vec<&'static str>,
    /// How many jobs are blocked at each checkpoint now.
    blocked: BTreeMap<&'static str, u32>,
    /// Counted up by every [`Gate::release`]: a blocked job goes on when it changed.
    releases: u64,
}

/// A test hook (never passed by product code): a job reaching a held checkpoint blocks there until [`Gate::release`];
/// [`Gate::wait_reached`] blocks the caller until a job is blocked at a checkpoint.
///
/// A hold is a breakpoint: it stays until the gate is dropped, so every arrival at a held checkpoint blocks, and
/// each [`Gate::release`] lets the jobs blocked now go on past it once. That makes "after page 2" exact: hold
/// `page`, wait until reached, release, wait until reached again, cancel, release.
#[derive(Clone, Default)]
pub struct Gate {
    inner: Arc<(Mutex<GateState>, Condvar)>,
}

impl Gate {
    /// A gate that holds nothing.
    pub fn new() -> Gate {
        Gate::default()
    }

    /// Holds checkpoint `at`: from now on every job that reaches it blocks there until the next release.
    pub fn hold(&self, at: &'static str) {
        let (state, _) = &*self.inner;
        let mut s = lock(state);
        if !s.held.contains(&at) {
            s.held.push(at);
        }
    }

    /// Lets every job that is blocked at a held checkpoint now go on past it (the holds stay).
    pub fn release(&self) {
        let (state, cv) = &*self.inner;
        let mut s = lock(state);
        s.releases += 1;
        s.blocked.clear();
        cv.notify_all();
    }

    /// Blocks until a job is blocked at checkpoint `at`.
    pub fn wait_reached(&self, at: &'static str) {
        let (state, cv) = &*self.inner;
        let mut s = lock(state);
        while s.blocked.get(at).copied().unwrap_or(0) == 0 {
            s = cv.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Blocks the calling job at `at` while `at` is held, until the next release.
    fn pass(&self, at: &'static str) {
        let (state, cv) = &*self.inner;
        let mut s = lock(state);
        if !s.held.contains(&at) {
            return;
        }
        let seen = s.releases;
        *s.blocked.entry(at).or_insert(0) += 1;
        cv.notify_all();
        while s.releases == seen {
            s = cv.wait(s).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

impl std::fmt::Debug for Gate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = lock(&self.inner.0);
        f.debug_struct("Gate")
            .field("held", &s.held)
            .field("blocked", &s.blocked)
            .finish()
    }
}

/// What the job thread and the [`Job`] handle share.
struct Shared {
    cancel: AtomicBool,
    finished: AtomicBool,
    progress: Mutex<Progress>,
}

/// The body's handle on its job: progress, checkpoints.
pub struct JobCtl {
    shared: Arc<Shared>,
    waker: Option<Waker>,
    gate: Option<Gate>,
}

impl JobCtl {
    /// Stores `p` as the job's progress and calls the waker.
    pub fn report(&self, p: Progress) {
        *lock(&self.shared.progress) = p;
        if let Some(w) = &self.waker {
            w();
        }
    }

    /// A checkpoint: waits while a gate holds `at`, then returns [`Cancelled`] if the job was cancelled.
    ///
    /// # Errors
    ///
    /// [`Cancelled`] when [`Job::cancel`] was called.
    pub fn checkpoint(&self, at: &'static str) -> Result<(), Cancelled> {
        if let Some(g) = &self.gate {
            g.pass(at);
        }
        if self.shared.cancel.load(Ordering::Acquire) {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for JobCtl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobCtl")
            .field("cancelled", &self.shared.cancel.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// A running or finished job with a result of type `T`.
pub struct Job<T> {
    shared: Arc<Shared>,
    end: Arc<Mutex<Option<JobEnd<T>>>>,
    thread: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> Job<T> {
    /// Starts `body` on a thread of its own named `ew-job-<name>` and returns at once.
    ///
    /// # Errors
    ///
    /// The thread could not be started.
    pub fn spawn<F>(options: JobOptions, body: F) -> std::io::Result<Job<T>>
    where
        F: FnOnce(&JobCtl) -> Result<T, JobError> + Send + 'static,
    {
        let shared = Arc::new(Shared {
            cancel: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            progress: Mutex::new(Progress::default()),
        });
        let end: Arc<Mutex<Option<JobEnd<T>>>> = Arc::new(Mutex::new(None));
        let ctl = JobCtl {
            shared: Arc::clone(&shared),
            waker: options.waker.clone(),
            gate: options.gate,
        };
        let waker = options.waker;
        let slot = Arc::clone(&end);
        let thread = std::thread::Builder::new()
            .name(format!("ew-job-{}", options.name))
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| body(&ctl)));
                let outcome = match result {
                    Ok(Ok(t)) => JobEnd::Done(t),
                    Ok(Err(JobError::Cancelled)) => JobEnd::Cancelled,
                    Ok(Err(JobError::Failed(cause))) => JobEnd::Failed(cause),
                    // The payload goes unread: a panic message may hold project text (doc 18 §12.3).
                    Err(_) => JobEnd::Panicked,
                };
                *lock(&slot) = Some(outcome);
                // The end first, then the flag, then the wake-up: the frame the waker starts finds it.
                ctl.shared.finished.store(true, Ordering::Release);
                if let Some(w) = &waker {
                    w();
                }
            })?;
        Ok(Job {
            shared,
            end,
            thread: Some(thread),
        })
    }

    /// The last reported progress.
    pub fn progress(&self) -> Progress {
        *lock(&self.shared.progress)
    }

    /// Asks the job to stop: sets the flag that its next checkpoint reads. Nothing is interrupted.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Release);
    }

    /// Whether the body has returned (or panicked) and its end is stored.
    pub fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire)
    }

    /// The end, once, after the body returned; `None` before and after it was taken.
    pub fn try_end(&mut self) -> Option<JobEnd<T>> {
        if !self.is_finished() {
            return None;
        }
        let end = lock(&self.end).take();
        if end.is_some() {
            // The thread has stored its end; it only calls the waker after that. Its handle is let go.
            self.thread = None;
        }
        end
    }

    /// Blocks until the body returned and gives its end ([`JobEnd::Panicked`] also when the end was taken already by
    /// [`Job::try_end`], which leaves nothing to give).
    pub fn wait(mut self) -> JobEnd<T> {
        if let Some(thread) = self.thread.take() {
            // The thread catches the body's panic itself, so joining it cannot fail but for a panicking waker.
            let _ = thread.join();
        }
        lock(&self.end).take().unwrap_or(JobEnd::Panicked)
    }
}

impl<T> std::fmt::Debug for Job<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("progress", &*lock(&self.shared.progress))
            .field("cancelled", &self.shared.cancel.load(Ordering::Relaxed))
            .field("finished", &self.shared.finished.load(Ordering::Relaxed))
            .finish()
    }
}

/// Compile-time checks of §4.2: a job handle crosses threads, and so does a gate.
const _: () = {
    const fn send<T: Send>() {}
    const fn send_sync<T: Send + Sync>() {}
    send::<Job<u8>>();
    send_sync::<Gate>();
    send_sync::<JobCtl>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    fn named(name: &str) -> JobOptions {
        JobOptions {
            name: name.to_owned(),
            ..JobOptions::default()
        }
    }

    fn counting_waker() -> (Waker, Arc<AtomicU32>) {
        let count = Arc::new(AtomicU32::new(0));
        let c = Arc::clone(&count);
        (
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
            }),
            count,
        )
    }

    /// Polls `job` until its end is there (bounded), the way the window's frames do.
    fn poll<T: Send + 'static>(job: &mut Job<T>) -> JobEnd<T> {
        for _ in 0..2000 {
            if let Some(end) = job.try_end() {
                return end;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the job did not end");
    }

    #[test]
    fn jb1_progress_and_the_end_arrive_through_try_end_once() {
        let gate = Gate::new();
        gate.hold("half");
        let (waker, wakes) = counting_waker();
        let mut job = Job::spawn(
            JobOptions {
                name: "t".into(),
                waker: Some(waker),
                gate: Some(gate.clone()),
            },
            |ctl| {
                ctl.report(Progress {
                    done: 1,
                    total: 2,
                    phase: "first",
                });
                ctl.checkpoint("half")?;
                ctl.report(Progress {
                    done: 2,
                    total: 2,
                    phase: "second",
                });
                Ok(7u8)
            },
        )
        .unwrap();
        gate.wait_reached("half");
        assert_eq!(
            job.progress(),
            Progress {
                done: 1,
                total: 2,
                phase: "first"
            }
        );
        assert!(!job.is_finished());
        assert_eq!(job.try_end(), None, "no end before the body returned");
        gate.release();
        assert_eq!(poll(&mut job), JobEnd::Done(7));
        assert!(job.is_finished());
        assert_eq!(job.try_end(), None, "the end is handed over once");
        assert_eq!(job.progress().done, 2);
        // The waker: once per report (2) and once at the end; the end's call follows the flag, so wait for it.
        for _ in 0..200 {
            if wakes.load(Ordering::SeqCst) == 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn jb1_a_cancel_at_a_held_gate_gives_cancelled() {
        let gate = Gate::new();
        gate.hold("page");
        let pages = Arc::new(AtomicU32::new(0));
        let seen = Arc::clone(&pages);
        let job = Job::spawn(
            JobOptions {
                name: "pages".into(),
                waker: None,
                gate: Some(gate.clone()),
            },
            move |ctl| {
                for _ in 0..10 {
                    seen.fetch_add(1, Ordering::SeqCst);
                    ctl.checkpoint("page")?;
                }
                Ok(())
            },
        )
        .unwrap();
        // After page 2: the first arrival is let past, the second is held.
        gate.wait_reached("page");
        gate.release();
        gate.wait_reached("page");
        assert_eq!(pages.load(Ordering::SeqCst), 2);
        job.cancel();
        assert!(!job.is_finished(), "cancel only sets the flag");
        gate.release();
        assert_eq!(job.wait(), JobEnd::Cancelled);
        assert_eq!(pages.load(Ordering::SeqCst), 2, "no page after the cancel");
    }

    #[test]
    fn jb1_a_panicking_body_is_reported_and_the_spawning_thread_goes_on() {
        let (waker, wakes) = counting_waker();
        let mut job: Job<u8> = Job::spawn(
            JobOptions {
                name: "boom".into(),
                waker: Some(waker),
                gate: None,
            },
            |_| panic!("project text that must not be kept"),
        )
        .unwrap();
        assert_eq!(poll(&mut job), JobEnd::Panicked);
        for _ in 0..200 {
            if wakes.load(Ordering::SeqCst) == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(wakes.load(Ordering::SeqCst), 1, "the end wakes once");
        // This thread is alive and can start another job.
        let again = Job::spawn(named("after"), |_| Ok(1u8)).unwrap();
        assert_eq!(again.wait(), JobEnd::Done(1));
    }

    #[test]
    fn jb1_wait_returns_the_end_and_failures_carry_their_cause() {
        let ok = Job::spawn(named("ok"), |_| Ok("x")).unwrap();
        assert_eq!(ok.wait(), JobEnd::Done("x"));
        let failed: Job<()> =
            Job::spawn(named("bad"), |_| Err(JobError::Failed("the disk is full".into()))).unwrap();
        assert_eq!(failed.wait(), JobEnd::Failed("the disk is full".into()));
        let cancelled: Job<()> = Job::spawn(named("c"), |_| Err(Cancelled.into())).unwrap();
        assert_eq!(cancelled.wait(), JobEnd::Cancelled);
    }

    #[test]
    fn jb1_the_thread_is_named_after_the_job() {
        let job = Job::spawn(named("pdf"), |_| {
            Ok(std::thread::current().name().map(str::to_owned))
        })
        .unwrap();
        assert_eq!(job.wait(), JobEnd::Done(Some("ew-job-pdf".to_owned())));
    }

    #[test]
    fn jb1_a_checkpoint_without_a_gate_or_a_hold_never_blocks() {
        let gate = Gate::new();
        gate.hold("other");
        let job = Job::spawn(
            JobOptions {
                name: "free".into(),
                waker: None,
                gate: Some(gate),
            },
            |ctl| {
                ctl.checkpoint("page")?;
                ctl.checkpoint("page")?;
                Ok(2u8)
            },
        )
        .unwrap();
        assert_eq!(job.wait(), JobEnd::Done(2));
    }

    #[test]
    fn jb1_a_job_handle_and_a_gate_cross_threads() {
        fn send<T: Send>(_: &T) {}
        fn send_sync<T: Send + Sync>(_: &T) {}
        let job = Job::spawn(named("s"), |_| Ok(0u8)).unwrap();
        send(&job);
        send_sync(&Gate::new());
        let moved = std::thread::spawn(move || job.wait()).join().unwrap();
        assert_eq!(moved, JobEnd::Done(0));
    }
}
