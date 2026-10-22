// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The seeded random source of the fuzzer.

/// SplitMix64 (Steele, Lea and Flood 2014): a 64-bit state, one addition and a mixing function per output.
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (`n > 0`); the modulo bias is irrelevant at these sizes.
    pub fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("a size fits u64")).expect("below n")
    }

    /// A number in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + i64::try_from(self.next() % u64::try_from(hi - lo + 1).expect("a range")).expect("small")
    }

    /// True once in `one_in` draws.
    pub fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        (!items.is_empty()).then(|| &items[self.below(items.len())])
    }

    /// A UUID-shaped string made of two draws; every call advances the generator.
    pub fn uuid(&mut self) -> String {
        let (a, b) = (self.next(), self.next());
        let v = (u128::from(a) << 64) | u128::from(b);
        // Version 7 and variant bits set, so that the text parses as the UUID of a record.
        let v = (v & !(0xF << 76) & !(0x3 << 62)) | (0x7 << 76) | (0x2 << 62);
        ew_ids::Uuid::from_u128(v).hyphenated().to_string()
    }

    /// A short upper-case word of `1..=max` letters.
    pub fn word(&mut self, max: usize) -> String {
        let len = 1 + self.below(max);
        (0..len)
            .map(|_| char::from(b'A' + u8::try_from(self.below(26)).expect("a letter")))
            .collect()
    }
}
