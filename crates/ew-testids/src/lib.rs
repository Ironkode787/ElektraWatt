// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The seeded, reproducible UUIDv7 source of the tests (doc 18 §2.2). A dev-only crate.
//!
//! It was the `testkit` feature of `ew-ids`. A crate of its own changes no feature of `ew-ids`, so a package set
//! that uses it compiles `ew-ids` (and every crate above it) the same way as one that does not (T1).
//!
//! **Test-only by policy:** every edge to this crate is a dev-dependency. `cargo xtask check-features` fails on a
//! normal or build edge (`banned_crates` in `ci/dep-rules.toml`) and on the crate in a shipped binary's graph, and
//! the release pipeline asserts that it is absent (CI-R-09).

use chacha20::ChaCha20Rng;
use ew_ids::{EntropyError, IdSource, UnixTime};
use rand_core::{Rng, SeedableRng};

/// A reproducible [`IdSource`] for tests and snapshots (doc 18 §2.2).
///
/// The clock is a counter: the first call reads [`START_MILLIS`](SeededSource::START_MILLIS)
/// (2026-01-01T00:00:00Z) and every further call one millisecond later, with a zero sub-millisecond
/// fraction. The random bytes come from a ChaCha20 stream seeded with `seed`, so the same seed yields
/// the same IDs on every platform.
///
/// ```
/// use ew_ids::{Id, V7Generator};
/// use ew_testids::SeededSource;
///
/// struct Device;
/// let a: Id<Device> = V7Generator::new(SeededSource::new(7)).next();
/// let b: Id<Device> = V7Generator::new(SeededSource::new(7)).next();
/// assert_eq!(a, b);
/// ```
#[derive(Debug)]
pub struct SeededSource {
    rng: ChaCha20Rng,
    next_millis: u64,
}

impl SeededSource {
    /// The first timestamp, 2026-01-01T00:00:00Z in Unix milliseconds.
    pub const START_MILLIS: u64 = 1_767_225_600_000;

    /// A source whose random stream is seeded with `seed`.
    pub fn new(seed: u64) -> SeededSource {
        SeededSource {
            rng: ChaCha20Rng::seed_from_u64(seed),
            next_millis: SeededSource::START_MILLIS,
        }
    }
}

impl IdSource for SeededSource {
    fn now(&mut self) -> UnixTime {
        let t = UnixTime {
            millis: self.next_millis,
            sub_ms_nanos: 0,
        };
        self.next_millis += 1;
        t
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        self.rng.fill_bytes(buf);
        Ok(())
    }
}

/// A reproducible ID source, boxed as a session takes it: the same seed yields the same IDs on every platform.
pub fn seeded_ids(seed: u64) -> Box<dyn IdSource + Send> {
    Box::new(SeededSource::new(seed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_ids::{Id, V7Generator};

    struct Rec;

    #[test]
    fn seeded_source_is_reproducible() {
        let mut g = V7Generator::new(SeededSource::new(42));
        let ids: Vec<Id<Rec>> = (0..3).map(|_| g.next()).collect();
        let again: Vec<Id<Rec>> = {
            let mut g = V7Generator::new(SeededSource::new(42));
            (0..3).map(|_| g.next()).collect()
        };
        assert_eq!(ids, again);
        // The clock advances one millisecond per ID from the fixed start.
        for (i, id) in ids.iter().enumerate() {
            let b = id.as_uuid().into_bytes();
            let ms = u64::from_be_bytes([0, 0, b[0], b[1], b[2], b[3], b[4], b[5]]);
            assert_eq!(ms, SeededSource::START_MILLIS + i as u64);
        }
        let other: Id<Rec> = V7Generator::new(SeededSource::new(43)).next();
        assert_ne!(ids[0], other);
    }

    /// Fixed vectors: a change here means snapshots built on the seeded source change too.
    #[test]
    fn seeded_source_fixed_vectors() {
        let mut g = V7Generator::new(SeededSource::new(0));
        let ids: Vec<String> = (0..2).map(|_| g.next::<Rec>().to_string()).collect();
        assert_eq!(ids, SEEDED_VECTORS);
    }

    const SEEDED_VECTORS: [&str; 2] = [
        "019b76da-a800-7000-b2f7-f581d6de3c06",
        "019b76da-a801-7000-a822-fd6e7e8265fb",
    ];

    /// The boxed form yields the same stream as the source itself.
    #[test]
    fn seeded_ids_is_the_seeded_source() {
        struct Boxed(Box<dyn IdSource + Send>);
        impl IdSource for Boxed {
            fn now(&mut self) -> UnixTime {
                self.0.now()
            }
            fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
                self.0.fill(buf)
            }
        }
        let mut boxed = V7Generator::new(Boxed(seeded_ids(0)));
        let ids: Vec<String> = (0..2).map(|_| boxed.next::<Rec>().to_string()).collect();
        assert_eq!(ids, SEEDED_VECTORS);
    }

    /// The distribution test of `ew-ids` with the seeded ChaCha source and a frozen clock.
    #[test]
    fn seeded_entropy_spreads_over_shards() {
        struct FrozenSeeded(SeededSource);
        impl IdSource for FrozenSeeded {
            fn now(&mut self) -> UnixTime {
                UnixTime {
                    millis: SeededSource::START_MILLIS,
                    sub_ms_nanos: 0,
                }
            }
            fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
                self.0.fill(buf)
            }
        }
        let mut g = V7Generator::new(FrozenSeeded(SeededSource::new(1)));
        let mut shards = [0u32; 256];
        for _ in 0..10_000 {
            shards[usize::from(g.next::<Rec>().shard())] += 1;
        }
        assert!(shards.iter().all(|c| (10..=80).contains(c)), "{shards:?}");
    }
}
