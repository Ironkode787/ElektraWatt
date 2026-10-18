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
    /// Text that is not one JSON value (EW-CORE-005).
    Json {
        /// The parser's message, with line and column.
        message: String,
    },
    /// A member name repeated within one object (doc 05 §3.6).
    DuplicateMember {
        /// The JSON Pointer of the object.
        path: String,
        /// The repeated name.
        key: String,
    },
    /// A set or keyed list out of order (doc 05 §3.2: sets sorted by element JSON, keyed lists by key).
    UnsortedList {
        /// The JSON Pointer of the first element out of order.
        path: String,
    },
    /// A set element or a keyed-list key that occurs twice.
    DuplicateKey {
        /// The JSON Pointer of the second occurrence.
        path: String,
        /// The element's JSON text or the key.
        key: String,
    },
    /// A value whose tag names a known variant but whose payload does not parse (EW-CORE-017,
    /// ADR-0011 amendment 2). Strict readers fail with it; `Record::load` keeps the value as `Unknown`.
    MalformedVariant(MalformedVariant),
    /// Any other violation at a path (EW-CORE-007): a wrong type, a missing member, a bad value.
    Invalid {
        /// The JSON Pointer of the value.
        path: String,
        /// What is wrong.
        message: String,
    },
    /// A company colour code that is one of the IEC 60757 codes, which would re-read as that code.
    CompanyCodeIsIec(String),
}

/// A value whose tag names a variant this reader knows but whose payload does not parse (doc 05 §6.3,
/// EW-CORE-017). Within a major a known variant's payload never changes, so such a value is damage rather
/// than data of a newer format. `Record::load` keeps it verbatim as `Unknown` and returns it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MalformedVariant {
    /// The JSON Pointer of the enum value, relative to the record or document.
    pub path: String,
    /// The variant name the tag names.
    pub variant: String,
    /// Why the payload does not parse.
    pub message: String,
}

impl SchemaError {
    /// The JSON Pointer the error refers to, for the errors that carry one.
    pub fn path(&self) -> Option<&str> {
        match self {
            SchemaError::DuplicateMember { path, .. }
            | SchemaError::UnsortedList { path }
            | SchemaError::DuplicateKey { path, .. }
            | SchemaError::Invalid { path, .. }
            | SchemaError::MalformedVariant(MalformedVariant { path, .. }) => Some(path),
            _ => None,
        }
    }
}

impl fmt::Display for MalformedVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "malformed value of the known variant \"{}\" at \"{}\": {}",
            self.variant, self.path, self.message
        )
    }
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
            SchemaError::Json { message } => write!(f, "not valid JSON: {message}"),
            SchemaError::DuplicateMember { path, key } => {
                write!(f, "duplicate member \"{key}\" in the object at \"{path}\"")
            }
            SchemaError::UnsortedList { path } => write!(
                f,
                "element at \"{path}\" is out of order (sets and keyed lists are sorted)"
            ),
            SchemaError::DuplicateKey { path, key } => {
                write!(f, "duplicate set element or key {key} at \"{path}\"")
            }
            SchemaError::MalformedVariant(m) => m.fmt(f),
            SchemaError::Invalid { path, message } => write!(f, "{message} at \"{path}\""),
            SchemaError::CompanyCodeIsIec(code) => {
                write!(f, "company colour code \"{code}\" is an IEC 60757 code")
            }
        }
    }
}

impl std::error::Error for SchemaError {}
