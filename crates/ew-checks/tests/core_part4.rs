// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.16 part 4: the EW-CORE, EW-RDS, EW-NUM and EW-TERM rules of pages, texts, designations and terminals
//! (EW-CORE-021, -022, -025, -026, -061, -063, -071; EW-RDS-001, -002, -003, -007, -009, -010; EW-NUM-002; EW-TERM-003,
//! -004), EW-CORE-008's `channel` half (FT-229) and EW-CORE-052's prevent mode (FT-194). One positive and one negative
//! fixture per rule (P4-1), the prevent rules refusing the candidate and only it, and the ID-permutation test of
//! WP-S1.16 §10. EW-CORE-083 and -084 (the rule system) are tested in the engine's unit tests (P4-3).

mod common;

use common::*;
use ew_checks::{CheckMode, FULL, Finding, Severity, prevent, run};
use ew_derive::Q;
use ew_derive::ew_schema::{AnyId, Kind};
use ew_model::Model;
use serde_json::{Value, json};

/// `(subject, message)` of each finding, in report order.
fn texts(f: &[Finding]) -> Vec<(String, String)> {
    f.iter().map(|f| (f.subject.clone(), f.message.clone())).collect()
}

fn subjects(f: &[Finding]) -> Vec<String> {
    f.iter().map(|f| f.subject.clone()).collect()
}

fn targets(f: &Finding) -> Vec<(Kind, AnyId)> {
    f.targets.iter().map(|t| (t.kind, t.id)).collect()
}

/// The findings of `rule` in a release run of the `full` scheme.
fn release_findings_of(m: &Model, rule: &str) -> Vec<Finding> {
    run(&Q::new(m), &FULL, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Pages: EW-CORE-021, -022, -025, -026

const S1: u128 = 0x8000;
const PA: u128 = 0x8101;
const PB: u128 = 0x8102;

/// Section 1 (folios 100 to 199) with two pages of folios `a` and `b`.
fn folios_fx(a: u32, b: u32) -> Fx {
    let mut fx = Fx::new();
    fx.push(json!({
        "$k": "section", "id": us(S1), "number": 1, "folio_range": [100, 199], "title": {"en": "Section"},
    }));
    for (n, folio) in [(PA, a), (PB, b)] {
        fx.push(json!({
            "$k": "page", "id": us(n), "folio": {"number": folio, "section": us(S1)}, "kind": "multi_line",
            "structure": {}, "title": {"en": "page"}, "grid": {"num": 4_000_000, "den": 1},
        }));
    }
    fx
}

/// EW-CORE-021: a page whose folio lies outside its section's range is an error on that page; folios inside the
/// range are not reported.
#[test]
fn core021_reports_a_folio_outside_its_section() {
    let f = findings_of(&folios_fx(101, 250).build(), "EW-CORE-021");
    assert_eq!(
        texts(&f),
        [(
            "folio 250".to_owned(),
            "folio 250 lies outside the folio range 100–199 of section 1; renumber the page or move it to the \
             section whose range holds 250"
                .to_owned()
        )]
    );
    assert_eq!(f[0].code, "101021");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "range");
    assert_eq!(targets(&f[0]), [(Kind::Section, uid(S1)), (Kind::Page, uid(PB))]);
    assert!(findings_of(&folios_fx(101, 199).build(), "EW-CORE-021").is_empty());
}

/// EW-CORE-022: two pages printing one folio are one error naming both; distinct folios are not reported.
#[test]
fn core022_reports_a_folio_printed_twice() {
    let f = findings_of(&folios_fx(101, 101).build(), "EW-CORE-022");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "101022");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "folio");
    assert_eq!(targets(&f[0]), [(Kind::Page, uid(PA)), (Kind::Page, uid(PB))]);
    assert!(
        f[0].message.starts_with("folio 101 is printed by 2 pages"),
        "{}",
        f[0].message
    );
    assert!(findings_of(&folios_fx(101, 102).build(), "EW-CORE-022").is_empty());
}

/// EW-CORE-022 prevent: renumbering a page onto another page's folio is refused; touching a page of a collision the
/// model already holds, without changing it, is not.
#[test]
fn core022_prevent_refuses_a_new_collision_only() {
    let before = folios_fx(101, 102).build();
    let after = folios_fx(101, 101).build();
    let key = (Kind::Page, uid(PB));
    let v = prevent(&Q::new(&before), &Q::new(&after), &[key]);
    assert_eq!(v.len(), 1, "{v:#?}");
    assert_eq!(v[0].rule, "EW-CORE-022");
    let v = prevent(&Q::new(&after), &Q::new(&after), &[key]);
    assert!(v.is_empty(), "{v:#?}");
    // A touch of another kind does not run the rule.
    let v = prevent(&Q::new(&before), &Q::new(&after), &[dev_key(0x1)]);
    assert!(v.iter().all(|f| f.rule != "EW-CORE-022"), "{v:#?}");
}

/// A strip `-X<counter>` with ID `n` and one terminal `n + 1`.
fn strip_with_terminal(fx: &mut Fx, n: u128, counter: u32) {
    fx.push(json!({"$k": "device", "id": us(n), "tag": {"letter": "X", "counter": counter}, "strip": {}}));
    fx.push(json!({
        "$k": "device", "id": us(n + 1), "tag": {"letter": "X"}, "kind": "terminal",
        "terminal": {"strip": us(n), "seq": 1, "designation": "1"},
    }));
}

/// Two strips -X1 and -X2 and a terminal-diagram block whose manual selection is `manual`.
fn coverage_fx(manual: Value) -> Fx {
    let mut fx = folios_fx(101, 102);
    strip_with_terminal(&mut fx, 0x8200, 1);
    strip_with_terminal(&mut fx, 0x8300, 2);
    fx.form(0x8401);
    fx.push(json!({
        "$k": "report_block", "id": us(0x8400), "source": {"kind": "terminal_diagram"}, "manual": manual,
        "form": us(0x8401), "section": us(S1), "first_folio": 150,
    }));
    fx
}

/// EW-CORE-026: a strip a terminal-diagram block's selection leaves out is an info online and a warning in release;
/// a block over every strip leaves nothing out.
#[test]
fn core026_reports_a_strip_no_terminal_diagram_covers() {
    let fx = coverage_fx(json!({"include": [us(0x8200)]}));
    let f = findings_of(&fx.build(), "EW-CORE-026");
    assert_eq!(subjects(&f), ["-?X2"], "{f:#?}");
    assert_eq!(f[0].code, "101026");
    assert_eq!(f[0].severity, Severity::Info);
    assert_eq!(f[0].discriminator, "terminal_diagram");
    assert_eq!(targets(&f[0]), [(Kind::Device, uid(0x8300))]);
    let r = release_findings_of(&fx.build(), "EW-CORE-026");
    assert_eq!(r[0].severity, Severity::Warning);
    // An excluded strip is left out as well; a block without a selection covers every strip.
    let f = findings_of(
        &coverage_fx(json!({"exclude": [us(0x8200)]})).build(),
        "EW-CORE-026",
    );
    assert_eq!(subjects(&f), ["-?X1"], "{f:#?}");
    assert!(findings_of(&coverage_fx(json!({})).build(), "EW-CORE-026").is_empty());
}

/// EW-CORE-025: a table of contents whose page count settles reports nothing (the unsettled case needs a form whose
/// rows per page are at most the number of TOC blocks; the derivation's own tests cover the fixpoint).
#[test]
fn core025_a_settled_toc_reports_nothing() {
    let mut fx = folios_fx(101, 102);
    fx.form(0x8501);
    fx.push(json!({
        "$k": "report_block", "id": us(0x8500), "source": {"kind": "toc"}, "form": us(0x8501),
        "section": us(S1), "first_folio": 150,
    }));
    let m = fx.build();
    assert!(findings_of(&m, "EW-CORE-025").is_empty());
    assert!(release_findings_of(&m, "EW-CORE-025").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// Texts and graphics: EW-CORE-061, -063, -071

/// The message of EW-CORE-061 for a style of 0.94 mm.
fn small_text(key: &str) -> (String, String) {
    (
        key.to_owned(),
        format!("text style {key} prints with a cap height of 0.94 mm, below the minimum of 1.20 mm"),
    )
}

/// Two functions on P1 (0x1001 and 0x1002) whose symbol has the text slots `slots`.
fn slots_fx(slots: Value) -> Fx {
    let mut fx = Fx::base();
    fx.with_symbol_slots(slots);
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx
}

/// The CP-designation slots of both CPs, shown when `visible`.
fn cp_slots(visible: bool) -> Value {
    json!([
        {"kind": {"cp_designation": 0}, "pos": [-1_000_000, 0], "align": "middle_right", "visible": visible},
        {"kind": {"cp_designation": 1}, "pos": [-1_000_000, 1_000_000], "align": "middle_right", "visible": visible},
    ])
}

/// EW-CORE-061 (D-31, EF-12): only a text style the project prints with is reported; a project without drawn texts
/// (a new one, or one whose symbols carry no texts) reports none, though `cp.small` and `marker` are below the
/// minimum.
#[test]
fn core061_reports_nothing_without_drawn_texts() {
    assert!(findings_of(&Fx::new().build(), "EW-CORE-061").is_empty());
    assert!(findings_of(&Fx::base().build(), "EW-CORE-061").is_empty());
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x3001, P1, cp(0x1001), cp(0x1002), &json!({}));
    assert!(findings_of(&fx.build(), "EW-CORE-061").is_empty());
    // Hidden CP-designation slots print nothing.
    assert!(findings_of(&slots_fx(cp_slots(false)).build(), "EW-CORE-061").is_empty());
}

/// EW-CORE-061: drawn CP labels (`cp.small`) give one finding for that style, however many labels there are, with no
/// target (FR-CHK-051); the other small style (`marker`) is not drawn and not reported.
#[test]
fn core061_reports_a_drawn_cp_label_style_once() {
    let f = findings_of(&slots_fx(cp_slots(true)).build(), "EW-CORE-061");
    assert_eq!(texts(&f), [small_text("cp.small")]);
    assert_eq!(f[0].code, "104061");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "cp.small");
    assert!(f[0].targets.is_empty());
    // A slot that names a larger style of the table prints with that style instead.
    let restyled = json!([
        {"kind": {"cp_designation": 0}, "pos": [0, 0], "align": "middle_right", "visible": true, "style": "pin"},
    ]);
    assert!(findings_of(&slots_fx(restyled).build(), "EW-CORE-061").is_empty());
}

/// EW-CORE-061: a shown wire marker prints in `marker`; with the CP labels both small styles are reported, in the
/// table's order.
#[test]
fn core061_reports_a_shown_wire_marker() {
    let mut fx = slots_fx(cp_slots(true));
    fx.conn(
        0x3001,
        P1,
        cp(0x1001),
        cp(0x1002),
        &json!({"overrides": {"csa": "2.5", "colour": ["BK"], "show_marker": true}}),
    );
    let f = findings_of(&fx.build(), "EW-CORE-061");
    assert_eq!(texts(&f), [small_text("cp.small"), small_text("marker")]);
    // Every other style of the table is at or above the minimum.
    let small: Vec<&str> = ew_checks::rules::page::TEXT_STYLES
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !["cp.small", "marker"].contains(k))
        .collect();
    assert!(small.iter().all(|k| f.iter().all(|x| x.subject != *k)));
}

/// A free text on P1 with the style `style` (none: the default).
fn text_fx(style: Option<&str>) -> Fx {
    let mut fx = Fx::base();
    let mut t = json!({"text": {"en": "Note"}, "at": [0, 0]});
    if let Some(s) = style {
        t["style"] = json!(s);
    }
    fx.push(json!({"$k": "page_item", "id": us(0x8600), "page": us(P1), "kind": {"free_text": t}}));
    fx
}

/// EW-CORE-063: a free text with a size outside the text-style table (an imported `qet.9`) is an info on its page;
/// a text of a table style or of the default style is not.
#[test]
fn core063_reports_a_text_outside_the_style_table() {
    let f = findings_of(&text_fx(Some("qet.9")).build(), "EW-CORE-063");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "104063");
    assert_eq!(f[0].severity, Severity::Info);
    assert_eq!(f[0].discriminator, "style");
    assert_eq!(targets(&f[0]), [(Kind::PageItem, uid(0x8600))]);
    assert!(f[0].message.contains("qet.9"), "{}", f[0].message);
    assert!(findings_of(&text_fx(Some("text.normal")).build(), "EW-CORE-063").is_empty());
    assert!(findings_of(&text_fx(None).build(), "EW-CORE-063").is_empty());
}

/// A line on P1 from the connection point of function 0x1001 (CP 0 at `(0, 0x1001 mm)`) to `b`, offset by `dy` nm.
fn line_fx(dy: i64) -> Fx {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    let y = 0x1001_i64 * 1_000_000 + dy;
    fx.push(json!({
        "$k": "page_item", "id": us(0x8700), "page": us(P1),
        "kind": {"graphic": {"shape": {"line": {"a": [0, y], "b": [8_000_000, y]}}}},
    }));
    fx
}

/// EW-CORE-071: a graphic line that ends on a connection point is an info (it looks like a wire and is none); a
/// line that ends beside it is not.
#[test]
fn core071_reports_a_graphic_line_on_a_connection_point() {
    let f = findings_of(&line_fx(0).build(), "EW-CORE-071");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "104071");
    assert_eq!(f[0].severity, Severity::Info);
    assert!(targets(&f[0]).contains(&(Kind::PageItem, uid(0x8700))), "{f:#?}");
    assert!(f[0].fix.is_none());
    assert!(findings_of(&line_fx(1_000).build(), "EW-CORE-071").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-CORE-008 `channel` (FT-229) and EW-CORE-052 prevent (FT-194)

/// A device `-A1` (a PLC module when `module`) with channel 0.
fn channel_fx(module: bool) -> Fx {
    let mut fx = Fx::new();
    let dev = 0x500u128;
    let mut d = json!({"$k": "device", "id": us(dev), "tag": {"letter": "A", "counter": 1}});
    if module {
        d["plc_module"] = json!({});
    }
    fx.push(d);
    let channel: AnyId = ew_derive::ew_ids::v5::plc_channel_id(uid(dev), 0);
    fx.push(
        json!({"$k": "channel", "id": channel.to_string(), "module": us(dev), "number": 0, "kind": "di"}),
    );
    fx
}

/// FT-229: a channel whose module device is no PLC module is EW-CORE-008 `channel` in release mode; a module's
/// channel is not.
#[test]
fn core008_reports_a_channel_of_a_device_that_is_no_module() {
    let f = release_findings_of(&channel_fx(false).build(), "EW-CORE-008");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].discriminator, "channel");
    assert_eq!(f[0].severity, Severity::Error);
    // The device is drawn on no page, so its designation keeps the open page number.
    assert_eq!(f[0].subject, "-?A1:0");
    assert_eq!(
        f[0].message,
        "channel -?A1:0 belongs to -?A1, which is not a PLC module; delete the channel or make -?A1 a module again"
    );
    assert_eq!(f[0].targets.len(), 2);
    assert!(release_findings_of(&channel_fx(true).build(), "EW-CORE-008").is_empty());
}

/// Interruption points named `L-1`: a source per entry of `sources`.
fn ips_fx(sources: &[u128]) -> Fx {
    let mut fx = Fx::base();
    for n in sources {
        fx.ip(*n, P1, "L-1", "source");
    }
    fx.ip(0x3009, P2, "L-1", "target");
    fx
}

/// FT-194: EW-CORE-052 refuses a second source point of a name; touching the points of an ambiguity the model
/// already holds, without a change, is not refused, and a touch of an ordinary device does not run the rule.
#[test]
fn core052_prevent_refuses_a_new_ambiguous_group_only() {
    let before = ips_fx(&[0x3001]).build();
    let after = ips_fx(&[0x3001, 0x3002]).build();
    let key = (Kind::Function, uid(0x3002));
    let v = prevent(&Q::new(&before), &Q::new(&after), &[key]);
    assert_eq!(v.len(), 1, "{v:#?}");
    assert_eq!(v[0].rule, "EW-CORE-052");
    let v = prevent(&Q::new(&after), &Q::new(&after), &[key]);
    assert!(v.is_empty(), "{v:#?}");
    let v = prevent(&Q::new(&before), &Q::new(&after), &[dev_key(0x1)]);
    assert!(v.is_empty(), "{v:#?}");
}

// ---------------------------------------------------------------------------------------------------------
// Designations: EW-RDS-001, -002, -003, -007, -009, -010

/// A drawn device `-1<letter>1` (ID 0x101) in the location `code` (none: no location).
fn designation_fx(letter: &str, code: Option<&str>) -> Fx {
    let mut fx = Fx::base();
    fx.drawn_device(0x101, letter, Some(1), P1);
    if let Some(c) = code {
        fx.push(json!({"$k": "structure_node", "id": us(0x900), "aspect": "location", "code": c}));
        let d = fx
            .records
            .iter_mut()
            .find(|r| r["id"] == json!(us(0x101)))
            .expect("the device");
        d["structure"] = json!({"location": us(0x900)});
    }
    fx
}

/// EW-RDS-001: a designation that does not parse (a lower-case letter code) is an error on the device; a
/// designation that parses is not.
#[test]
fn rds001_reports_a_designation_that_does_not_parse() {
    let f = findings_of(&designation_fx("k", None).build(), "EW-RDS-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017001");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "parse");
    assert_eq!(targets(&f[0]), [(Kind::Device, uid(0x101))]);
    assert!(
        f[0].message
            .contains("does not parse as an IEC 81346 designation"),
        "{}",
        f[0].message
    );
    assert!(findings_of(&designation_fx("K", Some("A1")).build(), "EW-RDS-001").is_empty());
}

/// EW-RDS-002: a structure code with separator characters is an info on its node; a plain code is not.
#[test]
fn rds002_reports_a_separator_in_a_structure_code() {
    let f = findings_of(&designation_fx("K", Some("J-E-206")).build(), "EW-RDS-002");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017002");
    assert_eq!(f[0].severity, Severity::Info);
    assert_eq!(f[0].subject, "+J-E-206");
    assert_eq!(targets(&f[0]), [(Kind::StructureNode, uid(0x900))]);
    assert!(findings_of(&designation_fx("K", Some("JE206")).build(), "EW-RDS-002").is_empty());
}

/// A device `-<letter>1` (ID 0x101) whose main function's definition has the category `category`.
fn letter_fx(letter: &str, category: &str) -> Fx {
    let mut fx = Fx::base();
    fx.fndef_with(
        0x520,
        0x521,
        &[(0, "1", Value::Null), (1, "2", Value::Null)],
        &json!([]),
        &json!({"category": [category, "Generic", "Plain"]}),
    );
    fx.device(0x101, letter, Some(1));
    fx.push(json!({"$k": "function", "id": us(0x1101), "device": us(0x101), "def": us(0x520)}));
    fx.push(json!({
        "$k": "representation", "id": us(0x11101), "functions": [us(0x1101)], "page": us(P1),
        "symbol": us(0x521), "pos": [0, 0],
    }));
    fx
}

/// EW-RDS-003 (EF-18): a letter code that does not match the main function's category is a warning naming the
/// identifier set's letter; a matching letter, or a generic assembly definition, is not reported.
#[test]
fn rds003_reports_a_letter_against_the_function_category() {
    let f = findings_of(&letter_fx("T", "SensingTemperature").build(), "EW-RDS-003");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017003");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "letter");
    assert!(
        f[0].message.contains(
            "the letter code T does not match its function category SensingTemperature; the identifier set \
             EN81346-2:2009 suggests B"
        ),
        "{}",
        f[0].message
    );
    assert!(findings_of(&letter_fx("B", "SensingTemperature").build(), "EW-RDS-003").is_empty());
    assert!(findings_of(&letter_fx("U", "Assembly").build(), "EW-RDS-003").is_empty());
}

/// EW-RDS-007: a letter code with I or O is a warning; an ordinary letter code is not.
#[test]
fn rds007_reports_reserved_letters() {
    let f = findings_of(&designation_fx("KO", None).build(), "EW-RDS-007");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017007");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "reserved");
    assert!(
        f[0].message.contains("O is not used in letter codes"),
        "{}",
        f[0].message
    );
    assert!(findings_of(&designation_fx("KF", None).build(), "EW-RDS-007").is_empty());
}

/// EW-RDS-009: a letter code with a Cyrillic look-alike of a Latin letter is an error; Latin letters are not
/// reported.
#[test]
fn rds009_reports_a_confusable_character() {
    let f = findings_of(&designation_fx("\u{041A}", None).build(), "EW-RDS-009");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017009");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(targets(&f[0]), [(Kind::Device, uid(0x101))]);
    assert!(findings_of(&designation_fx("K", None).build(), "EW-RDS-009").is_empty());
}

/// EW-RDS-009 prevent: renaming a letter code to a look-alike is refused; touching a device whose look-alike the
/// model already holds, without a change, is not.
#[test]
fn rds009_prevent_refuses_a_new_confusable_only() {
    let before = designation_fx("K", None).build();
    let after = designation_fx("\u{041A}", None).build();
    let v = prevent(&Q::new(&before), &Q::new(&after), &[dev_key(0x101)]);
    assert_eq!(v.len(), 1, "{v:#?}");
    assert_eq!(v[0].rule, "EW-RDS-009");
    let v = prevent(&Q::new(&after), &Q::new(&after), &[dev_key(0x101)]);
    assert!(v.is_empty(), "{v:#?}");
}

/// Two location nodes with the codes `a` and `b`.
fn variants_fx(a: &str, b: &str) -> Fx {
    let mut fx = Fx::new();
    fx.push(json!({"$k": "structure_node", "id": us(0x901), "aspect": "location", "code": a}));
    fx.push(json!({"$k": "structure_node", "id": us(0x902), "aspect": "location", "code": b}));
    fx
}

/// EW-RDS-010: two codes of one aspect that differ only in separators and case are one warning naming both;
/// distinct codes are not.
#[test]
fn rds010_reports_spelling_variants_of_a_structure_code() {
    let f = findings_of(&variants_fx("J-E-2416", "je2416").build(), "EW-RDS-010");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017010");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "spelling");
    assert_eq!(
        targets(&f[0]),
        [
            (Kind::StructureNode, uid(0x901)),
            (Kind::StructureNode, uid(0x902))
        ]
    );
    assert!(findings_of(&variants_fx("J-E-2416", "J-E-206").build(), "EW-RDS-010").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// EW-NUM-002

/// Two drawn functions joined by wire 0x2001 with the number `number`.
fn wire_fx(number: Value) -> Fx {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &json!({"number": number}));
    fx
}

/// EW-NUM-002: a frozen wire number that the numbering rule would not give is an info on the connection; a wire
/// without a frozen number is not reported.
#[test]
fn num002_reports_a_frozen_wire_number_that_deviates() {
    let f = findings_of(&wire_fx(json!({"frozen": "W99"})).build(), "EW-NUM-002");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017102");
    assert_eq!(f[0].severity, Severity::Info);
    assert_eq!(f[0].discriminator, "wire-number");
    assert_eq!(f[0].subject, "wire W99");
    assert_eq!(targets(&f[0]), [(Kind::Connection, uid(0x2001))]);
    assert!(
        f[0].message
            .starts_with("wire number W99: frozen, deviates from the rule"),
        "{}",
        f[0].message
    );
    assert!(findings_of(&wire_fx(json!("unassigned")).build(), "EW-NUM-002").is_empty());
}

// ---------------------------------------------------------------------------------------------------------
// Terminals: EW-TERM-003, -004

const STRIP: u128 = 0x10;
const FNDEF_TERM: u128 = 0x530;
const SYM_TERM: u128 = 0x531;

/// Strip -X1 with terminals `0x200 + seq` (seq 1 to `count`), each with a terminal function `0x1200 + seq` (CP 0
/// on side 1, CP 1 on side 2) drawn on P1; terminal 1 is spare when `spare`.
fn strip_fx(count: u32, spare: bool) -> Fx {
    let mut fx = Fx::base();
    fx.fndef_with(
        FNDEF_TERM,
        SYM_TERM,
        &[
            (0, "1", json!({"side": "side1"})),
            (1, "2", json!({"side": "side2"})),
        ],
        &json!([]),
        &json!({}),
    );
    fx.push(json!({"$k": "device", "id": us(STRIP), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    for seq in 1..=count {
        let t = 0x200 + u128::from(seq);
        let mut d = json!({
            "$k": "device", "id": us(t), "tag": {"letter": "X"}, "kind": "terminal",
            "terminal": {"strip": us(STRIP), "seq": seq, "designation": seq.to_string()},
        });
        if spare && seq == 1 {
            d["state"] = json!("spare");
        }
        fx.push(d);
        let f = 0x1200 + u128::from(seq);
        fx.push(json!({"$k": "function", "id": us(f), "device": us(t), "def": us(FNDEF_TERM)}));
        fx.push(json!({
            "$k": "representation", "id": us(f + 0x10_000), "functions": [us(f)], "page": us(P1),
            "symbol": us(SYM_TERM), "pos": [i64::from(seq) * 8_000_000, 0],
        }));
    }
    fx
}

/// The spare terminal 1 of [`strip_fx`] wired on CP `idx` to function 0x1001.
fn spare_fx(idx: u16) -> Fx {
    let mut fx = strip_fx(2, true);
    fx.func(0x1001, P1);
    fx.conn(0x2001, P1, Ep::Cp(0x1201, idx), cp(0x1001), &Value::Null);
    fx
}

/// EW-TERM-003: a spare terminal wired on its field side (side 2 by default) is a warning; wired on side 1 it is not.
#[test]
fn term003_reports_a_spare_terminal_wired_on_its_field_side() {
    let f = findings_of(&spare_fx(1).build(), "EW-TERM-003");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "001003");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "field-side");
    assert_eq!(targets(&f[0]), [(Kind::Device, uid(0x201))]);
    assert!(findings_of(&spare_fx(0).build(), "EW-TERM-003").is_empty());
}

/// Strip -X1 with three terminals and bridges of group 1 between the pairs `pairs` (by seq) on shaft `shaft`.
fn bridges_fx(pairs: &[(u32, u32, u8)]) -> Fx {
    let mut fx = strip_fx(3, false);
    for (i, (a, b, shaft)) in pairs.iter().enumerate() {
        fx.push(json!({
            "$k": "connection", "id": us(0x2100 + i as u128),
            "a": {"cp": [us(0x1200 + u128::from(*a)), 0]}, "b": {"cp": [us(0x1200 + u128::from(*b)), 0]},
            "kind": {"bridge": {"kind": "plug_in", "group": 1, "shaft": shaft}},
        }));
    }
    fx
}

/// EW-TERM-004: a bridge group over terminals 1 and 3 (skipping 2) is an error naming the group; a group over
/// terminals 1, 2 and 3 is not; a group on two shafts is.
#[test]
fn term004_reports_an_invalid_bridge_group() {
    let f = findings_of(&bridges_fx(&[(1, 3, 1)]).build(), "EW-TERM-004");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "001004");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "group-1");
    assert_eq!(targets(&f[0]), [(Kind::Connection, uid(0x2100))]);
    assert!(
        f[0].message.contains("its terminals are not consecutive"),
        "{}",
        f[0].message
    );
    assert!(findings_of(&bridges_fx(&[(1, 2, 1), (2, 3, 1)]).build(), "EW-TERM-004").is_empty());
    let f = findings_of(&bridges_fx(&[(1, 2, 1), (2, 3, 2)]).build(), "EW-TERM-004");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(f[0].message.contains("it runs over 2 shafts"), "{}", f[0].message);
}

/// EW-TERM-004 prevent: a bridge that puts a group on a second shaft is refused; touching a group the model already
/// holds broken, without a change, is not.
#[test]
fn term004_prevent_refuses_a_new_invalid_group_only() {
    let before = bridges_fx(&[(1, 2, 1)]).build();
    let after = bridges_fx(&[(1, 2, 1), (2, 3, 2)]).build();
    let key = (Kind::Connection, uid(0x2101));
    let v = prevent(&Q::new(&before), &Q::new(&after), &[key]);
    assert_eq!(v.len(), 1, "{v:#?}");
    assert_eq!(v[0].rule, "EW-TERM-004");
    let v = prevent(&Q::new(&after), &Q::new(&after), &[key]);
    assert!(v.is_empty(), "{v:#?}");
}

// ---------------------------------------------------------------------------------------------------------
// ID permutation (WP-S1.16 §10)

/// One permutation case: the rule, its fixture and the ID pairs to swap.
type Case = (&'static str, Fx, Vec<(u128, u128)>);

/// The fixture with the record and library IDs of each pair swapped everywhere they occur.
fn permuted(fx: &Fx, pairs: &[(u128, u128)]) -> Model {
    let swap = |v: &Value| -> Value {
        let mut text = v.to_string();
        for (i, (a, b)) in pairs.iter().enumerate() {
            let tmp = format!("@swap{i}@");
            text = text
                .replace(&us(*a), &tmp)
                .replace(&us(*b), &us(*a))
                .replace(&tmp, &us(*b));
        }
        serde_json::from_str(&text).expect("JSON")
    };
    let mut out = fx.clone();
    out.records = fx.records.iter().map(swap).collect();
    out.lib = fx.lib.iter().map(swap).collect();
    out.build()
}

/// Every finding of a release and an online run of the `full` scheme as printed: rule, severity, subject, message,
/// in report order.
fn printed(m: &Model) -> Vec<String> {
    let q = Q::new(m);
    [CheckMode::Online, CheckMode::Release]
        .into_iter()
        .flat_map(|mode| run(&q, &FULL, mode).findings)
        .map(|f| format!("{} {} {}: {}", f.rule, f.severity, f.subject, f.message))
        .collect()
}

/// The ID-permutation test of every part-4 rule: swapping record IDs (pages, strips, terminals, connections,
/// structure nodes, page items) changes no printed finding and no report order (ID-677, WP-S1.16 §10, FT-594).
#[test]
fn part4_findings_do_not_follow_record_ids() {
    let mut two_texts = text_fx(Some("qet.9"));
    two_texts.push(json!({"$k": "page_item", "id": us(0x8601), "page": us(P1),
        "kind": {"free_text": {"text": {"en": "Other"}, "at": [0, 4_000_000], "style": "qet.7"}}}));
    let cases: Vec<Case> = vec![
        ("EW-CORE-021", folios_fx(250, 260), vec![(PA, PB)]),
        ("EW-CORE-022", folios_fx(101, 101), vec![(PA, PB)]),
        (
            "EW-CORE-026",
            coverage_fx(json!({"include": [us(0x8200)]})),
            vec![(0x8200, 0x8300), (0x8201, 0x8301)],
        ),
        ("EW-CORE-061", slots_fx(cp_slots(true)), vec![(0x1001, 0x1002)]),
        ("EW-CORE-063", two_texts, vec![(0x8600, 0x8601)]),
        ("EW-CORE-071", line_fx(0), vec![(0x8700, 0x1001)]),
        ("EW-CORE-008", channel_fx(false), vec![]),
        (
            "EW-RDS-002",
            variants_fx("J-E-2416", "J-E-206"),
            vec![(0x901, 0x902)],
        ),
        (
            "EW-RDS-010",
            variants_fx("J-E-2416", "je2416"),
            vec![(0x901, 0x902)],
        ),
        (
            "EW-NUM-002",
            wire_fx(json!({"frozen": "W99"})),
            vec![(0x1001, 0x1002)],
        ),
        ("EW-TERM-003", spare_fx(1), vec![(0x201, 0x202)]),
        (
            "EW-TERM-004",
            bridges_fx(&[(1, 3, 1)]),
            vec![(0x1201, 0x1203), (0x201, 0x203)],
        ),
    ];
    for (rule, fx, pairs) in cases {
        let base = printed(&fx.build());
        assert!(base.iter().any(|l| l.starts_with(rule)), "{rule}: {base:#?}");
        assert_eq!(printed(&permuted(&fx, &pairs)), base, "{rule}");
    }
}
