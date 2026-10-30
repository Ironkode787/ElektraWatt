// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule messages in Fluent (doc 15 §3.8; WP-S1.16 D6, P5-1, P5-2).

mod common;

use std::collections::BTreeSet;

use common::*;
use ew_checks::messages::{
    self, DEFAULT_LANGUAGE, LANGUAGES, format, has, hint, message_id, render, resource_errors, title,
};
use ew_checks::rules;
use serde_json::json;

const SOURCES: [(&str, &str); 2] = [
    ("en-GB", include_str!("../messages/en-GB/rules.ftl")),
    ("nl-NL", include_str!("../messages/nl-NL/rules.ftl")),
];

/// The message IDs a resource defines, in file order (`rule-…` at the start of a line).
fn ids(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter(|l| l.starts_with("rule-"))
        .filter_map(|l| l.split_whitespace().next())
        .collect()
}

/// P5-1 (NFR-I18N-02): every registered rule has its message with `.title` and `.hint` in en-GB, and the title is the
/// catalogue's; nl-NL's missing messages and attributes are listed as warnings until 1.0, never a failure.
#[test]
fn every_rule_has_its_message_title_and_hint_in_en_gb() {
    assert!(resource_errors().is_empty(), "{:#?}", resource_errors());
    let mut missing = Vec::new();
    for r in rules() {
        let id = r.meta().id;
        for attr in [None, Some("title"), Some("hint")] {
            if !has(id, attr, DEFAULT_LANGUAGE) {
                missing.push(format!("{id} {}", attr.unwrap_or("message")));
            }
        }
        assert_eq!(
            title(id, DEFAULT_LANGUAGE).as_deref(),
            Some(r.meta().title),
            "{id}: the en-GB title is the catalogue title"
        );
        assert!(
            hint(id, DEFAULT_LANGUAGE).is_some_and(|h| h.ends_with('.') && h.len() > 10),
            "{id}: a hint is a sentence"
        );
    }
    assert!(missing.is_empty(), "en-GB lacks: {missing:#?}");
    for lang in LANGUAGES.iter().filter(|l| **l != DEFAULT_LANGUAGE) {
        let absent: Vec<&str> = rules()
            .iter()
            .map(|r| r.meta().id)
            .filter(|id| !has(id, Some("title"), lang) || !has(id, Some("hint"), lang))
            .collect();
        if !absent.is_empty() {
            println!(
                "warning: {lang} has no title or hint for {} rules (en-GB is used until 1.0): {}",
                absent.len(),
                absent.join(", ")
            );
        }
    }
}

/// Every message of every resource names a registered rule, once; a language never has a pattern (a value) that en-GB
/// lacks, so a translated text always has the en-GB text it falls back to.
#[test]
fn every_message_names_a_registered_rule_once() {
    let known: BTreeSet<String> = rules().iter().map(|r| message_id(r.meta().id)).collect();
    for (lang, source) in SOURCES {
        let list = ids(source);
        let distinct: BTreeSet<&str> = list.iter().copied().collect();
        assert_eq!(distinct.len(), list.len(), "{lang}: a message is defined twice");
        for id in &list {
            assert!(known.contains(*id), "{lang}: {id} names no registered rule");
        }
    }
    for r in rules() {
        let id = r.meta().id;
        for lang in LANGUAGES {
            if has(id, Some("value"), lang) {
                assert!(
                    has(id, Some("value"), DEFAULT_LANGUAGE),
                    "{lang} has a pattern for {id} that en-GB lacks"
                );
            }
        }
    }
}

/// A finding of one rule over `model`, by rule ID.
fn one(m: &ew_model::Model, rule: &str) -> ew_checks::Finding {
    let f = findings_of(m, rule);
    assert_eq!(f.len(), 1, "{rule}: {f:#?}");
    f.into_iter().next().expect("one finding")
}

/// P5-2's en-GB half on fixtures: a finding with arguments renders its message byte for byte from the en-GB pattern,
/// and `render` in en-GB is the message itself; nl-NL renders the Dutch pattern from the same arguments.
#[test]
fn en_gb_renders_the_message_byte_for_byte_and_nl_nl_renders_dutch() {
    // EW-TERM-002 has no pattern yet: every language prints the en-GB message.
    let m = model(vec![strip(1, 1), terminal(2, 1, 1, "1"), terminal(3, 1, 2, "1")]);
    let dup = one(&m, "EW-TERM-002");
    assert!(dup.args.is_empty());
    assert_eq!(format(&dup, DEFAULT_LANGUAGE), None);
    for lang in LANGUAGES {
        assert_eq!(render(&dup, lang), dup.message, "{lang}");
    }
    // An unknown language falls back to en-GB.
    assert_eq!(render(&dup, "fr-FR"), dup.message);
    assert_eq!(
        title("EW-TERM-002", "fr-FR").as_deref(),
        Some("Duplicate designation")
    );

    // A finding with arguments built by hand: the pattern's branches.
    let rule = ew_checks::rule("EW-POT-009").expect("registered").meta();
    for (unused, en, nl) in [
        (
            "no",
            "-K1:13 is not connected; connect it or mark it unused",
            "-K1:13 is niet aangesloten; sluit het aan of markeer het als ongebruikt",
        ),
        (
            "yes",
            "-K1:13 is not connected (marked unused)",
            "-K1:13 is niet aangesloten (gemarkeerd als ongebruikt)",
        ),
    ] {
        let f = ew_checks::Finding::new(rule, [], "13", "-K1:13", en)
            .arg("cp", "-K1:13")
            .arg("cp_shared", "no")
            .arg("unused", unused);
        assert_eq!(format(&f, DEFAULT_LANGUAGE).as_deref(), Some(en));
        assert_eq!(render(&f, DEFAULT_LANGUAGE), en);
        assert_eq!(render(&f, "nl-NL"), nl);
        assert_eq!(render(&f, "NL-nl"), nl, "language tags compare without case");
    }
    // FT-756 (ID-889): a pin whose designation the device shares is named in words with its position, in each
    // language's ordinal, never by the label's `#n`.
    for (nth, en, nl) in [
        ("2", "-1A4 pin − (2nd)", "-1A4 pen − (2e)"),
        ("12", "-1A4 pin − (12th)", "-1A4 pen − (12e)"),
        ("21", "-1A4 pin − (21st)", "-1A4 pen − (21e)"),
        ("3", "-1A4 pin − (3rd)", "-1A4 pen − (3e)"),
    ] {
        let message = format!("{en} is not connected; connect it or mark it unused");
        let f = ew_checks::Finding::new(rule, [], "13", "-1A4:−#2", message.clone())
            .arg("cp", "-1A4:−#2")
            .arg("cp_shared", "yes")
            .arg("cp_tag", "-1A4")
            .arg("cp_pin", "−")
            .arg("cp_nth", nth)
            .arg("unused", "no");
        assert_eq!(format(&f, DEFAULT_LANGUAGE).as_deref(), Some(message.as_str()));
        let dutch = render(&f, "nl-NL");
        assert!(dutch.starts_with(&format!("{nl} is niet aangesloten")), "{dutch}");
        assert!(!dutch.contains('#'), "{dutch}");
    }
    // A missing argument is an error in the pattern: the en-GB message is kept.
    let partial = ew_checks::Finding::new(rule, [], "13", "-K1:13", "the en-GB text").arg("unused", "no");
    assert_eq!(render(&partial, "nl-NL"), "the en-GB text");
    assert_eq!(
        messages::title("EW-POT-013", "nl-NL").as_deref(),
        Some("Schakelen in de nulleider")
    );
    let _ = json!(null);
}
