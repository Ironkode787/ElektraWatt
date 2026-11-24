// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule messages in Fluent (doc 15 §3.8; ADR-0019 as amended, ADR-0024 for the dependency).
//!
//! One resource per language, `texts/<lang>/rules.ftl`, compiled into the binary. Every registered rule has the
//! message `rule-<id lower case>` (Fluent identifiers hold no dots) with the attributes `.title` and `.hint`; a rule
//! whose findings carry typed arguments ([`Finding::args`]) also has the message's value, the pattern of its text:
//!
//! ```ftl
//! rule-ew-pot-013 = { $device } switches the neutral { $from } → { $to }{ $tail }.
//!     .title = N switching
//!     .hint = Move the contact into the line conductor, or use a device that opens L and N together.
//! ```
//!
//! **The en-GB text is the finding's message.** [`Finding::message`] stays the en-GB rendering the rule prints, byte
//! for byte (P5-2): the en-GB pattern of a rule with arguments renders exactly that text from the arguments (a test
//! asserts it on every finding with arguments), and [`render`] in en-GB returns the message itself. Another language
//! renders the rule's pattern of that language from the arguments where the pattern exists and resolves without an
//! error, and falls back to the en-GB message otherwise (doc 15 §3.8: en-GB is mandatory and the fallback). A
//! finding's arguments are its printed values (derived tags, names and quantities as text, rendered when the rule
//! ran), so a pattern never formats a number itself: the bytes cannot depend on a locale's number format.
//!
//! The languages are rendered by the one renderer, `ew_l10n::render`, which inserts no Unicode isolation marks: the
//! arguments are tags and names in the project's own text, and isolation marks would break the byte equality of P5-2.
//!
//! Two functions let a pattern of another language word an argument as that language does:
//!
//! - `DECIMAL($x)` writes the decimal points of a printed number (`1.5 mm²`, `40.0`) as the language writes them
//!   (`1,5 mm²` in nl-NL; [`ew_l10n::render::decimal`]); a pattern wraps every argument that holds a measured value,
//!   never a wire number or a version.
//! - `WORD($x)` writes the words a rule prints where a record has no name of its own ("a location", "a device without
//!   designation", "the junction in C11"; each piece of a comma list and each end of a connection `a → b` or a wire
//!   `a–b` alone) in the language: the en-GB resource's messages `word-…` hold
//!   those words, and the language's messages of the same ID its own.

use std::sync::OnceLock;

use ew_l10n::render::{Language, OnError, Value};

use crate::finding::Finding;

/// The language every rule must have a message in, and the fallback of every other (doc 15 §3.8).
pub const DEFAULT_LANGUAGE: &str = "en-GB";

/// The languages the rule messages ship in, the default first.
pub const LANGUAGES: &[&str] = &["en-GB", "nl-NL"];

/// The key of the message of the rule `rule` ([`RuleMeta::message`](crate::RuleMeta::message)): a registered rule's or
/// an integrity rule's of opening a project; `None` for another ID.
fn message_of(rule: &str) -> Option<&'static str> {
    crate::registry::rule(rule)
        .map(|r| r.meta())
        .or_else(|| crate::rules::core::INTEGRITY.iter().find(|m| m.id == rule))
        .map(|m| m.message)
}

/// The language of a shipped tag, built once: its patterns may call `NUMBER(…, type: "ordinal")` (a pin's position),
/// `DECIMAL` and `WORD` ([`words`]). A resource that does not parse is a defect of this crate, caught by the message
/// tests; the entries that parse are still served.
fn bundle(lang: &str) -> Option<&'static Language> {
    static BUNDLES: OnceLock<Vec<Language>> = OnceLock::new();
    let bundles = BUNDLES.get_or_init(|| {
        LANGUAGES
            .iter()
            .map(|tag| {
                Language::new(
                    tag,
                    sources(tag),
                    &[("DECIMAL", ew_l10n::render::decimal), ("WORD", words)],
                    true,
                )
            })
            .collect()
    });
    let i = LANGUAGES.iter().position(|l| l.eq_ignore_ascii_case(lang))?;
    bundles.get(i)
}

/// The text files of the language `tag`.
fn sources(tag: &str) -> impl Iterator<Item = &'static str> {
    crate::SOURCES
        .iter()
        .filter(move |(l, _)| *l == tag)
        .map(|(_, s)| *s)
}

/// The parse errors of the shipped resources, per language (empty when every resource parses): for the tests.
pub fn resource_errors() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (tag, source) in crate::SOURCES {
        for e in ew_l10n::render::parse_errors(source) {
            out.push((*tag, e));
        }
    }
    out
}

/// The arguments a pattern reads as numbers (an ordinal through `NUMBER(…, type: "ordinal")`).
const NUMBER_ARGS: &[&str] = &["cp_nth"];

/// `WORD($x)` in `lang`: each piece of a list, each end of a connection (`a → b`) and of a wire (`a–b`) alone, in the
/// words of `lang` where it is the en-GB text of a message `word-…` ([`word`]), else as it is.
fn words(lang: &str, text: &str) -> String {
    let words = |text: &str, sep: &str, inner: &dyn Fn(&str) -> String| {
        text.split(sep).map(inner).collect::<Vec<_>>().join(sep)
    };
    let end = |e: &str| word(lang, e).unwrap_or_else(|| e.to_owned());
    let wire = |w: &str| word(lang, w).unwrap_or_else(|| words(w, "–", &end));
    let connection = |c: &str| words(c, " → ", &wire);
    words(text, ", ", &connection)
}

/// The words of `lang` for `value` when `value` is the en-GB text of a message `word-…` (`WORD`); else `None`. A word
/// with one argument (`the junction in { $at }`) matches its text around the argument.
fn word(lang: &str, value: &str) -> Option<String> {
    let mut args = Vec::new();
    let id = sources(DEFAULT_LANGUAGE).flat_map(str::lines).find_map(|l| {
        let (id, text) = l.split_once(" = ")?;
        if !id.starts_with("word-") {
            return None;
        }
        if text == value {
            return Some(id);
        }
        let (before, rest) = text.split_once("{ $")?;
        let (name, after) = rest.split_once(" }")?;
        let arg = value.strip_prefix(before)?.strip_suffix(after)?;
        (!arg.is_empty()).then(|| {
            args.push((name, Value::Text(arg)));
            id
        })
    })?;
    bundle(lang)?.format(id, None, &args, OnError::Miss)
}

/// The name of text style `key` in `lang` (falling back to en-GB), as the Text styles page lists it and EW-CORE-061
/// names it: a built-in style by its message `word-style-<key>` (`word-style-tag-main`, "Device tag"), a style a
/// QElectroTech import made (`qet.<size>[.b][.i]`) as imported text of its point size, any other style a project
/// adds as a text style of this project. Never the key itself.
pub fn text_style_name(key: &str, lang: &str) -> String {
    let mut args = Vec::new();
    let id = if let Some(rest) = key.strip_prefix("qet.") {
        let rest = rest.strip_suffix(".i").unwrap_or(rest);
        let (size, bold) = rest.strip_suffix(".b").map_or((rest, false), |s| (s, true));
        args.push(("size", Value::Text(size)));
        if bold {
            "word-style-imported-bold".to_owned()
        } else {
            "word-style-imported".to_owned()
        }
    } else {
        let id = format!("word-style-{}", key.replace('.', "-"));
        match bundle(DEFAULT_LANGUAGE).is_some_and(|b| b.has_message(&id)) {
            true => id,
            false => "word-style-own".to_owned(),
        }
    };
    let lookup = |tag: &str| bundle(tag)?.format(&id, None, &args, OnError::Miss);
    lookup(lang)
        .or_else(|| lookup(DEFAULT_LANGUAGE))
        .unwrap_or_default()
}

/// The arguments of a finding as the pattern's arguments (every value a string, but [`NUMBER_ARGS`] as numbers).
fn fluent_args(f: &Finding) -> Vec<(&str, Value<'_>)> {
    f.text
        .named()
        .iter()
        .filter_map(|(k, v)| match v {
            ew_l10n::TextArg::Plain(v) if NUMBER_ARGS.contains(k) => Some((*k, Value::Number(v.as_str()))),
            ew_l10n::TextArg::Plain(v) => Some((*k, Value::Text(v.as_str()))),
            _ => None,
        })
        .collect()
}

/// The value of the argument `name` of a finding, as printed.
fn plain<'f>(f: &'f Finding, name: &str) -> Option<&'f str> {
    f.text.named().iter().find_map(|(k, v)| match v {
        ew_l10n::TextArg::Plain(v) if *k == name => Some(v.as_str()),
        _ => None,
    })
}

/// The finding's text rendered from the pattern of `lang`, when the language ships, the rule has a pattern in it, the
/// finding carries arguments and the pattern resolves without an error; else `None`.
pub fn format(f: &Finding, lang: &str) -> Option<String> {
    let b = bundle(lang)?;
    let id = f.text.key();
    if f.rule != crate::rules::core::MERGE_COLLISION.id
        && f.rule != crate::rules::pot::TYPE_OR_VOLTAGE_CONFLICT.id
    {
        return b.text(&f.text, OnError::Miss);
    }
    if !b.has(id, None) {
        return None;
    }
    // EW-CORE-013 carries the uniqueness finding it reports by that finding's arguments, its discriminator naming the
    // rule: the finding is worded by that rule's pattern where it has one, else stays the en-GB `collision`.
    let collision = (f.rule == crate::rules::core::MERGE_COLLISION.id)
        .then(|| {
            b.format(
                message_of(&f.discriminator)?,
                None,
                &fluent_args(f),
                OnError::Miss,
            )
        })
        .flatten();
    // EW-POT-001's conflicts, each a sentence worded by its attribute of the rule's message; a piece of another
    // shape, or one that does not resolve, leaves the finding to its en-GB message.
    let mut conflicts = None;
    if f.rule == crate::rules::pot::TYPE_OR_VOLTAGE_CONFLICT.id
        && let Some(list) = plain(f, "conflict_list")
    {
        let worded: Vec<String> = list
            .split('\n')
            .map(|piece| {
                let (attribute, fields) = crate::rules::pot::conflict_words(piece)?;
                let piece_args: Vec<(&str, Value<'_>)> = fields
                    .iter()
                    .map(|(k, v)| (*k, Value::Text(v.as_str())))
                    .collect();
                b.format(id, Some(attribute), &piece_args, OnError::Miss)
            })
            .collect::<Option<_>>()?;
        conflicts = Some(worded.join(" "));
    }
    let mut args = fluent_args(f);
    if let Some(text) = &collision {
        args.push(("collision", Value::Text(text)));
    }
    if let Some(text) = &conflicts {
        args.push(("conflicts", Value::Text(text)));
    }
    b.format(id, None, &args, OnError::Miss)
}

/// The finding's message in `lang` (`ew check --lang`, the message panel): in en-GB the message itself; in another
/// shipped language the rule's pattern of that language rendered from the finding's arguments where present, the
/// en-GB message otherwise (doc 15 §3.8).
pub fn render(f: &Finding, lang: &str) -> String {
    format(f, lang)
        .or_else(|| format(f, DEFAULT_LANGUAGE))
        .or_else(|| bundle(DEFAULT_LANGUAGE)?.text(&f.text, OnError::Keep))
        .unwrap_or_else(|| f.text.to_string())
}

/// A text of the rule messages in `lang` (a prevent rule's finding, which the window shows when the rule refuses an
/// edit): its pattern of that language where it resolves, else en-GB's, else its English as the key table holds it.
pub fn text(t: &ew_l10n::Text, lang: &str) -> String {
    let lookup = |tag: &str| bundle(tag)?.text(t, OnError::Miss);
    lookup(lang)
        .or_else(|| lookup(DEFAULT_LANGUAGE))
        .unwrap_or_else(|| t.to_string())
}

/// One attribute of a rule's message in `lang`, falling back to en-GB; `None` when en-GB lacks it too.
fn attribute(rule: &str, name: &str, lang: &str) -> Option<String> {
    let id = message_of(rule)?;
    let lookup = |tag: &str| bundle(tag)?.format(id, Some(name), &[], OnError::Miss);
    lookup(lang).or_else(|| lookup(DEFAULT_LANGUAGE))
}

/// Whether `lang` itself (no fallback) has the message of `rule` with the attribute `name` (`None`: the message).
pub fn has(rule: &str, name: Option<&str>, lang: &str) -> bool {
    let (Some(b), Some(id)) = (bundle(lang), message_of(rule)) else {
        return false;
    };
    match name {
        None => b.has_message(id),
        Some("value") => b.has(id, None),
        Some(attr) => b.has(id, Some(attr)),
    }
}

/// A quick fix's label in `lang`, falling back to en-GB.
pub fn fix_label(fix: &crate::finding::QuickFix, lang: &str) -> String {
    let lookup = |tag: &str| bundle(tag)?.text(&fix.label, OnError::Miss);
    lookup(lang)
        .or_else(|| lookup(DEFAULT_LANGUAGE))
        .unwrap_or_else(|| fix.label.to_string())
}

/// A rule's title in `lang` (`.title`), falling back to en-GB.
pub fn title(rule: &str, lang: &str) -> Option<String> {
    attribute(rule, "title", lang)
}

/// A rule's hint in `lang` (`.hint`): what to do about a finding, falling back to en-GB.
pub fn hint(rule: &str, lang: &str) -> Option<String> {
    attribute(rule, "hint", lang)
}
