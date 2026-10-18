// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared rounding helpers. Every rounding in this crate goes to the nearest integer with exact ties
//! toward zero.

/// `n / d` rounded to the nearest integer, exact ties toward zero.
///
/// `d` must be non-zero, and `(n, d)` must not be `(i128::MIN, -1)`; every caller keeps its operands far
/// inside the `i128` range.
pub(crate) fn div_round(n: i128, d: i128) -> i128 {
    debug_assert!(d != 0, "division by zero");
    let q = n / d;
    let r = n % d;
    let (ar, ad) = (r.unsigned_abs(), d.unsigned_abs());
    // |r| > |d| / 2, written without overflow. A tie (|r| = |d| / 2) keeps the truncated quotient.
    if ar > ad - ar {
        // r != 0 here; r has the sign of n, so the true quotient lies beyond q in the sign of n / d.
        if (n < 0) == (d < 0) { q + 1 } else { q - 1 }
    } else {
        q
    }
}

/// `v` rounded to the nearest integer, exact ties toward zero. NaN and infinities pass through.
///
/// `v - v.trunc()` is exact in IEEE 754 arithmetic, so the tie test sees the true fraction; no `± 0.5` is
/// ever added.
pub(crate) fn round_f64(v: f64) -> f64 {
    let t = v.trunc();
    let frac = v - t;
    if frac.abs() > 0.5 { t + frac.signum() } else { t }
}

/// `v` rounded by [`round_f64`] and converted to `i64`; `None` for NaN, infinities and values outside the
/// `i64` range.
pub(crate) fn f64_to_i64(v: f64) -> Option<i64> {
    if !v.is_finite() {
        return None;
    }
    let r = round_f64(v);
    // 2^63 is exactly representable; the range is [-2^63, 2^63).
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    if (-LIMIT..LIMIT).contains(&r) {
        // `r` is integral and in range, so the cast is exact.
        Some(r as i64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn div_round_ties_toward_zero() {
        assert_eq!(div_round(5, 2), 2);
        assert_eq!(div_round(-5, 2), -2);
        assert_eq!(div_round(5, -2), -2);
        assert_eq!(div_round(-5, -2), 2);
        assert_eq!(div_round(7, 2), 3);
        assert_eq!(div_round(-7, 2), -3);
    }

    #[test]
    fn div_round_nearest() {
        assert_eq!(div_round(6, 4), 1); // 1.5 tie → 1
        assert_eq!(div_round(7, 4), 2); // 1.75 → 2
        assert_eq!(div_round(-7, 4), -2);
        assert_eq!(div_round(5, 4), 1); // 1.25 → 1
        assert_eq!(div_round(1, 3), 0);
        assert_eq!(div_round(2, 3), 1);
        assert_eq!(div_round(-2, 3), -1);
        assert_eq!(div_round(0, 7), 0);
    }

    #[test]
    fn round_f64_ties_toward_zero() {
        assert_eq!(round_f64(0.5), 0.0);
        assert_eq!(round_f64(-0.5), 0.0);
        assert_eq!(round_f64(1.5), 1.0);
        assert_eq!(round_f64(-1.5), -1.0);
        assert_eq!(round_f64(2.5), 2.0);
        assert_eq!(round_f64(-2.5), -2.0);
        assert_eq!(round_f64(2.500_000_000_000_000_4), 3.0);
        assert_eq!(round_f64(0.499_999_999_999_999_94), 0.0);
        assert_eq!(round_f64(1.75), 2.0);
        assert_eq!(round_f64(-1.75), -2.0);
        assert_eq!(round_f64(4_503_599_627_370_497.0), 4_503_599_627_370_497.0);
    }

    #[test]
    fn f64_to_i64_rejects_non_finite_and_out_of_range() {
        assert_eq!(f64_to_i64(f64::NAN), None);
        assert_eq!(f64_to_i64(f64::INFINITY), None);
        assert_eq!(f64_to_i64(f64::NEG_INFINITY), None);
        assert_eq!(f64_to_i64(9_223_372_036_854_775_808.0), None);
        assert_eq!(f64_to_i64(-9_223_372_036_854_775_808.0), Some(i64::MIN));
        assert_eq!(f64_to_i64(-1e300), None);
        assert_eq!(f64_to_i64(-0.0), Some(0));
    }

    proptest! {
        /// `div_round` agrees with an exact reference on the doubled numerator.
        #[test]
        fn div_round_matches_reference(n in -1_000_000_000_000i128..1_000_000_000_000, d in 1i128..1_000_000) {
            for d in [d, -d] {
                let q = div_round(n, d);
                // |n/d - q| <= 1/2, i.e. |n - q·d| · 2 <= |d|, and a tie implies |q| <= |n/d|.
                let err = (n - q * d).abs() * 2;
                prop_assert!(err <= d.abs());
                if err == d.abs() {
                    prop_assert!(q.abs() * d.abs() <= n.abs());
                }
            }
        }
    }
}
