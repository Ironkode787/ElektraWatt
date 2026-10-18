// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The error types of this crate.

use std::fmt;

/// Why a reference designation does not parse, and where.
///
/// `offset` is a byte offset into the text that was passed to the parser.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ParseError {
    /// What is wrong.
    pub kind: ParseErrorKind,
    /// Byte offset of the problem in the input.
    pub offset: usize,
}

/// The kinds of [`ParseError`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// The text is empty (or only white space).
    Empty,
    /// The text does not start with a prefix sign: `S51` instead of `-S51`.
    MissingPrefix,
    /// A character where a prefix sign, a sub-level separator, `:` or `;` must follow.
    UnexpectedCharacter(char),
    /// A prefix sign or separator without a level after it: `=+L1`, `-K1-`.
    EmptyLevel,
    /// A character the grammar does not allow at this place: strictly, levels are `A`–`Z` and `0`–`9`, and
    /// connection points and signals add `+`, `-` and `.`.
    InvalidCharacter(char),
    /// White space inside the designation (strict: anywhere).
    Whitespace,
    /// Aspects out of their canonical order (`==`, `=`, `++`, `+`, `&`, `#`, `%`, `-`).
    AspectOrder,
    /// An aspect that appears again after another aspect: `=P01+L1=P02`.
    RepeatedAspect,
    /// `:` without a connection point after it.
    EmptyConnectionPoint,
    /// A connection point (`:1`) after a designation without a product aspect.
    ConnectionPointWithoutProduct,
    /// `;` without a signal designation after it.
    EmptySignal,
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorKind::Empty => f.write_str("empty reference designation"),
            ParseErrorKind::MissingPrefix => f.write_str("missing prefix sign (such as '-' or '+')"),
            ParseErrorKind::UnexpectedCharacter(c) => {
                write!(f, "unexpected character {c:?} (U+{:04X})", u32::from(*c))
            }
            ParseErrorKind::EmptyLevel => f.write_str("prefix sign or separator without a level"),
            ParseErrorKind::InvalidCharacter(c) => {
                write!(f, "character {c:?} (U+{:04X}) not allowed here", u32::from(*c))
            }
            ParseErrorKind::Whitespace => f.write_str("white space inside the designation"),
            ParseErrorKind::AspectOrder => f.write_str("aspects out of canonical order"),
            ParseErrorKind::RepeatedAspect => f.write_str("aspect appears more than once"),
            ParseErrorKind::EmptyConnectionPoint => f.write_str("empty connection point after ':'"),
            ParseErrorKind::ConnectionPointWithoutProduct => {
                f.write_str("connection point without a product designation")
            }
            ParseErrorKind::EmptySignal => f.write_str("empty signal designation after ';'"),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.kind, self.offset)
    }
}

impl std::error::Error for ParseError {}

/// An invalid value: a letter code, a syntax setting, a known code or an identifier-set row.
///
/// Serde errors carry this type's [`Display`](fmt::Display) text.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum RdsError {
    /// A letter code that is not 1–3 letters `A`–`Z`.
    InvalidLetterCode,
    /// An empty prefix sign or sub-level separator.
    EmptyToken,
    /// A prefix sign or separator with a character other than ASCII punctuation, or with `:`, `;` or `/`
    /// (fixed separators of connection points, signals and pages).
    InvalidTokenCharacter,
    /// Two aspects with the same prefix sign, or a separator equal to a sign or to another separator.
    DuplicateToken,
    /// An empty node code or alias.
    EmptyCode,
    /// A spelling already registered for another node code of the aspect, or as code and alias at once.
    CodeConflict,
    /// An empty identifier-set key or title, or a key with white space.
    InvalidSetKey,
    /// A category key that is not UpperCamelCase ASCII (`[A-Z][A-Za-z0-9]*`), or an empty or padded group.
    InvalidCategory,
    /// Two rows for the same category and group, or an alternative that repeats a letter code of its row.
    DuplicateMapping,
    /// A letter code that contains `I` or `O`, or starts with a letter its set reserves.
    ReservedLetter,
}

impl fmt::Display for RdsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RdsError::InvalidLetterCode => "letter code must be 1 to 3 letters A-Z",
            RdsError::EmptyToken => "prefix sign or separator is empty",
            RdsError::InvalidTokenCharacter => {
                "prefix signs and separators are ASCII punctuation other than ':', ';' and '/'"
            }
            RdsError::DuplicateToken => "prefix sign or separator used twice",
            RdsError::EmptyCode => "node code or alias is empty",
            RdsError::CodeConflict => "spelling already names another node code of this aspect",
            RdsError::InvalidSetKey => "identifier-set key or title is empty or malformed",
            RdsError::InvalidCategory => "category key must be UpperCamelCase ASCII, group non-empty",
            RdsError::DuplicateMapping => "duplicate category row or letter code",
            RdsError::ReservedLetter => "letter code uses I, O or a letter reserved by its set",
        })
    }
}

impl std::error::Error for RdsError {}
