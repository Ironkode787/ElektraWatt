// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule messages in Fluent (doc 15 §3.8; P5-1, P5-2).

use crate::common;

use std::collections::BTreeSet;

use common::*;
use ew_checks::messages::{
    self, DEFAULT_LANGUAGE, LANGUAGES, format, has, hint, message_id, render, resource_errors, title,
};
use ew_checks::rules;
use ew_checks::rules::core::INTEGRITY;
use serde_json::json;

const SOURCES: [(&str, &str); 2] = [
    ("en-GB", include_str!("../../messages/en-GB/rules.ftl")),
    ("nl-NL", include_str!("../../messages/nl-NL/rules.ftl")),
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
    // FT-680: every other shipped language has the title and the hint of every rule, and the pattern of every rule
    // whose en-GB message has one (the rule tables check that a finding carries its arguments, `assert_messages`).
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

/// FT-680: the rule system's own findings (a rule that stopped with an internal error, a pack whose data table does
/// not load), which no fixture raises, word their English and their Dutch from their arguments.
#[test]
fn nl_nl_words_the_rule_system_s_findings() {
    let scope = by_hand(
        "EW-CORE-083",
        "",
        "rule EW-POT-006 could not be evaluated for the project: it stopped with an internal error, so its findings \
         there are missing; report the defect",
        &[
            ("form", "scope"),
            ("rule", "EW-POT-006"),
            ("scope", "the project"),
        ],
    );
    assert_eq!(
        render(&scope, "nl-NL"),
        "regel EW-POT-006 kon niet worden beoordeeld voor het project: hij stopte met een interne fout, dus zijn \
         bevindingen daar ontbreken; meld het defect"
    );
    let edit = by_hand(
        "EW-CORE-083",
        "",
        "rule EW-POT-006 could not validate the edit: it stopped with an internal error, so the edit is not committed; \
         report the defect",
        &[("form", "prevent"), ("rule", "EW-POT-006")],
    );
    assert!(render(&edit, "nl-NL").starts_with("regel EW-POT-006 kon de bewerking niet toetsen"));
    let pack = by_hand(
        "EW-CORE-084",
        "",
        "pack EW-POT was not evaluated in full: its data table data/pe_sizing.toml does not load (`rows` is missing); \
         EW-POT-006 did not run",
        &[
            ("pack", "EW-POT"),
            ("file", "data/pe_sizing.toml"),
            ("error", "`rows` is missing"),
            ("rules", "EW-POT-006"),
        ],
    );
    assert!(render(&pack, "nl-NL").starts_with("pakket EW-POT is niet volledig beoordeeld"));
}

/// FT-912: every word a rule prints where a record has no name of its own (the en-GB messages `word-…`) has its text in
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
    let unplaced = by_hand(
        "EW-LAY-001",
        "",
        "a device without designation is not placed in the layout of a location; place it on a rail or on the mounting \
         plate",
        &[
            ("device", "a device without designation"),
            ("location", "a location"),
        ],
    );
    assert_eq!(
        render(&unplaced, "nl-NL"),
        "een apparaat zonder aanduiding is niet geplaatst in de layout van een locatie; plaats het op een rail of op de \
         montageplaat"
    );
    // FT-1600: a connection's junction end, and an end on another page, in Dutch.
    let unrouted = by_hand(
        "EW-CORE-073",
        "",
        "Wire the junction in C11 → a connection point of another page has no route: it is drawn as a dashed line.",
        &[
            ("line", "wire"),
            ("wire", "the junction in C11 → a connection point of another page"),
        ],
    );
    assert_eq!(
        render(&unrouted, "nl-NL"),
        "Draad de aftakking in C11 → een aansluitpunt van een andere pagina heeft geen route: hij is als gestreepte \
         lijn getekend."
    );
    let pe = by_hand(
        "EW-POT-006",
        "",
        "PE wire 101.3 has 1.5 mm², below the 2.5 mm² required for the 2.5 mm² line conductors at -X1:PE. Make it at \
         least 2.5 mm² in its Properties.",
        &[
            ("wire", "101.3"),
            ("pe", "1.5"),
            ("required", "2.5"),
            ("line", "2.5"),
            ("at", "-X1:PE"),
            ("verified", "yes"),
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
        .map(|m| message_id(m.id))
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
    assert_eq!(dup.message, "designation 1 is used by 2 terminals of -?X1");
    assert_eq!(render(&dup, DEFAULT_LANGUAGE), dup.message);
    assert_eq!(
        render(&dup, "nl-NL"),
        "aanduiding 1 wordt gebruikt door 2 klemmen van -?X1"
    );
    // An unknown language falls back to en-GB.
    assert_eq!(render(&dup, "fr-FR"), dup.message);
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
        let f = ew_checks::Finding::new(rule, [], "13", "-K1:13", en)
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
        ("2", "-1A4 pin − (2nd)", "-1A4 pen − (2e)"),
        ("12", "-1A4 pin − (12th)", "-1A4 pen − (12e)"),
        ("21", "-1A4 pin − (21st)", "-1A4 pen − (21e)"),
        ("3", "-1A4 pin − (3rd)", "-1A4 pen − (3e)"),
    ] {
        let message =
            format!("{en} has no wire. Draw one to it, or right-click it and choose Mark not used.");
        let f = ew_checks::Finding::new(rule, [], "13", "-1A4:−#2", message.clone())
            .arg("cp", "-1A4:−#2")
            .arg("cp_shared", "yes")
            .arg("cp_tag", "-1A4")
            .arg("cp_pin", "−")
            .arg("cp_nth", nth)
            .arg("unused", "no")
            .arg("near", "no")
            .arg("mark", "");
        assert_eq!(format(&f, DEFAULT_LANGUAGE).as_deref(), Some(message.as_str()));
        let dutch = render(&f, "nl-NL");
        assert!(dutch.starts_with(&format!("{nl} heeft geen draad")), "{dutch}");
        assert!(!dutch.contains('#'), "{dutch}");
    }
    // A missing argument is an error in the pattern: the en-GB message is kept.
    let partial = ew_checks::Finding::new(rule, [], "13", "-K1:13", "the en-GB text").arg("unused", "no");
    assert_eq!(render(&partial, "nl-NL"), "the en-GB text");
    assert_eq!(
        messages::title("EW-POT-013", "nl-NL").as_deref(),
        Some("Schakelen in de nulleider")
    );
}

/// A finding of `rule` built by hand with `args`, its en-GB message `en`: the en-GB pattern renders `en` byte for byte.
fn by_hand(rule: &str, discriminator: &str, en: &str, args: &[(&'static str, &str)]) -> ew_checks::Finding {
    let meta = ew_checks::rule(rule).expect("registered").meta();
    let mut f = ew_checks::Finding::new(meta, [], discriminator, "", en);
    for (k, v) in args {
        f = f.arg(k, *v);
    }
    assert_eq!(format(&f, DEFAULT_LANGUAGE).as_deref(), Some(en), "{rule}");
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
        let f = by_hand(
            "EW-TERM-002",
            "",
            en,
            &[
                ("form", "taken"),
                ("designation", "4"),
                ("count", "2"),
                ("strip", "-X1"),
                ("end", "named"),
                ("remedy", remedy),
                ("free", free),
                ("old", old),
            ],
        );
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
        let f = by_hand(
            "EW-TERM-002",
            "",
            en,
            &[
                ("form", form),
                ("designation", "4"),
                ("count", "2"),
                ("strip", fallback),
                ("end", "untagged"),
                ("remedy", "none"),
                ("free", ""),
                ("old", ""),
            ],
        );
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
        let f = by_hand(
            "EW-BUS-001",
            if form == "tbd" { "address" } else { "duplicate" },
            en,
            args,
        );
        assert_eq!(render(&f, "nl-NL"), nl, "{args:?}");
    }
}

/// The uniqueness rule EW-POT-015 (and so EW-CORE-013 that reports it after a
/// merge), EW-BUS-004's segment ends and EW-POT-001's conflicts read in Dutch in nl-NL.
#[test]
fn nl_nl_words_the_uniqueness_rules_the_bus_ends_and_the_conflicts() {
    let number = by_hand(
        "EW-POT-015",
        "",
        "Wire number 101.3 is printed on 2 wires that are not one conductor",
        &[("number", "101.3"), ("count", "2")],
    );
    assert_eq!(
        render(&number, "nl-NL"),
        "Draadnummer 101.3 staat op 2 draden die niet één geleider zijn"
    );
    // EW-CORE-013 words the group by the uniqueness rule's pattern.
    let merge = by_hand(
        "EW-CORE-013",
        "EW-POT-015",
        "Wire number 101.3 is printed on 2 wires that are not one conductor after a merge; the oldest record keeps \
         it, and the other must change",
        &[
            ("number", "101.3"),
            ("count", "2"),
            (
                "collision",
                "Wire number 101.3 is printed on 2 wires that are not one conductor",
            ),
            ("keeper", "oldest"),
            ("others", "one"),
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
        let f = by_hand(
            "EW-BUS-004",
            "missing",
            en,
            &[("form", "missing"), ("end", end), ("port", port)],
        );
        assert_eq!(render(&f, "nl-NL"), nl, "{end}");
    }
    // EW-POT-001: each shape of conflict, one per line of `conflict_list` (its fields separated by tabs: the attribute,
    // the two sides with what each is, the page).
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
    let lines: Vec<&str> = conflicts.iter().map(|(line, _, _)| *line).collect();
    let en: Vec<&str> = conflicts.iter().map(|(_, en, _)| *en).collect();
    let nl: Vec<&str> = conflicts.iter().map(|(_, _, nl)| *nl).collect();
    let pot = by_hand(
        "EW-POT-001",
        "attrs",
        &format!(
            "{} 2 more connection points are wired the same way.",
            en.join(" ")
        ),
        &[
            ("form", "attrs"),
            ("conflict_list", &lines.join("\n")),
            ("more", "2"),
        ],
    );
    assert_eq!(
        render(&pot, "nl-NL"),
        format!(
            "{} Nog 2 aansluitpunten zijn op dezelfde manier bedraad.",
            nl.join(" ")
        )
    );
    // A conflict of another shape keeps the en-GB message.
    let meta = ew_checks::rule("EW-POT-001").expect("registered").meta();
    let odd = ew_checks::Finding::new(meta, [], "attrs", "", "something else")
        .arg("form", "attrs")
        .arg("conflict_list", "something else")
        .arg("more", "none");
    assert_eq!(render(&odd, "nl-NL"), "something else");
}

// Every quick fix a rule offers names its key beside its English label; the key's en-GB message
/// is that label byte for byte (the CLI's text), nl-NL words it in Dutch, and a fix without a key keeps its label.
#[test]
fn every_quick_fix_label_has_its_key_in_both_languages() {
    let sources = [
        include_str!("../../src/rules/core.rs"),
        include_str!("../../src/rules/imp.rs"),
        include_str!("../../src/rules/layout.rs"),
        include_str!("../../src/rules/plc.rs"),
        include_str!("../../src/rules/pot.rs"),
    ];
    let mut pairs = Vec::new();
    for source in sources {
        let lines: Vec<&str> = source.lines().map(str::trim).collect();
        for w in lines.windows(2) {
            if let (Some(label), Some(key)) = (
                w[0].strip_prefix("label: \"")
                    .and_then(|l| l.strip_suffix("\".to_owned(),")),
                w[1].strip_prefix("key: Some(\"")
                    .and_then(|k| k.strip_suffix("\"),")),
            ) {
                pairs.push((label.to_owned(), key.to_owned()));
            }
        }
    }
    assert!(pairs.iter().any(|(l, _)| l == "Renumber the newer"), "{pairs:#?}");
    for (label, key) in pairs {
        let key: &'static str = Box::leak(key.into_boxed_str());
        let fix = |key: Option<&'static str>| ew_checks::QuickFix {
            label: label.clone(),
            key,
            command: String::new(),
            params: json!(null),
        };
        assert_eq!(
            messages::fix_label(&fix(Some(key)), DEFAULT_LANGUAGE),
            label,
            "{key}"
        );
        assert_ne!(
            messages::fix_label(&fix(Some(key)), "nl-NL"),
            label,
            "{key} has its nl-NL text"
        );
        assert_eq!(messages::fix_label(&fix(None), "nl-NL"), label);
    }
    let unfreeze = ew_checks::QuickFix {
        label: "Unfreeze the tag".to_owned(),
        key: Some("fix-unfreeze-tag"),
        command: String::new(),
        params: json!(null),
    };
    assert_eq!(messages::fix_label(&unfreeze, "nl-NL"), "Kenmerk losmaken");
}
