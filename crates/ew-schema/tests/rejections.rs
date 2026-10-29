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

/// One malformed variant per case; each line must be refused.
#[test]
fn malformed_variants() {
    let page = golden(r#""id":"@P1""#);
    let cases: Vec<(&str, Vec<String>)> = vec![
        (
            "malformed_endpoint_cp",
            [
                r#"{"cp":["D40B6E93-25F7-5C1A-9E8D-7A3F1C05B6E2",0]}"#,
                r#"{"cp":["d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2",70000]}"#,
                r#"{"cp":["d40b6e93-25f7-5c1a-9e8d-7a3f1c05b6e2"]}"#,
            ]
            .into_iter()
            .map(conn_a)
            .collect(),
        ),
        (
            "malformed_slot",
            vec![sub(
                &page,
                &fill("@LOC_JE2416"),
                "A41F6C2D-8E05-5D97-B3C1-5F2E8A7D9C60",
            )],
        ),
        (
            "malformed_tech_val",
            vec![device_tech(
                r#"[{"key":"i_out","value":{"value":"20.0","unit":"A"}}]"#,
            )],
        ),
        (
            "malformed_function_kind",
            vec![fill(
                r#"{"$k":"function","id":"@FN_IP24","def":"@FNDEF_IP","kind":{"ip":{"name":" bad","direction":"source"}}}"#,
            )],
        ),
        ("malformed_connection_kind", vec![conn_kind(r#"{"core":5}"#)]),
        (
            "malformed_variant_nested_in_another_variant",
            vec![conn_kind(
                r#"{"bridge":{"kind":{"plug_in":1},"group":1,"shaft":1}}"#,
            )],
        ),
    ];
    for (name, lines) in &cases {
        for line in lines {
            let err = serde_json::from_str::<Record>(line).err();
            assert!(err.is_some(), "case {name}: accepted: {line}");
        }
    }
}

#[test]
fn unit_variant_with_a_payload() {
    rejected(&sub(
        &golden(r#""id":"@JN_J1""#),
        r#""style":"dot""#,
        r#""style":{"dot":{}}"#,
    ));
}

// ---------------------------------------------------------------------------------------------------------
// Finding 3: repeated member names

/// A repeated member name at every depth, in a record and in a library document; each must be refused.
#[test]
fn duplicate_members() {
    let page = golden(r#""id":"@P1""#);
    let records = [
        (
            "duplicate_member_at_the_top_level",
            fill(r#"{"$k":"section","id":"@SEC1","number":1,"number":2,"folio_range":[100,199]}"#),
        ),
        (
            "duplicate_member_in_a_nested_object",
            sub(
                &page,
                r#""grid":{"num":7937500,"den":3}"#,
                r#""grid":{"num":7937500,"den":3,"num":7937500}"#,
            ),
        ),
        (
            "duplicate_member_in_ml_text",
            sub(
                &page,
                r#""title":{"en-GB":"Title Page"}"#,
                r#""title":{"en-GB":"a","en-GB":"b"}"#,
            ),
        ),
        (
            "duplicate_member_in_an_array_element",
            fill(
                r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"G","counter":1},"parts":[{"part":{"copy":"@PART_PSU"},"qty":"1","qty":"2"}]}"#,
            ),
        ),
    ];
    for (name, line) in &records {
        let err = serde_json::from_str::<Record>(line).err();
        assert!(err.is_some(), "case {name}: accepted: {line}");
    }
    let name = "duplicate_member_in_a_library_document";
    let doc = sub(
        &fill(DOC05_PART),
        r#"{"idx":0,"designation":"L"}"#,
        r#"{"idx":0,"idx":0,"designation":"L"}"#,
    );
    let err = serde_json::from_str::<LibDocument>(&doc).err();
    assert!(err.is_some(), "case {name}: accepted: {doc}");
}

// ---------------------------------------------------------------------------------------------------------
// Finding 1: unknown members must not be lost

/// An unknown member inside a known nested value survives a read and a write, in records and library
/// documents.
#[test]
fn unknown_member_in() {
    let cases: [(&str, String, bool); 6] = [
        (
            "assigned_number",
            sub(
                &golden(r#""id":"@CON_W20""#),
                r#"{"assigned":{"counter":1}}"#,
                r#"{"assigned":{"counter":1,"since":"x"}}"#,
            ),
            false,
        ),
        (
            "bevel",
            sub(
                &golden(r#""id":"@JN_J5""#),
                r#"{"bevel":{"orient":"r90"}}"#,
                r#"{"bevel":{"orient":"r90","x":1}}"#,
            ),
            false,
        ),
        (
            "lineage_copy",
            fill(
                r#"{"$k":"import_lineage","id":"@LIN","importer":"csv","source_name":"tables","anchor":{"copy":{"of":"@PRJ","n":2,"x":1}},"namespace":"@PRJ","last_source_sha256":"@HASH"}"#,
            ),
            false,
        ),
        (
            "item_status",
            sub(
                &golden_lib(r#""id":"@PART_NEBU""#),
                r#"{"deprecated":{}}"#,
                r#"{"deprecated":{"x":1}}"#,
            ),
            true,
        ),
        (
            "internal_kind",
            sub(
                &golden_lib(r#""$k":"fndef""#),
                r#"{"pass_through":{"opens":true}}"#,
                r#"{"pass_through":{"opens":true,"x":1}}"#,
            ),
            true,
        ),
        (
            "overflow",
            sub(
                &golden_lib(r#""$k":"form""#),
                r#"{"shrink_to_fit":{"min_cap_height":1200000}}"#,
                r#"{"shrink_to_fit":{"min_cap_height":1200000,"x":1}}"#,
            ),
            true,
        ),
    ];
    for (name, line, library) in &cases {
        let written = if *library {
            let doc: LibDocument =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("case {name}: {e}: {line}"));
            serde_json::to_string(&doc).unwrap()
        } else {
            let record: Record =
                serde_json::from_str(line).unwrap_or_else(|e| panic!("case {name}: {e}: {line}"));
            serde_json::to_string(&record).unwrap()
        };
        assert_eq!(&written, line, "case {name}: unknown member lost or moved");
    }
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
