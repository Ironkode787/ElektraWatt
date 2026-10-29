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
