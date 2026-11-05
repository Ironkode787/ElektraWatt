// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule messages in Fluent (doc 15 §3.8; WP-S1.16 D6, P5-1, P5-2).

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
    // The integrity rules of opening a project reach the Messages pane too (C51-p6#1): each has its title in every
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
            // FT-1422: and its hint, the remedy the finding's detail shows.
            assert!(has(m.id, Some("hint"), lang), "{lang} has no hint for {}", m.id);
        }
    }
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
    // EW-TERM-002 (FT-1334): its pattern words the duplicate in each language.
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

/// FT-1417: EW-TERM-002's prevent finding carries its pattern's arguments, so nl-NL words it with each remedy.
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

/// FT-1480: EW-TERM-002 words a strip without a printed designation in each language, in both its forms.
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

/// FT-1416: EW-BUS-001 words an untagged port in each language, a TBD address's port and a duplicate's ports alike.
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

/// FT-1334, FT-1370: the uniqueness rules EW-CORE-052 and EW-POT-015 (and so EW-CORE-013 that reports them after a
/// merge), EW-BUS-004's segment ends and EW-POT-001's conflicts read in Dutch in nl-NL.
#[test]
fn nl_nl_words_the_uniqueness_rules_the_bus_ends_and_the_conflicts() {
    let ip = by_hand(
        "EW-CORE-052",
        "star-source",
        "interruption point L+: 2 source points and no star source; set all but one to target, or flag one as the \
         star source",
        &[("form", "sources"), ("name", "L+"), ("count", "2")],
    );
    assert_eq!(
        render(&ip, "nl-NL"),
        "onderbrekingspunt L+: 2 bronpunten en geen sterbron; maak ze op één na tot doel, of markeer er één als \
         sterbron"
    );
    let flagged = by_hand(
        "EW-CORE-052",
        "star-source",
        "interruption point L+: 3 points are flagged as star source; keep the flag on one of them",
        &[("form", "flagged"), ("name", "L+"), ("count", "3")],
    );
    assert_eq!(
        render(&flagged, "nl-NL"),
        "onderbrekingspunt L+: 3 punten zijn als sterbron gemarkeerd; laat de markering op één ervan staan"
    );
    let number = by_hand(
        "EW-POT-015",
        "",
        "wire number 101.3 is printed on 2 wires; a wire number names one numbering unit",
        &[("number", "101.3"), ("count", "2")],
    );
    assert_eq!(
        render(&number, "nl-NL"),
        "draadnummer 101.3 staat op 2 draden; een draadnummer benoemt één nummeringseenheid"
    );
    // EW-CORE-013 words the group by the uniqueness rule's pattern.
    let merge = by_hand(
        "EW-CORE-013",
        "EW-POT-015",
        "wire number 101.3 is printed on 2 wires; a wire number names one numbering unit after a merge; the oldest \
         record keeps it, and the other must change",
        &[
            ("number", "101.3"),
            ("count", "2"),
            (
                "collision",
                "wire number 101.3 is printed on 2 wires; a wire number names one numbering unit",
            ),
            ("keeper", "oldest"),
            ("others", "one"),
        ],
    );
    assert_eq!(
        render(&merge, "nl-NL"),
        "draadnummer 101.3 staat op 2 draden; een draadnummer benoemt één nummeringseenheid na samenvoegen; het \
         oudste record behoudt het, en het andere moet wijzigen"
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
    // EW-POT-001: each shape of conflict, one per line of `conflict_list`.
    let conflicts = [
        (
            "24VDC-1 at -X1:1 sets type L1; LPlus applies (first definition in natural order)",
            "24VDC-1 at -X1:1 stelt type L1 in; LPlus geldt (eerste definitie in natuurlijke volgorde)",
        ),
        (
            "24VDC-1 at -X1:1 sets voltage 230 V; 24 V applies (first definition in natural order)",
            "24VDC-1 at -X1:1 stelt spanning 230 V in; 24 V geldt (eerste definitie in natuurlijke volgorde)",
        ),
        (
            "24VDC-1 at -X1:1 sets environment \"IT\"; \"TN-S\" applies (first definition in natural order)",
            "24VDC-1 at -X1:1 stelt omgeving \"IT\" in; \"TN-S\" geldt (eerste definitie in natuurlijke volgorde)",
        ),
        (
            "24VDC-1 at -X1:1 sets circuit class \"control\"; \"main\" applies (first definition in natural order)",
            "24VDC-1 at -X1:1 stelt stroomkringklasse \"control\" in; \"main\" geldt (eerste definitie in natuurlijke \
             volgorde)",
        ),
        (
            "member CP -K1:A1 has role Pe; the defined type LPlus applies",
            "aansluitpunt -K1:A1 heeft rol Pe; het gedefinieerde type LPlus geldt",
        ),
        (
            "member roles M, Pe meet LPlus; the first in role order applies",
            "aansluitpuntrollen M, Pe botsen met LPlus; de eerste in de rolvolgorde geldt",
        ),
    ];
    let en: Vec<&str> = conflicts.iter().map(|(en, _)| *en).collect();
    let nl: Vec<&str> = conflicts.iter().map(|(_, nl)| *nl).collect();
    let pot = by_hand(
        "EW-POT-001",
        "attrs",
        &format!("{}; and 2 more member CPs", en.join("; ")),
        &[
            ("conflicts", &en.join("; ")),
            ("conflict_list", &en.join("\n")),
            ("more", "2"),
            ("hint", "none"),
        ],
    );
    assert_eq!(
        render(&pot, "nl-NL"),
        format!("{}; en nog 2 aansluitpunten", nl.join("; "))
    );
    // A conflict of another shape keeps the en-GB conflicts.
    let odd = by_hand(
        "EW-POT-001",
        "attrs",
        "something else",
        &[
            ("conflicts", "something else"),
            ("conflict_list", "something else"),
            ("more", "none"),
            ("hint", "none"),
        ],
    );
    assert_eq!(render(&odd, "nl-NL"), "something else");
}
