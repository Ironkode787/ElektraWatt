// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `LangTag`: a BCP 47 language tag, checked for well-formedness without a registry.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::TextError;

/// A BCP 47 language tag such as `nl-NL`, `de-DE`, `en-GB` or `sr-Latn-RS` (ADR-0005, FR-I18N-003).
///
/// The tag keys the language strings of an [`MlText`](crate::MlText) and is the display-language key of
/// queries. It is held in canonical form, so `Eq`, `Ord` and `Hash` work on the canonical text and `Ord`
/// is the byte order that sorts the keys of the JSON object (doc 05 §3.2).
///
/// # Accepted tags
///
/// The `langtag` production of RFC 5646 §2.1, checked for **well-formedness only**: no subtag registry is
/// consulted, so an unregistered tag such as `qq-QQ` is accepted, while `nl-NL1` (a region of three
/// characters must be digits) is not.
///
/// - language: 2–3 letters with up to three 3-letter extended-language subtags, or 4–8 letters;
/// - script: 4 letters; region: 2 letters or 3 digits;
/// - variants: 5–8 alphanumerics, or a digit and 3 alphanumerics, each at most once;
/// - extensions: a singleton other than `x` followed by subtags of 2–8 alphanumerics, each singleton at
///   most once;
/// - private use: `x` followed by subtags of 1–8 alphanumerics.
///
/// Rejected besides malformed input: grandfathered tags that the `langtag` production does not cover
/// (`i-klingon`, `en-GB-oed`) and private-use-only tags (`x-foo`), because a language string needs a
/// language.
///
/// # Canonical text form
///
/// Subtags joined by `-`; the language and extended languages in lower case, the script in title case,
/// the region in upper case, and everything else in lower case (the case conventions of RFC 5646 §2.1.1).
/// [`FromStr`] and deserialisation accept **only** this form; [`parse_lenient`](LangTag::parse_lenient)
/// also accepts any letter case and `_` as separator, as in EPLAN's `de_DE` (FR-I18N-008).
///
/// **JSON:** the canonical string.
///
/// ```
/// use ew_mltext::LangTag;
///
/// let nl: LangTag = "nl-NL".parse().unwrap();
/// assert_eq!(nl.language(), "nl");
/// assert_eq!(nl.region(), Some("NL"));
/// assert!("nl-nl".parse::<LangTag>().is_err());
/// assert_eq!(LangTag::parse_lenient("de_de").unwrap().as_str(), "de-DE");
/// assert_eq!(serde_json::to_string(&nl).unwrap(), "\"nl-NL\"");
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct LangTag(Box<str>);

fn is_alpha(t: &str) -> bool {
    t.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_digit(t: &str) -> bool {
    t.bytes().all(|b| b.is_ascii_digit())
}

fn is_extlang(t: &str) -> bool {
    t.len() == 3 && is_alpha(t)
}

fn is_script(t: &str) -> bool {
    t.len() == 4 && is_alpha(t)
}

fn is_region(t: &str) -> bool {
    (t.len() == 2 && is_alpha(t)) || (t.len() == 3 && is_digit(t))
}

fn is_variant(t: &str) -> bool {
    (5..=8).contains(&t.len()) || (t.len() == 4 && t.as_bytes()[0].is_ascii_digit())
}

fn title_case(t: &str) -> String {
    let mut s = t.to_ascii_lowercase();
    s[..1].make_ascii_uppercase();
    s
}

/// The canonical form of a well-formed tag whose subtags are separated by `-` (or also `_` when
/// `lenient`), or `None`.
fn canonical(s: &str, lenient: bool) -> Option<String> {
    let subtags: Vec<&str> = if lenient {
        s.split(['-', '_']).collect()
    } else {
        s.split('-').collect()
    };
    if subtags
        .iter()
        .any(|t| t.is_empty() || t.len() > 8 || !t.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return None;
    }
    let n = subtags.len();
    let mut out: Vec<String> = Vec::with_capacity(n);

    // Language. A single letter (`x-…` private use, `i-…` grandfathered) is not a language.
    let lang = subtags[0];
    if !is_alpha(lang) || lang.len() < 2 {
        return None;
    }
    out.push(lang.to_ascii_lowercase());
    let mut i = 1;
    if lang.len() <= 3 {
        let mut extlangs = 0;
        while extlangs < 3 && i < n && is_extlang(subtags[i]) {
            out.push(subtags[i].to_ascii_lowercase());
            i += 1;
            extlangs += 1;
        }
    }
    if i < n && is_script(subtags[i]) {
        out.push(title_case(subtags[i]));
        i += 1;
    }
    if i < n && is_region(subtags[i]) {
        out.push(subtags[i].to_ascii_uppercase());
        i += 1;
    }
    let first_variant = out.len();
    while i < n && is_variant(subtags[i]) {
        let v = subtags[i].to_ascii_lowercase();
        if out[first_variant..].contains(&v) {
            return None;
        }
        out.push(v);
        i += 1;
    }
    let mut singletons: Vec<String> = Vec::new();
    while i < n && subtags[i].len() == 1 && !subtags[i].eq_ignore_ascii_case("x") {
        let singleton = subtags[i].to_ascii_lowercase();
        if singletons.contains(&singleton) {
            return None;
        }
        singletons.push(singleton.clone());
        out.push(singleton);
        i += 1;
        let start = i;
        while i < n && subtags[i].len() >= 2 {
            out.push(subtags[i].to_ascii_lowercase());
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    if i < n && subtags[i].eq_ignore_ascii_case("x") {
        out.push("x".to_owned());
        i += 1;
        if i == n {
            return None;
        }
        while i < n {
            out.push(subtags[i].to_ascii_lowercase());
            i += 1;
        }
    }
    (i == n).then(|| out.join("-"))
}

impl LangTag {
    /// Lenient parsing for importers: any letter case, and `_` as well as `-` between subtags
    /// (`de_DE`, `EN-gb`). The result is canonical.
    ///
    /// # Errors
    ///
    /// [`TextError::InvalidLangTag`] for a tag that is not well-formed even so.
    pub fn parse_lenient(s: &str) -> Result<LangTag, TextError> {
        canonical(s, true)
            .map(|c| LangTag(c.into_boxed_str()))
            .ok_or(TextError::InvalidLangTag)
    }

    /// The canonical text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The subtags after the language: extended languages, script and region, as (index of script,
    /// index of region) in the `-`-separated canonical text.
    fn positions(&self) -> (Option<usize>, Option<usize>) {
        let subtags: Vec<&str> = self.0.split('-').collect();
        let mut i = 1;
        if subtags[0].len() <= 3 {
            while i < subtags.len() && i <= 3 && is_extlang(subtags[i]) {
                i += 1;
            }
        }
        let script = (i < subtags.len() && is_script(subtags[i])).then_some(i);
        if script.is_some() {
            i += 1;
        }
        let region = (i < subtags.len() && is_region(subtags[i])).then_some(i);
        (script, region)
    }

    fn subtag(&self, index: usize) -> &str {
        self.0.split('-').nth(index).unwrap_or_default()
    }

    /// The primary language subtag: `nl` of `nl-NL`.
    pub fn language(&self) -> &str {
        self.subtag(0)
    }

    /// The script subtag: `Latn` of `sr-Latn-RS`.
    pub fn script(&self) -> Option<&str> {
        self.positions().0.map(|i| self.subtag(i))
    }

    /// The region subtag: `NL` of `nl-NL`, `419` of `es-419`.
    pub fn region(&self) -> Option<&str> {
        self.positions().1.map(|i| self.subtag(i))
    }
}

impl fmt::Display for LangTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for LangTag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for LangTag {
    type Err = TextError;

    /// Strict parsing: accepts only the canonical form.
    ///
    /// # Errors
    ///
    /// [`TextError::NonCanonicalLangTag`] for input that [`LangTag::parse_lenient`] would accept,
    /// [`TextError::InvalidLangTag`] otherwise.
    fn from_str(s: &str) -> Result<LangTag, TextError> {
        match canonical(s, false) {
            Some(c) if c == s => Ok(LangTag(c.into_boxed_str())),
            _ if canonical(s, true).is_some() => Err(TextError::NonCanonicalLangTag),
            _ => Err(TextError::InvalidLangTag),
        }
    }
}

impl Serialize for LangTag {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for LangTag {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(LangTagVisitor)
    }
}

struct LangTagVisitor;

impl Visitor<'_> for LangTagVisitor {
    type Value = LangTag;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a canonical BCP 47 language tag such as \"nl-NL\"")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<LangTag, E> {
        v.parse().map_err(E::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn strict(s: &str) -> Result<LangTag, TextError> {
        s.parse()
    }

    #[test]
    fn canonical_tags_are_accepted() {
        for s in [
            "nl",
            "nl-NL",
            "de-DE",
            "en-GB",
            "en-US",
            "fr-BE",
            "zh-Hans-CN",
            "sr-Latn-RS",
            "es-419",
            "de-CH-1996",
            "sl-rozaj-biske",
            "zh-yue-HK",
            "en-a-bbb-x-a-ccc",
            "de-u-co-phonebk",
            "qq-QQ",
            "abcdefgh",
            "art-lojban",
            "en-x-ew",
        ] {
            let t = strict(s).unwrap_or_else(|e| panic!("{s}: {e}"));
            assert_eq!(t.as_str(), s);
            assert_eq!(t.to_string(), s);
        }
    }

    #[test]
    fn components() {
        let t = strict("sr-Latn-RS").unwrap();
        assert_eq!(
            (t.language(), t.script(), t.region()),
            ("sr", Some("Latn"), Some("RS"))
        );
        let t = strict("es-419").unwrap();
        assert_eq!((t.language(), t.script(), t.region()), ("es", None, Some("419")));
        let t = strict("zh-yue-HK").unwrap();
        assert_eq!((t.language(), t.script(), t.region()), ("zh", None, Some("HK")));
        let t = strict("de-CH-1996").unwrap();
        assert_eq!(t.region(), Some("CH"));
        let t = strict("nl").unwrap();
        assert_eq!((t.language(), t.script(), t.region()), ("nl", None, None));
    }

    #[test]
    fn non_canonical_tags() {
        for (s, c) in [
            ("nl-nl", "nl-NL"),
            ("NL-NL", "nl-NL"),
            ("de_DE", "de-DE"),
            ("en_us", "en-US"),
            ("sr-latn-rs", "sr-Latn-RS"),
            ("ZH-HANS", "zh-Hans"),
            ("de-CH-X-Foo", "de-CH-x-foo"),
            ("EN-A-BBB", "en-a-bbb"),
            // A 3-letter subtag after a 2-letter language is an extended language, lower case.
            ("nl-NLD", "nl-nld"),
        ] {
            assert_eq!(strict(s), Err(TextError::NonCanonicalLangTag), "{s}");
            assert_eq!(LangTag::parse_lenient(s).unwrap().as_str(), c, "{s}");
        }
    }

    #[test]
    fn malformed_tags() {
        for s in [
            "",
            "-",
            "n",
            "nl-",
            "-nl",
            "nl--NL",
            "nl NL",
            " nl",
            "nl-NL1",
            "nl-N",
            "nl-1",
            "nl-12",
            "toolonglang",
            "i-klingon",
            "en-GB-oed",
            "x-foo",
            "x",
            "nl-x",
            "nl-a",
            "nl-a-x-foo",
            "de-1996-1996",
            "en-a-bb-a-cc",
            "nl-Ü",
            "nl-NL!",
            "12-NL",
            "nl-abcdefghi",
            "en-GB-a-b",
        ] {
            assert_eq!(strict(s), Err(TextError::InvalidLangTag), "{s:?}");
            assert_eq!(LangTag::parse_lenient(s), Err(TextError::InvalidLangTag), "{s:?}");
        }
    }

    #[test]
    fn order_is_byte_order_of_canonical_text() {
        let mut tags: Vec<LangTag> = ["nl-NL", "de-DE", "en-GB", "en", "de-AT"]
            .iter()
            .map(|s| strict(s).unwrap())
            .collect();
        tags.sort();
        let sorted: Vec<&str> = tags.iter().map(LangTag::as_str).collect();
        assert_eq!(sorted, ["de-AT", "de-DE", "en", "en-GB", "nl-NL"]);
    }

    #[test]
    fn json_encoding() {
        let t = strict("en-GB").unwrap();
        assert_eq!(serde_json::to_string(&t).unwrap(), "\"en-GB\"");
        assert_eq!(serde_json::from_str::<LangTag>("\"en-GB\"").unwrap(), t);
        for bad in [
            "\"en-gb\"",
            "\"en_GB\"",
            "\"\"",
            "\"x-foo\"",
            "1",
            "null",
            "[\"en\"]",
        ] {
            assert!(serde_json::from_str::<LangTag>(bad).is_err(), "{bad}");
        }
    }

    proptest! {
        /// Lenient parsing yields a canonical tag that strict parsing accepts unchanged.
        #[test]
        fn lenient_is_idempotent_and_canonical(s in "[a-zA-Z]{2,3}([-_][a-zA-Z]{4})?([-_]([a-zA-Z]{2}|[0-9]{3}))?([-_][0-9][a-zA-Z0-9]{3})?") {
            let t = LangTag::parse_lenient(&s).unwrap();
            prop_assert_eq!(strict(t.as_str()), Ok(t.clone()));
            prop_assert_eq!(LangTag::parse_lenient(t.as_str()), Ok(t.clone()));
            prop_assert!(t.as_str().eq_ignore_ascii_case(&s.replace('_', "-")));
        }

        #[test]
        fn never_panics(s in "\\PC{0,20}") {
            let _ = strict(&s);
            let _ = LangTag::parse_lenient(&s);
        }
    }
}
