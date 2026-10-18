// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The structured errors of the strict reader (variant, path, key), the loader's findings, `parse_value`,
//! the write-side order of sets and keyed lists, NFC normalisation and the smaller rules of the reader
//! rework (lineage namespaces, company colour codes, `validate`, appended members).

mod common;

use common::fill;
use common::lines::{DOC05_LINES, DOC05_PART, GOLDEN, GOLDEN_LIB};
use ew_ids::v5::{LineageAnchor as IdAnchor, lineage_namespace};
use ew_mltext::Ident;
use ew_schema::{
    ColourCode, Extra, KeyedList, LibDocument, LineageAnchor, MalformedVariant, Open, OrderUnit, Overflow,
    Record, SchemaError, Set, TechVal, TechValue, normalize_nfc, parse_value,
};
use serde_json::{Value, json};

/// `base` with the one occurrence of `from` replaced by `to`.
fn sub(base: &str, from: &str, to: &str) -> String {
    assert_eq!(base.matches(from).count(), 1, "`{from}` is not unique in {base}");
    base.replace(from, to)
}

/// The filled golden line whose template contains `needle`.
fn golden(needle: &str) -> String {
    let mut found = GOLDEN.iter().filter(|t| t.contains(needle));
    let template = found
        .next()
        .unwrap_or_else(|| panic!("no golden line with {needle}"));
    assert!(found.next().is_none(), "{needle} is not unique");
    fill(template)
}

/// The filled golden library document whose template contains `needle`.
fn golden_lib(needle: &str) -> String {
    let mut found = GOLDEN_LIB.iter().filter(|t| t.contains(needle));
    let template = found
        .next()
        .unwrap_or_else(|| panic!("no golden document with {needle}"));
    assert!(found.next().is_none(), "{needle} is not unique");
    fill(template)
}

fn record_err(line: &str) -> SchemaError {
    match Record::from_json(line) {
        Ok(_) => panic!("accepted: {line}"),
        Err(e) => e,
    }
}

fn lib_err(doc: &str) -> SchemaError {
    match LibDocument::from_json(doc) {
        Ok(_) => panic!("accepted: {doc}"),
        Err(e) => e,
    }
}

fn expect_malformed(line: &str, path: &str, variant: &str) {
    match record_err(line) {
        SchemaError::MalformedVariant(m) => {
            assert_eq!((m.path.as_str(), m.variant.as_str()), (path, variant), "{line}");
        }
        other => panic!("expected a malformed variant at {path}, got {other:?}: {line}"),
    }
    // The loader keeps the value verbatim and reports exactly this finding.
    let value = parse_value(line).unwrap();
    let (record, findings) =
        Record::load(value.clone()).unwrap_or_else(|e| panic!("load failed: {e}: {line}"));
    assert_eq!(findings.len(), 1, "{findings:?}: {line}");
    assert_eq!(
        (findings[0].path.as_str(), findings[0].variant.as_str()),
        (path, variant)
    );
    assert_eq!(
        serde_json::to_value(&record).unwrap(),
        value,
        "value not kept verbatim: {line}"
    );
}

fn expect_invalid(line: &str, path: &str) {
    match record_err(line) {
        SchemaError::Invalid { path: p, .. } => assert_eq!(p, path, "{line}"),
        other => panic!("expected Invalid at {path}, got {other:?}: {line}"),
    }
    let value = parse_value(line).unwrap();
    match Record::load(value) {
        Err(SchemaError::Invalid { path: p, .. }) => assert_eq!(p, path, "{line}"),
        other => panic!("the loader must fail with Invalid at {path}, got {other:?}: {line}"),
    }
}

fn expect_duplicate_member(err: SchemaError, path: &str, key: &str) {
    assert_eq!(
        err,
        SchemaError::DuplicateMember {
            path: path.into(),
            key: key.into()
        }
    );
}

fn expect_unsorted(err: SchemaError, path: &str) {
    assert_eq!(err, SchemaError::UnsortedList { path: path.into() });
}

fn expect_duplicate_key(err: SchemaError, path: &str, key: &str) {
    assert_eq!(
        err,
        SchemaError::DuplicateKey {
            path: path.into(),
            key: key.into()
        }
    );
}

fn conn_a(a: &str) -> String {
    fill(&format!(
        r#"{{"$k":"connection","id":"@CON_W7","a":{a},"b":{{"cp":["@FN_F1",0]}}}}"#
    ))
}

fn conn_kind(kind: &str) -> String {
    fill(&format!(
        r#"{{"$k":"connection","id":"@CON_W7","a":{{"cp":["@FN_S51",0]}},"b":{{"cp":["@FN_F1",0]}},"kind":{kind}}}"#
    ))
}

fn conductor(member: &str) -> String {
    fill(&format!(
        r#"{{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"X",{member}}}"#
    ))
}

fn rep(pin_map: &str, text_slots: &str) -> String {
    fill(&format!(
        r#"{{"$k":"representation","id":"@REP_X1031A","functions":["@FN_X1031"],"page":"@P103","symbol":"@SYM_MCB","pos":[1,2],"pin_map":{pin_map},"text_slots":{text_slots}}}"#
    ))
}

const PIN_1: &str = r#"{"symbol_cp":0,"function":"@FN_X1031","idx":1}"#;
const PIN_2: &str = r#"{"symbol_cp":1,"function":"@FN_X1031","idx":2}"#;
const TS_1: &str = r#"{"slot":0,"pos":[0,0],"visible":true}"#;
const TS_2: &str = r#"{"slot":1,"pos":[0,0],"visible":true}"#;

fn report_block(manual: &str) -> String {
    fill(&format!(
        r#"{{"$k":"report_block","id":"@RB_903","source":{{"kind":"terminal_diagram"}},"manual":{manual},"form":"@FORM_TOC","first_folio":903}}"#
    ))
}

fn device_tech(tech: &str) -> String {
    fill(&format!(
        r#"{{"$k":"device","id":"@DEV_G1","tag":{{"letter":"G","counter":1}},"tech_data":{tech}}}"#
    ))
}

// ---------------------------------------------------------------------------------------------------------
// Rule 1: malformed known variants

#[test]
fn malformed_variants_are_reported_at_the_enum_value() {
    let cp = "d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2";
    let page = golden(r#""id":"@P1""#);
    let cases: Vec<(String, &str, &str)> = vec![
        (
            conn_a(r#"{"cp":["D40B6E93-25F7-5C1A-9E8D-7A3F1C05B6E2",0]}"#),
            "/a",
            "cp",
        ),
        (conn_a(&format!(r#"{{"cp":["{cp}",70000]}}"#)), "/a", "cp"),
        (conn_a(&format!(r#"{{"cp":["{cp}"]}}"#)), "/a", "cp"),
        (
            sub(
                &page,
                &fill("@LOC_JE2416"),
                "A41F6C2D-8E05-5D97-B3C1-5F2E8A7D9C60",
            ),
            "/structure/location",
            "node",
        ),
        (
            device_tech(r#"[{"key":"i_out","value":{"value":"20.0","unit":"A"}}]"#),
            "/tech_data/0/value",
            "quantity",
        ),
        (
            fill(
                r#"{"$k":"function","id":"@FN_IP24","def":"@FNDEF_IP","kind":{"ip":{"name":" bad","direction":"source"}}}"#,
            ),
            "/kind",
            "ip",
        ),
        (conn_kind(r#"{"core":5}"#), "/kind", "core"),
        (
            sub(
                &golden(r#""id":"@JN_J1""#),
                r#""style":"dot""#,
                r#""style":{"dot":{}}"#,
            ),
            "/style",
            "dot",
        ),
        (conductor(r#""csa":{"awg":200}"#), "/csa", "awg"),
        (
            conductor(r#""colour":[{"foreign":" BR"}]"#),
            "/colour/0",
            "foreign",
        ),
        (
            conn_kind(r#"{"bridge":{"kind":{"plug_in":1},"group":1,"shaft":1}}"#),
            "/kind/bridge/kind",
            "plug_in",
        ),
    ];
    for (line, path, variant) in &cases {
        expect_malformed(line, path, variant);
    }
}

#[test]
fn malformed_variants_fail_every_strict_reader() {
    let line = conn_kind(r#"{"core":5}"#);
    let expected = SchemaError::MalformedVariant(MalformedVariant {
        path: "/kind".into(),
        variant: "core".into(),
        message: String::new(),
    });
    for err in [
        Record::from_json(&line).unwrap_err(),
        Record::from_value(parse_value(&line).unwrap()).unwrap_err(),
    ] {
        let (SchemaError::MalformedVariant(got), SchemaError::MalformedVariant(want)) = (&err, &expected)
        else {
            panic!("{err:?}");
        };
        assert_eq!((&got.path, &got.variant), (&want.path, &want.variant));
        assert!(!got.message.is_empty());
    }
    let serde_err = serde_json::from_str::<Record>(&line).unwrap_err().to_string();
    assert!(
        serde_err.contains("core") && serde_err.contains("/kind"),
        "{serde_err}"
    );
}

#[test]
fn the_loader_reports_findings_in_document_order_and_keeps_good_values() {
    // Two malformed variants in one record: both are kept verbatim and both reported.
    let line = fill(
        r#"{"$k":"connection","id":"@CON_W7","a":{"cp":["@FN_S51"]},"b":{"cp":["@FN_F1",0]},"kind":{"core":5}}"#,
    );
    let value = parse_value(&line).unwrap();
    let (record, findings) = Record::load(value.clone()).unwrap();
    let got: Vec<(&str, &str)> = findings
        .iter()
        .map(|m| (m.path.as_str(), m.variant.as_str()))
        .collect();
    assert_eq!(got, [("/a", "cp"), ("/kind", "core")]);
    assert_eq!(serde_json::to_value(&record).unwrap(), value);
}

// ---------------------------------------------------------------------------------------------------------
// Rule 2: non-variant shapes

#[test]
fn non_variant_shapes_are_invalid() {
    expect_invalid(&conn_kind("5"), "/kind");
    expect_invalid(&conn_kind(r#"["wire"]"#), "/kind");
    expect_invalid(&conn_a(r#"{"cp":["@FN_S51",0],"junction":"@JN_J1"}"#), "/a");
}

// ---------------------------------------------------------------------------------------------------------
// Rule 3: repeated member names

#[test]
fn duplicate_members_carry_the_path_of_the_object() {
    expect_duplicate_member(
        record_err(&fill(
            r#"{"$k":"section","id":"@SEC1","number":1,"number":2,"folio_range":[100,199]}"#,
        )),
        "",
        "number",
    );
    let page = golden(r#""id":"@P1""#);
    expect_duplicate_member(
        record_err(&sub(
            &page,
            r#""grid":{"num":7937500,"den":3}"#,
            r#""grid":{"num":7937500,"den":3,"num":7937500}"#,
        )),
        "/grid",
        "num",
    );
    expect_duplicate_member(
        record_err(&sub(
            &page,
            r#""title":{"en-GB":"Title Page"}"#,
            r#""title":{"en-GB":"a","en-GB":"b"}"#,
        )),
        "/title",
        "en-GB",
    );
    expect_duplicate_member(
        record_err(&fill(
            r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"G","counter":1},"parts":[{"part":{"copy":"@PART_PSU"},"qty":"1","qty":"2"}]}"#,
        )),
        "/parts/0",
        "qty",
    );
    expect_duplicate_member(
        lib_err(&sub(
            &fill(DOC05_PART),
            r#"{"idx":0,"designation":"L"}"#,
            r#"{"idx":0,"idx":0,"designation":"L"}"#,
        )),
        "/cp_pattern/0",
        "idx",
    );
}

#[test]
fn serde_json_reports_the_duplicate_member() {
    let line = fill(r#"{"$k":"section","id":"@SEC1","number":1,"number":2,"folio_range":[100,199]}"#);
    let msg = serde_json::from_str::<Record>(&line).unwrap_err().to_string();
    assert!(msg.contains("duplicate member \"number\""), "{msg}");
}

#[test]
fn parse_value_rejects_duplicates_and_bad_text() {
    assert_eq!(
        parse_value(r#"{"a":[1,{"b":null}],"c":"d"}"#).unwrap(),
        json!({"a":[1,{"b":null}],"c":"d"})
    );
    expect_duplicate_member(parse_value(r#"{"a":1,"a":2}"#).unwrap_err(), "", "a");
    expect_duplicate_member(parse_value(r#"{"x":{"k":1,"k":2}}"#).unwrap_err(), "/x", "k");
    expect_duplicate_member(
        parse_value(r#"{"x":[0,{"k":1,"k":2}]}"#).unwrap_err(),
        "/x/1",
        "k",
    );
    // `/` and `~` in member names are escaped in the pointer
    expect_duplicate_member(
        parse_value(r#"{"a/b~":{"k":1,"k":2}}"#).unwrap_err(),
        "/a~1b~0",
        "k",
    );
    for bad in ["{", "", r#"{"a":1} x"#, "{} {}", "[1,]"] {
        assert!(
            matches!(parse_value(bad), Err(SchemaError::Json { .. })),
            "{bad:?}"
        );
    }
    let SchemaError::Json { message } = parse_value("{} x").unwrap_err() else {
        panic!("a Json error");
    };
    assert!(message.contains("trailing"), "{message}");
}

#[test]
fn library_documents_report_duplicates_too() {
    let doc = fill(r#"{"$k":"symbol","$k":"symbol"}"#);
    expect_duplicate_member(LibDocument::from_json(&doc).unwrap_err(), "", "$k");
}

// ---------------------------------------------------------------------------------------------------------
// Rule 4: unsorted and repeated sets and keyed lists

#[test]
fn structure_node_aliases() {
    let node = |aliases: &str| {
        fill(&format!(
            r#"{{"$k":"structure_node","id":"@LOC_JE2416","aspect":"location","code":"J-E-2416","aliases":{aliases}}}"#
        ))
    };
    expect_unsorted(record_err(&node(r#"["JE2416","J-E-2416"]"#)), "/aliases/1");
    expect_duplicate_key(
        record_err(&node(r#"["JE2416","JE2416"]"#)),
        "/aliases/1",
        r#""JE2416""#,
    );
}

#[test]
fn project_translations() {
    let project = |t: &str| {
        sub(
            &golden(r#""$k":"project""#),
            r#""languages":{"source":"en-GB"}"#,
            &format!(r#""languages":{{"source":"en-GB","translations":{t}}}"#),
        )
    };
    expect_unsorted(
        record_err(&project(r#"["nl-NL","de-DE"]"#)),
        "/languages/translations/1",
    );
    expect_duplicate_key(
        record_err(&project(r#"["nl-NL","nl-NL"]"#)),
        "/languages/translations/1",
        r#""nl-NL""#,
    );
}

#[test]
fn symbol_keywords() {
    let symbol = |k: &str| {
        sub(
            &golden_lib(r#""$k":"symbol""#),
            r#""anchor":{"slot":"tag"}"#,
            &format!(r#""keywords":{k},"anchor":{{"slot":"tag"}}"#),
        )
    };
    expect_unsorted(lib_err(&symbol(r#"["b","a"]"#)), "/keywords/1");
    expect_duplicate_key(lib_err(&symbol(r#"["a","a"]"#)), "/keywords/1", r#""a""#);
}

#[test]
fn function_cp_overrides() {
    let function = |o: &str| {
        fill(&format!(
            r#"{{"$k":"function","id":"@FN_F2_102","device":"@DEV_G1","def":"@FNDEF_MCB","role":"aux","cp_overrides":{o}}}"#
        ))
    };
    expect_unsorted(
        record_err(&function(r#"[{"idx":3,"alias":"B"},{"idx":0,"alias":"A"}]"#)),
        "/cp_overrides/1",
    );
    expect_duplicate_key(
        record_err(&function(r#"[{"idx":3,"alias":"A"},{"idx":3,"alias":"B"}]"#)),
        "/cp_overrides/1",
        "3",
    );
}

#[test]
fn device_tech_data() {
    expect_unsorted(
        record_err(&device_tech(
            r#"[{"key":"i_rated","value":"C"},{"key":"curve","value":"C"}]"#,
        )),
        "/tech_data/1",
    );
    expect_duplicate_key(
        record_err(&device_tech(
            r#"[{"key":"curve","value":"C"},{"key":"curve","value":"D"}]"#,
        )),
        "/tech_data/1",
        "curve",
    );
}

#[test]
fn part_tech_data_cp_pattern_and_free_props() {
    let nebu = golden_lib(r#""id":"@PART_NEBU""#);
    let tech = r#"[{"key":"length","value":{"value":"2","unit":"m"}}]"#;
    let props = r#"[{"key":"connector","value":"7/8 inch"}]"#;
    let with_tech = |t: &str| sub(&nebu, tech, t);
    let with_pattern = |p: &str| {
        sub(
            &nebu,
            r#""unit":"m","function_template""#,
            &format!(r#""cp_pattern":{p},"unit":"m","function_template""#),
        )
    };
    let with_props = |p: &str| sub(&nebu, props, p);
    expect_unsorted(
        lib_err(&with_tech(r#"[{"key":"b","value":"C"},{"key":"a","value":"C"}]"#)),
        "/tech_data/1",
    );
    expect_duplicate_key(
        lib_err(&with_tech(r#"[{"key":"a","value":"C"},{"key":"a","value":"D"}]"#)),
        "/tech_data/1",
        "a",
    );
    expect_unsorted(
        lib_err(&with_pattern(
            r#"[{"idx":1,"designation":"2"},{"idx":0,"designation":"1"}]"#,
        )),
        "/cp_pattern/1",
    );
    expect_duplicate_key(
        lib_err(&with_pattern(
            r#"[{"idx":1,"designation":"2"},{"idx":1,"designation":"1"}]"#,
        )),
        "/cp_pattern/1",
        "1",
    );
    expect_unsorted(
        lib_err(&with_props(
            r#"[{"key":"b","value":"x"},{"key":"a","value":"x"}]"#,
        )),
        "/free_props/1",
    );
    expect_duplicate_key(
        lib_err(&with_props(
            r#"[{"key":"a","value":"x"},{"key":"a","value":"y"}]"#,
        )),
        "/free_props/1",
        "a",
    );
}

#[test]
fn symbol_cps_and_fndef_cps() {
    let symbol = golden_lib(r#""$k":"symbol""#);
    let sym_cps = r#""cps":[{"idx":0,"fn_cp":0,"pos":[0,0],"dir":"up"},{"idx":1,"fn_cp":1,"pos":[0,8000000],"dir":"down"}]"#;
    expect_unsorted(
        lib_err(&sub(
            &symbol,
            sym_cps,
            r#""cps":[{"idx":1,"fn_cp":1,"pos":[0,8000000],"dir":"down"},{"idx":0,"fn_cp":0,"pos":[0,0],"dir":"up"}]"#,
        )),
        "/cps/1",
    );
    expect_duplicate_key(
        lib_err(&sub(
            &symbol,
            sym_cps,
            r#""cps":[{"idx":0,"fn_cp":0,"pos":[0,0],"dir":"up"},{"idx":0,"fn_cp":1,"pos":[0,8000000],"dir":"down"}]"#,
        )),
        "/cps/1",
        "0",
    );
    let fndef = golden_lib(r#""$k":"fndef""#);
    let pair = r#"{"idx":0,"designation":"1"},{"idx":1,"designation":"2"}"#;
    expect_unsorted(
        lib_err(&sub(
            &fndef,
            pair,
            r#"{"idx":1,"designation":"2"},{"idx":0,"designation":"1"}"#,
        )),
        "/cps/1",
    );
    expect_duplicate_key(
        lib_err(&sub(
            &fndef,
            pair,
            r#"{"idx":0,"designation":"1"},{"idx":0,"designation":"2"}"#,
        )),
        "/cps/1",
        "0",
    );
}

#[test]
fn representation_pin_map_and_text_slots() {
    let pins = format!("[{PIN_1},{PIN_2}]");
    let slots = format!("[{TS_1},{TS_2}]");
    expect_unsorted(
        record_err(&rep(&format!("[{PIN_2},{PIN_1}]"), &slots)),
        "/pin_map/1",
    );
    expect_duplicate_key(
        record_err(&rep(&format!("[{PIN_1},{PIN_1}]"), &slots)),
        "/pin_map/1",
        "0",
    );
    expect_unsorted(
        record_err(&rep(&pins, &format!("[{TS_2},{TS_1}]"))),
        "/text_slots/1",
    );
    expect_duplicate_key(
        record_err(&rep(&pins, &format!("[{TS_1},{TS_1}]"))),
        "/text_slots/1",
        "0",
    );
}

#[test]
fn report_block_manual_selection() {
    let g1 = format!("\"{}\"", fill("@DEV_G1"));
    for member in ["include", "exclude"] {
        // `report_block` fills the tokens
        let manual = |ids: &str| report_block(&format!(r#"{{"{member}":{ids}}}"#));
        expect_unsorted(
            record_err(&manual(r#"["@DEV_G1","@DEV_X1031"]"#)),
            &format!("/manual/{member}/1"),
        );
        expect_duplicate_key(
            record_err(&manual(&format!("[{g1},{g1}]"))),
            &format!("/manual/{member}/1"),
            &g1,
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// Write-side order of sets and keyed lists

fn ident(s: &str) -> Ident {
    s.parse().unwrap()
}

#[test]
fn set_sorts_and_deduplicates_by_json_text() {
    let from_iter: Set<Ident> = ["JE2416", "J-E-2416", "JE2416"].into_iter().map(ident).collect();
    assert_eq!(
        serde_json::to_string(&from_iter).unwrap(),
        r#"["J-E-2416","JE2416"]"#
    );

    // `!` is below `"`, so the longer identifier sorts first.
    let pair: Set<Ident> = Set::from(vec![ident("ab"), ident("ab!")]);
    assert_eq!(pair.as_slice(), [ident("ab!"), ident("ab")]);
    assert_eq!(serde_json::to_string(&pair).unwrap(), r#"["ab!","ab"]"#);
    assert!(serde_json::from_str::<Set<Ident>>(r#"["ab!","ab"]"#).is_ok());
    assert!(serde_json::from_str::<Set<Ident>>(r#"["ab","ab!"]"#).is_err());

    // numbers sort as text: "10" before "9"
    let numbers = Set::from(vec![9_u32, 10, 9]);
    assert_eq!(serde_json::to_string(&numbers).unwrap(), "[10,9]");
    assert_eq!(numbers.into_vec(), [10, 9]);
}

#[test]
fn set_insert_remove_contains() {
    let mut set: Set<Ident> = Set::new();
    assert!(set.is_empty());
    assert!(set.insert(ident("b")));
    assert!(set.insert(ident("a")));
    assert!(!set.insert(ident("b")));
    assert!(set.insert(ident("ab")));
    assert_eq!(serde_json::to_string(&set).unwrap(), r#"["a","ab","b"]"#);
    assert!(set.contains(&ident("ab")));
    assert!(!set.contains(&ident("abc")));
    assert!(set.remove(&ident("ab")));
    assert!(!set.remove(&ident("ab")));
    assert_eq!(set.len(), 2);
    assert_eq!(set.iter().map(|i| i.as_str()).collect::<Vec<_>>(), ["a", "b"]);
}

fn tech(key: &str, value: &str) -> TechValue {
    TechValue {
        key: ident(key),
        value: TechVal::Ident(ident(value)),
        extra: Extra::new(),
    }
}

#[test]
fn keyed_list_sorts_rejects_duplicates_and_replaces() {
    let list = KeyedList::from_vec(vec![tech("i_rated", "C"), tech("curve", "C")]).unwrap();
    let keys: Vec<&str> = list.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys, ["curve", "i_rated"]);

    let err = KeyedList::from_vec(vec![tech("b", "C"), tech("a", "C"), tech("b", "D")]).unwrap_err();
    assert_eq!(
        err,
        SchemaError::DuplicateKey {
            path: "/2".into(),
            key: "b".into()
        }
    );

    let mut list = list;
    assert_eq!(
        list.get(&ident("curve")).map(|t| &t.value),
        Some(&TechVal::Ident(ident("C")))
    );
    assert!(list.get(&ident("missing")).is_none());
    let replaced = list
        .insert(tech("curve", "D"))
        .expect("the old element comes back");
    assert_eq!(replaced.value, TechVal::Ident(ident("C")));
    assert_eq!(list.len(), 2);
    assert!(list.insert(tech("a", "C")).is_none());
    assert_eq!(list[0].key.as_str(), "a");
    assert!(list.remove(&ident("a")).is_some());
    assert!(list.remove(&ident("a")).is_none());
    assert_eq!(list.len(), 2);
    assert!(!list.is_empty());
    assert_eq!(list.into_vec().len(), 2);
}

#[test]
fn serialising_a_struct_with_sets_and_keyed_lists_is_sorted() {
    #[derive(serde::Serialize)]
    struct Holder {
        aliases: Set<Ident>,
        tech: KeyedList<TechValue>,
    }
    let mut tech_list = KeyedList::new();
    tech_list.insert(tech("z", "C"));
    tech_list.insert(tech("a", "D"));
    let holder = Holder {
        aliases: Set::from(vec![ident("ab"), ident("ab!"), ident("a")]),
        tech: tech_list,
    };
    assert_eq!(
        serde_json::to_string(&holder).unwrap(),
        r#"{"aliases":["a","ab!","ab"],"tech":[{"key":"a","value":"D"},{"key":"z","value":"C"}]}"#
    );
}

// ---------------------------------------------------------------------------------------------------------
// NFC normalisation

const DECOMPOSED: &str = "Lu\u{0308}fter";
const COMPOSED: &str = "L\u{00fc}fter";

#[test]
fn normalize_nfc_covers_strings_keys_and_depth() {
    let mut s = Value::String(DECOMPOSED.into());
    assert!(normalize_nfc(&mut s));
    assert_eq!(s, json!(COMPOSED));
    assert!(!normalize_nfc(&mut s));

    let mut key = Value::Object(serde_json::Map::from_iter([(DECOMPOSED.to_owned(), json!(1))]));
    assert!(normalize_nfc(&mut key));
    assert_eq!(key, json!({ COMPOSED: 1 }));
    assert!(!normalize_nfc(&mut key));

    let mut deep = json!({"a": [1, {"b": [[DECOMPOSED]], "c": {DECOMPOSED: [DECOMPOSED, null, true]}}]});
    assert!(normalize_nfc(&mut deep));
    assert_eq!(
        deep,
        json!({"a": [1, {"b": [[COMPOSED]], "c": {COMPOSED: [COMPOSED, null, true]}}]})
    );
    assert!(!normalize_nfc(&mut deep));

    let mut plain = json!({"a": ["b", 1]});
    assert!(!normalize_nfc(&mut plain));
}

#[test]
fn normalize_nfc_keeps_both_members_on_a_key_collision() {
    let text = format!(r#"{{"{DECOMPOSED}":1,"{COMPOSED}":2}}"#);
    let mut value = parse_value(&text).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 2);
    // nothing could be changed: the decomposed name stays, the object keeps both members
    assert!(!normalize_nfc(&mut value));
    let map = value.as_object().unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(map[DECOMPOSED], json!(1));
    assert_eq!(map[COMPOSED], json!(2));
}

#[test]
fn a_non_nfc_title_fails_until_normalised() {
    let line = sub(
        &golden(r#""id":"@P1""#),
        r#""title":{"en-GB":"Title Page"}"#,
        &format!(r#""title":{{"en-GB":"{DECOMPOSED}"}}"#),
    );
    let mut value = parse_value(&line).unwrap();
    assert!(Record::from_value(value.clone()).is_err());
    assert!(normalize_nfc(&mut value));
    let record = Record::from_value(value).unwrap();
    let written = serde_json::to_string(&record).unwrap();
    assert!(
        written.contains(&format!(r#""title":{{"en-GB":"{COMPOSED}"}}"#)),
        "{written}"
    );
}

// ---------------------------------------------------------------------------------------------------------
// Import lineage namespaces

#[test]
fn lineage_copy_of_is_the_namespace_of_the_original() {
    let original = lineage_namespace("csv", &IdAnchor::Fixed("golden")).unwrap();
    let copy_ns = lineage_namespace("csv", &IdAnchor::Copy { of: original, n: 2 }).unwrap();
    let line = fill(&format!(
        r#"{{"$k":"import_lineage","id":"@LIN","importer":"csv","source_name":"tables","anchor":{{"copy":{{"of":"{original}","n":2}}}},"namespace":"{copy_ns}"}}"#
    ));
    let record = Record::from_json(&line).unwrap();
    assert_eq!(serde_json::to_string(&record).unwrap(), line);
    let Record::ImportLineage(lineage) = record else {
        panic!("a lineage");
    };
    let LineageAnchor::Copy(copy) = &lineage.anchor else {
        panic!("a copy anchor");
    };
    assert_eq!(copy.of.as_uuid(), original);
    assert_eq!(copy.n, 2);
    // the typed value plugs straight into the derivation of the copy's namespace
    let derived = lineage_namespace(
        lineage.importer.as_str(),
        &IdAnchor::Copy {
            of: copy.of.as_uuid(),
            n: copy.n,
        },
    )
    .unwrap();
    assert_eq!(derived, lineage.namespace.as_uuid());
}

// ---------------------------------------------------------------------------------------------------------
// Company colour codes and `validate`

#[test]
fn company_colour_codes_may_not_shadow_iec_codes() {
    assert!(matches!(
        ColourCode::company(ident("BK")),
        Err(SchemaError::CompanyCodeIsIec(code)) if code == "BK"
    ));
    assert_eq!(
        ColourCode::company(ident("DBU")),
        Ok(ColourCode::Company(ident("DBU")))
    );
    assert!(serde_json::to_string(&ColourCode::Company(ident("BK"))).is_err());
    assert_eq!(
        serde_json::to_string(&ColourCode::Company(ident("DBU"))).unwrap(),
        r#""DBU""#
    );
}

#[test]
fn validate_finds_extra_members_that_shadow_known_ones() {
    let Record::Section(mut section) = Record::from_json(&golden(r#""$k":"section""#)).unwrap() else {
        panic!("a section");
    };
    assert_eq!(Record::Section(section.clone()).validate(), Ok(()));
    section.extra.insert("number".into(), json!(5));
    assert_eq!(
        Record::Section(section).validate(),
        Err(SchemaError::DuplicateMember {
            path: String::new(),
            key: "number".into()
        })
    );

    let Record::Page(mut page) = Record::from_json(&golden(r#""id":"@P1""#)).unwrap() else {
        panic!("a page");
    };
    page.folio.extra.insert("number".into(), json!(5));
    assert_eq!(
        Record::Page(page).validate(),
        Err(SchemaError::DuplicateMember {
            path: "/folio".into(),
            key: "number".into()
        })
    );
}

#[test]
fn validate_accepts_every_golden_example() {
    for line in DOC05_LINES
        .iter()
        .map(|l| l.to_string())
        .chain(GOLDEN.iter().map(|t| fill(t)))
    {
        Record::from_json(&line)
            .unwrap()
            .validate()
            .unwrap_or_else(|e| panic!("{e}: {line}"));
    }
    for doc in GOLDEN_LIB.iter().chain([&DOC05_PART]).map(|t| fill(t)) {
        LibDocument::from_json(&doc)
            .unwrap()
            .validate()
            .unwrap_or_else(|e| panic!("{e}: {doc}"));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Doc additions

fn assert_canonical(line: &str) -> Record {
    let record = Record::from_json(line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(serde_json::to_string(&record).unwrap(), line);
    record
}

fn assert_lib_canonical(doc: &str) -> LibDocument {
    let parsed = LibDocument::from_json(doc).unwrap_or_else(|e| panic!("{e}: {doc}"));
    assert_eq!(serde_json::to_string(&parsed).unwrap(), doc);
    parsed
}

#[test]
fn an_embedded_report_block_needs_no_first_folio() {
    let line = sub(&golden(r#""id":"@RB_PD201""#), r#""first_folio":201,"#, "");
    let Record::ReportBlock(block) = assert_canonical(&line) else {
        panic!("a report block");
    };
    assert_eq!(block.first_folio, None);
    assert!(block.embedded.is_some());
}

#[test]
fn project_master_data_has_a_document_title() {
    let line = sub(
        &golden(r#""$k":"project""#),
        r#""edited":"2025-04-14""#,
        r#""edited":"2025-04-14","document_title":{"en-GB":"Cabinet drawings"}"#,
    );
    let Record::Project(project) = assert_canonical(&line) else {
        panic!("a project");
    };
    assert!(!project.master.document_title.is_empty());
    assert!(project.master.extra.is_empty());
}

#[test]
fn order_unit_metre_is_written_m() {
    let nebu = golden_lib(r#""id":"@PART_NEBU""#);
    let LibDocument::Part(part) = assert_lib_canonical(&nebu) else {
        panic!("a part");
    };
    assert_eq!(part.item.unit, OrderUnit::Metre);

    let legacy = sub(
        &nebu,
        r#""unit":"m","function_template""#,
        r#""unit":"metre","function_template""#,
    );
    let LibDocument::Part(part) = assert_lib_canonical(&legacy) else {
        panic!("a part");
    };
    assert_eq!(part.item.unit, OrderUnit::Unknown(json!("metre")));
}

#[test]
fn a_form_cell_can_report_overflow_as_an_error() {
    let form = sub(
        &golden_lib(r#""$k":"form""#),
        r#""overflow":{"shrink_to_fit":{"min_cap_height":1200000}}"#,
        r#""overflow":"error""#,
    );
    let LibDocument::Form(copy) = assert_lib_canonical(&form) else {
        panic!("a form");
    };
    assert_eq!(copy.item.cells[0].overflow, Overflow::Error);
}

#[test]
fn open_aspect_keeps_unknown_aspects_and_rejects_other_shapes() {
    let node = |aspect: &str| {
        fill(&format!(
            r#"{{"$k":"structure_node","id":"@FN_P01","aspect":{aspect},"code":"Z1"}}"#
        ))
    };
    let Record::StructureNode(zone) = assert_canonical(&node(r#""zone""#)) else {
        panic!("a node");
    };
    assert_eq!(zone.aspect, Open::Unknown(json!("zone")));
    let Record::StructureNode(known) = assert_canonical(&node(r#""function""#)) else {
        panic!("a node");
    };
    assert!(known.aspect.known().is_some());
    expect_invalid(&node("7"), "/aspect");
}
