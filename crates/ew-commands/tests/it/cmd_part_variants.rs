// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `part.assign` with a variant and with accessory choices (doc 07 §3.3 rules 7 and 8): a variant the part does not
//! hold is refused in words; the variant is written into the part use's reference; replacing a part drops the choices
//! of accessories the new part does not name.

use crate::common;

use common::*;
use ew_commands::CmdError;
use ew_commands::commands::{AssignModeParam, AssignPart};
use ew_ids::Id;
use ew_model::Model;
use ew_model::fixture::mini_project_parts;
use ew_schema::{Device, LibDocument, Record};
use serde_json::json;

const LIB: u128 = 0x7700;
const T_A: u128 = 1; // names the end plate EP per strip, variant BU
const T_B: u128 = 2; // names the end plate EP2 per strip
const EP: u128 = 3;
const EP2: u128 = 4;
const DEVICE: u128 = 0x5e40_0000_0000_4000_8000_0000_0000_0001;

/// A part of `item` naming `accessory` per strip, with `variants`.
fn part(item: u128, number: &str, accessory: Option<u128>, variants: serde_json::Value) -> LibDocument {
    let mut c = lib_part(LIB, item, number);
    if let Some(a) = accessory {
        c.item.accessories = serde_json::from_value(json!([{
            "part": {"library": uuid(LIB).to_string(), "item": uuid(a).to_string(), "version_req": "^1"},
            "qty": "1", "per": "strip", "kind": "end_plate",
        }]))
        .unwrap();
    }
    c.item.variants = serde_json::from_value(variants).unwrap();
    LibDocument::Part(c)
}

/// A strip device using T_A whose end plate EP is left off.
fn engine_with_choice() -> ew_commands::Engine {
    let (p, mut records, mut lib) = mini_project_parts();
    lib.extend([
        part(
            T_A,
            "T-A",
            Some(EP),
            json!([{"key": "BU", "name": {"en": "Blue"}, "part_number": "T-A-BU"}]),
        ),
        part(T_B, "T-B", Some(EP2), json!([])),
        part(EP, "EP", None, json!([])),
        part(EP2, "EP2", None, json!([])),
    ]);
    let device: Device = serde_json::from_value(json!({
        "id": uuid(DEVICE).to_string(), "tag": {"letter": "X", "counter": 9}, "strip": {},
        "parts": [{"part": {"copy": lib_part_id(LIB, T_A).to_string()}, "qty": "1"}],
        "accessory_choices": [{"of": lib_part_id(LIB, EP).to_string()}],
    }))
    .unwrap();
    records.push(Record::Device(device));
    engine(Model::from_records(p, records, lib).expect("valid model"))
}

fn assign(item: u128, mode: AssignModeParam, from: Option<u128>, variant: Option<&str>) -> AssignPart {
    AssignPart {
        devices: vec![Id::from_uuid(uuid(DEVICE))],
        part: lib_part_id(LIB, item),
        copies: vec![],
        mode,
        from: from.map(|f| lib_part_id(LIB, f)),
        qty: None,
        variant: variant.map(|v| v.parse().unwrap()),
    }
}

fn device(e: &ew_commands::Engine) -> Device {
    Device::clone(&row(e, Id::<Device>::from_uuid(uuid(DEVICE))))
}

/// A variant the part does not hold is refused in words at `/variant`; one it holds is written into the reference,
/// and the choice of an accessory the part still names stays.
#[test]
fn a_variant_is_written_and_an_unknown_one_refused_in_words() {
    let mut e = engine_with_choice();
    let rev = e.model().rev();
    match e.execute(&assign(T_A, AssignModeParam::ReplaceAll, None, Some("RD"))) {
        Err(CmdError::InvalidParams { path, text, .. }) => {
            assert_eq!(path, "/variant");
            assert_eq!(text.to_string(), "T-A has no variant RD");
        }
        other => panic!("refused: {other:?}"),
    }
    assert_eq!(e.model().rev(), rev);
    e.execute(&assign(T_A, AssignModeParam::ReplaceAll, None, Some("BU")))
        .unwrap();
    let d = device(&e);
    assert_eq!(d.parts[0].part.variant.as_ref().map(|v| v.as_str()), Some("BU"));
    assert_eq!(d.accessory_choices.len(), 1, "T-A still names the end plate");
}

/// Replacing T-A by T-B, which names another end plate, drops the choice that left T-A's end plate off.
#[test]
fn replacing_a_part_drops_the_choice_of_an_accessory_it_no_longer_names() {
    let mut e = engine_with_choice();
    e.execute(&assign(T_B, AssignModeParam::Replace, Some(T_A), None))
        .unwrap();
    let d = device(&e);
    assert_eq!(d.parts[0].part.copy, lib_part_id(LIB, T_B));
    assert!(d.accessory_choices.is_empty(), "{:?}", d.accessory_choices);
}
