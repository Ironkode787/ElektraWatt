// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The error type of this crate.

use std::fmt;

/// An invalid value: a rejected constructor argument or a rejected encoded form.
///
/// Serde errors carry this type's [`Display`](fmt::Display) text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum UnitsError {
    /// An integer outside ±(2⁵³ − 1), the JSON safety range of doc 05 §3.2.
    IntegerOutOfRange,
    /// A grid step of zero (numerator or denominator).
    ZeroGrid,
    /// A grid step finer than 1 nm (`num < den`); coordinates are whole nanometres.
    GridTooFine,
    /// An encoded grid that is not in lowest terms.
    UnreducedGrid,
    /// A quaternion whose squared norm is outside the tolerance of [`Quat9::is_unit`](crate::Quat9::is_unit).
    NotUnitQuaternion,
    /// An encoded quaternion that is not sign-normalised (w > 0, else the first non-zero component > 0).
    NotSignNormalized,
    /// A decimal string that is not well-formed.
    InvalidDecimal,
    /// A well-formed decimal string that is not in the canonical form of doc 05 §3.2.
    NonCanonicalDecimal,
    /// A decimal value outside the `i64` range of thousandths.
    DecimalOverflow,
}

impl fmt::Display for UnitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UnitsError::IntegerOutOfRange => "integer outside the JSON-safe range ±(2^53 - 1)",
            UnitsError::ZeroGrid => "grid step must be positive",
            UnitsError::GridTooFine => "grid step must be at least 1 nm (num >= den)",
            UnitsError::UnreducedGrid => "grid step must be in lowest terms",
            UnitsError::NotUnitQuaternion => "quaternion is not a unit quaternion",
            UnitsError::NotSignNormalized => {
                "quaternion is not sign-normalised (w > 0, else the first non-zero component > 0)"
            }
            UnitsError::InvalidDecimal => "malformed decimal string",
            UnitsError::NonCanonicalDecimal => {
                "decimal string is not canonical (at most 3 fraction digits, no trailing zeros, no '+', \
                 no '-0', no leading zeros)"
            }
            UnitsError::DecimalOverflow => "decimal value out of range",
        })
    }
}

impl std::error::Error for UnitsError {}
