// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `LetterCode`: an IEC 81346-2 class letter code.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::RdsError;

/// A letter code of 1–3 letters `A`–`Z`: `F`, `FC`, `QMB` (IEC 81346-2; FR-NUM-016 "letter codes
/// restricted to A–Z").
///
/// The type admits every such code; whether a code suits a set (no `I` or `O`, no reserved class letter,
/// EW-RDS-007) is [`IdentifierSet::reserved_letter_findings`](crate::IdentifierSet::reserved_letter_findings).
/// `Ord` is the alphabetical order of the text (`F < FA < FC < G`).
///
/// **JSON:** the string, strictly upper-case.
///
/// ```
/// use ew_rds::LetterCode;
///
/// let fc: LetterCode = "FC".parse().unwrap();
/// assert_eq!(fc.as_str(), "FC");
/// assert_eq!(fc.class_letter(), 'F');
/// assert!("fc".parse::<LetterCode>().is_err());
/// assert!("FCAB".parse::<LetterCode>().is_err());
/// const Q: LetterCode = LetterCode::from_static("Q");
/// assert!(fc < Q); // alphabetical: F… before Q
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LetterCode {
    // ASCII upper-case letters, padded with zero bytes (which sort before every letter).
    bytes: [u8; 3],
}

impl LetterCode {
    /// The letter code `s`.
    ///
    /// # Errors
    ///
    /// [`RdsError::InvalidLetterCode`] unless `s` is 1–3 letters `A`–`Z`.
    pub const fn parse(s: &str) -> Result<LetterCode, RdsError> {
        let b = s.as_bytes();
        if b.is_empty() || b.len() > 3 {
            return Err(RdsError::InvalidLetterCode);
        }
        let mut bytes = [0u8; 3];
        let mut i = 0;
        while i < b.len() {
            if !b[i].is_ascii_uppercase() {
                return Err(RdsError::InvalidLetterCode);
            }
            bytes[i] = b[i];
            i += 1;
        }
        Ok(LetterCode { bytes })
    }

    /// The letter code `s`, for constants and the shipped tables: in a `const` context an invalid code
    /// fails compilation.
    ///
    /// # Panics
    ///
    /// If `s` is not a valid letter code.
    pub const fn from_static(s: &'static str) -> LetterCode {
        match LetterCode::parse(s) {
            Ok(c) => c,
            Err(_) => panic!("invalid letter code"),
        }
    }

    /// The number of letters (1–3).
    pub const fn letter_count(self) -> usize {
        if self.bytes[2] != 0 {
            3
        } else if self.bytes[1] != 0 {
            2
        } else {
            1
        }
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        match std::str::from_utf8(&self.bytes[..self.letter_count()]) {
            Ok(s) => s,
            // The bytes are ASCII letters by construction.
            Err(_) => unreachable!("letter codes are ASCII"),
        }
    }

    /// The first letter, the class: `F` of `FC`.
    pub const fn class_letter(self) -> char {
        self.bytes[0] as char
    }

    /// The letters.
    pub fn letters(self) -> impl Iterator<Item = char> {
        self.bytes.into_iter().take(self.letter_count()).map(char::from)
    }
}

impl fmt::Debug for LetterCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LetterCode({:?})", self.as_str())
    }
}

impl fmt::Display for LetterCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LetterCode {
    type Err = RdsError;

    fn from_str(s: &str) -> Result<LetterCode, RdsError> {
        LetterCode::parse(s)
    }
}

impl Serialize for LetterCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for LetterCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(LetterCodeVisitor)
    }
}

struct LetterCodeVisitor;

impl Visitor<'_> for LetterCodeVisitor {
    type Value = LetterCode;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a letter code of 1 to 3 letters A-Z")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<LetterCode, E> {
        v.parse().map_err(E::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn vectors() {
        for s in ["A", "F", "FC", "QMB", "ZZZ"] {
            let c: LetterCode = s.parse().unwrap();
            assert_eq!(c.as_str(), s);
            assert_eq!(c.to_string(), s);
            assert_eq!(c.letter_count(), s.len());
            assert_eq!(c.letters().collect::<String>(), s);
        }
        for s in ["", "f", "Fc", "FCAB", "F1", "Ä", "F ", " F", "-F"] {
            assert_eq!(s.parse::<LetterCode>(), Err(RdsError::InvalidLetterCode), "{s:?}");
        }
        assert_eq!(
            format!("{:?}", LetterCode::from_static("FC")),
            "LetterCode(\"FC\")"
        );
    }

    #[test]
    fn order_is_alphabetical() {
        let mut v: Vec<LetterCode> = ["G", "FC", "F", "FA", "QMB", "QM"]
            .iter()
            .map(|s| s.parse().unwrap())
            .collect();
        v.sort();
        let s: Vec<&str> = v.iter().map(LetterCode::as_str).collect();
        assert_eq!(s, ["F", "FA", "FC", "G", "QM", "QMB"]);
    }

    #[test]
    fn json_encoding() {
        let c = LetterCode::from_static("BT");
        assert_eq!(serde_json::to_string(&c).unwrap(), "\"BT\"");
        assert_eq!(serde_json::from_str::<LetterCode>("\"BT\"").unwrap(), c);
        for bad in ["\"bt\"", "\"\"", "\"ABCD\"", "1", "null"] {
            assert!(serde_json::from_str::<LetterCode>(bad).is_err(), "{bad}");
        }
    }

    proptest! {
        #[test]
        fn round_trip(s in "[A-Z]{1,3}") {
            let c: LetterCode = s.parse().unwrap();
            prop_assert_eq!(c.as_str(), s.as_str());
        }

        #[test]
        fn order_matches_text(a in "[A-Z]{1,3}", b in "[A-Z]{1,3}") {
            let (ca, cb): (LetterCode, LetterCode) = (a.parse().unwrap(), b.parse().unwrap());
            prop_assert_eq!(ca.cmp(&cb), a.cmp(&b));
        }
    }
}
