// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The JSON integer rule of doc 05 §3.2: integers only, |n| ≤ 2⁵³ − 1, no fraction, no exponent.

use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::UnitsError;

/// The largest integer magnitude a persisted integer may have: 2⁵³ − 1, exact in JavaScript and jq
/// (doc 05 §3.2).
pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

/// Whether `v` lies within ±[`MAX_SAFE_INTEGER`].
pub(crate) const fn is_safe(v: i64) -> bool {
    -MAX_SAFE_INTEGER <= v && v <= MAX_SAFE_INTEGER
}

/// An `i64` with the strict JSON integer encoding: serialising a value outside the safe range fails, and
/// deserialising rejects floats (any number written with a fraction or an exponent) and out-of-range
/// integers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SafeInt(pub i64);

impl Serialize for SafeInt {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if is_safe(self.0) {
            serializer.serialize_i64(self.0)
        } else {
            Err(serde::ser::Error::custom(UnitsError::IntegerOutOfRange))
        }
    }
}

impl<'de> Deserialize<'de> for SafeInt {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_i64(SafeIntVisitor)
    }
}

struct SafeIntVisitor;

impl Visitor<'_> for SafeIntVisitor {
    type Value = SafeInt;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an integer within ±(2^53 - 1)")
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<SafeInt, E> {
        if is_safe(v) {
            Ok(SafeInt(v))
        } else {
            Err(E::custom(UnitsError::IntegerOutOfRange))
        }
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<SafeInt, E> {
        match i64::try_from(v) {
            Ok(v) => self.visit_i64(v),
            Err(_) => Err(E::custom(UnitsError::IntegerOutOfRange)),
        }
    }

    // `visit_f64` is deliberately not implemented: serde's default rejects floats with "invalid type".
    // serde_json reads `-0` as the float -0.0, so "no -0" (doc 05 §3.2) is enforced by the same rule.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn de(s: &str) -> Result<SafeInt, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn accepts_safe_integers() {
        assert_eq!(de("0").unwrap(), SafeInt(0));
        assert_eq!(de("-1").unwrap(), SafeInt(-1));
        assert_eq!(de("9007199254740991").unwrap(), SafeInt(MAX_SAFE_INTEGER));
        assert_eq!(de("-9007199254740991").unwrap(), SafeInt(-MAX_SAFE_INTEGER));
    }

    #[test]
    fn rejects_unsafe_and_float_forms() {
        for s in [
            "9007199254740992",
            "-9007199254740992",
            "18446744073709551615",
            "1.0",
            "1.5",
            "1e3",
            "1E3",
            "-0.0",
            "-0",
            "\"1\"",
            "null",
            "true",
        ] {
            assert!(de(s).is_err(), "{s} must be rejected");
        }
    }

    #[test]
    fn serialising_out_of_range_fails() {
        assert_eq!(
            serde_json::to_string(&SafeInt(MAX_SAFE_INTEGER)).unwrap(),
            "9007199254740991"
        );
        assert!(serde_json::to_string(&SafeInt(MAX_SAFE_INTEGER + 1)).is_err());
        assert!(serde_json::to_string(&SafeInt(i64::MIN)).is_err());
    }
}
