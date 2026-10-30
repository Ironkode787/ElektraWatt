// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's two clocks (WP-S6.13 part 4 §4.4, FT-570). Everything the window decides on time (the canvas's
//! dwells, the 200 ms job-field rule, the toasts, a double click) reads egui's frame time (`input.time`), which is the
//! wall clock in the window and the driver's frame count in a test or a visual run. The frame-phase statistics read
//! the wall clock through [`now`]: never in a run on the frame clock ([`use_frame_clock`]), so a visual run's frame
//! path reads no wall clock at all ([`wall_reads`] counts the reads for the test that proves it).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Whether this process runs on the frame clock only (a visual run).
static FRAME_CLOCK: AtomicBool = AtomicBool::new(false);

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

/// The statistics' time stamp of now: the wall clock in the window, none on the frame clock.
pub fn now() -> Stamp {
    if FRAME_CLOCK.load(Ordering::Relaxed) {
        Stamp(None)
    } else {
        WALL_READS.fetch_add(1, Ordering::Relaxed);
        Stamp(Some(ew_render2d::stats::now()))
    }
}

/// Puts this process on the frame clock: the statistics read no wall clock from now on (a visual run, for the rest
/// of the process).
pub fn use_frame_clock() {
    FRAME_CLOCK.store(true, Ordering::Relaxed);
}

/// Whether this process runs on the frame clock.
pub fn on_frame_clock() -> bool {
    FRAME_CLOCK.load(Ordering::Relaxed)
}

/// The wall-clock reads of the frame path so far in this process.
pub fn wall_reads() -> u64 {
    WALL_READS.load(Ordering::Relaxed)
}

/// The date a waiver records when it is committed from the window (FT-674): today's date as `YYYY-MM-DD`, from the
/// wall clock, by the proleptic Gregorian calendar in UTC (the workspace carries no time-zone data, so a waiver made
/// near midnight may carry the neighbouring day). A run on the frame clock reads no wall clock and gives
/// [`FRAME_CLOCK_DATE`], so a visual run is reproducible. Read on a click, never in the frame path; the shell never
/// reads a clock: the window passes this into `messages.waive`.
pub fn today() -> String {
    if on_frame_clock() {
        return FRAME_CLOCK_DATE.to_owned();
    }
    // The clock the lock and the journal use (`ew_store::SystemClock`), as the About dialog reads it.
    use ew_viewmodel::ew_api::Clock as _;
    let secs = ew_viewmodel::ew_api::SystemClock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date(secs / 86_400)
}

/// The date of a run on the frame clock.
pub const FRAME_CLOCK_DATE: &str = "2026-01-01";

/// `YYYY-MM-DD` of the day `days` after 1970-01-01 (the `civil_from_days` algorithm of H. Hinnant).
fn civil_date(days: u64) -> String {
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_of_known_days() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(59), "1970-03-01");
        assert_eq!(civil_date(11_016), "2000-02-29");
        assert_eq!(civil_date(20_756), "2026-10-30");
        assert_eq!(civil_date(20_819), "2027-01-01");
    }
}
