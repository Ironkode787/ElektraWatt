// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The seeded identifier source of the generator.
//!
//! `ew_ids::v7::SeededSource` lives behind the `testkit` feature of `ew-ids`; enabling it on the normal
//! dependency edge of this crate would put a test-only feature into every consumer, so `ci/dep-rules.toml`
//! would need an exception. The generator therefore brings its own tiny source: SplitMix64 for the random
//! bytes and the same counter clock as `SeededSource`.

use ew_ids::{EntropyError, IdSource, UnixTime};

/// A reproducible [`IdSource`]: the same seed yields the same IDs on every platform.
///
/// The random bytes come from a SplitMix64 stream seeded with the seed (each `u64` is consumed
/// little-endian, bytes in order; a partly used word is dropped). The clock starts at 2026-01-01T00:00:00Z
/// (1 767 225 600 000 ms) and advances one millisecond per [`now`](IdSource::now) call, with a zero
/// sub-millisecond fraction.
#[derive(Clone, Debug)]
pub struct SynthSource {
    state: u64,
    next_millis: u64,
}

impl SynthSource {
    /// The first timestamp, 2026-01-01T00:00:00Z in Unix milliseconds.
    pub const START_MILLIS: u64 = 1_767_225_600_000;

    /// A source whose random stream is seeded with `seed`.
    pub fn new(seed: u64) -> SynthSource {
        SynthSource {
            state: seed,
            next_millis: SynthSource::START_MILLIS,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl IdSource for SynthSource {
    fn now(&mut self) -> UnixTime {
        let t = UnixTime {
            millis: self.next_millis,
            sub_ms_nanos: 0,
        };
        self.next_millis += 1;
        t
    }

    fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError> {
        for chunk in buf.chunks_mut(8) {
            let word = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_ids::{Id, V7Generator};

    struct Rec;

    fn first_three(seed: u64) -> Vec<Id<Rec>> {
        let mut g = V7Generator::new(SynthSource::new(seed));
        (0..3).map(|_| g.next()).collect()
    }

    #[test]
    fn same_seed_same_ids() {
        assert_eq!(first_three(9), first_three(9));
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(first_three(1), first_three(2));
    }

    #[test]
    fn clock_advances_one_millisecond_per_call() {
        let mut s = SynthSource::new(0);
        assert_eq!(s.now().millis, SynthSource::START_MILLIS);
        assert_eq!(s.now().millis, SynthSource::START_MILLIS + 1);
        assert_eq!(s.now().sub_ms_nanos, 0);
    }

    #[test]
    fn splitmix64_reference_values() {
        // The first outputs of SplitMix64 seeded with 0 (reference implementation by Vigna).
        let mut s = SynthSource::new(0);
        assert_eq!(s.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(s.next_u64(), 0x6E78_9E6A_A1B9_65F4);
    }
}
