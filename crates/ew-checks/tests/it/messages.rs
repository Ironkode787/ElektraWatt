// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule messages in Fluent (doc 15 §3.8; P5-1, P5-2).

use crate::common;

use std::collections::BTreeSet;

use common::*;
use ew_checks::messages::{
    self, DEFAULT_LANGUAGE, LANGUAGES, format, has, hint, render, resource_errors, title,
};
use ew_checks::rules;
use ew_checks::rules::core::INTEGRITY;
use ew_l10n::TextArg;

const SOURCES: [(&str, &str); 2] = [
    ("en-GB", include_str!("../../texts/en-GB/rules.ftl")),
    ("nl-NL", include_str!("../../texts/nl-NL/rules.ftl")),
];

/// The message IDs a resource defines, in file order (`rule-…` at the start of a line).
fn ids(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter(|l| l.starts_with("rule-"))
        .filter_map(|l| l.split_whitespace().next())
        .collect()
}

/// P5-1 (NFR-I18N-02): every registered rule has its message `rule-<id in lower case>` (ADR-0019 amendment 4) with
/// `.title` and `.hint` in en-GB, and the title is the catalogue's; nl-NL's missing messages and attributes are listed
/// as warnings until 1.0, never a failure.
#[test]
fn every_rule_has_its_message_title_and_hint_in_en_gb() {
    assert!(resource_errors().is_empty(), "{:#?}", resource_errors());
    let mut missing = Vec::new();
    for m in rules().iter().map(|r| r.meta()).chain(INTEGRITY) {
        assert_eq!(
            m.message,
            format!("rule-{}", m.id.to_ascii_lowercase()),
            "{}",
            m.id
        );
    }
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
    // The integrity rules of opening a project reach the Messages pane too: each has its title in every
    // language, en-GB's the catalogue's.
    for m in INTEGRITY {
        assert_eq!(
            title(m.id, DEFAULT_LANGUAGE).as_deref(),
            Some(m.title),
            "{}: the en-GB title is the catalogue title",
            m.id
        );
        for lang in LANGUAGES {
            assert!(has(m.id, Some("title"), lang), "{lang} has no title for {}", m.id);
            // And its hint, the remedy the finding's detail shows.
            assert!(has(m.id, Some("hint"), lang), "{lang} has no hint for {}", m.id);
        }
    }
    // Every other shipped language has the title and the hint of every rule, and the pattern of every rule
    // whose en-GB message has one (the rule tables check that a finding carries its arguments, `assert_worded`).
    for lang in LANGUAGES.iter().filter(|l| **l != DEFAULT_LANGUAGE) {
        let absent: Vec<String> = rules()
            .iter()
            .map(|r| r.meta().id)
            .flat_map(|id| {
                [Some("title"), Some("hint"), Some("value")]
                    .into_iter()
                    .filter(move |attr| has(id, *attr, DEFAULT_LANGUAGE) && !has(id, *attr, lang))
                    .map(move |attr| format!("{id} {}", attr.unwrap_or_default()))
            })
            .collect();
        assert!(absent.is_empty(), "{lang} lacks: {absent:#?}");
    }
}

/// The rule system's own findings (a rule that stopped with an internal error, a pack whose data table does
/// not load), which no fixture raises, word their English and their Dutch from their arguments.
#[test]
fn nl_nl_words_the_rule_system_s_findings() {
    let scope = built(
        "EW-CORE-083",
        "",
        &[
            ("form", "scope".into()),
            ("rule", "EW-POT-006".into()),
            ("scope", ew_checks::text!("word-the-project").into()),
        ],
    );
    assert_eq!(
        render(&scope, "nl-NL"),
        "regel EW-POT-006 kon niet worden beoordeeld voor het project: hij stopte met een interne fout, dus zijn \
         bevindingen daar ontbreken; meld het defect"
    );
    let edit = built(
        "EW-CORE-083",
        "",
        &[("form", "prevent".into()), ("rule", "EW-POT-006".into())],
    );
    assert!(render(&edit, "nl-NL").starts_with("regel EW-POT-006 kon de bewerking niet toetsen"));
    let pack = built(
        "EW-CORE-084",
        "",
        &[
            ("pack", "EW-POT".into()),
            ("file", "data/pe_sizing.toml".into()),
            ("error", "`rows` is missing".into()),
            ("rules", "EW-POT-006".into()),
        ],
    );
    assert!(render(&pack, "nl-NL").starts_with("pakket EW-POT is niet volledig beoordeeld"));
}

/// Every word a rule prints where a record has no name of its own (the en-GB messages `word-…`) has its text in
/// every shipped language, and a Dutch pattern prints the Dutch words and decimal commas.
#[test]
fn every_word_is_in_every_language_and_nl_nl_words_names_and_decimals() {
    let (_, en) = SOURCES[0];
    let words: Vec<&str> = en
        .lines()
        .filter(|l| l.starts_with("word-"))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert!(words.contains(&"word-a-location"), "{words:?}");
    for (lang, source) in SOURCES {
        for w in &words {
            assert!(
                source.lines().any(|l| l.starts_with(&format!("{w} = "))),
                "{lang} lacks {w}"
            );
        }
    }
    // EW-LAY-001 of a device without designation outside a location without codes; EW-POT-006's cross-sections.
    let unplaced = built(
        "EW-LAY-001",
        "",
        &[
            (
                "device",
                ew_checks::text!("word-a-device-without-designation").into(),
            ),
            ("location", ew_checks::text!("word-a-location").into()),
        ],
    );
    assert_eq!(
        render(&unplaced, "nl-NL"),
        "een apparaat zonder aanduiding is niet geplaatst in de layout van een locatie; plaats het op een rail of op de \
         montageplaat"
    );
    // A connection's junction end, and an end on another page, in Dutch.
    let unrouted = built(
        "EW-CORE-073",
        "",
        &[
            ("line", "wire".into()),
            (
                "wire",
                ew_checks::rules::page::connection_words(
                    ew_derive::text::junction_in("C11"),
                    ew_checks::text!("word-a-connection-point-of-another-page"),
                ),
            ),
        ],
    );
    assert_eq!(
        render(&unrouted, "nl-NL"),
        "Draad de aftakking in C11 → een aansluitpunt van een andere pagina heeft geen route: hij is als gestreepte \
         lijn getekend."
    );
    let pe = built(
        "EW-POT-006",
        "",
        &[
            ("wire", "101.3".into()),
            ("pe", "1.5".into()),
            ("required", "2.5".into()),
            ("line", "2.5".into()),
            ("at", "-X1:PE".into()),
            ("verified", "yes".into()),
        ],
    );
    assert_eq!(
        render(&pe, "nl-NL"),
        "PE-draad 101.3 heeft 1,5 mm², minder dan de 2,5 mm² die nodig is voor de lijngeleiders van 2,5 mm² bij -X1:PE. \
         Maak hem minstens 2,5 mm² in zijn Eigenschappen."
    );
}

/// Every message of every resource names a registered rule or an integrity rule, once; a language never has a pattern (a value) that en-GB
/// lacks, so a translated text always has the en-GB text it falls back to.
#[test]
fn every_message_names_a_registered_rule_once() {
    let known: BTreeSet<String> = rules()
        .iter()
        .map(|r| r.meta())
        .chain(INTEGRITY)
        .map(|m| m.message.to_owned())
        .collect();
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
    // EW-TERM-002: its pattern words the duplicate in each language.
    let m = model(vec![strip(1, 1), terminal(2, 1, 1, "1"), terminal(3, 1, 2, "1")]);
    let dup = one(&m, "EW-TERM-002");
    assert_eq!(dup.message(), "designation 1 is used by 2 terminals of -?X1");
    assert_eq!(render(&dup, DEFAULT_LANGUAGE), dup.message());
    assert_eq!(
        render(&dup, "nl-NL"),
        "aanduiding 1 wordt gebruikt door 2 klemmen van -?X1"
    );
    // An unknown language falls back to en-GB.
    assert_eq!(render(&dup, "fr-FR"), dup.message());
    assert_eq!(
        title("EW-TERM-002", "fr-FR").as_deref(),
        Some("Duplicate designation")
    );

    // A finding with arguments built by hand: the pattern's branches.
    let rule = ew_checks::rule("EW-POT-009").expect("registered").meta();
    for (unused, near, en, nl) in [
        (
            "no",
            "no",
            "-K1:13 has no wire. Draw one to it, or right-click it and choose Mark not used.",
            "-K1:13 heeft geen draad. Teken er een draad naartoe, of klik er met rechts op en kies Markeren als niet gebruikt.",
        ),
        (
            "yes",
            "no",
            "-K1:13 has no wire (marked not used)",
            "-K1:13 heeft geen draad (gemarkeerd als niet gebruikt)",
        ),
        (
            "no",
            "yes",
            "-K1:13 has no wire, and the mark 24V stands right beside it without joining it. Join the two with the \
             quick fix, or draw a wire to it.",
            "-K1:13 heeft geen draad, en de markering 24V staat er vlak naast zonder ermee te verbinden. Verbind de \
             twee met de snelle oplossing, of teken er een draad naartoe.",
        ),
    ] {
        let f = ew_checks::Finding::new(rule, [], "13", "-K1:13")
            .arg("cp", "-K1:13")
            .arg("cp_shared", "no")
            .arg("unused", unused)
            .arg("near", near)
            .arg("mark", if near == "yes" { "24V" } else { "" });
        assert_eq!(format(&f, DEFAULT_LANGUAGE).as_deref(), Some(en));
        assert_eq!(render(&f, DEFAULT_LANGUAGE), en);
        assert_eq!(render(&f, "nl-NL"), nl);
        assert_eq!(render(&f, "NL-nl"), nl, "language tags compare without case");
    }
    // A pin whose designation the device shares is named in words with its position, in each
    // language's ordinal, never by the label's `#n`.
    for (nth, en, nl) in [
        (2, "-1A4 pin − (2nd)", "-1A4 pen − (2e)"),
        (12, "-1A4 pin − (12th)", "-1A4 pen − (12e)"),
        (21, "-1A4 pin − (21st)", "-1A4 pen − (21e)"),
        (3, "-1A4 pin − (3rd)", "-1A4 pen − (3e)"),
    ] {
        let message =
            format!("{en} has no wire. Draw one to it, or right-click it and choose Mark not used.");
        let f = ew_checks::Finding::new(rule, [], "13", "-1A4:−#2")
            .arg("cp", "-1A4:−#2")
            .arg("cp_shared", "yes")
            .arg("cp_tag", "-1A4")
            .arg("cp_pin", "−")
            .arg("cp_nth", ew_l10n::TextArg::Number(nth))
            .arg("unused", "no")
            .arg("near", "no")
            .arg("mark", "");
        assert_eq!(format(&f, DEFAULT_LANGUAGE).as_deref(), Some(message.as_str()));
        let dutch = render(&f, "nl-NL");
        assert!(dutch.starts_with(&format!("{nl} heeft geen draad")), "{dutch}");
        assert!(!dutch.contains('#'), "{dutch}");
    }
    // A missing argument is an error in the pattern: the language falls back to the en-GB message.
    let partial = ew_checks::Finding::new(rule, [], "13", "-K1:13").arg("unused", "no");
    assert_eq!(render(&partial, "nl-NL"), render(&partial, DEFAULT_LANGUAGE));
    assert_eq!(
        messages::title("EW-POT-013", "nl-NL").as_deref(),
        Some("Schakelen in de nulleider")
    );
}

/// A finding of `rule` built by hand with `args`.
fn built(rule: &str, discriminator: &str, args: &[(&'static str, TextArg)]) -> ew_checks::Finding {
    let meta = ew_checks::rule(rule).expect("registered").meta();
    let mut f = ew_checks::Finding::new(meta, [], discriminator, "");
    for (k, v) in args {
        f = f.arg(k, v.clone());
    }
    f
}

/// EW-TERM-002's prevent finding carries its pattern's arguments, so nl-NL words it with each remedy.
#[test]
fn nl_nl_words_ew_term_002_s_prevent_finding() {
    for (remedy, free, old, en, nl) in [
        (
            "none",
            "",
            "",
            "designation 4 is taken on -X1 (2 terminals)",
            "aanduiding 4 is al bezet op -X1 (2 klemmen)",
        ),
        (
            "free",
            "21",
            "",
            "designation 4 is taken on -X1 (2 terminals); use 21",
            "aanduiding 4 is al bezet op -X1 (2 klemmen); gebruik 21",
        ),
        (
            "swap",
            "",
            "7",
            "designation 4 is taken on -X1 (2 terminals); swap 7 \u{2194} 4 in one transaction (terminal.designate)",
            "aanduiding 4 is al bezet op -X1 (2 klemmen); verwissel 7 \u{2194} 4 in één transactie (terminal.designate)",
        ),
        (
            "both",
            "21",
            "7",
            "designation 4 is taken on -X1 (2 terminals); use 21 or swap 7 \u{2194} 4 in one transaction \
             (terminal.designate)",
            "aanduiding 4 is al bezet op -X1 (2 klemmen); gebruik 21 of verwissel 7 \u{2194} 4 in één transactie \
             (terminal.designate)",
        ),
    ] {
        let f = built(
            "EW-TERM-002",
            "",
            &[
                ("form", "taken".into()),
                ("designation", "4".into()),
                ("count", "2".into()),
                ("strip", "-X1".into()),
                ("end", "named".into()),
                ("remedy", remedy.into()),
                ("free", free.into()),
                ("old", old.into()),
            ],
        );
        assert_eq!(f.message(), en, "{remedy}");
        assert_eq!(render(&f, "nl-NL"), nl, "{remedy}");
    }
}

/// EW-TERM-002 words a strip without a printed designation in each language, in both its forms.
#[test]
fn nl_nl_words_ew_term_002_s_untagged_strip() {
    for (form, en, nl) in [
        (
            "used",
            "designation 4 is used by 2 terminals of strip",
            "aanduiding 4 wordt gebruikt door 2 klemmen van de klemmenstrook",
        ),
        (
            "taken",
            "designation 4 is taken on the strip (2 terminals)",
            "aanduiding 4 is al bezet op de klemmenstrook (2 klemmen)",
        ),
    ] {
        let fallback = if form == "used" { "strip" } else { "the strip" };
        let f = built(
            "EW-TERM-002",
            "",
            &[
                ("form", form.into()),
                ("designation", "4".into()),
                ("count", "2".into()),
                ("strip", fallback.into()),
                ("end", "untagged".into()),
                ("remedy", "none".into()),
                ("free", "".into()),
                ("old", "".into()),
            ],
        );
        assert_eq!(f.message(), en, "{form}");
        assert_eq!(render(&f, "nl-NL"), nl, "{form}");
    }
}

/// EW-BUS-001 words an untagged port in each language, a TBD address's port and a duplicate's ports alike.
#[test]
fn nl_nl_words_ew_bus_001_s_untagged_ports() {
    for (args, en, nl) in [
        (
            &[("form", "tbd"), ("end", "connector"), ("port", "")][..],
            "a connector: the station address is TBD; enter it before release",
            "een connector: het stationsadres is TBD; vul het in vóór de vrijgave",
        ),
        (
            &[("form", "tbd"), ("end", "device"), ("port", "DP")][..],
            "a device:DP: the station address is TBD; enter it before release",
            "een apparaat:DP: het stationsadres is TBD; vul het in vóór de vrijgave",
        ),
        (
            &[("form", "tbd"), ("end", "named"), ("port", "-3X1")][..],
            "-3X1: the station address is TBD; enter it before release",
            "-3X1: het stationsadres is TBD; vul het in vóór de vrijgave",
        ),
        (
            &[
                ("form", "duplicate"),
                ("named", "some"),
                ("others", "none"),
                ("untagged", "0"),
                ("ports", "-3X1, -3X2"),
                ("address", "3"),
                ("net", "DP1"),
            ][..],
            "-3X1, -3X2: station address 3 is used more than once in logical net DP1; give each port its own address",
            "-3X1, -3X2: stationsadres 3 komt meer dan eens voor in logisch net DP1; geef elke poort een eigen adres",
        ),
        (
            &[
                ("form", "duplicate"),
                ("named", "some"),
                ("others", "one"),
                ("untagged", "1"),
                ("ports", "-3X1"),
                ("address", "3"),
                ("net", "DP1"),
            ][..],
            "-3X1 and a port without a tag: station address 3 is used more than once in logical net DP1; give each \
             port its own address",
            "-3X1 en een poort zonder kenmerk: stationsadres 3 komt meer dan eens voor in logisch net DP1; geef elke \
             poort een eigen adres",
        ),
        (
            &[
                ("form", "duplicate"),
                ("named", "none"),
                ("others", "many"),
                ("untagged", "2"),
                ("ports", ""),
                ("address", "3"),
                ("net", "DP1"),
            ][..],
            "2 ports without a tag: station address 3 is used more than once in logical net DP1; give each port its \
             own address",
            "2 poorten zonder kenmerk: stationsadres 3 komt meer dan eens voor in logisch net DP1; geef elke poort een \
             eigen adres",
        ),
    ] {
        let form = args[0].1;
        let f = built(
            "EW-BUS-001",
            if form == "tbd" { "address" } else { "duplicate" },
            &args
                .iter()
                .map(|(k, v)| (*k, TextArg::from(*v)))
                .collect::<Vec<_>>(),
        );
        assert_eq!(f.message(), en, "{args:?}");
        assert_eq!(render(&f, "nl-NL"), nl, "{args:?}");
    }
}

/// The uniqueness rule EW-POT-015 (and so EW-CORE-013 that reports it after a
/// merge), EW-BUS-004's segment ends and EW-POT-001's conflicts read in Dutch in nl-NL.
#[test]
fn nl_nl_words_the_uniqueness_rules_the_bus_ends_and_the_conflicts() {
    let number = built(
        "EW-POT-015",
        "",
        &[("number", "101.3".into()), ("count", "2".into())],
    );
    assert_eq!(
        render(&number, "nl-NL"),
        "Draadnummer 101.3 staat op 2 draden die niet één geleider zijn"
    );
    // EW-CORE-013 words the group by the uniqueness rule's pattern.
    let merge = built(
        "EW-CORE-013",
        "EW-POT-015",
        &[
            ("number", "101.3".into()),
            ("count", "2".into()),
            (
                "collision",
                built(
                    "EW-POT-015",
                    "",
                    &[("number", "101.3".into()), ("count", "2".into())],
                )
                .text
                .into(),
            ),
            ("keeper", "oldest".into()),
            ("others", "one".into()),
        ],
    );
    assert_eq!(
        render(&merge, "nl-NL"),
        "Draadnummer 101.3 staat op 2 draden die niet één geleider zijn na samenvoegen; het oudste record behoudt \
         het, en het andere moet wijzigen"
    );
    // EW-BUS-004: every kind of segment end.
    for (end, port, en, nl) in [
        (
            "ip",
            "DP",
            "end DP: no terminating element at this segment end; terminate the segment here",
            "einde DP: geen afsluitend element aan dit segmenteinde; sluit het segment hier af",
        ),
        (
            "connector",
            "",
            "a connector: no terminating element at this segment end; terminate the segment here",
            "een connector: geen afsluitend element aan dit segmenteinde; sluit het segment hier af",
        ),
        (
            "device",
            "DP",
            "a device:DP: no terminating element at this segment end; terminate the segment here",
            "een apparaat:DP: geen afsluitend element aan dit segmenteinde; sluit het segment hier af",
        ),
        (
            "segment",
            "",
            "a segment end: no terminating element at this segment end; terminate the segment here",
            "een segmenteinde: geen afsluitend element aan dit segmenteinde; sluit het segment hier af",
        ),
        (
            "named",
            "-3X1",
            "-3X1: no terminating element at this segment end; terminate the segment here",
            "-3X1: geen afsluitend element aan dit segmenteinde; sluit het segment hier af",
        ),
    ] {
        let f = built(
            "EW-BUS-004",
            "missing",
            &[
                ("form", "missing".into()),
                ("end", end.into()),
                ("port", port.into()),
            ],
        );
        assert_eq!(f.message(), en, "{end}");
        assert_eq!(render(&f, "nl-NL"), nl, "{end}");
    }
    // EW-POT-001: each shape of conflict, as its message and its fields (separated by tabs here: the message, the two
    // sides with what each is, the page), each kind a word by key.
    let conflicts = [
        (
            "conflict-type\t24VDC-1 at -X1:1\tphase L1\tL1 at -X2:1\tplus L+\t3",
            "24VDC-1 at -X1:1 (phase L1) is wired to L1 at -X2:1 (plus L+) on page 3: a wire joins two different \
             potentials. If one of them is typed wrongly, correct its type; otherwise the wire goes to the wrong place: \
             move it or take it away.",
            "24VDC-1 at -X1:1 (fase L1) is verbonden met L1 at -X2:1 (plus L+) op pagina 3: een draad verbindt twee \
             verschillende potentialen. Is een van beide verkeerd getypeerd, verbeter dan het type; anders zit de draad \
             op de verkeerde plaats: verplaats hem of haal hem weg.",
        ),
        (
            "conflict-voltage\t24VDC-1 at -X1:1\t230 V\t24VDC-2\t24.5 V\t",
            "24VDC-1 at -X1:1 (230 V) is wired to 24VDC-2 (24.5 V): one conductor cannot carry two voltages. If one \
             voltage is typed wrongly, correct it; otherwise the wire goes to the wrong place: move it or take it away.",
            "24VDC-1 at -X1:1 (230 V) is verbonden met 24VDC-2 (24,5 V): één geleider kan geen twee spanningen voeren. \
             Is een spanning verkeerd ingevuld, verbeter die dan; anders zit de draad op de verkeerde plaats: verplaats \
             hem of haal hem weg.",
        ),
        (
            "conflict-environment\tL at -X1:1\tAC\t24VDC-1 at -X2:1\tDC, floating\t1",
            "L at -X1:1 (AC) is wired to 24VDC-1 at -X2:1 (DC, floating) on page 1: one conductor cannot be both. If \
             one setting is wrong, correct it; otherwise the wire goes to the wrong place: move it or take it away.",
            "L at -X1:1 (AC) is verbonden met 24VDC-1 at -X2:1 (DC, zwevend) op pagina 1: één geleider kan niet \
             allebei zijn. Is een instelling verkeerd, verbeter die dan; anders zit de draad op de verkeerde plaats: \
             verplaats hem of haal hem weg.",
        ),
        (
            "conflict-class\tL at -X1:1\tmains circuit\tS at -X2:1\tcontrol circuit\t1",
            "L at -X1:1 (mains circuit) is wired to S at -X2:1 (control circuit) on page 1: one conductor cannot be \
             in two circuit classes. If one class is wrong, correct it; otherwise the wire goes to the wrong place: move \
             it or take it away.",
            "L at -X1:1 (netstroomkring) is verbonden met S at -X2:1 (stuurstroomkring) op pagina 1: één geleider kan \
             niet in twee stroomkringklassen zitten. Is een klasse verkeerd, verbeter die dan; anders zit de draad op de \
             verkeerde plaats: verplaats hem of haal hem weg.",
        ),
        (
            "conflict-role\t-K1:A1\tprotective earth PE\t24VDC at -X1:1\tplus L+\t2",
            "-K1:A1 (protective earth PE) is wired to 24VDC at -X1:1 (plus L+) on page 2: a wire joins two different \
             potentials. If the pin is wired to the wrong conductor, move its wire; if 24VDC at -X1:1 is typed wrongly, \
             correct its type.",
            "-K1:A1 (beschermingsaarde PE) is verbonden met 24VDC at -X1:1 (plus L+) op pagina 2: een draad verbindt \
             twee verschillende potentialen. Zit de pen op de verkeerde geleider, verplaats dan zijn draad; is 24VDC at \
             -X1:1 verkeerd getypeerd, verbeter dan het type.",
        ),
        (
            "conflict-roles\t-G1:+\tplus L+\t-X1:N\tneutral N\t3",
            "-G1:+ (plus L+) is wired to -X1:N (neutral N) on page 3: a wire joins two different potentials, so one of \
             the two pins is wired to the wrong conductor. Move that wire to the right one, or take it away.",
            "-G1:+ (plus L+) is verbonden met -X1:N (nulleider N) op pagina 3: een draad verbindt twee verschillende \
             potentialen, dus een van beide pennen zit op de verkeerde geleider. Verplaats die draad naar de juiste, of \
             haal hem weg.",
        ),
        (
            "conflict-earth\t-G1:−\t0 V (M)\t-X1:PE\tprotective earth PE\t3",
            "-G1:− (0 V) is joined to the protective earth -X1:PE on page 3, but not by a wire drawn from the 0 V \
             straight to an earth point. Select the wire that joins them and choose Draw ▸ Connect ▸ Bond, or take \
             that wire away.",
            "-G1:− (0 V) is verbonden met de beschermingsaarde -X1:PE op pagina 3, maar niet met een draad die \
             van de 0 V recht naar een aardpunt loopt. Selecteer de draad die ze verbindt en kies Tekenen ▸ Verbinden \
             ▸ Potentiaalverbinding, of haal die draad weg.",
        ),
    ];
    let kind = |k: &str| -> TextArg {
        use ew_checks::text;
        match k {
            "phase L1" => text!("word-type-phase", "L1").into(),
            "plus L+" => text!("word-type-l-plus").into(),
            "protective earth PE" => text!("word-type-pe").into(),
            "neutral N" => text!("word-type-n").into(),
            "mains circuit" => text!("word-class-mains").into(),
            "control circuit" => text!("word-class-control").into(),
            "DC, floating" => ew_l10n::List::new(
                vec!["DC".into(), text!("word-earthing-floating").into()],
                text!("join-comma"),
                text!("join-comma"),
            )
            .into(),
            other => other.into(),
        }
    };
    let pieces: Vec<TextArg> = conflicts
        .iter()
        .map(|(line, _, _)| {
            use ew_checks::text;
            let f: Vec<&str> = line.split('\t').collect();
            let (a, ak, b, bk) = (f[1], kind(f[2]), f[3], kind(f[4]));
            let page: TextArg = if f[5].is_empty() {
                "".into()
            } else {
                text!("pot-conflict-on-page", f[5]).into()
            };
            match f[0] {
                "conflict-type" => text!("pot-conflict-type", a, ak, b, bk, page),
                "conflict-voltage" => text!("pot-conflict-voltage", a, ak, b, bk, page),
                "conflict-environment" => text!("pot-conflict-environment", a, ak, b, bk, page),
                "conflict-class" => text!("pot-conflict-class", a, ak, b, bk, page),
                "conflict-role" => text!("pot-conflict-role", a, ak, b, bk, page),
                "conflict-roles" => text!("pot-conflict-roles", a, ak, b, bk, page),
                _ => text!("pot-conflict-earth", a, ak, b, bk, page),
            }
            .into()
        })
        .collect();
    let en: Vec<&str> = conflicts.iter().map(|(_, en, _)| *en).collect();
    let nl: Vec<&str> = conflicts.iter().map(|(_, _, nl)| *nl).collect();
    let sentence = || ew_checks::text!("join-sentence");
    let pot = built(
        "EW-POT-001",
        "attrs",
        &[
            ("form", "attrs".into()),
            (
                "conflicts",
                ew_l10n::List::new(pieces, sentence(), sentence()).into(),
            ),
            ("more", "2".into()),
        ],
    );
    assert_eq!(
        pot.message(),
        format!(
            "{} 2 more connection points are wired the same way.",
            en.join(" ")
        )
    );
    assert_eq!(
        render(&pot, "nl-NL"),
        format!(
            "{} Nog 2 aansluitpunten zijn op dezelfde manier bedraad.",
            nl.join(" ")
        )
    );
}
