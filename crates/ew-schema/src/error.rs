// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The error type of the value encodings and the record envelope.

use std::fmt;

/// A violation of the canonical encoding (doc 05 §3.1–§3.2) found while reading or building a value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SchemaError {
    /// A number with a fraction or an exponent (EW-CORE-007: there are no floats anywhere).
    Float,
    /// An integer outside ±(2⁵³ − 1).
    IntegerOutOfRange,
    /// A rectangle whose second corner is smaller than its first on some axis.
    ReversedRect,
    /// A date that is not a valid `YYYY-MM-DD` calendar date.
    InvalidDate,
    /// A hash that is not `sha256:` followed by 64 lower-case hexadecimal digits.
    InvalidHash,
    /// A colour specification without codes or with more than two codes.
    ColourCount,
    /// A record or library document without a string `$k` member.
    MissingKind,
    /// A `$k` that names no implemented kind where one is required.
    UnknownKind(String),
    /// A `$k` of the wrong family: a library kind in a JSON Lines record or a record kind in a library
    /// document.
    WrongFamily(String),
    /// A `$v` this reader cannot read: migrations run before typed deserialisation (ADR-0011 decision 3).
    UnsupportedVersion {
        /// The `$k` of the record.
        kind: &'static str,
        /// The version in the file.
        found: u16,
        /// The version this reader implements.
        expected: u16,
    },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::Float => f.write_str("floats are not allowed (EW-CORE-007)"),
            SchemaError::IntegerOutOfRange => f.write_str("integer outside ±(2^53 - 1)"),
            SchemaError::ReversedRect => {
                f.write_str("rectangle corners reversed: the minimum corner comes first")
            }
            SchemaError::InvalidDate => f.write_str("not a valid YYYY-MM-DD date"),
            SchemaError::InvalidHash => {
                f.write_str("not a hash of the form sha256:<64 lower-case hex digits>")
            }
            SchemaError::ColourCount => f.write_str("a colour specification has one or two codes"),
            SchemaError::MissingKind => f.write_str("missing string member \"$k\""),
            SchemaError::UnknownKind(k) => write!(f, "unknown kind \"{k}\""),
            SchemaError::WrongFamily(k) => write!(f, "kind \"{k}\" does not belong here"),
            SchemaError::UnsupportedVersion {
                kind,
                found,
                expected,
            } => write!(
                f,
                "kind \"{kind}\" has $v {found}, this reader implements $v {expected}; migrate first"
            ),
        }
    }
}

impl std::error::Error for SchemaError {}
