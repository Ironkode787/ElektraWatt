// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 3 (D9 to D11, ID-1219): the shell's texts through Fluent. LO-1 every key's en-GB text equals the
//! committed snapshot `tests/expected/en-GB-texts.txt` (one `key = text` line per key, a backslash, a line feed and a
//! tab escaped as `\\`, `\n` and `\t`), taken at the base of part 3 before the table moved to Fluent; a difference is
//! a stop, never a re-bless. LO-3 the completeness gate; the pseudo-locale en-XA; the run-time switch. Part 4 (D12,
//! ID-1269): NL-1 the arguments of every Dutch message, NL-2 the terminology table.

use crate::common::{Tmp, done};
use ew_viewmodel::actions::{self, Menu, Submenu};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Outcome, ShellModel};
use ew_viewmodel::{key, strings};
use serde_json::{Value, json};

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

/// LO-3 and NL-1: the completeness gate lists every en-GB key a shipped locale (en-GB, nl-NL) lacks.
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

/// ID-1219 ruling 5, ID-1269 ruling 5: the shipped Dutch and a test language are offered with their own names and
/// switched at run time; a test language falls back to en-GB per message and never replaces a shipped one; the
/// pseudo-locale is never offered.
#[test]
fn a_test_language_switches_and_falls_back() {
    strings::register_locale_for_tests("fy", "language-name = Frysk\nmenu-view = Byld\n");
    strings::register_locale_for_tests("nl", "language-name = Test\nmenu-view = Test\n");
    assert!(strings::set_locale(strings::PSEUDO));
    let languages = strings::languages();
    assert_eq!(
        languages,
        [("en", "English"), ("nl", "Nederlands"), ("fy", "Frysk")]
    );
    assert!(strings::set_locale("fy"));
    assert_eq!(strings::text(key!("menu-view")), "Byld");
    assert_eq!(strings::text(key!("menu-file")), "File");
    assert_eq!(strings::rule_language(), "en-GB");
    assert!(strings::set_locale("nl"));
    assert_eq!(strings::locale(), "nl");
    assert_eq!(strings::text(key!("menu-view")), "Beeld");
    assert_eq!(strings::text(key!("menu-file")), "Bestand");
    assert_eq!(strings::rule_language(), "nl-NL");
    assert!(strings::set_locale("en"));
    assert_eq!(strings::text(key!("menu-view")), "View");
    assert_eq!(strings::rule_language(), "en-GB");
}

/// The `$pN` arguments of each message of the Fluent file `ftl`, by key: in a placeable `{ $pN }` or a function's
/// `{ IN_SENTENCE($pN) }` (ID-1278 ruling 9).
fn arguments(ftl: &str) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
    ftl.lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| l.split_once(" = "))
        .map(|(key, text)| {
            let args = text
                .split('$')
                .skip(1)
                .map(|rest| rest.chars().take_while(char::is_ascii_alphanumeric).collect())
                .collect();
            (key.to_owned(), args)
        })
        .collect()
}

/// The shell file of the shipped locale `locale`.
fn shipped_file(locale: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("locales")
        .join(locale)
        .join("shell.ftl");
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

/// NL-1 (ID-1269 ruling 3): every key of every shipped locale takes the arguments of its en-GB message, no more and
/// no fewer, and a locale has no key en-GB lacks.
#[test]
fn nl1_every_message_takes_the_arguments_of_en_gb() {
    let en = arguments(&shipped_file("en-GB"));
    let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    let mut differ = Vec::new();
    for entry in std::fs::read_dir(&locales).unwrap() {
        let locale = entry.unwrap().file_name().to_string_lossy().into_owned();
        for (key, args) in arguments(&shipped_file(&locale)) {
            match en.get(&key) {
                None => differ.push(format!("{locale}: {key} is not an en-GB key")),
                Some(want) if *want != args => {
                    differ.push(format!("{locale}: {key} takes {args:?}, en-GB {want:?}"))
                }
                Some(_) => {}
            }
        }
    }
    assert_eq!(differ, Vec::<String>::new());
}

/// A row of the terminology table: the en-GB term, the nl-NL term and the rejected alternatives.
struct Term {
    en: String,
    nl: String,
    not: Vec<String>,
}

/// The terminology table at the head of the nl-NL file: its `# term: <en-GB> = <nl-NL> (not: <alt>, <alt>)` lines.
fn terms(ftl: &str) -> Vec<Term> {
    ftl.lines()
        .filter_map(|l| l.strip_prefix("# term: "))
        .map(|row| {
            let (en, rest) = row.split_once(" = ").expect("a row has en-GB = nl-NL");
            let (nl, not) = match rest.split_once(" (not: ") {
                Some((nl, not)) => (
                    nl,
                    not.strip_suffix(')')
                        .expect("a closing parenthesis")
                        .split(", ")
                        .map(str::to_owned)
                        .collect(),
                ),
                None => (rest, Vec::new()),
            };
            Term {
                en: en.to_owned(),
                nl: nl.to_owned(),
                not,
            }
        })
        .collect()
}

/// The stem of a one-word term: the word without its last consonant and one vowel of a doubled vowel before it,
/// so that a Dutch plural or compound counts ("klemmenstrook" → "klemmenstro", which "klemmenstroken" starts with).
fn stem(word: &str) -> String {
    let mut s: Vec<char> = word.chars().collect();
    if s.len() > 3 && s.last().is_some_and(|c| !"aeiou".contains(*c)) {
        s.pop();
        let n = s.len();
        if n >= 2 && s[n - 1] == s[n - 2] && "aeiou".contains(s[n - 1]) {
            s.pop();
        }
    }
    s.into_iter().collect()
}

/// Whether `term` is used in `texts` (lower case), case-insensitive and at the start of a word: a one-word term as a
/// word that starts with its stem, or with the whole term when its stem is shorter than four letters ("goot" is no
/// stem of "goed"); a term of several words (or with a hyphen) as a phrase that starts a word.
fn uses(texts: &str, term: &str) -> bool {
    let term = term.to_lowercase();
    let starts_word = |i: usize| i == 0 || !texts[..i].chars().next_back().is_some_and(char::is_alphanumeric);
    if term.chars().all(char::is_alphanumeric) {
        let stem = Some(stem(&term))
            .filter(|s| s.chars().count() >= 4)
            .unwrap_or_else(|| term.clone());
        texts
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| word.starts_with(&stem))
    } else {
        texts.match_indices(&term).any(|(i, _)| starts_word(i))
    }
}

/// The message texts of the Fluent file `ftl` in lower case, one per line: each message's value, its attributes
/// (`.title = …`), the arms of its selectors (`[one] …`, `*[other] …`) and its continuation lines (ID-1278 ruling 4),
/// with the placeables (`{ $cp_tag }`, a selector's head) left out: they are no words of the text.
fn texts(ftl: &str) -> String {
    ftl.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let text = match l.split_once(" = ") {
                Some((_, text)) => text,
                None => {
                    let t = l.trim_start().trim_start_matches('*');
                    t.strip_prefix('[')
                        .and_then(|t| t.split_once(']'))
                        .map_or(t, |(_, text)| text)
                }
            };
            let mut words = String::new();
            let mut depth = 0usize;
            for c in text.chars() {
                match c {
                    '{' => depth += 1,
                    '}' => depth = depth.saturating_sub(1),
                    c if depth == 0 => words.push(c),
                    _ => {}
                }
            }
            words.to_lowercase()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The checks' nl-NL rule messages (`ew-checks`), which the Messages pane shows in Dutch (D10).
fn rule_messages_nl() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ew-checks/messages/nl-NL/rules.ftl");
    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

/// NL-2 (ID-1269 rulings 1 and 2): the terminology table heads the nl-NL file with the ruling's rows; over the
/// messages, each Dutch term is used where its en-GB term is, and no rejected alternative is (one that is the Dutch
/// term of another row, "kabel" for a cable, is that row's).
#[test]
fn nl2_the_terminology_table_is_followed() {
    let nl = shipped_file("nl-NL");
    let table = terms(&nl);
    let rows: Vec<String> = table.iter().map(|t| format!("{} = {}", t.en, t.nl)).collect();
    for fixed in [
        "device = apparaat",
        "function = functie",
        "terminal = klem",
        "connection point = aansluitpunt",
        "connection = verbinding",
        "wire = draad",
        "wire colour = aderkleur",
        "wire marker = adermarkering",
        "interruption point = onderbrekingspunt",
        "terminal strip = klemmenstrook",
        "potential = potentiaal",
        "part = artikel",
        "messages = meldingen",
        "tag = kenmerk",
        "structure node = structuurknooppunt",
        "basic project = basisproject",
        "title block = titelblok",
        "page = pagina",
        "cross-section = doorsnede",
    ] {
        assert!(rows.iter().any(|r| r == fixed), "the row {fixed}");
    }
    let header_ends = nl.find("\nmenu-file = ").unwrap();
    assert!(nl[..header_ends].contains("# term: device = apparaat (not: toestel, component)\n"));
    // ID-1278 ruling 4: the checks' Dutch messages follow the table too, select arms and attributes included.
    let rules = texts(&rule_messages_nl());
    let dutch = format!("{}\n{rules}", texts(&nl));
    let english = texts(&shipped_file("en-GB"));
    let mut wrong = Vec::new();
    for term in &table {
        if uses(&english, &term.en) && !uses(&dutch, &term.nl) {
            wrong.push(format!("{} is never {}", term.en, term.nl));
        }
        for alt in &term.not {
            if table.iter().any(|t| t.nl == *alt) {
                continue;
            }
            if uses(&dutch, alt) {
                wrong.push(format!("{} is {alt} somewhere, not {}", term.en, term.nl));
            }
        }
    }
    // The checks never speak of Git: there the whole word "tag" is rejected for kenmerk (the shell's Git tags are tags).
    if rules.split(|c: char| !c.is_alphanumeric()).any(|w| w == "tag") {
        wrong.push("tag is tag somewhere in the checks' messages, not kenmerk".to_owned());
    }
    assert_eq!(wrong, Vec::<String>::new());
    // The checks themselves: a plural counts for its stem, a rejected word is found.
    assert!(uses("twee klemmenstroken", "klemmenstrook"));
    assert!(uses("de apparaten", "apparaat"));
    assert!(!uses("een klemmenblok", "klemmenstrook"));
    assert!(uses("het toestel", "toestel"));
    assert!(!uses("de goedkeuring", "goot") && uses("de goot", "goot") && !uses("de kabelgoot", "goot"));
    let read =
        texts("k = a { $x ->\n    [one] de lay-out\n   *[other] { $y } onderdeel\n}\n    .hint = een tag\n");
    assert_eq!(read, "a \n de lay-out\n  onderdeel\n\neen tag");
}

/// The items of View ▸ Language: (label, parameters, checked).
fn language_items(shell: &ShellModel) -> Vec<(String, Value, bool)> {
    let menus = actions::menu_model(shell);
    let view = menus.iter().find(|m| m.menu == Menu::View).unwrap();
    view.submenu(Submenu::Language)
        .unwrap()
        .iter()
        .map(|i| (i.label.clone(), i.params.clone(), i.checked))
        .collect()
}

/// LO-2's model half: View ▸ Language lists the languages under their own names with the current one checked, and
/// `view.language` switches the menus at run time and back; an unknown value is refused in words.
#[test]
fn lo2_view_language_switches_the_menus_and_back() {
    let tmp = Tmp::new("lo2");
    let mut shell = tmp.shell("102");
    let english = ("English".to_owned(), json!({"value": "en"}), true);
    assert_eq!(
        language_items(&shell),
        [
            english.clone(),
            ("Nederlands".to_owned(), json!({"value": "nl"}), false)
        ]
    );
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(strings::locale(), "nl");
    assert_eq!(shell.settings().language, "nl");
    let view = actions::menu_model(&shell)
        .into_iter()
        .find(|m| m.menu == Menu::View)
        .unwrap();
    assert_eq!(view.label, "Beeld");
    assert!(
        language_items(&shell).contains(&("Nederlands".to_owned(), json!({"value": "nl"}), true)),
        "Nederlands is checked"
    );
    assert_eq!(
        shell.dispatch("view.language", &json!({"value": "xx"})),
        Outcome::Failed {
            message: "Parameter `value` is niet geldig.".to_owned()
        }
    );
    done(&mut shell, "view.language", json!({"value": "en"}));
    assert_eq!(strings::locale(), "en");
    assert_eq!(strings::text(key!("menu-view")), "View");
    shell.shutdown();
}

/// LO-4: with the UI in Dutch the Messages detail shows the nl-NL text of a Golden A rule that has one (EW-POT-013)
/// and the en-GB text of a rule without one (starting with a capital, FT-1140); in English it is the finding's own
/// message.
#[test]
fn lo4_the_messages_detail_follows_the_ui_language() {
    let tmp = Tmp::new("lo4");
    let mut shell = tmp.shell("102");
    done(&mut shell, "messages.show", json!({"on": true}));
    let table = shell
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .cloned()
        .unwrap();
    let first = |rule: &str| {
        table
            .nodes
            .iter()
            .find(|n| n.role == NavRole::Finding && n.cells[1] == rule)
            .unwrap_or_else(|| panic!("a {rule} finding"))
            .clone()
    };
    let detail = |shell: &mut ShellModel, key: &str| {
        done(shell, "navigator.row", json!({"kind": "messages", "key": key}));
        shell.messages_detail().unwrap().message
    };
    let pot = first("EW-POT-013");
    // A finding whose rule has no nl-NL message (FT-680: the rules beyond the twelve of Golden A).
    let other = table
        .nodes
        .iter()
        .find(|n| {
            n.role == NavRole::Finding
                && !ew_viewmodel::ew_api::checks::messages::has(&n.cells[1], Some("value"), "nl-NL")
        })
        .expect("a finding of a rule without a Dutch message")
        .clone();
    assert_eq!(detail(&mut shell, &pot.key), pot.cells[2]);
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(
        detail(&mut shell, &pot.key),
        "S56 schakelt de nulleider N-55 → N-55S; belasting E56 blijft verbonden met L-55."
    );
    assert_eq!(
        detail(&mut shell, &other.key),
        strings::sentence_start(&other.cells[2])
    );
    done(&mut shell, "view.language", json!({"value": "en"}));
    assert_eq!(detail(&mut shell, &pot.key), pot.cells[2]);
    shell.shutdown();
}
