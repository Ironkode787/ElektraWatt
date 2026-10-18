// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `MlText`: multilingual text, either language-neutral or a set of language strings.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{LangTag, TextError, is_nfc, to_nfc};

/// A translatable text: **either** one language-neutral string **or** a set of strings keyed by
/// [`LangTag`], never both (ADR-0005, FR-I18N-001). This is EPLAN's `MultiLangString` semantics.
///
/// - Setting a neutral text removes all language strings ([`set_neutral`](MlText::set_neutral)).
/// - Adding a language string removes the neutral text ([`set_lang`](MlText::set_lang)); to keep it as
///   the source-language string, call [`promote_neutral`](MlText::promote_neutral) first, which is what
///   the "keep the neutral text?" prompt of FR-I18N-001 does.
/// - The display language is a query key ([`text_for`](MlText::text_for)); rendered text is never stored.
///
/// # Invariants
///
/// All text is NFC (setters normalise). A language set is never empty and holds no empty string: removing
/// the last language, or setting a language to `""`, leaves the empty neutral text, which is the
/// [`Default`] and the schema default that record writers omit ([`is_empty`](MlText::is_empty)). The
/// representation is private so these invariants hold; [`view`](MlText::view) exposes it for matching.
///
/// # JSON (doc 05 §3.2)
///
/// A string for neutral text, or an object whose keys are canonical language tags in ascending byte order
/// and whose values are non-empty strings: `"1+"`, `{"en-GB":"Power Supply 1 - Main Current"}`.
/// Deserialisation is strict and rejects non-NFC text, non-canonical or unsorted or duplicate keys, empty
/// language strings, `{}` and every other JSON type.
///
/// ```
/// use ew_mltext::{LangTag, MlText};
///
/// let en: LangTag = "en-GB".parse().unwrap();
/// let nl: LangTag = "nl-NL".parse().unwrap();
///
/// let mut t = MlText::neutral("Power Supply 1");
/// t.promote_neutral(&en);
/// t.set_lang(nl.clone(), "Voeding 1");
/// assert_eq!(t.text_for(&nl), Some("Voeding 1"));
/// assert_eq!(
///     serde_json::to_string(&t).unwrap(),
///     r#"{"en-GB":"Power Supply 1","nl-NL":"Voeding 1"}"#
/// );
///
/// t.set_neutral("1+");
/// assert_eq!(t.languages().count(), 0);
/// assert_eq!(serde_json::to_string(&t).unwrap(), "\"1+\"");
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct MlText(Repr);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Repr {
    Neutral(String),
    Langs(BTreeMap<LangTag, String>),
}

/// A borrowed view of an [`MlText`] for matching on its two forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MlTextView<'a> {
    /// Language-neutral text (possibly empty).
    Neutral(&'a str),
    /// One or more non-empty language strings, in tag order.
    Langs(&'a BTreeMap<LangTag, String>),
}

impl Default for MlText {
    fn default() -> MlText {
        MlText::new()
    }
}

impl MlText {
    /// The empty neutral text.
    pub const fn new() -> MlText {
        MlText(Repr::Neutral(String::new()))
    }

    /// Neutral text, NFC-normalised.
    pub fn neutral(text: &str) -> MlText {
        MlText(Repr::Neutral(to_nfc(text).into_owned()))
    }

    /// A text in one language, NFC-normalised; an empty `text` gives the empty neutral text.
    pub fn with_lang(tag: LangTag, text: &str) -> MlText {
        let mut t = MlText::new();
        t.set_lang(tag, text);
        t
    }

    /// Language strings from pairs, NFC-normalised. Later pairs replace earlier ones with the same tag;
    /// empty strings are skipped. No pairs, or only empty strings, give the empty neutral text.
    pub fn from_langs<I, S>(pairs: I) -> MlText
    where
        I: IntoIterator<Item = (LangTag, S)>,
        S: AsRef<str>,
    {
        let mut t = MlText::new();
        for (tag, text) in pairs {
            t.set_lang(tag, text.as_ref());
        }
        t
    }

    /// The representation, for matching.
    pub fn view(&self) -> MlTextView<'_> {
        match &self.0 {
            Repr::Neutral(s) => MlTextView::Neutral(s),
            Repr::Langs(m) => MlTextView::Langs(m),
        }
    }

    /// Whether this is the empty neutral text (the schema default).
    pub fn is_empty(&self) -> bool {
        matches!(&self.0, Repr::Neutral(s) if s.is_empty())
    }

    /// Whether this is neutral text (empty or not).
    pub fn is_neutral(&self) -> bool {
        matches!(self.0, Repr::Neutral(_))
    }

    /// The neutral text, if this is neutral.
    pub fn as_neutral(&self) -> Option<&str> {
        match &self.0 {
            Repr::Neutral(s) => Some(s),
            Repr::Langs(_) => None,
        }
    }

    /// The string of exactly language `tag`, if present. Neutral text has no language strings.
    pub fn get(&self, tag: &LangTag) -> Option<&str> {
        match &self.0 {
            Repr::Neutral(_) => None,
            Repr::Langs(m) => m.get(tag).map(String::as_str),
        }
    }

    /// The text to show in display language `tag`: the neutral text, or the string of exactly that
    /// language. Fallback chains between languages are the query layer's choice, not this type's.
    pub fn text_for(&self, tag: &LangTag) -> Option<&str> {
        match &self.0 {
            Repr::Neutral(s) => Some(s),
            Repr::Langs(m) => m.get(tag).map(String::as_str),
        }
    }

    /// The languages present, in tag order; none for neutral text.
    pub fn languages(&self) -> impl Iterator<Item = &LangTag> {
        match &self.0 {
            Repr::Neutral(_) => None,
            Repr::Langs(m) => Some(m.keys()),
        }
        .into_iter()
        .flatten()
    }

    /// Makes this neutral text `text` (NFC-normalised), removing every language string.
    pub fn set_neutral(&mut self, text: &str) {
        self.0 = Repr::Neutral(to_nfc(text).into_owned());
    }

    /// Sets the string of language `tag` (NFC-normalised), removing a neutral text. An empty `text`
    /// removes that language instead, as [`remove_lang`](MlText::remove_lang) does.
    pub fn set_lang(&mut self, tag: LangTag, text: &str) {
        if text.is_empty() {
            self.remove_lang(&tag);
            return;
        }
        let text = to_nfc(text).into_owned();
        match &mut self.0 {
            Repr::Langs(m) => {
                m.insert(tag, text);
            }
            Repr::Neutral(_) => self.0 = Repr::Langs(BTreeMap::from([(tag, text)])),
        }
    }

    /// Removes the string of language `tag` and returns it. Removing the last language leaves the empty
    /// neutral text.
    pub fn remove_lang(&mut self, tag: &LangTag) -> Option<String> {
        let Repr::Langs(m) = &mut self.0 else {
            return None;
        };
        let removed = m.remove(tag);
        if m.is_empty() {
            self.0 = Repr::Neutral(String::new());
        }
        removed
    }

    /// Turns non-empty neutral text into the string of language `tag` (the project's source language,
    /// FR-I18N-003). Empty neutral text and language sets are left unchanged.
    pub fn promote_neutral(&mut self, tag: &LangTag) {
        if let Repr::Neutral(s) = &mut self.0 {
            if !s.is_empty() {
                let text = std::mem::take(s);
                self.0 = Repr::Langs(BTreeMap::from([(tag.clone(), text)]));
            }
        }
    }
}

impl Serialize for MlText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Repr::Neutral(s) => serializer.serialize_str(s),
            Repr::Langs(m) => {
                let mut map = serializer.serialize_map(Some(m.len()))?;
                for (tag, text) in m {
                    map.serialize_entry(tag.as_str(), text)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for MlText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(MlTextVisitor)
    }
}

struct MlTextVisitor;

impl<'de> Visitor<'de> for MlTextVisitor {
    type Value = MlText;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a neutral text string or an object of language strings keyed by sorted BCP 47 tags")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<MlText, E> {
        if !is_nfc(v) {
            return Err(E::custom(TextError::NotNfc));
        }
        Ok(MlText(Repr::Neutral(v.to_owned())))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<MlText, A::Error> {
        let mut map: BTreeMap<LangTag, String> = BTreeMap::new();
        while let Some(key) = access.next_key::<String>()? {
            let tag: LangTag = key.parse().map_err(de::Error::custom)?;
            if map.last_key_value().is_some_and(|(last, _)| *last >= tag) {
                return Err(de::Error::custom(TextError::UnsortedLanguages));
            }
            let text: String = access.next_value()?;
            if text.is_empty() {
                return Err(de::Error::custom(TextError::EmptyLanguageText));
            }
            if !is_nfc(&text) {
                return Err(de::Error::custom(TextError::NotNfc));
            }
            map.insert(tag, text);
        }
        if map.is_empty() {
            return Err(de::Error::custom(TextError::NoLanguages));
        }
        Ok(MlText(Repr::Langs(map)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn tag(s: &str) -> LangTag {
        s.parse().unwrap()
    }

    fn json(t: &MlText) -> String {
        serde_json::to_string(t).unwrap()
    }

    fn from_json(s: &str) -> Result<MlText, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn mutual_exclusion() {
        let mut t = MlText::neutral("Power Supply");
        assert!(t.is_neutral());
        t.set_lang(tag("nl-NL"), "Voeding");
        assert!(!t.is_neutral());
        assert_eq!(t.as_neutral(), None);
        assert_eq!(t.get(&tag("nl-NL")), Some("Voeding"));
        assert_eq!(t.get(&tag("en-GB")), None);
        t.set_neutral("PSU");
        assert_eq!(t.languages().count(), 0);
        assert_eq!(t.as_neutral(), Some("PSU"));
        assert_eq!(t.get(&tag("nl-NL")), None);
    }

    #[test]
    fn promote_keeps_the_source_text() {
        let mut t = MlText::neutral("Power Supply");
        t.promote_neutral(&tag("en-GB"));
        t.set_lang(tag("nl-NL"), "Voeding");
        assert_eq!(t.get(&tag("en-GB")), Some("Power Supply"));
        assert_eq!(
            t.languages().map(LangTag::as_str).collect::<Vec<_>>(),
            ["en-GB", "nl-NL"]
        );
        // Promoting empty neutral text or a language set changes nothing.
        let mut e = MlText::new();
        e.promote_neutral(&tag("en-GB"));
        assert!(e.is_empty());
        let before = t.clone();
        t.promote_neutral(&tag("de-DE"));
        assert_eq!(t, before);
    }

    #[test]
    fn empty_language_set_collapses_to_empty_neutral() {
        let mut t = MlText::with_lang(tag("en-GB"), "Main");
        assert_eq!(t.remove_lang(&tag("nl-NL")), None);
        assert_eq!(t.remove_lang(&tag("en-GB")), Some("Main".to_owned()));
        assert!(t.is_empty());
        assert_eq!(t, MlText::default());
        let mut t = MlText::with_lang(tag("en-GB"), "Main");
        t.set_lang(tag("en-GB"), "");
        assert!(t.is_empty());
        assert!(MlText::with_lang(tag("en-GB"), "").is_empty());
        assert!(MlText::from_langs([(tag("en-GB"), "")]).is_empty());
        assert!(MlText::neutral("").is_empty());
        assert!(!MlText::neutral(" ").is_empty());
        assert_eq!(MlText::neutral("x").remove_lang(&tag("en-GB")), None);
    }

    #[test]
    fn display_language_query() {
        let neutral = MlText::neutral("1+");
        assert_eq!(neutral.text_for(&tag("de-DE")), Some("1+"));
        let t = MlText::from_langs([(tag("nl-NL"), "Voeding"), (tag("en-GB"), "Power Supply")]);
        assert_eq!(t.text_for(&tag("en-GB")), Some("Power Supply"));
        assert_eq!(t.text_for(&tag("de-DE")), None);
        assert_eq!(t.text_for(&tag("en")), None);
        match t.view() {
            MlTextView::Langs(m) => assert_eq!(m.len(), 2),
            MlTextView::Neutral(_) => panic!("expected languages"),
        }
        assert_eq!(neutral.view(), MlTextView::Neutral("1+"));
    }

    #[test]
    fn setters_normalise_to_nfc() {
        assert_eq!(MlText::neutral("Lu\u{308}fter").as_neutral(), Some("Lüfter"));
        let t = MlText::with_lang(tag("de-DE"), "Lu\u{308}fter");
        assert_eq!(t.get(&tag("de-DE")), Some("Lüfter"));
    }

    #[test]
    fn json_vectors() {
        assert_eq!(json(&MlText::neutral("1+")), "\"1+\"");
        assert_eq!(json(&MlText::new()), "\"\"");
        let t = MlText::from_langs([
            (tag("nl-NL"), "Voeding 1"),
            (tag("en-GB"), "Power Supply 1 - Main Current"),
        ]);
        let s = r#"{"en-GB":"Power Supply 1 - Main Current","nl-NL":"Voeding 1"}"#;
        assert_eq!(json(&t), s);
        assert_eq!(from_json(s).unwrap(), t);
        assert_eq!(from_json("\"1+\"").unwrap(), MlText::neutral("1+"));
        assert_eq!(from_json("\"\"").unwrap(), MlText::new());
        assert_eq!(from_json("\"a\\nb\"").unwrap(), MlText::neutral("a\nb"));
    }

    #[test]
    fn json_rejections() {
        for (bad, why) in [
            (r#"{}"#, "no languages"),
            (r#"{"nl-NL":"b","en-GB":"a"}"#, "unsorted"),
            (r#"{"en-GB":"a","en-GB":"b"}"#, "duplicate"),
            (r#"{"en-gb":"a"}"#, "non-canonical tag"),
            (r#"{"en_GB":"a"}"#, "non-canonical tag"),
            (r#"{"x-foo":"a"}"#, "private use only"),
            (r#"{"en-GB":""}"#, "empty language string"),
            (r#"{"en-GB":"Lüfter"}"#, "not NFC"),
            ("\"Lu\\u0308fter\"", "not NFC"),
            (r#"{"en-GB":1}"#, "number value"),
            (r#"{"en-GB":null}"#, "null value"),
            ("null", "null"),
            ("1", "number"),
            ("true", "bool"),
            (r#"["en-GB","a"]"#, "array"),
        ] {
            assert!(from_json(bad).is_err(), "{why}: {bad}");
        }
    }

    fn arb_tag() -> impl Strategy<Value = LangTag> {
        prop_oneof![
            Just("en-GB"),
            Just("nl-NL"),
            Just("de-DE"),
            Just("de"),
            Just("fr-BE"),
            Just("sr-Latn-RS")
        ]
        .prop_map(tag)
    }

    proptest! {
        #[test]
        fn json_round_trip(neutral in proptest::option::of("\\PC{0,12}"), pairs in proptest::collection::vec((arb_tag(), "\\PC{0,8}"), 0..5)) {
            let t = match neutral {
                Some(s) => MlText::neutral(&s),
                None => MlText::from_langs(pairs),
            };
            let s = json(&t);
            prop_assert_eq!(from_json(&s).unwrap(), t.clone());
            // Invariants: no empty language set and no empty language string.
            if let MlTextView::Langs(m) = t.view() {
                prop_assert!(!m.is_empty());
                prop_assert!(m.values().all(|v| !v.is_empty() && is_nfc(v)));
            }
        }

        /// Any sequence of edits keeps the invariants and never holds both forms.
        #[test]
        fn edits_keep_invariants(ops in proptest::collection::vec((0u8..4, arb_tag(), "[a-z]{0,3}"), 0..20)) {
            let mut t = MlText::new();
            for (op, tg, text) in ops {
                match op {
                    0 => t.set_neutral(&text),
                    1 => t.set_lang(tg, &text),
                    2 => { t.remove_lang(&tg); }
                    _ => t.promote_neutral(&tg),
                }
                match t.view() {
                    MlTextView::Neutral(_) => prop_assert_eq!(t.languages().count(), 0),
                    MlTextView::Langs(m) => {
                        prop_assert!(!m.is_empty());
                        prop_assert!(m.values().all(|v| !v.is_empty()));
                    }
                }
                prop_assert_eq!(from_json(&json(&t)).unwrap(), t.clone());
            }
        }
    }
}
