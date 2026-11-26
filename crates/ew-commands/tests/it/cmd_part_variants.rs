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
const FER: u128 = 5; // a ferrule
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
    let LibDocument::Part(mut ferrule) = part(FER, "FER", None, json!([])) else {
        unreachable!()
    };
    ferrule.item.kind = ew_schema::PartKind::Ferrule;
    lib.push(LibDocument::Part(ferrule));
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

/// The fork of a library copy may change what the part brings with it and its variants (doc 07 §2.5); an accessory
/// that is a ferrule and a variant without a number of its own are refused at their rows.
#[test]
fn a_fork_changes_accessories_and_variants_and_refuses_a_ferrule_or_a_variant_without_a_number() {
    use ew_commands::commands::EditPartCopy;
    let mut e = engine_with_choice();
    let item = |accessory: u128, variants: serde_json::Value| {
        let LibDocument::Part(c) = part(T_A, "T-A", Some(accessory), variants) else {
            unreachable!()
        };
        serde_json::to_value(&c.item).unwrap()
    };
    let edit = |part: serde_json::Value, fork: u128| EditPartCopy {
        copy: lib_part_id(LIB, T_A),
        part,
        fork: Some(Id::from_uuid(uuid(fork))),
        copies: vec![],
    };
    // EP2 instead of EP, and a red variant with its own number: the fork takes them.
    let red = json!([{"key": "RD", "name": {"en": "Red"}, "part_number": "T-A-RD"}]);
    let p = e.preview(&edit(item(EP2, red.clone()), 0x0f01)).expect("the fork's preview");
    e.commit(p).expect("the fork");
    // A ferrule is no accessory: the consumables count it.
    match e.preview(&edit(item(FER, red), 0x0f02)) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/part/accessories/0"),
        other => panic!("refused: {other:?}"),
    }
    // A variant without a number of its own is refused at its row.
    let bare = json!([{"key": "RD", "name": {"en": "Red"}}]);
    match e.preview(&edit(item(EP, bare), 0x0f03)) {
        Err(CmdError::InvalidParams { path, .. }) => assert_eq!(path, "/part/variants/0"),
        other => panic!("refused: {other:?}"),
    }
}
