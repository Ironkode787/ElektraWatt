// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell's texts through Fluent: LO-3 the completeness gate; the pseudo-locale en-XA; the run-time switch. Part 4
//! (D12, ID-1269): NL-1 the arguments of every Dutch message, NL-2 the terminology table.

use crate::common::{Tmp, done};
use ew_viewmodel::actions::{self, Menu, Submenu};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Outcome, ShellModel};
use ew_viewmodel::{key, strings};
use serde_json::{Value, json};

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
/// `{ IN_SENTENCE($pN) }`.
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

/// FT-1505 (doc 16a §8 item 5): in every shipped locale every whole toast text ends with its full stop. Exempt
/// by name: the toasts' button and label words, and the fragments composed into another toast's sentence.
#[test]
fn every_toast_ends_with_its_full_stop() {
    const LABELS: [&str; 9] = [
        "toast-undo",
        "toast-close",
        "toast-open",
        "toast-dismiss",
        "toast-name",
        "toast-convert",
        "toast-show-block",
        "toast-show-changes",
        "toast-back-to-editor",
    ];
    const FRAGMENTS: [&str; 5] = [
        "toast-external-git",
        "toast-bind-displaced",
        "toast-cable-new-cores",
        "toast-cable-new-cores-one",
        "toast-device-retagged",
    ];
    for locale in ["en-GB", "nl-NL"] {
        let ftl = shipped_file(locale);
        let lines: Vec<&str> = ftl.lines().collect();
        let mut without = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let Some((key, text)) = line.split_once(" = ") else {
                continue;
            };
            if !key.starts_with("toast-") || LABELS.contains(&key) || FRAGMENTS.contains(&key) {
                continue;
            }
            // A message's last line holds its end.
            let end = lines[i + 1..]
                .iter()
                .take_while(|l| l.starts_with(' '))
                .last()
                .copied()
                .unwrap_or(text);
            if !end.trim_end().ends_with('.') {
                without.push(key);
            }
        }
        assert_eq!(without, Vec::<&str>::new(), "{locale}");
    }
}

/// NL-1: every key of every shipped locale takes the arguments of its en-GB message, no more and
/// no fewer, and a locale has no key en-GB lacks.
#[test]
fn every_message_takes_the_arguments_of_en_gb() {
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

/// Whether the en-GB `term` is used in `texts` (lower case) as whole words: the term at the start of a word and
/// ending one, or followed by a plural's "s" or "es" that ends it ("terminals" is "terminal", "termination" is not;
/// "parts" is "part", "parties" is not).
fn en_uses(texts: &str, term: &str) -> bool {
    let term = term.to_lowercase();
    // An identifier's underscore joins its words: `props.accept_tag_proposal` holds no "tag".
    let alnum = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    texts.match_indices(&term).any(|(i, _)| {
        let rest = &texts[i + term.len()..];
        let rest = ["es", "s"]
            .iter()
            .find_map(|p| rest.strip_prefix(p).filter(|r| !alnum(r.chars().next())))
            .unwrap_or(rest);
        !alnum(texts[..i].chars().next_back()) && !alnum(rest.chars().next())
    })
}

/// The Dutch forms of a one-word term: the term and its plurals, "klem" "klemmen", "draad" "draden",
/// "kast" "kasten", "ader" "aders", "doorsnede" "doorsneden" ("pagina's" splits into "pagina" and "s").
fn nl_forms(term: &str) -> Vec<String> {
    let last = term.chars().last().map(String::from).unwrap_or_default();
    let mut forms = vec![
        term.to_owned(),
        format!("{term}s"),
        format!("{term}n"),
        format!("{term}en"),
        format!("{term}{last}en"),
        format!("{}{last}en", stem(term)),
    ];
    forms.sort();
    forms.dedup();
    forms
}

/// Whether the Dutch `term` is in `texts` (lower case) as a word or at a compound part's edge, so that a
/// compound by the table counts ("klemmenstrook" holds "klem", "draadartikel" "artikel", "proceskenmerk" "kenmerk")
/// and a word that only shares letters does not ("kleur" and "klep" hold no "klem", "kader" no "ader"): a one-word
/// term or its plural ([`nl_forms`]) starting a word, or after at least three letters ending one or, for a term of
/// six letters or more, standing inside it ("paneellayoutpagina"); a term of several words as a phrase starting a
/// word.
fn nl_has(texts: &str, term: &str) -> bool {
    let term = term.to_lowercase();
    if term.chars().all(char::is_alphanumeric) {
        let forms = nl_forms(&term);
        let long = term.chars().count() >= 6;
        texts.split(|c: char| !c.is_alphanumeric()).any(|word| {
            forms.iter().any(|f| {
                word.match_indices(f.as_str()).any(|(i, _)| {
                    let head = word[..i].chars().count();
                    head == 0 || (head >= 3 && (long || i + f.len() == word.len()))
                })
            })
        })
    } else {
        texts
            .match_indices(&term)
            .any(|(i, _)| i == 0 || !texts[..i].chars().next_back().is_some_and(char::is_alphanumeric))
    }
}

/// Whether the rejected Dutch word `alt` is in `texts` (lower case), inside a word too (FT-1341: "standaarddraadkleur"
/// holds "draadkleur"), once the row's own term `own` is taken out ("kabelgoot" holds a duct's "goot" by right).
fn nl_rejected(texts: &str, alt: &str, own: &str) -> bool {
    let texts = if own.is_empty() {
        texts.to_owned()
    } else {
        texts.replace(&own.to_lowercase(), " ")
    };
    let alt = alt.to_lowercase();
    if alt.chars().all(char::is_alphanumeric) {
        nl_forms(&alt).iter().any(|f| texts.contains(f.as_str()))
    } else {
        texts.contains(&alt)
    }
}

/// The messages of the Fluent file `ftl` by key: each message's texts as `texts` reads them, its attributes, select
/// arms and continuation lines included.
fn messages_of(ftl: &str) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    let mut current: Option<(String, String)> = None;
    for line in ftl.lines() {
        let top = !line.starts_with(char::is_whitespace) && !line.starts_with('#') && !line.trim().is_empty();
        if top {
            if let Some((k, body)) = current.take() {
                out.insert(k, texts(&body));
            }
            let (k, v) = line.split_once('=').expect("a message has key = value");
            current = Some((k.trim().to_owned(), format!("k = {}", v.trim_start())));
        } else if let Some((_, body)) = &mut current
            && !line.starts_with('#')
        {
            body.push('\n');
            body.push_str(line);
        }
    }
    if let Some((k, body)) = current {
        out.insert(k, texts(&body));
    }
    out
}

/// The message texts of the Fluent file `ftl` in lower case, one per line: each message's value, its attributes
/// (`.title = …`), the arms of its selectors (`[one] …`, `*[other] …`) and its continuation lines,
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

/// The messages whose en-GB text holds a table term in another sense than the table's, by (key, term, the
/// sense): NL-2 does not ask the row's Dutch term there.
const OTHER_SENSES: &[(&str, &str, &str)] = &[
    ("action-view-frame-stats", "frame", "a drawn frame of the window"),
    ("canvas-stats-frame", "frame", "a drawn frame of the window"),
    (
        "action-view-workspace-reset",
        "layout",
        "the window's layout (indeling)",
    ),
    (
        "action-view-workspace-save",
        "layout",
        "the window's layout (indeling)",
    ),
    ("banner-workspace-bad", "layout", "the window's layout (indeling)"),
    (
        "dialog-save-workspace",
        "layout",
        "the window's layout (indeling)",
    ),
    ("save-workspace-title", "layout", "the window's layout (indeling)"),
    ("layout-too-deep", "layout", "the window's layout (indeling)"),
    ("layout-too-many", "layout", "the window's layout (indeling)"),
    ("keymap-ctrl-alt", "layout", "a keyboard layout (indeling)"),
    (
        "choice-report_blocks-golden",
        "layout",
        "the report pages' layout (indeling)",
    ),
    ("command-bond-create", "bond", "the verb (verbinden)"),
    ("toast-bind-off-card", "wire", "the verb (verbinden)"),
    ("rule-ew-pot-021", "wire", "the verb (verbinden)"),
    ("rule-ew-num-001", "unfreeze", "the separable verb (maak … los)"),
    (
        "rule-ew-core-071",
        "part",
        "the phrase \"takes part in\" (meetellen)",
    ),
    ("error-join-no-slot", "device", "the script parameter `device=`"),
    (
        "error-no-external",
        "connection",
        "the far device's connection (aansluiting) in `device:connection`",
    ),
    (
        "form-connection",
        "connection",
        "an external reference's far connection (aansluiting)",
    ),
    // The release's Git tag, named "the tag" or the verb (Git's own word, "tag").
    // A computer's terminal, where `ew` runs (de opdrachtregel).
    (
        "refusal-purge-human-only",
        "terminal",
        "a computer's terminal (opdrachtregel)",
    ),
    ("form-revision-git", "tag", "the verb of Git (taggen)"),
    ("error-revision-git", "tag", "a Git tag"),
    ("revision-complete-git", "tag", "a Git tag"),
    ("release-error-git-consent", "tag", "a Git tag"),
    ("release-error-tag-exists", "tag", "a Git tag"),
    // "not part of an assembly", a share (deel).
    (
        "refusal-layout-not-assembly",
        "part",
        "a share of an assembly (deel)",
    ),
    // A command, a member, a value or a library kind by its own name, a rule pack's code, a Git tag, a share.
    (
        "refusal-box-black-terminal",
        "device",
        "the command `device.place`",
    ),
    (
        "refusal-check-waive-rule-id",
        "core",
        "the rule pack code EW-CORE",
    ),
    ("refusal-library-copy-hash", "core", "the rule code EW-CORE-014"),
    // Another crate's error by its own words, a rule code, a language tag.
    ("refusal-schema-float", "core", "the rule code EW-CORE-007"),
    ("refusal-ident-lang-tag", "tag", "a language tag (taalcode)"),
    (
        "refusal-ident-lang-tag-canonical",
        "tag",
        "a language tag (taalcode)",
    ),
    (
        "refusal-fields-port-role-unknown",
        "device",
        "the port role value `device`",
    ),
    (
        "refusal-join-device-link",
        "part",
        "the phrase \"takes part in\" (neemt deel aan)",
    ),
    (
        "refusal-layout-carrier-edit-no-member",
        "part",
        "the member `part`",
    ),
    ("refusal-library-embed-kind", "part", "the library kind `part`"),
    (
        "refusal-library-embed-kind",
        "symbol",
        "the library kind `symbol`",
    ),
    ("refusal-paste-library-kind", "part", "the library kind `part`"),
    (
        "refusal-paste-library-kind",
        "symbol",
        "the library kind `symbol`",
    ),
    (
        "refusal-library-update-not-held",
        "library",
        "the command `library.embed`",
    ),
    (
        "refusal-library-update-not-held",
        "part",
        "the command `part.assign`",
    ),
    ("refusal-revision-git-tag", "tag", "a Git tag"),
];

/// NL-2: the terminology table heads the nl-NL file with the ruling's rows; over the
/// messages, each Dutch term is used where its en-GB term is, and no rejected alternative is (one that is the Dutch
/// term of another row, "kabel" for a cable, is that row's).
#[test]
fn the_terminology_table_is_followed() {
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
    // Message by message. Each table term an en-GB message holds as whole words (the longest first, a term
    // inside a longer one not counted again) is its Dutch term in the nl-NL message, inside a compound too. A rejected
    // word is refused at the start of a Dutch word: everywhere, or, when it is (in) another row's Dutch term ("kabel"
    // for a cable, "kabelgoot" for a duct), in the messages whose en-GB text holds this row's term and not that row's.
    let mut by_length: Vec<&Term> = table.iter().collect();
    by_length.sort_by_key(|t| std::cmp::Reverse(t.en.len()));
    let rule_file = |locale: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../ew-checks/messages")
            .join(locale)
            .join("rules.ftl");
        std::fs::read_to_string(path).unwrap().replace("\r\n", "\n")
    };
    let mut wrong = Vec::new();
    for (en_file, nl_file) in [
        (shipped_file("en-GB"), nl.clone()),
        (rule_file("en-GB"), rule_messages_nl()),
    ] {
        let dutch_messages = messages_of(&nl_file);
        for (key, en) in messages_of(&en_file) {
            let Some(dutch) = dutch_messages.get(&key) else {
                continue;
            };
            // A Git tag is Git's own word, "git tag" taken out; a message that names it "the tag" is listed
            // in OTHER_SENSES, so a device tag beside Git is still asked for its kenmerk.
            let mut english = en.replace("git tag", " ");
            let mut held = Vec::new();
            for term in &by_length {
                let other_sense = OTHER_SENSES.iter().any(|(k, t, _)| *k == key && *t == term.en);
                if !other_sense && en_uses(&english, &term.en) {
                    held.push(*term);
                    if !nl_has(dutch, &term.nl) {
                        wrong.push(format!("{key}: {} is {}, not {dutch:?}", term.en, term.nl));
                    }
                    english = english.replace(&term.en.to_lowercase(), " ");
                }
            }
            for term in &table {
                for alt in &term.not {
                    let owners: Vec<&Term> = table
                        .iter()
                        .filter(|t| t.en != term.en && nl_rejected(&t.nl, alt, ""))
                        .collect();
                    let allowed = !owners.is_empty()
                        && (!held.iter().any(|t| t.en == term.en)
                            || owners.iter().any(|o| en_uses(&en, &o.en)));
                    if !allowed && nl_rejected(dutch, alt, &term.nl) {
                        wrong.push(format!("{key}: {} is {alt}, not {}: {dutch:?}", term.en, term.nl));
                    }
                }
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
    // The en-GB side takes whole words and a plural, the Dutch side a compound by the table.
    assert!(en_uses("two terminals", "terminal") && en_uses("a wire's part", "part"));
    assert!(!en_uses("termination", "terminal") && !en_uses("parties", "part"));
    assert!(en_uses("cross-sections", "cross-section") && !en_uses("departure", "part"));
    assert!(nl_has("draadartikel overschreven", "artikel") && nl_has("proceskenmerk", "kenmerk"));
    assert!(nl_has("git-kenmerk", "kenmerk") && !nl_has("een onderdeel", "artikel"));
    // A term whole, by its plural or at a compound part's edge, never by letters it shares.
    assert!(!nl_has("de kleur", "klem") && !nl_has("de klep", "klem") && !nl_has("het kader", "ader"));
    assert!(
        nl_has("twee klemmen", "klem") && nl_has("de klemmenstrook", "klem") && nl_has("de draden", "draad")
    );
    assert!(nl_has("de aders", "ader") && nl_has("twee kasten", "kast") && !nl_has("de kassa", "kast"));
    assert!(nl_rejected("de standaarddraadkleur", "draadkleur", "aderkleur"));
    assert!(!nl_rejected("de kabelgoot", "goot", "kabelgoot") && nl_rejected("de goot", "goot", "kabelgoot"));
    let read =
        texts("k = a { $x ->\n    [one] de lay-out\n   *[other] { $y } onderdeel\n}\n    .hint = een tag\n");
    assert_eq!(read, "a \n de lay-out\n  onderdeel\n\neen tag");
}

/// The messages of `ftl` whose key starts with `prefix`, by key: their lower-case texts.
fn messages_with(ftl: &str, prefix: &str) -> std::collections::BTreeMap<String, String> {
    ftl.lines()
        .filter(|l| l.starts_with(prefix))
        .filter_map(|l| l.split_once(" = "))
        .map(|(k, v)| (k.to_owned(), v.to_lowercase()))
        .collect()
}

/// C49-p10b#1: the record kinds' and the fields' Dutch headings follow the terminology table message by
/// message, so a right use elsewhere cannot hide a wrong one: each table term an en-GB heading holds (the longest
/// first, a term inside a longer one not counted again) is its Dutch term in the nl-NL heading.
#[test]
fn the_kind_and_field_headings_follow_the_table_each() {
    let nl_file = shipped_file("nl-NL");
    let mut table = terms(&nl_file);
    table.sort_by_key(|t| std::cmp::Reverse(t.en.len()));
    let en_file = shipped_file("en-GB");
    let mut wrong = Vec::new();
    for prefix in ["record-kind-", "field-"] {
        let nl = messages_with(&nl_file, prefix);
        for (key, en) in messages_with(&en_file, prefix) {
            let dutch = nl.get(&key).unwrap_or_else(|| panic!("{key} has no Dutch text"));
            // FIX50b ruling 1: a Git tag is Git's own word ("Git-tag"), not the drawing's device tag (NL-2's Git
            // exemption).
            let mut english = en.replace("git tag", " ");
            for term in &table {
                if en_uses(&english, &term.en) {
                    if !nl_has(dutch, &term.nl) {
                        wrong.push(format!("{key}: {} is {}, not {dutch:?}", term.en, term.nl));
                    }
                    english = english.replace(&term.en.to_lowercase(), " ");
                }
            }
        }
    }
    assert_eq!(wrong, Vec::<String>::new());
}

/// With NL-2 by whole words and Dutch compounds, the en-GB words stand as they were
/// (English is never changed for a Dutch check) and the Dutch headings are compounds by the table.
#[test]
fn the_wordings_the_stem_match_forced_are_back() {
    let en = messages_with(&shipped_file("en-GB"), "");
    let nl = messages_with(&shipped_file("nl-NL"), "");
    let text = |m: &std::collections::BTreeMap<String, String>, k: &str| m.get(k).cloned().unwrap();
    assert_eq!(text(&en, "field-word-parties"), "parties");
    assert_eq!(text(&en, "field-word-termination"), "termination");
    assert_eq!(text(&nl, "field-word-git-tag"), "git-tag");
    assert_eq!(text(&nl, "field-word-process-tag"), "proceskenmerk");
    assert_eq!(
        text(&nl, "field-connection-overrides-wire-part"),
        "draadartikel overschreven"
    );
    assert_eq!(text(&nl, "field-function-template-entry"), "artikelfunctie");
    assert_eq!(text(&nl, "field-word-wire-part"), "draadartikel");
    assert_eq!(text(&nl, "column-connections-part"), "draadartikel");
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
fn view_language_switches_the_menus_and_back() {
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
            message: "\"xx\" is geen van de keuzes: en, nl.".to_owned()
        }
    );
    done(&mut shell, "view.language", json!({"value": "en"}));
    assert_eq!(strings::locale(), "en");
    assert_eq!(strings::text(key!("menu-view")), "View");
    shell.shutdown();
}

/// LO-4: with the UI in Dutch the Messages detail shows the nl-NL text of a Golden A rule that has one (EW-POT-013)
/// and the en-GB text of a rule without one (starting with a capital); in English it is the finding's own
/// message.
#[test]
fn the_messages_detail_follows_the_ui_language() {
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

/// FT-1482, beside LO-4: with the UI in Dutch a finding's quick fix is labelled in Dutch, in the Messages
/// detail and in the table's fix cell; in English it is the fix's English label, the CLI's.
#[test]
fn the_quick_fix_label_follows_the_ui_language() {
    let tmp = Tmp::new("lo4fix");
    let mut shell = tmp.shell_in(ew_viewmodel::shell::Mode::Headless, true, "102");
    // As MS3: page 102's own description gives EW-CORE-023's finding, whose fix drops it.
    let page = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "102")
        .map(|r| r.id.to_string())
        .unwrap();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [page], "path": "description_override",
            "value": "Overridden"}}),
    );
    done(&mut shell, "messages.show", json!({"on": true}));
    let table = |shell: &ShellModel| {
        shell
            .nav(NavKind::Messages)
            .and_then(|n| n.table())
            .cloned()
            .unwrap()
    };
    let fixed = table(&shell)
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Finding && n.cells[5] == "drop override")
        .expect("a finding with the drop-override fix")
        .clone();
    let fix = |shell: &mut ShellModel| {
        done(
            shell,
            "navigator.row",
            json!({"kind": "messages", "key": fixed.key}),
        );
        shell.messages_detail().unwrap().fix
    };
    assert_eq!(fix(&mut shell).as_deref(), Some("drop override"));
    done(&mut shell, "view.language", json!({"value": "nl"}));
    assert_eq!(fix(&mut shell).as_deref(), Some("overschrijving verwijderen"));
    let row = table(&shell)
        .nodes
        .iter()
        .find(|n| n.key == fixed.key)
        .cloned()
        .unwrap();
    assert_eq!(row.cells[5], "overschrijving verwijderen");
    done(&mut shell, "view.language", json!({"value": "en"}));
    assert_eq!(fix(&mut shell).as_deref(), Some("drop override"));
    shell.shutdown();
}

/// FT-1526, FT-1528 and FT-1529, whole in both languages: an imported record is "an item" / "een element";
/// a tutorial is a "zelfstudie" in the Dutch window, as Help ▸ Zelfstudies names it; the Properties card's title puts
/// the command's title mid-sentence in Dutch (`IN_SENTENCE`), as the Preview tab does.
#[test]
fn the_reworded_texts_in_both_languages() {
    let texts = || {
        [
            strings::format(key!("refusal-import-unknown-kind"), &["X"]),
            strings::format(key!("refusal-import-removed"), &["X"]),
            strings::text(key!("action-help-tutorials")).to_owned(),
            strings::text(key!("submenu-tutorials")).to_owned(),
            strings::text(key!("start-tutorials")).to_owned(),
            strings::text(key!("banner-tutorial-copy")).to_owned(),
            strings::format(
                key!("card-title-properties"),
                &[strings::command_title("fields.set")],
            ),
        ]
    };
    assert_eq!(
        texts(),
        [
            "an item of an unknown kind (X)",
            "a normaliser removed an item the import brought: X",
            "Tutorials",
            "Tutorials",
            "Tutorials",
            "A fresh copy of the sample project was made for the tutorials.",
            "Preview of Set property",
        ]
    );
    assert!(strings::set_locale("nl"));
    let dutch = texts();
    assert!(strings::set_locale("en"));
    assert_eq!(
        dutch,
        [
            "een element van een onbekende soort (X)",
            "een normalisator heeft een element verwijderd dat de import meebracht: X",
            "Zelfstudies",
            "Zelfstudies",
            "Zelfstudies",
            "Voor de zelfstudies is een nieuwe kopie van het voorbeeldproject gemaakt.",
            "Voorbeeld van eigenschap instellen",
        ]
    );
}
