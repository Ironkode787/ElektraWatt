// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The error type of this crate.

use std::fmt;

/// An invalid value: a rejected constructor argument or a rejected encoded form.
///
/// Serde errors carry this type's [`Display`](fmt::Display) text. Identifier *hygiene* (confusables, mixed
/// scripts) is not an error: [`Ident::findings`](crate::Ident::findings) reports it as data.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum TextError {
    /// A language tag that is not a well-formed BCP 47 `langtag` (RFC 5646 §2.1), or a form this crate
    /// does not accept: grandfathered tags, private-use-only tags, repeated variants or extension singletons.
    InvalidLangTag,
    /// A well-formed language tag that is not in canonical form: subtags joined by `-`, language lower
    /// case, script title case, region upper case, everything else lower case (`nl-NL`, `sr-Latn-RS`).
    NonCanonicalLangTag,
    /// Text that is not in Unicode Normalization Form C.
    NotNfc,
    /// An empty identifier; an absent identifier is `None`, never `""`.
    EmptyIdent,
    /// An identifier with leading or trailing white space.
    SurroundingWhitespace,
    /// An identifier containing a control character (line breaks and tabs included).
    ControlCharacter,
    /// An encoded multilingual text with an empty language string.
    EmptyLanguageText,
    /// An encoded multilingual text object without any language.
    NoLanguages,
    /// An encoded multilingual text object whose language keys are not in strictly ascending byte order
    /// (duplicates included).
    UnsortedLanguages,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TextError::InvalidLangTag => "not a well-formed BCP 47 language tag",
            TextError::NonCanonicalLangTag => {
                "language tag is not canonical (use '-', lower-case language, title-case script, upper-case \
                 region, e.g. \"nl-NL\")"
            }
            TextError::NotNfc => "text is not in Unicode Normalization Form C",
            TextError::EmptyIdent => "identifier is empty",
            TextError::SurroundingWhitespace => "identifier has leading or trailing white space",
            TextError::ControlCharacter => "identifier contains a control character",
            TextError::EmptyLanguageText => "language string is empty",
            TextError::NoLanguages => "multilingual text object has no language",
            TextError::UnsortedLanguages => "language keys are not sorted or not unique",
        })
    }
}

impl std::error::Error for TextError {}
