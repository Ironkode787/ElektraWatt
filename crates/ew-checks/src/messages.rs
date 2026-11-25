// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule messages in Fluent (doc 15 §3.8; ADR-0019 as amended, ADR-0024 for the dependency).
//!
//! One resource per language, `texts/<lang>/rules.ftl`, compiled into the binary. Every registered rule has the
//! message `rule-<id lower case>` ([`RuleMeta::message`](crate::RuleMeta::message); Fluent identifiers hold no dots)
//! with the attributes `.title` and `.hint`; a rule that reports findings also has the message's value, the pattern of
//! its text:
//!
//! ```ftl
//! rule-ew-core-037 = shield { $core } is connected at neither end; connect it at one end or both
//!     .title = Shield not connected
//!     .hint = Connect the shield to an earthing point, or mark it as not connected on purpose.
//! ```
//!
//! **A finding carries its text** ([`Finding::text`], ADR-0019 amendment 8): the key of its rule's pattern and its
//! arguments by name. Its message, [`Finding::message`], is the en-GB pattern rendered; [`render`] renders the
//! pattern of another language where it exists and resolves without an error, and falls back to en-GB (doc 15 §3.8:
//! en-GB is mandatory and the fallback). An argument is a printed value (a derived tag, a name, a quantity as text,
//! rendered when the rule ran), a number a pattern selects on, a word by key (`word-…`: "a location", "a device
//! without designation") or a list of them, joined by the language's words (`join-…`), so a pattern never formats a
//! number itself and never reads an English word back. The words of the domain's kinds come from `ew-derive`'s text
//! files, which each language here holds too.
//!
//! The languages are rendered by the one renderer, `ew_l10n::render`, which inserts no Unicode isolation marks: the
//! arguments are tags and names in the project's own text. `DECIMAL($x)` writes the decimal points of a printed number
//! (`1.5 mm²`, `40.0`) as the language writes them (`1,5 mm²` in nl-NL; [`ew_l10n::render::decimal`]); a pattern wraps
//! every argument that holds a measured value, never a wire number or a version.

use std::sync::OnceLock;

use ew_l10n::render::{Language, OnError};

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

/// The language of a shipped tag, built once from this crate's text files and `ew-derive`'s: its patterns may call
/// `NUMBER(…, type: "ordinal")` (a pin's position) and `DECIMAL`. A resource that does not parse is a defect of this
/// crate, caught by the message tests; the entries that parse are still served.
fn bundle(lang: &str) -> Option<&'static Language> {
    static BUNDLES: OnceLock<Vec<Language>> = OnceLock::new();
    let bundles = BUNDLES.get_or_init(|| {
        LANGUAGES
            .iter()
            .map(|tag| {
                let sources = crate::SOURCES
                    .iter()
                    .chain(ew_derive::SOURCES)
                    .filter(|(l, _)| l == tag)
                    .map(|(_, s)| *s);
                Language::new(tag, sources, &[("DECIMAL", ew_l10n::render::decimal)], true)
            })
            .collect()
    });
    let i = LANGUAGES.iter().position(|l| l.eq_ignore_ascii_case(lang))?;
    bundles.get(i)
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

/// A text style's name ([`ew_derive::text::text_style_name`]) in `lang`, falling back to en-GB.
pub fn text_style_name(key: &str, lang: &str) -> String {
    text(&ew_derive::text::text_style_name(key), lang)
}

/// The finding's text rendered from the pattern of `lang`, when the language ships, the text's key has a pattern in it
/// and the pattern resolves without an error; else `None`.
pub fn format(f: &Finding, lang: &str) -> Option<String> {
    bundle(lang)?.text(&f.text, OnError::Miss)
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
