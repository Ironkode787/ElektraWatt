// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `Ident`: an identifier, an NFC string that is never translated.

use std::borrow::{Borrow, Cow};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{IdentFinding, NaturalKey, TextError, ident_findings, is_nfc, natural_key, to_nfc};

/// An identifier: a tag, folio label, CP or terminal designation, part number, wire number, PLC symbol or
/// process tag (ADR-0005, domain model §3.3). Identifiers are **never translated** (FR-I18N-002).
///
/// # Invariants
///
/// Non-empty, in Unicode Normalization Form C, without leading or trailing white space and without
/// control characters (so always one line). Interior spaces are allowed: `PT 2.5-TWIN` is a part number.
/// An absent identifier is `None`, never `""`.
///
/// # Construction
///
/// - [`from_input`](Ident::from_input) for typed, pasted and imported text: trims surrounding white space
///   and NFC-normalises first ("NFC-normalised on input", FR-NUM-016).
/// - [`FromStr`] and deserialisation are strict: the text must already satisfy the invariants.
///
/// # Comparison
///
/// `Eq`, `Ord` and `Hash` compare the code points (byte-wise), which keeps maps deterministic and makes a
/// tag with a Cyrillic `А` a different identifier from the Latin one (FR-NUM-016). People see the
/// [natural order](Ident::natural_cmp) (FR-NUM-018); uniqueness and search use
/// [`comparison_key`](Ident::comparison_key) with the project's case setting (FR-NUM-017). Hygiene
/// [`findings`](Ident::findings) flag confusable and mixed-script text.
///
/// **JSON:** the string (doc 05 §3.2).
///
/// ```
/// use ew_mltext::Ident;
///
/// let tag = Ident::from_input("  -101F1 ").unwrap();
/// assert_eq!(tag.as_str(), "-101F1");
/// assert!(" -101F1".parse::<Ident>().is_err());
/// assert!(tag.findings().is_empty());
/// assert_eq!(serde_json::to_string(&tag).unwrap(), "\"-101F1\"");
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Ident(Box<str>);

/// Whether identifier comparison distinguishes letter case: one project-wide setting (FR-NUM-017).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum CaseSensitivity {
    /// `-k1` and `-K1` are different identifiers.
    #[default]
    Sensitive,
    /// `-k1` and `-K1` are the same identifier for uniqueness and search.
    Insensitive,
}

fn validate(s: &str) -> Result<(), TextError> {
    if s.is_empty() {
        Err(TextError::EmptyIdent)
    } else if s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace) {
        Err(TextError::SurroundingWhitespace)
    } else if s.chars().any(char::is_control) {
        Err(TextError::ControlCharacter)
    } else if !is_nfc(s) {
        Err(TextError::NotNfc)
    } else {
        Ok(())
    }
}

/// `s` with every character lower-cased by Unicode's default (locale-independent) mapping.
fn lower(s: &str) -> String {
    s.chars().flat_map(char::to_lowercase).collect()
}

impl Ident {
    /// An identifier from typed, pasted or imported text: surrounding white space is removed and the rest
    /// is NFC-normalised.
    ///
    /// # Errors
    ///
    /// [`TextError::EmptyIdent`] if nothing remains, [`TextError::ControlCharacter`] for a control
    /// character inside (a line break or tab).
    pub fn from_input(s: &str) -> Result<Ident, TextError> {
        let nfc = to_nfc(s.trim());
        validate(&nfc)?;
        Ok(Ident(nfc.into_owned().into_boxed_str()))
    }

    /// The text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The natural-sort key ([`natural_key`]).
    pub fn natural_key(&self) -> NaturalKey {
        natural_key(&self.0)
    }

    /// `self` against `other` in natural order: `X2 < X10` (FR-NUM-018).
    ///
    /// ```
    /// use std::cmp::Ordering;
    /// use ew_mltext::Ident;
    ///
    /// let a: Ident = "X2".parse().unwrap();
    /// let b: Ident = "X10".parse().unwrap();
    /// assert_eq!(a.natural_cmp(&b), Ordering::Less);
    /// assert_eq!(a.cmp(&b), Ordering::Greater); // the code-point order of maps
    /// ```
    pub fn natural_cmp(&self, other: &Ident) -> Ordering {
        self.natural_key().cmp(&other.natural_key())
    }

    /// The key that uniqueness checks and search compare under the project's case setting
    /// (FR-NUM-017): the text itself, or its locale-independent lower-case form, so the Turkish setting
    /// of an OS changes nothing (NFR-I18N-03).
    ///
    /// ```
    /// use ew_mltext::{CaseSensitivity, Ident};
    ///
    /// let a: Ident = "-k1".parse().unwrap();
    /// let b: Ident = "-K1".parse().unwrap();
    /// assert_ne!(a.comparison_key(CaseSensitivity::Sensitive), b.comparison_key(CaseSensitivity::Sensitive));
    /// assert_eq!(a.comparison_key(CaseSensitivity::Insensitive), b.comparison_key(CaseSensitivity::Insensitive));
    /// ```
    pub fn comparison_key(&self, case: CaseSensitivity) -> Cow<'_, str> {
        match case {
            CaseSensitivity::Sensitive => Cow::Borrowed(&self.0),
            CaseSensitivity::Insensitive => Cow::Owned(lower(&self.0)),
        }
    }

    /// The hygiene findings ([`ident_findings`]): confusable, mixed-script and restricted characters.
    pub fn findings(&self) -> Vec<IdentFinding> {
        ident_findings(&self.0)
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Ident {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl FromStr for Ident {
    type Err = TextError;

    /// Strict parsing: the text must already be a valid identifier.
    ///
    /// # Errors
    ///
    /// [`TextError::EmptyIdent`], [`TextError::SurroundingWhitespace`], [`TextError::ControlCharacter`]
    /// or [`TextError::NotNfc`].
    fn from_str(s: &str) -> Result<Ident, TextError> {
        validate(s)?;
        Ok(Ident(s.into()))
    }
}

impl Serialize for Ident {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Ident {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(IdentVisitor)
    }
}

struct IdentVisitor;

impl Visitor<'_> for IdentVisitor {
    type Value = Ident;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a non-empty NFC identifier string without surrounding white space")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Ident, E> {
        v.parse().map_err(E::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    fn strict(s: &str) -> Result<Ident, TextError> {
        s.parse()
    }

    #[test]
    fn valid_identifiers() {
        for s in [
            "-101F1",
            "A9F78116",
            "PT 2.5-TWIN",
            "Q0.1",
            "TA05-V-15",
            "1+",
            "Ü",
            "测试",
            "x",
        ] {
            assert_eq!(strict(s).unwrap().as_str(), s);
            assert_eq!(Ident::from_input(s).unwrap().as_str(), s);
        }
    }

    #[test]
    fn strict_rejections() {
        for (s, e) in [
            ("", TextError::EmptyIdent),
            (" X1", TextError::SurroundingWhitespace),
            ("X1 ", TextError::SurroundingWhitespace),
            ("X1\u{a0}", TextError::SurroundingWhitespace),
            ("X1\n2", TextError::ControlCharacter),
            ("X1\t2", TextError::ControlCharacter),
            ("X\u{0}", TextError::ControlCharacter),
            ("U\u{308}1", TextError::NotNfc),
        ] {
            assert_eq!(strict(s), Err(e), "{s:?}");
        }
    }

    #[test]
    fn input_normalisation() {
        assert_eq!(Ident::from_input("\t-101F1 \n").unwrap().as_str(), "-101F1");
        assert_eq!(Ident::from_input("U\u{308}1").unwrap().as_str(), "Ü1");
        assert_eq!(Ident::from_input(" \u{a0} "), Err(TextError::EmptyIdent));
        assert_eq!(Ident::from_input("X1\n2"), Err(TextError::ControlCharacter));
    }

    #[test]
    fn identity_is_code_points() {
        // A Cyrillic А is a different identifier, and flagged.
        let latin = strict("-KA1").unwrap();
        let cyrillic = strict("-K\u{410}1").unwrap();
        assert_ne!(latin, cyrillic);
        assert!(latin.findings().is_empty());
        assert!(cyrillic.findings().contains(&IdentFinding::MixedScript));
        // Borrow<str> lookups.
        let mut map = BTreeMap::new();
        map.insert(latin.clone(), 1);
        assert_eq!(map.get("-KA1"), Some(&1));
    }

    #[test]
    fn case_setting() {
        let a = strict("-k1").unwrap();
        let b = strict("-K1").unwrap();
        assert_ne!(
            a.comparison_key(CaseSensitivity::Sensitive),
            b.comparison_key(CaseSensitivity::Sensitive)
        );
        assert_eq!(a.comparison_key(CaseSensitivity::Insensitive), "-k1");
        assert_eq!(b.comparison_key(CaseSensitivity::Insensitive), "-k1");
        // Locale-independent: I is i (no dotless ı) under every OS locale.
        assert_eq!(
            strict("-I1")
                .unwrap()
                .comparison_key(CaseSensitivity::Insensitive),
            "-i1"
        );
        assert_eq!(CaseSensitivity::default(), CaseSensitivity::Sensitive);
    }

    #[test]
    fn json_encoding() {
        let id = strict("A9F78116").unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"A9F78116\"");
        assert_eq!(serde_json::from_str::<Ident>("\"A9F78116\"").unwrap(), id);
        for bad in [
            "\"\"",
            "\" A1\"",
            "\"A1 \"",
            "\"U\\u0308\"",
            "\"a\\nb\"",
            "1",
            "null",
            "[\"A\"]",
        ] {
            assert!(serde_json::from_str::<Ident>(bad).is_err(), "{bad}");
        }
    }

    proptest! {
        #[test]
        fn input_then_strict_round_trip(s in "\\PC{0,16}") {
            if let Ok(id) = Ident::from_input(&s) {
                prop_assert_eq!(strict(id.as_str()), Ok(id.clone()));
                let json = serde_json::to_string(&id).unwrap();
                prop_assert_eq!(serde_json::from_str::<Ident>(&json).unwrap(), id);
            }
        }
    }
}
