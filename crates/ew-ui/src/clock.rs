// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's two clocks. Everything the window decides on time (the canvas's
//! dwells, the 200 ms job-field rule, the toasts, a double click) reads egui's frame time (`input.time`), which is the
//! wall clock in the window and the driver's frame count in a test or a visual run. The frame-phase statistics read
//! the wall clock through [`Clock::now`]: never on [`Clock::Frame`] (a visual run, which gives it to
//! [`crate::app::App::new`]), so a visual run's frame path reads no wall clock at all ([`wall_reads`] counts the reads
//! for the test that proves it).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The clock a window runs on, given to [`crate::app::App::new`]; the window's parts read it from the frame's
/// context ([`Clock::of`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Clock {
    /// The wall clock beside the frame time (the window, the tests).
    #[default]
    Wall,
    /// The frame clock only: the statistics read no wall clock and the date is [`FRAME_CLOCK_DATE`] (a visual run).
    Frame,
}

/// The wall-clock reads of the frame path so far.
static WALL_READS: AtomicU64 = AtomicU64::new(0);

/// A frame-phase time stamp: the wall clock's, or none on the frame clock.
#[derive(Clone, Copy, Debug)]
pub struct Stamp(Option<Instant>);

impl std::ops::Sub for Stamp {
    type Output = Duration;

    /// The time between two stamps; zero on the frame clock.
    fn sub(self, earlier: Stamp) -> Duration {
        match (self.0, earlier.0) {
            (Some(a), Some(b)) => a.saturating_duration_since(b),
            _ => Duration::ZERO,
        }
    }
}

impl Clock {
    /// The statistics' time stamp of now: the wall clock's, none on the frame clock.
    pub fn now(self) -> Stamp {
        match self {
            Clock::Frame => Stamp(None),
            Clock::Wall => {
                WALL_READS.fetch_add(1, Ordering::Relaxed);
                Stamp(Some(ew_render2d::stats::now()))
            }
        }
    }

    /// Whether this is the frame clock.
    pub fn is_frame(self) -> bool {
        self == Clock::Frame
    }

    /// The clock of the window that paints into `ctx` ([`Clock::publish`]); the wall clock before its first frame.
    pub(crate) fn of(ctx: &egui::Context) -> Clock {
        ctx.data(|d| d.get_temp(clock_id())).unwrap_or_default()
    }

    /// Gives the frames of `ctx` this clock (the window, at the start of each frame).
    pub(crate) fn publish(self, ctx: &egui::Context) {
        ctx.data_mut(|d| d.insert_temp(clock_id(), self));
    }

    /// The date a waiver records when it is committed from the window: today's date as `YYYY-MM-DD`, from the
    /// wall clock, by the proleptic Gregorian calendar in UTC (the workspace carries no time-zone data, so a waiver made
    /// near midnight may carry the neighbouring day). A run on the frame clock reads no wall clock and gives
    /// [`FRAME_CLOCK_DATE`], so a visual run is reproducible. Read on a click, never in the frame path; the shell never
    /// reads a clock: the window passes this into `messages.waive` and, before each action, into the shell
    /// (`ShellModel::set_today`, the PLC export form's date).
    pub fn today(self) -> String {
        if self.is_frame() {
            return FRAME_CLOCK_DATE.to_owned();
        }
        // The clock the lock and the journal use (`ew_store::SystemClock`), as the About dialog reads it.
        use ew_viewmodel::ew_api::Clock as _;
        let secs = ew_viewmodel::ew_api::SystemClock
            .now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let (year, month, day) =
            ew_viewmodel::ew_api::clock::civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0));
        format!("{year:04}-{month:02}-{day:02}")
    }
}

/// The id under which a frame's context holds its [`Clock`].
fn clock_id() -> egui::Id {
    egui::Id::unique("ew-clock")
}

/// The wall-clock reads of the frame path so far in this process.
pub fn wall_reads() -> u64 {
    WALL_READS.load(Ordering::Relaxed)
}

/// The date of a run on the frame clock.
pub const FRAME_CLOCK_DATE: &str = "2026-01-01";
