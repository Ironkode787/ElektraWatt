// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frame statistics (WP-S2.02 §9): a ring of the last 1 024 frames with the time of each phase, the tiles
//! rastered and uploaded, and the memo counters' deltas, summarised as nearest-rank p50, p99 and maximum.
//!
//! The numbers are read against NFR-PERF-04 (frame ≤ 16 ms p99, render ≤ 5 ms) and NFR-PERF-06, reported and never
//! asserted (timings are runner-dependent; doc 18 §4.3 and the testing policy). [`now`] is the canvas's one clock
//! read; no time value reaches a transcript, a describe line or a file.

use std::time::{Duration, Instant};

/// The canvas's one clock read (WP-S2.02 §4.1 rule 7), listed in `ci/disallowed-methods-allow.toml`.
// Frame-phase timing only: the value feeds the statistics overlay and the `frame` tracing spans, never a persisted,
// exported or printed result.
#[allow(clippy::disallowed_methods)]
pub fn now() -> Instant {
    Instant::now()
}

/// The phases of a frame, in [`FrameSample::phases_ms`] order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Phase {
    /// Mapping the frame's input to actions and dispatching them.
    Input,
    /// Fetching or reusing the page's display list and its bounds.
    Scene,
    /// Tile planning, cache lookups and texture uploads.
    Render,
    /// Laying out the canvas pane and building the overlay shapes.
    Paint,
    /// The time since the previous frame.
    Interval,
}

impl Phase {
    /// Every phase, in sample order.
    pub const ALL: [Phase; 5] = [
        Phase::Input,
        Phase::Scene,
        Phase::Render,
        Phase::Paint,
        Phase::Interval,
    ];

    /// The phase's name (the suffix of its tracing span and bench family).
    pub fn name(self) -> &'static str {
        match self {
            Phase::Input => "input",
            Phase::Scene => "scene",
            Phase::Render => "render",
            Phase::Paint => "paint",
            Phase::Interval => "interval",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// One frame's numbers.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct FrameSample {
    /// Milliseconds per phase, in [`Phase::ALL`] order.
    pub phases_ms: [f64; 5],
    /// Tiles that finished rasterising and arrived this frame.
    pub tiles_rastered: u32,
    /// Tiles uploaded as textures this frame.
    pub tiles_uploaded: u32,
    /// Memo hits during the frame.
    pub memo_hits: u64,
    /// Memo re-executions during the frame.
    pub memo_re_executions: u64,
}

impl FrameSample {
    /// The time of `phase` in milliseconds.
    pub fn phase(&self, phase: Phase) -> f64 {
        self.phases_ms[phase.index()]
    }

    /// Sets the time of `phase`.
    pub fn set(&mut self, phase: Phase, d: Duration) {
        self.phases_ms[phase.index()] = d.as_secs_f64() * 1000.0;
    }

    /// The UI-thread time of the canvas in the frame: every phase but the interval.
    pub fn frame_ms(&self) -> f64 {
        Phase::ALL
            .iter()
            .filter(|p| **p != Phase::Interval)
            .map(|p| self.phase(*p))
            .sum()
    }
}

/// Nearest-rank p50, p99 and maximum.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Summary {
    /// The median.
    pub p50: f64,
    /// The 99th percentile.
    pub p99: f64,
    /// The largest value.
    pub max: f64,
}

/// The nearest-rank percentile `percent` of `sorted` (ascending): the value at rank `ceil(percent / 100 × n)`
/// (at least 1); 0 for an empty slice.
pub fn nearest_rank(sorted: &[f64], percent: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len();
    let rank = ((percent / 100.0) * n as f64).ceil() as usize;
    sorted[rank.clamp(1, n) - 1]
}

fn summarise(mut values: Vec<f64>) -> Summary {
    values.sort_by(f64::total_cmp);
    Summary {
        p50: nearest_rank(&values, 50.0),
        p99: nearest_rank(&values, 99.0),
        max: values.last().copied().unwrap_or(0.0),
    }
}

/// The ring of the last [`FrameStats::CAPACITY`] frames.
#[derive(Clone, Debug)]
pub struct FrameStats {
    ring: Vec<FrameSample>,
    next: usize,
}

impl Default for FrameStats {
    fn default() -> FrameStats {
        FrameStats::new()
    }
}

impl FrameStats {
    /// The number of frames the ring keeps.
    pub const CAPACITY: usize = 1024;

    /// An empty ring.
    pub fn new() -> FrameStats {
        FrameStats {
            ring: Vec::with_capacity(FrameStats::CAPACITY),
            next: 0,
        }
    }

    /// Adds a frame, replacing the oldest when the ring is full.
    pub fn push(&mut self, sample: FrameSample) {
        if self.ring.len() < FrameStats::CAPACITY {
            self.ring.push(sample);
        } else {
            self.ring[self.next] = sample;
        }
        self.next = (self.next + 1) % FrameStats::CAPACITY;
    }

    /// The number of frames held.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Whether no frame is held.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// The frames held, oldest first.
    pub fn samples(&self) -> impl Iterator<Item = &FrameSample> + '_ {
        let (newer, older) = if self.ring.len() < FrameStats::CAPACITY {
            (&self.ring[..], &self.ring[..0])
        } else {
            (&self.ring[..self.next], &self.ring[self.next..])
        };
        older.iter().chain(newer)
    }

    /// The summary of one phase.
    pub fn phase(&self, phase: Phase) -> Summary {
        summarise(self.ring.iter().map(|s| s.phase(phase)).collect())
    }

    /// The summary of the canvas's UI-thread time per frame ([`FrameSample::frame_ms`]).
    pub fn frame(&self) -> Summary {
        summarise(self.ring.iter().map(FrameSample::frame_ms).collect())
    }

    /// Frames per second from the median interval (0 without intervals).
    pub fn fps(&self) -> f64 {
        let p50 = self.phase(Phase::Interval).p50;
        if p50 > 0.0 { 1000.0 / p50 } else { 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(nearest_rank(&v, 50.0), 50.0);
        assert_eq!(nearest_rank(&v, 99.0), 99.0);
        assert_eq!(nearest_rank(&v, 100.0), 100.0);
        assert_eq!(nearest_rank(&v, 0.0), 1.0);
        assert_eq!(nearest_rank(&[7.0], 99.0), 7.0);
        assert_eq!(nearest_rank(&[], 50.0), 0.0);
        // Three values: p50 is rank 2, p99 rank 3.
        assert_eq!(nearest_rank(&[1.0, 2.0, 3.0], 50.0), 2.0);
        assert_eq!(nearest_rank(&[1.0, 2.0, 3.0], 99.0), 3.0);
    }

    fn sample(ms: f64) -> FrameSample {
        let mut s = FrameSample::default();
        s.set(Phase::Render, Duration::from_secs_f64(ms / 1000.0));
        s.set(Phase::Interval, Duration::from_millis(16));
        s
    }

    #[test]
    fn the_ring_wraps_at_its_capacity() {
        let mut stats = FrameStats::new();
        assert!(stats.is_empty());
        for i in 0..FrameStats::CAPACITY + 10 {
            stats.push(sample(i as f64));
        }
        assert_eq!(stats.len(), FrameStats::CAPACITY);
        // The oldest ten are gone: the samples run from 10 to capacity + 9, oldest first.
        let render: Vec<f64> = stats.samples().map(|s| s.phase(Phase::Render)).collect();
        assert!((render[0] - 10.0).abs() < 1e-9);
        assert!((render[render.len() - 1] - (FrameStats::CAPACITY + 9) as f64).abs() < 1e-9);
        assert!(render.windows(2).all(|w| w[0] < w[1]));
        let s = stats.phase(Phase::Render);
        assert!(s.max >= s.p99 && s.p99 >= s.p50);
        assert!((stats.fps() - 62.5).abs() < 1e-6);
    }

    #[test]
    fn phase_names_are_the_span_suffixes() {
        let names: Vec<&str> = Phase::ALL.iter().map(|p| p.name()).collect();
        assert_eq!(names, ["input", "scene", "render", "paint", "interval"]);
        let mut s = FrameSample::default();
        s.set(Phase::Input, Duration::from_millis(1));
        s.set(Phase::Paint, Duration::from_millis(2));
        s.set(Phase::Interval, Duration::from_millis(100));
        assert!(
            (s.frame_ms() - 3.0).abs() < 1e-9,
            "the interval is not frame time"
        );
    }
}
