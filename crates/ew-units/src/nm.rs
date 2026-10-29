// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Nm`: the one linear unit.

use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::arith;
use crate::json::SafeInt;

/// A length or coordinate in nanometres (ADR-0005): the only linear unit in the model and in files.
///
/// Page coordinates, symbol geometry, 3D translations, wire and cable lengths are all `Nm`. The `i64`
/// range is ±9.2 × 10⁹ m; the persisted range is ±(2⁵³ − 1) nm ≈ ±9 007 km (doc 05 §3.2).
///
/// The operators `+`, `-` and unary `-` follow Rust's integer overflow semantics (a panic in debug builds,
/// wrapping in release builds); the `checked_*` methods return `None` instead.
///
/// **JSON:** an integer within ±(2⁵³ − 1). Serialising a larger value fails; deserialising rejects
/// floats, `-0`, exponents and out-of-range integers.
///
/// ```
/// use ew_units::Nm;
///
/// assert_eq!(Nm::from_mm(4), Nm(4_000_000));
/// assert_eq!(serde_json::to_string(&Nm::from_um(-250)).unwrap(), "-250000");
/// assert!(serde_json::from_str::<Nm>("1.5").is_err());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Nm(pub i64);

impl Nm {
    /// Zero length.
    pub const ZERO: Nm = Nm(0);
    /// One micrometre.
    pub const UM: Nm = Nm(1_000);
    /// One millimetre.
    pub const MM: Nm = Nm(1_000_000);

    /// `mm` millimetres. Overflows like `i64` multiplication beyond ±9.2 × 10¹² mm.
    pub const fn from_mm(mm: i64) -> Nm {
        Nm(mm * 1_000_000)
    }

    /// `um` micrometres. Overflows like `i64` multiplication beyond ±9.2 × 10¹⁵ µm.
    pub const fn from_um(um: i64) -> Nm {
        Nm(um * 1_000)
    }

    /// `self + rhs`, or `None` on overflow.
    pub const fn checked_add(self, rhs: Nm) -> Option<Nm> {
        match self.0.checked_add(rhs.0) {
            Some(v) => Some(Nm(v)),
            None => None,
        }
    }

    /// `self - rhs`, or `None` on overflow.
    pub const fn checked_sub(self, rhs: Nm) -> Option<Nm> {
        match self.0.checked_sub(rhs.0) {
            Some(v) => Some(Nm(v)),
            None => None,
        }
    }

    /// `self × factor`, or `None` on overflow.
    pub const fn checked_mul(self, factor: i64) -> Option<Nm> {
        match self.0.checked_mul(factor) {
            Some(v) => Some(Nm(v)),
            None => None,
        }
    }

    /// `-self`, or `None` for `i64::MIN`.
    pub const fn checked_neg(self) -> Option<Nm> {
        match self.0.checked_neg() {
            Some(v) => Some(Nm(v)),
            None => None,
        }
    }

    /// The absolute value; overflows like [`i64::abs`] for `i64::MIN`.
    pub const fn abs(self) -> Nm {
        Nm(self.0.abs())
    }

    /// Boundary helper: `mm` millimetres as `Nm`, rounded to the nearest nanometre with exact ties
    /// **toward zero**; `None` for NaN, infinities and results outside the `i64` range.
    ///
    /// The product `mm × 10⁶` is formed in f64 (one rounding), then rounded to an integer. For importers
    /// and boundary code only; the model never holds f64 lengths.
    ///
    /// ```
    /// use ew_units::Nm;
    ///
    /// assert_eq!(Nm::from_mm_f64(2.5), Some(Nm(2_500_000)));
    /// // 1/128 mm = 7812.5 nm exactly: the tie goes toward zero.
    /// assert_eq!(Nm::from_mm_f64(0.0078125), Some(Nm(7_812)));
    /// assert_eq!(Nm::from_mm_f64(-0.0078125), Some(Nm(-7_812)));
    /// assert_eq!(Nm::from_mm_f64(f64::NAN), None);
    /// ```
    pub fn from_mm_f64(mm: f64) -> Option<Nm> {
        arith::f64_to_i64(mm * 1e6).map(Nm)
    }

    /// Boundary helper: this length in millimetres as f64 (`self / 10⁶`, correctly rounded for
    /// |n| ≤ 2⁵³). For renderers and boundary code only.
    pub fn to_mm_f64(self) -> f64 {
        self.0 as f64 / 1e6
    }
}

impl Add for Nm {
    type Output = Nm;
    fn add(self, rhs: Nm) -> Nm {
        Nm(self.0 + rhs.0)
    }
}

impl Sub for Nm {
    type Output = Nm;
    fn sub(self, rhs: Nm) -> Nm {
        Nm(self.0 - rhs.0)
    }
}

impl Neg for Nm {
    type Output = Nm;
    fn neg(self) -> Nm {
        Nm(-self.0)
    }
}

impl AddAssign for Nm {
    fn add_assign(&mut self, rhs: Nm) {
        self.0 += rhs.0;
    }
}

impl SubAssign for Nm {
    fn sub_assign(&mut self, rhs: Nm) {
        self.0 -= rhs.0;
    }
}

impl Serialize for Nm {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SafeInt(self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Nm {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        SafeInt::deserialize(deserializer).map(|v| Nm(v.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MAX_SAFE_INTEGER;
    use proptest::prelude::*;

    #[test]
    fn checked_arithmetic() {
        assert_eq!(Nm(2).checked_add(Nm(3)), Some(Nm(5)));
        assert_eq!(Nm(i64::MAX).checked_add(Nm(1)), None);
        assert_eq!(Nm(i64::MIN).checked_sub(Nm(1)), None);
        assert_eq!(Nm(5).checked_sub(Nm(7)), Some(Nm(-2)));
        assert_eq!(Nm(4).checked_mul(-3), Some(Nm(-12)));
        assert_eq!(Nm(i64::MAX).checked_mul(2), None);
        assert_eq!(Nm(i64::MIN).checked_neg(), None);
        assert_eq!(Nm(9).checked_neg(), Some(Nm(-9)));
        assert_eq!(Nm(-9).abs(), Nm(9));
    }

    #[test]
    fn operators() {
        let mut a = Nm(10);
        a += Nm(5);
        a -= Nm(2);
        assert_eq!(a, Nm(13));
        assert_eq!(Nm(1) + Nm(2) - Nm(4), Nm(-1));
        assert_eq!(-Nm(3), Nm(-3));
    }

    #[test]
    fn f64_boundary_rounding() {
        assert_eq!(Nm::from_mm_f64(0.0), Some(Nm(0)));
        assert_eq!(Nm::from_mm_f64(-0.0), Some(Nm(0)));
        assert_eq!(Nm::from_mm_f64(1.0), Some(Nm::MM));
        // Exact ties: 1/128 mm = 7812.5 nm, 3/128 mm = 23437.5 nm.
        assert_eq!(Nm::from_mm_f64(1.0 / 128.0), Some(Nm(7_812)));
        assert_eq!(Nm::from_mm_f64(-1.0 / 128.0), Some(Nm(-7_812)));
        assert_eq!(Nm::from_mm_f64(3.0 / 128.0), Some(Nm(23_437)));
        assert_eq!(Nm::from_mm_f64(-3.0 / 128.0), Some(Nm(-23_437)));
        // Not ties: nearest.
        assert_eq!(Nm::from_mm_f64(0.000_000_7), Some(Nm(1)));
        assert_eq!(Nm::from_mm_f64(-0.000_000_7), Some(Nm(-1)));
        assert_eq!(Nm::from_mm_f64(0.000_000_3), Some(Nm(0)));
        assert_eq!(Nm::from_mm_f64(f64::NAN), None);
        assert_eq!(Nm::from_mm_f64(f64::INFINITY), None);
        assert_eq!(Nm::from_mm_f64(1e13), None);
        assert_eq!(Nm(2_500_000).to_mm_f64(), 2.5);
        assert_eq!(Nm(-1).to_mm_f64(), -1e-6);
    }

    #[test]
    fn json_encoding() {
        assert_eq!(serde_json::to_string(&Nm(137_583_333)).unwrap(), "137583333");
        assert_eq!(serde_json::from_str::<Nm>("-42").unwrap(), Nm(-42));
        // serde_json reads `-0` as the float -0.0, so the doc 05 rule "no -0" holds.
        assert!(serde_json::from_str::<Nm>("-0").is_err());
        assert!(serde_json::to_string(&Nm(MAX_SAFE_INTEGER + 1)).is_err());
        assert!(serde_json::to_string(&Nm(-MAX_SAFE_INTEGER - 1)).is_err());
        for bad in ["1.0", "2e3", "9007199254740992", "\"5\"", "null"] {
            assert!(serde_json::from_str::<Nm>(bad).is_err(), "{bad}");
        }
    }

    proptest! {
        #[test]
        fn serde_round_trip(v in -MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER) {
            let s = serde_json::to_string(&Nm(v)).unwrap();
            prop_assert_eq!(s.clone(), v.to_string());
            prop_assert_eq!(serde_json::from_str::<Nm>(&s).unwrap(), Nm(v));
        }

        /// Integral millimetre values convert exactly in both directions.
        #[test]
        fn mm_f64_round_trip(mm in -9_000_000i64..9_000_000) {
            let nm = Nm::from_mm(mm);
            prop_assert_eq!(Nm::from_mm_f64(nm.to_mm_f64()), Some(nm));
        }

        /// Every safe nm value survives the f64 boundary in both directions.
        #[test]
        fn nm_f64_round_trip(v in -1_000_000_000_000i64..1_000_000_000_000) {
            prop_assert_eq!(Nm::from_mm_f64(Nm(v).to_mm_f64()), Some(Nm(v)));
        }
    }
}
