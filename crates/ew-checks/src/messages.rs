// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule messages in Fluent (doc 15 §3.8; ADR-0019 as amended, ADR-0024 for the dependency).
//!
//! One resource per language, `messages/<lang>/rules.ftl`, compiled into the binary. Every registered rule has the
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
//! The bundles do not insert Unicode isolation marks (`set_use_isolating(false)`): the arguments are tags and names
//! in the project's own text, and isolation marks would break the byte equality of P5-2.
//!
//! Two functions let a pattern of another language word an argument as that language does:
//!
//! - `DECIMAL($x)` writes the decimal points of a printed number (`1.5 mm²`, `40.0`) as the language writes them
//!   (`1,5 mm²` in nl-NL); a pattern wraps every argument that holds a measured value, never a wire number or a
//!   version.
//! - `WORD($x)` writes the words a rule prints where a record has no name of its own ("a location", "a device without
//!   designation", "the junction in C11"; each piece of a comma list and each end of a connection `a → b` or a wire
//!   `a–b` alone) in the language: the en-GB resource's messages `word-…` hold
//!   those words, and the language's messages of the same ID its own.

use std::sync::OnceLock;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};

use crate::finding::Finding;

/// The language every rule must have a message in, and the fallback of every other (doc 15 §3.8).
pub const DEFAULT_LANGUAGE: &str = "en-GB";

/// The languages the rule messages ship in, the default first.
pub const LANGUAGES: &[&str] = &["en-GB", "nl-NL"];

/// The Fluent source of each shipped language, in [`LANGUAGES`] order.
const SOURCES: &[&str] = &[
    include_str!("../messages/en-GB/rules.ftl"),
    include_str!("../messages/nl-NL/rules.ftl"),
];

type Bundle = FluentBundle<FluentResource>;

/// The Fluent message ID of a rule: `rule-` and the rule ID in lower case (`rule-ew-pot-013`).
pub fn message_id(rule: &str) -> String {
    format!("rule-{}", rule.to_ascii_lowercase())
}

/// The bundle of a shipped language, built once. A resource that does not parse is a defect of this crate, caught by
/// the message tests; the entries that parse are still served.
fn bundle(lang: &str) -> Option<&'static Bundle> {
    static BUNDLES: OnceLock<Vec<Bundle>> = OnceLock::new();
    let bundles = BUNDLES.get_or_init(|| {
        LANGUAGES
            .iter()
            .zip(SOURCES)
            .map(|(tag, source)| {
                let id: unic_langid::LanguageIdentifier = tag.parse().unwrap_or_default();
                let mut b = Bundle::new_concurrent(vec![id]);
                b.set_use_isolating(false);
                // NUMBER(…, type: "ordinal") of a pin's position
                let _ = b.add_builtins();
                let tag: &'static str = *tag;
                let _ = b.add_function("DECIMAL", move |positional, _| match positional.first() {
                    Some(FluentValue::String(s)) => decimal(tag, s).into(),
                    Some(other) => other.clone(),
                    None => FluentValue::None,
                });
                let _ = b.add_function("WORD", move |positional, _| match positional.first() {
                    Some(FluentValue::String(s)) => {
                        // Each piece of a list, each end of a connection (`a → b`) and of a wire (`a–b`) alone.
                        let words = |text: &str, sep: &str, inner: &dyn Fn(&str) -> String| {
                            text.split(sep).map(inner).collect::<Vec<_>>().join(sep)
                        };
                        let end = |e: &str| word(tag, e).unwrap_or_else(|| e.to_owned());
                        let wire = |w: &str| word(tag, w).unwrap_or_else(|| words(w, "–", &end));
                        let connection = |c: &str| words(c, " → ", &wire);
                        words(s, ", ", &connection).into()
                    }
                    Some(other) => other.clone(),
                    None => FluentValue::None,
                });
                let resource = match FluentResource::try_new((*source).to_owned()) {
                    Ok(r) => r,
                    Err((r, _)) => r,
                };
                // A duplicate entry keeps the first; the message tests reject duplicates.
                let _ = b.add_resource(resource);
                b
            })
            .collect()
    });
    let i = LANGUAGES.iter().position(|l| l.eq_ignore_ascii_case(lang))?;
    bundles.get(i)
}

/// The parse errors of the shipped resources, per language (empty when every resource parses): for the tests.
pub fn resource_errors() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (tag, source) in LANGUAGES.iter().zip(SOURCES) {
        if let Err((_, errors)) = FluentResource::try_new((*source).to_owned()) {
            for e in errors {
                out.push((*tag, format!("{e:?}")));
            }
        }
    }
    out
}

/// The arguments a pattern reads as numbers (an ordinal through `NUMBER(…, type: "ordinal")`).
const NUMBER_ARGS: &[&str] = &["cp_nth"];

/// The shipped languages that write a decimal comma (`1,5 mm²`).
const DECIMAL_COMMA: &[&str] = &["nl-NL"];

/// `text` with each decimal point between two digits written as `lang` writes it (`DECIMAL`).
fn decimal(lang: &str, text: &str) -> String {
    if !DECIMAL_COMMA.iter().any(|l| l.eq_ignore_ascii_case(lang)) {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let digit = |j: Option<usize>| j.and_then(|j| chars.get(j)).is_some_and(char::is_ascii_digit);
            if *c == '.' && digit(i.checked_sub(1)) && digit(Some(i + 1)) {
                ','
            } else {
                *c
            }
        })
        .collect()
}

/// The words of `lang` for `value` when `value` is the en-GB text of a message `word-…` (`WORD`); else `None`. A word
/// with one argument (`the junction in { $at }`) matches its text around the argument.
fn word(lang: &str, value: &str) -> Option<String> {
    let mut args = FluentArgs::new();
    let id = SOURCES[0].lines().find_map(|l| {
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
            args.set(name.to_owned(), arg.to_owned());
            id
        })
    })?;
    let b = bundle(lang)?;
    let pattern = b.get_message(id)?.value()?;
    let mut errors = Vec::new();
    let text = b.format_pattern(pattern, Some(&args), &mut errors);
    errors.is_empty().then(|| text.into_owned())
}

/// The arguments of a finding as Fluent arguments (every value a string, but [`NUMBER_ARGS`] as numbers).
fn fluent_args(f: &Finding) -> FluentArgs<'_> {
    let mut args = FluentArgs::with_capacity(f.args.len());
    for (k, v) in &f.args {
        if NUMBER_ARGS.contains(k) {
            args.set(*k, FluentValue::try_number(v.as_str()));
        } else {
            args.set(*k, v.as_str());
        }
    }
    args
}

/// The finding's text rendered from the pattern of `lang`, when the language ships, the rule has a pattern in it, the
/// finding carries arguments and the pattern resolves without an error; else `None`.
pub fn format(f: &Finding, lang: &str) -> Option<String> {
    if f.args.is_empty() {
        return None;
    }
    let b = bundle(lang)?;
    let id = message_id(f.rule);
    let pattern = b.get_message(&id)?.value()?;
    let mut args = fluent_args(f);
    // EW-CORE-013 carries the uniqueness finding it reports by that finding's arguments, its discriminator naming the
    // rule: the finding is worded by that rule's pattern where it has one, else stays the en-GB `collision`.
    if f.rule == crate::rules::core::MERGE_COLLISION.id {
        if let Some(inner) = b
            .get_message(&message_id(&f.discriminator))
            .and_then(|m| m.value())
        {
            let mut errors = Vec::new();
            let text = b.format_pattern(inner, Some(&args), &mut errors).into_owned();
            if errors.is_empty() {
                args.set("collision", text);
            }
        }
    }
    // EW-POT-001's conflicts, each worded by its attribute of the rule's message where every one has a shape
    // and an attribute, else the en-GB `conflicts`.
    if f.rule == crate::rules::pot::TYPE_OR_VOLTAGE_CONFLICT.id
        && let Some((_, list)) = f.args.iter().find(|(k, _)| *k == "conflict_list")
        && let Some(message) = b.get_message(&id)
    {
        let worded: Option<Vec<String>> = list
            .split('\n')
            .map(|piece| {
                let (attribute, subject, value, applied) = crate::rules::pot::conflict_words(piece)?;
                let pattern = message.get_attribute(attribute)?.value();
                let mut piece_args = FluentArgs::with_capacity(3);
                piece_args.set("subject", subject);
                piece_args.set("value", value);
                piece_args.set("applied", applied);
                let mut errors = Vec::new();
                let text = b
                    .format_pattern(pattern, Some(&piece_args), &mut errors)
                    .into_owned();
                errors.is_empty().then_some(text)
            })
            .collect();
        if let Some(worded) = worded {
            args.set("conflicts", worded.join("; "));
        }
    }
    let mut errors = Vec::new();
    let text = b.format_pattern(pattern, Some(&args), &mut errors);
    errors.is_empty().then(|| text.into_owned())
}

/// The finding's message in `lang` (`ew check --lang`, the message panel): in en-GB the message itself; in another
/// shipped language the rule's pattern of that language rendered from the finding's arguments where present, the
/// en-GB message otherwise (doc 15 §3.8).
pub fn render(f: &Finding, lang: &str) -> String {
    if lang.eq_ignore_ascii_case(DEFAULT_LANGUAGE) {
        return f.message.clone();
    }
    format(f, lang).unwrap_or_else(|| f.message.clone())
}

/// One attribute of a rule's message in `lang`, falling back to en-GB; `None` when en-GB lacks it too.
fn attribute(rule: &str, name: &str, lang: &str) -> Option<String> {
    let id = message_id(rule);
    let lookup = |tag: &str| -> Option<String> {
        let b = bundle(tag)?;
        let pattern = b.get_message(&id)?.get_attribute(name)?.value();
        let mut errors = Vec::new();
        let text = b.format_pattern(pattern, None, &mut errors);
        errors.is_empty().then(|| text.into_owned())
    };
    lookup(lang).or_else(|| lookup(DEFAULT_LANGUAGE))
}

/// Whether `lang` itself (no fallback) has the message of `rule` with the attribute `name` (`None`: the message).
pub fn has(rule: &str, name: Option<&str>, lang: &str) -> bool {
    let Some(b) = bundle(lang) else {
        return false;
    };
    let Some(m) = b.get_message(&message_id(rule)) else {
        return false;
    };
    match name {
        None => true,
        Some("value") => m.value().is_some(),
        Some(attr) => m.get_attribute(attr).is_some(),
    }
}

/// A quick fix's label in `lang`: its key's message of that language, falling back to en-GB's, and to the
/// English label for a fix without a key (an importer's).
pub fn fix_label(fix: &crate::finding::QuickFix, lang: &str) -> String {
    let lookup = |tag: &str| -> Option<String> {
        let b = bundle(tag)?;
        let pattern = b.get_message(fix.key?)?.value()?;
        let mut errors = Vec::new();
        let text = b.format_pattern(pattern, None, &mut errors);
        errors.is_empty().then(|| text.into_owned())
    };
    lookup(lang)
        .or_else(|| lookup(DEFAULT_LANGUAGE))
        .unwrap_or_else(|| fix.label.clone())
}

/// A rule's title in `lang` (`.title`), falling back to en-GB.
pub fn title(rule: &str, lang: &str) -> Option<String> {
    attribute(rule, "title", lang)
}

/// A rule's hint in `lang` (`.hint`): what to do about a finding, falling back to en-GB.
pub fn hint(rule: &str, lang: &str) -> Option<String> {
    attribute(rule, "hint", lang)
}
