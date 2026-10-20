// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Time for the lock and the journal (doc 05 §7, §8). The project files never carry a time (NFR-REL-07);
//! only the lock record and the journal, which are not project data, do.

use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The wall clock and sleeping, injectable for tests.
pub trait Clock: Send + Sync + fmt::Debug {
    /// The current time.
    fn now(&self) -> SystemTime;
    /// Waits `d` (the rename retries and the takeover re-read).
    fn sleep(&self, d: Duration);
}

/// The system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
}

/// A clock for tests: time moves only when slept or advanced.
#[derive(Debug)]
pub struct ManualClock(Mutex<SystemTime>);

impl ManualClock {
    /// A clock at `start`.
    pub fn new(start: SystemTime) -> ManualClock {
        ManualClock(Mutex::new(start))
    }

    /// A clock at 2026-10-17T07:42:10Z.
    pub fn fixed() -> ManualClock {
        ManualClock::new(UNIX_EPOCH + Duration::from_secs(1_792_222_930))
    }

    /// Moves the clock forward.
    pub fn advance(&self, d: Duration) {
        let mut t = self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *t += d;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn sleep(&self, d: Duration) {
        self.advance(d);
    }
}

/// Milliseconds since the Unix epoch (the journal's `t`); 0 before 1970.
pub fn epoch_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// RFC 3339 in UTC with whole seconds (`2026-10-17T07:42:10Z`), the time form of the lock record.
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_dates() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(ManualClock::fixed().now()), "2026-10-17T07:42:10Z");
        // A leap day and the end of a century year.
        assert_eq!(
            rfc3339(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
        assert_eq!(
            rfc3339(UNIX_EPOCH + Duration::from_secs(4_107_542_399)),
            "2100-02-28T23:59:59Z"
        );
    }
}
