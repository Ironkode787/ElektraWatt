// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! UUIDv7 for new authored records, by RFC 9562 §6.2 method 3 (ADR-0004 amendment 3).
//!
//! # Layout
//!
//! ```text
//!  0                   1                   2                   3
//!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                    unix_ts_ms (bits 47..16)                   |  bytes 0-3
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |   unix_ts_ms (bits 15..0)     |  ver  |  rand_a: sub-ms (12)  |  bytes 4-7
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |var|               rand_b: 62 fresh random bits                |  bytes 8-11
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                         rand_b (cont.)                        |  bytes 12-15
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! ```
//!
//! - `unix_ts_ms`: 48-bit Unix time in milliseconds, big-endian.
//! - `ver` = `0111` (7); `var` = `10` (RFC 9562).
//! - `rand_a` = the sub-millisecond fraction scaled to 12 bits, `nanos_in_ms × 4096 / 1_000_000` (a step
//!   of about 244 ns), so IDs from one clock are time-ordered down to that resolution (method 3,
//!   "increased clock precision").
//! - `rand_b` = 62 fresh random bits for **every** ID, **never a counter**: file line order (bytes 8–15,
//!   doc 05 §3.3) and sharding (byte 15, §2.2) need these bits uniformly random even for IDs minted in
//!   the same millisecond.
//!
//! The `uuid` crate's `Uuid::now_v7` and `ContextV7` are not used: `ContextV7` keeps a 42-bit counter
//! that spans `rand_a` and the top 30 bits of `rand_b` (checked in uuid 1.28.0), which would cluster
//! same-millisecond IDs in one sort position.
//!
//! Uniqueness rests on the 62 random bits; IDs are not guaranteed to be monotonic within the ~244 ns
//! resolution of `rand_a` or across clock adjustments, and nothing relies on that.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::{Builder, Uuid, Variant, Version};

use crate::Id;

/// A point in Unix time: whole milliseconds since 1970-01-01T00:00:00Z plus the nanoseconds within that
/// millisecond.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct UnixTime {
    /// Milliseconds since the Unix epoch. Only the low 48 bits enter a UUIDv7 (enough until the year
    /// 10889).
    pub millis: u64,
    /// Nanoseconds within the millisecond, `0..1_000_000`; larger values are clamped to 999 999.
    pub sub_ms_nanos: u32,
}

/// The OS random number generator failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntropyError {
    inner: getrandom::Error,
}

impl fmt::Display for EntropyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "random number generator failed: {}", self.inner)
    }
}

impl std::error::Error for EntropyError {}

/// The clock and the entropy behind a [`V7Generator`].
///
/// [`SystemSource`] is the production source; the `testkit` feature adds a seeded one, and tests can
/// supply their own (for example a frozen clock).
pub trait IdSource {
    /// The current time.
    fn now(&mut self) -> UnixTime;

    /// Fills `buf` with fresh random bytes.
    ///
    /// # Errors
    ///
    /// [`EntropyError`] if no random bytes are available.
    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError>;
}

/// The production [`IdSource`]: `std::time::SystemTime` and the OS random number generator (via
/// `getrandom`).
///
/// This crate is the one place allowed to read `SystemTime::now` for identity (doc 18 §2.2 static
/// rules). A clock before the Unix epoch reads as the epoch.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SystemSource;

impl IdSource for SystemSource {
    fn now(&mut self) -> UnixTime {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(d) => UnixTime {
                millis: u64::try_from(d.as_millis()).unwrap_or(u64::MAX),
                sub_ms_nanos: d.subsec_nanos() % 1_000_000,
            },
            Err(_) => UnixTime::default(),
        }
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        getrandom::fill(buf).map_err(|inner| EntropyError { inner })
    }
}

/// Mints UUIDv7 identifiers from an [`IdSource`]; see the [module docs](self) for the bit layout.
///
/// ```
/// use ew_ids::{Id, SystemSource, V7Generator};
///
/// struct Device;
/// let mut ids = V7Generator::new(SystemSource);
/// let a: Id<Device> = ids.next();
/// let b: Id<Device> = ids.next();
/// assert_ne!(a, b);
/// assert_eq!(a.as_uuid().get_version_num(), 7);
/// ```
#[derive(Clone, Debug, Default)]
pub struct V7Generator<S> {
    source: S,
}

impl<S: IdSource> V7Generator<S> {
    /// A generator drawing on `source`.
    pub const fn new(source: S) -> V7Generator<S> {
        V7Generator { source }
    }

    /// The source, for example to inspect a test clock.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    /// A new UUIDv7.
    ///
    /// # Errors
    ///
    /// [`EntropyError`] if the source has no random bytes.
    pub fn try_next<T>(&mut self) -> Result<Id<T>, EntropyError> {
        let t = self.source.now();
        let mut rand_b = [0u8; 8];
        self.source.fill(&mut rand_b)?;
        Ok(Id::from_uuid(assemble(t, rand_b)))
    }

    /// A new UUIDv7.
    ///
    /// # Panics
    ///
    /// If the source has no random bytes (for [`SystemSource`]: the OS random number generator failed),
    /// like `uuid`'s own generators. Use [`try_next`](V7Generator::try_next) to handle that case.
    #[allow(
        clippy::should_implement_trait,
        reason = "generic over the record kind T, so it cannot be Iterator::next"
    )]
    pub fn next<T>(&mut self) -> Id<T> {
        match self.try_next() {
            Ok(id) => id,
            Err(e) => panic!("cannot mint a UUIDv7: {e}"),
        }
    }
}

/// The 12-bit `rand_a`: the sub-millisecond fraction scaled to 4096 steps.
pub(crate) const fn rand_a(sub_ms_nanos: u32) -> u16 {
    let n = if sub_ms_nanos > 999_999 {
        999_999
    } else {
        sub_ms_nanos
    };
    // n < 10^6, so n × 4096 < 2^32 and the quotient is < 4096.
    ((n as u64 * 4096) / 1_000_000) as u16
}

/// Builds the UUID of the layout above from a time and 8 random bytes (of which 62 bits are used).
fn assemble(t: UnixTime, random: [u8; 8]) -> Uuid {
    let ms = t.millis.to_be_bytes(); // the low 48 bits are bytes 2..8
    let a = rand_a(t.sub_ms_nanos).to_be_bytes();
    let bytes = [
        ms[2], ms[3], ms[4], ms[5], ms[6], ms[7], // unix_ts_ms
        a[0], a[1], // version nibble (set below) and rand_a
        random[0], random[1], random[2], random[3], random[4], random[5], random[6],
        random[7], // var + rand_b
    ];
    Builder::from_bytes(bytes)
        .with_version(Version::SortRand)
        .with_variant(Variant::RFC4122)
        .into_uuid()
}

impl<T> Id<T> {
    /// A new UUIDv7 from [`SystemSource`].
    ///
    /// # Panics
    ///
    /// If the OS random number generator fails; see [`V7Generator::next`].
    pub fn new_v7() -> Id<T> {
        V7Generator::new(SystemSource).next()
    }

    /// A new UUIDv7 from [`SystemSource`].
    ///
    /// # Errors
    ///
    /// [`EntropyError`] if the OS random number generator fails.
    pub fn try_new_v7() -> Result<Id<T>, EntropyError> {
        V7Generator::new(SystemSource).try_next()
    }
}

#[cfg(feature = "testkit")]
pub use testkit::SeededSource;

#[cfg(feature = "testkit")]
mod testkit {
    use chacha20::ChaCha20Rng;
    use rand_core::{Rng, SeedableRng};

    use super::{EntropyError, IdSource, UnixTime};

    /// A reproducible [`IdSource`] for tests and snapshots (doc 18 §2.2; feature `testkit`).
    ///
    /// The clock is a counter: the first call reads [`START_MILLIS`](SeededSource::START_MILLIS)
    /// (2026-01-01T00:00:00Z) and every further call one millisecond later, with a zero sub-millisecond
    /// fraction. The random bytes come from a ChaCha20 stream seeded with `seed`, so the same seed yields
    /// the same IDs on every platform.
    ///
    /// **Test-only by policy:** the release pipeline asserts that the `testkit` feature is absent (CI-R-09).
    ///
    /// ```
    /// use ew_ids::{Id, V7Generator, v7::SeededSource};
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Rec;

    /// A frozen clock with a deterministic entropy stream (SplitMix64), so the distribution tests below
    /// are reproducible and never flaky.
    struct Frozen {
        time: UnixTime,
        state: u64,
    }

    impl Frozen {
        fn new(seed: u64) -> Frozen {
            Frozen {
                time: UnixTime {
                    millis: 1_767_225_600_123,
                    sub_ms_nanos: 456_789,
                },
                state: seed,
            }
        }
    }

    impl IdSource for Frozen {
        fn now(&mut self) -> UnixTime {
            self.time
        }

        fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
            for chunk in buf.chunks_mut(8) {
                self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = self.state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^= z >> 31;
                chunk.copy_from_slice(&z.to_le_bytes()[..chunk.len()]);
            }
            Ok(())
        }
    }

    fn mint(n: usize, seed: u64) -> Vec<Id<Rec>> {
        let mut g = V7Generator::new(Frozen::new(seed));
        (0..n).map(|_| g.next()).collect()
    }

    #[test]
    fn version_variant_and_timestamp() {
        let ids = mint(100, 1);
        for id in &ids {
            let u = id.as_uuid();
            assert_eq!(u.get_version(), Some(Version::SortRand));
            assert_eq!(u.get_variant(), Variant::RFC4122);
            let b = u.as_bytes();
            let ms = u64::from_be_bytes([0, 0, b[0], b[1], b[2], b[3], b[4], b[5]]);
            assert_eq!(ms, 1_767_225_600_123);
            let a = u16::from_be_bytes([b[6], b[7]]) & 0x0FFF;
            assert_eq!(a, rand_a(456_789));
        }
        // Same millisecond: the first 48 bits agree, rand_b differs.
        let (x, y) = (ids[0].as_uuid(), ids[1].as_uuid());
        assert_eq!(x.as_bytes()[..6], y.as_bytes()[..6]);
        assert_ne!(
            ids[0].sort_key() & ((1 << 62) - 1),
            ids[1].sort_key() & ((1 << 62) - 1)
        );
        assert_ne!(x, y);
    }

    #[test]
    fn layout_is_exact() {
        let t = UnixTime {
            millis: 0x0123_4567_89AB,
            sub_ms_nanos: 500_000,
        };
        let u = assemble(t, [0xFF; 8]);
        assert_eq!(u.to_string(), "01234567-89ab-7800-bfff-ffffffffffff");
        let u = assemble(
            UnixTime {
                millis: 0,
                sub_ms_nanos: 0,
            },
            [0; 8],
        );
        assert_eq!(u.to_string(), "00000000-0000-7000-8000-000000000000");
        // Bits above 48 are dropped.
        let u = assemble(
            UnixTime {
                millis: 0xFFFF_0000_0000_0001,
                sub_ms_nanos: 999_999,
            },
            [0; 8],
        );
        assert_eq!(u.to_string(), "00000000-0001-7fff-8000-000000000000");
    }

    #[test]
    fn rand_a_is_monotonic_in_sub_ms() {
        assert_eq!(rand_a(0), 0);
        assert_eq!(rand_a(244), 0);
        assert_eq!(rand_a(245), 1);
        assert_eq!(rand_a(500_000), 2048);
        assert_eq!(rand_a(999_999), 4095);
        assert_eq!(rand_a(5_000_000), 4095);
        let mut prev = 0;
        for n in (0..1_000_000).step_by(97) {
            let a = rand_a(n);
            assert!(a >= prev && a < 4096);
            prev = a;
        }
        // Through the generator: later sub-ms times give non-decreasing IDs within one millisecond.
        let mut last = Uuid::nil();
        for n in [0, 1_000, 250_000, 250_001, 600_000, 999_999] {
            let mut src = Frozen::new(9);
            src.time.sub_ms_nanos = n;
            let u = V7Generator::new(src).next::<Rec>().as_uuid();
            assert!(u.as_bytes()[..8] >= last.as_bytes()[..8], "{n}");
            last = u;
        }
    }

    /// ADR-0004 amendment 3 / doc 05 §15 item 12: 10,000 IDs minted within one millisecond spread
    /// evenly over the 256 shards and over sort positions.
    #[test]
    fn same_millisecond_ids_spread_over_shards_and_sort_positions() {
        const N: usize = 10_000;
        let ids = mint(N, 0x5EED);

        // Shards. Each count is Binomial(10000, 1/256): mean 39.1, standard deviation 6.2. The band
        // 10..=80 lies 4.7 sd below and 6.6 sd above the mean; for uniform bytes the chance that any of
        // the 256 shards leaves it is below 10⁻⁵ (and this test is deterministic anyway). A counter in
        // the low bytes would empty most shards or crowd a few, far outside the band.
        for (s, &c) in shard_counts(&ids).iter().enumerate() {
            assert!((10..=80).contains(&c), "shard {s:02x} holds {c} of {N}");
        }

        // Sort positions. For random keys, two consecutively minted IDs are neighbours in sort order with
        // probability about 2/N, so about 2 of 9 999 pairs; a counter in rand_b makes almost every pair
        // adjacent. The bound is 5 %.
        let adjacent = adjacent_pairs(&ids);
        assert!(
            adjacent < N / 20,
            "{adjacent} of {} consecutive pairs are adjacent in sort order",
            N - 1
        );

        // No duplicates.
        let unique: BTreeMap<Id<Rec>, ()> = ids.iter().map(|&id| (id, ())).collect();
        assert_eq!(unique.len(), N);
    }

    fn shard_counts(ids: &[Id<Rec>]) -> [u32; 256] {
        let mut shards = [0u32; 256];
        for id in ids {
            shards[usize::from(id.shard())] += 1;
        }
        shards
    }

    /// How many consecutively minted pairs are neighbours in `sort_key` order.
    fn adjacent_pairs(ids: &[Id<Rec>]) -> usize {
        let mut order: Vec<(u64, usize)> = ids.iter().enumerate().map(|(i, id)| (id.sort_key(), i)).collect();
        order.sort_unstable();
        let mut pos = vec![0usize; ids.len()];
        for (p, &(_, i)) in order.iter().enumerate() {
            pos[i] = p;
        }
        (1..ids.len())
            .filter(|&i| pos[i].abs_diff(pos[i - 1]) == 1)
            .count()
    }

    /// Negative control: each check catches the counters it is meant for. A counter in the leading
    /// `rand_b` bits (the scheme of uuid's `ContextV7`) or in the trailing bytes fails the sort-position
    /// check; a trailing counter cycles byte 15 evenly, so only the sort check sees it; a slowly changing
    /// byte 15 fails the shard band.
    #[test]
    fn distribution_checks_catch_counters() {
        let mut src = Frozen::new(3);
        let t = src.now();
        let leading: Vec<Id<Rec>> = (0u32..10_000)
            .map(|n| {
                let mut r = [0u8; 8];
                src.fill(&mut r).unwrap();
                r[..4].copy_from_slice(&(n << 2).to_be_bytes()); // a 30-bit counter below the variant bits
                Id::from_uuid(assemble(t, r))
            })
            .collect();
        assert!(adjacent_pairs(&leading) > 9_000);

        let trailing: Vec<Id<Rec>> = (0u32..10_000)
            .map(|n| {
                let mut r = [0u8; 8];
                r[4..].copy_from_slice(&n.to_be_bytes());
                Id::from_uuid(assemble(t, r))
            })
            .collect();
        assert!(adjacent_pairs(&trailing) > 9_000);
        assert!(shard_counts(&trailing).iter().all(|&c| (10..=80).contains(&c)));

        let slow: Vec<Id<Rec>> = (0u32..10_000)
            .map(|n| {
                let mut r = [0u8; 8];
                r[7] = (n / 1_000) as u8;
                Id::from_uuid(assemble(t, r))
            })
            .collect();
        assert!(shard_counts(&slow).iter().any(|&c| !(10..=80).contains(&c)));
    }

    /// The production source: real clock and entropy give distinct, well-formed IDs.
    #[test]
    fn system_source_mints_valid_ids() {
        let a: Id<Rec> = Id::new_v7();
        let b: Id<Rec> = Id::try_new_v7().unwrap();
        assert_ne!(a, b);
        assert_eq!(b.as_uuid().get_version_num(), 7);
        assert!(SystemSource.now().millis > 1_700_000_000_000);
    }

    #[test]
    fn failing_entropy_is_reported() {
        struct Broken;
        impl IdSource for Broken {
            fn now(&mut self) -> UnixTime {
                UnixTime::default()
            }
            fn fill(&mut self, _: &mut [u8]) -> Result<(), EntropyError> {
                Err(EntropyError {
                    inner: getrandom::Error::UNSUPPORTED,
                })
            }
        }
        let mut g = V7Generator::new(Broken);
        let e = g.try_next::<Rec>().unwrap_err();
        assert!(e.to_string().starts_with("random number generator failed"));
        let panicked = std::panic::catch_unwind(move || g.next::<Rec>()).is_err();
        assert!(panicked);
    }

    #[cfg(feature = "testkit")]
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
    #[cfg(feature = "testkit")]
    #[test]
    fn seeded_source_fixed_vectors() {
        let mut g = V7Generator::new(SeededSource::new(0));
        let ids: Vec<String> = (0..2).map(|_| g.next::<Rec>().to_string()).collect();
        assert_eq!(ids, SEEDED_VECTORS);
    }

    #[cfg(feature = "testkit")]
    const SEEDED_VECTORS: [&str; 2] = [
        "019b76da-a800-7000-b2f7-f581d6de3c06",
        "019b76da-a801-7000-a822-fd6e7e8265fb",
    ];

    /// The same distribution test with the seeded ChaCha source and a frozen clock.
    #[cfg(feature = "testkit")]
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
