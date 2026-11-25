// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The printed text of a value of the domain (`ew_derive::text`): one form each, wherever it prints.

use ew_schema::{ColourSpec, Csa};

/// A gauge from 1/0 up prints as the wire trade writes it, `AWG2/0`; a smaller wire as its number.
#[test]
fn a_heavy_gauge_prints_with_its_zeros() {
    assert_eq!(ew_derive::csa_text(&Csa::Awg(-1)).as_deref(), Some("AWG2/0"));
    assert_eq!(ew_derive::csa_text(&Csa::Awg(0)).as_deref(), Some("AWG1/0"));
    assert_eq!(ew_derive::csa_text(&Csa::Awg(18)).as_deref(), Some("AWG18"));
}

/// A colour prints its codes as written, a code this build does not know as `?`, in every place that prints it.
#[test]
fn an_unknown_colour_code_prints_one_word() {
    let colour = |v: serde_json::Value| -> String {
        ew_derive::colour_text(&serde_json::from_value::<ColourSpec>(v).expect("a colour"))
    };
    assert_eq!(colour(serde_json::json!(["GN", "YE"])), "GNYE");
    assert_eq!(
        colour(serde_json::json!([{"foreign": "ROSA"}, {"newer": 1}])),
        "ROSA?"
    );
}

/// A potential type of a newer format prints as the SQL view publishes it, `other`, in the checks' messages too; an
/// undefined type prints `undefined`.
#[test]
fn a_newer_potential_type_prints_other() {
    use ew_schema::PotentialType;
    assert_eq!(
        ew_derive::text::potential_type(&PotentialType::Unknown(serde_json::json!("x"))).to_string(),
        "other"
    );
    assert_eq!(
        ew_derive::text::potential_type(&PotentialType::Undefined).to_string(),
        "undefined"
    );
    assert_eq!(
        ew_derive::text::potential_type(&PotentialType::LPlus).to_string(),
        "L+"
    );
}

/// A cross-section with its unit is the lists' form, where Properties and the checks' messages print one.
#[test]
fn a_cross_section_with_its_unit_is_the_lists_form() {
    let dec = |s: &str| -> Csa { serde_json::from_value(serde_json::json!(s)).expect("a cross-section") };
    assert_eq!(ew_derive::csa_with_unit(&dec("2.5")).as_deref(), Some("2.5 mm²"));
    assert_eq!(ew_derive::csa_with_unit(&Csa::Awg(-1)).as_deref(), Some("AWG2/0"));
    assert_eq!(
        ew_derive::csa_text(&dec("2.5")).as_deref(),
        Some("2.5"),
        "a wire marker's form"
    );
}

/// A channel without a derived address is "no address" in a sentence, in the reader's language.
#[test]
fn a_channel_without_an_address_is_worded_in_the_readers_language() {
    let ew_l10n::TextArg::Text(words) = ew_derive::channel_address_words(None) else {
        panic!("words");
    };
    let ml = ew_derive::text::in_languages(&words);
    let tag = |t: &str| t.parse::<ew_mltext::LangTag>().expect("a tag");
    assert_eq!(ml.get(&tag("en-GB")), Some("no address"));
    assert_eq!(ml.get(&tag("nl-NL")), Some("geen adres"));
    assert_eq!(
        ew_derive::channel_address_words(Some(&ew_derive::ChannelAddr::Tbd)).to_string(),
        "TBD"
    );
}

/// A record kind has one table of words: a representation is a symbol, a connection (a wire, a core, a bridge, a bus
/// line or a tube) a connection, a reserved kind has its words as well, and a name no kind has is an item.
#[test]
fn a_record_kind_is_worded_by_one_table() {
    use ew_schema::Kind;
    let words = |k: Kind| ew_derive::text::record(k).to_string();
    assert_eq!(words(Kind::Representation), "a symbol");
    assert_eq!(words(Kind::Connection), "a connection");
    assert_eq!(words(Kind::Enclosure), "an enclosure");
    assert_eq!(words(Kind::Comment), "a comment");
    assert_eq!(words(Kind::NetDef), "a net definition");
    let [a, one, many] = ew_derive::text::record_words_named("frobnicator");
    assert_eq!(
        [a, one, many].map(|t| t.to_string()),
        ["an item", "item", "items"]
    );
}

/// A kind has three forms in the one table: with its article for a sentence, its name, the bare noun, for a cell or a
/// label, and its plural for a heading of many, in both languages.
#[test]
fn a_record_kind_has_its_name_beside_its_article_form() {
    use ew_schema::Kind;
    let tag = |t: &str| t.parse::<ew_mltext::LangTag>().expect("a tag");
    let both = |t: ew_l10n::Text| {
        let ml = ew_derive::text::in_languages(&t);
        (
            ml.get(&tag("en-GB")).unwrap_or_default().to_owned(),
            ml.get(&tag("nl-NL")).unwrap_or_default().to_owned(),
        )
    };
    let name = |k: Kind| both(ew_derive::text::record_name(k));
    assert_eq!(name(Kind::Part), ("part".into(), "artikel".into()));
    assert_eq!(
        name(Kind::PlcProfile),
        ("PLC profile".into(), "PLC-profiel".into())
    );
    assert_eq!(
        name(Kind::StructureNode),
        ("structure node".into(), "structuurknooppunt".into())
    );
    assert_eq!(name(Kind::Comment), ("comment".into(), "opmerking".into()));
    assert_eq!(
        both(ew_derive::text::record_names(Kind::StructureBox)),
        ("structure boxes".into(), "structuurkaders".into())
    );
    assert_eq!(
        both(ew_derive::text::record(Kind::Part)),
        ("a part".into(), "een artikel".into())
    );
}
