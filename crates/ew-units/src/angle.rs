// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Angle2`: an integer angle for free texts and graphics.

use serde::{Deserialize, Serialize};

use crate::arith;

/// An angle in units of 10⁻³ degree (ADR-0005 amendment 1), used **only** by free texts and graphics (QET
/// free texts, DXF `INSERT` and `TEXT`). Symbol representations use [`Orient2`](crate::Orient2).
///
/// This type only stores the value; the rotation sense and all trigonometry belong to the deterministic
/// helpers of `ew-dl`.
///
/// **JSON:** the integer; the default 0 is omitted by record writers ([`is_zero`](Angle2::is_zero)).
///
/// ```
/// use ew_units::Angle2;
///
/// assert_eq!(Angle2(-90_000).normalized(), Angle2(270_000));
/// assert_eq!(Angle2::from_degrees_f64(45.0), Some(Angle2(45_000)));
/// assert_eq!(serde_json::to_string(&Angle2(45_000)).unwrap(), "45000");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Angle2(pub i32);

impl Angle2 {
    /// Zero.
    pub const ZERO: Angle2 = Angle2(0);
    /// One full turn, 360°.
    pub const FULL_TURN: Angle2 = Angle2(360_000);

    /// Whether the angle is 0 (omitted in JSON). An angle of 360° is not zero; normalise first if that
    /// matters.
    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    /// The same direction in `[0°, 360°)`, i.e. `[0, 360_000)`.
    pub const fn normalized(self) -> Angle2 {
        Angle2(self.0.rem_euclid(360_000))
    }

    /// Boundary helper: `degrees` in 10⁻³ degree, rounded to the nearest unit with exact ties **toward
    /// zero**; `None` for NaN, infinities and results outside the `i32` range. The product
    /// `degrees × 1000` is formed in f64 (one rounding) before rounding to an integer.
    ///
    /// ```
    /// use ew_units::Angle2;
    ///
    /// // 0.0625° = 62.5 units exactly: the tie goes toward zero.
    /// assert_eq!(Angle2::from_degrees_f64(0.0625), Some(Angle2(62)));
    /// assert_eq!(Angle2::from_degrees_f64(-0.0625), Some(Angle2(-62)));
    /// ```
    pub fn from_degrees_f64(degrees: f64) -> Option<Angle2> {
        arith::f64_to_i64(degrees * 1e3)
            .and_then(|v| i32::try_from(v).ok())
            .map(Angle2)
    }

    /// Boundary helper: the angle in degrees as f64.
    pub fn to_degrees_f64(self) -> f64 {
        f64::from(self.0) / 1e3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn normalisation() {
        assert_eq!(Angle2(0).normalized(), Angle2(0));
        assert_eq!(Angle2(360_000).normalized(), Angle2(0));
        assert_eq!(Angle2(-1).normalized(), Angle2(359_999));
        assert_eq!(Angle2(720_500).normalized(), Angle2(500));
        assert_eq!(
            Angle2(i32::MIN).normalized(),
            Angle2(i32::MIN.rem_euclid(360_000))
        );
        assert!(Angle2::default().is_zero());
        assert!(!Angle2::FULL_TURN.is_zero());
    }

    #[test]
    fn f64_boundary_rounding() {
        assert_eq!(Angle2::from_degrees_f64(0.0625), Some(Angle2(62)));
        assert_eq!(Angle2::from_degrees_f64(-0.0625), Some(Angle2(-62)));
        assert_eq!(Angle2::from_degrees_f64(0.1875), Some(Angle2(187))); // 187.5 → 187
        assert_eq!(Angle2::from_degrees_f64(0.0007), Some(Angle2(1)));
        assert_eq!(Angle2::from_degrees_f64(-12.3456), Some(Angle2(-12_346)));
        assert_eq!(Angle2::from_degrees_f64(f64::NAN), None);
        assert_eq!(Angle2::from_degrees_f64(f64::NEG_INFINITY), None);
        assert_eq!(Angle2::from_degrees_f64(3e6), None);
        assert_eq!(Angle2(-90_500).to_degrees_f64(), -90.5);
    }

    #[test]
    fn json_encoding() {
        assert_eq!(serde_json::to_string(&Angle2(-45_000)).unwrap(), "-45000");
        assert_eq!(serde_json::from_str::<Angle2>("45000").unwrap(), Angle2(45_000));
        for bad in ["45.0", "4.5e4", "2147483648", "\"45000\""] {
            assert!(serde_json::from_str::<Angle2>(bad).is_err(), "{bad}");
        }
    }

    proptest! {
        #[test]
        fn serde_round_trip(v in any::<i32>()) {
            let s = serde_json::to_string(&Angle2(v)).unwrap();
            prop_assert_eq!(serde_json::from_str::<Angle2>(&s).unwrap(), Angle2(v));
        }

        #[test]
        fn normalized_is_in_range_and_congruent(v in any::<i32>()) {
            let n = Angle2(v).normalized().0;
            prop_assert!((0..360_000).contains(&n));
            prop_assert_eq!((i64::from(v) - i64::from(n)) % 360_000, 0);
        }

        #[test]
        fn degrees_round_trip(v in any::<i32>()) {
            prop_assert_eq!(Angle2::from_degrees_f64(Angle2(v).to_degrees_f64()), Some(Angle2(v)));
        }
    }
}
