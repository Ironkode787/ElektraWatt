// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Behavioural negative tests of the strict reader, written against `serde_json` only: malformed known
//! variants, non-variant shapes, repeated member names, unsorted or repeated sets and keyed lists, and
//! unknown members that must survive a round trip. Each line is valid except for the one defect under test.

mod common;

use common::fill;
use common::lines::{DOC05_PART, GOLDEN, GOLDEN_LIB};
use ew_schema::{LibDocument, Record};

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

fn rejected(line: &str) {
    let err = serde_json::from_str::<Record>(line).err();
    assert!(err.is_some(), "accepted: {line}");
}

fn lib_rejected(doc: &str) {
    let err = serde_json::from_str::<LibDocument>(doc).err();
    assert!(err.is_some(), "accepted: {doc}");
}

/// A connection whose `a` endpoint is `a` (a template).
fn conn_a(a: &str) -> String {
    fill(&format!(
        r#"{{"$k":"connection","id":"@CON_W7","a":{a},"b":{{"cp":["@FN_F1",0]}}}}"#
    ))
}

/// A connection of the kind `kind`.
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

/// The controls: the base lines of the cases below are themselves valid.
#[test]
fn control_lines_are_accepted() {
    for line in [
        conn_a(r#"{"cp":["@FN_S51",0]}"#),
        conn_kind(r#"{"bridge":{"kind":"plug_in","group":1,"shaft":1}}"#),
        conductor(r#""csa":{"awg":18}"#),
        rep(&format!("[{PIN_1},{PIN_2}]"), &format!("[{TS_1},{TS_2}]")),
        report_block(r#"{"include":["@DEV_X1031","@DEV_G1"]}"#),
        device_tech(r#"[{"key":"curve","value":"C"},{"key":"i_rated","value":"C"}]"#),
    ] {
        serde_json::from_str::<Record>(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Finding 2: malformed known variants

#[test]
fn malformed_endpoint_cp() {
    for a in [
        r#"{"cp":["D40B6E93-25F7-5C1A-9E8D-7A3F1C05B6E2",0]}"#,
        r#"{"cp":["d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2",70000]}"#,
        r#"{"cp":["d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2"]}"#,
    ] {
        rejected(&conn_a(a));
    }
}

#[test]
fn malformed_slot() {
    let page = golden(r#""id":"@P1""#);
    rejected(&sub(
        &page,
        &fill("@LOC_JE2416"),
        "A41F6C2D-8E05-5D97-B3C1-5F2E8A7D9C60",
    ));
}

#[test]
fn malformed_tech_val() {
    rejected(&device_tech(
        r#"[{"key":"i_out","value":{"value":"20.0","unit":"A"}}]"#,
    ));
}

#[test]
fn malformed_function_kind() {
    rejected(&fill(
        r#"{"$k":"function","id":"@FN_IP24","def":"@FNDEF_IP","kind":{"ip":{"name":" bad","direction":"source"}}}"#,
    ));
}

#[test]
fn malformed_connection_kind() {
    rejected(&conn_kind(r#"{"core":5}"#));
}

#[test]
fn unit_variant_with_a_payload() {
    rejected(&sub(
        &golden(r#""id":"@JN_J1""#),
        r#""style":"dot""#,
        r#""style":{"dot":{}}"#,
    ));
}

#[test]
fn malformed_variant_nested_in_another_variant() {
    rejected(&conn_kind(
        r#"{"bridge":{"kind":{"plug_in":1},"group":1,"shaft":1}}"#,
    ));
}

// ---------------------------------------------------------------------------------------------------------
// Non-variant shapes where an enum is expected

#[test]
fn non_variant_shapes() {
    rejected(&conn_kind("5"));
    rejected(&conn_kind(r#"["wire"]"#));
    rejected(&conn_a(r#"{"cp":["@FN_S51",0],"junction":"@JN_J1"}"#));
}

// ---------------------------------------------------------------------------------------------------------
// Finding 3: repeated member names

#[test]
fn duplicate_member_at_the_top_level() {
    rejected(&fill(
        r#"{"$k":"section","id":"@SEC1","number":1,"number":2,"folio_range":[100,199]}"#,
    ));
}

#[test]
fn duplicate_member_in_a_nested_object() {
    rejected(&sub(
        &golden(r#""id":"@P1""#),
        r#""grid":{"num":7937500,"den":3}"#,
        r#""grid":{"num":7937500,"den":3,"num":7937500}"#,
    ));
}

#[test]
fn duplicate_member_in_ml_text() {
    rejected(&sub(
        &golden(r#""id":"@P1""#),
        r#""title":{"en-GB":"Title Page"}"#,
        r#""title":{"en-GB":"a","en-GB":"b"}"#,
    ));
}

#[test]
fn duplicate_member_in_an_array_element() {
    rejected(&fill(
        r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"G","counter":1},"parts":[{"part":{"copy":"@PART_PSU"},"qty":"1","qty":"2"}]}"#,
    ));
}

#[test]
fn duplicate_member_in_a_library_document() {
    lib_rejected(&sub(
        &fill(DOC05_PART),
        r#"{"idx":0,"designation":"L"}"#,
        r#"{"idx":0,"idx":0,"designation":"L"}"#,
    ));
}

// ---------------------------------------------------------------------------------------------------------
// Finding 4: unsorted and repeated sets and keyed lists

#[test]
fn structure_node_aliases() {
    let node = |aliases: &str| {
        fill(&format!(
            r#"{{"$k":"structure_node","id":"@LOC_JE2416","aspect":"location","code":"J-E-2416","aliases":{aliases}}}"#
        ))
    };
    serde_json::from_str::<Record>(&node(r#"["J-E-2416","JE2416"]"#)).unwrap();
    rejected(&node(r#"["JE2416","J-E-2416"]"#));
    rejected(&node(r#"["JE2416","JE2416"]"#));
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
    serde_json::from_str::<Record>(&project(r#"["de-DE","nl-NL"]"#)).unwrap();
    rejected(&project(r#"["nl-NL","de-DE"]"#));
    rejected(&project(r#"["nl-NL","nl-NL"]"#));
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
    serde_json::from_str::<LibDocument>(&symbol(r#"["a","b"]"#)).unwrap();
    lib_rejected(&symbol(r#"["b","a"]"#));
    lib_rejected(&symbol(r#"["a","a"]"#));
}

#[test]
fn function_cp_overrides() {
    let function = |o: &str| {
        fill(&format!(
            r#"{{"$k":"function","id":"@FN_F2_102","device":"@DEV_G1","def":"@FNDEF_MCB","role":"aux","cp_overrides":{o}}}"#
        ))
    };
    serde_json::from_str::<Record>(&function(r#"[{"idx":0,"alias":"A"},{"idx":3,"alias":"B"}]"#)).unwrap();
    rejected(&function(r#"[{"idx":3,"alias":"B"},{"idx":0,"alias":"A"}]"#));
    rejected(&function(r#"[{"idx":3,"alias":"A"},{"idx":3,"alias":"B"}]"#));
}

#[test]
fn device_tech_data() {
    rejected(&device_tech(
        r#"[{"key":"i_rated","value":"C"},{"key":"curve","value":"C"}]"#,
    ));
    rejected(&device_tech(
        r#"[{"key":"curve","value":"C"},{"key":"curve","value":"D"}]"#,
    ));
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
    lib_rejected(&with_tech(r#"[{"key":"b","value":"C"},{"key":"a","value":"C"}]"#));
    lib_rejected(&with_tech(r#"[{"key":"a","value":"C"},{"key":"a","value":"D"}]"#));
    lib_rejected(&with_pattern(
        r#"[{"idx":1,"designation":"2"},{"idx":0,"designation":"1"}]"#,
    ));
    lib_rejected(&with_pattern(
        r#"[{"idx":1,"designation":"2"},{"idx":1,"designation":"1"}]"#,
    ));
    lib_rejected(&with_props(
        r#"[{"key":"b","value":"x"},{"key":"a","value":"x"}]"#,
    ));
    lib_rejected(&with_props(
        r#"[{"key":"a","value":"x"},{"key":"a","value":"y"}]"#,
    ));
}

#[test]
fn symbol_cps_and_fndef_cps() {
    let symbol = golden_lib(r#""$k":"symbol""#);
    let sym_cps = r#""cps":[{"idx":0,"fn_cp":0,"pos":[0,0],"dir":"up"},{"idx":1,"fn_cp":1,"pos":[0,8000000],"dir":"down"}]"#;
    lib_rejected(&sub(
        &symbol,
        sym_cps,
        r#""cps":[{"idx":1,"fn_cp":1,"pos":[0,8000000],"dir":"down"},{"idx":0,"fn_cp":0,"pos":[0,0],"dir":"up"}]"#,
    ));
    lib_rejected(&sub(
        &symbol,
        sym_cps,
        r#""cps":[{"idx":0,"fn_cp":0,"pos":[0,0],"dir":"up"},{"idx":0,"fn_cp":1,"pos":[0,8000000],"dir":"down"}]"#,
    ));
    let fndef = golden_lib(r#""$k":"fndef""#);
    let pair = r#"{"idx":0,"designation":"1"},{"idx":1,"designation":"2"}"#;
    lib_rejected(&sub(
        &fndef,
        pair,
        r#"{"idx":1,"designation":"2"},{"idx":0,"designation":"1"}"#,
    ));
    lib_rejected(&sub(
        &fndef,
        pair,
        r#"{"idx":0,"designation":"1"},{"idx":0,"designation":"2"}"#,
    ));
}

#[test]
fn representation_pin_map_and_text_slots() {
    let pins = format!("[{PIN_1},{PIN_2}]");
    let slots = format!("[{TS_1},{TS_2}]");
    rejected(&rep(&format!("[{PIN_2},{PIN_1}]"), &slots));
    rejected(&rep(&format!("[{PIN_1},{PIN_1}]"), &slots));
    rejected(&rep(&pins, &format!("[{TS_2},{TS_1}]")));
    rejected(&rep(&pins, &format!("[{TS_1},{TS_1}]")));
}

#[test]
fn report_block_manual_selection() {
    for member in ["include", "exclude"] {
        let manual = |ids: &str| report_block(&format!(r#"{{"{member}":{ids}}}"#));
        serde_json::from_str::<Record>(&manual(r#"["@DEV_X1031","@DEV_G1"]"#)).unwrap();
        rejected(&manual(r#"["@DEV_G1","@DEV_X1031"]"#));
        rejected(&manual(r#"["@DEV_G1","@DEV_G1"]"#));
    }
}

// ---------------------------------------------------------------------------------------------------------
// Finding 1: unknown members must not be lost

fn assert_kept(line: &str, library: bool) {
    let written = if library {
        let doc: LibDocument = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
        serde_json::to_string(&doc).unwrap()
    } else {
        let record: Record = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {line}"));
        serde_json::to_string(&record).unwrap()
    };
    assert_eq!(written, line, "unknown member lost or moved");
}

#[test]
fn unknown_member_in_assigned_number() {
    assert_kept(
        &sub(
            &golden(r#""id":"@CON_W20""#),
            r#"{"assigned":{"counter":1}}"#,
            r#"{"assigned":{"counter":1,"since":"x"}}"#,
        ),
        false,
    );
}

#[test]
fn unknown_member_in_bevel() {
    assert_kept(
        &sub(
            &golden(r#""id":"@JN_J5""#),
            r#"{"bevel":{"orient":"r90"}}"#,
            r#"{"bevel":{"orient":"r90","x":1}}"#,
        ),
        false,
    );
}

#[test]
fn unknown_member_in_lineage_copy() {
    let line = fill(
        r#"{"$k":"import_lineage","id":"@LIN","importer":"csv","source_name":"tables","anchor":{"copy":{"of":"@PRJ","n":2,"x":1}},"namespace":"@PRJ","last_source_sha256":"@HASH"}"#,
    );
    assert_kept(&line, false);
}

#[test]
fn unknown_member_in_item_status() {
    assert_kept(
        &sub(
            &golden_lib(r#""id":"@PART_NEBU""#),
            r#"{"deprecated":{}}"#,
            r#"{"deprecated":{"x":1}}"#,
        ),
        true,
    );
}

#[test]
fn unknown_member_in_internal_kind() {
    assert_kept(
        &sub(
            &golden_lib(r#""$k":"fndef""#),
            r#"{"pass_through":{"opens":true}}"#,
            r#"{"pass_through":{"opens":true,"x":1}}"#,
        ),
        true,
    );
}

#[test]
fn unknown_member_in_overflow() {
    assert_kept(
        &sub(
            &golden_lib(r#""$k":"form""#),
            r#"{"shrink_to_fit":{"min_cap_height":1200000}}"#,
            r#"{"shrink_to_fit":{"min_cap_height":1200000,"x":1}}"#,
        ),
        true,
    );
}

// ---------------------------------------------------------------------------------------------------------
// Rules that already held before the strict reader

#[test]
fn already_rejected_before() {
    // a decimal with a trailing zero
    rejected(&conductor(r#""csa":"2.50""#));
    // an AWG size outside i8 and a foreign colour code that is not an identifier (the payload types
    // already refused them; only the variant and path of the error are new)
    rejected(&conductor(r#""csa":{"awg":200}"#));
    rejected(&conductor(r#""colour":[{"foreign":" BR"}]"#));
    // floats, in a typed integer and in an unknown member
    rejected(&fill(
        r#"{"$k":"section","id":"@SEC1","number":1.0,"folio_range":[100,199]}"#,
    ));
    rejected(&fill(
        r#"{"$k":"section","id":"@SEC1","number":1,"folio_range":[100,199],"x":0.5}"#,
    ));
    // a library document where a record is expected
    rejected(&fill(GOLDEN_LIB[0]));
}
