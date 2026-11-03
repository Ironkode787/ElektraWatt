// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 3 (D9 to D11, ID-1219): the shell's texts through Fluent. LO-1 every key's en-GB text equals the
//! committed snapshot `tests/expected/en-GB-texts.txt` (one `key = text` line per key, a backslash, a line feed and a
//! tab escaped as `\\`, `\n` and `\t`), taken at the base of part 3 before the table moved to Fluent; a difference is
//! a stop, never a re-bless. LO-3 the completeness gate; the pseudo-locale en-XA; the run-time switch.

use ew_viewmodel::{key, strings};

const SNAPSHOT: &str = include_str!("../expected/en-GB-texts.txt");

/// `text` with a backslash, a line feed and a tab escaped.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

/// LO-1: every key of the snapshot is a key with its line's en-GB text (a key added since is not in it); the
/// arguments filled through Fluent equal the snapshot's text with `{0}`, `{1}`, … replaced.
#[test]
fn every_key_has_its_snapshot_text() {
    assert_eq!(strings::locale(), "en");
    let snapshot = SNAPSHOT.replace("\r\n", "\n");
    let have: String = snapshot
        .lines()
        .map(|line| {
            let (key, _) = line.split_once(" = ").unwrap();
            assert!(strings::has_key(key), "{key} is gone");
            format!("{key} = {}\n", escape(strings::text(strings::Key::run_time(key))))
        })
        .collect();
    assert_eq!(have, snapshot);
    for line in snapshot.lines() {
        let (key, text) = line.split_once(" = ").unwrap();
        let args = ["α", "β", "γ", "δ", "ε", "ζ"];
        let mut wanted = text.to_owned();
        for (i, arg) in args.iter().enumerate() {
            wanted = wanted.replace(&format!("{{{i}}}"), arg);
        }
        assert_eq!(
            strings::format(strings::Key::run_time(key), &args),
            wanted,
            "{key}"
        );
    }
}

/// An argument is a string, never reformatted as a number; a missing one shows as `{n}`, as before.
#[test]
fn arguments_are_strings() {
    assert_eq!(
        strings::format(key!("part-choose-detail"), &["1000", "1.50", "-0"]),
        "1000 \u{b7} version 1.50 \u{b7} -0"
    );
    assert_eq!(
        strings::format(key!("part-choose-detail"), &["A"]),
        "A \u{b7} version {1} \u{b7} {2}"
    );
    assert_eq!(
        strings::text(key!("wizard-error-format")),
        "A numbering format has {page}, {letter} and {counter}, in any order and with other text between them; braces must pair."
    );
}

/// LO-3: the completeness gate lists every en-GB key a shipped locale lacks. en-GB is the only shipped file until
/// part 4's nl-NL; the gate reports and passes for a locale that is not shipped yet.
#[test]
fn every_shipped_locale_has_every_key() {
    let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut shipped: Vec<String> = std::fs::read_dir(&locales)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    shipped.sort();
    assert!(shipped.iter().any(|l| l == "en-GB"), "{shipped:?}");
    let mut missing = Vec::new();
    for locale in &shipped {
        let ftl = std::fs::read_to_string(locales.join(locale).join("shell.ftl")).unwrap();
        let ids: Vec<&str> = ftl
            .lines()
            .filter_map(|l| l.split_once(" =").map(|(id, _)| id))
            .collect();
        for key in strings::KEYS {
            if !ids.contains(key) {
                missing.push(format!("{locale}: {key}"));
            }
        }
    }
    assert_eq!(missing, Vec::<String>::new());
}

/// D11: en-XA accents every letter, pads by 35 % of the length rounded up and brackets the text; the arguments stay
/// and are filled; en-GB is back after the switch.
#[test]
fn the_pseudo_locale_accents_pads_and_brackets() {
    assert!(strings::set_locale(strings::PSEUDO));
    assert_eq!(strings::locale(), "en-XA");
    // "File": 4 letters, 2 of padding (1.4 rounded up).
    assert_eq!(strings::text(key!("menu-file")), "⟦Ƒíĺé~~⟧");
    assert_eq!(
        strings::format(key!("part-choose-detail-functions"), &["X1", "2"]),
        "⟦X1 · ƒúñçţíóñš 2~~~~~~~⟧"
    );
    assert!(!strings::set_locale("xx"));
    assert_eq!(strings::locale(), "en-XA");
    assert!(strings::set_locale("en"));
    assert_eq!(strings::text(key!("menu-file")), "File");
}

/// ID-1219 ruling 5: a test language is offered with its own name, switched at run time, and falls back to en-GB
/// per message; the pseudo-locale is never offered.
#[test]
fn a_test_language_switches_and_falls_back() {
    strings::register_locale_for_tests("nl", "language-name = Nederlands\nmenu-view = Beeld\n");
    assert!(strings::set_locale(strings::PSEUDO));
    assert_eq!(strings::languages(), [("en", "English"), ("nl", "Nederlands")]);
    assert!(strings::set_locale("nl"));
    assert_eq!(strings::text(key!("menu-view")), "Beeld");
    assert_eq!(strings::text(key!("menu-file")), "File");
    assert_eq!(strings::rule_language(), "nl-NL");
    assert!(strings::set_locale("en"));
    assert_eq!(strings::text(key!("menu-view")), "View");
    assert_eq!(strings::rule_language(), "en-GB");
}
